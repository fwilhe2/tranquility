//! Accumulating triangles with per-vertex colour into one mesh.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

#[derive(Default)]
pub struct MeshBuilder {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    /// Free per-vertex parameters for custom materials (e.g. a facade's
    /// distance along the wall and height above its foot), carried as UV 0.
    pub params: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn vertex(&mut self, p: Vec3, n: Vec3, color: LinearRgba, param: Vec2) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push(p.to_array());
        self.normals.push(n.to_array());
        self.colors.push(color.to_f32_array());
        self.params.push(param.to_array());
        i
    }

    /// A flat-shaded triangle, wound to face `facing` (any vector on the side
    /// it should be seen from).
    pub fn triangle(&mut self, a: Vec3, b: Vec3, c: Vec3, facing: Vec3, color: LinearRgba) {
        self.triangle_with(a, b, c, facing, color, [Vec2::ZERO; 3]);
    }

    pub fn triangle_with(
        &mut self,
        a: Vec3,
        b: Vec3,
        c: Vec3,
        facing: Vec3,
        color: LinearRgba,
        params: [Vec2; 3],
    ) {
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            return;
        }
        let (b, c, pb, pc, n) = if n.dot(facing) >= 0.0 {
            (b, c, params[1], params[2], n.normalize())
        } else {
            (c, b, params[2], params[1], -n.normalize())
        };
        let i = self.vertex(a, n, color, params[0]);
        self.vertex(b, n, color, pb);
        self.vertex(c, n, color, pc);
        self.indices.extend_from_slice(&[i, i + 1, i + 2]);
    }

    /// A flat quad a–b–c–d (in order round its edge), facing `facing`.
    pub fn quad(&mut self, q: [Vec3; 4], facing: Vec3, color: LinearRgba, params: [Vec2; 4]) {
        self.triangle_with(q[0], q[1], q[2], facing, color, [params[0], params[1], params[2]]);
        self.triangle_with(q[0], q[2], q[3], facing, color, [params[0], params[2], params[3]]);
    }

    /// A box along a segment: `a` and `b` on the ground line, `width` across,
    /// from `y0` to `y1`. Walls, railings, decks.
    pub fn slab(&mut self, a: Vec2, b: Vec2, width: f32, y0: [f32; 2], y1: [f32; 2], color: LinearRgba) {
        let d = b - a;
        if d.length_squared() < 1e-8 {
            return;
        }
        let side = d.normalize().perp() * width * 0.5;
        let corners = [a - side, b - side, b + side, a + side];
        let ys = |i: usize, top: bool| {
            let k = if i == 0 || i == 3 { 0 } else { 1 };
            if top { y1[k] } else { y0[k] }
        };
        let at = |i: usize, top: bool| Vec3::new(corners[i].x, ys(i, top), corners[i].y);
        let centre = (a + b) * 0.5;
        let mid = Vec3::new(centre.x, (y0[0] + y1[0]) * 0.5, centre.y);
        for i in 0..4 {
            let j = (i + 1) % 4;
            let q = [at(i, false), at(j, false), at(j, true), at(i, true)];
            let out = (q[0] + q[1]) * 0.5 - mid;
            self.quad(q, Vec3::new(out.x, 0.0, out.z), color, [Vec2::ZERO; 4]);
        }
        self.quad([at(0, true), at(1, true), at(2, true), at(3, true)], Vec3::Y, color, [Vec2::ZERO; 4]);
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.params)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// `#rrggbb` to linear colour; grey if malformed.
pub fn hex(s: &str) -> LinearRgba {
    Srgba::hex(s).map_or(LinearRgba::gray(0.5), LinearRgba::from)
}
