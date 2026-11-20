//! What hangs in the sky: the Sun's disc, and the body's company — the
//! Earth from the Moon, Jupiter from its moons, Mars filling a quarter of
//! the sky from Phobos. All are lit by the same Sun as the ground, so each
//! shows the phase the geometry gives. They ride along with the camera, far
//! beyond the terrain: they are directions, not places, given as they stand
//! over the world's zero and turned as the sky turns for a traveller round
//! the body (`tranquility::sky::turn`).
//!
//! Angular sizes are the true ones, from the bodies' radii and distances.

use bevy::{
    asset::RenderAssetUsages,
    light::{NotShadowCaster, NotShadowReceiver},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, Task, block_on},
};
use tranquility::{
    noise::{fbm3, smoothstep},
    prelude::*,
    sky,
    terrain::Chunks,
};

use crate::Current;

/// Inside the camera's far plane, outside everything else: at least this,
/// and further when the ground reaches further (seen from orbit).
const DISTANCE: f32 = 37_000.0;
/// A disc smaller than this (degrees) would flicker between pixels: it is
/// drawn this size and dimmed to the same total light.
pub const SMALLEST: f32 = 0.11;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Map {
    Earth,
    Mars,
    Jupiter,
    Saturn,
    Charon,
    /// A small grey moon, too far for any face.
    Rock,
}

#[derive(Clone, Copy, Debug)]
pub struct Companion {
    pub map: Map,
    /// Angular radius, degrees.
    pub radius: f32,
    /// Degrees from north through east, and above the horizon.
    pub azimuth: f32,
    pub elevation: f32,
    /// Which way its pole leans and which face it shows: ZYX angles.
    pub turn: [f32; 3],
    /// Rings, from and to, in the planet's radii.
    pub rings: Option<(f32, f32)>,
    /// What it is called, and a sentence about it: shown while you look
    /// at it.
    pub name: &'static str,
    pub about: &'static str,
}

impl Companion {
    pub fn new(map: Map, radius: f32, azimuth: f32, elevation: f32) -> Self {
        let name = match map {
            Map::Earth => "Earth",
            Map::Mars => "Mars",
            Map::Jupiter => "Jupiter",
            Map::Saturn => "Saturn",
            Map::Charon => "Charon",
            Map::Rock => "a small moon",
        };
        Self { map, radius, azimuth, elevation, turn: [0.35, 2.1, 0.2], rings: None, name, about: "" }
    }

    pub fn direction(&self) -> Vec3 {
        let (el, az) = (self.elevation.to_radians(), self.azimuth.to_radians());
        Vec3::new(el.cos() * az.sin(), el.sin(), -el.cos() * az.cos())
    }
}

pub struct HeavensPlugin;

impl Plugin for HeavensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Maps>()
            .add_systems(PreStartup, start_painting)
            .add_systems(Startup, setup)
            .add_systems(Update, populate.run_if(resource_changed::<Current>))
            .add_systems(PostUpdate, follow.before(TransformSystems::Propagate));
    }
}

/// The Sun's disc, and its drawn angular radius, degrees.
#[derive(Component)]
struct SunDisc(f32);

/// A companion: the direction it hangs in over the world's zero, its drawn
/// angular radius (degrees), and how it is turned there.
#[derive(Component)]
struct Hanging {
    direction: Vec3,
    radius: f32,
    rotation: Quat,
}

/// Planet maps are painted once, all of them on the compute pool from the
/// start: Jupiter's takes seconds, and the first visit must not wait.
#[derive(Resource, Default)]
struct Maps {
    painted: HashMap<Map, Handle<Image>>,
    painting: HashMap<Map, Task<Image>>,
}

const PAINTED: [Map; 5] = [Map::Earth, Map::Mars, Map::Jupiter, Map::Saturn, Map::Charon];

fn start_painting(mut maps: ResMut<Maps>) {
    let pool = AsyncComputeTaskPool::get();
    for map in PAINTED {
        maps.painting.insert(map, pool.spawn(async move { paint(map) }));
    }
}

