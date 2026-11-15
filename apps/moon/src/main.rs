//! A lunar surface: saturated craters in size classes over gently rolling
//! highlands and dark maria, a low sun that stands still, a black sky with
//! stars, lunar gravity, and boulders scattered as the ground streams in.
//!
//! The lunar-walk recipe, reduced: craters are never stored, each size class
//! re-derives the craters of a cell from a hash of the cell's coordinates,
//! so the height function stays pure and the meshing jobs and the walker
//! agree on every crater.

mod heavens;
mod rocks;

use std::sync::Arc;

use tranquility::prelude::*;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

/// One size class of craters: at most one crater per square cell.
struct Class {
    cell: f32,
    /// Chance that a cell has a crater.
    density: f32,
    r_min: f32,
    r_max: f32,
    salt: u32,
}

/// Radii stay under half a cell, so a crater and its ejecta (out to twice
/// its radius) never reach past the 3×3 cells round a query.
const CLASSES: [Class; 4] = [
    Class { cell: 3000.0, density: 0.45, r_min: 150.0, r_max: 1300.0, salt: 11 },
    Class { cell: 600.0, density: 0.65, r_min: 30.0, r_max: 260.0, salt: 23 },
    Class { cell: 120.0, density: 0.8, r_min: 6.0, r_max: 52.0, salt: 37 },
    Class { cell: 28.0, density: 0.85, r_min: 1.5, r_max: 12.0, salt: 41 },
];

fn bits(h: u32) -> f32 {
    (h & 0xffffff) as f32 / 16777216.0
}

struct Crater {
    centre: Vec2,
    radius: f32,
    /// 1 fresh … 0.15 worn nearly flat. Most craters are old.
    fresh: f32,
    /// Per-crater random bits, for its rays.
    seed: u32,
}

fn crater_in(class: &Class, i: i32, j: i32) -> Option<Crater> {
    let h = hash(i, j, class.salt);
    if bits(h) > class.density {
        return None;
    }
    let (hx, hz, hr, ha) = (hash(i, j, h), hash(j, i, h), hash(i ^ 0x55, j, h), hash(i, j ^ 0x33, h));
    let span = class.cell * 0.4;
    let centre = Vec2::new(
        (i as f32 + 0.5) * class.cell + (bits(hx) - 0.5) * 2.0 * (class.cell * 0.5 - span * 0.5),
        (j as f32 + 0.5) * class.cell + (bits(hz) - 0.5) * 2.0 * (class.cell * 0.5 - span * 0.5),
    );
    // Small craters are far more common than large ones.
    let radius = class.r_min * (class.r_max / class.r_min).powf(bits(hr).powf(2.2));
    Some(Crater {
        centre,
        radius: radius.min(span),
        fresh: 0.15 + 0.85 * bits(ha).powf(2.5),
        seed: ha,
    })
}

/// Height of a simple crater's profile at `x` crater radii from its centre:
/// a bowl a fifth of the diameter deep, a rim lifted 3.6% of it, ejecta
/// falling off with the cube of the distance to zero at two radii.
fn profile(x: f32, radius: f32, fresh: f32) -> f32 {
    let d = 2.0 * radius;
    let (depth, rim) = (0.2 * d * fresh, 0.036 * d * fresh);
    if x < 1.0 {
        // A flattened bowl: steep walls, a broad floor.
        let s = x * x;
        rim - depth * (1.0 - s) * (1.0 - 0.35 * s)
    } else if x < 2.0 {
        rim * (1.0 / (x * x * x) - 0.125) / 0.875
    } else {
        0.0
    }
}

pub struct Moon {
    regional: Fbm<Perlin>,
    rolling: Fbm<Perlin>,
    maria: Fbm<Perlin>,
    mottle: Fbm<Perlin>,
}

