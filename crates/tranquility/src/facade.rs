//! A material for buildings: the StandardMaterial with windows, storey bands
//! and a plinth drawn on the walls by `facade.wesl`, from the parameters
//! [`geom::Massing`](crate::geom::Massing) writes into the mesh.

use bevy::{
    asset::embedded_asset,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

pub type FacadeMaterial = ExtendedMaterial<StandardMaterial, Facade>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct Facade {}

impl MaterialExtension for Facade {
    fn fragment_shader() -> ShaderRef {
        "embedded://tranquility/facade.wesl".into()
    }
}

pub struct FacadePlugin;

impl Plugin for FacadePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "facade.wesl");
        app.add_plugins(MaterialPlugin::<FacadeMaterial>::default());
    }
}

/// A facade material: the wall colour comes from the vertices.
pub fn material(roughness: f32) -> FacadeMaterial {
    ExtendedMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: roughness,
            ..default()
        },
        extension: Facade {},
    }
}

/// How the walls of one building are drawn, packed into its vertex colours'
/// alpha for the shader.
#[derive(Clone, Copy, Debug)]
pub enum Windows {
    /// No openings: sheds, gates, colonnades, roofs.
    Blank,
    /// Rows of windows, one per storey of this height (m).
    Storeys { storey: f32, shop: bool },
}

impl Windows {
    pub fn code(self) -> f32 {
        match self {
            Windows::Blank => 1.0,
            Windows::Storeys { storey, shop } => (storey / 20.0).clamp(0.05, 0.45) + if shop { 0.5 } else { 0.0 },
        }
    }
}
