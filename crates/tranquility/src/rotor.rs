//! Rotorcraft: a rigid body hung from its rotors, flown by a flight
//! computer.
//!
//! Each rotor is worked out from its blades and the air. Blade-element
//! theory gives the thrust a collective pitch makes at a given inflow;
//! momentum theory (Glauert's, for a disc moving edgewise as well as along
//! its axis) gives the inflow that thrust draws through the disc. So thrust
//! goes with the air's density and the square of the tip speed, a climb
//! takes some away, flying forward gives some back (translational lift),
//! and low over the ground the disc rides on its own downwash (ground
//! effect, after Cheeseman and Bennett). The power is the induced power
//! that pushes the air down plus the profile drag of the blades; the
//! motors give so much of it, the blades stall past a blade loading, and
//! the battery holds so much energy. Run the rotor too slowly, carry too
//! much, fly too high a climb, and it will not hold; on a body without air
//! nothing turns into thrust at all.
//!
//! Nobody flies one of these by hand on another world, and nobody could:
//! a small one answers its controls in a tenth of a second. A flight
//! computer, as on the real machines, takes the speeds asked for
//! ([`Command`]: forward, sideways, up, a turn) and works the rotors to
//! hold them. It tilts the craft to speed up or slow down, which is how a
//! helicopter goes anywhere, so it is no faster than its thrust margin and
//! its tilt limit allow, and gusts push it about until the loop catches
//! up.
//!
//! An app spawns a [`Rotorcraft`] beside a `Transform` (the pose of the
//! centre of mass), sets [`Rotorcraft::air`] and [`Rotorcraft::running`],
//! writes [`Rotorcraft::command`], and draws the machine at the pose with
//! its blades at [`Rotorcraft::azimuth`].

use std::f32::consts::{PI, TAU};

use bevy::{math::DVec2, prelude::*};

use crate::{
    origin::Origin,
    vehicle::{Lattice, drawn_height},
    world::WorldDef,
};

/// Physics step, s: a small rotorcraft's attitude moves in hundredths.
const STEP: f32 = 1.0 / 480.0;
/// Induced power over the ideal for a real disc: uneven inflow, tip loss.
const INDUCED: f32 = 1.15;
/// The lower rotor of a coaxial pair works in the upper's wake. With both
/// carrying the same thrust, the pair needs 1.28 times the induced power
/// of two rotors apart (Leishman); all of the excess is put on the lower.
const IN_WAKE: f32 = 1.56;
/// Feet on the ground, per unit mass: a natural frequency near 6 Hz.
const FOOT_K: f32 = 1500.0;
const FOOT_C: f32 = 50.0;
/// Roughness length of the ground for the wind's profile, m.
const ROUGHNESS: f32 = 0.03;

#[derive(Clone, Debug)]
pub struct Rotor {
    /// The hub, relative to the centre of mass (x right, y up, −z forward).
    pub hub: Vec3,
    pub radius: f32,
    pub blades: u32,
    /// Blade area over disc area.
    pub solidity: f32,
    /// +1 turning anticlockwise seen from above, −1 clockwise.
    pub turn: f32,
    /// The lower of a coaxial pair: it works in the upper's wake.
    pub in_wake: bool,
    /// Shaft tilted round the centre, rad, the way the rotor's torque
    /// turns the body: a multirotor's yaw, which torque alone hardly moves.
    pub cant: f32,
}

#[derive(Clone, Debug)]
pub struct RotorcraftSpec {
    /// kg, as flown.
    pub mass: f32,
    /// Moments of inertia about the body's x (pitch), y (yaw) and z
    /// (roll) axes, kg m².
    pub inertia: Vec3,
    pub rotors: Vec<Rotor>,
    /// Governed rotor speed, rpm, and how long the rotors take to reach
    /// it from rest, s.
    pub rpm: f32,
    pub spin_up: f32,
    /// The blade section: lift-curve slope per radian, profile drag
    /// coefficient, the highest collective pitch, rad, and the blade
    /// loading (thrust coefficient over solidity) at which it stalls.
    pub lift_slope: f32,
    pub profile_drag: f32,
    pub max_pitch: f32,
    pub stall: f32,
    /// How far the swashplates can tilt the rotors' thrust, rad, and the
    /// arm that tilt has about the centre of mass, m (the hub's height plus
    /// the stiffness of a hingeless hub). 0 for a craft that steers by
    /// differential thrust alone.
    pub cyclic: f32,
    pub cyclic_arm: f32,
    /// How fast the blade pitch follows the computer, s.
    pub response: f32,
    /// Shaft power the motors can give, all together, W, and the share of
    /// what they draw that reaches the shafts.
    pub power: f32,
    pub efficiency: f32,
    /// Drawn besides the motors while the rotors turn, W: computers,
    /// sensors, radio.
    pub avionics: f32,
    /// Energy the battery may give a flight, J.
    pub battery: f32,
    /// Drag area of everything but the rotor discs, Cd·A, m².
    pub drag_area: f32,
    /// Feet, relative to the centre of mass.
    pub feet: Vec<Vec3>,
    pub grip: f32,
    /// The flight computer's limits: speed over the ground, climb and
    /// descent, m/s; a turn, rad/s; tilt, rad.
    pub top_speed: f32,
    pub climb: f32,
    pub descent: f32,
    pub turn_rate: f32,
    pub max_tilt: f32,
    /// Bandwidth of the attitude loop, rad/s: a small machine is quick.
    pub agility: f32,
    /// The fastest the feet may meet the ground, m/s, before something
    /// breaks.
    pub toughness: f32,
}

