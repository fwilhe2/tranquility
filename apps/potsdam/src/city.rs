//! The city as historical-walk's tools write it (`src/city/types.ts` there):
//! metres in a local frame, x east, z south, rings as flat `[x, z, …]`.
//! Only the fields drawn here are read; serde skips the rest.

use serde::Deserialize;

pub type Ring = Vec<f32>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct City {
    pub name: String,
    pub year: i32,
    pub bounds: [f32; 4],
    pub region: Vec<Area>,
    pub region_bounds: [f32; 4],
    pub terrain: Option<TerrainGrid>,
    pub spawn: SpawnPoint,
    pub buildings: Vec<Building>,
    pub areas: Vec<Area>,
    pub streets: Vec<Street>,
    pub walls: Vec<Wall>,
    pub rails: Vec<Rail>,
    pub trees: Vec<Tree>,
    pub columns: Vec<Column>,
    pub lamps: Vec<Point>,
    pub fixtures: Vec<Fixture>,
    pub labels: Vec<Label>,
    pub water: f32,
    pub credits: Vec<String>,
}

#[derive(Deserialize)]
pub struct TerrainGrid {
    pub x0: f32,
    pub z0: f32,
    pub step: f32,
    pub nx: usize,
    pub nz: usize,
    /// Decimetres as base64 little-endian i16, row by row from (x0, z0).
    pub dm: String,
}

#[derive(Deserialize)]
pub struct SpawnPoint {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Building {
    pub outer: Ring,
    #[serde(default)]
    pub holes: Vec<Ring>,
    pub base: Option<f32>,
    pub eave: f32,
    pub roof_h: f32,
    pub roof: String,
    pub ridge: Option<f32>,
    pub storey: f32,
    pub colour: String,
    pub roof_colour: String,
    pub wall: Option<String>,
    pub windows: Option<bool>,
    pub shop: Option<bool>,
}

#[derive(Deserialize)]
pub struct Area {
    pub kind: String,
    pub outer: Ring,
    #[serde(default)]
    pub holes: Vec<Ring>,
    pub paving: Option<String>,
}

#[derive(Deserialize)]
pub struct Street {
    pub kind: String,
    pub w: f32,
    pub pts: Ring,
    pub paving: String,
}

#[derive(Deserialize)]
pub struct Wall {
    pub pts: Ring,
    pub h: f32,
    pub t: f32,
    pub kind: String,
    pub base: Option<f32>,
}

#[derive(Deserialize)]
pub struct Rail {
    pub pts: Ring,
}

#[derive(Deserialize)]
pub struct Tree {
    pub x: f32,
    pub z: f32,
    pub h: f32,
    pub r: f32,
}

#[derive(Deserialize)]
pub struct Column {
    pub x: f32,
    pub z: f32,
    pub r: f32,
    pub h: f32,
    pub base: f32,
}

#[derive(Deserialize)]
pub struct Point {
    pub x: f32,
    pub z: f32,
}

#[derive(Deserialize)]
pub struct Fixture {
    pub x: f32,
    pub z: f32,
    pub kind: String,
}

#[derive(Deserialize)]
pub struct Label {
    pub x: f32,
    pub z: f32,
    pub text: String,
    pub about: Option<String>,
    pub r: Option<f32>,
}

impl TerrainGrid {
    pub fn decode(&self) -> Vec<i16> {
        let bytes = base64(&self.dm);
        bytes.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect()
    }
}

fn base64(s: &str) -> Vec<u8> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for v in s.bytes().filter_map(value) {
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}
