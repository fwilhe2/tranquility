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
//! `MOON_EARTHRISE=film` (`make earthrise CUT=long`) shows it as a film, in
//! real time: a title over black, the scene from 75:46 with the views
//! changed on cue (the black and white frame through the lens, the search
//! for colour film as the Earth crosses to the hatch window, the colour
//! photograph, turned slowly to the way it was printed), the crew's words
//! as subtitles, a closing card. `film,short` is the minute and a half
//! from the Earth coming into sight to the colour photograph. Recorded
//! (`TRANQUILITY_RECORD`), the video is as long as the film; **V** takes
//! the view back. The cabin is heard throughout (`cabin.rs`): its fans,
//! the jets of the roll, the camera.
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
    render::view::ColorGrading,
    ui::{BackgroundGradient, ColorStop, Gradient, LinearGradient, RadialGradient, RadialGradientShape, UiPosition},
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

/// When a magazine of colour film went on the camera: between Anders
/// asking for it and Lovell reading out its number. Not on the tape: a
/// guess, and the end of black and white through the lens.
const MAGAZINE: f64 = get(75, 48, 12);

/// The films: where each starts and ends, and the shots, each from its
/// time on. Both run in real time; the short one is the minute and a half
/// from the Earth coming into sight to the colour photograph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    Short,
    Long,
}

/// A shot: from when, through what, and whether the frame turns from as
/// shot toward as printed between two times.
struct Shot {
    at: f64,
    view: View,
    turn: Option<(f64, f64)>,
}

const fn shot(at: f64, view: View) -> Shot {
    Shot { at, view, turn: None }
}

/// The black and white frame through the lens; the search for colour film
/// as the Earth crosses from window 5 to the hatch window (Anders: "I
/// think we missed it"); the colour photograph through the lens, then the
/// frame turned slowly the way the world came to know it.
const LONG: &[Shot] = &[
    shot(0.0, View::Window),
    shot(get(75, 47, 41), View::Lens),
    shot(get(75, 47, 50), View::Window),
    shot(get(75, 48, 12), View::Hatch),
    Shot { at: get(75, 48, 36), view: View::Lens, turn: Some((get(75, 48, 43), get(75, 48, 56))) },
    shot(get(75, 48, 59), View::Hatch),
    shot(get(75, 49, 2), View::Lens),
    shot(get(75, 49, 9), View::Hatch),
    Shot { at: get(75, 50, 2), view: View::Lens, turn: Some((get(75, 50, 0), get(75, 50, 1))) },
];
const SHORT: &[Shot] = &[
    shot(0.0, View::Window),
    shot(get(75, 47, 41), View::Lens),
    shot(get(75, 47, 50), View::Window),
    shot(get(75, 48, 12), View::Hatch),
    Shot { at: get(75, 48, 36), view: View::Lens, turn: Some((get(75, 48, 43), get(75, 48, 57))) },
];

impl Cut {
    fn span(self) -> (f64, f64) {
        match self {
            Cut::Short => (get(75, 47, 22), get(75, 49, 2)),
            Cut::Long => (FROM, UNTIL),
        }
    }

    fn shots(self) -> &'static [Shot] {
        match self {
            Cut::Short => SHORT,
            Cut::Long => LONG,
        }
    }

    /// Seconds of the whole film: title, scene, fade and closing card.
    pub fn length(self) -> f64 {
        let (from, until) = self.span();
        TITLE + (until - from) + FADE + CLOSING
    }
}

/// The title over black before the scene, the fade at its end, and the
/// closing card after it, s.
const TITLE: f64 = 7.0;
const FADE: f64 = 3.0;
const CLOSING: f64 = 9.0;

pub struct EarthrisePlugin;

