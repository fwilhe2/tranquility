//! Helicopters, where the air will hold one up: Ingenuity on Mars and
//! Dragonfly on Titan, to their published dimensions, flown through the
//! engine's rotor physics ([`tranquility::rotor`]).
//!
//! Mars's air is a hundredth of Earth's, so Ingenuity is all rotor: 1.8 kg
//! under two counter-rotating rotors 1.21 m across, turning at 2537 rpm
//! with their tips at two thirds of the speed of sound, and it needs some
//! 350 W to hover. It made 72 flights from 2021 to January 2024, when a
//! landing broke its blades. Titan is the other way round: four times
//! Earth's air density under a seventh of its gravity, so a lander near a
//! tonne flies on eight small rotors in four coaxial pairs on a few
//! kilowatts. That is Dragonfly, due to launch in 2028 and to fly from
//! dune to dune from 2034. Both were built to fly themselves: Mars is
//! minutes of light away, Titan more than an hour. Here you are standing
//! beside them, so you can do what their teams could not and fly them by
//! hand, through the same kind of flight computer.
//!
//! **R** next to one takes the controls (and gives them back: it lands
//! itself); **Space** spins the rotors up and climbs, **C** descends,
//! **W/S** fly forward and back (faster with **Shift**), **A/D** turn,
//! **←/→** slip sideways, **V** changes between a camera behind it, its own
//! camera, and your eyes where you stand.

use std::f32::consts::{PI, TAU};

use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    math::DVec2,
    mesh::{Indices, PrimitiveTopology},
};
use tranquility::{
    ground::prop,
    prelude::*,
    rotor::{Airflow, Command, Fly, Rotor, RotorPlugin, Rotorcraft, RotorcraftSpec},
};

use crate::{
    Current,
    rover::{Driving, Rover, Thrown},
};

/// Which helicopter a body has, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Craft {
    Ingenuity,
    Dragonfly,
}

impl Craft {
    pub fn name(self) -> &'static str {
        match self {
            Craft::Ingenuity => "Ingenuity",
            Craft::Dragonfly => "Dragonfly",
        }
    }

    fn spec(self) -> RotorcraftSpec {
        match self {
            // Centre of mass in the fuselage, 0.2 m up. Rotors 1.21 m
            // across, the lower 0.33 m and the upper 0.42 m above the
            // ground; broad carbon blades, about 0.15 of the disc. The
            // motors: about 510 W from the battery at most.
            Craft::Ingenuity => {
                let rotor = |y: f32, turn: f32, in_wake: bool| Rotor {
                    hub: Vec3::new(0.0, y, 0.0),
                    radius: 0.605,
                    blades: 2,
                    solidity: 0.15,
                    turn,
                    in_wake,
                    cant: 0.0,
                };
                RotorcraftSpec {
                    mass: 1.8,
                    inertia: Vec3::new(0.045, 0.03, 0.045),
                    rotors: vec![rotor(0.22, 1.0, false), rotor(0.13, -1.0, true)],
                    rpm: 2537.0,
                    spin_up: 6.0,
                    lift_slope: 5.7,
                    // Blades at a Reynolds number of some 10⁴: draggy.
                    profile_drag: 0.035,
                    max_pitch: 0.4,
                    stall: 0.11,
                    cyclic: 0.15,
                    cyclic_arm: 0.35,
                    response: 0.03,
                    power: 330.0,
                    efficiency: 0.65,
                    avionics: 20.0,
                    // Six lithium-ion cells of some 36 Wh; a flight may use
                    // the share the night's heating does not need.
                    battery: 20.0 * 3600.0,
                    drag_area: 0.03,
                    feet: FEET_INGENUITY.to_vec(),
                    grip: 0.6,
                    // Its fastest was 10 m/s, its highest 24 m.
                    top_speed: 10.0,
                    climb: 2.0,
                    descent: 1.0,
                    turn_rate: 0.6,
                    max_tilt: 0.3,
                    agility: 6.0,
                    toughness: 2.0,
                }
            }
            // About 3.9 m long, 875 kg; eight rotors 1.35 m across, in
            // coaxial pairs at the corners, canted for yaw. A guess at the
            // battery: enough for the half-hour flights planned.
            Craft::Dragonfly => {
                let mut rotors = Vec::new();
                for (x, z) in ROTORS_DRAGONFLY {
                    let turn = if (x > 0.0) == (z > 0.0) { 1.0 } else { -1.0 };
                    rotors.push(Rotor { hub: Vec3::new(x, 0.55, z), radius: 0.675, blades: 2, solidity: 0.1, turn, in_wake: false, cant: 0.1 });
                    rotors.push(Rotor { hub: Vec3::new(x, 0.3, z), radius: 0.675, blades: 2, solidity: 0.1, turn: -turn, in_wake: true, cant: 0.1 });
                }
                RotorcraftSpec {
                    mass: 875.0,
                    inertia: Vec3::new(1250.0, 1600.0, 900.0),
                    rotors,
                    rpm: 700.0,
                    spin_up: 10.0,
                    lift_slope: 5.7,
                    profile_drag: 0.012,
                    max_pitch: 0.35,
                    stall: 0.13,
                    cyclic: 0.0,
                    cyclic_arm: 0.0,
                    response: 0.15,
                    power: 12_000.0,
                    efficiency: 0.85,
                    avionics: 300.0,
                    battery: 4000.0 * 3600.0,
                    drag_area: 2.5,
                    feet: FEET_DRAGONFLY.to_vec(),
                    grip: 0.5,
                    top_speed: 10.0,
                    climb: 2.0,
                    descent: 1.5,
                    turn_rate: 0.3,
                    max_tilt: 0.25,
                    agility: 2.5,
                    toughness: 1.5,
                }
            }
        }
    }

    /// How near you must stand to take the controls, m.
    fn reach(self) -> f32 {
        match self {
            Craft::Ingenuity => 3.0,
            Craft::Dragonfly => 7.0,
        }
    }

    /// The camera behind it: how far back and how far up, m.
    fn chase(self) -> (f32, f32) {
        match self {
            Craft::Ingenuity => (1.9, 0.6),
            Craft::Dragonfly => (12.0, 4.0),
        }
    }

    /// Its own camera: where on it, and how far it looks down, rad.
    /// Ingenuity's colour camera looked ahead and down from the fuselage.
    fn camera(self) -> (Vec3, f32) {
        match self {
            Craft::Ingenuity => (Vec3::new(0.0, -0.05, -0.1), 0.38),
            Craft::Dragonfly => (Vec3::new(0.0, 0.75, -1.35), 0.25),
        }
    }

    /// Half its footprint, m: how near a walker may come.
    fn footprint(self) -> Vec2 {
        match self {
            Craft::Ingenuity => Vec2::new(0.3, 0.3),
            Craft::Dragonfly => Vec2::new(1.9, 2.0),
        }
    }

    /// Said when a flat battery is found charged again.
    fn charged(self) -> &'static str {
        match self {
            Craft::Ingenuity => "charged again: a sol of sunshine on its panel",
            Craft::Dragonfly => "charged again: its plutonium generator, over the Titan night",
        }
    }
}

