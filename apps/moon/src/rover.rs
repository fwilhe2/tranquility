//! The rover: the Lunar Roving Vehicle, to its real dimensions — 3.1 m
//! long, a 2.3 m wheelbase and 1.83 m track, wheels 82 cm across with
//! tyres of woven wire, both axles steering, four quarter-horsepower hub
//! motors — and its real mass with you and your kit aboard, 480 kg.
//!
//! The driving is the engine's rigid body on springs ([`Vehicle`]); what
//! is here is the machine's numbers, how it looks, the seat and the chase
//! camera, the soil its wheels throw and the tracks they leave. It takes
//! six seconds to reach its 13 km/h on the Moon, because a wire tyre pulls
//! about 0.4 of the weight on it and there is not much weight; it takes as
//! long to stop. On Mars or Mercury the same machine is brisk.
//!
//! **R** climbs aboard (within a few metres) and out again, **WASD**
//! drives, **Space** brakes, **V** changes between the seat and a camera
//! behind.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    math::DVec2,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tranquility::{
    ground::prop,
    prelude::*,
    vehicle::{Drive, Lattice, Vehicle, VehicleSpec, WheelSpec, drawn_height},
};

use crate::Current;

const WHEEL_R: f32 = 0.41;
const WHEELBASE: f32 = 2.3;
const TRACK: f32 = 1.83;
/// Centre of mass above the contact patches, m.
const HCG: f32 = 0.55;
/// The frame's rails above the ground, m: the model is built from there.
const CHASSIS_H: f32 = 0.46;
const TRAVEL: f32 = 0.237;
/// The driver's eyes, in the model's frame (left seat).
const EYE: Vec3 = Vec3::new(-0.33, 1.2, 0.26);
/// How near you must stand to climb aboard, m.
const REACH: f32 = 5.0;

pub struct RoverPlugin;

impl Plugin for RoverPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(tranquility::vehicle::VehiclePlugin)
            .init_resource::<Driving>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    (park.run_if(resource_changed::<WorldDef>), board, steer).chain().before(Drive),
                    (pose_wheels, ride, throw_dust, lay_tracks).chain().after(Drive),
                    (fly_dust, draw_dust).chain().after(Drive),
                ),
            );
    }
}

/// Whether you are at the wheel, and which camera.
#[derive(Resource, Default)]
pub struct Driving {
    pub aboard: bool,
    chase: bool,
    /// The chase camera's place, eased.
    behind: Option<Vec3>,
    demo: bool,
}

#[derive(Component)]
pub struct Rover;

/// A wheel's knuckle (steers and rides the suspension) and its spinning
/// part, by index into the vehicle's wheels.
#[derive(Component)]
struct Knuckle(usize);
#[derive(Component)]
struct Spinner(usize);
/// A suspension arm of a wheel: from its pivot on the frame to the hub.
#[derive(Component)]
struct Arm {
    wheel: usize,
    pivot: Vec3,
    drop: f32,
}
/// The figure in the left seat, seen only from outside.
#[derive(Component)]
struct Crew;

#[derive(Resource)]
struct Parts {
    cube: Handle<Mesh>,
    tube: Handle<Mesh>,
    ball: Handle<Mesh>,
    tyre: Handle<Mesh>,
    hub: Handle<Mesh>,
    fender: Handle<Mesh>,
    dish: Handle<Mesh>,
    alu: Handle<StandardMaterial>,
    dull: Handle<StandardMaterial>,
    blanket: Handle<StandardMaterial>,
    webbing: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    tan: Handle<StandardMaterial>,
    gold: Handle<StandardMaterial>,
    wire: Handle<StandardMaterial>,
    suit: Handle<StandardMaterial>,
    visor: Handle<StandardMaterial>,
}