/// The air the craft flies in: density, kg/m³; the speed of sound, m/s;
/// and the wind ten metres up (it falls off toward the ground).
#[derive(Clone, Copy, Debug, Default)]
pub struct Airflow {
    pub density: f32,
    pub sound: f32,
    pub wind: Vec3,
}

impl Airflow {
    /// The wind at a height above the ground: the log profile of a
    /// boundary layer.
    pub fn wind_at(&self, height: f32) -> Vec3 {
        let z = height.max(ROUGHNESS * 2.0);
        self.wind * ((z / ROUGHNESS).ln() / (10.0 / ROUGHNESS).ln()).clamp(0.0, 1.3)
    }
}

/// What the pilot asks for, in the craft's heading: m/s forward, right and
/// up, and a turn, rad/s anticlockwise from above. The flight computer
/// holds to its own limits.
#[derive(Clone, Copy, Debug, Default)]
pub struct Command {
    pub forward: f32,
    pub right: f32,
    pub up: f32,
    pub turn: f32,
}

#[derive(Component, Clone, Debug)]
pub struct Rotorcraft {
    pub spec: RotorcraftSpec,
    pub air: Airflow,
    pub command: Command,
    /// Whether the rotors are wanted turning. They spin up over
    /// `spin_up`; the craft lifts off only at full speed, and only when
    /// asked to climb.
    pub running: bool,
    pub velocity: Vec3,
    /// Angular velocity, world frame, rad/s.
    pub spin: Vec3,
    /// Rotor speed now, rpm.
    pub rpm: f32,
    /// Each rotor's blade angle, rad, for drawing.
    pub azimuth: Vec<f32>,
    /// Each rotor's thrust, N, and its collective pitch, rad.
    pub thrust: Vec<f32>,
    pub pitch: Vec<f32>,
    /// Swashplate tilt of the thrust, in the body's frame.
    pub tilt: Vec2,
    /// Battery energy left, J, and the power drawn from it now, W.
    pub energy: f32,
    pub drawn: f32,
    /// The most thrust the rotors could give now, over the weight: under 1
    /// the craft cannot hold its height.
    pub margin: f32,
    /// What limits the thrust: the blades stall, or the motors are at full
    /// power.
    pub stalled: bool,
    pub power_limited: bool,
    /// Whether any foot is on the ground, and the height of the lowest
    /// above it, m (a laser altimeter's reading).
    pub grounded: bool,
    pub height: f32,
    /// Why it no longer flies, if something broke.
    pub wrecked: Option<String>,
    /// The heading the computer holds, rad, and its integrators.
    heading: f32,
    hold: Vec3,
    twist: f32,
    /// Rotor torque per newton of thrust, m, from the last step: what the
    /// mixer needs to turn by torque.
    torque_arm: f32,
    still: f32,
    asleep: bool,
}

impl Rotorcraft {
    pub fn new(spec: RotorcraftSpec) -> Self {
        let n = spec.rotors.len();
        let arm = 0.08 * spec.rotors.first().map_or(1.0, |r| r.radius);
        Self {
            energy: spec.battery,
            spec,
            air: Airflow::default(),
            command: Command::default(),
            running: false,
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            rpm: 0.0,
            azimuth: (0..n).map(|k| k as f32 * 1.3).collect(),
            thrust: vec![0.0; n],
            pitch: vec![0.0; n],
            tilt: Vec2::ZERO,
            drawn: 0.0,
            margin: 0.0,
            stalled: false,
            power_limited: false,
            grounded: true,
            height: 0.0,
            wrecked: None,
            heading: 0.0,
            hold: Vec3::ZERO,
            twist: 0.0,
            torque_arm: arm,
            still: 0.0,
            asleep: false,
        }
    }

    /// Height of the centre of mass over level ground, standing on its feet.
    pub fn rest_height(&self) -> f32 {
        -self.spec.feet.iter().map(|f| f.y).fold(0.0, f32::min)
    }

    /// Weight, N, at a gravity.
    pub fn weight(&self, g: f32) -> f32 {
        self.spec.mass * g
    }

    /// Blade tips' speed, m/s, and their Mach number in this air.
    pub fn tip_speed(&self) -> f32 {
        self.rpm * TAU / 60.0 * self.spec.rotors.first().map_or(0.0, |r| r.radius)
    }

    pub fn tip_mach(&self) -> f32 {
        if self.air.sound > 0.0 { self.tip_speed() / self.air.sound } else { 0.0 }
    }

