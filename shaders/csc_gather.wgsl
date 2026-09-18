struct Edge {
    src: u32,
    weight: f32,
}

struct NeuronState {
    membrane: f32,
    rate: f32,
}

struct GatherParams {
    dst_start: u32,
    dst_count: u32,
    _padding0: u32,
    _padding1: u32,
}

@group(0) @binding(0) var<storage, read> local_offsets: array<u32>;
@group(0) @binding(1) var<storage, read> edges: array<Edge>;
@group(0) @binding(2) var<storage, read> previous_state: array<NeuronState>;
@group(0) @binding(3) var<storage, read_write> gathered_state: array<NeuronState>;
@group(0) @binding(4) var<uniform> params: GatherParams;

@compute @workgroup_size(64)
fn gather(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let local_dst = global_id.x;
    if (local_dst >= params.dst_count) {
        return;
    }

    let start = local_offsets[local_dst];
    let end = local_offsets[local_dst + 1u];
    var input = 0.0;
    for (var edge_index = start; edge_index < end; edge_index += 1u) {
        let edge = edges[edge_index];
        input += edge.weight * previous_state[edge.src].rate;
    }
    gathered_state[params.dst_start + local_dst].membrane = input;
}
