// The mesh vertex shader of bevy_pbr, plus the curve of a round world: every
// vertex is lowered by how far a sphere of the body's radius has fallen away
// at its distance from the camera. Physics never sees this; at the camera
// the drop is nothing, and it grows with the square of the distance.

import bevy_pbr::render::{
    mesh_functions,
    mesh_view_bindings::view,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};

// z: 1 / (2 · radius), or 0 on a flat world. The start of the extension's
// uniform, whatever follows it.
struct Curve {
    at: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> curve: Curve;

fn sink(world: vec4<f32>) -> vec4<f32> {
    let d = world.xz - view.world_position.xz;
    let d2 = dot(d, d);
    let k = curve.at.z;
    // The first two terms of R − √(R² − d²): the root itself loses every
    // digit in single precision.
    return vec4(world.x, world.y - d2 * k * (1.0 + d2 * k * k), world.z, world.w);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(vertex_in, vertex_in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);

@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(vertex.normal, vertex_in.instance_index);

@if(VERTEX_POSITIONS)
    out.world_position = sink(mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0)));
@if(VERTEX_POSITIONS)
    out.position = position_world_to_clip(out.world_position.xyz);

@if(VERTEX_UVS_A)
    out.uv = vertex.uv;
@if(VERTEX_UVS_B)
    out.uv_b = vertex.uv_b;

@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(world_from_local, vertex.tangent, vertex_in.instance_index);

@if(VERTEX_COLORS)
    out.color = vertex.color;

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = vertex_in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_in.instance_index, world_from_local[3]);

    return out;
}
