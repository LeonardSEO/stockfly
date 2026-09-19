# Task 6 report — canonical experimental release prepared

Prepared for `v0.3.0-standard`. No tag, push, merge, workflow dispatch, GitHub release or asset upload was performed. Publication remains the root task's reviewed external action.

## Integration

The canonical local catalog and copied macOS catalog contain exactly Bio Full Standard curriculum-order seed 45 (6,898 trials; SHA-256 `5e2094743fe960f307d4a39647afbd05682e47867f3a4e771a24f9634d01b3c5`) and Max Full Standard curriculum-order seed 43 (5,649 trials; SHA-256 `a10de3f743ba9b0a2f590ba2a19fae794c0da615facd54e1b88d1595cd1c0bd3`). No catalog regeneration was needed outside isolated package staging. StockFly Lite remains removed; Stockfish 19 Lite and its GPL/source materials remain preserved.

README/current roadmap and `docs/releases/v0.3.0-standard.md` now distinguish the selected Standard evidence from historical quick-model results. Both post-FMA causal gates remain FAIL; the low-node ladder retains 200/200 checkmate losses and no finite point Elo. Native parity is PASS only for the fixed 24-case Apple M4 Metal suite. The training sweep remains unchanged historical candidate evidence, not a held-out/scientific promotion claim.

`package-local.py` selects the tracked Standard summaries plus the exact canonical and superseded raw causal reports, low-node ladder raw reports, baseline/diagnostic/final native parity reports, and verified Windows smoke. Publisher preparation binds these report digests and their checkpoint/graph/map identities to the exact catalog. The schema now describes both models and bounded training/causal/ladder/parity/Windows evidence. Focused fixtures reject wrong checkpoint evidence, changed raw digests, failed Windows smoke and extra catalog identities.

## macOS arm64 model-bearing package

Successful command (the installed rustup toolchain supplies the WASM target):

```sh
env PATH=/Users/leonard/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin ./scripts/package-local.sh --out dist/standard-v0.3.0 --stockfish-source-archive dist/releases/stockfly-macos-arm64-5pjg1yo9/stockfly/apps/web/dist/vendor/stockfish/stockfish-js-v19.0.0-source.tar.gz
```

Portable root: `dist/standard-v0.3.0/stockfly-macos-arm64-w35e0a_1/stockfly`.

| Artifact, relative to `dist/standard-v0.3.0/stockfly-macos-arm64-w35e0a_1/` | Bytes | SHA-256 |
|---|---:|---|
| `stockfly-macos-arm64.zip` | 124189477 | `fa0b81ba2cc446b4a51caa4fcd919da8df27829abf355a06a7134d88e5dc26a9` |
| `model-runtime-smoke.json` | 12385 | `660f2153c0659dd69973e085e1955fc24c6560915d3bee960d2891de199c0a19` |
| `stockfly/stockfly-server` | 3360064 | `e08f3f593a43f8847f55c4a0b5f0468bb932fcdf2988c85ba9b1b97ad18b86aa` |
| `stockfly/stockfly-source.tar.gz` | 447970 | `fcfddffd361c947e4fd48272bd41ac26aafcc34cda0ddefbda0a40a289dfa303` |

The ZIP was CRC-checked, extracted into isolated staging, checked for symlinks, and started on a fresh loopback port. The smoke passed 20 HTTP routes with isolation headers: app/favicon/health, legacy `/models`, exact two-model catalog/checkpoints, graph/geometry/metadata/maps, Stockfish JS/WASM, and every built web asset including StockFly WASM. Served checkpoint and built-asset digests matched packaged files. The legacy `/models` route correctly remains empty because canonical browser models use the catalog endpoint. All ten tracked integration source files in the corresponding-source archive matched the current working tree. Licenses and the pinned Stockfish source archive were present. The smoke server was terminated; the existing development server was not interrupted.

This is extracted native-runtime/HTTP/artifact integrity evidence, not new browser model inference or browser parity evidence. The existing Task 4 fixed-suite native numerical evidence remains the inference parity boundary.

Two prior staging attempts are preserved and are not release inputs: `stockfly-macos-arm64-m1tn960y` failed because default Homebrew rustc lacked the WASM target; `stockfly-macos-arm64-ry3qbtwn` built but exposed an evidence-loop variable collision in the ZIP basename. The corrected packager uses a separate `bundle_name`; the final successful package above has the intended name. No user files were removed.

## Content-addressed model staging

```sh
node tools/models/publish-release.mjs --tag v0.3.0-standard --input dist/standard-v0.3.0/stockfly-macos-arm64-w35e0a_1/stockfly --out dist/model-release-v0.3.0-standard
```

The output directory did not exist before preparation. It now contains **37 content-addressed assets (407,630,874 total bytes)** and `release-manifest.json`, 38 entries total. Every asset size, full SHA-256, hash-prefixed name and entry count was independently checked. Exactly two checkpoint files are included.

Manifest: `dist/model-release-v0.3.0-standard/release-manifest.json`, **20,939 bytes**, SHA-256 **`82079134f4e4683d59038b38f31641dcd4de01e0628b750e85e7bad8466e7694`**. Verification record: `dist/standard-v0.3.0/manifest-verification.json`.

Manifest status remains `experimental` / causal `failed`. Scientific evidence includes original per-budget totals/scope and raw-report digest binding. SHA-256 is integrity, not publisher authentication.

## Windows dependency resolved within its measured scope

Root supplied passing [workflow 35434707879](https://github.com/LeonardSEO/stockfly/actions/runs/35434707879), attempt 1, exact source `d29039f37a24076acbf2d79776349059a2d5deba`. This task independently checked the downloaded ZIP size/digest and smoke digest. The smoke JSON is copied unmodified into `docs/results/2026-09-19-windows-runtime.json`; the companion Markdown describes scope and provenance.

- ZIP: `dist/ci-35434707879/stockfly-windows-x64/stockfly-windows-x64-qkpa6ifu/stockfly-windows-x64.zip`, 4,223,666 bytes, SHA-256 `e90cbadc6f6f3eb8a5a7ebf761544dd6b9bde167a174cd9689922c774baf967a`.
- Smoke: `dist/ci-35434707879/stockfly-windows-x64/stockfly-windows-x64-smoke.json`, 4,724 bytes, SHA-256 `ffcf3fb5b07615114581e2f9b7fa3eebdaf72e4310b6d97be1abb70b17cae65e`.
- PASS: native extracted x64 runtime, required HTTP/isolation checks and headless Chrome 152 no-model UI/download instructions; all unexpected error arrays empty.

No remaining Windows *download-first runtime/browser smoke* blocker. The tested ZIP predates the documentation/evidence integration and retains its original bundled documentation. It contains no models; Windows model inference and DX12 parity remain unverified. The exact tested ZIP should be retained as the Windows release input.

## Validation performed

- `node --test tools/models/test-publish-release.mjs tools/models/test-fetch-release.mjs`: 17/17 pass (six publisher, eleven installer tests).
- After tightening the finite-point-Elo summary handling, only `node --test tools/models/test-publish-release.mjs` reran: 6/6 pass.
- Successful WASM release, Vite production and native server release builds through the packager; existing advisory package metadata/Vite size warnings only.
- One final extracted model-bearing package smoke: PASS; 20 routes, exact model and source identities, source/license material preserved.
- Actual model publisher schema/evidence validation: PASS; independent complete staging hash/size/count verification: PASS.
- Scoped `git diff --check`: PASS.

Only the integration source/docs/tests and this report are committed as `release: prepare v0.3.0 standard models`. Ignored dist/data artifacts are retained locally and are not staged. Final review and external publication remain with root.
