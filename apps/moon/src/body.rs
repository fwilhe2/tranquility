//! A body, as data: what it is (gravity, size, distance from the sun, what
//! its ground is made of), from which everything the engine needs follows —
//! how bright the sun is and how large, how a suited walker moves, how far
//! the horizon is.

use std::sync::Arc;

use tranquility::prelude::*;

use crate::{heavens::Companion, rocks::Strewn, suit};

/// A body's ground, and where its loose rocks lie.
pub trait Land: Ground {
    /// 0–1: how strewn with rocks the ground is at a point.
    fn rockiness(&self, _x: f64, _z: f64) -> f32 {
        0.03
    }

    /// Where the walk starts: level, open, sunlit ground.
    fn landing_site(&self) -> (f64, f64) {
        level_site(self, 0.0, 0.0, 0.6)
    }
}

/// The first level, open spot on a spiral out from a point.
pub fn level_site<L: Land + ?Sized>(land: &L, x0: f64, z0: f64, rough: f32) -> (f64, f64) {
    for k in 0..8000 {
        let (angle, radius) = (k as f64 * 2.4, 15.0 * (k as f64).sqrt());
        let (x, z) = (x0 + radius * angle.cos(), z0 + radius * angle.sin());
        let h = land.height(x, z);
        let worst = [(25.0, 0.0), (-25.0, 0.0), (0.0, 25.0), (0.0, -25.0), (60.0, 0.0), (0.0, 60.0), (8.0, 8.0), (-8.0, 8.0)]
            .iter()
            .map(|(dx, dz)| (land.height(x + dx, z + dz) - h).abs() / (dx * dx + dz * dz).sqrt() as f32 * 25.0)
            .fold(0.0, f32::max);
        if worst < rough && !land.blocked(x, z) {
            return (x, z);
        }
    }
    (x0, z0)
}

/// How the loose rocks of a body look and how many there are.
#[derive(Clone)]
pub struct Rocks {
    /// Rocks over 10 cm per square metre where `Land::rockiness` is 1.
    pub density: f32,
    /// Largest block, m.
    pub largest: f32,
    /// Albedo, darkest to brightest, and tint.
    pub albedo: [f32; 2],
    pub tint: [f32; 3],
    pub roughness: f32,
    /// Rounded by rolling in a stream bed (Titan), not broken.
    pub rounded: bool,
}

impl Default for Rocks {
    fn default() -> Self {
        Self { density: 0.5, largest: 4.0, albedo: [0.13, 0.22], tint: [1.03, 1.0, 0.96], roughness: 0.95, rounded: false }
    }
}

#[derive(Clone)]
pub struct Body {
    pub name: &'static str,
    /// Where on it the walk starts.
    pub site: &'static str,
    /// A line for the screen: what to look for.
    pub note: &'static str,
    /// Surface gravity, m/s².
    pub gravity: f32,
    /// Mean radius, m.
    pub radius: f32,
    /// Distance from the sun, astronomical units.
    pub au: f32,
    /// Mean albedo of the ground, for exposure and for the light the ground
    /// throws into its own shadows.
    pub albedo: f32,
    /// Friction of a boot or a tyre on the ground.
    pub grip: f32,
    pub land: Arc<dyn Land>,
    pub soil: Soil,
    pub roughness: f32,
    pub reflectance: f32,
    pub air: Air,
    /// Quadratic drag of the air on a suited walker, 1/m.
    pub drag: f32,
    /// Density of the air at the ground, kg/m³, and a usual wind, m/s:
    /// what carries sound from outside to a helmet, and what blows on it.
    pub air_density: f32,
    pub wind: f32,
    /// Where the sun stands: degrees from north through east, and up.
    pub sun: (f32, f32),
    /// Stars' brightness; 0 under a sky that hides them.
    pub stars: f32,
    /// Fill light as a share of what the sunlit ground throws back: more
    /// in a crater than on a plain, more under a planet than a black sky.
    pub fill: f32,
    pub fill_color: Color,
    /// Stops to add to the exposure the sunlight and albedo give.
    pub exposure_bias: f32,
    pub sea: Option<WaterSettings>,
    pub rocks: Rocks,
    pub companions: Vec<Companion>,
    /// Whether the sun's disc shows (not through Titan's haze).
    pub sun_disc: bool,
    /// How far the ground is streamed: number of levels.
    pub levels: u32,
    /// Where to look on arrival: yaw and pitch, radians.
    pub look: (f32, f32),
    pub rover: bool,
}

