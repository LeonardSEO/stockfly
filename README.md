# StockFly

**An experimental chess engine driven by a simulated fruit-fly connectome.**

StockFly simulates the compiled Janelia MaleCNS v1.0 graph: **165,122 neurons and 25,563,197 edges**. Fixed chess inputs drive sensory populations; fixed neural readouts score legal moves. The browser displays sampled activity from that simulation. It runs locally, using WebGPU where available and CPU/WASM fallback when GPU initialization or inference fails.

**Status, 19 September 2026:** the canonical catalog selects **Bio Full Standard seed 45** and **Max Full Standard seed 43**. Both **FAIL the post-FMA causal acceptance gate**. The selected models lost all **200/200** low-node ladder games by checkmate. Native Apple M4 Metal parity passes the fixed 24-case suite after the arithmetic correction. [`v0.3.0-standard`](https://github.com/LeonardSEO/stockfly/releases/tag/v0.3.0-standard) is published as an **experimental, causal-failed prerelease**. The Windows download-first runtime/browser smoke passed on the identified CI artifact. See the [release record](docs/releases/v0.3.0-standard.md).

## Models and measured results

| Model | Canonical local artifact | Scientific status |
|---|---|---|
| **Bio Full** | Standard, curriculum-order seed 45, 6,898 training trials; biological plasticity mask | Post-FMA causal gate FAIL |
| **Max Full** | Standard, curriculum-order seed 43, 5,649 training trials; all synaptic magnitudes eligible | Post-FMA causal gate FAIL |

“Full” describes stepping the complete compiled graph. Topology and signs remain fixed during training; local plasticity changes synaptic magnitudes, without a trainable chess-policy layer after the graph. StockFly Lite has been removed from the product; **Stockfish 19 Lite remains the external teacher/opponent**.

The two checkpoints were selected from ten Standard candidates by the highest observed online training-stream top-1 agreement: 10.50% for Bio and 9.93% for Max. Selection is order-dependent and is neither held-out accuracy nor evidence of stronger play. The [historical sweep](docs/results/2026-09-19-standard-training-sweep.md) records its original candidate-only state; the canonical selection is recorded here and in the release manifest.

The canonical post-FMA CPU audits cover 175 rows at 16 settling steps. Bio intact top-1/top-3 is **22/175 and 51/175**, versus the strongest top-1 control, brain bypass, at **25/175 and 52/175**. Max intact is **23/175 and 43/175**, versus shuffled graph at **26/175 and 50/175**. Their exact paired one-sided p-values are 0.744312 and 0.833847; both fail all three preregistered checks. There are 170 unique FENs and 38 rows overlap the available Standard curriculum. These are teacher-agreement measurements, not certified held-out accuracy or Elo. [Canonical audits and preserved pre-fix evidence](docs/results/2026-09-19-standard-causal-audit.md).

The selected Standard checkpoints played Stockfish 19 Lite at **1, 5, 10, 25 and 50 nodes/move**, using persistent Apple M4 Metal inference, **16 settling steps**, and ten color-swapped opening pairs per model/budget. Every row is **0 wins, 0 draws, 20 checkmate losses**; no games were excluded, timed out or capped. There is **no finite point Elo estimate**. Each row has a one-sided 95% score upper bound of 0.387 and a local Elo-difference upper bound of about −80 against that exact opponent condition. Rows cannot be pooled into a stronger interval. The 50-node results match the earlier quick models' observed W/D/L; this does not establish equal strength or improvement. [Full Standard ladder and limitations](docs/results/2026-09-19-standard-playing-strength.md).

The [earlier quick-model 440-game experiment](docs/results/2026-09-18-playing-strength.md) remains historical negative evidence. It does not describe the current Standard catalog.

## Browser features

- Human vs Fly and isolated Stockfish 19 Lite vs Fly exhibition modes, with side selection, restart, pause and one-ply controls.
- Bio/Max selector backed by content-addressed checkpoint identities. Loading failures stay visible; models do not silently switch.
- Local SVG chess pieces, legal moves and promotions, move history, and a game-over dialog that identifies the winner or draw reason and offers another game.
- Interactive 3D soma point cloud with real source annotations, filters, cell inspection and simulator activation. These are measured soma centroids, not full neuron skeletons. The visible geometry contains 140,024 neurons; all 165,122 participate in Full simulation.
- Sample-specific from/to/promotion readouts, legal move scores, timeline scrubbing, integrity-checked JSON/binary trace export and import, and explicit CPU replay verification. Imported traces do not change the board.

The corrected native CPU and Apple M4 Metal implementations pass **24/24 fixed model/FEN cases** at every step from 1 through 16, with zero measured membrane, activation, population and legal-policy differences. All 408 recorded CPU/GPU state hashes and selected moves match. The initial 16/24 activation failures remain preserved; the fix made fused arithmetic explicit without relaxing tolerances. This is fixed-suite native numerical evidence, not universal FEN coverage, browser WebGPU/Windows DX12 parity or playing strength. [Full parity report](docs/results/2026-09-19-fullgraph-parity.md).

## Run a locally prepared portable bundle

The package contains a native server, built web/WASM files, Stockfish, licenses and corresponding source archives. A normal local build also includes the selected graph, soma metadata and Bio/Max model catalog; it requires no Node, Python, Rust or training to **run**. Download-first CI bundles omit model artifacts and need a separately published experimental model release before they can play.

Extract `stockfly-macos-arm64.zip` or `stockfly-windows-x64.zip`, enter its `stockfly` folder, and launch `Start StockFly.command` on macOS or `Start StockFly.cmd` on Windows. The launcher prints and opens `http://127.0.0.1:8765`. These local bundles are unsigned. The Windows download-first runtime/browser smoke passed on [the CI artifact from commit d29039f](docs/results/2026-09-19-windows-runtime.md); Windows model inference and macOS notarization remain unverified.

When models are absent, the UI shows an executable download instruction. Run the bundled `Download models.command` / `Download models.cmd`, or use the native CLI:

```sh
# macOS; Windows uses .\stockfly-server.exe
./stockfly-server fetch-models                 # newest published release, including prereleases
./stockfly-server fetch-models --tag v0.3.0-standard
./stockfly-server --open
```

These commands use the published experimental prerelease. The installer reports total manifest bytes, skips matching files, and stages all downloads before installing hash/size-verified artifacts. Failed downloads preserve existing files. It never invokes training. SHA-256 verifies integrity, **not a publisher signature**. Retry model loading after a successful download. The server binds to loopback by default, serves configured asset roots, sets COOP/COEP, and revalidates mutable assets.

## Build from source

Build prerequisites: Node 22+, npm, Python 3, Rust/Cargo, the `wasm32-unknown-unknown` target, and `wasm-pack` 0.13.1. Use the lockfiles. The package script supports a macOS arm64 or Windows x64 host:

```sh
npm ci
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --version 0.13.1 --locked

# Runtime-only build; no graph/checkpoint/raw download or training:
./scripts/package-local.sh --without-models
# Windows PowerShell: .\scripts\package-local.ps1 --without-models
```

This single command builds WASM, Vite web assets and the native server in isolated staging under `dist/releases/`, materializes approved runtime files, and produces a ZIP with no external symlinks. It verifies the pinned Stockfish browser binaries and corresponding v19.0.0 source archive, reusing local files when available. It does not change the development catalog or generated development WASM. Custom `--out` directories must remain inside this repository's `dist/` directory, where the staged app can resolve the installed workspace dependencies. The GitHub workflow builds CI artifacts only; it neither creates nor uploads GitHub Releases.

For a bundle with locally prepared models, omit `--without-models`. This requires `data/compiled/malecns-v1`, `data/browser`, the selected `data/browser-models/catalog.json`, and the audit/training/ladder artifacts named in `scripts/package-local.py`. The package preserves the catalog's exact checkpoint selection using copy mode. The catalog must contain exactly Bio Standard seed 45 and Max Standard seed 43 for this canonical release. Packaging includes the Standard training sweep, post-FMA causal reports, low-node ladder, and baseline/final native parity evidence.

For development, an authorized published release can populate a fresh clone without training:

```sh
node tools/models/fetch-release.mjs --tag v0.3.0-standard
# The release uses data/browser-models; keep the tracked legacy public link valid.
mkdir -p data/checkpoints
# Prepare the pinned browser opponent, source archive and license files before Vite.
npm run assets:stockfish --workspace apps/web
wasm-pack build crates/stockfly-wasm --target web --release --out-dir ../../apps/web/src/wasm-gen
npm run build --workspace apps/web
cargo run --release -p stockfly-server -- --open
```

The installer supplies the graph, browser metadata, maps and content-addressed model catalog/checkpoints from the experimental release. The empty legacy `data/checkpoints` directory only resolves its tracked public symlink; it does not duplicate or substitute a model. The Stockfish preparation command verifies its pinned assets. To reuse an existing offline cache, append `-- --source-dir /path/to/prepared-stockfish --offline` to that npm command. Together these steps resolve every tracked `public/vendor` symlink before Vite copies public files. The source wrapper uses the native server installer, compiling it when necessary. To reproduce data from upstream instead, use the existing [MaleCNS compiler](tools/malecns/compile.py), [geometry exporter](tools/malecns/geometry.py), and [browser metadata/model preparation instructions](tools/browser/README.md). Raw MaleCNS data never enters portable or model release artifacts. Fixed chess maps are tracked source files and do not need regeneration for an ordinary build.

## Prepare a model release locally

After building a model-containing package, pass its printed `PORTABLE_ROOT` and an empty output directory:

```sh
node tools/models/publish-release.mjs --tag v0.3.0-standard \
  --input dist/releases/BUILD_DIRECTORY/stockfly --out dist/model-release-v0.3.0-standard
```

The default is a local dry run. It writes content-addressed assets and `release-manifest.json` with each file's SHA-256, byte size, model kind, graph identity, training preset and attribution. The manifest retains **experimental / causal failed** status and includes measured evidence. It binds the causal, ladder and parity evidence to the catalog, graph, maps and raw-report digests. It refuses uncatalogued checkpoints, mismatched evidence/graph/checkpoint identities, symlinks and output reuse. An explicit `--publish` invokes `gh release create --prerelease`. The reviewed `v0.3.0-standard` inputs are published; all 42 remote assets were checked against their local byte sizes and SHA-256 digests. The Windows runtime asset is the exact artifact that passed its workflow smoke.

Focused verification:

```sh
cargo test -p stockfly-server
node --test tools/models/test-fetch-release.mjs tools/models/test-publish-release.mjs
```

## Training locally

Training is optional. Bio restricts plasticity to its biological mask; Max permits every magnitude to learn. Both preserve graph topology and signs. The selected Standard checkpoints use 16 settling steps during training and canonical inference/audit/ladder measurements. Historical quick checkpoints used 12 training steps; those results remain separate.

```sh
cargo run --release -p stockfly-train -- train \
  --kind max-full --preset quick --curriculum data/teacher/quick.jsonl \
  --out data/checkpoints/stockfly-max-full.sfckpt
```

Presets target smoke ≤10 minutes, quick ≤30 minutes, standard ≤2 hours and overnight ≤6 hours. See the [training design](docs/superpowers/specs/2026-09-18-stockfly-design.md), [causal audit tooling](tools/audit/README.md), and [paired-match ladder](tools/ladder/README.md).

## Roadmap

- [x] Official MaleCNS graph compilation, geometry and exact neuron ordering.
- [x] CPU reference plus persistent native/browser GPU execution and CPU fallback.
- [x] Fixed sensory/output maps and Bio/Max local plasticity; selected Bio/Max Standard checkpoints.
- [x] Human and Stockfish exhibition chess UI, model selector and game-over dialog.
- [x] Real 3D soma activity, annotations, decision readouts and recorded trace replay.
- [x] Canonical post-FMA causal controls and the 200-game Standard low-node ladder, with failed gates and losses retained.
- [x] Experimental manifest installer/publisher and local portable macOS/Windows packaging automation.
- [ ] Successful Full causal acceptance and demonstrated playing-strength improvement.
- [x] Full-graph native Apple M4 Metal parity on the fixed 24-case suite.
- [ ] Browser WebGPU and Windows DX12 numerical parity.
- [x] Historical experimental releases and Standard candidate sweep publication.
- [x] Published reviewed `v0.3.0-standard` canonical experimental assets.
- [x] Windows download-first runtime/browser smoke on the identified CI artifact.

## License

StockFly is [GPL-3.0-or-later](LICENSE). MaleCNS-derived assets retain Janelia/FlyEM CC-BY attribution. Bundled Stockfish.js v19.0.0 preserves GPL text, authors, pinned source/binary identities and its corresponding source archive/build instructions. Chess-piece provenance is included. See [third-party notices](THIRD_PARTY_NOTICES.md); these preserved materials are not a legal certification of distribution compliance.