/// The map, waiting for it if it is not done yet.
fn map_image(maps: &mut Maps, images: &mut Assets<Image>, map: Map) -> Handle<Image> {
    if let Some(handle) = maps.painted.get(&map) {
        return handle.clone();
    }
    let image = match maps.painting.remove(&map) {
        Some(task) => block_on(task),
        None => paint(map),
    };
    let handle = images.add(image);
    maps.painted.insert(map, handle.clone());
    handle
}

#[derive(Resource)]
struct Globe {
    fine: Handle<Mesh>,
    coarse: Handle<Mesh>,
    sun: Handle<StandardMaterial>,
    /// What keeps the fill light off the companions (see `populate`).
    dark: Handle<Image>,
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let globe = Globe {
        // Bevy's UV sphere has its poles on z; the maps want them on y.
        fine: meshes.add(Sphere::new(1.0).mesh().uv(128, 64).rotated_by(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))),
        coarse: meshes.add(Sphere::new(1.0).mesh().ico(3).expect("valid subdivision")),
        sun: materials.add(StandardMaterial { base_color: Color::BLACK, ..default() }),
        dark: images.add(Image::new(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![0, 0, 0, 255],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        )),
    };
    commands.spawn((
        SunDisc(1.0),
        Mesh3d(globe.coarse.clone()),
        MeshMaterial3d(globe.sun.clone()),
        NotShadowCaster,
        NotShadowReceiver,
    ));
    commands.insert_resource(globe);
}

fn size(degrees: f32, distance: f32) -> Vec3 {
    Vec3::splat(distance * degrees.to_radians().tan())
}

/// Hangs the current body's sky: when the app starts and on every change
/// of body.
fn populate(
    mut commands: Commands,
    current: Res<Current>,
    globe: Res<Globe>,
    mut maps: ResMut<Maps>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<Hanging>>,
    mut disc: Single<(&mut SunDisc, &mut Visibility)>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let body = &current.0;

    // The Sun's surface is as bright from Pluto as from Mercury, only
    // smaller: far beyond white at any exposure, and the bloom makes the
    // glare. Under air the sky draws its own disc, dimmed by the air.
    let true_radius = body.sun_radius();
    let drawn = true_radius.max(SMALLEST);
    let dim = (true_radius / drawn).powi(2);
    if let Some(mut material) = materials.get_mut(&globe.sun) {
        material.emissive = LinearRgba::rgb(4.0e6, 3.8e6, 3.5e6) * dim;
    }
    disc.0.0 = drawn;
    *disc.1 = if body.sun_disc && !body.sky_draws_sun() { Visibility::Inherited } else { Visibility::Hidden };

    for c in &body.companions {
        let mut rotation = Quat::from_euler(EulerRot::ZYX, c.turn[0], c.turn[1], c.turn[2]);
        if c.rings.is_some() {
            // A moon in the ring plane sees the rings edge-on: the pole
            // stands square to the line of sight, a hair off so the rings
            // keep a pixel's width.
            let dir = c.direction();
            let across = Vec3::Y.cross(dir).normalize_or(Vec3::X);
            let pole = Quat::from_axis_angle(across, 0.3f32.to_radians()) * dir.cross(across);
            rotation = Quat::from_rotation_arc(Vec3::Y, pole.normalize()) * Quat::from_rotation_y(c.turn[1]);
        }
        let drawn = c.radius.max(SMALLEST);
        let dim = (c.radius / drawn).powi(2);
        // The fill light stands for what the ground round you throws into
        // its own shadows; none of it reaches a planet's night side. An
        // occlusion map of black takes it off (occlusion only dims the
        // ambient light).
        let shut = Some(globe.dark.clone());
        let material = match c.map {
            Map::Rock => StandardMaterial {
                base_color: Color::linear_rgb(0.09 * dim, 0.08 * dim, 0.07 * dim),
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                occlusion_texture: shut,
                ..default()
            },
            map => {
                let image = map_image(&mut maps, &mut images, map);
                StandardMaterial {
                    base_color: Color::linear_rgb(dim, dim, dim),
                    base_color_texture: Some(image),
                    perceptual_roughness: 0.9,
                    reflectance: 0.15,
                    occlusion_texture: shut,
                    ..default()
                }
            }
        };
        let mut globe_entity = commands.spawn((
            Hanging { direction: c.direction(), radius: drawn, rotation },
            Mesh3d(if c.radius > 0.4 { globe.fine.clone() } else { globe.coarse.clone() }),
            MeshMaterial3d(materials.add(material)),
            Transform::from_scale(size(drawn, DISTANCE)).with_rotation(rotation),
            NotShadowCaster,
            NotShadowReceiver,
        ));
        if let Some((inner, outer)) = c.rings {
            let rings = meshes.add(annulus(inner, outer));
            let texture = images.add(ring_profile());
            globe_entity.with_child((
                Mesh3d(rings),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(texture),
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    double_sided: true,
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    // Ice grains scatter the light on through the rings:
                    // they are lit from either side.
                    diffuse_transmission: 0.6,
                    ..default()
                })),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }
}

