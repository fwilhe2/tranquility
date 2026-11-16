//! Wheeled vehicles: a rigid body on sprung wheels, driven over the ground.
//!
//! Nothing here is a speed dial. The body has mass and inertia; each wheel
//! presses on the ground as hard as its spring is squeezed, and can push
//! forward, brake or hold sideways only as hard as friction on that load
//! allows; the motors have a power and a top speed. So a vehicle squats
//! under power, dives under the brake, leans out of a turn, ploughs wide
//! when a turn asks more than the tyres have, slides down slopes too steep
//! for it, and leaves the ground over a crest. What that feels like depends
//! on gravity: at a sixth of a g the springs swing slowly and far, braking
//! takes six times the distance, and every bump is a flight.
//!
//! An app spawns a [`Vehicle`] beside a `Transform` (the pose of the centre
//! of mass), writes [`Vehicle::throttle`], `steer` and `brake` each frame,
//! and draws whatever it likes at the pose and at [`Vehicle::wheels`].
//!
//! Wheels stand on the ground as it is *drawn* ([`drawn_height`]): the
//! finest level's mesh, rebuilt from [`Ground::height`] at its lattice
//! points with the mesh's own triangles, so a tyre neither sinks into a
//! hollow the mesh does not show nor floats over one it does.

use bevy::{math::DVec2, platform::collections::HashMap, prelude::*};

use crate::{
    origin::Origin,
    world::{Ground, WorldDef},
};

/// Physics step, s.
const STEP: f32 = 1.0 / 240.0;
/// Share of its ride-height load a wheel still carries at full droop.
const PRELOAD: f32 = 0.1;
/// The bump stops, per unit mass: stiff and lossy.
const STOP_K: f32 = 2500.0;
const STOP_C: f32 = 60.0;
/// Slip angle at which a tyre in soil gives all the side force it has, rad.
const SLIP_PEAK: f32 = 0.14;
/// Hull contacts, per unit mass.
const HULL_K: f32 = 900.0;
const HULL_C: f32 = 30.0;

#[derive(Clone, Debug)]
pub struct WheelSpec {
    /// The hub at ride height, relative to the centre of mass (x right,
    /// y up, −z forward).
    pub mount: Vec3,
    pub radius: f32,
    /// How far this wheel turns for full lock, as a share of
    /// [`VehicleSpec::max_steer`]: 1 at the front, −1 on a rear axle that
    /// steers against it, 0 if it does not steer.
    pub steers: f32,
    pub driven: bool,
}

