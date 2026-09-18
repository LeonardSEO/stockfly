# StockFly Simulator & Chess Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run the full compiled MaleCNS graph on CPU and GPU, encode chess into real sensory populations, and decode moves from fixed neural populations without a trainable chess policy after the brain.

**Architecture:** A deterministic CPU backend establishes correctness. A wgpu backend uses the same model state and WGSL kernels for native Metal/DX12/Vulkan and browser WebGPU. Chess input and output maps are fixed/versioned artifacts.

**Tech Stack:** Rust, wgpu 30.x, WGSL, chess crate or chess.js-compatible FEN semantics, serde.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- Full inference must step the complete compiled graph.
- No trainable post-brain MLP or policy network.
- Fixed output populations are deterministic and versioned.
- Legal mask only removes illegal moves.

---

### Task 1: Implement the deterministic CPU reference simulator

**Files:**
- Create: `crates/stockfly-sim/Cargo.toml`
- Create: `crates/stockfly-sim/src/lib.rs`
- Create: `crates/stockfly-sim/src/cpu.rs`
- Create: `crates/stockfly-sim/src/state.rs`
- Test: `crates/stockfly-sim/tests/cpu_fixture.rs`

**Interfaces:**
- Consumes: `stockfly_connectome::Connectome`.
- Produces:

```rust
pub struct SimConfig { pub dt_ms: f32, pub settle_steps: u32, pub threshold: f32, pub decay: f32 }
pub struct BrainState { pub membrane: Vec<f32>, pub rate: Vec<f32> }
pub trait Simulator { fn step(&mut self, stimulus: &Stimulus) -> Result<FrameSummary>; }
```

- [ ] **Step 1: Write a four-neuron propagation test**

Stimulate neuron 0, verify excitation reaches neuron 2 and inhibitory edge suppresses neuron 3 at the documented step.

- [ ] **Step 2: Run and confirm failure**

```bash
cargo test -p stockfly-sim --test cpu_fixture
```

- [ ] **Step 3: Implement minimal LIF/rate update**

For each destination neuron `j`:

```text
input_j = sum(sign_e * magnitude_e * rate_src)
membrane_j = membrane_j * decay + input_j + stimulus_j
rate_j = max(0, membrane_j - threshold)
```

Keep the first implementation deliberately simple and documented. Biological refinements require separate benchmark evidence.

