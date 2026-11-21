//! The ground, streamed in square chunks on nested levels around the camera.
//!
//! Level 0 chunks are the smallest and finest; each further level's are
//! twice the size with the same number of quads. A level covers a block of
//! the next coarser level's chunks round the one the camera is in, and that
//! coarser level draws everything of its own block but those: no level
//! overlaps another, and the levels' edges move only when the camera crosses
//! a chunk of the coarser one. Chunks are meshed on the async compute pool.
//! Skirts hang down from every chunk edge to hide the cracks between
//! neighbours of different levels.
//!
//! Chunks are placed relative to the floating [`Origin`], and on a round
//! world they fall away from the camera by the body's curvature (in the
//! vertex shader: see `ground.rs`).

use std::sync::Arc;

use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    light::NotShadowCaster,
    math::{DVec2, I64Vec2},
    mesh::Indices,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::render_resource::PrimitiveTopology,
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};

use crate::{
    ground::{self, GroundMaterial, Pattern},
    quality::Quality,
    origin::Origin,
    world::{Ground, TerrainSettings, WorldDef},
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
            .add_systems(
                Update,
                (reset.run_if(crate::world_replaced), finish_chunks, stream_chunks, cast.run_if(resource_changed::<Quality>)).chain(),
            );
    }
}

/// Sent when a chunk has been meshed. Apps decorate chunks from it (rocks,
/// grass), as children of `entity` so they go when the chunk goes.
#[derive(Message, Clone, Copy)]
pub struct ChunkMeshed {
    pub level: u32,
    /// The chunk's place on its level's grid.
    pub coord: IVec2,
    pub entity: Entity,
    /// The chunk's north-west corner in the world, and its edge, m.
    pub origin: DVec2,
    pub size: f32,
    /// Distance between its vertices, m.
    pub step: f32,
}

#[derive(Resource)]
pub(crate) struct TerrainMaterial(pub Handle<GroundMaterial>);

/// A chunk of ground, and the level it is on.
#[derive(Component)]
pub struct Chunk(pub u32);

/// A chunk's level and its place on that level's grid.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub level: u32,
    pub coord: IVec2,
}

impl Key {
    /// The square it covers, in level 0 chunks: min and (exclusive) max.
    fn span(self) -> (I64Vec2, I64Vec2) {
        let min = I64Vec2::new(self.coord.x as i64, self.coord.y as i64) << self.level;
        (min, min + (I64Vec2::ONE << self.level))
    }

    fn overlaps(self, other: Key) -> bool {
        let ((a0, a1), (b0, b1)) = (self.span(), other.span());
        a0.x < b1.x && b0.x < a1.x && a0.y < b1.y && b0.y < a1.y
    }
}

#[derive(Resource, Default)]
pub struct Chunks {
    loaded: HashMap<Key, Entity>,
    jobs: HashMap<Key, Task<Built>>,
    /// The plan, and what it was made for: the levels streamed, the chunk
    /// of the finest the camera was in, and the view (see `View`).
    wanted: HashSet<Key>,
    planned_for: Option<((u32, u32), IVec2, Option<IVec4>)>,
    /// The levels streamed now, finest and coarsest.
    span: (u32, u32),
}

impl Chunks {
    /// True when every chunk in view is there.
    pub fn is_settled(&self) -> bool {
        self.jobs.is_empty() && !self.loaded.is_empty() && self.wanted.iter().all(|k| self.loaded.contains_key(k))
    }

    /// The levels streamed now, finest and coarsest
    /// ([`TerrainSettings::span`]).
    pub fn span(&self) -> (u32, u32) {
        self.span
    }
}

