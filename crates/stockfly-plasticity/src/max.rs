use crate::{apply_signed_clamp, EdgeMetadata, PlasticityRule};

/// StockFly Max: every edge is eligible, but an activity threshold skips
/// edges whose pre/post activity is too small to matter this trial --
/// this is what keeps a per-trial update sparse (touching only active
/// edges) instead of a dense pass over all 25.6M edges.
pub struct MaxRule {
    pub learning_rate: f32,
    pub max_multiplier: f32,
    pub activity_threshold: f32,
}

impl Default for MaxRule {
    fn default() -> Self {
        MaxRule {
            learning_rate: 0.02,
            max_multiplier: 4.0,
            activity_threshold: 1e-3,
        }
    }
}

impl PlasticityRule for MaxRule {
    fn eligible(&self, _edge_index: u64, _metadata: &EdgeMetadata) -> bool {
        true
    }

    fn update(&mut self, pre: f32, post: f32, reward: f32, base_magnitude: f32, current_mag: f32) -> f32 {
        if pre <= self.activity_threshold && post <= self.activity_threshold {
            return current_mag;
        }
        let delta_abs = self.learning_rate * pre.max(0.0) * post.max(0.0) * reward;
        apply_signed_clamp(base_magnitude, current_mag, delta_abs, self.max_multiplier)
    }
}
