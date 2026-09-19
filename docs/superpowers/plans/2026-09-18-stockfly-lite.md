# StockFly Lite Distillation Implementation Plan

> **Cancelled by the user (2026-09-19):** StockFly Lite is no longer product scope. This document is preserved as historical planning evidence and must not be implemented.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a smaller browser-friendly StockFly Lite by pruning and quantizing the trained Full connectome while preserving original MaleCNS neuron/edge identities for every retained element.

**Architecture:** Lite is a causally selected MaleCNS subgraph, not a generic neural network. Full traces and ablations rank contribution; low-contribution structure is pruned, then retained weights are quantized and briefly retrained.

**Tech Stack:** Rust, wgpu, existing audit/training pipeline.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- Start only after Full causal audit passes.
- Lite may never be labeled or visualized as complete MaleCNS.
- Every retained neuron and edge must map back to original MaleCNS IDs.

---

### Task 1: Collect causal saliency from Full traces

**Files:**
- Create: `crates/stockfly-train/src/lite/saliency.rs`
- Test: `crates/stockfly-train/tests/lite_saliency.rs`

**Interfaces:**
- Per-neuron score combines activation frequency, output-path contribution, and ablation delta.
- Per-edge score combines active eligibility magnitude and endpoint scores.

- [ ] **Step 1: Write ranking test on a synthetic graph with one known necessary path**
- [ ] **Step 2: Implement deterministic score aggregation**
- [ ] **Step 3: Commit**

### Task 2: Prune a connected MaleCNS subgraph

**Files:**
- Create: `crates/stockfly-train/src/lite/prune.rs`
- Test: `crates/stockfly-train/tests/lite_prune.rs`

**Interfaces:**
- CLI targets `--neurons 50000`, `--neurons 25000`, `--neurons 10000`.

- [ ] **Step 1: Preserve all sensory and output population neurons**
- [ ] **Step 2: Preserve graph paths required to connect sensory roots to outputs**
- [ ] **Step 3: Fill remaining budget by saliency**
- [ ] **Step 4: Re-index while storing original dense index/body ID**
- [ ] **Step 5: Commit**

### Task 3: Quantize and benchmark Lite

**Files:**
- Create: `crates/stockfly-connectome/src/quantized.rs`
- Create: `crates/stockfly-train/src/lite/quantize.rs`
- Test: `crates/stockfly-train/tests/lite_quantize.rs`

**Interfaces:**
- Weight magnitude: signed biological sign stored separately; magnitude quantized to `u16` per block scale.

- [ ] **Step 1: Write quantization error test**
- [ ] **Step 2: Implement per-block scale quantization**
- [ ] **Step 3: Benchmark model file size, load time, move latency, and strength loss**
- [ ] **Step 4: Commit**

### Task 4: Retrain and publish Lite truth metadata

**Files:**
- Modify: `crates/stockfly-train/src/train.rs`
- Modify: `apps/web/src/ui/DecisionPanel.ts`
- Create: `data/checkpoints/README.md`

**Interfaces:**
- Lite manifest records exact retained neuron count, edge count, Full parent hash, pruning config, and quantization config.

- [ ] **Step 1: Run `quick` retraining on each candidate size**
- [ ] **Step 2: Choose smallest candidate retaining an acceptable fraction of Full benchmark score**
- [ ] **Step 3: Run causal audit on Lite separately**
- [ ] **Step 4: UI shows `LITE · N neurons · M edges`, never `complete MaleCNS`**
- [ ] **Step 5: Commit**