impl Moon {
    fn new() -> Self {
        Self {
            regional: Fbm::<Perlin>::new(3).set_octaves(4).set_frequency(1.0 / 9000.0),
            rolling: Fbm::<Perlin>::new(4).set_octaves(5).set_frequency(1.0 / 700.0),
            maria: Fbm::<Perlin>::new(5).set_octaves(3).set_frequency(1.0 / 14000.0),
            mottle: Fbm::<Perlin>::new(6).set_octaves(4).set_frequency(1.0 / 400.0),
        }
    }

    /// 1 on the dark mare plains, 0 in the highlands.
    fn mare(&self, x: f32, z: f32) -> f32 {
        let m = self.maria.get([x as f64, z as f64]) as f32;
        ((m + 0.05) / 0.2).clamp(0.0, 1.0)
    }

    /// Calls `visit` with every crater whose surroundings (out to `reach`
    /// radii, at most 2.8) hold the point, and the point's distance from its
    /// centre in radii.
    fn each_crater(&self, x: f32, z: f32, mare: f32, reach: f32, mut visit: impl FnMut(&Crater, f32, Vec2)) {
        let p = Vec2::new(x, z);
        for class in &CLASSES {
            let (ci, cj) = ((x / class.cell).floor() as i32, (z / class.cell).floor() as i32);
            for j in cj - 1..=cj + 1 {
                for i in ci - 1..=ci + 1 {
                    let Some(c) = crater_in(class, i, j) else { continue };
                    // The maria are younger: fewer large craters survive on them.
                    if class.cell > 500.0 && mare > 0.5 && c.fresh < 0.5 {
                        continue;
                    }
                    let d = p - c.centre;
                    let x = d.length() / c.radius;
                    if x < reach {
                        visit(&c, x, d);
                    }
                }
            }
        }
    }

    fn craters(&self, x: f32, z: f32, mare: f32) -> f32 {
        let mut h = 0.0;
        self.each_crater(x, z, mare, 2.0, |c, x, _| h += profile(x, c.radius, c.fresh));
        h
    }

    /// How much brighter than its surroundings the soil is here: young
    /// craters have thrown out fresh, unweathered material, as a bright
    /// collar round the rim that breaks up into rays further out, and their
    /// walls have not darkened yet.
    fn ejecta(&self, x: f32, z: f32, mare: f32) -> f32 {
        let mut bright = 0.0f32;
        self.each_crater(x, z, mare, 2.8, |c, x, d| {
            let young = ((c.fresh - 0.55) / 0.45).max(0.0);
            if young == 0.0 {
                return;
            }
            let b = if x < 1.0 {
                0.25 + 0.75 * x * x
            } else {
                let out = 1.0 - (x - 1.0) / 1.8;
                let angle = d.y.atan2(d.x);
                let n = 5 + (c.seed >> 8) % 5;
                let phase = bits(c.seed >> 3) * std::f32::consts::TAU;
                let ray = (0.5 + 0.5 * (angle * n as f32 + phase).sin()).powi(3)
                    + 0.5 * (0.5 + 0.5 * (angle * (2 * n + 3) as f32 - phase).sin()).powi(4);
                // Solid at the rim, only the rays reaching far.
                let collar = (1.0 - (x - 1.0) / 0.6).max(0.0);
                out * out * (collar + (1.0 - collar) * ray.min(1.0))
            };
            bright = bright.max(young * b);
        });
        bright
    }

    /// 0–1: how strewn with blocks the ground is. Young craters dug theirs
    /// out of the bedrock and dropped them on the rim and just beyond; old
    /// ground has had its rocks ground to soil.
    pub fn rockiness(&self, x: f32, z: f32) -> f32 {
        let mut rocky = 0.035f32;
        self.each_crater(x, z, self.mare(x, z), 2.4, |c, x, _| {
            if c.radius < 2.5 || c.fresh < 0.4 {
                return;
            }
            let near = if x < 1.0 { x * x * x } else { ((2.4 - x) / 1.4).powi(2) };
            rocky = rocky.max((c.fresh - 0.4) / 0.6 * near);
        });
        rocky
    }
}

