//! Mars: a dusty plain among mesas, under six millibars of carbon dioxide
//! that is enough to hold dust, and dust is what you see. The sky is
//! butterscotch, the shadows are filled, and the glow round the sun is
//! blue, forward-scattered by the same dust.
//!
//! The ground is what the landers found: bright dust over darker soil,
//! strewn with basalt blocks (a tenth of the surface at the Viking sites),
//! with dark sand gathered into ripples and dunes in the low ground, and
//! layered buttes standing where the plain has been stripped back.

use std::sync::Arc;

use bevy::{
    light::atmosphere::{Falloff, PhaseFunction, ScatteringMedium, ScatteringTerm},
    curve::{FunctionCurve, Interval},
};
use tranquility::{
    noise::{fbm, fbm_to, smoothstep},
    prelude::*,
};

use crate::{
    body::{Body, Land, Rocks},
    craters::{Craters, Population},
    heavens::{Companion, Map},
    rotorcraft::Craft,
};

struct Mars {
    craters: Craters,
}

impl Mars {
    /// 0 on the plain, 1 on a mesa's top.
    fn mesa(&self, x: f64, z: f64, step: f32) -> f32 {
        let m = fbm_to(x, z, 7000.0, 6, 0.5, 91, step) + 0.25 * fbm_to(x, z, 30_000.0, 3, 0.5, 92, step);
        smoothstep(0.2, 0.34, m)
    }

    /// How much dark sand has gathered here, 0–1: in the lows.
    fn sand(&self, x: f64, z: f64) -> f32 {
        smoothstep(0.08, 0.3, fbm(x, z, 2200.0, 4, 0.5, 93))
    }

    /// Dunes where the sand lies: transverse ridges 70 m apart and a few
    /// metres high, gentle to the wind and steep in its lee, their crests
    /// wandering and merging.
    fn dunes(&self, x: f64, z: f64, step: f32) -> f32 {
        if step > 16.0 {
            return 0.0;
        }
        let sand = self.sand(x, z);
        if sand <= 0.0 {
            return 0.0;
        }
        let warp = 1.4 * fbm(x, z, 260.0, 3, 0.5, 94) as f64;
        let phase = (x / 70.0 + z / 500.0 + warp).rem_euclid(1.0) as f32;
        // Windward slope over 70 % of the wavelength, slip face over the rest.
        let saw = if phase < 0.7 { phase / 0.7 } else { (1.0 - phase) / 0.3 };
        let crest = saw * saw * (3.0 - 2.0 * saw);
        sand * (4.5 + 2.0 * fbm(x, z, 900.0, 2, 0.5, 95)) * crest
    }
}

impl Ground for Mars {
    fn height(&self, x: f64, z: f64) -> f32 {
        self.height_at(x, z, 0.0)
    }

    fn height_at(&self, x: f64, z: f64, step: f32) -> f32 {
        let regional = 160.0 * fbm_to(x, z, 28_000.0, 5, 0.5, 96, step);
        let rolling = 7.0 * fbm_to(x, z, 600.0, 5, 0.5, 97, step);
        // Mesas: cliffs over talus, with benches where harder beds crop out.
        let t = self.mesa(x, z, step);
        let benches = t + 0.035 * (t * 5.0 * std::f32::consts::TAU).sin() * (t * (1.0 - t) * 4.0);
        let mesa = (230.0 + 90.0 * fbm_to(x, z, 9000.0, 3, 0.5, 98, step)) * benches;
        regional + rolling + mesa + self.dunes(x, z, step) + self.craters.height(x, z, step, f32::MAX, &|_, _| true)
    }

    fn tint(&self, x: f64, z: f64, _height: f32, normal: Vec3) -> LinearRgba {
        let t = self.mesa(x, z, 0.0);
        let sand = self.sand(x, z) * (1.0 - smoothstep(0.02, 0.2, t));
        // Bright dust, a film of it over everything; darker, duskier soil
        // where the wind has stripped it; dark basalt sand in the dunes.
        let dust = Vec3::new(0.34, 0.185, 0.095);
        let soil = Vec3::new(0.2, 0.115, 0.065);
        let basalt = Vec3::new(0.075, 0.055, 0.045);
        let stripped = smoothstep(-0.1, 0.3, fbm(x, z, 350.0, 4, 0.55, 99));
        let mut c = dust.lerp(soil, 0.7 * stripped);
        c = c.lerp(basalt, 0.85 * sand);
        // Cliffs: layered rock, paler and darker beds by the height in the
        // wall, shedding dust they cannot hold.
        let slope = smoothstep(0.08, 0.35, 1.0 - normal.y);
        let bed = 0.75 + 0.35 * (t * 31.0).sin() * (t * 77.0).cos();
        c = c.lerp(Vec3::new(0.24, 0.14, 0.085) * bed, slope);
        let mottle = 1.0 + 0.08 * fbm(x, z, 60.0, 3, 0.5, 100);
        let c = c * mottle;
        Pattern::Regolith.on(LinearRgba::rgb(c.x, c.y, c.z))
    }
}

impl Land for Mars {
    fn rockiness(&self, x: f64, z: f64) -> f32 {
        // Rock fields in patches, few rocks on the dunes, many below
        // the cliffs.
        let field = smoothstep(-0.25, 0.35, fbm(x, z, 500.0, 4, 0.5, 101));
        let t = self.mesa(x, z, 0.0);
        let talus = (t * (1.0 - t) * 4.0).powi(2);
        ((0.1 + 0.5 * field) * (1.0 - 0.9 * self.sand(x, z)) + talus + self.craters.blocks(x, z)).min(1.0)
    }
}

