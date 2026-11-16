//! The bodies, in order out from the sun: the order of the number keys.

mod enceladus;
mod europa;
mod io;
mod mars;
mod mercury;
mod moon;
mod phobos;
mod pluto;
mod titan;

use tranquility::prelude::*;

use crate::body::Body;

pub fn all() -> Vec<Body> {
    vec![
        mercury::body(),
        moon::body(),
        mars::body(),
        phobos::body(),
        io::body(),
        europa::body(),
        enceladus::body(),
        titan::body(),
        pluto::body(),
    ]
}

/// Long straight features (ridges, troughs, grooves) come one to a lane:
/// the plane is cut into parallel strips `spacing` wide across the
/// direction `bearing` (radians from north), and each strip holds one
/// feature, somewhere in its middle 40 %. Returns the signed distance from
/// the point to its strip's feature, the distance along it, and the
/// strip's hash. A feature narrower than 0.3 `spacing` stays inside its
/// strip, so strips never seam.
pub fn lane(x: f64, z: f64, bearing: f32, spacing: f64, seed: u32) -> (f32, f64, u32) {
    let (s, c) = (bearing as f64).sin_cos();
    // Along the feature, and across it.
    let (along, across) = (x * s - z * c, x * c + z * s);
    let i = (across / spacing).floor();
    let h = hash(i as i64 as i32, seed as i32, 0x1a9e);
    let centre = (i + 0.3 + 0.4 * (h & 0xffff) as f64 / 65536.0) * spacing;
    ((across - centre) as f32, along, h)
}

/// A random number in 0–1 from a lane's hash.
pub fn roll(h: u32, k: u32) -> f32 {
    (hash(h as i32, k as i32, 7) & 0xffffff) as f32 / 16777216.0
}
