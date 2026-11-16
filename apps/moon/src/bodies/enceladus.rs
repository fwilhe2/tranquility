//! Enceladus, at the south pole: the whitest ground in the solar system,
//! snow fallen back from its own geysers. Between the tiger stripes the
//! plains are folded into ropy ridges; the stripe itself, a kilometre
//! north, is a trough half a kilometre deep between raised lips, floored
//! with coarse blue ice and littered with house-sized blocks. Saturn sits
//! on the horizon, thirty degrees across, its rings a line: the moon
//! orbits in their plane. A ninetieth of Earth's sunlight, a hundredth of
//! its gravity: the suit carries jets.

use std::sync::Arc;

use tranquility::{
    noise::{fbm, fbm_to, smoothstep},
    prelude::*,
};

use super::lane;
use crate::{
    body::{Body, Land, Rocks, level_site},
    craters::{Craters, Population},
    heavens::{Companion, Map},
};

struct Enceladus {
    craters: Craters,
}

impl Enceladus {
    /// Signed distance to the nearest tiger stripe's axis, m. The stripes
    /// lie 35 km apart and wander a little.
    fn stripe(&self, x: f64, z: f64) -> f32 {
        let wander = 900.0 * fbm(x, z, 12_000.0, 3, 0.5, 151) as f64;
        lane(x, z + wander - 14_800.0, std::f32::consts::FRAC_PI_2, 35_000.0, 152).0
    }

    fn base(&self, x: f64, z: f64, step: f32) -> (f32, f32) {
        let d = self.stripe(x, z).abs();
        // The trough: 2 km across, half a kilometre deep, V-shaped, with
        // lips a hundred metres high on either side.
        let trough = -480.0 * (1.0 - smoothstep(0.0, 1000.0, d)).powf(1.5);
        let lip = 110.0 * (1.0 - ((d - 1300.0) / 700.0).powi(2)).max(0.0).powi(2);
        // Funiscular plains: folds half a kilometre apart, fading toward
        // the stripe, their crests wavering.
        let warp = 1.2 * fbm(x, z, 2500.0, 3, 0.5, 153) as f64;
        let fold = ((z / 520.0 + x / 4000.0 + warp) * std::f64::consts::TAU).sin() as f32;
        let folds = 28.0 * fold * (0.6 + 0.4 * fbm(x, z, 3000.0, 2, 0.5, 154)) * smoothstep(1500.0, 4000.0, d);
        let swell = 120.0 * fbm_to(x, z, 15_000.0, 4, 0.5, 155, step) + 5.0 * fbm_to(x, z, 300.0, 4, 0.5, 156, step);
        (trough + lip + folds + swell, d)
    }
}

impl Ground for Enceladus {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        self.base(x, z, step).0 + self.craters.height(x, z, step, f32::MAX, &|_, _| true)
    }

    fn tint(&self, x: f64, z: f64, _height: f32, normal: Vec3) -> LinearRgba {
        let d = self.stripe(x, z).abs();
        // Fine fresh snow everywhere, coarse-grained ice (which reads
        // blue-green) in and beside the stripe and on steep faces.
        let snow = Vec3::new(0.93, 0.94, 0.95);
        let ice = Vec3::new(0.52, 0.7, 0.78);
        let coarse = (1.0 - smoothstep(600.0, 2600.0, d)).max(smoothstep(0.12, 0.4, 1.0 - normal.y) * 0.7);
        let c = snow.lerp(ice, coarse * (0.7 + 0.3 * fbm(x, z, 120.0, 3, 0.5, 157))) * (1.0 + 0.03 * fbm(x, z, 60.0, 3, 0.5, 158));
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Enceladus {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        // Ice blocks by the hundred on the stripe's flanks.
        let d = self.stripe(x, z).abs();
        0.03 + 0.8 * (1.0 - smoothstep(900.0, 2600.0, d))
    }

    fn landing_site(&self) -> (f64, f64) {
        level_site(self, 0.0, 0.0, 0.5)
    }
}

pub fn body() -> Body {
    let land = Enceladus {
        // The south polar terrain is young: hardly a crater.
        craters: Craters::new(Population { d_min: 8.0, d_max: 1000.0, density: 0.04, saturated: 40.0, depth: 0.15, seed: 1789, ..default() }),
    };
    Body {
        note: "The trough to the north is a tiger stripe: the ocean vents through it. Saturn does not move; its rings are edge-on from here.",
        gravity: 0.113,
        radius: 252_100.0,
        au: 9.58,
        albedo: 0.9,
        grip: 0.5,
        soil: Soil { craterlets: 0.0, grain: 0.45, ripples: 0.0, pebbles: 0.15, pebble_tone: 1.0, sparkle: 1.5, powder: 0.2, surge: 0.4, surge_width: 0.06, backscatter: 0.1 },
        roughness: 0.75,
        reflectance: 0.3,
        sun: (95.0, 11.0),
        stars: 200.0,
        // Snow lights its own shadows as nothing else does.
        fill: 1.8,
        fill_color: Color::linear_rgb(0.85, 0.93, 1.0),
        rocks: Rocks { density: 0.12, largest: 14.0, albedo: [0.55, 0.85], tint: [0.9, 0.98, 1.05], roughness: 0.6, ..default() },
        companions: vec![Companion { turn: [0.0, 1.0, 0.0], rings: Some((1.24, 2.27)), ..Companion::new(Map::Saturn, 14.7, 5.0, 11.5) }],
        // A 250 km moon: 12 km of ground is already well round the curve.
        levels: 7,
        look: (-0.1, 0.1),
        ..Body::airless("Enceladus", "Tiger stripe, south pole", Arc::new(land))
    }
}
