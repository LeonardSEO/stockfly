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
use stockfly_sim::{CpuSimulator, GpuAdapterOptions, GpuSimulator, SimConfig, Simulator};
use stockfly_train::checkpoint::Checkpoint;

#[wasm_bindgen]
pub struct StockFlyEngine {
    connectome: Connectome,
    sensory_map: SensoryMap,
    output_map: OutputMap,
    /// Working weights (compiled base magnitude * SimConfig::weight_scale,
    /// matching exactly what the native trainer computed its checkpoint
    /// deltas against). Starts as the untrained baseline; `load_checkpoint`
    /// overwrites the edges a checkpoint touched.
    weights: Vec<f32>,
    /// Human-readable label for whichever checkpoint (if any) is loaded,
    /// shown in the UI's model badge.
    model_label: String,
    gpu: Option<GpuSimulator>,
    gpu_fallback_reason: Option<String>,
}

#[derive(Serialize)]
struct InferResponse {
    selected_move: String,
    legal_scores: Vec<(String, f32)>,
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
    model_label: String,
    backend: String,
    adapter: String,
    gpu_fallback_reason: Option<String>,
}

#[derive(Serialize)]
struct FrameDecisionResponse {
    selected_move: String,
    legal_scores: Vec<(String, f32)>,
    from_rates: Vec<f32>,
    to_rates: Vec<f32>,
    promotion_rates: Vec<f32>,
}

#[derive(Serialize)]
struct BackendResponse {
    backend: String,
    adapter: String,
    fallback_reason: Option<String>,
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

        let sensory_map =
            SensoryMap::load_str(sensory_map_json).map_err(|e| js_err(e.to_string()))?;
        let output_map = OutputMap::load_str(output_map_json).map_err(|e| js_err(e.to_string()))?;

        let weight_scale = SimConfig::default().weight_scale;
        let weights = connectome
            .edge_weight
            .iter()
            .map(|w| w * weight_scale)
            .collect();

