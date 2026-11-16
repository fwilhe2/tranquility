//! Impact craters, never stored: each size class re-derives the craters of a
//! cell from a hash of the cell's coordinates, so the height function stays
//! pure, the meshing jobs and the walker agree on every crater, and no two
//! places are alike however far you walk.
//!
//! The numbers are the measured ones.
//!
//! - **How many.** A surface old enough is in equilibrium: new craters
//!   erase old ones as fast as they form, and the count settles at a few
//!   per cent of the densest packing possible, N(>D) ≈ 0.08 D⁻² per unit
//!   area for craters wider than D (Gault 1970; Xiao & Werner 2015). That
//!   holds up to a diameter set by the surface's age (some 200 m on the
//!   lunar maria); above it the count falls off faster, as D⁻³, because
//!   there has not been time to saturate.
//! - **What shape.** A fresh simple crater is a fifth as deep as it is
//!   wide, with a rim standing 3.6 % of its diameter above the plain
//!   (Pike 1977) and an ejecta blanket thinning with the cube of the
//!   distance (McGetchin et al. 1973).
//! - **How worn.** Erosion by later impacts works like diffusion, and a
//!   population in equilibrium holds every state of it: a few crisp bowls
//!   among many soft dimples.

use tranquility::prelude::*;

/// One size class: diameters from `d` to four times that, in square cells
/// eight times `d` across. A crater and its ejecta (out to one diameter
/// from the centre) therefore never reach further than half a cell, and the
/// four cells nearest a point hold everything that touches it.
struct Class {
    d: f32,
    cell: f64,
    /// Chance that each of a cell's [`SLOTS`] holds a crater.
    chance: f32,
    salt: u32,
}

const SLOTS: u32 = 6;

pub struct Crater {
    /// Offset of the queried point from the centre, m.
    pub offset: Vec2,
    pub radius: f32,
    /// 1 fresh … 0 worn flat.
    pub fresh: f32,
    /// Per-crater random bits, for its rays.
    pub seed: u32,
}

pub struct Craters {
    classes: Vec<Class>,
    /// Depth of a fresh crater over its diameter.
    depth: f32,
    /// Diameter above which craters get flat floors instead of deeper
    /// bowls (the change from simple to complex craters), m.
    complex: f32,
}

pub struct Population {
    /// Smallest and largest diameter, m.
    pub d_min: f32,
    pub d_max: f32,
    /// Crater density as a share of lunar equilibrium.
    pub density: f32,
    /// Diameter up to which the surface is saturated, m.
    pub saturated: f32,
    /// Depth of a fresh crater over its diameter: 0.2 in rock and regolith,
    /// less in ice that flows.
    pub depth: f32,
    pub complex: f32,
    pub seed: u32,
}

impl Default for Population {
    fn default() -> Self {
        Self { d_min: 2.5, d_max: 10_000.0, density: 1.0, saturated: 200.0, depth: 0.2, complex: 15_000.0, seed: 1 }
    }
}

fn bits(h: u32) -> f32 {
    (h & 0xffffff) as f32 / 16777216.0
}

impl Craters {
    pub fn new(p: Population) -> Self {
        let mut classes = Vec::new();
        let mut d = p.d_min;
        let mut k = 0;
        while d < p.d_max {
            // Craters between d and 4d in a cell 8d across, at 0.08 D⁻².
            let saturation = (p.saturated / (2.0 * d)).min(1.0);
            let count = 0.08 * 64.0 * (15.0 / 16.0) * p.density * saturation;
            classes.push(Class {
                d,
                cell: 8.0 * d as f64,
                chance: (count / SLOTS as f32).min(1.0),
                salt: p.seed.wrapping_mul(7919).wrapping_add(k * 101 + 11),
            });
            d *= 4.0;
            k += 1;
        }
        Self { classes, depth: p.depth, complex: p.complex }
    }

