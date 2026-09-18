use std::collections::BTreeMap;

use stockfly_chess::output_map::OutputMap;
use stockfly_connectome::{Connectome, NeuronRecord};
use stockfly_sim::{CpuSimulator, SimConfig, Simulator, Stimulus};
use stockfly_train::{
    audit::{
        ablation::{AuditMetadata, NeuronMetadata, RegionAblation},
        bypass::random_readout_rates,
        output_permutation,
        reset::checkpoint_without_learning,
        shuffle,
    },
    checkpoint::Checkpoint,
};

fn manifest(neurons: usize, edges: usize) -> stockfly_connectome::format::CompiledManifest {
    serde_json::from_value(serde_json::json!({
        "format_version": 1,
        "dataset": "fixture",
        "neuron_count": neurons,
        "edge_count": edges,
        "neurons_sha256": "fixture-neurons",
        "offsets_sha256": "fixture-offsets",
        "edge_blocks": [],
        "sign_policy": {
            "excitatory_transmitters": ["acetylcholine"],
            "inhibitory_transmitters": ["gaba"],
            "unresolved_default": "excitatory",
            "source_column": "consensus_nt",
            "derived_from": "presynaptic fixture"
        }
    }))
    .unwrap()
}

fn metadata(transmitters: &[&str], regions: &[&str]) -> AuditMetadata {
    AuditMetadata {
        format_version: 1,
        graph_neurons_sha256: "fixture-neurons".into(),
        source: "synthetic fixture".into(),
        neurons: transmitters
            .iter()
            .zip(regions)
            .enumerate()
            .map(|(body_id, (&transmitter, &region))| NeuronMetadata {
                body_id: body_id as u64,
                transmitter: transmitter.into(),
                region: region.into(),
            })
            .collect(),
    }
}

fn checkpoint() -> Checkpoint {
    Checkpoint {
        format_version: 1,
        model_kind: "bio-full".into(),
        graph_neurons_sha256: "fixture-neurons".into(),
        sensory_map_sha256: "sensory".into(),
        output_map_sha256: "output".into(),
        preset: "smoke".into(),
        rng_seed: 7,
        trials_run: 4,
        teacher_top1_accuracy: 0.25,
        deltas: vec![(0, 2.0), (3, -3.0)],
    }
}

#[test]
fn shuffled_graph_is_seeded_degree_and_transmitter_compatible_and_non_destructive() {
    let graph = Connectome {
        manifest: manifest(8, 6),
        neurons: (0..8).map(|body_id| NeuronRecord { body_id }).collect(),
        // dst 2 gets two excitatory inputs, dst 3 gets two excitatory
        // inputs, and dst 4 gets two inhibitory inputs.
        offsets: vec![0, 0, 0, 2, 4, 6, 6, 6, 6],
        edge_src: vec![0, 1, 2, 3, 4, 5],
        edge_weight: vec![1.0, 1.0, 2.0, 2.0, -1.0, -1.0],
    };
    let metadata = metadata(
        &[
            "acetylcholine",
            "acetylcholine",
            "acetylcholine",
            "acetylcholine",
            "gaba",
            "gaba",
            "acetylcholine",
            "gaba",
        ],
        &["a", "a", "a", "a", "b", "b", "c", "c"],
    );
    let original_sources = graph.edge_src.clone();
    let original_weights = graph.edge_weight.clone();
    let (first, stats) = shuffle::degree_aware(&graph, &metadata, 91).unwrap();
    let (second, _) = shuffle::degree_aware(&graph, &metadata, 91).unwrap();

    assert_eq!(first.edge_src, second.edge_src);
    assert_ne!(first.edge_src, graph.edge_src);
    assert_eq!(first.offsets, graph.offsets, "destination fan-in is exact");
    assert_eq!(
        first.edge_weight, graph.edge_weight,
        "signs/magnitudes stay on edges"
    );
    assert!(stats.changed_edges > 0);
    assert_eq!(stats.destination_fan_in_max_abs_delta, 0);
    assert!(stats.source_out_degree_distribution_preserved);
    for (&source, &weight) in first.edge_src.iter().zip(&first.edge_weight) {
        let transmitter = &metadata.neurons[source as usize].transmitter;
        assert_eq!(weight.is_sign_negative(), transmitter == "gaba");
    }
    assert_eq!(graph.edge_src, original_sources);
    assert_eq!(graph.edge_weight, original_weights);
}

