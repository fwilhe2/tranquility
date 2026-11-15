//! Building meshes from data: polygons, polylines, buildings.
//!
//! Conventions, as in historical-walk: x east, z south, y up; rings are flat
//! `[x0, z0, x1, z1, …]` slices, not closed.

pub mod massing;
pub mod mesh;
pub mod poly;
pub mod raster;

pub use massing::{Massing, Roof};
pub use mesh::MeshBuilder;
pub use raster::Raster;