impl Plugin for EarthrisePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Earthrise>()
            .add_systems(Startup, setup)
            .add_systems(Update, (dress, toggle, direct, fly, (frame, cabin)).chain().after(Walking));
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
    /// How far the frame is turned toward as printed, degrees.
    turn: f32,
    /// The head turned from where the view looks, radians.
    head: Vec2,
    /// Where the free view looks: yaw and pitch.
    free: Vec2,
    /// The words, frames and jets already gone by.
    said: usize,
    shot: usize,
    fired: usize,
    /// The jets' firings: when, where (−1 left to 1 right), how hard.
    jets: Vec<(f64, f32, f32)>,
    /// The app's lines, put aside.
    put_aside: Option<(String, String)>,
    /// The shutter, for the blink through the lens: 1 when it goes.
    blink: f32,
    /// Whether the ground has come in since the start.
    ground: bool,
    /// A film being shown, and seconds into it.
    film: Option<Cut>,
    film_time: f64,
    /// The last frame taken, for the caption, and when.
    taken: Option<(usize, f64)>,
}

impl Earthrise {
    /// A clock jumped to: what has been said, shot and fired by then goes
    /// unheard.
    fn catch_up(&mut self) {
        let clock = self.clock;
        self.said = WORDS.iter().take_while(|w| w.0 < clock - 6.0).count();
        self.shot = FRAMES.iter().take_while(|f| f.0 < clock).count();
        self.fired = self.jets.iter().take_while(|j| j.0 < clock).count();
    }
}

/// The window's frame: the cabin round it, a rim, and the glass.
#[derive(Component)]
struct Frame;
#[derive(Component)]
struct Surround;
#[derive(Component)]
struct Rim;

/// The film's black, its title and closing card, and its lines.
#[derive(Component)]
struct Curtain;
#[derive(Component)]
struct Card;
#[derive(Component)]
struct CardTitle;
#[derive(Component)]
struct Subtitle;
#[derive(Component)]
struct Caption;

fn setup(mut commands: Commands) {
    // Shown in the scene's window views only. The surround reaches well
    // past the screen, so its outer corners never show.
    commands
        .spawn((
            Frame,
            Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), bottom: px(0), ..default() },
            Visibility::Hidden,
            GlobalZIndex(-1),
        ))
        .with_children(|frame| {
            frame.spawn((Surround, Node { position_type: PositionType::Absolute, ..default() }, BorderColor::all(Color::srgb(0.05, 0.05, 0.052))));
            frame.spawn((
                Rim,
                Node { position_type: PositionType::Absolute, border: UiRect::all(px(3)), ..default() },
                BorderColor::all(Color::srgba(0.42, 0.42, 0.4, 0.55)),
                // Darker toward the glass's edges, as a lens and a thick
                // pane are, and a faint sheen across it.
                BackgroundGradient(vec![
                    Gradient::Radial(RadialGradient::new(
                        UiPosition::CENTER,
                        RadialGradientShape::FarthestCorner,
                        vec![ColorStop::percent(Color::NONE, 55.0), ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.45), 100.0)],
                    )),
                    Gradient::Linear(LinearGradient::new(
                        LinearGradient::TO_BOTTOM_RIGHT,
                        vec![
                            ColorStop::percent(Color::NONE, 18.0),
                            ColorStop::percent(Color::srgba(0.75, 0.8, 0.9, 0.01), 30.0),
                            ColorStop::percent(Color::NONE, 42.0),
                        ],
                    )),
                ]),
            ));
        });
    commands.spawn((
        Curtain,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        Visibility::Hidden,
        GlobalZIndex(10),
    ));
}

