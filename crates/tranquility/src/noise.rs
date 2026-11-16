//! Noise that never repeats, for height and tint functions.
//!
//! Gradient noise on the integer lattice, the gradients taken from a hash of
//! the lattice point rather than a permutation table: a table of 256 tiles
//! the plane, this does not come round again for four thousand million
//! cells. Positions are `f64`, so it is as smooth a million kilometres out
//! as at the origin.

use crate::terrain::hash;

/// Gradient noise at `(x, z)`, about −1…1, zero at lattice points.
pub fn gradient(x: f64, z: f64, seed: u32) -> f32 {
    let (fx, fz) = (x.floor(), z.floor());
    // Wrapping to 32 bits is the only period there is.
    let (i, j) = (fx as i64 as i32, fz as i64 as i32);
    let (u, v) = ((x - fx) as f32, (z - fz) as f32);
    let corner = |di: i32, dj: i32| {
        let h = hash(i.wrapping_add(di), j.wrapping_add(dj), seed);
        // One of 16 directions round the circle, from a table.
        let (gx, gz) = DIRS[(h >> 9 & 15) as usize];
        gx * (u - di as f32) + gz * (v - dj as f32)
    };
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (su, sv) = (fade(u), fade(v));
    let a = corner(0, 0) + su * (corner(1, 0) - corner(0, 0));
    let b = corner(0, 1) + su * (corner(1, 1) - corner(0, 1));
    (a + sv * (b - a)) * 1.5
}

const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
const DIRS: [(f32, f32); 16] = [
    (1.0, 0.0),
    (0.9239, 0.3827),
    (S, S),
    (0.3827, 0.9239),
    (0.0, 1.0),
    (-0.3827, 0.9239),
    (-S, S),
    (-0.9239, 0.3827),
    (-1.0, 0.0),
    (-0.9239, -0.3827),
    (-S, -S),
    (-0.3827, -0.9239),
    (0.0, -1.0),
    (0.3827, -0.9239),
    (S, -S),
    (0.9239, -0.3827),
];

/// Fractal noise: `octaves` of [`gradient`], the first with features
/// `wavelength` metres across, each further one half the size and `gain`
/// times the strength. About −1…1. Octaves are turned against each other
/// so their lattices do not line up.
pub fn fbm(x: f64, z: f64, wavelength: f64, octaves: u32, gain: f32, seed: u32) -> f32 {
    let (mut x, mut z) = (x / wavelength, z / wavelength);
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * gradient(x, z, seed.wrapping_add(o));
        norm += amp;
        amp *= gain;
        // Twice the frequency, turned by about 37°.
        (x, z) = (1.6 * x - 1.2 * z, 1.2 * x + 1.6 * z);
    }
    sum / norm
}

/// [`fbm`] that stops at the octave a grid of `step` metres can no longer
/// carry (its features would be under two steps across): what a distant
/// chunk should draw of it.
pub fn fbm_to(x: f64, z: f64, wavelength: f64, octaves: u32, gain: f32, seed: u32, step: f32) -> f32 {
    let (mut x, mut z) = (x / wavelength, z / wavelength);
    let (mut sum, mut amp, mut norm) = (0.0, 1.0, 0.0);
    let mut size = wavelength as f32;
    for o in 0..octaves {
        // The octave is always counted in the norm, so the coarse part
        // keeps its size whether or not the fine part is drawn.
        norm += amp;
        if size >= 2.0 * step {
            sum += amp * gradient(x, z, seed.wrapping_add(o));
        }
        amp *= gain;
        size *= 0.5;
        (x, z) = (1.6 * x - 1.2 * z, 1.2 * x + 1.6 * z);
    }
    sum / norm
}

/// Ridged fractal noise, 0…1: sharp crests where the octaves cross zero,
/// each octave weighted by the one above so ridges carry spurs and valleys
/// stay smooth. Mountains and scarps.
pub fn ridged(x: f64, z: f64, wavelength: f64, octaves: u32, seed: u32) -> f32 {
    let (mut x, mut z) = (x / wavelength, z / wavelength);
    let (mut sum, mut amp, mut norm, mut weight) = (0.0, 1.0, 0.0, 1.0f32);
    for o in 0..octaves {
        let r = 1.0 - gradient(x, z, seed.wrapping_add(o)).abs();
        let r = r * r * weight;
        weight = (r * 2.0).clamp(0.0, 1.0);
        sum += amp * r;
        norm += amp;
        amp *= 0.5;
        (x, z) = (1.6 * x - 1.2 * z, 1.2 * x + 1.6 * z);
    }
    sum / norm
}

