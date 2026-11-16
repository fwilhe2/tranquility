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
mod craters;
mod heavens;
mod rocks;
mod suit;

use tranquility::prelude::*;

use body::Body;

/// The body underfoot.
#[derive(Resource)]
pub struct Current(pub Body);

#[derive(Resource)]
struct Catalogue(Vec<Body>);

const KEYS: [KeyCode; 10] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];

fn main() {
    let bodies = bodies::all();
    let wanted = std::env::args().nth(1).or_else(|| std::env::var("TRANQUILITY_BODY").ok());
    let start = wanted
        .and_then(|name| bodies.iter().find(|b| b.name.eq_ignore_ascii_case(&name)))
        .or_else(|| bodies.iter().find(|b| b.name == "Moon"))
        .unwrap_or(&bodies[0])
        .clone();
    let mut app = tranquility::app(start.world());
    app.insert_resource(Current(start))
        .insert_resource(Catalogue(bodies))
        .add_plugins((rocks::RocksPlugin, heavens::HeavensPlugin))
        .add_systems(Update, (travel, describe));
    app.run();
}

/// Number keys: another body.
fn travel(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, bodies: Res<Catalogue>, mut current: ResMut<Current>) {
    for (key, body) in KEYS.iter().zip(&bodies.0) {
        if keys.just_pressed(*key) && body.name != current.0.name {
            current.0 = body.clone();
            commands.insert_resource(body.world());
        }
    }
}

fn describe(current: Res<Current>, bodies: Res<Catalogue>, world: Res<WorldDef>, mut hud: ResMut<Hud>) {
    if !current.is_changed() {
        return;
    }
    let b = &current.0;
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
    let names: Vec<String> = bodies.0.iter().enumerate().map(|(k, b)| format!("{} {}", (k + 1) % 10, b.name)).collect();
    hud.keys = names.join("  ");
}