    /// Calls `visit` with every crater whose surroundings, out to `reach`
    /// radii (at most 2, or 3.5 with `wide`), hold the point. Classes whose
    /// largest crater is narrower than `min_d` are left out, and craters
    /// wider than `ask_above` only count if `keep` says so of their centre
    /// (a world's way of saying "lava flooded this one").
    pub fn each(
        &self,
        x: f64,
        z: f64,
        min_d: f32,
        reach: f32,
        wide: bool,
        ask_above: f32,
        keep: &dyn Fn(f64, f64) -> bool,
        mut visit: impl FnMut(&Crater),
    ) {
        for class in &self.classes {
            if class.d * 4.0 < min_d {
                continue;
            }
            // The 2×2 cells nearest the point, or the 3×3 round it.
            let (u, v) = (x / class.cell, z / class.cell);
            let (i0, j0, n) = if wide {
                (u.floor() as i64 - 1, v.floor() as i64 - 1, 3)
            } else {
                ((u - 0.5).floor() as i64, (v - 0.5).floor() as i64, 2)
            };
            for j in j0..j0 + n {
                for i in i0..i0 + n {
                    let cell = hash(i as i32, j as i32, class.salt);
                    for slot in 0..SLOTS {
                        let h = hash(cell as i32, slot as i32, class.salt);
                        if bits(h) >= class.chance {
                            continue;
                        }
                        let (h1, h2) = (hash(h as i32, 1, class.salt), hash(h as i32, 2, class.salt));
                        // N(>D) ∝ D⁻² between d and 4d.
                        let d = class.d / (1.0 - bits(h2) * (15.0 / 16.0)).sqrt();
                        let radius = 0.5 * d;
                        // Centres stay a diameter inside the 3×3 block.
                        let cx = (i as f64 + (h1 & 0xffff) as f64 / 65536.0) * class.cell;
                        let cz = (j as f64 + (h1 >> 16) as f64 / 65536.0) * class.cell;
                        let offset = Vec2::new((x - cx) as f32, (z - cz) as f32);
                        if offset.length_squared() >= (reach * radius) * (reach * radius) {
                            continue;
                        }
                        if d > ask_above && !keep(cx, cz) {
                            continue;
                        }
                        // Most of an equilibrium population is old.
                        let fresh = (1.0 - bits(h2 >> 8 ^ h1)).powf(2.4);
                        visit(&Crater { offset, radius, fresh, seed: h1 ^ h2 });
                    }
                }
            }
        }
    }

    /// Height of a crater's profile at `x` radii from its centre.
    fn profile(&self, x: f32, radius: f32, fresh: f32) -> f32 {
        let d = 2.0 * radius;
        // Past the transition diameter craters stop getting deeper in
        // proportion: their floors rebound and fill.
        let deep = if d > self.complex { self.complex.powf(0.7) * d.powf(0.3) } else { d };
        let (depth, rim) = (self.depth * deep * fresh, 0.18 * self.depth * deep * fresh);
        // A flattened bowl: steep walls, a broad floor.
        let bowl = |x: f32| {
            let s = x * x;
            rim - depth * (1.0 - s) * (1.0 - 0.35 * s)
        };
        let blanket = |x: f32| rim * (1.0 / (x * x * x) - 0.125) / 0.875;
        // Wear rounds the rim crest: the older, the wider the round.
        let soft = 0.06 + 0.3 * (1.0 - fresh);
        if x < 1.0 - soft {
            bowl(x)
        } else if x < 1.0 + soft {
            let t = (x - (1.0 - soft)) / (2.0 * soft);
            let t = t * t * (3.0 - 2.0 * t);
            // Both sides meet the crest's height at x = 1; between them
            // the crest is lowered a little, as wear does.
            let crest = bowl(x.min(1.0)) * (1.0 - t) + blanket(x.max(1.0)) * t;
            crest - rim * 0.25 * (1.0 - fresh) * (1.0 - (2.0 * t - 1.0).powi(2))
        } else if x < 2.0 {
            blanket(x)
        } else {
            0.0
        }
    }

    /// What the craters add to the ground at a point, as a mesh of `step`
    /// metres should draw it: craters under two steps wide are left out.
    pub fn height(&self, x: f64, z: f64, step: f32, ask_above: f32, keep: &dyn Fn(f64, f64) -> bool) -> f32 {
        let mut h = 0.0;
        self.each(x, z, 2.0 * step, 2.0, false, ask_above, keep, |c| {
            h += self.profile(c.offset.length() / c.radius, c.radius, c.fresh);
        });
        h
    }