/// Which chunks should exist with the camera at `camera`, streaming the
/// levels `finest..=coarsest`: every level's block, less the part the
/// next finer level covers.
pub fn plan(settings: &TerrainSettings, camera: DVec2, (finest, coarsest): (u32, u32)) -> HashSet<Key> {
    let mut wanted = HashSet::new();
    let q = settings.ring.max(1);
    let cell = |level: u32| {
        let c = (camera / settings.size(level)).floor();
        IVec2::new(c.x as i32, c.y as i32)
    };
    // The coarsest level's block is centred on the camera's own chunk; each
    // finer one is a block of its parent's chunks, cut in four.
    let top = cell(coarsest);
    let (mut lo, mut hi) = (top - IVec2::splat(2 * q + 1), top + IVec2::splat(2 * q + 1));
    for level in (finest..=coarsest).rev() {
        let (hole_lo, hole_hi) = if level > finest {
            (cell(level) - IVec2::splat(q), cell(level) + IVec2::splat(q))
        } else {
            (IVec2::MAX, IVec2::MIN)
        };
        for z in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                let inside = x >= hole_lo.x && x <= hole_hi.x && z >= hole_lo.y && z <= hole_hi.y;
                if !inside {
                    wanted.insert(Key { level, coord: IVec2::new(x, z) });
                }
            }
        }
        if level > finest {
            (lo, hi) = (hole_lo * 2, hole_hi * 2 + IVec2::ONE);
        }
    }
    wanted
}

/// What a narrow view needs of the ground beyond the plan: where the eye
/// is (world x, z, and its height over the ground), which way it looks
/// in render space, and its field of view. The plan's levels give the
/// ground a grid step of about a 200th of its distance, right for the
/// usual 70°; a telephoto view magnifies that, so chunks in its frustum
/// are split until their step is as small, to its scale, as the plan's is
/// to 70°'s. Nothing is split for a view of 70° or more, unless the
/// quality tier asks for finer ground everywhere in view (`detail`).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub at: DVec2,
    pub height: f32,
    pub forward: Vec3,
    /// Vertical field of view and width over height.
    pub fov: f32,
    pub aspect: f32,
    /// How many times finer than the plan's the ground in view should be
    /// cut, beyond what the field asks (`Quality::detail`): 1 usually.
    pub detail: f32,
}

/// The field of view the plan's levels are made for, radians.
const USUAL_FOV: f32 = 70.0 * std::f32::consts::PI / 180.0;
/// Step over distance that the plan's levels keep to at the usual field.
const STEP_OVER_DISTANCE: f32 = 1.0 / 192.0;

impl View {
    /// The view, coarsely, for knowing when to plan again: the direction to
    /// two degrees and the field to a tenth of a degree.
    fn key(&self) -> Option<IVec4> {
        if self.fov >= USUAL_FOV * 0.98 && self.detail <= 1.0 {
            return None;
        }
        let q = |v: f32| (v * 28.6).round() as i32;
        Some(IVec4::new(q(self.forward.x), q(self.forward.y), q(self.forward.z), (self.fov.to_degrees() * 10.0) as i32))
    }
}

