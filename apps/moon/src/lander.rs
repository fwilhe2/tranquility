//! A lunar module at the landing site, as it stood while its crew was out:
//! the descent stage, an octagonal body in gold and black blankets on four
//! splayed legs, the engine bell underneath and the ladder down the
//! forward leg; and on top of it the ascent stage that will take them home,
//! all facets: the cabin's flat face with its two triangular windows and
//! the square hatch over the porch, the propellant tanks bulging out to
//! either side, the docking tunnel on top, the dishes and the four thruster
//! quads at its corners. It is where the rover came from, so it stands
//! wherever one is parked. Real dimensions: 4.2 m across the body, footpads
//! 9.4 m apart across the diagonal, nearly 7 m to the top.
//!
//! Each leg reaches the ground under its own footpad, so on a slope the
//! struts are of different lengths, as they were at Hadley; the probes
//! under three of the pads lie bent where the landing folded them.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    math::{Affine2, DVec2},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tranquility::{ground::prop, prelude::*};

use crate::{Current, rover::Rover};

/// Footpads' distance from the centre, m; the body's half-width; the
/// contact probes' length under the pads.
pub const PAD: f32 = 4.7;
pub const PROBE: f32 = 1.73;
const BODY: f32 = 2.1;
/// How near anyone on foot may come to the centre, m.
pub const KEEP_OFF: f32 = 5.2;

pub struct LanderPlugin;

impl Plugin for LanderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (land.run_if(resource_changed::<WorldDef>).after(crate::rover::Parked), stand_clear.after(Walking)),
        );
    }
}

#[derive(Component)]
pub struct Lander;

/// The meshes and materials a lunar module is built from.
#[derive(Resource)]
pub struct Kit {
    tube: Handle<Mesh>,
    box_: Handle<Mesh>,
    octagon: Handle<Mesh>,
    bell: Handle<Mesh>,
    pad: Handle<Mesh>,
    ball: Handle<Mesh>,
    dish: Handle<Mesh>,
    nozzle: Handle<Mesh>,
    window: Handle<Mesh>,
    gold: Handle<PropMaterial>,
    black: Handle<PropMaterial>,
    silver: Handle<PropMaterial>,
    dark: Handle<PropMaterial>,
    /// The ascent stage's skin: grey anodised panels, and darker ones.
    panel: Handle<PropMaterial>,
    panel_dark: Handle<PropMaterial>,
    /// Inconel foil round the tanks and the quads: pale, nearly a mirror.
    inconel: Handle<PropMaterial>,
    glass: Handle<PropMaterial>,
    white: Handle<PropMaterial>,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PropMaterial>>, mut images: ResMut<Assets<Image>>) {
    let crinkle = images.add(crinkles());
    let seams = images.add(panels());
    let foil = |r: f32, g: f32, b: f32, roughness: f32, metallic: f32, materials: &mut Assets<PropMaterial>| {
        materials.add(prop(StandardMaterial {
            base_color: Color::linear_rgb(r, g, b),
            perceptual_roughness: roughness,
            metallic,
            normal_map_texture: Some(crinkle.clone()),
            // A couple of metres of foil to a tile, round the body.
            uv_transform: Affine2::from_scale(Vec2::new(5.0, 0.7)),
            ..default()
        }))
    };
    let skin = |r: f32, g: f32, b: f32, roughness: f32, metallic: f32, materials: &mut Assets<PropMaterial>| {
        materials.add(prop(StandardMaterial {
            base_color: Color::linear_rgb(r, g, b),
            perceptual_roughness: roughness,
            metallic,
            normal_map_texture: Some(seams.clone()),
            ..default()
        }))
    };
    // Every mesh gets tangents, so any of them can wear the foil.
    let mut flat = |mesh: Mesh| {
        let mut mesh = mesh;
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        mesh.generate_tangents().expect("a triangle mesh with UVs");
        meshes.add(mesh)
    };
    let octagon = flat(Cylinder::new(1.0, 1.0).mesh().resolution(8).into());
    let tube = flat(Cylinder::new(1.0, 1.0).mesh().resolution(10).into());
    let box_ = flat(Cuboid::new(1.0, 1.0, 1.0).into());
    // A triangle, point up, a metre on a side, as a thin plate: a window.
    let window = flat(Extrusion::new(Triangle2d::new(Vec2::new(-0.5, -0.29), Vec2::new(0.5, -0.29), Vec2::new(0.0, 0.58)), 1.0).into());
    // Faceted, like the covers over the tanks.
    let ball = flat(Sphere::new(1.0).mesh().ico(1).expect("valid subdivision"));
    let gold = foil(0.62, 0.38, 0.09, 0.32, 0.7, &mut materials);
    let black = foil(0.025, 0.025, 0.028, 0.55, 0.2, &mut materials);
    let inconel = foil(0.55, 0.53, 0.5, 0.28, 0.9, &mut materials);
    let panel = skin(0.23, 0.23, 0.225, 0.55, 0.45, &mut materials);
    let panel_dark = skin(0.09, 0.09, 0.095, 0.45, 0.5, &mut materials);
    let mut material = |r: f32, g: f32, b: f32, roughness: f32, metallic: f32| {
        materials.add(prop(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: roughness, metallic, ..default() }))
    };
    commands.insert_resource(Kit {
        tube,
        box_,
        // Flats facing the legs' gaps, as on the real one.
        octagon,
        bell: meshes.add(ConicalFrustum { radius_top: 0.45, radius_bottom: 0.82, height: 1.0 }.mesh().resolution(24)),
        pad: meshes.add(ConicalFrustum { radius_top: 0.2, radius_bottom: 0.47, height: 1.0 }.mesh().resolution(20)),
        ball,
        // A shallow dish, its open side up the y axis.
        dish: meshes.add(ConicalFrustum { radius_top: 1.0, radius_bottom: 0.12, height: 0.35 }.mesh().resolution(20)),
        nozzle: meshes.add(ConicalFrustum { radius_top: 0.02, radius_bottom: 0.05, height: 0.14 }.mesh().resolution(10)),
        window,
        // Kapton over aluminium: amber gold, crinkled, shining in creases.
        gold,
        black,
        silver: material(0.62, 0.62, 0.64, 0.38, 0.8),
        dark: material(0.12, 0.11, 0.1, 0.5, 0.6),
        panel,
        panel_dark,
        inconel,
        glass: material(0.01, 0.012, 0.015, 0.06, 0.0),
        white: material(0.75, 0.75, 0.73, 0.6, 0.0),
    });
}

