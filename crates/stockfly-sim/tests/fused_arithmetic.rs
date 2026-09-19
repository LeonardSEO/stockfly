use stockfly_connectome::format::{CompiledManifest, SignPolicy};
use stockfly_connectome::{Connectome, NeuronRecord};
use stockfly_sim::{CpuSimulator, SimConfig, Simulator, Stimulus};

// (1 + 2^-23)^2 - (1 + 2^-22) = 2^-46 with a fused multiply-add,
// but separate f32 multiply/add rounds this cancellation to zero.
fn fixture(gather: bool) -> (Connectome, SimConfig, Stimulus, usize) {
    let a = f32::from_bits(1.0f32.to_bits() + 1);
    let b = f32::from_bits(1.0f32.to_bits() + 2);
    let (offsets, edge_src, edge_weight, stimulus, destination) = if gather {
        (
            vec![0, 0, 0, 2],
            vec![0, 1],
            vec![-b, a],
            vec![1.0, a, 0.0],
            2,
        )
    } else {
        (vec![0, 1, 1, 1], vec![1], vec![-b], vec![a, 1.0, 0.0], 0)
    };
    let graph = Connectome {
        manifest: CompiledManifest {
            format_version: 1,
            dataset: "fused-cancellation".into(),
            neuron_count: 3,
            edge_count: edge_src.len() as u64,
            neurons_sha256: "unused".into(),
            offsets_sha256: "unused".into(),
            edge_blocks: vec![],
            sign_policy: SignPolicy {
                excitatory_transmitters: vec![],
                inhibitory_transmitters: vec![],
                unresolved_default: "excitatory".into(),
                source_column: "unused".into(),
                derived_from: "unused".into(),
            },
        },
        neurons: (0..3).map(|body_id| NeuronRecord { body_id }).collect(),
        offsets,
        edge_src,
        edge_weight,
    };
    let config = SimConfig {
        decay: if gather { 0.0 } else { a },
        threshold: 0.0,
        weight_scale: 1.0,
        max_rate: 2.0,
        settle_steps: 2,
        dt_ms: 1.0,
    };
    (graph, config, Stimulus { values: stimulus }, destination)
}
fn assert_cpu_cancellation(gather: bool) {
    let (graph, config, first, destination) = fixture(gather);
    let mut cpu = CpuSimulator::new(&graph, config);
    cpu.step(&first);
    cpu.step(&Stimulus::zeroed(3));
    let expected = 2.0f32.powi(-46);
    assert_eq!(cpu.state().membrane[destination], expected);
    assert_eq!(cpu.state().rate[destination], expected);
}
#[test]
fn cpu_gather_preserves_fused_cancellation() {
    assert_cpu_cancellation(true);
}
#[test]
fn cpu_membrane_preserves_fused_cancellation() {
    assert_cpu_cancellation(false);
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "requires actual Apple Metal; run --test fused_arithmetic -- --ignored --nocapture"]
fn metal_preserves_both_fused_cancellations() {
    use stockfly_sim::{GpuAdapterOptions, GpuSimulator};
    for gather in [true, false] {
        let (graph, config, first, destination) = fixture(gather);
        let mut gpu = GpuSimulator::new(&graph, config, GpuAdapterOptions::default()).unwrap();
        assert_eq!(gpu.backend(), "metal");
        gpu.run_steps(&first, 1).unwrap();
        gpu.run_steps(&Stimulus::zeroed(3), 1).unwrap();
        assert_eq!(gpu.state().membrane[destination], 2.0f32.powi(-46));
        assert_eq!(gpu.state().rate[destination], 2.0f32.powi(-46));
    }
}
