mod checkpoint;
mod presets;
mod train;

use std::env;
use std::fs;
use std::process::ExitCode;
use std::time::Instant;

use stockfly_chess::output_map::OutputMap;
use stockfly_chess::policy::choose_move;
use stockfly_chess::sensory::{encode_position, SensoryMap};
use stockfly_connectome::Connectome;
use stockfly_plasticity::{BioRule, MaxRule};
use stockfly_sim::{CpuSimulator, SimConfig, Simulator};

use checkpoint::Checkpoint;

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = match args.next() {
        Some(c) => c,
        None => {
            eprintln!("usage: stockfly-train <command> [args]");
            eprintln!("commands:");
            eprintln!("  infer-untrained --fen <fen|startpos> [--graph <dir>] [--chess-dir <dir>]");
            eprintln!("  train --kind <bio-full|max-full> --preset <smoke|quick|standard|overnight> --curriculum <path> --out <path.sfckpt>");
            return ExitCode::FAILURE;
        }
    };

    match command.as_str() {
        "infer-untrained" => cmd_infer_untrained(args),
        "train" => cmd_train(args),
        other => {
            eprintln!("unknown command: {other}");
            ExitCode::FAILURE
        }
    }
}

fn cmd_infer_untrained(args: impl Iterator<Item = String>) -> ExitCode {
    let mut fen = STARTPOS.to_string();
    let mut graph_dir = "data/compiled/malecns-v1".to_string();
    let mut chess_dir = "crates/stockfly-chess/resources".to_string();

    let args: Vec<String> = args.collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fen" => {
                i += 1;
                fen = args.get(i).cloned().unwrap_or_else(|| STARTPOS.to_string());
                if fen == "startpos" {
                    fen = STARTPOS.to_string();
                }
            }
            "--graph" => {
                i += 1;
                graph_dir = args.get(i).cloned().unwrap_or(graph_dir);
            }
            "--chess-dir" => {
                i += 1;
                chess_dir = args.get(i).cloned().unwrap_or(chess_dir);
            }
            other => {
                eprintln!("unknown arg: {other}");
                return ExitCode::FAILURE;
            }
        }
        i += 1;
    }

    let t_load = Instant::now();
    let connectome = match Connectome::open(&graph_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to open compiled graph at {graph_dir}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let neuron_count = connectome.neurons.len();

    let sensory_json = match fs::read_to_string(format!("{chess_dir}/sensory-map.json")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read sensory-map.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let sensory_map = match SensoryMap::load_str(&sensory_json) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("invalid sensory map: {e}");
            return ExitCode::FAILURE;
        }
    };

    let output_json = match fs::read_to_string(format!("{chess_dir}/output-map.json")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read output-map.json: {e}");
            return ExitCode::FAILURE;
        }
    };
    let output_map = match OutputMap::load_str(&output_json) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("invalid output map: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("load_time_ms: {}", t_load.elapsed().as_millis());

    let stimulus = match encode_position(&fen, &sensory_map, neuron_count) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to encode position: {e}");
            return ExitCode::FAILURE;
        }
    };

    let config = SimConfig::default();
    let mut sim = CpuSimulator::new(&connectome, config);

    let t_settle = Instant::now();
    for _ in 0..config.settle_steps {
        sim.step(&stimulus);
    }
    let settle_ms = t_settle.elapsed().as_millis();

    let brain_rates = &sim.state().rate;

    let decision = match choose_move(&fen, brain_rates, &output_map) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("failed to choose a move: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("dataset: {}", connectome.manifest.dataset);
    println!("neuron_count: {neuron_count}");
    println!("edge_count: {}", connectome.edge_src.len());
    println!("graph_neurons_sha256: {}", connectome.manifest.neurons_sha256);
    println!("sensory_map_sha256: {}", sensory_map.sha256());
    println!("output_map_sha256: {}", output_map.sha256());
    println!("fen: {fen}");
    println!("settle_steps: {}", config.settle_steps);
    println!("settle_time_ms: {settle_ms}");

    let mut top_from: Vec<(usize, f32)> = decision.from_rates.iter().copied().enumerate().collect();
    top_from.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let mut top_to: Vec<(usize, f32)> = decision.to_rates.iter().copied().enumerate().collect();
    top_to.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("top_from_squares: {:?}", &top_from[..3.min(top_from.len())]);
    println!("top_to_squares: {:?}", &top_to[..3.min(top_to.len())]);
    println!("legal_move_count: {}", decision.legal_scores.len());
    println!("selected_move: {}", decision.uci);

    ExitCode::SUCCESS
}