fn spec(grip: f32, drag: f32) -> VehicleSpec {
    let wheel = |x: f32, z: f32, steers: f32| WheelSpec { mount: Vec3::new(x, WHEEL_R - HCG, z), radius: WHEEL_R, steers, driven: true };
    let (x, z) = (TRACK / 2.0, WHEELBASE / 2.0);
    VehicleSpec {
        // Rover 210 kg, a suited driver 170, tools and samples 100.
        mass: 480.0,
        gyration: Vec3::new(0.9, 1.3, 0.4),
        // Both axles steer, the rear against the front: a 3 m turning
        // radius.
        wheels: vec![wheel(-x, -z, 1.0), wheel(x, -z, 1.0), wheel(-x, z, -1.0), wheel(x, z, -1.0)],
        travel: TRAVEL,
        damping: 0.4,
        roll_bar: 1.5,
        // Four 186 W motors.
        power: 600.0,
        top_speed: 3.6,
        reverse_speed: 1.5,
        max_steer: 0.36,
        // A wire-mesh tyre pulls about 0.6 of what a boot sole grips; it
        // holds sideways far better, sunk a little into the soil.
        grip: 0.6 * grip,
        side_grip: 2.0 * grip,
        rolling: 0.1,
        drag,
        hull: vec![
            Vec3::new(-0.65, CHASSIS_H - HCG, -1.5),
            Vec3::new(0.65, CHASSIS_H - HCG, -1.5),
            Vec3::new(-0.65, CHASSIS_H - HCG, 1.5),
            Vec3::new(0.65, CHASSIS_H - HCG, 1.5),
            Vec3::new(-0.6, CHASSIS_H - HCG + 0.85, 0.7),
            Vec3::new(0.6, CHASSIS_H - HCG + 0.85, 0.7),
            Vec3::new(0.42, CHASSIS_H - HCG + 1.5, -1.3),
            Vec3::new(0.0, CHASSIS_H - HCG + 1.0, -1.45),
        ],
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut props: ResMut<Assets<PropMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    fn metal(materials: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32, roughness: f32, metallic: f32) -> Handle<StandardMaterial> {
        materials.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: roughness, metallic, ..default() })
    }
    let parts = Parts {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        tube: meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(10)),
        ball: meshes.add(Sphere::new(1.0).mesh().ico(2).expect("valid subdivision")),
        tyre: meshes.add(tyre_mesh()),
        hub: meshes.add(Cylinder::new(0.26, 0.05).mesh().resolution(28)),
        fender: meshes.add(fender_mesh()),
        dish: meshes.add(dish_mesh()),
        alu: metal(&mut materials, 0.78, 0.79, 0.81, 0.42, 0.75),
        dull: metal(&mut materials, 0.62, 0.64, 0.65, 0.7, 0.5),
        // White in the lunar sun is brighter than any exposure set for grey
        // soil can hold; these are what dusty beta cloth measures.
        blanket: metal(&mut materials, 0.78, 0.77, 0.74, 0.88, 0.0),
        webbing: metal(&mut materials, 0.6, 0.65, 0.69, 0.95, 0.0),
        dark: metal(&mut materials, 0.17, 0.18, 0.19, 0.85, 0.15),
        tan: metal(&mut materials, 0.62, 0.58, 0.5, 0.72, 0.05),
        gold: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.71, 0.42),
            perceptual_roughness: 0.45,
            metallic: 0.8,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
        wire: materials.add(StandardMaterial {
            base_color_texture: Some(images.add(tyre_weave())),
            perceptual_roughness: 0.7,
            metallic: 0.35,
            ..default()
        }),
        suit: metal(&mut materials, 0.77, 0.76, 0.73, 0.92, 0.0),
        visor: metal(&mut materials, 0.79, 0.6, 0.24, 0.12, 1.0),
    };
    commands.insert_resource(parts);

    // The soil the wheels throw, and the tracks they leave: one mesh each,
    // rebuilt as they change.
    let blank = || Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    commands.spawn((
        Dust::default(),
        Mesh3d(meshes.add(blank())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(images.add(puff())),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            cull_mode: None,
            double_sided: true,
            ..default()
        })),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
    ));
    commands.spawn((
        Tracks::default(),
        Mesh3d(meshes.add(blank())),
        // Pressed soil is a factor on the ground under it, not a colour of
        // its own: it multiplies what is already drawn.
        MeshMaterial3d(props.add(prop(StandardMaterial {
            base_color_texture: Some(images.add(tread_print())),
            alpha_mode: AlphaMode::Multiply,
            unlit: true,
            depth_bias: 4.0,
            cull_mode: None,
            ..default()
        }))),
        Transform::default(),
        NoFrustumCulling,
        NotShadowCaster,
    ));
}

/// Parks a rover near the landing site of every body it can drive on.
fn park(
    mut commands: Commands,
    current: Res<Current>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    parts: Res<Parts>,
    old: Query<Entity, With<Rover>>,
    mut driving: ResMut<Driving>,
    mut hud: ResMut<Hud>,
    mut player: Single<&mut Player>,
    mut dust: Single<&mut Dust>,
    mut tracks: Single<&mut Tracks>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    *driving = Driving { demo: driving.demo, ..default() };
    hud.mode.clear();
    player.controlled = true;
    dust.grains.clear();
    tracks.clear();
    let body = &current.0;
    if !body.rover {
        return;
    }
    // Ahead and to the right of where you arrive, on ground level enough
    // to stand on.
    let heading = Quat::from_rotation_y(world.spawn.yaw);
    let mut best = (f32::MAX, DVec2::ZERO);
    for k in 0..24 {
        let offset = heading * Quat::from_rotation_y(-0.5 + 0.09 * k as f32) * Vec3::new(0.0, 0.0, -(8.0 + 0.7 * k as f32));
        let at = DVec2::new(world.spawn.x + offset.x as f64, world.spawn.z + offset.z as f64);
        let h = |dx: f64, dz: f64| world.ground.height(at.x + dx, at.y + dz);
        let tilt = (h(1.5, 0.0) - h(-1.5, 0.0)).abs() + (h(0.0, 1.5) - h(0.0, -1.5)).abs();
        let dry = body.sea.as_ref().is_none_or(|sea| h(0.0, 0.0) > sea.level + 0.5);
        if dry && tilt < best.0 {
            best = (tilt, at);
        }
    }
    let at = best.1;
    let vehicle = Vehicle::new(spec(body.grip, body.drag * 2.2));
    let y = world.ground.height(at.x, at.y) + vehicle.ride_height() + 0.05;
    let rover = commands
        .spawn((
            Rover,
            vehicle,
            Transform::from_translation(origin.render(at, y)).with_rotation(heading * Quat::from_rotation_y(0.9)),
            Visibility::default(),
        ))
        .id();
    build(&mut commands, rover, &parts);
}

