// The prepass vertex shader of bevy_pbr (depth, motion vectors, and the
// sun's shadow maps) with the same curve as `curve_vertex.wesl`, in the
// camera's own passes only. The sun's shadow maps are drawn flat: their
// views do not know where the camera is, and the fragment shaders look
// shadows up at the flat position (see `rise` in `ground.wesl` and
// `curve_fragment.wesl`), so lit and shadowed still meet where they should.

import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{
        mesh_functions,
        mesh_view_bindings::view,
        view_transformations::position_world_to_clip,
    },
};

struct Curve {
    at: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> curve: Curve;

fn sink(world: vec4<f32>) -> vec4<f32> {
    // An orthographic view is the sun's.
    if view.clip_from_view[3][3] == 1.0 {
        return world;
    }
    let d = world.xz - view.world_position.xz;
    let d2 = dot(d, d);
    let k = curve.at.z;
    return vec4(world.x, world.y - d2 * k * (1.0 + d2 * k * k), world.z, world.w);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);

    out.world_position = sink(mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)));
    out.position = position_world_to_clip(out.world_position.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);
@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);
}

@if(VERTEX_COLORS)
    out.color = vertex.color;

@if(MOTION_VECTOR_PREPASS) {
    // Where it was last frame, under this frame's curve: the camera's own
    // movement between two frames changes the drop by less than a pixel.
    let prev_model = mesh_functions::get_previous_world_from_local(vertex_in.instance_index);
    out.previous_world_position = sink(mesh_functions::mesh_position_local_to_world(prev_model, vec4<f32>(vertex.position, 1.0)));
}

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_in.instance_index, world_from_local[3]);

    return out;
}
