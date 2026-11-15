//! A grid of values over a rectangle of the ground, painted with polygons and
//! thick lines: land use for tint functions, solid cells for collision.

use bevy::math::Vec2;

#[derive(Clone)]
pub struct Raster<T> {
    pub x0: f32,
    pub z0: f32,
    pub cell: f32,
    pub nx: usize,
    pub nz: usize,
    pub data: Vec<T>,
}

impl<T: Copy> Raster<T> {
    /// Covers `[x0, x1) × [z0, z1)` with square cells of `cell` metres.
    pub fn new(x0: f32, z0: f32, x1: f32, z1: f32, cell: f32, fill: T) -> Self {
        let nx = ((x1 - x0) / cell).ceil().max(1.0) as usize;
        let nz = ((z1 - z0) / cell).ceil().max(1.0) as usize;
        Self {
            x0,
            z0,
            cell,
            nx,
            nz,
            data: vec![fill; nx * nz],
        }
    }

    pub fn index(&self, x: f32, z: f32) -> Option<usize> {
        let i = ((x - self.x0) / self.cell).floor();
        let j = ((z - self.z0) / self.cell).floor();
        if i < 0.0 || j < 0.0 || i >= self.nx as f32 || j >= self.nz as f32 {
            return None;
        }
        Some(j as usize * self.nx + i as usize)
    }

    pub fn get(&self, x: f32, z: f32) -> Option<T> {
        self.index(x, z).map(|k| self.data[k])
    }

    /// Paints every cell whose centre is inside the rings (even-odd, so holes
    /// stay unpainted) with `f(old value)`.
    pub fn fill_polygon(&mut self, rings: &[&[f32]], mut f: impl FnMut(T) -> T) {
        let (mut zmin, mut zmax) = (f32::INFINITY, f32::NEG_INFINITY);
        for ring in rings {
            for p in ring.chunks_exact(2) {
                zmin = zmin.min(p[1]);
                zmax = zmax.max(p[1]);
            }
        }
        if !zmin.is_finite() {
            return;
        }
        let j0 = (((zmin - self.z0) / self.cell).floor().max(0.0)) as usize;
        let j1 = (((zmax - self.z0) / self.cell).ceil().max(0.0) as usize).min(self.nz);
        let mut xs = Vec::new();
        for j in j0..j1 {
            let zc = self.z0 + (j as f32 + 0.5) * self.cell;
            xs.clear();
            for ring in rings {
                let n = ring.len() / 2;
                for i in 0..n {
                    let k = (i + 1) % n;
                    let (xa, za, xb, zb) = (ring[2 * i], ring[2 * i + 1], ring[2 * k], ring[2 * k + 1]);
                    if (za > zc) != (zb > zc) {
                        xs.push(xa + (zc - za) / (zb - za) * (xb - xa));
                    }
                }
            }
            xs.sort_by(|a, b| a.total_cmp(b));
            for span in xs.chunks_exact(2) {
                let i0 = ((span[0] - self.x0) / self.cell - 0.5).ceil().max(0.0) as usize;
                let i1 = (((span[1] - self.x0) / self.cell - 0.5).floor() + 1.0).max(0.0) as usize;
                for i in i0..i1.min(self.nx) {
                    let k = j * self.nx + i;
                    self.data[k] = f(self.data[k]);
                }
            }
        }
    }

    /// Paints a polyline `width` metres wide, with round-ish joints.
    pub fn stroke(&mut self, line: &[f32], width: f32, mut f: impl FnMut(T) -> T) {
        let h = width * 0.5;
        let pts: Vec<Vec2> = line.chunks_exact(2).map(|p| Vec2::new(p[0], p[1])).collect();
        for w in pts.windows(2) {
            let d = w[1] - w[0];
            if d.length_squared() < 1e-8 {
                continue;
            }
            let s = d.normalize().perp() * h;
            let quad = [
                w[0].x - s.x, w[0].y - s.y,
                w[1].x - s.x, w[1].y - s.y,
                w[1].x + s.x, w[1].y + s.y,
                w[0].x + s.x, w[0].y + s.y,
            ];
            self.fill_polygon(&[&quad], &mut f);
        }
        for p in &pts[1..pts.len().saturating_sub(1)] {
            let oct: Vec<f32> = (0..8)
                .flat_map(|k| {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    [p.x + h * a.cos(), p.y + h * a.sin()]
                })
                .collect();
            self.fill_polygon(&[&oct], &mut f);
        }
    }
}

impl Raster<f32> {
    /// Bilinear sample between cell centres, clamped at the edges.
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        let fx = ((x - self.x0) / self.cell - 0.5).clamp(0.0, (self.nx - 1) as f32);
        let fz = ((z - self.z0) / self.cell - 0.5).clamp(0.0, (self.nz - 1) as f32);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (i1, j1) = ((i + 1).min(self.nx - 1), (j + 1).min(self.nz - 1));
        let (u, v) = (fx - i as f32, fz - j as f32);
        let at = |i: usize, j: usize| self.data[j * self.nx + i];
        (at(i, j) * (1.0 - u) + at(i1, j) * u) * (1.0 - v) + (at(i, j1) * (1.0 - u) + at(i1, j1) * u) * v
    }

    /// Box blur of `radius` cells, `passes` times (three is close to a
    /// Gaussian).
    pub fn blur(&mut self, radius: usize, passes: usize) {
        let (nx, nz) = (self.nx, self.nz);
        let mut tmp = vec![0.0; self.data.len()];
        for _ in 0..passes {
            // Along rows into `tmp`, then along columns back.
            for (horizontal, n, m, stride, step) in [(true, nx, nz, nx, 1), (false, nz, nx, 1, nx)] {
                let (from, to) = if horizontal { (&self.data, &mut tmp) } else { (&tmp, &mut self.data) };
                for row in 0..m {
                    let base = row * stride;
                    let at = |i: isize| from[base + (i.clamp(0, n as isize - 1) as usize) * step];
                    let r = radius as isize;
                    let mut sum: f32 = (-r..=r).map(at).sum();
                    for i in 0..n as isize {
                        to[base + i as usize * step] = sum / (2 * r + 1) as f32;
                        sum += at(i + r + 1) - at(i - r);
                    }
                }
            }
        }
    }
}
