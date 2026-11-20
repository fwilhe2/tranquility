//! Earthrise: Apollo 8's fourth orbit of the Moon, 24 December 1968, the
//! three minutes in which the crew saw the Earth come up over the lunar
//! horizon and Bill Anders photographed it.
//!
//! **E** starts it (or `MOON_EARTHRISE=1`, or `MOON_EARTHRISE=75:48:30` to
//! start at that ground elapsed time, `…,lens,printed` through the lens and
//! turned as printed: see `asked`); **E** again ends it. **V** changes
//! the view: out of window 5 as Anders saw it, out of the hatch window as
//! Lovell did, through the Hasselblad's
//! 250 mm lens, or looking round freely. **O** turns the picture the way
//! it was printed (the frame was shot with the horizon upright, and turned
//! a quarter for publication). **Space** pauses, the arrows go back and
//! forth by 15 s and change the pace. The mouse turns the head.
//!
//! Everything in it is a number from the record:
//!
//! - the time: the colour photograph AS08-14-2383 at 75:48:39 ground
//!   elapsed time, 16:39:39 UT (the onboard tape's timing, from the 2013
//!   reconstruction by NASA's Scientific Visualization Studio);
//! - the orbit: 112 km up, 12° to the equator and retrograde, so going
//!   west; 128.7 minutes round;
//! - where the Sun and the Earth stood, how the Earth was turned and lit:
//!   JPL's Horizons (DE441), worked out by `tools/earthrise.py`, which
//!   also puts the spacecraft over 11.97° S, 112.6° E, where the view
//!   toward the Earth, 5° above the horizon, crosses Pasteur;
//! - the attitude: nose straight down, and Frank Borman's roll to the
//!   right that brought the Earth into window 5, then the hatch window;
//! - the crew's words: the onboard voice transcript.
//!
//! What is not known is made up plainly: the roll's rate (a steady 1.1°/s,
//! which brings the Earth into window 5 and the hatch window about when
//! the tape has the crew see it there) and the Earth's clouds. Sources:
//! `data/earthrise/SOURCES.md`.

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    math::DVec2,
};
use tranquility::{prelude::*, sky};

use crate::{Catalogue, Current, heavens::{Companion, Map}, options, sites};

/// Ground elapsed time, seconds, from `hh:mm:ss`.
const fn get(h: u32, m: u32, s: u32) -> f64 {
    (h * 3600 + m * 60 + s) as f64
}

/// Launch, 21 December 1968 12:51:00 UT, as seconds into 24 December.
const LAUNCH_ON_THE_24TH: f64 = -(2.0 * 86_400.0 + 11.0 * 3600.0 + 9.0 * 60.0);

/// The colour photograph, AS08-14-2383.
const PHOTO: f64 = get(75, 48, 39);
/// Where the scene starts and ends, unless asked otherwise.
const FROM: f64 = get(75, 46, 0);
const UNTIL: f64 = get(75, 50, 30);

/// From `tools/earthrise.py`: over the ground under the spacecraft at the
/// photograph (x east, y up, z south), the way to the Sun, to the Earth
/// and the Earth's north pole; the point on the Earth facing the Moon
/// (latitude, east longitude) and the Earth's angular radius, degrees.
const OVER: (f64, f64) = (-11.965, 112.625);
const HEADING: f64 = 269.08;
const TO_SUN: Vec3 = Vec3::new(0.137_297, 0.974_182, -0.179_216);
const TO_EARTH: Vec3 = Vec3::new(-0.965_716, -0.259_532, 0.005_969);
const EARTH_POLE: Vec3 = Vec3::new(-0.031_313, -0.556_203, -0.830_456);
const EARTH_FACE: (f32, f32) = (-9.833, 359.237);
const EARTH_RADIUS: f32 = 0.9666;

/// The orbit: height over the mean radius, and speed over the ground, m/s
/// (1,505 m/s at 112 km, 128.7 minutes round).
const ALTITUDE: f32 = 112_000.0;
const GROUND_SPEED: f64 = 1413.7;

/// Borman's roll: from when, how fast (°/s) and how far.
const ROLL_FROM: f64 = get(75, 46, 30);
const ROLL_RATE: f64 = 1.1;
const ROLL: f64 = 180.0;
/// Where window 5 looked when the roll began, degrees back from straight
/// ahead: it comes round to face along the track (and the Earth) at 76°,
/// a few seconds after Anders saw the Earth in it, and the hatch window,
/// 70° behind it, as Lovell framed it there and Anders took the colour
/// photographs.
const ROLL_START: f64 = 76.0;

