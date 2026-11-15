//! The ground, streamed in square chunks around the camera.
//!
//! Each chunk is meshed on the async compute pool at a resolution chosen by its
//! distance from the camera, and re-meshed when that distance band changes.
//! Skirts hang down from every chunk edge to hide the cracks between
//! neighbours of different resolution.

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::Indices,
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::PrimitiveTopology,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};

use crate::{
    ground::{GroundDetail, GroundMaterial, Pattern},
    world::{Ground, WorldDef},
};

/// Chunk meshing jobs allowed to run at once. In the browser the "pool" is the
/// main thread, so fewer jobs there keep frames from stalling.
#[cfg(not(target_arch = "wasm32"))]
const MAX_JOBS: usize = 8;
#[cfg(target_arch = "wasm32")]
const MAX_JOBS: usize = 2;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chunks>()
            .add_message::<ChunkMeshed>()
            .add_systems(Startup, setup_material)
            .add_systems(Update, (finish_chunks, stream_chunks).chain());
    }
}

/// Sent whenever a chunk gets a new mesh, the first time or at a new
/// resolution. Apps decorate chunks from it (rocks, grass), as children of
/// `entity` so they go when the chunk goes.
#[derive(Message, Clone, Copy)]
pub struct ChunkMeshed {
    pub coord: IVec2,
    pub entity: Entity,
    pub resolution: u32,
    /// The chunk's south-west corner and edge, world metres.
    pub origin: Vec2,
    pub size: f32,
}

#[derive(Resource)]
struct TerrainMaterial(Handle<GroundMaterial>);

#[derive(Component)]
pub struct Chunk;

struct Loaded {
    entity: Entity,
    resolution: u32,
}

struct Job {
    resolution: u32,
    task: Task<Mesh>,
}

#[derive(Resource, Default)]
pub struct Chunks {
    loaded: HashMap<IVec2, Loaded>,
    jobs: HashMap<IVec2, Job>,
}

impl Chunks {
    /// True when every chunk in view is meshed at its proper resolution.
    pub fn is_settled(&self) -> bool {
        self.jobs.is_empty() && !self.loaded.is_empty()
    }
}

fn setup_material(
    mut commands: Commands,
    world: Res<WorldDef>,
    mut materials: ResMut<Assets<GroundMaterial>>,
) {
    // The colour lives in the vertices; the material only sets how it
    // reflects, and the extension draws the detail.
    let material = materials.add(GroundMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: world.terrain.roughness,
            reflectance: world.terrain.reflectance,
            // Skirts are seen from either side.
            cull_mode: None,
            ..default()
        },
        extension: GroundDetail {},
    });
    commands.insert_resource(TerrainMaterial(material));
}

fn stream_chunks(
    mut commands: Commands,
    mut chunks: ResMut<Chunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    world: Res<WorldDef>,
    camera: Single<&Transform, With<Camera3d>>,
    mesh_handles: Query<&Mesh3d, With<Chunk>>,
) {
    let settings = &world.terrain;
    let size = settings.chunk_size;
    let radius = settings.view_radius;
    let centre = IVec2::new(
        (camera.translation.x / size).floor() as i32,
        (camera.translation.z / size).floor() as i32,
    );
    let ring = |c: IVec2| (c - centre).abs().max_element();

    // Drop what has fallen out of range, keeping one ring of slack so chunks
    // on the border don't flicker in and out.
    let chunks = &mut *chunks;
    chunks.loaded.retain(|&coord, loaded| {
        let keep = ring(coord) <= radius + 1;
        if !keep {
            if let Ok(mesh) = mesh_handles.get(loaded.entity) {
                meshes.remove(&mesh.0);
            }
            commands.entity(loaded.entity).despawn();
        }
        keep
    });
    chunks.jobs.retain(|&coord, _| ring(coord) <= radius + 1);

    // Queue the nearest chunks that are missing or at the wrong resolution.
    let mut wanted = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let coord = centre + IVec2::new(dx, dz);
            let res = settings.resolution(ring(coord));
            let current = chunks.loaded.get(&coord).map(|l| l.resolution);
            let queued = chunks.jobs.get(&coord).map(|j| j.resolution);
            if current != Some(res) && queued != Some(res) {
                wanted.push((dx * dx + dz * dz, coord, res));
            }
        }
    }
    wanted.sort_unstable_by_key(|w| w.0);

    let pool = AsyncComputeTaskPool::get();
    for (_, coord, res) in wanted {
        if chunks.jobs.len() >= MAX_JOBS {
            break;
        }
        let ground = world.ground.clone();
        let task = pool.spawn(async move { build_chunk_mesh(&*ground, coord, size, res) });
        chunks.jobs.insert(coord, Job { resolution: res, task });
    }
}

