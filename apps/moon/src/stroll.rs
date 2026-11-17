//! The stroll: the walk walks itself, for watching. `0` starts and ends
//! it (`MOON_STROLL=1` starts in it); any movement key hands the legs back.
//!
//! It holds no privileges. It pushes the same stick a player does
//! ([`Steer`]), so grip, slopes, rocks and the streaming of the ground
//! answer exactly as they would to a pair of hands. What it adds is
//! restraint:
//!
//! - **The head faces the planet.** The Earth from the Moon, Jupiter from
//!   Io, Saturn from Enceladus: whatever hangs in the body's sky is held
//!   in the frame, the horizon under it, and stays there. Where nothing
//!   hangs (Mercury, Mars, Titan) the head keeps the body's own view.
//!   It never swings; changes of aim are a slow drift.
//! - **The feet go that way too**, at a walk, never a lope, never a hop;
//!   and round what is in the way. A few times a second it looks along
//!   some thirty bearings and takes the one whose next forty metres are
//!   the most level and clear of boulders, the rover and the sea, for
//!   the least turning. The bearing changes slowly: seen from a steady
//!   head, a detour is the ground drifting a little sideways.
//! - **A longer lens** than the walker's own, so the planet is not a dot
//!   and the horizon is not bent.
//! - **The sun creeps**, a degree in two minutes, back and forth: on an
//!   airless world the shadows are the scenery.
//! - On a body too small to walk on (Phobos) it floats on the suit's jets
//!   a couple of metres up, as slowly.
//!
//! The text goes; a caption names the body and fades.
//!
//! `T` (or `MOON_STROLL=tour`) tours the bodies: a few minutes on each,
//! out to black, and on to the next.

use bevy::{math::DVec2, post_process::effect_stack::Vignette};
use tranquility::{prelude::*, terrain::Chunks};

use crate::{
    Catalogue, Current,
    body::Body,
    heavens::{Companion, Map},
    lander::{KEEP_OFF, Lander},
    rover::Rover,
};

/// The walker's own lens, degrees: what the stroll gives back.
const OWN_LENS: f32 = 70.0;
/// How far ahead the ground is looked at, m, and what each look counts.
const AHEAD: [(f32, f32); 8] = [(1.5, 1.0), (3.0, 1.0), (5.0, 0.8), (8.0, 0.6), (12.0, 0.45), (18.0, 0.3), (27.0, 0.2), (40.0, 0.12)];
/// Bearings tried either side of the way to the planet: 10° apart.
const BEARINGS: i32 = 15;
/// The feet's bearing turns no faster than this, rad/s.
const TURN: f32 = 0.16;
/// Seconds on each body when touring, and the fade out at the end.
/// (`MOON_STAY=seconds` for another stay.)
fn stay() -> f32 {
    crate::options::get("MOON_STAY").and_then(|v| v.parse().ok()).unwrap_or(240.0)
}
const FADE: f32 = 3.0;
/// Floating on jets: height of the feet above the ground, m, and pace, m/s.
const FLOAT: f32 = 1.8;
const DRIFT: f32 = 0.4;

pub struct StrollPlugin;

impl Plugin for StrollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stroll>()
            .add_systems(PostStartup, setup)
            .add_systems(Update, ((switch, guide, lens).chain().after(Walking), creep, tour, caption));
    }
}

#[derive(Resource)]
pub struct Stroll {
    pub on: bool,
    /// The way the feet are going and the way they have decided to go, yaw.
    bearing: f32,
    chosen: f32,
    /// s until the ground ahead is looked at again.
    look_in: f32,
    /// The stick, 0–1: eased, so the walk starts and ends gently.
    stick: f32,
    /// How fast the head is turning, rad/s (yaw, pitch), and the lens
    /// changing, degrees/s.
    turning: Vec2,
    zooming: f32,
    lens: f32,
    /// Which way the sun is creeping.
    sun_up: bool,
    /// Whether the text was already hidden when the stroll began.
    was_hidden: bool,
    /// s since the body's ground was all there; `None` while it streams in.
    arrived: Option<f32>,
    /// Asked for from outside (`MOON_STROLL`), not yet begun.
    pending: bool,
    /// Where the land stands highest, on a body with nothing in the sky.
    skyline: Option<f32>,
    /// Moving on to the next body every few minutes.
    pub touring: bool,
    /// When the body last changed, s since the start.
    changed_at: f32,
    /// How far the picture has gone to black, 0–1: the sound follows.
    pub dark: f32,
}

