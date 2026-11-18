//! Rocket landers: a rigid body on one throttled engine, turned by
//! thruster jets, flown by a flight computer, and set down on legs.
//!
//! Nothing is a speed dial. The engine pushes along the craft's axis as
//! hard as the throttle says, within the band it can be throttled in, and
//! burns propellant at thrust over specific impulse (the rocket equation
//! follows: a lighter craft answers the same throttle harder). The jets
//! turn it no faster than their torque allows, and burn propellant of their
//! own. Gravity pulls toward the body's centre; flying across it fast,
//! some of that pull goes into curving the path round the body, so
//! weight falls with the square of the speed across (orbit is where none
//! is left).
//!
//! The flight computer runs one program at a time ([`Program`]):
//! `Guided` steers to a target point, arriving at a given velocity, by
//! commanding the acceleration that cancels the predicted miss in position
//! and velocity at the time left; `Descent` holds a rate of descent with
//! the throttle while the attitude is flown by hand; `Vertical` holds one
//! and takes out the speed across by itself; `Manual` leaves both to the
//! pilot. Attitude is always held by the computer's autopilot
//! against what the pilot commands: in `Guided`, the attitude the thrust
//! needs; otherwise the attitude the hand controller has turned it to.
//!
//! Probes under the feet light a contact light before the feet touch. The
//! legs take a landing up to a vertical and a horizontal speed and a tilt;
//! beyond that the craft is damaged or tips over.

use std::f32::consts::FRAC_PI_2;

use bevy::{math::DVec2, prelude::*};

use crate::{
    origin::Origin,
    vehicle::{Lattice, drawn_height},
    world::WorldDef,
};

const STEP: f32 = 1.0 / 240.0;
/// Standard gravity, for specific impulse.
const G0: f32 = 9.80665;
/// Feet on the ground: stiff, the struts' crushable cartridges giving way
/// under a hard landing. Per unit mass.
const FOOT_K: f32 = 400.0;
const FOOT_C: f32 = 60.0;

#[derive(Clone, Debug)]
pub struct LanderSpec {
    /// Everything but the propellant the engine has left to burn, kg.
    pub dry_mass: f32,
    pub propellant: f32,
    /// Full thrust, N, and specific impulse, s.
    pub thrust: f32,
    pub isp: f32,
    /// The band the throttle works in, as shares of full thrust, and the
    /// fixed full-thrust position above it (the engine is not run between).
    pub throttle: (f32, f32),
    pub full: f32,
    /// Moments of inertia about the body's x (pitch), y (yaw, the thrust
    /// axis) and z (roll), kg m².
    pub inertia: Vec3,
    /// Torque a pair of jets gives about any axis, N m; what such a pair
    /// burns, kg/s; and how much jet propellant there is, kg.
    pub jet_torque: f32,
    pub jet_flow: f32,
    pub jet_propellant: f32,
    /// Footpads' undersides, relative to the centre of mass.
    pub feet: Vec<Vec3>,
    /// Which feet carry contact probes, and how far below the pad they
    /// reach, m.
    pub probes: Vec<usize>,
    pub probe: f32,
    /// Touchdown limits: speed down, speed across, m/s, and tilt, rad.
    pub limits: (f32, f32, f32),
    /// Fastest the hand controller turns it, rad/s, and the autopilot's
    /// bandwidth, rad/s.
    pub max_rate: f32,
    pub agility: f32,
    /// How far from upright the computer will tilt it, rad.
    pub max_tilt: f32,
}

/// What the flight computer is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Program {
    /// Engine off, attitude held.
    Off,
    /// Steering to `target` (render space; the app moves it with the
    /// origin), to arrive with `velocity` in `time` seconds from now.
    Guided { target: Vec3, velocity: Vec3, time: f32 },
    /// Holding a rate of climb (negative: descent), m/s, by throttle.
    Descent { rate: f32 },
    /// Straight down at a rate, the computer tilting to take out any speed
    /// across: an automatic final descent (Apollo's P65).
    Vertical { rate: f32 },
    /// Throttle as the pilot sets it (a share of full thrust).
    Manual { throttle: f32 },
}

