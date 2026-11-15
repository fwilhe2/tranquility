//! A small first-person world engine on Bevy, shaped after lunar-walk and
//! historical-walk: a world is a height function, a tint function and some
//! numbers ([`WorldDef`]); the engine streams the ground around the camera,
//! lights it with a sun and a physically based sky, and walks you over it.
//! Anything else in a world (buildings, rocks, trees) is the app's own Bevy
//! systems, with [`geom`] for building meshes and [`terrain::ChunkMeshed`]
//! for decorating the ground as it streams in.
//!
//! ```no_run
//! use tranquility::prelude::*;
//!
//! struct Flat;
//! impl Ground for Flat {
//!     fn height(&self, _: f32, _: f32) -> f32 { 0.0 }
//!     fn tint(&self, _: f32, _: f32, _: f32, _: Vec3) -> LinearRgba { LinearRgba::gray(0.3) }
//! }
//!
//! fn main() {
//!     tranquility::app(WorldDef {
//!         name: "flat".into(),
//!         ground: ground(Flat),
//!         terrain: default(),
//!         sky: default(),
//!         water: None,
//!         walker: default(),
//!         spawn: default(),
//!     })
//!     .run();
//! }
//! ```

pub mod facade;
pub mod geom;
pub mod ground;
pub mod hud;
pub mod player;
pub mod quality;
pub mod sky;
pub mod terrain;
pub mod tools;
pub mod water;
pub mod world;

use bevy::{prelude::*, window::PresentMode};

pub use world::WorldDef;

pub mod prelude {
    pub use crate::{
        ground::Pattern,
        hud::Hud,
        player::Player,
        sky::Sun,
        terrain::{ChunkMeshed, ground, hash, hash2, unit},
        world::{
            Air, Ground, SkySettings, Spawn, SunPath, TerrainSettings, WalkerSettings,
            WaterSettings, WorldDef,
        },
    };
    pub use bevy::prelude::*;
}

/// Everything but the window: add this to an app that sets up its own.
pub struct EnginePlugin(pub WorldDef);

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone()).add_plugins((
            terrain::TerrainPlugin,
            sky::SkyPlugin,
            water::WaterPlugin,
            player::PlayerPlugin,
            hud::HudPlugin,
            tools::ToolsPlugin,
            facade::FacadePlugin,
            ground::GroundPlugin,
            quality::QualityPlugin,
        ));
    }
}

/// An app with Bevy's default plugins, a window (or, in the browser, the
/// page's `#bevy` canvas) and the engine, ready for the world's own systems.
pub fn app(world: WorldDef) -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: world.name.clone(),
            present_mode: PresentMode::AutoVsync,
            canvas: Some("#bevy".into()),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    }))
    .add_plugins(EnginePlugin(world));
    app
}
