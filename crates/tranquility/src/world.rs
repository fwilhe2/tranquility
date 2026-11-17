//! What an app hands the engine: the ground as a few functions, and the
//! numbers for the sky, the water and the walker.
//!
//! The engine never asks which world it is in. Anything a world does
//! differently is either a number here or a method on [`Ground`].

use std::sync::Arc;

use bevy::{light::atmosphere::ScatteringMedium, prelude::*};

/// The ground, as pure functions of world position (metres; x east, z south,
/// y up). They are called from the chunk meshing jobs as well as from the
/// main thread, so they must agree with themselves everywhere: this is the
/// single source of truth for where the ground is.
///
/// Positions are `f64`: a world may go on for ever, and a walker a thousand
/// kilometres out still needs the ground to the millimetre. Everything drawn
/// is placed relative to a floating [`Origin`](crate::origin::Origin).
pub trait Ground: Send + Sync + 'static {
    /// Height of the ground: where feet, wheels and anything else standing
    /// on it find it.
    fn height(&self, x: f64, z: f64) -> f32;

    /// The same ground as a mesh with vertices `step` metres apart should
    /// draw it. A world with relief far finer than a distant chunk's grid
    /// may leave out what that grid cannot show (sampling a field of small
    /// craters every hundred metres draws noise, not craters). It must
    /// return `height` itself for any step a walker sees close up.
    fn height_at(&self, x: f64, z: f64, _step: f32) -> f32 {
        self.height(x, z)
    }

    /// Albedo of the ground at a point, given its height and normal. The
    /// alpha picks the detail the ground shader draws: return colours through
    /// [`Pattern::on`](crate::ground::Pattern::on); plain alpha 1 is
    /// [`Pattern::Plain`](crate::ground::Pattern::Plain).
    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba;

    /// The tint for a vertex of a mesh `step` metres apart: a world may
    /// leave out markings a distant grid cannot show, which is most of the
    /// cost of colouring it.
    fn tint_at(&self, x: f64, z: f64, height: f32, normal: Vec3, _step: f32) -> LinearRgba {
        self.tint(x, z, height, normal)
    }

    /// Where feet come to rest. Defaults to the drawn ground; override it for
    /// things that can be stood on but are not terrain (bridge decks, rocks).
    fn stand_height(&self, x: f64, z: f64) -> f32 {
        self.height(x, z)
    }

    /// Whether a walker may not stand at this point (walls, deep water).
    fn blocked(&self, _x: f64, _z: f64) -> bool {
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

/// How the ground is streamed and drawn.
///
/// The ground is cut into square chunks on nested levels: level 0 chunks are
/// `chunk_size` across, each further level twice the one before, and every
/// chunk has the same number of quads, so the grid step doubles with each
/// level. Fine levels cover the ground near the camera and coarser ones the
/// ground beyond, out to a few chunks of the coarsest level.
#[derive(Clone)]
pub struct TerrainSettings {
    /// Edge of a level 0 chunk, m.
    pub chunk_size: f32,
    /// Quads per chunk edge, on every level.
    pub resolution: u32,
    /// Number of levels.
    pub levels: u32,
    /// How many chunks of the next coarser level a level reaches out from
    /// the one the camera is in: 1 keeps detail to between two and four
    /// chunks away, 2 to between four and six.
    pub ring: i32,
    pub roughness: f32,
    pub reflectance: f32,
    /// Whether the ground draws into the sun's shadow maps. Hills and crater
    /// rims need it to shadow themselves; on flat ground it only costs.
    pub casts_shadows: bool,
    /// Radius of the body, m: the ground falls away from the camera as on a
    /// sphere of that size, so the horizon is where it should be and far
    /// hills sink behind it. `None` for a flat world.
    pub radius: Option<f32>,
    /// What [`Pattern::Regolith`](crate::ground::Pattern::Regolith) draws.
    pub soil: Soil,
    /// Shadows of hills and crater walls beyond the reach of the sun's
    /// shadow maps, marched through a wide height map of the ground. For
    /// worlds whose relief is the scenery under a low sun.
    pub far_shadows: bool,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            chunk_size: 128.0,
            resolution: 128,
            levels: 6,
            ring: 1,
            roughness: 0.92,
            reflectance: 0.3,
            casts_shadows: true,
            radius: None,
            soil: Soil::default(),
            far_shadows: false,
        }
    }
}

impl TerrainSettings {
    /// Edge of a chunk on `level`, m.
    pub fn size(&self, level: u32) -> f64 {
        self.chunk_size as f64 * (1u64 << level) as f64
    }

    /// Grid step on `level`, m.
    pub fn step(&self, level: u32) -> f32 {
        (self.size(level) / self.resolution as f64) as f32
    }

    /// The streamed ground reaches at least this far from the camera, m.
    pub fn reach(&self) -> f32 {
        (self.size(self.levels.max(1) - 1) * (2 * self.ring + 1) as f64) as f32
    }
}

