//! Paired, frozen-checkpoint evaluation. Both conditions start each FEN
//! with zero state and identical calibration; reset omits learned weights.
pub mod ablation;
pub mod bypass;
pub mod hashes;
pub mod output_permutation;
pub mod reset;
pub mod shuffle;

use std::{
    collections::{BTreeMap, HashSet},
    fs,
};

use serde::{Deserialize, Serialize};
use stockfly_chess::{
    output_map::OutputMap,
    policy::{choose_move, MoveDecision},
    sensory::{encode_position, SensoryMap},
};
use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, SimConfig, Simulator};

use crate::checkpoint::Checkpoint;
use ablation::{AblationStats, AuditMetadata, RegionAblation};
use output_permutation::OutputPermutationStats;
use shuffle::ShuffleStats;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
pub struct Suite {
    pub suite_version: u32,
    pub source: String,
    pub sections: BTreeMap<String, Vec<Position>>,
}

#[derive(Deserialize)]
pub struct Position {
    pub fen: String,
    pub bestmove: String,
}

#[derive(Serialize)]
pub struct Trial {
    pub section: String,
    pub fen: String,
    pub bestmove: String,
    pub intact_move: String,
    pub reset_move: String,
    pub intact_top3: bool,
    pub reset_top3: bool,
    pub readout_max_abs_difference: f32,
}

#[derive(Debug, Clone)]
pub struct CausalControlConfig {
    pub shuffle_seed: u64,
    pub output_permutation_seed: u64,
    pub bypass_seed: u64,
    pub ablation_region: String,
}

#[derive(Serialize)]
pub struct CausalControlStats {
    pub shuffled_graph: ShuffleStats,
    pub region_ablation: AblationStats,
    pub output_permutation: OutputPermutationStats,
    pub weight_reset_delta_edges_removed: usize,
    pub brain_bypass: BrainBypassStats,
}

#[derive(Serialize)]
pub struct BrainBypassStats {
    pub seed: u64,
    pub inputs: &'static str,
    pub outputs: usize,
    pub learning: bool,
}

#[derive(Serialize)]
pub struct CausalTrial {
    pub section: String,
    pub fen: String,
    pub bestmove: String,
    pub intact_move: String,
    pub reset_move: String,
    pub shuffled_move: String,
    pub ablated_move: String,
    pub output_permuted_move: String,
    pub brain_bypass_move: String,
    pub intact_top3: bool,
    pub reset_top3: bool,
    pub shuffled_top3: bool,
    pub ablated_top3: bool,
    pub output_permuted_top3: bool,
    pub brain_bypass_top3: bool,
    pub reset_readout_max_abs_difference: f32,
    pub shuffled_readout_max_abs_difference: f32,
    pub ablated_readout_max_abs_difference: f32,
}

fn top3(decision: &MoveDecision, bestmove: &str) -> bool {
    decision
        .legal_scores
        .iter()
        .take(3)
        .any(|(uci, _)| uci == bestmove)
}

fn readout_difference(left: &MoveDecision, right: &MoveDecision) -> f32 {
    left.from_rates
        .iter()
        .chain(&left.to_rates)
        .chain(&left.promotion_rates)
        .zip(
            right
                .from_rates
                .iter()
                .chain(&right.to_rates)
                .chain(&right.promotion_rates),
        )
        .map(|(left, right)| (left - right).abs())
        .fold(0.0, f32::max)
}

/// Validate before applying: silently skipped deltas or mismatched maps
/// would turn a purported checkpoint comparison into a different experiment.
pub fn validate_checkpoint(
    checkpoint: &Checkpoint,
    graph: &Connectome,
    sensory: &SensoryMap,
    output: &OutputMap,
) -> Result<()> {
    if checkpoint.format_version != 1
        || checkpoint.graph_neurons_sha256 != graph.manifest.neurons_sha256
        || checkpoint.sensory_map_sha256 != sensory.sha256()
        || checkpoint.output_map_sha256 != output.sha256()
    {
        return Err("checkpoint version or graph/map identity mismatch".into());
    }
    let mut seen = HashSet::new();
    for &(index, weight) in &checkpoint.deltas {
        let base = graph
            .edge_weight
            .get(index as usize)
            .ok_or("checkpoint edge out of range")?;
        if !seen.insert(index)
            || !weight.is_finite()
            || (weight != 0.0
                && (base == &0.0 || weight.is_sign_positive() != base.is_sign_positive()))
        {
            return Err(
                "checkpoint contains duplicate, nonfinite, or sign-changing weights".into(),
            );
        }
    }
    Ok(())
}

