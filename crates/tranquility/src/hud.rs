//! A text overlay: frame rate, position, mode, sun; a line the app can fill;
//! the keys.

use bevy::prelude::*;

use crate::{
    origin::Origin,
    player::Player,
    quality::Quality,
    sky::Sun,
    world::{SunPath, WorldDef},
};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hud>()
            .add_systems(Startup, setup)
            .add_systems(Update, (update, toggle, (quality_said, notice).chain()));
    }
}

/// Text an app wants on screen under the status line: a place name, what
/// you are looking at. Empty hides it.
#[derive(Resource, Default)]
pub struct Hud {
    pub info: String,
    /// Extra keys the app handles, appended to the help line.
    pub keys: String,
    /// Replaces WALK/FLY in the status line while the app has the player
    /// doing something else (driving).
    pub mode: String,
    /// No text at all: `H`, or an app that wants a clean picture.
    pub hidden: bool,
    /// Room an app's own panel takes along the bottom of the window, px:
    /// the help line stands above it.
    pub lift: f32,
    /// A line said in answer to a key, for a few seconds (see `say`).
    notice: (String, u32),
}

impl Hud {
    /// Says something in the middle of the screen for a few seconds: what
    /// a key has just done, or why it did nothing. Shown even with the
    /// text hidden, as the answer to a key just pressed.
    pub fn say(&mut self, text: impl Into<String>) {
        self.notice = (text.into(), self.notice.1.wrapping_add(1));
    }
}

static CLEAN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// The notice's line.
#[derive(Component)]
struct Notice;

/// The overlay's own text, as against anything else an app writes.
#[derive(Component)]
struct Overlay;

/// The overlay's typeface, for an app's own text.
#[derive(Resource, Clone)]
pub struct HudFont(pub Handle<Font>);

#[derive(Component)]
struct Status {
    frames: u32,
    elapsed: f32,
    fps: f32,
}

#[derive(Component)]
struct Info;

#[derive(Component)]
struct Help;

fn shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::splat(1.0),
        color: Color::srgba(0.0, 0.0, 0.0, 0.7),
    }
}

/// Liberation Sans (SIL Open Font License, see assets/fonts): Bevy's
/// built-in font is a small subset that lacks ß, dashes and the degree sign.
const FONT: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");

fn setup(mut commands: Commands, mut fonts: ResMut<Assets<Font>>, mut hud: ResMut<Hud>) {
    if std::env::var_os("TRANQUILITY_CLEAN").is_some() {
        hud.hidden = true;
    }
    let handle = fonts.add(Font::from_bytes(FONT.to_vec()));
    commands.insert_resource(HudFont(handle.clone()));
    let font = TextFont {
        font: handle.into(),
        font_size: FontSize::Px(15.0),
        ..default()
    };
    let at = |top: Option<f32>, bottom: Option<f32>| Node {
        position_type: PositionType::Absolute,
        top: top.map_or(Val::Auto, px),
        bottom: bottom.map_or(Val::Auto, px),
        left: px(12),
        right: px(12),
        ..default()
    };
    commands.spawn((
        Text::new(""),
        font.clone(),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
        shadow(),
        at(Some(10.0), None),
        Overlay,
        Status {
            frames: 0,
            elapsed: 0.0,
            fps: 0.0,
        },
    ));
    commands.spawn((
        Text::new(""),
        font.clone(),
        TextColor(Color::srgba(1.0, 0.95, 0.85, 0.9)),
        shadow(),
        at(Some(30.0), None),
        Overlay,
        Info,
    ));
    commands.spawn((
        Text::new(""),
        font.clone(),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        shadow(),
        at(None, Some(10.0)),
        Overlay,
        Help,
    ));
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(19.0), ..font },
        TextColor(Color::NONE),
        shadow(),
        TextLayout::justify(Justify::Center),
        Node { position_type: PositionType::Absolute, left: px(0), right: px(0), bottom: percent(24), ..default() },
        Notice,
    ));
}

/// The quality tier, said when `Q` changes it.
fn quality_said(quality: Res<Quality>, mut hud: ResMut<Hud>) {
    if quality.is_changed() && !quality.is_added() {
        hud.say(format!("quality: {}", quality.name()));
    }
}