fn finish_chunks(
    mut commands: Commands,
    mut chunks: ResMut<Chunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    world: Res<WorldDef>,
    material: Res<TerrainMaterial>,
    mut mesh_handles: Query<&mut Mesh3d, With<Chunk>>,
    mut meshed: MessageWriter<ChunkMeshed>,
) {
    let size = world.terrain.chunk_size;
    let chunks = &mut *chunks;
    let mut done = Vec::new();
    for (&coord, job) in chunks.jobs.iter_mut() {
        if let Some(mesh) = check_ready(&mut job.task) {
            done.push((coord, job.resolution, mesh));
        }
    }
    for (coord, resolution, mesh) in done {
        chunks.jobs.remove(&coord);
        let handle = meshes.add(mesh);
        let origin = coord.as_vec2() * size;
        let entity = if let Some(loaded) = chunks.loaded.get_mut(&coord) {
            if let Ok(mut mesh3d) = mesh_handles.get_mut(loaded.entity) {
                meshes.remove(&mesh3d.0);
                mesh3d.0 = handle;
            }
            loaded.resolution = resolution;
            loaded.entity
        } else {
            let mut chunk = commands.spawn((
                Chunk,
                Mesh3d(handle),
                MeshMaterial3d(material.0.clone()),
                Transform::from_xyz(origin.x, 0.0, origin.y),
            ));
            if !world.terrain.casts_shadows {
                chunk.insert(NotShadowCaster);
            }
            let entity = chunk.id();
            chunks.loaded.insert(coord, Loaded { entity, resolution });
            entity
        };
        meshed.write(ChunkMeshed {
            coord,
            entity,
            resolution,
            origin,
            size,
        });
    }
}

