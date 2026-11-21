//! A walk on other worlds: the Moon and its neighbours out to Pluto, each
//! with its own ground, sky, light and gravity, and all of it unbounded.
//! The lunar-walk recipe on the tranquility engine.
//!
//! A body is data ([`body::Body`]) plus a ground ([`body::Land`]): from its
//! gravity, radius and distance from the sun follow the walker's pace, the
//! horizon, the size and brightness of the sun. `bodies/` has one file per
//! body. Number keys change body; `TRANQUILITY_BODY=mars` (or the first
//! argument) picks the one to start on.

mod bodies;
mod body;
mod cabin;
mod craters;
mod descent;
mod earthrise;
mod facts;
mod figure;
mod gait;
mod heavens;
mod lander;
mod options;
mod picker;
mod plumes;
mod prints;
mod rocks;
mod rotorcraft;
mod rover;
mod sites;
mod sound;
mod stroll;
mod suit;

use tranquility::prelude::*;

use body::Body;

/// The body underfoot.
#[derive(Resource)]
pub struct Current(pub Body);

#[derive(Resource)]
pub struct Catalogue(pub Vec<Body>);

const KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn main() {
    let bodies = bodies::all();
    // A body or a site by name, as the first argument or in
    // TRANQUILITY_BODY / TRANQUILITY_SITE: `mars`, `gale`; a site picks
    // its body. TRANQUILITY_SITE=invented walks the body's own ground.
    let arg = std::env::args().skip(1).find(|a| !a.starts_with("--"));
    let site_name = options::get("TRANQUILITY_SITE").or_else(|| arg.clone());
    let site = site_name.as_deref().and_then(sites::by_id);
    let wanted = site.map(|s| s.body.to_string()).or(arg).or_else(|| options::get("TRANQUILITY_BODY"));
    let body = wanted
        .and_then(|name| bodies.iter().find(|b| b.name.eq_ignore_ascii_case(&name)))
        .or_else(|| bodies.iter().find(|b| b.name == "Moon"))
        .unwrap_or(&bodies[0]);
    let invented = site_name.is_some_and(|n| n.eq_ignore_ascii_case("invented"));
    let start = body.at(if invented { None } else { site.or_else(|| first_site(body)) });
    // MOON_SOUND_WAV=out.wav: a minute of the body's sound to a file, and
    // nothing else.
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(path) = std::env::var("MOON_SOUND_WAV") {
        sound::render(&path, &start).expect("writing the sound");
        return;
    }
    // A recording of an Earthrise film is as long as the film (less the
    // second the recorder waits once the ground is in).
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("TRANQUILITY_RECORD").is_some()
        && std::env::var_os("TRANQUILITY_RECORD_SECONDS").is_none()
        && let Some(length) = earthrise::film_length()
    {
        // SAFETY: nothing else runs yet; no thread reads the environment.
        unsafe { std::env::set_var("TRANQUILITY_RECORD_SECONDS", format!("{:.2}", length - 1.0)) };
    }
    let mut app = tranquility::app(start.world());
    app.insert_resource(Current(start))
        .insert_resource(Catalogue(bodies))
        .add_plugins((gait::GaitPlugin, stroll::StrollPlugin, sound::SoundPlugin, facts::FactsPlugin, picker::PickerPlugin))
        .add_systems(Update, (travel, next_site, describe));
    // MOON_OFF=rocks,heavens,prints,figure,lander,plumes,rover,heli leaves parts out, for measuring what
    // each costs.
    let off = options::get("MOON_OFF").unwrap_or_default();
    if !off.contains("rocks") {
        app.add_plugins(rocks::RocksPlugin);
    }
    if !off.contains("heavens") {
        app.add_plugins(heavens::HeavensPlugin);
    }
    if !off.contains("prints") {
        app.add_plugins(prints::PrintsPlugin);
    }
    if !off.contains("plumes") {
        app.add_plugins(plumes::PlumesPlugin);
    }
    if !off.contains("figure") {
        app.add_plugins(figure::FigurePlugin);
    }
    if !off.contains("lander") && !off.contains("rover") {
        app.add_plugins((lander::LanderPlugin, descent::DescentPlugin));
    }
    if !off.contains("earthrise") {
        app.add_plugins(earthrise::EarthrisePlugin);
    }
    if !off.contains("rover") {
        app.add_plugins(rover::RoverPlugin);
    }
    if !off.contains("heli") {
        app.add_plugins(rotorcraft::RotorcraftPlugin);
    }
    app.run();
}

