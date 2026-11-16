//! Europa: a shell of water ice over an ocean, broken and healed again and
//! again. The plains are ridges laid over ridges: each a pair of crests a
//! hundred metres high with a trough between, running straight for
//! hundreds of kilometres, each younger one cutting across all the older.
//! In places the shell has come apart altogether: chaos, where rafts of
//! the old ridged surface lie tilted in a frozen matrix. Almost no craters;
//! the surface is younger than the dinosaurs.

use std::sync::Arc;

use tranquility::{
    noise::{cells, fbm, fbm_to, smoothstep},
    prelude::*,
};

use super::{lane, roll};
use crate::{
    body::{Body, Land, Rocks, level_site},
    craters::{Craters, Population},
    heavens::{Companion, Map},
};

struct Europa {
    craters: Craters,
}

/// Ridge families, oldest first: bearing, strip width (m), the widest half
/// width of a ridge (m), its greatest height (m).
const FAMILIES: [(f32, f64, f32, f32, u32); 7] = [
    (1.7, 330.0, 60.0, 9.0, 129),
    (0.9, 620.0, 110.0, 18.0, 130),
    (0.3, 1400.0, 260.0, 35.0, 131),
    (2.0, 2300.0, 420.0, 60.0, 132),
    (1.1, 4200.0, 700.0, 110.0, 133),
    (2.7, 7000.0, 900.0, 160.0, 134),
    (0.75, 12_000.0, 1100.0, 220.0, 135),
];

impl Europa {
    /// Height of the ridged plains, and how much dark (non-ice) material
    /// the point has: it gathers in the troughs and on the flanks.
    fn ridges(&self, x: f64, z: f64) -> (f32, f32) {
        let (mut h, mut dark) = (0.0, 0.0);
        for (bearing, spacing, half, high, seed) in FAMILIES {
            let (across, along, id) = lane(x, z, bearing, spacing, seed);
            if roll(id, 1) > 0.8 {
                continue;
            }
            let half = half * (0.5 + 0.5 * roll(id, 2));
            let high = high * (0.4 + 0.6 * roll(id, 3)) * (0.8 + 0.2 * ((along / 3000.0) as f32 + roll(id, 4) * 6.0).sin());
            let u = (across / half).abs();
            if u >= 1.0 {
                continue;
            }
            // Two crests at ±0.45 with a trough between, flanks falling
            // away outside; the ridge wipes out what was there before.
            let wipe = 1.0 - smoothstep(0.75, 1.0, u);
            let crest = (1.0 - ((u - 0.45) / 0.4).powi(2)).max(0.0);
            let profile = crest * crest - 0.25 * (1.0 - smoothstep(0.0, 0.25, u));
            h = h * (1.0 - wipe) + high * profile;
            let stain = (1.0 - smoothstep(0.0, 0.3, u)).max(smoothstep(0.6, 0.8, u) * (1.0 - smoothstep(0.85, 1.0, u)));
            dark = dark * (1.0 - wipe) + stain * (0.4 + 0.6 * roll(id, 5));
        }
        (h, dark)
    }

    /// How much of the shell has broken up here.
    fn chaos(&self, x: f64, z: f64) -> f32 {
        smoothstep(0.22, 0.34, fbm(x + 4000.0, z, 16_000.0, 4, 0.5, 136))
    }

    fn surface(&self, x: f64, z: f64, step: f32) -> (f32, f32) {
        let (mut h, mut dark) = self.ridges(x, z);
        let chaos = self.chaos(x, z);
        if chaos > 0.0 {
            // Rafts: polygons a kilometre or two across, each standing
            // at its own height, and the matrix between them low,
            // lumpy and dark.
            let (f1, f2, id) = cells(x / 1500.0, z / 1500.0, 137);
            let raft = smoothstep(0.06, 0.16, f2 - f1);
            let stand = 40.0 + 110.0 * roll(id, 2);
            let matrix = -30.0 + 25.0 * fbm_to(x, z, 300.0, 4, 0.55, 138, step);
            let broken = raft * (0.35 * h + stand) + (1.0 - raft) * matrix;
            h += chaos * (broken - h);
            dark += chaos * ((1.0 - raft) * 0.9 - dark * (1.0 - raft));
        }
        // Ice rubble underfoot.
        h += 0.5 * fbm_to(x, z, 25.0, 3, 0.5, 139, step) + 6.0 * fbm_to(x, z, 900.0, 4, 0.5, 140, step);
        (h, dark)
    }
}