const FEET_INGENUITY: [Vec3; 4] = [Vec3::new(-0.25, -0.2, -0.25), Vec3::new(0.25, -0.2, -0.25), Vec3::new(-0.25, -0.2, 0.25), Vec3::new(0.25, -0.2, 0.25)];
const FEET_DRAGONFLY: [Vec3; 4] = [Vec3::new(-0.62, -0.92, -1.15), Vec3::new(0.62, -0.92, -1.15), Vec3::new(-0.62, -0.92, 1.15), Vec3::new(0.62, -0.92, 1.15)];
const ROTORS_DRAGONFLY: [(f32, f32); 4] = [(-1.25, -1.25), (1.25, -1.25), (-1.25, 1.25), (1.25, 1.25)];

pub struct RotorcraftPlugin;

impl Plugin for RotorcraftPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RotorPlugin).add_message::<Thrown>().init_resource::<Piloting>().add_systems(Startup, setup).add_systems(
            Update,
            (
                (park.run_if(resource_changed::<WorldDef>).after(crate::rover::Parked), take.before(crate::rover::Boarding), pilot).chain().before(Fly),
                (pose, watch, downwash).chain().after(Fly),
                stand_clear.after(Walking),
            ),
        );
    }
}

#[derive(Component)]
pub struct Helicopter(pub Craft);

/// A rotor's hub, turning, by index into the craft's rotors; the blades on
/// it, and the blur they make at speed.
#[derive(Component)]
struct Hub(usize);
#[derive(Component)]
struct Blades;
#[derive(Component)]
struct Blur;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum View {
    #[default]
    Behind,
    Onboard,
    Eyes,
}

/// Whether you have the controls, which camera, and where you stand.
#[derive(Resource, Default)]
pub struct Piloting {
    pub on: bool,
    view: View,
    /// Where you stood when you took the controls: world position, the
    /// eye's height, and which way you looked.
    stand: Option<(DVec2, f32, f32, f32)>,
    /// The chase camera's place, eased.
    behind: Option<Vec3>,
    /// What has been said this flight.
    low: bool,
    wreck: bool,
    since_down: f32,
    /// The app's line, put aside for the instruments while you fly.
    info: Option<String>,
}

