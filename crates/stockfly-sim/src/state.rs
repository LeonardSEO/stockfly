/// Per-neuron dynamical state shared by every simulator backend.
#[derive(Debug, Clone)]
pub struct BrainState {
    pub membrane: Vec<f32>,
    pub rate: Vec<f32>,
}

impl BrainState {
    pub fn zeroed(neuron_count: usize) -> Self {
        BrainState {
            membrane: vec![0.0; neuron_count],
            rate: vec![0.0; neuron_count],
        }
    }

    pub fn reset(&mut self) {
        self.membrane.iter_mut().for_each(|v| *v = 0.0);
        self.rate.iter_mut().for_each(|v| *v = 0.0);
    }
}

/// External per-neuron input for one simulation step (sensory injection,
/// context signals). Dense for the CPU reference backend; most entries are
/// zero for any given chess position.
#[derive(Debug, Clone)]
pub struct Stimulus {
    pub values: Vec<f32>,
}

impl Stimulus {
    pub fn zeroed(neuron_count: usize) -> Self {
        Stimulus {
            values: vec![0.0; neuron_count],
        }
    }
}

/// Simplified LIF/rate-hybrid update parameters. This is a documented
/// simplification, not a biophysical emulation — see the design spec's
/// "Core simulation model" section.
#[derive(Debug, Clone, Copy)]
pub struct SimConfig {
    pub dt_ms: f32,
    pub settle_steps: u32,
    pub threshold: f32,
    pub decay: f32,
    /// Uniform multiplier applied to every compiled edge magnitude before
    /// simulation (sign untouched). The compiled base magnitude is the raw
    /// MaleCNS synapse count (median 2, up to 2591), which is not on a
    /// scale a threshold=0.5 LIF-style unit expects -- without rescaling,
    /// per-step input to most neurons is far too small for activity to
    /// reach output populations within a training-affordable number of
    /// settle steps (empirically verified: even 16 settle steps left the
    /// descending-neuron output populations at exactly zero rate). This is
    /// a documented simulation-scale choice, not a change to which
    /// connections exist or their biological sign.
    pub weight_scale: f32,
    /// Hard per-step cap on `rate`. The compiled graph is strongly
    /// recurrent (~155 average in-degree across 165k neurons); without a
    /// saturating nonlinearity, the linear LIF-hybrid update can enter
    /// unbounded positive feedback and rates diverge to infinity within a
    /// handful of steps (empirically observed). This cap is a documented
    /// simulation-stability simplification, applied uniformly and not a
    /// per-neuron/per-region choice.
    pub max_rate: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            dt_ms: 1.0,
            settle_steps: 16,
            threshold: 0.5,
            weight_scale: 0.5, // 1 / median(|base_magnitude|) over the real compiled malecns-v1 graph
            decay: 0.9,
            max_rate: 20.0,
        }
    }
}

/// A lightweight per-step readout, cheap enough to log every step; full
/// `rate` access for visualization/decision-making goes through the
/// simulator's state accessor instead of being copied into every summary.
#[derive(Debug, Clone, Copy)]
pub struct FrameSummary {
    pub step: u32,
    pub max_rate: f32,
    pub mean_rate: f32,
}