/// Foil wrapped by hand: a normal map of creases, tileable (every wave
/// has a whole number of periods across the tile).
fn crinkles() -> Image {
    let n = 256u32;
    let mut waves = Vec::new();
    for k in 0..28 {
        let (a, b) = (unit(k, 1, 0xf011), unit(k, 2, 0xf011));
        let f = IVec2::new((a * 7.0) as i32 - 3, (b * 7.0) as i32 - 3);
        if f == IVec2::ZERO {
            continue;
        }
        waves.push((f.as_vec2(), unit(k, 3, 0xf011) * std::f32::consts::TAU, 0.6 + unit(k, 4, 0xf011)));
    }
    let height = |u: f32, v: f32| -> f32 {
        // Ridges: folds, not ripples.
        waves.iter().map(|(f, phase, a)| a * (std::f32::consts::TAU * (f.x * u + f.y * v) + phase).sin().abs() / f.length()).sum()
    };
    normal_map(n, |u, v| height(u, v) * 0.035)
}

/// Skin panels riveted on a frame: the seams between them, a few to a
/// face, and rows of rivets along them; tileable.
fn panels() -> Image {
    // Seams at these fractions of the tile, across and along.
    const ACROSS: [f32; 3] = [0.0, 0.37, 0.71];
    const ALONG: [f32; 2] = [0.0, 0.55];
    let groove = |t: f32, at: &[f32]| -> f32 {
        at.iter()
            .map(|a| {
                let d = (t - a).rem_euclid(1.0).min((a - t).rem_euclid(1.0));
                (1.0 - d / 0.006).max(0.0)
            })
            .fold(0.0, f32::max)
    };
    let rivet = |u: f32, v: f32, at: &[f32], along: f32| -> f32 {
        at.iter()
            .map(|a| {
                let d = (u - a - 0.016).rem_euclid(1.0).min((a + 0.016 - u).rem_euclid(1.0));
                let s = (v * along).fract() - 0.5;
                let r = (d * d + (s / along) * (s / along)).sqrt();
                (1.0 - r / 0.005).max(0.0)
            })
            .fold(0.0, f32::max)
    };
    normal_map(256, |u, v| {
        let seam = groove(u, &ACROSS).max(groove(v, &ALONG));
        let rivets = rivet(u, v, &ACROSS, 24.0).max(rivet(v, u, &ALONG, 24.0));
        // Each panel bows out a hair between its frames.
        let bow = (u * ACROSS.len() as f32 * std::f32::consts::PI).sin().abs() * 0.0015;
        -0.004 * seam + 0.002 * rivets + bow
    })
}