#[derive(Resource)]
struct Kit {
    cube: Handle<Mesh>,
    tube: Handle<Mesh>,
    disc: Handle<Mesh>,
    ball: Handle<Mesh>,
    dish: Handle<Mesh>,
    paddle: Handle<Mesh>,
    slender: Handle<Mesh>,
    film: Handle<PropMaterial>,
    carbon: Handle<PropMaterial>,
    cells: Handle<PropMaterial>,
    shell: Handle<PropMaterial>,
    grey: Handle<PropMaterial>,
    dark: Handle<PropMaterial>,
    gold: Handle<PropMaterial>,
    blur: Handle<PropMaterial>,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PropMaterial>>) {
    let mut material = |r: f32, g: f32, b: f32, roughness: f32, metallic: f32| {
        materials.add(prop(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: roughness, metallic, ..default() }))
    };
    let film = material(0.42, 0.36, 0.27, 0.35, 0.8);
    let carbon = material(0.035, 0.035, 0.04, 0.45, 0.1);
    let cells = material(0.02, 0.03, 0.08, 0.18, 0.3);
    let shell = material(0.62, 0.62, 0.6, 0.7, 0.0);
    let grey = material(0.3, 0.3, 0.31, 0.5, 0.6);
    let dark = material(0.06, 0.06, 0.065, 0.6, 0.2);
    let gold = material(0.62, 0.4, 0.1, 0.3, 0.8);
    let blur = materials.add(prop(StandardMaterial {
        base_color: Color::linear_rgba(0.03, 0.03, 0.035, 0.22),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.6,
        cull_mode: None,
        double_sided: true,
        ..default()
    }));
    commands.insert_resource(Kit {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        tube: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(12)),
        disc: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(40)),
        ball: meshes.add(Sphere::new(1.0).mesh().ico(2).expect("valid subdivision")),
        dish: meshes.add(ConicalFrustum { radius_top: 1.0, radius_bottom: 0.15, height: 0.3 }.mesh().resolution(28)),
        // Ingenuity's blades are broad paddles; Dragonfly's slender.
        paddle: meshes.add(blade(&[(0.06, 0.05), (0.25, 0.14), (0.55, 0.15), (0.85, 0.11), (1.0, 0.05)])),
        slender: meshes.add(blade(&[(0.12, 0.05), (0.3, 0.1), (0.7, 0.09), (1.0, 0.05)])),
        film,
        carbon,
        cells,
        shell,
        grey,
        dark,
        gold,
        blur,
    });
}

/// A blade along +x out to 1, flat, from (span, chord) stations, chord as a
/// share of the radius; drawn from both sides.
fn blade(stations: &[(f32, f32)]) -> Mesh {
    let mut positions = Vec::new();
    for &(r, c) in stations {
        // The leading edge straight, the chord behind it.
        positions.push([r, 0.0, -0.3 * c]);
        positions.push([r, 0.0, 0.7 * c]);
    }
    let mut indices = Vec::new();
    for k in 0..stations.len() as u32 - 1 {
        let (a, b, c, d) = (2 * k, 2 * k + 1, 2 * k + 2, 2 * k + 3);
        indices.extend([a, b, c, b, d, c]);
    }
    let n = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_indices(Indices::U32(indices))
}

/// Sets a body's helicopter down beside where you arrive, on level, dry
/// ground off to the right (the rover parks ahead and to the left).
fn park(
    mut commands: Commands,
    current: Res<Current>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    kit: Res<Kit>,
    old: Query<Entity, With<Helicopter>>,
    mut piloting: ResMut<Piloting>,
    mut hud: ResMut<Hud>,
    mut player: Single<&mut Player>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    if piloting.on {
        // Gone to another body with the controls in hand.
        player.controlled = true;
        hud.mode.clear();
    }
    *piloting = Piloting::default();
    let body = &current.0;
    let Some(craft) = body.craft else { return };
    let heading = Quat::from_rotation_y(world.spawn.yaw);
    let near = match craft {
        Craft::Ingenuity => 6.0,
        Craft::Dragonfly => 16.0,
    };
    let mut best = (f32::MAX, DVec2::ZERO);
    for k in 0..20 {
        let offset = heading * Quat::from_rotation_y(-0.75 - 0.05 * k as f32) * Vec3::new(0.0, 0.0, -(near + 0.4 * k as f32));
        let at = DVec2::new(world.spawn.x + offset.x as f64, world.spawn.z + offset.z as f64);
        let h = |dx: f64, dz: f64| world.ground.height(at.x + dx, at.y + dz);
        let tilt = (h(1.5, 0.0) - h(-1.5, 0.0)).abs() + (h(0.0, 1.5) - h(0.0, -1.5)).abs();
        let dry = body.sea.as_ref().is_none_or(|sea| h(0.0, 0.0) > sea.level + 0.5);
        if dry && !world.ground.blocked(at.x, at.y) && tilt < best.0 {
            best = (tilt, at);
        }
    }
    let at = best.1;
    let rotorcraft = Rotorcraft::new(craft.spec());
    let y = world.ground.height(at.x, at.y) + rotorcraft.rest_height() + 0.02;
    let entity = commands
        .spawn((
            Helicopter(craft),
            rotorcraft,
            Transform::from_translation(origin.render(at, y)).with_rotation(heading * Quat::from_rotation_y(-0.6)),
            Visibility::default(),
        ))
        .id();
    match craft {
        Craft::Ingenuity => ingenuity(&mut commands, entity, &kit),
        Craft::Dragonfly => dragonfly(&mut commands, entity, &kit),
    }
}

fn between(a: Vec3, b: Vec3, radius: f32) -> Transform {
    let axis = b - a;
    Transform::from_translation((a + b) / 2.0)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis.normalize_or(Vec3::Y)))
        .with_scale(Vec3::new(radius, axis.length(), radius))
}

