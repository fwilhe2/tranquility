//! Jets of ice from Enceladus's tiger stripes: water from the ocean under
//! the crust, venting through the fissure as vapour and grains of ice at
//! hundreds of metres a second, high above anything gravity this weak can
//! hold down. From the ground beside the stripe they are columns of pale
//! haze standing out of the trough and fanning into the black, too tall to
//! see the tops of.
//!
//! Each jet is a few upright sheets that turn about their own axis to face
//! the camera, with a column painted on them (narrow at the vent, fanning
//! and fading upward, streaked along its length). The sheets swell and
//! sway out of step over tens of seconds: the jets vary, slowly. Only the
//! sheets' transforms change, never their material.
//!
//! Where they are is the land's (`Land::vents`); how they look, the body's
//! (`Body::plume`).

use bevy::{
    asset::RenderAssetUsages,
    math::DVec2,
    light::{NotShadowCaster, NotShadowReceiver},
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tranquility::{
    ground::prop,
    noise::{fbm, smoothstep},
    prelude::*,
};

use crate::Current;

/// How a body's jets look.
#[derive(Clone, Copy, Debug)]
pub struct Plume {
    /// Width of the fan at the top of the visible column, and the
    /// column's height, m. At the vent it is a twentieth as wide.
    pub width: f32,
    pub height: f32,
    pub color: [f32; 3],
    /// Opacity at the densest.
    pub opacity: f32,
}

/// Sheets per jet.
const SHEETS: usize = 3;

pub struct PlumesPlugin;

impl Plugin for PlumesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (vent.run_if(resource_changed::<WorldDef>), breathe).chain());
    }
}

#[derive(Resource)]
struct Kit {
    sheet: Handle<Mesh>,
    textures: Vec<Handle<Image>>,
}

/// A sheet of a jet, and how it breathes.
#[derive(Component)]
struct Sheet {
    width: f32,
    height: f32,
    /// Seconds per swell, and where in it this sheet starts.
    period: f32,
    phase: f32,
    lean: f32,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>) {
    // A unit sheet standing on its bottom edge, facing +z.
    let sheet = meshes.add(Rectangle::new(1.0, 1.0).mesh().build().translated_by(Vec3::Y * 0.5));
    let textures = (0..SHEETS as u32).map(|k| images.add(column(k))).collect();
    commands.insert_resource(Kit { sheet, textures });
}

/// A column of haze: narrow and dense at the bottom, fanning and fading
/// toward the top, streaked along its length. Linear colour white; the
/// alpha is the picture.
fn column(seed: u32) -> Image {
    let (w, h) = (128u32, 512u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for j in 0..h {
        // v: 0 at the vent, 1 at the top.
        let v = 1.0 - (j as f32 + 0.5) / h as f32;
        // A fan: a narrow throat that opens fast, then more slowly.
        let spread = 0.05 + 0.95 * v.powf(0.55);
        for i in 0..w {
            let u = (i as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let x = u / spread;
            // Streaks: noise across the fan, stretched along it, fading
            // as the grains spread.
            let streak = (1.0 + (0.6 - 0.3 * v) * fbm(x as f64 * 5.0, v as f64 * 0.8, 1.0, 4, 0.55, 300 + seed)).max(0.0);
            let body = (-2.2 * x * x).exp();
            // Densest just above the vent and thinning as it spreads (the
            // same grains over a wider fan); the top fades out.
            let along = smoothstep(0.0, 0.03, v) * (1.0 - smoothstep(0.4, 1.0, v)) * (0.25 + 0.75 * (0.05 / spread).powf(0.6));
            let a = (body * along * streak).clamp(0.0, 1.0);
            data.extend([255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[derive(Component)]
struct Jet;

/// Sets the body's jets going, wherever its land vents.
fn vent(
    mut commands: Commands,
    current: Res<Current>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    kit: Res<Kit>,
    old: Query<Entity, With<Jet>>,
    mut materials: ResMut<Assets<PropMaterial>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let body = &current.0;
    let Some(look) = body.plume else { return };
    let spawn = (world.spawn.x, world.spawn.z);
    for (k, (x, z)) in body.land.vents(spawn).into_iter().enumerate() {
        let floor = world.ground.height(x, z);
        let r = |salt: u32| unit(k as i32, salt as i32, 0x9e7);
        let strength = 0.55 + 0.45 * r(1);
        commands
            .spawn((Jet, Transform::from_translation(origin.render(DVec2::new(x, z), floor - 20.0)), Visibility::default()))
            .with_children(|jet| {
                for s in 0..SHEETS {
                    let material = materials.add(prop(StandardMaterial {
                        base_color: Color::linear_rgba(look.color[0], look.color[1], look.color[2], look.opacity * strength * (1.0 - 0.25 * s as f32)),
                        base_color_texture: Some(kit.textures[s].clone()),
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        cull_mode: None,
                        double_sided: true,
                        ..default()
                    }));
                    let rs = |salt: u32| unit(k as i32 * 7 + s as i32, salt as i32, 0x9e8);
                    jet.spawn((
                        Sheet {
                            width: look.width * (0.8 + 0.6 * rs(1)) * (1.0 + 0.5 * s as f32),
                            height: look.height * (0.6 + 0.5 * rs(2)) * strength,
                            period: 14.0 + 20.0 * rs(3),
                            phase: rs(4) * std::f32::consts::TAU,
                            lean: (rs(5) - 0.5) * 0.12,
                        },
                        Mesh3d(kit.sheet.clone()),
                        MeshMaterial3d(material),
                        Transform::default(),
                        NotShadowCaster,
                        NotShadowReceiver,
                    ));
                }
            });
    }
}

/// Each sheet turns to the camera about its own upright axis and swells and
/// leans a little, slowly.
fn breathe(
    time: Res<Time>,
    camera: Single<&Transform, (With<Camera3d>, Without<Sheet>, Without<Jet>)>,
    jets: Query<(&Transform, &Children), (With<Jet>, Without<Sheet>)>,
    mut sheets: Query<(&Sheet, &mut Transform), Without<Jet>>,
) {
    let t = time.elapsed_secs();
    for (jet, children) in &jets {
        let to_eye = camera.translation - jet.translation;
        let facing = to_eye.x.atan2(to_eye.z);
        for child in children.iter() {
            let Ok((sheet, mut transform)) = sheets.get_mut(child) else { continue };
            let swell = (t / sheet.period * std::f32::consts::TAU + sheet.phase).sin();
            let sway = (t / (sheet.period * 1.7) * std::f32::consts::TAU + sheet.phase * 2.0).sin();
            transform.rotation = Quat::from_rotation_y(facing) * Quat::from_rotation_z(sheet.lean + 0.03 * sway);
            transform.scale = Vec3::new(sheet.width * (1.0 + 0.15 * swell), sheet.height * (1.0 + 0.06 * swell), 1.0);
        }
    }
}
