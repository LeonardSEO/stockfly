use stockfly_chess::{output_map::OutputMap, sensory::SensoryMap};
use stockfly_connectome::{Connectome, NeuronRecord};
use stockfly_sim::SimConfig;
use stockfly_train::{
    audit::{evaluate, Suite},
    checkpoint::Checkpoint,
};

const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn fixture() -> (Connectome, SensoryMap, OutputMap, Checkpoint, Suite) {
    let sensory = SensoryMap::load_str(&serde_json::json!({
        "format_version": 1, "seed": 1, "neurons_per_square": 1,
        "board_populations": (0..64).map(|_| vec![132]).collect::<Vec<_>>(),
        "piece_embeddings": (["P", "N", "B", "R", "Q", "K", "p", "n", "b", "r", "q", "k"]
            .iter().map(|p| (p.to_string(), vec![1.0])).collect::<std::collections::BTreeMap<_, _>>()),
        "context": {"side_to_move": 133, "castling_rights": [134,135,136,137], "en_passant_file": [138,139,140,141,142,143,144,145]}
    }).to_string()).unwrap();
    let output = OutputMap::load_str(
        &serde_json::json!({
            "format_version": 1, "seed": 1, "source_population": "fixture",
            "from_groups": (0..64).map(|i| vec![i]).collect::<Vec<_>>(),
            "to_groups": (64..128).map(|i| vec![i]).collect::<Vec<_>>(),
            "promotion_groups": {"queen": [128], "rook": [129], "bishop": [130], "knight": [131]}
        })
        .to_string(),
    )
    .unwrap();
    let graph = Connectome {
        manifest: serde_json::from_value(serde_json::json!({
            "format_version": 1, "dataset": "fixture", "neuron_count": 146, "edge_count": 1,
            "neurons_sha256": "fixture", "offsets_sha256": "fixture", "edge_blocks": [],
            "sign_policy": {"excitatory_transmitters": [], "inhibitory_transmitters": [],
                "unresolved_default": "excitatory", "source_column": "fixture", "derived_from": "fixture"}
        })).unwrap(),
        neurons: (0..146).map(|body_id| NeuronRecord {body_id}).collect(),
        // Sensory neuron 132 -> to-square e4 (64 + 28 = 92).
        offsets: (0..147).map(|i| if i <= 92 {0} else {1}).collect(),
        edge_src: vec![132], edge_weight: vec![0.01],
    };
    let checkpoint = Checkpoint::from_dense(
        "bio-full",
        "fixture",
        sensory.sha256(),
        output.sha256(),
        "smoke",
        1,
        1,
        0.0,
        &[0.005],
        &[2.0],
    );
    let suite = serde_json::from_value(serde_json::json!({
        "suite_version": 1, "source": "synthetic regression fixture",
        "sections": {"legal": [{"fen": FEN, "bestmove": "e2e4"}, {"fen": FEN, "bestmove": "e2e4"}]}
    }))
    .unwrap();
    (graph, sensory, output, checkpoint, suite)
}

#[test]
fn learned_weights_change_move_and_reset_is_non_destructive_and_repeatable() {
    let (graph, sensory, output, checkpoint, suite) = fixture();
    let config = SimConfig {
        settle_steps: 2,
        ..SimConfig::default()
    };
    let checkpoint_before = serde_json::to_string(&checkpoint).unwrap();
    let trials = evaluate(&graph, &sensory, &output, &checkpoint, &suite, config).unwrap();
    for trial in &trials {
        assert_eq!(trial.intact_move, "e2e4");
        assert_eq!(trial.reset_move, "a2a3");
        assert!(trial.readout_max_abs_difference > 0.0);
    }
    assert_eq!(
        serde_json::to_string(&trials[0]).unwrap(),
        serde_json::to_string(&trials[1]).unwrap()
    );
    assert_eq!(graph.edge_weight, vec![0.01]);
    assert_eq!(
        serde_json::to_string(&checkpoint).unwrap(),
        checkpoint_before
    );
    let empty = Checkpoint {
        deltas: vec![],
        ..checkpoint
    };
    let baseline = evaluate(&graph, &sensory, &output, &empty, &suite, config).unwrap();
    assert!(baseline
        .iter()
        .all(|t| t.intact_move == t.reset_move && t.readout_max_abs_difference == 0.0));
}

#[test]
fn rejects_incompatible_checkpoint_invalid_deltas_and_invalid_suite_targets() {
    let (graph, sensory, output, mut checkpoint, mut suite) = fixture();
    let config = SimConfig::default();
    checkpoint.output_map_sha256 = "wrong".into();
    assert!(evaluate(&graph, &sensory, &output, &checkpoint, &suite, config).is_err());
    checkpoint.output_map_sha256 = output.sha256().into();
    for deltas in [
        vec![(1, 2.0)],
        vec![(0, -2.0)],
        vec![(0, f32::NAN)],
        vec![(0, 2.0), (0, 3.0)],
    ] {
        checkpoint.deltas = deltas;
        assert!(evaluate(&graph, &sensory, &output, &checkpoint, &suite, config).is_err());
    }
    checkpoint.deltas.clear();
    suite.sections.get_mut("legal").unwrap()[0].bestmove = "e2e5".into();
    assert!(evaluate(&graph, &sensory, &output, &checkpoint, &suite, config).is_err());
}