/// A rotor: a hub that turns, with its blades, and the disc they blur into.
fn rotor(parent: &mut ChildSpawnerCommands, kit: &Kit, k: usize, rotor: &Rotor, blade: &Handle<Mesh>, material: &Handle<PropMaterial>) {
    let r = rotor.radius;
    parent.spawn((Hub(k), Transform::from_translation(rotor.hub), Visibility::default())).with_children(|hub| {
        hub.spawn((Mesh3d(kit.tube.clone()), MeshMaterial3d(kit.grey.clone()), Transform::from_scale(Vec3::new(0.05 * r, 0.06 * r, 0.05 * r))));
        hub.spawn((Blades, Transform::default(), Visibility::default())).with_children(|blades| {
            for b in 0..rotor.blades {
                let around = Quat::from_rotation_y(b as f32 * TAU / rotor.blades as f32);
                blades.spawn((Mesh3d(blade.clone()), MeshMaterial3d(material.clone()), Transform::from_rotation(around).with_scale(Vec3::splat(r))));
            }
        });
    });
    // The blur does not turn, so it hangs off the craft, not the hub.
    parent.spawn((
        Blur,
        Mesh3d(kit.disc.clone()),
        MeshMaterial3d(kit.blur.clone()),
        Transform::from_translation(rotor.hub).with_scale(Vec3::new(r, 0.004, r)),
        Visibility::Hidden,
        NotShadowCaster,
    ));
}

/// Ingenuity, in its own frame from the centre of mass (ground at −0.2):
/// a fuselage the size of a tissue box wrapped in film, four splayed
/// carbon legs, the mast, the two rotors and the solar panel on top.
fn ingenuity(commands: &mut Commands, entity: Entity, kit: &Kit) {
    let spec = Craft::Ingenuity.spec();
    commands.entity(entity).with_children(|c| {
        let part = |c: &mut ChildSpawnerCommands, mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, transform: Transform| {
            c.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform));
        };
        part(c, &kit.cube, &kit.film, Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(0.195, 0.136, 0.163)));
        // The colour camera under the front, the navigation camera beneath.
        part(c, &kit.cube, &kit.dark, Transform::from_xyz(0.0, -0.05, -0.085).with_scale(Vec3::new(0.04, 0.03, 0.02)));
        part(c, &kit.tube, &kit.dark, Transform::from_xyz(0.03, -0.072, 0.0).with_scale(Vec3::new(0.012, 0.012, 0.012)));
        for foot in FEET_INGENUITY {
            let top = Vec3::new(0.085 * foot.x.signum(), -0.06, 0.07 * foot.z.signum());
            part(c, &kit.tube, &kit.carbon, between(top, foot + Vec3::Y * 0.01, 0.0065));
            part(c, &kit.disc, &kit.grey, Transform::from_translation(foot + Vec3::Y * 0.004).with_scale(Vec3::new(0.025, 0.008, 0.025)));
        }
        part(c, &kit.tube, &kit.grey, between(Vec3::new(0.0, 0.068, 0.0), Vec3::new(0.0, 0.285, 0.0), 0.011));
        // The solar panel: cells in a carbon frame, over the upper rotor.
        part(c, &kit.cube, &kit.carbon, Transform::from_xyz(0.0, 0.282, 0.0).with_scale(Vec3::new(0.43, 0.006, 0.17)));
        part(c, &kit.cube, &kit.cells, Transform::from_xyz(0.0, 0.286, 0.0).with_scale(Vec3::new(0.41, 0.003, 0.15)));
        // The radio's antenna on the panel's edge.
        part(c, &kit.tube, &kit.grey, between(Vec3::new(0.19, 0.29, 0.06), Vec3::new(0.19, 0.33, 0.06), 0.004));
        for (k, r) in spec.rotors.iter().enumerate() {
            rotor(c, kit, k, r, &kit.paddle, &kit.carbon);
        }
    });
}

