//! Io: the most volcanic body there is, resurfaced a centimetre a year, so
//! there is not one impact crater on it. Plains of sulphur and sulphur
//! dioxide frost, layered and yellow-white, with red and orange where the
//! sulphur has been hot; fresh lava flows lying black across them in
//! lobes; and, rarely, a mountain: a block of crust kilometres high,
//! tilted up as the layers sink under their own weight. Jupiter stands
//! twenty degrees across in a sky kept black by too little air.

use std::sync::Arc;

use tranquility::{
    noise::{fbm, fbm_to, ridged, smoothstep},
    prelude::*,
};

use crate::{
    body::{Body, Land, Rocks, level_site},
    heavens::{Companion, Map},
};

struct Io;

impl Io {
    /// 1 on a lava flow, 0 off it, with a sharp lobate edge.
    fn flow(&self, x: f64, z: f64) -> f32 {
        // Lobes: a noise field pulled out along the way the lava ran.
        let lobes = fbm(x * 0.6 + 0.35 * z, z, 1700.0, 5, 0.55, 111);
        let field = fbm(x, z, 9000.0, 3, 0.5, 112);
        smoothstep(0.0, 0.035, lobes + 0.9 * field - 0.32)
    }

    /// Height of the mountains: tilted blocks, a long back slope and a
    /// scarp, in the few places the gate opens.
    fn mountain(&self, x: f64, z: f64, step: f32) -> f32 {
        let gate = smoothstep(0.25, 0.45, fbm(x + 14_000.0, z + 6000.0, 60_000.0, 3, 0.5, 113));
        if gate <= 0.0 {
            return 0.0;
        }
        // Each block rises toward the north-east and breaks off there.
        let tilt = (fbm_to(x, z, 16_000.0, 4, 0.5, 114, step) + 0.3 + 0.5 * ((x - z) / 26_000.0) as f32).clamp(0.0, 1.0);
        let crags = ridged(x, z, 5000.0, 5, 115);
        gate * (5200.0 * tilt * tilt + 900.0 * tilt * crags)
    }
}

impl Ground for Io {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        // Layered plains: broad steps a few tens of metres high where
        // the frost has sapped a layer back.
        let plain = fbm_to(x, z, 6000.0, 5, 0.5, 116, step);
        let layers = 60.0 * plain + 22.0 * smoothstep(0.05, 0.09, plain) + 16.0 * smoothstep(-0.2, -0.17, plain);
        // A flow stands a few metres proud, its skin broken and ropy.
        let flow = self.flow(x, z);
        let ropy = if step < 4.0 { 0.9 * fbm_to(x, z, 14.0, 3, 0.5, 117, step) + 0.5 } else { 0.5 };
        layers + flow * (5.0 + 2.0 * ropy) + 1.2 * fbm_to(x, z, 180.0, 3, 0.5, 118, step) + self.mountain(x, z, step)
    }

    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba {
        // Sulphur dioxide frost, white; sulphur, yellow where cold and
        // orange to red where it was quenched from hot; grey-green where
        // sulphur meets iron.
        let frost = Vec3::new(0.78, 0.76, 0.6);
        let sulphur = Vec3::new(0.72, 0.52, 0.14);
        let red = Vec3::new(0.52, 0.19, 0.07);
        let green = Vec3::new(0.38, 0.4, 0.2);
        let a = fbm(x, z, 2600.0, 5, 0.55, 119);
        let b = fbm(x, z, 800.0, 4, 0.55, 120);
        let mut c = sulphur.lerp(frost, smoothstep(-0.15, 0.3, a + 0.3 * b));
        c = c.lerp(green, 0.6 * smoothstep(0.25, 0.5, -a + 0.4 * b));
        // Red rings the fresh flows: fallout from their vents.
        let flow = self.flow(x, z);
        let near = smoothstep(0.12, 0.3, fbm(x * 0.6 + 0.35 * z, z, 1700.0, 5, 0.55, 111) + 0.9 * fbm(x, z, 9000.0, 3, 0.5, 112));
        c = c.lerp(red, 0.7 * near * (1.0 - flow));
        // Lava: black when new, dusted with sulphur as it ages.
        let age = smoothstep(-0.2, 0.4, fbm(x, z, 500.0, 3, 0.5, 121));
        c = c.lerp(Vec3::new(0.035, 0.03, 0.03).lerp(Vec3::new(0.2, 0.13, 0.05), 0.5 * age), flow);
        // Mountains: bare grey-brown rock, frost in the hollows.
        let rock = smoothstep(120.0, 500.0, height) * smoothstep(0.02, 0.2, 1.0 - normal.y);
        c = c.lerp(Vec3::new(0.3, 0.24, 0.16), rock);
        let c = c * (1.0 + 0.07 * fbm(x, z, 40.0, 3, 0.5, 122));
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Io {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        // Broken crust on the flows, clean frost elsewhere.
        0.01 + 0.35 * self.flow(x, z)
    }

    fn landing_site(&self) -> (f64, f64) {
        // On the frost, with the snout of a lava flow a couple of hundred
        // metres ahead.
        for k in 0..6000 {
            let (angle, radius) = (k as f64 * 2.4, 40.0 * (k as f64).sqrt());
            let (x, z) = (radius * angle.cos(), radius * angle.sin());
            if self.flow(x, z) < 0.01 && self.flow(x - 30.0, z - 60.0) < 0.01 && self.flow(x - 90.0, z - 200.0) > 0.9 {
                return level_site(self, x, z, 0.5);
            }
        }
        level_site(self, 0.0, 0.0, 0.4)
    }
}

pub fn body() -> Body {
    Body {
        note: "No craters: the ground here is younger than a human lifetime. The black tongues are lava; the mountain is a tilted block of crust.",
        gravity: 1.796,
        radius: 1_821_600.0,
        au: 5.2,
        albedo: 0.55,
        grip: 0.6,
        soil: Soil { craterlets: 0.0, grain: 0.7, ripples: 0.0, pebbles: 0.25, pebble_tone: 0.7, sparkle: 0.25, powder: 0.3, surge: 0.3, surge_width: 0.1, backscatter: 0.2 },
        roughness: 0.9,
        reflectance: 0.3,
        sun: (100.0, 18.0),
        stars: 300.0,
        // Jupiter's light and the bright ground fill the shadows.
        fill: 1.4,
        fill_color: Color::linear_rgb(1.0, 0.95, 0.8),
        rocks: Rocks { density: 0.08, largest: 2.5, albedo: [0.04, 0.3], tint: [1.1, 0.95, 0.6], ..default() },
        companions: vec![Companion { turn: [0.05, 4.4, 0.06], ..Companion::new(Map::Jupiter, 9.8, 335.0, 30.0) }],
        look: (0.45, 0.06),
        ..Body::airless("Io", "Sulphur plains", Arc::new(Io))
    }
}
