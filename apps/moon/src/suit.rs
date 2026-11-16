//! The suited body that walks on these worlds, after lunar-walk: nothing
//! about moving on foot is tuned for feel. You are an 80 kg person in 91 kg
//! of suit and backpack, and everything follows from that mass, the local
//! gravity, the grip of the ground, and what a pair of legs can do.
//!
//! - **Pace.** Legs are pendulums, so natural speeds scale with the Froude
//!   number v²/gL. Suited Apollo crews changed from walking to loping at
//!   Fr 0.36 (Carr & McGee 2009) and loped for long stretches at Fr 1.35,
//!   which is 1.4 m/s on the Moon. The same two numbers set the pace
//!   everywhere: 2.1 m/s on Mars, 0.9 on Pluto, 0.37 on Enceladus.
//! - **Grip.** Horizontal force comes from friction, so no gait can speed
//!   you up, slow you down or hold you on a slope harder than μ·g. On the
//!   Moon that is 1 m/s²: three strides to stop. On Phobos it is 4 mm/s²,
//!   and walking is not a thing you can do there: the suit carries jets.
//! - **Jumping.** Leg force falls off linearly with extension speed (the
//!   force–velocity profile of jumping, Samozino et al. 2008): 36 N per
//!   kilogram of body at rest, nothing left at 4 m/s. The suit's joints
//!   take 42 % of it and its knees allow a 0.25 m crouch, which puts a
//!   full jump on the Moon at 0.8 m — Charlie Duke's, on Apollo 16.
//! - **In flight** you are a projectile: nothing you do with your legs
//!   changes where you land.

use tranquility::prelude::*;

/// kg, person and suit.
const MASS: f32 = 171.0;
/// N, leg-extension force at zero speed (36 N per kg of an 80 kg body).
const LEG_FORCE: f32 = 36.0 * 80.0;
/// m/s, extension speed at which the legs stop pushing.
const LEG_SPEED: f32 = 4.0;
/// Share of leg force the suit's joints leave you.
const SUIT_SHARE: f32 = 0.58;
/// m, deepest crouch the suit allows.
const CROUCH: f32 = 0.25;
/// m, leg length.
const LEG: f32 = 0.9;
const FROUDE_WALK: f32 = 0.36;
const FROUDE_LOPE: f32 = 1.35;
/// m/s², the Manned Maneuvering Unit's: 0.3 ft/s² in any axis.
const JET: f32 = 0.091;
/// Below this gravity a jump lasts most of a minute and a stride goes
/// nowhere: the suit gets jets.
const JET_BELOW: f32 = 0.3;

/// Take-off speed of a push at `effort` (0–1) under gravity `g`: the legs'
/// force–velocity curve integrated through the crouch.
pub fn jump_speed(effort: f32, g: f32) -> f32 {
    let (mut u, mut x) = (0.0f32, 0.0f32);
    let h = 0.0005;
    for _ in 0..4000 {
        if x >= CROUCH {
            break;
        }
        let a = effort * SUIT_SHARE * LEG_FORCE * (1.0 - u / LEG_SPEED).max(0.0) / MASS - g;
        if u <= 0.0 && a <= 0.0 {
            return 0.0;
        }
        u = (u + a * h).max(0.0);
        x += u * h;
    }
    u
}

pub fn walker(gravity: f32, grip: f32, drag: f32) -> WalkerSettings {
    WalkerSettings {
        eye_height: 1.62,
        gravity,
        jump: jump_speed(0.85, gravity),
        walk: (FROUDE_WALK * gravity * LEG).sqrt(),
        run: (FROUDE_LOPE * gravity * LEG).sqrt(),
        fly: 60.0,
        fly_fast: 900.0,
        radius: 0.3,
        walk_on_water: false,
        grip: grip * gravity,
        air_control: false,
        step_down: 0.0,
        drag,
        jets: if gravity < JET_BELOW { JET } else { 0.0 },
        // The ground is all small craters: let the eye glide over them.
        eye_smoothing: 0.35,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_jump_on_the_moon_is_dukes() {
        let v = jump_speed(1.0, 1.62);
        let height = v * v / (2.0 * 1.62);
        assert!((0.7..0.95).contains(&height), "{height} m");
    }

    #[test]
    fn nobody_jumps_in_a_suit_on_earth() {
        let v = jump_speed(1.0, 9.81);
        assert!(v * v / (2.0 * 9.81) < 0.05);
    }
}
