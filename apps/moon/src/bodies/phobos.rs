//! Phobos: twenty-two kilometres of dark rubble, six thousand above Mars.
//! Gravity is a two-thousandth of Earth's: a step is a slow leap of minutes,
//! a jump would take a quarter of an hour to come down, and the horizon is
//! a couple of hundred metres off and visibly curved. The regolith is
//! deep, dark and reddish, pocked to saturation, and scored by families of
//! parallel grooves, chains of pits run together. Mars fills a quarter of
//! the sky and never moves.

use std::sync::Arc;

use tranquility::{
    noise::{fbm, fbm_to, smoothstep},
    prelude::*,
};

use super::{lane, roll};
use crate::{
    body::{Body, Land, Rocks, level_site},
    craters::{Craters, Population},
    heavens::{Companion, Map},
};

struct Phobos {
    craters: Craters,
}

impl Phobos {
    /// Grooves: 100–200 m wide, 10–30 m deep, each a row of merged pits.
    /// Two families cross at a low angle.
    fn grooves(&self, x: f64, z: f64) -> f32 {
        let mut h = 0.0;
        for (bearing, spacing, seed) in [(0.25f32, 420.0, 81u32), (0.62, 760.0, 82)] {
            let (across, along, id) = lane(x, z, bearing, spacing, seed);
            if roll(id, 1) > 0.8 {
                continue;
            }
            let half = 45.0 + 45.0 * roll(id, 2);
            let depth = 9.0 + 18.0 * roll(id, 3);
            let u = (across / half).abs();
            if u >= 1.0 {
                continue;
            }
            // Scalloped along its length, pit after pit.
            let pits = 0.75 + 0.25 * ((along / (half as f64 * 1.3)) as f32 * std::f32::consts::TAU).cos();
            h -= depth * pits * (1.0 - u * u).powi(2);
        }
        h
    }
}

impl Ground for Phobos {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        // A lumpy little body: swells of tens of metres over a kilometre.
        let swell = 70.0 * fbm_to(x, z, 2400.0, 5, 0.5, 83, step);
        swell + self.grooves(x, z) + self.craters.height(x, z, step, f32::MAX, &|_, _| true)
    }

    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba {
        self.tint_at(x, z, height, normal, 0.0)
    }

    fn tint_at(&self, x: f64, z: f64, _height: f32, normal: Vec3, step: f32) -> LinearRgba {
        // Two units: the red one that covers most of it, and a bluer,
        // fresher one in patches (round Stickney, in life).
        let blue = smoothstep(0.1, 0.4, fbm(x, z, 1500.0, 4, 0.5, 84));
        let mottle = 1.0 + 0.12 * fbm(x, z, 200.0, 4, 0.5, 85);
        let slope = (1.0 - normal.y).clamp(0.0, 0.3) / 0.3;
        let rays = self.craters.ejecta(x, z, step, f32::MAX, &|_, _| true);
        let a = 0.068 * mottle * (1.0 + 0.25 * slope + 0.35 * rays) * (1.0 + 0.15 * blue);
        let c = Vec3::new(1.1, 1.0, 0.88).lerp(Vec3::new(1.02, 1.0, 0.97), blue) * a;
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Phobos {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        (0.04 + self.craters.blocks(x, z)).min(1.0)
    }

    fn landing_site(&self) -> (f64, f64) {
        level_site(self, 0.0, 0.0, 0.8)
    }
}

pub fn body() -> Body {
    let land = Phobos {
        // Loose rubble in almost no gravity keeps shallower bowls.
        craters: Craters::new(Population { d_max: 1200.0, saturated: 600.0, depth: 0.16, seed: 1877, ..default() }),
    };
    Body {
        note: "Walking is not a thing you can do here. Space jumps (you will be a while); in flight WASD, Space and C fire the suit's jets.",
        gravity: 0.0057,
        radius: 11_100.0,
        au: 1.524,
        albedo: 0.07,
        grip: 0.6,
        sun: (100.0, 22.0),
        rocks: Rocks { density: 0.1, largest: 8.0, albedo: [0.06, 0.1], tint: [1.08, 1.0, 0.9], ..default() },
        // Mars, 42° across from 6,000 km up.
        companions: vec![Companion { turn: [0.2, 0.9, 0.1], ..Companion::new(Map::Mars, 21.2, 0.0, 33.0) }],
        // The whole moon is a few kilometres round the curve.
        levels: 5,
        look: (0.0, 0.3),
        rover: false,
        ..Body::airless("Phobos", "Sub-Mars hemisphere", Arc::new(land))
    }
}