/// The machine, from boxes, tubes and lathed wheels, in the frame of its
/// rails: x right, y up from the rails, −z forward.
fn build(commands: &mut Commands, rover: Entity, parts: &Parts) {
    let frame = commands.spawn((Transform::from_xyz(0.0, CHASSIS_H - HCG, 0.0), Visibility::default(), ChildOf(rover))).id();
    let block = |commands: &mut Commands, parent: Entity, material: &Handle<StandardMaterial>, size: Vec3, at: Vec3, tilt: f32| {
        commands.spawn((
            Mesh3d(parts.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at).with_rotation(Quat::from_rotation_x(tilt)).with_scale(size),
            ChildOf(parent),
        ));
    };
    let tube = |commands: &mut Commands, parent: Entity, material: &Handle<StandardMaterial>, a: Vec3, b: Vec3, r: f32| {
        commands.spawn((Mesh3d(parts.tube.clone()), MeshMaterial3d(material.clone()), between(a, b, r), ChildOf(parent)));
    };
    let v = Vec3::new;

    // Frame: two long rails and cross members, and the floor.
    for sx in [-0.62, 0.62] {
        tube(commands, frame, &parts.alu, v(sx, 0.04, -1.45), v(sx, 0.04, 1.45), 0.024);
    }
    for z in [-1.45, -0.95, -0.35, 0.3, 0.85, 1.45] {
        tube(commands, frame, &parts.alu, v(-0.62, 0.04, z), v(0.62, 0.04, z), 0.02);
    }
    block(commands, frame, &parts.dull, v(1.2, 0.02, 1.25), v(0.0, 0.02, -0.02), 0.0);
    block(commands, frame, &parts.dull, v(1.2, 0.02, 0.5), v(0.0, 0.02, 1.15), 0.0);

    // Seats: tube frames, webbing, arm rests and footrests.
    for sx in [-0.33f32, 0.33] {
        block(commands, frame, &parts.webbing, v(0.5, 0.025, 0.46), v(sx, 0.3, 0.42), 0.0);
        block(commands, frame, &parts.webbing, v(0.5, 0.5, 0.025), v(sx, 0.55, 0.66), -0.2);
        for dx in [-0.25, 0.25] {
            tube(commands, frame, &parts.alu, v(sx + dx, 0.3, 0.2), v(sx + dx, 0.3, 0.65), 0.012);
            tube(commands, frame, &parts.alu, v(sx + dx, 0.04, 0.3), v(sx + dx, 0.3, 0.3), 0.012);
            tube(commands, frame, &parts.alu, v(sx + dx, 0.3, 0.65), v(sx + dx, 0.78, 0.74), 0.012);
        }
        let out = sx + 0.27 * sx.signum();
        tube(commands, frame, &parts.alu, v(out, 0.45, 0.25), v(out, 0.45, 0.6), 0.012);
        block(commands, frame, &parts.dull, v(0.42, 0.02, 0.24), v(sx, 0.13, -0.55), -0.5);
    }
    // Console on its post, and the T-handle hand controller behind it.
    tube(commands, frame, &parts.alu, v(0.0, 0.04, -0.35), v(0.0, 0.62, -0.35), 0.02);
    block(commands, frame, &parts.dark, v(0.3, 0.2, 0.05), v(0.0, 0.7, -0.38), -0.5);
    block(commands, frame, &parts.dark, v(0.08, 0.22, 0.12), v(0.0, 0.36, 0.05), 0.0);
    tube(commands, frame, &parts.dark, v(0.0, 0.45, 0.05), v(0.0, 0.62, 0.0), 0.012);
    tube(commands, frame, &parts.dark, v(-0.05, 0.62, 0.0), v(0.05, 0.62, 0.0), 0.012);

    // Forward: batteries and the communications unit under thermal
    // blankets, its radiator mirror, the television camera on its mast.
    block(commands, frame, &parts.blanket, v(0.34, 0.2, 0.36), v(-0.3, 0.14, -1.18), 0.0);
    block(commands, frame, &parts.blanket, v(0.34, 0.2, 0.36), v(0.3, 0.14, -1.18), 0.0);
    block(commands, frame, &parts.blanket, v(0.46, 0.3, 0.3), v(0.0, 0.42, -1.3), 0.0);
    block(commands, frame, &parts.alu, v(0.4, 0.02, 0.26), v(0.0, 0.58, -1.3), 0.0);
    tube(commands, frame, &parts.alu, v(0.0, 0.58, -1.3), v(0.0, 0.88, -1.38), 0.018);
    block(commands, frame, &parts.blanket, v(0.16, 0.14, 0.26), v(0.0, 0.95, -1.42), 0.0);
    tube(commands, frame, &parts.dark, v(0.0, 0.95, -1.54), v(0.0, 0.95, -1.64), 0.048);
    // Low-gain antenna, left front; the high-gain umbrella of gold mesh
    // on its mast, right front, tipped up toward home.
    tube(commands, frame, &parts.alu, v(-0.42, 0.3, -1.35), v(-0.42, 1.05, -1.35), 0.012);
    tube(commands, frame, &parts.alu, v(-0.42, 1.05, -1.35), v(-0.42, 1.6, -1.35), 0.034);
    tube(commands, frame, &parts.alu, v(0.42, 0.3, -1.3), v(0.42, 1.25, -1.3), 0.02);
    commands.spawn((
        Mesh3d(parts.dish.clone()),
        MeshMaterial3d(parts.gold.clone()),
        Transform::from_xyz(0.42, 1.38, -1.3).with_rotation(Quat::from_euler(EulerRot::YXZ, 0.6, -0.9, 0.0)),
        ChildOf(frame),
    ));

    // Aft pallet: sample bags and the geology tools standing in it.
    block(commands, frame, &parts.dull, v(0.9, 0.3, 0.36), v(0.0, 0.19, 1.3), 0.0);
    block(commands, frame, &parts.blanket, v(0.86, 0.1, 0.32), v(0.0, 0.39, 1.3), 0.0);
    for (tx, tz, h) in [(-0.35, 1.2, 0.7), (-0.2, 1.38, 0.85), (0.28, 1.25, 0.6), (0.36, 1.4, 0.75)] {
        tube(commands, frame, &parts.alu, v(tx, 0.3, tz), v(tx + 0.02, 0.3 + h, tz + 0.04), 0.011);
    }
    block(commands, frame, &parts.dark, v(0.14, 0.05, 0.12), v(-0.2, 1.17, 1.42), 0.0);

    // The commander in the left seat: backpack over the seat back, boots
    // on the footrest, a hand on the T-handle.
    let crew = commands.spawn((Crew, Transform::default(), Visibility::default(), ChildOf(frame))).id();
    let cx = -0.33;
    block(commands, crew, &parts.suit, v(0.44, 0.52, 0.28), v(cx, 0.7, 0.36), 0.08);
    block(commands, crew, &parts.blanket, v(0.42, 0.62, 0.2), v(cx, 0.86, 0.58), 0.08);
    block(commands, crew, &parts.blanket, v(0.3, 0.22, 0.18), v(cx, 1.26, 0.62), 0.08);
    block(commands, crew, &parts.dark, v(0.26, 0.1, 0.12), v(cx, 0.86, 0.17), 0.08);
    block(commands, crew, &parts.suit, v(0.4, 0.16, 0.34), v(cx, 0.4, 0.44), 0.0);
    let ball = |commands: &mut Commands, material: &Handle<StandardMaterial>, at: Vec3, r: Vec3| {
        commands.spawn((Mesh3d(parts.ball.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(at).with_scale(r), ChildOf(crew)));
    };
    ball(commands, &parts.suit, v(cx, 1.14, 0.3), Vec3::splat(0.16));
    ball(commands, &parts.visor, v(cx, 1.15, 0.24), v(0.13, 0.12, 0.12));
    for dx in [-0.11, 0.11] {
        tube(commands, crew, &parts.suit, v(cx + dx, 0.42, 0.42), v(cx + dx * 1.1, 0.46, 0.0), 0.085);
        tube(commands, crew, &parts.suit, v(cx + dx * 1.1, 0.46, 0.0), v(cx + dx * 1.15, 0.2, -0.44), 0.075);
        block(commands, crew, &parts.blanket, v(0.13, 0.1, 0.26), v(cx + dx * 1.15, 0.17, -0.5), -0.5);
    }
    tube(commands, crew, &parts.suit, v(cx - 0.25, 0.9, 0.36), v(cx - 0.29, 0.62, 0.3), 0.07);
    tube(commands, crew, &parts.suit, v(cx - 0.29, 0.62, 0.3), v(cx - 0.27, 0.5, 0.02), 0.06);
    tube(commands, crew, &parts.suit, v(cx + 0.25, 0.9, 0.36), v(cx + 0.28, 0.66, 0.22), 0.07);
    tube(commands, crew, &parts.suit, v(cx + 0.28, 0.66, 0.22), v(0.0, 0.64, 0.04), 0.06);
    ball(commands, &parts.dark, v(0.0, 0.64, 0.03), Vec3::splat(0.055));

    // Wheels: a knuckle that rides the suspension and steers, a hub and a
    // woven tyre that spin in it, a fender over each, and two wishbones.
    let wheels = spec(1.0, 0.0).wheels;
    for (k, wheel) in wheels.iter().enumerate() {
        let side = wheel.mount.x.signum();
        let knuckle = commands
            .spawn((Knuckle(k), Transform::from_translation(wheel.mount), Visibility::default(), ChildOf(rover)))
            .id();
        let spinner = commands.spawn((Spinner(k), Transform::default(), Visibility::default(), ChildOf(knuckle))).id();
        // Tyre and hub are lathed about y; laid on their side the axle
        // runs along x.
        let on_side = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        commands.spawn((Mesh3d(parts.tyre.clone()), MeshMaterial3d(parts.wire.clone()), Transform::from_rotation(on_side), ChildOf(spinner)));
        commands.spawn((Mesh3d(parts.hub.clone()), MeshMaterial3d(parts.alu.clone()), Transform::from_rotation(on_side), ChildOf(spinner)));
        for spoke in 0..4 {
            let a = spoke as f32 * std::f32::consts::FRAC_PI_4;
            let across = Vec3::new(0.0, a.cos(), a.sin()) * 0.3;
            tube(commands, spinner, &parts.dull, -across, across, 0.012);
        }
        commands.spawn((
            Mesh3d(parts.fender.clone()),
            MeshMaterial3d(parts.tan.clone()),
            Transform::from_xyz(0.0, 0.0, 0.0),
            ChildOf(knuckle),
        ));
        for drop in [-0.07, 0.07] {
            let pivot = Vec3::new(side * (TRACK / 2.0 - 0.46), wheel.mount.y + drop, wheel.mount.z);
            commands.spawn((
                Arm { wheel: k, pivot, drop },
                Mesh3d(parts.tube.clone()),
                MeshMaterial3d(parts.alu.clone()),
                between(pivot, wheel.mount + Vec3::Y * drop - Vec3::X * side * 0.1, 0.016),
                ChildOf(rover),
            ));
        }
    }
}

/// A unit cylinder (along y) stretched from `a` to `b`.
fn between(a: Vec3, b: Vec3, radius: f32) -> Transform {
    let axis = b - a;
    Transform::from_translation((a + b) / 2.0)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis.normalize_or(Vec3::Y)))
        .with_scale(Vec3::new(radius, axis.length(), radius))
}

