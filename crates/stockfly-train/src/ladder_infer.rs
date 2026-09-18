//! Persistent, FEN-only move service. No opponent scores, PVs, or candidates.
use crate::{
    audit::{self, ablation::AuditMetadata, hashes},
    checkpoint::Checkpoint,
    presets,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufRead, Write},
};
use stockfly_chess::{
    output_map::OutputMap,
    policy::choose_move,
    sensory::{encode_position, SensoryMap},
};
use stockfly_connectome::Connectome;
use stockfly_sim::{GpuAdapterOptions, GpuSimulator, SimConfig};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub fen: String,
}

pub fn run_cli(mut args: impl Iterator<Item = String>) -> Result<()> {
    let mut options = BTreeMap::new();
    while let Some(key) = args.next() {
        if ![
            "--model",
            "--graph",
            "--chess-dir",
            "--metadata",
            "--control",
            "--seed",
            "--protocol",
            "--settle-steps",
        ]
        .contains(&key.as_str())
            || options.contains_key(&key)
        {
            return Err(format!("unknown/duplicate option {key}").into());
        }
        options.insert(key, args.next().ok_or("missing option value")?);
    }
    let get =
        |key: &str, fallback: &str| options.get(key).cloned().unwrap_or_else(|| fallback.into());
    let model = options.get("--model").ok_or("--model required")?;
    let graph_dir = get("--graph", "data/compiled/malecns-v1");
    let chess_dir = get("--chess-dir", "crates/stockfly-chess/resources");
    let metadata_path = get("--metadata", "data/reports/malecns-v1-audit-metadata.json");
    let protocol = get(
        "--protocol",
        "crates/stockfly-train/resources/ladder.json",
    );
    let control = get("--control", "intact");
    if ![
        "intact",
        "reset",
        "shuffled",
        "output-permuted",
        "brain-bypass",
    ]
    .contains(&control.as_str())
    {
        return Err("invalid control".into());
    }
    let seed: u64 = get("--seed", "42").parse()?;
    let identity = hashes::input_hashes(
        &graph_dir,
        model,
        protocol,
        &chess_dir,
        &metadata_path,
        std::env::current_exe()?,
    )?;
    let graph = Connectome::open(&graph_dir)?;
    let checkpoint = Checkpoint::load(model)?;
    let sensory = SensoryMap::load_str(&fs::read_to_string(format!(
        "{chess_dir}/sensory-map.json"
    ))?)?;
    let output = OutputMap::load_str(&fs::read_to_string(format!("{chess_dir}/output-map.json"))?)?;
    audit::validate_checkpoint(&checkpoint, &graph, &sensory, &output)?;
    let preset = presets::lookup(&checkpoint.preset).ok_or("unknown checkpoint preset")?;
    let config = SimConfig {
        settle_steps: get("--settle-steps", "16").parse()?,
        ..SimConfig::default()
    };
    if config.settle_steps == 0 {
        return Err("settle steps must be positive".into());
    }
    let mut control_stats = json!({});
    let graph = if control == "shuffled" {
        let metadata: AuditMetadata = serde_json::from_slice(&fs::read(metadata_path)?)?;
        let (graph, stats) = audit::shuffle::degree_aware(&graph, &metadata, seed)?;
        control_stats = serde_json::to_value(stats)?;
        graph
    } else {
        graph
    };
    let output = if control == "output-permuted" {
        let (output, stats) = audit::output_permutation::permute(&output, seed)?;
        control_stats = serde_json::to_value(stats)?;
        output
    } else {
        output
    };
    let mut weights: Vec<f32> = graph
        .edge_weight
        .iter()
        .map(|w| w * config.weight_scale)
        .collect();
    if control != "reset" {
        checkpoint.apply_to(&mut weights);
    }
    let mut gpu = if control == "brain-bypass" {
        None
    } else {
        Some(GpuSimulator::new_with_weights(
            &graph,
            config,
            GpuAdapterOptions::default(),
            &weights,
        )?)
    };
    let backend = gpu
        .as_ref()
        .map(|g| json!({"backend": g.backend(), "adapter": g.adapter_name()}))
        .unwrap_or(json!({"backend":"fixed-random-linear-readout-cpu"}));
    let identity = json!({"hashes":identity,"backend":backend,"control":control,"seed":seed,"control_stats":control_stats,
        "calibration":{"settle_steps":config.settle_steps,"weight_scale":config.weight_scale,"threshold":config.threshold,"decay":config.decay,"max_rate":config.max_rate,"dt_ms":config.dt_ms},
        "model_kind":checkpoint.model_kind,"preset":checkpoint.preset,"training_preset_settle_steps":preset.settle_steps,"trials_run":checkpoint.trials_run});
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{}", json!({"ready":true,"identity":identity}))?;
    stdout.flush()?;
    for line in io::stdin().lock().lines() {
        let result = (|| -> Result<String> {
            let request: Request = serde_json::from_str(&line?)?;
            let stimulus = encode_position(&request.fen, &sensory, graph.neurons.len())?;
            let rates = if let Some(gpu) = gpu.as_mut() {
                gpu.reset_state();
                gpu.run_steps(&stimulus, config.settle_steps)?;
                &gpu.state().rate
            } else {
                &audit::bypass::random_readout_rates(&stimulus, &output, graph.neurons.len(), seed)
            };
            Ok(choose_move(&request.fen, rates, &output)?.uci)
        })();
        match result {
            Ok(uci) => writeln!(stdout, "{}", json!({"move":uci}))?,
            Err(error) => writeln!(stdout, "{}", json!({"error":error.to_string()}))?,
        }
        stdout.flush()?;
    }
    Ok(())
}
