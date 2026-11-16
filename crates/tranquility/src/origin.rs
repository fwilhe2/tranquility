//! A floating origin, so a world can go on for ever.
//!
//! Bevy places things with `f32`, which is good to a millimetre for some
//! kilometres and no further. So what is drawn never strays far from zero:
//! [`Origin`] says where in the world (in `f64`) the renderer's zero is, and
//! whenever the camera has gone a few kilometres from it, the origin jumps
//! to the camera and everything drawn is moved back by the same amount.
//! Nothing shows: the picture depends only on where things are relative to
//! each other.
//!
//! Every entity without a parent is moved (children follow their parents).
//! Anything that keeps positions of its own in render space listens for
//! [`OriginShifted`].

use bevy::{math::DVec2, prelude::*};

/// The origin jumps in whole multiples of this, m. The ground shader's
/// lattices have power-of-two denominators up to this, so the detail it
/// draws comes out the same before and after a jump.
pub const GRAIN: f64 = 256.0;
/// How far the camera may stray from the origin before it jumps, m.
const LEASH: f32 = 3000.0;

/// Where the renderer's zero is in the world, metres (x east, z south).
#[derive(Resource, Clone, Copy, Default, Debug)]
pub struct Origin(pub DVec2);

impl Origin {
    /// World position of a point in render space.
    pub fn world(&self, render: Vec3) -> DVec2 {
        self.0 + DVec2::new(render.x as f64, render.z as f64)
    }

    /// Render-space position of a world point at height `y`.
    pub fn render(&self, world: DVec2, y: f32) -> Vec3 {
        let d = world - self.0;
        Vec3::new(d.x as f32, y, d.y as f32)
    }
}

/// The origin has jumped: every render-space position kept outside a
/// `Transform` must have `delta` subtracted.
#[derive(Message, Clone, Copy)]
pub struct OriginShifted {
    pub delta: Vec3,
}

pub struct OriginPlugin;

impl Plugin for OriginPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Origin>()
            .add_message::<OriginShifted>()
            .add_systems(First, shift);
    }
}

fn shift(
    mut origin: ResMut<Origin>,
    mut roots: Query<&mut Transform, (Without<ChildOf>, Without<Camera3d>)>,
    mut cameras: Query<&mut Transform, (With<Camera3d>, Without<ChildOf>)>,
    mut shifted: MessageWriter<OriginShifted>,
) {
    let Some(at) = cameras.iter().next().map(|t| t.translation) else { return };
    if at.x.abs().max(at.z.abs()) < LEASH {
        return;
    }
    let jump = DVec2::new(
        (at.x as f64 / GRAIN).round() * GRAIN,
        (at.z as f64 / GRAIN).round() * GRAIN,
    );
    origin.0 += jump;
    let delta = Vec3::new(jump.x as f32, 0.0, jump.y as f32);
    for mut t in roots.iter_mut().chain(cameras.iter_mut()) {
        t.translation -= delta;
    }
    shifted.write(OriginShifted { delta });
}
