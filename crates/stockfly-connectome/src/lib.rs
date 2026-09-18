pub mod format;

use format::CompiledManifest;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub enum ConnectomeError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Corrupt(String),
}

impl fmt::Display for ConnectomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectomeError::Io(e) => write!(f, "io error: {e}"),
            ConnectomeError::Json(e) => write!(f, "manifest parse error: {e}"),
            ConnectomeError::Corrupt(msg) => write!(f, "corrupt compiled graph: {msg}"),
        }
    }
}

impl std::error::Error for ConnectomeError {}
impl From<std::io::Error> for ConnectomeError {
    fn from(e: std::io::Error) -> Self {
        ConnectomeError::Io(e)
    }
}
impl From<serde_json::Error> for ConnectomeError {
    fn from(e: serde_json::Error) -> Self {
        ConnectomeError::Json(e)
    }
}

pub type Result<T> = std::result::Result<T, ConnectomeError>;

/// A single retained MaleCNS neuron, keyed by its stable original body ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeuronRecord {
    pub body_id: u64,
}

/// The full compiled connectome graph, destination-major (CSC).
///
/// `offsets[dst]..offsets[dst + 1]` indexes into `edge_src`/`edge_weight`
/// for neuron `dst`'s incoming edges. `edge_weight` already has the
/// immutable biological connection sign baked into its value; per-model
/// (Bio/Max) trainable deltas are layered on top at load time by the
/// trainer, never mutating this compiled base graph.
pub struct Connectome {
    pub manifest: CompiledManifest,
    pub neurons: Vec<NeuronRecord>,
    pub offsets: Vec<u64>,
    pub edge_src: Vec<u32>,
    pub edge_weight: Vec<f32>,
}

pub struct ValidationReport {
    pub neuron_count: usize,
    pub edge_count: usize,
    pub offsets_monotonic: bool,
    pub all_src_indices_in_range: bool,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.offsets_monotonic && self.all_src_indices_in_range
    }
}

fn read_u64_le_vec(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_u32_le_vec(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn read_f32_le_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

impl Connectome {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let dir = path.as_ref();

        let manifest_bytes = fs::read(dir.join("manifest.json"))?;
        let manifest: CompiledManifest = serde_json::from_slice(&manifest_bytes)?;

        let neurons_bytes = fs::read(dir.join("neurons.bin"))?;
        let neurons: Vec<NeuronRecord> = read_u64_le_vec(&neurons_bytes)
            .into_iter()
            .map(|body_id| NeuronRecord { body_id })
            .collect();

        let offsets_bytes = fs::read(dir.join("offsets.bin"))?;
        let offsets = read_u64_le_vec(&offsets_bytes);

        let mut edge_src = Vec::with_capacity(manifest.edge_count as usize);
        let mut edge_weight = Vec::with_capacity(manifest.edge_count as usize);

        let mut sorted_blocks = manifest.edge_blocks.clone();
        sorted_blocks.sort_by_key(|b| b.index);
        for block in &sorted_blocks {
            let src_path = dir
                .join("edge_src_blocks")
                .join(format!("{:04}.bin", block.index));
            let weight_path = dir
                .join("edge_weight_blocks")
                .join(format!("{:04}.bin", block.index));
            edge_src.extend(read_u32_le_vec(&fs::read(src_path)?));
            edge_weight.extend(read_f32_le_vec(&fs::read(weight_path)?));
        }

        Ok(Connectome {
            manifest,
            neurons,
            offsets,
            edge_src,
            edge_weight,
        })
    }

    pub fn validate(&self) -> Result<ValidationReport> {
        let neuron_count = self.neurons.len();
        let edge_count = self.edge_src.len();

        if self.offsets.len() != neuron_count + 1 {
            return Err(ConnectomeError::Corrupt(format!(
                "offsets length {} != neuron_count + 1 ({})",
                self.offsets.len(),
                neuron_count + 1
            )));
        }
        if self.edge_src.len() != self.edge_weight.len() {
            return Err(ConnectomeError::Corrupt(format!(
                "edge_src length {} != edge_weight length {}",
                self.edge_src.len(),
                self.edge_weight.len()
            )));
        }

        let offsets_monotonic = self.offsets.windows(2).all(|w| w[0] <= w[1]);
        let all_src_indices_in_range = self
            .edge_src
            .iter()
            .all(|&idx| (idx as usize) < neuron_count);

        let last_offset = *self.offsets.last().unwrap_or(&0);
        if last_offset as usize != edge_count {
            return Err(ConnectomeError::Corrupt(format!(
                "final offset {last_offset} != edge_count {edge_count}"
            )));
        }

        Ok(ValidationReport {
            neuron_count,
            edge_count,
            offsets_monotonic,
            all_src_indices_in_range,
        })
    }
}