/// Splits the plan's chunks in a narrow view's frustum until their step
/// is fine enough for it (see [`View`]). Chunks keep tiling the ground
/// exactly once: a chunk is replaced by its four quarters.
pub fn refine(wanted: &mut HashSet<Key>, settings: &TerrainSettings, view: &View) {
    if view.fov >= USUAL_FOV * 0.98 && view.detail <= 1.0 {
        return;
    }
    let limit = STEP_OVER_DISTANCE * view.fov.min(USUAL_FOV) / USUAL_FOV / view.detail.max(1.0);
    // Half the diagonal field, for a cone round the view.
    let half = ((view.fov / 2.0).tan() * (1.0 + view.aspect * view.aspect).sqrt()).atan();
    let radius = settings.radius;
    let needs = |key: Key| -> bool {
        if key.level == 0 {
            return false;
        }
        let size = settings.size(key.level);
        let centre = (key.coord.as_dvec2() + 0.5) * size;
        let d = centre - view.at;
        let flat = d.length() as f32;
        let drop = radius.map_or(0.0, |r| crate::ground::drop_at(flat.min(r), r));
        let to = Vec3::new(d.x as f32, -view.height - drop, d.y as f32);
        let distance = to.length().max(1.0);
        // In the cone, allowing for the chunk's own size.
        let reach = (size as f32 * 0.75 / distance).min(1.5);
        let angle = to.normalize().dot(view.forward).clamp(-1.0, 1.0).acos();
        if angle > half + reach {
            return false;
        }
        // Its nearest point, roughly: the centre less half a diagonal.
        let near = (distance - size as f32 * 0.71).max(view.height.max(1.0));
        settings.step(key.level) / near > limit
    };
    let mut todo: Vec<Key> = wanted.iter().copied().filter(|k| needs(*k)).collect();
    while let Some(key) = todo.pop() {
        if !wanted.remove(&key) {
            continue;
        }
        for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let child = Key { level: key.level - 1, coord: key.coord * 2 + IVec2::new(dx, dz) };
            wanted.insert(child);
            if needs(child) {
                todo.push(child);
            }
        }
    }
}

fn setup_material(mut commands: Commands, world: Res<WorldDef>, mut materials: ResMut<Assets<GroundMaterial>>) {
    commands.insert_resource(TerrainMaterial(materials.add(ground::material(&world.terrain))));
}

/// Another world: none of this ground is its ground.
fn reset(
    mut commands: Commands,
    mut chunks: ResMut<Chunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GroundMaterial>>,
    material: Res<TerrainMaterial>,
    world: Res<WorldDef>,
    mesh_handles: Query<&Mesh3d, With<Chunk>>,
) {
    for (_, entity) in chunks.loaded.drain() {
        if let Ok(mesh) = mesh_handles.get(entity) {
            meshes.remove(&mesh.0);
        }
        commands.entity(entity).despawn();
    }
    chunks.jobs.clear();
    chunks.wanted.clear();
    chunks.planned_for = None;
    chunks.span = (0, world.terrain.levels.max(1) - 1);
    if let Some(mut m) = materials.get_mut(&material.0) {
        *m = ground::material(&world.terrain);
    }
}

fn stream_chunks(
    mut commands: Commands,
    mut chunks: ResMut<Chunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    camera: Single<(&Transform, &mut Projection), With<Camera3d>>,
    window: Option<Single<&Window>>,
    quality: Option<Res<crate::quality::Quality>>,
    mesh_handles: Query<&Mesh3d, With<Chunk>>,
) {
    let settings = &world.terrain;
    let (camera, mut projection) = camera.into_inner();
    let at = origin.world(camera.translation);
    let height = camera.translation.y - world.ground.height(at.x, at.y);
    let span = settings.span(height);
    let here = (at / settings.size(span.0)).floor();
    let here = IVec2::new(here.x as i32, here.y as i32);
    let fov = match &*projection {
        Projection::Perspective(p) => p.fov,
        _ => USUAL_FOV,
    };
    let aspect = window.map_or(16.0 / 9.0, |w| w.width() / w.height().max(1.0));
    let detail = quality.map_or(1.0, |q| q.detail());
    let view = View { at, height, forward: *camera.forward(), fov, aspect, detail };
    let chunks = &mut *chunks;
    if chunks.planned_for != Some((span, here, view.key())) {
        chunks.wanted = plan(settings, at, span);
        refine(&mut chunks.wanted, settings, &view);
        debug!("terrain: {} chunks for levels {:?}, view {:?}", chunks.wanted.len(), span, view.key());
        chunks.planned_for = Some((span, here, view.key()));
        if chunks.span.1 != span.1
            && let Projection::Perspective(p) = &mut *projection
        {
            p.far = far(settings, span.1);
        }
        chunks.span = span;
        // Jobs for chunks no longer wanted are dropped, which cancels them.
        chunks.jobs.retain(|key, _| chunks.wanted.contains(key));
    }

    // What is still missing, the finest and nearest first.
    let mut missing: Vec<(u32, i64, Key)> = chunks
        .wanted
        .iter()
        .filter(|key| !chunks.loaded.contains_key(*key))
        .map(|&key| {
            let (min, max) = key.span();
            let centre = (min + max) * (settings.chunk_size as i64) / 2;
            let d = centre - I64Vec2::new(at.x as i64, at.y as i64);
            (key.level, d.x * d.x + d.y * d.y, key)
        })
        .collect();

    // A chunk that is no longer wanted stays until the ground replacing it
    // is there, so moving never opens a hole. (For a few frames it may
    // overlap the new ground.)
    chunks.loaded.retain(|&key, entity| {
        let keep = chunks.wanted.contains(&key) || missing.iter().any(|(_, _, m)| m.overlaps(key));
        if !keep {
            if let Ok(mesh) = mesh_handles.get(*entity) {
                meshes.remove(&mesh.0);
            }
            commands.entity(*entity).despawn();
        }
        keep
    });

    missing.sort_unstable_by_key(|m| (m.0, m.1));
    let pool = AsyncComputeTaskPool::get();
    for (_, _, key) in missing {
        if chunks.jobs.len() >= MAX_JOBS {
            break;
        }
        if chunks.jobs.contains_key(&key) {
            continue;
        }
        let ground = world.ground.clone();
        let (size, n) = (settings.size(key.level), settings.resolution);
        let task = pool.spawn(async move { build_chunk(&*ground, key, size, n) });
        chunks.jobs.insert(key, task);
    }
}

