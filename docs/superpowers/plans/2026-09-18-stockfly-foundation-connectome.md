# StockFly Foundation & Connectome Implementation Plan

> **Cancelled by the user (2026-09-19):** StockFly Lite is no longer product scope. This document is preserved as historical planning evidence; its original Lite instructions must not be implemented.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the reproducible repository foundation and compile official MaleCNS v1.0 data into a validated browser/native-friendly graph artifact.

**Architecture:** Official Feather/annotation/skeleton data is downloaded by a Python preprocessing tool, verified by hashes, and compiled into a destination-major chunked binary format. Rust loads that format and enforces graph invariants before simulation can start.

**Tech Stack:** Rust stable, serde, bytemuck, memmap2, Python 3.12, pyarrow, requests, pytest.

**Spec:** `docs/superpowers/specs/2026-09-18-stockfly-design.md`

## Global Constraints

- Do not commit raw MaleCNS files.
- Preserve stable original body IDs alongside dense indices.
- Preserve every connection retained by the official flat connectome compiler rules.
- Preserve connection signs separately from trainable magnitudes.
- Chunk edge data to <= 64 MiB blocks.

---

### Task 1: Bootstrap the monorepo and third-party boundaries

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `package.json`
- Create: `LICENSE`
- Create: `THIRD_PARTY_NOTICES.md`
- Create: `crates/stockfly-types/Cargo.toml`
- Create: `crates/stockfly-types/src/lib.rs`
- Test: `crates/stockfly-types/src/lib.rs`

**Interfaces:**
- Produces: `ModelKind`, `NeuronId`, `DenseNeuronIndex`, `MoveTraceHeader`, `ModelManifest`.

- [ ] **Step 1: Write the failing serialization test**

```rust
#[test]
fn model_manifest_round_trips_json() {
    let m = ModelManifest::test_fixture();
    let json = serde_json::to_string(&m).unwrap();
    let restored: ModelManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(m, restored);
}
```

- [ ] **Step 2: Run the test and verify failure**

Run:

```bash
cargo test -p stockfly-types model_manifest_round_trips_json
```

Expected: compile failure because `ModelManifest` does not exist.

- [ ] **Step 3: Implement the minimal shared types**

Define:

```rust
pub enum ModelKind { BioFull, MaxFull, Lite }

pub struct NeuronId(pub u64);
pub struct DenseNeuronIndex(pub u32);

pub struct ModelManifest {
    pub format_version: u32,
    pub model_kind: ModelKind,
    pub dataset: String,
    pub neuron_count: u32,
    pub edge_count: u64,
    pub graph_sha256: String,
    pub output_map_sha256: String,
}
```

Add `Serialize`, `Deserialize`, `PartialEq`, `Eq`, and `test_fixture()` behind `#[cfg(test)]`.

- [ ] **Step 4: Re-run the test**

Expected: PASS.

- [ ] **Step 5: Add licensing boundaries**

`THIRD_PARTY_NOTICES.md` must explicitly name Janelia MaleCNS v1.0 / CC-BY and Stockfish.js 19 / GPLv3, with source URLs and a note that raw MaleCNS data is fetched, not vendored.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml package.json .gitignore LICENSE THIRD_PARTY_NOTICES.md crates/stockfly-types
git commit -m "chore: bootstrap StockFly workspace"
```

### Task 2: Fetch official MaleCNS files reproducibly

**Files:**
- Create: `tools/malecns/requirements.txt`
- Create: `tools/malecns/manifest.json`
- Create: `tools/malecns/fetch.py`
- Test: `tools/malecns/test_fetch.py`

**Interfaces:**
- Produces: `data/raw/malecns-v1/<filename>` files whose hashes match `manifest.json`.

- [ ] **Step 1: Write the failing manifest validation test**

```python
def test_manifest_has_required_artifacts():
    manifest = load_manifest("tools/malecns/manifest.json")
    names = {x["name"] for x in manifest["artifacts"]}
    assert "connectome_weights" in names
    assert "annotations" in names
    assert "neurotransmitters" in names
```

- [ ] **Step 2: Run it**

```bash
python -m pytest tools/malecns/test_fetch.py -q
```

Expected: FAIL because loader/manifest do not exist.

- [ ] **Step 3: Implement fetch semantics**

`fetch.py` must:

- read `manifest.json`
- download only missing artifacts
- stream SHA-256 while writing to `*.partial`
- atomically rename only after successful hash verification
- print total bytes required before download
- support `--list`, `--verify`, `--fetch graph`, and `--fetch geometry`

- [ ] **Step 4: Add an offline unit test using a local temporary file URL**

Verify a bad hash leaves no final artifact.

- [ ] **Step 5: Run tests**

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add tools/malecns
git commit -m "feat: add reproducible MaleCNS fetcher"
```

