//! Shadows beyond the sun's shadow maps. The cascades reach a kilometre or
//! three; under a low sun a crater wall or a massif throws its shadow much
//! further than that, and without it the far ground lies flat and evenly
//! lit, which is how a render gives itself away.
//!
//! So the ground round the camera is also kept as a height map, coarse but
//! wide (1024² over 32 km, about 31 m a texel), and the ground shader marches
//! from each far fragment toward the sun through it (`ground.wesl`,
//! `sunlit`). Near the camera the cascades do the work and the march fades
//! in only where they fade out.
//!
//! The map is built on the compute pool from the world's own height
//! function (`Ground::height_at` at the map's step) and built again when
//! the camera has gone an eighth of its width. It holds heights as half
//! floats relative to its centre's, which leaves them filterable and good
//! to a metre. The sun's direction is the shader's own: the material changes
//! only when a new map is in, never from frame to frame.

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    math::DVec2,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, futures::check_ready},
};

use crate::{
    ground::GroundMaterial,
    origin::Origin,
    quality::Quality,
    terrain::TerrainMaterial,
    world::WorldDef,
};

/// Texels along an edge, and the edge, m.
const TEXELS: u32 = 1024;
const SIZE: f64 = 32_768.0;
/// Rows built per task.
const BAND: u32 = 128;

pub struct HorizonPlugin;

impl Plugin for HorizonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(PostUpdate, (rebuild, tell_shader).chain());
    }
}

#[derive(Resource)]
pub(crate) struct Horizon {
    image: Handle<Image>,
    /// World position of the map's centre and the height it is relative
    /// to, once there is a map.
    ready: Option<(DVec2, f32)>,
    /// The map being built: its centre, its base height, and the bands.
    building: Option<(DVec2, f32, Vec<Task<Vec<u8>>>, Vec<Option<Vec<u8>>>)>,
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d { width: TEXELS, height: TEXELS, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0],
        TextureFormat::R16Float,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.sampler = ImageSampler::linear();
    commands.insert_resource(Horizon { image: images.add(image), ready: None, building: None });
}

fn rebuild(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    camera: Query<&Transform, With<Camera3d>>,
    mut horizon: ResMut<Horizon>,
    mut images: ResMut<Assets<Image>>,
) {
    if world.is_changed() {
        horizon.ready = None;
        horizon.building = None;
    }
    if !world.terrain.far_shadows {
        return;
    }
    let Some(eye) = camera.iter().next() else { return };
    let at = origin.world(eye.translation);
    let texel = SIZE / TEXELS as f64;

    if let Some((centre, base, tasks, done)) = &mut horizon.building {
        for (task, slot) in tasks.iter_mut().zip(done.iter_mut()) {
            if slot.is_none() {
                *slot = check_ready(task);
            }
        }
        if done.iter().all(Option::is_some) {
            let (centre, base) = (*centre, *base);
            let data: Vec<u8> = done.iter_mut().flat_map(|band| band.take().unwrap_or_default()).collect();
            if let Some(mut image) = images.get_mut(&horizon.image) {
                image.data = Some(data);
            }
            horizon.ready = Some((centre, base));
            horizon.building = None;
        }
        return;
    }

    let stale = horizon.ready.is_none_or(|(centre, _)| (centre - at).abs().max_element() > SIZE / 8.0);
    if !stale {
        return;
    }
    // On whole texels, so the same ground comes out the same.
    let centre = (at / texel).round() * texel;
    let base = world.ground.height_at(centre.x, centre.y, texel as f32);
    let pool = AsyncComputeTaskPool::get();
    let tasks: Vec<Task<Vec<u8>>> = (0..TEXELS / BAND)
        .map(|band| {
            let ground = world.ground.clone();
            pool.spawn(async move {
                let mut data = Vec::with_capacity((BAND * TEXELS * 2) as usize);
                for row in band * BAND..(band + 1) * BAND {
                    let z = centre.y + (row as f64 + 0.5 - TEXELS as f64 / 2.0) * texel;
                    for column in 0..TEXELS {
                        let x = centre.x + (column as f64 + 0.5 - TEXELS as f64 / 2.0) * texel;
                        let h = ground.height_at(x, z, texel as f32) - base;
                        data.extend(half::f16::from_f32(h).to_le_bytes());
                    }
                }
                data
            })
        })
        .collect();
    let done = vec![None; tasks.len()];
    horizon.building = Some((centre, base, tasks, done));
}

/// Where the map lies relative to the floating origin, and where the
/// march fades in: only what changes, only when it does.
fn tell_shader(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    quality: Res<Quality>,
    horizon: Res<Horizon>,
    terrain: Option<Res<TerrainMaterial>>,
    mut grounds: ResMut<Assets<GroundMaterial>>,
) {
    let Some(terrain) = terrain else { return };
    let (place, fade) = match horizon.ready {
        Some((centre, base)) if world.terrain.far_shadows && !crate::quality::off("far") => {
            let reach = quality.shadow_reach();
            let d = centre - origin.0;
            // The march's penumbra: the sun's disc, and never sharper
            // than the map can draw.
            let sun = world.sky.sun_radius.to_radians().tan().max(0.006);
            (Vec4::new(d.x as f32, d.y as f32, SIZE as f32, base), Vec4::new(0.55 * reach, 0.9 * reach, 1.0, sun))
        }
        _ => (Vec4::ZERO, Vec4::ZERO),
    };
    let Some(material) = grounds.get(&terrain.0) else { return };
    let same = material.extension.horizon_map.as_ref() == Some(&horizon.image) && material.extension.params.horizon == place && material.extension.params.horizon_fade == fade;
    if same {
        return;
    }
    if let Some(mut material) = grounds.get_mut(&terrain.0) {
        material.extension.horizon_map = Some(horizon.image.clone());
        material.extension.params.horizon = place;
        material.extension.params.horizon_fade = fade;
    }
}
