//! Fly the landing: the Apollo lunar module's powered descent from high
//! gate, on the engine's rocket physics ([`tranquility::rocket`]) with the
//! module's published numbers.
//!
//! High gate is where the computer's approach program, P64, took over
//! from the braking phase: some 2 km up and 7 km short of the site,
//! coming in from the east with the sun low behind, pitched back against
//! the way the module goes and slowing. P64 steers to the landing point and
//! shows the commander the angle to look along in his window to see where
//! it is going (the landing point designator, LPD); a click of the hand
//! controller moves that point, half a degree long or short, two degrees
//! left or right. Near low gate the crews took over: P66, attitude by hand,
//! the computer holding a rate of descent the commander nudged a foot per
//! second at a time with a switch. P65 would have landed by itself; nobody
//! let it. Probes under three footpads touch first and light the contact
//! light; then engine stop, and the legs take what is left of the
//! descent, up to 3 m/s down and 1.2 m/s across.
//!
//! The descent engine: 45 kN at full thrust, throttled between 10 and 60 %
//! (never between 60 and full), 311 s of specific impulse. At high gate the
//! module weighs some 7.9 t: the ascent stage 4.7, the empty descent stage
//! 2.0, and about 1.15 t of descent propellant left of the 8.2 t it set out
//! with. The quads' sixteen 445 N jets turn it. "Low level" lights at 5.6 %
//! of the load; then the hover time left is called at 60 and 30 seconds.
//!
//! **L** starts it at high gate, **Shift+L** at low gate (on a body
//! without air); **W/S/A/D** redesignate in
//! P64 and fly the attitude in P66; **P** takes over (P66), **Shift+P**
//! lets the computer land (P65); **Space/C** the rate of descent switch;
//! **Enter** engine stop; **V** the commander's window, a chase camera, or
//! the ground at the site; **R** out once down.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::math::DVec2;
use tranquility::{
    prelude::*,
    rocket::{Burn, Lander as Craft, LanderSpec, Program, RocketPlugin},
};

use crate::{
    Current,
    lander::{self, Kit, Lander as Parked, PAD},
    rover::{Rover, Thrown},
};

/// Centre of mass above the footpads' undersides, m.
const CG: f32 = 3.2;
/// The commander's eyes at his window, from the centre of mass: just
/// outside the glass, so the cabin is behind the camera.
const EYE: Vec3 = Vec3::new(-0.48, 1.35, -1.68);
/// Where a descent starts, from the landing point: m back along the
/// approach, m up, the velocity then (along, up), m/s, how far the module
/// is pitched back, rad, and how long P64 has left to its aim, s.
struct Gate {
    back: f32,
    up: f32,
    along: f32,
    sink: f32,
    pitch: f32,
    time: f32,
}

/// High gate, where P64 took over from the braking phase.
const HIGH_GATE: Gate = Gate { back: 7400.0, up: 2100.0, along: 140.0, sink: -40.0, pitch: 0.78, time: 110.0 };
/// Low gate, near where the crews took over by hand: the last minute.
const LOW_GATE: Gate = Gate { back: 420.0, up: 150.0, along: 16.0, sink: -4.0, pitch: 0.3, time: 30.0 };
/// P64's aim: this far above the landing point, still sinking at this rate.
const AIM: (f32, f32) = (30.0, -1.0);
/// The descent stage's full propellant load, kg, and the share at which
/// "low level" lights.
const LOAD: f32 = 8200.0;
const LOW_LEVEL: f32 = 0.056;
/// A foot per second: the rate of descent switch's click.
const FOOT: f32 = 0.3048;

pub fn spec() -> LanderSpec {
    let feet = (0..4)
        .map(|k| Quat::from_rotation_y(k as f32 * FRAC_PI_2) * Vec3::new(0.0, 0.0, -PAD) + Vec3::Y * -CG)
        .collect();
    LanderSpec {
        dry_mass: 6730.0,
        propellant: 1150.0,
        thrust: 45_040.0,
        isp: 311.0,
        throttle: (0.10, 0.60),
        full: 0.925,
        // Roughly a 7.5 t body 7 m tall and 4 m across.
        inertia: Vec3::new(40_000.0, 22_000.0, 40_000.0),
        // Two pairs of 445 N jets on quads some 1.65 m out.
        jet_torque: 2940.0,
        jet_flow: 0.62,
        jet_propellant: 280.0,
        feet,
        probes: vec![1, 2, 3],
        probe: lander::PROBE,
        limits: (3.0, 1.2, 12f32.to_radians()),
        max_rate: 10f32.to_radians(),
        agility: 1.2,
        max_tilt: 0.8,
    }
}