/// The command module's windows: how far round the roll axis from window
/// 5 (degrees, toward the hatch), and every one 57° off the nose (the
/// cone's side). The rendezvous windows look nearly along the nose.
const WINDOW_5: f32 = 0.0;
const HATCH: f32 = -70.0;
const OFF_NOSE: f32 = 57.0;

/// The 250 mm lens on 56 mm square film.
const LENS_FOV: f32 = 12.8;
/// Anders' eye at the window.
const EYE_FOV: f32 = 62.0;

/// The crew's words from the onboard tape (NASA's transcript, as the
/// Apollo Flight Journal gives it), some shortened.
const WORDS: &[(f64, &str, &str)] = &[
    (get(75, 46, 27), "Borman", "Alright, we're going to roll."),
    (get(75, 46, 33), "Borman", "Ready? Set."),
    (get(75, 47, 30), "Anders", "Oh, my God! Look at that picture over there! Here's the Earth coming up. Wow, is that pretty!"),
    (get(75, 47, 37), "Borman", "Hey, don't take that, it's not scheduled."),
    (get(75, 47, 39), "Anders", "You got a color film, Jim?"),
    (get(75, 47, 46), "Anders", "Hand me that roll of color quick, will you..."),
    (get(75, 47, 48), "Lovell", "Oh man, that's great!"),
    (get(75, 47, 56), "Anders", "Just grab me a color. That color exterior."),
    (get(75, 48, 1), "Anders", "Hurry up!"),
    (get(75, 48, 10), "Lovell", "C 368."),
    (get(75, 48, 17), "Anders", "Well, I think we missed it."),
    (get(75, 48, 31), "Lovell", "Hey, I got it right here!"),
    (get(75, 48, 33), "Anders", "Let me get it out this window. It's a lot clearer."),
    (get(75, 48, 37), "Lovell", "Bill, I got it framed; it's very clear right here."),
    (get(75, 48, 42), "Borman", "Well, take several of them."),
    (get(75, 48, 44), "Anders", "Wait a minute, let's get the right setting, here now; just calm down."),
    (get(75, 48, 49), "Lovell", "Oh, that's a beautiful shot."),
    (get(75, 48, 54), "Lovell", "250 at f/11."),
    (get(75, 49, 8), "Lovell", "Now vary the exposure a little bit."),
    (get(75, 49, 9), "Anders", "I did. I took two of them."),
    (get(75, 49, 17), "Lovell", "Just take another one, Bill."),
];

/// The shutter: when, and the frame it made.
const FRAMES: &[(f64, &str)] = &[
    (get(75, 47, 44), "AS08-13-2329, black and white"),
    (PHOTO, "AS08-14-2383: Earthrise"),
    (get(75, 49, 4), "AS08-14-2384"),
];

pub struct EarthrisePlugin;

impl Plugin for EarthrisePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Earthrise>()
            .add_systems(Startup, setup)
            .add_systems(Update, (toggle, fly).chain().after(Walking));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum View {
    #[default]
    Window,
    Hatch,
    Lens,
    Free,
}

#[derive(Resource, Default)]
pub struct Earthrise {
    pub on: bool,
    /// Ground elapsed time, s.
    clock: f64,
    pace: f64,
    paused: bool,
    view: View,
    published: bool,
    /// The head turned from where the view looks, radians.
    head: Vec2,
    /// Where the free view looks: yaw and pitch.
    free: Vec2,
    /// The words and frames already shown.
    said: usize,
    shot: usize,
    /// The app's lines, put aside.
    put_aside: Option<(String, String)>,
    /// When the shutter last went, for the flash.
    flash: f32,
}

#[derive(Component)]
struct Frame;

#[derive(Component)]
struct Flash;

fn setup(mut commands: Commands) {
    // The window's frame, and the square of the film; shown in the
    // scene only.
    commands.spawn((
        Frame,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            border: UiRect::all(px(28)),
            border_radius: BorderRadius::all(px(90)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.035, 0.037, 0.04)),
        Visibility::Hidden,
        GlobalZIndex(-1),
    ));
    commands.spawn((
        Flash,
        Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), bottom: px(0), ..default() },
        BackgroundColor(Color::NONE),
        GlobalZIndex(-1),
    ));
}

/// The map of the Pasteur site: metres east and south of its centre for a
/// latitude and longitude (the inverse of `Site::lat_lon`).
fn place(site: &sites::Site, radius: f32, (lat, lon): (f64, f64)) -> DVec2 {
    let (lat0, lon0) = site.map().centre;
    let r = radius as f64;
    DVec2::new((lon - lon0).to_radians() * r * lat0.to_radians().cos(), -(lat - lat0).to_radians() * r)
}

