//! Reproducible, all-neuron CPU/Metal comparison. Acceptance is embedded before measurement.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Write, path::Path, process::Command};
use stockfly_chess::{
    output_map::OutputMap,
    policy::{choose_move, MoveDecision},
    sensory::{encode_position, SensoryMap},
};
use stockfly_connectome::Connectome;
use stockfly_sim::{
    BrainState, CpuSimulator, GpuAdapterOptions, GpuSimulator, SimConfig, Simulator,
};
use stockfly_train::{
    audit::{hashes, validate_checkpoint},
    checkpoint::Checkpoint,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const PROTOCOL: &str = include_str!("../../resources/fullgraph-parity.json");
const PROTOCOL_PATH: &str = "crates/stockfly-train/resources/fullgraph-parity.json";
const GRAPH: &str = "data/compiled/malecns-v1";
const CHESS: &str = "crates/stockfly-chess/resources";

#[derive(Deserialize)]
struct Protocol {
    settle_steps: u32,
    tolerances: Tolerances,
    models: Vec<Model>,
    positions: Vec<Position>,
}
#[derive(Deserialize)]
struct Tolerances {
    activation_atol: f64,
    activation_rtol: f64,
    policy_atol: f64,
    policy_rtol: f64,
}
#[derive(Deserialize)]
struct Model {
    id: String,
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
struct Position {
    id: String,
    fen: String,
}

#[derive(Debug, Serialize)]
struct Difference {
    count: usize,
    failures: usize,
    nonfinite: usize,
    max_abs: f64,
    max_abs_index: usize,
    max_abs_cpu: f32,
    max_abs_gpu: f32,
    max_tolerance_ratio: f64,
    worst_ratio_index: usize,
    pass: bool,
}

fn compare(cpu: &[f32], gpu: &[f32], atol: f64, rtol: f64) -> Result<Difference> {
    if cpu.len() != gpu.len() {
        return Err("comparison length mismatch".into());
    }
    let mut result = Difference {
        count: cpu.len(),
        failures: 0,
        nonfinite: 0,
        max_abs: 0.0,
        max_abs_index: 0,
        max_abs_cpu: 0.0,
        max_abs_gpu: 0.0,
        max_tolerance_ratio: 0.0,
        worst_ratio_index: 0,
        pass: true,
    };
    for (index, (&left, &right)) in cpu.iter().zip(gpu).enumerate() {
        if !left.is_finite() || !right.is_finite() {
            result.nonfinite += 1;
            result.failures += 1;
            continue;
        }
        // Evaluate the acceptance rule in f64 so rounding the test cannot admit a failed f32 value.
        let error = (left as f64 - right as f64).abs();
        let allowed = atol + rtol * (left as f64).abs().max((right as f64).abs());
        let ratio = error / allowed;
        if error > result.max_abs {
            result.max_abs = error;
            result.max_abs_index = index;
            result.max_abs_cpu = left;
            result.max_abs_gpu = right;
        }
        if ratio > result.max_tolerance_ratio {
            result.max_tolerance_ratio = ratio;
            result.worst_ratio_index = index;
        }
        if error > allowed {
            result.failures += 1;
        }
    }
    result.pass = result.failures == 0;
    Ok(result)
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
fn state_digest(state: &BrainState) -> String {
    let mut hash = Sha256::new();
    for value in state.membrane.iter().chain(&state.rate) {
        hash.update(value.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn readout(decision: &MoveDecision) -> Vec<f32> {
    decision
        .from_rates
        .iter()
        .chain(&decision.to_rates)
        .chain(&decision.promotion_rates)
        .copied()
        .collect()
}
fn ordered_scores(cpu: &MoveDecision, gpu: &MoveDecision) -> Result<Vec<(String, f32, f32)>> {
    let cpu: BTreeMap<_, _> = cpu.legal_scores.iter().cloned().collect();
    let gpu: BTreeMap<_, _> = gpu.legal_scores.iter().cloned().collect();
    if cpu.keys().ne(gpu.keys()) {
        return Err("CPU/GPU legal move sets differ".into());
    }
    Ok(cpu
        .into_iter()
        .map(|(uci, left)| {
            let right = gpu[&uci];
            (uci, left, right)
        })
        .collect())
}
fn observation(
    step: u32,
    cpu: &BrainState,
    gpu: &BrainState,
    fen: &str,
    output: &OutputMap,
    tolerance: &Tolerances,
) -> Result<Value> {
    let membrane = compare(
        &cpu.membrane,
        &gpu.membrane,
        tolerance.activation_atol,
        tolerance.activation_rtol,
    )?;
    let rates = compare(
        &cpu.rate,
        &gpu.rate,
        tolerance.activation_atol,
        tolerance.activation_rtol,
    )?;
    let left = choose_move(fen, &cpu.rate, output)?;
    let right = choose_move(fen, &gpu.rate, output)?;
    let populations = compare(
        &readout(&left),
        &readout(&right),
        tolerance.policy_atol,
        tolerance.policy_rtol,
    )?;
    let scores = ordered_scores(&left, &right)?;
    let policy = compare(
        &scores.iter().map(|s| s.1).collect::<Vec<_>>(),
        &scores.iter().map(|s| s.2).collect::<Vec<_>>(),
        tolerance.policy_atol,
        tolerance.policy_rtol,
    )?;
    let move_identity = left.uci == right.uci;
    let pass = membrane.pass && rates.pass && populations.pass && policy.pass && move_identity;
    Ok(
        json!({"step":step,"pass":pass,"membrane":membrane,"rates":rates,"populations":populations,"legal_policy":policy,
        "cpu_move":left.uci,"gpu_move":right.uci,"move_identity":move_identity,"legal_scores_uci_cpu_gpu":scores,
        "cpu_state_sha256":state_digest(cpu),"gpu_state_sha256":state_digest(gpu)}),
    )
}
// Replay one GPU update from its own previous state. This separates local arithmetic
// contraction from recurrent amplification; it never changes acceptance or simulation.
fn rounding_diagnosis(
    graph: &Connectome,
    weights: &[f32],
    previous: &BrainState,
    actual: &BrainState,
    stimulus: &[f32],
    config: SimConfig,
) -> Value {
    let mut candidates: BTreeMap<&str, Vec<f32>> = [
        "separate_gather_separate_lif",
        "fused_gather_separate_lif",
        "separate_gather_fused_lif",
        "fused_gather_fused_lif",
    ]
    .into_iter()
    .map(|name| (name, Vec::with_capacity(graph.neurons.len())))
    .collect();
    for dst in 0..graph.neurons.len() {
        let mut separate = 0.0f32;
        let mut fused = 0.0f32;
        for edge in graph.offsets[dst] as usize..graph.offsets[dst + 1] as usize {
            let rate = previous.rate[graph.edge_src[edge] as usize];
            separate += weights[edge] * rate;
            fused = weights[edge].mul_add(rate, fused);
        }
        for (name, input, use_fma) in [
            ("separate_gather_separate_lif", separate, false),
            ("fused_gather_separate_lif", fused, false),
            ("separate_gather_fused_lif", separate, true),
            ("fused_gather_fused_lif", fused, true),
        ] {
            let membrane = if use_fma {
                previous.membrane[dst].mul_add(config.decay, input)
            } else {
                previous.membrane[dst] * config.decay + input
            } + stimulus[dst];
            candidates.get_mut(name).unwrap().push(membrane);
        }
    }
    let result: BTreeMap<_, _> = candidates
        .into_iter()
        .map(|(name, values)| {
            let different = values
                .iter()
                .zip(&actual.membrane)
                .filter(|(a, b)| a.to_bits() != b.to_bits())
                .count();
            let max_abs = values
                .iter()
                .zip(&actual.membrane)
                .map(|(&a, &b)| (a as f64 - b as f64).abs())
                .fold(0.0, f64::max);
            (
                name,
                json!({"bitwise_different_neurons":different,"max_abs_membrane":max_abs}),
            )
        })
        .collect();
    json!(result)
}
fn command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!("identity command failed: {program}").into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
fn embedded_sources() -> BTreeMap<&'static str, String> {
    // include_str binds these digests to the executable, not possibly edited files at run time.
    macro_rules! source {
        ($path:literal) => {
            ($path, digest(include_str!(concat!("../../../../", $path))))
        };
    }
    BTreeMap::from([
        source!("Cargo.toml"),
        source!("Cargo.lock"),
        source!("crates/stockfly-train/Cargo.toml"),
        source!("crates/stockfly-train/src/bin/fullgraph-parity.rs"),
        source!("crates/stockfly-train/src/checkpoint.rs"),
        source!("crates/stockfly-train/src/audit.rs"),
        source!("crates/stockfly-train/src/audit/hashes.rs"),
        source!("crates/stockfly-sim/src/cpu.rs"),
        source!("crates/stockfly-sim/src/gpu.rs"),
        source!("crates/stockfly-sim/src/gpu_buffers.rs"),
        source!("crates/stockfly-sim/src/state.rs"),
        source!("crates/stockfly-chess/src/sensory.rs"),
        source!("crates/stockfly-chess/src/output_map.rs"),
        source!("crates/stockfly-chess/src/policy.rs"),
        source!("crates/stockfly-connectome/src/lib.rs"),
        source!("shaders/csc_gather.wgsl"),
        source!("shaders/lif_step.wgsl"),
    ])
}
fn run(output_path: &Path) -> Result<bool> {
    if output_path.exists() {
        return Err("refusing to overwrite an existing measurement".into());
    }
    let protocol: Protocol = serde_json::from_str(PROTOCOL)?;
    if digest(fs::read(PROTOCOL_PATH)?) != digest(PROTOCOL) {
        return Err("on-disk protocol differs from embedded protocol; rebuild".into());
    }
    let graph = Connectome::open(GRAPH)?;
    let sensory = SensoryMap::load_str(&fs::read_to_string(format!("{CHESS}/sensory-map.json"))?)?;
    let output = OutputMap::load_str(&fs::read_to_string(format!("{CHESS}/output-map.json"))?)?;
    // Validate every FEN before any measurement; no cases can be silently omitted.
    for position in &protocol.positions {
        encode_position(&position.fen, &sensory, graph.neurons.len())?;
        choose_move(&position.fen, &vec![0.0; graph.neurons.len()], &output)?;
    }
    let started_at = command("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])?;
    let config = SimConfig {
        settle_steps: protocol.settle_steps,
        ..SimConfig::default()
    };
    let configuration = json!({"dt_ms":config.dt_ms,"settle_steps":config.settle_steps,"threshold":config.threshold,"decay":config.decay,
        "weight_scale":config.weight_scale,"max_rate":config.max_rate,"reset":"zero per position","steps":"all 1..=16 plus single-submission final GPU check"});
    let mut models = Vec::new();
    for model in &protocol.models {
        if hashes::sha256_file(&model.path)? != model.sha256 {
            return Err(format!("selected checkpoint hash mismatch: {}", model.id).into());
        }
        let checkpoint = Checkpoint::load(&model.path)?;
        validate_checkpoint(&checkpoint, &graph, &sensory, &output)?;
        let identity = hashes::input_hashes(
            GRAPH,
            &model.path,
            PROTOCOL_PATH,
            CHESS,
            "data/reports/malecns-v1-audit-metadata.json",
            std::env::current_exe()?,
        )?;
        let mut cpu = CpuSimulator::new(&graph, config);
        checkpoint.apply_to(cpu.weights_mut());
        let mut gpu = GpuSimulator::new_with_weights(
            &graph,
            config,
            GpuAdapterOptions::default(),
            cpu.weights(),
        )?;
        if gpu.backend() != "metal" || gpu.adapter_name() != "Apple M4" {
            return Err("this protocol requires actual Apple M4 Metal".into());
        }
        let backend = json!({"name":gpu.backend(),"adapter":gpu.adapter_name(),"edge_chunks":gpu.edge_chunk_count(),
            "max_bound_edge_buffer_bytes":gpu.max_bound_edge_buffer_bytes(),"peak_buffer_bytes":gpu.peak_buffer_bytes()});
        let mut positions = Vec::new();
        for position in &protocol.positions {
            cpu.reset();
            gpu.reset_state();
            let stimulus = encode_position(&position.fen, &sensory, graph.neurons.len())?;
            let mut steps = Vec::new();
            for step in 1..=protocol.settle_steps {
                let previous_gpu =
                    (position.id == protocol.positions[0].id).then(|| gpu.state().clone());
                cpu.step(&stimulus);
                gpu.run_steps(&stimulus, 1)?;
                let mut measured = observation(
                    step,
                    cpu.state(),
                    gpu.state(),
                    &position.fen,
                    &output,
                    &protocol.tolerances,
                )?;
                if let Some(previous) = previous_gpu {
                    measured["rounding_diagnosis"] = rounding_diagnosis(
                        &graph,
                        cpu.weights(),
                        &previous,
                        gpu.state(),
                        &stimulus.values,
                        config,
                    );
                }
                steps.push(measured);
            }
            let incremental_gpu_hash = state_digest(gpu.state());
            gpu.reset_state();
            gpu.run_steps(&stimulus, protocol.settle_steps)?;
            let batched = observation(
                protocol.settle_steps,
                cpu.state(),
                gpu.state(),
                &position.fen,
                &output,
                &protocol.tolerances,
            )?;
            let batch_identical = incremental_gpu_hash == state_digest(gpu.state());
            let pass = steps.iter().all(|s| s["pass"] == true)
                && batched["pass"] == true
                && batch_identical;
            let mut maxima = BTreeMap::new();
            for field in ["membrane", "rates", "populations", "legal_policy"] {
                maxima.insert(
                    field,
                    steps
                        .iter()
                        .chain(std::iter::once(&batched))
                        .map(|s| s[field]["max_abs"].as_f64().unwrap())
                        .fold(0.0, f64::max),
                );
            }
            let mismatches = steps.iter().filter(|s| s["move_identity"] == false).count();
            eprintln!(
                "{} {} {} max_rate={:.9} max_policy={:.9} move_mismatches={mismatches}",
                model.id,
                position.id,
                if pass { "PASS" } else { "FAIL" },
                maxima["rates"],
                maxima["legal_policy"]
            );
            positions.push(json!({"id":position.id,"fen":position.fen,"pass":pass,"max_abs":maxima,"move_mismatches":mismatches,
                "steps":steps,"batched_final":batched,"gpu_batched_state_bitwise_identical":batch_identical}));
        }
        let pass = positions.iter().all(|p| p["pass"] == true);
        models.push(json!({"id":model.id,"checkpoint_path":model.path,"identity":identity,"model_kind":checkpoint.model_kind,
            "preset":checkpoint.preset,"seed":checkpoint.rng_seed,"trials_run":checkpoint.trials_run,"backend":backend,"pass":pass,"positions":positions}));
    }
    let pass = models.iter().all(|m| m["pass"] == true);
    let sources = embedded_sources();
    let result = json!({"schema_version":1,"status":if pass {"PASS"} else {"FAIL"},"started_at":started_at,
        "completed_at":command("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])?,"protocol":serde_json::from_str::<Value>(PROTOCOL)?,
        "protocol_sha256":digest(PROTOCOL),"configuration":configuration,"configuration_sha256":digest(serde_json::to_vec(&configuration)?),
        "source_sha256":digest(serde_json::to_vec(&sources)?),"embedded_source_files":sources,
        "git_head_at_run":command("git", &["rev-parse","HEAD"])?,
        "machine":{"uname":command("uname", &["-srm"])?,"os":command("sw_vers", &[])?,"cpu":command("sysctl", &["-n","machdep.cpu.brand_string"])?},
        "graph":{"neurons":graph.neurons.len(),"edges":graph.edge_src.len()},"models":models,
        "limitations":["Fixed numerical fixture, not held-out accuracy or playing strength", "Native Apple M4 Metal only; no browser WebGPU or Windows DX12 equivalence claim", "All failures retained; acceptance limits fixed before measurement"]});
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?;
    file.write_all(&serde_json::to_vec_pretty(&result)?)?;
    file.write_all(b"\n")?;
    eprintln!(
        "{} {}",
        if pass { "PASS" } else { "FAIL" },
        output_path.display()
    );
    Ok(pass)
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 2 {
        eprintln!("usage: fullgraph-parity NEW_OUTPUT.json (run from repository root)");
        std::process::exit(2);
    }
    match run(Path::new(&args[1])) {
        Ok(true) => (),
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("measurement incomplete: {error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acceptance_has_absolute_and_relative_terms_and_rejects_nonfinite() {
        assert!(
            compare(&[0.0, 1e6], &[0.00009, 1e6 + 10.0], 1e-4, 2e-5)
                .unwrap()
                .pass
        );
        assert!(
            !compare(&[0.0, 20.0], &[0.00011, 20.001], 1e-4, 2e-5)
                .unwrap()
                .pass
        );
        let nonfinite = compare(&[f32::NAN, 1.0], &[0.0, f32::INFINITY], 1e-4, 2e-5).unwrap();
        assert_eq!(nonfinite.nonfinite, 2);
        assert!(!nonfinite.pass);
        assert!(compare(&[], &[0.0], 1e-4, 2e-5).is_err());
    }
    #[test]
    fn legal_scores_are_joined_by_uci_not_rank() {
        let decision = |scores| MoveDecision {
            uci: "a2a3".into(),
            legal_scores: scores,
            from_rates: [0.0; 64],
            to_rates: [0.0; 64],
            promotion_rates: [0.0; 4],
        };
        let left = decision(vec![("b2b3".into(), 2.0), ("a2a3".into(), 1.0)]);
        let right = decision(vec![("a2a3".into(), 1.01), ("b2b3".into(), 1.99)]);
        assert_eq!(
            ordered_scores(&left, &right).unwrap(),
            vec![("a2a3".into(), 1.0, 1.01), ("b2b3".into(), 2.0, 1.99)]
        );
        assert!(ordered_scores(&left, &decision(vec![("a2a3".into(), 1.0)])).is_err());
    }
    #[test]
    fn embedded_protocol_has_both_selected_models_and_intermediate_steps() {
        let protocol: Protocol = serde_json::from_str(PROTOCOL).unwrap();
        assert_eq!(protocol.models.len(), 2);
        assert_eq!(protocol.positions.len(), 12);
        assert_eq!(protocol.settle_steps, 16);
        assert_eq!(protocol.tolerances.activation_atol, 1e-4);
        assert_eq!(protocol.tolerances.policy_rtol, 2e-5);
    }
}