#[derive(Clone, Debug)]
pub struct VehicleSpec {
    /// kg, as driven.
    pub mass: f32,
    /// Radius of gyration squared about the body's x (pitch), y (yaw) and
    /// z (roll) axes, m².
    pub gyration: Vec3,
    pub wheels: Vec<WheelSpec>,
    /// Suspension travel either way from ride height, m.
    pub travel: f32,
    /// Damping ratio of the suspension.
    pub damping: f32,
    /// Anti-roll bar per axle, in units of a wheel's spring. Wheels are
    /// taken in pairs: 0 with 1, 2 with 3.
    pub roll_bar: f32,
    /// Power at the wheels, W, and the speed the gearing allows, m/s.
    pub power: f32,
    pub top_speed: f32,
    pub reverse_speed: f32,
    /// Full steering lock, rad.
    pub max_steer: f32,
    /// Friction of tyre on ground, driving and braking; and sideways,
    /// where a wheel sunk in soil has to bulldoze to slide.
    pub grip: f32,
    pub side_grip: f32,
    /// Rolling resistance, as a share of the load.
    pub rolling: f32,
    /// Quadratic drag of the air, 1/m.
    pub drag: f32,
    /// Points of the body that hit the ground when the wheels do not hold
    /// it off: corners, the tops of seats. Relative to the centre of mass.
    pub hull: Vec<Vec3>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WheelState {
    /// How far the spring is squeezed from ride height, m (negative:
    /// drooping), within the travel.
    pub compression: f32,
    pub contact: bool,
    /// Load on the wheel, N.
    pub load: f32,
    /// Steering angle, rad, positive to the left.
    pub steer: f32,
    /// How far it has rolled, rad.
    pub roll: f32,
    /// Speed of the ground past the wheel, m/s, forward positive.
    pub speed: f32,
    /// How hard it last came down, m/s; reset by whoever uses it.
    pub landed: f32,
    /// Where it touches the ground, in render space.
    pub patch: Vec3,
    last: f32,
}

#[derive(Component)]
pub struct Vehicle {
    pub spec: VehicleSpec,
    /// m/s, of the centre of mass.
    pub velocity: Vec3,
    /// rad/s, about world axes.
    pub spin: Vec3,
    /// −1 (reverse) to 1.
    pub throttle: f32,
    /// −1 (right) to 1 (left).
    pub steer: f32,
    /// 0 to 1; 1 is also the parking brake.
    pub brake: f32,
    pub wheels: Vec<WheelState>,
    /// Seconds since any wheel last touched the ground.
    pub airborne: f32,
    asleep: bool,
    still: f32,
}

impl Vehicle {
    pub fn new(spec: VehicleSpec) -> Self {
        let wheels = vec![WheelState::default(); spec.wheels.len()];
        Self {
            spec,
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            throttle: 0.0,
            steer: 0.0,
            brake: 1.0,
            wheels,
            airborne: 0.0,
            asleep: false,
            still: 0.0,
        }
    }

    /// Speed along the body's own forward axis, m/s.
    pub fn forward_speed(&self, transform: &Transform) -> f32 {
        self.velocity.dot(*transform.forward())
    }

    /// Height of the centre of mass above the wheels' contact patches at
    /// rest: where to put a vehicle so it starts standing.
    pub fn ride_height(&self) -> f32 {
        self.spec.wheels.iter().map(|w| w.radius - w.mount.y).fold(0.0, f32::max)
    }
}

/// Add to an app that has vehicles.
pub struct VehiclePlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Drive;

impl Plugin for VehiclePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive.in_set(Drive));
    }
}

/// Height of the ground as its finest mesh draws it: the lattice the
/// streamer samples, the same two triangles per cell.
pub fn drawn_height(ground: &dyn Ground, step: f64, x: f64, z: f64, cache: &mut Lattice) -> f32 {
    let (u, v) = (x / step, z / step);
    let (i, j) = (u.floor(), v.floor());
    let (fu, fv) = ((u - i) as f32, (v - j) as f32);
    let (i, j) = (i as i64, j as i64);
    let mut at = |di: i64, dj: i64| cache.height(ground, step, i + di, j + dj);
    // The chunk mesh cuts each cell along the diagonal from (0, 1) to (1, 0).
    if fu + fv <= 1.0 {
        let a = at(0, 0);
        a + (at(1, 0) - a) * fu + (at(0, 1) - a) * fv
    } else {
        let d = at(1, 1);
        d + (at(0, 1) - d) * (1.0 - fu) + (at(1, 0) - d) * (1.0 - fv)
    }
}

/// Heights at lattice points already asked for. Cleared when the world
/// changes; a few thousand points cover a long drive.
#[derive(Default)]
pub struct Lattice {
    heights: HashMap<(i64, i64), f32>,
}

impl Lattice {
    fn height(&mut self, ground: &dyn Ground, step: f64, i: i64, j: i64) -> f32 {
        if self.heights.len() > 20_000 {
            self.heights.clear();
        }
        *self.heights.entry((i, j)).or_insert_with(|| ground.height_at(i as f64 * step, j as f64 * step, step as f32))
    }

    pub fn clear(&mut self) {
        self.heights.clear();
    }
}