/// Where the track runs on the map: the point under the spacecraft at the
/// photograph, and which way it goes.
fn track(site: &sites::Site, radius: f32) -> (DVec2, DVec2) {
    let h = HEADING.to_radians();
    (place(site, radius, OVER), DVec2::new(h.sin(), -h.cos()))
}

/// Degrees from north through east and above the level, of a direction.
fn az_el(d: Vec3) -> (f32, f32) {
    (d.x.atan2(-d.z).to_degrees().rem_euclid(360.0), d.y.clamp(-1.0, 1.0).asin().to_degrees())
}

/// The Moon as it was that day: the Pasteur site, with the Sun and the
/// Earth where they stood. The engine hangs the sky as it stands over the
/// map's centre; the numbers are for the point under the photograph, so
/// they are turned back from there.
fn scene(moon: &crate::body::Body) -> Option<crate::body::Body> {
    let site = sites::by_id("pasteur")?;
    let mut body = moon.at(Some(site));
    let (under, _) = track(site, body.radius);
    let back = sky::turn(Some(body.radius), under).inverse();
    body.sun = az_el(back * TO_SUN);
    // The Earth's globe: its pole up its axis, and the point facing the
    // Moon toward it. On the globe's mesh east longitude L lies at
    // 180° + L round from +x (see `heavens::paint`), north up.
    let (lat, lon) = (EARTH_FACE.0.to_radians(), (EARTH_FACE.1 + 180.0).to_radians());
    let facing = Vec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin());
    let frame = |a: Vec3, b: Vec3| {
        let b = (b - a * a.dot(b)).normalize();
        Mat3::from_cols(a, b, a.cross(b))
    };
    let globe = Quat::from_mat3(&(frame(EARTH_POLE, -TO_EARTH) * frame(Vec3::Y, facing).transpose()));
    let (z, y, x) = (back * globe).to_euler(EulerRot::ZYX);
    let (az, el) = az_el(back * TO_EARTH);
    body.companions = vec![Companion {
        turn: [z, y, x],
        name: "Earth",
        about: "384,000 km away, seen from Apollo 8 on 24 December 1968: South America and the Atlantic in sunlight, the evening terminator across Africa.",
        ..Companion::new(Map::Earth, EARTH_RADIUS, az, el)
    }];
    body.site = "Apollo 8, fourth orbit";
    body.note = "24 December 1968: Frank Borman, Jim Lovell and Bill Anders come round the Moon's far side and see the Earth rise.";
    Some(body)
}

/// `MOON_EARTHRISE=75:48:30` (or `=1`): the time to start at; and after
/// a comma what to look through (`hatch`, `lens`, `free`) and `printed`,
/// for pictures: `MOON_EARTHRISE=75:48:39,lens,printed`.
fn asked() -> Option<(f64, View, bool)> {
    let v = options::get("MOON_EARTHRISE")?;
    let mut words = v.split(',');
    let parts: Vec<f64> = words.next().unwrap_or("").split(':').filter_map(|p| p.trim().parse().ok()).collect();
    let clock = match parts[..] {
        [h, m, s] => h * 3600.0 + m * 60.0 + s,
        _ => FROM,
    };
    let (mut view, mut printed) = (View::Window, false);
    for w in words {
        match w.trim() {
            "hatch" => view = View::Hatch,
            "lens" => view = View::Lens,
            "free" => view = View::Free,
            "printed" => printed = true,
            _ => {}
        }
    }
    Some((clock, view, printed))
}

#[allow(clippy::too_many_arguments)]
fn toggle(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    catalogue: Res<Catalogue>,
    mut current: ResMut<Current>,
    mut e: ResMut<Earthrise>,
    mut hud: ResMut<Hud>,
    mut started: Local<bool>,
    player: Single<(&mut Player, &mut Projection)>,
    mut frame: Single<&mut Visibility, With<Frame>>,
) {
    let from = if !*started {
        *started = true;
        asked()
    } else {
        None
    };
    if from.is_none() && !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let (mut player, mut projection) = player.into_inner();
    let Some(moon) = catalogue.0.iter().find(|b| b.name == "Moon") else { return };
    if e.on {
        // Back to walking, on the Moon's first site.
        e.on = false;
        player.controlled = true;
        if let Projection::Perspective(p) = &mut *projection {
            p.fov = 70f32.to_radians();
        }
        **frame = Visibility::Hidden;
        if let Some((info, keys)) = e.put_aside.take() {
            (hud.info, hud.keys) = (info, keys);
        }
        hud.mode.clear();
        crate::arrive(&mut commands, &mut current, moon.at(crate::first_site(moon)));
        return;
    }
    let Some(body) = scene(moon) else { return };
    crate::arrive(&mut commands, &mut current, body);
    let (clock, view, published) = from.unwrap_or((FROM, View::Window, false));
    *e = Earthrise {
        on: true,
        clock,
        view,
        published,
        pace: 1.0,
        put_aside: Some((hud.info.clone(), hud.keys.clone())),
        free: Vec2::new(0.0, -0.3),
        ..default()
    };
    player.controlled = false;
    player.flying = true;
    hud.say("Apollo 8, 24 December 1968: the fourth orbit");
}