/// Sunlight at one astronomical unit, lux.
const SUN_LUX: f32 = 133_000.0;

impl Body {
    /// An airless body of grey regolith under a black sky, the Moon's
    /// numbers for everything not given: the starting point for a row.
    pub fn airless(name: &'static str, site: &'static str, land: Arc<dyn Land>) -> Self {
        Self {
            name,
            site,
            note: "",
            gravity: 1.62,
            radius: 1_737_400.0,
            au: 1.0,
            albedo: 0.11,
            grip: 0.65,
            land,
            soil: Soil::default(),
            roughness: 0.98,
            reflectance: 0.2,
            air: Air::None,
            drag: 0.0,
            air_density: 0.0,
            wind: 0.0,
            sun: (100.0, 14.0),
            stars: 900.0,
            fill: 1.0,
            fill_color: Color::WHITE,
            exposure_bias: 0.0,
            sea: None,
            rocks: Rocks::default(),
            companions: Vec::new(),
            sun_disc: true,
            levels: 8,
            look: (0.5, -0.1),
            rover: true,
        }
    }

    /// Angular radius of the sun's disc from here, degrees.
    pub fn sun_radius(&self) -> f32 {
        0.267 / self.au
    }

    /// Whether the air's own sun disc is the one to show: there is air,
    /// and the disc is more than a pixel.
    pub fn sky_draws_sun(&self) -> bool {
        !self.air.is_none() && self.sun_disc && self.sun_radius() >= crate::heavens::SMALLEST
    }

    /// Sunlight at the top of the air, lux.
    pub fn sunlight(&self) -> f32 {
        SUN_LUX / (self.au * self.au)
    }

    pub fn world(&self) -> WorldDef {
        let (x, z) = self.land.landing_site();
        let sunlight = self.sunlight();
        // A camera set for sunlit ground of this albedo: the Moon's grey
        // soil at one unit from the sun reads right at EV 12.6.
        let exposure = 12.6 + (sunlight / SUN_LUX * self.albedo / 0.11).log2() + self.exposure_bias;
        // Lambertian ground of that albedo, seen from its own shadow.
        let ambient = self.fill * sunlight * self.albedo * (3000.0 / (SUN_LUX * 0.11));
        // The walker meets the land with its rocks on it.
        let ground: Arc<dyn Ground> = Arc::new(Strewn::new(self.land.clone(), self.rocks.clone(), self.sea.as_ref().map(|sea| sea.level)));
        WorldDef {
            name: self.name.into(),
            ground,
            terrain: TerrainSettings {
                // Two thirds of a metre between vertices underfoot.
                chunk_size: 64.0,
                resolution: 96,
                levels: self.levels,
                ring: 1,
                roughness: self.roughness,
                reflectance: self.reflectance,
                casts_shadows: true,
                radius: Some(self.radius),
                soil: self.soil,
            },
            sky: SkySettings {
                air: self.air.clone(),
                sun: SunPath::Fixed { azimuth: self.sun.0, elevation: self.sun.1 },
                stars: self.stars,
                ambient,
                ambient_color: self.fill_color,
                exposure,
                sun_illuminance: sunlight,
                sun_radius: if self.sky_draws_sun() { self.sun_radius() } else { 0.0 },
                ..default()
            },
            water: self.sea.clone(),
            walker: suit::walker(self.gravity, self.grip, self.drag),
            spawn: Spawn { x, z, yaw: self.look.0, pitch: self.look.1 },
        }
    }
}