impl Default for Stroll {
    fn default() -> Self {
        Self {
            on: false,
            bearing: 0.0,
            chosen: 0.0,
            look_in: 0.0,
            stick: 0.0,
            turning: Vec2::ZERO,
            zooming: 0.0,
            lens: OWN_LENS,
            sun_up: true,
            was_hidden: false,
            arrived: None,
            pending: crate::options::get("MOON_STROLL").is_some(),
            skyline: None,
            touring: crate::options::get("MOON_STROLL").is_some_and(|v| v == "tour"),
            changed_at: 0.0,
            dark: 0.0,
        }
    }
}

/// What the head is held on.
struct View {
    yaw: f32,
    pitch: f32,
    /// Vertical field of view, degrees.
    lens: f32,
}

/// The planet in the sky, if the body has one worth facing.
fn planet(body: &Body) -> Option<&Companion> {
    body.companions.iter().find(|c| c.map != Map::Rock)
}

fn view(body: &Body, skyline: Option<f32>) -> View {
    match planet(body) {
        Some(c) => {
            // The planet's top inside the frame with a little sky over it,
            // the horizon three eighths of the way up: the lens is as long
            // as that allows.
            let top = c.elevation + c.radius + 4.0;
            let lens = (top / 0.62).clamp(50.0, 82.0);
            let pitch = (0.12 * lens).max(top - 0.5 * lens);
            View { yaw: -c.azimuth.to_radians(), pitch: pitch.to_radians(), lens }
        }
        None => View { yaw: skyline.unwrap_or(body.look.0), pitch: 2.5f32.to_radians(), lens: 55.0 },
    }
}

fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Where nothing hangs in the sky, the head turns to the land: the bearing
/// whose skyline stands highest within a few kilometres, not into the sun
/// (whose glare flattens everything) and not straight away from it (where
/// nothing casts a shadow you can see).
fn skyline(ground: &dyn Ground, from: DVec2, sun_azimuth: f32) -> f32 {
    let base = ground.height(from.x, from.y) + 1.6;
    let sun = -sun_azimuth.to_radians();
    let mut best = (f32::MIN, sun + 1.6);
    for k in 0..36 {
        let yaw = k as f32 * 10f32.to_radians();
        let off = wrap(yaw - sun).abs();
        if off < 0.9 || off > 2.6 {
            continue;
        }
        let dir = DVec2::new(-yaw.sin() as f64, -yaw.cos() as f64);
        let rise = [400.0, 800.0, 1600.0, 2400.0, 3200.0, 4800.0, 6400.0]
            .iter()
            .map(|d| ((ground.height(from.x + dir.x * d, from.y + dir.y * d) - base) / *d as f32).atan())
            .fold(f32::MIN, f32::max);
        if rise > best.0 {
            best = (rise, yaw);
        }
    }
    wrap(best.1)
}

/// Moves `x` toward `to` like a mass on a critically damped spring of
/// natural frequency `w`: no start, no stop, no overshoot.
fn ease(x: &mut f32, speed: &mut f32, to: f32, w: f32, dt: f32) {
    *speed += (w * w * (to - *x) - 2.0 * w * *speed) * dt;
    *x += *speed * dt;
}

/// `0`, or any key that moves you.
fn switch(
    keys: Res<ButtonInput<KeyCode>>,
    mut stroll: ResMut<Stroll>,
    mut hud: ResMut<Hud>,
    player: Single<(&mut Player, &Projection)>,
) {
    let (mut player, projection) = player.into_inner();
    let taken = [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD, KeyCode::Space, KeyCode::KeyF, KeyCode::KeyR, KeyCode::Escape];
    let toggle = keys.just_pressed(KeyCode::Digit0);
    let begin = !stroll.on && (toggle || stroll.pending) && player.controlled;
    let end = stroll.on && (toggle || !player.controlled || keys.any_just_pressed(taken));
    if begin {
        let lens = if let Projection::Perspective(p) = projection { p.fov.to_degrees() } else { OWN_LENS };
        *stroll = Stroll {
            on: true,
            bearing: player.yaw,
            chosen: player.yaw,
            lens,
            was_hidden: hud.hidden,
            arrived: if stroll.pending { None } else { Some(1e3) },
            pending: false,
            touring: stroll.touring,
            ..default()
        };
        player.flying = false;
        hud.hidden = true;
    } else if end {
        stroll.on = false;
        player.steer = Steer::default();
        hud.hidden = stroll.was_hidden;
    }
}

