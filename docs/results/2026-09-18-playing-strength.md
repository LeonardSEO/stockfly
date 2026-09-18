# Actual canonical16-step playing-strength results

Completed 440 games: 280 intact + 160 controls. Terminal outcomes: {'checkmate': 440}. Excluded opening pairs: 0.

These are observed full games against official Stockfish19Lite fixed node budgets. They are separate from training/teacher accuracy. Both real quick4934 checkpoints were measured on the persistent fullgraph Apple M4 Metal backend, at16settlingsteps.

| Model | Condition | Nodes/move | W-D-L | Eligible/planned | Opening pairs | Score [95% bound] | Local Elo difference |
|---|---|---:|---:|---:|---:|---|---|
| Bio quick4934 | intact | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | intact | 100 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | intact | 250 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | intact | 500 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | intact | 1000 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | intact | 2000 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | intact | 5000 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | reset | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | shuffled | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | output-permuted | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Bio quick4934 | brain-bypass | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 100 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 250 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 500 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 1000 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 2000 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | intact | 5000 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | reset | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | shuffled | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | output-permuted | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |
| Max quick4934 | brain-bypass | 50 | 0-0-20 | 20/20 | 10 | 0.000 [0.000, 0.387] | unbounded below; 95% upper bound -80 |

Every all-loss row has no finite point Elo; the displayed upper bound is a one-sided95% Hoeffding bound over ten opening pairs. It is a relative difference against that exact opponent condition, never an absolute strength rating. The experiment did not distinguish Bio from Max, or intact models from controls, by W/D/L.

## Identity and artifacts

- Implementation commit: `3bd460f` (independent implementation review approved).
- Bio SHA256: `d7debd7770a5423753c96ffae498b30cab2b0c19a0e7bb0ff7545936c43ab95e`
- Max SHA256: `1ac336e9059b9a327a8e17c0d33b29e587c263e58a06b456439ae06dacd293d8`
- Graph SHA256: `5daa6237066c5e16e57e4e6dd14195f8b139ed784d79c7328e1fa02d4a350679`
- Frozen native binary SHA256: `852bf41e3c2cd0a81a1e390f1e5576cca8f3389751fbfc96de3cb91ed272d5b6`
- Controller SHA256: `bc62a5e14f642cd4e3e88bb3d8d84e297ad4d9125e37f2fb3ae9a2e5d37895cf`
- Summary SHA256: `b1c41b9a82aa81943fe9e2fbf270730a277183a5ba168f2f3c827eb6f80bd84d`

- `bio-intact.json`: SHA256 `8df65b4def9665ed7953c8590b5db7caf833d8b798609f6e709da88295af9ac2` (140 games)
- `bio-reset.json`: SHA256 `072c095be09aa349ce331b859ce81a48e3a693d9e234d11eed77a7a238f9fe90` (20 games)
- `bio-shuffled.json`: SHA256 `99af2ad2fc1b2e7889d06da9837801fef173940ecfc62e1fccd0267bb9d8ae24` (20 games)
- `bio-output-permuted.json`: SHA256 `8288eba821a145d4dd492c23a0888a9f3b88a10ac5063d99f3d23ae19182c19e` (20 games)
- `bio-brain-bypass.json`: SHA256 `96d4a2558cd22827b1c7e6a82d85bd6e4666bbd0abd0be0153aeb33821170b03` (20 games)
- `max-intact.json`: SHA256 `e9bc105e70e0811928fd90bc47384309419b5abf0e8ed2ace87faa92c73dc304` (140 games)
- `max-reset.json`: SHA256 `d88b14865c1ed291b29961171e5332897ee618f1113fe65d9ac575dfff947c34` (20 games)
- `max-shuffled.json`: SHA256 `a1451d9f676f465bb7670a684da2b5f86c69a73fa06378d183ee1a758977bf5d` (20 games)
- `max-output-permuted.json`: SHA256 `3990e514ea20ef899b99cfea00fca7770c8caa8c2eaacce3321637256ae25e98` (20 games)
- `max-brain-bypass.json`: SHA256 `fdd344898328a911f80a1936323baa854c39a1c932008e3655773290937a2bfe` (20 games)

## Limits

- No human, FIDE, Chess.com, universal Stockfish or absolute Elo calibration. Node budgets are conditions, not ratings.
- All-loss/all-win samples have no finite point Elo estimate. Per-budget score and conservative bounds are primary; no aggregate Elo is fitted.
- Ten distinct opening-pair clusters per row is a coarse sample. Hoeffding bounds assume independent opening clusters; deterministic random eight-ply openings are not a representative human opening distribution.
- Same ten opening pairs are reused across budgets/models/controls. Rows are correlated; do not pool all 440 games into one stronger uncertainty claim.
- Full models/graph controls use Apple M4 Metal at16steps. CPU/GPU fullgraph numerical parity has not been established; these results cannot be claimed as CPU or browser numerical equivalence.
- Brain bypass is a fixed random CPU sensory-to-readout baseline, not a trained full model. Region-ablation strength was not measured here; it remains in the separate causal audit.
- Training quick uses12steps; this canonical benchmark uses explicit16steps matching intended deployed inference calibration. Aborted exploratory12step reports are separate and excluded.
- Scientific causal gates failed for both families in separate audits; these match outcomes do not establish a scientific pass or authorize Lite.
