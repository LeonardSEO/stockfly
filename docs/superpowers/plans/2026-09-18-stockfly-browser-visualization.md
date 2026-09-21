# StockFly Browser & Neural Visualization Implementation Plan

> **Cancelled by the user (2026-09-19):** StockFly Lite is no longer product scope. This document is preserved as historical planning evidence; its original Lite instructions must not be implemented.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a fully local browser experience on macOS and Windows with Human vs StockFly, Stockfish 19 Lite vs StockFly, and live visualization of the exact neural activity driving each move.

**Architecture:** A small Rust localhost server serves a Vite/TypeScript application with COOP/COEP headers. StockFly and Stockfish run in separate workers. Three.js renders compiled MaleCNS geometry and colors it from sampled simulator activation frames.

**Tech Stack:** Rust axum or equivalent minimal server, wasm-bindgen, Vite, TypeScript, Three.js, chess.js, Vitest/Playwright.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- App must remain usable without internet once assets are installed.
- Stockfish worker may communicate only opponent moves to the match controller.
- Visualization data must originate from StockFly simulation frames.
- Full/Lite/Bio/Max labeling is always visible.

---

### Task 1: Compile StockFly simulator to WASM and WebGPU

**Files:**
- Create: `crates/stockfly-wasm/Cargo.toml`
- Create: `crates/stockfly-wasm/src/lib.rs`
- Create: `apps/web/src/engine/stockfly.worker.ts`
- Create: `apps/web/src/engine/protocol.ts`
- Test: `apps/web/src/engine/protocol.test.ts`

**Interfaces:**

```ts
type StockFlyRequest =
  | {type: 'load'; modelUrl: string}
  | {type: 'position'; fen: string; traceId: string}
  | {type: 'reset'};

type StockFlyResponse =
  | {type: 'loaded'; manifest: ModelManifest}
  | {type: 'activation'; traceId: string; frame: ActivationFrame}
  | {type: 'decision'; traceId: string; decision: MoveDecision};
```

- [ ] **Step 1: Write protocol exhaustive-switch test**

- [ ] **Step 2: Implement wasm-bindgen surface**

Expose model load, position submit, frame polling, and decision retrieval. Do not expose any teacher API.

- [ ] **Step 3: Implement worker**

Use WebGPU when adapter is available; fallback to CPU/WASM with a visible `CPU fallback` status.

- [ ] **Step 4: Run unit tests and a browser smoke page**

```bash
npm --prefix apps/web test
```

- [ ] **Step 5: Commit**

### Task 2: Build local server and offline asset loading

**Files:**
- Create: `crates/stockfly-server/Cargo.toml`
- Create: `crates/stockfly-server/src/main.rs`
- Test: `crates/stockfly-server/tests/headers.rs`

**Interfaces:**
- CLI:

```bash
stockfly-server --web-dir apps/web/dist --model-dir data/checkpoints --open
```

- [ ] **Step 1: Write response-header test**

Assert:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Cache-Control for model blobs: public, max-age=31536000, immutable
```

- [ ] **Step 2: Implement static server with localhost-only bind by default**

Default bind `127.0.0.1:5187`; the browser opens `http://localhost:5187/#play`, and `--bind` remains an explicit override.

- [ ] **Step 3: Add `/health` and `/models` endpoints**

`/models` lists only local manifests, not arbitrary filesystem paths.

- [ ] **Step 4: Test and commit**

### Task 3: Implement chess board and both match modes

**Files:**
- Create: `apps/web/src/chess/game.ts`
- Create: `apps/web/src/chess/HumanVsFly.ts`
- Create: `apps/web/src/chess/EngineVsFly.ts`
- Create: `apps/web/src/engine/stockfish.worker.ts`
- Test: `apps/web/src/chess/game.test.ts`

**Interfaces:**
- Match controller only sends FEN to StockFly.
- Stockfish response passed to match controller as `{move: string}`; evaluation strings are dropped inside the Stockfish worker.

- [ ] **Step 1: Write worker-isolation test**

Feed a fake Stockfish UCI response containing `score cp`, `pv`, and `bestmove`; assert the only emitted application message is `{type:'move', uci:'...'}`.

- [ ] **Step 2: Implement Human vs Fly**

Human move updates FEN; StockFly gets FEN; chosen move is applied after `decision` event.

- [ ] **Step 3: Implement Stockfish vs Fly**

Alternate workers automatically. Add pause, step-one-move, restart, and side selection.

- [ ] **Step 4: Run game tests and commit**

