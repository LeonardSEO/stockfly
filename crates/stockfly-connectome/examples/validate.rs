use std::env;
use std::process::ExitCode;
use std::time::Instant;

use stockfly_connectome::Connectome;

fn main() -> ExitCode {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: validate <compiled-graph-dir>");
            return ExitCode::FAILURE;
        }
    };

    let t0 = Instant::now();
    let connectome = match Connectome::open(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to open compiled graph at {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let load_ms = t0.elapsed().as_millis();

    match connectome.validate() {
        Ok(report) => {
            println!("dataset: {}", connectome.manifest.dataset);
            println!("neuron_count: {}", report.neuron_count);
            println!("edge_count: {}", report.edge_count);
            println!("offsets_monotonic: {}", report.offsets_monotonic);
            println!("all_src_indices_in_range: {}", report.all_src_indices_in_range);
            println!("load_time_ms: {load_ms}");
            if report.is_ok() {
                println!("VALIDATION: PASS");
                ExitCode::SUCCESS
            } else {
                println!("VALIDATION: FAIL");
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("validation error: {e}");
            ExitCode::FAILURE
        }
    }
}