fn follow(
    sun: Res<Sun>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    chunks: Res<Chunks>,
    camera: Single<&Transform, (With<Camera3d>, Without<SunDisc>, Without<Hanging>)>,
    disc: Single<(&mut Transform, &SunDisc), Without<Hanging>>,
    mut hanging: Query<(&mut Transform, &Hanging), Without<SunDisc>>,
) {
    let distance = (1.25 * world.terrain.reach_to(chunks.span().1)).max(DISTANCE);
    let turned = sky::turn(world.terrain.radius, origin.world(camera.translation));
    let (mut transform, disc) = disc.into_inner();
    transform.translation = camera.translation + turned * sun.direction() * distance;
    transform.scale = size(disc.0, distance);
    for (mut transform, hanging) in &mut hanging {
        transform.translation = camera.translation + turned * hanging.direction * distance;
        transform.rotation = turned * hanging.rotation;
        transform.scale = size(hanging.radius, distance);
    }
}

/// A flat ring in the equatorial plane, radii in units of the globe's; the
/// texture runs across it from the inner edge to the outer.
fn annulus(inner: f32, outer: f32) -> Mesh {
    let n = 160;
    let (mut positions, mut normals, mut uvs, mut indices) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for k in 0..=n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        for (r, u) in [(inner, 0.0), (outer, 1.0)] {
            positions.push([r * a.cos(), 0.0, r * a.sin()]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([u, 0.5]);
        }
        if k < n {
            let i = 2 * k as u32;
            indices.extend_from_slice(&[i, i + 1, i + 2, i + 2, i + 1, i + 3]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

/// Saturn's rings from 1.24 to 2.27 radii: the dusky C ring, the bright
/// opaque B ring, the Cassini division, the A ring with the Encke gap.
fn ring_profile() -> Image {
    let width = 512;
    let mut data = Vec::with_capacity(width * 4);
    for i in 0..width {
        let r = 1.24 + (2.27 - 1.24) * (i as f32 + 0.5) / width as f32;
        let fine = 0.85 + 0.15 * (r * 310.0).sin() * (r * 97.0).cos();
        let (tone, opacity) = if r < 1.53 {
            (0.45, 0.12 + 0.1 * (r - 1.24) / 0.29)
        } else if r < 1.95 {
            (0.9, 0.85 + 0.12 * ((r - 1.53) / 0.42))
        } else if r < 2.03 {
            (0.5, 0.08)
        } else if (r - 2.214).abs() < 0.004 {
            (0.5, 0.02)
        } else {
            (0.8, 0.55)
        };
        let c = [0.86 * tone * fine, 0.78 * tone * fine, 0.62 * tone * fine];
        data.extend(c.map(|v: f32| (v.powf(1.0 / 2.2) * 255.0).min(255.0) as u8));
        data.push((opacity * fine * 255.0) as u8);
    }
    Image::new(
        Extent3d { width: width as u32, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

type Rgb = [f32; 3];

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A globe as an equirectangular map of linear albedo, painted from noise
/// on the sphere itself (so there is no seam and the poles do not pinch).
/// Not the real coastlines or the real storms: the same kind of world.
fn paint(map: Map) -> Image {
    let (width, height) = match map {
        Map::Mars | Map::Jupiter | Map::Saturn => (2048u32, 1024u32),
        _ => (1024, 512),
    };
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for j in 0..height {
        let lat = (0.5 - (j as f32 + 0.5) / height as f32) * std::f32::consts::PI;
        for i in 0..width {
            let lon = (i as f32 + 0.5) / width as f32 * std::f32::consts::TAU;
            let p = [(lat.cos() * lon.cos()) as f64, lat.sin() as f64, (lat.cos() * lon.sin()) as f64];
            let c = match map {
                Map::Earth => earth(p, lat),
                Map::Mars => mars(p, lat),
                Map::Jupiter => jupiter(p, lat),
                Map::Saturn => saturn(p, lat),
                Map::Charon => charon(p, lat),
                Map::Rock => [0.08; 3],
            };
            data.extend(c.map(|v| (v.max(0.0).powf(1.0 / 2.2) * 255.0).min(255.0) as u8));
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

/// Oceans, continents green to tan by latitude and dryness, polar ice, and
/// weather over all of it.
fn earth(p: [f64; 3], lat: f32) -> Rgb {
    let h = fbm3(p, 1.3, 6, 0.5, 21);
    let d = fbm3(p, 2.2, 4, 0.5, 22);
    let polar = lat.abs() / std::f32::consts::FRAC_PI_2;
    // The sea darkest, deeper further from shore.
    let sea = mix([0.03, 0.09, 0.2], [0.012, 0.035, 0.11], -h * 4.0);
    let green = mix([0.05, 0.11, 0.035], [0.1, 0.13, 0.05], d + 0.5);
    let tan = [0.36, 0.27, 0.15];
    // Deserts sit in the horse latitudes, forests at the equator and
    // further poleward.
    let arid = smoothstep(0.1, 0.5, 1.0 - (polar - 0.3).abs() * 4.0 + d * 1.5);
    let mut c = mix(sea, mix(green, tan, arid), smoothstep(0.0, 0.03, h - 0.04));
    let ice = smoothstep(0.78, 0.9, polar + 0.08 * d + 0.1 * h.max(0.0));
    c = mix(c, [0.8, 0.83, 0.86], ice);
    // Weather: fronts drawn out along the latitudes and stirred.
    let s = fbm3(p, 1.1, 3, 0.5, 24) as f64 * 0.5;
    let q = [p[0] + s, p[1] * 2.2 - s, p[2] + s];
    let cloud = smoothstep(-0.02, 0.45, fbm3(q, 2.6, 6, 0.5, 23) + 0.12 * polar);
    c = mix(c, [0.82, 0.83, 0.85], cloud * 0.95);
    // Air: a thin blue veil.
    mix(c, [0.25, 0.4, 0.7], 0.12)
}

/// Bright dusty highlands, dark basalt regions scoured clean by wind, the
/// polar caps, and a little dust haze over all.
fn mars(p: [f64; 3], lat: f32) -> Rgb {
    let broad = fbm3(p, 1.1, 5, 0.55, 31);
    let fine = fbm3(p, 6.0, 5, 0.55, 32);
    let polar = lat.abs() / std::f32::consts::FRAC_PI_2;
    let bright = mix([0.42, 0.21, 0.09], [0.55, 0.33, 0.17], fine * 1.5 + 0.5);
    let dark = mix([0.11, 0.065, 0.045], [0.2, 0.11, 0.06], fine + 0.5);
    // The dark markings keep to the low and middle latitudes.
    let basalt = smoothstep(0.02, 0.2, broad + 0.25 * fine - 0.25 * (polar - 0.25).abs());
    let mut c = mix(bright, dark, basalt);
    // The giant canyon: a dark gash along the equator on one side.
    let lon = (p[2].atan2(p[0])) as f32;
    let rift = (1.0 - ((lat + 0.15 + 0.06 * (lon * 3.0).sin()) / 0.035).abs()).max(0.0) * smoothstep(0.3, 0.8, lon) * (1.0 - smoothstep(1.6, 2.1, lon));
    c = mix(c, [0.12, 0.06, 0.04], rift * 0.8);
    let cap = smoothstep(0.84, 0.9, polar + 0.03 * fine + if lat > 0.0 { 0.0 } else { -0.03 });
    c = mix(c, [0.82, 0.78, 0.74], cap);
    mix(c, [0.5, 0.33, 0.2], 0.1)
}

/// Bands by latitude — pale zones of high ammonia cloud, brown belts where
/// you see deeper — sheared against each other into festoons, with a
/// great red oval in the south.
fn jupiter(p: [f64; 3], lat: f32) -> Rgb {
    let lon = (p[2].atan2(p[0])) as f32;
    // Turbulence stretched along the latitudes.
    let q = [p[0] * 3.0, p[1] * 16.0, p[2] * 3.0];
    let swirl = fbm3(q, 1.0, 5, 0.55, 41);
    let wisps = fbm3([p[0] * 9.0, p[1] * 40.0, p[2] * 9.0], 1.0, 4, 0.6, 42);
    let l = lat + 0.035 * swirl + 0.012 * wisps;
    // The belts: a sum of a few waves in latitude, never quite regular.
    let band = (l * 13.0).sin() * 0.55 + (l * 27.0 + 1.0).sin() * 0.25 + (l * 5.0 + 0.4).sin() * 0.35;
    let zone = [0.74, 0.68, 0.58];
    let belt = [0.44, 0.27, 0.16];
    let mut c = mix(belt, zone, band * 0.9 + 0.5 + 0.25 * wisps);
    // Blue-grey festoons on the edges of the equatorial zone.
    let edge = (1.0 - (band.abs() / 0.15)).max(0.0) * smoothstep(0.1, 0.5, swirl);
    c = mix(c, [0.4, 0.42, 0.46], edge * 0.35);
    // The poles: darker, mottled, bluer.
    let polar = smoothstep(0.95, 1.35, lat.abs() + 0.1 * swirl);
    c = mix(c, [0.33, 0.31, 0.3], polar * 0.8);
    // The Great Red Spot, 22° south.
    let (dx, dy) = (((lon - 1.2 + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI) / 0.2, (lat + 0.38) / 0.095);
    let spot = (1.0 - (dx * dx + dy * dy)).max(0.0);
    c = mix(c, [0.58, 0.24, 0.13], smoothstep(0.0, 0.5, spot) * 0.85);
    c
}

/// Jupiter's bands under a thick butterscotch haze, which mutes them, and
/// the shadow of the rings across the equator.
fn saturn(p: [f64; 3], lat: f32) -> Rgb {
    let swirl = fbm3([p[0] * 3.0, p[1] * 18.0, p[2] * 3.0], 1.0, 4, 0.5, 51);
    let l = lat + 0.02 * swirl;
    let band = (l * 17.0).sin() * 0.5 + (l * 37.0).sin() * 0.2 + (l * 7.0 + 0.3).sin() * 0.3;
    let mut c = mix([0.6, 0.47, 0.3], [0.76, 0.66, 0.46], band * 0.5 + 0.5);
    let polar = smoothstep(1.0, 1.4, lat.abs());
    c = mix(c, [0.42, 0.42, 0.38], polar * 0.6);
    // The rings' shadow: a few dark lines just off the equator.
    let shade = smoothstep(0.015, 0.03, lat) * (1.0 - smoothstep(0.09, 0.11, lat)) * (0.6 + 0.4 * (lat * 400.0).sin());
    mix(c, [0.1, 0.08, 0.05], shade * 0.7)
}

/// Grey water ice, cratered, with the dark red cap at its north pole.
fn charon(p: [f64; 3], lat: f32) -> Rgb {
    let n = fbm3(p, 2.5, 6, 0.55, 61);
    let mut c = mix([0.3, 0.29, 0.28], [0.45, 0.44, 0.43], n + 0.5);
    let cap = smoothstep(0.95, 1.25, lat + 0.15 * n);
    c = mix(c, [0.2, 0.1, 0.07], cap * 0.85);
    // The belt of chasms round the equator.
    let chasm = (1.0 - ((lat - 0.1 + 0.08 * n) / 0.03).abs()).max(0.0);
    mix(c, [0.2, 0.2, 0.2], chasm * 0.5)
}
