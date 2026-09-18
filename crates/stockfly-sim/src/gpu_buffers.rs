use std::fmt;

use stockfly_connectome::Connectome;

/// A conservative ceiling that is legal on baseline WebGPU adapters and
/// keeps every bound edge buffer within the project's explicit limit.
pub const MAX_EDGE_BUFFER_BYTES: u64 = 64 * 1024 * 1024;
const EDGE_STRIDE_BYTES: u64 = 8;

#[derive(Debug)]
pub(crate) struct EdgeChunk {
    pub dst_start: u32,
    pub dst_count: u32,
    pub edge_start: usize,
    pub edge_end: usize,
    pub local_offsets: Vec<u32>,
    pub packed_edges: Vec<u8>,
}

impl EdgeChunk {
    pub fn bound_edge_bytes(&self) -> u64 {
        self.packed_edges.len() as u64
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkError {
    InvalidGraph(String),
    WeightCount {
        expected: usize,
        actual: usize,
    },
    FanInExceedsLimit {
        destination: usize,
        edges: u64,
        max_edges: u64,
    },
}

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGraph(message) => write!(f, "invalid connectome: {message}"),
            Self::WeightCount { expected, actual } => {
                write!(f, "weight count {actual} does not match edge count {expected}")
            }
            Self::FanInExceedsLimit {
                destination,
                edges,
                max_edges,
            } => write!(
                f,
                "destination {destination} has {edges} incoming edges, exceeding the {max_edges}-edge GPU chunk limit"
            ),
        }
    }
}

impl std::error::Error for ChunkError {}

