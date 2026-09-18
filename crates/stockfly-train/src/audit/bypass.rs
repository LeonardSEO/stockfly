use stockfly_chess::output_map::{OutputMap, PROMOTION_ORDER};
use stockfly_sim::Stimulus;

fn random_weight(seed: u64, input: usize, output: usize) -> f32 {
    let mut value = seed
        ^ (input as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (output as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    ((value >> 40) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
}

/// A fixed 132-output random linear baseline. It consumes only the encoded
/// sensory vector and has no graph, recurrent state, learning, or teacher.
pub fn random_readout_rates(
    stimulus: &Stimulus,
    output: &OutputMap,
    neuron_count: usize,
    seed: u64,
) -> Vec<f32> {
    let active: Vec<_> = stimulus
        .values
        .iter()
        .enumerate()
        .filter(|(_, value)| **value != 0.0)
        .collect();
    let groups: Vec<&[u32]> = output
        .from_groups
        .iter()
        .chain(&output.to_groups)
        .map(Vec::as_slice)
        .chain(
            PROMOTION_ORDER
                .iter()
                .map(|name| output.promotion_group(name).unwrap_or(&[])),
        )
        .collect();
    let mut rates = vec![0.0; neuron_count];
    for (output_index, group) in groups.iter().enumerate() {
        let score = active
            .iter()
            .map(|(input_index, value)| random_weight(seed, *input_index, output_index) * **value)
            .sum::<f32>()
            .max(0.0);
        for &neuron in group.iter() {
            if let Some(rate) = rates.get_mut(neuron as usize) {
                *rate = score;
            }
        }
    }
    rates
}