    /// Whether it is on its feet with the rotors stopped.
    pub fn parked(&self) -> bool {
        self.grounded && self.rpm == 0.0
    }

    /// Fresh batteries, nothing broken, at rest: for a craft set down again.
    pub fn reset(&mut self) {
        *self = Self::new(self.spec.clone());
    }

    /// The flight computer: the thrust and the body-frame moment that
    /// would bring the craft to the speeds asked for.
    fn steer(&mut self, transform: &Transform, g: f32, h: f32) -> (f32, Vec3) {
        let s = &self.spec;
        let rot = transform.rotation;
        let forward = rot * Vec3::NEG_Z;
        let yaw = (-forward.x).atan2(-forward.z);
        let up_to_speed = self.rpm >= 0.97 * s.rpm;
        // On the ground it waits for full rotor speed and a climb; till
        // then the blades sit at flat pitch and the computer holds nothing.
        if self.grounded && (!up_to_speed || self.command.up <= 0.05 || !self.running) {
            self.heading = yaw;
            self.hold = Vec3::ZERO;
            self.twist = 0.0;
            return (0.0, Vec3::ZERO);
        }
        let c = self.command;
        self.heading += c.turn.clamp(-s.turn_rate, s.turn_rate) * h;
        let (sin, cos) = self.heading.sin_cos();
        let ahead = Vec3::new(-sin, 0.0, -cos);
        let right = Vec3::new(cos, 0.0, -sin);
        let mut want = ahead * c.forward + right * c.right;
        want = want.clamp_length_max(s.top_speed);
        // It comes down gently over the last metre, as the real ones do.
        let floor = if self.height < 1.0 { -0.5f32.max(-s.descent) } else { -s.descent };
        let climb = c.up.clamp(floor, s.climb);
        let flat = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
        let k_v = s.agility / 6.0;
        let k_z = s.agility / 4.0;
        let err = want - flat;
        let err_z = climb - self.velocity.y;
        // The integrators take up the wind and what the model of the
        // rotors leaves out.
        // Only small errors are integrated, so a long acceleration does
        // not wind them up into an overshoot.
        let small = Vec3::new(err.x, err_z, err.z) * (-Vec3::new(err.x, err_z, err.z).length_squared() / 0.25).exp();
        self.hold += small * (k_v * k_v / 4.0) * h;
        self.hold = self.hold.clamp_length_max(0.5 * g);
        let reach = g * s.max_tilt.tan();
        let along = (err * k_v + Vec3::new(self.hold.x, 0.0, self.hold.z)).clamp_length_max(reach);
        let lift = (err_z * k_z + self.hold.y).clamp(-0.6 * g, 1.5 * g);
        let mut a = along + Vec3::Y * (g + lift);
        a.y = a.y.max(0.2 * g);
        // The thrust points where the acceleration should; no further over
        // than the tilt limit.
        let mut n = a.normalize();
        let tilt = n.y.clamp(-1.0, 1.0).acos();
        if tilt > s.max_tilt {
            let side = Vec3::new(n.x, 0.0, n.z).normalize_or_zero();
            n = Vec3::Y * s.max_tilt.cos() + side * s.max_tilt.sin();
        }
        let up = rot * Vec3::Y;
        let thrust = s.mass * a.dot(up).max(0.0);
        let goal = Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(self.heading);
        let mut error = goal * rot.inverse();
        if error.w < 0.0 {
            error = -error;
        }
        let (axis, angle) = error.to_axis_angle();
        let e = if angle.is_finite() { axis * angle } else { Vec3::ZERO };
        let w = s.agility;
        let accel = e * w * w - self.spin * 1.6 * w;
        let mut body = rot.inverse() * accel;
        // Rotors that do not quite balance each other's torque leave a
        // steady twist: an integrator holds the heading against it.
        self.twist = (self.twist + (rot.inverse() * e).y * w * w * 0.25 * h).clamp(-0.2 * w * w, 0.2 * w * w);
        body.y += self.twist;
        (thrust, body * s.inertia)
    }