pub struct DescentPlugin;

impl Plugin for DescentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RocketPlugin).init_resource::<Descent>().add_systems(
            Update,
            (
                (stop.run_if(tranquility::world_replaced), start, fly).chain().before(Burn).after(crate::rover::Boarding),
                (watch, blow, settle).chain().after(Burn),
            ),
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum View {
    #[default]
    Window,
    Chase,
    Ground,
}

/// The descent under way, if one is.
#[derive(Resource, Default)]
pub struct Descent {
    pub on: bool,
    view: View,
    /// The landing point, world, and its ground's height.
    target: DVec2,
    floor: f32,
    /// The rate the switch has set, m/s.
    rate: f32,
    /// Callouts made.
    said: Vec<&'static str>,
    behind: Option<Vec3>,
    /// The app's line, put aside for the instruments.
    info: Option<String>,
    demoed: bool,
    craft: Option<Entity>,
}

#[derive(Component)]
struct Flying;

fn heading_of(rot: Quat) -> f32 {
    let ahead = rot * Vec3::NEG_Z;
    (-ahead.x).atan2(-ahead.z)
}

/// **L**: from high gate, with the site's landing point ahead.
#[allow(clippy::too_many_arguments)]
fn start(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<Current>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    kit: Res<Kit>,
    mut d: ResMut<Descent>,
    mut hud: ResMut<Hud>,
    player: Single<&mut Player>,
    parked: Query<Entity, With<Parked>>,
    flying: Query<(), With<Flying>>,
    mut rovers: Query<&mut Visibility, With<Rover>>,
) {
    let demo = std::env::var("MOON_DESCENT").unwrap_or_default();
    let asked = keys.just_pressed(KeyCode::KeyL) || (!demo.is_empty() && !d.demoed);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let gate = if shift || demo.contains("lowgate") { &LOW_GATE } else { &HIGH_GATE };
    // One at a time.
    if !asked || d.on || !flying.is_empty() {
        return;
    }
    let mut player = player.into_inner();
    if current.0.air_density > 0.0 || !current.0.air.is_none() {
        hud.say("the lunar module lands where there is no air: try the Moon");
        return;
    }
    if !player.controlled || player.flying {
        return;
    }
    d.demoed = true;
    // The landing point: the levellest spot for the footpads near where
    // the walk starts, as the planners picked one in the site's photographs.
    let target = level_spot(&*world.ground, DVec2::new(world.spawn.x, world.spawn.z));
    let floor = world.ground.height(target.x, target.y);
    // Coming in from the east, low sun behind: flying west.
    let west = Vec3::NEG_X;
    let at = target - DVec2::new(west.x as f64, west.z as f64) * gate.back as f64;
    let y = floor + gate.up;
    let pitch = Quat::from_rotation_arc(Vec3::Y, Vec3::new(gate.pitch.sin(), gate.pitch.cos(), 0.0));
    let rot = pitch * Quat::from_rotation_y(FRAC_PI_2);
    let program = Program::Guided {
        target: origin.render(target, floor + AIM.0),
        velocity: Vec3::new(0.0, AIM.1, 0.0),
        time: gate.time,
    };
    let mut craft = Craft::new(spec(), program);
    craft.velocity = west * gate.along + Vec3::Y * gate.sink;
    // At low gate a minute's flying less propellant is left: the hover
    // time Apollo 11 had there.
    if gate.time < HIGH_GATE.time {
        craft.propellant = 640.0;
    }
    let entity = commands
        .spawn((Flying, craft, Transform::from_translation(origin.render(at, y)).with_rotation(rot), Visibility::default()))
        .with_children(|c| {
            c.spawn((Transform::from_xyz(0.0, -CG, 0.0), Visibility::default())).with_children(|lm| lander::model(lm, &kit, None));
        })
        .id();
    // The site is empty until it lands; the rover rides folded in its bay.
    for e in &parked {
        commands.entity(e).despawn();
    }
    for mut v in &mut rovers {
        v.set_if_neq(Visibility::Hidden);
    }
    let view = if demo.contains("chase") {
        View::Chase
    } else if demo.contains("ground") {
        View::Ground
    } else {
        View::Window
    };
    *d = Descent { on: true, view, target, floor, rate: AIM.1, info: Some(hud.info.clone()), demoed: true, craft: Some(entity), ..default() };
    player.controlled = false;
    player.velocity = Vec3::ZERO;
    (player.yaw, player.pitch) = (0.0, 0.0);
    hud.say(if gate.time < HIGH_GATE.time {
        "low gate: P64 a little longer, then yours. W/S/A/D redesignate · P takes over · V the view"
    } else {
        "high gate: P64, the computer steering for the site. W/S/A/D redesignate · P takes over · V the view"
    });
}

/// The spot within some 60 m of a point where four footpads would stand
/// most nearly level.
fn level_spot(ground: &dyn Ground, near: DVec2) -> DVec2 {
    let pad = PAD as f64;
    let tilt = |at: DVec2| {
        let h = |dx: f64, dz: f64| ground.height(at.x + dx, at.y + dz);
        let (n, e, s, w, c) = (h(0.0, -pad), h(pad, 0.0), h(0.0, pad), h(-pad, 0.0), h(0.0, 0.0));
        (n - s).abs().max((e - w).abs()) + 0.5 * (c - 0.25 * (n + e + s + w)).abs()
    };
    let mut best = (tilt(near), near);
    for k in 1..400 {
        let (angle, radius) = (k as f64 * 2.4, 3.0 * (k as f64).sqrt());
        let at = near + DVec2::new(angle.cos(), angle.sin()) * radius;
        let t = tilt(at);
        if t < best.0 && !ground.blocked(at.x, at.y) {
            best = (t, at);
        }
    }
    best.1
}

/// The keys into the computer, every frame.
fn fly(
    keys: Res<ButtonInput<KeyCode>>,
    origin: Res<Origin>,
    mut d: ResMut<Descent>,
    mut hud: ResMut<Hud>,
    craft: Option<Single<(&mut Craft, &Transform), With<Flying>>>,
) {
    let Some(craft) = craft else { return };
    if !d.on {
        return;
    }
    let (mut c, pose) = craft.into_inner();
    let auto = std::env::var("MOON_DESCENT").is_ok_and(|v| v.contains("auto"));
    let press = |k: KeyCode| keys.just_pressed(k);
    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if press(KeyCode::Enter) || press(KeyCode::NumpadEnter) {
        c.program = Program::Off;
        hud.say("engine stop");
    }
    match c.program {
        Program::Guided { velocity, time, .. } => {
            // Landing point redesignation: half a degree of LPD angle long
            // or short, two degrees of azimuth left or right.
            let here = origin.world(pose.translation);
            let to = d.target - here;
            let range = to.length().max(1.0);
            let height = (pose.translation.y - d.floor).max(1.0) as f64;
            let depression = height.atan2(range);
            let click = |angle: f64| height / angle.tan();
            let along = to / range;
            if press(KeyCode::KeyW) {
                d.target += along * (click((depression - 0.5f64.to_radians()).max(0.02)) - range);
            }
            if press(KeyCode::KeyS) {
                d.target += along * (click(depression + 0.5f64.to_radians()) - range);
            }
            let turn = press(KeyCode::KeyA) as i32 - press(KeyCode::KeyD) as i32;
            if turn != 0 {
                let a = 2f64.to_radians() * turn as f64;
                let (sin, cos) = a.sin_cos();
                // Anticlockwise from above is to the left, the way it flies.
                d.target = here + DVec2::new(to.x * cos + to.y * sin, -to.x * sin + to.y * cos);
            }
            if press(KeyCode::KeyW) || press(KeyCode::KeyS) || turn != 0 {
                d.said.retain(|s| *s != "redesignated");
            }
            c.program = Program::Guided { target: origin.render(d.target, d.floor + AIM.0), velocity, time };
            // Low gate: the computer's approach is done.
            if time < 5.0 {
                c.program = if auto { Program::Vertical { rate: -0.9 } } else { Program::Descent { rate: velocity.y } };
                d.rate = velocity.y;
                hud.say(if auto {
                    "low gate: P65, the computer setting it down"
                } else {
                    "low gate: P66, yours. W/S/A/D the attitude, Space/C the rate of descent a foot a second, Enter at contact light"
                });
            }
        }
        Program::Descent { .. } | Program::Vertical { .. } | Program::Manual { .. } | Program::Off => {}
    }
    if press(KeyCode::KeyP) && !matches!(c.program, Program::Off) {
        if shift {
            c.program = Program::Vertical { rate: c.velocity.y.clamp(-3.0, -0.5) };
            hud.say("P65: the computer setting it down");
        } else if !matches!(c.program, Program::Descent { .. }) {
            d.rate = (c.velocity.y / FOOT).round() * FOOT;
            c.program = Program::Descent { rate: d.rate };
            hud.say("P66: attitude by hand, Space/C the rate of descent");
        }
    }
    if let Program::Descent { .. } = c.program {
        if press(KeyCode::Space) {
            d.rate += FOOT;
        }
        if press(KeyCode::KeyC) {
            d.rate -= FOOT;
        }
        c.program = Program::Descent { rate: d.rate };
        // The hand controller: pitch forward with W, roll right with D.
        c.stick = Vec3::new(-axis(KeyCode::KeyW, KeyCode::KeyS), 0.0, -axis(KeyCode::KeyD, KeyCode::KeyA));
    } else {
        c.stick = Vec3::ZERO;
    }
    if auto && c.contact && !matches!(c.program, Program::Off) {
        c.program = Program::Off;
        hud.say("contact light. engine stop");
    }
}

/// The camera, the instruments and the callouts.
#[allow(clippy::too_many_arguments)]
fn watch(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut shifted: MessageReader<OriginShifted>,
    mut d: ResMut<Descent>,
    mut hud: ResMut<Hud>,
    keys: Res<ButtonInput<KeyCode>>,
    player: Single<(&Player, &mut Transform), Without<Flying>>,
    craft: Option<Single<(&Craft, &Transform), With<Flying>>>,
) {
    for s in shifted.read() {
        if let Some(b) = d.behind.as_mut() {
            *b -= s.delta;
        }
    }
    let Some(craft) = craft else { return };
    if !d.on {
        return;
    }
    let (c, pose) = craft.into_inner();
    let (player, mut eye) = player.into_inner();
    if keys.just_pressed(KeyCode::KeyV) {
        d.view = match d.view {
            View::Window => View::Chase,
            View::Chase => View::Ground,
            View::Ground => View::Window,
        };
        d.behind = None;
    }
    let look = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    match d.view {
        View::Window => {
            // Out of the commander's window, forward and down.
            eye.translation = pose.translation + pose.rotation * EYE;
            eye.rotation = pose.rotation * Quat::from_rotation_x(-0.45) * look;
        }
        View::Chase => {
            let heading = Quat::from_rotation_y(heading_of(pose.rotation) + player.yaw);
            let want = pose.translation + heading * Vec3::new(8.0, 9.0 - 25.0 * player.pitch.min(0.3), 34.0);
            let ease = 1.0 - (-time.delta_secs() * 3.0).exp();
            let mut at = d.behind.map_or(want, |b| b + (want - b) * ease);
            let w = origin.world(at);
            at.y = at.y.max(world.ground.height(w.x, w.y) + 1.0);
            d.behind = Some(at);
            *eye = Transform::from_translation(at).looking_at(pose.translation, Vec3::Y);
        }
        View::Ground => {
            // Standing by the landing point, as a camera on the ground
            // would: off to the north, out of the plume's way.
            let at = d.target + DVec2::new(0.0, -60.0);
            let floor = world.ground.height(at.x, at.y);
            eye.translation = origin.render(at, floor + 1.7);
            *eye = eye.looking_at(pose.translation, Vec3::Y);
        }
    }
    // Instruments: the program, the LPD angle, the radar's altitude and
    // rate, speed across, propellant and the hover time it buys.
    let g = world.walker.gravity;
    let program = match c.program {
        Program::Guided { .. } => "P64",
        Program::Descent { .. } => "P66",
        Program::Vertical { .. } => "P65",
        Program::Manual { .. } => "P67",
        Program::Off => "ENGINE OFF",
    };
    let here = origin.world(pose.translation);
    let target = origin.render(d.target, d.floor);
    let sight = pose.rotation.inverse() * (target - (pose.translation + pose.rotation * EYE));
    let lpd = (-sight.y).atan2(-sight.z).to_degrees();
    let across = pose.rotation.inverse() * Vec3::new(c.velocity.x, 0.0, c.velocity.z);
    let left = (c.propellant / LOAD * 100.0).max(0.0);
    let hover = c.hover_time(g);
    let to_go = (d.target - here).length();
    let program_line = match c.program {
        Program::Guided { .. } => format!("{program}  LPD {lpd:.0}° (look along it in the window)  {to_go:.0} m to the landing point"),
        Program::Descent { rate } => format!("{program}  rate of descent set {:+.0} ft/s ({:+.1} m/s)", rate / FOOT, rate),
        _ => program.to_string(),
    };
    let lights = [(c.contact, "CONTACT LIGHT"), (left < LOW_LEVEL * 100.0, "LOW LEVEL")]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, l)| *l)
        .collect::<Vec<_>>()
        .join("  ");
    let info = format!(
        "{program_line}   {lights}\nALT {:.0} m  ALT RATE {:+.1} m/s  FWD {:+.1} m/s  LAT {:+.1} m/s · THROTTLE {:.0} % · DESCENT PROPELLANT {:.1} % ({:.0} s of hover) · jets {:.0} kg · {:.1} t",
        c.height,
        c.velocity.y,
        -across.z,
        across.x,
        c.throttle * 100.0,
        left,
        hover,
        c.jet_propellant,
        c.mass() / 1000.0,
    );
    if hud.info != info {
        hud.info = info;
    }
    let mode = format!("DESCENT {program} (V: view)");
    if hud.mode != mode {
        hud.mode = mode;
    }
    let call = |d: &mut Descent, key: &'static str, line: &str, hud: &mut Hud| {
        if !d.said.contains(&key) {
            d.said.push(key);
            hud.say(line.to_string());
        }
    };
    if left < LOW_LEVEL * 100.0 {
        call(&mut d, "low", "LOW LEVEL: the descent propellant is down to its last few percent", &mut hud);
    }
    if left < LOW_LEVEL * 100.0 && hover < 60.0 {
        call(&mut d, "60", "60 seconds", &mut hud);
    }
    if hover < 30.0 {
        call(&mut d, "30", "30 seconds", &mut hud);
    }
    if c.contact {
        call(&mut d, "contact", "CONTACT LIGHT", &mut hud);
    }
    if c.propellant <= 0.0 && !c.landed {
        call(&mut d, "dry", "descent propellant gone: the engine has stopped", &mut hud);
    }
}