/// Dragonfly, in its own frame (ground at −0.92): a lander body in its
/// insulating shell on two skids, the rotor pairs at the ends of four
/// booms, the plutonium generator behind, the dish on top and the camera
/// mast ahead.
fn dragonfly(commands: &mut Commands, entity: Entity, kit: &Kit) {
    let spec = Craft::Dragonfly.spec();
    commands.entity(entity).with_children(|c| {
        let part = |c: &mut ChildSpawnerCommands, mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, transform: Transform| {
            c.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform));
        };
        part(c, &kit.cube, &kit.shell, Transform::from_xyz(0.0, -0.05, 0.0).with_scale(Vec3::new(1.1, 0.9, 2.9)));
        // A bevelled nose, and the darker deck.
        part(c, &kit.cube, &kit.shell, Transform::from_xyz(0.0, -0.12, -1.48).with_rotation(Quat::from_rotation_x(0.5)).with_scale(Vec3::new(1.06, 0.6, 0.4)));
        part(c, &kit.cube, &kit.grey, Transform::from_xyz(0.0, 0.41, 0.0).with_scale(Vec3::new(1.0, 0.02, 2.7)));
        // Skids and their struts; the drills at the skids' feet.
        for x in [-0.62f32, 0.62] {
            part(c, &kit.tube, &kit.grey, between(Vec3::new(x, -0.88, -1.4), Vec3::new(x, -0.88, 1.4), 0.045));
            for z in [-0.9f32, 0.9] {
                part(c, &kit.tube, &kit.grey, between(Vec3::new(x * 0.8, -0.5, z), Vec3::new(x, -0.86, z * 1.1), 0.03));
            }
            part(c, &kit.cube, &kit.dark, Transform::from_xyz(x, -0.78, -1.0).with_scale(Vec3::new(0.08, 0.16, 0.08)));
        }
        // Booms out to the rotor pairs, and the posts they turn on.
        for (x, z) in ROTORS_DRAGONFLY {
            let post = Vec3::new(x, 0.0, z);
            part(c, &kit.tube, &kit.grey, between(Vec3::new(0.5 * x.signum(), 0.3, 1.05 * z.signum()), post + Vec3::Y * 0.42, 0.05));
            part(c, &kit.tube, &kit.grey, between(Vec3::new(0.5 * x.signum(), -0.3, 1.05 * z.signum()), post + Vec3::Y * 0.2, 0.035));
            part(c, &kit.tube, &kit.dark, between(post + Vec3::Y * 0.18, post + Vec3::Y * 0.6, 0.07));
        }
        // The generator, its fins, behind and down at an angle.
        let rtg = Transform::from_xyz(0.0, 0.05, 1.72).with_rotation(Quat::from_rotation_x(-0.6));
        part(c, &kit.tube, &kit.dark, rtg * Transform::from_scale(Vec3::new(0.23, 0.66, 0.23)));
        for k in 0..8 {
            let around = Quat::from_rotation_y(k as f32 * TAU / 8.0);
            part(c, &kit.cube, &kit.dark, rtg * Transform::from_rotation(around) * Transform::from_xyz(0.0, 0.0, 0.3).with_scale(Vec3::new(0.012, 0.6, 0.16)));
        }
        // The high-gain antenna, tipped back toward an Earth low in the
        // haze, and its feed.
        let dish = Transform::from_xyz(0.0, 0.62, 0.5).with_rotation(Quat::from_rotation_x(0.35));
        part(c, &kit.dish, &kit.shell, dish * Transform::from_scale(Vec3::new(0.5, 0.5, 0.5)));
        part(c, &kit.tube, &kit.grey, between(Vec3::new(0.0, 0.42, 0.5), Vec3::new(0.0, 0.55, 0.5), 0.06));
        part(c, &kit.tube, &kit.grey, dish * between(Vec3::ZERO, Vec3::Y * 0.45, 0.012));
        // The camera mast ahead, with its head.
        part(c, &kit.tube, &kit.grey, between(Vec3::new(0.0, 0.42, -1.2), Vec3::new(0.0, 0.75, -1.2), 0.03));
        part(c, &kit.cube, &kit.dark, Transform::from_xyz(0.0, 0.8, -1.22).with_scale(Vec3::new(0.24, 0.1, 0.1)));
        part(c, &kit.ball, &kit.gold, Transform::from_xyz(0.0, 0.2, -1.47).with_scale(Vec3::splat(0.05)));
        for (k, r) in spec.rotors.iter().enumerate() {
            rotor(c, kit, k, r, &kit.slender, &kit.carbon);
        }
    });
}

