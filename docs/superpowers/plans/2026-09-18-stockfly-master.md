# StockFly Master Implementation Plan

> **Cancelled by the user (2026-09-19):** StockFly Lite is no longer product scope. This document is preserved as historical planning evidence; its original Lite instructions must not be implemented.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a local browser chess application where a complete MaleCNS fruit-fly connectome chooses moves, can be trained on a 16 GB M4 MacBook, and exposes live causal neural visualization.

**Architecture:** A Rust workspace owns the canonical graph, simulator, plasticity, model format, trainer, WASM bindings, and localhost server. Native and browser simulation share `wgpu` compute kernels, so macOS uses Metal, Windows uses DX12/Vulkan, and browsers use WebGPU. A TypeScript/Vite/Three.js UI runs StockFly and Stockfish in separate workers and visualizes the same neural state used for move selection.

**Tech Stack:** Rust stable, wgpu 30.x, WGSL, wasm-bindgen, TypeScript, Vite, Three.js, chess.js, Node.js 20+, Python 3.12 + pyarrow only for official MaleCNS preprocessing, Stockfish.js 19 Lite, Janelia MaleCNS v1.0.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- Training target machine: Apple M4 MacBook, 16 GB unified memory.
- External GPU/server infrastructure is optional and never required.
- Weekend path uses local reward-modulated plasticity, not full BPTT through the entire graph.
- Full inference may not consume Stockfish evaluations, candidate moves, or policy hints.
- Full output populations are fixed and non-trainable.
- Graph topology and biological connection sign are immutable after compile.
- `StockFly Bio` restricts plastic edges; `StockFly Max` allows every edge magnitude to learn.
- Full and Lite labels and metrics must never be mixed.
- Browser runtime must work locally on macOS and Windows.
- Brain rendering must be sourced from actual simulator activation.
- Raw MaleCNS data is never committed.
- Stockfish and MaleCNS licensing/attribution must be preserved.
- Trained checkpoints and compiled graph artifacts must be publishable as GitHub Release assets so a fresh clone can play without training locally first.
- Training on the M4 (Bio/Max, all presets) happens entirely on this machine; no external GPU/server is required, per the confirmed decision to train fully locally (option A).

---

## Execution order

This master plan is intentionally split into six independently testable subplans:

1. `2026-09-18-stockfly-foundation-connectome.md`
2. `2026-09-18-stockfly-simulator-policy.md`
3. `2026-09-18-stockfly-training.md`
4. `2026-09-18-stockfly-browser-visualization.md`
5. `2026-09-18-stockfly-audit-benchmark.md`
6. `2026-09-18-stockfly-lite.md`

Do not start Lite work until the Full causal audit passes.

## Weekend critical path

### Friday: prove the full graph runs

- [ ] Complete Foundation & Connectome tasks 1-4.
- [ ] Complete Simulator & Policy tasks 1-4.
- [ ] Run the full compiled graph on the M4 Metal backend.
- [ ] Record positions/sec, step latency, peak RSS, and GPU buffer size.

**Friday gate:** one deterministic chess position must enter the sensory encoder, propagate through the full graph, and produce fixed from/to population logits.

### Saturday morning: teach it enough chess to be measurable

- [ ] Complete Training tasks 1-4.
- [ ] Generate the first curriculum dataset with Stockfish 19 Lite.
- [ ] Run `smoke`, `quick`, then `standard` training presets.
- [ ] Save `stockfly-bio-full.sfckpt` and `stockfly-max-full.sfckpt`.

**Saturday noon gate:** trained Full must beat its own untrained baseline on teacher top-1 accuracy and mate-in-1 suite.

### Saturday afternoon/evening: build the viral demo

- [ ] Complete Browser & Visualization tasks 1-5.
- [ ] Run Human vs StockFly locally.
- [ ] Run Stockfish 19 Lite vs StockFly locally.
- [ ] Confirm displayed activation is sourced from worker simulation frames.

**Saturday gate:** a user can watch the full brain activate and see the chosen from/to populations before the chess piece moves.

### Sunday: prove the claim, tune strength

- [ ] Complete Audit & Benchmark tasks 1-5.
- [ ] Run no-teacher, shuffle, reset, output-permutation, and ablation controls.
- [ ] Run a fixed reference match ladder.
- [ ] Spend remaining time on Bio/Max hyperparameter sweeps within the M4 wall-clock budget.

**Sunday gate:** publishable local demo plus an evidence report that distinguishes what the full connectome contributes from what the adapters contribute.

## Cut line for the first weekend

If time is short, defer in this order:

1. Lite model.
2. advanced region filters and polished brain rendering.
3. self-play reinforcement beyond a small validation run.
4. Bio circuit expansion beyond the initial curated mask.
5. Windows native training. Windows browser inference remains required.

Never cut:

- complete Full graph path
- fixed output populations
- Stockfish isolation
- live true activation
- causal shuffle/no-teacher control
- local M4 training path

## Final verification commands

```bash
cargo test --workspace
cargo run -p stockfly-connectome -- validate data/compiled/malecns-v1
cargo run -p stockfly-train -- benchmark-hardware --model data/checkpoints/stockfly-max-full.sfckpt
cargo run -p stockfly-train -- eval-suite --model data/checkpoints/stockfly-max-full.sfckpt --suite tests/fixtures/chess-suite.json
cargo run -p stockfly-train -- causal-audit --model data/checkpoints/stockfly-max-full.sfckpt
npm --prefix apps/web test
npm --prefix apps/web run build
cargo run -p stockfly-server -- --web-dir apps/web/dist --model-dir data/checkpoints
```

Expected final state:

- all Rust tests pass
- all web tests pass
- full model loads every compiled neuron/edge
- browser app works offline after assets are installed
- Human vs Fly works
- Stockfish vs Fly works
- causal audit emits a signed JSON report with no critical failures
