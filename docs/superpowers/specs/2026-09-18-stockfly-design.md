# StockFly Design Specification

> **Historical scope update (2026-09-19):** The user cancelled StockFly Lite. The original Lite requirements below are preserved as historical design evidence and no longer apply to the current product.

## Goal

Build a local, browser-first chess application in which a simulated full MaleCNS v1.0 fruit-fly connectome chooses chess moves, with live neural visualization and reproducible evidence that each move is caused by activity inside the fly model.

The project has two runtime models:

- **StockFly Bio Full**: full MaleCNS topology, biological connection signs, plasticity restricted to biologically motivated learning circuits.
- **StockFly Max Full**: full MaleCNS topology, biological connection signs, all connection magnitudes eligible for learning.
- **StockFly Lite**: a separately labeled distilled/pruned derivative for slower hardware. It must never be presented as the full brain.

## Non-negotiable requirements

1. Training must be feasible locally on a 16 GB Apple M4 MacBook. External GPU infrastructure is not required.
2. The first useful end-to-end system must fit a weekend development/training cycle. Long-running optional experiments may continue later.
3. Runtime is local and browser based on macOS and Windows.
4. The local browser application supports:
   - Human vs StockFly.
   - Stockfish 19 Lite vs StockFly, fully automated.
5. Stockfish 19 Lite may act as a teacher during training and as an opponent in exhibition mode.
6. Stockfish must not provide candidates, evaluation, search results, or hidden hints to StockFly during StockFly inference.
7. The final Full move must be computed from the MaleCNS simulation. A trainable chess policy network after the brain is prohibited for Full models.
8. Legal move filtering is allowed outside the brain. It may remove illegal moves but may not reorder legal moves using chess knowledge.
9. The live brain visualization must consume the exact simulation state that produces the move. It may downsample for rendering but may not fabricate activation.
10. Every move must be replayable with a trace containing model hash, position, sensory input hash, neural readouts, selected move, and simulator configuration.
11. The application must include causal controls: shuffled-connectome, ablation, and brain-bypass baselines.
12. Full and Lite results, visualizations, model names, and Elo measurements must remain clearly separated.

## Canonical data

Use Janelia MaleCNS v1.0 as the canonical connectome source. The repository must not redistribute the raw third-party dataset. A fetch/compile tool downloads the required official files and writes project-specific binary artifacts.

Required upstream data:

- connection graph / weights
- neuron annotations
- neurotransmitter predictions
- soma or skeleton geometry sufficient for visualization

Raw data stays under `data/raw/` and is gitignored. Compiled data stays under `data/compiled/` and is also gitignored.

## Distribution of trained models

Anyone who clones the repository and runs the local browser app must be able to play against a trained StockFly without training anything themselves.

- Trained checkpoints (`*.sfckpt`) and the compiled MaleCNS graph/geometry artifacts (not raw upstream files) are published as **GitHub Release assets**, each with a SHA-256 recorded in a release manifest.
- Raw MaleCNS upstream files stay excluded from both git and releases, per unresolved raw redistribution terms.
- Compiled/trained derived artifacts may be redistributed under MaleCNS CC-BY terms with attribution preserved in `THIRD_PARTY_NOTICES.md` and in each checkpoint's manifest.
- `tools/models/fetch-release.mjs` downloads the latest (or pinned) release's compiled graph and checkpoints into `data/compiled/` and `data/checkpoints/`, verifying hashes before use.
- The browser app and `stockfly-server` detect a missing local checkpoint and default to "download pretrained model" rather than requiring the user to run the trainer.
- Training locally remains fully supported and is only needed by contributors who want to retrain or fine-tune.

## Licensing

- MaleCNS data: preserve CC-BY attribution and dataset citations.
- Stockfish.js 19: GPLv3. The simplest distribution strategy is to license the distributable StockFly application under GPL-3.0-compatible terms and keep explicit third-party notices.
- Do not commit downloaded MaleCNS raw data or Stockfish release assets without verifying redistribution terms. Prefer deterministic fetch scripts plus SHA-256 verification.

## Repository architecture