fn drive(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut vehicles: Query<(&mut Vehicle, &mut Transform)>,
    mut lattice: Local<Lattice>,
) {
    if world.is_changed() {
        lattice.clear();
    }
    let dt = time.delta_secs().min(0.06);
    if dt <= 0.0 {
        return;
    }
    let ground = &*world.ground;
    let step = world.terrain.step(0) as f64;
    let g = world.walker.gravity;
    for (mut vehicle, mut transform) in &mut vehicles {
        let vehicle = &mut *vehicle;
        let awake = vehicle.throttle.abs() > 0.01 || vehicle.brake < 1.0;
        if vehicle.asleep && !awake {
            continue;
        }
        vehicle.asleep = false;
        let n = (dt / STEP).ceil().max(1.0);
        let h = dt / n;
        for _ in 0..n as usize {
            let mut height = |p: Vec3| {
                let at: DVec2 = origin.world(p);
                drawn_height(ground, step, at.x, at.y, &mut lattice)
            };
            substep(vehicle, &mut transform, g, h, &mut height);
        }
        // A parked vehicle that has come to rest stays put: tyres in soil
        // do not creep.
        let quiet = vehicle.velocity.length() < 0.03 && vehicle.spin.length() < 0.03;
        vehicle.still = if quiet && !awake && vehicle.airborne == 0.0 { vehicle.still + dt } else { 0.0 };
        if vehicle.still > 1.0 {
            vehicle.asleep = true;
            vehicle.velocity = Vec3::ZERO;
            vehicle.spin = Vec3::ZERO;
        }
        vehicle.airborne = if vehicle.wheels.iter().any(|w| w.contact) { 0.0 } else { vehicle.airborne + dt };
    }
}

