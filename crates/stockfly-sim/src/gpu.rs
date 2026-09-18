use std::fmt;
use std::ops::Range;

use futures_channel::oneshot;
use stockfly_connectome::Connectome;

use crate::gpu_buffers::{build_edge_chunks, ChunkError, MAX_EDGE_BUFFER_BYTES};
use crate::{BrainState, FrameSummary, SimConfig, Stimulus};

const WORKGROUP_SIZE: u32 = 64;
const STATE_STRIDE_BYTES: u64 = 8;

#[derive(Debug, Clone, Copy)]
pub struct GpuAdapterOptions {
    pub power_preference: wgpu::PowerPreference,
    pub force_fallback_adapter: bool,
}

impl Default for GpuAdapterOptions {
    fn default() -> Self {
        Self {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        }
    }
}

#[derive(Debug)]
pub enum GpuError {
    Adapter(String),
    Device(String),
    Unsupported(String),
    InvalidInput(String),
    Chunk(ChunkError),
    Mapping(String),
}

impl fmt::Display for GpuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adapter(message) => write!(f, "GPU adapter unavailable: {message}"),
            Self::Device(message) => write!(f, "GPU device unavailable: {message}"),
            Self::Unsupported(message) => write!(f, "GPU adapter is unsupported: {message}"),
            Self::InvalidInput(message) => write!(f, "invalid GPU simulation input: {message}"),
            Self::Chunk(error) => error.fmt(f),
            Self::Mapping(message) => write!(f, "GPU readback failed: {message}"),
        }
    }
}

impl std::error::Error for GpuError {}

impl From<ChunkError> for GpuError {
    fn from(value: ChunkError) -> Self {
        Self::Chunk(value)
    }
}

struct GpuChunk {
    edge_range: Range<usize>,
    edge_buffer: wgpu::Buffer,
    _offset_buffer: wgpu::Buffer,
    _params_buffer: wgpu::Buffer,
    gather_bind_groups: [wgpu::BindGroup; 2],
    dispatch_count: u32,
    bound_edge_bytes: u64,
}

/// Owned compute backend shared by native Metal and browser WebGPU. The graph
/// is used only while buffers are created or learned weights are uploaded, so
/// callers can keep this simulator beside their `Connectome` without a
/// self-referential borrow.
pub struct GpuSimulator {
    device: wgpu::Device,
    queue: wgpu::Queue,
    state: BrainState,
    state_buffers: [wgpu::Buffer; 2],
    stimulus_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    chunks: Vec<GpuChunk>,
    lif_bind_groups: [wgpu::BindGroup; 2],
    gather_pipeline: wgpu::ComputePipeline,
    lif_pipeline: wgpu::ComputePipeline,
    current_state: usize,
    step_count: u32,
    neuron_count: usize,
    adapter_name: String,
    backend: String,
    peak_buffer_bytes: u64,
}

