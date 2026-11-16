// Ground detail below the mesh's resolution: stones, gravel, grass, grain,
// and the loose soil of other worlds.
//
// The terrain mesh carries the ground's colour per vertex; the alpha says
// which pattern to draw (see `ground::Pattern`). Patterns are functions of
// world position only. They bend the normal and vary the colour, never the
// geometry, so the walker's ground and the drawn ground stay the same.
//
// Everything random here comes from an integer hash of a lattice cell, and
// the cell is counted from the world's zero, not the renderer's: the
// floating origin (`origin.rs`) is added in whole cells, so the same stone
// lies in the same place wherever the origin happens to be, exactly, a
// thousand kilometres out as well as at the start.

import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::lights,
};

// `ground::GroundParams`.
struct Ground {
    curve: vec4<f32>,
    // The floating origin, whole metres (a multiple of 256).
    origin: vec4<i32>,
    // Craterlets, grain, ripples, pebbles.
    soil_a: vec4<f32>,
    // Sparkle, powder, surge, surge width.
    soil_b: vec4<f32>,
    // Backscatter, pebble tone.
    soil_c: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> ground: Ground;

struct Lattice {
    cell: vec2<i32>,
    f: vec2<f32>,
}

// The lattice with `num / den` cells to the metre along each axis, at the
// render-space point `p`. `den` must be a power of two up to 256, so the
// origin is a whole number of cells.
fn lattice2(p: vec2<f32>, num: vec2<i32>, den: i32) -> Lattice {
    let q = p * (vec2<f32>(num) / f32(den));
    let fl = floor(q);
    return Lattice(vec2<i32>(fl) + (ground.origin.xy / den) * num, q - fl);
}

fn lattice(p: vec2<f32>, num: i32, den: i32) -> Lattice {
    return lattice2(p, vec2(num), den);
}

// Integer hash of a lattice cell and a salt, to [0, 1). The same hash as
// `terrain::hash` on the CPU.
fn ihash(q: vec2<i32>, salt: u32) -> f32 {
    var h = (u32(q.x) * 0x8da6b343u) ^ (u32(q.y) * 0xd8163841u) ^ (salt * 0xcb1ab31fu);
    h ^= h >> 13u;
    h *= 0x5bd1e995u;
    h ^= h >> 15u;
    return f32(h & 0xffffffu) / 16777216.0;
}

// Value noise on a lattice: the value, 0–1, and its gradient per cell.
fn vnoise_l(l: Lattice, salt: u32) -> vec3<f32> {
    let u = l.f * l.f * (3.0 - 2.0 * l.f);
    let du = 6.0 * l.f * (1.0 - l.f);
    let a = ihash(l.cell, salt);
    let b = ihash(l.cell + vec2(1, 0), salt);
    let c = ihash(l.cell + vec2(0, 1), salt);
    let d = ihash(l.cell + vec2(1, 1), salt);
    let ab = mix(a, b, u.x);
    let cd = mix(c, d, u.x);
    return vec3(mix(ab, cd, u.y), du.x * mix(b - a, d - c, u.y), du.y * (cd - ab));
}

// Value noise with `num / den` cells to the metre: the value, and its
// gradient per metre.
fn vnoise(p: vec2<f32>, num: i32, den: i32, salt: u32) -> vec3<f32> {
    let n = vnoise_l(lattice(p, num, den), salt);
    return vec3(n.x, n.yz * (f32(num) / f32(den)));
}

fn value_noise(p: vec2<f32>, num: i32, den: i32, salt: u32) -> f32 {
    return vnoise_l(lattice(p, num, den), salt).x;
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
fn cobbles(l: Lattice) -> Stones {
    var f1 = 9.0;
    var f2 = 9.0;
    var best = vec2(0.0);
    var id = 0.0;
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = l.cell + vec2(i, j);
            let centre = vec2(f32(i), f32(j)) + 0.2 + 0.6 * vec2(ihash(c, 1u), ihash(c, 2u));
            let d = length(l.f - centre);
            if d < f1 {
                f2 = f1;
                f1 = d;
                best = l.f - centre;
                id = ihash(c, 3u);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    return Stones(best, (f2 - f1) * 0.5, id);
}

// Square setts in running bond: every other row is shifted half a sett.
fn setts(p: vec2<f32>) -> Stones {
    let rows = lattice(p, 33, 4);
    let odd = f32(rows.cell.y & 1) * 0.5;
    let l = lattice(p + vec2(odd * 4.0 / 33.0, 0.0), 33, 4);
    let f = l.f - 0.5;
    let joint = 0.5 - max(abs(f.x), abs(f.y));
    return Stones(f, joint, ihash(l.cell, 4u));
}

// The sun as the patterns need it: the unit vector toward it along the
// ground, and how far a shadow runs per unit of height.
struct Sun {
    along: vec2<f32>,
    reach: f32,
    tan_el: f32,
}

fn sun_over(l: vec3<f32>) -> Sun {
    let flat = length(l.xz);
    let up = max(l.y, 0.02);
    return Sun(l.xz / max(flat, 1e-4), min(flat / up, 12.0), up / max(flat, 1e-4));
}

// Small impact craters on a jittered grid, one per cell at most: the slope
// of their bowls and raised rims, pointing downhill (`bend`), how deep in a
// bowl the point is, and how much of the sun the bowl's own rim hides.
struct Pits {
    bend: vec2<f32>,
    depth: f32,
    shade: f32,
}

fn pits(l: Lattice, salt: u32, sun: Sun) -> Pits {
    var out = Pits(vec2(0.0), 0.0, 0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = l.cell + vec2(i, j);
            // Two hashes a cell, their bits shared out: this runs nine
            // times a scale for every pixel of ground.
            let h = ihash(c, salt);
            if h > 0.45 {
                continue;
            }
            let k = ihash(c, salt + 1u);
            let centre = vec2(f32(i), f32(j)) + 0.15 + 0.7 * vec2(fract(h * 37.0), fract(k * 53.0));
            // Many small, few large; most worn soft, some crisp.
            let r = 0.06 + 0.34 * pow(k, 4.0);
            let fresh = 0.15 + 0.85 * pow(fract(k * 211.0), 2.5);
            let d = l.f - centre;
            let x = length(d) / r;
            if x >= 1.6 || x < 0.001 {
                continue;
            }
            let dir = d / (x * r);
            if x < 1.0 {
                // The bowl: steeper toward the wall, level on the floor.
                out.bend -= dir * 0.7 * fresh * x * x * (1.0 - smoothstep(0.85, 1.0, x));
                out.depth += fresh * (1.0 - x * x);
                // The sun's ray from here leaves the bowl over the rim a
                // distance t away: in shadow if the rim stands higher than
                // the ray has climbed by then. A fifth of the diameter
                // deep when fresh.
                let along = dot(d, sun.along);
                let t = -along + sqrt(max(r * r - dot(d, d) + along * along, 0.0));
                let below = 0.4 * r * fresh * (1.0 - x * x);
                out.shade = max(out.shade, smoothstep(0.0, 0.12 * r, below - t * sun.tan_el));
            }
            // The rim's outer flank.
            out.bend += dir * 0.3 * fresh * smoothstep(0.85, 1.0, x) * (1.0 - smoothstep(1.0, 1.6, x));
        }
    }
    return out;
}

// Stones lying on the soil, one per cell at most: domes that bend the
// normal, each a shade of its own, and the shadow each throws down-sun.
struct Pebbles {
    bend: vec2<f32>,
    cover: f32,
    tone: f32,
    shade: f32,
}

fn pebbles(l: Lattice, salt: u32, density: f32, sun: Sun) -> Pebbles {
    var out = Pebbles(vec2(0.0), 0.0, 1.0, 0.0);
    for (var j = -1; j <= 1; j++) {
        for (var i = -1; i <= 1; i++) {
            let c = l.cell + vec2(i, j);
            let h = ihash(c, salt);
            if h > density {
                continue;
            }
            let k = ihash(c, salt + 1u);
            let centre = vec2(f32(i), f32(j)) + 0.2 + 0.6 * vec2(fract(h * 61.0 / density), fract(k * 53.0));
            let r = 0.04 + 0.13 * k * k * k;
            let d = l.f - centre;
            let dist = length(d);
            if dist < r {
                let x = dist / r;
                // Half buried: a low dome.
                out.bend = d / max(dist, 1e-4) * 0.8 * x / sqrt(1.08 - x * x);
                out.cover = 1.0 - smoothstep(0.85, 1.0, x);
                out.tone = ground.soil_c.y * (0.8 + 0.45 * fract(k * 91.0));
            } else {
                // Down-sun of it: its shadow, an ellipse as wide as the
                // stone and as long as its height and the sun's make it.
                let len = r * (1.0 + 0.5 * sun.reach);
                let along = -dot(d, sun.along) - 0.5 * len;
                let across = length(d + sun.along * dot(d, sun.along));
                let e = length(vec2(along / (0.5 * len + 0.6 * r), across / (0.95 * r)));
                out.shade = max(out.shade, 1.0 - smoothstep(0.7, 1.0, e));
            }
        }
    }
    return out;
}

// How much brighter or darker than a matte (Lambertian) surface loose soil
// looks. A porous powder scatters by Lommel-Seeliger's law (no darkening
// toward grazing views), hides its own shadows when the sun is behind the
// viewer (the opposition surge) and shows them when looking into the sun.
// Ice and wind-packed dust do less of each; the soil's numbers say how much.
fn soil_photometry(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> f32 {
    let powder = ground.soil_b.y;
    let surge = ground.soil_b.z;
    let width = ground.soil_b.w;
    let back = ground.soil_c.x;
    let nl = max(dot(n, l), 0.0);
    let nv = max(dot(n, v), 0.0);
    let g = acos(clamp(dot(v, l), -1.0, 1.0));
    let seeliger = mix(1.0, 2.0 / (nl + nv + 0.35) * 0.62, powder);
    let phase = 1.0 - 0.58 * back * (1.0 - exp(-1.7 * g)) + 0.3 * surge * exp(-g / width);
    return clamp(seeliger * phase, 0.35, 2.2);
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

    var to_sun = vec3(0.0, 1.0, 0.0);
    if lights.n_directional_lights > 0u {
        to_sun = lights.directional_lights[0].direction_to_light;
    }

    var total = 0.0;
    var colour = vec3(0.0);
    var bend = vec2(0.0);
    var roughness = 0.0;
    for (var k = 0; k < 5; k++) {
        let weight = w[k];
        if weight < 0.01 {
            continue;
        }
        let d = detail(k, p, base, base_roughness, pixel, to_sun, pbr_input.V);
        total += weight;
        colour += d.colour * weight;
        bend += d.bend * weight;
        roughness += d.roughness * weight;
    }
    let rest = max(1.0 - total, 0.0);
    if rest > 0.01 {
        // Fine grain everywhere: no surface is one flat tone up close.
        let grain = value_noise(p, 9, 1, 0u) * 0.6 + value_noise(p, 31, 1, 0u) * 0.4;
        colour += base * mix(1.0, 0.85 + 0.3 * grain, 1.0 - smoothstep(0.02, 0.08, pixel)) * rest;
    }
    if w[4] > 0.01 && lights.n_directional_lights > 0u {
        let k = soil_photometry(pbr_input.N, pbr_input.V, to_sun);
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

fn detail(kind: i32, p: vec2<f32>, colour: vec3<f32>, roughness: f32, pixel: f32, to_sun: vec3<f32>, v: vec3<f32>) -> Detail {
    var d = Detail(colour, vec2(0.0), roughness);
    if kind == 0 {
        // Cobbles, about 18 cm.
        let size = 0.18;
        let fade = 1.0 - smoothstep(size * 0.15, size * 0.5, pixel);
        let s = cobbles(lattice(p, 11, 2));
        let mortar = 1.0 - smoothstep(0.03, 0.09, s.joint);
        let stone = colour * mix(1.0, 0.75 + 0.5 * s.id, fade);
        d.colour = mix(stone, colour * 0.55, mortar * fade);
        d.bend = s.offset * 0.9 * fade * (1.0 - mortar);
        d.roughness = mix(roughness, 0.65, fade * (1.0 - mortar));
    } else if kind == 1 {
        // Setts, about 12 cm.
        let size = 0.12;
        let fade = 1.0 - smoothstep(size * 0.15, size * 0.5, pixel);
        let s = setts(p);
        let joint = 1.0 - smoothstep(0.02, 0.07, s.joint);
        d.colour = mix(colour * mix(1.0, 0.82 + 0.36 * s.id, fade), colour * 0.6, joint * fade);
        d.bend = s.offset * 0.35 * fade * (1.0 - joint);
    } else if kind == 2 {
        // Gravel: a speckle of pebbles.
        let fade = 1.0 - smoothstep(0.01, 0.05, pixel);
        let pebble = vnoise(p, 45, 1, 0u);
        d.colour = colour * mix(1.0, 0.7 + 0.6 * pebble.x, fade);
        d.bend = (vec2(value_noise(p, 45, 1, 5u), value_noise(p, 45, 1, 6u)) - 0.5) * 0.4 * fade;
    } else if kind == 3 {
        // Grass: blades as streaks of light and dark green, in patches.
        let fade = 1.0 - smoothstep(0.01, 0.06, pixel);
        let blades = vnoise_l(lattice2(p, vec2(60, 14), 1), 0u).x * 0.5 + vnoise_l(lattice2(p, vec2(14, 60), 1), 0u).x * 0.5;
        let patches = value_noise(p, 11, 16, 0u);
        d.colour = colour * (0.8 + 0.4 * patches) * mix(1.0, 0.6 + 0.8 * blades, fade);
    } else {
        d = soil(p, colour, roughness, pixel, to_sun, v);
    }
    return d;
}

// The world's loose ground: craterlets from a few metres down to a hand's
// breadth, clods and grain, wind ripples, stones and frost, each as much as
// the world's `Soil` asks for. Each scale fades out before it would alias,
// the coarse ones carrying furthest, and is skipped where it has faded: a
// distant pixel pays for almost none of this.
fn soil(p: vec2<f32>, colour: vec3<f32>, roughness: f32, pixel: f32, to_sun: vec3<f32>, v: vec3<f32>) -> Detail {
    var d = Detail(colour, vec2(0.0), roughness);
    let craterlets = ground.soil_a.x;
    let grain = ground.soil_a.y;
    let ripples = ground.soil_a.z;
    let stones = ground.soil_a.w;
    let sparkle = ground.soil_b.x;
    let sun = sun_over(to_sun);
    let fade = 1.0 - smoothstep(0.01, 0.06, pixel);
    let far = 1.0 - smoothstep(0.04, 0.3, pixel);
    var tone = 1.0;
    var shade = 0.0;

    if craterlets > 0.0 {
        let w_big = (1.0 - smoothstep(0.3, 1.5, pixel)) * craterlets;
        let w_mid = (1.0 - smoothstep(0.08, 0.4, pixel)) * craterlets;
        let w_small = (1.0 - smoothstep(0.02, 0.1, pixel)) * craterlets;
        if w_big > 0.0 {
            let c = pits(lattice(p, 17, 64), 11u, sun);
            d.bend += c.bend * w_big;
            tone -= 0.05 * c.depth * w_big;
            shade = max(shade, c.shade * w_big);
        }
        if w_mid > 0.0 {
            let c = pits(lattice(p, 9, 8), 21u, sun);
            d.bend += c.bend * w_mid;
            tone -= 0.05 * c.depth * w_mid;
            shade = max(shade, c.shade * w_mid);
        }
        if w_small > 0.0 {
            let c = pits(lattice(p, 35, 8), 31u, sun);
            d.bend += c.bend * 0.7 * w_small;
            tone -= 0.05 * c.depth * w_small;
            shade = max(shade, c.shade * w_small);
        }
    }

    if grain > 0.0 && far > 0.0 {
        // Clods a hand across, grain a finger across.
        let n = vnoise(p, 6, 1, 0u);
        d.bend -= n.yz * 0.0114 * far * grain;
        if fade > 0.0 {
            let m = vnoise(p, 23, 1, 0u);
            d.bend -= m.yz * 0.0017 * fade * grain;
            tone *= 1.0 + (m.x - 0.5) * 0.12 * fade * grain;
        }
    }

    if ripples > 0.0 {
        // Wind ripples: crests a couple of metres apart across the wind,
        // which blows from the west, wandering and forking as a noise
        // field bends their phase; steep lee faces to the east. Smaller
        // ones ride on them.
        let w_large = (1.0 - smoothstep(0.25, 1.2, pixel)) * ripples;
        if w_large > 0.0 {
            let warp = vnoise(p, 3, 16, 7u);
            // Whole cycles to 32 m, so the origin's jumps do not show.
            let x = p + vec2<f32>(ground.origin.xy % 32);
            let phase = 6.2832 * (x.x * 0.40625 + x.y * 0.0625) + warp.x * 9.0;
            let dphase = 6.2832 * vec2(0.40625, 0.0625) + warp.yz * 9.0;
            // A sawtooth-ish wave: sin with its own harmonic.
            let slope = cos(phase) + 0.35 * cos(2.0 * phase + 0.6);
            d.bend -= dphase * slope * 0.035 * w_large;
            tone *= 1.0 + 0.05 * sin(phase) * w_large;
        }
        let w_fine = (1.0 - smoothstep(0.015, 0.07, pixel)) * ripples;
        if w_fine > 0.0 {
            let warp = vnoise(p, 2, 1, 8u);
            let phase = 6.2832 * (p.x * 8.0 + p.y) + warp.x * 7.0;
            let dphase = 6.2832 * vec2(8.0, 1.0) + warp.yz * 7.0;
            d.bend -= dphase * cos(phase) * 0.0035 * w_fine;
        }
    }

    if stones > 0.0 {
        // Fist-sized stones in half-metre cells, pebbles in cells an
        // eighth of a metre across.
        let w_stones = 1.0 - smoothstep(0.015, 0.06, pixel);
        if w_stones > 0.0 {
            let s = pebbles(lattice(p, 2, 1), 41u, 0.55 * stones, sun);
            d.bend = mix(d.bend, s.bend, s.cover * w_stones);
            tone *= mix(1.0, s.tone, s.cover * w_stones);
            shade = max(shade, s.shade * w_stones);
        }
        let w_pebbles = 1.0 - smoothstep(0.004, 0.016, pixel);
        if w_pebbles > 0.0 {
            let s = pebbles(lattice(p, 8, 1), 51u, 0.7 * stones, sun);
            d.bend = mix(d.bend, s.bend, s.cover * w_pebbles);
            tone *= mix(1.0, s.tone, s.cover * w_pebbles);
            shade = max(shade, s.shade * w_pebbles);
        }
    }

    // Soil in a bowl lies a little darker; disturbed patches a little
    // lighter, in drifts a few metres across.
    let drift = value_noise(p, 7, 32, 40u) * 0.6 + value_noise(p, 7, 128, 41u) * 0.4;
    // What the relief hides of the sun is left with the light of the sky
    // and of the ground around it.
    d.colour = colour * (0.9 + 0.2 * drift) * tone * (1.0 - 0.8 * shade);

    if sparkle > 0.0 && to_sun.y > 0.0 {
        // Crystal faces that happen to mirror the sun into the eye: which
        // ones changes with every step, so the ground twinkles as you
        // move, and there are more of them toward the sun, where a face
        // needs less of a tilt to do it. The cells grow with distance, so
        // there is about one to a pixel.
        var num = 256;
        if pixel > 0.03 {
            num = 16;
        } else if pixel > 0.007 {
            num = 64;
        }
        let l = lattice(p, num, 1);
        let mirror = max(dot(reflect(-to_sun, vec3(0.0, 1.0, 0.0)), v), 0.0);
        let chance = 0.006 + 0.07 * pow(mirror, 5.0);
        // In steps, so the antialiasing's sub-pixel jitter of the view
        // does not make every glint flicker.
        let turn = floor(dot(v, vec3(37.1, 17.7, 29.3)) * 4.0) * 0.25;
        let face = fract(ihash(l.cell, 61u) + turn);
        let glint = step(1.0 - chance, face) * (1.0 - smoothstep(0.1, 0.5, pixel));
        d.colour += vec3(2.5 * sparkle * glint * (1.0 - shade));
    }
    return d;
}