/// What a bearing would cost over the next forty metres.
fn cost(body: &Body, ground: &dyn Ground, rover: Option<DVec2>, lander: Option<DVec2>, from: DVec2, yaw: f32) -> f32 {
    let dir = DVec2::new(-yaw.sin() as f64, -yaw.cos() as f64);
    let across = DVec2::new(-dir.y, dir.x);
    let sea = body.sea.as_ref().map(|sea| sea.level);
    let (mut before, mut gone) = (ground.height(from.x, from.y), 0.0);
    let mut cost = 0.0;
    for (d, weight) in AHEAD {
        let p = from + dir * d as f64;
        let h = ground.height(p.x, p.y);
        // Slopes: a little for any, a lot for one a boot would slide on.
        let grade = ((h - before) / (d - gone)).abs();
        let over = (grade - 0.1).max(0.0);
        let mut c = 2.0 * grade + 400.0 * over * over;
        if sea.is_some_and(|level| h < level + 0.4) {
            c += 200.0;
        }
        // Boulders, with room to pass.
        if d <= 12.0 && [0.0, -0.9, 0.9].iter().any(|side| { let q = p + across * *side; ground.blocked(q.x, q.y) }) {
            c += 120.0;
        }
        if rover.is_some_and(|r| r.distance(p) < 4.0) {
            c += 150.0;
        }
        if lander.is_some_and(|l| l.distance(p) < KEEP_OFF as f64 + 1.5) {
            c += 150.0;
        }
        cost += weight * c;
        (before, gone) = (h, d);
    }
    cost
}

/// Holds the head on the planet and the stick toward it.
fn guide(
    time: Res<Time>,
    current: Res<Current>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    chunks: Res<Chunks>,
    mut stroll: ResMut<Stroll>,
    player: Single<(&mut Player, &mut Transform), (Without<Rover>, Without<Lander>)>,
    rover: Option<Single<&Transform, (With<Rover>, Without<Lander>)>>,
    lander: Option<Single<&Transform, (With<Lander>, Without<Rover>)>>,
) {
    let dt = time.delta_secs().min(0.1);
    if !stroll.on || dt <= 0.0 {
        return;
    }
    let stroll = &mut *stroll;
    let (mut player, mut eye) = player.into_inner();
    let body = &current.0;
    if current.is_changed() {
        // Another body: wait for its ground, then set off again.
        stroll.arrived = None;
        stroll.changed_at = time.elapsed_secs();
        stroll.stick = 0.0;
        stroll.skyline = None;
        stroll.look_in = 0.0;
    }
    let here = origin.world(eye.translation);
    if stroll.skyline.is_none() && planet(body).is_none() {
        stroll.skyline = Some(skyline(&*world.ground, here, body.sun.0));
    }
    if current.is_changed() {
        stroll.bearing = view(body, stroll.skyline).yaw;
        stroll.chosen = stroll.bearing;
    }
    match &mut stroll.arrived {
        Some(since) => *since += dt,
        // The old ground counts as settled until the new one is asked
        // for: give it a moment.
        None if chunks.is_settled() && time.elapsed_secs() > 2.0 && time.elapsed_secs() - stroll.changed_at > 2.0 => {
            stroll.arrived = Some(0.0)
        }
        None => {}
    }

    // The head.
    let aim = view(body, stroll.skyline);
    let (mut yaw, mut pitch) = (player.yaw, player.pitch);
    let to = yaw + wrap(aim.yaw - yaw);
    ease(&mut yaw, &mut stroll.turning.x, to, 0.45, dt);
    ease(&mut pitch, &mut stroll.turning.y, aim.pitch, 0.45, dt);
    (player.yaw, player.pitch) = (wrap(yaw), pitch);
    eye.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);

    // The feet: stand until the ground is there, then go.
    let going = stroll.arrived.is_some_and(|since| since > 1.5);
    stroll.stick += ((if going { 1.0 } else { 0.0 }) - stroll.stick) * (1.0 - (-dt / 1.5).exp());
    stroll.look_in -= dt;
    if stroll.look_in <= 0.0 && going {
        stroll.look_in = 0.4;
        let rover = rover.map(|r| origin.world(r.translation));
        let lander = lander.map(|l| origin.world(l.translation));
        let mut best = (f32::MAX, stroll.chosen);
        for k in -BEARINGS..=BEARINGS {
            let off = (k as f32 * 10.0).to_radians();
            let yaw = aim.yaw + off;
            // Straight at the planet if nothing says otherwise, and no
            // change of mind without a reason.
            let c = cost(body, &*world.ground, rover, lander, here, yaw) + 6.0 * off * off + 1.5 * wrap(yaw - stroll.chosen).abs();
            if c < best.0 {
                best = (c, yaw);
            }
        }
        stroll.chosen = best.1;
    }
    let turn = wrap(stroll.chosen - stroll.bearing);
    stroll.bearing = wrap(stroll.bearing + (turn * 1.2 * dt).clamp(-TURN * dt, TURN * dt));
    let dir = Vec3::new(-stroll.bearing.sin(), 0.0, -stroll.bearing.cos());

    let w = &world.walker;
    if w.jets > 1.5 * w.gravity {
        // Too little weight to walk: float on the jets, which only ever
        // add velocity, so holding a slow drift means thrusting against
        // it as well as with it.
        let floor = |p: DVec2| world.ground.stand_height(p.x, p.y);
        let flat = Vec3::new(player.velocity.x, 0.0, player.velocity.z);
        let lead = here + (flat.normalize_or(dir) * 4.0).xz().as_dvec2();
        let height = player.feet - floor(here).max(floor(lead));
        let push = (dir * DRIFT * stroll.stick - flat) / 4.0;
        let up = w.gravity + 0.02 * (FLOAT - height) - 0.28 * player.velocity.y;
        let ask = Vec3::new(push.x, if going { up } else { 0.0 }, push.z) / w.jets;
        player.steer.go = ask.clamp_length_max(1.0);
    } else {
        player.steer.go = dir * stroll.stick;
    }
}

