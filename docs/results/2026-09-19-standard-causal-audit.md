# Selected Standard checkpoints: full causal audit — 19 September 2026

**Project gate: Bio Standard seed 45 FAIL; Max Standard seed 43 FAIL.** Neither checkpoint earns scientific promotion from this audit.

Both selected checkpoints were evaluated on every row of the existing 175-position suite at 16 settling steps, using the full MaleCNS graph (165,122 neurons; 25,563,197 edges). The canonical frozen release executable, rebuilt from the exact FMA parity-fix commit `0a0b498`, ran the CPU reference backend on an Apple M4, macOS 27.0 (26A428), arm64. This is teacher-move agreement, not playing strength or Elo.

## Preregistered acceptance rule

The [validation plan](../superpowers/plans/2026-09-19-standard-validation-release.md) recorded this project gate before measurement: intact top-1 must strictly exceed every control; intact top-3 must not be below any control; and the one-sided exact paired McNemar test against the strongest top-1 control must yield `p < 0.05`. All conditions must pass. This is a project gate, not a universal neuroscience standard.

For discordant pairs, `b` counts intact-correct/control-incorrect and `c` counts intact-incorrect/control-correct. The exact test is the upper binomial tail `P[Binomial(b+c, 0.5) >= b]`, with no continuity correction or mid-p adjustment. The dependency-free summarizer computes an exact rational probability before comparing with 1/20. It reports/tests every control tied for highest control top-1; neither measured result has a strongest-control tie.

## Measurements

| Condition | Bio top-1 | Bio top-3 | Max top-1 | Max top-3 |
|---|---:|---:|---:|---:|
| Intact checkpoint | 22/175 (12.57%) | 51/175 (29.14%) | 23/175 (13.14%) | 43/175 (24.57%) |
| Weight reset | 23/175 (13.14%) | 51/175 (29.14%) | 23/175 (13.14%) | 51/175 (29.14%) |
| Degree-aware graph shuffle | 23/175 (13.14%) | 51/175 (29.14%) | 26/175 (14.86%) | 50/175 (28.57%) |
| Descending-neuron ablation | 17/175 (9.71%) | 41/175 (23.43%) | 17/175 (9.71%) | 41/175 (23.43%) |
| Output-label permutation | 15/175 (8.57%) | 41/175 (23.43%) | 18/175 (10.29%) | 50/175 (28.57%) |
| Fixed random brain bypass | 25/175 (14.29%) | 52/175 (29.71%) | 25/175 (14.29%) | 52/175 (29.71%) |

| Gate check | Bio seed 45 | Max seed 43 |
|---|---|---|
| Intact top-1 strictly exceeds every control | FAIL | FAIL |
| Intact top-3 is not below any control | FAIL | FAIL |
| Exact paired p < 0.05 | FAIL | FAIL |

## Exact paired evidence

| Model | Strongest control | b | c | Both correct | Both incorrect | Exact one-sided p |
|---|---|---:|---:|---:|---:|---:|
| Bio seed 45 | Fixed random brain bypass | 17 | 20 | 5 | 133 | 0.744312109207 |
| Max seed 43 | Degree-aware graph shuffle | 7 | 10 | 16 | 142 | 0.833847045898 |

Bio seed 45 exact probability: `51148738673/68719476736`.

Max seed 43 exact probability: `54647/65536`.

## Identities and reproducibility

The [tracked JSON summary](2026-09-19-standard-causal-audit.json) preserves every graph block hash, checkpoint/map/suite/metadata/executable hash, calibration parameter, control seed/statistic, section count, exact paired count, and source-file snapshot hash. The native evaluator and Python wrapper independently verified the actual input files before and after evaluation. Both raw reports were created exclusively and then made read-only; they were not overwritten.

| Artifact | SHA-256 |
|---|---|
| Bio checkpoint | `5e2094743fe960f307d4a39647afbd05682e47867f3a4e771a24f9634d01b3c5` |
| Max checkpoint | `a10de3f743ba9b0a2f590ba2a19fae794c0da615facd54e1b88d1595cd1c0bd3` |
| Frozen evaluator | `17863755bf537adf91fafcd8bc8bcb170644f6663f867c14732fed8a82b1d6e0` |
| Graph aggregate | `5daa6237066c5e16e57e4e6dd14195f8b139ed784d79c7328e1fa02d4a350679` |
| Suite | `ac0c3f68905b1c75d116894dc3da082d252bc408fa5cc5860e15f86aef582639` |
| Sensory map | `fb5ba4e541b13a34a50973a0760de2b056ed4ac7a2d81425b199c095cdf569b5` |
| Output map | `cd6af3b0961c9e096938a9f1802ec32c00d6afb6c9a0125cf72162c64448e5b2` |
| Metadata | `8a194dd63ee05cae071d4c02692867ba87133c5498d4cc6223de87b467173c71` |
| Bio raw JSON | `583cba370b6083029c28369b742e9633c367dfa7ceb935ebc0c85fef26a96054` |
| Max raw JSON | `734c928190f370c88b87f7629bb0d29c801d316202552b0bfa74a4bca598405d` |
| Source file manifest | `318b0c6482b0a17118123dfb06a30bbcf770c8cdfff339db6d128ccaca21ce77` |

Exact build-source commit: `0a0b498f304657d0d1836f39372342b01b7e8775`. A Git archive of that commit was built with `cargo build --release --locked` in the separate `target/causal-audit-fma-0a0b498` directory. All 59 relevant archived source/configuration/resource/test files were SHA-256 verified against Git. The resulting executable was copied exclusively into the frozen FMA directory before either audit. Build command, toolchain, source tree, and per-file source hashes are preserved in JSON. The earlier executable's uncertain build provenance applies only to the superseded pre-fix measurements.