#[derive(Component, Clone, Debug)]
pub struct Lander {
    pub spec: LanderSpec,
    pub program: Program,
    /// The hand controller: body rates asked for (pitch, yaw, roll), as
    /// shares of `max_rate`; zero holds the attitude.
    pub stick: Vec3,
    pub velocity: Vec3,
    /// Angular velocity, world frame.
    pub spin: Vec3,
    pub propellant: f32,
    pub jet_propellant: f32,
    /// Thrust now, N, and as a share of full.
    pub thrust: f32,
    pub throttle: f32,
    /// Height of the lowest footpad above the ground under it (the landing
    /// radar's reading), m.
    pub height: f32,
    pub contact: bool,
    pub landed: bool,
    /// What the legs made of it, once down: `Ok(())` within limits, or why
    /// not.
    pub verdict: Option<Result<(), String>>,
    /// Speeds at the first touch, down and across, m/s.
    pub touchdown: Option<(f32, f32)>,
    /// The attitude the autopilot holds.
    hold: Quat,
    started: bool,
    /// Feet on the ground at the last step, and how long it has been
    /// still on them, s.
    touching: usize,
    quiet: f32,
}

impl Lander {
    pub fn new(spec: LanderSpec, program: Program) -> Self {
        Self {
            propellant: spec.propellant,
            jet_propellant: spec.jet_propellant,
            spec,
            program,
            stick: Vec3::ZERO,
            velocity: Vec3::ZERO,
            spin: Vec3::ZERO,
            thrust: 0.0,
            throttle: 0.0,
            height: f32::MAX,
            contact: false,
            landed: false,
            verdict: None,
            touchdown: None,
            hold: Quat::IDENTITY,
            started: false,
            touching: 0,
            quiet: 0.0,
        }
    }

    pub fn mass(&self) -> f32 {
        self.spec.dry_mass + self.propellant
    }

    /// Weight at a gravity, N.
    pub fn weight(&self, g: f32) -> f32 {
        self.mass() * g
    }

    /// How long the propellant left would hold it hovering, s.
    pub fn hover_time(&self, g: f32) -> f32 {
        // Burning at hover thrust, the mass falls exponentially.
        let flow = g / (self.spec.isp * G0);
        if flow <= 0.0 {
            return f32::INFINITY;
        }
        (self.mass() / self.spec.dry_mass).ln() / flow
    }

    /// Effective gravity at a speed across the body's surface: the pull,
    /// less what curving the path round the body takes.
    fn gravity(g: f32, radius: Option<f32>, velocity: Vec3, height: f32) -> f32 {
        match radius {
            Some(r) => {
                let across = velocity.x * velocity.x + velocity.z * velocity.z;
                g - across / (r + height.max(0.0))
            }
            None => g,
        }
    }

    /// The thrust (N, along the craft's axis) the program asks for, and the
    /// direction it wants that axis in.
    fn want(&mut self, pos: Vec3, rot: Quat, g: f32) -> (f32, Option<Vec3>) {
        let up = rot * Vec3::Y;
        let m = self.mass();
        let gravity = Vec3::NEG_Y * g;
        match self.program {
            Program::Off => (0.0, None),
            Program::Manual { throttle } => (throttle * self.spec.thrust, None),
            Program::Descent { rate } => {
                // Up as hard as gravity, plus what closes the rate.
                let a = g + 1.2 * (rate - self.velocity.y);
                (m * a.max(0.0) / up.y.max(0.3), None)
            }
            Program::Vertical { rate } => {
                let across = Vec3::new(self.velocity.x, 0.0, self.velocity.z);
                let push = -0.5 * across + Vec3::Y * (g + 1.2 * (rate - self.velocity.y)).max(0.0);
                let mut axis = push.normalize_or(Vec3::Y);
                let most = 0.3f32;
                if axis.y.clamp(-1.0, 1.0).acos() > most {
                    let side = Vec3::new(axis.x, 0.0, axis.z).normalize_or_zero();
                    axis = Vec3::Y * most.cos() + side * most.sin();
                }
                (m * push.dot(up).max(0.0), Some(axis))
            }
            Program::Guided { target, velocity, time } => {
                let t = time.max(1.0);
                // The miss, coasting from here: in position, and in
                // velocity.
                let miss = target - (pos + self.velocity * t + 0.5 * gravity * t * t);
                let short = velocity - (self.velocity + gravity * t);
                // The thrust's own acceleration (gravity is already in the
                // misses): zero-effort miss and velocity guidance.
                let push = 6.0 * miss / (t * t) - 2.0 * short / t;
                let mut axis = push.normalize_or(Vec3::Y);
                let tilt = axis.y.clamp(-1.0, 1.0).acos();
                if tilt > self.spec.max_tilt {
                    let side = Vec3::new(axis.x, 0.0, axis.z).normalize_or_zero();
                    axis = Vec3::Y * self.spec.max_tilt.cos() + side * self.spec.max_tilt.sin();
                }
                (m * push.dot(up).max(0.0), Some(axis))
            }
        }
    }

