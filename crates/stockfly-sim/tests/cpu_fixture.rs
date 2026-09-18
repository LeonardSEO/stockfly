use std::fs;
use std::path::Path;

use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, SimConfig, Simulator, Stimulus};

/// Four neurons: 0 is stimulated externally; 0->2 is an excitatory edge,
/// 0->3 is an inhibitory edge that suppresses neuron 3's own strong direct
/// stimulus; neuron 1 has no edges at all (passive control).
fn write_fixture(dir: &Path) {
    fs::create_dir_all(dir.join("edge_src_blocks")).unwrap();
    fs::create_dir_all(dir.join("edge_weight_blocks")).unwrap();

    let body_ids: [u64; 4] = [0, 1, 2, 3];
    fs::write(
        dir.join("neurons.bin"),
        body_ids.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>(),
    )
    .unwrap();

    // dst=0: no incoming edges. dst=1: none. dst=2: one edge from src=0.
    // dst=3: one edge from src=0.
    let offsets: [u64; 5] = [0, 0, 0, 1, 2];
    fs::write(
        dir.join("offsets.bin"),
        offsets.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>(),
    )
    .unwrap();

    let edge_src: [u32; 2] = [0, 0];
    fs::write(
        dir.join("edge_src_blocks/0000.bin"),
        edge_src.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>(),
    )
    .unwrap();

    let edge_weight: [f32; 2] = [2.0, -2.0]; // 0->2 excitatory, 0->3 inhibitory
    fs::write(
        dir.join("edge_weight_blocks/0000.bin"),
        edge_weight.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>(),
    )
    .unwrap();

    let manifest = serde_json::json!({
        "format_version": 1, "dataset": "fixture", "neuron_count": 4, "edge_count": 2,
        "neurons_sha256": "unused", "offsets_sha256": "unused",
        "edge_blocks": [{"index": 0, "edge_start": 0, "edge_count": 2, "src_sha256": "unused", "weight_sha256": "unused"}],
        "sign_policy": {"excitatory_transmitters": [], "inhibitory_transmitters": [], "unresolved_default": "excitatory", "source_column": "consensus_nt", "derived_from": "presynaptic (body_pre) neuron"},
    });
    fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

#[test]
fn excitation_reaches_neuron_2_and_inhibition_suppresses_neuron_3() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());
    let connectome = Connectome::open(tmp.path()).unwrap();

    let config = SimConfig {
        dt_ms: 1.0,
        settle_steps: 2,
        threshold: 0.5,
        decay: 0.0, // no membrane leak/history: isolates the effect to each step's inputs
        weight_scale: 1.0,
        max_rate: 100.0,
    };
    let mut sim = CpuSimulator::new(&connectome, config);

    // Step 1: stimulate neuron 0 only. No propagation yet (synchronous
    // update uses the *previous* step's rates), so 2 and 3 stay silent.
    let mut stim1 = Stimulus::zeroed(4);
    stim1.values[0] = 1.0;
    sim.step(&stim1);
    assert!(sim.state().rate[0] > 0.0, "neuron 0 should fire from direct stimulus");
    assert_eq!(sim.state().rate[2], 0.0, "excitation has not propagated yet at step 1");
    assert_eq!(sim.state().rate[3], 0.0);

    // Step 2: neuron 0's step-1 rate now drives neurons 2 and 3. Neuron 3
    // also receives a strong direct stimulus that would fire it on its own,
    // but the inhibitory edge from 0 must cancel that out.
    let mut stim2 = Stimulus::zeroed(4);
    stim2.values[3] = 1.0;
    sim.step(&stim2);

    assert!(
        sim.state().rate[2] > 0.0,
        "excitatory edge 0->2 should have propagated neuron 0's activity by step 2"
    );
    assert_eq!(
        sim.state().rate[3], 0.0,
        "inhibitory edge 0->3 should suppress neuron 3 despite its own direct stimulus"
    );
    assert_eq!(sim.state().rate[1], 0.0, "unconnected, unstimulated neuron 1 stays silent");
}

#[test]
fn reset_zeroes_state_and_step_counter_restarts_summary() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());
    let connectome = Connectome::open(tmp.path()).unwrap();
    let mut sim = CpuSimulator::new(&connectome, SimConfig::default());

    let mut stim = Stimulus::zeroed(4);
    stim.values[0] = 5.0;
    let first = sim.step(&stim);
    assert_eq!(first.step, 1);
    assert!(sim.state().rate.iter().any(|&r| r > 0.0));

    sim.reset();
    assert!(sim.state().rate.iter().all(|&r| r == 0.0));
    assert!(sim.state().membrane.iter().all(|&m| m == 0.0));

    let after_reset = sim.step(&Stimulus::zeroed(4));
    assert_eq!(after_reset.step, 1, "step counter restarts after reset");
}