/// The lens: longer on a stroll, the walker's own again after it.
fn lens(time: Res<Time>, current: Res<Current>, mut stroll: ResMut<Stroll>, mut projection: Single<&mut Projection, With<Player>>) {
    let dt = time.delta_secs().min(0.1);
    let to = if stroll.on { view(&current.0, stroll.skyline).lens } else { OWN_LENS };
    if (stroll.lens - to).abs() < 0.01 && stroll.zooming.abs() < 0.01 {
        return;
    }
    let stroll = &mut *stroll;
    ease(&mut stroll.lens, &mut stroll.zooming, to, 0.6, dt);
    if let Projection::Perspective(p) = &mut **projection {
        p.fov = stroll.lens.to_radians();
    }
}

/// The sun creeps between a little under and a little over where the body
/// has it, turning at the ends.
fn creep(time: Res<Time>, current: Res<Current>, mut stroll: ResMut<Stroll>, mut sun: ResMut<Sun>) {
    if !stroll.on || stroll.arrived.is_none() {
        return;
    }
    let home = current.0.sun.1;
    let (low, high) = ((home - 5.0).max(6.0), home + 9.0);
    if sun.elevation >= high {
        stroll.sun_up = false;
    } else if sun.elevation <= low {
        stroll.sun_up = true;
    }
    sun.elevation += if stroll.sun_up { 0.008 } else { -0.008 } * time.delta_secs();
}

/// `T`: touring or not. When the time on a body is up, the picture goes to
/// black and the next body comes up out of it.
fn tour(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    bodies: Res<Catalogue>,
    mut current: ResMut<Current>,
    mut stroll: ResMut<Stroll>,
) {
    if keys.just_pressed(KeyCode::KeyT) && stroll.on {
        stroll.touring = !stroll.touring;
    }
    if !stroll.on || !stroll.touring || stroll.arrived.is_none_or(|since| since < stay()) {
        return;
    }
    let here = bodies.0.iter().position(|b| b.name == current.0.name).unwrap_or(0);
    let next = bodies.0[(here + 1) % bodies.0.len()].clone();
    commands.insert_resource(next.world());
    current.0 = next;
}

/// The black the picture comes out of on arriving somewhere.
#[derive(Component)]
struct Veil;

#[derive(Component)]
struct Caption(u8);

