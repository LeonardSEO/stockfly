# StockFly Local Training Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Train StockFly Bio Full and Max Full locally on a 16 GB M4 MacBook within bounded wall-clock presets using Stockfish 19 Lite curriculum teaching and local plasticity.

**Architecture:** Stockfish.js 19 Lite generates a compact curriculum dataset. The native Rust trainer runs the same wgpu simulator used at inference, computes reward from teacher/game outcomes, and applies local activity-gated updates without storing a full recurrent autograd graph.

**Tech Stack:** Rust, wgpu, serde/JSONL or Arrow IPC, Node.js 20+, Stockfish.js 19 Lite.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- All weekend training must run on the M4/16 GB machine.
- Training presets are wall-clock bounded.
- Teacher information is training-only.
- Full model output mapping remains fixed.
- Bio and Max preserve graph topology and connection sign.

---

### Task 1: Vendor/fetch and isolate Stockfish 19 Lite teacher assets

**Files:**
- Create: `tools/teacher/fetch-stockfish.mjs`
- Create: `tools/teacher/stockfish.sha256.json`
- Create: `tools/teacher/uci-worker.mjs`
- Test: `tools/teacher/test-uci-worker.mjs`

**Interfaces:**
- Produces local files under `data/vendor/stockfish-19-lite/`.
- Exposes newline JSON stdin/stdout protocol:

```json
{"id":1,"fen":"...","nodes":2000,"multiPv":3}
```

Response:

```json
{"id":1,"bestmove":"e2e4","lines":[{"move":"e2e4","cp":31}]}
```

- [ ] **Step 1: Write protocol smoke test**

Launch worker, send start position with `nodes=100`, assert response has a legal UCI move.

- [ ] **Step 2: Run and verify failure**

```bash
node --test tools/teacher/test-uci-worker.mjs
```

- [ ] **Step 3: Implement hash-verified fetch**

Fetch the v19.0.0 lite single-threaded JS/WASM pair. Store expected SHA-256 values in source. Never silently accept a changed upstream asset.

- [ ] **Step 4: Implement worker isolation**

Only UCI requests enter this process. No StockFly model files are loaded by the teacher worker.

- [ ] **Step 5: Run test and commit**

```bash
node --test tools/teacher/test-uci-worker.mjs
git add tools/teacher
git commit -m "feat: add isolated Stockfish 19 Lite teacher"
```

### Task 2: Generate the staged chess curriculum

**Files:**
- Create: `tools/teacher/generate-curriculum.mjs`
- Create: `tools/teacher/curriculum-config.json`
- Create: `tests/fixtures/curriculum-small.jsonl`
- Test: `tools/teacher/test-curriculum.mjs`

**Interfaces:**
- Produces JSONL rows:

```json
{"fen":"...","bestmove":"...","stage":"mate1","nodes":2000,"seed":42}
```

- [ ] **Step 1: Write stage distribution test**

The fixture generator must emit examples for `legal`, `capture`, `check-evasion`, `mate1`, `endgame`, and `mixed`.

- [ ] **Step 2: Implement deterministic position generators**

Use seeded legal random play and hand-coded minimal-material generators. Do not scrape the web for weekend MVP.

- [ ] **Step 3: Label each position with teacher worker**

Default node budgets:

- legal/capture: 250 nodes
- check/mate1: 500 nodes
- endgame: 1,000 nodes
- mixed/full: 2,000 nodes

- [ ] **Step 4: Add presets**

`smoke=500`, `quick=5_000`, `standard=25_000`, `overnight=100_000` total examples. Stop on wall-clock budget even if target count is not reached.

- [ ] **Step 5: Run tests and generate smoke data**

```bash
node --test tools/teacher/test-curriculum.mjs
node tools/teacher/generate-curriculum.mjs --preset smoke --out data/teacher/smoke.jsonl
```

- [ ] **Step 6: Commit**

```bash
git add tools/teacher tests/fixtures/curriculum-small.jsonl
git commit -m "feat: generate local StockFly curriculum"
```

### Task 3: Implement plasticity masks and local three-factor update

**Files:**
- Create: `crates/stockfly-plasticity/Cargo.toml`
- Create: `crates/stockfly-plasticity/src/lib.rs`
- Create: `crates/stockfly-plasticity/src/bio.rs`
- Create: `crates/stockfly-plasticity/src/max.rs`
- Create: `shaders/plasticity.wgsl`
- Test: `crates/stockfly-plasticity/tests/update.rs`

**Interfaces:**
- Produces:

```rust
pub trait PlasticityRule {
    fn eligible(&self, edge_index: u64, metadata: &EdgeMetadata) -> bool;
    fn update(&mut self, pre: f32, post: f32, reward: f32, current_mag: f32) -> f32;
}
```

- [ ] **Step 1: Write sign-preservation tests**

Positive biological edge never becomes negative; inhibitory edge never becomes excitatory.

- [ ] **Step 2: Write reward-direction test**

For active pre/post neurons, positive reward increases magnitude and negative reward decreases it within configured clamps.

- [ ] **Step 3: Implement Bio mask**

Build eligibility from committed annotation selectors for mushroom-body / dopamine-related learning circuits. Emit exact eligible edge count at model creation.

- [ ] **Step 4: Implement Max rule**

All edges eligible. Activity threshold skips inactive edges. Clamp magnitude to `[0, max_multiplier * base_magnitude]`, default `max_multiplier=4.0`.

- [ ] **Step 5: Implement wgpu update kernel and CPU parity**

- [ ] **Step 6: Run tests and commit**