- [ ] **Step 4: Add state-reset and deterministic seed tests**

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p stockfly-sim
git add crates/stockfly-sim
git commit -m "feat: add deterministic MaleCNS CPU simulator"
```

### Task 2: Add native/browser wgpu simulation with CPU parity fixtures

**Files:**
- Create: `crates/stockfly-sim/src/gpu.rs`
- Create: `crates/stockfly-sim/src/gpu_buffers.rs`
- Create: `shaders/csc_gather.wgsl`
- Create: `shaders/lif_step.wgsl`
- Test: `crates/stockfly-sim/tests/gpu_parity.rs`

**Interfaces:**
- Produces: `GpuSimulator::new(connectome, config, adapter_options)` implementing `Simulator`.

- [ ] **Step 1: Write GPU parity test on the synthetic fixture**

Assert max absolute rate difference after 16 steps is `< 1e-4` against CPU.

- [ ] **Step 2: Run and verify failure**

```bash
cargo test -p stockfly-sim gpu_matches_cpu_fixture -- --nocapture
```

- [ ] **Step 3: Implement chunked CSC buffers**

Never bind an edge storage buffer larger than 64 MiB. Each chunk contains a destination range and its local offset table.

- [ ] **Step 4: Implement gather kernel**

One invocation computes one destination neuron by reading incoming source rates and signed magnitudes. Avoid atomics in the first implementation.

- [ ] **Step 5: Run parity test on Metal**

Expected: PASS within tolerance.

- [ ] **Step 6: Add real-graph hardware benchmark**

Command:

```bash
cargo run -p stockfly-sim --release --example benchmark -- data/compiled/malecns-v1 --steps 32
```

Output JSON fields: adapter, backend, neuron_count, edge_count, steps, elapsed_ms, peak_buffer_bytes, steps_per_second.

- [ ] **Step 7: Commit**

```bash
git add crates/stockfly-sim shaders
git commit -m "feat: accelerate MaleCNS simulation with wgpu"
```

### Task 3: Implement synthetic fly-eye chess input

**Files:**
- Create: `crates/stockfly-chess/Cargo.toml`
- Create: `crates/stockfly-chess/src/lib.rs`
- Create: `crates/stockfly-chess/src/sensory.rs`
- Create: `crates/stockfly-chess/src/board_pattern.rs`
- Create: `crates/stockfly-chess/resources/sensory-map.json`
- Test: `crates/stockfly-chess/tests/sensory.rs`

**Interfaces:**
- Produces:

```rust
pub fn encode_position(fen: &str, map: &SensoryMap) -> Result<Stimulus>;
```

- [ ] **Step 1: Write a test for exact reproducibility**

Same FEN + map must produce byte-identical stimulus values.

- [ ] **Step 2: Write a test that moving one piece changes only its board patch plus context signals**

- [ ] **Step 3: Run and confirm failure**

- [ ] **Step 4: Implement the encoder**

Rules:

- board squares map to fixed ommatidial/visual sensory groups
- 12 piece identities use fixed spectral/spatial codes
- side to move, castling, and en-passant use fixed auxiliary sensory groups
- no legal-move information enters the stimulus

- [ ] **Step 5: Save an encoder hash**

`SensoryMap` serializes to canonical JSON and exposes SHA-256 used in every checkpoint/trace.

- [ ] **Step 6: Run tests and commit**

```bash
cargo test -p stockfly-chess sensory
git add crates/stockfly-chess
git commit -m "feat: encode chess as fly sensory input"
```

### Task 4: Create fixed neural from/to/promotion populations

**Files:**
- Create: `crates/stockfly-chess/src/output_map.rs`
- Create: `tools/models/generate_output_map.rs`
- Create: `crates/stockfly-chess/resources/output-map.json`
- Test: `crates/stockfly-chess/tests/output_map.rs`

**Interfaces:**
- Produces:

```rust
pub struct OutputMap {
    pub from_groups: [Vec<u32>; 64],
    pub to_groups: [Vec<u32>; 64],
    pub promotion_groups: [Vec<u32>; 4],
}
```

- [ ] **Step 1: Write invariants test**

Assert all 132 groups are non-empty, disjoint, contain only eligible output neurons, and are stable for seed `0x53544F434B464C59`.

- [ ] **Step 2: Run and verify failure**

- [ ] **Step 3: Implement deterministic generator**

Filter eligible annotated descending/motor/downstream neurons, sort by body ID, deterministic shuffle with fixed seed, partition into balanced groups, write body IDs and dense indices.

- [ ] **Step 4: Commit generated map**

The map is project source, not a trained artifact. Any change requires a format/version bump.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test -p stockfly-chess output_map
git add crates/stockfly-chess tools/models/generate_output_map.rs
git commit -m "feat: define fixed neural chess outputs"
```

### Task 5: Decode a legal chess move from neural activity

**Files:**
- Create: `crates/stockfly-chess/src/policy.rs`
- Test: `crates/stockfly-chess/tests/policy.rs`

**Interfaces:**
- Produces:

```rust
pub struct MoveDecision {
    pub uci: String,
    pub legal_scores: Vec<(String, f32)>,
    pub from_rates: [f32; 64],
    pub to_rates: [f32; 64],
    pub promotion_rates: [f32; 4],
}

pub fn choose_move(position: &Position, brain_rates: &[f32], map: &OutputMap) -> Result<MoveDecision>;
```

- [ ] **Step 1: Write legal-mask test**

Construct rates whose globally highest `from+to` pair is illegal and verify the best legal pair is chosen.

- [ ] **Step 2: Write stable tie-break test**

Equal scores choose lexicographically smallest UCI move after stable legal move ordering.

- [ ] **Step 3: Implement score formula exactly as spec**

No Stockfish evaluation and no chess heuristic term is allowed.

- [ ] **Step 4: Run tests**

```bash
cargo test -p stockfly-chess policy
```

- [ ] **Step 5: Add end-to-end CLI smoke command**

```bash
cargo run -p stockfly-train -- infer-untrained --fen "startpos"
```

It must print model hash, top neural groups, legal move scores, and selected move.

- [ ] **Step 6: Commit**

```bash
git add crates/stockfly-chess
git commit -m "feat: choose legal moves from fixed brain outputs"
```