fn finish_chunks(
    mut commands: Commands,
    mut chunks: ResMut<Chunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    material: Res<TerrainMaterial>,
    quality: Res<Quality>,
    mut meshed: MessageWriter<ChunkMeshed>,
) {
    let settings = &world.terrain;
    let chunks = &mut *chunks;
    let mut done = Vec::new();
    for (&key, task) in chunks.jobs.iter_mut() {
        if let Some(built) = check_ready(task) {
            done.push((key, built));
        }
    }
    for (key, built) in done {
        chunks.jobs.remove(&key);
        let size = settings.size(key.level);
        let corner = key.coord.as_dvec2() * size;
        // The curve can pull a far chunk a long way down: the box it is
        // culled by has to reach that far, or it vanishes at the top of
        // the screen while still in view.
        let sink = settings.radius.map_or(0.0, |r| {
            let far = 6.0 * size as f32 * settings.ring.max(1) as f32;
            ground::drop_at(far, r)
        });
        let (lo, hi) = (built.low - sink, built.high);
        let half = size as f32 / 2.0;
        let mut chunk = commands.spawn((
            Chunk(key.level),
            Mesh3d(meshes.add(built.mesh)),
            MeshMaterial3d(material.0.clone()),
            Transform::from_translation(origin.render(corner, 0.0)),
            Aabb::from_min_max(Vec3::new(0.0, lo, 0.0), Vec3::new(2.0 * half, hi, 2.0 * half)),
            bevy::camera::visibility::NoAutoAabb,
        ));
        if !world.terrain.casts_shadows || key.level > quality.shadow_levels() {
            chunk.insert(NotShadowCaster);
        }
        let entity = chunk.id();
        if let Some(old) = chunks.loaded.insert(key, entity) {
            commands.entity(old).despawn();
        }
        meshed.write(ChunkMeshed {
            level: key.level,
            coord: key.coord,
            entity,
            origin: corner,
            size: size as f32,
            step: settings.step(key.level),
        });
    }
}