/// The spacecraft's attitude at a time: the nose straight down, rolled
/// about it. Returns its axes in the frame of the ground under it: X out
/// of the nose, Y toward window 5, Z toward the crew's heads.
fn attitude(clock: f64, along: Vec3) -> (Vec3, Vec3, Vec3) {
    let t = (clock - ROLL_FROM).max(0.0);
    // Easing into and out of the roll over five seconds.
    let ease = |t: f64| if t < 5.0 { t * t / 10.0 } else { t - 2.5 };
    let end = ROLL / ROLL_RATE;
    let rolled = if t < end + 5.0 { ease(t) * ROLL_RATE } else { ROLL }.min(ROLL);
    let x = Vec3::NEG_Y;
    // Rolling right is a right-handed turn about the nose.
    let y = Quat::from_axis_angle(x, (rolled - ROLL_START).to_radians() as f32) * along;
    (x, y, x.cross(y))
}

/// Which way a window looks, round the roll axis from window 5 (`HATCH`
/// for the hatch window).
fn window(axes: (Vec3, Vec3, Vec3), round: f32) -> Vec3 {
    let (x, y, z) = axes;
    let r = round.to_radians();
    let side = y * r.cos() + z * r.sin();
    let n = OFF_NOSE.to_radians();
    (x * n.cos() + side * n.sin()).normalize()
}

/// A rotation looking along `forward` with `up` up.
fn looking(forward: Vec3, up: Vec3) -> Quat {
    Transform::IDENTITY.looking_to(forward, up).rotation
}