/// Distance to the nearest and second nearest of a set of points scattered
/// one per unit cell (cells: Voronoi), with the nearest point's cell hash.
/// Cracked plains, convection cells, blocks of ice.
pub fn cells(x: f64, z: f64, seed: u32) -> (f32, f32, u32) {
    let (fx, fz) = (x.floor(), z.floor());
    let (i, j) = (fx as i64 as i32, fz as i64 as i32);
    let (u, v) = ((x - fx) as f32, (z - fz) as f32);
    let (mut f1, mut f2, mut id) = (9.0f32, 9.0f32, 0);
    for dj in -1..=1 {
        for di in -1..=1 {
            let h = hash(i.wrapping_add(di), j.wrapping_add(dj), seed);
            let px = di as f32 + (h & 0xfff) as f32 / 4096.0;
            let pz = dj as f32 + (h >> 12 & 0xfff) as f32 / 4096.0;
            let d = ((px - u) * (px - u) + (pz - v) * (pz - v)).sqrt();
            if d < f1 {
                (f2, f1, id) = (f1, d, h);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id)
}

/// Gradient noise in three dimensions, about −1…1: for maps wrapped on a
/// sphere, which have no seam and no poles this way.
pub fn gradient3(p: [f64; 3], seed: u32) -> f32 {
    let f = [p[0].floor(), p[1].floor(), p[2].floor()];
    let c = [f[0] as i64 as i32, f[1] as i64 as i32, f[2] as i64 as i32];
    let t = [(p[0] - f[0]) as f32, (p[1] - f[1]) as f32, (p[2] - f[2]) as f32];
    let corner = |d: [i32; 3]| {
        let h = hash(hash(c[0].wrapping_add(d[0]), c[1].wrapping_add(d[1]), seed) as i32, c[2].wrapping_add(d[2]), seed);
        // The twelve edge directions of a cube.
        let (x, y, z) = (t[0] - d[0] as f32, t[1] - d[1] as f32, t[2] - d[2] as f32);
        match (h >> 8) % 12 {
            0 => x + y,
            1 => -x + y,
            2 => x - y,
            3 => -x - y,
            4 => x + z,
            5 => -x + z,
            6 => x - z,
            7 => -x - z,
            8 => y + z,
            9 => -y + z,
            10 => y - z,
            _ => -y - z,
        }
    };
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v, w) = (fade(t[0]), fade(t[1]), fade(t[2]));
    let lerp = |a: f32, b: f32, t: f32| a + t * (b - a);
    let x00 = lerp(corner([0, 0, 0]), corner([1, 0, 0]), u);
    let x10 = lerp(corner([0, 1, 0]), corner([1, 1, 0]), u);
    let x01 = lerp(corner([0, 0, 1]), corner([1, 0, 1]), u);
    let x11 = lerp(corner([0, 1, 1]), corner([1, 1, 1]), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

/// Fractal [`gradient3`]: `octaves` from `frequency` up, each twice the
/// frequency and `gain` times the strength of the one before.
pub fn fbm3(p: [f64; 3], frequency: f64, octaves: u32, gain: f32, seed: u32) -> f32 {
    let (mut f, mut sum, mut amp, mut norm) = (frequency, 0.0, 1.0, 0.0);
    for o in 0..octaves {
        sum += amp * gradient3([p[0] * f, p[1] * f + 17.3 * o as f64, p[2] * f], seed.wrapping_add(o));
        norm += amp;
        amp *= gain;
        f *= 2.0;
    }
    sum / norm
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_is_continuous_and_bounded() {
        let mut worst = 0.0f32;
        let mut prev = gradient(0.0, 0.3, 1);
        for k in 1..20_000 {
            let v = gradient(k as f64 * 0.01, 0.3 + k as f64 * 0.003, 1);
            assert!(v.abs() <= 1.2, "{v}");
            worst = worst.max((v - prev).abs());
            prev = v;
        }
        assert!(worst < 0.06, "jump of {worst}");
    }

    #[test]
    fn gradient_does_not_tile() {
        // A permutation-table noise repeats every 256 cells.
        let same = (0..200)
            .filter(|&k| {
                let x = k as f64 * 0.37 + 0.11;
                (gradient(x, 0.4, 7) - gradient(x + 256.0, 0.4, 7)).abs() < 1e-4
            })
            .count();
        assert!(same < 10);
    }

    #[test]
    fn far_from_the_origin_is_as_smooth_as_near() {
        let far = 3.0e9;
        let d = (gradient(far + 0.5, 0.25, 3) - gradient(far + 0.5001, 0.25, 3)).abs();
        assert!(d < 1e-3, "{d}");
    }
}
