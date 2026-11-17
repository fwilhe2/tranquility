//! Bootprints. Every footfall (see `gait`) presses a print into the soil: a
//! ribbed sole a third of a metre long, left and right in turn, on the
//! ground as it is drawn. Like the rover's tracks they are a factor on the
//! ground's own colour (pressed soil is darker, the ribs' far walls
//! darker still), so they look right on any body in any light; and in a
//! vacuum nothing will ever smooth them out. The last few hundred are kept.

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tranquility::{
    ground::prop,
    prelude::*,
    vehicle::{Lattice, drawn_height},
};

use crate::gait::Footfall;

/// Half the sole's width and length, m; how far apart the feet are.
const SOLE: Vec2 = Vec2::new(0.075, 0.17);
const STANCE: f32 = 0.13;
const MOST: usize = 800;

pub struct PrintsPlugin;

impl Plugin for PrintsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, tread.after(crate::gait::Striding));
    }
}

#[derive(Component, Default)]
struct Prints {
    /// Corners of each print, in the entity's own frame.
    soles: Vec<[Vec3; 4]>,
    lattice: Lattice,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut props: ResMut<Assets<PropMaterial>>, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        Prints::default(),
        Visibility::Hidden,
        Mesh3d(meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()))),
        MeshMaterial3d(props.add(prop(StandardMaterial {
            base_color_texture: Some(images.add(sole())),
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

fn tread(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut falls: MessageReader<Footfall>,
    prints: Single<(&mut Prints, &Mesh3d, &Transform, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let (mut prints, handle, frame, mut visibility) = prints.into_inner();
    let prints = &mut *prints;
    let mut stale = false;
    if world.is_changed() {
        // Another body: these were left on the last one.
        prints.soles.clear();
        prints.lattice.clear();
        falls.clear();
        stale = true;
    }
    for fall in falls.read() {
        let here = fall.at - frame.translation;
        // Toes the way the walker is going.
        let heading = Quat::from_rotation_y(fall.heading);
        let (forward, right) = (heading * Vec3::NEG_Z, heading * Vec3::X);
        let side = if fall.left { -STANCE } else { STANCE };
        let step = world.terrain.step(0) as f64;
        let mut corner = |x: f32, z: f32| {
            let p = here + right * (side + x * SOLE.x) + forward * (z * SOLE.y);
            let w = origin.world(p + frame.translation);
            Vec3::new(p.x, drawn_height(&*world.ground, step, w.x, w.y, &mut prints.lattice) + 0.01, p.z)
        };
        let sole = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
        prints.soles.push(sole);
        if prints.soles.len() > MOST {
            prints.soles.drain(0..MOST / 8);
        }
        stale = true;
    }
    if !stale {
        return;
    }
    // None yet: hidden, rather than a mesh of nothing.
    visibility.set_if_neq(if prints.soles.is_empty() { Visibility::Hidden } else { Visibility::Inherited });
    if prints.soles.is_empty() {
        return;
    }
    let Some(mut mesh) = meshes.get_mut(&handle.0) else { return };
    let n = prints.soles.len();
    let (mut positions, mut normals, mut uvs, mut indices) =
        (Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(4 * n), Vec::with_capacity(6 * n));
    for (k, sole) in prints.soles.iter().enumerate() {
        positions.extend(sole.iter().map(|p| p.to_array()));
        normals.extend_from_slice(&[[0.0, 1.0, 0.0]; 4]);
        uvs.extend_from_slice(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        let i = 4 * k as u32;
        indices.extend_from_slice(&[i, i + 2, i + 1, i, i + 3, i + 2]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
}

/// The print of a lunar overshoe, as a factor on the ground's colour:
/// a rounded outline pressed a little darker, nine ribs across it darker
/// still, and nothing (factor 1) outside.
fn sole() -> Image {
    let (w, h) = (32u32, 72u32);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for j in 0..h {
        for i in 0..w {
            let (u, v) = ((i as f32 + 0.5) / w as f32 * 2.0 - 1.0, (j as f32 + 0.5) / h as f32 * 2.0 - 1.0);
            // A sole: a superellipse, a little narrower at the heel.
            let wide = 0.92 - 0.14 * (-v).max(0.0);
            let inside = (u / wide).abs().powf(2.6) + v.abs().powf(3.2);
            let mut f = 1.0f32;
            if inside < 1.0 {
                let rib = ((v * 4.5).fract().abs() - 0.5).abs() < 0.2;
                f = if rib { 0.55 } else { 0.8 };
                // A soft edge, so the outline does not shimmer.
                f += (1.0 - f) * ((inside - 0.8) / 0.2).clamp(0.0, 1.0);
            }
            data.extend([(f.powf(1.0 / 2.2) * 255.0) as u8; 3]);
            data.push(255);
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
