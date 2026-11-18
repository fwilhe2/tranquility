//! **I**: a card about the body underfoot. Numbers that follow from its row
//! (how big, how strong its pull, how far its horizon, how bright its sun,
//! what its air would do), then what there is to know about it
//! ([`Body::facts`]): its day, its heat, the features that make it what it
//! is, and who has been there.

use tranquility::{hud::HudFont, prelude::*};

use crate::{Current, body::Body};

pub struct FactsPlugin;

impl Plugin for FactsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Open>().add_systems(PostStartup, setup).add_systems(Update, (toggle, fill).chain());
    }
}

#[derive(Resource, Default)]
struct Open(bool);

#[derive(Component)]
struct Card;

/// The text the card holds, as spans: headings brighter than what follows.
#[derive(Component)]
struct Page;

const HEADING: Color = Color::srgba(1.0, 0.86, 0.62, 0.95);
const BODY: Color = Color::srgba(1.0, 1.0, 1.0, 0.86);

fn setup(mut commands: Commands, font: Res<HudFont>) {
    commands
        .spawn((
            Card,
            Node {
                position_type: PositionType::Absolute,
                right: px(12),
                top: px(76),
                width: px(460),
                max_width: percent(46),
                max_height: percent(78),
                padding: UiRect::all(px(14)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.62)),
            Visibility::Hidden,
        ))
        .with_children(|card| {
            card.spawn((
                Page,
                Text::new(""),
                TextFont { font: font.0.clone().into(), font_size: FontSize::Px(14.0), ..default() },
                TextColor(BODY),
                TextLayout::linebreak(LineBreak::WordBoundary),
            ));
        });
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, hud: Res<Hud>, mut open: ResMut<Open>, mut card: Single<&mut Visibility, With<Card>>) {
    if keys.just_pressed(KeyCode::KeyI) {
        open.0 = !open.0;
    }
    card.set_if_neq(if open.0 && !hud.hidden { Visibility::Inherited } else { Visibility::Hidden });
}

fn fill(mut commands: Commands, current: Res<Current>, font: Res<HudFont>, page: Single<(Entity, &mut Text), With<Page>>) {
    if !current.is_changed() {
        return;
    }
    let (page, mut text) = page.into_inner();
    let b = &current.0;
    text.0 = format!("{} · {}\n", b.name.to_uppercase(), b.site);
    let font = TextFont { font: font.0.clone().into(), font_size: FontSize::Px(14.0), ..default() };
    commands.entity(page).despawn_related::<Children>().with_children(|p| {
        for line in numbers(b) {
            p.spawn((TextSpan::new(format!("{line}\n")), font.clone(), TextColor(Color::srgba(0.8, 0.9, 1.0, 0.86))));
        }
        for (heading, sentence) in b.facts {
            p.spawn((TextSpan::new(format!("\n{heading}  ")), font.clone(), TextColor(HEADING)));
            p.spawn((TextSpan::new(sentence.to_string()), font.clone(), TextColor(BODY)));
        }
        p.spawn((TextSpan::new("\n\nI: close"), font.clone(), TextColor(Color::srgba(1.0, 1.0, 1.0, 0.5))));
    });
}

/// What follows from the row: size, pull, escape, horizon, sunlight, air.
fn numbers(b: &Body) -> Vec<String> {
    let thousands = |v: f32| {
        let s = format!("{:.0}", v);
        let mut out = String::new();
        for (k, c) in s.chars().enumerate() {
            if k > 0 && (s.len() - k) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out
    };
    let escape = (2.0 * b.gravity * b.radius).sqrt();
    let escape = if escape < 1000.0 { format!("{escape:.0} m/s") } else { format!("{:.1} km/s", escape / 1000.0) };
    let share = b.gravity / 9.81 * 100.0;
    let share = if share < 1.0 { format!("{share:.2}") } else { format!("{share:.0}") };
    // How far the ground meets the sky for eyes 1.7 m up, on a smooth ball.
    let horizon = (2.0 * b.radius * 1.7).sqrt();
    let sunlight = 100.0 / (b.au * b.au);
    let sunlight = if sunlight >= 10.0 { format!("{sunlight:.0} %") } else if sunlight >= 0.1 { format!("{sunlight:.1} %") } else { format!("{:.0} ‰", sunlight * 10.0) };
    let sun = 2.0 * b.sun_radius() * 60.0;
    let sun = if sun >= 1.0 { format!("{sun:.0}′ across") } else { format!("{:.0}″ across, a point", sun * 60.0) };
    let mut lines = vec![
        format!(
            "Radius {} km · gravity {} m/s², {share} % of Earth's · escape velocity {escape}",
            thousands(b.radius / 1000.0),
            if b.gravity < 0.1 { format!("{:.4}", b.gravity) } else { format!("{:.2}", b.gravity) }
        ),
        format!("Sun at {:.2} AU: {sunlight} of the sunlight on Earth; its disc {sun} (32′ from Earth)", b.au),
        format!(
            "The horizon {} away for eyes 1.7 m up (4.7 km on Earth)",
            if horizon < 1000.0 { format!("{:.0} m", horizon) } else { format!("{:.1} km", horizon / 1000.0) }
        ),
    ];
    if b.air_density > 0.0 {
        let density = if b.air_density < 0.1 { format!("{:.3}", b.air_density) } else { format!("{:.1}", b.air_density) };
        let mut air = format!("Air at the ground {density} kg/m³ (Earth 1.2), sound at {:.0} m/s", b.sound_speed);
        if let Some(craft) = b.craft {
            air.push_str(&format!(" · {} flies here: R beside it", craft.name()));
        }
        lines.push(air);
    } else if !b.air.is_none() {
        lines.push("A trace of air: enough for a haze, too thin to carry sound or hold up a rotor".into());
    } else {
        lines.push("No air: no sound, no wind, no colour in the sky, nothing a rotor can push on".into());
    }
    lines
}
