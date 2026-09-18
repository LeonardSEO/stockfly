use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// A trained StockFly checkpoint: the compiled base graph is never
/// mutated, so a checkpoint only needs to record which edges moved from
/// their base magnitude and by how much (sparse -- most edges never
/// receive a nonzero update, especially in Bio mode). JSON is used
/// deliberately for the weekend MVP: sizes stay in the tens of MB at
/// `smoke`/`quick` scale and JSON keeps the format trivially inspectable,
/// matching the project's transparency goals; a denser binary format can
/// replace this without changing the trainer once checkpoints grow large
/// enough to matter.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub format_version: u32,
    pub model_kind: String, // "bio-full" | "max-full"
    pub graph_neurons_sha256: String,
    pub sensory_map_sha256: String,
    pub output_map_sha256: String,
    pub preset: String,
    pub rng_seed: u64,
    pub trials_run: u64,
    pub teacher_top1_accuracy: f32,
    /// (edge_index, signed magnitude) for every edge whose trained value
    /// differs from the compiled base magnitude.
    pub deltas: Vec<(u32, f32)>,
}

impl Checkpoint {
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let json = serde_json::to_string(self).expect("checkpoint always serializes");
        fs::write(path, json)
    }

    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let json = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json).expect("checkpoint file should be valid JSON"))
    }

    /// Applies this checkpoint's deltas onto a base edge-weight array
    /// (e.g. a fresh `CpuSimulator::weights_mut()`), returning the number
    /// of edges actually touched.
    pub fn apply_to(&self, weights: &mut [f32]) -> usize {
        let mut touched = 0;
        for &(edge_index, magnitude) in &self.deltas {
            if let Some(slot) = weights.get_mut(edge_index as usize) {
                *slot = magnitude;
                touched += 1;
            }
        }
        touched
    }

    pub fn from_dense(
        model_kind: &str,
        graph_neurons_sha256: &str,
        sensory_map_sha256: &str,
        output_map_sha256: &str,
        preset: &str,
        rng_seed: u64,
        trials_run: u64,
        teacher_top1_accuracy: f32,
        base_weights: &[f32],
        trained_weights: &[f32],
    ) -> Self {
        let mut deltas: BTreeMap<u32, f32> = BTreeMap::new();
        for (i, (&base, &trained)) in base_weights.iter().zip(trained_weights.iter()).enumerate() {
            if (base - trained).abs() > 1e-9 {
                deltas.insert(i as u32, trained);
            }
        }
        Checkpoint {
            format_version: 1,
            model_kind: model_kind.to_string(),
            graph_neurons_sha256: graph_neurons_sha256.to_string(),
            sensory_map_sha256: sensory_map_sha256.to_string(),
            output_map_sha256: output_map_sha256.to_string(),
            preset: preset.to_string(),
            rng_seed,
            trials_run,
            teacher_top1_accuracy,
            deltas: deltas.into_iter().collect(),
        }
    }
}