/// The plume on the ground: within a few tens of metres it blows the
/// regolith out in flat sheets, grains flying fast and low and, with no
/// air to stop them, far. Stronger as the plume's pressure on the ground
/// grows with the square of the closeness.
fn blow(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    craft: Option<Single<(&Craft, &Transform), With<Flying>>>,
    mut thrown: MessageWriter<Thrown>,
    mut seed: Local<u32>,
) {
    let Some(craft) = craft else { return };
    let (c, pose) = craft.into_inner();
    if c.thrust <= 0.0 || c.height > 45.0 {
        return;
    }
    let mut random = || {
        *seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
        let x = (*seed >> ((*seed >> 28) + 4)) ^ *seed;
        (x.wrapping_mul(277803737) >> 8) as f32 / 16777216.0
    };
    let nozzle = pose.translation + pose.rotation * Vec3::new(0.0, -CG + 1.0, 0.0);
    let axis = pose.rotation * Vec3::NEG_Y;
    // Where the plume's axis meets the ground.
    let w = origin.world(nozzle);
    let floor = world.ground.height(w.x, w.y);
    let drop = (nozzle.y - floor).max(1.0);
    let hit = nozzle + axis * (drop / axis.y.abs().max(0.3)).min(80.0);
    let strength = (c.thrust / 15_000.0) * (12.0 / drop).powi(2).min(4.0);
    let count = (2500.0 * strength * time.delta_secs() + random()) as usize;
    for _ in 0..count.min(400) {
        let a = random() * std::f32::consts::TAU;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        let reach = 2.0 + 14.0 * random();
        let at = Vec3::new(hit.x, floor + 0.05, hit.z) + out * reach;
        // A degree or two off the ground, most at tens of m/s and a few
        // much faster. The finest real grains go faster still, at hundreds
        // of m/s, and are gone from sight in a moment: drawn, they would
        // only empty the budget of grains far away, so the sheet is drawn
        // from the slower ones that stay near.
        let speed = 15.0 + 90.0 * random() * random() + if random() < 0.1 { 200.0 * random() } else { 0.0 };
        let rise = (0.3 + 1.7 * random()).to_radians();
        let w = origin.world(at);
        thrown.write(Thrown { at, velocity: out * speed * rise.cos() + Vec3::Y * speed * rise.sin(), floor: world.ground.height(w.x, w.y), size: 0.05 + 0.12 * random() });
    }
}

