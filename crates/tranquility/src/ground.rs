//! The terrain's material: the StandardMaterial plus sub-metre detail drawn by
//! `ground.wesl`, chosen per vertex by the world's tint function.

use bevy::{
    asset::embedded_asset,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

pub type GroundMaterial = ExtendedMaterial<StandardMaterial, GroundDetail>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct GroundDetail {}

impl MaterialExtension for GroundDetail {
    fn fragment_shader() -> ShaderRef {
        "embedded://tranquility/ground.wesl".into()
    }
}

pub struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ground.wesl");
        app.add_plugins(MaterialPlugin::<GroundMaterial>::default());
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
    /// Lunar soil: pits and clods that catch a low sun.
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
