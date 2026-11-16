//! The Moon: a mare's edge. Dark basalt plains, flooded three and a half
//! thousand million years ago and level to the horizon but for craters and
//! the long low swells of wrinkle ridges; and north of them the highlands,
//! the old bright crust, heaped into rounded massifs kilometres high by the
//! basin impacts and softened ever since by a rain of small ones. The view
//! from Hadley or Taurus-Littrow, not from Tranquility Base.

use std::sync::Arc;

use tranquility::{
    noise::{fbm, fbm_to, ridged, smoothstep},
    prelude::*,
};

use crate::{
    body::{Body, Land, Rocks, level_site},
    craters::{Craters, Population},
    heavens::{Companion, Map},
};

pub struct Moon {
    craters: Craters,
}

/// Craters wider than this may have been flooded by the mare, m.
const FLOODED: f32 = 250.0;

impl Moon {
    fn new() -> Self {
        Self {
            // Saturated to a couple of hundred metres: the mare's age.
            craters: Craters::new(Population { d_max: 12_000.0, saturated: 220.0, seed: 1969, ..default() }),
        }
    }

    /// 1 on the dark mare plains, 0 in the highlands.
    fn mare(&self, x: f64, z: f64) -> f32 {
        // A bay of the mare runs east–west past the start; the shore
        // wanders by tens of kilometres.
        let shore = fbm(x, z, 70_000.0, 4, 0.5, 5) + (z as f32 + 9000.0) / 60_000.0;
        smoothstep(-0.06, 0.06, shore)
    }

    /// Most large craters on the mare are older than the lava that
    /// filled it; one in four came after.
    fn kept(&self, cx: f64, cz: f64) -> bool {
        self.mare(cx, cz) < 0.5 || hash2(cx / 50.0, cz / 50.0) < 0.25
    }

    fn base(&self, x: f64, z: f64, step: f32) -> (f32, f32) {
        let mare = self.mare(x, z);
        let mut h = 0.0;
        if mare < 1.0 {
            // Massifs: broad, steep-shouldered and round on top, with
            // spurs; and the hummocky ground between them.
            let range = fbm_to(x, z, 26_000.0, 7, 0.52, 11, step);
            let massif = smoothstep(-0.1, 0.55, range);
            let hummocks = fbm_to(x, z, 1800.0, 5, 0.5, 12, step);
            h += (1.0 - mare) * (3400.0 * massif * massif + 90.0 * hummocks + 250.0);
        }
        if mare > 0.0 {
            // Wrinkle ridges: narrow sinuous crests a few tens of metres
            // high on a broad low arch, kilometres apart.
            let crest = ridged(x, z, 14_000.0, 3, 21);
            let ridge = 45.0 * crest.powi(8) + 25.0 * crest.powi(3);
            let swell = 18.0 * fbm_to(x, z, 5000.0, 4, 0.5, 22, step);
            h += mare * (ridge + swell);
        }
        (h, mare)
    }
}

impl Ground for Moon {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        let (base, _) = self.base(x, z, step);
        base + self.craters.height(x, z, step, FLOODED, &|cx, cz| self.kept(cx, cz))
    }

    fn tint(&self, x: f64, z: f64, _height: f32, normal: Vec3) -> LinearRgba {
        let mare = self.mare(x, z);
        // Highland anorthosite reflects twice what mare basalt does.
        let albedo = 0.165 * (1.0 - mare) + 0.078 * mare;
        // Patches of older and younger soil, hundreds of metres across.
        let mottle = 1.0 + 0.1 * fbm(x, z, 420.0, 4, 0.5, 6);
        // Steep slopes shed their weathered top and show brighter, fresher
        // material; so does what young craters threw out.
        let slope = (1.0 - normal.y).clamp(0.0, 0.3) / 0.3;
        let rays = self.craters.ejecta(x, z, FLOODED, &|cx, cz| self.kept(cx, cz));
        let grain = hash2(x * 0.7, z * 0.7) * 0.03 - 0.015;
        let a = albedo * mottle * (1.0 + 0.3 * slope + 0.6 * rays) + grain * albedo;
        // Mature soil reddens: reflectance climbs toward long wavelengths,
        // the highlands' a little less than the mare's.
        Pattern::Regolith.on(LinearRgba::rgb(a * (1.045 + 0.015 * mare), a, a * (0.94 - 0.02 * mare)))
    }
}

impl Land for Moon {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        // Mature regolith is nearly rock-free; blocks ring the young
        // craters that reached bedrock.
        (0.02 + self.craters.blocks(x, z)).min(1.0)
    }

    fn landing_site(&self) -> (f64, f64) {
        level_site(self, 0.0, 0.0, 0.5)
    }
}

pub fn body() -> Body {
    Body {
        note: "The highlands to the north are kilometres high and further than they look: nothing in the vacuum says how far.",
        rocks: Rocks { density: 0.12, largest: 5.0, albedo: [0.1, 0.17], tint: [1.03, 1.0, 0.95], ..default() },
        // The Earth, four times the size the Moon shows us, hardly moves
        // in the lunar sky.
        companions: vec![Companion::new(Map::Earth, 0.95, 322.0, 21.0)],
        ..Body::airless("Moon", "Mare margin", Arc::new(Moon::new()))
    }
}
