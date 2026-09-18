use std::fs;
use std::path::Path;

use stockfly_connectome::Connectome;

/// Writes the same 5-neuron compiled graph that
/// `tools/malecns/test_compile.py::test_compile_fixture_filters_signs_and_aggregates`
/// produces from its synthetic raw fixture, so the Python compiler and the
/// Rust reader are checked against one agreed-upon expected graph:
///
///   10 -(+3.0)-> 20 -(-2.0)-> 30 -(+2.0)-> 40 -(-5.0)-> 50
///
/// dense indices: 10->0, 20->1, 30->2, 40->3, 50->4
fn write_fixture(dir: &Path) {
    fs::create_dir_all(dir.join("edge_src_blocks")).unwrap();
    fs::create_dir_all(dir.join("edge_weight_blocks")).unwrap();

    let body_ids: [u64; 5] = [10, 20, 30, 40, 50];
    let neurons_bytes: Vec<u8> = body_ids.iter().flat_map(|v| v.to_le_bytes()).collect();
    fs::write(dir.join("neurons.bin"), neurons_bytes).unwrap();

    // dst=0(10): no edges; dst=1(20): 1 edge from 0; dst=2(30): 1 edge from 1;
    // dst=3(40): 1 edge from 2; dst=4(50): 1 edge from 3.
    let offsets: [u64; 6] = [0, 0, 1, 2, 3, 4];
    let offsets_bytes: Vec<u8> = offsets.iter().flat_map(|v| v.to_le_bytes()).collect();
    fs::write(dir.join("offsets.bin"), offsets_bytes).unwrap();

    let edge_src: [u32; 4] = [0, 1, 2, 3];
    let edge_src_bytes: Vec<u8> = edge_src.iter().flat_map(|v| v.to_le_bytes()).collect();
    fs::write(dir.join("edge_src_blocks/0000.bin"), &edge_src_bytes).unwrap();

    let edge_weight: [f32; 4] = [3.0, -2.0, 2.0, -5.0];
    let edge_weight_bytes: Vec<u8> = edge_weight.iter().flat_map(|v| v.to_le_bytes()).collect();
    fs::write(dir.join("edge_weight_blocks/0000.bin"), &edge_weight_bytes).unwrap();

    let manifest = serde_json::json!({
        "format_version": 1,
        "dataset": "fixture",
        "neuron_count": 5,
        "edge_count": 4,
        "neurons_sha256": "unused-in-fixture",
        "offsets_sha256": "unused-in-fixture",
        "edge_blocks": [{
            "index": 0,
            "edge_start": 0,
            "edge_count": 4,
            "src_sha256": "unused-in-fixture",
            "weight_sha256": "unused-in-fixture",
        }],
        "sign_policy": {
            "excitatory_transmitters": ["acetylcholine", "dopamine", "octopamine", "serotonin"],
            "inhibitory_transmitters": ["gaba", "glutamate", "histamine"],
            "unresolved_default": "excitatory",
            "source_column": "consensus_nt",
            "derived_from": "presynaptic (body_pre) neuron",
        },
    });
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn loads_and_validates_the_shared_fixture_graph() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());

    let connectome = Connectome::open(tmp.path()).expect("open compiled fixture");
    let report = connectome.validate().expect("validate compiled fixture");

    assert!(report.is_ok());
    assert_eq!(report.neuron_count, 5);
    assert_eq!(report.edge_count, 4);

    assert_eq!(connectome.neurons[0].body_id, 10);
    assert_eq!(connectome.neurons[4].body_id, 50);

    // dst=1 (body 20) has one incoming edge from dense index 0 (body 10), weight +3.0
    let (start, end) = (connectome.offsets[1] as usize, connectome.offsets[2] as usize);
    assert_eq!(&connectome.edge_src[start..end], &[0]);
    assert_eq!(&connectome.edge_weight[start..end], &[3.0]);

    // dst=4 (body 50) has one incoming edge from dense index 3 (body 40), weight -5.0
    let (start, end) = (connectome.offsets[4] as usize, connectome.offsets[5] as usize);
    assert_eq!(&connectome.edge_src[start..end], &[3]);
    assert_eq!(&connectome.edge_weight[start..end], &[-5.0]);
}

#[test]
fn rejects_a_graph_whose_final_offset_does_not_match_edge_count() {
    let tmp = tempfile::tempdir().unwrap();
    write_fixture(tmp.path());
    // Corrupt: truncate the offsets file's last entry so it disagrees with edge_count.
    let bad_offsets: [u64; 6] = [0, 0, 1, 2, 3, 999];
    let bytes: Vec<u8> = bad_offsets.iter().flat_map(|v| v.to_le_bytes()).collect();
    fs::write(tmp.path().join("offsets.bin"), bytes).unwrap();

    let connectome = Connectome::open(tmp.path()).expect("open compiled fixture");
    let result = connectome.validate();
    assert!(result.is_err());
}
