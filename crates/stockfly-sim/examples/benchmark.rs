use std::env;
use std::process;
use std::time::{Duration, Instant};

use serde::Serialize;
use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, GpuAdapterOptions, GpuSimulator, SimConfig, Simulator, Stimulus};

#[derive(Clone, Copy, PartialEq, Eq)]
enum BackendChoice {
    Auto,
    Cpu,
    Gpu,
}

#[derive(Serialize)]
struct BenchmarkReport {
    adapter: String,
    backend: String,
    neuron_count: usize,
    edge_count: usize,
    steps: u32,
    elapsed_ms: f64,
    peak_buffer_bytes: u64,
    steps_per_second: f64,
    edge_chunk_count: usize,
    max_bound_edge_buffer_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_reason: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("benchmark failed: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (path, steps, backend_choice) = parse_args()?;
    let connectome =
        Connectome::open(&path).map_err(|error| format!("open compiled graph: {error}"))?;
    let neuron_count = connectome.neurons.len();
    let edge_count = connectome.edge_src.len();
    let mut stimulus = Stimulus::zeroed(neuron_count);
    if let Some(first) = stimulus.values.first_mut() {
        *first = 1.0;
    }

    let report = match backend_choice {
        BackendChoice::Cpu => run_cpu(&connectome, &stimulus, steps, None),
        BackendChoice::Gpu | BackendChoice::Auto => {
            match GpuSimulator::new(
                &connectome,
                SimConfig::default(),
                GpuAdapterOptions::default(),
            ) {
                Ok(mut simulator) => {
                    let adapter = simulator.adapter_name().to_string();
                    let backend = simulator.backend().to_string();
                    let peak_buffer_bytes = simulator.peak_buffer_bytes();
                    let edge_chunk_count = simulator.edge_chunk_count();
                    let max_bound_edge_buffer_bytes = simulator.max_bound_edge_buffer_bytes();
                    let started = Instant::now();
                    match simulator.run_steps(&stimulus, steps) {
                        Ok(_) => make_report(
                            adapter,
                            backend,
                            neuron_count,
                            edge_count,
                            steps,
                            started.elapsed(),
                            peak_buffer_bytes,
                            edge_chunk_count,
                            max_bound_edge_buffer_bytes,
                            None,
                        ),
                        Err(error) if backend_choice == BackendChoice::Auto => {
                            run_cpu(&connectome, &stimulus, steps, Some(error.to_string()))
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                }
                Err(error) if backend_choice == BackendChoice::Auto => {
                    run_cpu(&connectome, &stimulus, steps, Some(error.to_string()))
                }
                Err(error) => return Err(error.to_string()),
            }
        }
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn run_cpu(
    connectome: &Connectome,
    stimulus: &Stimulus,
    steps: u32,
    fallback_reason: Option<String>,
) -> BenchmarkReport {
    let mut simulator = CpuSimulator::new(connectome, SimConfig::default());
    let started = Instant::now();
    for _ in 0..steps {
        simulator.step(stimulus);
    }
    let backend = if fallback_reason.is_some() {
        "cpu-fallback"
    } else {
        "cpu"
    };
    make_report(
        "CPU".to_string(),
        backend.to_string(),
        connectome.neurons.len(),
        connectome.edge_src.len(),
        steps,
        started.elapsed(),
        0,
        0,
        0,
        fallback_reason,
    )
}

#[allow(clippy::too_many_arguments)]
fn make_report(
    adapter: String,
    backend: String,
    neuron_count: usize,
    edge_count: usize,
    steps: u32,
    elapsed: Duration,
    peak_buffer_bytes: u64,
    edge_chunk_count: usize,
    max_bound_edge_buffer_bytes: u64,
    fallback_reason: Option<String>,
) -> BenchmarkReport {
    let elapsed_seconds = elapsed.as_secs_f64();
    BenchmarkReport {
        adapter,
        backend,
        neuron_count,
        edge_count,
        steps,
        elapsed_ms: elapsed_seconds * 1000.0,
        peak_buffer_bytes,
        steps_per_second: if elapsed_seconds > 0.0 {
            steps as f64 / elapsed_seconds
        } else {
            0.0
        },
        edge_chunk_count,
        max_bound_edge_buffer_bytes,
        fallback_reason,
    }
}

fn parse_args() -> Result<(String, u32, BackendChoice), String> {
    let mut path = None;
    let mut positional_steps = None;
    let mut flag_steps = None;
    let mut backend = BackendChoice::Auto;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--steps" => {
                let value = args.next().ok_or("--steps requires a value")?;
                flag_steps = Some(value.parse::<u32>().map_err(|_| "--steps must be a u32")?);
            }
            "--backend" => {
                backend = match args
                    .next()
                    .ok_or("--backend requires cpu, gpu, or auto")?
                    .as_str()
                {
                    "cpu" => BackendChoice::Cpu,
                    "gpu" => BackendChoice::Gpu,
                    "auto" => BackendChoice::Auto,
                    _ => return Err("--backend requires cpu, gpu, or auto".to_string()),
                };
            }
            value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
            value if path.is_none() => path = Some(value.to_string()),
            value if positional_steps.is_none() => {
                positional_steps = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| "positional steps must be a u32")?,
                );
            }
            value => return Err(format!("unexpected positional argument: {value}")),
        }
    }

    let path = path.ok_or(
        "usage: benchmark <compiled-graph-dir> [steps] [--steps N] [--backend auto|gpu|cpu]",
    )?;
    Ok((path, flag_steps.or(positional_steps).unwrap_or(16), backend))
}
