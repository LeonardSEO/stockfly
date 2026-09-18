pub mod bio;
pub mod max;

pub use bio::BioRule;
pub use max::MaxRule;

/// Per-edge metadata a plasticity rule needs to decide eligibility.
/// `bio_eligible` is precomputed offline (see
/// `tools/models/generate_bio_mask.py`) from the real MaleCNS
/// mushroom-body / dopaminergic circuit classes (Kenyon_Cell, DAN, MBON) --
/// it is not derived at runtime.
#[derive(Debug, Clone, Copy)]
pub struct EdgeMetadata {
    pub bio_eligible: bool,
}

/// A local, three-factor (pre * post * reward) learning rule. Deliberately
/// has no access to the full recurrent computation graph: each update only
/// needs this edge's own pre/post activity, the global reward signal, its
/// immutable compiled base magnitude (sign + starting scale), and its own
/// current trainable magnitude. This is what makes training tractable
/// without storing gradients through the whole 25.6M-edge graph.
pub trait PlasticityRule {
    fn eligible(&self, edge_index: u64, metadata: &EdgeMetadata) -> bool;

    /// Returns the new signed magnitude for this edge. `base_magnitude` is
    /// the immutable compiled value (its sign is the connection's
    /// biological sign and must never flip); `current_mag` is this edge's
    /// present trainable value (same sign as `base_magnitude`, or exactly
    /// 0.0 if fully depressed).
    fn update(&mut self, pre: f32, post: f32, reward: f32, base_magnitude: f32, current_mag: f32) -> f32;
}

/// Scales `current_mag`'s absolute value by `delta_abs` (positive grows,
/// negative shrinks) while clamping to `[0, max_multiplier * |base|]` and
/// re-applying `base_magnitude`'s sign -- the shared clamp logic both Bio
/// and Max use to guarantee sign preservation.
pub(crate) fn apply_signed_clamp(base_magnitude: f32, current_mag: f32, delta_abs: f32, max_multiplier: f32) -> f32 {
    let sign = if base_magnitude >= 0.0 { 1.0 } else { -1.0 };
    let max_abs = base_magnitude.abs() * max_multiplier;
    let new_abs = (current_mag.abs() + delta_abs).clamp(0.0, max_abs);
    sign * new_abs
}