    /// The share of full thrust the engine can give for a wish: in the
    /// band, or full; nothing below the band but off.
    fn throttle_for(&self, share: f32) -> f32 {
        let (low, high) = self.spec.throttle;
        if share <= 0.0 {
            0.0
        } else if share <= high {
            share.max(low)
        } else {
            self.spec.full
        }
    }

    fn substep(&mut self, transform: &mut Transform, g: f32, radius: Option<f32>, h: f32, ground: &mut dyn FnMut(Vec3) -> f32) {
        let s = self.spec.clone();
        let rot = transform.rotation;
        let pos = transform.translation;
        if !self.started {
            self.started = true;
            self.hold = rot;
        }
        let g_eff = Self::gravity(g, radius, self.velocity, self.height);
        // The program: thrust, and where it wants the axis.
        let (wish, axis) = if self.landed { (0.0, None) } else { self.want(pos, rot, g_eff) };
        let share = if self.propellant > 0.0 { self.throttle_for(wish / s.thrust) } else { 0.0 };
        self.throttle = share;
        self.thrust = share * s.thrust;
        // Attitude: the program's axis, keeping the heading; or the hand
        // controller's rates, turning the held attitude.
        if let Some(axis) = axis {
            let ahead = rot * Vec3::NEG_Z;
            let heading = (-ahead.x).atan2(-ahead.z);
            self.hold = Quat::from_rotation_arc(Vec3::Y, axis) * Quat::from_rotation_y(heading);
        } else if self.stick != Vec3::ZERO && !self.landed {
            let rate = self.stick.clamp(Vec3::splat(-1.0), Vec3::splat(1.0)) * s.max_rate;
            self.hold = (self.hold * Quat::from_scaled_axis(rate * h)).normalize();
        }
        let mut torque = Vec3::ZERO;
        let mut jets = 0.0;
        // On its feet with the engine off, the crew safe the jets: holding
        // an attitude against the ground would only rock it.
        let grounded = self.touching > 0 && self.thrust == 0.0;
        if !self.landed && !grounded && self.jet_propellant > 0.0 {
            let mut error = self.hold * rot.inverse();
            if error.w < 0.0 {
                error = -error;
            }
            let (e_axis, angle) = error.to_axis_angle();
            let e = if angle.is_finite() { e_axis * angle } else { Vec3::ZERO };
            let w = s.agility;
            // Rate toward the held attitude, no faster than the jets can
            // stop it again in the angle left; then the jets at what that
            // rate needs.
            let most = (s.jet_torque / s.inertia.max_element()).max(1e-4);
            let stop = (2.0 * 0.8 * most * e.length()).sqrt();
            let wanted = (e * w).clamp_length_max(s.max_rate.min(stop));
            let accel = (wanted - self.spin) * 2.0 * w;
            let body = rot.inverse() * accel * s.inertia;
            let jet = body.clamp(Vec3::splat(-s.jet_torque), Vec3::splat(s.jet_torque));
            jets = (jet.x.abs() + jet.y.abs() + jet.z.abs()) / s.jet_torque;
            torque += rot * jet;
        }
        // Propellant.
        self.propellant = (self.propellant - self.thrust / (s.isp * G0) * h).max(0.0);
        self.jet_propellant = (self.jet_propellant - jets * s.jet_flow * h).max(0.0);
        let m = self.mass();
        let up = rot * Vec3::Y;
        let mut force = up * self.thrust + Vec3::NEG_Y * m * g_eff;
        // Feet and probes.
        let share = m / s.feet.len().max(1) as f32;
        let mut lowest = f32::MAX;
        let mut touching = 0;
        for (k, foot) in s.feet.iter().enumerate() {
            let arm = rot * *foot;
            let p = pos + arm;
            let floor = ground(p);
            let depth = floor - p.y;
            lowest = lowest.min(-depth);
            if s.probes.contains(&k) && !self.contact {
                let tip = p - up * s.probe;
                if ground(tip) >= tip.y {
                    self.contact = true;
                }
            }
            if depth <= 0.0 {
                continue;
            }
            touching += 1;
            let v = self.velocity + self.spin.cross(arm);
            if self.touchdown.is_none() {
                let across = Vec2::new(v.x, v.z).length();
                self.touchdown = Some((-v.y, across));
            }
            let push = (FOOT_K * depth - FOOT_C * v.y).max(0.0) * share;
            let slide = Vec3::new(v.x, 0.0, v.z);
            let hold = (slide * share * 0.5 / h).clamp_length_max(0.8 * push);
            let f = Vec3::Y * push - hold;
            force += f;
            torque += arm.cross(f);
        }
        self.height = lowest.max(0.0);
        self.touching = touching;
        // Down: still on its feet a moment, the engine off.
        let still = self.velocity.length() < 0.1 && self.spin.length() < 0.05;
        self.quiet = if touching >= 3 && still && self.thrust == 0.0 { self.quiet + h } else { 0.0 };
        if self.quiet > 0.5 && self.verdict.is_none() {
            self.landed = true;
            let tilt = up.y.clamp(-1.0, 1.0).acos();
            let (down, across) = self.touchdown.unwrap_or((0.0, 0.0));
            let (v_max, h_max, t_max) = s.limits;
            self.verdict = Some(if down > v_max {
                Err(format!("came down at {down:.1} m/s, past the legs' {v_max:.1}: the struts crushed through"))
            } else if across > h_max {
                Err(format!("touched down moving {across:.1} m/s across, past the legs' {h_max:.1}"))
            } else if tilt > t_max {
                Err(format!("stands {:.0}° from upright, past the {:.0}° it can lift off from", tilt.to_degrees(), t_max.to_degrees()))
            } else {
                Ok(())
            });
        }
        if up.y < (FRAC_PI_2 - 0.9).sin() && touching > 0 && self.verdict.is_none() {
            self.verdict = Some(Err("tipped over".into()));
        }
        // Integrate.
        self.velocity += force / m * h;
        let body = rot.inverse() * self.spin;
        let gyro = body.cross(s.inertia * body);
        self.spin = rot * (body + (rot.inverse() * torque - gyro) / s.inertia * h);
        if self.landed {
            self.velocity = Vec3::ZERO;
            self.spin = Vec3::ZERO;
            return;
        }
        transform.translation += self.velocity * h;
        transform.rotation = (Quat::from_scaled_axis(self.spin * h) * rot).normalize();
    }
}