/// Down: the module stays where it is, standing on its legs; **R** steps
/// out at the foot of the ladder.
#[allow(clippy::too_many_arguments)]
fn settle(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    kit: Res<Kit>,
    mut d: ResMut<Descent>,
    mut hud: ResMut<Hud>,
    player: Single<(&mut Player, &mut Transform), Without<Flying>>,
    craft: Option<Single<(Entity, &Craft, &Transform), With<Flying>>>,
    mut rovers: Query<&mut Visibility, With<Rover>>,
) {
    let Some(craft) = craft else { return };
    let (entity, c, pose) = craft.into_inner();
    if !d.on {
        return;
    }
    let Some(verdict) = &c.verdict else { return };
    if !d.said.contains(&"down") {
        d.said.push("down");
        let (down, across) = c.touchdown.unwrap_or_default();
        let off = (origin.world(pose.translation) - d.target).length();
        let (down_, across_) = (down, across);
        info!("descent: down at {down_:.2} m/s down, {across_:.2} m/s across, {off:.0} m from the landing point, {:.0} kg of descent propellant left: {verdict:?}", c.propellant);
        hud.say(match verdict {
            Ok(()) => format!(
                "down: {down:.1} m/s down, {across:.1} across, {off:.0} m from the landing point, {:.0} s of hover left. R to step out",
                c.hover_time(world.walker.gravity)
            ),
            Err(why) => format!("the lunar module {why}. R to step out"),
        });
    }
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }
    // The flying module becomes the one standing at the site.
    let at = origin.world(pose.translation);
    let facing = heading_of(pose.rotation);
    commands.entity(entity).despawn();
    lander::place(&mut commands, &kit, &*world.ground, &origin, at, facing);
    for mut v in &mut rovers {
        v.set_if_neq(Visibility::Inherited);
    }
    let (mut player, mut eye) = player.into_inner();
    let ladder = Quat::from_rotation_y(facing) * Vec3::new(0.6, 0.0, -(PAD + 1.2));
    let foot = origin.render(at, 0.0) + ladder;
    player.controlled = true;
    player.stand_at(&mut eye, &world, &origin, foot);
    info!("descent: stepped out at the foot of the ladder, {:.1} m from the module's centre", Vec2::new(ladder.x, ladder.z).length());
    // Facing back toward the module.
    player.yaw = facing + PI;
    player.pitch = 0.05;
    eye.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    if let Some(info) = d.info.take() {
        hud.info = info;
    }
    hud.mode.clear();
    d.on = false;
    d.craft = None;
}

/// Changing world mid-descent ends it.
fn stop(mut commands: Commands, mut d: ResMut<Descent>, mut hud: ResMut<Hud>, mut player: Single<&mut Player>, flying: Query<Entity, With<Flying>>) {
    for e in &flying {
        commands.entity(e).despawn();
    }
    if d.on {
        player.controlled = true;
        hud.mode.clear();
        if let Some(info) = d.info.take() {
            hud.info = info;
        }
    }
    let demoed = d.demoed;
    *d = Descent { demoed, ..default() };
}
