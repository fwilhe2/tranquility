//! Quality tiers: what the frame costs on the GPU. `Q` cycles them live.
//!
//! - `TRANQUILITY_QUALITY=high|medium|low` picks the starting tier.
//! - `TRANQUILITY_OFF=ssao,taa,bloom,shadows` switches single features off on
//!   top of the tier, for measuring what each costs.

use bevy::{
    anti_alias::{fxaa::Fxaa, smaa::Smaa, taa::TemporalAntiAliasing},
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, ShadowFilteringMethod},
    pbr::ScreenSpaceAmbientOcclusion,
    post_process::bloom::Bloom,
    prelude::*,
};

use crate::player::Player;

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quality {
    /// 4096² shadows in four cascades to 3 km, SSAO, TAA, bloom.
    High,
    /// 2048² shadows in three cascades to 1.5 km, TAA, bloom.
    Medium,
    /// 1024² shadows in two cascades to 600 m, FXAA.
    Low,
}

impl Quality {
    fn next(self) -> Self {
        match self {
            Quality::High => Quality::Medium,
            Quality::Medium => Quality::Low,
            Quality::Low => Quality::High,
        }
    }

    /// The coarsest level of the ground that still draws into the sun's
    /// shadow maps. Each level doubles the grid step, and the far cascade
    /// is drawn from every chunk in its reach: past a few levels the
    /// shadows gained are a texel or two wide and cost a third of the frame.
    pub fn shadow_levels(self) -> u32 {
        match self {
            Quality::High => 5,
            Quality::Medium => 3,
            Quality::Low => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Quality::High => "high",
            Quality::Medium => "medium",
            Quality::Low => "low",
        }
    }
}

pub struct QualityPlugin;

impl Plugin for QualityPlugin {
    fn build(&self, app: &mut App) {
        let start = match std::env::var("TRANQUILITY_QUALITY").as_deref() {
            Ok("low") => Quality::Low,
            Ok("medium") => Quality::Medium,
            Ok("high") => Quality::High,
            // Medium holds ~30 fps on a small discrete GPU; Q goes up.
            _ => Quality::Medium,
        };
        app.insert_resource(start).add_systems(Update, (cycle, apply).chain());
    }
}

fn cycle(keys: Res<ButtonInput<KeyCode>>, mut quality: ResMut<Quality>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        *quality = quality.next();
    }
}

fn off(feature: &str) -> bool {
    std::env::var("TRANQUILITY_OFF").is_ok_and(|v| v.split(',').any(|f| f.trim() == feature))
}

/// Applies the tier when it changes and once the camera exists.
fn apply(
    mut commands: Commands,
    quality: Res<Quality>,
    camera: Query<Entity, Added<Player>>,
    cameras: Query<Entity, With<Player>>,
    lights: Query<Entity, With<DirectionalLight>>,
    mut light_settings: Query<&mut DirectionalLight>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
) {
    if !quality.is_changed() && camera.is_empty() {
        return;
    }
    let q = *quality;
    for entity in &cameras {
        let mut cam = commands.entity(entity);
        cam.remove::<(ScreenSpaceAmbientOcclusion, TemporalAntiAliasing, Fxaa, Smaa, Bloom)>();
        if q == Quality::High && !off("ssao") {
            cam.insert(ScreenSpaceAmbientOcclusion::default());
        }
        if q != Quality::Low && !off("taa") {
            cam.insert(TemporalAntiAliasing::default());
        } else {
            cam.insert(Fxaa::default());
        }
        // A shadow lookup of one tap where the frame is tight: the ground
        // fills half the screen and every pixel of it asks.
        cam.insert(if q == Quality::High || off("hardshadow") { ShadowFilteringMethod::Gaussian } else { ShadowFilteringMethod::Hardware2x2 });
        if q != Quality::Low && !off("bloom") {
            cam.insert(Bloom::NATURAL);
        }
    }
    let (size, cascades, reach, first) = match q {
        Quality::High => (4096, 4, 3000.0, 30.0),
        Quality::Medium => (2048, 3, 1500.0, 25.0),
        Quality::Low => (1024, 2, 600.0, 30.0),
    };
    shadow_map.size = size;
    for entity in &lights {
        commands.entity(entity).insert(
            CascadeShadowConfigBuilder {
                num_cascades: cascades,
                first_cascade_far_bound: first,
                maximum_distance: reach,
                ..default()
            }
            .build(),
        );
    }
    for mut light in &mut light_settings {
        light.shadow_maps_enabled = !off("shadows");
    }
}
