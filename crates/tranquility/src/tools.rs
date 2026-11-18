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
//! - `TRANQUILITY_RECORD=out.mp4` records a video offline (see `record`).
//! - `TRANQUILITY_KEYS="2:KeyQ,4+3:KeyW"` presses keys by itself (see
//!   `Script`).
//! - `TRANQUILITY_CLEAN=1` shows no text at all, notices included: for
//!   pictures.
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
        #[cfg(not(target_arch = "wasm32"))]
        crate::record::add(app);
        if crate::unattended() {
            app.add_systems(PreUpdate, hands_off.before(bevy::input::InputSystems));
        }
        if let Some(keys) = std::env::var("TRANQUILITY_KEYS").ok().and_then(|v| Script::parse(&v)) {
            app.insert_resource(keys).add_systems(PreUpdate, press.before(bevy::input::InputSystems).after(hands_off));
        }
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

/// A run that saves a frame, measures or records goes by itself: keys
/// and the mouse are dropped, so someone working at the machine meanwhile,
/// with the window popping up under their hands, changes nothing in it.
fn hands_off(
    mut keys: ResMut<Messages<bevy::input::keyboard::KeyboardInput>>,
    mut buttons: ResMut<Messages<bevy::input::mouse::MouseButtonInput>>,
    mut motion: ResMut<Messages<bevy::input::mouse::MouseMotion>>,
    mut wheel: ResMut<Messages<bevy::input::mouse::MouseWheel>>,
) {
    keys.clear();
    buttons.clear();
    motion.clear();
    wheel.clear();
}

/// `TRANQUILITY_KEYS="2:KeyQ,4+3:KeyW,9:F11"`: keys pressed by themselves,
/// seconds after the ground has settled (the screenshot's clock), held a
/// frame or, after a `+`, that many seconds. For trying a key's effect in
/// a screenshot without sitting at the window.
#[derive(Resource)]
struct Script {
    /// When, for how long, which key; and whether it is down.
    keys: Vec<(f32, f32, KeyCode, Stage)>,
    settled: Option<f32>,
}

impl Script {
    fn parse(v: &str) -> Option<Self> {
        let keys = v
            .split(',')
            .map(|item| {
                let (when, name) = item.trim().split_once(':')?;
                let (at, hold) = when.split_once('+').map_or((when, "0"), |(a, h)| (a, h));
                let code = key_named(name.trim());
                if code.is_none() {
                    warn!("TRANQUILITY_KEYS: no key named {name}");
                }
                Some((at.trim().parse().ok()?, hold.trim().parse().ok()?, code?, Stage::Waiting))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { keys, settled: None })
    }
}

/// A key by its `KeyCode` name: `KeyQ`, `Digit3`, `F11`, `Space`, ….
fn key_named(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    let all = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, Escape, Space,
        Enter, Tab, ShiftLeft, ShiftRight, BracketLeft, BracketRight, ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    ];
    all.into_iter().find(|k| format!("{k:?}") == name)
}

fn press(
    time: Res<Time>,
    terrain: Res<crate::terrain::Chunks>,
    window: Single<Entity, With<bevy::window::PrimaryWindow>>,
    mut script: ResMut<Script>,
    mut input: MessageWriter<bevy::input::keyboard::KeyboardInput>,
) {
    use bevy::input::{ButtonState, keyboard::{Key, KeyboardInput}};
    let t = time.elapsed_secs();
    if script.settled.is_none() && t > 2.0 && terrain.is_settled() {
        script.settled = Some(t);
    }
    let Some(settled) = script.settled else { return };
    let t = t - settled;
    for (at, hold, key, state) in &mut script.keys {
        // Waiting, down, done: a tap is down for one frame.
        let next = match *state {
            Stage::Waiting if t >= *at => Stage::Down,
            Stage::Down if t >= *at + *hold => Stage::Done,
            s => s,
        };
        if next != *state {
            *state = next;
            input.write(KeyboardInput {
                key_code: *key,
                logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                state: if next == Stage::Down { ButtonState::Pressed } else { ButtonState::Released },
                text: None,
                repeat: false,
                window: *window,
            });
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Waiting,
    Down,
    Done,
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