pub fn evaluate(
    graph: &Connectome,
    sensory: &SensoryMap,
    output: &OutputMap,
    checkpoint: &Checkpoint,
    suite: &Suite,
    config: SimConfig,
) -> Result<Vec<Trial>> {
    let validation = graph.validate()?;
    if !validation.is_ok()
        || graph.offsets.first() != Some(&0)
        || graph.neurons.len() != graph.manifest.neuron_count as usize
        || graph.edge_src.len() as u64 != graph.manifest.edge_count
        || graph.edge_weight.iter().any(|w| !w.is_finite())
    {
        return Err("invalid graph".into());
    }
    validate_checkpoint(checkpoint, graph, sensory, output)?;
    if suite.suite_version != 1
        || suite.sections.values().all(Vec::is_empty)
        || config.settle_steps == 0
    {
        return Err("unsupported/empty suite or zero settle steps".into());
    }
    let indices = sensory
        .board_populations
        .iter()
        .flatten()
        .chain(std::iter::once(&sensory.context.side_to_move))
        .chain(sensory.context.castling_rights.iter())
        .chain(sensory.context.en_passant_file.iter())
        .chain(output.from_groups.iter().flatten())
        .chain(output.to_groups.iter().flatten())
        .chain(output.promotion_groups.values().flatten());
    if indices
        .into_iter()
        .any(|&n| n as usize >= graph.neurons.len())
    {
        return Err("map neuron out of range".into());
    }
    let mut intact = CpuSimulator::new(graph, config);
    checkpoint.apply_to(intact.weights_mut());
    let mut reset = CpuSimulator::new(graph, config);
    let mut trials = Vec::new();
    for (section, positions) in &suite.sections {
        for position in positions {
            let stimulus = encode_position(&position.fen, sensory, graph.neurons.len())?;
            // Fail the entire audit on invalid targets rather than quietly
            // dropping cases and changing the denominator.
            let legal = choose_move(&position.fen, &reset.state().rate, output)?;
            if !legal
                .legal_scores
                .iter()
                .any(|(uci, _)| uci == &position.bestmove)
            {
                return Err(format!(
                    "illegal target {} in {section}: {}",
                    position.bestmove, position.fen
                )
                .into());
            }
            intact.reset();
            reset.reset();
            for _ in 0..config.settle_steps {
                intact.step(&stimulus);
                reset.step(&stimulus);
            }
            let a = choose_move(&position.fen, &intact.state().rate, output)?;
            let b = choose_move(&position.fen, &reset.state().rate, output)?;
            let difference = a
                .from_rates
                .iter()
                .chain(&a.to_rates)
                .chain(&a.promotion_rates)
                .zip(
                    b.from_rates
                        .iter()
                        .chain(&b.to_rates)
                        .chain(&b.promotion_rates),
                )
                .map(|(x, y)| (x - y).abs())
                .fold(0.0, f32::max);
            trials.push(Trial {
                section: section.clone(),
                fen: position.fen.clone(),
                bestmove: position.bestmove.clone(),
                intact_top3: a
                    .legal_scores
                    .iter()
                    .take(3)
                    .any(|(uci, _)| uci == &position.bestmove),
                reset_top3: b
                    .legal_scores
                    .iter()
                    .take(3)
                    .any(|(uci, _)| uci == &position.bestmove),
                intact_move: a.uci,
                reset_move: b.uci,
                readout_max_abs_difference: difference,
            });
        }
    }
    Ok(trials)
}

