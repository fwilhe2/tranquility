//! First-person camera: walk (with gravity, jumps and collision) or fly.
//!
//! Walking is a velocity and the forces on it. The legs push the walker
//! toward the pace asked for, but no harder than the ground's grip allows
//! ([`WalkerSettings::grip`]); gravity pulls them down a slope whatever the
//! legs do; and off the ground they are a projectile. With the defaults
//! (endless grip, steering in the air, a step's worth of sticking to the
//! ground) that comes out as the usual game walk; with a world's real
//! numbers it comes out as that world.

use bevy::{
    camera::Exposure,
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::AccumulatedMouseMotion,
    light::AtmosphereEnvironmentMapLight,
    math::DVec2,
    pbr::AtmosphereSettings,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{
    origin::Origin,
    world::{Ground, WorldDef},
};

const LOOK_SENSITIVITY: f32 = 0.0022;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostStartup, spawn).add_systems(
            Update,
            (
                respawn.run_if(crate::world_replaced),
                grab_cursor,
                look,
                (toggle_fly, walk).chain(),
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub flying: bool,
    /// False while something else has the camera (a vehicle's seat): the
    /// engine then neither moves nor turns it.
    pub controlled: bool,
    /// m/s, while walking.
    pub velocity: Vec3,
    pub grounded: bool,
    /// Height of the feet; the eye follows it, eased if the world asks.
    pub feet: f32,
}

impl Player {
    /// Puts the walker's feet on the ground at a render-space point,
    /// standing still.
    pub fn stand_at(&mut self, transform: &mut Transform, world: &WorldDef, origin: &Origin, at: Vec3) {
        let floor = feet_floor(world, origin.world(at));
        self.feet = floor;
        self.velocity = Vec3::ZERO;
        self.grounded = true;
        self.flying = false;
        transform.translation = Vec3::new(at.x, floor + world.walker.eye_height, at.z);
    }
}

fn start(world: &WorldDef, origin: &Origin) -> (Player, Transform) {
    let spawn = world.spawn;
    // Wherever that is: if it is far from the renderer's zero, the origin
    // jumps there on the next frame, taking everything drawn with it.
    let at = DVec2::new(spawn.x, spawn.z);
    let floor = feet_floor(world, at);
    let mut position = origin.render(at, floor + world.walker.eye_height);
    let mut player = Player {
        yaw: spawn.yaw,
        pitch: spawn.pitch,
        flying: false,
        controlled: true,
        velocity: Vec3::ZERO,
        grounded: true,
        feet: floor,
    };
    // TRANQUILITY_POSE="x,y,z,yaw,pitch" starts there, flying.
    if let Some([x, y, z, yaw, pitch]) = crate::tools::env_floats::<5>("TRANQUILITY_POSE") {
        position = origin.render(DVec2::new(x as f64, z as f64), y);
        (player.yaw, player.pitch, player.flying) = (yaw, pitch, true);
    }
    let rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    (player, Transform::from_translation(position).with_rotation(rotation))
}

fn spawn(mut commands: Commands, world: Res<WorldDef>, origin: Res<Origin>) {
    let (player, transform) = start(&world, &origin);
    commands.spawn((
        player,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 70f32.to_radians(),
            near: 0.1,
            far: (world.terrain.reach() * 1.6).max(40_000.0),
            ..default()
        }),
        transform,
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
}

/// Another world: start again at its starting point.
fn respawn(
    world: Res<WorldDef>,
    origin: Res<Origin>,
    player: Single<(&mut Player, &mut Transform, &mut Projection)>,
) {
    let (mut player, mut transform, mut projection) = player.into_inner();
    (*player, *transform) = start(&world, &origin);
    if let Projection::Perspective(p) = &mut *projection {
        p.far = (world.terrain.reach() * 1.6).max(40_000.0);
    }
}

/// Height the feet rest at: the ground, or the water's surface where the
/// world lets you walk on it.
fn feet_floor(world: &WorldDef, at: DVec2) -> f32 {
    let ground = world.ground.stand_height(at.x, at.y);
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
    if player.controlled {
        transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    }
}

fn toggle_fly(keys: Res<ButtonInput<KeyCode>>, mut player: Single<&mut Player>) {
    if player.controlled && keys.just_pressed(KeyCode::KeyF) {
        player.flying = !player.flying;
        player.velocity = Vec3::ZERO;
        player.grounded = false;
    }
}

fn walk(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    player: Single<(&mut Player, &mut Transform)>,
) {
    let (mut player, mut transform) = player.into_inner();
    if !player.controlled {
        return;
    }
    let w = &world.walker;
    let dt = time.delta_secs().min(0.1);
    if dt <= 0.0 {
        return;
    }
    let (forward, right) = if player.flying {
        (*transform.forward(), *transform.right())
    } else {
        let flat = Quat::from_rotation_y(player.yaw);
        (flat * Vec3::NEG_Z, flat * Vec3::X)
    };

    let axis = |pos: KeyCode, neg: KeyCode| keys.pressed(pos) as i32 as f32 - keys.pressed(neg) as i32 as f32;
    let mut wish = forward * axis(KeyCode::KeyW, KeyCode::KeyS) + right * axis(KeyCode::KeyD, KeyCode::KeyA);
    // TRANQUILITY_WALK=back (or forward) walks by itself, for screenshots
    // of what walking leaves behind.
    match std::env::var("TRANQUILITY_WALK").as_deref() {
        Ok("back") => wish -= forward,
        Ok(_) => wish += forward,
        Err(_) => {}
    }
    let fast = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let floor_at = |p: Vec3| feet_floor(&world, origin.world(p));

    if player.flying {
        wish += Vec3::Y * axis(KeyCode::Space, KeyCode::KeyC);
        let speed = if fast { w.fly_fast } else { w.fly };
        transform.translation += wish.normalize_or_zero() * speed * dt;
        let floor = floor_at(transform.translation) + w.eye_height;
        transform.translation.y = transform.translation.y.max(floor);
        player.feet = transform.translation.y - w.eye_height;
        return;
    }

    let ground = &*world.ground;
    let p = transform.translation;
    let here = Vec2::new(p.x, p.z);
    let floor_here = floor_at(p);
    let pace = if fast { w.run } else { w.walk };
    let target = wish.normalize_or_zero() * pace;
    let mut v = player.velocity;
    let mut flat = Vec2::new(v.x, v.z);

    if player.grounded {
        // The slope under the feet, and the pull of gravity along it.
        let e = 0.4;
        let gx = (floor_at(p + Vec3::X * e) - floor_at(p - Vec3::X * e)) / (2.0 * e);
        let gz = (floor_at(p + Vec3::Z * e) - floor_at(p - Vec3::Z * e)) / (2.0 * e);
        let grade = Vec2::new(gx, gz);
        let steep = 1.0 + grade.length_squared();
        let downhill = -grade * w.gravity / steep;
        if w.grip.is_finite() {
            // The legs cancel the slope's pull and change the pace as far
            // as friction lets them; what is left over is sliding.
            let want = (Vec2::new(target.x, target.z) - flat) / dt - downhill;
            let legs = want.clamp_length_max(w.grip / steep.sqrt());
            flat += (legs + downhill) * dt;
        } else {
            flat = Vec2::new(target.x, target.z);
        }
    } else {
        if w.air_control {
            flat = Vec2::new(target.x, target.z);
        }
        if w.jets > 0.0 {
            // Suit jets: the keys are thrust, not a pace.
            let thrust = wish + Vec3::Y * axis(KeyCode::Space, KeyCode::KeyC);
            let a = thrust.normalize_or_zero() * w.jets * if fast { 3.0 } else { 1.0 };
            flat += Vec2::new(a.x, a.z) * dt;
            v.y += a.y * dt;
        }
    }
    if w.drag > 0.0 {
        let all = Vec3::new(flat.x, v.y, flat.y);
        let slowed = all / (1.0 + w.drag * all.length() * dt);
        (flat, v.y) = (Vec2::new(slowed.x, slowed.z), slowed.y);
    }

    // Slide along whatever blocks the way, one axis at a time.
    let step = flat * dt;
    let mut next = here;
    if blocked(ground, &origin, next + Vec2::new(step.x, 0.0), w.radius) {
        flat.x = 0.0;
    } else {
        next.x += step.x;
    }
    if blocked(ground, &origin, next + Vec2::new(0.0, step.y), w.radius) {
        flat.y = 0.0;
    } else {
        next.y += step.y;
    }

    // Vertical: on the ground the feet follow it, and leave it when it
    // falls away faster than a falling body would (or further than a
    // step); in the air they fall.
    let floor = floor_at(Vec3::new(next.x, 0.0, next.y));
    let mut feet = player.feet;
    let old_target = feet + w.eye_height;
    if player.grounded {
        if keys.just_pressed(KeyCode::Space) {
            v.y = w.jump + ((floor - floor_here) / dt).max(0.0);
            player.grounded = false;
        } else {
            let fallen = feet - 0.5 * w.gravity * dt * dt;
            if floor >= fallen - w.step_down {
                // Still standing; remember how fast the ground is rising
                // or falling under the feet, for the moment it lets go.
                v.y = ((floor - feet) / dt).clamp(-3.0 * pace.max(1.0), 3.0 * pace.max(1.0));
                feet = floor;
            } else {
                player.grounded = false;
                // Leaving a crest: no faster upward than the walk was
                // carrying them.
                v.y = v.y.min(flat.length());
            }
        }
    }
    if !player.grounded {
        v.y -= w.gravity * dt;
        feet += v.y * dt;
        if feet <= floor {
            feet = floor;
            v.y = 0.0;
            player.grounded = true;
        }
    }
    player.feet = feet;
    player.velocity = Vec3::new(flat.x, v.y, flat.y);

    // The eye eases after the feet instead of copying every bump under them:
    // a head on a body, not a wheel on the ground. Never far behind, so it
    // cannot sink into a slope.
    let target = feet + w.eye_height;
    let eye = if w.eye_smoothing > 0.0 && player.grounded {
        let eased = p.y + (target - p.y) * (1.0 - (-dt / w.eye_smoothing).exp());
        eased.clamp(target - 0.5 * w.eye_height, target + 0.5 * w.eye_height)
    } else if w.eye_smoothing > 0.0 {
        // In flight the eye rides with the body, keeping whatever it was
        // behind by at take-off and making that up gently.
        target + (p.y - old_target).clamp(-0.5, 0.5) * (-dt / w.eye_smoothing).exp()
    } else {
        target
    };
    transform.translation = Vec3::new(next.x, eye, next.y);
}

fn blocked(ground: &dyn Ground, origin: &Origin, p: Vec2, radius: f32) -> bool {
    [Vec2::ZERO, Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y]
        .iter()
        .any(|d| {
            let q = origin.0 + (p + *d * radius).as_dvec2();
            ground.blocked(q.x, q.y)
        })
}