### Task 4: Render the MaleCNS and live activation

**Files:**
- Create: `apps/web/src/brain/BrainView.ts`
- Create: `apps/web/src/brain/geometry.ts`
- Create: `apps/web/src/brain/activation.ts`
- Create: `apps/web/src/brain/BrainLegend.ts`
- Test: `apps/web/src/brain/activation.test.ts`

**Interfaces:**

```ts
interface ActivationFrame {
  tMs: number;
  neuronRates: Float32Array | QuantizedActivation;
  topNeurons: Array<{denseIndex:number; rate:number}>;
  regionRates: Array<{region:string; rate:number}>;
}
```

- [ ] **Step 1: Write activation-to-color test**

Zero activity maps to background intensity; increasing rate is monotonic and clamped.

- [ ] **Step 2: Load LOD0 geometry once**

Geometry vertex attributes include dense neuron index. Update color/activation buffer rather than rebuilding geometry each frame.

- [ ] **Step 3: Implement true simulation frame sampling**

StockFly worker emits full rates only at low frequency or selected mode; default renderer uses quantized sampled frames at 20-30 Hz to avoid copying hundreds of thousands of floats at 60 Hz.

- [ ] **Step 4: Add region selection and top-neuron inspection**

Clicking a region/neuron shows original body ID, type/class metadata, current rate, and whether it belongs to an output group.

- [ ] **Step 5: Commit**

### Task 5: Visualize the decision and replay each move

**Files:**
- Create: `apps/web/src/traces/MoveTrace.ts`
- Create: `apps/web/src/traces/TraceTimeline.ts`
- Create: `apps/web/src/ui/DecisionPanel.ts`
- Test: `apps/web/src/traces/MoveTrace.test.ts`

**Interfaces:**
- Trace contains sampled activation frames plus exact final from/to/promotion rates and hashes.

- [ ] **Step 1: Write trace round-trip test**

Export JSON + binary activation payload, import it, verify same selected move and hashes.

- [ ] **Step 2: Implement DecisionPanel**

Show all 64 from rates and 64 to rates as compact bars, highlight selected squares, and list top legal move scores.

- [ ] **Step 3: Implement timeline**

Scrubbing changes both brain colors and decision graphs to recorded frames.

- [ ] **Step 4: Add model truth badge**

Display exactly one of:

- `BIO FULL · complete MaleCNS`
- `MAX FULL · complete MaleCNS`
- `LITE · pruned MaleCNS subset`

- [ ] **Step 5: Browser E2E test**

Play one human move, wait for fly response, assert brain activation canvas updated before piece animation completes.

- [ ] **Step 6: Commit**

### Task 6: Package macOS and Windows local launchers

**Files:**
- Create: `.github/workflows/release.yml`
- Create: `scripts/package-local.sh`
- Create: `scripts/package-local.ps1`
- Modify: `crates/stockfly-server/src/main.rs`

**Interfaces:**
- Release bundles:
  - `stockfly-macos-arm64.zip`
  - `stockfly-windows-x64.zip`

- [ ] **Step 1: Build web assets and server in one command**

```bash
./scripts/package-local.sh
```

- [ ] **Step 2: Bundle local model manifests separately from raw MaleCNS data**

- [ ] **Step 3: Launcher opens default browser to localhost and prints the URL**

- [ ] **Step 4: Verify on macOS Chrome/Safari and Windows Chrome/Edge**

WebGPU failure must fall back instead of showing a blank screen.

- [ ] **Step 5: Commit**

### Task 7: Select trained Bio and Max models and show their training identity

User addition (2026-09-18): expose the actually trained models, not only Bio, and benchmark each separately. Lite remains unavailable until the Full causal prerequisite passes.

- [ ] Add a visible Bio Full / Max Full model selector using real local checkpoints; show unavailable states and explain why Lite is not available.
- [ ] Show actual checkpoint training preset and trial count, model kind, and hash/provenance; never infer training status from a filename.
- [ ] Switching models cancels old match/frame/decision/error/verification events, safely reloads the selected model, and resets the game. Preserve mode and side preferences when safe.
- [ ] Use a reusable local model catalog/asset arrangement that release packaging can reproduce; prefer the completed new Max quick checkpoint once available, without overwriting previous checkpoints or shared sources.
- [ ] Add meaningful model-selection/lifecycle and incompatible-kind tests; run affected checks and one narrow actual Max-loading browser scenario.
- [ ] Commit scoped implementation and report exact model identities tested.