fn cmd_train(args: impl Iterator<Item = String>) -> ExitCode {
    let mut kind = "bio-full".to_string();
    let mut preset_name = "smoke".to_string();
    let mut curriculum_path = "data/teacher/smoke.jsonl".to_string();
    let mut out_path = "data/checkpoints/stockfly-bio-full.sfckpt".to_string();
    let mut graph_dir = "data/compiled/malecns-v1".to_string();
    let mut chess_dir = "crates/stockfly-chess/resources".to_string();
    let mut seed: u64 = 42;

    let args: Vec<String> = args.collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--kind" => { i += 1; kind = args.get(i).cloned().unwrap_or(kind); }
            "--preset" => { i += 1; preset_name = args.get(i).cloned().unwrap_or(preset_name); }
            "--curriculum" => { i += 1; curriculum_path = args.get(i).cloned().unwrap_or(curriculum_path); }
            "--out" => { i += 1; out_path = args.get(i).cloned().unwrap_or(out_path); }
            "--graph" => { i += 1; graph_dir = args.get(i).cloned().unwrap_or(graph_dir); }
            "--chess-dir" => { i += 1; chess_dir = args.get(i).cloned().unwrap_or(chess_dir); }
            "--seed" => { i += 1; seed = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(seed); }
            other => { eprintln!("unknown arg: {other}"); return ExitCode::FAILURE; }
        }
        i += 1;
    }

    let preset = match presets::lookup(&preset_name) {
        Some(p) => p,
        None => { eprintln!("unknown preset: {preset_name}"); return ExitCode::FAILURE; }
    };

    let connectome = match Connectome::open(&graph_dir) {
        Ok(c) => c,
        Err(e) => { eprintln!("failed to open compiled graph at {graph_dir}: {e}"); return ExitCode::FAILURE; }
    };
    let sensory_map = match fs::read_to_string(format!("{chess_dir}/sensory-map.json"))
        .map_err(|e| e.to_string())
        .and_then(|s| SensoryMap::load_str(&s).map_err(|e| e.to_string()))
    {
        Ok(m) => m,
        Err(e) => { eprintln!("failed to load sensory map: {e}"); return ExitCode::FAILURE; }
    };
    let output_map = match fs::read_to_string(format!("{chess_dir}/output-map.json"))
        .map_err(|e| e.to_string())
        .and_then(|s| OutputMap::load_str(&s).map_err(|e| e.to_string()))
    {
        Ok(m) => m,
        Err(e) => { eprintln!("failed to load output map: {e}"); return ExitCode::FAILURE; }
    };

    let examples = match train::load_curriculum(&curriculum_path) {
        Ok(e) if !e.is_empty() => e,
        Ok(_) => { eprintln!("curriculum file {curriculum_path} has no examples"); return ExitCode::FAILURE; }
        Err(e) => { eprintln!("failed to load curriculum {curriculum_path}: {e}"); return ExitCode::FAILURE; }
    };
    println!("loaded {} curriculum examples from {curriculum_path}", examples.len());

    let edge_dst = train::edge_dst_lookup(&connectome);

    let bio_eligible_edges: Option<Vec<u32>> = if kind == "bio-full" {
        match fs::read(format!("{graph_dir}/bio_mask.bin")) {
            Ok(mask_bytes) => Some(
                mask_bytes
                    .iter()
                    .enumerate()
                    .filter(|(_, &b)| b != 0)
                    .map(|(i, _)| i as u32)
                    .collect(),
            ),
            Err(e) => { eprintln!("failed to load bio_mask.bin from {graph_dir}: {e}"); return ExitCode::FAILURE; }
        }
    } else {
        None
    };
    if let Some(edges) = &bio_eligible_edges {
        println!("bio-eligible edges: {}", edges.len());
    }

    let sim_config = SimConfig { settle_steps: preset.settle_steps, ..SimConfig::default() };

    // Stage-0 baseline: identical trials, identical simulator path, plasticity disabled.
    let mut noop_rule = MaxRule { learning_rate: 0.0, max_multiplier: 1.0, activity_threshold: f32::INFINITY };
    let t0 = Instant::now();
    let baseline = train::train(
        &connectome, &edge_dst, &sensory_map, &output_map, &examples, &mut noop_rule,
        train::TrainOptions {
            sim_config,
            deadline: Instant::now() + preset.wall_clock_budget / 4,
            bio_eligible_edges: bio_eligible_edges.as_deref(),
            apply_plasticity: false,
        },
    );
    println!(
        "baseline (untrained): trials={} top1_accuracy={:.4} ({:.1}s)",
        baseline.trials_run, baseline.teacher_top1_accuracy, t0.elapsed().as_secs_f32()
    );

    let deadline = Instant::now() + preset.wall_clock_budget;
    let t1 = Instant::now();
    let (trained, model_kind_label): (train::TrainResult, &str) = if kind == "bio-full" {
        let mut rule = BioRule::default();
        let result = train::train(
            &connectome, &edge_dst, &sensory_map, &output_map, &examples, &mut rule,
            train::TrainOptions { sim_config, deadline, bio_eligible_edges: bio_eligible_edges.as_deref(), apply_plasticity: true },
        );
        (result, "bio-full")
    } else if kind == "max-full" {
        let mut rule = MaxRule::default();
        let result = train::train(
            &connectome, &edge_dst, &sensory_map, &output_map, &examples, &mut rule,
            train::TrainOptions { sim_config, deadline, bio_eligible_edges: None, apply_plasticity: true },
        );
        (result, "max-full")
    } else {
        eprintln!("unknown --kind: {kind} (expected bio-full or max-full)");
        return ExitCode::FAILURE;
    };
    println!(
        "trained ({model_kind_label}): trials={} top1_accuracy={:.4} ({:.1}s)",
        trained.trials_run, trained.teacher_top1_accuracy, t1.elapsed().as_secs_f32()
    );

    let checkpoint = Checkpoint::from_dense(
        model_kind_label,
        &connectome.manifest.neurons_sha256,
        sensory_map.sha256(),
        output_map.sha256(),
        &preset_name,
        seed,
        trained.trials_run,
        trained.teacher_top1_accuracy,
        &connectome
            .edge_weight
            .iter()
            .map(|w| w * sim_config.weight_scale)
            .collect::<Vec<f32>>(),
        &trained.trained_weights,
    );
    println!("checkpoint delta edges: {}", checkpoint.deltas.len());

    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = checkpoint.save(&out_path) {
        eprintln!("failed to save checkpoint to {out_path}: {e}");
        return ExitCode::FAILURE;
    }
    println!("saved checkpoint: {out_path}");

    ExitCode::SUCCESS
}
