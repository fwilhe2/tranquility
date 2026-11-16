//! Pluto: the shore of Sputnik Planitia. A glacier of nitrogen ice a
//! thousand kilometres across, soft enough at 38 kelvin to churn slowly
//! from below: its surface is divided into cells tens of kilometres wide
//! by shallow troughs, pocked with fields of sublimation pits, and has not
//! one crater. Along its edge stand mountains of water ice, hard as rock
//! at this temperature, in blocks kilometres high, stained red-brown by
//! tholins and capped with methane frost. The sun is a point fifteen
//! hundred times fainter than on Earth, which is still a few hundred
//! full moons; Charon hangs seven Moons wide and never sets.

use std::sync::Arc;

use bevy::light::atmosphere::{Falloff, PhaseFunction, ScatteringMedium, ScatteringTerm};
use tranquility::{
    noise::{cells, fbm, fbm_to, ridged, smoothstep},
    prelude::*,
};

use crate::{
    body::{Body, Land, Rocks, level_site},
    heavens::{Companion, Map},
};

struct Pluto;

impl Pluto {
    /// 0 on the ice plain, 1 in the mountains east of it.
    fn shore(&self, x: f64, z: f64) -> f32 {
        smoothstep(-0.05, 0.12, fbm(x, z, 22_000.0, 4, 0.5, 171) + (x as f32 - 9000.0) / 30_000.0)
    }

    fn plain(&self, x: f64, z: f64, step: f32) -> f32 {
        // Convection cells: each a low dome, a trough round its edge.
        let (f1, f2, _) = cells(x / 18_000.0, z / 18_000.0, 172);
        let trough = -55.0 * (1.0 - smoothstep(0.0, 0.07, f2 - f1));
        let dome = 45.0 * (1.0 - f1).max(0.0);
        // Sublimation pits, in swarms: elongated hollows a few hundred
        // metres long, all trending one way.
        let swarm = smoothstep(0.05, 0.3, fbm(x, z, 6000.0, 3, 0.5, 173));
        let mut pits = 0.0;
        if swarm > 0.0 && step < 150.0 {
            let (p1, _, id) = cells(x / 340.0, (z + 0.4 * x) / 700.0, 174);
            let depth = 12.0 + 22.0 * (id & 0xff) as f32 / 255.0;
            // Steep walls, flat floors; a coarse grid draws them fainter.
            pits = -swarm * depth * (1.0 - smoothstep(0.2, 0.36, p1)) * (1.0 - smoothstep(30.0, 150.0, step));
        }
        trough + dome + pits + 1.5 * fbm_to(x, z, 400.0, 3, 0.5, 175, step)
    }

    fn mountains(&self, x: f64, z: f64, step: f32) -> f32 {
        // Angular blocks jumbled together, each a few kilometres across,
        // with sharp crests: ice mountains have had nothing to wear them.
        let (f1, f2, id) = cells(x / 5200.0, z / 5200.0, 176);
        let block = smoothstep(0.02, 0.3, f2 - f1) * (0.35 + 0.65 * (id >> 8 & 0xff) as f32 / 255.0);
        let crags = ridged(x, z, 3800.0, 6, 177);
        2600.0 * block * (0.45 + 0.55 * crags) + 300.0 * fbm_to(x, z, 9000.0, 4, 0.5, 178, step)
    }
}

impl Ground for Pluto {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        let shore = self.shore(x, z);
        let mut h = 0.0;
        if shore < 1.0 {
            h += (1.0 - shore) * self.plain(x, z, step);
        }
        if shore > 0.0 {
            h += shore * shore * self.mountains(x, z, step);
        }
        h
    }

    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba {
        let shore = self.shore(x, z);
        // Nitrogen ice with a little methane: cream, faintly pink.
        let n2 = Vec3::new(0.78, 0.73, 0.64);
        let (f1, f2, _) = cells(x / 18_000.0, z / 18_000.0, 172);
        // Troughs gather darker, redder grains.
        let seam = 1.0 - smoothstep(0.0, 0.05, f2 - f1);
        let mut c = n2.lerp(Vec3::new(0.45, 0.33, 0.24), 0.6 * seam);
        c *= 1.0 + 0.05 * fbm(x, z, 800.0, 4, 0.5, 179);
        // Mountains: water ice under red-brown tholin, methane frost
        // white on the heights and the gentle slopes.
        let tholin = Vec3::new(0.3, 0.16, 0.1);
        let bare = Vec3::new(0.42, 0.4, 0.4);
        let steep = smoothstep(0.1, 0.4, 1.0 - normal.y);
        let frost = smoothstep(1100.0, 1700.0, height + 300.0 * fbm(x, z, 2500.0, 3, 0.5, 180)) * (1.0 - 0.7 * steep);
        let rock = tholin.lerp(bare, steep).lerp(Vec3::new(0.85, 0.84, 0.82), frost);
        let c = c.lerp(rock, smoothstep(0.15, 0.6, shore));
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Pluto {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        0.01 + 0.5 * smoothstep(0.3, 0.8, self.shore(x, z))
    }

    fn landing_site(&self) -> (f64, f64) {
        level_site(self, 0.0, 0.0, 0.35)
    }
}

/// A trace of nitrogen with layers of haze: too thin to dim anything, but
/// along the horizon and round the sun it shows, and it is blue.
fn air() -> Air {
    const HEIGHT: f32 = 200_000.0;
    Air::Medium(Arc::new(AirDef {
        radius: 1_188_300.0,
        height: HEIGHT,
        ground_albedo: Vec3::splat(0.5),
        medium: ScatteringMedium::new(
            128,
            128,
            [ScatteringTerm {
                absorption: Vec3::ZERO,
                scattering: Vec3::new(2.0e-8, 4.5e-8, 1.1e-7),
                falloff: Falloff::Exponential { scale: 50_000.0 / HEIGHT },
                phase: PhaseFunction::Mie { asymmetry: 0.55 },
            }],
        )
        .with_label("pluto_air"),
        thick: false,
    }))
}

pub fn body() -> Body {
    Body {
        note: "The plain is nitrogen ice, slowly overturning; the mountains are water ice. Noon here is as bright as a living room.",
        gravity: 0.62,
        radius: 1_188_300.0,
        au: 39.5,
        albedo: 0.6,
        grip: 0.5,
        soil: Soil { craterlets: 0.0, grain: 0.4, ripples: 0.0, pebbles: 0.1, pebble_tone: 1.0, sparkle: 0.7, powder: 0.3, surge: 0.4, surge_width: 0.08, backscatter: 0.2 },
        roughness: 0.8,
        reflectance: 0.3,
        air: air(),
        sun: (250.0, 16.0),
        stars: 60.0,
        fill: 1.2,
        fill_color: Color::linear_rgb(0.9, 0.95, 1.0),
        rocks: Rocks { density: 0.08, largest: 6.0, albedo: [0.3, 0.6], tint: [1.05, 0.95, 0.9], roughness: 0.7, ..default() },
        // Charon: 1.2 thousand kilometres across at 19,600.
        companions: vec![Companion { turn: [0.3, 1.0, 0.2], ..Companion::new(Map::Charon, 1.77, 60.0, 34.0) }],
        look: (-1.2, 0.05),
        ..Body::airless("Pluto", "Sputnik Planitia, eastern shore", Arc::new(Pluto))
    }
}