/// **R**: the controls, or back to your own two feet. Runs before the
/// rover's own **R**, and takes the key when it is this one's.
fn take(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    driving: Option<Res<Driving>>,
    mut piloting: ResMut<Piloting>,
    mut hud: ResMut<Hud>,
    player: Single<(&mut Player, &mut Transform), (Without<Helicopter>, Without<Rover>)>,
    craft: Option<Single<(&Helicopter, &mut Rotorcraft, &mut Transform), Without<Rover>>>,
    rover: Option<Single<&Transform, (With<Rover>, Without<Helicopter>)>>,
) {
    let Some(craft) = craft else { return };
    let (which, mut rc, mut pose) = craft.into_inner();
    let (mut player, mut eye) = player.into_inner();
    // MOON_FLY=behind|onboard|eyes starts at the controls, for looking at
    // it without sitting at the window; with "up" it climbs away, with
    // "low" it hovers just off the ground, with "side" the camera behind
    // it looks from the side.
    let demo = std::env::var("MOON_FLY").unwrap_or_default();
    let start = !demo.is_empty() && !piloting.on && piloting.stand.is_none() && player.controlled && !player.flying;
    if keys.just_pressed(KeyCode::KeyV) && piloting.on {
        piloting.view = match piloting.view {
            View::Behind => View::Onboard,
            View::Onboard => View::Eyes,
            View::Eyes => View::Behind,
        };
        piloting.behind = None;
        hud.say(match piloting.view {
            View::Behind => "the camera behind it",
            View::Onboard => "its own camera",
            View::Eyes => "your own eyes, where you stand",
        });
    }
    if !keys.just_pressed(KeyCode::KeyR) && !start || driving.is_some_and(|d| d.aboard) {
        return;
    }
    if piloting.on {
        keys.clear_just_pressed(KeyCode::KeyR);
        piloting.on = false;
        if let Some((at, y, yaw, pitch)) = piloting.stand.take() {
            let back = origin.render(at, y);
            player.stand_at(&mut eye, &world, &origin, back);
            (player.yaw, player.pitch) = (yaw, pitch);
            eye.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        }
        player.controlled = true;
        hud.mode.clear();
        if let Some(info) = piloting.info.take() {
            hud.info = info;
        }
        if rc.wrecked.is_some() {
            // A real one would be lost. This one is set right again.
            let forward = pose.forward();
            let at = origin.world(pose.translation);
            let floor = world.ground.height(at.x, at.y);
            rc.reset();
            pose.translation.y = floor + rc.rest_height() + 0.02;
            pose.rotation = Quat::from_rotation_y((-forward.x).atan2(-forward.z));
            hud.say("set back on its feet, as a real one never would be");
        } else if !rc.grounded {
            hud.say(format!("{} lands itself", which.0.name()));
        }
        return;
    }
    if player.flying {
        return;
    }
    let to = pose.translation - eye.translation;
    let distance = Vec2::new(to.x, to.z).length();
    let rover_distance = rover.map_or(f32::MAX, |r| Vec2::new(r.translation.x - eye.translation.x, r.translation.z - eye.translation.z).length());
    if distance < which.0.reach() || start {
        if rover_distance < distance && !start {
            // The rover is nearer: the key is its.
            return;
        }
        keys.clear_just_pressed(KeyCode::KeyR);
        piloting.on = true;
        piloting.behind = None;
        piloting.view = if demo.contains("onboard") {
            View::Onboard
        } else if demo.contains("eyes") {
            View::Eyes
        } else {
            View::Behind
        };
        piloting.stand = Some((origin.world(eye.translation), eye.translation.y, player.yaw, player.pitch));
        (piloting.low, piloting.wreck, piloting.since_down) = (false, false, 0.0);
        piloting.info = Some(hud.info.clone());
        player.controlled = false;
        player.velocity = Vec3::ZERO;
        (player.yaw, player.pitch) = (if demo.contains("side") { 2.2 } else { 0.0 }, -0.12);
        if rc.energy < 0.5 * rc.spec.battery && rc.parked() {
            rc.energy = rc.spec.battery;
            hud.say(which.0.charged());
        } else {
            hud.say("Space: spin up and climb · C down · W/S forward, back · A/D turn · ←/→ sideways · V view · R let go");
        }
        return;
    }
    if distance < rover_distance {
        keys.clear_just_pressed(KeyCode::KeyR);
        let bearing = (-to.x).atan2(-to.z);
        let hour = ((player.yaw - bearing) / TAU * 12.0).rem_euclid(12.0).round() as u32;
        let hour = if hour == 0 { 12 } else { hour };
        hud.say(format!("{} is {distance:.0} m away, at {hour} o'clock: walk up to it, then R", which.0.name()));
    }
}

/// The keys into the flight computer, and the wind into its air; a craft
/// let go lands itself and stops its rotors.
fn pilot(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    current: Res<Current>,
    mut piloting: ResMut<Piloting>,
    craft: Option<Single<&mut Rotorcraft>>,
) {
    let Some(mut rc) = craft else { return };
    let body = &current.0;
    // The wind ten metres up: the body's usual breeze, gusting and slowly
    // veering.
    let t = time.elapsed_secs();
    let gust = 1.0 + 0.35 * (0.31 * t).sin() + 0.2 * (1.7 * t + 2.0).sin() + 0.08 * (5.3 * t + 0.4).sin();
    let from = 2.0 + 0.3 * (0.05 * t).sin();
    let wind = Vec3::new(from.sin(), 0.0, from.cos()) * body.wind * gust.max(0.0);
    rc.air = Airflow { density: body.air_density, sound: body.sound_speed, wind };
    let dt = time.delta_secs();
    let s = rc.spec.clone();
    if !piloting.on {
        // Let go: it comes down where it is, and spins down on its feet.
        rc.command = Command { up: if rc.grounded { 0.0 } else { -s.descent }, ..default() };
        if rc.grounded && rc.running && rc.command.up <= 0.0 {
            rc.running = false;
        }
        return;
    }
    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let fast = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let pace = if fast { s.top_speed } else { 0.4 * s.top_speed };
    let mut command = Command {
        forward: axis(KeyCode::KeyW, KeyCode::KeyS) * pace,
        right: axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) * pace * 0.5,
        up: axis(KeyCode::Space, KeyCode::KeyC) * if fast { s.climb } else { 0.5 * s.climb.max(1.0) },
        turn: axis(KeyCode::KeyA, KeyCode::KeyD) * s.turn_rate,
    };
    let demo = std::env::var("MOON_FLY").unwrap_or_default();
    if demo.contains("up") || demo.contains("low") {
        // Up to six metres and away, or a hover just off the ground.
        let (aim, go) = if demo.contains("low") { (0.6, false) } else { (6.0, true) };
        command.up = ((aim - rc.height) * 1.5).clamp(-s.descent, s.climb);
        command.forward = if go && rc.height > 4.0 { 0.5 * s.top_speed } else { 0.0 };
    }
    if command.up > 0.0 && rc.wrecked.is_none() {
        rc.running = true;
    }
    // Down and asked for nothing more for two seconds: it spins down.
    piloting.since_down = if rc.grounded && command.up <= 0.0 { piloting.since_down + dt } else { 0.0 };
    if piloting.since_down > 2.0 {
        rc.running = false;
    }
    rc.command = command;
}

