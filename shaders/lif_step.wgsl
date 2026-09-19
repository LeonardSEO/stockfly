struct NeuronState {
    membrane: f32,
    rate: f32,
}

struct LifParams {
    threshold: f32,
    decay: f32,
    max_rate: f32,
    neuron_count: u32,
}

@group(0) @binding(0) var<storage, read> previous_state: array<NeuronState>;
@group(0) @binding(1) var<storage, read> stimulus: array<f32>;
@group(0) @binding(2) var<storage, read_write> next_state: array<NeuronState>;
@group(0) @binding(3) var<uniform> params: LifParams;

@compute @workgroup_size(64)
fn step(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let neuron = global_id.x;
    if (neuron >= params.neuron_count) {
        return;
    }

    let gathered_input = next_state[neuron].membrane;
    // Request fused decay plus recurrent input, then sensory addition.
    // Apple M4 Metal measured single rounding; WGSL does not guarantee it.
    let membrane = fma(previous_state[neuron].membrane, params.decay, gathered_input)
        + stimulus[neuron];
    let rate = clamp(membrane - params.threshold, 0.0, params.max_rate);
    next_state[neuron] = NeuronState(membrane, rate);
}
