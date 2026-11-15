//! What hangs in the black sky: the Sun's disc, and the Earth, four times
//! the size the Moon shows us and lit by the same Sun, so its phase is always
//! the one the geometry gives. Both ride along with the camera, far beyond
//! the terrain: they are directions, not places.

use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use tranquility::prelude::*;

/// Inside the camera's far plane, outside everything else.
const DISTANCE: f32 = 30_000.0;
/// Angular radii, degrees, as seen from the Moon.
const SUN_RADIUS: f32 = 0.267;
const EARTH_RADIUS: f32 = 0.95;
/// Where the Earth hangs (it hardly moves in the lunar sky): degrees from
/// north through east, and above the horizon.
const EARTH_AZIMUTH: f32 = 322.0;
const EARTH_ELEVATION: f32 = 21.0;

pub struct HeavensPlugin;

impl Plugin for HeavensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, follow);
    }
}

#[derive(Component)]
struct SunDisc;

#[derive(Component)]
struct Earth;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let size = |degrees: f32| Vec3::splat(DISTANCE * degrees.to_radians().tan());
    commands.spawn((
        SunDisc,
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(3).expect("valid subdivision"))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            // Far beyond white at any exposure: the bloom makes the glare.
            emissive: LinearRgba::rgb(4.0e6, 3.8e6, 3.5e6),
            ..default()
        })),
        Transform::from_scale(size(SUN_RADIUS)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    commands.spawn((
        Earth,
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(96, 48))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(images.add(earth_map(1024, 512))),
            perceptual_roughness: 0.85,
            reflectance: 0.3,
            ..default()
        })),
        // Tilted, the Atlantic-ish side toward us.
        Transform::from_scale(size(EARTH_RADIUS)).with_rotation(Quat::from_euler(EulerRot::ZYX, 0.35, 2.1, 0.2)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

fn follow(
    sun: Res<Sun>,
    camera: Single<&Transform, (With<Camera3d>, Without<SunDisc>, Without<Earth>)>,
    mut disc: Single<&mut Transform, (With<SunDisc>, Without<Earth>)>,
    mut earth: Single<&mut Transform, (With<Earth>, Without<SunDisc>)>,
) {
    disc.translation = camera.translation + sun.direction() * DISTANCE;
    let (el, az) = (EARTH_ELEVATION.to_radians(), EARTH_AZIMUTH.to_radians());
    earth.translation = camera.translation + Vec3::new(el.cos() * az.sin(), el.sin(), -el.cos() * az.cos()) * DISTANCE;
}

/// An Earth-like globe as an equirectangular map: oceans, continents green
/// to tan by latitude and dryness, polar ice, and weather over all of it.
/// Not our Earth's coastlines, but the same blue marble.
fn earth_map(width: u32, height: u32) -> Image {
    let land = Fbm::<Perlin>::new(21).set_octaves(6).set_frequency(1.3);
    let dry = Fbm::<Perlin>::new(22).set_octaves(4).set_frequency(2.2);
    let weather = Fbm::<Perlin>::new(23).set_octaves(6).set_frequency(2.6);
    let swirl = Fbm::<Perlin>::new(24).set_octaves(3).set_frequency(1.1);
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| {
        let t = t.clamp(0.0, 1.0);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    };
    let step = |a: f32, b: f32, x: f32| {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for j in 0..height {
        let lat = (0.5 - (j as f32 + 0.5) / height as f32) * std::f32::consts::PI;
        for i in 0..width {
            let lon = (i as f32 + 0.5) / width as f32 * std::f32::consts::TAU;
            let p = [(lat.cos() * lon.cos()) as f64, lat.sin() as f64, (lat.cos() * lon.sin()) as f64];
            let h = land.get(p) as f32;
            let d = dry.get(p) as f32;
            let polar = lat.abs() / std::f32::consts::FRAC_PI_2;
            // Linear albedo; the sea darkest, deeper further from shore.
            let sea = mix([0.03, 0.09, 0.2], [0.012, 0.035, 0.11], -h * 4.0);
            let green = mix([0.05, 0.11, 0.035], [0.1, 0.13, 0.05], d + 0.5);
            let tan = [0.36, 0.27, 0.15];
            // Deserts sit in the horse latitudes, forests at the equator
            // and further poleward.
            let arid = step(0.1, 0.5, 1.0 - (polar - 0.3).abs() * 4.0 + d * 1.5);
            let ground = mix(green, tan, arid);
            let mut c = mix(sea, ground, step(0.0, 0.03, h - 0.04));
            let ice = step(0.78, 0.9, polar + 0.08 * d + 0.1 * h.max(0.0));
            c = mix(c, [0.8, 0.83, 0.86], ice);
            // Weather: fronts drawn out along the latitudes and stirred.
            let s = swirl.get(p) * 0.5;
            let q = [p[0] + s, p[1] * 2.2 - s, p[2] + s];
            let cloud = step(-0.02, 0.45, weather.get(q) as f32 + 0.12 * polar);
            c = mix(c, [0.82, 0.83, 0.85], cloud * 0.95);
            // Air: a thin blue veil.
            c = mix(c, [0.25, 0.4, 0.7], 0.12);
            data.extend(c.map(|v| (v.powf(1.0 / 2.2) * 255.0).min(255.0) as u8));
            data.push(255);
        }
    }
    Image::new(
        Extent3d { width, height, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}
