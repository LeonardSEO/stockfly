# Selected Standard checkpoints: measured low-node playing strength

Completed 200 actual games with Bio Standard seed 45 and Max Standard seed 43 against official Stockfish 19 Lite v19.0.0. Each row contains ten deterministic, color-swapped opening pairs on Apple M4 Metal at 16 settling steps. These are game outcomes, separate from training-stream agreement and causal validation.

| Model | Nodes/move | Terminal W-D-L | Eligible/requested | Pairs | Excluded pairs / timeouts / errors / caps | Score [95% bound] | Local Elo difference |
|---|---:|---:|---:|---:|---|---|---|
| Bio Standard seed 45 | 1 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio Standard seed 45 | 5 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio Standard seed 45 | 10 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio Standard seed 45 | 25 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio Standard seed 45 | 50 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max Standard seed 43 | 1 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max Standard seed 43 | 5 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max Standard seed 43 | 10 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max Standard seed 43 | 25 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max Standard seed 43 | 50 | 0-0-20 | 20/20 | 10 | 0 / 0 / 0 / 0 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |

Every score interval is paired: the color-swapped opening cluster is the sampling unit. The local Elo difference is relative only to the exact Stockfish build and node budget in that row. Endpoint scores have no finite point Elo. The machine-readable summary retains exact bounds, terminal reasons, per-pair scores, all exclusions and complete input identities.

## Comparison with the prior quick checkpoints

- Bio: No observed W/D/L improvement at 50 nodes: both quick and selected Standard scored 0 wins, 0 draws and 20 losses.
- Max: No observed W/D/L improvement at 50 nodes: both quick and selected Standard scored 0 wins, 0 draws and 20 losses.

The 50-node comparison matches graph, maps, frozen evaluator, calibration, backend, opening pairs, opponent settings/assets, Node and chess.js. The controller changed only to accept explicit low budgets; its new hash is recorded. This is a comparison of observed outcomes, not proof that the checkpoints have equal strength. No Standard-versus-quick improvement is inferred at 1, 5, 10 or 25 nodes because matched quick rows at those budgets were not measured.

## Protocol and identities

The persistent evaluator retains graph/weights but resets neural state for every FEN. Fly sees only the current FEN; Stockfish receives full move history. Ten distinct eight-ply random openings use seed 20260918 plus pair index. Opponent settings: one thread, 16 MiB hash, Skill Level 20, strength limiting off, Ponder off, MultiPV 1, no tablebases, and new-game/cleared hash before every game. The 600-ply cap and 120-second per-request timeout are unscored. Exact commands and settings are in the accompanying JSON.

- Frozen release executable SHA-256: `852bf41e3c2cd0a81a1e390f1e5576cca8f3389751fbfc96de3cb91ed272d5b6`.
- Controller SHA-256: `2cd4ad0cdaa5614af83eeecfd04bad3f579f1ce82b842b9cecf97dab04206e29`.
- Pre-task source snapshot: `f260d6687d055f127288043f3de04bf65d215cab`; individual native source hashes are retained in JSON. The frozen existing binary is byte-identical to the prior quick 16-step ladder binary; no fresh build-to-source attestation is claimed.
- Graph SHA-256: `5daa6237066c5e16e57e4e6dd14195f8b139ed784d79c7328e1fa02d4a350679`.
- Bio checkpoint: `data/training-runs/bio-standard-seed45-2026-09-18/stockfly-bio-full-standard-seed45.sfckpt`; SHA-256 `5e2094743fe960f307d4a39647afbd05682e47867f3a4e771a24f9634d01b3c5`.
- Bio immutable raw report: `data/reports/standard-low-ladder-2026-09-19/bio-standard-seed45.json`; SHA-256 `3942ce77b7f1a01d6dbcbce94e3a3edd9fa9907675f62db8f1947b4a8688480c` (127368 bytes).
- Max checkpoint: `data/training-runs/max-standard-seed43-2026-09-18/stockfly-max-full-standard-seed43.sfckpt`; SHA-256 `a10de3f743ba9b0a2f590ba2a19fae794c0da615facd54e1b88d1595cd1c0bd3`.
- Max immutable raw report: `data/reports/standard-low-ladder-2026-09-19/max-standard-seed43.json`; SHA-256 `3549e21515e0ca4fd579c0733a6010b02aceb62201b7abca41cd17d1b7b2cbb1` (129247 bytes).

The ignored raw reports retain every opening, move, final FEN, termination, elapsed time and completed timestamp. Completed reports are made read-only. The tracked JSON preserves their digests, complete identities and exact summaries. `identity.budgets` records the actual low-node overrides; the embedded protocol also retains the unchanged historical default budget list.

## Validation

The focused ladder controller tests passed 6/6, including explicit low budgets, invalid budget rejection, paired openings, outcome handling, resumability and process failures. All ten completed row summaries were recomputed from their raw games using the frozen `elo-summary`; paired color/opening identity, expected checkpoint digests, backend, 16 steps, row completeness and matched prior 50-node conditions were checked.

## Limits

- Only local score and Elo differences against the exact fixed-node Stockfish 19 Lite condition; no human, FIDE, Chess.com or absolute Elo calibration.
- Ten deterministic random eight-ply opening clusters per row are a coarse sample, not a representative human opening distribution. Colors are swapped within each cluster.
- Rows reuse opening clusters and are correlated. Do not pool rows or models into a stronger confidence claim.
- Complete terminal pairs determine score. Terminal W/D/L also retains individual completed games from excluded pairs. Errors, timeouts and 600-ply caps are unscored. Excluded pairs are replaced up to 50 attempted pairs; this stopping rule may bias results if exclusions occur.
- Intervals use 10,000 deterministic whole-pair bootstrap samples when cluster scores vary; constant scores use conservative Hoeffding bounds (one-sided 95% at endpoints, two-sided otherwise). Bounds assume independent opening clusters and are per-row, without a simultaneous multiple-comparison guarantee.
- Endpoint scores have no finite maximum-likelihood local Elo point estimate.
- Apple M4 Metal backend and 16 settling steps only. This experiment does not establish CPU/browser numerical equivalence or causal validity.
- Quick-checkpoint comparison is restricted to the matched 50-node row. No quick comparison exists here for budgets 1, 5, 10 or 25.
- Both model runs shared the machine with other work. Timing is retained as provenance, not interpreted as a performance benchmark.
- Native executable was frozen from an existing release build and is byte-identical to the prior quick ladder executable. Source snapshot hashes are recorded, but this run does not claim a fresh reproducible build attestation.
