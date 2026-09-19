# StockFly

**An experimental chess engine driven by a simulated fruit-fly connectome.**

StockFly simulates the compiled Janelia MaleCNS v1.0 graph: **165,122 neurons and 25,563,197 edges**. Fixed chess inputs drive sensory populations; fixed neural readouts score legal moves. The browser displays sampled activity from that simulation. It runs locally, using WebGPU where available and CPU/WASM fallback when GPU initialization or inference fails.

**Status, 18 September 2026:** the browser application, two trained Full models, native GPU inference, causal controls, actual match benchmark, and local release automation are implemented. Both models **failed the scientific causal acceptance gate**. All 440 measured games were checkmate losses. This is an experimental runtime, not a validated strong chess engine. The verified assets and macOS arm64 runtime are published in the [v0.1.0 experimental prerelease](https://github.com/LeonardSEO/stockfly/releases/tag/v0.1.0-experimental).

## Models and measured results

| Model | Local artifact | Scientific status |
|---|---|---|
| **Bio Full** | Quick preset, 4,934 training trials; learning restricted to the biological mask | Causal gate failed |
| **Max Full** | Quick preset, 4,934 training trials; all synaptic magnitudes eligible for learning | Causal gate failed |

“Full” describes stepping the complete compiled graph. It does not mean the causal gate passed. Topology and signs remain fixed during training; local plasticity changes synaptic magnitudes, without a trainable chess-policy layer after the graph.

The frozen 175-position audit measured teacher top-1 agreement of **24/175 for Bio**, versus 23 reset, 28 shuffled and 25 brain bypass; **18/175 for Max**, versus 23 reset, 22 shuffled and 25 brain bypass. Those controls do not support a causal acceptance claim. Region-ablation and output-permutation controls also run and are recorded. Teacher agreement is not Elo. Max's 10.17% online training agreement (4,934 trials) is also a different metric; its 8.92% untrained stream baseline covered 2,198 trials.

An additional ten-run `standard` candidate sweep completed on 19 September: five deterministic curriculum orderings each for Bio and Max, with two hours of learning per candidate. The best online scores were 10.50% for Bio and 9.93% for Max, but these are order-dependent training-stream measurements. The candidates are published separately and have not replaced the audited quick checkpoints. See the [full sweep table and identities](docs/results/2026-09-19-standard-training-sweep.md).

The completed playing-strength ladder used the real quick checkpoints on Apple M4 Metal at **16 settling steps**, with ten paired openings per condition. Each model played 140 intact games across Stockfish 19 Lite budgets of 50, 100, 250, 500, 1,000, 2,000 and 5,000 nodes/move, plus 80 control games at 50 nodes/move. All **440/440** games ended in checkmate losses; none were excluded or timed out. Every 20-game row is 0 wins, 0 draws, 20 losses. There is **no finite point Elo estimate**. Each row's one-sided 95% score upper bound is 0.387, corresponding to a local Elo-difference upper bound of about −80 against that exact opponent condition. This is not a human or absolute rating, and these floor results cannot rank Bio, Max or their controls. See the [full table and limitations](docs/results/2026-09-18-playing-strength.md) and [model/audit identities](docs/results/2026-09-18-model-summary.json).

## Browser features

- Human vs Fly and isolated Stockfish 19 Lite vs Fly exhibition modes, with side selection, restart, pause and one-ply controls.
- Bio/Max selector backed by content-addressed checkpoint identities. Loading failures stay visible; models do not silently switch.
- Local SVG chess pieces, legal moves and promotions, move history, and a game-over dialog that identifies the winner or draw reason and offers another game.
- Interactive 3D soma point cloud with real source annotations, filters, cell inspection and simulator activation. These are measured soma centroids, not full neuron skeletons. The visible geometry contains 140,024 neurons; all 165,122 participate in Full simulation.
- Sample-specific from/to/promotion readouts, legal move scores, timeline scrubbing, integrity-checked JSON/binary trace export and import, and explicit CPU replay verification. Imported traces do not change the board.

GPU/CPU equivalence is not established for the complete graph. One recorded Bio position selected the same move on both backends but failed activation and policy tolerances (activation difference 2.62; policy difference 2.40e-5). Deterministic CPU replay passed on that position. Synthetic GPU tests and successful fallback do not establish full-graph numerical parity.

## Run a locally prepared portable bundle

The package contains a native server, built web/WASM files, Stockfish, licenses and corresponding source archives. A normal local build also includes the selected graph, soma metadata and Bio/Max model catalog; it requires no Node, Python, Rust or training to **run**. Download-first CI bundles omit model artifacts and need a separately published experimental model release before they can play.

Extract `stockfly-macos-arm64.zip` or `stockfly-windows-x64.zip`, enter its `stockfly` folder, and launch `Start StockFly.command` on macOS or `Start StockFly.cmd` on Windows. The launcher prints and opens `http://127.0.0.1:8765`. These local bundles are unsigned; Windows execution and macOS notarization have not been validated here.

When models are absent, the UI shows an executable download instruction. Run the bundled `Download models.command` / `Download models.cmd`, or use the native CLI:

```sh
# macOS; Windows uses .\stockfly-server.exe
./stockfly-server fetch-models                 # newest published release, including prereleases
./stockfly-server fetch-models --tag v0.1.0-experimental  # pin an existing release
./stockfly-server --open
```

These example tags are not a claim that an asset has already been published. The installer reports total manifest bytes, skips matching files, and stages all downloads before installing hash/size-verified artifacts. Failed downloads preserve existing files. It never invokes training. SHA-256 verifies integrity, **not a publisher signature**. Retry model loading after a successful download. The server binds to loopback by default, serves configured asset roots, sets COOP/COEP, and revalidates mutable assets.

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

For a bundle with locally prepared models, omit `--without-models`. This requires `data/compiled/malecns-v1`, `data/browser`, the selected `data/browser-models/catalog.json`, and the audit/training/ladder artifacts named in `scripts/package-local.py`. The package preserves the catalog's exact checkpoint selection using copy mode. Current Max is the quick run, not the older smoke checkpoint under `data/checkpoints`.

For development, an authorized published release can populate a fresh clone without training:

```sh
node tools/models/fetch-release.mjs --tag v0.1.0-experimental
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
node tools/models/publish-release.mjs --tag v0.1.0-experimental \
  --input dist/releases/BUILD_DIRECTORY/stockfly --out dist/model-release-v0.1.0-experimental
```

The default is a local dry run. It writes content-addressed assets and `release-manifest.json` with each file's SHA-256, byte size, model kind, graph identity, training preset and attribution. The manifest retains **experimental / causal failed** status and includes measured evidence. It refuses uncatalogued checkpoints, mismatched graph/checkpoint identities, symlinks and output reuse. An explicit `--publish` invokes `gh release create --prerelease`. This was used for `v0.1.0-experimental`; its verified macOS arm64 runtime ZIP was uploaded as an additional release asset. Future publication remains a separate explicit action.

Focused verification:

```sh
cargo test -p stockfly-server
node --test tools/models/test-fetch-release.mjs
```

## Training locally

Training is optional. Bio restricts plasticity to its biological mask; Max permits every magnitude to learn. Both preserve graph topology and signs. The quick preset uses 12 settling steps during training; the canonical deployed/ladder calibration uses 16. That difference is recorded rather than presented as interchangeable evidence.

```sh
cargo run --release -p stockfly-train -- train \
  --kind max-full --preset quick --curriculum data/teacher/quick.jsonl \
  --out data/checkpoints/stockfly-max-full.sfckpt
```

Presets target smoke ≤10 minutes, quick ≤30 minutes, standard ≤2 hours and overnight ≤6 hours. See the [training design](docs/superpowers/specs/2026-09-18-stockfly-design.md), [causal audit tooling](tools/audit/README.md), and [paired-match ladder](tools/ladder/README.md).

## Roadmap

- [x] Official MaleCNS graph compilation, geometry and exact neuron ordering.
- [x] CPU reference plus persistent native/browser GPU execution and CPU fallback.
- [x] Fixed sensory/output maps and Bio/Max local plasticity; two trained quick checkpoints.
- [x] Human and Stockfish exhibition chess UI, model selector and game-over dialog.
- [x] Real 3D soma activity, annotations, decision readouts and recorded trace replay.
- [x] Frozen causal controls and actual 440-game playing-strength experiment, with negative evidence retained.
- [x] Experimental manifest installer/publisher and local portable macOS/Windows packaging automation.
- [ ] Successful Full causal acceptance and demonstrated playing-strength improvement.
- [ ] Broad full-graph CPU/GPU numerical parity.
- [x] Experimental GitHub publication, including model/runtime assets and the standard candidate sweep.
- [ ] Windows runtime/browser validation.

## License

StockFly is [GPL-3.0-or-later](LICENSE). MaleCNS-derived assets retain Janelia/FlyEM CC-BY attribution. Bundled Stockfish.js v19.0.0 preserves GPL text, authors, pinned source/binary identities and its corresponding source archive/build instructions. Chess-piece provenance is included. See [third-party notices](THIRD_PARTY_NOTICES.md); these preserved materials are not a legal certification of distribution compliance.
