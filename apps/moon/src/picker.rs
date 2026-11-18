//! The opening screen and the picker, after lunar-walk's: a title over the
//! body you are on, and a strip along the bottom of the window that is the
//! solar system. Its first level shows the planets at their true relative
//! size, the sun cropped at the left in the same scale; choosing a system
//! zooms in: its planet grows into the left edge, the neighbours fly out,
//! and its moons come in, at true size among themselves. Bodies that cannot
//! be walked here are shown, dimmed; the walkable ones are the catalogue's,
//! so a new body lights up here once it is in `bodies::all` (and in
//! `SYSTEMS` below).
//!
//! It is up while the mouse is free: at the start, until a click takes the
//! mouse (the title says so), and again after **Esc**, as a strip alone
//! with "click to resume". **← →** move along it, **↓** zooms into the
//! system under the cursor, **↑** out again, **Enter** goes there. A click
//! on a body goes there; "or watch it walk itself" starts the stroll.
//! Screenshot, benchmark and recording runs never show it (`MOON_PICKER=1`
//! does, for a picture of it; `MOON_PICKER=paused` as after Esc).

use std::collections::HashMap;

use bevy::window::{CursorGrabMode, CursorOptions};
use tranquility::{hud::HudFont, player::PointerOverUi, prelude::*};

use crate::{Catalogue, Current, stroll::Stroll};

pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Picker>()
            .init_resource::<Drawn>()
            .add_systems(PostStartup, setup)
            .add_systems(Update, (keys, click, follow, lay_out, paint, title).chain().before(Walking));
    }
}

/// A body on the strip: its radius, km, and the colour of its silhouette.
struct Disc {
    id: &'static str,
    name: &'static str,
    radius: f32,
    colour: u32,
    gas: bool,
}

const fn d(id: &'static str, name: &'static str, radius: f32, colour: u32) -> Disc {
    Disc { id, name, radius, colour, gas: false }
}

const fn gas(id: &'static str, name: &'static str, radius: f32, colour: u32) -> Disc {
    Disc { id, name, radius, colour, gas: true }
}

const SUN: f32 = 696_000.0;

const DISCS: &[Disc] = &[
    d("mercury", "Mercury", 2440.0, 0xa39d95),
    d("venus", "Venus", 6052.0, 0xdcc596),
    d("earth", "Earth", 6371.0, 0x7d9cc4),
    d("moon", "Moon", 1737.0, 0xbdb9b2),
    d("mars", "Mars", 3390.0, 0xc4744c),
    d("phobos", "Phobos", 11.3, 0x86796e),
    d("deimos", "Deimos", 6.2, 0xa3937f),
    d("vesta", "Vesta", 263.0, 0xa59d92),
    d("ceres", "Ceres", 470.0, 0x7d7a76),
    gas("jupiter", "Jupiter", 69_911.0, 0xcfae8c),
    d("io", "Io", 1822.0, 0xdcc65e),
    d("europa", "Europa", 1561.0, 0xd2c8b5),
    d("ganymede", "Ganymede", 2634.0, 0x9e958a),
    d("callisto", "Callisto", 2410.0, 0x756b62),
    gas("saturn", "Saturn", 58_232.0, 0xdcc794),
    d("mimas", "Mimas", 198.0, 0xbfbcb8),
    d("enceladus", "Enceladus", 252.0, 0xeef3f6),
    d("tethys", "Tethys", 531.0, 0xd8d8d5),
    d("dione", "Dione", 561.0, 0xcbc9c5),
    d("rhea", "Rhea", 764.0, 0xc6c3be),
    d("titan", "Titan", 2575.0, 0xd3a458),
    d("iapetus", "Iapetus", 735.0, 0x8d7d6b),
    gas("uranus", "Uranus", 25_362.0, 0xacd7de),
    d("miranda", "Miranda", 236.0, 0xb2b1ae),
    d("ariel", "Ariel", 579.0, 0xbfbebb),
    d("umbriel", "Umbriel", 585.0, 0x7e7d7b),
    d("titania", "Titania", 789.0, 0xb5aea7),
    d("oberon", "Oberon", 761.0, 0xa59c94),
    gas("neptune", "Neptune", 24_622.0, 0x7193d8),
    d("triton", "Triton", 1353.0, 0xd8cac2),
    d("pluto", "Pluto", 1188.0, 0xd6b698),
    d("charon", "Charon", 606.0, 0xa8a49f),
];

/// Out from the sun, a planet (or a lone body) and its moons out from it.
const SYSTEMS: &[&[&str]] = &[
    &["mercury"],
    &["venus"],
    &["earth", "moon"],
    &["mars", "phobos", "deimos"],
    &["vesta"],
    &["ceres"],
    &["jupiter", "io", "europa", "ganymede", "callisto"],
    &["saturn", "mimas", "enceladus", "tethys", "dione", "rhea", "titan", "iapetus"],
    &["uranus", "miranda", "ariel", "umbriel", "titania", "oberon"],
    &["neptune", "triton"],
    &["pluto", "charon"],
];

fn disc(id: &str) -> &'static Disc {
    DISCS.iter().find(|d| d.id == id).expect("every body on the strip has a disc")
}

fn system_of(id: &str) -> usize {
    SYSTEMS.iter().position(|s| s.contains(&id)).expect("every body is in a system")
}