/// A tileable tangent-space normal map from a height function on the unit
/// square (heights in tiles).
fn normal_map(n: u32, height: impl Fn(f32, f32) -> f32) -> Image {
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    let e = 1.0 / n as f32;
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f32 * e, j as f32 * e);
            let dx = (height(u + e, v) - height(u - e, v)) / (2.0 * e);
            let dy = (height(u, v + e) - height(u, v - e)) / (2.0 * e);
            let normal = Vec3::new(-dx, -dy, 1.0).normalize();
            data.extend(normal.to_array().map(|c| ((c * 0.5 + 0.5) * 255.0) as u8));
            data.push(255);
        }
    }
    let mut image = Image::new(Extent3d { width: n, height: n, depth_or_array_layers: 1 }, TextureDimension::D2, data, TextureFormat::Rgba8Unorm, RenderAssetUsages::RENDER_WORLD);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// A tube from `a` to `b`.
fn strut(a: Vec3, b: Vec3, radius: f32) -> Transform {
    let axis = b - a;
    Transform::from_translation((a + b) / 2.0)
        .with_rotation(Quat::from_rotation_arc(Vec3::Y, axis.normalize_or(Vec3::Y)))
        .with_scale(Vec3::new(radius, axis.length(), radius))
}

/// Sets the lander down on a body the rover is parked on, behind the
/// rover's flank: level ground a short walk from the start.
fn land(
    mut commands: Commands,
    current: Res<Current>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    kit: Res<Kit>,
    old: Query<Entity, With<Lander>>,
    rover: Option<Single<&Transform, With<Rover>>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let body = &current.0;
    if !body.rover {
        return;
    }
    let ground = &*world.ground;
    // Ahead and to the left, where the walk is likely to pass it.
    let heading = Quat::from_rotation_y(world.spawn.yaw);
    let rover = rover.map(|r| origin.world(r.translation));
    let mut best = (f32::MAX, DVec2::ZERO);
    for k in 0..48 {
        let turn = 0.3 + 0.6 * (k / 8) as f32 / 5.0;
        let reach = 25.0 + 2.8 * (k % 8) as f32;
        let offset = heading * Quat::from_rotation_y(turn) * Vec3::new(0.0, 0.0, -reach);
        let at = DVec2::new(world.spawn.x + offset.x as f64, world.spawn.z + offset.z as f64);
        let h = |dx: f64, dz: f64| ground.height(at.x + dx, at.y + dz);
        let tilt = (h(PAD as f64, 0.0) - h(-PAD as f64, 0.0)).abs() + (h(0.0, PAD as f64) - h(0.0, -PAD as f64)).abs();
        let dry = body.sea.as_ref().is_none_or(|sea| h(0.0, 0.0) > sea.level + 1.0);
        let clear = rover.is_none_or(|r| r.distance(at) > 14.0) && !ground.blocked(at.x, at.y);
        if dry && clear && tilt < best.0 {
            best = (tilt, at);
        }
    }
    let at = best.1;
    // Turned so its ladder (−z) faces the start, a little to one side so
    // the light finds the face of the cabin.
    let facing = (at.x - world.spawn.x).atan2(at.y - world.spawn.z) as f32 + 0.35;
    place(&mut commands, &kit, ground, &origin, at, facing);
}

