//! Mercury: intercrater plains, the most heavily cratered ground among
//! these worlds, under a sun two and a half times the size it is from
//! Earth and nearly seven times as bright. The planet shrank as its core
//! froze, and the crust took it up in thrust faults: lobate scarps, cliffs
//! a kilometre high and hundreds long. One stands north of the site.

use std::sync::Arc;

use tranquility::{
    noise::{fbm, fbm_to, smoothstep},
    prelude::*,
};

use super::lane;
use crate::{
    body::{Body, Land, Rocks},
    craters::{Craters, Population},
};

struct Mercury {
    craters: Craters,
}

impl Mercury {
    /// The scarp nearest a point: 0 below it, 1 on the raised block behind
    /// its face. One fault to a 60 km strip, its trace bowed like the
    /// lobes that give these scarps their name.
    fn scarp(&self, x: f64, z: f64) -> f32 {
        let bow = 2200.0 * fbm(x, z, 30_000.0, 3, 0.5, 71) as f64;
        let (across, _, _) = lane(x, z + bow, 1.45, 60_000.0, 73);
        // A steep face 700 m wide, a long gentle back.
        smoothstep(-350.0, 350.0, across) * (1.0 - smoothstep(350.0, 14_000.0, across))
    }
}

impl Ground for Mercury {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        let regional = 500.0 * fbm_to(x, z, 40_000.0, 5, 0.5, 61, step);
        let rolling = 60.0 * fbm_to(x, z, 2600.0, 5, 0.5, 62, step);
        let scarp = 950.0 * self.scarp(x, z);
        regional + rolling + scarp + self.craters.height(x, z, step, f32::MAX, &|_, _| true)
    }

    fn tint(&self, x: f64, z: f64, _height: f32, normal: Vec3) -> LinearRgba {
        // Darker than the Moon and less red: carbon, perhaps, from the
        // old crust. Smooth volcanic plains run a little brighter.
        let plains = smoothstep(-0.1, 0.25, fbm(x, z, 22_000.0, 4, 0.5, 63));
        let albedo = 0.1 + 0.035 * plains;
        let mottle = 1.0 + 0.1 * fbm(x, z, 500.0, 4, 0.5, 64);
        let slope = (1.0 - normal.y).clamp(0.0, 0.3) / 0.3;
        let rays = self.craters.ejecta(x, z, f32::MAX, &|_, _| true);
        let a = albedo * mottle * (1.0 + 0.3 * slope + 0.75 * rays);
        // Hollows: bright, faintly blue patches where something volatile
        // has gone out of the ground. They keep to low spots.
        let hollow = smoothstep(0.42, 0.5, fbm(x, z, 900.0, 4, 0.55, 65)) * smoothstep(0.1, 0.5, fbm(x, z, 9000.0, 3, 0.5, 66));
        let c = Vec3::new(a * 1.03, a, a * 0.95).lerp(Vec3::new(0.3, 0.33, 0.37), hollow * 0.8);
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Mercury {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        (0.025 + self.craters.blocks(x, z)).min(1.0)
    }
}

pub fn body() -> Body {
    let land = Mercury {
        // Older than any lunar mare: saturated to nearly a kilometre,
        // with the swarms of secondaries Mercury's gravity keeps close.
        craters: Craters::new(Population { d_max: 12_000.0, saturated: 800.0, density: 1.15, complex: 10_000.0, seed: 1974, ..default() }),
    };
    Body {
        note: "The sun is two and a half times the width it has from Earth. The cliff to the north is a thrust fault: the planet has shrunk.",
        gravity: 3.70,
        radius: 2_439_700.0,
        au: 0.387,
        albedo: 0.11,
        sun: (100.0, 12.0),
        soil: Soil { pebbles: 0.4, ..default() },
        rocks: Rocks { density: 0.12, largest: 5.0, albedo: [0.09, 0.16], tint: [1.02, 1.0, 0.96], ..default() },
        look: (0.2, -0.08),
        ..Body::airless("Mercury", "Intercrater plains", Arc::new(land))
    }
}
