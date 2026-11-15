//! The city's ground for the engine: the terrain grid, the river and lakes
//! carved into it, the land use painted on it, and what a walker may not
//! walk through.

use tranquility::{geom::Raster, prelude::*};

use crate::city::City;

/// What covers the ground, painted into rasters in drawing order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Cover {
    /// Unpaved ground in town.
    Earth,
    Water,
    Park,
    Grass,
    Square,
    SquareSetts,
    Garden,
    Yard,
    Sand,
    Cemetery,
    Rail,
    Wood,
    Cobble,
    Sett,
    Gravel,
    Track,
    Plank,
    /// Fields and meadows beyond the mapped region.
    Country,
}

impl Cover {
    fn area(kind: &str, paving: Option<&str>) -> Self {
        match kind {
            "water" => Cover::Water,
            "park" => Cover::Park,
            "grass" => Cover::Grass,
            "square" if paving == Some("setts") => Cover::SquareSetts,
            "square" => Cover::Square,
            "garden" => Cover::Garden,
            "yard" => Cover::Yard,
            "sand" => Cover::Sand,
            "cemetery" => Cover::Cemetery,
            "rail" => Cover::Rail,
            "wood" => Cover::Wood,
            _ => Cover::Earth,
        }
    }

    fn paving(paving: &str) -> Self {
        match paving {
            "cobble" => Cover::Cobble,
            "sett" => Cover::Sett,
            "gravel" => Cover::Gravel,
            "plank" => Cover::Plank,
            _ => Cover::Track,
        }
    }

    fn pattern(self) -> Pattern {
        match self {
            Cover::Cobble => Pattern::Cobbles,
            Cover::Sett | Cover::SquareSetts => Pattern::Setts,
            Cover::Gravel | Cover::Square | Cover::Yard | Cover::Track => Pattern::Gravel,
            Cover::Park | Cover::Grass | Cover::Garden | Cover::Cemetery | Cover::Country => Pattern::Grass,
            _ => Pattern::Plain,
        }
    }

    fn albedo(self) -> [f32; 3] {
        match self {
            Cover::Earth => [0.40, 0.36, 0.29],
            Cover::Water => [0.16, 0.14, 0.10],
            Cover::Park | Cover::Grass | Cover::Cemetery => [0.22, 0.30, 0.12],
            Cover::Garden => [0.25, 0.30, 0.14],
            Cover::Square => [0.52, 0.48, 0.40],
            Cover::SquareSetts | Cover::Sett => [0.46, 0.44, 0.41],
            Cover::Yard => [0.40, 0.37, 0.32],
            Cover::Sand => [0.62, 0.56, 0.44],
            Cover::Rail => [0.27, 0.24, 0.22],
            Cover::Wood => [0.12, 0.19, 0.08],
            Cover::Cobble => [0.36, 0.35, 0.33],
            Cover::Gravel => [0.54, 0.50, 0.42],
            Cover::Track => [0.45, 0.39, 0.30],
            Cover::Plank => [0.36, 0.28, 0.20],
            Cover::Country => [0.30, 0.34, 0.16],
        }
    }
}

/// Solid cells for the walker.
const OPEN: u8 = 0;
const SOLID: u8 = 1;
/// A bridge deck: open, and stood on at street level over the water.
const DECK: u8 = 2;

/// Depth of the river bed below the water's surface, m.
const BED: f32 = 2.5;

pub struct CityGround {
    grid: Option<(f32, f32, f32, usize, usize, Vec<i16>)>,
    water_level: f32,
    /// How much of a cell is water, blurred into a bank: town and region.
    wet: Raster<f32>,
    wet_region: Raster<f32>,
    cover: Raster<Cover>,
    cover_region: Raster<Cover>,
    solid: Raster<u8>,
}

