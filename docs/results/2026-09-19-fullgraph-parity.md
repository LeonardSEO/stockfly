# Full-graph CPU/GPU parity — 19 September 2026

**PASS after the explicit-FMA correction.** 24/24 model/FEN cases pass at every step from 1 through 16. Maximum membrane / activation / population / legal-policy differences are 0 / 0 / 0 / 0. CPU/GPU full-state hashes are identical in 408/408 observations. Selected moves match in 384/384 intermediate/final steps and 24/24 additional batched-final checks. GPU incremental/batched state hashes match in 24/24 positions.

The pre-fix baseline remains **FAIL**: 16/24 positions exceeded the declared activation tolerance, despite zero move mismatches. Maximum rate error was 0.186384201; maximum legal-policy error was 0.000075817. No tolerance was changed after either result.

## Protocol and diagnosis

The acceptance rule, fixed before the first run in [the protocol](../../crates/stockfly-train/resources/fullgraph-parity.json), is `abs(cpu-gpu) <= 1e-4 + 2e-5 * max(abs(cpu), abs(gpu))` for both activation and policy, with finite values and exact selected UCI identity required. The relative term is about 168 f32 epsilon across high-fan-in reductions and sixteen recurrent updates; the absolute floor covers near-zero cancellation. Rates cap at 20, limiting permitted rate error to 0.0005; membranes are unbounded. A position fails if any neuron, policy rate, legal score, move or batched-final check fails. Legal scores are joined by UCI rather than ranking position.

The Metal compiler contracted multiplication plus addition into FMA, while the CPU used separate f32 operations. A diagnostic replay from each GPU previous state found that fused gather plus fused membrane integration matched every membrane bit for both models on all sixteen initial-position steps; separate arithmetic did not. The correction uses explicit Rust `mul_add` and WGSL `fma` for both operations, preserving sensory addition order. Focused cancellation tests proved the old CPU returned zero where fused arithmetic returns `2^-46`; both CPU and Metal now preserve that result.

All 408/408 pre/post-fix Metal state hashes match, including batched finals. Metal behavior is unchanged on this suite, while CPU rounding changes. Historical pre-fix CPU causal results cannot represent the corrected executable; fresh CPU audits are required for post-fix release claims. Native Metal parity does not establish browser WebGPU or Windows DX12 equivalence.

The twelve cases include both sides to move, initial boards, tactical/castling positions, en passant, all promotion choices, check evasion, sparse pawn/rook endgames and a closed middlegame. Every position is nonterminal and starts at zero state. Promotion-white, check-evasion, pawn-endgame and closed-middlegame remain numerically equal even before the fix; those easier cases are retained. This fixture is numerical evidence, not playing strength, held-out accuracy or a guarantee for every possible FEN.

## Per-position maxima

Final maxima cover all sixteen steps and the separate batched-final run. Full per-step comparisons, failures, CPU/GPU state hashes and selected moves are retained in [the tracked JSON](2026-09-19-fullgraph-parity.json); identified raw files also retain every actual legal-move score.

| Model | Position | Baseline | Baseline max rate error | Baseline max policy error | Final membrane / rate / policy error | Final |
|---|---|---|---:|---:|---|---|
| bio-standard-seed45 | initial-white | FAIL | 0.022151947 | 0.000004768 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | initial-black | FAIL | 0.040549815 | 0.000017166 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | kiwipete-tactical-castling | FAIL | 0.013841629 | 0.000052452 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | open-castling-black | FAIL | 0.044393539 | 0.000011444 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | en-passant-white | FAIL | 0.010249138 | 0.000000000 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | en-passant-black | FAIL | 0.017677307 | 0.000000000 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | promotion-white | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | promotion-black | FAIL | 0.015903473 | 0.000003815 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | check-evasion | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | pawn-endgame | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | rook-endgame | FAIL | 0.186384201 | 0.000000000 | 0 / 0 / 0 | PASS |
| bio-standard-seed45 | closed-middlegame | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | initial-white | FAIL | 0.030698776 | 0.000003815 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | initial-black | FAIL | 0.105178833 | 0.000010490 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | kiwipete-tactical-castling | FAIL | 0.030698776 | 0.000022888 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | open-castling-black | FAIL | 0.105178833 | 0.000075817 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | en-passant-white | FAIL | 0.011152267 | 0.000000000 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | en-passant-black | FAIL | 0.012425423 | 0.000003815 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | promotion-white | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | promotion-black | FAIL | 0.014322281 | 0.000008583 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | check-evasion | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | pawn-endgame | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | rook-endgame | FAIL | 0.041013956 | 0.000008106 | 0 / 0 / 0 | PASS |
| max-standard-seed43 | closed-middlegame | PASS | 0.000000000 | 0.000000000 | 0 / 0 / 0 | PASS |