pub fn evaluate_causal_controls(
    graph: &Connectome,
    metadata: &AuditMetadata,
    sensory: &SensoryMap,
    output: &OutputMap,
    checkpoint: &Checkpoint,
    suite: &Suite,
    config: SimConfig,
    controls: &CausalControlConfig,
) -> Result<(Vec<CausalTrial>, CausalControlStats)> {
    let validation = graph.validate()?;
    if !validation.is_ok()
        || graph.offsets.first() != Some(&0)
        || graph.neurons.len() != graph.manifest.neuron_count as usize
        || graph.edge_src.len() as u64 != graph.manifest.edge_count
        || graph.edge_weight.iter().any(|weight| !weight.is_finite())
    {
        return Err("invalid graph".into());
    }
    validate_checkpoint(checkpoint, graph, sensory, output)?;
    metadata.validate(graph)?;
    if suite.suite_version != 1
        || suite.sections.values().all(Vec::is_empty)
        || config.settle_steps == 0
    {
        return Err("unsupported/empty suite or zero settle steps".into());
    }

    let mapped_indices = sensory
        .board_populations
        .iter()
        .flatten()
        .chain(std::iter::once(&sensory.context.side_to_move))
        .chain(sensory.context.castling_rights.iter())
        .chain(sensory.context.en_passant_file.iter())
        .chain(output.from_groups.iter().flatten())
        .chain(output.to_groups.iter().flatten())
        .chain(output.promotion_groups.values().flatten());
    if mapped_indices
        .into_iter()
        .any(|&index| index as usize >= graph.neurons.len())
    {
        return Err("map neuron out of range".into());
    }

    let reset_checkpoint = reset::checkpoint_without_learning(checkpoint);
    let (shuffled_graph, shuffled_graph_stats) =
        shuffle::degree_aware(graph, metadata, controls.shuffle_seed)?;
    let ablation = RegionAblation::new(graph, metadata, &controls.ablation_region)?;
    let (permuted_output, output_permutation_stats) =
        output_permutation::permute(output, controls.output_permutation_seed)?;

    let mut intact = CpuSimulator::new(graph, config);
    checkpoint.apply_to(intact.weights_mut());
    let mut reset = CpuSimulator::new(graph, config);
    reset_checkpoint.apply_to(reset.weights_mut());
    let mut shuffled = CpuSimulator::new(&shuffled_graph, config);
    checkpoint.apply_to(shuffled.weights_mut());
    let mut ablated = CpuSimulator::new(graph, config);
    checkpoint.apply_to(ablated.weights_mut());

    let mut trials = Vec::new();
    for (section, positions) in &suite.sections {
        for position in positions {
            let stimulus = encode_position(&position.fen, sensory, graph.neurons.len())?;
            let target_validation =
                choose_move(&position.fen, reset.state().rate.as_slice(), output)?;
            if !target_validation
                .legal_scores
                .iter()
                .any(|(uci, _)| uci == &position.bestmove)
            {
                return Err(format!(
                    "illegal target {} in {section}: {}",
                    position.bestmove, position.fen
                )
                .into());
            }

            intact.reset();
            reset.reset();
            shuffled.reset();
            ablated.reset();
            for _ in 0..config.settle_steps {
                intact.step(&stimulus);
                reset.step(&stimulus);
                shuffled.step(&stimulus);
                ablation.step(&mut ablated, &stimulus);
            }

            let (intact_decision, output_permuted_decision) = output_permutation::paired_decisions(
                &position.fen,
                &intact.state().rate,
                output,
                &permuted_output,
            )?;
            let reset_decision = choose_move(&position.fen, &reset.state().rate, output)?;
            let shuffled_decision = choose_move(&position.fen, &shuffled.state().rate, output)?;
            let ablated_decision = choose_move(&position.fen, &ablated.state().rate, output)?;
            let bypass_rates = bypass::random_readout_rates(
                &stimulus,
                output,
                graph.neurons.len(),
                controls.bypass_seed,
            );
            let bypass_decision = choose_move(&position.fen, &bypass_rates, output)?;

            trials.push(CausalTrial {
                section: section.clone(),
                fen: position.fen.clone(),
                bestmove: position.bestmove.clone(),
                intact_move: intact_decision.uci.clone(),
                reset_move: reset_decision.uci.clone(),
                shuffled_move: shuffled_decision.uci.clone(),
                ablated_move: ablated_decision.uci.clone(),
                output_permuted_move: output_permuted_decision.uci.clone(),
                brain_bypass_move: bypass_decision.uci.clone(),
                intact_top3: top3(&intact_decision, &position.bestmove),
                reset_top3: top3(&reset_decision, &position.bestmove),
                shuffled_top3: top3(&shuffled_decision, &position.bestmove),
                ablated_top3: top3(&ablated_decision, &position.bestmove),
                output_permuted_top3: top3(&output_permuted_decision, &position.bestmove),
                brain_bypass_top3: top3(&bypass_decision, &position.bestmove),
                reset_readout_max_abs_difference: readout_difference(
                    &intact_decision,
                    &reset_decision,
                ),
                shuffled_readout_max_abs_difference: readout_difference(
                    &intact_decision,
                    &shuffled_decision,
                ),
                ablated_readout_max_abs_difference: readout_difference(
                    &intact_decision,
                    &ablated_decision,
                ),
            });
        }
    }

    let stats = CausalControlStats {
        shuffled_graph: shuffled_graph_stats,
        region_ablation: ablation.stats().clone(),
        output_permutation: output_permutation_stats,
        weight_reset_delta_edges_removed: checkpoint.deltas.len(),
        brain_bypass: BrainBypassStats {
            seed: controls.bypass_seed,
            inputs: "fixed encoded sensory vector",
            outputs: 132,
            learning: false,
        },
    };
    Ok((trials, stats))
}