        Ok(StockFlyEngine {
            connectome,
            sensory_map,
            output_map,
            weights,
            model_label: "untrained baseline".to_string(),
            gpu: None,
            gpu_fallback_reason: Some("WebGPU initialization has not run".to_string()),
        })
    }

    pub fn neuron_count(&self) -> usize {
        self.connectome.neurons.len()
    }

    pub fn edge_count(&self) -> usize {
        self.connectome.edge_src.len()
    }

    pub fn model_label(&self) -> String {
        self.model_label.clone()
    }

    /// Applies a trained checkpoint's deltas onto this engine's working
    /// weights. Returns the number of edges the checkpoint actually
    /// touched. The compiled base graph itself is never mutated -- this
    /// only changes `self.weights`, the same design as the native
    /// trainer's `Checkpoint::apply_to`.
    pub fn load_checkpoint(&mut self, checkpoint_json: &str) -> Result<usize, JsValue> {
        let checkpoint: Checkpoint =
            serde_json::from_str(checkpoint_json).map_err(|e| js_err(e.to_string()))?;
        if checkpoint.format_version != 1
            || checkpoint.graph_neurons_sha256 != self.connectome.manifest.neurons_sha256
            || checkpoint.sensory_map_sha256 != self.sensory_map.sha256()
            || checkpoint.output_map_sha256 != self.output_map.sha256()
            || checkpoint
                .deltas
                .iter()
                .any(|(index, value)| *index as usize >= self.weights.len() || !value.is_finite())
        {
            return Err(js_err(
                "checkpoint is incompatible with the loaded graph/maps",
            ));
        }
        let touched = checkpoint.apply_to(&mut self.weights);
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.set_weights(&self.connectome, &self.weights)
                .map_err(|error| js_err(error.to_string()))?;
        }
        self.model_label = checkpoint.model_kind.clone();
        Ok(touched)
    }

    /// Requests a browser WebGPU device and uploads the current learned
    /// weights. Failures are represented as an explicit CPU/WASM backend so
    /// the worker can display the fallback rather than mislabeling it as GPU.
    pub async fn initialize_gpu(&mut self) -> Result<String, JsValue> {
        let result = GpuSimulator::new_with_weights_async(
            &self.connectome,
            SimConfig::default(),
            GpuAdapterOptions::default(),
            &self.weights,
        )
        .await;
        let response = match result {
            Ok(gpu) => {
                let response = BackendResponse {
                    backend: gpu.backend().to_string(),
                    adapter: gpu.adapter_name().to_string(),
                    fallback_reason: None,
                };
                self.gpu = Some(gpu);
                self.gpu_fallback_reason = None;
                response
            }
            Err(error) => {
                let reason = error.to_string();
                self.gpu = None;
                self.gpu_fallback_reason = Some(reason.clone());
                BackendResponse {
                    backend: "cpu-wasm".to_string(),
                    adapter: "CPU/WASM".to_string(),
                    fallback_reason: Some(reason),
                }
            }
        };
        serde_json::to_string(&response).map_err(|error| js_err(error.to_string()))
    }

    /// Runs the full encode -> settle -> decide pipeline for one position
    /// and returns a JSON string (parsed with `JSON.parse` on the JS side)
    /// containing the selected move and the exact neural readouts that
    /// chose it.
    pub fn infer(&self, fen: &str, settle_steps: u32) -> Result<String, JsValue> {
        let neuron_count = self.connectome.neurons.len();
        let stimulus = encode_position(fen, &self.sensory_map, neuron_count)
            .map_err(|e| js_err(e.to_string()))?;

        let config = SimConfig {
            settle_steps,
            ..SimConfig::default()
        };
        let mut sim = CpuSimulator::new(&self.connectome, config);
        sim.weights_mut().copy_from_slice(&self.weights);
        for _ in 0..settle_steps {
            sim.step(&stimulus);
        }
        let rates = &sim.state().rate;

        self.serialize_inference(
            fen,
            settle_steps,
            rates,
            "cpu-wasm".to_string(),
            "CPU/WASM".to_string(),
            self.gpu_fallback_reason.clone(),
        )
    }

    /// Samples actual state at step boundaries. Awaiting the callback allows
    /// the worker to pace transferable frames and cancel an obsolete request.
    /// Decisions always use the final full-precision simulator rates.
    pub async fn infer_stream(
        &mut self,
        fen: &str,
        settle_steps: u32,
        on_frame: js_sys::Function,
    ) -> Result<String, JsValue> {
        let stimulus = encode_position(fen, &self.sensory_map, self.connectome.neurons.len())
            .map_err(|e| js_err(e.to_string()))?;
        let interval = settle_steps.div_ceil(16).max(1);
        let dt_ms = SimConfig::default().dt_ms;
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.reset_state();
            let mut step = 0;
            let mut failure = None;
            while step < settle_steps {
                let count = interval.min(settle_steps - step);
                if let Err(error) = gpu.run_steps_async(&stimulus, count).await {
                    failure = Some(error.to_string());
                    break;
                }
                step += count;
                let decision_json =
                    serialize_frame_decision(fen, &gpu.state().rate, &self.output_map)?;
                emit_frame(
                    &on_frame,
                    step,
                    dt_ms,
                    &gpu.state().rate,
                    gpu.backend(),
                    &decision_json,
                )
                .await?;
            }
            if let Some(reason) = failure {
                self.gpu_fallback_reason = Some(reason);
                self.gpu = None;
            } else {
                let rates = gpu.state().rate.clone();
                let backend = gpu.backend().to_string();
                let adapter = gpu.adapter_name().to_string();
                return self.serialize_inference(fen, settle_steps, &rates, backend, adapter, None);
            }
        }
        let config = SimConfig {
            settle_steps,
            ..SimConfig::default()
        };
        let mut sim = CpuSimulator::new(&self.connectome, config);
        sim.weights_mut().copy_from_slice(&self.weights);
        for step in 1..=settle_steps {
            sim.step(&stimulus);
            if step % interval == 0 || step == settle_steps {
                let decision_json =
                    serialize_frame_decision(fen, &sim.state().rate, &self.output_map)?;
                emit_frame(
                    &on_frame,
                    step,
                    dt_ms,
                    &sim.state().rate,
                    "cpu-wasm",
                    &decision_json,
                )
                .await?;
            }
        }
        self.serialize_inference(
            fen,
            settle_steps,
            &sim.state().rate,
            "cpu-wasm".to_string(),
            "CPU/WASM".to_string(),
            self.gpu_fallback_reason.clone(),
        )
    }

    /// Deterministic CPU reference replay with the same sampled step
    /// boundaries as `infer_stream`. This is deliberately separate from the
    /// selected live backend so callers can report CPU replay/reference
    /// evidence without implying general GPU/CPU parity.
    pub async fn infer_cpu_stream(
        &self,
        fen: &str,
        settle_steps: u32,
        on_frame: js_sys::Function,
    ) -> Result<String, JsValue> {
        let stimulus = encode_position(fen, &self.sensory_map, self.connectome.neurons.len())
            .map_err(|e| js_err(e.to_string()))?;
        let interval = settle_steps.div_ceil(16).max(1);
        let dt_ms = SimConfig::default().dt_ms;
        let config = SimConfig {
            settle_steps,
            ..SimConfig::default()
        };
        let mut sim = CpuSimulator::new(&self.connectome, config);
        sim.weights_mut().copy_from_slice(&self.weights);
        for step in 1..=settle_steps {
            sim.step(&stimulus);
            if step % interval == 0 || step == settle_steps {
                let decision_json =
                    serialize_frame_decision(fen, &sim.state().rate, &self.output_map)?;
                emit_frame(
                    &on_frame,
                    step,
                    dt_ms,
                    &sim.state().rate,
                    "cpu-wasm",
                    &decision_json,
                )
                .await?;
            }
        }
        self.serialize_inference(
            fen,
            settle_steps,
            &sim.state().rate,
            "cpu-wasm".to_string(),
            "CPU/WASM".to_string(),
            None,
        )
    }

    /// Browser inference path. GPU batches all settle steps and awaits one
    /// asynchronous readback. A runtime WebGPU failure retries the same
    /// request on CPU/WASM and records the exact fallback reason.
    pub async fn infer_async(&mut self, fen: &str, settle_steps: u32) -> Result<String, JsValue> {
        let neuron_count = self.connectome.neurons.len();
        let stimulus = encode_position(fen, &self.sensory_map, neuron_count)
            .map_err(|e| js_err(e.to_string()))?;

        let gpu_result = if let Some(gpu) = self.gpu.as_mut() {
            gpu.reset_state();
            Some(gpu.run_steps_async(&stimulus, settle_steps).await)
        } else {
            None
        };

        match gpu_result {
            Some(Ok(_)) => {
                let gpu = self.gpu.as_ref().unwrap();
                let rates = gpu.state().rate.clone();
                let backend = gpu.backend().to_string();
                let adapter = gpu.adapter_name().to_string();
                return self.serialize_inference(fen, settle_steps, &rates, backend, adapter, None);
            }
            Some(Err(error)) => {
                self.gpu_fallback_reason = Some(error.to_string());
                self.gpu = None;
            }
            None => {}
        }

        let config = SimConfig {
            settle_steps,
            ..SimConfig::default()
        };
        let mut sim = CpuSimulator::new(&self.connectome, config);
        sim.weights_mut().copy_from_slice(&self.weights);
        for _ in 0..settle_steps {
            sim.step(&stimulus);
        }
        self.serialize_inference(
            fen,
            settle_steps,
            &sim.state().rate,
            "cpu-wasm".to_string(),
            "CPU/WASM".to_string(),
            self.gpu_fallback_reason.clone(),
        )
    }
}

