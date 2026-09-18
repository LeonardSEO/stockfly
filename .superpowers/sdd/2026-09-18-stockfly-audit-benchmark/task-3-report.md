# Task 3 report: graph and learning controls

## Status

Implemented and locally verified. The controls execute in the native evaluation
path and derive all variants in memory. They do not edit graph artifacts or
checkpoints. The real Bio measurement completed, but its results do **not** pass
a scientific causal gate: the shuffled and bypass controls did not perform worse
than intact on this suite.

## Implementation

- `audit/shuffle.rs` derives a seeded source-ID permutation within exact
  `consensus_nt`, compiled sign, and `floor(log2(out_degree))` strata. It keeps
  every destination fan-in count exact, preserves the global source out-degree
  distribution, retains edge weights/signs, and records seed and graph stats.
- `audit/reset.rs` clones the checkpoint and clears learned edge deltas while
  retaining its identity and training metadata. Simulation calibration stays in
  the report and is identical between intact and reset.
- `audit/ablation.rs` validates graph-aligned MaleCNS annotations and creates an
  in-memory mask for one exact `superclass` value. `CpuSimulator` applies the mask
  on every step: masked prior rates cannot drive outgoing edges, and masked
  membrane/rate values are forced to zero after integration.
- `audit/output_permutation.rs` uses a seeded Sattolo permutation, so all 64
  square labels move. It derives an output map while consuming the unchanged
  intact simulator rate vector.
- `audit/bypass.rs` implements the design-spec brain-bypass baseline as a fixed,
  deterministic, non-learning 132-output random linear readout over the encoded
  sensory vector. It has no graph, recurrent state, teacher input, or training.
- `stockfly-train causal-audit` evaluates intact, reset, shuffled, ablated,
  output-permuted, and bypass decisions for every supplied FEN.
- `export_neuron_metadata.py` exports exact raw MaleCNS `consensus_nt` and
  `superclass` values in compiled neuron order to an audit-only JSON file.
- `causal_controls.py` verifies all graph blocks against the manifest, hashes all
  inputs and the executable before and after evaluation, and creates a new report
  without overwriting existing output.

No runtime dependency was added. The metadata exporter reuses PyArrow already
required by the repository's MaleCNS compile tools.

## Synthetic regression tests

Command:

```text
cargo test -p stockfly-train --test controls
```

Result: 5 passed, 0 failed. Covered deterministic transmitter/sign/degree
shuffle and graph immutability; reset checkpoint immutability; step-by-step
ablation blocking a three-neuron propagation chain; output relabeling with
unchanged activity; and deterministic non-learning bypass behavior.

Preserved-path regression commands:

```text
cargo test -p stockfly-train --test weight_reset
cargo test -p stockfly-sim --test cpu_fixture
python3 -m unittest discover -s tools/audit -p 'test_*.py'
```

Results: weight reset 2 passed; CPU simulator 2 passed; Python audit wrapper 1
passed. `cargo build -p stockfly-train --release` also completed successfully.

## Real full-graph audit

Commands:

```text
.venv/bin/python tools/audit/export_neuron_metadata.py \
  --out data/reports/malecns-v1-audit-metadata.json
cargo build -p stockfly-train --release
python3 tools/audit/causal_controls.py \
  --model data/checkpoints/stockfly-bio-full.sfckpt \
  --metadata data/reports/malecns-v1-audit-metadata.json \
  --out data/reports/bio-causal-controls-2026-09-18-final.json
```

Scope: 165,122 neurons, 25,563,197 edges, 175 positions, 16 settling steps,
Bio checkpoint. Calibration was `dt_ms=1`, `threshold=0.5`, `decay=0.9`,
`weight_scale=0.5`, and `max_rate=20` in every simulated condition.

| Condition | Top-1 | Top-3 | Moves changed vs intact |
|---|---:|---:|---:|
| Intact | 24/175 | 51/175 | 0 |
| Weight reset | 23/175 | 51/175 | 17 |
| Shuffled graph | 28/175 | 50/175 | 96 |
| `superclass == descending_neuron` ablation | 17/175 | 41/175 | 97 |
| Output-label permutation | 16/175 | 38/175 | 102 |
| Fixed random brain bypass | 25/175 | 52/175 | 162 |