    fn substep(&mut self, transform: &mut Transform, g: f32, h: f32, measure: bool, ground: &mut dyn FnMut(Vec3) -> (f32, bool)) {
        let target = if self.running && self.energy > 0.0 && self.wrecked.is_none() { self.spec.rpm } else { 0.0 };
        let rate = self.spec.rpm / self.spec.spin_up;
        // Spinning down is slower: only the blades' drag stops them.
        self.rpm = (self.rpm + (target - self.rpm).clamp(-0.5 * rate * h, rate * h)).max(0.0);
        let (thrust, moment) = self.steer(transform, g, h);
        let s = &self.spec;
        let n = s.rotors.len();
        let rot = transform.rotation;
        let pos = transform.translation;
        let omega = self.rpm * TAU / 60.0;
        let sxx: f32 = s.rotors.iter().map(|r| r.hub.x * r.hub.x).sum();
        let szz: f32 = s.rotors.iter().map(|r| r.hub.z * r.hub.z).sum();
        let mut tilt = Vec2::ZERO;
        if s.cyclic > 0.0 && thrust > 0.0 {
            // A thrust tilted toward body x makes a moment about −z, toward
            // z one about x.
            let fx = if sxx > 0.01 { 0.0 } else { -moment.z / s.cyclic_arm };
            let fz = if szz > 0.01 { 0.0 } else { moment.x / s.cyclic_arm };
            tilt = (Vec2::new(fx, fz) / thrust).clamp_length_max(s.cyclic.tan());
        }
        self.tilt += (tilt - self.tilt) * (h / s.response).min(1.0);
        let axis_body = Vec3::new(self.tilt.x, 1.0, self.tilt.y).normalize();
        // Each rotor's shaft: the swashplates' tilt, and the rotor's cant
        // round the centre, which turns a difference in thrust into yaw.
        let axes: Vec<Vec3> = s
            .rotors
            .iter()
            .map(|r| {
                let round = Vec3::new(-r.hub.z, 0.0, r.hub.x).normalize_or_zero();
                rot * (axis_body + round * r.turn * r.cant.tan()).normalize()
            })
            .collect();
        // Each rotor's air: its own motion through the wind, along its axis
        // and across it, and how near the ground (or the sea) under it is.
        let rho = self.air.density;
        let mut air = Vec::with_capacity(n);
        for (r, axis) in s.rotors.iter().zip(&axes) {
            let hub = pos + rot * r.hub;
            let (floor, _) = ground(hub);
            let above = (hub.y - floor).max(0.0);
            let v = self.velocity + self.spin.cross(rot * r.hub) - self.air.wind_at(above);
            let climb = v.dot(*axis);
            let edge = (v - *axis * climb).length();
            // Cheeseman and Bennett: the same power holds more thrust near
            // the ground, as if the induced velocity were less.
            let ge = 1.0 - (r.radius / (4.0 * above.max(0.5 * r.radius))).powi(2);
            let wake = INDUCED * if r.in_wake { IN_WAKE } else { 1.0 };
            air.push(Disc { rho, area: PI * r.radius * r.radius, tip: omega * r.radius, climb, edge, induced: wake * ge, solidity: r.solidity });
        }
        let blade = (s.lift_slope, s.profile_drag);
        let most: Vec<f32> = air.iter().map(|d| d.most(s.max_pitch, s.stall, blade)).collect();
        // The mixer: thrust shared out, then pitch and roll by differential
        // thrust where the rotors stand apart (the swashplates do it where
        // they do not), then yaw with what is left: by the torque between
        // rotors turning opposite ways and by their cant.
        let mut demand: Vec<f32> = s
            .rotors
            .iter()
            .map(|r| {
                let mut t = thrust / n as f32;
                if sxx > 0.01 {
                    t += moment.z * r.hub.x / sxx;
                }
                if szz > 0.01 {
                    t -= moment.x * r.hub.z / szz;
                }
                t
            })
            .collect();
        // The body feels each rotor's torque the other way round to its
        // turning, and a canted rotor's thrust the same way.
        let twist: Vec<f32> = s
            .rotors
            .iter()
            .map(|r| {
                let arm = self.torque_arm + r.cant.sin() * Vec2::new(r.hub.x, r.hub.z).length();
                -moment.y * r.turn / (arm * n as f32)
            })
            .collect();
        let mut share = 1.0f32;
        for k in 0..n {
            let (t, d) = (demand[k].clamp(0.0, most[k]), twist[k]);
            if d > 0.0 && t + d > most[k] {
                share = share.min((most[k] - t) / d);
            } else if d < 0.0 && t + d < 0.0 {
                share = share.min(t / -d);
            }
        }
        for k in 0..n {
            demand[k] += twist[k] * share.max(0.0);
        }
        self.stalled = false;
        for (k, t) in demand.iter_mut().enumerate() {
            *t = t.max(0.0);
            if *t > most[k] {
                *t = most[k];
                self.stalled |= most[k] < air[k].thrust_at(s.max_pitch, blade) + 1e-6;
            }
        }
        // Over the motors' power the computer takes collective off every
        // rotor alike.
        let shaft = |demand: &[f32]| -> f32 { demand.iter().zip(&air).map(|(t, d)| d.power(*t, blade.1)).sum() };
        self.power_limited = false;
        if shaft(&demand) > s.power {
            self.power_limited = true;
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            for _ in 0..12 {
                let mid = 0.5 * (lo + hi);
                let scaled: Vec<f32> = demand.iter().map(|t| t * mid).collect();
                if shaft(&scaled) > s.power { hi = mid } else { lo = mid }
            }
            for t in &mut demand {
                *t *= lo;
            }
        }
        let follow = (h / s.response).min(1.0);
        let mut force = Vec3::NEG_Y * s.mass * g;
        let mut torque = Vec3::ZERO;
        let mut power = 0.0;
        let mut yaw = 0.0;
        let (mut thrusts, mut torques) = (0.0, 0.0);
        for (k, r) in s.rotors.iter().enumerate() {
            self.thrust[k] += (demand[k] - self.thrust[k]) * follow;
            let t = self.thrust[k];
            let p = air[k].power(t, blade.1);
            power += p.max(0.0);
            let q = if omega > 1.0 { p / omega } else { 0.0 };
            yaw -= r.turn * q;
            (thrusts, torques) = (thrusts + t, torques + q);
            self.pitch[k] = air[k].pitch_for(t, blade.0);
            let f = axes[k] * t;
            force += f;
            torque += (rot * r.hub).cross(f);
            self.azimuth[k] = (self.azimuth[k] + r.turn * omega * h).rem_euclid(TAU);
        }
        if thrusts > 0.1 {
            self.torque_arm = (torques / thrusts).max(1e-3);
        }
        torque += rot * Vec3::new(0.0, yaw, 0.0);
        if s.cyclic > 0.0 {
            // A hingeless hub turns the shaft with the disc: the tilt's arm
            // is longer than the hub's height.
            let hub_y = s.rotors.iter().map(|r| r.hub.y).sum::<f32>() / n as f32;
            let lateral = axis_body * thrusts;
            torque += rot * (Vec3::new(lateral.z, 0.0, -lateral.x) * (s.cyclic_arm - hub_y));
        }
        // The rotors damp a roll or pitch, a little (their discs lag
        // behind the shaft and tilt against it); the body's drag in the
        // wind.
        let r0 = s.rotors.first().map_or(0.0, |r| r.radius);
        let level = self.spin - (rot * Vec3::Y) * self.spin.dot(rot * Vec3::Y);
        torque -= level * 0.3 * thrusts * r0 * r0 / s.agility.max(0.1);
        let (floor, _) = ground(pos);
        let v_air = self.velocity - self.air.wind_at((pos.y - floor).max(0.0));
        force -= v_air * v_air.length() * 0.5 * rho * s.drag_area;
        // The margin: the most thrust shared alike that the blades and the
        // motors' power allow. Once a frame is enough for a readout.
        if measure {
            let shared = |f: f32| -> Vec<f32> { most.iter().map(|m| (f * s.mass * g / n as f32).min(*m)).collect() };
            let (mut lo, mut hi) = (0.0f32, 4.0f32);
            for _ in 0..14 {
                let mid = 0.5 * (lo + hi);
                let t = shared(mid);
                let capped = t.iter().zip(&most).all(|(t, m)| t >= m);
                if shaft(&t) > s.power || capped { hi = mid } else { lo = mid }
            }
            self.margin = if g > 0.0 && rho > 0.0 && omega > 1.0 { shared(lo).iter().sum::<f32>() / (s.mass * g) } else { 0.0 };
        }
        // The feet.
        let share = s.mass / s.feet.len().max(1) as f32;
        let mut contact = false;
        let mut lowest = f32::MAX;
        for foot in &s.feet {
            let arm = rot * *foot;
            let p = pos + arm;
            let (floor, wet) = ground(p);
            let depth = floor - p.y;
            lowest = lowest.min(-depth);
            if depth <= 0.0 {
                continue;
            }
            let v = self.velocity + self.spin.cross(arm);
            if !self.grounded && self.wrecked.is_none() {
                if wet {
                    self.wrecked = Some("came down in the sea".into());
                } else if -v.y > s.toughness {
                    self.wrecked = Some(format!("met the ground at {:.1} m/s: the legs gave way", -v.y));
                }
            }
            contact = true;
            let push = (FOOT_K * depth - FOOT_C * v.y).max(0.0) * share;
            let slide = Vec3::new(v.x, 0.0, v.z);
            // Sticks where friction allows, slides where it does not.
            let hold = (slide * 0.5 * share / h).clamp_length_max(s.grip * push);
            let f = Vec3::Y * push - hold;
            force += f;
            torque += arm.cross(f);
        }
        self.grounded = contact;
        self.height = lowest.max(0.0);
        if contact && (rot * Vec3::Y).y < 0.6 && self.wrecked.is_none() {
            self.wrecked = Some("tipped over: the blades struck the ground".into());
        }
        if self.wrecked.is_some() {
            self.running = false;
        }
        // Battery.
        self.drawn = if self.rpm > 0.0 { power / s.efficiency + s.avionics } else { 0.0 };
        self.energy = (self.energy - self.drawn * h).max(0.0);
        // Integrate.
        self.velocity += force / s.mass * h;
        let body = rot.inverse() * self.spin;
        let gyro = body.cross(s.inertia * body);
        let spun = body + (rot.inverse() * torque - gyro) / s.inertia * h;
        self.spin = rot * spun;
        transform.translation += self.velocity * h;
        transform.rotation = (Quat::from_scaled_axis(self.spin * h) * rot).normalize();
    }
}

