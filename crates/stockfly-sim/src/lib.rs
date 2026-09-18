pub mod cpu;
pub mod state;

pub use cpu::CpuSimulator;
pub use state::{BrainState, FrameSummary, SimConfig, Stimulus};

/// Common interface implemented by every simulation backend (CPU reference,
/// native wgpu, browser WebGPU/WASM) so training, inference, and
/// visualization code can be backend-agnostic.
pub trait Simulator {
    fn step(&mut self, stimulus: &Stimulus) -> FrameSummary;
    fn reset(&mut self);
}
