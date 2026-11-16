//! The terrain's material: the StandardMaterial plus sub-metre detail drawn by
//! `ground.wesl`, chosen per vertex by the world's tint function; and the
//! curve of a round world, which the vertex shaders of the ground and of
//! [`PropMaterial`] apply alike.

use bevy::{
    asset::embedded_asset,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

use crate::{
    origin::Origin,
    terrain::TerrainMaterial,
    world::{TerrainSettings, WorldDef},
};

pub type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundDetail>;

/// A StandardMaterial for things that stand on the ground of a round world:
/// drawn lowered by the same curve as the ground under them, so a rock a
/// kilometre off on a small moon sits on its horizon rather than above it.
/// On a flat world it is the StandardMaterial.
pub type PropMaterial = ExtendedMaterial<StandardMaterial, Curved>;

const VERTEX: &str = "embedded://tranquility/curve_vertex.wesl";
const PREPASS: &str = "embedded://tranquility/curve_prepass.wesl";

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct GroundDetail {
    #[uniform(100)]
    pub(crate) params: GroundParams,
    /// The ground round the camera, wide and coarse, for shadows beyond
    /// the shadow maps (`horizon`).
    #[texture(101)]
    #[sampler(102)]
    pub(crate) horizon_map: Option<Handle<Image>>,
}

/// Layout shared with `ground.wesl`; `curve` comes first, where the vertex
/// shaders expect it.
#[derive(ShaderType, Reflect, Debug, Clone, Default)]
pub(crate) struct GroundParams {
    /// z: 1 / (2 · radius), 0 when flat.
    curve: Vec4,
    /// The floating origin in whole metres (x, z).
    origin: IVec4,
    /// Craterlets, grain, ripples, pebbles.
    soil_a: Vec4,
    /// Sparkle, powder, surge, surge width.
    soil_b: Vec4,
    /// Backscatter, pebble tone.
    soil_c: Vec4,
    /// The horizon map's centre relative to the origin (x, z), its edge,
    /// and the height its heights are relative to.
    pub(crate) horizon: Vec4,
    /// Distance at which the march fades in and is whole, whether there is
    /// a map at all, and the tangent of the penumbra's half-width.
    pub(crate) horizon_fade: Vec4,
}

impl MaterialExtension for GroundDetail {
    fn fragment_shader() -> ShaderRef {
        "embedded://tranquility/ground.wesl".into()
    }
    fn vertex_shader() -> ShaderRef {
        VERTEX.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        PREPASS.into()
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Curved {
    #[uniform(100)]
    curve: Vec4,
}

impl MaterialExtension for Curved {
    fn fragment_shader() -> ShaderRef {
        "embedded://tranquility/curve_fragment.wesl".into()
    }
    fn vertex_shader() -> ShaderRef {
        VERTEX.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        PREPASS.into()
    }
}

/// A [`PropMaterial`] from a StandardMaterial.
pub fn prop(base: StandardMaterial) -> PropMaterial {
    ExtendedMaterial { base, extension: Curved::default() }
}

/// How far the ground has fallen away `distance` metres from the camera on
/// a body of `radius`: what the vertex shaders subtract.
pub fn drop_at(distance: f32, radius: f32) -> f32 {
    let k = 1.0 / (2.0 * radius);
    let d2 = distance * distance;
    d2 * k * (1.0 + d2 * k * k)
}

pub(crate) fn material(terrain: &TerrainSettings) -> GroundMaterial {
    let s = terrain.soil;
    // The colour lives in the vertices; the material only sets how it
    // reflects, and the extension draws the detail.
    GroundMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: terrain.roughness,
            reflectance: terrain.reflectance,
            // Skirts are seen from either side.
            cull_mode: None,
            ..default()
        },
        extension: GroundDetail {
            params: GroundParams {
                curve: Vec4::ZERO,
                origin: IVec4::ZERO,
                soil_a: Vec4::new(s.craterlets, s.grain, s.ripples, s.pebbles),
                soil_b: Vec4::new(s.sparkle, s.powder, s.surge, s.surge_width.max(1e-3)),
                soil_c: Vec4::new(s.backscatter, s.pebble_tone, 0.0, 0.0),
                horizon: Vec4::ZERO,
                horizon_fade: Vec4::ZERO,
            },
            horizon_map: None,
        },
    }
}

pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ground.wesl");
        embedded_asset!(app, "curve_vertex.wesl");
        embedded_asset!(app, "curve_prepass.wesl");
        embedded_asset!(app, "curve_fragment.wesl");
        app.add_plugins((
            MaterialPlugin::<GroundMaterial>::default(),
            MaterialPlugin::<PropMaterial>::default(),
        ))
        .add_systems(PostUpdate, tell_shaders);
    }
}

