// Ground detail below the mesh's resolution: stones, gravel, grass, grain.
//
// The terrain mesh carries the ground's colour per vertex; the alpha says
// which pattern to draw (see `ground::Pattern`). Patterns are functions of
// world position only. They bend the normal and vary the colour, never the
// geometry, so the walker's ground and the drawn ground stay the same.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::lights,
};

// Integer hash of a lattice cell (pass whole-numbered coordinates) and a
// salt, to [0, 1): exact at any distance from the origin, where float hashes
// of world positions fall apart. The same hash as `terrain::hash` on the CPU.
fn ihash(c: vec2<f32>, salt: u32) -> f32 {
    let q = vec2<i32>(floor(c));
    var h = (u32(q.x) * 0x8da6b343u) ^ (u32(q.y) * 0xd8163841u) ^ (salt * 0xcb1ab31fu);
    h ^= h >> 13u;
    h *= 0x5bd1e995u;
    h ^= h >> 15u;
    return f32(h & 0xffffffu) / 16777216.0;
}

fn hash22(c: vec2<f32>) -> vec2<f32> {
    return vec2(ihash(c, 1u), ihash(c, 2u));
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = ihash(i, 0u);
    let b = ihash(i + vec2(1.0, 0.0), 0u);
    let c = ihash(i + vec2(0.0, 1.0), 0u);
    let d = ihash(i + vec2(1.0, 1.0), 0u);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

struct Stones {
    // Offset from the nearest stone's centre, in cells.
    offset: vec2<f32>,
    // Distance to the joint, in cells.
    joint: f32,
    // Per-stone random value.
    id: f32,
}

// Rounded stones on a jittered grid (Voronoi): cobbles.
fn cobbles(p: vec2<f32>) -> Stones {
    let cell = floor(p);
    var f1 = 9.0;
    var f2 = 9.0;
    var best = vec2(0.0);
    var id = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = cell + vec2(f32(i), f32(j));
            let centre = c + 0.2 + 0.6 * hash22(c);
            let d = length(p - centre);
            if d < f1 {
                f2 = f1;
                f1 = d;
                best = p - centre;
                id = ihash(c, 3u);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    return Stones(best, (f2 - f1) * 0.5, id);
}

// Square setts in running bond.
fn setts(p: vec2<f32>) -> Stones {
    let row = floor(p.y);
    let q = vec2(p.x + fract(row * 0.5), p.y);
    let c = floor(q);
    let f = fract(q) - 0.5;
    let joint = 0.5 - max(abs(f.x), abs(f.y));
    return Stones(f, joint, ihash(c, 4u));
}

// Small impact craters on a jittered grid, one per cell at most: the slope
// of their bowls and raised rims, pointing downhill (`bend`), and how deep
// in a bowl the point is (for a touch of darkening).
struct Pits {
    bend: vec2<f32>,
    depth: f32,
}

fn pits(p: vec2<f32>, salt: u32) -> Pits {
    let cell = floor(p);
    var out = Pits(vec2(0.0), 0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = cell + vec2(f32(i), f32(j));
            // Two hashes a cell, their bits shared out: this runs nine
            // times a scale for every pixel of ground.
            let h = ihash(c, salt);
            if h > 0.45 {
                continue;
            }
            let k = ihash(c, salt + 1u);
            let centre = c + 0.15 + 0.7 * vec2(fract(h * 37.0), fract(k * 53.0));
            // Many small, few large; most worn soft, some crisp.
            let r = 0.06 + 0.34 * pow(k, 4.0);
            let fresh = 0.15 + 0.85 * pow(fract(k * 211.0), 2.5);
            let d = p - centre;
            let x = length(d) / r;
            if x >= 1.6 || x < 0.001 {
                continue;
            }
            let dir = d / (x * r);
            if x < 1.0 {
                // The bowl: steeper toward the wall, level on the floor.
                out.bend -= dir * 0.7 * fresh * x * x * (1.0 - smoothstep(0.85, 1.0, x));
                out.depth += fresh * (1.0 - x * x);
            }
            // The rim's outer flank.
            out.bend += dir * 0.3 * fresh * smoothstep(0.85, 1.0, x) * (1.0 - smoothstep(1.0, 1.6, x));
        }
    }
    return out;
}

// How much brighter or darker than a matte (Lambertian) surface lunar soil
// looks. Regolith is a porous powder: it scatters by Lommel-Seeliger's law
// (no darkening toward grazing views), hides its own shadows when the sun is
// behind the viewer (the opposition surge) and shows them when looking into
// the sun.
fn regolith_photometry(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> f32 {
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.0);
    let g = acos(clamp(dot(v, l), -1.0, 1.0));
    let seeliger = 2.0 / (nl + nv + 0.35);
    let phase = 0.42 + 0.58 * exp(-1.7 * g) + 0.3 * exp(-g / 0.12);
    return clamp(seeliger * phase * 0.62, 0.35, 2.0);
}

struct Detail {
    colour: vec3<f32>,
    bend: vec2<f32>,
    roughness: f32,
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    // Pattern weights: cobbles, setts, gravel, grass, regolith; the rest is
    // plain (see `ground::Pattern::weights`).
    let w = array<f32, 5>(in.uv.x, in.uv.y, in.uv_b.x, in.uv_b.y, in.color.a);
    let p = in.world_position.xz;
    let base = pbr_input.material.base_color.rgb;
    let base_roughness = pbr_input.material.perceptual_roughness;

    // How many metres a pixel covers here: patterns fade into their mean
    // colour before they would alias.
    let pixel = length(fwidth(p));

    // Fine grain everywhere: no surface is one flat tone up close.
    let grain = value_noise(p * 9.0) * 0.6 + value_noise(p * 31.0) * 0.4;
    let plain = base * mix(1.0, 0.85 + 0.3 * grain, 1.0 - smoothstep(0.02, 0.08, pixel));

    var total = 0.0;
    var colour = vec3(0.0);
    var bend = vec2(0.0);
    var roughness = 0.0;
    for (var k = 0; k < 5; k++) {
        let weight = w[k];
        if weight < 0.01 {
            continue;
        }
        let d = detail(k, p, plain, base_roughness, pixel);
        total += weight;
        colour += d.colour * weight;
        bend += d.bend * weight;
        roughness += d.roughness * weight;
    }
    let rest = max(1.0 - total, 0.0);
    colour += plain * rest;
    if w[4] > 0.01 && lights.n_directional_lights > 0u {
        let k = regolith_photometry(pbr_input.N, pbr_input.V, lights.directional_lights[0].direction_to_light);
        colour *= mix(1.0, k, w[4]);
    }
    roughness += base_roughness * rest;

    if any(bend != vec2(0.0)) {
        // Tilt the normal by the bump's slope, in the ground's own plane:
        // `bend` points downhill, so stones bulge.
        let n = pbr_input.N;
        let t = normalize(cross(n, vec3(0.0, 0.0, 1.0)));
        let b = normalize(cross(t, n));
        pbr_input.N = normalize(n + t * bend.x + b * bend.y);
    }

    pbr_input.material.base_color = vec4(colour, 1.0);
    pbr_input.material.perceptual_roughness = roughness;
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

fn detail(kind: i32, p: vec2<f32>, colour: vec3<f32>, roughness: f32, pixel: f32) -> Detail {
    var d = Detail(colour, vec2(0.0), roughness);
    if kind == 0 {
        // Cobbles, about 18 cm.
        let size = 0.18;
        let fade = 1.0 - smoothstep(size * 0.15, size * 0.5, pixel);
        let s = cobbles(p / size);
        let mortar = 1.0 - smoothstep(0.03, 0.09, s.joint);
        let stone = colour * mix(1.0, 0.75 + 0.5 * s.id, fade);
        d.colour = mix(stone, colour * 0.55, mortar * fade);
        d.bend = s.offset * 0.9 * fade * (1.0 - mortar);
        d.roughness = mix(roughness, 0.65, fade * (1.0 - mortar));
    } else if kind == 1 {
        // Setts, about 12 cm.
        let size = 0.12;
        let fade = 1.0 - smoothstep(size * 0.15, size * 0.5, pixel);
        let s = setts(p / size);
        let joint = 1.0 - smoothstep(0.02, 0.07, s.joint);
        d.colour = mix(colour * mix(1.0, 0.82 + 0.36 * s.id, fade), colour * 0.6, joint * fade);
        d.bend = s.offset * 0.35 * fade * (1.0 - joint);
    } else if kind == 2 {
        // Gravel: a speckle of pebbles.
        let fade = 1.0 - smoothstep(0.01, 0.05, pixel);
        let pebble = value_noise(p * 45.0);
        d.colour = colour * mix(1.0, 0.7 + 0.6 * pebble, fade);
        d.bend = (vec2(value_noise(p * 45.0 + 3.7), value_noise(p * 45.0 + 9.1)) - 0.5) * 0.4 * fade;
    } else if kind == 3 {
        // Grass: blades as streaks of light and dark green, in patches.
        let fade = 1.0 - smoothstep(0.01, 0.06, pixel);
        let blades = value_noise(vec2(p.x * 60.0, p.y * 14.0)) * 0.5 + value_noise(vec2(p.x * 14.0, p.y * 60.0)) * 0.5;
        let patches = value_noise(p * 0.7);
        d.colour = colour * (0.8 + 0.4 * patches) * mix(1.0, 0.6 + 0.8 * blades, fade);
    } else {
        // Regolith: craterlets from a few metres down to a hand's breadth,
        // over clods and grain; each scale fades out before it would alias,
        // the coarse ones carrying furthest.
        let fade = 1.0 - smoothstep(0.01, 0.06, pixel);
        let far = 1.0 - smoothstep(0.04, 0.3, pixel);
        let w_big = 1.0 - smoothstep(0.3, 1.5, pixel);
        let w_mid = 1.0 - smoothstep(0.08, 0.4, pixel);
        let w_small = 1.0 - smoothstep(0.02, 0.1, pixel);
        var big = Pits(vec2(0.0), 0.0);
        var mid = Pits(vec2(0.0), 0.0);
        var small = Pits(vec2(0.0), 0.0);
        if w_big > 0.0 {
            big = pits(p / 3.7 + 17.0, 11u);
        }
        if w_mid > 0.0 {
            mid = pits(p / 0.9 + 5.0, 21u);
        }
        if w_small > 0.0 {
            small = pits(p / 0.23, 31u);
        }
        let n0 = value_noise(p * 6.0);
        let n = vec2(n0 - value_noise(p * 6.0 + vec2(0.31, 0.0)), n0 - value_noise(p * 6.0 + vec2(0.0, 0.31)));
        let m0 = value_noise(p * 23.0);
        let m = vec2(m0 - value_noise(p * 23.0 + vec2(0.13, 0.0)), m0 - value_noise(p * 23.0 + vec2(0.0, 0.13)));
        d.bend = big.bend * w_big + mid.bend * w_mid + small.bend * 0.7 * w_small
            + n * 0.22 * far + m * 0.3 * fade;
        // Soil in a bowl lies a little darker; disturbed patches a little
        // lighter, in drifts a few metres across.
        let drift = value_noise(p * 0.21 + 40.0) * 0.6 + value_noise(p * 0.057) * 0.4;
        d.colour = colour * (0.9 + 0.2 * drift)
            * (1.0 - 0.05 * (big.depth * w_big + mid.depth * w_mid + small.depth * w_small));
    }
    return d;
}