/// **R**: aboard, or out. **V**: seat or chase camera.
fn board(
    keys: Res<ButtonInput<KeyCode>>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut driving: ResMut<Driving>,
    mut hud: ResMut<Hud>,
    player: Single<(&mut Player, &mut Transform), Without<Rover>>,
    rover: Option<Single<(&mut Vehicle, &Transform), With<Rover>>>,
    mut crew: Query<&mut Visibility, With<Crew>>,
) {
    let Some(rover) = rover else { return };
    let (mut vehicle, pose) = rover.into_inner();
    let (mut player, mut eye) = player.into_inner();
    // MOON_DRIVE=seat|chase starts at the wheel, for looking at the rover
    // without sitting at the window; with "go" it drives off.
    let demo = std::env::var("MOON_DRIVE").unwrap_or_default();
    if !demo.is_empty() && !driving.aboard && !driving.demo {
        driving.demo = true;
        driving.aboard = true;
        driving.chase = demo.contains("chase");
        player.controlled = false;
        player.flying = false;
        (player.yaw, player.pitch) = (if demo.contains("side") { 1.9 } else { 0.0 }, -0.08);
    }
    if keys.just_pressed(KeyCode::KeyV) && driving.aboard {
        driving.chase = !driving.chase;
        driving.behind = None;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        if driving.aboard {
            // Step out on the left, facing the way the rover does.
            driving.aboard = false;
            (vehicle.throttle, vehicle.steer, vehicle.brake) = (0.0, 0.0, 1.0);
            let out = pose.translation - *pose.right() * 1.7;
            player.controlled = true;
            player.stand_at(&mut eye, &world, &origin, out);
            let forward = pose.forward();
            player.yaw = (-forward.x).atan2(-forward.z);
            player.pitch = 0.0;
            eye.rotation = Quat::from_rotation_y(player.yaw);
            hud.mode.clear();
        } else if !player.flying && eye.translation.distance(pose.translation) < REACH {
            driving.aboard = true;
            driving.behind = None;
            player.controlled = false;
            player.velocity = Vec3::ZERO;
            // The mouse now looks about from the seat.
            (player.yaw, player.pitch) = (0.0, -0.08);
        }
    }
    let show = if driving.aboard && !driving.chase { Visibility::Hidden } else if driving.aboard { Visibility::Inherited } else { Visibility::Hidden };
    for mut v in &mut crew {
        v.set_if_neq(show);
    }
}

