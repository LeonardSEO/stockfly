# StockFly Causal Audit & Strength Benchmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce reproducible evidence that Full StockFly move quality depends on the complete MaleCNS simulation, then measure its actual chess strength without inflating claims.

**Architecture:** The benchmark CLI executes the same positions/games under intact, shuffled, reset, ablated, and bypass conditions. Results are emitted as machine-readable JSON and summarized in the browser.

**Tech Stack:** Rust, serde_json, statistical bootstrap helper, browser report viewer.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- Never assign an Elo from puzzle accuracy alone.
- Every report records model and graph hashes.
- Stockfish inference isolation is tested by physically removing teacher assets.

---

### Task 1: Define the signed move-trace and audit report format

**Files:**
- Modify: `crates/stockfly-types/src/lib.rs`
- Create: `crates/stockfly-types/src/audit.rs`
- Test: `crates/stockfly-types/tests/audit_roundtrip.rs`

**Interfaces:**

```rust
pub struct MoveTrace {
    pub trace_version: u32,
    pub model_sha256: String,
    pub graph_sha256: String,
    pub sensory_sha256: String,
    pub output_map_sha256: String,
    pub fen: String,
    pub selected_move: String,
    pub from_rates: [f32; 64],
    pub to_rates: [f32; 64],
    pub promotion_rates: [f32; 4],
    pub sampled_frames_sha256: String,
}
```

- [ ] **Step 1: Write canonical serialization hash test**
- [ ] **Step 2: Implement canonical field ordering/versioning**
- [ ] **Step 3: Run and commit**

### Task 2: Prove Stockfish absence during inference

**Files:**
- Create: `crates/stockfly-train/src/audit/no_teacher.rs`
- Test: `crates/stockfly-train/tests/no_teacher.rs`

**Interfaces:**
- `causal-audit --check no-teacher` runs a fixed FEN suite twice: with teacher assets available and after moving them out of the search path.

- [ ] **Step 1: Write test fixture where a fake teacher would change a move if called**
- [ ] **Step 2: Ensure production inference result is identical with fake teacher missing/present**
- [ ] **Step 3: Add file-access logging for the inference process**
- [ ] **Step 4: Commit**

### Task 3: Implement graph and learning controls

**Files:**
- Create: `crates/stockfly-train/src/audit/shuffle.rs`
- Create: `crates/stockfly-train/src/audit/ablation.rs`
- Create: `crates/stockfly-train/src/audit/reset.rs`
- Create: `crates/stockfly-train/src/audit/output_permutation.rs`
- Test: `crates/stockfly-train/tests/controls.rs`

**Interfaces:**
- Controls accept a model checkpoint and emit a derived in-memory variant without overwriting the checkpoint.

- [ ] **Step 1: Degree-aware shuffled graph control**

Shuffle source IDs within compatible transmitter/sign strata while keeping destination fan-in distribution close to intact graph. Record exact seed and graph statistics.

- [ ] **Step 2: Weight reset control**

Replace learned magnitude deltas with zero, retaining all calibration settings separately reported.

- [ ] **Step 3: Region ablation control**

Set outgoing rates of selected annotated neurons to zero during inference; do not edit graph files.

- [ ] **Step 4: Output permutation control**

Permute fixed square-group labels while leaving neuron activity unchanged.

- [ ] **Step 5: Test all variants are non-destructive and commit**

### Task 4: Build tactical and policy benchmark suites

**Files:**
- Create: `tests/fixtures/chess-suite.json`
- Create: `crates/stockfly-train/src/eval_suite.rs`
- Test: `crates/stockfly-train/tests/eval_suite.rs`

**Interfaces:**
- Suite sections: legal, check-evasion, mate1, capture, tactical, endgame, mixed.

- [ ] **Step 1: Create at least 25 hand-verified positions per basic section**

For tactical/mixed teacher targets, store the exact Stockfish 19 Lite node budget used to define the target.

- [ ] **Step 2: Implement metrics**

- top-1 teacher agreement
- top-3 teacher agreement
- mate1 success
- legal-before-mask rate
- average chosen move teacher centipawn loss for diagnostics only
- latency and simulation steps

- [ ] **Step 3: Emit JSON and Markdown report**
- [ ] **Step 4: Commit**

### Task 5: Measure playing strength with a fixed ladder

**Files:**
- Create: `crates/stockfly-train/src/elo.rs`
- Create: `crates/stockfly-train/resources/ladder.json`
- Test: `crates/stockfly-train/tests/elo.rs`

**Interfaces:**
- Ladder uses Stockfish 19 Lite with fixed node budgets, not claimed Stockfish Elo labels.
- Report match score per budget and derive a local ladder rating from observed results.

- [ ] **Step 1: Define opponents**

Example fixed budgets: 50, 100, 250, 500, 1,000, 2,000, 5,000 nodes per move. Keep this mapping as a project-local strength ladder, not universal Elo.

- [ ] **Step 2: Run color-balanced matches**

Minimum weekend report: 20 games/opponent for coarse signal. Publish wide uncertainty. Later release target: >=100 games/opponent around the model's crossing point.

- [ ] **Step 3: Implement bootstrap confidence interval for score rate**
- [ ] **Step 4: Produce intact vs control comparison table**
- [ ] **Step 5: Commit**

### Task 6: Generate a one-command evidence report

**Files:**
- Create: `crates/stockfly-train/src/audit/mod.rs`
- Create: `apps/web/src/traces/AuditReport.ts`

**Interfaces:**

```bash
cargo run -p stockfly-train --release -- causal-audit \
  --model data/checkpoints/stockfly-max-full.sfckpt \
  --suite tests/fixtures/chess-suite.json \
  --out data/reports/max-full-audit
```

- [ ] **Step 1: Run intact suite**
- [ ] **Step 2: Run no-teacher, reset, shuffled, output-permutation controls**
- [ ] **Step 3: Rank regions by simple ablation sensitivity and test top 10**
- [ ] **Step 4: Hash all report inputs/outputs**
- [ ] **Step 5: Browser report clearly separates measured facts from interpretation**
- [ ] **Step 6: Commit**