pub fn run_cli(args: impl Iterator<Item = String>) -> Result<String> {
    let mut args = args;
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--model",
            "--suite",
            "--graph",
            "--chess-dir",
            "--settle-steps",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unknown argument: {key}").into());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?;
        if options.insert(key.clone(), value).is_some() {
            return Err(format!("duplicate argument: {key}").into());
        }
    }
    let model = options.get("--model").ok_or("--model is required")?;
    let suite_path = options.get("--suite").ok_or("--suite is required")?;
    let graph_path = options
        .get("--graph")
        .map(String::as_str)
        .unwrap_or("data/compiled/malecns-v1");
    let chess = options
        .get("--chess-dir")
        .map(String::as_str)
        .unwrap_or("crates/stockfly-chess/resources");
    let checkpoint: Checkpoint = serde_json::from_slice(&fs::read(model)?)?;
    let suite: Suite = serde_json::from_slice(&fs::read(suite_path)?)?;
    let graph = Connectome::open(graph_path)?;
    let sensory = SensoryMap::load_str(&fs::read_to_string(format!("{chess}/sensory-map.json"))?)?;
    let output = OutputMap::load_str(&fs::read_to_string(format!("{chess}/output-map.json"))?)?;
    // Default to production inference's calibration, not the training preset.
    let mut config = SimConfig::default();
    if let Some(steps) = options.get("--settle-steps") {
        config.settle_steps = steps.parse()?;
    }
    let trials = evaluate(&graph, &sensory, &output, &checkpoint, &suite, config)?;
    let summary = |rows: &[&Trial]| {
        serde_json::json!({
            "positions": rows.len(),
            "intact_top1": rows.iter().filter(|t| t.intact_move == t.bestmove).count(),
            "reset_top1": rows.iter().filter(|t| t.reset_move == t.bestmove).count(),
            "intact_top3": rows.iter().filter(|t| t.intact_top3).count(),
            "reset_top3": rows.iter().filter(|t| t.reset_top3).count(),
            "changed_moves": rows.iter().filter(|t| t.intact_move != t.reset_move).count(),
            "changed_readouts": rows.iter().filter(|t| t.readout_max_abs_difference > 0.0).count(),
        })
    };
    let sections: BTreeMap<_, _> = suite
        .sections
        .keys()
        .map(|section| {
            let rows: Vec<_> = trials.iter().filter(|t| &t.section == section).collect();
            (section, summary(&rows))
        })
        .collect();
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "report_version": 1, "control": "weight-reset", "model_kind": checkpoint.model_kind,
        "suite_source": suite.source, "graph_neurons_sha256": graph.manifest.neurons_sha256,
        "sensory_map_sha256": sensory.sha256(), "output_map_sha256": output.sha256(),
        "checkpoint_delta_edges": checkpoint.deltas.len(),
        "calibration": { "dt_ms": config.dt_ms, "settle_steps": config.settle_steps,
            "threshold": config.threshold, "decay": config.decay,
            "weight_scale": config.weight_scale, "max_rate": config.max_rate },
        "summary": summary(&trials.iter().collect::<Vec<_>>()), "sections": sections, "trials": trials,
    }))?)
}

