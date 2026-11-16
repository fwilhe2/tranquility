//! When the feet come down. The engine moves the walker at the pace legs
//! and grip allow; this counts the steps on the way, so that each can leave
//! a print, make its sound and swing the figure's legs.
//!
//! Step length follows Alexander's dynamic similarity, a stride of
//! 2.3 L Fr^0.3 (Alexander 1976), so it scales with gravity through the
//! Froude number and needs nothing per body. The suit takes about 30 % off
//! it: pose estimation of Apollo video found 0.39 m steps at 0.42 m/s,
//! against 0.55 m unsuited at the same Froude number.

use tranquility::prelude::*;

/// m, leg length (as in `suit`).
const LEG: f32 = 0.9;
/// Suited step length over unsuited.
const SUIT: f32 = 0.71;

/// A boot has come down.
#[derive(Message, Clone, Copy)]
pub struct Footfall {
    /// Where, in render space, at the height of the feet.
    pub at: Vec3,
    pub left: bool,
    /// The way the walker is going (not the way they look), as a yaw.
    pub heading: f32,
    /// How hard, about 0–1: a third at a walk, more at a lope or a landing.
    pub strength: f32,
}

/// Where the walker is in their stride.
#[derive(Resource, Default)]
pub struct Gait {
    /// 0–1 through the step now being taken.
    pub phase: f32,
    /// Which foot comes down next.
    pub left: bool,
    /// 0 standing, 1 in full stride; eased, for anything that swings.
    pub swing: f32,
    /// The way the walker is going, as a yaw; kept while standing.
    pub heading: f32,
    /// m walked since the last footfall.
    walked: f32,
    aloft: bool,
    falling: f32,
}

/// Length of a step at a pace under a gravity, m.
pub fn step_length(pace: f32, gravity: f32) -> f32 {
    let froude = (pace * pace / (gravity * LEG)).max(1e-4);
    (0.5 * SUIT * 2.3 * LEG * froude.powf(0.3)).max(0.2)
}

/// The system that counts the steps: what uses them runs after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Striding;

pub struct GaitPlugin;

impl Plugin for GaitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Gait>().add_message::<Footfall>().add_systems(Update, stride.after(Walking).in_set(Striding));
    }
}

fn stride(time: Res<Time>, world: Res<WorldDef>, player: Single<(&Player, &Transform)>, mut gait: ResMut<Gait>, mut falls: MessageWriter<Footfall>) {
    let (player, eye) = player.into_inner();
    let dt = time.delta_secs().min(0.1);
    if world.is_changed() {
        *gait = Gait::default();
    }
    let on_foot = player.controlled && !player.flying;
    let flat = Vec2::new(player.velocity.x, player.velocity.z);
    let pace = flat.length();
    let at = Vec3::new(eye.translation.x, player.feet, eye.translation.z);
    let walking = on_foot && player.grounded && pace > 0.05;
    if pace > 0.05 {
        gait.heading = (-flat.x).atan2(-flat.y);
    }
    let heading = gait.heading;
    if on_foot && player.grounded && gait.aloft && gait.falling < -0.3 {
        // Both feet, from a jump or a drop.
        let strength = (0.25 - 0.35 * gait.falling).min(1.2);
        falls.write(Footfall { at, left: gait.left, heading, strength });
        gait.walked = 0.0;
    }
    gait.aloft = on_foot && !player.grounded;
    gait.falling = player.velocity.y;
    let goal = if walking { 1.0 } else { 0.0 };
    gait.swing += (goal - gait.swing) * (1.0 - (-dt / 0.3).exp());
    if !walking {
        return;
    }
    let step = step_length(pace, world.walker.gravity);
    gait.walked += pace * dt;
    if gait.walked >= step {
        gait.walked -= step;
        // A lope lands harder than a walk.
        let strength = 0.3 + 0.25 * ((pace - world.walker.walk) / (world.walker.run - world.walker.walk).max(0.01)).clamp(0.0, 1.5);
        let left = gait.left;
        falls.write(Footfall { at, left, heading, strength });
        gait.left = !left;
    }
    gait.phase = (gait.walked / step).clamp(0.0, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lunar_walk_takes_steps_of_half_a_metre() {
        let pace = (0.36f32 * 1.62 * LEG).sqrt();
        assert!((0.45..0.6).contains(&step_length(pace, 1.62)));
        // Apollo video: 0.39 m at 0.42 m/s.
        assert!((0.33..0.45).contains(&step_length(0.42, 1.62)));
    }
}