/// Partitions destination-major edges into whole-destination chunks. Keeping
/// each destination in exactly one chunk means the gather reduction has the
/// same edge order as the CPU reference and never needs floating-point
/// atomics. A graph with a single destination larger than one legal binding is
/// rejected explicitly so callers can use the CPU fallback without dropping
/// any edges.
pub(crate) fn build_edge_chunks(
    connectome: &Connectome,
    weights: &[f32],
    max_edge_buffer_bytes: u64,
) -> Result<Vec<EdgeChunk>, ChunkError> {
    if weights.len() != connectome.edge_src.len() {
        return Err(ChunkError::WeightCount {
            expected: connectome.edge_src.len(),
            actual: weights.len(),
        });
    }
    if connectome.offsets.len() != connectome.neurons.len() + 1 {
        return Err(ChunkError::InvalidGraph(format!(
            "offset count {} does not equal neuron count + 1 ({})",
            connectome.offsets.len(),
            connectome.neurons.len() + 1
        )));
    }
    if max_edge_buffer_bytes < EDGE_STRIDE_BYTES {
        return Err(ChunkError::InvalidGraph(
            "edge-buffer limit is smaller than one packed edge".to_string(),
        ));
    }

    let max_edges = max_edge_buffer_bytes / EDGE_STRIDE_BYTES;
    let mut chunks = Vec::new();
    let mut dst_start = 0usize;

    while dst_start < connectome.neurons.len() {
        let first_edge = connectome.offsets[dst_start];
        let mut dst_end = dst_start;

        while dst_end < connectome.neurons.len() {
            let destination_edges = connectome.offsets[dst_end + 1]
                .checked_sub(connectome.offsets[dst_end])
                .ok_or_else(|| ChunkError::InvalidGraph("offsets are not monotonic".to_string()))?;
            if destination_edges > max_edges {
                return Err(ChunkError::FanInExceedsLimit {
                    destination: dst_end,
                    edges: destination_edges,
                    max_edges,
                });
            }

            let candidate_edges = connectome.offsets[dst_end + 1]
                .checked_sub(first_edge)
                .ok_or_else(|| ChunkError::InvalidGraph("offsets are not monotonic".to_string()))?;
            if candidate_edges > max_edges && dst_end > dst_start {
                break;
            }
            dst_end += 1;
            if candidate_edges == max_edges {
                break;
            }
        }

        let edge_start = usize::try_from(first_edge)
            .map_err(|_| ChunkError::InvalidGraph("edge offset does not fit usize".to_string()))?;
        let edge_end = usize::try_from(connectome.offsets[dst_end])
            .map_err(|_| ChunkError::InvalidGraph("edge offset does not fit usize".to_string()))?;
        if edge_end > connectome.edge_src.len() || edge_start > edge_end {
            return Err(ChunkError::InvalidGraph(
                "chunk edge range is outside edge arrays".to_string(),
            ));
        }

        let mut local_offsets = Vec::with_capacity(dst_end - dst_start + 1);
        for &offset in &connectome.offsets[dst_start..=dst_end] {
            let local = offset
                .checked_sub(first_edge)
                .ok_or_else(|| ChunkError::InvalidGraph("offsets are not monotonic".to_string()))?;
            local_offsets.push(u32::try_from(local).map_err(|_| {
                ChunkError::InvalidGraph("local chunk offset exceeds WGSL u32 range".to_string())
            })?);
        }

        let mut packed_edges =
            Vec::with_capacity((edge_end - edge_start) * EDGE_STRIDE_BYTES as usize);
        for edge_index in edge_start..edge_end {
            packed_edges.extend_from_slice(&connectome.edge_src[edge_index].to_le_bytes());
            packed_edges.extend_from_slice(&weights[edge_index].to_le_bytes());
        }
        debug_assert!(packed_edges.len() as u64 <= max_edge_buffer_bytes);

        chunks.push(EdgeChunk {
            dst_start: u32::try_from(dst_start).map_err(|_| {
                ChunkError::InvalidGraph("destination index exceeds u32".to_string())
            })?,
            dst_count: u32::try_from(dst_end - dst_start).map_err(|_| {
                ChunkError::InvalidGraph("destination count exceeds u32".to_string())
            })?,
            edge_start,
            edge_end,
            local_offsets,
            packed_edges,
        });
        dst_start = dst_end;
    }

    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use stockfly_connectome::format::{CompiledManifest, SignPolicy};
    use stockfly_connectome::{Connectome, NeuronRecord};

    use super::*;

    fn graph(offsets: Vec<u64>) -> Connectome {
        let edge_count = *offsets.last().unwrap() as usize;
        let neuron_count = offsets.len() - 1;
        Connectome {
            manifest: CompiledManifest {
                format_version: 1,
                dataset: "chunk-test".to_string(),
                neuron_count: neuron_count as u32,
                edge_count: edge_count as u64,
                neurons_sha256: "unused".to_string(),
                offsets_sha256: "unused".to_string(),
                edge_blocks: Vec::new(),
                sign_policy: SignPolicy {
                    excitatory_transmitters: Vec::new(),
                    inhibitory_transmitters: Vec::new(),
                    unresolved_default: "excitatory".to_string(),
                    source_column: "unused".to_string(),
                    derived_from: "unused".to_string(),
                },
            },
            neurons: (0..neuron_count)
                .map(|body_id| NeuronRecord {
                    body_id: body_id as u64,
                })
                .collect(),
            offsets,
            edge_src: vec![0; edge_count],
            edge_weight: vec![1.0; edge_count],
        }
    }

    #[test]
    fn chunks_cover_zero_degree_destinations_and_every_edge() {
        let graph = graph(vec![0, 0, 3, 3, 5]);
        let chunks = build_edge_chunks(&graph, &graph.edge_weight, 3 * EDGE_STRIDE_BYTES).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!((chunks[0].dst_start, chunks[0].dst_count), (0, 2));
        assert_eq!(chunks[0].local_offsets, vec![0, 0, 3]);
        assert_eq!((chunks[1].dst_start, chunks[1].dst_count), (2, 2));
        assert_eq!(chunks[1].local_offsets, vec![0, 0, 2]);
        assert_eq!(
            chunks
                .iter()
                .map(|c| c.edge_end - c.edge_start)
                .sum::<usize>(),
            5
        );
        assert!(chunks.iter().all(|c| c.bound_edge_bytes() <= 24));
    }

    #[test]
    fn oversized_single_destination_is_rejected_for_cpu_fallback() {
        let graph = graph(vec![0, 4]);
        assert!(matches!(
            build_edge_chunks(&graph, &graph.edge_weight, 3 * EDGE_STRIDE_BYTES),
            Err(ChunkError::FanInExceedsLimit { destination: 0, .. })
        ));
    }
}