The shuffle changed 25,498,181 edge sources and 165,046 neuron source
assignments across 92 compatibility strata. Destination fan-in maximum delta was
zero, and the source out-degree distribution was preserved. The ablation
silenced all 1,314 neurons annotated `superclass == descending_neuron`; this is
an annotated output-population control, not anatomical-region ranking.

Final artifact hashes:

- report: `287b52da25db52426692e988ee45a5f8e51646c8445f1291ee8eb9820c7c6ebb`
- evaluator binary: `996bcddc724bfac748994b53011b240cf5db60320ba1c0dbaa6ce463bf08d003`
- model: `d7debd7770a5423753c96ffae498b30cab2b0c19a0e7bb0ff7545936c43ab95e`
- graph aggregate: `5daa6237066c5e16e57e4e6dd14195f8b139ed784d79c7328e1fa02d4a350679`
- suite: `ac0c3f68905b1c75d116894dc3da082d252bc408fa5cc5860e15f86aef582639`
- audit metadata: `8a194dd63ee05cae071d4c02692867ba87133c5498d4cc6223de87b467173c71`

The complete per-position report is ignored by Git and remains at
`data/reports/bio-causal-controls-2026-09-18-final.json`.

## Concerns and scientific interpretation

- The causal gate is not passed. Shuffled graph top-1 was 28 versus intact 24,
  and random bypass top-1 was 25 versus intact 24. The implementation therefore
  provides falsifiable controls, but this measurement does not show that the
  complete connectome or learned checkpoint improves suite agreement.
- Reset changed 17 moves and 116 neural readouts, but reduced top-1 by only one
  and left top-3 unchanged. That demonstrates sensitivity, not learned skill.
- Output permutation and descending-neuron ablation reduced agreement, but both
  directly disrupt the output mapping/pathway. They do not establish which
  upstream anatomical regions are causal.
- The suite overlaps the local quick curriculum by 30 exact FENs and smoke by
  13. It must not be described as fully held-out. Teacher agreement is not Elo.
- Individual shuffled source degrees are matched within logarithmic bins rather
  than exactly; destination fan-in and the global source-degree distribution are
  exact.
- Trace replay, top-region ranking, preregistered collapse thresholds, independent
  held-out data, and Elo/game measurements remain outside this task.

## Review round 1 fixes

The direct `stockfly-train causal-audit` JSON now contains an `inputs` object
with actual-byte SHA-256 values for the checkpoint, complete graph file set and
canonical aggregate, manifest, suite, sensory/output maps, audit metadata, and
running executable. Graph blocks are streamed in 1 MiB chunks, verified against
the manifest, and never buffered as another full graph. The native path hashes
before and after evaluation and refuses to emit a report if inputs change. The
Python wrapper independently computes the same canonical values and rejects any
disagreement rather than injecting provenance after native execution.

The shuffle regression now contains acetylcholine and dopamine sources with the
same positive sign, GABA and glutamate sources with the same negative sign, and
connected sources in out-degree bins 0, 1, and 2. Every remapped edge is checked
against the original source's exact transmitter, sign, and degree-bin stratum.
The output-permutation regression now exercises the same paired-decision helper
used by production evaluation, observes both intact and permuted readouts from
one immutable rate slice, and verifies that the relabeled readout changes while
the intact readout still reflects the original activity.

Validation commands and results:

```text
cargo test -p stockfly-train --test controls --test native_hashes
```

Result: 6 passed, 0 failed (5 control regressions and 1 native hash/CLI
fixture). The final native fixture also proved that changing an actual graph
block without its manifest digest makes the CLI reject the input. No
175-position audit was repeated because the scientific computation did not
change. The previous full report retains its original evaluator binary hash;
the amended report-producing binary was verified with the one-position native
CLI fixture (0.44 seconds).