fn colour(hex: u32, alpha: f32) -> Color {
    Color::srgba_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Where the strip is and what it shows.
#[derive(Resource)]
pub struct Picker {
    /// 1: the solar system; 2: one system, zoomed into.
    level: u8,
    /// The system zoomed into, and the keyboard's cursor.
    view: usize,
    cursor: &'static str,
    /// The body underfoot, as the strip knows it.
    body: &'static str,
    /// Whether the mouse has been taken once: then the title is gone for
    /// good and the strip comes back alone.
    started: bool,
    shown: bool,
    hovered: Option<&'static str>,
}

impl Picker {
    /// Whether the opening screen's title is up, dimming the world.
    pub fn titled(&self) -> bool {
        self.shown && !self.started
    }
}

impl Default for Picker {
    fn default() -> Self {
        Self { level: 1, view: 2, cursor: "earth", body: "moon", started: false, shown: false, hovered: None }
    }
}

/// What a click or Enter does.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Act {
    Zoom(usize),
    Go(&'static str),
    Back,
}

/// Every element of the strip, by what it stands for.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Key {
    Ecliptic,
    Disc(&'static str),
    Ring(&'static str),
    Label(&'static str),
    Hit(&'static str),
    Back,
    Crumb,
    Scale,
    NoMoons,
    Resume,
}

#[derive(Component)]
struct Part(Key);

/// Where an element should be, and where it is now: eased toward it, so
/// moving between levels is a zoom rather than a cut.
#[derive(Component, Clone, Copy, Default, PartialEq)]
struct Place {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    opacity: f32,
}


#[derive(Component)]
struct Strip;
#[derive(Component)]
struct Backdrop;
#[derive(Component)]
struct Heading;
#[derive(Component)]
struct Sub;
#[derive(Component)]
struct Fine;
#[derive(Component)]
struct Watch;
/// The look a label or disc should have this frame.
#[derive(Component, Clone, Copy, Default, PartialEq)]
struct Look {
    on: bool,
    off: bool,
    hover: bool,
}
#[derive(Component)]
struct Hit(Option<Act>);

/// What the layout said beyond places: each body's second line, the way
/// back, the scale.
#[derive(Resource, Default)]
struct Drawn {
    subs: HashMap<&'static str, String>,
    back: Option<String>,
    scale: String,
}

/// Labels anchored by their middle, left or right edge.
#[derive(Component, Clone, Copy, PartialEq)]
enum Anchor {
    Middle,
    Left,
    Right,
}

const LABEL_W: f32 = 180.0;
const MINR: f32 = 1.3;
const ACCENT: u32 = 0xe8b04b;

fn shown_at_all() -> bool {
    std::env::var_os("MOON_PICKER").is_some() || !tranquility::unattended()
}

fn setup(mut commands: Commands, font: Res<HudFont>, mut picker: ResMut<Picker>, current: Res<Current>) {
    let face = |size: f32| TextFont { font: font.0.clone().into(), font_size: FontSize::Px(size), ..default() };
    let id = DISCS.iter().find(|d| d.name == current.0.name).map_or("moon", |d| d.id);
    picker.body = id;
    picker.cursor = SYSTEMS[system_of(id)][0];
    picker.view = system_of(id);
    // MOON_PICKER=paused: as after Esc, the strip alone.
    picker.started = std::env::var("MOON_PICKER").is_ok_and(|v| v == "paused");
    // The opening screen: the world dimmed behind a title.
    commands
        .spawn((
            Backdrop,
            Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), bottom: px(0), ..default() },
            BackgroundGradient(vec![Gradient::Radial(RadialGradient::new(
                UiPosition::CENTER,
                RadialGradientShape::FarthestCorner,
                vec![ColorStop::auto(Color::srgba_u8(18, 18, 24, 178)), ColorStop::auto(Color::srgba_u8(0, 0, 0, 237))],
            ))]),
            GlobalZIndex(18),
            Visibility::Hidden,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        right: px(0),
                        top: px(0),
                        bottom: px(120),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Heading,
                ))
                .with_children(|panel| {
                    let centred = TextLayout::justify(Justify::Center);
                    panel.spawn((Text::new(""), face(44.0), TextColor(Color::srgb_u8(0xee, 0xf0, 0xf2)), centred, Heading));
                    panel.spawn((Text::new(""), face(13.0), TextColor(Color::srgb_u8(0x85, 0x8d, 0x97)), centred, Node { margin: UiRect::top(px(12)), ..default() }, Sub));
                    panel
                        .spawn((
                            Node {
                                margin: UiRect::top(px(30)),
                                padding: UiRect::axes(px(28), px(11)),
                                border: UiRect::all(px(1)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.93, 0.94, 0.95, 0.45)),
                            BackgroundColor(Color::srgba_u8(8, 10, 14, 90)),
                        ))
                        .with_child((Text::new("C L I C K   T O   D E S C E N D"), face(12.0), TextColor(Color::srgb_u8(0xee, 0xf0, 0xf2))));
                    panel
                        .spawn((Watch, Node { margin: UiRect::top(px(14)), padding: UiRect::all(px(4)), ..default() }))
                        .with_child((Text::new("O R   W A T C H   I T   W A L K   I T S E L F"), face(10.5), TextColor(Color::srgb_u8(0x85, 0x8d, 0x97))));
                    panel.spawn((
                        Text::new(""),
                        face(13.0),
                        TextColor(Color::srgba(0.86, 0.88, 0.9, 0.85)),
                        centred,
                        Node { margin: UiRect::top(px(26)), max_width: px(660), padding: UiRect::horizontal(px(24)), ..default() },
                        Fine,
                    ));
                });
        });
    // The strip.
    commands
        .spawn((
            Strip,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                bottom: px(0),
                height: px(120),
                border: UiRect::top(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(Color::srgb_u8(0x26, 0x2c, 0x35)),
            BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
                LinearGradient::TO_TOP,
                vec![ColorStop::auto(Color::srgba_u8(7, 9, 12, 240)), ColorStop::auto(Color::srgba_u8(7, 9, 12, 178))],
            ))]),
            GlobalZIndex(20),
            Visibility::Hidden,
        ))
        .with_children(|strip| {
            let absolute = || Node { position_type: PositionType::Absolute, ..default() };
            strip.spawn((
                Part(Key::Ecliptic),
                Place::default(),
                Node { height: px(1), ..absolute() },
                BackgroundGradient(vec![Gradient::Linear(LinearGradient::new(
                    LinearGradient::TO_RIGHT,
                    vec![
                        ColorStop::auto(Color::srgba(1.0, 0.84, 0.63, 0.55)),
                        ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.1), 35.0),
                        ColorStop::auto(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                    ],
                ))]),
            ));
            // The sun: hot white to gold at the rim, glowing.
            strip.spawn((
                Part(Key::Disc("sun")),
                Place::default(),
                Look::default(),
                Node { border_radius: BorderRadius::MAX, ..absolute() },
                BackgroundGradient(vec![Gradient::Radial(RadialGradient::new(
                    UiPosition::CENTER,
                    RadialGradientShape::ClosestSide,
                    vec![
                        ColorStop::percent(Color::srgb_u8(0xff, 0xfa, 0xf0), 0.0),
                        ColorStop::percent(Color::srgb_u8(0xff, 0xee, 0xc8), 92.0),
                        ColorStop::percent(Color::srgb_u8(0xff, 0xd6, 0x90), 100.0),
                    ],
                ))]),
                BoxShadow(vec![
                    ShadowStyle { color: Color::srgba(1.0, 0.77, 0.47, 0.45), x_offset: px(0), y_offset: px(0), spread_radius: px(14), blur_radius: px(40) },
                    ShadowStyle { color: Color::srgba(1.0, 0.67, 0.35, 0.18), x_offset: px(0), y_offset: px(0), spread_radius: px(40), blur_radius: px(120) },
                ]),
            ));
            for body in DISCS {
                strip
                    .spawn((
                        Part(Key::Disc(body.id)),
                        Place::default(),
                                Look::default(),
                        Node { border_radius: BorderRadius::MAX, ..absolute() },
                        BackgroundColor(colour(body.colour, 0.0)),
                        Outline::new(px(1.5), px(2.0), Color::NONE),
                    ))
                    .with_children(|d| {
                        // Rings: Saturn's broad and tipped a little, Uranus's
                        // thin and nearly upright.
                        if body.id == "saturn" || body.id == "uranus" {
                            let tilt = if body.id == "saturn" { -12f32 } else { -8f32 };
                            d.spawn((
                                Part(Key::Ring(body.id)),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: percent(50),
                                    top: percent(50),
                                    // Bevy counts the border inside the box, CSS
                                    // outside: the ring's hole is as wide as
                                    // lunar-walk's when the box takes the border
                                    // in as well (0.95 and 0.22 of the radius
                                    // on Saturn, 0.05 on Uranus).
                                    width: percent(if body.id == "saturn" { 322 } else { 51 }),
                                    height: percent(if body.id == "saturn" { 80 } else { 210 }),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                UiTransform { translation: Val2::new(percent(-50), percent(-50)), rotation: Rot2::degrees(tilt), ..default() },
                                BorderColor::all(Color::NONE),
                            ));
                        }
                    });
                strip
                    .spawn((
                        Part(Key::Label(body.id)),
                        Place::default(),
                                Look::default(),
                        Anchor::Middle,
                        Text::new(body.name.to_uppercase()),
                        face(12.0),
                        TextColor(Color::NONE),
                        TextLayout::justify(Justify::Center),
                        TextShadow { offset: Vec2::new(0.0, 1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.8) },
                        Node { width: px(LABEL_W), ..absolute() },
                    ))
                    .with_child((TextSpan::new(""), face(10.5), TextColor(Color::NONE)));
                strip.spawn((Part(Key::Hit(body.id)), Hit(None), Place::default(), Node { ..absolute() }, Visibility::Hidden));
            }
            let note = |key: Key, anchor: Anchor, text: &str| {
                (
                    Part(key),
                    Place::default(),
                        anchor,
                    Text::new(text.to_string()),
                    face(10.5),
                    TextColor(Color::NONE),
                    TextLayout::justify(match anchor {
                        Anchor::Right => Justify::Right,
                        Anchor::Middle => Justify::Center,
                        Anchor::Left => Justify::Left,
                    }),
                    Node { width: px(360), ..absolute() },
                )
            };
            strip.spawn(note(Key::Crumb, Anchor::Left, "S O L A R   S Y S T E M"));
            strip.spawn(note(Key::Scale, Anchor::Right, ""));
            strip.spawn(note(Key::NoMoons, Anchor::Left, "no moons"));
            strip.spawn(note(Key::Resume, Anchor::Right, "CLICK TO RESUME  ·  ← → ↓ ↑ ENTER"));
            strip
                .spawn((
                    Part(Key::Back),
                    Hit(Some(Act::Back)),
                    Place::default(),
                        Node { padding: UiRect::new(px(8), px(10), px(3), px(3)), border: UiRect::all(px(1)), ..absolute() },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.14)),
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.06)),
                    Visibility::Hidden,
                ))
                .with_child((Text::new(""), face(11.0), TextColor(Color::srgb_u8(0xc3, 0xc8, 0xcf))));
        });
}

