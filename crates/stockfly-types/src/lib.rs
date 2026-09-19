use serde::{Deserialize, Serialize};

/// Which trained Full variant a checkpoint or trace belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelKind {
    BioFull,
    MaxFull,
}

/// Stable original MaleCNS body ID for a neuron, preserved across compile,
/// pruning, and quantization so every retained element remains traceable
/// back to the source connectome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NeuronId(pub u64);

/// Dense, contiguous index assigned to a retained neuron at compile time.
/// Used for array/buffer indexing in the simulator and renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DenseNeuronIndex(pub u32);

/// Header fields every replayable move trace must carry, per the design
/// spec's "every move must be replayable" requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveTraceHeader {
    pub trace_version: u32,
    pub model_sha256: String,
    pub graph_sha256: String,
    pub fen: String,
}

/// Identity and validation metadata for a compiled connectome or trained
/// checkpoint artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub format_version: u32,
    pub model_kind: ModelKind,
    pub dataset: String,
    pub neuron_count: u32,
    pub edge_count: u64,
    pub graph_sha256: String,
    pub output_map_sha256: String,
}

#[cfg(test)]
impl ModelManifest {
    pub fn test_fixture() -> Self {
        ModelManifest {
            format_version: 1,
            model_kind: ModelKind::MaxFull,
            dataset: "malecns-v1".to_string(),
            neuron_count: 167_000,
            edge_count: 25_600_000,
            graph_sha256: "0".repeat(64),
            output_map_sha256: "1".repeat(64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_manifest_round_trips_json() {
        let m = ModelManifest::test_fixture();
        let json = serde_json::to_string(&m).unwrap();
        let restored: ModelManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m, restored);
    }

    #[test]
    fn model_kind_rejects_removed_lite_variant() {
        assert!(serde_json::from_str::<ModelKind>("\"Lite\"").is_err());
    }
}