fn steer(keys: Res<ButtonInput<KeyCode>>, time: Res<Time>, driving: Res<Driving>, rover: Option<Single<(&mut Vehicle, &Transform), With<Rover>>>) {
    let Some(rover) = rover else { return };
    let (mut vehicle, pose) = rover.into_inner();
    if !driving.aboard {
        return;
    }
    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let mut want = axis(KeyCode::KeyW, KeyCode::KeyS);
    if std::env::var("MOON_DRIVE").is_ok_and(|d| d.contains("go")) {
        want = 1.0;
    }
    let speed = vehicle.forward_speed(pose);
    // Pulling the handle back while rolling forward is the brake; from
    // rest it is reverse. Space is the brake always; hands off, it coasts
    // to a stop and holds.
    let against = want != 0.0 && speed * want < -0.3;
    vehicle.throttle = if against { 0.0 } else { want };
    let resting = want == 0.0 && speed.abs() < 0.3;
    vehicle.brake = if keys.pressed(KeyCode::Space) || against || resting { 1.0 } else { 0.0 };
    // The hand controller is rate-limited, and gentler at speed.
    let lock = 1.0 / (1.0 + 0.12 * speed.abs());
    let target = axis(KeyCode::KeyA, KeyCode::KeyD) * lock;
    let rate = 2.5 * time.delta_secs();
    vehicle.steer += (target - vehicle.steer).clamp(-rate, rate);
}

fn pose_wheels(
    rover: Option<Single<&Vehicle, With<Rover>>>,
    mut knuckles: Query<(&Knuckle, &mut Transform), (Without<Spinner>, Without<Arm>)>,
    mut spinners: Query<(&Spinner, &mut Transform), (Without<Knuckle>, Without<Arm>)>,
    mut arms: Query<(&Arm, &mut Transform), (Without<Knuckle>, Without<Spinner>)>,
) {
    let Some(vehicle) = rover else { return };
    let hub = |k: usize| vehicle.spec.wheels[k].mount + Vec3::Y * vehicle.wheels[k].compression;
    for (knuckle, mut transform) in &mut knuckles {
        transform.translation = hub(knuckle.0);
        transform.rotation = Quat::from_rotation_y(vehicle.wheels[knuckle.0].steer);
    }
    for (spinner, mut transform) in &mut spinners {
        transform.rotation = Quat::from_rotation_x(-vehicle.wheels[spinner.0].roll);
    }
    for (arm, mut transform) in &mut arms {
        let end = hub(arm.wheel);
        let side = end.x.signum();
        *transform = between(arm.pivot, end + Vec3::Y * arm.drop - Vec3::X * side * 0.1, 0.016);
    }
}