fn substep(v: &mut Vehicle, transform: &mut Transform, g: f32, h: f32, height: &mut dyn FnMut(Vec3) -> f32) {
    let spec = &v.spec;
    let m = spec.mass;
    let count = spec.wheels.len() as f32;
    let (pos, rot) = (transform.translation, transform.rotation);

    // Springs sized to the gravity they work in, so the vehicle rides at
    // its design height on any body; what changes with gravity is how
    // slowly everything happens.
    let spring = g * (1.0 - PRELOAD) / (count * spec.travel);
    let damper = spec.damping * spring.sqrt() * 2.0 / count.sqrt();

    let mut force = Vec3::new(0.0, -g * m, 0.0) - spec.drag * m * v.velocity * v.velocity.length();
    let mut torque = Vec3::ZERO;
    let point_velocity = |p: Vec3| v.velocity + v.spin.cross(p - pos);

    // Where each wheel stands and how far it is pressed in.
    let mut normals = [Vec3::Y; 8];
    for (k, (wheel, state)) in spec.wheels.iter().zip(v.wheels.iter_mut()).enumerate() {
        let hub = pos + rot * wheel.mount;
        let e = 0.25;
        let y = height(hub);
        let normal = Vec3::new(
            height(hub - Vec3::X * e) - height(hub + Vec3::X * e),
            2.0 * e,
            height(hub - Vec3::Z * e) - height(hub + Vec3::Z * e),
        )
        .normalize();
        // The tyre as a ball on the plane of the ground under the hub.
        let clear = (hub.y - y) * normal.y;
        let squeezed = wheel.radius - clear;
        let was = state.contact;
        state.contact = squeezed > -spec.travel;
        let rate = -point_velocity(hub).dot(normal);
        if state.contact && !was {
            state.landed = state.landed.max(rate);
        }
        state.last = squeezed;
        state.compression = squeezed.clamp(-spec.travel, spec.travel);
        state.patch = hub - normal * clear;
        state.steer = v.steer * wheel.steers * spec.max_steer;
        normals[k.min(7)] = normal;
    }

    for k in 0..spec.wheels.len() {
        let wheel = &spec.wheels[k];
        let state = v.wheels[k];
        if !state.contact {
            v.wheels[k].load = 0.0;
            continue;
        }
        let normal = normals[k.min(7)];
        let hub = pos + rot * wheel.mount;
        let rate = -point_velocity(hub).dot(normal);
        // The other end of the axle, for the anti-roll bar.
        let other = v.wheels.get(k ^ 1).map_or(state.compression, |o| o.compression);
        let mut load = g / count * PRELOAD
            + spring * (state.compression + spec.travel)
            + damper * rate
            + spring * spec.roll_bar * (state.compression - other);
        if state.last > spec.travel {
            load += STOP_K * (state.last - spec.travel) + STOP_C * rate.max(0.0);
        }
        let load = (load * m).max(0.0);
        v.wheels[k].load = load;

        // The tyre's own frame on the ground: forward and to the right.
        let heading = rot * Quat::from_rotation_y(state.steer) * Vec3::NEG_Z;
        let forward = (heading - normal * heading.dot(normal)).normalize_or(heading);
        let side = forward.cross(normal);
        let slip = point_velocity(state.patch);
        let (along, across) = (slip.dot(forward), slip.dot(side));
        v.wheels[k].speed = along;
        let share = m / count;

        // Sideways: force grows with the slip angle up to what friction
        // gives.
        let hold = spec.side_grip * load;
        let angle = across.atan2(along.abs() + 0.12);
        // (Never more than would stop the slide within the step.)
        let mut lateral = -across.signum() * (hold * (angle.abs() / SLIP_PEAK).min(1.0)).min(share * across.abs() / h);

        // Lengthways: the motor, the brake, and the soil's resistance.
        let traction = spec.grip * load;
        let mut drive = 0.0;
        if wheel.driven && v.brake < 1.0 {
            let driven = spec.wheels.iter().filter(|w| w.driven).count().max(1) as f32;
            let limit = if v.throttle >= 0.0 { spec.top_speed } else { spec.reverse_speed };
            let speed = along * v.throttle.signum();
            // A motor's push falls away as it nears the speed its gearing
            // allows.
            let fade = ((limit - speed) / (0.15 * limit)).clamp(0.0, 1.0);
            drive = v.throttle * fade * (spec.power / driven / speed.abs().max(0.5)).min(traction);
        }
        let stop = |want: f32| -along.signum() * want.min(share * along.abs() / h);
        drive += stop(v.brake * traction) + stop(spec.rolling * load);
        // All of it inside the friction the load affords.
        let used = ((drive / traction.max(1e-6)).powi(2) + (lateral / hold.max(1e-6)).powi(2)).sqrt();
        if used > 1.0 {
            drive /= used;
            lateral /= used;
        }
        let push = normal * load + forward * drive + side * lateral;
        force += push;
        torque += (state.patch - pos).cross(push);
    }

    // The body itself on the ground, when it comes to that.
    for point in &spec.hull {
        let p = pos + rot * *point;
        let depth = height(p) - p.y;
        if depth <= 0.0 {
            continue;
        }
        let pv = point_velocity(p);
        let press = (m * (HULL_K * depth - HULL_C * pv.y)).max(0.0);
        let slide = Vec3::new(pv.x, 0.0, pv.z);
        let drag = -slide.normalize_or_zero() * (spec.grip * press).min(m * slide.length() / h * 0.25);
        let push = Vec3::Y * press + drag;
        force += push;
        torque += (p - pos).cross(push);
    }

    // Semi-implicit Euler; the spin in the body's own axes, where the
    // inertia is diagonal.
    v.velocity += force / m * h;
    let inertia = m * spec.gyration;
    let inverse = rot.inverse();
    let (spin_b, torque_b) = (inverse * v.spin, inverse * torque);
    let spin_b = spin_b + (torque_b - spin_b.cross(inertia * spin_b)) / inertia * h;
    v.spin = rot * spin_b;
    transform.translation += v.velocity * h;
    transform.rotation = (Quat::from_scaled_axis(v.spin * h) * rot).normalize();
    let radii: Vec<f32> = spec.wheels.iter().map(|w| w.radius).collect();
    for (state, radius) in v.wheels.iter_mut().zip(radii) {
        if state.contact {
            state.roll += state.speed / radius * h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Slope(f32);
    impl Ground for Slope {
        fn height(&self, x: f64, _z: f64) -> f32 {
            x as f32 * self.0
        }
        fn tint(&self, _: f64, _: f64, _: f32, _: Vec3) -> LinearRgba {
            LinearRgba::WHITE
        }
    }

    fn car() -> VehicleSpec {
        let wheel = |x: f32, z: f32, steers: f32| WheelSpec { mount: Vec3::new(x, -0.2, z), radius: 0.4, steers, driven: true };
        VehicleSpec {
            mass: 480.0,
            gyration: Vec3::new(0.9, 1.2, 0.4),
            wheels: vec![wheel(-0.9, -1.15, 1.0), wheel(0.9, -1.15, 1.0), wheel(-0.9, 1.15, -1.0), wheel(0.9, 1.15, -1.0)],
            travel: 0.24,
            damping: 0.4,
            roll_bar: 1.5,
            power: 600.0,
            top_speed: 3.6,
            reverse_speed: 1.5,
            max_steer: 0.35,
            grip: 0.4,
            side_grip: 1.3,
            rolling: 0.1,
            drag: 0.0,
            hull: vec![],
        }
    }

    fn run(v: &mut Vehicle, t: &mut Transform, ground: &dyn Ground, g: f32, seconds: f32) {
        let mut cache = Lattice::default();
        for _ in 0..(seconds / STEP) as usize {
            let mut height = |p: Vec3| drawn_height(ground, 0.5, p.x as f64, p.z as f64, &mut cache);
            substep(v, t, g, STEP, &mut height);
        }
    }

    /// Dropped onto level ground, it comes to rest at ride height, level.
    #[test]
    fn settles_at_ride_height_under_any_gravity() {
        for g in [1.62, 3.72, 9.81, 0.62] {
            let mut v = Vehicle::new(car());
            let mut t = Transform::from_xyz(0.0, v.ride_height() + 0.3, 0.0);
            run(&mut v, &mut t, &Slope(0.0), g, 20.0);
            assert!((t.translation.y - v.ride_height()).abs() < 0.02, "g {g}: rides at {}", t.translation.y);
            assert!(v.velocity.length() < 0.02, "g {g}: still moving at {}", v.velocity.length());
            assert!((t.rotation * Vec3::Y).y > 0.999);
        }
    }

    /// Full throttle on the level reaches the geared top speed and holds
    /// it; on the Moon that takes several seconds, because grip is weight.
    #[test]
    fn reaches_top_speed_no_faster_than_grip_allows() {
        let mut v = Vehicle::new(car());
        let mut t = Transform::from_xyz(0.0, v.ride_height(), 0.0);
        (v.brake, v.throttle) = (0.0, 1.0);
        run(&mut v, &mut t, &Slope(0.0), 1.62, 1.0);
        let early = v.forward_speed(&t);
        // No more than (grip − rolling) · g, less a little for squat.
        assert!(early < 0.4 * 1.62 * 1.0 + 0.05, "{early} m/s after a second");
        run(&mut v, &mut t, &Slope(0.0), 1.62, 30.0);
        let top = v.forward_speed(&t);
        assert!((3.0..3.7).contains(&top), "top speed {top}");
    }

    /// Parked across a slope it can hold, it stays; on one it cannot, it
    /// slides.
    #[test]
    fn holds_on_a_slope_within_its_grip() {
        let mut v = Vehicle::new(car());
        let mut t = Transform::from_xyz(0.0, v.ride_height(), 0.0);
        run(&mut v, &mut t, &Slope(0.2), 1.62, 10.0);
        let x = t.translation.x;
        run(&mut v, &mut t, &Slope(0.2), 1.62, 10.0);
        assert!((t.translation.x - x).abs() < 0.3, "crept {} m", t.translation.x - x);
    }
}
