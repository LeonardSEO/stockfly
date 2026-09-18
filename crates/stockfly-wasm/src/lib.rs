//! Browser bindings for the StockFly simulator: loads the compiled graph
//! and neural maps from bytes fetched over HTTP (no filesystem access in
//! WASM), then runs the exact same encode -> settle -> decide pipeline the
//! native `stockfly-train` CLI uses. This is deliberately a thin wrapper
//! around the same `stockfly-connectome` / `stockfly-sim` / `stockfly-chess`
//! crates the native path uses, not a reimplementation -- so the browser's
//! displayed activation is provably the same computation that chose the
//! move, not a separate approximation.
//!
//! No teacher/Stockfish API is exposed here at all: this crate has no
//! dependency on `tools/teacher`, and its only inputs are graph bytes, map
//! JSON, and a FEN string.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use stockfly_chess::output_map::OutputMap;
use stockfly_chess::policy::choose_move;
use stockfly_chess::sensory::{encode_position, SensoryMap};
use stockfly_connectome::Connectome;
use stockfly_sim::{CpuSimulator, SimConfig, Simulator};

#[wasm_bindgen]
pub struct StockFlyEngine {
    connectome: Connectome,
    sensory_map: SensoryMap,
    output_map: OutputMap,
}

#[derive(Serialize)]
struct InferResponse {
    selected_move: String,
    from_rates: Vec<f32>,
    to_rates: Vec<f32>,
    promotion_rates: Vec<f32>,
    top_neurons: Vec<(u32, f32)>,
    graph_neurons_sha256: String,
    sensory_map_sha256: String,
    output_map_sha256: String,
    settle_steps: u32,
    neuron_count: usize,
    edge_count: usize,
}

#[wasm_bindgen]
impl StockFlyEngine {
    /// `edge_src_bytes` / `edge_weight_bytes` are the concatenation of every
    /// `edge_src_blocks/*.bin` / `edge_weight_blocks/*.bin` file in block-
    /// index order -- the 64 MiB-per-block chunking exists for the GPU
    /// storage-buffer limit, not this load step, so the JS caller
    /// concatenates fetched blocks before calling `new`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        manifest_json: &str,
        neurons_bytes: &[u8],
        offsets_bytes: &[u8],
        edge_src_bytes: &[u8],
        edge_weight_bytes: &[u8],
        sensory_map_json: &str,
        output_map_json: &str,
    ) -> Result<StockFlyEngine, JsValue> {
        let manifest = serde_json::from_str(manifest_json).map_err(js_err)?;
        let connectome = Connectome::from_parts(
            manifest,
            neurons_bytes,
            offsets_bytes,
            std::iter::once((edge_src_bytes, edge_weight_bytes)),
        )
        .map_err(|e| js_err(e.to_string()))?;

        let sensory_map = SensoryMap::load_str(sensory_map_json).map_err(|e| js_err(e.to_string()))?;
        let output_map = OutputMap::load_str(output_map_json).map_err(|e| js_err(e.to_string()))?;

        Ok(StockFlyEngine {
            connectome,
            sensory_map,
            output_map,
        })
    }

    pub fn neuron_count(&self) -> usize {
        self.connectome.neurons.len()
    }

    pub fn edge_count(&self) -> usize {
        self.connectome.edge_src.len()
    }

    /// Runs the full encode -> settle -> decide pipeline for one position
    /// and returns a JSON string (parsed with `JSON.parse` on the JS side)
    /// containing the selected move and the exact neural readouts that
    /// chose it.
    pub fn infer(&self, fen: &str, settle_steps: u32) -> Result<String, JsValue> {
        let neuron_count = self.connectome.neurons.len();
        let stimulus =
            encode_position(fen, &self.sensory_map, neuron_count).map_err(|e| js_err(e.to_string()))?;

        let config = SimConfig {
            settle_steps,
            ..SimConfig::default()
        };
        let mut sim = CpuSimulator::new(&self.connectome, config);
        for _ in 0..settle_steps {
            sim.step(&stimulus);
        }
        let rates = &sim.state().rate;

        let decision = choose_move(fen, rates, &self.output_map).map_err(|e| js_err(e.to_string()))?;

        let mut top_neurons: Vec<(u32, f32)> = rates
            .iter()
            .enumerate()
            .map(|(i, &r)| (i as u32, r))
            .filter(|(_, r)| *r > 0.0)
            .collect();
        top_neurons.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        top_neurons.truncate(64);

        let response = InferResponse {
            selected_move: decision.uci,
            from_rates: decision.from_rates.to_vec(),
            to_rates: decision.to_rates.to_vec(),
            promotion_rates: decision.promotion_rates.to_vec(),
            top_neurons,
            graph_neurons_sha256: self.connectome.manifest.neurons_sha256.clone(),
            sensory_map_sha256: self.sensory_map.sha256().to_string(),
            output_map_sha256: self.output_map.sha256().to_string(),
            settle_steps,
            neuron_count,
            edge_count: self.connectome.edge_src.len(),
        };

        serde_json::to_string(&response).map_err(|e| js_err(e.to_string()))
    }
}

fn js_err(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
}
