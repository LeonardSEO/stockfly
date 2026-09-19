#![cfg(target_os = "macos")]

use stockfly_connectome::format::{CompiledManifest, SignPolicy};
use stockfly_connectome::{Connectome, NeuronRecord};
use stockfly_sim::{CpuSimulator, GpuAdapterOptions, GpuSimulator, SimConfig, Simulator, Stimulus};

fn synthetic_connectome() -> Connectome {
    // Destination-major incoming edges. Neuron 0 has no incoming edge,
    // which also exercises zero-degree handling at the start of a chunk.
    let offsets = vec![0, 0, 1, 3, 5, 7, 9];
    let edge_src = vec![0, 0, 1, 2, 0, 2, 3, 4, 1];
    let edge_weight = vec![1.0, 0.75, -0.5, 0.4, 0.2, -0.3, 0.6, 0.25, -0.1];
    Connectome {
        manifest: CompiledManifest {
            format_version: 1,
            dataset: "gpu-parity-fixture".to_string(),
            neuron_count: 6,
            edge_count: edge_src.len() as u64,
            neurons_sha256: "unused".to_string(),
            offsets_sha256: "unused".to_string(),
            edge_blocks: Vec::new(),
            sign_policy: SignPolicy {
                excitatory_transmitters: Vec::new(),
                inhibitory_transmitters: Vec::new(),
                unresolved_default: "excitatory".to_string(),
                source_column: "unused".to_string(),
                derived_from: "unused".to_string(),
            },
        },
        neurons: (0..6).map(|body_id| NeuronRecord { body_id }).collect(),
        offsets,
        edge_src,
        edge_weight,
    }
}

#[test]
#[ignore = "requires an Apple Metal GPU; run with: cargo test -p stockfly-sim gpu_matches_cpu_fixture -- --ignored --nocapture"]
fn gpu_matches_cpu_fixture() {
    let connectome = synthetic_connectome();
    let config = SimConfig {
        dt_ms: 1.0,
        settle_steps: 16,
        threshold: 0.2,
        decay: 0.85,
        weight_scale: 0.25,
        max_rate: 2.5,
    };

    let mut cpu = CpuSimulator::new(&connectome, config);
    // Mimic learned checkpoint application with simulator-ready values. This
    // catches accidental re-scaling or loss of trained weights on the GPU.
    cpu.weights_mut()[2] = -0.075;
    cpu.weights_mut()[6] = 0.19;
    let learned_weights = cpu.weights().to_vec();

    let mut stimulus = Stimulus::zeroed(connectome.neurons.len());
    stimulus.values[0] = 0.9;
    stimulus.values[3] = 0.15;
    let mut gpu = GpuSimulator::new_with_weights(
        &connectome,
        config,
        GpuAdapterOptions::default(),
        &learned_weights,
    )
    .expect("create Metal simulator");
    assert_eq!(
        gpu.backend(),
        "metal",
        "test must exercise the real Metal backend"
    );
    for step in 1..=config.settle_steps {
        cpu.step(&stimulus);
        gpu.run_steps(&stimulus, 1).expect("run Metal simulation");
        for (field, left, right) in [
            ("membrane", &cpu.state().membrane, &gpu.state().membrane),
            ("rate", &cpu.state().rate, &gpu.state().rate),
        ] {
            for (neuron, (&cpu, &gpu)) in left.iter().zip(right).enumerate() {
                let allowed = 1e-4 + 2e-5 * cpu.abs().max(gpu.abs());
                assert!(
                    cpu.is_finite() && gpu.is_finite() && (cpu - gpu).abs() <= allowed,
                    "step {step} neuron {neuron} {field}: CPU={cpu} GPU={gpu} allowed={allowed}"
                );
            }
        }
    }
    let incremental = gpu.state().clone();
    gpu.reset_state();
    gpu.run_steps(&stimulus, config.settle_steps)
        .expect("run batched Metal simulation");
    assert_eq!(
        incremental.membrane,
        gpu.state().membrane,
        "reset/batched membrane differs"
    );
    assert_eq!(
        incremental.rate,
        gpu.state().rate,
        "reset/batched rates differ"
    );

    let max_abs_rate_difference = cpu
        .state()
        .rate
        .iter()
        .zip(&gpu.state().rate)
        .map(|(cpu, gpu)| (cpu - gpu).abs())
        .fold(0.0f32, f32::max);
    eprintln!(
        "adapter={} backend={} max_abs_rate_difference={max_abs_rate_difference:.9}",
        gpu.adapter_name(),
        gpu.backend()
    );
    assert!(
        max_abs_rate_difference < 1e-4,
        "CPU/GPU rate difference {max_abs_rate_difference} exceeds 1e-4"
    );
}
