// The StandardMaterial's fragment shader for things drawn on a round world
// (`ground::PropMaterial`): lit at the place they would have on a flat one,
// which is where the sun's shadow maps have them, and facing the way the
// sphere turns them.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
};

struct Curve {
    at: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> curve: Curve;

// What the vertex shader took off the height here (`curve_vertex.wesl`).
fn rise(world: vec4<f32>) -> f32 {
    let d = world.xz - view.world_position.xz;
    let d2 = dot(d, d);
    let r = curve.at.w;
    if r <= 0.0 {
        return 0.0;
    }
    return d2 / (r + sqrt(max(r * r - d2, 0.0)));
}

// Turns a direction from the flat world's frame at a point into the
// sphere's, where up is away from the body's centre: by the arc from the
// camera, about the horizontal axis square to it. `away` is the point's
// offset from the camera; `sign` −1 turns the other way.
fn on_sphere(v: vec3<f32>, away: vec2<f32>, sign: f32) -> vec3<f32> {
    let r = curve.at.w;
    if r <= 0.0 {
        return v;
    }
    let d2 = dot(away, away);
    let up = vec3(away.x, sqrt(max(r * r - d2, 0.0)), away.y) / r;
    // Rodrigues, from y to `up`: a = y × up, cos = up.y.
    let a = sign * vec3(up.z, 0.0, -up.x);
    return v * up.y + cross(a, v) + a * dot(a, v) / (1.0 + up.y);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.world_position.y += rise(in.world_position);
    // Lit as the sphere has it: far off, the ground faces another way.
    pbr_input.N = normalize(on_sphere(pbr_input.N, in.world_position.xz - view.world_position.xz, 1.0));
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );
    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr_input);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
