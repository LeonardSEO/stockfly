# Full-graph numerical parity protocol

The fixed protocol is `crates/stockfly-train/resources/fullgraph-parity.json`. It was written before the first measurement. It selects Bio Standard seed 45 and Max Standard seed 43, twelve diverse nonterminal FENs, and every intermediate/final step from 1 through 16. Every FEN resets both simulators to zero. The GPU also reruns each position in one 16-step submission to check equivalence with canonical batched inference.

The activation and policy rule is **absolute difference <= 1e-4 + 2e-5 * max(abs(cpu), abs(gpu))**, with no nonfinite values and **exact selected-move identity**. All membrane values, all activation rates, all 132 from/to/promotion readouts and all legal move scores are compared. Legal scores are joined by UCI, never ranked-vector index. A position passes only if every recorded step and its batched final check pass; a model/report passes only if every position passes. These limits are acceptance criteria, and are never increased after observing a result.

The relative allowance is about 168 f32 epsilon across ordered high-fan-in reductions and sixteen recurrent updates; the absolute floor covers near-zero cancellation. Rate error is bounded by 0.0005 because rates cap at 20. Membranes are not capped, requiring a relative term. This allowance does not assert universal equivalence of different f32 evaluation orders. Unmatched moves fail even if numerical differences meet tolerance.

Run from the repository root using an isolated build directory; existing audit/ladder binaries and output directories stay untouched:

```sh
CARGO_TARGET_DIR=target/fullgraph-parity cargo test -p stockfly-train --bin fullgraph-parity
CARGO_TARGET_DIR=target/fullgraph-parity cargo test -p stockfly-sim --test fused_arithmetic --test cpu_fixture --test gpu_parity -- --include-ignored --nocapture
CARGO_TARGET_DIR=target/fullgraph-parity cargo build --release -p stockfly-train --bin fullgraph-parity
mkdir -p data/reports/standard-validation-2026-09-19/fullgraph-parity
target/fullgraph-parity/release/fullgraph-parity data/reports/standard-validation-2026-09-19/fullgraph-parity/baseline.json
```

The executable embeds the protocol and source hashes at compile time, hashes its actual executable and verified graph/map/checkpoint inputs, identifies the adapter/configuration, and writes output exclusively (existing results are never overwritten). Exit 1 means measured parity FAIL after the report is saved; exit 2 means the experiment could not complete. Per-step full state hashes, error maxima/worst coordinates and actual CPU/GPU legal scores are retained. Terminal positions are excluded because they have no selected move; this is a numerical inference fixture, not playing-strength or held-out accuracy evidence. Native Metal evidence does not prove browser WebGPU or Windows DX12 parity.

The 19 September baseline failed because Metal fused multiply-add while CPU rounded operations separately. Explicit FMA now defines the two arithmetic updates on both backends. The harness also replays the initial-position GPU steps using separate/fused operation combinations to identify contraction independently of recurrent drift. Preserve each stage in its own raw file and frozen executable; never replace the failed baseline.

After collecting `baseline.json`, `diagnostic.json` and `final.json`, publish a new report basename with:

```sh
python3 tools/parity/publish.py data/reports/standard-validation-2026-09-19/fullgraph-parity docs/results/2026-09-19-fullgraph-parity
```

The publisher verifies matching protocol and input identities, retains baseline/final per-step metrics and identities, and counts pre/post Metal state-hash matches. It refuses to overwrite an existing report. Actual legal-score triples remain in the identified immutable raw files; the tracked JSON keeps their comparison metrics and exact moves.