/// The camera while you drive: your own eyes in the left seat, or a view
/// from behind that follows.
fn ride(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut driving: ResMut<Driving>,
    mut hud: ResMut<Hud>,
    player: Single<(&Player, &mut Transform), Without<Rover>>,
    rover: Option<Single<(&Vehicle, &Transform), With<Rover>>>,
) {
    let Some(rover) = rover else { return };
    if !driving.aboard {
        return;
    }
    let (vehicle, pose) = rover.into_inner();
    let (player, mut eye) = player.into_inner();
    let look = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    if driving.chase {
        // Behind and above, level with the horizon, turning with the
        // rover's heading and not with its pitching.
        let forward = pose.forward();
        let heading = Quat::from_rotation_y((-forward.x).atan2(-forward.z) + player.yaw);
        let target = pose.translation + Vec3::Y * 0.9;
        let want = target + heading * Vec3::new(0.0, 2.0 - 6.0 * player.pitch.min(0.3), 7.0);
        let ease = 1.0 - (-time.delta_secs() * 4.0).exp();
        let mut at = driving.behind.map_or(want, |at| at + (want - at) * ease);
        let floor = world.ground.height(origin.world(at).x, origin.world(at).y) + 0.4;
        at.y = at.y.max(floor);
        driving.behind = Some(at);
        *eye = Transform::from_translation(at).looking_at(target, Vec3::Y);
    } else {
        eye.translation = pose.translation + pose.rotation * (EYE + Vec3::Y * (CHASSIS_H - HCG));
        eye.rotation = pose.rotation * look;
    }
    let speed = vehicle.forward_speed(pose);
    let mode = format!(
        "DRIVE {:.0} km/h{}   (R: out   V: view   Space: brake)",
        speed.abs() * 3.6,
        if vehicle.airborne > 0.0 { "  airborne" } else { "" }
    );
    if hud.mode != mode {
        hud.mode = mode;
    }
}

/// The origin of a mesh that is rebuilt in place: its vertices are in the
/// entity's own frame, which the floating origin moves with everything else.
fn local(transform: &Transform, render: Vec3) -> Vec3 {
    render - transform.translation
}

struct Grain {
    at: Vec3,
    velocity: Vec3,
    age: f32,
    size: f32,
    /// Height of the ground where it was thrown: about where it lands.
    floor: f32,
}

/// Soil in flight.
#[derive(Component, Default)]
struct Dust {
    grains: Vec<Grain>,
    owed: f32,
    seed: u32,
}

impl Dust {
    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(747796405).wrapping_add(2891336453);
        let x = (self.seed >> ((self.seed >> 28) + 4)) ^ self.seed;
        (x.wrapping_mul(277803737) >> 8) as f32 / 16777216.0
    }
}

const MOST_GRAINS: usize = 6000;
/// Grains thrown per wheel per metre driven.
const SPRAY: f32 = 130.0;

/// Rooster tails. The tread picks grains up where it leaves the ground
/// and lets them go a little way round the back of the wheel. A point on
/// the tyre an angle φ past the contact patch moves, over the ground, at
/// v(1 − cos φ) forward and v·sin φ up — so the spray leaves slower than
/// the rover and falls behind it. The fender takes everything released
/// much above the axle; what is left goes out low.
fn throw_dust(time: Res<Time>, rover: Option<Single<(&mut Vehicle, &Transform), With<Rover>>>, dust: Single<(&mut Dust, &Transform)>) {
    let Some(rover) = rover else { return };
    let (mut vehicle, pose) = rover.into_inner();
    let (mut dust, frame) = dust.into_inner();
    let speed = vehicle.forward_speed(pose);
    let dt = time.delta_secs();
    let forward = *pose.forward() * speed.signum();
    let right = *pose.right();
    let patches: Vec<(Vec3, bool, f32)> = vehicle.wheels.iter().map(|w| (w.patch, w.contact, w.landed)).collect();
    for wheel in &mut vehicle.wheels {
        wheel.landed = 0.0;
    }
    let throw = |dust: &mut Dust, at: Vec3, velocity: Vec3, floor: f32| {
        if dust.grains.len() < MOST_GRAINS {
            // Clumps of fines, a thumb to a fist across.
            let size = 0.012 + 0.03 * dust.random().powi(2);
            dust.grains.push(Grain { at: local(frame, at), velocity, age: 0.0, size, floor });
        }
    };
    // A wheel coming down hard kicks up a ring of soil.
    for (patch, _, landed) in &patches {
        if *landed > 0.5 {
            for _ in 0..(40.0 * landed.min(2.5)) as usize {
                let a = dust.random() * std::f32::consts::TAU;
                let out = Vec3::new(a.cos(), 0.0, a.sin());
                let v = out * landed * (0.3 + 0.7 * dust.random()) + Vec3::Y * landed * (0.2 + 0.5 * dust.random());
                throw(&mut dust, *patch + out * 0.25 + Vec3::Y * 0.02, v + vehicle.velocity * 0.5, patch.y);
            }
        }
    }
    if speed.abs() < 1.0 || vehicle.airborne > 0.0 {
        return;
    }
    dust.owed += SPRAY * speed.abs() * dt;
    let n = dust.owed as usize;
    dust.owed -= n as f32;
    for (patch, contact, _) in patches {
        if !contact {
            continue;
        }
        for _ in 0..n {
            // Most of it lets go early, low and slow; the fines that
            // cling to the mesh ride further round and go out faster.
            let phi = 0.13 + dust.random().powf(1.8) * 0.72;
            let k = speed.abs() * (0.4 + 0.6 * dust.random());
            let back = WHEEL_R * phi.sin() + dust.random() * speed.abs() * dt;
            let across = (dust.random() - 0.5) * 0.22;
            let v = forward * k * (1.0 - phi.cos()) + Vec3::Y * k * phi.sin() + right * (dust.random() - 0.5) * 0.35 * k;
            throw(&mut dust, patch - forward * back + right * across + Vec3::Y * WHEEL_R * (1.0 - phi.cos()), v, patch.y);
        }
    }
}

/// In a vacuum every grain is a projectile: no billowing, no hanging
/// cloud, a clean parabola back to the ground. Air slows them and holds
/// the fines up a while.
fn fly_dust(time: Res<Time>, world: Res<WorldDef>, current: Res<Current>, mut dust: Single<&mut Dust>) {
    let dt = time.delta_secs();
    let g = world.walker.gravity;
    // Grains are light: the air has a hundred times the hold on them it
    // has on a suit.
    let air = current.0.drag * 150.0;
    dust.grains.retain_mut(|grain| {
        grain.velocity.y -= g * dt;
        if air > 0.0 {
            let slowed = 1.0 / (1.0 + air * grain.velocity.length() * dt);
            grain.velocity *= slowed;
        }
        grain.at += grain.velocity * dt;
        grain.age += dt;
        grain.age < 12.0 && (grain.at.y > grain.floor - 0.05 || grain.velocity.y > 0.0)
    });
}

