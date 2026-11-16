//! An endless island landscape: domain-warped hills and ridged ranges, sea,
//! Earth's sky. The smallest complete world: a height function, a tint
//! function and some numbers.

use tranquility::prelude::*;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin, RidgedMulti};

const SEA_LEVEL: f32 = 0.0;

struct Island {
    continents: Fbm<Perlin>,
    ridges: RidgedMulti<Perlin>,
    warp: Fbm<Perlin>,
    detail: Fbm<Perlin>,
}

impl Island {
    fn new(seed: u32) -> Self {
        Self {
            continents: Fbm::<Perlin>::new(seed).set_octaves(5).set_frequency(1.0 / 6000.0),
            ridges: RidgedMulti::<Perlin>::new(seed + 1).set_octaves(7).set_frequency(1.0 / 2600.0),
            warp: Fbm::<Perlin>::new(seed + 2).set_octaves(3).set_frequency(1.0 / 3500.0),
            detail: Fbm::<Perlin>::new(seed + 3).set_octaves(4).set_frequency(1.0 / 40.0),
        }
    }

    /// The first spot of dry, gently sloping ground on a spiral out from the
    /// origin, so the walk doesn't start in the sea.
    fn landfall(&self) -> (f64, f64) {
        for k in 0..4000 {
            let (angle, radius) = (k as f64 * 2.4, 20.0 * (k as f64).sqrt());
            let (x, z) = (radius * angle.cos(), radius * angle.sin());
            let h = self.height(x, z);
            let slope = (self.height(x + 5.0, z) - h).abs().max((self.height(x, z + 5.0) - h).abs());
            if (40.0..250.0).contains(&h) && slope < 1.0 {
                return (x, z);
            }
        }
        (0.0, 0.0)
    }
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Ground for Island {
    fn height(&self, x: f64, z: f64) -> f32 {
        // Domain warping bends the ridgelines so they don't run in
        // grid-aligned streaks.
        let wx = x + 700.0 * self.warp.get([x, z]);
        let wz = z + 700.0 * self.warp.get([x + 5200.0, z - 1300.0]);
        let continent = self.continents.get([wx, wz]);
        let mountains = smoothstep(-0.15, 0.45, continent);
        // Ridged noise alone grows lone cone peaks; blending in some of the
        // smooth continent noise gives the ranges shoulders.
        let ridges = (self.ridges.get([wx, wz]) * 0.5 + 0.5).clamp(0.0, 1.0);
        let massif = 0.7 * ridges + 0.3 * (continent * 0.5 + 0.5);
        let h = 25.0 + 160.0 * continent + 650.0 * mountains * massif.powf(1.6) + 1.5 * self.detail.get([x, z]);
        h as f32
    }

    /// Sand by the water, grass on gentle slopes, rock on steep ones, snow on
    /// high flat ground.
    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba {
        let flat = normal.y;
        let jitter = hash2(x * 0.37, z * 0.37) * 0.12 - 0.06;
        let srgb = |r: f32, g: f32, b: f32| LinearRgba::from(Color::srgb(r, g, b));
        let sand = srgb(0.62, 0.56, 0.43);
        let grass = srgb(0.24 + jitter, 0.31 + jitter, 0.12);
        let dry = srgb(0.42, 0.38, 0.22);
        let rock = srgb(0.36 + jitter, 0.34 + jitter, 0.31 + jitter);
        let snow = srgb(0.88, 0.9, 0.94);

        let mut color = grass.mix(&dry, ((height - 250.0) / 300.0).clamp(0.0, 1.0));
        color = sand.mix(&color, ((height - SEA_LEVEL - 1.0) / 3.0).clamp(0.0, 1.0));
        color = color.mix(&rock, ((0.86 - flat) / 0.1).clamp(0.0, 1.0));
        let snowline = 620.0 + 800.0 * jitter;
        let snowy = ((height - snowline) / 60.0).clamp(0.0, 1.0) * ((flat - 0.72) / 0.1).clamp(0.0, 1.0);
        let grassy = height > SEA_LEVEL + 3.0 && height < 450.0 && flat > 0.88;
        let pattern = if grassy && snowy < 0.5 { Pattern::Grass } else { Pattern::Plain };
        pattern.on(color.mix(&snow, snowy))
    }
}

fn main() {
    let island = Island::new(7);
    let (x, z) = island.landfall();
    tranquility::app(WorldDef {
        name: "Island".into(),
        ground: ground(island),
        terrain: TerrainSettings::default(),
        sky: SkySettings {
            sun: SunPath::Daily {
                latitude: 45.0,
                declination: 10.0,
            },
            start_hour: 9.5,
            ..default()
        },
        water: Some(WaterSettings {
            level: SEA_LEVEL,
            color: Color::srgb(0.02, 0.06, 0.08),
            roughness: 0.06,
        }),
        walker: WalkerSettings::default(),
        spawn: Spawn {
            x,
            z,
            yaw: 0.6,
            pitch: -0.05,
        },
    })
    .run();
}
