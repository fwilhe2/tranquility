//! First-person camera: walk (with gravity, jumps and collision) or fly.

use bevy::{
    camera::Exposure,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    light::AtmosphereEnvironmentMapLight,
    pbr::AtmosphereSettings,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{
    sky::{self, Stars},
    world::{Ground, WorldDef},
};

const LOOK_SENSITIVITY: f32 = 0.0022;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // After the sky, which makes the star field the camera may carry.
        app.add_systems(PostStartup, spawn)
            .add_systems(Update, (grab_cursor, look, (toggle_fly, walk).chain()));
    }
}

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub flying: bool,
    /// Vertical speed while walking, m/s.
    fall: f32,
    grounded: bool,
    /// Height of the feet; the eye follows it, eased if the world asks.
    feet: f32,
}

fn spawn(mut commands: Commands, world: Res<WorldDef>, stars: Option<Res<Stars>>) {
    let spawn = world.spawn;
    let floor = feet_floor(&world, spawn.x, spawn.z);
    let mut position = Vec3::new(spawn.x, floor + world.walker.eye_height, spawn.z);
    let mut player = Player {
        yaw: spawn.yaw,
        pitch: spawn.pitch,
        flying: false,
        fall: 0.0,
        grounded: true,
        feet: floor,
    };
    // TRANQUILITY_POSE="x,y,z,yaw,pitch" starts there, flying.
    if let Some([x, y, z, yaw, pitch]) = crate::tools::env_floats::<5>("TRANQUILITY_POSE") {
        position = Vec3::new(x, y, z);
        (player.yaw, player.pitch, player.flying) = (yaw, pitch, true);
    }
    let rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);

    let mut camera = commands.spawn((
        player,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 70f32.to_radians(),
            near: 0.1,
            far: 40_000.0,
            ..default()
        }),
        Transform::from_translation(position).with_rotation(rotation),
        // Set from the sun's height every frame, see `sky::place_sun`.
        Exposure {
            ev100: world.sky.exposure,
        },
        Tonemapping::AcesFitted,
        // Antialiasing, SSAO and bloom are set by the quality tier.
        Msaa::Off,
        AtmosphereSettings::default(),
        AtmosphereEnvironmentMapLight::default(),
    ));
    if let Some(skybox) = sky::skybox(&world, stars.as_deref()) {
        camera.insert(skybox);
    }
}

/// Height the feet rest at: the ground, or the water's surface where the
/// world lets you walk on it.
fn feet_floor(world: &WorldDef, x: f32, z: f32) -> f32 {
    let ground = world.ground.stand_height(x, z);
    match &world.water {
        Some(water) if world.walker.walk_on_water => ground.max(water.level),
        _ => ground,
    }
}

/// Click to capture the mouse, Escape to let it go.
fn grab_cursor(
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        cursor.visible = false;
        cursor.grab_mode = CursorGrabMode::Locked;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
}

fn look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    player: Single<(&mut Player, &mut Transform)>,
) {
    if cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    let (mut player, mut transform) = player.into_inner();
    player.yaw -= motion.delta.x * LOOK_SENSITIVITY;
    player.pitch = (player.pitch - motion.delta.y * LOOK_SENSITIVITY).clamp(-1.54, 1.54);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
}

fn toggle_fly(keys: Res<ButtonInput<KeyCode>>, mut player: Single<&mut Player>) {
    if keys.just_pressed(KeyCode::KeyF) {
        player.flying = !player.flying;
        player.fall = 0.0;
    }
}

fn walk(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    world: Res<WorldDef>,
    player: Single<(&mut Player, &mut Transform)>,
) {
    let (mut player, mut transform) = player.into_inner();
    let w = &world.walker;
    let dt = time.delta_secs().min(0.1);
    let (forward, right) = if player.flying {
        (*transform.forward(), *transform.right())
    } else {
        let flat = Quat::from_rotation_y(player.yaw);
        (flat * Vec3::NEG_Z, flat * Vec3::X)
    };

    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let mut wish = forward * axis(KeyCode::KeyW, KeyCode::KeyS) + right * axis(KeyCode::KeyD, KeyCode::KeyA);
    if player.flying {
        wish += Vec3::Y * axis(KeyCode::Space, KeyCode::KeyC);
    }
    let fast = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let speed = match (player.flying, fast) {
        (false, false) => w.walk,
        (false, true) => w.run,
        (true, false) => w.fly,
        (true, true) => w.fly_fast,
    };
    let step = wish.normalize_or_zero() * speed * dt;

    if player.flying {
        transform.translation += step;
        let floor = feet_floor(&world, transform.translation.x, transform.translation.z) + w.eye_height;
        transform.translation.y = transform.translation.y.max(floor);
        player.feet = transform.translation.y - w.eye_height;
        return;
    }

    // Walking: slide along whatever blocks the way, one axis at a time.
    let ground = &*world.ground;
    let p = transform.translation;
    let mut next = Vec2::new(p.x, p.z);
    if !blocked(ground, next + Vec2::new(step.x, 0.0), w.radius) {
        next.x += step.x;
    }
    if !blocked(ground, next + Vec2::new(0.0, step.z), w.radius) {
        next.y += step.z;
    }

    // Vertical: stick to the ground going down slopes and steps, fall off
    // anything higher than a step, jump from the ground.
    let floor = feet_floor(&world, next.x, next.y);
    let mut feet = player.feet;
    if player.grounded && keys.just_pressed(KeyCode::Space) {
        player.fall = w.jump;
        player.grounded = false;
    }
    if player.grounded && feet - floor < 0.6 {
        feet = floor;
    } else {
        player.fall -= w.gravity * dt;
        feet += player.fall * dt;
        player.grounded = false;
    }
    if feet <= floor {
        feet = floor;
        player.fall = 0.0;
        player.grounded = true;
    }
    player.feet = feet;

    // The eye eases after the feet instead of copying every bump under them:
    // a head on a body, not a wheel on the ground. Never far behind, so it
    // cannot sink into a slope.
    let target = feet + w.eye_height;
    let eye = if w.eye_smoothing > 0.0 {
        let eased = p.y + (target - p.y) * (1.0 - (-dt / w.eye_smoothing).exp());
        eased.clamp(target - 0.5 * w.eye_height, target + 0.5 * w.eye_height)
    } else {
        target
    };
    transform.translation = Vec3::new(next.x, eye, next.y);
}

fn blocked(ground: &dyn Ground, p: Vec2, radius: f32) -> bool {
    [Vec2::ZERO, Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
        .iter()
        .any(|d| {
            let q = p + *d * radius;
            ground.blocked(q.x, q.y)
        })
}