fn pose(
    craft: Option<Single<&Rotorcraft>>,
    mut hubs: Query<(&Hub, &mut Transform, &Children)>,
    mut blades: Query<&mut Visibility, (With<Blades>, Without<Blur>)>,
    mut blurs: Query<&mut Visibility, (With<Blur>, Without<Blades>)>,
) {
    let Some(rc) = craft else { return };
    // Turning slowly the blades show; fast, they are a disc and nothing
    // more, as a camera sees them.
    let fast = rc.rpm > 0.15 * rc.spec.rpm;
    for (hub, mut transform, children) in &mut hubs {
        transform.rotation = Quat::from_rotation_y(rc.azimuth[hub.0]);
        for child in children {
            if let Ok(mut v) = blades.get_mut(*child) {
                v.set_if_neq(if fast { Visibility::Hidden } else { Visibility::Inherited });
            }
        }
    }
    for mut v in &mut blurs {
        v.set_if_neq(if fast { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// The camera while you fly, and the status line.
#[allow(clippy::too_many_arguments)]
fn watch(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut shifted: MessageReader<OriginShifted>,
    mut piloting: ResMut<Piloting>,
    mut hud: ResMut<Hud>,
    player: Single<(&Player, &mut Transform), Without<Helicopter>>,
    craft: Option<Single<(&Helicopter, &Rotorcraft, &Transform)>>,
) {
    for shift in shifted.read() {
        if let Some(at) = piloting.behind.as_mut() {
            *at -= shift.delta;
        }
    }
    let Some(craft) = craft else { return };
    if !piloting.on {
        return;
    }
    let (which, rc, pose) = craft.into_inner();
    let (player, mut eye) = player.into_inner();
    let forward = pose.forward();
    let yaw = (-forward.x).atan2(-forward.z);
    match piloting.view {
        View::Behind => {
            let (back, up) = which.0.chase();
            let heading = Quat::from_rotation_y(yaw + player.yaw);
            let target = pose.translation + Vec3::Y * 0.3 * up;
            let want = target + heading * Vec3::new(0.0, up - back * player.pitch.min(0.3), back);
            let ease = 1.0 - (-time.delta_secs() * 3.0).exp();
            let mut at = piloting.behind.map_or(want, |at| at + (want - at) * ease);
            let w = origin.world(at);
            at.y = at.y.max(world.ground.height(w.x, w.y) + 0.3);
            piloting.behind = Some(at);
            *eye = Transform::from_translation(at).looking_at(target, Vec3::Y);
        }
        View::Onboard => {
            let (at, down) = which.0.camera();
            eye.translation = pose.translation + pose.rotation * at;
            eye.rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-down);
        }
        View::Eyes => {
            if let Some((at, y, _, _)) = piloting.stand {
                eye.translation = origin.render(at, y);
            }
            let to = pose.translation - eye.translation;
            if to.length() > 0.5 {
                *eye = eye.looking_to(to, Vec3::Y);
            }
        }
    }
    // Things worth saying once a flight.
    let left = rc.energy / rc.spec.battery;
    if left < 0.2 && !piloting.low {
        piloting.low = true;
        hud.say("battery low: bring it down");
    }
    if let Some(why) = &rc.wrecked
        && !piloting.wreck
    {
        piloting.wreck = true;
        hud.say(format!("{} {why}. R to let go", which.0.name()));
    }
    let flat = Vec2::new(rc.velocity.x, rc.velocity.z).length();
    let state = if let Some(why) = &rc.wrecked {
        format!("wrecked: {why}")
    } else if rc.rpm == 0.0 {
        "rotors stopped: Space starts them".to_string()
    } else if rc.rpm < 0.97 * rc.spec.rpm {
        format!("rotors spinning {}, {:.0} %", if rc.running { "up" } else { "down" }, rc.rpm / rc.spec.rpm * 100.0)
    } else if rc.grounded {
        "on the ground, rotors at speed: Space lifts off".to_string()
    } else {
        format!("{:.1} m up · climbing {:+.1} m/s · {:.1} m/s over the ground", rc.height, rc.velocity.y, flat)
    };
    let limit = if rc.power_limited {
        " (motors at full power)"
    } else if rc.stalled {
        " (blades stalling)"
    } else {
        ""
    };
    let wind = rc.air.wind_at(rc.height + rc.rest_height()).length();
    let info = format!(
        "{} · {}\n{:.0} rpm, tips at Mach {:.2} · {:.0} W from the battery, {:.0} % left · thrust to spare: {:.1}× its weight{} · air {} kg/m³, wind {:.1} m/s",
        which.0.name(),
        state,
        rc.rpm,
        rc.tip_mach(),
        // Rounded to settle the line.
        (rc.drawn / 5.0).round() * 5.0,
        left * 100.0,
        rc.margin,
        limit,
        if rc.air.density < 0.1 { format!("{:.3}", rc.air.density) } else { format!("{:.1}", rc.air.density) },
        wind,
    );
    if hud.info != info {
        hud.info = info;
    }
    let mode = format!("FLY {} (R: let go, V: view)", which.0.name());
    if hud.mode != mode {
        hud.mode = mode;
    }
}

/// Grains a dust-raising wind just lifts: diameter, m, and density,
/// kg/m³ (basalt dust; on Titan, organic sand is lighter, which only
/// lowers its threshold further).
const GRAIN: f32 = 1e-4;
const GRAIN_DENSITY: f32 = 2000.0;

/// The downwash raises the ground. Under a hovering rotor the air leaves
/// at twice the induced velocity, turns at the ground and runs out along
/// it as a wall jet, which loses speed as it spreads. Where its friction
/// speed (some 7 % of the jet's) passes the threshold that lifts a grain,
/// which goes as √(grain density · g · diameter / air density), the soil
/// goes up. On Mars that takes a jet near 9 m/s, so only a low hover
/// raises dust, as Perseverance filmed. Titan's sand would lift in a
/// breath.
#[allow(clippy::too_many_arguments)]
fn downwash(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    current: Res<Current>,
    craft: Option<Single<(&Rotorcraft, &Transform)>>,
    mut thrown: MessageWriter<Thrown>,
    mut seed: Local<u32>,
) {
    let Some(craft) = craft else { return };
    let (rc, pose) = craft.into_inner();
    let rho = rc.air.density;
    if rho <= 0.0 || rc.rpm < 0.3 * rc.spec.rpm {
        return;
    }
    let mut random = || {
        *seed = seed.wrapping_mul(747796405).wrapping_add(2891336453);
        let x = (*seed >> ((*seed >> 28) + 4)) ^ *seed;
        (x.wrapping_mul(277803737) >> 8) as f32 / 16777216.0
    };
    let g = world.walker.gravity;
    let threshold = 0.1 * (GRAIN_DENSITY * g * GRAIN / rho).sqrt();
    let soil = &current.0.soil;
    let loose = soil.craterlets.max(soil.ripples).max(0.3);
    let n = rc.spec.rotors.len() as f32;
    let dt = time.delta_secs();
    for rotor in &rc.spec.rotors {
        if rotor.in_wake {
            // A coaxial pair blows as one.
            continue;
        }
        let r = rotor.radius;
        let hub = pose.translation + pose.rotation * rotor.hub;
        let w = origin.world(hub);
        let floor = world.ground.height(w.x, w.y);
        let above = hub.y - floor;
        let area = PI * r * r;
        let thrust: f32 = rc.thrust.iter().enumerate().filter(|(j, _)| rc.spec.rotors[*j].hub.xz() == rotor.hub.xz()).map(|(_, t)| *t).sum();
        let wake = 2.0 * (thrust / (2.0 * rho * area)).sqrt();
        // The jet at the ground: the whole wake within a couple of radii,
        // fading out by five.
        let jet = wake * (1.0 - (above / r - 2.0) / 3.0).clamp(0.0, 1.0);
        let excess = (0.07 * jet / threshold - 1.0).clamp(0.0, 2.0);
        if excess <= 0.0 {
            continue;
        }
        let count = (1500.0 / n.max(1.0) * excess * loose * dt + random()) as usize;
        let centre = Vec3::new(hub.x, floor, hub.z);
        for _ in 0..count {
            let a = random() * TAU;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            let reach = r * (0.6 + 1.6 * random());
            let at = centre + out * reach + Vec3::Y * 0.03;
            // Grains leave the ground in hops at a few times the friction
            // speed, out along the jet and up off the ground; the fines
            // among them are a puff, not a speck.
            let friction = 0.07 * jet * (r / reach).sqrt();
            let velocity = out * friction * (0.5 + 1.5 * random()) + Vec3::Y * friction * (0.2 + 0.6 * random());
            let w = origin.world(at);
            thrown.write(Thrown { at, velocity, floor: world.ground.height(w.x, w.y), size: 0.04 + 0.08 * random() * random() });
        }
    }
}

/// Someone on foot cannot walk through it.
fn stand_clear(
    piloting: Res<Piloting>,
    driving: Option<Res<Driving>>,
    craft: Option<Single<(&Helicopter, &Transform)>>,
    player: Single<(&mut Player, &mut Transform), Without<Helicopter>>,
) {
    let Some(craft) = craft else { return };
    let (which, pose) = craft.into_inner();
    let (mut player, mut eye) = player.into_inner();
    if piloting.on || driving.is_some_and(|d| d.aboard) || player.flying || !player.controlled {
        return;
    }
    let half = which.0.footprint() + Vec2::splat(0.3);
    let d = pose.rotation.inverse() * (eye.translation - pose.translation);
    // Only what stands on the ground: you may walk under it in flight.
    if d.y > 2.5 + half.y {
        return;
    }
    let over = half - Vec2::new(d.x.abs(), d.z.abs());
    if over.x <= 0.0 || over.y <= 0.0 {
        return;
    }
    let out = if over.x < over.y { Vec3::X * d.x.signum() * over.x } else { Vec3::Z * d.z.signum() * over.y };
    let out = pose.rotation * out;
    eye.translation += Vec3::new(out.x, 0.0, out.z);
    let normal = Vec3::new(out.x, 0.0, out.z).normalize_or_zero();
    let into = player.velocity.dot(normal).min(0.0);
    player.velocity -= normal * into;
}

