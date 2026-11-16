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
    params: GroundParams,
}

/// Layout shared with `ground.wesl`; `curve` comes first, where the vertex
/// shaders expect it.
#[derive(ShaderType, Reflect, Debug, Clone, Default)]
struct GroundParams {
    /// Camera x, z in render space; 1 / (2 · radius), 0 when flat.
    curve: Vec4,
    /// The floating origin in whole metres (x, z).
    origin: IVec4,
    /// Craterlets, grain, ripples, pebbles.
    soil_a: Vec4,
    /// Sparkle, powder, surge, surge width.
    soil_b: Vec4,
    /// Backscatter.
    soil_c: Vec4,
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
                soil_c: Vec4::new(s.backscatter, 0.0, 0.0, 0.0),
            },
        },
    }
}

pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ground.wesl");
        embedded_asset!(app, "curve_vertex.wesl");
        embedded_asset!(app, "curve_prepass.wesl");
        app.add_plugins((
            MaterialPlugin::<GroundMaterial>::default(),
            MaterialPlugin::<PropMaterial>::default(),
        ))
        // After everything that moves the camera this frame.
        .add_systems(PostUpdate, follow_camera.before(TransformSystems::Propagate));
    }
}

/// Tells the shaders where the camera and the floating origin are.
fn follow_camera(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    camera: Single<&Transform, With<Camera3d>>,
    terrain: Option<Res<TerrainMaterial>>,
    mut grounds: ResMut<Assets<GroundMaterial>>,
    mut props: ResMut<Assets<PropMaterial>>,
    mut last: Local<Vec4>,
) {
    let k = world.terrain.radius.map_or(0.0, |r| 1.0 / (2.0 * r));
    let curve = Vec4::new(camera.translation.x, camera.translation.z, k, 0.0);
    if let Some(mut material) = terrain.and_then(|t| grounds.get_mut(&t.0)) {
        material.extension.params.curve = curve;
        material.extension.params.origin = IVec4::new(origin.0.x as i64 as i32, origin.0.y as i64 as i32, 0, 0);
    }
    // On a flat world nothing changes, and the props are left alone.
    if curve != *last && (k > 0.0 || last.z > 0.0) {
        for (_, material) in props.iter_mut() {
            material.extension.curve = curve;
        }
    }
    *last = curve;
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