fn causal_summary(rows: &[&CausalTrial]) -> serde_json::Value {
    let metric = |moves: fn(&CausalTrial) -> &str, top3: fn(&CausalTrial) -> bool| {
        serde_json::json!({
            "top1": rows.iter().filter(|trial| moves(trial) == trial.bestmove).count(),
            "top3": rows.iter().filter(|trial| top3(trial)).count(),
            "changed_moves_vs_intact": rows.iter()
                .filter(|trial| moves(trial) != trial.intact_move)
                .count(),
        })
    };
    serde_json::json!({
        "positions": rows.len(),
        "intact": metric(|trial| &trial.intact_move, |trial| trial.intact_top3),
        "weight_reset": metric(|trial| &trial.reset_move, |trial| trial.reset_top3),
        "shuffled_graph": metric(|trial| &trial.shuffled_move, |trial| trial.shuffled_top3),
        "region_ablation": metric(|trial| &trial.ablated_move, |trial| trial.ablated_top3),
        "output_permutation": metric(
            |trial| &trial.output_permuted_move,
            |trial| trial.output_permuted_top3,
        ),
        "brain_bypass": metric(|trial| &trial.brain_bypass_move, |trial| trial.brain_bypass_top3),
        "changed_readouts_vs_intact": {
            "weight_reset": rows.iter()
                .filter(|trial| trial.reset_readout_max_abs_difference > 0.0)
                .count(),
            "shuffled_graph": rows.iter()
                .filter(|trial| trial.shuffled_readout_max_abs_difference > 0.0)
                .count(),
            "region_ablation": rows.iter()
                .filter(|trial| trial.ablated_readout_max_abs_difference > 0.0)
                .count(),
        },
    })
}