/// One rotor's disc in its air.
struct Disc {
    rho: f32,
    area: f32,
    tip: f32,
    /// Air speed along the disc's axis (up through it is a climb) and
    /// across it, m/s.
    climb: f32,
    edge: f32,
    /// Induced power over the ideal: losses, the wake, the ground.
    induced: f32,
    solidity: f32,
}

impl Disc {
    /// Induced velocity at a thrust: Glauert's momentum relation, solved by
    /// iteration from the hover value.
    fn inflow(&self, thrust: f32) -> f32 {
        if thrust <= 0.0 || self.rho <= 0.0 {
            return 0.0;
        }
        let k = thrust / (2.0 * self.rho * self.area);
        let mut v = k.sqrt();
        for _ in 0..10 {
            let through = (self.edge * self.edge + (self.climb + v) * (self.climb + v)).sqrt().max(0.1 * k.sqrt());
            v = 0.5 * (v + k / through);
        }
        v
    }

    /// Thrust at a collective pitch: blade element and momentum together.
    fn thrust_at(&self, pitch: f32, (slope, _): (f32, f32)) -> f32 {
        if self.tip < 1.0 || self.rho <= 0.0 {
            return 0.0;
        }
        let mu = self.edge / self.tip;
        let scale = self.rho * self.area * self.tip * self.tip;
        let mut t = 0.0;
        for _ in 0..16 {
            let lambda = (self.climb + self.induced * self.inflow(t)) / self.tip;
            let ct = (self.solidity * slope / 2.0 * (pitch / 3.0 * (1.0 + 1.5 * mu * mu) - lambda / 2.0)).max(0.0);
            t = 0.5 * (t + ct * scale);
        }
        t
    }