/// The loose ground of a world, as the ground shader draws it below the
/// mesh's resolution and as it answers the light.
#[derive(Clone, Copy, Debug)]
pub struct Soil {
    /// Strength of the small impact craters (a few metres down to a hand's
    /// breadth): 1 for an airless body gardened by impacts, 0 where wind,
    /// rain or resurfacing fills them in.
    pub craterlets: f32,
    /// Clods and grain: the roughness of the surface at the scale of a boot.
    pub grain: f32,
    /// Wind ripples a few centimetres high, decimetres apart.
    pub ripples: f32,
    /// Pebbles and small stones lying on the surface, 0–1.
    pub pebbles: f32,
    /// How bright those stones are beside the soil they lie on: 1 the
    /// same, less for dark rock on bright dust.
    pub pebble_tone: f32,
    /// Glints from crystal faces: frost and clean ice.
    pub sparkle: f32,
    /// 0 scatters like a matte surface (Lambert), 1 like a porous powder
    /// (Lommel–Seeliger): no darkening toward the limb, as on the Moon.
    pub powder: f32,
    /// Strength of the opposition surge, the brightening round the point
    /// opposite the sun where grains hide their own shadows.
    pub surge: f32,
    /// Angular half-width of the surge, radians.
    pub surge_width: f32,
    /// How much darker the soil looks toward the sun than away from it
    /// (shadows between grains face the viewer): 0 none, 1 lunar.
    pub backscatter: f32,
}

impl Default for Soil {
    fn default() -> Self {
        // Lunar regolith.
        Self {
            craterlets: 1.0,
            grain: 1.0,
            ripples: 0.0,
            pebbles: 0.3,
            pebble_tone: 1.0,
            sparkle: 0.0,
            powder: 1.0,
            surge: 1.0,
            surge_width: 0.12,
            backscatter: 1.0,
        }
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
    /// Colour of the fill light.
    pub ambient_color: Color,
    /// Camera exposure while the sun is well up, EV100.
    pub exposure: f32,
    /// Sunlight arriving at the top of any air, lux: 133,000 at one
    /// astronomical unit, falling with the square of the distance.
    pub sun_illuminance: f32,
    /// Angular radius of the sun's disc, degrees (0.267 from Earth).
    pub sun_radius: f32,
    /// Light from the whole sky on everything, rendered from the
    /// atmosphere. Set for the world the app starts in and never changed
    /// after: changing the sky under it (another world's air, `B`) hangs
    /// the GPU. An app that changes worlds goes without, and lights the
    /// shade with `ambient`.
    pub sky_light: bool,
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
            ambient_color: Color::WHITE,
            exposure: 13.0,
            sun_illuminance: 133_000.0,
            sun_radius: 0.267,
            sky_light: true,
        }
    }
}

#[derive(Clone)]
pub enum Air {
    /// Bevy's physically based Earth atmosphere.
    Earth,
    /// A black sky.
    None,
    /// Any other air: what it is made of and how deep it is.
    Medium(Arc<AirDef>),
}

impl Air {
    pub fn is_none(&self) -> bool {
        matches!(self, Air::None)
    }
}

/// An atmosphere other than Earth's, for Bevy's physically based sky.
pub struct AirDef {
    /// Radius of the body, m.
    pub radius: f32,
    /// Height at which the air is taken to end, m.
    pub height: f32,
    /// Mean albedo of the ground, which lights the air from below.
    pub ground_albedo: Vec3,
    /// The gases, hazes and dust: what each absorbs and scatters, how it
    /// thins with height and where it sends the light.
    pub medium: ScatteringMedium,
    /// March every view ray through the air instead of looking the answer
    /// up in tables: slower, but under air many optical depths thick the
    /// tables run out of precision and draw lines across the ground.
    pub thick: bool,
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
    /// The hardest the ground lets the walker speed up, slow down or turn,
    /// m/s²: grip times gravity. Infinite sets the pace at once, as a game
    /// does; a real figure makes low gravity feel like low gravity, and the
    /// walker slides on slopes too steep to hold.
    pub grip: f32,
    /// Whether the walker can change course in mid-air. A body in flight is
    /// a projectile, so a physical world says no.
    pub air_control: bool,
    /// A drop of at most this much is walked down rather than fallen off,
    /// m: steps and kerbs. With 0 the walker leaves the ground whenever it
    /// falls away faster than gravity brings them down, which at a run in
    /// low gravity is every crest.
    pub step_down: f32,
    /// Drag of the air on a body in flight, 1/m: deceleration per (m/s)².
    pub drag: f32,
    /// Acceleration of suit jets, m/s², for bodies too small to walk on;
    /// 0 for none. In flight the movement keys fire them, Space and C up
    /// and down.
    pub jets: f32,
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
            grip: f32::INFINITY,
            air_control: true,
            step_down: 0.6,
            drag: 0.0,
            jets: 0.0,
            eye_smoothing: 0.0,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Spawn {
    pub x: f64,
    pub z: f64,
    /// Radians, 0 looking north (−z), increasing toward the west.
    pub yaw: f32,
    pub pitch: f32,
}