pub fn run_causal_cli(args: impl Iterator<Item = String>) -> Result<String> {
    let mut args = args;
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--model",
            "--suite",
            "--graph",
            "--chess-dir",
            "--metadata",
            "--settle-steps",
            "--shuffle-seed",
            "--permutation-seed",
            "--bypass-seed",
            "--ablate-region",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unknown argument: {key}").into());
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?;
        if options.insert(key.clone(), value).is_some() {
            return Err(format!("duplicate argument: {key}").into());
        }
    }
    let model = options.get("--model").ok_or("--model is required")?;
    let suite_path = options.get("--suite").ok_or("--suite is required")?;
    let metadata_path = options.get("--metadata").ok_or("--metadata is required")?;
    let graph_path = options
        .get("--graph")
        .map(String::as_str)
        .unwrap_or("data/compiled/malecns-v1");
    let chess = options
        .get("--chess-dir")
        .map(String::as_str)
        .unwrap_or("crates/stockfly-chess/resources");
    let input_hashes = hashes::input_hashes(
        graph_path,
        model,
        suite_path,
        chess,
        metadata_path,
        std::env::current_exe()?,
    )?;
    let checkpoint: Checkpoint = serde_json::from_slice(&fs::read(model)?)?;
    let suite: Suite = serde_json::from_slice(&fs::read(suite_path)?)?;
    let metadata: AuditMetadata = serde_json::from_slice(&fs::read(metadata_path)?)?;
    let graph = Connectome::open(graph_path)?;
    let sensory = SensoryMap::load_str(&fs::read_to_string(format!("{chess}/sensory-map.json"))?)?;
    let output = OutputMap::load_str(&fs::read_to_string(format!("{chess}/output-map.json"))?)?;
    let mut config = SimConfig::default();
    if let Some(steps) = options.get("--settle-steps") {
        config.settle_steps = steps.parse()?;
    }
    let controls = CausalControlConfig {
        shuffle_seed: options
            .get("--shuffle-seed")
            .map(|v| v.parse())
            .transpose()?
            .unwrap_or(0x5348_5546_464C_4501),
        output_permutation_seed: options
            .get("--permutation-seed")
            .map(|v| v.parse())
            .transpose()?
            .unwrap_or(0x5045_524D_5554_4501),
        bypass_seed: options
            .get("--bypass-seed")
            .map(|v| v.parse())
            .transpose()?
            .unwrap_or(0x4259_5041_5353_0001),
        ablation_region: options
            .get("--ablate-region")
            .cloned()
            .unwrap_or_else(|| "descending_neuron".into()),
    };
    let (trials, control_stats) = evaluate_causal_controls(
        &graph,
        &metadata,
        &sensory,
        &output,
        &checkpoint,
        &suite,
        config,
        &controls,
    )?;
    let sections: BTreeMap<_, _> = suite
        .sections
        .keys()
        .map(|section| {
            let rows: Vec<_> = trials
                .iter()
                .filter(|trial| &trial.section == section)
                .collect();
            (section, causal_summary(&rows))
        })
        .collect();
    let all_rows: Vec<_> = trials.iter().collect();
    let after_hashes = hashes::input_hashes(
        graph_path,
        model,
        suite_path,
        chess,
        metadata_path,
        std::env::current_exe()?,
    )?;
    if input_hashes != after_hashes {
        return Err("audit inputs changed during evaluation; refusing to emit report".into());
    }
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "report_version": 1,
        "control_set": "causal-controls-v1",
        "model_kind": checkpoint.model_kind,
        "suite_source": suite.source,
        "graph": {
            "dataset": graph.manifest.dataset,
            "neurons_sha256": graph.manifest.neurons_sha256,
            "neuron_count": graph.neurons.len(),
            "edge_count": graph.edge_src.len(),
        },
        "maps": {
            "sensory_sha256": sensory.sha256(),
            "output_sha256": output.sha256(),
        },
        "metadata_source": metadata.source,
        "inputs": input_hashes,
        "checkpoint_delta_edges": checkpoint.deltas.len(),
        "calibration": {
            "dt_ms": config.dt_ms,
            "settle_steps": config.settle_steps,
            "threshold": config.threshold,
            "decay": config.decay,
            "weight_scale": config.weight_scale,
            "max_rate": config.max_rate,
        },
        "controls": control_stats,
        "summary": causal_summary(&all_rows),
        "sections": sections,
        "trials": trials,
        "interpretation": "Measured deltas only; this report does not assign a causal pass/fail verdict.",
        "limitations": [
            "Teacher move agreement is neither Elo nor proof of playing strength.",
            "A supplied suite may overlap training data; the evaluator does not label it held-out.",
            "The shuffled graph preserves exact fan-in counts and the global out-degree distribution; individual source degree is matched only within logarithmic bins.",
            "Population ablation measures only the selected MaleCNS superclass annotation value; it is not anatomical-region ranking.",
            "The brain bypass is a fixed random sensory readout baseline and is not trained.",
            "Version-1 checkpoints do not bind edge-block hashes or calibration; this report hashes the actual evaluated files and records calibration separately.",
            "Trace replay and Elo measurement are separate audit tasks and are not evaluated here."
        ],
    }))?)
}
