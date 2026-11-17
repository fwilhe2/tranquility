//! Titan: the shore of a methane sea, under a bar and a half of nitrogen
//! and a haze three hundred kilometres deep. The haze absorbs blue and
//! scatters the rest, so what reaches the ground is orange, nearly all of
//! it from the whole sky, and a thousandth of Earth's daylight: Huygens
//! carried a lamp for colour, not for light. The sun is a brighter patch
//! of sky; nothing casts a shadow with an edge.
//!
//! The ground is what Huygens landed on: a plain of dark organic sand
//! damp with methane, strewn with cobbles of water ice rounded by flash
//! floods, cut by the dry channels that brought them; bright icy uplands
//! behind; and in front, dead calm, the sea.

use std::sync::Arc;

use bevy::{
    curve::{FunctionCurve, Interval},
    light::atmosphere::{Falloff, PhaseFunction, ScatteringMedium, ScatteringTerm},
};
use tranquility::{
    noise::{fbm, fbm_to, ridged, smoothstep},
    prelude::*,
};

use crate::body::{Body, Land, Rocks, level_site};

struct Titan;

impl Titan {
    /// How deep a channel is cut here, 0–1: a branching net of valleys.
    fn channel(&self, x: f64, z: f64) -> f32 {
        let warp = 300.0 * fbm(x, z, 1500.0, 3, 0.5, 161) as f64;
        let net = ridged(x + warp, z - warp, 2600.0, 3, 162);
        smoothstep(0.7, 0.9, net)
    }

    /// 0 on the plain, 1 in the uplands to the south.
    fn upland(&self, x: f64, z: f64) -> f32 {
        smoothstep(-0.1, 0.25, fbm(x, z, 9000.0, 4, 0.5, 163) + (z as f32 - 3500.0) / 9000.0)
    }
}

impl Ground for Titan {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        // The coast: land rising to the south, a sea to the north whose
        // shore winds in bays and headlands. Height 0 is the sea's level.
        let coast = (z as f32 + 700.0) / 260.0 + 9.0 * fbm_to(x, z, 5000.0, 5, 0.5, 164, step);
        let plain = if coast > 0.0 { 3.0 * coast.min(6.0) + 0.6 * (coast - 6.0).max(0.0) } else { 2.0 * coast.max(-30.0) };
        let upland = self.upland(x, z);
        let hills = upland * (160.0 + 220.0 * fbm_to(x, z, 6000.0, 6, 0.5, 165, step).abs());
        // Channels cut the uplands deeply and the plain a little.
        let cut = self.channel(x, z) * (2.5 + 60.0 * upland) * smoothstep(0.0, 2.0, coast);
        plain + hills - cut + 0.6 * fbm_to(x, z, 120.0, 4, 0.5, 166, step)
    }

    fn tint(&self, x: f64, z: f64, height: f32, _normal: Vec3) -> LinearRgba {
        // Dark organic sand; the uplands are water ice under a thinner
        // coat of it, and brighter. Wet ground by the sea is darker.
        let sand = Vec3::new(0.17, 0.135, 0.1);
        let ice = Vec3::new(0.4, 0.33, 0.25);
        let mut c = sand.lerp(ice, self.upland(x, z) * (0.6 + 0.4 * fbm(x, z, 700.0, 4, 0.5, 167)));
        c = c.lerp(Vec3::new(0.3, 0.25, 0.18), 0.6 * self.channel(x, z));
        let wet = 1.0 - smoothstep(0.0, 1.2, height);
        c *= 1.0 - 0.45 * wet;
        let c = c * (1.0 + 0.1 * fbm(x, z, 45.0, 3, 0.5, 168));
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }

    fn blocked(&self, _x: f64, _z: f64) -> bool {
        false
    }
}

impl Land for Titan {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        // Cobbles lie where the floods dropped them: in and beside the
        // channels, and along the strand.
        (0.25 + 0.75 * self.channel(x, z)).min(1.0)
    }

    fn landing_site(&self) -> (f64, f64) {
        // On the plain, a few hundred metres back from the water.
        level_site(self, 0.0, -150.0, 0.35)
    }
}

