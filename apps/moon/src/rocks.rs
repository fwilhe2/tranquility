//! Rocks, scattered on the ground near the camera as it streams in.
//!
//! Where a rock lies, how big it is and which way it is turned all come
//! from a hash of the 8 m cell it is in, never from the chunk that happens
//! to carry it: the same boulder is there whichever level of the ground it
//! is seen on, and when you come back after a day's walk. Far levels carry
//! only the rocks big enough to see from there.
//!
//! Sizes follow a power law, as counted from Surveyor to Curiosity: many
//! stones to every boulder. The ground shader draws everything under a
//! quarter of a metre; these are the rest.

use std::sync::Arc;

use tranquility::{ground::prop, prelude::*, terrain::ChunkMeshed};

use crate::{
    Current,
    body::{Land, Rocks},
};

/// Rocks are placed in square cells this size, m, each with so many slots.
const CELL: f64 = 8.0;
const SLOTS: u32 = 12;
/// Smallest rock that is a mesh, m; and the factor by which that grows
/// with each level of the ground.
const SMALLEST: f32 = 0.25;
const PER_LEVEL: f32 = 2.6;
/// Levels of the ground that carry rocks.
const LEVELS: u32 = 3;
const SHAPES: usize = 12;
const TONES: usize = 4;

pub struct RocksPlugin;

impl Plugin for RocksPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, (repaint.run_if(resource_changed::<Current>), scatter).chain());
    }
}

#[derive(Resource)]
struct Kit {
    shapes: Vec<Handle<Mesh>>,
    rounded: Vec<Handle<Mesh>>,
    materials: Vec<Handle<PropMaterial>>,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<PropMaterial>>) {
    commands.insert_resource(Kit {
        shapes: (0..SHAPES).map(|k| meshes.add(lumpy(k as u32, false))).collect(),
        rounded: (0..SHAPES).map(|k| meshes.add(lumpy(k as u32 + 100, true))).collect(),
        materials: (0..TONES).map(|_| materials.add(prop(StandardMaterial::default()))).collect(),
    });
}

/// The body's rocks: a few tones between its darkest and brightest.
fn repaint(current: Res<Current>, kit: Res<Kit>, mut materials: ResMut<Assets<PropMaterial>>) {
    let look = &current.0.rocks;
    for (k, handle) in kit.materials.iter().enumerate() {
        let a = look.albedo[0] + (look.albedo[1] - look.albedo[0]) * k as f32 / (TONES - 1) as f32;
        if let Some(mut material) = materials.get_mut(handle) {
            material.base = StandardMaterial {
                base_color: Color::linear_rgb(a * look.tint[0], a * look.tint[1], a * look.tint[2]),
                perceptual_roughness: look.roughness,
                reflectance: 0.25,
                ..default()
            };
        }
    }
}

/// A boulder: a sphere pushed about in broad lumps, then sliced by a few
/// random planes (rocks break along flat faces), every vertex nudged and
/// the bottom cut flat. No two shapes alike, each reused many times.
/// `rounded` leaves out the slicing: a cobble rolled smooth.
fn lumpy(seed: u32, rounded: bool) -> Mesh {
    let s = seed as i32;
    let dir = |k: i32, salt: u32| {
        Vec3::new(unit(s, k, salt) - 0.5, unit(s, k, salt + 1) - 0.5, unit(s, k, salt + 2) - 0.5).normalize_or(Vec3::Y)
    };
    let lumps: Vec<(Vec3, f32)> = (0..4).map(|k| (dir(k, 20), 0.12 + 0.2 * unit(s, k, 23))).collect();
    let cuts: Vec<(Vec3, f32)> = (0..if rounded { 0 } else { 8 }).map(|k| (dir(k, 30), 0.5 + 0.38 * unit(s, k, 33))).collect();
    let mut mesh = Sphere::new(1.0).mesh().ico(3).expect("valid subdivision");
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) {
        for p in positions.iter_mut() {
            let v = Vec3::from(*p);
            let swell: f32 = lumps.iter().map(|(d, a)| a * v.dot(*d)).sum();
            let mut q = v * (0.95 + swell);
            for (d, at) in &cuts {
                let over = q.dot(*d) - at;
                if over > 0.0 {
                    q -= *d * over;
                }
            }
            // Pitted and chipped, by a hash of where the vertex is (the
            // sphere's seam vertices are doubled and must agree).
            let key = (v * 512.0).round().as_ivec3();
            let chip = unit(key.x.wrapping_mul(31).wrapping_add(key.z), key.y, seed);
            q *= if rounded { 0.98 + 0.04 * chip } else { 0.95 + 0.1 * chip };
            // A buried bottom, as on most real boulders.
            q.y = q.y.max(-0.3);
            *p = q.to_array();
        }
    }
    if rounded {
        mesh.compute_smooth_normals();
    } else {
        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
    }
    mesh
}

/// A rock, as its cell's hash has it.
pub struct Rock {
    pub x: f64,
    pub z: f64,
    /// Height of the ground under it.
    pub y: f32,
    /// Its longest axis, m.
    pub size: f32,
    pub wide: f32,
    pub tall: f32,
    pub turn: f32,
    shape: usize,
    tone: usize,
}

impl Rock {
    /// How far its flanks reach from its middle, m (it is half buried and
    /// narrows toward the top: a body meets it a little inside its outline).
    pub fn reach(&self) -> f32 {
        0.42 * self.size * (0.5 + 0.5 * self.wide)
    }
}

