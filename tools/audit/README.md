# Frozen-checkpoint weight-reset audit

From the repository root:

```sh
cargo build -p stockfly-train --release
python3 tools/audit/weight_reset.py \
  --model data/checkpoints/stockfly-bio-full.sfckpt \
  --out data/reports/bio-weight-reset.json
```

The Python standard-library wrapper verifies every compiled graph block against
its manifest, hashes the checkpoint, suite, maps and executable, and invokes the
native `stockfly-train audit-reset` evaluator. It checks the input hashes again
before writing a new JSON report. Existing output files are never overwritten.
No dependencies, teacher process, training, or model-file changes are required.

Both conditions reset neural state before every FEN. Intact loads the checkpoint's
signed weights; weight-reset uses the compiled base weights with the same uniform
scale. The evaluator rejects incompatible checkpoint identities, invalid weight
indices/signs, duplicate deltas, malformed positions and illegal teacher targets.
It never silently drops examples. Reports contain per-position moves, differences
in from/to/promotion readouts, top-1/top-3 agreement counts, and section totals.

The default is 16 settling steps (native inference default). Use `--settle-steps 12`
to compare with quick training's settling budget. Every simulation parameter is
recorded: resetting weights does not reset calibration. Version-1 checkpoints do
not store calibration or edge-block hashes, so compatibility with the *original
training graph* cannot be established from a checkpoint alone. The report hashes
the actual graph evaluated.

This tests whether learned weights affect decisions, not whether the entire
connectome is necessary or whether playing strength improved. It produces no Elo.
The existing suite's section labels are inherited, not independently certified
(e.g. teacher top-1 agreement in `mate1` is not a test accepting all mating moves).
The current suite overlaps the local quick curriculum by 30 exact FEN strings
(13 with smoke); these results must not be called fully held-out accuracy.

Targeted regression checks:

```sh
cargo test -p stockfly-train --test weight_reset
python3 -m unittest discover -s tools/audit -p 'test_*.py'
```

## Local measurement, 2026-09-18

The full MaleCNS graph (165,122 neurons, 25,563,197 edges), Bio checkpoint,
175 suite positions and 16 settling steps produced:

| Metric | Intact checkpoint | Weight reset |
|---|---:|---:|
| Teacher top-1 | 24/175 (13.71%) | 23/175 (13.14%) |
| Teacher top-3 | 51/175 (29.14%) | 51/175 (29.14%) |

17/175 selected moves changed; 116/175 neural output readouts changed. This
demonstrates sensitivity to the learned weights, but the one-position top-1
difference does not establish improved playing strength. See the checked-in
[summary and input hashes](bio-weight-reset-2026-09-18.summary.json). The complete
local report, including all per-position results, is at
`data/reports/bio-weight-reset-2026-09-18.json` (git-ignored).

## Full causal controls

Generate graph-aligned transmitter and region metadata into the ignored report
directory, build the native evaluator, then run all controls:

```sh
.venv/bin/python tools/audit/export_neuron_metadata.py \
  --out data/reports/malecns-v1-audit-metadata.json
cargo build -p stockfly-train --release
python3 tools/audit/causal_controls.py \
  --model data/checkpoints/stockfly-bio-full.sfckpt \
  --metadata data/reports/malecns-v1-audit-metadata.json \
  --out data/reports/bio-causal-controls.json
```

The native evaluator runs intact, weight-reset, shuffled-graph, selected
MaleCNS-superclass population ablation, output-label permutation, and fixed
random brain-bypass conditions.
The native evaluator and wrapper independently verify all compiled blocks and
hash the checkpoint, suite, maps, metadata, and executable before and after
evaluation. Reports record seeds,
calibration, graph/control statistics, per-position decisions, measured deltas,
and limitations. No report declares a scientific pass from implementation tests;
the observed control deltas must be interpreted against a preregistered criterion.
