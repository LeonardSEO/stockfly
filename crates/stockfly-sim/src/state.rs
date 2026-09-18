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
}

impl Default for SimConfig {
    fn default() -> Self {
        SimConfig {
            dt_ms: 1.0,
            settle_steps: 16,
            threshold: 0.5,
            decay: 0.9,
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