fn draw_dust(
    current: Res<Current>,
    camera: Single<&Transform, (With<Camera3d>, Without<Dust>)>,
    dust: Single<(&Dust, &Mesh3d, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut was_empty: Local<bool>,
) {
    let (dust, handle, _) = dust.into_inner();
    if dust.grains.is_empty() && *was_empty {
        return;
    }
    *was_empty = dust.grains.is_empty();
    let Some(mut mesh) = meshes.get_mut(&handle.0) else { return };
    // Each grain a small square facing the camera, lit like the ground it
    // came from: its normal is the ground's.
    let (right, up) = (*camera.right(), *camera.up());
    let a = current.0.albedo * 1.15;
    let n = dust.grains.len();
    let (mut positions, mut normals, mut colors, mut uvs, mut indices) =
        (Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(6 * n));
    let tint = current.0.rocks.tint;
    for (k, grain) in dust.grains.iter().enumerate() {
        let alpha = 0.8 * (1.0 - (grain.age / 12.0).powi(2));
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push((grain.at + (right * sx + up * sy) * grain.size).to_array());
            normals.push([0.0, 1.0, 0.0]);
            colors.push([a * tint[0].min(1.3), a, a * tint[2], alpha]);
            uvs.push([0.5 + 0.5 * sx, 0.5 + 0.5 * sy]);
        }
        let i = 4 * k as u32;
        indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
}

/// Half the width of a tyre's print, m; how far apart its samples are
/// laid; and how many are kept (a kilometre and more of track).
const PRINT: f32 = 0.115;
const STRIDE: f32 = 0.3;
const MOST_PRINTS: usize = 4000;

/// A cross-section of one wheel's track: its two edges on the ground and
/// how far along the track it is. `fresh` starts a new strip.
struct Print {
    left: Vec3,
    right: Vec3,
    along: f32,
    fresh: bool,
}

/// The tracks, one strip per side, laid under the front wheels (the rear
/// ones, steering against them, run in the same ruts).
#[derive(Component, Default)]
struct Tracks {
    sides: [Vec<Print>; 2],
    lattice: Lattice,
    stale: bool,
}

impl Tracks {
    fn clear(&mut self) {
        self.sides = default();
        self.lattice.clear();
        self.stale = true;
    }
}

fn lay_tracks(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    rover: Option<Single<(&Vehicle, &Transform), With<Rover>>>,
    tracks: Single<(&mut Tracks, &Mesh3d, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (mut tracks, handle, frame) = tracks.into_inner();
    let tracks = &mut *tracks;
    if let Some(rover) = rover {
        let (vehicle, pose) = rover.into_inner();
        let step = world.terrain.step(0) as f64;
        for side in 0..2 {
            let wheel = &vehicle.wheels[side];
            let strip = &mut tracks.sides[side];
            let at = local(frame, wheel.patch);
            if !wheel.contact {
                // Off the ground: the track stops, and starts again where
                // the wheel comes down.
                if let Some(last) = strip.last_mut() {
                    last.fresh = true;
                }
                continue;
            }
            let (moved, along, broken) = match strip.last() {
                Some(last) => (((last.left + last.right) / 2.0).distance(at), last.along, last.fresh),
                None => (f32::MAX, 0.0, true),
            };
            if moved < STRIDE {
                continue;
            }
            // Across the way the wheel points, on the ground as drawn.
            let across = pose.rotation * Quat::from_rotation_y(wheel.steer) * Vec3::X * PRINT;
            let mut on_ground = |p: Vec3| {
                let w = origin.world(p + frame.translation);
                Vec3::new(p.x, drawn_height(&*world.ground, step, w.x, w.y, &mut tracks.lattice) + 0.012, p.z)
            };
            let print = Print {
                left: on_ground(at - across),
                right: on_ground(at + across),
                along: if moved < 3.0 { along + moved } else { 0.0 },
                fresh: false,
            };
            if (broken || moved >= 3.0) && !strip.is_empty() {
                let n = strip.len();
                strip[n - 1].fresh = true;
            }
            strip.push(print);
            if strip.len() > MOST_PRINTS {
                strip.drain(0..MOST_PRINTS / 8);
            }
            tracks.stale = true;
        }
    }
    if !tracks.stale {
        return;
    }
    tracks.stale = false;
    let Some(mut mesh) = meshes.get_mut(&handle.0) else { return };
    let (mut positions, mut normals, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for strip in &tracks.sides {
        for (k, print) in strip.iter().enumerate() {
            let i = positions.len() as u32;
            positions.push(print.left.to_array());
            positions.push(print.right.to_array());
            normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 2]);
            // The tread repeats every 40 cm.
            uvs.push([0.0, print.along / 0.4]);
            uvs.push([1.0, print.along / 0.4]);
            if k + 1 < strip.len() && !print.fresh {
                indices.extend_from_slice(&[i, i + 1, i + 2, i + 2, i + 1, i + 3]);
            }
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
}

fn image(width: u32, height: u32, data: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d { width, height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        mipmap_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A clump of dust: a soft round blot.
fn puff() -> Image {
    let n = 32u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for j in 0..n {
        for i in 0..n {
            let (x, y) = ((i as f32 + 0.5) / n as f32 * 2.0 - 1.0, (j as f32 + 0.5) / n as f32 * 2.0 - 1.0);
            let a = (1.0 - (x * x + y * y)).clamp(0.0, 1.0);
            data.extend([255, 255, 255, (a * a * 255.0) as u8]);
        }
    }
    image(n, n, data)
}

/// The tyre's cloth of zinc-coated piano wire, bright wire over the dark
/// inside of the carcass, and the titanium chevrons riveted over the
/// middle of the tread. u runs round the wheel, v across the profile.
fn tyre_weave() -> Image {
    let (w, h) = (512u32, 96u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for j in 0..h {
        for i in 0..w {
            let (x, y) = (i as f32, j as f32);
            let wire = ((x + y) % 6.0 < 1.5) || ((x - y).rem_euclid(6.0) < 1.5);
            let mut c = if wire { [0.66, 0.67, 0.69] } else { [0.23, 0.23, 0.24] };
            // Chevrons across the middle 40 %, pointing the way it rolls.
            let v = y / h as f32;
            if (0.3..0.7).contains(&v) {
                let u = (x + (v - 0.5).abs() * 60.0) % (w as f32 / 40.0);
                if u < 2.6 {
                    c = [0.77, 0.76, 0.74];
                }
            }
            data.extend(c.map(|v| (v * 255.0) as u8));
            data.push(255);
        }
    }
    image(w, h, data)
}

/// What a tyre leaves in the soil, as a factor on the ground's own
/// colour: pressed a little darker all across, with the chevrons' cuts
/// darker still (their far walls in shadow) and a brighter lip pushed up
/// along each edge.
fn tread_print() -> Image {
    let (w, h) = (32u32, 64u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for j in 0..h {
        for i in 0..w {
            let (u, v) = ((i as f32 + 0.5) / w as f32, (j as f32 + 0.5) / h as f32);
            let edge = (u - 0.5).abs() * 2.0;
            let mut f: f32 = 0.8;
            // Eight chevrons to the repeat.
            let phase = (v * 8.0 + (u - 0.5).abs() * 2.2).fract();
            if edge < 0.62 && phase < 0.3 {
                f = 0.6;
            }
            if edge > 0.82 {
                f = 1.0 + 0.1 * (1.0 - (edge - 0.9).abs() / 0.1).max(0.0);
            }
            let f = (f / 1.1).clamp(0.0, 1.0);
            data.extend([(f.powf(1.0 / 2.2) * 255.0) as u8; 3]);
            data.push(255);
        }
    }
    image(w, h, data)
}

/// The tyre: a squarish section lathed about y, with texture coordinates
/// that run round the wheel and across the profile.
fn tyre_mesh() -> Mesh {
    let (rim, half_width) = (0.25, 0.115);
    let profile: Vec<Vec2> = (0..=16)
        .map(|i| {
            // A superellipse from rim to rim: boxy, like an inflated section.
            let t = i as f32 / 16.0 * std::f32::consts::PI;
            let (s, c) = t.sin_cos();
            Vec2::new(rim + (WHEEL_R - rim) * s.powf(0.45), -half_width * c.signum() * c.abs().powf(0.45))
        })
        .collect();
    lathe(&profile, 48, 14.0)
}

/// A shallow parabolic dish, opening along +y.
fn dish_mesh() -> Mesh {
    let profile: Vec<Vec2> = (0..=8).map(|i| Vec2::new(0.001 + i as f32 / 8.0 * 0.46, (i as f32 / 8.0).powi(2) * 0.13)).collect();
    lathe(&profile, 32, 1.0)
}

/// Turns a profile (radius, height) about the y axis.
fn lathe(profile: &[Vec2], around: usize, repeat: f32) -> Mesh {
    let (mut positions, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    let rows = profile.len();
    for k in 0..=around {
        let a = k as f32 / around as f32 * std::f32::consts::TAU;
        for (j, p) in profile.iter().enumerate() {
            positions.push([p.x * a.cos(), p.y, p.x * a.sin()]);
            uvs.push([k as f32 / around as f32 * repeat, j as f32 / (rows - 1) as f32]);
        }
    }
    for k in 0..around {
        for j in 0..rows - 1 {
            let (a, b) = ((k * rows + j) as u32, ((k + 1) * rows + j) as u32);
            indices.extend_from_slice(&[a, a + 1, b, b, a + 1, b + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices));
    mesh.compute_smooth_normals();
    mesh
}

/// A fibreglass shell arching over the top of a wheel, from ahead of the
/// axle to behind it.
fn fender_mesh() -> Mesh {
    let (r0, r1, half) = (0.47, 0.485, 0.15);
    let (from, to, n) = (0.32f32, std::f32::consts::PI - 0.32, 20);
    let (mut positions, mut normals, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    // Outer and inner skins, each with its own normals.
    for (radius, out) in [(r1, 1.0f32), (r0, -1.0)] {
        let base = positions.len() as u32;
        for k in 0..=n {
            let a = from + (to - from) * k as f32 / n as f32;
            // The arc is in the wheel's plane: z fore and aft, y up.
            let (y, z) = (a.sin(), a.cos());
            for x in [-half, half] {
                positions.push([x, radius * y, radius * z]);
                normals.push([0.0, out * y, out * z]);
            }
            if k < n {
                let i = base + 2 * k as u32;
                if out > 0.0 {
                    indices.extend_from_slice(&[i, i + 1, i + 2, i + 2, i + 1, i + 3]);
                } else {
                    indices.extend_from_slice(&[i, i + 2, i + 1, i + 2, i + 3, i + 1]);
                }
            }
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}
