use std::time::Instant;

use serde::Deserialize;
use stockfly_chess::output_map::OutputMap;
use stockfly_chess::policy::choose_move;
use stockfly_chess::sensory::{encode_position, SensoryMap};
use stockfly_connectome::Connectome;
use stockfly_plasticity::{EdgeMetadata, PlasticityRule};
use stockfly_sim::{CpuSimulator, SimConfig, Simulator};

#[derive(Debug, Clone, Deserialize)]
pub struct CurriculumExample {
    pub fen: String,
    pub bestmove: String,
    pub stage: String,
    #[allow(dead_code)]
    pub nodes: u32,
    #[allow(dead_code)]
    pub seed: u64,
}

pub fn load_curriculum(path: impl AsRef<std::path::Path>) -> std::io::Result<Vec<CurriculumExample>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect())
}

/// Derives, once, the destination dense index for every edge in CSC order
/// (the compiled format stores only the source per edge plus per-neuron
/// offsets; training's per-edge plasticity update needs both endpoints).
pub fn edge_dst_lookup(connectome: &Connectome) -> Vec<u32> {
    let neuron_count = connectome.neurons.len();
    let mut dst = vec![0u32; connectome.edge_src.len()];
    for n in 0..neuron_count {
        let start = connectome.offsets[n] as usize;
        let end = connectome.offsets[n + 1] as usize;
        for slot in dst.iter_mut().take(end).skip(start) {
            *slot = n as u32;
        }
    }
    dst
}

pub struct TrainResult {
    pub trials_run: u64,
    pub teacher_top1_accuracy: f32,
    pub trained_weights: Vec<f32>,
}

pub struct TrainOptions<'a> {
    pub sim_config: SimConfig,
    pub deadline: Instant,
    /// `Some(edge indices)` restricts plasticity to that subset (Bio);
    /// `None` scans every edge with the rule's own activity gate (Max).
    pub bio_eligible_edges: Option<&'a [u32]>,
    /// When false, runs the identical encode/settle/decide loop and
    /// accuracy measurement but skips every plasticity update -- used to
    /// measure the untrained Stage-0 baseline on the exact same trials
    /// and simulator path a trained run uses, so the comparison is fair.
    pub apply_plasticity: bool,
}

/// Runs the curriculum trial loop: encode -> settle -> decide -> reward ->
/// local plasticity update, entirely on the CPU reference simulator's own
/// working weights (never mutating the compiled `Connectome`).
pub fn train(
    connectome: &Connectome,
    edge_dst: &[u32],
    sensory_map: &SensoryMap,
    output_map: &OutputMap,
    examples: &[CurriculumExample],
    rule: &mut dyn PlasticityRule,
    options: TrainOptions,
) -> TrainResult {
    let neuron_count = connectome.neurons.len();
    let mut sim = CpuSimulator::new(connectome, options.sim_config);
    // `sim`'s initial working weights are already `weight_scale`-scaled
    // (see CpuSimulator::new); using the same scaled values as the
    // plasticity "base_magnitude" keeps clamp ranges and checkpoint deltas
    // consistent with what the simulator actually runs on.
    let base_weights = sim.weights().to_vec();

    let mut trials_run: u64 = 0;
    let mut correct: u64 = 0;

    for example in examples {
        if Instant::now() >= options.deadline {
            break;
        }

        let stimulus = match encode_position(&example.fen, sensory_map, neuron_count) {
            Ok(s) => s,
            Err(_) => continue, // skip malformed FENs rather than abort the whole run
        };

        sim.reset();
        for _ in 0..options.sim_config.settle_steps {
            sim.step(&stimulus);
        }
        let rates = sim.state().rate.clone();

        let decision = match choose_move(&example.fen, &rates, output_map) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let is_correct = decision.uci == example.bestmove;
        if is_correct {
            correct += 1;
        }
        let reward = if is_correct { 1.0 } else { -0.25 };

        if options.apply_plasticity {
            apply_plasticity(&mut sim, &connectome.edge_src, &rates, &base_weights, edge_dst, rule, reward, options.bio_eligible_edges);
        }

        trials_run += 1;
    }

    let accuracy = if trials_run > 0 {
        correct as f32 / trials_run as f32
    } else {
        0.0
    };

    TrainResult {
        trials_run,
        teacher_top1_accuracy: accuracy,
        trained_weights: sim.weights().to_vec(),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_plasticity(
    sim: &mut CpuSimulator,
    edge_src: &[u32],
    post_rates: &[f32],
    base_weights: &[f32],
    edge_dst: &[u32],
    rule: &mut dyn PlasticityRule,
    reward: f32,
    bio_eligible_edges: Option<&[u32]>,
) {
    let weights = sim.weights_mut();

    match bio_eligible_edges {
        Some(edges) => {
            // Bio: iterate only the precomputed eligible subset.
            for &e in edges {
                let e_idx = e as usize;
                update_one_edge(edge_src[e_idx], edge_dst[e_idx], e_idx, weights, base_weights, post_rates, rule, reward, true);
            }
        }
        None => {
            // Max: scan every edge; the rule's own activity threshold
            // makes this cheap for edges whose pre/post activity is ~0.
            for e_idx in 0..weights.len() {
                update_one_edge(edge_src[e_idx], edge_dst[e_idx], e_idx, weights, base_weights, post_rates, rule, reward, false);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn update_one_edge(
    src: u32,
    dst: u32,
    edge_index: usize,
    weights: &mut [f32],
    base_weights: &[f32],
    post_rates: &[f32],
    rule: &mut dyn PlasticityRule,
    reward: f32,
    bio_eligible: bool,
) {
    let metadata = EdgeMetadata { bio_eligible };
    if !rule.eligible(edge_index as u64, &metadata) {
        return;
    }
    let pre = post_rates.get(src as usize).copied().unwrap_or(0.0);
    let post = post_rates.get(dst as usize).copied().unwrap_or(0.0);
    let base = base_weights[edge_index];
    let current = weights[edge_index];
    weights[edge_index] = rule.update(pre, post, reward, base, current);
}
