use shakmaty::fen::Fen;
use shakmaty::uci::UciMove;
use shakmaty::{CastlingMode, Chess, Position};
use std::str::FromStr;

use stockfly_chess::output_map::OutputMap;
use stockfly_chess::policy::choose_move;

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// A tiny synthetic map: from_groups[i] = neuron i, to_groups[i] = neuron
/// 64+i, promotion groups at 128..132. Isolated from the real 165k-neuron
/// map so test rates are easy to reason about by hand.
fn synthetic_map() -> OutputMap {
    let from_groups: Vec<Vec<u32>> = (0..64).map(|i| vec![i]).collect();
    let to_groups: Vec<Vec<u32>> = (0..64).map(|i| vec![64 + i]).collect();
    let json = serde_json::json!({
        "format_version": 1,
        "seed": 1,
        "source_population": "synthetic",
        "from_groups": from_groups,
        "to_groups": to_groups,
        "promotion_groups": {
            "queen": [128], "rook": [129], "bishop": [130], "knight": [131],
        },
    });
    OutputMap::load_str(&json.to_string()).unwrap()
}

fn square_index(name: &str) -> usize {
    let b = name.as_bytes();
    ((b[1] - b'1') as usize) * 8 + (b[0] - b'a') as usize
}

#[test]
fn best_legal_pair_is_chosen_over_a_higher_scoring_illegal_pair() {
    let map = synthetic_map();
    let mut rates = vec![0.0f32; 132];

    // Illegal, highest-scoring pair: a1 (rook, blocked) -> e8 (occupied by
    // enemy king, unreachable anyway from a1 at startpos).
    rates[square_index("a1")] = 50.0;
    rates[64 + square_index("e8")] = 100.0;

    // Legal pair with the next-highest score: e2 -> e4.
    rates[square_index("e2")] = 10.0;
    rates[64 + square_index("e4")] = 5.0;

    let decision = choose_move(STARTPOS, &rates, &map).unwrap();
    assert_eq!(decision.uci, "e2e4");
}

#[test]
fn ties_break_to_the_lexicographically_smallest_legal_uci_move() {
    let map = synthetic_map();
    let rates = vec![0.0f32; 132]; // every legal move scores identically (0)

    let decision = choose_move(STARTPOS, &rates, &map).unwrap();

    let setup = Fen::from_str(STARTPOS).unwrap();
    let position: Chess = setup.into_position(CastlingMode::Standard).unwrap();
    let mut legal_ucis: Vec<String> = position
        .legal_moves()
        .iter()
        .map(|m| UciMove::from_standard(m).to_string())
        .collect();
    legal_ucis.sort();

    assert_eq!(decision.uci, legal_ucis[0]);
}