/// Height of the strip for a window of this height, px.
fn strip_height(h: f32) -> f32 {
    (h * 0.19).clamp(96.0, 150.0).round()
}

/// Lays out slots of these widths over [x0, x1], spare room shared out:
/// each slot's centre and width.
fn pack(widths: &[f32], x0: f32, x1: f32) -> Vec<(f32, f32)> {
    let extra = ((x1 - x0 - widths.iter().sum::<f32>()) / widths.len().max(1) as f32).max(0.0);
    let mut x = x0;
    widths
        .iter()
        .map(|w| {
            let slot = w + extra;
            let centre = x + slot / 2.0;
            x += slot;
            (centre, slot)
        })
        .collect()
}

/// Half the drawn width of a disc of radius r: Saturn's rings reach
/// further, and the two lumpy moons are wider than tall.
fn half_width(id: &str, r: f32) -> f32 {
    match id {
        "saturn" => r * 2.27,
        "phobos" | "deimos" => r * 1.18,
        _ => r,
    }
}

fn scale_note(scale: f32) -> String {
    let v = 1.0 / scale;
    if v < 100.0 {
        let digits = (v.log10().floor() as i32 - 1).max(0) as usize;
        return format!("1 px ≈ {v:.digits$} km");
    }
    let p = 10f32.powi(v.log10().floor() as i32 - 1);
    let rounded = ((v / p).round() * p) as u64;
    let s = rounded.to_string();
    let mut out = String::new();
    for (k, c) in s.chars().enumerate() {
        if k > 0 && (s.len() - k).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    format!("1 px ≈ {out} km")
}

/// Everything the strip shows this frame, where, and how.
struct Layout {
    height: f32,
    places: HashMap<Key, Place>,
    looks: HashMap<&'static str, Look>,
    hits: HashMap<&'static str, Option<Act>>,
    subs: HashMap<&'static str, String>,
    anchors: HashMap<&'static str, Anchor>,
    back: Option<String>,
    scale: String,
}

fn lay(p: &Picker, walkable: &dyn Fn(&str) -> bool, gravity: &dyn Fn(&str) -> String, w: f32, h: f32) -> Layout {
    let hb = strip_height(h);
    let (y, ly) = (hb * 0.42, hb * 0.72);
    let mut out = Layout {
        height: hb,
        places: HashMap::new(),
        looks: HashMap::new(),
        hits: HashMap::new(),
        subs: HashMap::new(),
        anchors: HashMap::new(),
        back: None,
        scale: String::new(),
    };
    out.places.insert(Key::Ecliptic, Place { x: 0.0, y, w, h: 1.0, opacity: 1.0 });
    let avail = |s: &[&str]| s.iter().any(|id| walkable(id));
    let sub = |id: &str| if walkable(id) { gravity(id) } else if disc(id).gas { "no surface".into() } else { "not yet".into() };
    let jupiter = disc("jupiter").radius;
    // Level one: one scale for the sun and every planet.
    let s1 = (hb * 0.3).min(w * 0.042) / jupiter;
    let widths: Vec<f32> = SYSTEMS.iter().map(|s| (64f32.min(w * 0.065)).max(2.0 * half_width(s[0], disc(s[0]).radius * s1) + 12.0)).collect();
    let slots = pack(&widths, w * 0.07, w * 0.99);
    let sol: Vec<(f32, f32, f32)> = SYSTEMS.iter().zip(&slots).map(|(s, (c, sw))| (*c, *sw, (disc(s[0]).radius * s1).max(MINR))).collect();
    let sun_r = SUN * s1;
    let sun_c = w * 0.03 - sun_r;
    // Level two: one scale for the moons, the planet cropped at the left
    // when it is too big to show whole.
    let zoomed = p.level == 2;
    let view = SYSTEMS[p.view];
    let selected = system_of(p.body);
    let (mut scale, mut big_r, mut full, mut pc, mut edge, mut k, mut xp) = (s1, 0.0, true, 0.0, 0.0, 1.0, 0.0);
    let mut moon_at: HashMap<&str, (f32, f32, f32)> = HashMap::new();
    if zoomed {
        let moons = &view[1..];
        let most = if moons.is_empty() { disc(view[0]).radius } else { moons.iter().map(|m| disc(m).radius).fold(0.0, f32::max) };
        scale = hb * if moons.is_empty() { 0.3 } else { 0.26 } / most;
        big_r = disc(view[0]).radius * scale;
        full = big_r <= hb * 0.36;
        pc = if full { w * 0.04 + big_r + 10.0 } else { w * 0.055 - big_r };
        edge = if full { pc + big_r } else { w * 0.055 };
        xp = sol[p.view].0;
        k = (scale / s1).min(30.0);
        let x0 = if full { edge + 44.0 } else { edge + 120.0 };
        let slot = (w * 0.11).clamp(84.0, 120.0);
        let widths: Vec<f32> = moons.iter().map(|m| (2.0 * half_width(m, disc(m).radius * scale) + 16.0).max(slot)).collect();
        let slots = pack(&widths, x0, (w * 0.97).min(x0 + moons.len() as f32 * slot * 1.15));
        for (m, (c, sw)) in moons.iter().zip(slots) {
            moon_at.insert(m, (c, sw, (disc(m).radius * scale).max(MINR)));
        }
    }
    // Everything else flies out as if the camera zoomed onto the planet.
    let away = |c: f32| if full { pc } else { edge } + (c - xp) * k;
    let dim = |ok: bool| if ok { 1.0 } else { 0.28 };
    let hover = |id: &str| p.cursor == id || p.hovered == Some(id);
    let put_disc = |out: &mut Layout, id: &'static str, cx: f32, r: f32, look: Look, opacity: f32| {
        let (dw, dh) = if id == "phobos" || id == "deimos" { (r * 2.36, r * 1.68) } else { (r * 2.0, r * 2.0) };
        out.places.insert(Key::Disc(id), Place { x: cx - dw / 2.0, y: y - dh / 2.0, w: dw, h: dh, opacity });
        out.looks.insert(id, look);
    };
    put_disc(&mut out, "sun", if zoomed { away(sun_c) } else { sun_c }, sun_r, Look::default(), if zoomed { 0.0 } else { 1.0 });
    for (n, system) in SYSTEMS.iter().enumerate() {
        let id = system[0];
        let (sc, sw, sr) = sol[n];
        let ok = avail(system);
        let primary = zoomed && n == p.view;
        let (mut c, mut r, mut op, mut lx) = (sc, sr, dim(ok), sc);
        if primary {
            c = pc;
            r = big_r;
            lx = if full { pc } else { edge + 46.0 };
            op = dim(walkable(id));
            if !full {
                out.anchors.insert(id, Anchor::Left);
            }
        } else if zoomed {
            c = away(sc);
            lx = c;
            r = sr * k.min(3.0);
            op = 0.0;
        }
        let on = if primary { p.body == id } else { !zoomed && selected == n };
        let usable = if primary { walkable(id) } else { ok };
        put_disc(&mut out, id, c, r, Look { on, off: !usable, hover: hover(id) }, op);
        out.places.insert(Key::Label(id), Place { x: lx, y: ly, w: LABEL_W, h: 0.0, opacity: if op == 0.0 { 0.0 } else { 1.0 } });
        out.subs.insert(id, if primary || (ok && system.len() == 1) { sub(id) } else if !ok { "later".into() } else { String::new() });
        if !zoomed {
            out.places.insert(Key::Hit(id), Place { x: sc - sw / 2.0, y: 0.0, w: sw, h: hb, opacity: 1.0 });
            out.hits.insert(id, ok.then_some(Act::Zoom(n)));
        }
        if primary {
            out.places.insert(Key::Hit(id), Place { x: 0.0, y: 0.0, w: if full { edge + 22.0 } else { edge + 104.0 }, h: hb, opacity: 1.0 });
            out.hits.insert(id, walkable(id).then_some(Act::Go(id)));
        }
        for &m in &system[1..] {
            let here = moon_at.get(m).copied();
            let mc = here.map_or(if zoomed { away(sc) } else { sc }, |(c, _, _)| c);
            put_disc(&mut out, m, mc, here.map_or(0.5, |(_, _, r)| r), Look { on: p.body == m, off: !walkable(m), hover: hover(m) }, here.map_or(0.0, |_| dim(walkable(m))));
            out.places.insert(Key::Label(m), Place { x: mc, y: ly, w: LABEL_W, h: 0.0, opacity: if here.is_some() { 1.0 } else { 0.0 } });
            out.subs.insert(m, sub(m));
            if let Some((c, sw, _)) = here {
                out.places.insert(Key::Hit(m), Place { x: c - sw / 2.0, y: 0.0, w: sw, h: hb, opacity: 1.0 });
                out.hits.insert(m, walkable(m).then_some(Act::Go(m)));
            }
        }
    }
    if zoomed {
        if view.len() == 1 {
            out.places.insert(Key::NoMoons, Place { x: edge + 44.0, y: y - 7.0, w: 360.0, h: 0.0, opacity: 1.0 });
        }
        out.places.insert(Key::Back, Place { x: edge + if full { 16.0 } else { 10.0 }, y: 7.0, w: 0.0, h: 0.0, opacity: 1.0 });
        out.back = Some(format!("‹  S O L A R   S Y S T E M   /   {}", disc(view[0]).name.to_uppercase()));
    } else {
        out.places.insert(Key::Crumb, Place { x: 12.0, y: 8.0, w: 360.0, h: 0.0, opacity: 1.0 });
    }
    out.places.insert(Key::Scale, Place { x: w - 10.0, y: 8.0, w: 360.0, h: 0.0, opacity: 1.0 });
    if p.started {
        out.places.insert(Key::Resume, Place { x: w - 150.0, y: 8.0, w: 360.0, h: 0.0, opacity: 1.0 });
    }
    out.scale = scale_note(scale);
    out
}

