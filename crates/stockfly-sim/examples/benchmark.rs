use std::env;
use std::time::Instant;

use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, SimConfig, Simulator, Stimulus};

fn main() {
    let path = env::args().nth(1).expect("usage: benchmark <compiled-graph-dir> [steps]");
    let steps: u32 = env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(16);

    let connectome = Connectome::open(&path).expect("open compiled graph");
    let neuron_count = connectome.neurons.len();
    println!("neurons: {neuron_count}, edges: {}", connectome.edge_src.len());

    let mut sim = CpuSimulator::new(&connectome, SimConfig::default());
    let mut stim = Stimulus::zeroed(neuron_count);
    stim.values[0] = 1.0;

    let t0 = Instant::now();
    for _ in 0..steps {
        sim.step(&stim);
    }
    let elapsed = t0.elapsed();
    let steps_per_second = steps as f64 / elapsed.as_secs_f64();

    println!("steps: {steps}");
    println!("elapsed_ms: {}", elapsed.as_millis());
    println!("steps_per_second: {steps_per_second:.2}");
    println!("ms_per_step: {:.3}", elapsed.as_secs_f64() * 1000.0 / steps as f64);
}
