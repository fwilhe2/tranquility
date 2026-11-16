//! A water surface at one level: one big plane kept centred under the camera.
//! The ground below it is whatever the world's height function says.

use bevy::prelude::*;

use crate::world::WorldDef;

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (setup.run_if(resource_changed::<WorldDef>), follow).chain());
    }
}

#[derive(Component)]
struct Water;

fn setup(
    mut commands: Commands,
    world: Res<WorldDef>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<Water>>,
) {
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let Some(water) = &world.water else {
        return;
    };
    let extent = world.terrain.reach() * 1.5;
    commands.spawn((
        Water,
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(extent)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: water.color,
            perceptual_roughness: water.roughness,
            reflectance: 0.5,
            ..default()
        })),
        Transform::from_xyz(0.0, water.level, 0.0),
    ));
}

fn follow(
    camera: Single<&Transform, (With<Camera3d>, Without<Water>)>,
    mut water: Query<&mut Transform, With<Water>>,
) {
    for mut t in &mut water {
        t.translation.x = camera.translation.x;
        t.translation.z = camera.translation.z;
    }
}