/// Whether the strip is up: the mouse is free, no stroll is walking.
fn up(cursor: &CursorOptions, stroll: Option<&Stroll>) -> bool {
    shown_at_all() && cursor.grab_mode == CursorGrabMode::None && !stroll.is_some_and(|s| s.on)
}

fn catalogue_id(name: &str) -> Option<&'static str> {
    DISCS.iter().find(|d| d.name.eq_ignore_ascii_case(name)).map(|d| d.id)
}

/// Goes to a body by its id on the strip.
fn go(commands: &mut Commands, bodies: &Catalogue, current: &mut Current, id: &str) {
    if let Some(body) = bodies.0.iter().find(|b| catalogue_id(b.name) == Some(id)) {
        crate::visit(commands, current, body);
    }
}

/// Zooming in goes to the system's first walkable body, unless you are on
/// one of its bodies already.
fn zoom(p: &mut Picker, n: usize, commands: &mut Commands, bodies: &Catalogue, current: &mut Current) {
    p.level = 2;
    p.view = n;
    if system_of(p.body) == n {
        p.cursor = p.body;
    } else if let Some(first) = SYSTEMS[n].iter().find(|id| bodies.0.iter().any(|b| catalogue_id(b.name) == Some(**id))) {
        p.cursor = first;
        go(commands, bodies, current, first);
    }
}