/// Nitrogen four and a half times as dense as Earth's air at the ground,
/// thinning slowly in the weak gravity; and far above, the haze: tholin
/// aerosols that scatter forward and absorb more the bluer the light,
/// optical depth about 3 in red and 6 in blue. The haze is made high up
/// and most of it stays there: the lowest tens of kilometres are clear
/// enough that Huygens saw the ground sharply from eight kilometres.
fn air() -> Air {
    const HEIGHT: f32 = 300_000.0;
    let haze = Falloff::from_curve(FunctionCurve::new(Interval::UNIT, |p: f32| {
        let h = (1.0 - p) * HEIGHT;
        let low = 0.12 + 0.88 * tranquility::noise::smoothstep(30_000.0, 80_000.0, h);
        low * (-(h - 100_000.0).max(0.0) / 50_000.0).exp()
    }));
    Air::Medium(Arc::new(AirDef {
        radius: 2_574_700.0,
        height: HEIGHT,
        ground_albedo: Vec3::new(0.2, 0.13, 0.07),
        medium: ScatteringMedium::new(
            256,
            256,
            [
                ScatteringTerm {
                    absorption: Vec3::ZERO,
                    scattering: Vec3::new(2.6e-5, 6.1e-5, 1.49e-4),
                    falloff: Falloff::Exponential { scale: 20_000.0 / HEIGHT },
                    phase: PhaseFunction::Rayleigh,
                },
                ScatteringTerm {
                    absorption: Vec3::new(2.0e-6, 8.0e-6, 3.0e-5),
                    scattering: Vec3::new(3.0e-5, 2.8e-5, 2.2e-5),
                    falloff: haze,
                    phase: PhaseFunction::Mie { asymmetry: 0.65 },
                },
            ],
        )
        .with_label("titan_air"),
        thick: true,
    }))
}

pub fn body() -> Body {
    Body {
        note: "The sea is liquid methane, and the cobbles are water ice. In air this thick and gravity this weak, a jump is a slow fall.",
        gravity: 1.352,
        radius: 2_574_700.0,
        au: 9.58,
        albedo: 0.18,
        grip: 0.6,
        // Damp sediment, sorted by floods: smooth, no pits, many stones;
        // no opposition surge under light from the whole sky.
        soil: Soil { craterlets: 0.0, grain: 0.55, ripples: 0.25, pebbles: 1.0, pebble_tone: 1.6, sparkle: 0.0, powder: 0.25, surge: 0.0, surge_width: 0.1, backscatter: 0.0 },
        // Matte: the sky's mirror image in damp ground is where the thick
        // air's lookup tables show their seams.
        roughness: 1.0,
        reflectance: 0.05,
        air: air(),
        // ½ρC_dA/m for a suited walker in 5.4 kg/m³ of nitrogen.
        drag: 0.0124,
        // Huygens measured a few tenths of a metre a second at the ground.
        air_density: 5.4,
        wind: 0.4,
        sun: (130.0, 28.0),
        stars: 0.0,
        // Nearly all of Titan's light comes from the orange sky, not the
        // sun: matched to how Bevy's atmosphere lit it (`sky_light`).
        fill: 0.8,
        fill_color: Color::linear_rgb(1.0, 0.36, 0.1),
        exposure_bias: -2.6,
        sea: Some(WaterSettings { level: 0.0, color: Color::linear_rgb(0.012, 0.008, 0.004), roughness: 0.25 }),
        rocks: Rocks { density: 0.25, largest: 0.7, albedo: [0.3, 0.5], tint: [1.0, 0.9, 0.78], roughness: 0.8, rounded: true },
        sun_disc: false,
        look: (0.35, -0.04),
        ..Body::airless("Titan", "Shore of a methane sea", Arc::new(Titan))
    }
}
