//! Plane geometry on flat `[x, z, …]` rings.

use bevy::math::Vec2;

pub fn points(ring: &[f32]) -> impl Iterator<Item = Vec2> + Clone + '_ {
    ring.chunks_exact(2).map(|p| Vec2::new(p[0], p[1]))
}

/// Shoelace area: positive for a ring that runs clockwise on a north-up map
/// (x east, z south), which is how outer rings come.
pub fn area(ring: &[f32]) -> f32 {
    let n = ring.len() / 2;
    let mut a = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        a += ring[2 * i] * ring[2 * j + 1] - ring[2 * j] * ring[2 * i + 1];
    }
    a * 0.5
}

/// The ring with its points in reverse order.
pub fn reversed(ring: &[f32]) -> Vec<f32> {
    ring.chunks_exact(2).rev().flatten().copied().collect()
}

/// Even-odd point-in-polygon over several rings (outer and holes).
pub fn contains(rings: &[&[f32]], p: Vec2) -> bool {
    let mut inside = false;
    for ring in rings {
        let n = ring.len() / 2;
        let mut j = n - 1;
        for i in 0..n {
            let (xi, zi) = (ring[2 * i], ring[2 * i + 1]);
            let (xj, zj) = (ring[2 * j], ring[2 * j + 1]);
            if (zi > p.y) != (zj > p.y) && p.x < (xj - xi) * (p.y - zi) / (zj - zi) + xi {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

/// The minimum-area rectangle around a ring among those aligned with one of
/// its edges: returns (unit long axis, centre, half length, half width).
pub fn oriented_box(ring: &[f32]) -> (Vec2, Vec2, f32, f32) {
    let pts: Vec<Vec2> = points(ring).collect();
    let mut best = (Vec2::X, Vec2::ZERO, 0.0, 0.0, f32::INFINITY);
    for i in 0..pts.len() {
        let d = pts[(i + 1) % pts.len()] - pts[i];
        if d.length_squared() < 1e-6 {
            continue;
        }
        let u = d.normalize();
        if let Some(b) = box_along(&pts, u)
            && b.4 < best.4
        {
            best = b;
        }
    }
    (best.0, best.1, best.2, best.3)
}

/// The rectangle around a ring aligned with `u`, its long side along the
/// returned axis.
pub fn box_with_axis(ring: &[f32], u: Vec2) -> (Vec2, Vec2, f32, f32) {
    let pts: Vec<Vec2> = points(ring).collect();
    let b = box_along(&pts, u).unwrap_or((u, Vec2::ZERO, 0.0, 0.0, 0.0));
    (b.0, b.1, b.2, b.3)
}

fn box_along(pts: &[Vec2], u: Vec2) -> Option<(Vec2, Vec2, f32, f32, f32)> {
    let v = u.perp();
    let (mut u0, mut u1, mut v0, mut v1) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY);
    for p in pts {
        let (a, b) = (p.dot(u), p.dot(v));
        u0 = u0.min(a);
        u1 = u1.max(a);
        v0 = v0.min(b);
        v1 = v1.max(b);
    }
    if !u0.is_finite() {
        return None;
    }
    let centre = u * (u0 + u1) * 0.5 + v * (v0 + v1) * 0.5;
    let (hl, hw) = ((u1 - u0) * 0.5, (v1 - v0) * 0.5);
    let area = hl * hw;
    Some(if hl >= hw {
        (u, centre, hl, hw, area)
    } else {
        (v, centre, hw, hl, area)
    })
}