    /// The most thrust it has: the collective's travel or the blades'
    /// stall, whichever comes first.
    fn most(&self, max_pitch: f32, stall: f32, blade: (f32, f32)) -> f32 {
        let stall = stall * self.solidity * self.rho * self.area * self.tip * self.tip;
        self.thrust_at(max_pitch, blade).min(stall)
    }

    /// Collective pitch for a thrust, rad.
    fn pitch_for(&self, thrust: f32, slope: f32) -> f32 {
        if self.tip < 1.0 || self.rho <= 0.0 {
            return 0.0;
        }
        let mu = self.edge / self.tip;
        let ct = thrust / (self.rho * self.area * self.tip * self.tip);
        let lambda = (self.climb + self.induced * self.inflow(thrust)) / self.tip;
        3.0 * (2.0 * ct / (self.solidity * slope) + lambda / 2.0) / (1.0 + 1.5 * mu * mu)
    }

    /// Shaft power at a thrust, W: pushing the air down, climbing, and the
    /// blades' profile drag, which grows edgewise.
    fn power(&self, thrust: f32, profile_drag: f32) -> f32 {
        let mu = if self.tip > 1.0 { self.edge / self.tip } else { 0.0 };
        let profile = self.rho * self.area * self.tip.powi(3) * self.solidity * profile_drag / 8.0 * (1.0 + 4.65 * mu * mu);
        thrust * (self.induced * self.inflow(thrust) + self.climb) + profile
    }
}

pub struct RotorPlugin;

/// Where the rotorcraft move: an app's steering runs before, its drawing
/// after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fly;

impl Plugin for RotorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fly.in_set(Fly));
    }
}