## Machine and identities

Measured 2026-09-19T09:09:01Z–2026-09-19T09:09:21Z on `Apple M4`, `Darwin 27.0.0 arm64`; OS: ProductName:		macOS; ProductVersion:		27.0; BuildVersion:		26A428. Backend: `metal` / `Apple M4`. Full graph: 165,122 neurons and 25,563,197 edges. Release build: Rust 1.96.1, aarch64-apple-darwin, LLVM 22.1.8, isolated `target/fullgraph-parity`.

Configuration: `{"decay": 0.8999999761581421, "dt_ms": 1.0, "max_rate": 20.0, "reset": "zero per position", "settle_steps": 16, "steps": "all 1..=16 plus single-submission final GPU check", "threshold": 0.5, "weight_scale": 0.5}`. The graph, maps and checkpoint identities were validated before measurement. Source digests are embedded at compile time; the run hashes the actual executable. Source files, graph blocks and each model identity are listed in the JSON.

| Input | SHA-256 |
|---|---|
| Bio Standard seed 45 | `5e2094743fe960f307d4a39647afbd05682e47867f3a4e771a24f9634d01b3c5` |
| Max Standard seed 43 | `a10de3f743ba9b0a2f590ba2a19fae794c0da615facd54e1b88d1595cd1c0bd3` |
| Graph aggregate | `5daa6237066c5e16e57e4e6dd14195f8b139ed784d79c7328e1fa02d4a350679` |
| Sensory map | `fb5ba4e541b13a34a50973a0760de2b056ed4ac7a2d81425b199c095cdf569b5` |
| Output map | `cd6af3b0961c9e096938a9f1802ec32c00d6afb6c9a0125cf72162c64448e5b2` |
| Protocol | `083745e67476e90e89741236c649034d9eb1fac58a25665a848e6655fdef1cf8` |
| Configuration | `71f392627ca4aba7cc48ab993210d2bfffd12054906705486bfac6b0732a7071` |
| Final executable | `db1dc3be9a2f7a1849047897f85282598058e93cf523a29f44669b418c4c8ef4` |
| Embedded source aggregate | `74ab1820bd46d8c809068667d5d23fbc77d1565212590cfcca028ea5094ebf3e` |
| Baseline executable | `72a62062eee64524bf6f3160e2e69e146b02760574c52b9fae9ee323091484fa` |

## Reproduction and validation

See [tools/parity/README.md](../../tools/parity/README.md) for commands. Raw baseline, diagnostic and final reports plus frozen executables are kept separately under `data/reports/standard-validation-2026-09-19/fullgraph-parity/`; their report/executable digests are retained in the tracked JSON. Output creation is exclusive: reruns require a new filename.

Validation performed: three comparator/protocol tests; two CPU cancellation regressions; real Metal cancellation for gather and membrane integration; two existing CPU fixture tests; the expanded learned-weight Metal fixture covering every intermediate membrane/rate and reset/batched state; targeted causal-control and weight-reset tests; optimized full-graph measurement. The old cancellation tests failed before the fix and passed afterward. The fixed full-graph result is stated above; no tolerance was changed.