/// The Martian air after Bevy's own (Schneegans et al. 2024): carbon
/// dioxide, and dust in two layers. Dust scatters red light better than it
/// absorbs it and blue the other way round, which makes the sky
/// butterscotch; and it throws blue forward in a tighter cone than red,
/// which makes the glow round the sun, and the sunset, blue.
fn air() -> Air {
    const HEIGHT: f32 = 120_000.0;
    let dust_falloff = Falloff::from_curve(FunctionCurve::new(Interval::UNIT, |p: f32| {
        let h = (1.0 - p) * HEIGHT;
        0.75 * (1.0 - (h / 4_000.0).exp()).exp() + 0.25 * (1.0 - (h / 20_000.0).exp()).exp()
    }));
    let lobe = |g: f32, c: f32| (1.0 - g * g) / (4.0 * std::f32::consts::PI * (1.0 + g * g - 2.0 * g * c).powf(1.5));
    let phase = FunctionCurve::new(Interval::new(-1.0, 1.0).expect("a valid interval"), move |c: f32| {
        let back = 0.12 * lobe(-0.25, c);
        LinearRgba::rgb(0.88 * lobe(0.62, c) + back, 0.88 * lobe(0.7, c) + back, 0.88 * lobe(0.8, c) + back)
    });
    Air::Medium(Arc::new(AirDef {
        radius: 3_389_500.0,
        height: HEIGHT,
        ground_albedo: Vec3::new(0.3, 0.17, 0.09),
        medium: ScatteringMedium::new(
            256,
            256,
            [
                ScatteringTerm {
                    absorption: Vec3::ZERO,
                    scattering: Vec3::new(9.91e-8, 2.32e-7, 5.65e-7),
                    falloff: Falloff::Exponential { scale: 8_000.0 / HEIGHT },
                    phase: PhaseFunction::Rayleigh,
                },
                ScatteringTerm {
                    absorption: Vec3::new(1.26e-6, 5.25e-6, 9.33e-6),
                    scattering: Vec3::new(30.67e-6, 25.39e-6, 20.93e-6),
                    falloff: dust_falloff,
                    phase: PhaseFunction::from_chromatic_curve(phase),
                },
            ],
        )
        .with_label("mars_air"),
        thick: false,
    }))
}

pub fn body() -> Body {
    let land = Mars {
        // Air, wind and dust wear small craters away: a fifth of the
        // lunar count, shallow, their floors drifted in.
        craters: Craters::new(Population { d_min: 4.0, d_max: 4000.0, density: 0.2, saturated: 60.0, depth: 0.14, complex: 7000.0, seed: 1976 }),
    };
    Body {
        note: "The dust in the air lights the shadows and hides the stars. Look toward the sun: the glow round it is blue.",
        facts: &[
            ("Day", "A sol is 24 h 40 min; a year is 687 Earth days, so its seasons are twice as long as ours."),
            ("Air", "95 % carbon dioxide at about 6 millibars: a hundredth of the air pressure at sea level on Earth, too thin for liquid water to last at the surface."),
            ("Heat", "About −60 °C on average; a summer noon at the equator can reach 20 °C at the ground, a polar winter night −125 °C."),
            ("Sky", "Fine dust makes the day sky butterscotch and the sunset blue: the grains scatter blue light forward, round the sun. Dust storms can cover the whole planet for months."),
            ("Giants", "Olympus Mons stands about 22 km high; the canyons of Valles Marineris run some 4,000 km and reach 7 km deep."),
            ("Moons", "Phobos and Deimos, small and dark, probably captured or knocked off by an impact. Phobos crosses the sky west to east twice a day."),
            ("Visits", "Viking 1 and 2 landed in 1976; Sojourner rolled in 1997; Spirit, Opportunity, Curiosity, Perseverance and China's Zhurong have driven here since. Ingenuity, which came with Perseverance, made the first powered flights on another planet: 72 of them, 2021 to 2024."),
        ],
        gravity: 3.72,
        radius: 3_389_500.0,
        au: 1.524,
        albedo: 0.2,
        grip: 0.65,
        // Dust packs: brighter, less porous, with little surge.
        soil: Soil { craterlets: 0.12, grain: 0.8, ripples: 0.7, pebbles: 1.0, pebble_tone: 0.55, sparkle: 0.0, powder: 0.45, surge: 0.25, surge_width: 0.15, backscatter: 0.35 },
        roughness: 0.95,
        reflectance: 0.25,
        air: air(),
        // Six millibars of cold carbon dioxide, and an afternoon breeze.
        air_density: 0.020,
        wind: 7.0,
        // Perseverance's microphones heard it: about 240 m/s.
        sound_speed: 240.0,
        craft: Some(Craft::Ingenuity),
        sun: (100.0, 24.0),
        stars: 0.0,
        // The dusty sky lights the shade, butterscotch: matched to how
        // Bevy's atmosphere lit it (see `SkySettings::sky_light`).
        fill: 0.6,
        fill_color: Color::linear_rgb(1.0, 0.75, 0.55),
        exposure_bias: -0.9,
        // Dark basalt under a film of the bright dust.
        rocks: Rocks { density: 0.07, largest: 3.0, albedo: [0.07, 0.15], tint: [1.25, 0.92, 0.72], ..default() },
        companions: vec![
            Companion {
                name: "Phobos",
                about: "6,000 km up and less than half the width of the full Moon from Earth. It rises in the west and crosses the sky in about four hours.",
                ..Companion::new(Map::Rock, 0.1, 250.0, 50.0)
            },
            Companion { name: "Deimos", about: "The outer moon, 20,000 km up: hardly more than a bright star.", ..Companion::new(Map::Rock, 0.017, 140.0, 35.0) },
        ],
        look: (0.9, -0.06),
        ..Body::airless("Mars", "Layered buttes", Arc::new(land))
    }
}