    /// How much brighter than its surroundings the soil is here, 0–1: young
    /// craters have thrown out fresh, unweathered material, as a bright
    /// collar round the rim that breaks up into rays further out, and their
    /// walls have not darkened yet.
    pub fn ejecta(&self, x: f64, z: f64, ask_above: f32, keep: &dyn Fn(f64, f64) -> bool) -> f32 {
        let mut bright = 0.0f32;
        self.each(x, z, 8.0, 3.5, true, ask_above, keep, |c| {
            let young = ((c.fresh - 0.55) / 0.45).max(0.0);
            if young == 0.0 {
                return;
            }
            let x = c.offset.length() / c.radius;
            let b = if x < 1.0 {
                0.25 + 0.75 * x * x
            } else {
                let out = 1.0 - (x - 1.0) / 2.5;
                let angle = c.offset.y.atan2(c.offset.x);
                let n = 5 + (c.seed >> 8) % 6;
                let phase = bits(c.seed >> 3) * std::f32::consts::TAU;
                let ray = (0.5 + 0.5 * (angle * n as f32 + phase).sin()).powi(3)
                    + 0.5 * (0.5 + 0.5 * (angle * (2 * n + 3) as f32 - phase).sin()).powi(4);
                // Solid at the rim, only the rays reaching far.
                let collar = (1.0 - (x - 1.0) / 0.7).max(0.0);
                out * out * (collar + (1.0 - collar) * ray.min(1.0))
            };
            bright = bright.max(young * b);
        });
        bright
    }

    /// 0–1: how strewn with blocks the ground is, from the craters alone.
    /// Young craters dug theirs out of the bedrock and dropped them on the
    /// rim and just beyond; old ground has had its rocks ground to soil.
    pub fn blocks(&self, x: f64, z: f64) -> f32 {
        let mut rocky = 0.0f32;
        self.each(x, z, 12.0, 2.0, false, f32::MAX, &|_, _| true, |c| {
            if c.fresh < 0.35 {
                return;
            }
            let x = c.offset.length() / c.radius;
            let near = if x < 1.0 { x * x * x } else { (2.0 - x).powi(2) };
            rocky = rocky.max((c.fresh - 0.35) / 0.65 * near);
        });
        rocky
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always(_: f64, _: f64) -> bool {
        true
    }

    /// The count over a large area matches 0.08 D⁻².
    #[test]
    fn population_follows_the_equilibrium_law() {
        let craters = Craters::new(Population { saturated: f32::MAX, ..Default::default() });
        let class = &craters.classes[1];
        let cells = 40;
        let mut wider_than = [0u32; 2];
        for j in 0..cells {
            for i in 0..cells {
                let cell = hash(i, j, class.salt);
                for slot in 0..SLOTS {
                    let h = hash(cell as i32, slot as i32, class.salt);
                    if bits(h) >= class.chance {
                        continue;
                    }
                    let d = class.d / (1.0 - bits(hash(h as i32, 2, class.salt)) * (15.0 / 16.0)).sqrt();
                    wider_than[0] += 1;
                    wider_than[1] += (d > 2.0 * class.d) as u32;
                }
            }
        }
        let area = (cells as f64 * class.cell).powi(2) as f32;
        let expect = |d: f32| 0.08 * area * (1.0 / (d * d) - 1.0 / (16.0 * class.d * class.d));
        for (n, d) in wider_than.iter().zip([class.d, 2.0 * class.d]) {
            let ratio = *n as f32 / expect(d);
            assert!((0.85..1.15).contains(&ratio), "N(>{d}) off by {ratio}");
        }
    }

    /// The ground is continuous: no steps where a crater's reach ends or a
    /// cell begins.
    #[test]
    fn height_has_no_steps() {
        let craters = Craters::new(Population::default());
        let mut prev = craters.height(0.0, 3.3, 0.5, f32::MAX, &always);
        for k in 1..40_000 {
            let h = craters.height(k as f64 * 0.05, 3.3 + k as f64 * 0.013, 0.5, f32::MAX, &always);
            assert!((h - prev).abs() < 0.2, "step of {} m at {k}", h - prev);
            prev = h;
        }
    }

    /// The same far out as near: positions are f64 throughout.
    #[test]
    fn far_out_is_as_fine_as_near() {
        let craters = Craters::new(Population::default());
        let far = 4.0e8;
        let a = craters.height(far, far, 0.5, f32::MAX, &always);
        let b = craters.height(far + 0.01, far, 0.5, f32::MAX, &always);
        assert!((a - b).abs() < 0.05);
    }
}
