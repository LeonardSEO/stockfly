use std::env;
use std::fs;
use std::process::ExitCode;
use std::time::Instant;

use stockfly_chess::output_map::OutputMap;
use stockfly_chess::policy::choose_move;
use stockfly_chess::sensory::{encode_position, SensoryMap};
use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, SimConfig, Simulator};

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let command = match args.next() {
        Some(c) => c,
        None => {
            eprintln!("usage: stockfly-train <command> [args]");
            eprintln!("commands: infer-untrained --fen <fen|startpos> [--graph <dir>] [--chess-dir <dir>]");
            return ExitCode::FAILURE;
        }
    };

    match command.as_str() {
        "infer-untrained" => cmd_infer_untrained(args),
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