/// The rocks at least `least` across in cell (i, j): the same ones whoever
/// asks, the meshes strewn on a chunk or a walker about to bark a shin.
pub fn in_cell(land: &dyn Land, look: &Rocks, sea: Option<f32>, i: i64, j: i64, least: f32, mut each: impl FnMut(Rock)) {
    if look.density <= 0.0 {
        return;
    }
    let cell = hash(i as i32, j as i32, 0x70c5);
    for slot in 0..SLOTS {
        let r = |salt: u32| unit(cell as i32, slot as i32, salt);
        // N(>D) ∝ D^-1.8.
        let size = SMALLEST * (1.0 - r(3)).max(1e-4).powf(-1.0 / 1.8);
        if size < least {
            continue;
        }
        let (x, z) = ((i as f64 + r(1) as f64) * CELL, (j as f64 + r(2) as f64) * CELL);
        let rocky = land.rockiness(x, z);
        if r(9) > look.density * rocky * (CELL * CELL) as f32 / SLOTS as f32 {
            continue;
        }
        // The big blocks lie only where something dug them out.
        let size = size.min(look.largest * (0.15 + 0.85 * rocky));
        if size < least {
            continue;
        }
        let y = land.height(x, z);
        if sea.is_some_and(|level| y < level) {
            continue;
        }
        each(Rock {
            x,
            z,
            y,
            size,
            wide: 0.75 + 0.5 * r(7),
            tall: 0.45 + 0.4 * r(8),
            turn: r(6) * std::f32::consts::TAU,
            shape: (r(4) * SHAPES as f32) as usize % SHAPES,
            tone: (r(5) * TONES as f32) as usize % TONES,
        });
    }
}

/// Rocks from this size up are in the way; smaller ones are stepped over.
pub const SOLID: f32 = 0.5;

/// Whether a rock too big to step over stands within `margin` of a point.
pub fn solid(land: &dyn Land, look: &Rocks, sea: Option<f32>, x: f64, z: f64, margin: f32) -> bool {
    if look.density <= 0.0 || look.largest < SOLID {
        return false;
    }
    // No rock reaches further than half the largest: at most the cells
    // that far round the point.
    let far = (0.5 * look.largest + margin) as f64;
    let (i0, i1) = (((x - far) / CELL).floor() as i64, ((x + far) / CELL).floor() as i64);
    let (j0, j1) = (((z - far) / CELL).floor() as i64, ((z + far) / CELL).floor() as i64);
    let mut hit = false;
    for j in j0..=j1 {
        for i in i0..=i1 {
            in_cell(land, look, sea, i, j, SOLID, |rock| {
                let (dx, dz) = ((rock.x - x) as f32, (rock.z - z) as f32);
                let reach = rock.reach() + margin;
                hit |= dx * dx + dz * dz < reach * reach;
            });
        }
    }
    hit
}

/// A body's land with its rocks on it: the ground the walker meets.
pub struct Strewn {
    pub land: Arc<dyn Land>,
    pub rocks: Rocks,
    pub sea: Option<f32>,
}

impl Ground for Strewn {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.land.height(x, z)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        self.land.height_at(x, z, step)
    }

    fn tint(&self, x: f64, z: f64, height: f32, normal: Vec3) -> LinearRgba {
        self.land.tint(x, z, height, normal)
    }

    fn tint_at(&self, x: f64, z: f64, height: f32, normal: Vec3, step: f32) -> LinearRgba {
        self.land.tint_at(x, z, height, normal, step)
    }

    fn stand_height(&self, x: f64, z: f64) -> f32 {
        self.land.stand_height(x, z)
    }

    fn blocked(&self, x: f64, z: f64) -> bool {
        self.land.blocked(x, z) || solid(&*self.land, &self.rocks, self.sea, x, z, 0.0)
    }
}

#[derive(Component)]
struct Scattered;

fn scatter(
    mut commands: Commands,
    mut meshed: MessageReader<ChunkMeshed>,
    current: Res<Current>,
    kit: Res<Kit>,
    done: Query<(), With<Scattered>>,
) {
    let body = &current.0;
    let look = &body.rocks;
    if look.density <= 0.0 {
        meshed.clear();
        return;
    }
    let shapes = if look.rounded { &kit.rounded } else { &kit.shapes };
    let sea = body.sea.as_ref().map(|sea| sea.level);
    for chunk in meshed.read() {
        if chunk.level >= LEVELS || done.contains(chunk.entity) {
            continue;
        }
        let Ok(mut entity) = commands.get_entity(chunk.entity) else { continue };
        entity.insert(Scattered);
        let least = SMALLEST * PER_LEVEL.powi(chunk.level as i32);
        let cells = (chunk.size as f64 / CELL).round() as i64;
        let (i0, j0) = ((chunk.origin.x / CELL).round() as i64, (chunk.origin.y / CELL).round() as i64);
        entity.with_children(|parent| {
            for j in j0..j0 + cells {
                for i in i0..i0 + cells {
                    in_cell(&*body.land, look, sea, i, j, least, |rock| {
                        parent.spawn((
                            Mesh3d(shapes[rock.shape].clone()),
                            MeshMaterial3d(kit.materials[rock.tone].clone()),
                            Transform::from_xyz((rock.x - chunk.origin.x) as f32, rock.y + 0.5 * rock.size * rock.tall * 0.2, (rock.z - chunk.origin.y) as f32)
                                .with_rotation(Quat::from_rotation_y(rock.turn))
                                .with_scale(0.5 * rock.size * Vec3::new(rock.wide, rock.tall, 1.0)),
                        ));
                    });
                }
            }
        });
    }
}
