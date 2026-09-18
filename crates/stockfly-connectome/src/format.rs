use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EdgeBlockManifest {
    pub index: u32,
    pub edge_start: u64,
    pub edge_count: u64,
    pub src_sha256: String,
    pub weight_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignPolicy {
    pub excitatory_transmitters: Vec<String>,
    pub inhibitory_transmitters: Vec<String>,
    pub unresolved_default: String,
    pub source_column: String,
    pub derived_from: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompiledManifest {
    pub format_version: u32,
    pub dataset: String,
    pub neuron_count: u32,
    pub edge_count: u64,
    pub neurons_sha256: String,
    pub offsets_sha256: String,
    pub edge_blocks: Vec<EdgeBlockManifest>,
    pub sign_policy: SignPolicy,
}