impl StockFlyEngine {
    fn serialize_inference(
        &self,
        fen: &str,
        settle_steps: u32,
        rates: &[f32],
        backend: String,
        adapter: String,
        gpu_fallback_reason: Option<String>,
    ) -> Result<String, JsValue> {
        let decision =
            choose_move(fen, rates, &self.output_map).map_err(|e| js_err(e.to_string()))?;
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
            legal_scores: decision.legal_scores,
            from_rates: decision.from_rates.to_vec(),
            to_rates: decision.to_rates.to_vec(),
            promotion_rates: decision.promotion_rates.to_vec(),
            top_neurons,
            graph_neurons_sha256: self.connectome.manifest.neurons_sha256.clone(),
            sensory_map_sha256: self.sensory_map.sha256().to_string(),
            output_map_sha256: self.output_map.sha256().to_string(),
            settle_steps,
            neuron_count: self.connectome.neurons.len(),
            edge_count: self.connectome.edge_src.len(),
            model_label: self.model_label.clone(),
            backend,
            adapter,
            gpu_fallback_reason,
        };

        serde_json::to_string(&response).map_err(|e| js_err(e.to_string()))
    }
}

fn serialize_frame_decision(
    fen: &str,
    rates: &[f32],
    output_map: &OutputMap,
) -> Result<String, JsValue> {
    let decision = choose_move(fen, rates, output_map).map_err(|e| js_err(e.to_string()))?;
    serde_json::to_string(&FrameDecisionResponse {
        selected_move: decision.uci,
        legal_scores: decision.legal_scores,
        from_rates: decision.from_rates.to_vec(),
        to_rates: decision.to_rates.to_vec(),
        promotion_rates: decision.promotion_rates.to_vec(),
    })
    .map_err(|e| js_err(e.to_string()))
}

fn js_err(e: impl ToString) -> JsValue {
    JsValue::from_str(&e.to_string())
}

async fn emit_frame(
    callback: &js_sys::Function,
    step: u32,
    dt_ms: f32,
    rates: &[f32],
    backend: &str,
    decision_json: &str,
) -> Result<(), JsValue> {
    // Copy before crossing the async boundary: a view into WASM memory
    // would become invalid if the linear memory grows while JS is awaiting.
    let values = js_sys::Float32Array::from(rates);
    let args = js_sys::Array::new();
    args.push(&JsValue::from(step));
    args.push(&JsValue::from(step as f32 * dt_ms));
    args.push(&values);
    args.push(&JsValue::from_str(backend));
    args.push(&JsValue::from_str(decision_json));
    let result = callback.apply(&JsValue::NULL, &args)?;
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&result)).await?;
    Ok(())
}