/// A lunar module standing on the ground at a place, its ladder (−z) turned
/// to `facing` (yaw), each leg reaching the ground under its own pad.
pub fn place(commands: &mut Commands, kit: &Kit, ground: &dyn Ground, origin: &Origin, at: DVec2, facing: f32) -> Entity {
    let floor = ground.height(at.x, at.y);
    let turn = Quat::from_rotation_y(facing);
    let ground_at = |p: Vec3| {
        let w = origin.world(origin.render(at, 0.0) + turn * p);
        ground.height(w.x, w.y) - floor
    };
    commands
        .spawn((Lander, Transform::from_translation(origin.render(at, floor)).with_rotation(turn), Visibility::default()))
        .with_children(|lm| model(lm, kit, Some(&ground_at)))
        .id()
}

/// The lunar module, in its own frame from the ground under its centre
/// (or, in flight, the plane under its footpads). With `ground_at`, the
/// height of the ground at a point of that frame, each leg reaches down to
/// it and the probes lie bent on it; without, the legs are at full length
/// and the probes hang straight down.
pub fn model(lm: &mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>, kit: &Kit, ground_at: Option<&dyn Fn(Vec3) -> f32>) {
        let part = |mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, transform: Transform| (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform);
        let eighth = Quat::from_rotation_y(std::f32::consts::FRAC_PI_8);
        // The body: gold blankets, a black band at the top, the deck.
        let (bottom, top) = (1.15, 2.75);
        lm.spawn(part(&kit.octagon, &kit.gold, Transform::from_xyz(0.0, (bottom + top) / 2.0, 0.0).with_rotation(eighth).with_scale(Vec3::new(BODY / 0.924, top - bottom, BODY / 0.924))));
        lm.spawn(part(&kit.octagon, &kit.black, Transform::from_xyz(0.0, top - 0.12, 0.0).with_rotation(eighth).with_scale(Vec3::new(BODY / 0.924 + 0.02, 0.26, BODY / 0.924 + 0.02))));
        // Black blankets over the underside, round the engine.
        lm.spawn(part(&kit.octagon, &kit.black, Transform::from_xyz(0.0, bottom + 0.06, 0.0).with_rotation(eighth).with_scale(Vec3::new(BODY / 0.924 + 0.02, 0.12, BODY / 0.924 + 0.02))));
        lm.spawn(part(&kit.box_, &kit.dark, Transform::from_xyz(0.0, top + 0.03, 0.0).with_scale(Vec3::new(3.2, 0.06, 3.2))));
        // The equipment bay by the ladder, folded down, and the bay the
        // rover rode in, its blankets open.
        lm.spawn(part(&kit.box_, &kit.silver, Transform::from_xyz(-1.15, 1.75, -1.55).with_scale(Vec3::new(0.9, 0.9, 0.06))));
        lm.spawn(part(&kit.box_, &kit.black, Transform::from_xyz(-1.95, 1.95, 0.9).with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_4)).with_scale(Vec3::new(1.3, 1.2, 0.05))));
        // The engine bell.
        lm.spawn(part(&kit.bell, &kit.dark, Transform::from_xyz(0.0, bottom - 0.35, 0.0).with_scale(Vec3::new(1.0, 0.7, 1.0))));
        // Four legs: −z is forward, with the ladder.
        for k in 0..4 {
            let angle = k as f32 * std::f32::consts::FRAC_PI_2;
            let out = Quat::from_rotation_y(angle);
            let (dir, side) = (out * Vec3::NEG_Z, out * Vec3::X);
            // The footpad, on the ground under it.
            let pad_at = dir * PAD;
            let pad_y = ground_at.map_or(0.0, |g| g(pad_at));
            let foot = pad_at + Vec3::Y * (pad_y + 0.16);
            lm.spawn(part(&kit.pad, &kit.silver, Transform::from_translation(pad_at + Vec3::Y * (pad_y + 0.08)).with_scale(Vec3::new(1.0, 0.16, 1.0))));
            // The primary strut from high on the body, gold-wrapped,
            // and its shock absorber's bright inner tube near the foot.
            let hip = dir * BODY + Vec3::Y * (top - 0.3);
            lm.spawn(part(&kit.tube, &kit.gold, strut(hip, foot + (hip - foot) * 0.22, 0.11)));
            lm.spawn(part(&kit.tube, &kit.silver, strut(foot + (hip - foot) * 0.25, foot, 0.075)));
            // Secondary struts from the body's lower corners to a
            // knuckle a third of the way up the leg.
            let knee = foot + (hip - foot) * 0.32;
            for s in [-1.0, 1.0] {
                let corner = dir * (BODY - 0.1) + side * s * 1.05 + Vec3::Y * (bottom + 0.1);
                lm.spawn(part(&kit.tube, &kit.silver, strut(corner, knee, 0.045)));
            }
            // A deployment truss across under the body.
            lm.spawn(part(&kit.tube, &kit.silver, strut(dir * (BODY - 0.2) + Vec3::Y * (bottom + 0.05), knee, 0.035)));
            if k == 0 {
                // The ladder: two rails down the forward strut, rungs
                // between.
                let along = (foot - hip).normalize();
                let lift = Vec3::Y * 0.05 - dir * 0.14;
                for s in [-0.24, 0.24] {
                    lm.spawn(part(&kit.tube, &kit.silver, strut(hip + side * s + lift, foot + along * -0.6 + side * s + lift, 0.022)));
                }
                let length = (foot + along * -0.6 - hip).length();
                for r in 1..9 {
                    let p = hip + along * (r as f32 / 9.0 * length) + lift;
                    lm.spawn(part(&kit.tube, &kit.silver, strut(p - side * 0.24, p + side * 0.24, 0.016)));
                }
            } else if let Some(ground_at) = ground_at {
                // The contact probe, 1.7 m long under the pad, bent
                // back on landing and lying across the soil.
                let swing = Quat::from_rotation_y(0.6 * (unit(k, 7, 0x1a0d) - 0.5));
                let knuckle = pad_at + dir * 0.35 + Vec3::Y * (pad_y + 0.05);
                let tip = pad_at + swing * dir * 1.8;
                let tip = tip + Vec3::Y * (ground_at(tip) + 0.015);
                lm.spawn(part(&kit.tube, &kit.silver, strut(pad_at + Vec3::Y * (pad_y + 0.04), knuckle, 0.014)));
                lm.spawn(part(&kit.tube, &kit.silver, strut(knuckle, tip, 0.014)));
            } else {
                // In flight the probe hangs straight down, 1.73 m under the
                // pad, to touch first and light the contact light.
                let top = pad_at + dir * 0.35;
                lm.spawn(part(&kit.tube, &kit.silver, strut(top, top - Vec3::Y * PROBE, 0.014)));
            }
        }
        ascent_stage(lm, kit, top + 0.06);
}

