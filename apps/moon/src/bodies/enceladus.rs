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
    plumes::Plume,
};

struct Enceladus {
    craters: Craters,
}

impl Enceladus {
    /// Signed distance to the nearest tiger stripe's axis, m. The stripes
    /// lie 35 km apart and wander a little.
    fn stripe(&self, x: f64, z: f64) -> f32 {
        let wander = 900.0 * fbm(x, z, 12_000.0, 3, 0.5, 151) as f64;
        lane(x, z + wander - 9_200.0, std::f32::consts::FRAC_PI_2, 35_000.0, 152).0
    }

    /// The nearest stripe's axis north or south of `z` at `x`: where the
    /// signed distance changes sign, within twenty kilometres.
    fn axis(&self, x: f64, z: f64) -> Option<f64> {
        let step = 500.0;
        let mut best: Option<(f64, f64)> = None;
        for i in -40i32..40 {
            let (z0, z1) = (z + i as f64 * step, z + (i + 1) as f64 * step);
            let (d0, d1) = (self.stripe(x, z0), self.stripe(x, z1));
            // A crossing of the axis, not the jump between two lanes.
            if d0.signum() != d1.signum() && (d0 - d1).abs() < 2.0 * step as f32 && best.is_none_or(|(a, _)| (a - z).abs() > (z0 - z).abs()) {
                best = Some((z0, z1));
            }
        }
        let (mut a, mut b) = best?;
        for _ in 0..30 {
            let m = 0.5 * (a + b);
            if self.stripe(x, a).signum() == self.stripe(x, m).signum() {
                a = m;
            } else {
                b = m;
            }
        }
        Some(0.5 * (a + b))
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

    /// Along the nearest stripe's axis, a jet every couple of kilometres
    /// (a hundred-odd have been counted along the four stripes), each a
    /// little off its slot.
    fn vents(&self, near: (f64, f64)) -> Vec<(f64, f64)> {
        let mut vents = Vec::new();
        for k in -6i32..=6 {
            let slot = (near.0 / 2200.0).round() as i32 + k;
            let x = (slot as f64 + 0.8 * (unit(slot, 1, 0x7e7) as f64 - 0.5)) * 2200.0;
            if let Some(z) = self.axis(x, near.1) {
                vents.push((x, z));
            }
        }
        vents
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
        plume: Some(Plume { width: 1000.0, height: 5000.0, color: [0.85, 0.92, 1.0], opacity: 0.3 }),
        // A 250 km moon: 12 km of ground is already well round the curve.
        levels: 7,
        look: (-0.1, 0.1),
        ..Body::airless("Enceladus", "Tiger stripe, south pole", Arc::new(land))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jets_stand_on_the_stripe_within_sight() {
        let land = Enceladus { craters: Craters::new(Population { seed: 1789, ..default() }) };
        let start = land.landing_site();
        let vents = land.vents(start);
        assert!(vents.len() >= 10, "{}", vents.len());
        for (x, z) in vents {
            assert!(land.stripe(x, z).abs() < 1.0, "off the axis at {x}, {z}");
            assert!((z - start.1).abs() < 5000.0, "too far: {} m", (z - start.1).abs());
        }
    }
}