#[test]
fn reset_derives_an_empty_delta_checkpoint_without_mutating_training_metadata() {
    let checkpoint = checkpoint();
    let serialized = serde_json::to_string(&checkpoint).unwrap();
    let reset = checkpoint_without_learning(&checkpoint);
    assert!(reset.deltas.is_empty());
    assert_eq!(reset.rng_seed, checkpoint.rng_seed);
    assert_eq!(reset.trials_run, checkpoint.trials_run);
    assert_eq!(serde_json::to_string(&checkpoint).unwrap(), serialized);
}

#[test]
fn region_ablation_silences_activity_before_it_can_propagate_on_every_step() {
    let graph = Connectome {
        manifest: manifest(4, 2),
        neurons: (0..4).map(|body_id| NeuronRecord { body_id }).collect(),
        offsets: vec![0, 0, 1, 2, 2],
        edge_src: vec![0, 1],
        edge_weight: vec![1.0, 1.0],
    };
    let metadata = metadata(
        &["acetylcholine"; 4],
        &["sensory", "target-region", "output", "other"],
    );
    let control = RegionAblation::new(&graph, &metadata, "target-region").unwrap();
    let config = SimConfig {
        threshold: 0.0,
        decay: 0.0,
        weight_scale: 1.0,
        max_rate: 100.0,
        settle_steps: 3,
        ..SimConfig::default()
    };
    let mut intact = CpuSimulator::new(&graph, config);
    let mut ablated = CpuSimulator::new(&graph, config);
    let mut stimulus = Stimulus::zeroed(4);
    stimulus.values[0] = 1.0;
    for _ in 0..3 {
        intact.step(&stimulus);
        control.step(&mut ablated, &stimulus);
        assert_eq!(ablated.state().rate[1], 0.0);
    }
    assert!(intact.state().rate[2] > 0.0);
    assert_eq!(ablated.state().rate[2], 0.0);
    assert_eq!(graph.edge_weight, vec![1.0, 1.0]);
}

fn output_map() -> OutputMap {
    let promotion_groups = BTreeMap::from([
        ("queen", vec![128]),
        ("rook", vec![129]),
        ("bishop", vec![130]),
        ("knight", vec![131]),
    ]);
    OutputMap::load_str(
        &serde_json::json!({
            "format_version": 1,
            "seed": 1,
            "source_population": "fixture",
            "from_groups": (0..64).map(|i| vec![i]).collect::<Vec<_>>(),
            "to_groups": (64..128).map(|i| vec![i]).collect::<Vec<_>>(),
            "promotion_groups": promotion_groups,
        })
        .to_string(),
    )
    .unwrap()
}

#[test]
fn output_permutation_relabels_every_square_without_touching_activity() {
    let output = output_map();
    let activity: Vec<f32> = (0..132).map(|value| value as f32).collect();
    let before = activity.clone();
    let (permuted, stats) = output_permutation::permute(&output, 1234).unwrap();
    assert_eq!(stats.fixed_square_labels, 0);
    assert_eq!(activity, before);
    assert!(permuted
        .from_groups
        .iter()
        .zip(&output.from_groups)
        .all(|(left, right)| left != right));
    let mut old_from = output.from_groups.clone();
    let mut new_from = permuted.from_groups.clone();
    old_from.sort();
    new_from.sort();
    assert_eq!(old_from, new_from);
}

#[test]
fn brain_bypass_is_fixed_non_learning_and_does_not_mutate_sensory_activity() {
    let output = output_map();
    let mut stimulus = Stimulus::zeroed(146);
    stimulus.values[140] = 1.0;
    stimulus.values[141] = -1.0;
    let before = stimulus.values.clone();
    let first = random_readout_rates(&stimulus, &output, 146, 77);
    let second = random_readout_rates(&stimulus, &output, 146, 77);
    let other_seed = random_readout_rates(&stimulus, &output, 146, 78);
    assert_eq!(first, second);
    assert_ne!(first, other_seed);
    assert_eq!(stimulus.values, before);
    assert!(first.iter().any(|&rate| rate > 0.0));
}