```text
stockfly/
├── Cargo.toml
├── package.json
├── LICENSE
├── THIRD_PARTY_NOTICES.md
├── data/
│   ├── raw/                  # gitignored upstream downloads
│   ├── compiled/             # gitignored CSR/CSC + geometry
│   ├── teacher/              # gitignored chess curriculum examples
│   └── checkpoints/          # gitignored trained models
├── crates/
│   ├── stockfly-types/       # shared model/config/trace types
│   ├── stockfly-connectome/  # compiled graph reader and validation
│   ├── stockfly-chess/       # sensory encoding, legal mask, fixed output map
│   ├── stockfly-sim/         # CPU reference + wgpu simulator
│   ├── stockfly-plasticity/  # Bio and Max local learning rules
│   ├── stockfly-train/       # native local trainer/benchmark CLI
│   ├── stockfly-wasm/        # browser bindings
│   └── stockfly-server/      # localhost static server + COOP/COEP
├── shaders/
│   ├── lif_step.wgsl
│   ├── csc_gather.wgsl
│   ├── plasticity.wgsl
│   └── reduce_outputs.wgsl
├── apps/
│   └── web/
│       ├── src/
│       │   ├── chess/
│       │   ├── brain/
│       │   ├── engine/
│       │   ├── traces/
│       │   └── ui/
│       └── public/
│           └── vendor/stockfish/
├── tools/
│   ├── malecns/
│   ├── teacher/
│   └── models/
└── tests/
    ├── fixtures/
    ├── integration/
    └── browser/
```

## Core simulation model

The first Full simulator uses a documented simplified LIF/rate hybrid. The claim is "full MaleCNS connectome simulation", not a perfect biophysical emulation.

### Graph invariants

- Every retained MaleCNS neuron is assigned a stable dense index.
- Every retained MaleCNS connection is represented in the compiled graph.
- Topology is immutable after compile.
- Connection sign is derived from biological transmitter metadata and remains immutable in both Bio and Max.
- Connection magnitude may change according to model mode.
- Compiled graph is stored destination-major (CSC) for gather-based GPU computation without global atomic accumulation.
- Large edge arrays are chunked so no individual browser storage buffer exceeds 64 MiB.

### Neuron state

Each neuron stores at minimum:

- membrane/state value
- firing/rate value
- refractory/adaptation state if enabled
- optional eligibility summary for local plasticity

The CPU reference backend is deterministic and used for correctness tests. The wgpu backend is the production accelerator on native Metal/DX12/Vulkan and browser WebGPU.

## Chess sensory encoding

### Visual channel

The board is rendered to a synthetic fly-eye stimulus. The mapping is fixed and versioned.

- 8x8 board occupies a fixed field of view.
- Piece identity and side are represented using fixed spatial/spectral patterns.
- The pattern drives real visual sensory populations from the compiled MaleCNS annotation map.
- The visual encoder contains no trainable chess policy.

### Context channel

Small non-visual context signals are injected into fixed sensory populations:

- side to move
- castling rights
- en-passant availability/file
- halfmove phase bucket if retained after ablation tests

The legal-move list is never injected as a feature.

## Chess output encoding

Full models use fixed neural output populations.

- 64 disjoint groups for `from-square` logits.
- 64 disjoint groups for `to-square` logits.
- 4 disjoint promotion groups: queen, rook, bishop, knight.
- Group membership is generated once from eligible MaleCNS downstream/motor neurons using a deterministic seed and committed as `output-map.json`.
- Group mapping is not trainable.

For each legal move `m`:

```text
score(m) = from_rate[m.from] + to_rate[m.to] + promotion_rate[m.promotion]
```

Non-promotion moves receive zero promotion contribution. The legal move mask removes illegal combinations only. Highest remaining score is chosen, with deterministic stable tie-breaking.

## Training strategy

No full backpropagation-through-time through the entire 25.6M-edge recurrent graph is required for the weekend path.

### Curriculum

1. legal move behavior and piece motion
2. capture preference and material safety
3. check evasion
4. mate in one
5. mate in two / elementary tactics
6. simple king-and-pawn endings
7. simple piece endings
8. mixed middlegame positions
9. full games and self-play

### Teacher

Stockfish 19 Lite generates local examples with bounded node budgets. Stored examples contain FEN, teacher best move, optional top-k scores for training diagnostics, curriculum stage, and seed. Teacher evaluations are never available to StockFly inference.

### Training stages

**Stage 0: fixed-brain baseline**

- Full graph frozen.
- Fixed sensory encoding.
- Fixed output populations.
- Measure untrained accuracy and Elo baseline.

**Stage 1: calibration**

- Learn only scalar sensory gains, homeostatic neuron thresholds, and output group normalization parameters.
- No chess-capable MLP/readout is allowed.

**Stage 2: StockFly Bio**