impl Moon {
    /// The first level, open spot on a spiral out from the origin, so the walk
    /// starts on sunlit plain rather than in a crater's shadow.
    fn landing_site(&self) -> (f32, f32) {
        for k in 0..6000 {
            let (angle, radius) = (k as f32 * 2.4, 15.0 * (k as f32).sqrt());
            let (x, z) = (radius * angle.cos(), radius * angle.sin());
            let h = self.height(x, z);
            let rough = [(25.0, 0.0), (-25.0, 0.0), (0.0, 25.0), (0.0, -25.0), (60.0, 0.0), (0.0, 60.0)]
                .iter()
                .map(|(dx, dz)| (self.height(x + dx, z + dz) - h).abs())
                .fold(0.0, f32::max);
            if rough < 0.6 {
                return (x, z);
            }
        }
        (0.0, 0.0)
    }
}

impl Ground for Moon {
    fn height(&self, x: f32, z: f32) -> f32 {
        let mare = self.mare(x, z);
        let (xd, zd) = (x as f64, z as f64);
        let highlands = 300.0 * self.regional.get([xd, zd]) as f32 + 25.0 * self.rolling.get([xd, zd]) as f32;
        // Maria are flooded flat and lie lower.
        let base = highlands * (1.0 - 0.8 * mare) - 250.0 * mare;
        base + self.craters(x, z, mare)
    }

    fn tint(&self, x: f32, z: f32, _height: f32, normal: Vec3) -> LinearRgba {
        let mare = self.mare(x, z);
        let (xd, zd) = (x as f64, z as f64);
        let albedo = 0.17 * (1.0 - mare) + 0.085 * mare;
        // Patches of older and younger soil, hundreds of metres across.
        let mottle = 1.0 + 0.12 * self.mottle.get([xd, zd]) as f32;
        // Steep slopes shed their weathered top and show brighter, fresher
        // material; so does what young craters threw out.
        let slope = (1.0 - normal.y).clamp(0.0, 0.3) / 0.3;
        let grain = hash2(x * 0.7, z * 0.7) * 0.03 - 0.015;
        let a = albedo * mottle * (1.0 + 0.35 * slope + 0.55 * self.ejecta(x, z, mare)) + grain * albedo;
        // Highland soil is faintly warm, mare basalt a little bluer.
        Pattern::Regolith.on(LinearRgba::rgb(a * (1.05 - 0.03 * mare), a, a * (0.93 + 0.05 * mare)))
    }
}

fn main() {
    let moon = Arc::new(Moon::new());
    let (x, z) = moon.landing_site();
    let mut app = tranquility::app(WorldDef {
        name: "Moon".into(),
        ground: moon.clone(),
        terrain: TerrainSettings {
            chunk_size: 256.0,
            view_radius: 16,
            lods: vec![(1, 256), (3, 128), (7, 64), (i32::MAX, 32)],
            roughness: 0.98,
            reflectance: 0.2,
            casts_shadows: true,
        },
        sky: SkySettings {
            air: Air::None,
            sun: SunPath::Fixed {
                azimuth: 100.0,
                elevation: 14.0,
            },
            stars: 900.0,
            ambient: 3000.0,
            exposure: 12.6,
            ..default()
        },
        water: None,
        walker: WalkerSettings {
            gravity: 1.62,
            jump: 2.2,
            walk: 1.2,
            run: 2.6,
            // The ground is all small craters: let the eye glide over them.
            eye_smoothing: 0.4,
            ..default()
        },
        // Looking north-west, the sun low on the right: relief shows best
        // across the light.
        spawn: Spawn {
            x,
            z,
            yaw: 0.5,
            pitch: -0.12,
        },
    });
    app.insert_resource(rocks::Field(moon))
        .add_plugins((rocks::RocksPlugin, heavens::HeavensPlugin));
    app.run();
}
