# Measured local playing-strength ladder

This runner plays actual chess games against the official Stockfish.js 19 Lite
single-threaded WASM engine, with `go nodes` budgets from
`crates/stockfly-train/resources/ladder.json`. Node budgets are experimental
conditions, **not calibrated human/FIDE/Chess.com Elo ratings**.

```sh
cargo build --release -p stockfly-train
node tools/ladder/run.mjs --model data/checkpoints/stockfly-bio-full.sfckpt --out data/reports/bio-ladder.json
node tools/ladder/run.mjs --model data/training-runs/max-quick-2026-09-18/stockfly-max-full.sfckpt --out data/reports/max-ladder.json
```

Prerequisites are the compiled graph, chess maps, matching audit neuron metadata
(default `data/reports/malecns-v1-audit-metadata.json`), and verified official
assets in `data/vendor/stockfish-19-lite` (`tools/browser/fetch_stockfish.py`).
Native GPU availability is required for intact/reset/shuffled/permuted runs;
there is no silent CPU fallback. The benchmark uses 16 settling steps, matching the deployed browser and causal
audit inference policy. The quick training preset uses 12 steps; that training
setting is recorded separately and does not change benchmark inference.
Every move resets neural state, retains
learned weights and the loaded full graph, and executes all steps without trace
or visualization delays. Report actual adapter/backend: fullgraph CPU/GPU
numerical equivalence has not been established, so do not pool backend results.

Use `--control reset`, `shuffled`, `output-permuted`, or `brain-bypass` for
separate comparison reports; these default to the common 50-node condition.
Repeat these commands for both checkpoints. Shuffling/permutation/bypass reuse
the causal-audit transformations and fixed seed 42; reset omits all learned
weights. Bypass is the fixed random sensory-to-readout CPU baseline, not a
trained full model. Region ablation remains in the separate causal audit.

## Match protocol

At each of 50, 100, 250, 500, 1000, 2000 and 5000 nodes, the default is ten
distinct deterministic opening pairs (20 eligible games). Each opening is eight
plies sampled uniformly from sorted legal moves using the documented LCG in
`opening()`, starting with seed 20260918 + pair index. The two games swap the
model's color. These deliberately diverse random openings are not a standard
human opening distribution; the result applies to this distribution. Opening
FEN, seed and moves are retained, and the same pair set is used at all budgets
and for all models/controls. Stockfish receives full move history; Fly receives
only the current FEN, enforced by an NDJSON schema rejecting extra fields.
Stockfish evaluation/PV lines are discarded and never sent to Fly.

Stockfish uses Threads=1, Hash=16 MiB, Ponder=false, MultiPV=1,
UCI_LimitStrength=false, Skill Level=20, no tablebases, and new-game/cleared-hash
state before every game. Its build/source hashes, the model, graph blocks,
maps, metadata, native binary, controller and chess.js hashes, runtime version,
calibration and actual backend are bound into report identity. The native
`hashes.suite_sha256` field here hashes the fixed ladder protocol JSON.

The chess.js controller records checkmate, stalemate, insufficient material,
threefold repetition, and the fifty-move rule as terminal outcomes (draw claims
are automatic). A 600-ply cap is **unscored**, as are errors/timeouts. A failed or
incomplete opening pair is excluded in full from paired score/uncertainty;
terminal W/D/L still counts observed terminal games and eligible game counts
are reported separately. Extra deterministic pairs replace capped/incomplete
pairs until ten complete pairs exist, with a declared ceiling of 50 attempted
pairs; this stopping/replacement rule may bias results if exclusions occur, so
always publish their counts. A process failure stops the run after checkpointing
its unscored game; it is never silently a draw. Per-request timeout is 120 s.

## Statistics and resume

Reports are atomically checkpointed after every game. Reusing the same command
resumes only when **all** recorded identities/settings match. A sibling `.lock`
prevents concurrent writers. After an uncatchable kill, verify the recorded PID
is no longer alive before removing its stale lock. Freeze the executable with
`--binary` if other work may rebuild it. `--pairs` and `--budgets` support narrow
runs, but are recorded and cannot silently fulfill the default 20-game protocol.

`elo-summary` computes score from complete color pairs and 10,000 deterministic
bootstrap samples of whole pairs. It reports the local difference
`400 * log10(score / (1 - score))` against each exact opponent. No aggregate or
invented opponent ratings are assigned. Endpoint differences are infinite;
all-loss/all-win data have no finite point estimate. For constant cluster
scores, conservative Hoeffding bounds avoid false zero-width uncertainty:
one-sided 95% at score 0/1, two-sided 95% for other constants. These assume
independent opening clusters, and 10 clusters is a coarse, wide-uncertainty
measurement, not a release-quality strength estimate. Generated pseudo-random
opening seeds do not establish a representative human game distribution.

Focused validation:

```sh
cargo test -p stockfly-train --test elo
node --test tools/ladder/test-run.mjs
```

The original protocol/method justification is recorded in the task's method
notes. Statistical references: [cluster bootstrap](https://doi.org/10.1111/j.1467-9868.2007.00593.x),
[Hoeffding bounds](https://doi.org/10.1080/01621459.1963.10500830), and
[Bradley–Terry separation](https://arxiv.org/abs/math/0412232).
