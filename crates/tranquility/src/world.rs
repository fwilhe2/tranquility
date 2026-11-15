//! What an app hands the engine: the ground as a few functions, and the
//! numbers for the sky, the water and the walker.
//!
//! The engine never asks which world it is in. Anything a world does
//! differently is either a number here or a method on [`Ground`].

use std::sync::Arc;

use bevy::prelude::*;

/// The ground, as pure functions of world position (metres; x east, z south,
/// y up). They are called from the chunk meshing jobs as well as from the
/// main thread, so they must agree with themselves everywhere: this is the
/// single source of truth for where the ground is.
pub trait Ground: Send + Sync + 'static {
    /// Height of the drawn ground.
    fn height(&self, x: f32, z: f32) -> f32;

    /// Albedo of the ground at a point, given its height and normal. The
    /// alpha picks the detail the ground shader draws: return colours through
    /// [`Pattern::on`](crate::ground::Pattern::on); plain alpha 1 is
    /// [`Pattern::Plain`](crate::ground::Pattern::Plain).
    fn tint(&self, x: f32, z: f32, height: f32, normal: Vec3) -> LinearRgba;

    /// Where feet come to rest. Defaults to the drawn ground; override it for
    /// things that can be stood on but are not terrain (bridge decks, rocks).
    fn stand_height(&self, x: f32, z: f32) -> f32 {
        self.height(x, z)
    }

    /// Whether a walker may not stand at this point (walls, deep water).
    fn blocked(&self, _x: f32, _z: f32) -> bool {
        false
    }
}

#[derive(Resource, Clone)]
pub struct WorldDef {
    /// Shown in the window title and the HUD.
    pub name: String,
    pub ground: Arc<dyn Ground>,
    pub terrain: TerrainSettings,
    pub sky: SkySettings,
    pub water: Option<WaterSettings>,
    pub walker: WalkerSettings,
    pub spawn: Spawn,
}

#[derive(Clone)]
pub struct TerrainSettings {
    /// Edge of one chunk, m.
    pub chunk_size: f32,
    /// Chunks are kept this many chunks out from the camera's chunk.
    pub view_radius: i32,
    /// `(up to ring, quads per chunk edge)`, nearest first: a chunk `ring`
    /// chunks from the camera's takes the first entry whose ring reaches it.
    pub lods: Vec<(i32, u32)>,
    pub roughness: f32,
    pub reflectance: f32,
    /// Whether the ground draws into the sun's shadow maps. Hills and crater
    /// rims need it to shadow themselves; on flat ground it only costs (the
    /// finest chunks are most of the triangles in every cascade).
    pub casts_shadows: bool,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            chunk_size: 512.0,
            view_radius: 12,
            lods: vec![(1, 256), (3, 128), (6, 64), (i32::MAX, 32)],
            roughness: 0.92,
            reflectance: 0.3,
            casts_shadows: true,
        }
    }
}

impl TerrainSettings {
    pub fn resolution(&self, ring: i32) -> u32 {
        self.lods
            .iter()
            .find(|(reach, _)| ring <= *reach)
            .or(self.lods.last())
            .map_or(32, |(_, n)| *n)
    }
}

#[derive(Clone)]
pub struct SkySettings {
    pub air: Air,
    pub sun: SunPath,
    /// Local solar time at start, hours.
    pub start_hour: f32,
    /// Brightness of the star field; 0 for none. Stars only show where there
    /// is no air: an atmosphere's sky would cover them anyway.
    pub stars: f32,
    /// Flat fill light, in the units of `GlobalAmbientLight::brightness`. Air
    /// lights the shade itself; an airless world needs a little (earthshine,
    /// light bounced off the ground) or its shadows are black holes.
    pub ambient: f32,
    /// Camera exposure while the sun is well up, EV100.
    pub exposure: f32,
}

impl Default for SkySettings {
    fn default() -> Self {
        Self {
            air: Air::Earth,
            sun: SunPath::Daily {
                latitude: 45.0,
                declination: 10.0,
            },
            start_hour: 9.5,
            stars: 0.0,
            ambient: 0.0,
            exposure: 13.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Air {
    /// Bevy's physically based Earth atmosphere.
    Earth,
    /// A black sky.
    None,
}

#[derive(Clone, Copy)]
pub enum SunPath {
    /// The sun's daily arc at a latitude, for a declination (−23.4° at the
    /// December solstice, +23.4° in June); `[` `]` turn the clock.
    Daily { latitude: f32, declination: f32 },
    /// A sun that stands still, as on a slowly turning moon; `[` `]` raise
    /// and lower it. Degrees; azimuth from north through east.
    Fixed { azimuth: f32, elevation: f32 },
}

#[derive(Clone)]
pub struct WaterSettings {
    pub level: f32,
    pub color: Color,
    pub roughness: f32,
}

#[derive(Clone)]
pub struct WalkerSettings {
    pub eye_height: f32,
    /// m/s².
    pub gravity: f32,
    /// Upward speed of a jump, m/s.
    pub jump: f32,
    pub walk: f32,
    pub run: f32,
    pub fly: f32,
    pub fly_fast: f32,
    /// How close the walker may come to anything [`Ground::blocked`], m.
    pub radius: f32,
    /// Whether walking on the water surface is allowed (else you wade on the
    /// bed, if [`Ground::blocked`] lets you in at all).
    pub walk_on_water: bool,
    /// Time constant, s, with which the eye follows the feet's height. 0
    /// copies the ground exactly; a few tenths of a second glide over small
    /// bumps, which is kinder to the stomach on rough ground.
    pub eye_smoothing: f32,
}

impl Default for WalkerSettings {
    fn default() -> Self {
        Self {
            eye_height: 1.7,
            gravity: 9.81,
            jump: 3.5,
            walk: 1.6,
            run: 6.0,
            fly: 60.0,
            fly_fast: 600.0,
            radius: 0.3,
            walk_on_water: true,
            eye_smoothing: 0.0,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Spawn {
    pub x: f32,
    pub z: f32,
    /// Radians, 0 looking north (−z), increasing toward the west.
    pub yaw: f32,
    pub pitch: f32,
}