/// Shows the latest notice for three seconds, fading out.
fn notice(time: Res<Time<Real>>, hud: Res<Hud>, line: Single<(&mut Text, &mut TextColor, &mut Visibility), With<Notice>>, mut since: Local<(u32, f32)>) {
    let (mut text, mut colour, mut visibility) = line.into_inner();
    let t = time.elapsed_secs();
    if hud.notice.1 != since.0 {
        *since = (hud.notice.1, t);
        text.0.clone_from(&hud.notice.0);
    }
    // TRANQUILITY_CLEAN=1: no text at all, notices included, for pictures.
    let clean = *CLEAN.get_or_init(|| std::env::var_os("TRANQUILITY_CLEAN").is_some());
    let alpha = if hud.notice.1 == 0 || clean { 0.0 } else { ((3.0 - (t - since.1)) / 0.6).clamp(0.0, 1.0) * 0.9 };
    if colour.0.alpha() != alpha {
        colour.0 = Color::srgba(1.0, 1.0, 1.0, alpha);
    }
    // Gone, not just clear: its shadow would still draw.
    visibility.set_if_neq(if alpha > 0.0 { Visibility::Inherited } else { Visibility::Hidden });
}

fn update(
    time: Res<Time>,
    sun: Res<Sun>,
    world: Res<WorldDef>,
    hud: Res<Hud>,
    origin: Res<Origin>,
    quality: Res<Quality>,
    player: Single<(&Player, &Transform)>,
    status: Single<(&mut Text, &mut Status), (Without<Info>, Without<Help>)>,
    mut info: Single<&mut Text, (With<Info>, Without<Help>)>,
    help: Single<(&mut Text, &mut Node), With<Help>>,
) {
    let (mut help, mut help_node) = help.into_inner();
    let (mut text, mut status) = status.into_inner();
    status.frames += 1;
    status.elapsed += time.delta_secs();
    if status.elapsed >= 0.5 {
        status.fps = status.frames as f32 / status.elapsed;
        status.frames = 0;
        status.elapsed = 0.0;
    }
    let (player, transform) = player.into_inner();
    let p = transform.translation;
    let at = origin.world(p);
    let sun_text = match world.sky.sun {
        SunPath::Daily { .. } => {
            let minutes = (sun.hours * 60.0) as u32;
            format!("{:02}:{:02}", minutes / 60, minutes % 60)
        }
        SunPath::Fixed { .. } => format!("sun {:.0}°", sun.elevation),
    };
    let status_text = format!(
        "{}   {:.0} fps ({})   x {:.0}  z {:.0}  alt {:.0} m   {}   {}",
        world.name,
        status.fps,
        quality.name(),
        at.x,
        at.y,
        p.y,
        if !hud.mode.is_empty() { hud.mode.as_str() } else if player.flying { "FLY" } else { "WALK" },
        sun_text,
    );
    if text.0 != status_text {
        text.0 = status_text;
    }
    if hud.is_changed() || quality.is_changed() || world.is_changed() {
        // Only what has really changed: laying text out again costs.
        if info.0 != hud.info {
            info.0 = hud.info.clone();
        }
        let keys = format!(
            "click: capture mouse   WASD: walk   Shift: faster   Space: jump   F: fly (Space/C up/down)\n\
             [ ]: sun   {}Q: quality   M: sound   H: hide text   F11: full screen   Esc: release mouse{}",
            // `B` works only where there is no sky light (see `sky::toggle_air`).
            if world.sky.sky_light || world.sky.air.is_none() { "" } else { "B: air on/off   " },
            if hud.keys.is_empty() { String::new() } else { format!("   {}", hud.keys) }
        );
        if help.0 != keys {
            help.0 = keys;
        }
        let bottom = px(10.0 + hud.lift);
        if help_node.bottom != bottom {
            help_node.bottom = bottom;
        }
    }
}

/// `H` hides and shows all text.
fn toggle(keys: Res<ButtonInput<KeyCode>>, mut hud: ResMut<Hud>, mut nodes: Query<&mut Visibility, With<Overlay>>) {
    if keys.just_pressed(KeyCode::KeyH) {
        hud.hidden = !hud.hidden;
    }
    let show = if hud.hidden { Visibility::Hidden } else { Visibility::Inherited };
    for mut v in &mut nodes {
        v.set_if_neq(show);
    }
}