/// While the strip is up: ← → move the cursor, ↓ zooms into the system
/// under it, ↑ back out, Enter goes there (or, on the body you are on,
/// takes the mouse again).
#[allow(clippy::too_many_arguments)]
fn keys(
    mut commands: Commands,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    bodies: Res<Catalogue>,
    mut current: ResMut<Current>,
    mut p: ResMut<Picker>,
    stroll: Option<Res<Stroll>>,
    mut cursor: Single<&mut CursorOptions>,
) {
    let walkable = |id: &str| bodies.0.iter().any(|b| catalogue_id(b.name) == Some(id));
    if !up(&cursor, stroll.as_deref()) {
        if cursor.grab_mode != CursorGrabMode::None {
            p.started = true;
        }
        p.shown = false;
        return;
    }
    p.shown = true;
    let stops: Vec<&'static str> = if p.level == 1 {
        SYSTEMS.iter().filter(|s| s.iter().any(|id| walkable(id))).map(|s| s[0]).collect()
    } else {
        SYSTEMS[p.view].iter().copied().filter(|id| walkable(id)).collect()
    };
    let take = |keys: &mut ButtonInput<KeyCode>, k: KeyCode| {
        let hit = keys.just_pressed(k);
        if hit {
            keys.clear_just_pressed(k);
        }
        hit
    };
    let step = take(&mut keys, KeyCode::ArrowRight) as i32 - take(&mut keys, KeyCode::ArrowLeft) as i32;
    if step != 0 && !stops.is_empty() {
        let n = stops.len() as i32;
        let i = stops.iter().position(|id| *id == p.cursor).map_or(0, |i| (i as i32 + step).rem_euclid(n));
        p.cursor = stops[i as usize];
    }
    if take(&mut keys, KeyCode::ArrowDown) && p.level == 1 {
        let n = system_of(p.cursor);
        zoom(&mut p, n, &mut commands, &bodies, &mut current);
    }
    if take(&mut keys, KeyCode::ArrowUp) && p.level == 2 {
        p.level = 1;
        p.cursor = SYSTEMS[p.view][0];
    }
    if take(&mut keys, KeyCode::Enter) || take(&mut keys, KeyCode::NumpadEnter) {
        if p.level == 1 {
            let n = system_of(p.cursor);
            zoom(&mut p, n, &mut commands, &bodies, &mut current);
        } else if p.cursor != p.body {
            let id = p.cursor;
            go(&mut commands, &bodies, &mut current, id);
        } else {
            cursor.visible = false;
            cursor.grab_mode = CursorGrabMode::Locked;
        }
    }
}