impl GpuSimulator {
    /// Native convenience constructor. Browser callers must use
    /// `new_async`/`new_with_weights_async`, which yields to WebGPU while the
    /// adapter and device are requested.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(
        connectome: &Connectome,
        config: SimConfig,
        adapter_options: GpuAdapterOptions,
    ) -> Result<Self, GpuError> {
        pollster::block_on(Self::new_async(connectome, config, adapter_options))
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_with_weights(
        connectome: &Connectome,
        config: SimConfig,
        adapter_options: GpuAdapterOptions,
        weights: &[f32],
    ) -> Result<Self, GpuError> {
        pollster::block_on(Self::new_with_weights_async(
            connectome,
            config,
            adapter_options,
            weights,
        ))
    }

    pub async fn new_async(
        connectome: &Connectome,
        config: SimConfig,
        adapter_options: GpuAdapterOptions,
    ) -> Result<Self, GpuError> {
        let weights: Vec<f32> = connectome
            .edge_weight
            .iter()
            .map(|weight| weight * config.weight_scale)
            .collect();
        Self::new_with_weights_async(connectome, config, adapter_options, &weights).await
    }

    pub async fn new_with_weights_async(
        connectome: &Connectome,
        config: SimConfig,
        adapter_options: GpuAdapterOptions,
        weights: &[f32],
    ) -> Result<Self, GpuError> {
        let neuron_count = connectome.neurons.len();
        if neuron_count == 0 {
            return Err(GpuError::InvalidInput(
                "connectome has no neurons".to_string(),
            ));
        }
        if neuron_count > u32::MAX as usize {
            return Err(GpuError::Unsupported(
                "neuron count exceeds WGSL u32 indexing".to_string(),
            ));
        }

        let cpu_chunks = build_edge_chunks(connectome, weights, MAX_EDGE_BUFFER_BYTES)?;

        #[cfg(target_arch = "wasm32")]
        let backends = wgpu::Backends::BROWSER_WEBGPU;
        #[cfg(target_os = "macos")]
        let backends = wgpu::Backends::METAL;
        #[cfg(target_os = "windows")]
        let backends = wgpu::Backends::DX12;
        #[cfg(all(
            not(target_arch = "wasm32"),
            not(target_os = "macos"),
            not(target_os = "windows")
        ))]
        let backends = wgpu::Backends::VULKAN;

        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = backends;
        let instance = wgpu::Instance::new(instance_descriptor);
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: adapter_options.power_preference,
                force_fallback_adapter: adapter_options.force_fallback_adapter,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|error| GpuError::Adapter(error.to_string()))?;

        let adapter_limits = adapter.limits();
        let max_edge_binding = cpu_chunks
            .iter()
            .map(|chunk| chunk.bound_edge_bytes().max(8))
            .max()
            .unwrap_or(8);
        let state_bytes = neuron_count as u64 * STATE_STRIDE_BYTES;
        let stimulus_bytes = neuron_count as u64 * 4;
        let largest_buffer = max_edge_binding.max(state_bytes).max(stimulus_bytes);
        if max_edge_binding > MAX_EDGE_BUFFER_BYTES {
            return Err(GpuError::Unsupported(format!(
                "edge binding is {max_edge_binding} bytes, above the 64 MiB project limit"
            )));
        }
        if max_edge_binding > adapter_limits.max_storage_buffer_binding_size as u64 {
            return Err(GpuError::Unsupported(format!(
                "max storage binding is {} bytes, need {max_edge_binding}",
                adapter_limits.max_storage_buffer_binding_size
            )));
        }
        if largest_buffer > adapter_limits.max_buffer_size {
            return Err(GpuError::Unsupported(format!(
                "max buffer is {} bytes, need {largest_buffer}",
                adapter_limits.max_buffer_size
            )));
        }
        if adapter_limits.max_storage_buffers_per_shader_stage < 4 {
            return Err(GpuError::Unsupported(format!(
                "only {} storage buffers per compute stage; need 4",
                adapter_limits.max_storage_buffers_per_shader_stage
            )));
        }
        if adapter_limits.max_compute_workgroup_size_x < WORKGROUP_SIZE
            || adapter_limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        {
            return Err(GpuError::Unsupported(format!(
                "compute workgroup limits are x={} invocations={}, need {WORKGROUP_SIZE}",
                adapter_limits.max_compute_workgroup_size_x,
                adapter_limits.max_compute_invocations_per_workgroup
            )));
        }
        let required_dispatches = div_ceil_u32(neuron_count as u32, WORKGROUP_SIZE);
        if required_dispatches > adapter_limits.max_compute_workgroups_per_dimension {
            return Err(GpuError::Unsupported(format!(
                "graph needs {required_dispatches} workgroups, adapter allows {}",
                adapter_limits.max_compute_workgroups_per_dimension
            )));
        }

        let mut required_limits = wgpu::Limits::downlevel_defaults();
        required_limits.max_storage_buffer_binding_size = required_limits
            .max_storage_buffer_binding_size
            .max(max_edge_binding);
        required_limits.max_buffer_size = required_limits.max_buffer_size.max(largest_buffer);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("stockfly-compute"),
                required_features: wgpu::Features::empty(),
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|error| GpuError::Device(error.to_string()))?;

        let adapter_info = adapter.get_info();
        let gather_layout = create_gather_layout(&device);
        let lif_layout = create_lif_layout(&device);
        let gather_pipeline = create_pipeline(
            &device,
            "stockfly-csc-gather",
            include_str!("../../../shaders/csc_gather.wgsl"),
            "gather",
            &gather_layout,
        );
        let lif_pipeline = create_pipeline(
            &device,
            "stockfly-lif-step",
            include_str!("../../../shaders/lif_step.wgsl"),
            "step",
            &lif_layout,
        );

        let zero_state = vec![0u8; state_bytes as usize];
        let state_buffers = [
            create_buffer_with_bytes(
                &device,
                &queue,
                "stockfly-state-a",
                &zero_state,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                4,
            ),
            create_buffer_with_bytes(
                &device,
                &queue,
                "stockfly-state-b",
                &zero_state,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                4,
            ),
        ];
        let stimulus_buffer = create_buffer_with_bytes(
            &device,
            &queue,
            "stockfly-stimulus",
            &vec![0u8; stimulus_bytes as usize],
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
        );
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("stockfly-state-readback"),
            size: state_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let lif_params = encode_lif_params(config, neuron_count as u32);
        let lif_params_buffer = create_buffer_with_bytes(
            &device,
            &queue,
            "stockfly-lif-params",
            &lif_params,
            wgpu::BufferUsages::UNIFORM,
            4,
        );
        let lif_bind_groups = [
            create_lif_bind_group(
                &device,
                &lif_layout,
                &state_buffers[0],
                &stimulus_buffer,
                &state_buffers[1],
                &lif_params_buffer,
                "stockfly-lif-a-to-b",
            ),
            create_lif_bind_group(
                &device,
                &lif_layout,
                &state_buffers[1],
                &stimulus_buffer,
                &state_buffers[0],
                &lif_params_buffer,
                "stockfly-lif-b-to-a",
            ),
        ];

        let mut chunks = Vec::with_capacity(cpu_chunks.len());
        let mut peak_buffer_bytes = state_bytes * 3 + stimulus_bytes + lif_params.len() as u64;
        for (index, chunk) in cpu_chunks.into_iter().enumerate() {
            let offset_bytes = encode_u32_slice(&chunk.local_offsets);
            let edge_buffer = create_buffer_with_bytes(
                &device,
                &queue,
                &format!("stockfly-edges-{index}"),
                &chunk.packed_edges,
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                8,
            );
            let offset_buffer = create_buffer_with_bytes(
                &device,
                &queue,
                &format!("stockfly-offsets-{index}"),
                &offset_bytes,
                wgpu::BufferUsages::STORAGE,
                4,
            );
            let params_bytes = encode_gather_params(chunk.dst_start, chunk.dst_count);
            let params_buffer = create_buffer_with_bytes(
                &device,
                &queue,
                &format!("stockfly-gather-params-{index}"),
                &params_bytes,
                wgpu::BufferUsages::UNIFORM,
                4,
            );
            let gather_bind_groups = [
                create_gather_bind_group(
                    &device,
                    &gather_layout,
                    &offset_buffer,
                    &edge_buffer,
                    &state_buffers[0],
                    &state_buffers[1],
                    &params_buffer,
                    &format!("stockfly-gather-{index}-a-to-b"),
                ),
                create_gather_bind_group(
                    &device,
                    &gather_layout,
                    &offset_buffer,
                    &edge_buffer,
                    &state_buffers[1],
                    &state_buffers[0],
                    &params_buffer,
                    &format!("stockfly-gather-{index}-b-to-a"),
                ),
            ];
            let edge_allocated = (chunk.packed_edges.len() as u64).max(8);
            let offset_allocated = (offset_bytes.len() as u64).max(4);
            let params_allocated = params_bytes.len() as u64;
            let allocated_bytes = edge_allocated + offset_allocated + params_allocated;
            peak_buffer_bytes += allocated_bytes;
            chunks.push(GpuChunk {
                edge_range: chunk.edge_start..chunk.edge_end,
                edge_buffer,
                _offset_buffer: offset_buffer,
                _params_buffer: params_buffer,
                gather_bind_groups,
                dispatch_count: div_ceil_u32(chunk.dst_count, WORKGROUP_SIZE),
                bound_edge_bytes: chunk.bound_edge_bytes(),
            });
        }

        Ok(Self {
            device,
            queue,
            state: BrainState::zeroed(neuron_count),
            state_buffers,
            stimulus_buffer,
            staging_buffer,
            chunks,
            lif_bind_groups,
            gather_pipeline,
            lif_pipeline,
            current_state: 0,
            step_count: 0,
            neuron_count,
            adapter_name: adapter_info.name,
            backend: format!("{:?}", adapter_info.backend).to_lowercase(),
            peak_buffer_bytes,
        })
    }

    pub fn state(&self) -> &BrainState {
        &self.state
    }

    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }

    pub fn backend(&self) -> &str {
        &self.backend
    }

    pub fn peak_buffer_bytes(&self) -> u64 {
        self.peak_buffer_bytes
    }

    pub fn edge_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn max_bound_edge_buffer_bytes(&self) -> u64 {
        self.chunks
            .iter()
            .map(|chunk| chunk.bound_edge_bytes)
            .max()
            .unwrap_or(0)
    }

    /// Uploads a complete learned weight vector while retaining the exact
    /// compiled source indices and destination chunk boundaries. The values
    /// are simulator-ready weights (the same scaled values exposed by
    /// `CpuSimulator::weights`), so checkpoint deltas apply identically.
    pub fn set_weights(
        &mut self,
        connectome: &Connectome,
        weights: &[f32],
    ) -> Result<(), GpuError> {
        if connectome.edge_src.len() != weights.len() {
            return Err(GpuError::InvalidInput(format!(
                "weight count {} does not match edge count {}",
                weights.len(),
                connectome.edge_src.len()
            )));
        }
        for chunk in &self.chunks {
            let bytes = encode_edge_range(connectome, weights, chunk.edge_range.clone());
            if bytes.len() as u64 > MAX_EDGE_BUFFER_BYTES {
                return Err(GpuError::Unsupported(
                    "learned edge upload exceeds the 64 MiB binding limit".to_string(),
                ));
            }
            self.queue.write_buffer(&chunk.edge_buffer, 0, &bytes);
        }
        Ok(())
    }

    /// Runs one or more recurrent steps with one fixed stimulus, then awaits a
    /// single state readback. This is the reusable inference path for native
    /// callers and the nonblocking path exported to browser WASM.
    pub async fn run_steps_async(
        &mut self,
        stimulus: &Stimulus,
        steps: u32,
    ) -> Result<FrameSummary, GpuError> {
        if stimulus.values.len() != self.neuron_count {
            return Err(GpuError::InvalidInput(format!(
                "stimulus has {} values, expected {}",
                stimulus.values.len(),
                self.neuron_count
            )));
        }
        if steps == 0 {
            return Ok(self.summary());
        }

        let stimulus_bytes = encode_f32_slice(&stimulus.values);
        self.queue
            .write_buffer(&self.stimulus_buffer, 0, &stimulus_bytes);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stockfly-simulation-encoder"),
            });
        let mut current_state = self.current_state;
        for _ in 0..steps {
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("stockfly-csc-gather-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.gather_pipeline);
                for chunk in &self.chunks {
                    pass.set_bind_group(0, &chunk.gather_bind_groups[current_state], &[]);
                    pass.dispatch_workgroups(chunk.dispatch_count, 1, 1);
                }
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("stockfly-lif-step-pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.lif_pipeline);
                pass.set_bind_group(0, &self.lif_bind_groups[current_state], &[]);
                pass.dispatch_workgroups(
                    div_ceil_u32(self.neuron_count as u32, WORKGROUP_SIZE),
                    1,
                    1,
                );
            }
            current_state = 1 - current_state;
        }

        let state_bytes = self.neuron_count as u64 * STATE_STRIDE_BYTES;
        encoder.copy_buffer_to_buffer(
            &self.state_buffers[current_state],
            0,
            &self.staging_buffer,
            0,
            state_bytes,
        );
        self.queue.submit(Some(encoder.finish()));
        self.current_state = current_state;
        self.step_count = self.step_count.saturating_add(steps);

        let slice = self.staging_buffer.slice(..state_bytes);
        let (sender, receiver) = oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result.map_err(|error| error.to_string()));
        });
        #[cfg(not(target_arch = "wasm32"))]
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| GpuError::Mapping(error.to_string()))?;
        receiver
            .await
            .map_err(|_| GpuError::Mapping("readback callback was canceled".to_string()))?
            .map_err(GpuError::Mapping)?;

        let mapped = slice
            .get_mapped_range()
            .map_err(|error| GpuError::Mapping(error.to_string()))?;
        for (index, bytes) in mapped.chunks_exact(8).enumerate() {
            self.state.membrane[index] = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
            self.state.rate[index] = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
        }
        drop(mapped);
        self.staging_buffer.unmap();

        Ok(self.summary())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run_steps(&mut self, stimulus: &Stimulus, steps: u32) -> Result<FrameSummary, GpuError> {
        pollster::block_on(self.run_steps_async(stimulus, steps))
    }

    pub fn reset_state(&mut self) {
        self.state.reset();
        let zero_state = vec![0u8; self.neuron_count * STATE_STRIDE_BYTES as usize];
        self.queue
            .write_buffer(&self.state_buffers[0], 0, &zero_state);
        self.queue
            .write_buffer(&self.state_buffers[1], 0, &zero_state);
        self.current_state = 0;
        self.step_count = 0;
    }

    fn summary(&self) -> FrameSummary {
        let max_rate = self.state.rate.iter().copied().fold(0.0f32, f32::max);
        let mean_rate = self.state.rate.iter().sum::<f32>() / self.neuron_count as f32;
        FrameSummary {
            step: self.step_count,
            max_rate,
            mean_rate,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl crate::Simulator for GpuSimulator {
    fn step(&mut self, stimulus: &Stimulus) -> FrameSummary {
        self.run_steps(stimulus, 1)
            .expect("an initialized GPU simulator should complete a step")
    }

    fn reset(&mut self) {
        self.reset_state();
    }
}

fn create_gather_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("stockfly-gather-layout"),
        entries: &[
            storage_layout_entry(0, true),
            storage_layout_entry(1, true),
            storage_layout_entry(2, true),
            storage_layout_entry(3, false),
            uniform_layout_entry(4),
        ],
    })
}