```bash
cargo test -p stockfly-plasticity
git add crates/stockfly-plasticity shaders/plasticity.wgsl
git commit -m "feat: add Bio and Max fly plasticity"
```

### Task 4: Build the M4 wall-clock trainer

**Files:**
- Create: `crates/stockfly-train/Cargo.toml`
- Create: `crates/stockfly-train/src/main.rs`
- Create: `crates/stockfly-train/src/train.rs`
- Create: `crates/stockfly-train/src/presets.rs`
- Create: `crates/stockfly-train/src/checkpoint.rs`
- Test: `crates/stockfly-train/tests/smoke_train.rs`

**Interfaces:**
- CLI:

```bash
stockfly-train train --kind bio-full --preset standard --curriculum data/teacher/standard.jsonl --out data/checkpoints/stockfly-bio-full.sfckpt
```

- [ ] **Step 1: Write a synthetic training test**

On a six-neuron fixture, 100 positive teacher trials must increase accuracy above its initial deterministic baseline.

- [ ] **Step 2: Run and verify failure**

- [ ] **Step 3: Implement trial loop**

Per example:

1. encode FEN
2. settle brain for preset steps
3. choose move from fixed outputs
4. reward `+1` if teacher move, `-0.25` otherwise
5. optionally shape reward by teacher top-k rank during training only
6. apply Bio or Max local update
7. reset or carry state according to curriculum stage

- [ ] **Step 4: Implement wall-clock governor**

Presets:

```text
smoke     10 min max,  8 settle steps
quick     30 min max, 12 settle steps
standard   2 h max,    16 settle steps
overnight  6 h max,    24 settle steps
```

At startup run a 20-position benchmark and choose batch/settle configuration that fits the deadline. Never allocate > 12 GiB total process RSS on the target machine; abort before OS memory pressure if the measured estimate exceeds the cap.

- [ ] **Step 5: Checkpoint every 5 minutes or 2,000 trials**

Checkpoint includes graph hash, sensory-map hash, output-map hash, plasticity mode, base weights hash, delta weights, calibration scalars, training counters, and RNG seed.

- [ ] **Step 6: Run `smoke` on the M4**

```bash
cargo run -p stockfly-train --release -- train --kind max-full --preset smoke --curriculum data/teacher/smoke.jsonl --out data/checkpoints/smoke.sfckpt
```

Expected: completes within 10 minutes and produces a loadable checkpoint.

- [ ] **Step 7: Commit**

```bash
git add crates/stockfly-train
git commit -m "feat: train StockFly locally with wall-clock budgets"
```

### Task 5: Add self-play and Stockfish-opponent refinement

**Files:**
- Create: `crates/stockfly-train/src/selfplay.rs`
- Create: `crates/stockfly-train/src/matchplay.rs`
- Test: `crates/stockfly-train/tests/selfplay.rs`

**Interfaces:**
- CLI:

```bash
stockfly-train refine --model data/checkpoints/stockfly-max-full.sfckpt --games 100 --opponents self,stockfish-250,stockfish-1000
```

- [ ] **Step 1: Write result-reward test**

Win `+1`, draw `0`, loss `-1`, with bounded per-move eligibility traces so final result can reinforce recent active edges.

- [ ] **Step 2: Implement self-play game runner**

No Stockfish process exists in pure self-play mode.

- [ ] **Step 3: Implement teacher-opponent mode**

Stockfish process returns only its own move to match runner. Its evaluation is available to trainer only behind `--training-shaping` and never passed to the simulator.

- [ ] **Step 4: Run 10-game smoke test and commit**

```bash
cargo test -p stockfly-train selfplay
git add crates/stockfly-train/src/selfplay.rs crates/stockfly-train/src/matchplay.rs
git commit -m "feat: refine StockFly with self play"
```

### Task 6: Publish trained checkpoints as GitHub Release assets

**Files:**
- Create: `tools/models/publish-release.mjs`
- Create: `tools/models/fetch-release.mjs`
- Create: `tools/models/release-manifest.schema.json`
- Test: `tools/models/test-fetch-release.mjs`

**Interfaces:**
- `publish-release.mjs` uploads `data/checkpoints/*.sfckpt`, `data/compiled/malecns-v1/**` (compiled, not raw), and geometry LODs to a GitHub Release, writing a `release-manifest.json` with per-file SHA-256, model kind, graph hash, and training preset used.
- `fetch-release.mjs --tag <release>` downloads and hash-verifies those assets into `data/compiled/` and `data/checkpoints/` for a fresh clone.

- [ ] **Step 1: Write a fetch test against a local fixture manifest/server**

Verify a corrupted download is rejected and no partial file is left in place.

- [ ] **Step 2: Run and confirm failure**

```bash
node --test tools/models/test-fetch-release.mjs
```

- [ ] **Step 3: Implement `fetch-release.mjs`**

Default to the latest published release; support `--tag` pinning; print total download size before fetching; never overwrite a file whose hash already matches.

- [ ] **Step 4: Implement `publish-release.mjs`**

Require explicit `--tag`; refuse to include anything under `data/raw/`; embed MaleCNS CC-BY attribution and Stockfish GPLv3 notice references in the manifest.

- [ ] **Step 5: Wire `stockfly-server` and the web app to offer "Download pretrained model" when `data/checkpoints/` is empty, instead of requiring `stockfly-train`**

- [ ] **Step 6: Run tests and commit**

```bash
node --test tools/models/test-fetch-release.mjs
git add tools/models/publish-release.mjs tools/models/fetch-release.mjs tools/models/release-manifest.schema.json tools/models/test-fetch-release.mjs
git commit -m "feat: publish and fetch trained StockFly checkpoints via GitHub Releases"
```