/// Clicks on the strip, and the pointer over it. The strip takes every
/// click on it; the title's "watch it walk itself" starts the stroll.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn click(
    mut commands: Commands,
    bodies: Res<Catalogue>,
    mut current: ResMut<Current>,
    mut p: ResMut<Picker>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut over: ResMut<PointerOverUi>,
    window: Single<&Window>,
    hits: Query<(&Part, &Hit, &Place, &ComputedNode, &Visibility)>,
    watch: Single<(&ComputedNode, &UiGlobalTransform), With<Watch>>,
    mut stroll: Option<ResMut<Stroll>>,
    mut cursor: Single<&mut CursorOptions>,
) {
    let pointer = window.cursor_position();
    if !p.shown || pointer.is_none() {
        if over.0 {
            over.0 = false;
        }
        p.hovered = None;
        return;
    }
    let at = pointer.unwrap_or_default();
    let scale = window.scale_factor();
    let top = window.height() - strip_height(window.height());
    let on_strip = at.y >= top;
    // What is under the pointer: a body's slot, or the way back.
    let local = Vec2::new(at.x, at.y - top);
    let mut under: Option<(Key, Option<Act>)> = None;
    for (part, hit, place, node, vis) in &hits {
        if *vis == Visibility::Hidden {
            continue;
        }
        let size = if part.0 == Key::Back { node.size() / scale } else { Vec2::new(place.w, place.h) };
        let inside = local.x >= place.x && local.x < place.x + size.x && local.y >= place.y && local.y < place.y + size.y;
        // The way back lies over the planet's slot: it wins.
        if inside && (under.is_none() || part.0 == Key::Back) {
            under = Some((part.0, hit.0));
        }
    }
    let hovered = match under {
        Some((Key::Hit(id), Some(_))) => Some(id),
        _ => None,
    };
    if p.hovered != hovered {
        p.hovered = hovered;
    }
    let (node, transform) = watch.into_inner();
    let centre = transform.translation / scale;
    let half = node.size() / scale / 2.0;
    let on_watch = !p.started && (at - centre).abs().cmple(half).all();
    let taken = on_strip || on_watch;
    if over.0 != taken {
        over.0 = taken;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if on_watch && let Some(stroll) = stroll.as_mut() {
        stroll.ask();
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
        return;
    }
    match under.and_then(|(_, act)| act) {
        Some(Act::Zoom(n)) => zoom(&mut p, n, &mut commands, &bodies, &mut current),
        Some(Act::Go(id)) => go(&mut commands, &bodies, &mut current, id),
        Some(Act::Back) => {
            p.level = 1;
            p.cursor = SYSTEMS[p.view][0];
        }
        None => {}
    }
}

/// The strip follows the body underfoot, however it was chosen.
fn follow(current: Res<Current>, mut p: ResMut<Picker>) {
    if !current.is_changed() {
        return;
    }
    let Some(id) = catalogue_id(current.0.name) else { return };
    p.body = id;
    if p.level == 2 {
        p.view = system_of(id);
    }
    p.cursor = if p.level == 1 { SYSTEMS[system_of(id)][0] } else { id };
}

/// Lays the strip out for the window and eases everything to its place.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn lay_out(
    time: Res<Time<Real>>,
    p: Res<Picker>,
    bodies: Res<Catalogue>,
    mut drawn: ResMut<Drawn>,
    mut hud: ResMut<Hud>,
    window: Single<&Window>,
    strip: Single<(&mut Node, &mut Visibility), (With<Strip>, Without<Part>)>,
    mut parts: Query<(&Part, &mut Place, &mut Node, Option<&mut Look>, Option<&mut Hit>, Option<&mut Anchor>), Without<Strip>>,
    mut last: Local<(f32, f32)>,
) {
    let (mut node, mut vis) = strip.into_inner();
    if !p.shown {
        vis.set_if_neq(Visibility::Hidden);
        if hud.lift != 0.0 {
            hud.lift = 0.0;
        }
        *last = (0.0, 0.0);
        return;
    }
    vis.set_if_neq(Visibility::Inherited);
    let (w, h) = (window.width(), window.height());
    let walkable = |id: &str| bodies.0.iter().any(|b| catalogue_id(b.name) == Some(id));
    let gravity = |id: &str| {
        bodies.0.iter().find(|b| catalogue_id(b.name) == Some(id)).map_or(String::new(), |b| {
            if b.gravity < 0.1 { format!("{:.4} m/s²", b.gravity) } else { format!("{:.2} m/s²", b.gravity) }
        })
    };
    let layout = lay(&p, &walkable, &gravity, w, h);
    node.height = px(layout.height);
    if hud.lift != layout.height {
        hud.lift = layout.height;
    }
    // Coming up, or a new window size, snaps everything into place; a new
    // level eases, as a zoom.
    let snap = *last != (w, h);
    *last = (w, h);
    let ease = 1.0 - (-time.delta_secs() / 0.22).exp();
    for (part, mut place, mut node, look, hit, anchor) in &mut parts {
        let goal = layout.places.get(&part.0).copied();
        let want = goal.unwrap_or(Place { opacity: 0.0, ..*place });
        let now = if snap || matches!(part.0, Key::Hit(_) | Key::Back) {
            want
        } else {
            let mix = |a: f32, b: f32| a + (b - a) * ease;
            Place { x: mix(place.x, want.x), y: mix(place.y, want.y), w: mix(place.w, want.w), h: mix(place.h, want.h), opacity: mix(place.opacity, want.opacity) }
        };
        if *place != now {
            *place = now;
        }
        if let (Some(mut look), Key::Disc(id) | Key::Label(id)) = (look, part.0) {
            let wanted = layout.looks.get(id).copied().unwrap_or_default();
            if *look != wanted {
                *look = wanted;
            }
        }
        let a = match (part.0, anchor) {
            (Key::Label(id), Some(mut a)) => {
                let wanted = layout.anchors.get(id).copied().unwrap_or(Anchor::Middle);
                if *a != wanted {
                    *a = wanted;
                }
                wanted
            }
            (_, Some(a)) => *a,
            _ => Anchor::Left,
        };
        // Discs, hits and the ecliptic are boxes; labels and notes hang
        // from their anchor.
        match part.0 {
            Key::Disc(_) | Key::Hit(_) | Key::Ecliptic => {
                node.left = px(place.x);
                node.top = px(place.y);
                node.width = px(place.w.max(0.0));
                node.height = px(place.h.max(0.0));
            }
            _ => {
                let width = if matches!(part.0, Key::Label(_)) { LABEL_W } else { 360.0 };
                node.left = px(match a {
                    Anchor::Middle => place.x - width / 2.0,
                    Anchor::Left => place.x,
                    Anchor::Right => place.x - width,
                });
                node.top = px(place.y);
            }
        }
        if let (Some(mut hit), Key::Hit(id)) = (hit, part.0) {
            let act = layout.hits.get(id).copied().flatten();
            if hit.0 != act {
                hit.0 = act;
            }
        }
    }
    drawn.subs = layout.subs;
    drawn.back = layout.back;
    drawn.scale = layout.scale;
}