pub struct RocketPlugin;

/// Where landers move: an app's controls run before, its drawing after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Burn;

impl Plugin for RocketPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, burn.in_set(Burn));
    }
}

fn burn(time: Res<Time>, world: Res<WorldDef>, origin: Res<Origin>, mut landers: Query<(&mut Lander, &mut Transform)>, mut lattice: Local<Lattice>) {
    if world.is_changed() {
        lattice.clear();
    }
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }
    let ground = &*world.ground;
    let step = world.terrain.step(0) as f64;
    let g = world.walker.gravity;
    let radius = world.terrain.radius;
    for (mut lander, mut transform) in &mut landers {
        if lander.landed {
            continue;
        }
        let n = (dt / STEP).ceil().max(1.0);
        let h = dt / n;
        for _ in 0..n as usize {
            let mut height = |p: Vec3| {
                let at: DVec2 = origin.world(p);
                drawn_height(ground, step, at.x, at.y, &mut lattice)
            };
            if let Program::Guided { time, .. } = &mut lander.program {
                *time -= h;
            }
            lander.substep(&mut transform, g, radius, h, &mut height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOON: f32 = 1.62;

    /// The Apollo lunar module at high gate, near enough: the ascent
    /// stage and the empty descent stage, and what is left to burn.
    fn lm() -> LanderSpec {
        let foot = |x: f32, z: f32| Vec3::new(x, -3.0, z);
        LanderSpec {
            dry_mass: 6730.0,
            propellant: 1150.0,
            thrust: 45_040.0,
            isp: 311.0,
            throttle: (0.10, 0.60),
            full: 0.925,
            inertia: Vec3::new(40_000.0, 22_000.0, 40_000.0),
            jet_torque: 2940.0,
            jet_flow: 0.62,
            jet_propellant: 280.0,
            feet: vec![foot(0.0, -4.7), foot(4.7, 0.0), foot(0.0, 4.7), foot(-4.7, 0.0)],
            probes: vec![1, 2, 3],
            probe: 1.73,
            limits: (3.0, 1.2, 0.21),
            max_rate: 0.17,
            agility: 1.2,
            max_tilt: 0.8,
        }
    }

    fn run(l: &mut Lander, t: &mut Transform, seconds: f32, mut each: impl FnMut(&mut Lander, &Transform, f32)) {
        let mut ground = |_: Vec3| 0.0;
        for k in 0..(seconds / STEP) as usize {
            each(l, t, k as f32 * STEP);
            if let Program::Guided { time, .. } = &mut l.program {
                *time -= STEP;
            }
            l.substep(t, MOON, Some(1_737_400.0), STEP, &mut ground);
        }
    }

    /// Holding zero rate it hovers, burning propellant at weight over
    /// specific impulse.
    #[test]
    fn hovers_on_the_rocket_equation() {
        let mut l = Lander::new(lm(), Program::Descent { rate: 0.0 });
        let mut t = Transform::from_xyz(0.0, 100.0, 0.0);
        let before = l.propellant;
        run(&mut l, &mut t, 20.0, |_, _, _| {});
        assert!((t.translation.y - 100.0).abs() < 1.0, "at {}", t.translation.y);
        let burnt = before - l.propellant;
        // About weight / (Isp g0) for 20 s.
        let expected = (6730.0 + 1150.0) * MOON / (311.0 * G0) * 20.0;
        assert!((burnt - expected).abs() < 0.05 * expected, "{burnt} kg, expected {expected}");
        assert!((0.25..0.35).contains(&l.throttle), "throttle {}", l.throttle);
    }

    /// From high gate the computer flies it to a point above the target,
    /// and a held descent rate sets it down within limits, with contact
    /// light first.
    #[test]
    fn flies_the_approach_and_lands() {
        let target = Vec3::new(0.0, 30.0, 0.0);
        let program = Program::Guided { target, velocity: Vec3::new(0.0, -1.0, 0.0), time: 110.0 };
        let mut l = Lander::new(lm(), program);
        l.velocity = Vec3::new(-140.0, -40.0, 0.0);
        // Coming in from the east, its windows toward the west, pitched
        // back against the way it goes.
        let back = Quat::from_rotation_arc(Vec3::Y, Vec3::new(0.7, 0.71, 0.0).normalize());
        let mut t = Transform::from_xyz(7400.0, 2100.0, 0.0).with_rotation(back * Quat::from_rotation_y(FRAC_PI_2));
        // The computer hands over a few seconds out, as at low gate.
        run(&mut l, &mut t, 105.0, |_, _, _| {});
        let miss = Vec2::new(t.translation.x, t.translation.z).length();
        assert!(miss < 30.0, "{miss} m from the target, at {:?}", t.translation);
        assert!(l.velocity.length() < 8.0, "still moving {:?}", l.velocity);
        l.program = Program::Vertical { rate: -0.9 };
        let mut lit = None;
        run(&mut l, &mut t, 80.0, |l, _, s| {
            if l.contact && lit.is_none() {
                lit = Some(s);
                l.program = Program::Off;
            }
        });
        assert!(lit.is_some(), "no contact light");
        assert!(l.landed, "not down: {} m up", l.height);
        assert_eq!(l.verdict, Some(Ok(())), "{:?}", l.touchdown);
        assert!(l.propellant > 0.0, "ran dry");
    }

    /// Out of propellant high up, it falls, and the legs cannot take it.
    #[test]
    fn running_dry_is_a_crash() {
        let mut l = Lander::new(lm(), Program::Descent { rate: 0.0 });
        l.propellant = 5.0;
        let mut t = Transform::from_xyz(0.0, 40.0, 0.0);
        run(&mut l, &mut t, 30.0, |_, _, _| {});
        assert!(matches!(l.verdict, Some(Err(_))), "{:?}", l.verdict);
    }

    /// The hand controller turns it at the rate asked, and letting go holds
    /// the attitude it reached.
    #[test]
    fn the_stick_turns_it_and_letting_go_holds() {
        let mut l = Lander::new(lm(), Program::Descent { rate: 0.0 });
        let mut t = Transform::from_xyz(0.0, 200.0, 0.0);
        run(&mut l, &mut t, 6.0, |l, _, s| l.stick = if s < 3.0 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::ZERO });
        let tilt = (t.rotation * Vec3::Y).y.acos();
        assert!((0.3..0.6).contains(&tilt), "tilted {tilt}");
        run(&mut l, &mut t, 5.0, |_, _, _| {});
        let later = (t.rotation * Vec3::Y).y.acos();
        assert!((later - tilt).abs() < 0.05, "drifted from {tilt} to {later}");
    }

    /// Flying across fast, it weighs less: at orbital speed, nothing.
    #[test]
    fn speed_across_lightens_it() {
        let orbital = (MOON * 1_737_400.0f32).sqrt();
        assert!(Lander::gravity(MOON, Some(1_737_400.0), Vec3::new(orbital, 0.0, 0.0), 0.0).abs() < 1e-3);
        assert_eq!(Lander::gravity(MOON, None, Vec3::new(orbital, 0.0, 0.0), 0.0), MOON);
    }
}