fn setup(mut commands: Commands, font: Res<tranquility::hud::HudFont>) {
    commands.spawn((
        Veil,
        Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), bottom: px(0), ..default() },
        BackgroundColor(Color::NONE),
        Visibility::Hidden,
    ));
    let line = |k: u8, size: f32, bottom: f32| {
        (
            Caption(k),
            Text::new(""),
            TextFont { font: font.0.clone().into(), font_size: FontSize::Px(size), ..default() },
            TextColor(Color::NONE),
            TextLayout::justify(Justify::Center),
            Node { position_type: PositionType::Absolute, left: px(0), right: px(0), bottom: percent(bottom), ..default() },
        )
    };
    commands.spawn(line(0, 34.0, 13.0));
    commands.spawn(line(1, 16.0, 9.0));
}

/// Spaced capitals: a title, not a label.
fn title(name: &str) -> String {
    name.to_uppercase().chars().flat_map(|c| [c, ' ', ' ']).collect::<String>().trim_end().to_string()
}

fn caption(
    current: Res<Current>,
    mut stroll: ResMut<Stroll>,
    camera: Query<(Entity, Has<Vignette>), With<Player>>,
    mut commands: Commands,
    mut veil: Single<(&mut BackgroundColor, &mut Visibility), With<Veil>>,
    mut lines: Query<(&Caption, &mut Text, &mut TextColor)>,
) {
    // A little fall-off toward the corners, as any lens has.
    for (entity, has) in &camera {
        if stroll.on && !has {
            commands.entity(entity).insert(Vignette { intensity: 0.45, radius: 0.9, smoothness: 1.6, ..default() });
        } else if !stroll.on && has {
            commands.entity(entity).remove::<Vignette>();
        }
    }
    // Black until the ground is there, then three seconds out of it; the
    // caption comes after and stays ten seconds.
    let since = stroll.arrived;
    let leaving = |t: f32| if stroll.touring { ((t - (stay() - FADE)) / FADE).clamp(0.0, 1.0) } else { 0.0 };
    let dark = if !stroll.on { 0.0 } else { since.map_or(1.0, |t| (1.0 - (t / 3.0).clamp(0.0, 1.0)).max(leaving(t))) };
    let shown = if !stroll.on { 0.0 } else { since.map_or(0.0, |t| ((t - 2.5) / 2.5).clamp(0.0, 1.0) * ((15.0 - t) / 3.0).clamp(0.0, 1.0)) };
    stroll.dark = dark;
    let (color, visibility) = &mut *veil;
    let want = if dark > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    visibility.set_if_neq(want);
    if dark > 0.0 || color.0.alpha() > 0.0 {
        color.0 = Color::srgba(0.0, 0.0, 0.0, dark * dark * (3.0 - 2.0 * dark));
    }
    let body = &current.0;
    for (line, mut text, mut color) in &mut lines {
        let words = if line.0 == 0 { title(body.name) } else { body.site.to_string() };
        if shown > 0.0 && text.0 != words {
            text.0 = words;
        }
        let alpha = shown * if line.0 == 0 { 0.92 } else { 0.7 };
        if color.0.alpha() != alpha {
            color.0 = Color::srgba(1.0, 1.0, 1.0, alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_planet_is_in_the_frame_above_the_horizon() {
        for body in crate::bodies::all() {
            let Some(c) = planet(&body) else { continue };
            let v = view(&body, None);
            let (pitch, half) = (v.pitch.to_degrees(), v.lens / 2.0);
            // The whole disc inside the frame, with room over it.
            assert!(c.elevation + c.radius < pitch + half - 1.0, "{}: top of {:?} out of the frame", body.name, c.map);
            assert!(c.elevation - c.radius > pitch - half, "{}: bottom out of the frame", body.name);
            // The horizon in the lower half, but in the picture.
            assert!(pitch > 0.0 && pitch < half - 3.0, "{}: horizon at {pitch}° in a {}° lens", body.name, v.lens);
            assert!(wrap(v.yaw + c.azimuth.to_radians()).abs() < 1e-4, "{}: not facing it", body.name);
        }
    }

    #[test]
    fn easing_neither_overshoots_nor_jerks() {
        let (mut x, mut speed) = (0.0f32, 0.0f32);
        let mut fastest = 0.0f32;
        for _ in 0..2000 {
            ease(&mut x, &mut speed, 1.0, 0.45, 1.0 / 60.0);
            assert!(x <= 1.0 + 1e-4);
            fastest = fastest.max(speed);
        }
        assert!((x - 1.0).abs() < 0.01);
        // A radian's turn never faster than a tenth of a radian a second.
        assert!(fastest < 0.2, "{fastest}");
    }
}