/// The film's text, once the overlay's typeface is there.
fn dress(mut commands: Commands, font: Option<Res<tranquility::hud::HudFont>>, curtain: Single<Entity, With<Curtain>>, mut done: Local<bool>) {
    let Some(font) = font else { return };
    if *done {
        return;
    }
    *done = true;
    let face = |size: f32| TextFont { font: font.0.clone().into(), font_size: FontSize::Px(size), ..default() };
    commands.entity(*curtain).with_children(|card| {
        card.spawn((CardTitle, Text::new(""), face(54.0), TextColor(Color::NONE), Node { margin: UiRect::bottom(px(28)), ..default() }));
        card.spawn((Card, Text::new(""), face(21.0), TextColor(Color::NONE), TextLayout::justify(Justify::Center)));
    });
    let shadow = TextShadow { offset: Vec2::splat(1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.8) };
    commands.spawn((
        Subtitle,
        Node { position_type: PositionType::Absolute, left: percent(10), right: percent(10), bottom: percent(6), justify_content: JustifyContent::Center, ..default() },
        Visibility::Hidden,
        GlobalZIndex(5),
        children![(Text::new(""), face(24.0), TextColor(Color::NONE), shadow, TextLayout::justify(Justify::Center))],
    ));
    commands.spawn((
        Caption,
        Text::new(""),
        face(14.0),
        TextColor(Color::NONE),
        shadow,
        Node { position_type: PositionType::Absolute, left: percent(3.5), top: percent(4), ..default() },
        Visibility::Hidden,
        GlobalZIndex(5),
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

/// What `MOON_EARTHRISE` asks for.
#[derive(Clone, Copy)]
struct Asked {
    clock: f64,
    view: View,
    printed: bool,
    still: bool,
    film: Option<Cut>,
}

/// `MOON_EARTHRISE=75:48:30` (or `=1`): the time to start at; and after
/// a comma what to look through (`hatch`, `lens`, `free`) and `printed`,
/// and `still` (the clock stopped), for pictures:
/// `MOON_EARTHRISE=75:48:39,lens,printed,still`. `MOON_EARTHRISE=film`
/// (or `film,short`) shows a film of it, directed, in real time.
fn asked() -> Option<Asked> {
    let v = options::get("MOON_EARTHRISE")?;
    let mut words = v.split(',');
    let first = words.next().unwrap_or("").trim();
    let parts: Vec<f64> = first.split(':').filter_map(|p| p.trim().parse().ok()).collect();
    let mut a = Asked {
        clock: match parts[..] {
            [h, m, s] => h * 3600.0 + m * 60.0 + s,
            _ => FROM,
        },
        view: View::Window,
        printed: false,
        still: false,
        film: (first == "film").then_some(Cut::Long),
    };
    for w in words {
        match w.trim() {
            "hatch" => a.view = View::Hatch,
            "lens" => a.view = View::Lens,
            "free" => a.view = View::Free,
            "printed" => a.printed = true,
            "still" => a.still = true,
            "short" if a.film.is_some() => a.film = Some(Cut::Short),
            _ => {}
        }
    }
    Some(a)
}

/// The asked film's length, for a recording of it (natively only).
#[cfg(not(target_arch = "wasm32"))]
pub fn film_length() -> Option<f64> {
    asked()?.film.map(Cut::length)
}

/// The jets that fire, from a little before the scene to a little after:
/// a run of pulses as the roll starts and as it stops (a pair across the
/// spacecraft each time, which turns it without pushing it), and a single
/// firing now and then as the autopilot holds the attitude in its
/// deadband. Placed by a hash, the same every time.
fn jets() -> Vec<(f64, f32, f32)> {
    let unit = |k: i32, salt: u32| tranquility::terrain::unit(k, 0, salt) as f64;
    let mut out = Vec::new();
    let burst = |from: f64, salt: u32, out: &mut Vec<(f64, f32, f32)>| {
        let mut t = from;
        let mut k = 0;
        while t < from + EASE {
            // Two jets across the spacecraft, heard as one bang: each
            // about half as loud as a lone firing.
            let s = 0.5 + 0.2 * unit(k, salt) as f32;
            out.push((t, -0.6, s));
            out.push((t + 0.004, 0.6, s));
            t += 0.32 + 0.25 * unit(k, salt + 1);
            k += 1;
        }
    };
    burst(ROLL_FROM, 11, &mut out);
    burst(ROLL_FROM + ROLL / ROLL_RATE, 13, &mut out);
    let mut t = FROM - 130.0;
    let mut k = 0;
    while t < UNTIL + 130.0 {
        t += 7.0 + 14.0 * unit(k, 17);
        out.push((t, (2.0 * unit(k, 18) - 1.0) as f32, 0.45 + 0.4 * unit(k, 19) as f32));
        k += 1;
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
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
        if e.film.is_some() {
            hud.hidden = false;
        }
        e.film = None;
        crate::arrive(&mut commands, &mut current, moon.at(crate::first_site(moon)));
        return;
    }
    let Some(body) = scene(moon) else { return };
    crate::arrive(&mut commands, &mut current, body);
    let a = from.unwrap_or(Asked { clock: FROM, view: View::Window, printed: false, still: false, film: None });
    *e = Earthrise {
        on: true,
        clock: a.film.map_or(a.clock, |cut| cut.span().0 - TITLE),
        view: a.view,
        published: a.printed,
        turn: if a.printed { 95.0 } else { 0.0 },
        paused: a.still,
        pace: 1.0,
        put_aside: Some((hud.info.clone(), hud.keys.clone())),
        free: Vec2::new(0.0, -0.3),
        jets: jets(),
        film: a.film,
        ..default()
    };
    e.catch_up();
    player.controlled = false;
    player.flying = true;
    if let Some(cut) = a.film {
        hud.hidden = true;
        info!("earthrise: the {cut:?} film, {:.0} s", cut.length());
    } else {
        hud.say("Apollo 8, 24 December 1968: the fourth orbit");
    }
}

/// How long the roll eases in and out, s.
const EASE: f64 = 5.0;

/// How far Borman's roll has turned the spacecraft at a time, degrees:
/// easing in, steady, easing out.
fn rolled(clock: f64) -> f64 {
    let ease = |t: f64| if t <= 0.0 { 0.0 } else if t < EASE { t * t / (2.0 * EASE) } else { t - EASE / 2.0 };
    let t = clock - ROLL_FROM;
    let coast = ROLL / ROLL_RATE;
    (ROLL_RATE * (ease(t) - ease(t - coast))).min(ROLL)
}

/// The spacecraft's attitude at a time: the nose straight down, rolled
/// about it. Returns its axes in the frame of the ground under it: X out
/// of the nose, Y toward window 5, Z toward the crew's heads.
fn attitude(clock: f64, along: Vec3) -> (Vec3, Vec3, Vec3) {
    let x = Vec3::NEG_Y;
    // Rolling right is a right-handed turn about the nose.
    let y = Quat::from_axis_angle(x, (rolled(clock) - ROLL_START).to_radians() as f32) * along;
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

/// The film: what to look through, where the head turns and how the frame
/// is turned, from the shots; the head drifting toward the Earth when it
/// is in sight, the way an eye goes to it.
fn direct(mut e: ResMut<Earthrise>, mut last: Local<Option<View>>) {
    let Some(cut) = e.film.filter(|_| e.on) else {
        *last = None;
        return;
    };
    let clock = e.clock;
    let Some(shot) = cut.shots().iter().rev().find(|s| s.at <= clock) else { return };
    let cut_to = *last != Some(shot.view);
    e.view = shot.view;
    *last = Some(shot.view);
    if cut_to {
        e.head = Vec2::ZERO;
    }
    e.turn = shot.turn.map_or(0.0, |(from, to)| 95.0 * tranquility::noise::smoothstep(from as f32, to as f32, clock as f32));
}

#[allow(clippy::too_many_arguments)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    current: Res<Current>,
    chunks: Res<tranquility::terrain::Chunks>,
    mut e: ResMut<Earthrise>,
    mut hud: ResMut<Hud>,
    camera: Single<(Entity, &mut Transform, &mut Projection, Option<&mut ColorGrading>), With<Player>>,
) {
    if !e.on {
        return;
    }
    let Some(site) = sites::by_id("pasteur") else { return };
    let dt = time.delta_secs_f64().min(0.1);
    let film = e.film.is_some();

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
        if film {
            e.film_time += jump;
        }
        e.catch_up();
    }
    // The clock waits for the ground to come in first.
    if chunks.is_settled() {
        e.ground = true;
    }
    if !e.paused && e.ground {
        e.clock += dt * e.pace;
        if film {
            e.film_time += dt * e.pace;
        }
    }
    // A film's clock stops where its scene ends.
    if let Some(cut) = e.film {
        e.clock = e.clock.min(cut.span().1);
    }
    if keys.just_pressed(KeyCode::KeyV) {
        // Taking the view over ends the film.
        if film {
            e.film = None;
            hud.hidden = false;
        }
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
    if e.film.is_none() {
        e.turn = if e.published { 95.0 } else { 0.0 };
    }

    // Where the spacecraft is.
    let radius = current.0.radius;
    let (under, along) = track(site, radius);
    let here = under + along * (e.clock - PHOTO) * GROUND_SPEED;
    let (entity, mut eye, mut projection, grading) = camera.into_inner();
    eye.translation = origin.render(here, ALTITUDE);
    let along3 = Vec3::new(along.x as f32, 0.0, along.y as f32);
    let axes = attitude(e.clock, along3);

    // The Earth from here: the sky turned for the spacecraft's place.
    let back = sky::turn(world.terrain.radius, under).inverse();
    let to_earth = sky::turn(world.terrain.radius, here) * (back * TO_EARTH);
    let (nadir, _, heads) = axes;
    let dip = (radius / (radius + ALTITUDE)).acos().to_degrees();
    let above = to_earth.y.asin().to_degrees() + dip;
    // In a film the lens keeps the horizon in the frame as the Earth
    // climbs: aimed below it by as much as it has gone over 4.5°.
    let aim = if film {
        let below = (above - 4.5).max(0.0).to_radians();
        Quat::from_axis_angle(to_earth.cross(nadir).normalize(), below) * to_earth
    } else {
        to_earth
    };
    let (rotation, fov) = match e.view {
        View::Window => (looking(window(axes, WINDOW_5), heads), EYE_FOV),
        View::Hatch => (looking(window(axes, HATCH), heads), EYE_FOV),
        // Anders aims at the Earth, lying face down: the Moon below is on
        // his right, the horizon upright in the frame.
        View::Lens => {
            let right = (nadir - aim * nadir.dot(aim)).normalize();
            (looking(aim, right.cross(aim)), LENS_FOV)
        }
        View::Free => (Quat::from_euler(EulerRot::YXZ, e.free.x + along3.x.atan2(along3.z) + std::f32::consts::PI, e.free.y, 0.0), 70.0),
    };

    // The head: turned by the mouse, or in a film drawn toward the Earth
    // once it is up and near the window's middle.
    let reach = if e.view == View::Free { Vec2::new(f32::INFINITY, 1.5) } else { Vec2::new(1.0, 0.8) };
    if film {
        if matches!(e.view, View::Window | View::Hatch) {
            let local = rotation.inverse() * to_earth;
            let (yaw, pitch) = ((-local.x).atan2(-local.z), local.y.clamp(-1.0, 1.0).asin());
            let off = local.z.clamp(-1.0, 1.0).acos().to_degrees();
            let up = tranquility::noise::smoothstep(-1.5, 0.5, above);
            let near = 1.0 - tranquility::noise::smoothstep(30.0, 60.0, 180.0 - off);
            let want = (Vec2::new(yaw, pitch) * 0.7 * up * near).clamp(-reach, reach);
            let k = 1.0 - (-dt as f32 / 2.5).exp();
            let head = e.head;
            e.head = head + (want - head) * k;
        }
    } else {
        let turn = motion.delta * -0.0022;
        if e.view == View::Free {
            e.free.x += turn.x;
            e.free.y = (e.free.y + turn.y).clamp(-reach.y, reach.y);
        } else {
            let scale = if e.view == View::Lens { LENS_FOV / EYE_FOV } else { 1.0 };
            e.head = (e.head + turn * scale).clamp(-reach, reach);
        }
    }
    // Weightless, nothing holds still: the eye drifts a little at the
    // window, and the long lens in the hands more, magnified.
    let t = e.clock as f32;
    let sway = |a: f32, b: f32| Vec2::new((a * t).sin() + 0.6 * (2.3 * a * t + 1.0).sin(), (b * t + 2.0).sin() + 0.5 * (1.9 * b * t).sin());
    let drift = match e.view {
        View::Lens => sway(0.9, 0.7) * 0.0009,
        View::Window | View::Hatch => sway(0.11, 0.08) * 0.006,
        View::Free => Vec2::ZERO,
    };
    let head = Quat::from_euler(EulerRot::YXZ, e.head.x + drift.x, e.head.y + drift.y, 0.0);
    // As printed: the frame turned 95° clockwise.
    let print = if e.view != View::Free { Quat::from_rotation_z(e.turn.to_radians()) } else { Quat::IDENTITY };
    eye.rotation = rotation * head * print;
    if let Projection::Perspective(p) = &mut *projection
        && (p.fov - fov.to_radians()).abs() > 1e-4
    {
        p.fov = fov.to_radians();
    }
    // Through the lens before the colour magazine went on: black and white.
    let saturation = if e.view == View::Lens && e.clock < MAGAZINE { 0.0 } else { 1.0 };
    match grading {
        Some(mut g) if g.global.post_saturation != saturation => g.global.post_saturation = saturation,
        Some(_) => {}
        None if saturation != 1.0 => {
            let mut g = ColorGrading::default();
            g.global.post_saturation = saturation;
            commands.entity(entity).insert(g);
        }
        None => {}
    }

    // The words, the shutter, the clock.
    while e.said < WORDS.len() && WORDS[e.said].0 <= e.clock {
        e.said += 1;
    }
    while e.shot < FRAMES.len() && FRAMES[e.shot].0 <= e.clock {
        if !film {
            hud.say(format!("click: {}", FRAMES[e.shot].1));
        }
        e.taken = Some((e.shot, e.clock));
        e.blink = 1.0;
        e.shot += 1;
    }
    while e.fired < e.jets.len() && e.jets[e.fired].0 <= e.clock {
        e.fired += 1;
    }
    e.blink = (e.blink - 3.0 * time.delta_secs()).max(0.0);

    let lat_lon = site.lat_lon(radius, (here.x, here.y));
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
    let mode = format!("APOLLO 8 · {}", view_name(e.view));
    if hud.mode != mode {
        hud.mode = mode;
    }
    let help = "E: end   V: view   O: as printed   Space: pause   ←/→: 15 s   ↑/↓: pace   mouse: look";
    if hud.keys != help {
        hud.keys = help.to_string();
    }
}

fn view_name(view: View) -> &'static str {
    match view {
        View::Window => "Anders' window (5)",
        View::Hatch => "Lovell's hatch window (3)",
        View::Lens => "Hasselblad, 250 mm",
        View::Free => "looking round",
    }
}

/// Everything drawn over the picture: the window's frame, the blink of the
/// shutter through the lens, and in a film its title, lines and closing
/// card.
#[allow(clippy::too_many_arguments)]
fn frame(
    e: Res<Earthrise>,
    camera: Single<&Camera, With<Player>>,
    scale: Res<bevy::ui::UiScale>,
    mut shown: Single<&mut Visibility, (With<Frame>, Without<Curtain>, Without<Subtitle>, Without<Caption>)>,
    mut surround: Single<&mut Node, (With<Surround>, Without<Rim>, Without<Curtain>, Without<Subtitle>, Without<Caption>)>,
    mut rim: Single<&mut Node, (With<Rim>, Without<Surround>, Without<Curtain>, Without<Subtitle>, Without<Caption>)>,
    curtain: Single<(&mut BackgroundColor, &mut Visibility), (With<Curtain>, Without<Subtitle>, Without<Caption>)>,
    mut card: Query<(&mut Text, &mut TextColor, Has<CardTitle>), (Or<(With<Card>, With<CardTitle>)>, Without<Subtitle>, Without<Caption>)>,
    subtitle: Option<Single<(&mut Visibility, &Children), (With<Subtitle>, Without<Caption>)>>,
    mut lines: Query<(&mut Text, &mut TextColor), (Without<Card>, Without<CardTitle>, Without<Caption>)>,
    caption: Option<Single<(&mut Text, &mut TextColor, &mut Visibility), (With<Caption>, Without<Card>, Without<CardTitle>)>>,
) {
    let window = e.on && matches!(e.view, View::Window | View::Hatch);
    shown.set_if_neq(if window { Visibility::Inherited } else { Visibility::Hidden });
    if window {
        // The opening: window 5 a rounded square a little wider than high,
        // the hatch window round; set in the UI's own pixels.
        let size = camera.logical_viewport_size().unwrap_or(Vec2::new(1280.0, 720.0)) / scale.0;
        let h = 0.86 * size.y;
        let (w, r) = if e.view == View::Hatch { (h, h / 2.0) } else { ((1.18 * h).min(0.96 * size.x), 0.16 * h) };
        let (x, y) = ((size.x - w) / 2.0, (size.y - h) / 2.0);
        // As wide a border on every side, so the inside's corners keep
        // the radius asked for (the outer radius less the border).
        let past = 2.0 * size.x.max(size.y);
        let want = Node {
            position_type: PositionType::Absolute,
            left: px(x - past),
            top: px(y - past),
            width: px(w + 2.0 * past),
            height: px(h + 2.0 * past),
            border: UiRect::all(px(past)),
            border_radius: BorderRadius::all(px(past + r)),
            ..default()
        };
        if **surround != want {
            **surround = want;
        }
        let want = Node {
            position_type: PositionType::Absolute,
            left: px(x - 2.0),
            top: px(y - 2.0),
            width: px(w + 4.0),
            height: px(h + 4.0),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(r + 2.0)),
            ..default()
        };
        if **rim != want {
            **rim = want;
        }
    }

    // The black: the shutter's blink through the lens, the film's ends.
    let (mut black, mut curtain_shown) = curtain.into_inner();
    let mut dark = if e.view == View::Lens { e.blink.min(0.6) / 0.6 } else { 0.0 };
    let mut text = ("", String::new(), 0.0);
    if let (true, Some(cut)) = (e.on, e.film) {
        let f = e.film_time;
        let (from, until) = cut.span();
        let end = TITLE + (until - from);
        let ramp = |a: f64, b: f64| ((f - a) / (b - a)).clamp(0.0, 1.0) as f32;
        dark = dark.max(1.0 - ramp(TITLE - 2.5, TITLE)).max(ramp(end, end + FADE));
        let card_in = |a: f64, hold: f64| ramp(a, a + 1.2) * (1.0 - ramp(a + hold, a + hold + 1.2));
        if f < TITLE {
            text = (
                "APOLLO 8",
                "24 December 1968 · the fourth orbit of the Moon · 112 km over the far side\n\nFrank Borman   Jim Lovell   Bill Anders".to_string(),
                card_in(0.6, TITLE - 3.4),
            );
        } else if f > end {
            text = (
                "EARTHRISE",
                "AS08-14-2383, taken by Bill Anders at 16:39:39 UT\n\n\
                 The crew's words: the onboard voice transcript. The Sun and the Earth: JPL Horizons.\n\
                 The Moon: LOLA and Clementine. The Earth: Blue Marble, under invented clouds. All public domain."
                    .to_string(),
                card_in(end + FADE - 0.5, CLOSING - 2.0),
            );
        }
    }
    let colour = Color::srgba(0.0, 0.0, 0.0, dark);
    if black.0 != colour {
        black.0 = colour;
    }
    curtain_shown.set_if_neq(if dark > 0.0 || text.2 > 0.0 { Visibility::Inherited } else { Visibility::Hidden });
    for (mut t, mut c, title) in &mut card {
        let want = if title { text.0 } else { text.1.as_str() };
        if t.0 != want {
            t.0 = want.to_string();
        }
        let want = Color::srgba(0.92, 0.9, 0.86, if title { text.2 } else { 0.8 * text.2 });
        if c.0 != want {
            c.0 = want;
        }
    }

    // A film's lines: who says what, a moment after and a while.
    let film = e.on && e.film.is_some();
    if let Some(subtitle) = subtitle {
        let (mut visibility, children) = subtitle.into_inner();
        let line = e.said.checked_sub(1).map(|k| WORDS[k]).filter(|w| e.clock - w.0 < 5.5);
        visibility.set_if_neq(if film && line.is_some() { Visibility::Inherited } else { Visibility::Hidden });
        if let Some((at, who, said)) = line
            && let Ok((mut t, mut c)) = lines.get_mut(children[0])
        {
            let alpha = ((e.clock - at) / 0.25).min((5.5 - (e.clock - at)) / 0.5).clamp(0.0, 1.0) as f32;
            let want = format!("{}: {said}", who.to_uppercase());
            if t.0 != want {
                t.0 = want;
            }
            let want = Color::srgba(1.0, 0.97, 0.9, alpha);
            if c.0 != want {
                c.0 = want;
            }
        }
    }
    if let Some(caption) = caption {
        let (mut t, mut c, mut visibility) = caption.into_inner();
        visibility.set_if_neq(if film { Visibility::Inherited } else { Visibility::Hidden });
        if film {
            let ut = (e.clock + LAUNCH_ON_THE_24TH).rem_euclid(86_400.0).floor() as u32;
            let g = e.clock.floor() as u32;
            let mut want = format!(
                "24 DECEMBER 1968   {:02}:{:02}:{:02} UT   GET {:03}:{:02}:{:02}\n{}",
                ut / 3600,
                ut / 60 % 60,
                ut % 60,
                g / 3600,
                g / 60 % 60,
                g % 60,
                view_name(e.view)
            );
            if let Some((k, at)) = e.taken
                && e.clock - at < 6.0
            {
                want.push_str(&format!("\nclick: {}", FRAMES[k].1));
            }
            if t.0 != want {
                t.0 = want;
            }
            let alpha = if e.film_time < TITLE { 0.0 } else { 0.7 };
            let want = Color::srgba(1.0, 1.0, 1.0, alpha);
            if c.0 != want {
                c.0 = want;
            }
        }
    }
}

/// What the cabin sounds: aboard or not, and the jets and the camera as
/// the clock passes them.
fn cabin(e: Res<Earthrise>, line: Option<Res<crate::sound::CabinLine>>, mut was: Local<(f64, usize, usize, bool)>) {
    use crate::cabin::Clunk;
    let Some(line) = line else { return };
    let Ok(mut aboard) = line.0.lock() else { return };
    // In a film the cabin fades in and out with the picture.
    let level = match (e.on, e.film) {
        (false, _) => 0.0,
        (true, None) => 1.0,
        (true, Some(cut)) => {
            let (from, until) = cut.span();
            let end = TITLE + (until - from);
            let f = e.film_time;
            ((f - 1.0) / (TITLE - 1.0)).clamp(0.0, 1.0).min(1.0 - (f - end) / (FADE + 2.0)).clamp(0.0, 1.0) as f32
        }
    };
    aboard.level = level;
    let (clock, fired, shot, on) = *was;
    // Only going forward, at about the pace of life: no barrage on a jump.
    let onward = on && e.on && e.clock > clock && e.clock - clock < 0.5;
    if onward {
        for &(_, pan, strength) in &e.jets[fired.min(e.fired)..e.fired] {
            aboard.clunks.push(Clunk::Jet { pan, strength });
        }
        for _ in shot.min(e.shot)..e.shot {
            aboard.clunks.push(Clunk::Shutter);
        }
        if clock < MAGAZINE && e.clock >= MAGAZINE {
            aboard.clunks.push(Clunk::Magazine);
        }
    }
    *was = (e.clock, e.fired, e.shot, e.on);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tranquility::sound::{RATE, Voice};

    use super::*;
    use crate::cabin::{Aboard, Cabin, Clunk};

    /// The loudest stretch of the scene, the run of jets as the roll
    /// starts, stays under full scale with room for the encoder: AAC
    /// overshoots a signal driven to the limit by a decibel or two.
    #[test]
    fn the_roll_s_jets_leave_headroom() {
        let line = Arc::new(Mutex::new(Aboard { level: 1.0, ..Default::default() }));
        let mut cabin = Cabin::new(line.clone());
        let jets = jets();
        let mut next = 0;
        let start = ROLL_FROM - 2.0;
        let mut peak = 0.0f32;
        for i in 0..((EASE + 4.0) * RATE as f64) as usize {
            let t = start + i as f64 / RATE as f64;
            while next < jets.len() && jets[next].0 <= t {
                let (at, pan, strength) = jets[next];
                if at >= start {
                    line.lock().unwrap().clunks.push(Clunk::Jet { pan, strength });
                }
                next += 1;
            }
            // The cabin reads its line every 256 samples; push in time.
            let [l, r] = cabin.frame();
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak > 0.3 && peak < 0.8, "peak {peak}");
    }
}