/// Colours, from each part's opacity and look: silhouettes dimmed where
/// nobody walks yet, the body underfoot ringed in the accent, the one under
/// the cursor brighter; labels in grey, gold for the body underfoot, white
/// under the cursor.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn paint(
    p: Res<Picker>,
    drawn: Res<Drawn>,
    mut discs: Query<(&Part, &Place, &Look, Option<&mut BackgroundColor>, Option<&mut Outline>, &mut Visibility), (Without<Hit>, Without<Text>)>,
    mut rings: Query<(&Part, &ChildOf, &mut Node, &mut BorderColor), Without<Place>>,
    places: Query<(&Place, &Look)>,
    mut labels: Query<
        (&Part, &Place, Option<&Look>, &mut Text, &mut TextColor, Option<&mut TextShadow>, &mut Visibility, Option<&Anchor>, &mut TextLayout, Option<&Children>),
        Without<Hit>,
    >,
    mut spans: Query<(&mut TextSpan, &mut TextColor), Without<Text>>,
    mut hits: Query<(&Part, &Place, &Children, &mut Visibility), (With<Hit>, Without<Look>)>,
    mut hit_texts: Query<&mut Text, (Without<Part>, Without<Heading>, Without<Sub>, Without<Fine>)>,
) {
    if !p.shown {
        return;
    }
    let grey = |hex: u32, a: f32| colour(hex, a);
    for (part, place, look, bg, outline, mut vis) in &mut discs {
        let Key::Disc(id) = part.0 else { continue };
        vis.set_if_neq(if place.opacity > 0.01 { Visibility::Inherited } else { Visibility::Hidden });
        if id == "sun" {
            continue;
        }
        let mut c = colour(disc(id).colour, place.opacity);
        if look.hover && !look.off {
            let l = c.to_linear();
            c = Color::linear_rgba((l.red * 1.6).min(1.0), (l.green * 1.6).min(1.0), (l.blue * 1.6).min(1.0), l.alpha);
        }
        if let Some(mut bg) = bg
            && bg.0 != c
        {
            bg.0 = c;
        }
        if let Some(mut outline) = outline {
            let ring = if look.on { colour(ACCENT, place.opacity) } else { Color::NONE };
            if outline.color != ring {
                outline.color = ring;
            }
        }
    }
    for (part, parent, mut node, mut border) in &mut rings {
        let Key::Ring(id) = part.0 else { continue };
        let Ok((place, _)) = places.get(parent.parent()) else { continue };
        let r = place.h / 2.0;
        let (side, top) = if id == "saturn" { (0.95 * r, 0.22 * r) } else { (0.05 * r, 0.05 * r) };
        node.border = UiRect::new(px(side), px(side), px(top), px(top));
        // Zoomed in, the primary's rings would lie across its own moons.
        let primary = p.level == 2 && SYSTEMS[p.view][0] == id;
        let alpha = if primary { 0.0 } else { place.opacity * if id == "saturn" { 0.8 } else { 0.55 } };
        let c = colour(disc(id).colour, alpha);
        if border.top != c {
            *border = BorderColor::all(c);
        }
    }
    for (part, place, look, mut text, mut colour_of, shadow, mut vis, anchor, mut layout, children) in &mut labels {
        let a = place.opacity;
        let justify = match anchor {
            Some(Anchor::Left) => Justify::Left,
            Some(Anchor::Right) => Justify::Right,
            _ => Justify::Center,
        };
        if layout.justify != justify {
            layout.justify = justify;
        }
        // Gone, rather than drawn clear: a shadow would still show.
        vis.set_if_neq(if a > 0.02 { Visibility::Inherited } else { Visibility::Hidden });
        if let Some(mut shadow) = shadow {
            let c = Color::srgba(0.0, 0.0, 0.0, 0.8 * a);
            if shadow.color != c {
                shadow.color = c;
            }
        }
        let look = look.copied().unwrap_or_default();
        let (main, small) = match part.0 {
            Key::Label(_) => {
                if look.on && look.hover {
                    (grey(0xf8cd78, a), grey(0xf0c79a, a))
                } else if look.on {
                    (grey(ACCENT, a), grey(0xf0c79a, a))
                } else if look.off {
                    (grey(0x555c66, a), grey(0x434951, a))
                } else if look.hover {
                    (grey(0xffffff, a), grey(0x858d97, a))
                } else {
                    (grey(0xc3c8cf, a), grey(0x858d97, a))
                }
            }
            Key::NoMoons => (grey(0x555c66, a), Color::NONE),
            _ => (grey(0x858d97, a), Color::NONE),
        };
        if colour_of.0 != main {
            colour_of.0 = main;
        }
        if part.0 == Key::Scale && text.0 != drawn.scale {
            text.0.clone_from(&drawn.scale);
        }
        if let (Key::Label(id), Some(children)) = (part.0, children) {
            let sub = format!("\n{}", drawn.subs.get(id).map_or("", String::as_str));
            for child in children {
                if let Ok((mut span, mut c)) = spans.get_mut(*child) {
                    if span.0 != sub {
                        span.0.clone_from(&sub);
                    }
                    if c.0 != small {
                        c.0 = small;
                    }
                }
            }
        }
    }
    for (part, place, children, mut vis) in &mut hits {
        let show = place.opacity > 0.5 && (part.0 != Key::Back || drawn.back.is_some());
        vis.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
        if part.0 == Key::Back
            && let Some(back) = &drawn.back
        {
            for child in children {
                if let Ok(mut t) = hit_texts.get_mut(*child)
                    && t.0 != *back
                {
                    t.0.clone_from(back);
                }
            }
        }
    }
}

