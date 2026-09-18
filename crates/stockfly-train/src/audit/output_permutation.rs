use serde::Serialize;
use stockfly_chess::{
    output_map::OutputMap,
    policy::{choose_move, MoveDecision},
};

use super::shuffle::SeededRng;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Serialize)]
pub struct OutputPermutationStats {
    pub seed: u64,
    pub rng: &'static str,
    pub neural_activity: &'static str,
    pub square_labels: Vec<usize>,
    pub fixed_square_labels: usize,
}

pub fn permute(output: &OutputMap, seed: u64) -> Result<(OutputMap, OutputPermutationStats)> {
    let mut permutation = [0usize; 64];
    for (index, value) in permutation.iter_mut().enumerate() {
        *value = index;
    }
    // Sattolo's algorithm creates one cycle, guaranteeing that every square
    // label changes while retaining all original neural group memberships.
    let mut rng = SeededRng::new(seed);
    for upper in (1..permutation.len()).rev() {
        let selected = (rng.next_u64() % upper as u64) as usize;
        permutation.swap(upper, selected);
    }
    let derived = output.permute_square_labels(&permutation)?;
    let fixed_square_labels = permutation
        .iter()
        .enumerate()
        .filter(|(index, value)| *index == **value)
        .count();
    Ok((
        derived,
        OutputPermutationStats {
            seed,
            rng: "splitmix64-v1 + sattolo-v1",
            neural_activity: "unchanged intact simulator rates",
            square_labels: permutation.to_vec(),
            fixed_square_labels,
        },
    ))
}

/// Evaluates both output labelings from the exact same immutable simulator
/// rate slice. Keeping this pairing here makes the activity-control invariant
/// explicit and directly testable.
pub fn paired_decisions(
    fen: &str,
    intact_rates: &[f32],
    intact: &OutputMap,
    permuted: &OutputMap,
) -> stockfly_chess::policy::Result<(MoveDecision, MoveDecision)> {
    Ok((
        choose_move(fen, intact_rates, intact)?,
        choose_move(fen, intact_rates, permuted)?,
    ))
}
