//! The descent stage of a lunar module, at the landing site, as the crews
//! left it: the ascent stage gone, the octagonal body in its gold and black
//! blankets on four splayed legs, the engine bell underneath, the ladder on
//! the forward leg. It is where the rover came from, so it stands wherever
//! one is parked. Real dimensions: 4.2 m across the body, footpads 9.4 m
//! apart across the diagonal.
//!
//! Each leg reaches the ground under its own footpad, so on a slope the
//! struts are of different lengths, as they were at Hadley.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    math::{Affine2, DVec2},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tranquility::{ground::prop, prelude::*};

use crate::{Current, rover::Rover};

/// Footpads' distance from the centre, m; the body's half-width.
const PAD: f32 = 4.7;
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

#[derive(Resource)]
struct Kit {
    tube: Handle<Mesh>,
    box_: Handle<Mesh>,
    octagon: Handle<Mesh>,
    bell: Handle<Mesh>,
    pad: Handle<Mesh>,
    gold: Handle<PropMaterial>,
    black: Handle<PropMaterial>,
    silver: Handle<PropMaterial>,
    dark: Handle<PropMaterial>,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PropMaterial>>, mut images: ResMut<Assets<Image>>) {
    let crinkle = images.add(crinkles());
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
    let mut flat = |mesh: Mesh| {
        let mut mesh = mesh;
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        mesh.generate_tangents().expect("a triangle mesh with UVs");
        meshes.add(mesh)
    };
    let octagon = flat(Cylinder::new(1.0, 1.0).mesh().resolution(8).into());
    let tube = flat(Cylinder::new(1.0, 1.0).mesh().resolution(10).into());
    let gold = foil(0.62, 0.38, 0.09, 0.32, 0.7, &mut materials);
    let black = foil(0.025, 0.025, 0.028, 0.55, 0.2, &mut materials);
    let mut material = |r: f32, g: f32, b: f32, roughness: f32, metallic: f32| {
        materials.add(prop(StandardMaterial { base_color: Color::linear_rgb(r, g, b), perceptual_roughness: roughness, metallic, ..default() }))
    };
    commands.insert_resource(Kit {
        tube,
        box_: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        // Flats facing the legs' gaps, as on the real one.
        octagon,
        bell: meshes.add(ConicalFrustum { radius_top: 0.45, radius_bottom: 0.82, height: 1.0 }.mesh().resolution(24)),
        pad: meshes.add(ConicalFrustum { radius_top: 0.2, radius_bottom: 0.47, height: 1.0 }.mesh().resolution(20)),
        // Kapton over aluminium: amber gold, crinkled, shining in creases.
        gold,
        black,
        silver: material(0.62, 0.62, 0.64, 0.38, 0.8),
        dark: material(0.12, 0.11, 0.1, 0.5, 0.6),
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
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    let e = 1.0 / n as f32;
    for j in 0..n {
        for i in 0..n {
            let (u, v) = (i as f32 * e, j as f32 * e);
            let dx = (height(u + e, v) - height(u - e, v)) / (2.0 * e);
            let dy = (height(u, v + e) - height(u, v - e)) / (2.0 * e);
            let normal = Vec3::new(-dx * 0.035, -dy * 0.035, 1.0).normalize();
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
    let floor = ground.height(at.x, at.y);
    // Turned so its ladder faces the start.
    let facing = (world.spawn.x - at.x).atan2(world.spawn.z - at.y) as f32;
    let turn = Quat::from_rotation_y(facing);
    commands
        .spawn((Lander, Transform::from_translation(origin.render(at, floor)).with_rotation(turn), Visibility::default()))
        .with_children(|lm| {
            let part = |mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, transform: Transform| (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), transform);
            // The body: gold blankets, a black band at the top, the deck.
            let (bottom, top) = (1.15, 2.75);
            lm.spawn(part(&kit.octagon, &kit.gold, Transform::from_xyz(0.0, (bottom + top) / 2.0, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_8)).with_scale(Vec3::new(BODY / 0.924, top - bottom, BODY / 0.924))));
            lm.spawn(part(&kit.octagon, &kit.black, Transform::from_xyz(0.0, top - 0.12, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_8)).with_scale(Vec3::new(BODY / 0.924 + 0.02, 0.26, BODY / 0.924 + 0.02))));
            lm.spawn(part(&kit.box_, &kit.dark, Transform::from_xyz(0.0, top + 0.03, 0.0).with_scale(Vec3::new(3.2, 0.06, 3.2))));
            // Quadrant bays on the deck, and the equipment bay by the ladder.
            for (x, z, w, d, h) in [(-0.9, -0.9, 1.0, 0.9, 0.35), (0.95, 0.8, 0.8, 1.0, 0.25), (0.9, -1.0, 0.7, 0.6, 0.5)] {
                lm.spawn(part(&kit.box_, &kit.silver, Transform::from_xyz(x, top + 0.06 + h / 2.0, z).with_scale(Vec3::new(w, h, d))));
            }
            lm.spawn(part(&kit.box_, &kit.silver, Transform::from_xyz(-1.15, 1.75, -1.55).with_scale(Vec3::new(0.9, 0.9, 0.06))));
            // The engine bell.
            lm.spawn(part(&kit.bell, &kit.dark, Transform::from_xyz(0.0, bottom - 0.35, 0.0).with_scale(Vec3::new(1.0, 0.7, 1.0))));
            // Four legs: −z is forward, with the ladder.
            for k in 0..4 {
                let angle = k as f32 * std::f32::consts::FRAC_PI_2;
                let out = Quat::from_rotation_y(angle);
                let (dir, side) = (out * Vec3::NEG_Z, out * Vec3::X);
                // The footpad, on the ground under it.
                let pad_at = dir * PAD;
                let pad_world = origin.world(origin.render(at, 0.0) + turn * pad_at);
                let pad_y = ground.height(pad_world.x, pad_world.y) - floor;
                let foot = pad_at + Vec3::Y * (pad_y + 0.16);
                lm.spawn(part(&kit.pad, &kit.silver, Transform::from_translation(pad_at + Vec3::Y * (pad_y + 0.08)).with_scale(Vec3::new(1.0, 0.16, 1.0))));
                // The primary strut from high on the body, gold-wrapped.
                let hip = dir * BODY + Vec3::Y * (top - 0.3);
                lm.spawn(part(&kit.tube, &kit.gold, strut(hip, foot, 0.11)));
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
                }
            }
        });
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
