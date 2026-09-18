//! Paired, frozen-checkpoint evaluation. Both conditions start each FEN
//! with zero state and identical calibration; reset omits learned weights.
use std::{
    collections::{BTreeMap, HashSet},
    fs,
};

use serde::{Deserialize, Serialize};
use stockfly_chess::{
    output_map::OutputMap,
    policy::choose_move,
    sensory::{encode_position, SensoryMap},
};
use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, SimConfig, Simulator};

use crate::checkpoint::Checkpoint;

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
