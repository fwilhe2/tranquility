//! The walker's own body: a suit and backpack under the eye. Its head is
//! round the camera and so never drawn (you are inside it); the rest is
//! there when you look down, and, what matters more, it stands in the
//! sunlight. With the sun behind you your shadow walks ahead of you,
//! legs swinging, the one thing in an airless landscape that says someone
//! is here.
//!
//! The legs swing with the gait and the arms against them; the view does
//! not move with either. Motion belongs to the body and its shadow, never
//! to the eye.

use tranquility::{ground::prop, prelude::*};

use crate::gait::{Gait, Striding};

/// m: hips and shoulders above the feet.
const HIP: f32 = 0.9;
const SHOULDER: f32 = 1.42;
/// The body turns no further from the way you look than this, rad.
const TWIST: f32 = 1.0;

pub struct FigurePlugin;

impl Plugin for FigurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, pose.after(Striding));
    }
}

#[derive(Component)]
struct Figure {
    /// Which way the body faces, eased.
    yaw: f32,
}

/// A limb's joint: leg or arm, left or right.
#[derive(Component)]
struct Limb {
    leg: bool,
    left: bool,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PropMaterial>>) {
    let cloth = materials.add(prop(StandardMaterial { base_color: Color::linear_rgb(0.62, 0.62, 0.6), perceptual_roughness: 0.95, reflectance: 0.2, ..default() }));
    let grey = materials.add(prop(StandardMaterial { base_color: Color::linear_rgb(0.2, 0.2, 0.21), perceptual_roughness: 0.9, reflectance: 0.2, ..default() }));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let ball = meshes.add(Sphere::new(1.0).mesh().ico(3).expect("valid subdivision"));
    let tube = meshes.add(Capsule3d::new(0.5, 1.0));
    let part = |mesh: &Handle<Mesh>, material: &Handle<PropMaterial>, at: Vec3, size: Vec3| {
        (Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(at).with_scale(size))
    };
    commands
        .spawn((Figure { yaw: 0.0 }, Transform::default(), Visibility::Hidden))
        .with_children(|body| {
            // Forward is −z. Torso, the backpack behind it, the chest
            // pack, and the helmet round the eye.
            body.spawn(part(&cube, &cloth, Vec3::new(0.0, 1.2, 0.0), Vec3::new(0.5, 0.6, 0.3)));
            body.spawn(part(&cube, &cloth, Vec3::new(0.0, 1.28, 0.27), Vec3::new(0.52, 0.72, 0.24)));
            body.spawn(part(&cube, &grey, Vec3::new(0.0, 1.22, -0.2), Vec3::new(0.3, 0.22, 0.1)));
            body.spawn(part(&ball, &cloth, Vec3::new(0.0, 1.62, 0.0), Vec3::splat(0.19)));
            body.spawn(part(&cube, &cloth, Vec3::new(0.0, 0.88, 0.0), Vec3::new(0.46, 0.2, 0.28)));
            for left in [true, false] {
                let x = if left { -1.0 } else { 1.0 };
                // A leg: thigh and shin as one, and a boot.
                body.spawn((Limb { leg: true, left }, Transform::from_xyz(0.13 * x, HIP, 0.0), Visibility::Inherited)).with_children(|leg| {
                    leg.spawn(part(&tube, &cloth, Vec3::new(0.0, -0.4, 0.0), Vec3::new(0.2, 0.42, 0.2)));
                    leg.spawn(part(&cube, &grey, Vec3::new(0.0, -0.84, -0.05), Vec3::new(0.15, 0.12, 0.33)));
                });
                // An arm, hanging a little out from the body, and a glove.
                body.spawn((Limb { leg: false, left }, Transform::from_xyz(0.32 * x, SHOULDER, 0.0), Visibility::Inherited)).with_children(|arm| {
                    arm.spawn(part(&tube, &cloth, Vec3::new(0.03 * x, -0.3, 0.0), Vec3::new(0.15, 0.34, 0.15)));
                    arm.spawn(part(&ball, &grey, Vec3::new(0.04 * x, -0.66, 0.0), Vec3::splat(0.075)));
                });
            }
        });
}

fn wrap(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn pose(
    time: Res<Time>,
    world: Res<WorldDef>,
    gait: Res<Gait>,
    player: Single<(&Player, &Transform), Without<Figure>>,
    figure: Single<(&mut Figure, &mut Transform, &mut Visibility), Without<Limb>>,
    mut limbs: Query<(&Limb, &mut Transform), (Without<Figure>, Without<Player>)>,
) {
    let (player, eye) = player.into_inner();
    let (mut figure, mut frame, mut visibility) = figure.into_inner();
    // In a rover's seat there is a figure of the rover's own; in the air
    // above the ground (fly mode) there is nobody.
    let here = player.controlled && !player.flying;
    visibility.set_if_neq(if here { Visibility::Inherited } else { Visibility::Hidden });
    if !here {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    // The body faces the way it walks, as far as a neck allows.
    let walking = if gait.swing > 0.05 { wrap(gait.heading - player.yaw).clamp(-TWIST, TWIST) } else { wrap(figure.yaw - player.yaw).clamp(-TWIST, TWIST) };
    let to = player.yaw + walking;
    figure.yaw = wrap(figure.yaw + wrap(to - figure.yaw) * (1.0 - (-dt / 0.5).exp()));
    // Hung from the eye, not stood on the ground: the eye glides over
    // small bumps and the body must not slide about under it.
    frame.translation = eye.translation - Vec3::Y * world.walker.eye_height;
    frame.rotation = Quat::from_rotation_y(figure.yaw);

    // The foot that has just come down is ahead and the body passes over
    // it; the other swings through from behind to come down in its turn.
    let pace = Vec2::new(player.velocity.x, player.velocity.z).length();
    let reach = (0.5 * crate::gait::step_length(pace.max(0.05), world.walker.gravity) / HIP).min(0.6).asin() * gait.swing;
    let swing = reach * (std::f32::consts::PI * gait.phase).cos();
    for (limb, mut joint) in &mut limbs {
        let down = if limb.left != gait.left { 1.0 } else { -1.0 };
        // Arms swing against the legs, and less.
        let angle = if limb.leg { swing * down } else { 0.08 - 0.45 * swing * down };
        joint.rotation = Quat::from_rotation_x(angle);
    }
}
