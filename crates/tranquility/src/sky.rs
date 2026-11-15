//! Sun, sky (Bevy's physically based atmosphere, or black with stars), and
//! exposure.

use std::f32::consts::PI;

use bevy::{
    asset::RenderAssetUsages,
    camera::Exposure,
    light::{
        Atmosphere, DirectionalLightShadowMap, Skybox, SunDisk,
        atmosphere::ScatteringMedium, light_consts::lux,
    },
    prelude::*,
    render::render_resource::{
        Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
};

use crate::world::{Air, SunPath, WorldDef};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DirectionalLightShadowMap>()
            .insert_resource(ClearColor(Color::BLACK))
            .add_systems(Startup, setup)
            .add_systems(Update, (control_sun, place_sun, toggle_air).chain());
    }
}

/// Where the sun is: the clock for a daily path, the elevation for a fixed
/// one. `[` and `]` change whichever it is.
#[derive(Resource)]
pub struct Sun {
    /// Local solar time, 0–24.
    pub hours: f32,
    /// Degrees above the horizon (for a fixed sun, the setting; for a daily
    /// one, the result).
    pub elevation: f32,
    /// Degrees from north through east.
    pub azimuth: f32,
}

impl Sun {
    /// Unit vector from the ground toward the sun.
    pub fn direction(&self) -> Vec3 {
        let (el, az) = (self.elevation.to_radians(), self.azimuth.to_radians());
        let north = el.cos() * az.cos();
        let east = el.cos() * az.sin();
        Vec3::new(east, el.sin(), -north)
    }
}

#[derive(Resource)]
struct AirMedium(Handle<ScatteringMedium>);

fn setup(
    mut commands: Commands,
    world: Res<WorldDef>,
    mut media: ResMut<Assets<ScatteringMedium>>,
    mut images: ResMut<Assets<Image>>,
) {
    let sky = &world.sky;
    let start_hour = crate::tools::env_f32("TRANQUILITY_HOUR").unwrap_or(sky.start_hour);
    let (elevation, azimuth) = match sky.sun {
        SunPath::Daily { latitude, declination } => daily(latitude, declination, start_hour),
        SunPath::Fixed { azimuth, elevation } => (
            crate::tools::env_f32("TRANQUILITY_SUN").unwrap_or(elevation),
            azimuth,
        ),
    };
    commands.insert_resource(Sun {
        hours: start_hour,
        elevation,
        azimuth,
    });
    commands.insert_resource(GlobalAmbientLight {
        brightness: sky.ambient,
        ..default()
    });

    let air = media.add(ScatteringMedium::earth(256, 256));
    if sky.air == Air::Earth {
        commands.spawn(Atmosphere::earth(air.clone()));
    }
    commands.insert_resource(AirMedium(air));

    if sky.stars > 0.0 {
        commands.insert_resource(Stars(images.add(star_field(1024, 7))));
    }

    commands.spawn((
        DirectionalLight {
            // Unscattered sunlight: the atmosphere, where there is one, does
            // the dimming and reddening.
            illuminance: lux::RAW_SUNLIGHT,
            shadow_maps_enabled: true,
            ..default()
        },
        // Shadow cascades are set by the quality tier.
        SunDisk::EARTH,
    ));
}

/// The star field, handed to the camera as a skybox by the player module.
#[derive(Resource)]
pub struct Stars(pub Handle<Image>);

/// Elevation and azimuth of the sun, degrees, at a latitude and solar
/// declination at local solar time `hours`.
pub fn daily(latitude: f32, declination: f32, hours: f32) -> (f32, f32) {
    let (phi, delta) = (latitude.to_radians(), declination.to_radians());
    let h = (hours - 12.0) / 12.0 * PI;
    let elevation = (phi.sin() * delta.sin() + phi.cos() * delta.cos() * h.cos()).asin();
    // Azimuth from north through east.
    let azimuth = h.sin().atan2(h.cos() * phi.sin() - delta.tan() * phi.cos()) + PI;
    (elevation.to_degrees(), azimuth.to_degrees())
}

fn control_sun(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    world: Res<WorldDef>,
    mut sun: ResMut<Sun>,
) {
    let dir = keys.pressed(KeyCode::BracketRight) as i32 - keys.pressed(KeyCode::BracketLeft) as i32;
    let dt = time.delta_secs();
    match world.sky.sun {
        SunPath::Daily { latitude, declination } => {
            sun.hours = (sun.hours + dir as f32 * 2.0 * dt).rem_euclid(24.0);
            (sun.elevation, sun.azimuth) = daily(latitude, declination, sun.hours);
        }
        SunPath::Fixed { .. } => {
            sun.elevation = (sun.elevation + dir as f32 * 10.0 * dt).clamp(-10.0, 90.0);
        }
    }
}

fn place_sun(
    sun: Res<Sun>,
    world: Res<WorldDef>,
    atmospheres: Query<(), With<Atmosphere>>,
    mut light: Single<&mut Transform, With<DirectionalLight>>,
    mut exposure: Query<&mut Exposure>,
) {
    **light = Transform::from_translation(sun.direction()).looking_at(Vec3::ZERO, Vec3::Y);

    // A camera operator's rule of thumb instead of metering: full daylight
    // exposure while the sun is well up; under air, opening up about six
    // stops as it sinks below the horizon. Without air the light doesn't fade
    // until the sun is gone.
    let base = world.sky.exposure;
    let ev = if atmospheres.is_empty() {
        base
    } else {
        base - 6.0 * ((8.0 - sun.elevation) / 14.0).clamp(0.0, 1.0)
    };
    for mut e in &mut exposure {
        e.ev100 = ev;
    }
}

/// `B` takes the air away and gives it back.
fn toggle_air(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    air: Res<AirMedium>,
    atmospheres: Query<Entity, With<Atmosphere>>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    if atmospheres.is_empty() {
        commands.spawn(Atmosphere::earth(air.0.clone()));
    } else {
        for entity in &atmospheres {
            commands.entity(entity).despawn();
        }
    }
}

/// A cube map of stars: magnitudes from a power law, so a few bright ones
/// and many faint, scattered uniformly per face (the slight crowding toward
/// face corners doesn't show).
fn star_field(size: u32, seed: u32) -> Image {
    let face = (size * size) as usize;
    let mut data = vec![0u8; face * 6 * 4];
    for px in data.chunks_exact_mut(4) {
        px[3] = 255;
    }
    let mut state = seed.wrapping_mul(747796405).wrapping_add(2891336453);
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state as f32) / (u32::MAX as f32)
    };
    for f in 0..6 {
        for _ in 0..1600 {
            let x = (next() * size as f32) as u32 % size;
            let y = (next() * size as f32) as u32 % size;
            let b = next().powf(6.0);
            let warm = next();
            let rgb = [
                (0.75 + 0.25 * warm) * b,
                0.85 * b + 0.1 * b * warm,
                (1.0 - 0.3 * warm) * b,
            ];
            let i = (f as usize * face + (y * size + x) as usize) * 4;
            for c in 0..3 {
                data[i + c] = (rgb[c] * 255.0).min(255.0) as u8;
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image
}

/// Attaches the star field to new cameras.
pub(crate) fn skybox(world: &WorldDef, stars: Option<&Stars>) -> Option<Skybox> {
    stars.map(|stars| Skybox {
        image: Some(stars.0.clone()),
        brightness: world.sky.stars,
        ..default()
    })
}
