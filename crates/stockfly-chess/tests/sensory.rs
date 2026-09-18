use stockfly_chess::{encode_position, SensoryMap};

const SENSORY_MAP_JSON: &str = include_str!("../resources/sensory-map.json");
const NEURON_COUNT: usize = 165_122; // real compiled malecns-v1 neuron count

fn load_map() -> SensoryMap {
    SensoryMap::load_str(SENSORY_MAP_JSON).expect("real sensory map should load")
}

#[test]
fn same_fen_and_map_produce_byte_identical_stimulus() {
    let map = load_map();
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    let s1 = encode_position(fen, &map, NEURON_COUNT).unwrap();
    let s2 = encode_position(fen, &map, NEURON_COUNT).unwrap();

    assert_eq!(s1.values, s2.values);
}

#[test]
fn moving_one_piece_changes_only_its_patch_and_context() {
    let map = load_map();
    let before = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    // e2-e4: clears e2's patch, sets e4's patch, flips side-to-move context,
    // sets the en-passant file context (e-file).
    let after = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";

    let s_before = encode_position(before, &map, NEURON_COUNT).unwrap();
    let s_after = encode_position(after, &map, NEURON_COUNT).unwrap();

    let mut changed_indices: Vec<usize> = (0..NEURON_COUNT)
        .filter(|&i| (s_before.values[i] - s_after.values[i]).abs() > 1e-9)
        .collect();
    changed_indices.sort_unstable();

    // Every changed neuron must belong to: e2's patch, e4's patch, or a context neuron.
    let mut allowed: Vec<u32> = Vec::new();
    allowed.extend(&map.board_populations[squares_index("e2")]);
    allowed.extend(&map.board_populations[squares_index("e4")]);
    allowed.push(map.context.side_to_move);
    allowed.extend(&map.context.castling_rights);
    allowed.extend(&map.context.en_passant_file);
    let allowed: std::collections::HashSet<u32> = allowed.into_iter().collect();

    for idx in &changed_indices {
        assert!(
            allowed.contains(&(*idx as u32)),
            "unexpected neuron {idx} changed outside e2/e4 patches and context"
        );
    }
    assert!(!changed_indices.is_empty(), "moving a piece should change something");
}

/// shakmaty::Square enumerates a1=0..h8=63 file-major within rank; this
/// mirrors that indexing for the test's own expectations.
fn squares_index(name: &str) -> usize {
    let bytes = name.as_bytes();
    let file = (bytes[0] - b'a') as usize;
    let rank = (bytes[1] - b'1') as usize;
    rank * 8 + file
}
