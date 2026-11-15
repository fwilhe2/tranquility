//! Walls and roof of one building from its footprint.
//!
//! Every roof is the lower envelope of a few planes over the footprint (the
//! idea of historical-walk's massing.ts): a gable is two planes meeting at
//! the ridge, a hip roof adds two end planes, a mansard is eight, a dome is
//! many tangent planes. The roof surface is cut exactly where each plane is
//! the lowest, and the walls rise to the envelope along their top edge, so
//! gable ends come out as walls and every wall top meets its roof edge.

use bevy::prelude::*;

use crate::facade::Windows;

use super::{
    mesh::MeshBuilder,
    poly::{self, points},
};

/// Height of the plinth below the ground storey's windows; the facade
/// shader counts storeys from here.
pub const PLINTH: f32 = 0.55;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Roof {
    Flat,
    Gabled,
    Hipped,
    HalfHipped,
    Pyramidal,
    Mansard,
    Dome,
    Spire,
}

pub struct Massing<'a> {
    pub outer: &'a [f32],
    pub holes: Vec<&'a [f32]>,
    /// The street level the facade's storeys count from (the `y` of the
    /// facade parameter is height above it).
    pub ground: f32,
    /// Where the walls start: below `ground` to bury them on a slope, above
    /// it for arcades.
    pub foot: f32,
    /// Height of the eaves above `ground`.
    pub eave: f32,
    /// Height of the roof's top above the eaves.
    pub roof_height: f32,
    pub roof: Roof,
    /// Ridge bearing, radians from +x; `None` takes the footprint's long axis.
    pub ridge: Option<f32>,
    pub wall: LinearRgba,
    pub roof_color: LinearRgba,
    /// What the facade shader draws on the walls.
    pub windows: Windows,
}

/// A roof plane: height above the eaves `a·x + b·z + c`.
#[derive(Clone, Copy)]
struct Plane {
    a: f32,
    b: f32,
    c: f32,
}

impl Plane {
    fn at(&self, p: Vec2) -> f32 {
        self.a * p.x + self.b * p.y + self.c
    }
}