fn create_lif_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("stockfly-lif-layout"),
        entries: &[
            storage_layout_entry(0, true),
            storage_layout_entry(1, true),
            storage_layout_entry(2, false),
            uniform_layout_entry(3),
        ],
    })
}

fn storage_layout_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    label: &str,
    source: &str,
    entry_point: &str,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        module: &module,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    })
}

fn create_buffer_with_bytes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
    minimum_size: u64,
) -> wgpu::Buffer {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (bytes.len() as u64).max(minimum_size),
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    if !bytes.is_empty() {
        queue.write_buffer(&buffer, 0, bytes);
    }
    buffer
}

#[allow(clippy::too_many_arguments)]
fn create_gather_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    offsets: &wgpu::Buffer,
    edges: &wgpu::Buffer,
    previous_state: &wgpu::Buffer,
    next_state: &wgpu::Buffer,
    params: &wgpu::Buffer,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            bind_group_entry(0, offsets),
            bind_group_entry(1, edges),
            bind_group_entry(2, previous_state),
            bind_group_entry(3, next_state),
            bind_group_entry(4, params),
        ],
    })
}

fn create_lif_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    previous_state: &wgpu::Buffer,
    stimulus: &wgpu::Buffer,
    next_state: &wgpu::Buffer,
    params: &wgpu::Buffer,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            bind_group_entry(0, previous_state),
            bind_group_entry(1, stimulus),
            bind_group_entry(2, next_state),
            bind_group_entry(3, params),
        ],
    })
}

fn bind_group_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

fn encode_edge_range(connectome: &Connectome, weights: &[f32], range: Range<usize>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(range.len() * 8);
    for edge_index in range {
        bytes.extend_from_slice(&connectome.edge_src[edge_index].to_le_bytes());
        bytes.extend_from_slice(&weights[edge_index].to_le_bytes());
    }
    bytes
}

fn encode_u32_slice(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn encode_f32_slice(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn encode_gather_params(dst_start: u32, dst_count: u32) -> Vec<u8> {
    [dst_start, dst_count, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn encode_lif_params(config: SimConfig, neuron_count: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&config.threshold.to_le_bytes());
    bytes.extend_from_slice(&config.decay.to_le_bytes());
    bytes.extend_from_slice(&config.max_rate.to_le_bytes());
    bytes.extend_from_slice(&neuron_count.to_le_bytes());
    bytes
}

fn div_ceil_u32(value: u32, divisor: u32) -> u32 {
    value / divisor + u32::from(value % divisor != 0)
}