#[allow(clippy::too_many_arguments)]
fn fly(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    current: Res<Current>,
    mut e: ResMut<Earthrise>,
    mut hud: ResMut<Hud>,
    camera: Single<(&mut Transform, &mut Projection), With<Player>>,
    mut frame: Single<&mut Visibility, With<Frame>>,
    mut flash: Single<&mut BackgroundColor, With<Flash>>,
) {
    if !e.on {
        return;
    }
    let Some(site) = sites::by_id("pasteur") else { return };
    let dt = time.delta_secs_f64().min(0.1);

    // The clock.
    if keys.just_pressed(KeyCode::Space) {
        e.paused = !e.paused;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        e.pace = (e.pace * 2.0).min(16.0);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        e.pace = (e.pace / 2.0).max(0.25);
    }
    let mut jump = 0.0;
    if keys.just_pressed(KeyCode::ArrowRight) {
        jump += 15.0;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        jump -= 15.0;
    }
    if jump != 0.0 {
        e.clock = (e.clock + jump).clamp(FROM - 120.0, UNTIL + 120.0);
        // Words and frames from the new time on.
        let clock = e.clock;
        e.said = WORDS.iter().take_while(|w| w.0 < clock - 6.0).count();
        e.shot = FRAMES.iter().take_while(|f| f.0 < clock).count();
    }
    if !e.paused {
        e.clock += dt * e.pace;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        e.view = match e.view {
            View::Window => View::Hatch,
            View::Hatch => View::Lens,
            View::Lens => View::Free,
            View::Free => View::Window,
        };
        e.head = Vec2::ZERO;
    }
    if keys.just_pressed(KeyCode::KeyO) {
        e.published = !e.published;
    }

    // Where the spacecraft is.
    let radius = current.0.radius;
    let (under, along) = track(site, radius);
    let here = under + along * (e.clock - PHOTO) * GROUND_SPEED;
    let (mut eye, mut projection) = camera.into_inner();
    eye.translation = origin.render(here, ALTITUDE);
    let along3 = Vec3::new(along.x as f32, 0.0, along.y as f32);
    let axes = attitude(e.clock, along3);

    // The head, turned by the mouse.
    let turn = motion.delta * -0.0022;
    let reach = if e.view == View::Free { Vec2::new(f32::INFINITY, 1.5) } else { Vec2::new(1.0, 0.8) };
    if e.view == View::Free {
        e.free.x += turn.x;
        e.free.y = (e.free.y + turn.y).clamp(-reach.y, reach.y);
    } else {
        let scale = if e.view == View::Lens { LENS_FOV / EYE_FOV } else { 1.0 };
        e.head = (e.head + turn * scale).clamp(-reach, reach);
    }

    // The Earth from here: the sky turned for the spacecraft's place.
    let back = sky::turn(world.terrain.radius, under).inverse();
    let to_earth = sky::turn(world.terrain.radius, here) * (back * TO_EARTH);
    let (nadir, _, heads) = axes;
    let (rotation, fov) = match e.view {
        View::Window => (looking(window(axes, WINDOW_5), heads), EYE_FOV),
        View::Hatch => (looking(window(axes, HATCH), heads), EYE_FOV),
        // Anders aims at the Earth, lying face down: the Moon below is on
        // his right, the horizon upright in the frame.
        View::Lens => {
            let right = (nadir - to_earth * nadir.dot(to_earth)).normalize();
            (looking(to_earth, right.cross(to_earth)), LENS_FOV)
        }
        View::Free => (Quat::from_euler(EulerRot::YXZ, e.free.x + along3.x.atan2(along3.z) + std::f32::consts::PI, e.free.y, 0.0), 70.0),
    };
    let head = Quat::from_euler(EulerRot::YXZ, e.head.x, e.head.y, 0.0);
    // As printed: the frame turned 95° clockwise.
    let print = if e.published && e.view != View::Free { Quat::from_rotation_z(95f32.to_radians()) } else { Quat::IDENTITY };
    eye.rotation = rotation * head * print;
    if let Projection::Perspective(p) = &mut *projection
        && (p.fov - fov.to_radians()).abs() > 1e-4
    {
        p.fov = fov.to_radians();
    }
    let frame_shown = if matches!(e.view, View::Window | View::Hatch) { Visibility::Inherited } else { Visibility::Hidden };
    if **frame != frame_shown {
        **frame = frame_shown;
    }

    // The words, the shutter, the clock.
    while e.said < WORDS.len() && WORDS[e.said].0 <= e.clock {
        e.said += 1;
    }
    while e.shot < FRAMES.len() && FRAMES[e.shot].0 <= e.clock {
        hud.say(format!("click: {}", FRAMES[e.shot].1));
        e.flash = 1.0;
        e.shot += 1;
    }
    e.flash = (e.flash - 4.0 * time.delta_secs()).max(0.0);
    let glow = Color::srgba(1.0, 1.0, 1.0, 0.5 * e.flash * e.flash);
    if flash.0 != glow {
        flash.0 = glow;
    }

    let lat_lon = site.lat_lon(radius, (here.x, here.y));
    let dip = (radius / (radius + ALTITUDE)).acos().to_degrees();
    let above = to_earth.y.asin().to_degrees() + dip;
    let ut = e.clock + LAUNCH_ON_THE_24TH;
    let hms = |s: f64| {
        let s = s.rem_euclid(86_400.0).floor() as u32;
        format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    };
    let get_s = e.clock.floor() as u32;
    let earth = if above > -EARTH_RADIUS { format!("the Earth {above:.1}° over the horizon") } else { "the Earth below the horizon ahead".to_string() };
    let words = e
        .said
        .checked_sub(1)
        .map(|k| WORDS[k])
        .filter(|w| e.clock - w.0 < 6.0)
        .map(|(_, who, line)| format!("\n{who}: \u{201c}{line}\u{201d}"))
        .unwrap_or_default();
    let info = format!(
        "24 December 1968 {} UT · GET {:03}:{:02}:{:02} · {:.0} km over {} · {}{}{}",
        hms(ut),
        get_s / 3600,
        get_s / 60 % 60,
        get_s % 60,
        ALTITUDE / 1000.0,
        sites::degrees(lat_lon),
        earth,
        if e.paused { " · paused" } else if e.pace != 1.0 { " · ×" } else { "" },
        words,
    );
    let info = if !e.paused && e.pace != 1.0 { info.replacen(" · ×", &format!(" · ×{}", e.pace), 1) } else { info };
    if hud.info != info {
        hud.info = info;
    }
    let view = match e.view {
        View::Window => "Anders' window (5)",
        View::Hatch => "Lovell's hatch window (3)",
        View::Lens => "Hasselblad, 250 mm",
        View::Free => "looking round",
    };
    let mode = format!("APOLLO 8 · {view}");
    if hud.mode != mode {
        hud.mode = mode;
    }
    let help = "E: end   V: view   O: as printed   Space: pause   ←/→: 15 s   ↑/↓: pace   mouse: look";
    if hud.keys != help {
        hud.keys = help.to_string();
    }
}