/// The title over the opening screen: the body, its site and pull, what to
/// know before stepping out.
fn title(
    current: Res<Current>,
    p: Res<Picker>,
    mut backdrop: Single<&mut Visibility, With<Backdrop>>,
    mut heading: Query<&mut Text, (With<Heading>, Without<Sub>, Without<Fine>)>,
    mut sub: Single<&mut Text, (With<Sub>, Without<Heading>, Without<Fine>)>,
    mut fine: Single<&mut Text, (With<Fine>, Without<Heading>, Without<Sub>)>,
) {
    backdrop.set_if_neq(if p.shown && !p.started { Visibility::Inherited } else { Visibility::Hidden });
    if !current.is_changed() {
        return;
    }
    let b = &current.0;
    for mut t in &mut heading {
        t.0 = format!("{} WALK", b.name.to_uppercase()).chars().flat_map(|c| [c, ' ']).collect::<String>().trim_end().to_string();
    }
    let g = b.gravity / 9.81;
    let g = if g < 0.01 { format!("{g:.4}") } else { format!("{g:.3}") };
    sub.0 = format!("{} · {g} g · surface unbounded", b.site);
    let mut lines = vec![b.note.to_string(), "Walk in any direction for as long as you like: the ground never ends.".to_string()];
    let mut keys = Vec::new();
    if b.rover {
        keys.push("R drives the rover.".to_string());
    }
    if let Some(craft) = b.craft {
        keys.push(format!("R beside {} flies it.", craft.name()));
    }
    keys.push(format!("F flies. I tells you about {}.", b.name));
    lines.push(keys.join(" "));
    fine.0 = lines.join("\n");
}
