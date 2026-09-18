//! Causal-audit control: proves StockFly inference is identical whether or
//! not a Stockfish binary is reachable at all, by actually invoking the
//! compiled binary twice -- once with a normal PATH, once with every
//! directory containing a `stockfish` executable stripped from PATH and
//! `STOCKFISH_BIN` unset -- and diffing the output byte-for-byte. This is a
//! real environment manipulation, not just an assertion that the code
//! "doesn't call" Stockfish: if `infer-untrained`'s decision path secretly
//! shelled out to Stockfish, removing it from PATH would change the output
//! or make the process fail. It does neither.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const FIXED_FEN_SUITE: &[&str] = &[
    "startpos",
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
];

fn workspace_root() -> PathBuf {
    // crates/stockfly-train -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn binary_path() -> PathBuf {
    // Cargo provides this at compile time for integration tests in the
    // same package as the `stockfly-train` bin target -- guaranteed built
    // and guaranteed the right path, unlike guessing from current_exe().
    PathBuf::from(env!("CARGO_BIN_EXE_stockfly-train"))
}

fn strip_timing_lines(output: &str) -> String {
    output
        .lines()
        .filter(|l| !l.starts_with("load_time_ms:") && !l.starts_with("settle_time_ms:"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn path_without_stockfish() -> String {
    let original = env::var("PATH").unwrap_or_default();
    env::split_paths(&original)
        .filter(|dir| !dir.join("stockfish").exists())
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(":")
}

#[test]
fn inference_is_identical_with_stockfish_reachable_or_removed_from_path() {
    let bin = binary_path();
    let stripped_path = path_without_stockfish();
    assert!(
        !stripped_path.split(':').any(|d| PathBuf::from(d).join("stockfish").exists()),
        "test setup bug: stockfish should be absent from the stripped PATH"
    );

    let root = workspace_root();
    for fen in FIXED_FEN_SUITE {
        let with_stockfish = Command::new(&bin)
            .args(["infer-untrained", "--fen", fen])
            .current_dir(&root)
            .output()
            .expect("run with normal PATH");

        let without_stockfish = Command::new(&bin)
            .args(["infer-untrained", "--fen", fen])
            .current_dir(&root)
            .env("PATH", &stripped_path)
            .env_remove("STOCKFISH_BIN")
            .output()
            .expect("run with stockfish stripped from PATH");

        assert!(with_stockfish.status.success(), "inference failed with stockfish present for {fen}");
        assert!(without_stockfish.status.success(), "inference failed with stockfish ABSENT for {fen} -- this would mean it was depended on");

        // Compare everything except wall-clock timing lines, which are
        // expected to vary run-to-run and carry no causal information --
        // the decision, hashes, and neural readouts are what this audit
        // is actually checking.
        assert_eq!(
            strip_timing_lines(&String::from_utf8_lossy(&with_stockfish.stdout)),
            strip_timing_lines(&String::from_utf8_lossy(&without_stockfish.stdout)),
            "inference output differed depending on Stockfish's presence on PATH for {fen}"
        );
    }
}
