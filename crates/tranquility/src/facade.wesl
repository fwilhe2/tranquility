// Facades: windows, storey bands and a plinth drawn on plain walls.
//
// The mesh carries the drawing's coordinates (see geom/massing.rs):
//   uv.x  distance along the building's outline, m
//   uv.y  height above street level, m
//   color.a  storey height / 20, plus 0.5 for shopfronts in the ground
//            storey; 1 means a blank wall (and roofs)
// Everything else is the StandardMaterial's, with the vertex colour as the
// wall's colour.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
};

const BAY: f32 = 3.1;
const PLINTH: f32 = 0.55;

// 1 inside [a, b], softened over about a pixel by the derivative `w`.
fn band(x: f32, a: f32, b: f32, w: f32) -> f32 {
    return smoothstep(a - w, a + w, x) * (1.0 - smoothstep(b - w, b + w, x));
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let code = in.color.a;
    let wall = abs(in.world_normal.y) < 0.3;

    if (wall && code < 0.99) {
        let shop = code >= 0.5;
        let storey = max((code - select(0.0, 0.5, shop)) * 20.0, 2.5);
        let along = in.uv.x;
        let height = in.uv.y;
        let fx = fract(along / BAY);
        let level = (height - PLINTH) / storey;
        let row = floor(level);
        let fy = fract(level);
        let wx = fwidth(along) / BAY;
        let wy = fwidth(height) / storey;
        var colour = pbr_input.material.base_color.rgb;

        // Window openings: upright in the upper storeys, wide shopfronts in the
        // ground storey of shops.
        var half_w = 0.2;
        var y0 = 0.3;
        var y1 = 0.84;
        if (row < 0.5 && shop) {
            half_w = 0.4;
            y0 = 0.08;
            y1 = 0.88;
        }
        let opening = band(fx, 0.5 - half_w, 0.5 + half_w, wx) * band(fy, y0, y1, wy) * step(0.0, level);
        // A stucco surround, a little lighter than the wall.
        let surround = band(fx, 0.5 - half_w - 0.05, 0.5 + half_w + 0.05, wx)
            * band(fy, y0 - 0.04, y1 + 0.05, wy) * step(0.0, level);
        // A storey band under each row of windows, and the plinth.
        let ledge = band(fy, 0.0, 0.05, wy) * step(0.5, level);
        let plinth = 1.0 - smoothstep(PLINTH - 0.02, PLINTH + 0.02, height);

        colour = mix(colour, colour * 1.18, max(surround, ledge * 0.7));
        colour = mix(colour, colour * 0.55, plinth);

        // Sashes: a cross of light frames over dark glass.
        let mullion = band(fx, 0.49, 0.51, wx);
        let transom = band(fy, y0 + (y1 - y0) * 0.62, y0 + (y1 - y0) * 0.62 + 0.02, wy);
        let edge = 1.0 - band(fx, 0.5 - half_w + 0.02, 0.5 + half_w - 0.02, wx) * band(fy, y0 + 0.02, y1 - 0.02, wy);
        let frame = max(max(mullion, transom), edge);
        let glass = opening * (1.0 - frame);
        colour = mix(colour, vec3(0.86, 0.84, 0.78), opening * frame);
        colour = mix(colour, vec3(0.025, 0.03, 0.035), glass);

        pbr_input.material.base_color = vec4(colour, 1.0);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, 0.08, glass);
        pbr_input.material.reflectance = mix(pbr_input.material.reflectance, vec3(0.5), glass);
    }

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color
    );
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