Raw local artifacts (git-ignored; retain/distribute alongside the summary):

- `data/reports/standard-validation-2026-09-19/causal-audit-fma/bio-standard-seed45.json`
- `data/reports/standard-validation-2026-09-19/causal-audit-fma/max-standard-seed43.json`

Run from the repository root with the frozen executable and immutable inputs:

```sh
python3 data/reports/standard-validation-2026-09-19/causal-audit-fma/source/tools/audit/causal_controls.py \
  --binary data/reports/standard-validation-2026-09-19/causal-audit-fma/stockfly-train \
  --model data/training-runs/bio-standard-seed45-2026-09-18/stockfly-bio-full-standard-seed45.sfckpt \
  --metadata data/reports/malecns-v1-audit-metadata.json --settle-steps 16 \
  --out data/reports/standard-validation-2026-09-19/causal-audit-fma/bio-standard-seed45.json
python3 data/reports/standard-validation-2026-09-19/causal-audit-fma/source/tools/audit/causal_controls.py \
  --binary data/reports/standard-validation-2026-09-19/causal-audit-fma/stockfly-train \
  --model data/training-runs/max-standard-seed43-2026-09-18/stockfly-max-full-standard-seed43.sfckpt \
  --metadata data/reports/malecns-v1-audit-metadata.json --settle-steps 16 \
  --out data/reports/standard-validation-2026-09-19/causal-audit-fma/max-standard-seed43.json
python3 tools/audit/summarize_causal.py \
  --report data/reports/standard-validation-2026-09-19/causal-audit-fma/bio-standard-seed45.json \
  --report data/reports/standard-validation-2026-09-19/causal-audit-fma/max-standard-seed43.json \
  --provenance data/reports/standard-validation-2026-09-19/causal-audit-fma/provenance.json \
  --out data/reports/standard-validation-2026-09-19/causal-audit-fma/canonical-summary.json
```

These are the original commands. Reproduction must choose fresh output paths because the tools refuse to overwrite results. Control seeds use the documented wrapper defaults and are preserved in JSON.

## Limitations

- The suite has 175 rows and 170 unique FEN strings. It overlaps the available Standard curriculum by 33 unique FENs (38 suite rows). Actual consumed examples depend on training time limits. These results are not certified held-out accuracy.
- Repeated FENs and potentially related chess positions weaken independence assumptions. The requested exact row-paired test implements the project gate; its p-value is not an independently sampled population-level claim.
- Seeds 45/43 were selected by the highest observed training-stream top-1 in their model families. That selection telemetry is not validation.
- Teacher top-1 uses the single provided best move. Section names are inherited labels, not independently certified tactical categories or acceptance of all equally good moves.
- Shuffle preserves exact fan-in and global out-degree distribution; individual source degree is matched within logarithmic bins. Ablation targets the MaleCNS `descending_neuron` superclass annotation, not a ranked anatomical region. Bypass is a fixed random, untrained sensory readout.
- Version-1 checkpoints bind neuron/map identities but not all original training edge blocks or calibration. The reports bind the actual evaluated files and parameters.
- CPU causal results do not establish GPU parity, browser inference, or playing strength. Those are separate measurements. No scientific or strength promotion is supported by these failed gates.

Validation: `python3 -m unittest discover -s tools/audit -p 'test_summarize_causal.py'` — 7 tests passed. Both full native causal audits completed successfully; the scientific gate outcomes above remain separate from successful execution.

## Superseded pre-fix evidence

The post-FMA measurements above are canonical. All six conditions retained the same top-1/top-3 counts, and the exact paired gate statistics are unchanged. The pre-fix reports remain immutable and retain their original gate outcomes; they are not deleted or relabelled as post-fix measurements. The acceptance rule and every non-executable input, calibration value, control seed/statistic, and paired position identity were checked for equality before comparison.

| Model | Pre-fix intact top-1 / top-3 | Post-FMA intact top-1 / top-3 | Intact moves changed | Top-3 flags changed | Pre-fix → post-FMA gate |
|---|---|---|---:|---:|---|
| Bio seed 45 | 22/175 / 51/175 | 22/175 / 51/175 | 0 | 0 | FAIL → FAIL |
| Max seed 43 | 23/175 / 43/175 | 23/175 / 43/175 | 0 | 0 | FAIL → FAIL |

| Superseded artifact | SHA-256 |
|---|---|
| `data/reports/standard-validation-2026-09-19/causal-audit-frozen/bio-standard-seed45.json` | `03caa9b606193e477409ad83ac5e2cb05df0fc5da901bcfaa3147e94d43c79d7` |
| `data/reports/standard-validation-2026-09-19/causal-audit-frozen/max-standard-seed43.json` | `2d55da1d7fced172694add4ee16474e08136ab481730698f57deadf77d86e5d5` |
| Pre-fix executable | `852bf41e3c2cd0a81a1e390f1e5576cca8f3389751fbfc96de3cb91ed272d5b6` |
| Tracked pre-fix summary at commit `4dabca4` | `74fb8ff8c2a9d299b78f6d261d75776711725fe9531a84b9d6f7fe840fe3e068` |

The tracked JSON preserves the complete pre-fix gate counts, exact paired probabilities, raw/input hashes, and provenance alongside the canonical results. The original Markdown/JSON remain available in commit `4dabca4`. Both the canonical and superseded raw reports must be retained with release evidence.
