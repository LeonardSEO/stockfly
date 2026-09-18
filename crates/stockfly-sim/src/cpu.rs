use stockfly_connectome::Connectome;

use crate::state::{BrainState, FrameSummary, SimConfig, Stimulus};
use crate::Simulator;

/// Deterministic, single-threaded CPU reference backend. Used for
/// correctness tests and as the ground truth the wgpu backend's parity
/// tests are checked against.
pub struct CpuSimulator<'a> {
    connectome: &'a Connectome,
    /// Trainable working weights, cloned from `connectome.edge_weight` at
    /// construction. Training mutates this array in place (see
    /// `weights_mut`); `connectome`'s own arrays are never mutated, so the
    /// compiled base graph always remains the source of truth for a
    /// checkpoint's delta.
    edge_weight: Vec<f32>,
    config: SimConfig,
    state: BrainState,
    step_count: u32,
}

impl<'a> CpuSimulator<'a> {
    pub fn new(connectome: &'a Connectome, config: SimConfig) -> Self {
        let neuron_count = connectome.neurons.len();
        let edge_weight = connectome
            .edge_weight
            .iter()
            .map(|w| w * config.weight_scale)
            .collect();
        CpuSimulator {
            edge_weight,
            connectome,
            config,
            state: BrainState::zeroed(neuron_count),
            step_count: 0,
        }
    }

    pub fn state(&self) -> &BrainState {
        &self.state
    }

    /// Mutable access to this simulator's own working weights (initially a
    /// clone of the compiled base graph), used by the trainer to apply
    /// plasticity updates without touching the immutable `Connectome`.
    pub fn weights_mut(&mut self) -> &mut [f32] {
        &mut self.edge_weight
    }

    pub fn weights(&self) -> &[f32] {
        &self.edge_weight
    }

    /// Read-only access to the immutable compiled graph's source index for
    /// one edge (topology never changes during training).
    pub fn connectome_edge_src(&self, edge_index: usize) -> u32 {
        self.connectome.edge_src[edge_index]
    }
}

impl<'a> Simulator for CpuSimulator<'a> {
    fn step(&mut self, stimulus: &Stimulus) -> FrameSummary {
        let neuron_count = self.connectome.neurons.len();
        debug_assert_eq!(stimulus.values.len(), neuron_count);

        let prev_rate = self.state.rate.clone();
        let offsets = &self.connectome.offsets;
        let edge_src = &self.connectome.edge_src;
        let edge_weight = &self.edge_weight;

        for dst in 0..neuron_count {
            let start = offsets[dst] as usize;
            let end = offsets[dst + 1] as usize;
            let mut input = 0.0f32;
            for e in start..end {
                let src = edge_src[e] as usize;
                input += edge_weight[e] * prev_rate[src];
            }
            let stim = stimulus.values.get(dst).copied().unwrap_or(0.0);
            let membrane = self.state.membrane[dst] * self.config.decay + input + stim;
            self.state.membrane[dst] = membrane;
            self.state.rate[dst] = (membrane - self.config.threshold).clamp(0.0, self.config.max_rate);
        }

        self.step_count += 1;

        let max_rate = self.state.rate.iter().cloned().fold(0.0f32, f32::max);
        let mean_rate = if neuron_count > 0 {
            self.state.rate.iter().sum::<f32>() / neuron_count as f32
        } else {
            0.0
        };

        FrameSummary {
            step: self.step_count,
            max_rate,
            mean_rate,
        }
    }

    fn reset(&mut self) {
        self.state.reset();
        self.step_count = 0;
    }
}
