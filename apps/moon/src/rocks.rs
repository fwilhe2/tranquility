//! Boulders, scattered on each chunk once it is meshed finely enough to be
//! seen up close. Positions and sizes come from a hash of the chunk, so a
//! chunk gets the same rocks every time it streams back in.

use tranquility::{prelude::*, terrain::ChunkMeshed};

/// Rocks are added when a chunk reaches this resolution (quads per edge).
const MIN_RESOLUTION: u32 = 128;
const PER_CHUNK: u32 = 260;
const SHAPES: usize = 6;

pub struct RocksPlugin;

impl Plugin for RocksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, scatter);
    }
}

#[derive(Resource)]
struct Kit {
    shapes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<StandardMaterial>>,
}

#[derive(Component)]
struct Scattered;

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let shapes = (0..SHAPES).map(|k| meshes.add(lumpy(k as u32))).collect();
    let materials = [0.15, 0.19, 0.24]
        .iter()
        .map(|&a| {
            materials.add(StandardMaterial {
                base_color: Color::linear_rgb(a * 1.03, a, a * 0.96),
                perceptual_roughness: 0.95,
                reflectance: 0.25,
                ..default()
            })
        })
        .collect();
    commands.insert_resource(Kit { shapes, materials });
}

/// A coarse icosphere with every vertex pushed in or out at random and the
/// bottom cut flat: broken, faceted stones, no two shapes alike, each reused
/// many times.
fn lumpy(seed: u32) -> Mesh {
    let mut mesh = Sphere::new(1.0).mesh().ico(1).expect("valid subdivision");
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for (k, p) in positions.iter_mut().enumerate() {
            let v = Vec3::from(*p);
            let mut q = v * (0.7 + 0.45 * unit(seed as i32, k as i32, 9));
            // A flattened top and a buried bottom, as on most real boulders.
            q.y = q.y.clamp(-0.3, 0.75);
            *p = q.to_array();
        }
    }
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();
    mesh
}

fn scatter(
    mut commands: Commands,
    mut meshed: MessageReader<ChunkMeshed>,
    world: Res<WorldDef>,
    kit: Res<Kit>,
    done: Query<(), With<Scattered>>,
) {
    for chunk in meshed.read() {
        if chunk.resolution < MIN_RESOLUTION || done.contains(chunk.entity) {
            continue;
        }
        let Ok(mut entity) = commands.get_entity(chunk.entity) else { continue };
        entity.insert(Scattered);
        let cell = chunk.coord.x.wrapping_mul(7919) ^ chunk.coord.y;
        let r = |k: u32, salt: u32| unit(cell, k as i32, salt);
        entity.with_children(|parent| {
            for k in 0..PER_CHUNK {
                let local = Vec2::new(r(k, 1), r(k, 2)) * chunk.size;
                let at = chunk.origin + local;
                // Mostly pebbles, now and then a boulder.
                let size = 0.15 * (25.0f32).powf(r(k, 3).powf(2.5));
                let y = world.ground.height(at.x, at.y);
                let shape = (r(k, 4) * SHAPES as f32) as usize % SHAPES;
                let material = (r(k, 5) * 3.0) as usize % 3;
                parent.spawn((
                    Mesh3d(kit.shapes[shape].clone()),
                    MeshMaterial3d(kit.materials[material].clone()),
                    Transform::from_xyz(local.x, y + size * 0.15, local.y)
                        .with_rotation(Quat::from_rotation_y(r(k, 6) * std::f32::consts::TAU))
                        .with_scale(Vec3::new(size * (0.8 + 0.5 * r(k, 7)), size * (0.5 + 0.4 * r(k, 8)), size)),
                ));
            }
        });
    }
}