impl Massing<'_> {
    pub fn build(&self, mesh: &mut MeshBuilder) {
        if self.outer.len() < 6 {
            return;
        }
        // Outer ring positive, holes negative: then every edge's outward
        // normal is (dz, −dx).
        let outer = if poly::area(self.outer) >= 0.0 { self.outer.to_vec() } else { poly::reversed(self.outer) };
        let holes: Vec<Vec<f32>> = self
            .holes
            .iter()
            .filter(|h| h.len() >= 6)
            .map(|h| if poly::area(h) <= 0.0 { h.to_vec() } else { poly::reversed(h) })
            .collect();

        let planes = self.planes(&outer);
        let eave = self.ground + self.eave;
        // Fit the storeys to the eaves, so the top row of windows ends under
        // the cornice rather than being cut by it.
        let windows = match self.windows {
            Windows::Storeys { storey, shop } if storey > 0.5 => {
                let span = (self.eave - PLINTH).max(storey);
                let rows = (span / storey).round().max(1.0);
                Windows::Storeys { storey: span / rows, shop }
            }
            _ => Windows::Blank,
        };
        let mut wall = self.wall;
        wall.alpha = windows.code();
        let mut roof_color = self.roof_color;
        roof_color.alpha = Windows::Blank.code();
        let envelope = |p: Vec2| planes.iter().map(|pl| pl.at(p)).fold(f32::INFINITY, f32::min).max(0.0);

        // Walls, broken at every point along an edge where the envelope bends.
        let mut along = 0.0;
        for ring in std::iter::once(&outer).chain(holes.iter()) {
            let pts: Vec<Vec2> = points(ring).collect();
            for i in 0..pts.len() {
                let (p0, p1) = (pts[i], pts[(i + 1) % pts.len()]);
                let d = p1 - p0;
                let len = d.length();
                if len < 1e-3 {
                    continue;
                }
                let normal = Vec3::new(d.y, 0.0, -d.x);
                let mut ts = vec![0.0, 1.0];
                for (k, pk) in planes.iter().enumerate() {
                    let (g0, g1) = (pk.at(p0), pk.at(p1));
                    if (g0 > 0.0) != (g1 > 0.0) {
                        ts.push(g0 / (g0 - g1));
                    }
                    for pj in &planes[k + 1..] {
                        let (f0, f1) = (pk.at(p0) - pj.at(p0), pk.at(p1) - pj.at(p1));
                        if (f0 > 0.0) != (f1 > 0.0) {
                            ts.push(f0 / (f0 - f1));
                        }
                    }
                }
                ts.sort_by(|a, b| a.total_cmp(b));
                ts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
                for w in ts.windows(2) {
                    let (qa, qb) = (p0 + d * w[0], p0 + d * w[1]);
                    let (ha, hb) = (eave + envelope(qa), eave + envelope(qb));
                    let (sa, sb) = (along + len * w[0], along + len * w[1]);
                    mesh.quad(
                        [
                            Vec3::new(qa.x, self.foot, qa.y),
                            Vec3::new(qb.x, self.foot, qb.y),
                            Vec3::new(qb.x, hb, qb.y),
                            Vec3::new(qa.x, ha, qa.y),
                        ],
                        normal,
                        wall,
                        [
                            Vec2::new(sa, self.foot - self.ground),
                            Vec2::new(sb, self.foot - self.ground),
                            Vec2::new(sb, hb - self.ground),
                            Vec2::new(sa, ha - self.ground),
                        ],
                    );
                }
                along += len;
            }
        }

        // Roof: triangulate the footprint, then cut each triangle into the
        // pieces where one plane is lowest.
        let all: Vec<[f32; 2]> = outer
            .chunks_exact(2)
            .chain(holes.iter().flat_map(|h| h.chunks_exact(2)))
            .map(|p| [p[0], p[1]])
            .collect();
        let mut hole_starts = Vec::new();
        let mut n = outer.len() / 2;
        for h in &holes {
            hole_starts.push(n as u32);
            n += h.len() / 2;
        }
        let mut triangles: Vec<u32> = Vec::new();
        earcut::Earcut::new().earcut(all.iter().copied(), &hole_starts, &mut triangles);

        // A roof seen from below (under an arcade's eaves) is dark anyway, so
        // one-sided faces facing up are enough.
        for tri in triangles.chunks_exact(3) {
            let corners: Vec<Vec2> = tri.iter().map(|&i| Vec2::from(all[i as usize])).collect();
            for (i, pi) in planes.iter().enumerate() {
                let mut piece = corners.clone();
                for (j, pj) in planes.iter().enumerate() {
                    if i == j || piece.len() < 3 {
                        continue;
                    }
                    // Keep where plane i is at or below plane j; ties go to
                    // the lower index so no piece is drawn twice.
                    let tie = if j < i { -1e-4 } else { 1e-4 };
                    piece = clip(&piece, |p| pi.at(p) - pj.at(p) - tie);
                }
                if piece.len() < 3 {
                    continue;
                }
                let lift = |p: Vec2| Vec3::new(p.x, eave + pi.at(p).max(0.0), p.y);
                for k in 1..piece.len() - 1 {
                    mesh.triangle(lift(piece[0]), lift(piece[k]), lift(piece[k + 1]), Vec3::Y, roof_color);
                }
            }
        }
    }

    /// The roof's planes in world x/z, from the shape in the frame of the
    /// footprint's box (u along the ridge, v across it).
    fn planes(&self, outer: &[f32]) -> Vec<Plane> {
        let h = self.roof_height;
        if self.roof == Roof::Flat || h <= 0.01 {
            return vec![Plane { a: 0.0, b: 0.0, c: 0.0 }];
        }
        let (u, centre, hl, hw) = match self.ridge {
            Some(bearing) => {
                let u = Vec2::new(bearing.cos(), bearing.sin());
                let (axis, centre, a, b) = poly::box_with_axis(outer, u);
                // Keep the given bearing even where the footprint is longer
                // across it.
                if axis.dot(u).abs() > 0.5 { (u, centre, a, b) } else { (u, centre, b, a) }
            }
            None => poly::oriented_box(outer),
        };
        let (l, w) = (hl.max(0.5), hw.max(0.5));
        let v = u.perp();
        // Local planes as (A, B, C): height = A·du + B·dv + C.
        let mut local: Vec<(f32, f32, f32)> = Vec::new();
        let s = h / w;
        match self.roof {
            Roof::Flat => unreachable!(),
            Roof::Gabled => local.extend([(0.0, -s, h), (0.0, s, h)]),
            Roof::Hipped => local.extend([(0.0, -s, h), (0.0, s, h), (-s, 0.0, s * l), (s, 0.0, s * l)]),
            Roof::HalfHipped => local.extend([
                (0.0, -s, h),
                (0.0, s, h),
                (-s, 0.0, s * l + h * 0.5),
                (s, 0.0, s * l + h * 0.5),
            ]),
            Roof::Pyramidal => local.extend([(0.0, -h / w, h), (0.0, h / w, h), (-h / l, 0.0, h), (h / l, 0.0, h)]),
            Roof::Mansard => {
                // Steep lower slopes up to a break, a shallow hip above it.
                let d = (w * 0.3).min(1.6);
                let hb = h * 0.65;
                let k1 = hb / d;
                let k2 = (h - hb) / (w - d).max(0.1);
                for (half, a, b) in [(w, 0.0, 1.0), (l, 1.0, 0.0)] {
                    for sign in [-1.0, 1.0] {
                        let (ga, gb) = (a * sign, b * sign);
                        local.push((-k1 * ga, -k1 * gb, k1 * half));
                        local.push((-k2 * ga, -k2 * gb, hb + k2 * (half - d)));
                    }
                }
            }
            Roof::Dome => {
                // Tangent planes of a half-ellipsoid on three rings, plus the
                // flat top.
                let r = (l + w) * 0.5;
                local.push((0.0, 0.0, h));
                for (ring, count) in [(0.45, 10), (0.72, 14), (0.9, 18)] {
                    let r0 = ring * r;
                    let root = (1.0 - ring * ring).sqrt();
                    let h0 = h * root;
                    let g = -h * r0 / (r * r * root);
                    for k in 0..count {
                        let beta = k as f32 / count as f32 * std::f32::consts::TAU;
                        local.push((g * beta.cos(), g * beta.sin(), h0 - g * r0));
                    }
                }
            }
            Roof::Spire => {
                for k in 0..8 {
                    let beta = (k as f32 + 0.5) / 8.0 * std::f32::consts::TAU;
                    local.push((-h * beta.cos() / w, -h * beta.sin() / w, h));
                }
            }
        }
        local
            .into_iter()
            .map(|(pa, pb, pc)| {
                let a = pa * u.x + pb * v.x;
                let b = pa * u.y + pb * v.y;
                Plane { a, b, c: pc - a * centre.x - b * centre.y }
            })
            .collect()
    }
}

/// Sutherland–Hodgman: the part of a convex polygon where `f ≤ 0`, for a
/// linear `f`.
fn clip(poly: &[Vec2], f: impl Fn(Vec2) -> f32) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (fa, fb) = (f(a), f(b));
        if fa <= 0.0 {
            out.push(a);
        }
        if (fa <= 0.0) != (fb <= 0.0) {
            out.push(a + (b - a) * (fa / (fa - fb)));
        }
    }
    out
}