fn fly(
    time: Res<Time>,
    world: Res<WorldDef>,
    origin: Res<Origin>,
    mut craft: Query<(&mut Rotorcraft, &mut Transform)>,
    mut lattice: Local<Lattice>,
) {
    if world.is_changed() {
        lattice.clear();
    }
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }
    let ground = &*world.ground;
    let step = world.terrain.step(0) as f64;
    let sea = world.water.as_ref().map(|w| w.level);
    let g = world.walker.gravity;
    for (mut craft, mut transform) in &mut craft {
        let craft = &mut *craft;
        if craft.asleep && !craft.running {
            continue;
        }
        craft.asleep = false;
        let n = (dt / STEP).ceil().max(1.0);
        let h = dt / n;
        for k in 0..n as usize {
            let mut height = |p: Vec3| {
                let at: DVec2 = origin.world(p);
                let floor = drawn_height(ground, step, at.x, at.y, &mut lattice);
                match sea {
                    Some(level) if level > floor => (level, true),
                    _ => (floor, false),
                }
            };
            craft.substep(&mut transform, g, h, k + 1 == n as usize, &mut height);
        }
        let quiet = craft.velocity.length() < 0.02 && craft.spin.length() < 0.02;
        craft.still = if quiet && craft.parked() && !craft.running { craft.still + dt } else { 0.0 };
        if craft.still > 1.0 {
            craft.asleep = true;
            craft.velocity = Vec3::ZERO;
            craft.spin = Vec3::ZERO;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small coaxial helicopter on Ingenuity's numbers.
    fn small() -> RotorcraftSpec {
        let rotor = |y: f32, turn: f32, in_wake: bool| Rotor { hub: Vec3::new(0.0, y, 0.0), radius: 0.605, blades: 2, solidity: 0.15, turn, in_wake, cant: 0.0 };
        let foot = |x: f32, z: f32| Vec3::new(x, -0.2, z);
        RotorcraftSpec {
            mass: 1.8,
            inertia: Vec3::new(0.045, 0.03, 0.045),
            rotors: vec![rotor(0.17, 1.0, false), rotor(0.09, -1.0, true)],
            rpm: 2537.0,
            spin_up: 6.0,
            lift_slope: 5.7,
            profile_drag: 0.035,
            max_pitch: 0.4,
            stall: 0.11,
            cyclic: 0.15,
            cyclic_arm: 0.35,
            response: 0.03,
            power: 330.0,
            efficiency: 0.65,
            avionics: 20.0,
            battery: 20.0 * 3600.0,
            drag_area: 0.03,
            feet: vec![foot(-0.25, -0.25), foot(0.25, -0.25), foot(-0.25, 0.25), foot(0.25, 0.25)],
            grip: 0.6,
            top_speed: 10.0,
            climb: 2.0,
            descent: 1.0,
            turn_rate: 0.6,
            max_tilt: 0.3,
            agility: 6.0,
            toughness: 2.0,
        }
    }

    fn run(c: &mut Rotorcraft, t: &mut Transform, g: f32, seconds: f32, mut each: impl FnMut(&mut Rotorcraft, f32)) {
        let mut ground = |_: Vec3| (0.0, false);
        for k in 0..(seconds / STEP) as usize {
            each(c, k as f32 * STEP);
            c.substep(t, g, STEP, true, &mut ground);
        }
    }

    fn mars(spec: RotorcraftSpec) -> (Rotorcraft, Transform) {
        let mut c = Rotorcraft::new(spec);
        c.air = Airflow { density: 0.020, sound: 240.0, wind: Vec3::ZERO };
        let t = Transform::from_xyz(0.0, c.rest_height(), 0.0);
        (c, t)
    }

    /// It spins up, lifts off, climbs to its height and hovers there, on
    /// about the power the real one drew (some 350 W).
    #[test]
    fn hovers_on_mars_on_its_real_power() {
        let (mut c, mut t) = mars(small());
        c.running = true;
        run(&mut c, &mut t, 3.72, 8.0, |_, _| {});
        assert!(t.translation.y < c.rest_height() + 0.02, "lifted before it was asked to");
        run(&mut c, &mut t, 3.72, 20.0, |c, s| c.command.up = if s < 3.0 { 1.0 } else { 0.0 });
        let y = t.translation.y;
        assert!((2.0..4.5).contains(&y), "hovers at {y}");
        assert!(c.velocity.length() < 0.05, "drifting at {}", c.velocity);
        assert!((280.0..420.0).contains(&c.drawn), "draws {} W", c.drawn);
        assert!(c.margin > 1.1 && c.margin < 2.0, "margin {}", c.margin);
        assert!(c.wrecked.is_none());
    }

    /// Where there is no air, the rotors turn and nothing happens.
    #[test]
    fn no_air_no_flight() {
        let (mut c, mut t) = mars(small());
        c.air.density = 0.0;
        c.running = true;
        run(&mut c, &mut t, 1.62, 15.0, |c, _| c.command.up = 1.0);
        assert!(t.translation.y < c.rest_height() + 0.01);
        assert_eq!(c.margin, 0.0);
    }

    /// The air cushion near the ground: hovering low takes less power.
    #[test]
    fn ground_effect_saves_power() {
        let hover = |height: f32| {
            let (mut c, mut t) = mars(small());
            c.running = true;
            c.rpm = c.spec.rpm;
            run(&mut c, &mut t, 3.72, 30.0, |c, _| {
                c.command.up = (height - c.height).clamp(-1.0, 1.0);
            });
            c.drawn
        };
        let (low, high) = (hover(0.3), hover(6.0));
        assert!(low < 0.99 * high, "{low} W low, {high} W high");
    }

    /// It flies at the speed asked, leaning into it, and stops again.
    #[test]
    fn flies_forward_and_stops() {
        let (mut c, mut t) = mars(small());
        c.running = true;
        c.rpm = c.spec.rpm;
        run(&mut c, &mut t, 3.72, 25.0, |c, s| {
            c.command.up = if s < 3.0 { 1.0 } else { 0.0 };
            c.command.forward = if s > 4.0 { 5.0 } else { 0.0 };
        });
        let speed = -c.velocity.z;
        assert!((4.5..5.5).contains(&speed), "{speed} m/s");
        assert!(c.velocity.x.abs() < 0.3);
        run(&mut c, &mut t, 3.72, 10.0, |c, _| c.command.forward = 0.0);
        assert!(c.velocity.length() < 0.1, "still at {}", c.velocity);
        assert!(c.wrecked.is_none());
    }

    /// A battery that runs flat in flight: the rotors run down, the
    /// thrust goes with them, and from a few metres the landing breaks it.
    #[test]
    fn a_flat_battery_brings_it_down() {
        let (mut c, mut t) = mars(small());
        c.running = true;
        c.rpm = c.spec.rpm;
        run(&mut c, &mut t, 3.72, 8.0, |c, s| c.command.up = if s < 3.0 { 1.5 } else { 0.0 });
        assert!(c.height > 3.0, "only {} m up", c.height);
        c.energy = 200.0;
        run(&mut c, &mut t, 3.72, 10.0, |_, _| {});
        assert_eq!(c.energy, 0.0);
        assert!(c.grounded);
        assert!(c.wrecked.is_some(), "landed softly at {:?}", c.velocity);
    }

    /// With the rotors stopped in the air, it falls and breaks.
    #[test]
    fn a_fall_breaks_it() {
        let (mut c, mut t) = mars(small());
        t.translation.y += 6.0;
        c.grounded = false;
        run(&mut c, &mut t, 3.72, 4.0, |_, _| {});
        assert!(c.wrecked.is_some());
    }

    /// Eight rotors in four coaxial pairs, on Dragonfly's numbers.
    fn octo() -> RotorcraftSpec {
        let mut rotors = Vec::new();
        for (x, z) in [(-1.25, -1.25), (1.25, -1.25), (-1.25, 1.25), (1.25, 1.25)] {
            let turn = if (x > 0.0) == (z > 0.0) { 1.0 } else { -1.0 };
            rotors.push(Rotor { hub: Vec3::new(x, 0.55, z), radius: 0.675, blades: 2, solidity: 0.1, turn, in_wake: false, cant: 0.1 });
            rotors.push(Rotor { hub: Vec3::new(x, 0.3, z), radius: 0.675, blades: 2, solidity: 0.1, turn: -turn, in_wake: true, cant: 0.1 });
        }
        let foot = |x: f32, z: f32| Vec3::new(x, -0.9, z);
        RotorcraftSpec {
            mass: 875.0,
            inertia: Vec3::new(1250.0, 1600.0, 900.0),
            rotors,
            rpm: 700.0,
            spin_up: 10.0,
            lift_slope: 5.7,
            profile_drag: 0.012,
            max_pitch: 0.35,
            stall: 0.13,
            cyclic: 0.0,
            cyclic_arm: 0.0,
            response: 0.15,
            power: 12_000.0,
            efficiency: 0.85,
            avionics: 300.0,
            battery: 4000.0 * 3600.0,
            drag_area: 2.5,
            feet: vec![foot(-0.7, -1.2), foot(0.7, -1.2), foot(-0.7, 1.2), foot(0.7, 1.2)],
            grip: 0.5,
            top_speed: 10.0,
            climb: 2.0,
            descent: 1.5,
            turn_rate: 0.3,
            max_tilt: 0.25,
            agility: 2.5,
            toughness: 1.5,
        }
    }

    /// On Titan it hovers by differential thrust alone, turns where it is
    /// asked to, and sets itself down unharmed.
    #[test]
    fn octocopter_hovers_turns_and_lands_on_titan() {
        let mut c = Rotorcraft::new(octo());
        c.air = Airflow { density: 5.4, sound: 194.0, wind: Vec3::ZERO };
        let mut t = Transform::from_xyz(0.0, c.rest_height(), 0.0);
        c.running = true;
        run(&mut c, &mut t, 1.352, 40.0, |c, s| {
            c.command.up = if (10.0..15.0).contains(&s) { 1.5 } else { 0.0 };
            c.command.turn = if (25.0..30.0).contains(&s) { 0.2 } else { 0.0 };
        });
        assert!(!c.grounded && t.translation.y > 5.0, "at {}", t.translation.y);
        let forward = t.rotation * Vec3::NEG_Z;
        let yaw = (-forward.x).atan2(-forward.z);
        assert!((yaw - 1.0).abs() < 0.1, "turned to {yaw}");
        assert!(c.velocity.length() < 0.1, "drifting {}", c.velocity);
        assert!((4000.0..15_000.0).contains(&c.drawn), "draws {} W", c.drawn);
        run(&mut c, &mut t, 1.352, 30.0, |c, _| c.command.up = -1.5);
        assert!(c.grounded, "still {} m up", c.height);
        assert!(c.wrecked.is_none(), "{:?}", c.wrecked);
    }

    /// In a steady wind it holds its place, leaning into it.
    #[test]
    fn holds_station_in_the_wind() {
        let (mut c, mut t) = mars(small());
        c.running = true;
        c.rpm = c.spec.rpm;
        run(&mut c, &mut t, 3.72, 6.0, |c, s| c.command.up = if s < 3.0 { 1.0 } else { 0.0 });
        c.air.wind = Vec3::new(8.0, 0.0, 0.0);
        let x = t.translation.x;
        run(&mut c, &mut t, 3.72, 30.0, |_, _| {});
        assert!(c.velocity.length() < 0.1, "drifting {}", c.velocity);
        assert!((t.translation.x - x).abs() < 3.0, "blown {} m", t.translation.x - x);
        assert!((t.rotation * Vec3::Y).x < -0.001, "not leaning into the wind");
    }
}
