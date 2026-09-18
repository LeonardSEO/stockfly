use crate::{apply_signed_clamp, EdgeMetadata, PlasticityRule};

/// StockFly Bio: plasticity restricted to the real mushroom-body /
/// dopaminergic learning circuit (see `EdgeMetadata::bio_eligible`).
/// Ineligible edges are returned unchanged by `update` if called (callers
/// should skip them entirely using `eligible` first, for performance).
pub struct BioRule {
    pub learning_rate: f32,
    pub max_multiplier: f32,
}

impl Default for BioRule {
    fn default() -> Self {
        BioRule {
            learning_rate: 0.05,
            max_multiplier: 4.0,
        }
    }
}

impl PlasticityRule for BioRule {
    fn eligible(&self, _edge_index: u64, metadata: &EdgeMetadata) -> bool {
        metadata.bio_eligible
    }

    fn update(&mut self, pre: f32, post: f32, reward: f32, base_magnitude: f32, current_mag: f32) -> f32 {
        let delta_abs = self.learning_rate * pre.max(0.0) * post.max(0.0) * reward;
        apply_signed_clamp(base_magnitude, current_mag, delta_abs, self.max_multiplier)
    }
}