/// Number keys: another body.
fn travel(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, bodies: Res<Catalogue>, mut current: ResMut<Current>) {
    for (key, body) in KEYS.iter().zip(&bodies.0) {
        if keys.just_pressed(*key) {
            visit(&mut commands, &mut current, body);
        }
    }
}

/// Goes to a body, at its first real site, unless already there.
pub fn visit(commands: &mut Commands, current: &mut Current, body: &Body) {
    if body.name != current.0.name {
        arrive(commands, current, body.at(first_site(body)));
    }
}

/// A body's first real site, if it has any: where it is visited.
pub fn first_site(body: &Body) -> Option<&'static sites::Site> {
    sites::on(body.name).next()
}

/// Puts the walk on a body, at a site or its own ground.
pub fn arrive(commands: &mut Commands, current: &mut Current, body: Body) {
    commands.insert_resource(body.world());
    current.0 = body;
}

/// **N**: the body's next site, its own invented ground after the last.
fn next_site(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, bodies: Res<Catalogue>, mut current: ResMut<Current>, mut hud: ResMut<Hud>) {
    if !keys.just_pressed(KeyCode::KeyN) {
        return;
    }
    let Some(body) = bodies.0.iter().find(|b| b.name == current.0.name) else { return };
    let places: Vec<Option<&'static sites::Site>> = sites::on(body.name).map(Some).chain([None]).collect();
    if places.len() < 2 {
        hud.say(format!("{} has no mapped sites yet: its ground here is invented", body.name));
        return;
    }
    let here = places.iter().position(|p| p.map(|s| s.id) == current.0.place.map(|s| s.id)).unwrap_or(0);
    let next = places[(here + 1) % places.len()];
    let said = match next {
        Some(site) => format!("{}: {}", site.name, sites::degrees(site.lat_lon(body.radius, site.start))),
        None => format!("{}: invented ground", body.name),
    };
    arrive(&mut commands, &mut current, body.at(next));
    hud.say(said);
}

fn describe(current: Res<Current>, bodies: Res<Catalogue>, world: Res<WorldDef>, mut hud: ResMut<Hud>, mut window: Single<&mut Window>) {
    if !current.is_changed() {
        return;
    }
    let b = &current.0;
    window.title = format!("{} · {}", b.name, b.site);
    let w = &world.walker;
    let g = if b.gravity < 0.1 { format!("{:.4}", b.gravity) } else if b.gravity < 1.0 { format!("{:.2}", b.gravity) } else { format!("{:.1}", b.gravity) };
    let share = b.gravity / 9.81 * 100.0;
    let share = if share < 1.0 { format!("{share:.2}") } else { format!("{share:.0}") };
    let metres = |v: f32| if v < 0.1 { format!("{:.0} cm/s", v * 100.0) } else { format!("{v:.1} m/s") };
    hud.info = format!(
        "{} · {} m/s² ({} % of Earth's) · sun at {:.1} AU · walk {}, lope {}, a jump {:.1} m high{}\n{}",
        b.site,
        g,
        share,
        b.au,
        metres(w.walk),
        metres(w.run),
        w.jump * w.jump / (2.0 * w.gravity),
        if w.jets > 0.0 { " · suit jets in flight" } else { "" },
        b.note,
    );
    let names: Vec<String> = bodies.0.iter().enumerate().map(|(k, b)| format!("{} {}", k + 1, b.name)).collect();
    let ride = match (b.rover, b.craft) {
        (true, Some(c)) => format!("   R: drive the rover, fly {}", c.name()),
        (true, None) => "   R: drive the rover".to_string(),
        (false, Some(c)) => format!("   R: fly {}", c.name()),
        (false, None) => String::new(),
    };
    let land = if b.air.is_none() && b.rover { "   L: land the lunar module" } else { "" };
    hud.keys = format!("0: stroll (T tours the bodies){ride}{land}   E: Earthrise, Apollo 8   I: about {}\n{}", b.name, names.join("  "));
}