impl Ground for Europa {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        self.surface(x, z, step).0 + self.craters.height(x, z, step, f32::MAX, &|_, _| true)
    }

    fn tint(&self, x: f64, z: f64, _height: f32, normal: Vec3) -> LinearRgba {
        let (_, dark) = self.surface(x, z, 0.0);
        // Clean ice, faintly blue, and frost where it has snowed back;
        // the dark stuff is salt, reddened by Jupiter's radiation.
        let ice = Vec3::new(0.66, 0.69, 0.74);
        let frost = Vec3::new(0.8, 0.81, 0.82);
        let salt = Vec3::new(0.36, 0.22, 0.13);
        let n = fbm(x, z, 700.0, 4, 0.55, 141);
        let mut c = ice.lerp(frost, smoothstep(-0.2, 0.3, n));
        // Besides the ridges' own stains, patches of it lie about.
        let patches = 0.35 * smoothstep(0.05, 0.4, fbm(x, z, 260.0, 4, 0.55, 143));
        let stain = ((dark + patches) * (0.75 + 0.5 * fbm(x, z, 90.0, 3, 0.5, 142))).clamp(0.0, 1.0);
        c = c.lerp(salt, 0.8 * stain);
        // Steep faces are bare, bluer ice.
        let slope = smoothstep(0.1, 0.4, 1.0 - normal.y);
        c = c.lerp(Vec3::new(0.5, 0.6, 0.72), 0.5 * slope);
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Europa {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        // Blocks of ice in the chaos and at the foot of ridges.
        (0.04 + 0.5 * self.chaos(x, z) + 0.2 * self.surface(x, z, 0.0).1).min(1.0)
    }

    fn landing_site(&self) -> (f64, f64) {
        level_site(self, 0.0, 0.0, 0.45)
    }
}

pub fn body() -> Body {
    let land = Europa {
        craters: Craters::new(Population { d_min: 8.0, d_max: 2000.0, density: 0.03, saturated: 30.0, depth: 0.14, seed: 1610, ..default() }),
    };
    Body {
        note: "Ridges across ridges across ridges: each is a crack that opened and closed with the tide. The brown is salt from the ocean below.",
        gravity: 1.315,
        radius: 1_560_800.0,
        au: 5.2,
        albedo: 0.62,
        // Ice far below freezing has no film of water: it grips like rock.
        grip: 0.55,
        soil: Soil { craterlets: 0.06, grain: 0.8, ripples: 0.0, pebbles: 0.35, pebble_tone: 1.0, sparkle: 1.0, powder: 0.35, surge: 0.5, surge_width: 0.08, backscatter: 0.25 },
        roughness: 0.8,
        reflectance: 0.35,
        sun: (100.0, 20.0),
        stars: 300.0,
        fill: 1.4,
        fill_color: Color::linear_rgb(0.9, 0.95, 1.0),
        rocks: Rocks { density: 0.07, largest: 6.0, albedo: [0.45, 0.72], tint: [0.95, 0.98, 1.05], roughness: 0.7, ..default() },
        companions: vec![Companion { turn: [0.05, 4.4, 0.06], ..Companion::new(Map::Jupiter, 6.1, 20.0, 32.0) }],
        look: (-0.25, 0.08),
        ..Body::airless("Europa", "Ridged plains", Arc::new(land))
    }
}
