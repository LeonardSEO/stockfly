use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};
use stockfly_chess::{output_map::OutputMap, sensory::SensoryMap};
use stockfly_train::{audit::run_causal_cli, checkpoint::Checkpoint};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let unique = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "stockfly-native-hashes-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, bytes).unwrap();
}

#[test]
fn native_causal_report_hashes_actual_checkpoint_and_complete_graph_files() {
    let root = TestDir::new();
    let graph = root.0.join("graph");
    let chess = root.0.join("chess");
    let neurons: Vec<u8> = (0..146u64).flat_map(u64::to_le_bytes).collect();
    let offsets: Vec<u8> = (0..147u64)
        .map(|index| if index <= 92 { 0 } else { 1u64 })
        .flat_map(u64::to_le_bytes)
        .collect();
    let edge_src = 132u32.to_le_bytes();
    let edge_weight = 0.01f32.to_le_bytes();
    let graph_files = BTreeMap::from([
        ("edge_src_blocks/0000.bin", edge_src.as_slice()),
        ("edge_weight_blocks/0000.bin", edge_weight.as_slice()),
        ("neurons.bin", neurons.as_slice()),
        ("offsets.bin", offsets.as_slice()),
    ]);
    for (name, bytes) in &graph_files {
        write(graph.join(name), bytes);
    }
    let manifest = serde_json::json!({
        "format_version": 1,
        "dataset": "fixture",
        "neuron_count": 146,
        "edge_count": 1,
        "neurons_sha256": sha256(&neurons),
        "offsets_sha256": sha256(&offsets),
        "edge_blocks": [{
            "index": 0, "edge_start": 0, "edge_count": 1,
            "src_sha256": sha256(&edge_src),
            "weight_sha256": sha256(&edge_weight)
        }],
        "sign_policy": {
            "excitatory_transmitters": ["acetylcholine"],
            "inhibitory_transmitters": ["gaba"],
            "unresolved_default": "excitatory",
            "source_column": "consensus_nt",
            "derived_from": "presynaptic fixture"
        }
    });
    write(
        graph.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    );

    let sensory_json = serde_json::json!({
        "format_version": 1, "seed": 1, "neurons_per_square": 1,
        "board_populations": (0..64).map(|_| vec![132]).collect::<Vec<_>>(),
        "piece_embeddings": (["P", "N", "B", "R", "Q", "K", "p", "n", "b", "r", "q", "k"]
            .iter().map(|piece| (piece.to_string(), vec![1.0]))
            .collect::<BTreeMap<_, _>>()),
        "context": {
            "side_to_move": 133,
            "castling_rights": [134, 135, 136, 137],
            "en_passant_file": [138, 139, 140, 141, 142, 143, 144, 145]
        }
    })
    .to_string();
    let output_json = serde_json::json!({
        "format_version": 1, "seed": 1, "source_population": "fixture",
        "from_groups": (0..64).map(|index| vec![index]).collect::<Vec<_>>(),
        "to_groups": (64..128).map(|index| vec![index]).collect::<Vec<_>>(),
        "promotion_groups": {
            "queen": [128], "rook": [129], "bishop": [130], "knight": [131]
        }
    })
    .to_string();
    write(chess.join("sensory-map.json"), &sensory_json);
    write(chess.join("output-map.json"), &output_json);
    let sensory = SensoryMap::load_str(&sensory_json).unwrap();
    let output = OutputMap::load_str(&output_json).unwrap();

    let model = root.0.join("model.sfckpt");
    let model_json = serde_json::to_vec(&Checkpoint::from_dense(
        "bio-full",
        &sha256(&neurons),
        sensory.sha256(),
        output.sha256(),
        "smoke",
        1,
        1,
        0.0,
        &[0.005],
        &[2.0],
    ))
    .unwrap();
    write(&model, &model_json);
    let suite = root.0.join("suite.json");
    write(
        &suite,
        serde_json::to_vec(&serde_json::json!({
            "suite_version": 1,
            "source": "native hash fixture",
            "sections": {"legal": [{
                "fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "bestmove": "e2e4"
            }]}
        }))
        .unwrap(),
    );
    let metadata = root.0.join("metadata.json");
    write(
        &metadata,
        serde_json::to_vec(&serde_json::json!({
            "format_version": 1,
            "graph_neurons_sha256": sha256(&neurons),
            "source": "native hash fixture",
            "neurons": (0..146u64).map(|body_id| serde_json::json!({
                "body_id": body_id,
                "transmitter": "acetylcholine",
                "region": if body_id == 92 {"target"} else {"other"}
            })).collect::<Vec<_>>()
        }))
        .unwrap(),
    );

    let cli_args = || {
        vec![
            "--model",
            model.to_str().unwrap(),
            "--suite",
            suite.to_str().unwrap(),
            "--graph",
            graph.to_str().unwrap(),
            "--chess-dir",
            chess.to_str().unwrap(),
            "--metadata",
            metadata.to_str().unwrap(),
            "--settle-steps",
            "2",
            "--ablate-region",
            "target",
        ]
        .into_iter()
        .map(str::to_owned)
    };
    let report: serde_json::Value =
        serde_json::from_str(&run_causal_cli(cli_args()).unwrap()).unwrap();
    assert_eq!(report["inputs"]["model_sha256"], sha256(&model_json));
    let actual_graph_files: BTreeMap<_, _> = graph_files
        .iter()
        .map(|(name, bytes)| (name.to_string(), sha256(bytes)))
        .collect();
    let expected_graph_hash = sha256(&serde_json::to_vec(&actual_graph_files).unwrap());
    assert_eq!(report["inputs"]["graph_sha256"], expected_graph_hash);
    assert_eq!(
        report["inputs"]["graph_files"],
        serde_json::to_value(actual_graph_files).unwrap()
    );

    write(
        graph.join("edge_weight_blocks/0000.bin"),
        3.0f32.to_le_bytes(),
    );
    let error = run_causal_cli(cli_args()).unwrap_err().to_string();
    assert!(error.contains("compiled graph hash mismatch: edge_weight_blocks/0000.bin"));
}