/// Another quality tier: which levels of the ground cast shadows changes.
fn cast(mut commands: Commands, world: Res<WorldDef>, quality: Res<Quality>, chunks: Query<(Entity, &Chunk)>) {
    for (entity, chunk) in &chunks {
        if world.terrain.casts_shadows && chunk.0 <= quality.shadow_levels() {
            commands.entity(entity).remove::<NotShadowCaster>();
        } else {
            commands.entity(entity).insert(NotShadowCaster);
        }
    }
}

/// The camera's far plane with ground out to `coarsest`.
pub(crate) fn far(settings: &TerrainSettings, coarsest: u32) -> f32 {
    (settings.reach_to(coarsest) * 1.6).max(40_000.0)
}

struct Built {
    mesh: Mesh,
    low: f32,
    high: f32,
}

/// Meshes one chunk: an (n+1)² grid of vertices in chunk-local space, plus a
/// skirt around its edge.
fn build_chunk(ground: &dyn Ground, key: Key, size: f64, n: u32) -> Built {
    let n = n as usize;
    let step = size / n as f64;
    let origin = key.coord.as_dvec2() * size;
    let stepf = step as f32;

    // Heights on the grid plus a one-sample border, so normals along the edge
    // come out the same as in the neighbouring chunk.
    let side = n + 3;
    let mut heights = vec![0.0f32; side * side];
    for j in 0..side {
        for i in 0..side {
            let x = origin.x + (i as f64 - 1.0) * step;
            let z = origin.y + (j as f64 - 1.0) * step;
            heights[j * side + i] = ground.height_at(x, z, stepf);
        }
    }

    let verts = (n + 1) * (n + 1);
    let mut positions = Vec::with_capacity(verts + 4 * n + 1);
    let mut normals = Vec::with_capacity(verts + 4 * n + 1);
    let mut colors = Vec::with_capacity(verts + 4 * n + 1);
    let mut uv0 = Vec::with_capacity(verts + 4 * n + 1);
    let mut uv1 = Vec::with_capacity(verts + 4 * n + 1);
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for j in 0..=n {
        for i in 0..=n {
            let y = heights[(j + 1) * side + i + 1];
            (low, high) = (low.min(y), high.max(y));
            // Central differences, reaching into the border where needed.
            let hx = heights[(j + 1) * side + i + 2] - heights[(j + 1) * side + i];
            let hz = heights[(j + 2) * side + i + 1] - heights[j * side + i + 1];
            let normal = Vec3::new(-hx, 2.0 * stepf, -hz).normalize();
            positions.push([i as f32 * stepf, y, j as f32 * stepf]);
            normals.push(normal.to_array());
            let tint = ground.tint_at(origin.x + i as f64 * step, origin.y + j as f64 * step, y, normal, stepf);
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
    // only has to cover the height mismatch with a neighbour of the next
    // level, which is what a grid twice as coarse cannot follow.
    let drop = 1.5 * stepf + 0.5;
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

    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uv1)
        .with_inserted_indices(Indices::U32(indices));
    Built { mesh, low: low - drop, high }
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
pub fn hash2(x: f64, z: f64) -> f32 {
    unit(x.floor() as i64 as i32, z.floor() as i64 as i32, 0x51ed)
}

/// Shareable handle for a ground; `World::ground` takes one.
pub fn ground<G: Ground>(g: G) -> Arc<dyn Ground> {
    Arc::new(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A narrow view splits chunks in its frustum only, and the plan still
    /// tiles; a usual one changes nothing.
    #[test]
    fn a_narrow_view_refines_its_frustum() {
        let moon = TerrainSettings { levels: 8, chunk_size: 64.0, radius: Some(1_737_400.0), ..Default::default() };
        let at = DVec2::new(0.0, 0.0);
        let span = moon.span(112_000.0);
        let base = plan(&moon, at, span);
        let forward = Vec3::new(-0.95, -0.3, 0.0).normalize();
        let mut wide = base.clone();
        refine(&mut wide, &moon, &View { at, height: 112_000.0, forward, fov: 70f32.to_radians(), aspect: 1.5, detail: 1.0 });
        assert_eq!(wide, base);
        let mut narrow = base.clone();
        refine(&mut narrow, &moon, &View { at, height: 112_000.0, forward, fov: 12.8f32.to_radians(), aspect: 1.0, detail: 1.0 });
        assert!(narrow.len() > base.len());
        // Finer ground ahead (west), none behind: the level of the chunk
        // holding a point 375 km off either way.
        let level_at = |set: &HashSet<Key>, x: f64| {
            set.iter()
                .find(|k| {
                    let (lo, size) = (k.coord.as_dvec2() * moon.size(k.level), moon.size(k.level));
                    (lo.x..lo.x + size).contains(&x) && (lo.y..lo.y + size).contains(&10.0)
                })
                .map(|k| k.level)
        };
        assert!(level_at(&narrow, -375_000.0) < level_at(&base, -375_000.0));
        assert_eq!(level_at(&narrow, 375_000.0), level_at(&base, 375_000.0));
        let mut covered: HashMap<I64Vec2, u32> = HashMap::default();
        for key in &narrow {
            let (min, max) = key.span();
            let step = 1i64 << span.0.min(key.level);
            let mut z = min.y;
            while z < max.y {
                let mut x = min.x;
                while x < max.x {
                    *covered.entry(I64Vec2::new(x, z)).or_default() += 1;
                    x += step;
                }
                z += step;
            }
        }
        assert!(covered.values().all(|&n| n == 1));
    }

    /// From high up the finest levels go and coarser ones come; on the
    /// ground nothing changes, and a plan of any span still tiles.
    #[test]
    fn span_follows_the_eye() {
        let moon = TerrainSettings { levels: 8, chunk_size: 64.0, radius: Some(1_737_400.0), ..Default::default() };
        assert_eq!(moon.span(1.7), (0, 7));
        let (finest, coarsest) = moon.span(112_000.0);
        assert!(finest >= 6, "{finest}");
        let horizon = (2.0f32 * 1_737_400.0 * 112_000.0).sqrt();
        assert!(moon.reach_to(coarsest) > horizon, "{coarsest}");
        let flat = TerrainSettings { levels: 6, ..Default::default() };
        assert_eq!(flat.span(5000.0).1, 5);
        let wanted = plan(&moon, DVec2::new(1.0e5, -3.0e4), (finest, coarsest));
        assert!(wanted.iter().all(|k| (finest..=coarsest).contains(&k.level)));
    }

    /// At any camera position the plan tiles its whole square exactly once.
    #[test]
    fn plan_covers_without_holes_or_overlap() {
        for (levels, ring) in [(1, 1), (4, 1), (6, 1), (5, 2)] {
            let settings = TerrainSettings { levels, ring, chunk_size: 64.0, ..Default::default() };
            for camera in [DVec2::ZERO, DVec2::new(-0.5, 63.9), DVec2::new(12345.6, -98765.4), DVec2::new(-3.0e7, 5.0e6)] {
                let wanted = plan(&settings, camera, (0, levels - 1));
                // Count level 0 cells covered.
                let mut covered: HashMap<I64Vec2, u32> = HashMap::default();
                for key in &wanted {
                    let (min, max) = key.span();
                    for z in min.y..max.y {
                        for x in min.x..max.x {
                            *covered.entry(I64Vec2::new(x, z)).or_default() += 1;
                        }
                    }
                }
                assert!(covered.values().all(|&n| n == 1), "overlap at {camera}");
                let top = (camera / settings.size(levels - 1)).floor();
                let r = (2 * ring + 1) as i64;
                let edge = (2 * r + 1) << (levels - 1);
                assert_eq!(covered.len() as i64, edge * edge, "holes at {camera} ({levels} levels)");
                let lo = (I64Vec2::new(top.x as i64, top.y as i64) - r) << (levels - 1);
                assert!(covered.contains_key(&lo));
            }
        }
    }
}
