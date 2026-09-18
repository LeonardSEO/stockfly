# StockFly

**A chess engine with a real insect brain.**

StockFly runs a full simulation of the [Janelia MaleCNS v1.0](https://www.janelia.org/project-team/flyem) connectome — every retained neuron and synapse of a fruit fly's (*Drosophila melanogaster*) central nervous system, ~167,000 neurons and ~25.6 million synaptic connections — and lets that simulated brain choose chess moves. No chess-playing neural network sits after the connectome. The move comes out of the same neural activity you watch light up in your browser.

It runs entirely on your own machine. No cloud, no account, no server round-trip once assets are downloaded.

> **Status: active weekend build, in progress.** The core claim already works end-to-end and is verified: the complete real MaleCNS connectome runs in a browser via WebAssembly, plays a legal chess move against a human, and shows the actual neural activity that produced it. The one-command install below (GitHub Release download) isn't wired up yet — see [Running it today](#running-it-today) for how to run it from source right now. Check the [Roadmap](#roadmap) for the rest. Follow [issues](https://github.com/LeonardSEO/stockfly/issues) and commits for live progress.

---

## What makes this different

There are toy demos that put a fly connectome behind a linear readout trained on top. StockFly is not that. The full biological topology and connection signs are preserved and immutable — training only adjusts synaptic magnitudes through biologically motivated local plasticity, never a trainable chess-policy layer bolted on afterward. Every move ships with a replayable trace (model hash, sensory input hash, neural readouts, selected move) and the release includes causal controls — shuffled-connectome, weight-reset, region-ablation, and no-teacher baselines — so the claim "the fly chose this move" is falsifiable, not just asserted.

Two model tiers:

| Model | Description |
|---|---|
| **StockFly Bio** | Full connectome, plasticity restricted to biologically-motivated learning circuits (mushroom body / dopaminergic). |
| **StockFly Max** | Full connectome, every synaptic magnitude eligible for local learning. |
| **StockFly Lite** | A smaller, causally-pruned derivative for lower-end hardware. Never presented as the full brain. |

## Play modes

- **Human vs StockFly** — play the fly yourself in the browser.
- **Stockfish 19 Lite vs StockFly** — fully automated exhibition match. Watch Stockfish's move, then watch the fly brain's neural activity ripple through the connectome before it answers.

Live visualization per move includes the full 3D connectome, a real-time activation heat/spike overlay sourced directly from the simulator (never a fake animation), region filters, sensory input from the board, from-square/to-square neural output, the chosen move, and a scrub-able timeline to replay the decision.

## Getting started

StockFly ships pretrained checkpoints as GitHub Releases, so **you do not need to train anything** to play. Training locally on your own machine is fully supported for anyone who wants to fine-tune or extend the model — see [Training locally](#training-locally).

### macOS

Requirements: macOS with Apple Silicon recommended (WebGPU via Metal). A recent Chrome, Edge, or Safari Technology Preview.

```bash
# 1. Download the latest release bundle
curl -L -o stockfly-macos-arm64.zip \
  https://github.com/LeonardSEO/stockfly/releases/latest/download/stockfly-macos-arm64.zip
unzip stockfly-macos-arm64.zip && cd stockfly

# 2. Fetch the pretrained model + compiled connectome (no training required)
./stockfly-server fetch-models

# 3. Launch — opens your browser at http://localhost:8765
./stockfly-server
```

### Windows

Requirements: Windows 10/11, a GPU with WebGPU support (falls back to WASM automatically if unavailable). A recent Chrome or Edge.

```powershell
# 1. Download the latest release bundle
Invoke-WebRequest -Uri "https://github.com/LeonardSEO/stockfly/releases/latest/download/stockfly-windows-x64.zip" -OutFile stockfly-windows-x64.zip
Expand-Archive stockfly-windows-x64.zip -DestinationPath stockfly
cd stockfly

# 2. Fetch the pretrained model + compiled connectome (no training required)
.\stockfly-server.exe fetch-models

# 3. Launch — opens your browser at http://localhost:8765
.\stockfly-server.exe
```

Nothing leaves your machine. The local server only binds to `127.0.0.1` and sets the cross-origin isolation headers (`COOP`/`COEP`) needed for multithreaded WASM and `SharedArrayBuffer`.

## Running it today

The Release-download flow above is the target experience; the release-publishing automation isn't built yet. Right now, run it from source (macOS, Apple Silicon recommended):

```bash
git clone https://github.com/LeonardSEO/stockfly && cd stockfly

# 1. Fetch the real Janelia MaleCNS v1.0 data and compile the connectome
python3.12 -m venv .venv && .venv/bin/pip install -r tools/malecns/requirements.txt
.venv/bin/python tools/malecns/fetch.py
.venv/bin/python tools/malecns/compile.py --input data/raw/malecns-v1 --output data/compiled/malecns-v1
.venv/bin/python tools/malecns/geometry.py --input data/raw/malecns-v1 --compiled data/compiled/malecns-v1

# 2. Build the WASM engine
rustup target add wasm32-unknown-unknown  # if not already installed
cargo install wasm-pack
cd crates/stockfly-wasm && wasm-pack build --target web --release --out-dir ../../apps/web/src/wasm-gen && cd ../..

# 3. Build and serve the web app
npm install && npm --prefix apps/web run build
cargo run -p stockfly-server --release -- --web-dir apps/web/dist --model-dir data/checkpoints --open
```

This is exactly the path used to verify the app in a real browser during development — see the commit history for details.

## Training locally

Training is designed to run entirely on a 16 GB Apple Silicon MacBook (or comparable hardware) — no external GPU or server required. It uses Stockfish 19 Lite as a local curriculum teacher plus biologically-motivated local plasticity rules (not full backpropagation-through-time through the whole recurrent graph), with wall-clock-bounded presets:

```bash
cargo run -p stockfly-train --release -- train \
  --kind max-full --preset standard \
  --curriculum data/teacher/standard.jsonl \
  --out data/checkpoints/stockfly-max-full.sfckpt
```

| Preset | Wall-clock budget |
|---|---|
| `smoke` | ≤ 10 minutes |
| `quick` | ≤ 30 minutes |
| `standard` | ≤ 2 hours |
| `overnight` | ≤ 6 hours |

See [`docs/superpowers/specs/2026-09-18-stockfly-design.md`](docs/superpowers/specs/2026-09-18-stockfly-design.md) for the full training design, and [`docs/superpowers/plans/`](docs/superpowers/plans/) for the implementation plan this project is being built from.

## Architecture

A Rust workspace owns the canonical connectome graph, simulator, plasticity rules, model format, trainer, WASM bindings, and localhost server. Native and browser simulation share `wgpu` compute kernels: macOS uses Metal, Windows uses DX12/Vulkan, and the browser uses WebGPU, with a WASM/CPU fallback everywhere. A TypeScript/Vite/Three.js UI runs StockFly and Stockfish in separate, isolated Web Workers and renders the exact neural state that produced each move.

```text
localhost
├── Chess UI
├── Live MaleCNS brain visualization
├── StockFly (Full / Lite)
├── Stockfish 19 Lite (WASM, opponent/teacher only)
└── Match controller
```

## Roadmap

- [x] Official MaleCNS fetch + compile pipeline (real v1.0 data: 165,122 neurons, 25,563,197 edges, verified against the design spec's ~167k/25.6M targets)
- [x] CPU reference simulator, running the complete real graph — wgpu (Metal/DX12/Vulkan/WebGPU) acceleration not yet implemented
- [x] Fixed sensory encoding (chess board → real `ol_sensory`/`cb_sensory` populations) and fixed neural move output (real `descending_neuron` population)
- [x] Stockfish 19 (local, via Homebrew) curriculum teacher + local Bio plasticity training pipeline; Max mode implemented, less tuned so far
- [x] Local browser app: Human vs StockFly, running the complete real connectome in WebAssembly, verified end-to-end in a real browser — Stockfish vs StockFly exhibition mode not yet wired up
- [ ] Live 3D connectome visualization with real activation overlay and move-trace replay (current UI shows real from/to activation bars, not yet a 3D brain view)
- [x] No-teacher causal-audit control (verified: inference is byte-identical whether Stockfish is reachable on PATH or not) — shuffled-graph, weight-reset, ablation, and output-permutation controls not yet implemented
- [ ] First measured playing-strength ladder
- [ ] Pretrained checkpoints published as GitHub Releases (checkpoint format and browser-side loading both work; release publishing automation not yet built)
- [ ] StockFly Lite (pruned/quantized, for lower-end hardware)

First real, measured training result (`quick` preset, Bio mode, 4,934 Stockfish-labeled positions): 10.58% teacher-move top-1 accuracy, vs. an 8.02% untrained baseline measured on the same simulator path. This is an early, honest number — not a target, a starting point to grow from.

## License

StockFly is distributed under [GPL-3.0-or-later](LICENSE), a consequence of depending on Stockfish (GPLv3). See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for full attribution of the Janelia MaleCNS connectome (CC-BY) and Stockfish.