/// Tells the shaders the world's curve and where the floating origin is.
/// Neither changes from frame to frame: a material that changes makes Bevy
/// prepare every mesh that wears it again, and there are thousands of rocks.
fn tell_shaders(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    terrain: Option<Res<TerrainMaterial>>,
    mut grounds: ResMut<Assets<GroundMaterial>>,
    mut props: ResMut<Assets<PropMaterial>>,
) {
    let k = world.terrain.radius.map_or(0.0, |r| 1.0 / (2.0 * r));
    let curve = Vec4::new(0.0, 0.0, k, 0.0);
    let at = IVec4::new(origin.0.x as i64 as i32, origin.0.y as i64 as i32, 0, 0);
    if let Some(terrain) = terrain {
        let stale = grounds.get(&terrain.0).is_some_and(|m| m.extension.params.curve != curve || m.extension.params.origin != at);
        if stale && let Some(mut material) = grounds.get_mut(&terrain.0) {
            material.extension.params.curve = curve;
            material.extension.params.origin = at;
        }
    }
    if world.is_changed() || props.is_changed() {
        let stale: Vec<_> = props.iter().filter(|(_, m)| m.extension.curve != curve).map(|(id, _)| id).collect();
        for id in stale {
            if let Some(mut material) = props.get_mut(id) {
                material.extension.curve = curve;
            }
        }
    }
}

/// What the ground shader draws below the mesh's resolution. A tint function
/// returns its colour through [`Pattern::on`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// Fine grain only.
    Plain,
    /// Rounded field stones, about 18 cm.
    Cobbles,
    /// Square setts in running bond, about 12 cm.
    Setts,
    Gravel,
    Grass,
    /// The world's loose ground, as its [`Soil`](crate::world::Soil)
    /// describes it: lunar regolith by default.
    Regolith,
}

impl Pattern {
    fn code(self) -> f32 {
        match self {
            Pattern::Plain => 1.0,
            Pattern::Cobbles => 0.1,
            Pattern::Setts => 0.2,
            Pattern::Gravel => 0.3,
            Pattern::Grass => 0.4,
            Pattern::Regolith => 0.5,
        }
    }

    /// The colour, tagged with this pattern for the ground shader.
    pub fn on(self, color: LinearRgba) -> LinearRgba {
        color.with_alpha(self.code())
    }

    pub fn from_code(alpha: f32) -> Self {
        [Pattern::Cobbles, Pattern::Setts, Pattern::Gravel, Pattern::Grass, Pattern::Regolith]
            .into_iter()
            .find(|p| (p.code() - alpha).abs() < 0.05)
            .unwrap_or(Pattern::Plain)
    }

    /// One weight per pattern, as the terrain mesh carries them (UV 0, UV 1,
    /// colour alpha): weights interpolate cleanly across a border between
    /// two patterns, where a single code would sweep through all the others.
    pub(crate) fn weights(self) -> ([f32; 2], [f32; 2], f32) {
        match self {
            Pattern::Plain => ([0.0, 0.0], [0.0, 0.0], 0.0),
            Pattern::Cobbles => ([1.0, 0.0], [0.0, 0.0], 0.0),
            Pattern::Setts => ([0.0, 1.0], [0.0, 0.0], 0.0),
            Pattern::Gravel => ([0.0, 0.0], [1.0, 0.0], 0.0),
            Pattern::Grass => ([0.0, 0.0], [0.0, 1.0], 0.0),
            Pattern::Regolith => ([0.0, 0.0], [0.0, 0.0], 1.0),
        }
    }
}