impl CityGround {
    pub fn new(city: &City) -> Self {
        let [bx0, bz0, bx1, bz1] = city.bounds;
        let [rx0, rz0, rx1, rz1] = city.region_bounds;

        let mut cover_region = Raster::new(rx0, rz0, rx1, rz1, 4.0, Cover::Country);
        for a in &city.region {
            let rings = rings(&a.outer, &a.holes);
            let c = Cover::area(&a.kind, a.paving.as_deref());
            cover_region.fill_polygon(&rings, |_| c);
        }

        let mut cover = Raster::new(bx0, bz0, bx1, bz1, 1.0, Cover::Earth);
        // Start the town from the region's coarse cover, so rivers and parks
        // that cross the bounds carry on.
        for j in 0..cover.nz {
            for i in 0..cover.nx {
                let (x, z) = (bx0 + (i as f32 + 0.5), bz0 + (j as f32 + 0.5));
                if let Some(c @ (Cover::Water | Cover::Park | Cover::Wood)) = cover_region.get(x, z) {
                    cover.data[j * cover.nx + i] = c;
                }
            }
        }
        for a in &city.areas {
            let c = Cover::area(&a.kind, a.paving.as_deref());
            cover.fill_polygon(&rings(&a.outer, &a.holes), |_| c);
        }
        for r in &city.rails {
            cover.stroke(&r.pts, 3.0, |_| Cover::Rail);
        }
        for s in city.streets.iter().filter(|s| !is_bridge(&s.kind)) {
            let c = Cover::paving(&s.paving);
            cover.stroke(&s.pts, s.w, |_| c);
        }

        let water_mask = |raster: &Raster<Cover>, cell: f32, x0, z0, x1, z1| {
            let mut wet = Raster::new(x0, z0, x1, z1, cell, 0.0f32);
            for j in 0..wet.nz {
                for i in 0..wet.nx {
                    let x = x0 + (i as f32 + 0.5) * cell;
                    let z = z0 + (j as f32 + 0.5) * cell;
                    if raster.get(x, z) == Some(Cover::Water) {
                        wet.data[j * wet.nx + i] = 1.0;
                    }
                }
            }
            wet.blur(1, 3);
            wet
        };
        let wet = water_mask(&cover, 2.0, bx0, bz0, bx1, bz1);
        let wet_region = water_mask(&cover_region, 8.0, rx0, rz0, rx1, rz1);

        let mut solid = Raster::new(bx0, bz0, bx1, bz1, 0.5, OPEN);
        for j in 0..solid.nz {
            for i in 0..solid.nx {
                let x = bx0 + (i as f32 + 0.5) * 0.5;
                let z = bz0 + (j as f32 + 0.5) * 0.5;
                if cover.get(x, z) == Some(Cover::Water) {
                    solid.data[j * solid.nx + i] = SOLID;
                }
            }
        }
        for b in &city.buildings {
            // Arcades are walked under.
            if b.base.unwrap_or(0.0) < 2.2 {
                solid.fill_polygon(&rings(&b.outer, &b.holes), |_| SOLID);
            }
        }
        for w in &city.walls {
            solid.stroke(&w.pts, w.t.max(0.3), |_| SOLID);
        }
        for s in city.streets.iter().filter(|s| is_bridge(&s.kind)) {
            solid.stroke(&s.pts, s.w, |_| DECK);
        }

        let grid = city.terrain.as_ref().map(|t| (t.x0, t.z0, t.step, t.nx, t.nz, t.decode()));
        Self {
            grid,
            water_level: city.water,
            wet,
            wet_region,
            cover,
            cover_region,
            solid,
        }
    }

    /// The land's height before the water is carved in: the terrain grid,
    /// bilinear; zero (the town's level) off it.
    pub fn land(&self, x: f32, z: f32) -> f32 {
        let Some((x0, z0, step, nx, nz, h)) = &self.grid else { return 0.0 };
        let (fx, fz) = ((x - x0) / step, (z - z0) / step);
        if fx < 0.0 || fz < 0.0 || fx >= (*nx - 1) as f32 || fz >= (*nz - 1) as f32 {
            return 0.0;
        }
        let (i, j) = (fx as usize, fz as usize);
        let (u, v) = (fx - i as f32, fz - j as f32);
        let k = j * nx + i;
        let at = |k: usize| h[k] as f32;
        ((at(k) * (1.0 - u) + at(k + 1) * u) * (1.0 - v) + (at(k + nx) * (1.0 - u) + at(k + nx + 1) * u) * v) / 10.0
    }

    fn wetness(&self, x: f32, z: f32) -> f32 {
        if self.wet.index(x, z).is_some() {
            self.wet.sample(x, z)
        } else {
            self.wet_region.sample(x, z)
        }
    }

    fn cover_at(&self, x: f32, z: f32) -> Cover {
        self.cover
            .get(x, z)
            .or_else(|| self.cover_region.get(x, z))
            .unwrap_or(Cover::Country)
    }
}

impl Ground for CityGround {
    fn height(&self, x: f32, z: f32) -> f32 {
        let land = self.land(x, z);
        let bed = self.water_level - BED;
        // The blurred mask is ½ on the shoreline: the bank drops from the
        // land to the bed across it.
        let t = ((self.wetness(x, z) - 0.25) / 0.5).clamp(0.0, 1.0);
        land + (bed.min(land) - land) * t * t * (3.0 - 2.0 * t)
    }

    fn tint(&self, x: f32, z: f32, height: f32, _normal: Vec3) -> LinearRgba {
        let mut cover = self.cover_at(x, z);
        // Wet bank and river bed.
        if height < self.water_level + 0.3 {
            cover = Cover::Water;
        }
        let c = cover.albedo();
        let jitter = 1.0 + 0.16 * (hash2(x * 2.0, z * 2.0) - 0.5) + 0.1 * (hash2(x * 0.2, z * 0.2) - 0.5);
        cover
            .pattern()
            .on(LinearRgba::from(Color::srgb(c[0] * jitter, c[1] * jitter, c[2] * jitter)))
    }

    fn stand_height(&self, x: f32, z: f32) -> f32 {
        if self.solid.get(x, z) == Some(DECK) {
            self.land(x, z).max(self.height(x, z))
        } else {
            self.height(x, z)
        }
    }

    fn blocked(&self, x: f32, z: f32) -> bool {
        match self.solid.get(x, z) {
            Some(cell) => cell == SOLID,
            None => self.height(x, z) < self.water_level - 0.5,
        }
    }
}

pub fn rings<'a>(outer: &'a [f32], holes: &'a [Vec<f32>]) -> Vec<&'a [f32]> {
    std::iter::once(outer).chain(holes.iter().map(Vec::as_slice)).collect()
}

pub fn is_bridge(kind: &str) -> bool {
    kind == "bridge" || kind == "footbridge"
}
