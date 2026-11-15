//! Hooks for looking at a world without sitting at the window.
//!
//! - `TRANQUILITY_POSE="x,y,z,yaw,pitch"` starts there, flying.
//! - `TRANQUILITY_HOUR=17.5` sets the clock (daily sun),
//!   `TRANQUILITY_SUN=12` the elevation (fixed sun).
//! - `TRANQUILITY_SCREENSHOT=out.png` saves one frame once the terrain has
//!   streamed in, then quits.

use bevy::prelude::*;

pub(crate) fn env_f32(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.trim().parse().ok()
}

pub(crate) fn env_floats<const N: usize>(name: &str) -> Option<[f32; N]> {
    let v = std::env::var(name).ok()?;
    let parsed: Option<Vec<f32>> = v.split(',').map(|n| n.trim().parse().ok()).collect();
    parsed?.try_into().ok()
}

pub struct ToolsPlugin;

impl Plugin for ToolsPlugin {
    #[allow(unused_variables)]
    fn build(&self, app: &mut App) {
        #[cfg(not(target_arch = "wasm32"))]
        screenshot::add(app);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod screenshot {
    use bevy::{
        prelude::*,
        render::view::screenshot::{Screenshot, save_to_disk},
    };

    use crate::terrain::Chunks;

    #[derive(Resource)]
    struct Shot {
        path: String,
        taken: bool,
    }

    pub fn add(app: &mut App) {
        if let Ok(path) = std::env::var("TRANQUILITY_SCREENSHOT") {
            app.insert_resource(Shot { path, taken: false })
                .add_systems(Update, take);
        }
    }

    fn take(
        mut commands: Commands,
        time: Res<Time>,
        terrain: Res<Chunks>,
        mut shot: ResMut<Shot>,
        mut settled_at: Local<Option<f32>>,
        mut exit: MessageWriter<AppExit>,
    ) {
        let t = time.elapsed_secs();
        // Wait for the terrain to finish streaming in, then give TAA and the
        // atmosphere LUTs a moment to converge.
        if t < 2.0 || !terrain.is_settled() {
            return;
        }
        let settled = *settled_at.get_or_insert(t);
        if !shot.taken && t > settled + 2.0 {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(shot.path.clone()));
            shot.taken = true;
        }
        if t > settled + 4.0 {
            exit.write(AppExit::Success);
        }
    }
}