fn ascent_stage(lm: &mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>, kit: &Kit, b: f32) {
    let part = |mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, transform: Transform| (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform);
    let level = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    // The cabin: an octagonal drum lying fore and aft, its axis 1.3 m up,
    // and the face, a little proud of it, darker round the windows.
    let (axis_y, cabin_r) = (b + 1.3, 1.18);
    let (front, back) = (-1.5, -0.25);
    lm.spawn(part(&kit.octagon, &kit.panel, Transform::from_xyz(0.0, axis_y, (front + back) / 2.0).with_rotation(level * Quat::from_rotation_y(std::f32::consts::FRAC_PI_8)).with_scale(Vec3::new(cabin_r / 0.924, back - front, cabin_r / 0.924))));
    // Windows: two triangles leaning back in the upper facets, framed in
    // black, the docking window over the commander's.
    for s in [-1.0f32, 1.0] {
        let lean = Quat::from_rotation_y(-0.32 * s) * Quat::from_rotation_x(-0.42);
        let at = Vec3::new(0.48 * s, axis_y + 0.48, front - 0.02);
        let tip_in = Quat::from_rotation_z(0.42 * s);
        lm.spawn(part(&kit.window, &kit.panel_dark, Transform::from_translation(at).with_rotation(lean * tip_in).with_scale(Vec3::new(0.72, 0.72, 0.05))));
        lm.spawn(part(&kit.window, &kit.glass, Transform::from_translation(at + lean * Vec3::Z * -0.03).with_rotation(lean * tip_in).with_scale(Vec3::new(0.6, 0.6, 0.05))));
    }
    lm.spawn(part(&kit.box_, &kit.glass, Transform::from_xyz(-0.62, axis_y + 1.08, -0.9).with_rotation(Quat::from_rotation_x(0.25)).with_scale(Vec3::new(0.22, 0.03, 0.3))));
    // The forward hatch, square, low on the face, and the porch under it
    // with its rails, out over the top of the ladder.
    lm.spawn(part(&kit.box_, &kit.panel_dark, Transform::from_xyz(0.0, b + 0.55, front - 0.02).with_scale(Vec3::new(0.96, 0.96, 0.06))));
    lm.spawn(part(&kit.box_, &kit.panel, Transform::from_xyz(0.0, b + 0.55, front - 0.05).with_scale(Vec3::new(0.8, 0.8, 0.04))));
    lm.spawn(part(&kit.box_, &kit.silver, Transform::from_xyz(0.32, b + 0.55, front - 0.09).with_scale(Vec3::new(0.05, 0.16, 0.05))));
    let porch = (front - 0.05, -BODY - 0.25);
    lm.spawn(part(&kit.box_, &kit.silver, Transform::from_xyz(0.0, b + 0.01, (porch.0 + porch.1) / 2.0).with_scale(Vec3::new(0.95, 0.04, porch.0 - porch.1))));
    for s in [-0.47f32, 0.47] {
        lm.spawn(part(&kit.tube, &kit.silver, strut(Vec3::new(s, b, porch.1), Vec3::new(s, b + 0.75, porch.1 + 0.05), 0.018)));
        lm.spawn(part(&kit.tube, &kit.silver, strut(Vec3::new(s, b + 0.75, porch.1 + 0.05), Vec3::new(s, b + 0.8, porch.0), 0.018)));
    }
    // The midsection behind it, wider, and the aft equipment bay, black,
    // with the oxygen and water tanks on its back.
    let mid = (back, 1.15);
    lm.spawn(part(&kit.box_, &kit.panel, Transform::from_xyz(0.0, b + 1.2, (mid.0 + mid.1) / 2.0).with_scale(Vec3::new(2.1, 2.3, mid.1 - mid.0))));
    // Its upper half under black blankets.
    lm.spawn(part(&kit.box_, &kit.black, Transform::from_xyz(0.0, b + 1.85, (mid.0 + mid.1) / 2.0 + 0.02).with_scale(Vec3::new(2.14, 1.04, mid.1 - mid.0 + 0.02))));
    lm.spawn(part(&kit.box_, &kit.black, Transform::from_xyz(0.0, b + 1.0, mid.1 + 0.42).with_scale(Vec3::new(1.85, 1.5, 0.84))));
    for s in [-0.45f32, 0.45] {
        lm.spawn(part(&kit.octagon, &kit.inconel, Transform::from_xyz(s, b + 1.1, mid.1 + 0.95).with_scale(Vec3::new(0.2, 1.1, 0.2))));
    }
    // The propellant tanks, fuel to the left and oxidiser to the right,
    // in bulges standing out from the midsection, the left one further.
    for (s, out, r) in [(-1.0f32, 1.3, 0.66), (1.0, 1.18, 0.62)] {
        lm.spawn(part(&kit.ball, &kit.inconel, Transform::from_xyz(s * out, b + 1.05, 0.45).with_scale(Vec3::new(r, r * 1.08, r * 1.15))));
    }
    // The docking tunnel on top, its rim, and the target on its standoff.
    lm.spawn(part(&kit.octagon, &kit.panel, Transform::from_xyz(0.0, b + 2.6, 0.2).with_scale(Vec3::new(0.46, 0.45, 0.46))));
    lm.spawn(part(&kit.octagon, &kit.panel_dark, Transform::from_xyz(0.0, b + 2.85, 0.2).with_scale(Vec3::new(0.52, 0.06, 0.52))));
    lm.spawn(part(&kit.tube, &kit.white, strut(Vec3::new(-0.75, axis_y + 1.1, -0.75), Vec3::new(-0.75, axis_y + 1.4, -0.75), 0.02)));
    lm.spawn(part(&kit.box_, &kit.white, Transform::from_xyz(-0.75, axis_y + 1.42, -0.75).with_scale(Vec3::new(0.32, 0.32, 0.02))));
    // The rendezvous radar on the cabin's roof, looking forward and up;
    // the steerable S-band dish on its boom to the right; the two VHF
    // antennas raked back.
    let radar = Vec3::new(0.35, axis_y + 1.3, -0.85);
    lm.spawn(part(&kit.tube, &kit.silver, strut(radar - Vec3::Y * 0.35, radar, 0.05)));
    lm.spawn(part(&kit.dish, &kit.white, Transform::from_translation(radar + Vec3::new(0.0, 0.1, -0.08)).with_rotation(Quat::from_rotation_x(-0.9)).with_scale(Vec3::splat(0.33))));
    let boom = Vec3::new(1.3, b + 2.4, -0.15);
    lm.spawn(part(&kit.tube, &kit.silver, strut(Vec3::new(1.0, b + 2.1, -0.05), boom, 0.03)));
    lm.spawn(part(&kit.dish, &kit.white, Transform::from_translation(boom + Vec3::Y * 0.12).with_rotation(Quat::from_rotation_z(-0.5) * Quat::from_rotation_x(-0.3)).with_scale(Vec3::splat(0.34))));
    for s in [-1.0f32, 1.0] {
        let foot = Vec3::new(0.7 * s, b + 2.34, 0.8);
        lm.spawn(part(&kit.tube, &kit.silver, strut(foot, foot + Vec3::new(0.25 * s, 0.7, 0.35), 0.012)));
    }
    // The four thruster quads on their struts at the corners, each four
    // nozzles: up, down and two to the sides.
    for k in 0..4 {
        let angle = std::f32::consts::FRAC_PI_4 + k as f32 * std::f32::consts::FRAC_PI_2;
        let out = Quat::from_rotation_y(angle);
        let (dir, side) = (out * Vec3::NEG_Z, out * Vec3::X);
        let centre = Vec3::new(0.0, b + 1.85, 0.0) + dir * 2.0 + Vec3::Z * 0.15;
        lm.spawn(part(&kit.tube, &kit.dark, strut(centre - dir * 0.85, centre, 0.035)));
        lm.spawn(part(&kit.box_, &kit.inconel, Transform::from_translation(centre).with_rotation(out).with_scale(Vec3::new(0.3, 0.28, 0.3))));
        for (towards, length) in [(Vec3::Y, 0.14), (Vec3::NEG_Y, 0.14), (side, 0.15), (-side, 0.15)] {
            let at = centre + towards * (length + 0.07);
            lm.spawn(part(&kit.nozzle, &kit.dark, Transform::from_translation(at).with_rotation(Quat::from_rotation_arc(Vec3::NEG_Y, towards)).with_scale(Vec3::splat(1.6))));
        }
    }
}

/// Nobody walks through it.
fn stand_clear(lander: Option<Single<&Transform, With<Lander>>>, player: Single<(&mut Player, &mut Transform), Without<Lander>>) {
    let Some(lander) = lander else { return };
    let (mut player, mut eye) = player.into_inner();
    if player.flying || !player.controlled {
        return;
    }
    let d = Vec2::new(eye.translation.x - lander.translation.x, eye.translation.z - lander.translation.z);
    let r = d.length();
    if r >= KEEP_OFF || r < 1e-3 {
        return;
    }
    let normal = d / r;
    eye.translation += Vec3::new(normal.x, 0.0, normal.y) * (KEEP_OFF - r);
    let into = (player.velocity.x * normal.x + player.velocity.z * normal.y).min(0.0);
    player.velocity -= Vec3::new(normal.x, 0.0, normal.y) * into;
}