- Enable local reward-modulated plasticity only on a curated biologically motivated edge mask, primarily mushroom-body / dopamine-modulated learning circuits.
- Preserve graph and signs.

**Stage 3: StockFly Max**

- All edge magnitudes are eligible for local three-factor learning.
- Preserve graph topology and signs.
- Use sparse activity-gated updates so only active eligible edges are touched in each trial.

**Stage 4: self-play refinement**

- StockFly plays itself and Stockfish Lite opponents at multiple bounded strengths.
- Game result and optional training-only Stockfish deltas provide reward.
- Stockfish is removed entirely from StockFly inference.

## M4 weekend constraints

Training must expose wall-clock budgets rather than promising a target Elo.

Required presets:

- `smoke`: <= 10 minutes, verifies the entire pipeline.
- `quick`: <= 30 minutes.
- `standard`: <= 2 hours.
- `overnight`: <= 6 hours.

Each preset caps:

- curriculum examples
- neural settle steps per position
- plasticity trials
- self-play games

The training CLI measures actual positions/sec and adapts batch size and simulation steps to stay inside the selected wall-clock budget.

## Full vs Lite

Lite is produced only after Full works.

Preferred Lite approach:

1. collect Full activation and ablation traces
2. rank neurons/edges by causal contribution to move logits
3. prune low-contribution neurons/edges while preserving original IDs and edges
4. quantize retained weights
5. retrain only allowed plastic magnitudes inside the retained subgraph

Lite must publish its retained neuron/edge counts and may not display the full-brain visualization as though all neurons were simulated.

## Browser runtime

The local server serves a compiled Vite/TypeScript application and sets cross-origin isolation headers.

Browser workers:

- `stockfly.worker`: Full/Lite simulation via WASM + WebGPU, with CPU/WASM fallback.
- `stockfish.worker`: Stockfish 19 Lite only for opponent mode.
- main UI thread: chess board, brain visualization, controls, trace playback.

The workers communicate using explicit typed messages. `stockfish.worker` can send only an opponent move to the match controller. It cannot send evaluation/candidates to `stockfly.worker`.

## Visualization

The UI shows:

- interactive chess board
- 3D MaleCNS geometry
- real-time activation color/intensity from the current simulator state
- optional region filters
- top active neuron types/regions
- `from` and `to` output histograms
- current selected move
- per-move neural timeline
- clear Full/Bio/Max/Lite badge

Rendering may sample or aggregate activation for frame rate, but every displayed value must be derived from the actual simulation state.

## Causal audit

Every Full model release must pass:

1. **No-teacher inference test**: StockFly produces identical outputs when the Stockfish worker/assets are absent.
2. **Brain bypass baseline**: fixed sensory input connected directly to a tiny non-learning random readout should not match trained Full strength.
3. **Shuffled graph control**: preserve degree distribution as practical while shuffling endpoints; chess performance should materially collapse.
4. **Weight reset control**: restore original weights and measure loss of learned skill.
5. **Region ablation**: disable top causally implicated regions and quantify move-accuracy/Elo change.
6. **Output-map permutation**: permuting fixed output groups should collapse accuracy.
7. **Trace replay**: same model/input on CPU reference reproduces the decision path within documented numeric tolerance.

A model may be called `Full` only if the complete compiled MaleCNS graph is loaded and stepped for the move.

## Elo measurement

Do not infer Elo from puzzle accuracy. Measure playing strength through repeated games against fixed reference engines with known configured strengths or through a calibrated internal ladder. Report confidence intervals and game count.

For weekend development, primary metrics are:

- teacher move top-1 accuracy
- legal move rate before mask
- mate-in-1 success
- tactical suite score
- match score vs fixed Stockfish Lite node budgets
- causal-control deltas
- measured move latency
- peak RAM / GPU memory

## Weekend definition of done

A successful first weekend build has:

- official MaleCNS data fetch/compile
- full graph validation
- CPU reference simulator
- native wgpu simulator running on M4 Metal
- fixed visual/context input mapping
- fixed neural from/to output mapping
- Stockfish 19 Lite teacher generator
- one complete local training run under `standard`
- Bio and Max checkpoints, even if weak
- local browser app
- Human vs StockFly
- Stockfish Lite vs StockFly
- live true activation visualization
- move trace export/replay
- no-teacher and shuffled-connectome controls
- first measured playing-strength ladder
- trained checkpoints published as a GitHub Release with a verified fetch script, so a fresh clone can play without local training

Strength is optimized after this proof is complete, not before.