/// Meshes one chunk: an (n+1)² grid of vertices in chunk-local space, plus a
/// skirt around its edge.
fn build_chunk_mesh(ground: &dyn Ground, coord: IVec2, size: f32, n: u32) -> Mesh {
    let n = n as usize;
    let step = size / n as f32;
    let origin = coord.as_vec2() * size;

    // Heights on the grid plus a one-sample border, so normals along the edge
    // come out the same as in the neighbouring chunk.
    let side = n + 3;
    let mut heights = vec![0.0f32; side * side];
    for j in 0..side {
        for i in 0..side {
            let x = origin.x + (i as f32 - 1.0) * step;
            let z = origin.y + (j as f32 - 1.0) * step;
            heights[j * side + i] = ground.height(x, z);
        }
    }

    let verts = (n + 1) * (n + 1);
    let mut positions = Vec::with_capacity(verts + 4 * n + 1);
    let mut normals = Vec::with_capacity(verts + 4 * n + 1);
    let mut colors = Vec::with_capacity(verts + 4 * n + 1);
    let mut uv0 = Vec::with_capacity(verts + 4 * n + 1);
    let mut uv1 = Vec::with_capacity(verts + 4 * n + 1);
    for j in 0..=n {
        for i in 0..=n {
            let y = heights[(j + 1) * side + i + 1];
            // Central differences, reaching into the border where needed.
            let hx = heights[(j + 1) * side + i + 2] - heights[(j + 1) * side + i];
            let hz = heights[(j + 2) * side + i + 1] - heights[j * side + i + 1];
            let normal = Vec3::new(-hx, 2.0 * step, -hz).normalize();
            let world = origin + Vec2::new(i as f32, j as f32) * step;
            positions.push([i as f32 * step, y, j as f32 * step]);
            normals.push(normal.to_array());
            let tint = ground.tint(world.x, world.y, y, normal);
            let (a, b, c) = Pattern::from_code(tint.alpha).weights();
            colors.push(tint.with_alpha(c).to_f32_array());
            uv0.push(a);
            uv1.push(b);
        }
    }

    let index = |i: usize, j: usize| (j * (n + 1) + i) as u32;
    let mut indices = Vec::with_capacity(n * n * 6 + n * 24);
    for j in 0..n {
        for i in 0..n {
            let (a, b, c, d) = (index(i, j), index(i, j + 1), index(i + 1, j), index(i + 1, j + 1));
            indices.extend_from_slice(&[a, b, c, c, b, d]);
        }
    }

    // Walk the perimeter once and hang a strip of quads below it. The drop
    // only has to cover the largest height mismatch between neighbouring
    // resolutions, which is a few grid steps at most.
    let drop = 4.0 * step + 4.0;
    let mut perimeter = Vec::with_capacity(4 * n + 1);
    perimeter.extend((0..n).map(|i| (i, 0)));
    perimeter.extend((0..n).map(|j| (n, j)));
    perimeter.extend((1..=n).rev().map(|i| (i, n)));
    perimeter.extend((1..=n).rev().map(|j| (0, j)));
    perimeter.push((0, 0));
    let first_skirt = positions.len() as u32;
    for &(i, j) in &perimeter {
        let k = index(i, j) as usize;
        let [x, y, z] = positions[k];
        positions.push([x, y - drop, z]);
        normals.push(normals[k]);
        colors.push(colors[k]);
        uv0.push(uv0[k]);
        uv1.push(uv1[k]);
    }
    for s in 0..perimeter.len() - 1 {
        let top0 = index(perimeter[s].0, perimeter[s].1);
        let top1 = index(perimeter[s + 1].0, perimeter[s + 1].1);
        let (low0, low1) = (first_skirt + s as u32, first_skirt + s as u32 + 1);
        indices.extend_from_slice(&[top0, low0, top1, top1, low0, low1]);
    }

    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uv1)
        .with_inserted_indices(Indices::U32(indices))
}

/// Integer hash of two coordinates and a salt: the same inputs always give
/// the same bits, so anything placed from it is a pure function of where it
/// is (and comes back the same when a chunk streams in again).
pub fn hash(i: i32, j: i32, salt: u32) -> u32 {
    let mut h = (i as u32).wrapping_mul(0x8da6b343) ^ (j as u32).wrapping_mul(0xd8163841) ^ salt.wrapping_mul(0xcb1ab31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^ (h >> 15)
}

/// [`hash`] as a float in [0, 1).
pub fn unit(i: i32, j: i32, salt: u32) -> f32 {
    (hash(i, j, salt) & 0xffffff) as f32 / 16777216.0
}

/// Hash of the 1 m cell a point is in, to [0, 1): per-vertex variation in tint
/// functions. Inputs are floored, so scale them first for finer or coarser
/// grain.
pub fn hash2(x: f32, z: f32) -> f32 {
    let q = Vec2::new(x, z).floor();
    (q.dot(Vec2::new(12.9898, 78.233)).sin() * 43758.547).fract().abs()
}

/// Shareable handle for a ground; `World::ground` takes one.
pub fn ground<G: Ground>(g: G) -> Arc<dyn Ground> {
    Arc::new(g)
}
