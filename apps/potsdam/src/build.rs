//! Everything that stands in the city, built once at startup: buildings
//! (merged into one mesh per 200 m tile), walls, bridge decks, trees, lamps,
//! columns, street furniture.

use std::sync::Arc;

use bevy::platform::collections::HashMap;
use tranquility::{
    facade::{self, FacadeMaterial, Windows},
    geom::{Massing, MeshBuilder, Roof, mesh::hex},
    prelude::*,
};

use crate::{
    city::City,
    ground::{CityGround, is_bridge},
};

const TILE: f32 = 200.0;

#[derive(Resource, Clone)]
pub struct CityData(pub Arc<City>, pub Arc<CityGround>);

pub fn spawn(
    mut commands: Commands,
    data: Res<CityData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut facades: ResMut<Assets<FacadeMaterial>>,
) {
    let (city, ground) = (&*data.0, &*data.1);
    let masonry = facades.add(facade::material(0.85));

    // Buildings, walls and decks: vertex-coloured, merged per tile.
    let mut tiles: HashMap<IVec2, MeshBuilder> = HashMap::new();
    let tile_of = |x: f32, z: f32| IVec2::new((x / TILE).floor() as i32, (z / TILE).floor() as i32);
    for b in &city.buildings {
        let corners = b.outer.chunks_exact(2);
        // Stand on the lowest corner, as historical-walk does; bury the foot.
        let level = corners.clone().map(|p| ground.land(p[0], p[1])).fold(f32::INFINITY, f32::min);
        let (sx, sz) = corners.fold((0.0, 0.0), |(x, z), p| (x + p[0], z + p[1]));
        let n = (b.outer.len() / 2) as f32;
        let mesh = tiles.entry(tile_of(sx / n, sz / n)).or_default();
        let base = b.base.unwrap_or(0.0);
        Massing {
            outer: &b.outer,
            holes: b.holes.iter().map(Vec::as_slice).collect(),
            ground: level,
            foot: if base > 0.0 { level + base } else { level - 0.6 },
            eave: b.eave,
            roof_height: b.roof_h,
            roof: roof(&b.roof),
            ridge: b.ridge,
            wall: wall_colour(&b.colour, b.wall.as_deref()),
            roof_color: hex(&b.roof_colour),
            windows: if b.windows == Some(false) || b.wall.as_deref() == Some("copper") {
                Windows::Blank
            } else {
                Windows::Storeys {
                    storey: b.storey,
                    shop: b.shop == Some(true),
                }
            },
        }
        .build(mesh);
    }

    for w in &city.walls {
        let colour = match w.kind.as_str() {
            "railing" => LinearRgba::from(Color::srgb(0.08, 0.09, 0.08)),
            "parapet" => LinearRgba::from(Color::srgb(0.55, 0.52, 0.46)),
            "city" => LinearRgba::from(Color::srgb(0.62, 0.55, 0.45)),
            _ => LinearRgba::from(Color::srgb(0.58, 0.50, 0.42)),
        };
        let t = if w.kind == "railing" { w.t.min(0.06) } else { w.t };
        for seg in w.pts.windows(4).step_by(2) {
            let (a, b) = (Vec2::new(seg[0], seg[1]), Vec2::new(seg[2], seg[3]));
            let (ga, gb) = (ground.land(a.x, a.y), ground.land(b.x, b.y));
            let base = w.base.unwrap_or(0.0);
            tiles.entry(tile_of(a.x, a.y)).or_default().slab(
                a,
                b,
                t,
                [ga + base - 0.5, gb + base - 0.5],
                [ga + base + w.h, gb + base + w.h],
                colour,
            );
        }
    }

    for s in city.streets.iter().filter(|s| is_bridge(&s.kind)) {
        let colour = if s.paving == "plank" {
            LinearRgba::from(Color::srgb(0.33, 0.26, 0.19))
        } else {
            LinearRgba::from(Color::srgb(0.45, 0.43, 0.40))
        };
        for seg in s.pts.windows(4).step_by(2) {
            let (a, b) = (Vec2::new(seg[0], seg[1]), Vec2::new(seg[2], seg[3]));
            let (ga, gb) = (ground.land(a.x, a.y), ground.land(b.x, b.y));
            tiles.entry(tile_of(a.x, a.y)).or_default().slab(
                a,
                b,
                s.w,
                [ga - 0.9, gb - 0.9],
                [ga + 0.02, gb + 0.02],
                colour,
            );
        }
    }

    for (_, mesh) in tiles {
        if !mesh.is_empty() {
            commands.spawn((Mesh3d(meshes.add(mesh.build())), MeshMaterial3d(masonry.clone())));
        }
    }

    // Trees: one trunk and one crown mesh, scaled per tree, so they batch.
    let trunk = meshes.add(Cylinder::new(0.5, 1.0));
    let crown = meshes.add(Sphere::new(1.0).mesh().ico(2).expect("valid subdivision"));
    let bark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.18, 0.14),
        perceptual_roughness: 0.9,
        ..default()
    });
    let leaves: Vec<_> = [(0.16, 0.25, 0.08), (0.20, 0.28, 0.10), (0.13, 0.21, 0.08)]
        .iter()
        .map(|&(r, g, b)| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                perceptual_roughness: 0.8,
                ..default()
            })
        })
        .collect();
    for (k, t) in city.trees.iter().enumerate() {
        let y = ground.land(t.x, t.z);
        let girth = 0.18 + t.h * 0.012;
        commands.spawn((
            Mesh3d(trunk.clone()),
            MeshMaterial3d(bark.clone()),
            Transform::from_xyz(t.x, y + t.h * 0.3, t.z).with_scale(Vec3::new(girth * 2.0, t.h * 0.6, girth * 2.0)),
        ));
        commands.spawn((
            Mesh3d(crown.clone()),
            MeshMaterial3d(leaves[k % leaves.len()].clone()),
            Transform::from_xyz(t.x, y + t.h * 0.62, t.z)
                .with_scale(Vec3::new(t.r, t.h * 0.38, t.r))
                .with_rotation(Quat::from_rotation_y(k as f32)),
        ));
    }

    // Gas lanterns: a dark post and a lamp that glows; at noon the glow is
    // far below the daylight exposure, at night it carries.
    let post = meshes.add(Cylinder::new(0.06, 3.2));
    let lantern = meshes.add(Cuboid::new(0.32, 0.45, 0.32));
    let iron = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.08, 0.07),
        perceptual_roughness: 0.6,
        metallic: 0.6,
        ..default()
    });
    let glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.85, 0.7),
        emissive: LinearRgba::rgb(500.0, 380.0, 180.0),
        ..default()
    });
    for l in &city.lamps {
        let y = ground.land(l.x, l.z);
        commands.spawn((Mesh3d(post.clone()), MeshMaterial3d(iron.clone()), Transform::from_xyz(l.x, y + 1.6, l.z)));
        commands.spawn((Mesh3d(lantern.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(l.x, y + 3.4, l.z)));
    }

    // Columns of porticos and colonnades, and the street furniture.
    let shaft = meshes.add(Cylinder::new(1.0, 1.0));
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.68, 0.6),
        perceptual_roughness: 0.8,
        ..default()
    });
    let green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.22, 0.16),
        perceptual_roughness: 0.6,
        ..default()
    });
    for c in &city.columns {
        let y = ground.land(c.x, c.z) + c.base;
        commands.spawn((
            Mesh3d(shaft.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(c.x, y + c.h * 0.5, c.z).with_scale(Vec3::new(c.r, c.h, c.r)),
        ));
    }
    for f in &city.fixtures {
        let y = ground.land(f.x, f.z);
        let (r, h) = if f.kind == "column" { (0.6, 3.0) } else { (0.14, 1.5) };
        commands.spawn((
            Mesh3d(shaft.clone()),
            MeshMaterial3d(green.clone()),
            Transform::from_xyz(f.x, y + h * 0.5, f.z).with_scale(Vec3::new(r, h, r)),
        ));
    }
}

fn roof(shape: &str) -> Roof {
    match shape {
        "flat" => Roof::Flat,
        "gabled" => Roof::Gabled,
        "hipped" => Roof::Hipped,
        "half-hipped" => Roof::HalfHipped,
        "pyramidal" => Roof::Pyramidal,
        "mansard" => Roof::Mansard,
        "dome" => Roof::Dome,
        "spire" => Roof::Spire,
        _ => Roof::Gabled,
    }
}

/// The facade's colour, darkened for brick and copper, which the data gives
/// as their overall tone.
fn wall_colour(colour: &str, wall: Option<&str>) -> LinearRgba {
    let c = hex(colour);
    match wall {
        Some("brick") => c * 0.85,
        _ => c,
    }
}
