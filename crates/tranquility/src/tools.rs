//! Hooks for looking at a world without sitting at the window.
//!
//! - `TRANQUILITY_POSE="x,y,z,yaw,pitch"` starts there, flying.
//! - `TRANQUILITY_LOOK="yaw,pitch"` turns the head, on foot.
//! - `TRANQUILITY_HOUR=17.5` sets the clock (daily sun),
//!   `TRANQUILITY_SUN=12` the elevation (fixed sun),
//!   `TRANQUILITY_SUN=12,230` elevation and azimuth.
//! - `TRANQUILITY_WALK=forward|back` walks by itself.
//! - `TRANQUILITY_SCREENSHOT=out.png` saves one frame once the terrain has
//!   streamed in, then quits; `TRANQUILITY_SHOT_AFTER=seconds` waits longer.
//! - `TRANQUILITY_BENCH=10` measures frame times for that many seconds once
//!   the terrain has streamed in, prints them, then quits.

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
        if let Some(seconds) = env_f32("TRANQUILITY_BENCH") {
            app.insert_resource(Bench { seconds, frames: Vec::new(), since: None, began: None, busy: Vec::new() })
                .add_systems(First, |mut bench: ResMut<Bench>| bench.began = Some(std::time::Instant::now()))
                .add_systems(Last, |mut bench: ResMut<Bench>| {
                    if let Some(began) = bench.began {
                        let ms = began.elapsed().as_secs_f32() * 1000.0;
                        bench.busy.push(ms);
                    }
                })
                .add_systems(Update, bench);
        }
    }
}

#[derive(Resource)]
struct Bench {
    seconds: f32,
    frames: Vec<f32>,
    since: Option<f32>,
    /// When this frame's update began, and how long each update took, ms:
    /// the app's own share of the frame, before the renderer's.
    began: Option<std::time::Instant>,
    busy: Vec<f32>,
}

fn bench(time: Res<Time<Real>>, terrain: Res<crate::terrain::Chunks>, mut bench: ResMut<Bench>, mut exit: MessageWriter<AppExit>) {
    let t = time.elapsed_secs();
    if t < 3.0 || (bench.since.is_none() && !terrain.is_settled()) {
        return;
    }
    if bench.since.is_none() {
        bench.busy.clear();
        println!("bench: ground settled after {t:.1} s");
    }
    let since = *bench.since.get_or_insert(t);
    bench.frames.push(time.delta_secs() * 1000.0);
    if t - since >= bench.seconds {
        let mut ms = std::mem::take(&mut bench.frames);
        ms.sort_by(f32::total_cmp);
        let mean = ms.iter().sum::<f32>() / ms.len() as f32;
        let at = |q: f32| ms[((ms.len() - 1) as f32 * q) as usize];
        let mut busy = std::mem::take(&mut bench.busy);
        busy.sort_by(f32::total_cmp);
        let update = busy.get(busy.len() / 2).copied().unwrap_or(0.0);
        println!(
            "bench: {} frames, mean {:.1} ms ({:.1} fps), median {:.1} ms, 95 % {:.1} ms, worst {:.1} ms; update median {:.1} ms, worst {:.1} ms",
            ms.len(),
            mean,
            1000.0 / mean,
            at(0.5),
            at(0.95),
            at(1.0),
            update,
            busy.last().copied().unwrap_or(0.0),
        );
        exit.write(AppExit::Success);
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
        // TRANQUILITY_SHOT_AFTER=30 waits that much longer: for a scene
        // that has to play a while first.
        let wait = 2.0 + crate::tools::env_f32("TRANQUILITY_SHOT_AFTER").unwrap_or(0.0);
        if !shot.taken && t > settled + wait {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(shot.path.clone()));
            shot.taken = true;
        }
        if t > settled + wait + 2.0 {
            exit.write(AppExit::Success);
        }
    }
}