### Task 3: Compile MaleCNS into a chunked CSC artifact

**Files:**
- Create: `tools/malecns/compile.py`
- Create: `tools/malecns/format.py`
- Create: `tools/malecns/test_compile.py`
- Create: `crates/stockfly-connectome/Cargo.toml`
- Create: `crates/stockfly-connectome/src/lib.rs`
- Create: `crates/stockfly-connectome/src/format.rs`
- Test: `crates/stockfly-connectome/tests/load_fixture.rs`

**Interfaces:**
- Produces compiled directory:
  - `manifest.json`
  - `neurons.bin`
  - `offsets.bin`
  - `edge_src_blocks/*.bin`
  - `edge_weight_blocks/*.bin`
  - `metadata.bin`

- [ ] **Step 1: Create a six-neuron synthetic Feather fixture in the Python test**

The fixture must include excitatory, inhibitory, and zero/unknown transmitter examples plus duplicate source-destination rows to verify aggregation semantics.

- [ ] **Step 2: Assert expected CSC ordering and sign separation**

Expected destination-major order and exact summed magnitudes for the fixture.

- [ ] **Step 3: Run Python test and verify failure**

```bash
python -m pytest tools/malecns/test_compile.py -q
```

- [ ] **Step 4: Implement `compile.py`**

Rules:

- stable dense index ordered by original body ID
- aggregate duplicate `(src,dst)` connections
- write immutable sign as `i8`
- write base magnitude as `f32` in training artifact
- emit quantized `f16`/`i16` runtime artifact later, not now
- split edge arrays at 64 MiB
- emit SHA-256 per file into manifest

- [ ] **Step 5: Implement Rust reader**

Expose:

```rust
pub struct Connectome {
    pub neurons: Vec<NeuronRecord>,
    pub csc_offsets: Vec<u64>,
    pub edge_blocks: Vec<EdgeBlock>,
}

impl Connectome {
    pub fn open(path: impl AsRef<Path>) -> Result<Self>;
    pub fn validate(&self) -> Result<ValidationReport>;
}
```

- [ ] **Step 6: Run Python and Rust fixture tests**

```bash
python -m pytest tools/malecns/test_compile.py -q
cargo test -p stockfly-connectome
```

Expected: PASS.

- [ ] **Step 7: Compile real graph and validate counts/hashes**

```bash
python tools/malecns/fetch.py --fetch graph
python tools/malecns/compile.py --input data/raw/malecns-v1 --output data/compiled/malecns-v1
cargo run -p stockfly-connectome --example validate -- data/compiled/malecns-v1
```

Record real counts in `data/compiled/malecns-v1/manifest.json`; never hard-code approximate counts into tests.

- [ ] **Step 8: Commit**

```bash
git add tools/malecns crates/stockfly-connectome
git commit -m "feat: compile MaleCNS graph for StockFly"
```

### Task 4: Build rendering geometry artifacts without loading full skeletons at runtime

**Files:**
- Create: `tools/malecns/geometry.py`
- Create: `tools/malecns/test_geometry.py`
- Modify: `tools/malecns/manifest.json`

**Interfaces:**
- Produces: `geometry_lod0.bin`, `geometry_lod1.bin`, `geometry_index.bin` keyed by dense neuron index.

- [ ] **Step 1: Write a test that a neuron has stable geometry ranges in both LODs**

```python
def test_geometry_index_points_to_each_neuron():
    lod0, lod1, index = compile_fixture_geometry()
    assert index[3].lod0_count > 0
    assert index[3].lod1_count <= index[3].lod0_count
```

- [ ] **Step 2: Run test and verify failure**

- [ ] **Step 3: Implement geometry compiler**

`lod0`: soma/centroid + coarse path points suitable for always-on view.

`lod1`: more detailed downsampled skeleton used when zoomed/selected.

Each vertex stores neuron dense index so the renderer can color from activation textures/buffers without duplicating geometry per frame.

- [ ] **Step 4: Enforce memory budget**

Compiler fails with a clear message if default `lod0` exceeds 64 MiB or `lod1` exceeds 256 MiB. Allow CLI overrides but keep defaults safe for 16 GB systems.

- [ ] **Step 5: Run fixture and real compile**

```bash
python -m pytest tools/malecns/test_geometry.py -q
python tools/malecns/geometry.py --input data/raw/malecns-v1 --compiled data/compiled/malecns-v1
```

- [ ] **Step 6: Commit**

```bash
git add tools/malecns/geometry.py tools/malecns/test_geometry.py
git commit -m "feat: compile browser brain geometry"
```
