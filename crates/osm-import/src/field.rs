//! Measured roofs: the building returns over a footprint as a height field, built as it is instead
//! of fitted with a parametric shape (`roof::fit`). Only a classified cloud (IGN's class 6) has
//! returns that are all roof, with no foliage among them, so only French worlds get these.
//!
//! The field is a grid of `STEP` cells, each the highest return in it. Cells without a return (a
//! glass roof, a shadow in the scan) are filled from their neighbours. The roof mesh is the field over
//! the footprint, cut along both the footprint's triangles and the grid's cells, and the walls rise
//! from the floor to the field along each edge. `voxels::bake` then cuts all of it into the
//! building's cubes like any other roof, so a spire, a stepped roof or a dome comes out as measured.

use shared::world::{CollisionMesh, ROOF_FLAT, ROOF_GABLE};
use std::collections::HashMap;

type P = [f64; 2];

/// Side of a field cell, metres: the MNT's pixel, finer than the 0.72 m cubes the roof becomes.
pub const STEP: f64 = 0.5;
/// Share of the footprint's cells that must hold a return for the field to stand for the roof.
const MIN_COVER: f64 = 0.35;
/// Fewest returns a field is made from.
const MIN_POINTS: usize = 12;
/// Smallest wall height under a roof, metres, as for fitted roofs.
const MIN_WALL: f64 = 2.5;
/// Below this slope (rise per run, about 9°) a cell is flat; a roof mostly of flat cells is drawn as
/// a flat roof.
const FLAT_SLOPE: f64 = 0.16;
const FLAT_SHARE: f64 = 0.6;

/// A roof measured over a footprint: heights of `STEP` cells from `(x0, z0)`, local y metres.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    x0: f64,
    z0: f64,
    cols: usize,
    rows: usize,
    top: Vec<f32>,
}

impl Field {
    /// The field of `points` (`[x, z, y, _]`, as `roof::Cloud::over` gives them) over `footprint`,
    /// or `None` when they cover too little of it to be the roof.
    pub fn measure(footprint: &[P], points: &[[f64; 4]]) -> Option<Field> {
        if points.len() < MIN_POINTS || footprint.len() < 3 {
            return None;
        }
        let (lo, hi) = bounds(footprint);
        let (cols, rows) = (((hi[0] - lo[0]) / STEP).ceil().max(1.0) as usize, ((hi[1] - lo[1]) / STEP).ceil().max(1.0) as usize);
        let mut top = vec![f32::NAN; cols * rows];
        for p in points {
            let (i, j) = (((p[0] - lo[0]) / STEP).floor(), ((p[1] - lo[1]) / STEP).floor());
            if i < 0.0 || j < 0.0 || i as usize >= cols || j as usize >= rows {
                continue;
            }
            let cell = &mut top[j as usize * cols + i as usize];
            if cell.is_nan() || *cell < p[2] as f32 {
                *cell = p[2] as f32;
            }
        }
        let centre = |i: usize, j: usize| [lo[0] + (i as f64 + 0.5) * STEP, lo[1] + (j as f64 + 0.5) * STEP];
        let (mut wanted, mut known) = (0usize, 0usize);
        for j in 0..rows {
            for i in 0..cols {
                if inside(centre(i, j), footprint) {
                    wanted += 1;
                    known += usize::from(!top[j * cols + i].is_nan());
                }
            }
        }
        if wanted == 0 || (known as f64) < MIN_COVER * wanted as f64 {
            return None;
        }
        fill(&mut top, cols, rows);
        Some(Field { x0: lo[0], z0: lo[1], cols, rows, top })
    }

    fn at(&self, i: isize, j: isize) -> f64 {
        let (i, j) = (i.clamp(0, self.cols as isize - 1) as usize, j.clamp(0, self.rows as isize - 1) as usize);
        f64::from(self.top[j * self.cols + i])
    }

    /// The roof's height over `(x, z)`: the cell centres' heights, bilinearly between them.
    pub fn height(&self, [x, z]: P) -> f64 {
        let (u, v) = ((x - self.x0) / STEP - 0.5, (z - self.z0) / STEP - 0.5);
        let (i, j) = (u.floor(), v.floor());
        let (fu, fv) = (u - i, v - j);
        let (i, j) = (i as isize, j as isize);
        let row = |j: isize| self.at(i, j) * (1.0 - fu) + self.at(i + 1, j) * fu;
        row(j) * (1.0 - fv) + row(j + 1) * fv
    }

    /// `ROOF_FLAT` when most of the roof is flat, else `ROOF_GABLE`, for the palette it is drawn in.
    pub fn kind(&self) -> u8 {
        let mut flat = 0;
        for j in 0..self.rows as isize {
            for i in 0..self.cols as isize {
                let (dx, dz) = ((self.at(i + 1, j) - self.at(i - 1, j)) / (2.0 * STEP), (self.at(i, j + 1) - self.at(i, j - 1)) / (2.0 * STEP));
                flat += usize::from(dx.hypot(dz) < FLAT_SLOPE);
            }
        }
        if flat as f64 >= FLAT_SHARE * (self.cols * self.rows) as f64 { ROOF_FLAT } else { ROOF_GABLE }
    }
}

/// Fills the cells without a return: first from the mean of their known neighbours, ring by ring
/// inwards, then whatever is left (a gap wider than that) with the median of the known cells.
fn fill(top: &mut [f32], cols: usize, rows: usize) {
    for _ in 0..8 {
        let before = top.to_vec();
        let mut changed = false;
        for j in 0..rows {
            for i in 0..cols {
                if !before[j * cols + i].is_nan() {
                    continue;
                }
                let (mut sum, mut count) = (0.0f32, 0);
                for (di, dj) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let (a, b) = (i as isize + di, j as isize + dj);
                    if a < 0 || b < 0 || a as usize >= cols || b as usize >= rows {
                        continue;
                    }
                    let v = before[b as usize * cols + a as usize];
                    if !v.is_nan() {
                        sum += v;
                        count += 1;
                    }
                }
                if count >= 2 {
                    top[j * cols + i] = sum / count as f32;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut known: Vec<f32> = top.iter().copied().filter(|v| !v.is_nan()).collect();
    if known.is_empty() {
        return;
    }
    known.sort_by(f32::total_cmp);
    let median = known[known.len() / 2];
    top.iter_mut().filter(|v| v.is_nan()).for_each(|v| *v = median);
}

fn bounds(points: &[P]) -> (P, P) {
    points.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]))
}

fn inside([x, z]: P, ring: &[P]) -> bool {
    let mut hit = false;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            hit = !hit;
        }
    }
    hit
}

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
}

/// Clips a convex polygon to the half-plane `axis` ≥ `at` (`keep_above`) or ≤ `at`.
fn clip(polygon: &[P], axis: usize, at: f64, keep_above: bool) -> Vec<P> {
    let keeps = |p: &P| if keep_above { p[axis] >= at } else { p[axis] <= at };
    let mut out = Vec::with_capacity(polygon.len() + 2);
    for k in 0..polygon.len() {
        let (a, b) = (polygon[k], polygon[(k + 1) % polygon.len()]);
        if keeps(&a) {
            out.push(a);
        }
        if keeps(&a) != keeps(&b) {
            let t = (at - a[axis]) / (b[axis] - a[axis]);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}

/// Appends a building with a measured roof to `mesh`, in the order `roof::build` does: walls, roof,
/// then floor. `footprint` is counter-clockwise from above; the floor is at `floor` metres.
pub fn build(mesh: &mut CollisionMesh, footprint: &[P], floor: f64, field: &Field) -> crate::roof::Parts {
    let mut ids: HashMap<[i32; 3], u32> = HashMap::new();
    let mut vertex = |mesh: &mut CollisionMesh, x: f64, y: f64, z: f64| -> u32 {
        let key = [cm(x), cm(y), cm(z)];
        *ids.entry(key).or_insert_with(|| {
            mesh.vertices.push(key);
            mesh.vertices.len() as u32 - 1
        })
    };
    let push = |mesh: &mut CollisionMesh, tri: [u32; 3]| {
        if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
            mesh.triangles.push(tri);
        }
    };
    let roof_y = |p: P| field.height(p).max(floor + MIN_WALL);
    let mut highest = floor;

    // Walls, each edge cut every half cell so the top follows the field.
    let first_wall = mesh.triangles.len();
    for k in 0..footprint.len() {
        let (a, b) = (footprint[k], footprint[(k + 1) % footprint.len()]);
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / (STEP / 2.0)).ceil().max(1.0) as usize;
        let at = |s: usize| {
            let t = s as f64 / steps as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        };
        for s in 0..steps {
            let (p, q) = (at(s), at(s + 1));
            let (tp, tq) = (roof_y(p), roof_y(q));
            highest = highest.max(tp).max(tq);
            let (bp, bq) = (vertex(mesh, p[0], floor, p[1]), vertex(mesh, q[0], floor, q[1]));
            let (up, uq) = (vertex(mesh, p[0], tp, p[1]), vertex(mesh, q[0], tq, q[1]));
            push(mesh, [bp, bq, uq]);
            push(mesh, [bp, uq, up]);
        }
    }
    let walls = (mesh.triangles.len() - first_wall) as u32;

    // Roof: each of the footprint's triangles cut into the grid's cells, fan-triangulated.
    let flat: Vec<f64> = footprint.iter().flat_map(|p| [p[0], p[1]]).collect();
    let triangles: Vec<[usize; 3]> = earcutr::earcut(&flat, &[], 2).map(|i| i.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect()).unwrap_or_default();
    let first_roof = mesh.triangles.len();
    for t in &triangles {
        let tri: Vec<P> = t.iter().map(|&k| footprint[k]).collect();
        let (lo, hi) = bounds(&tri);
        let cell = |v: f64, origin: f64| ((v - origin) / STEP).floor() as i64;
        for j in cell(lo[1], field.z0)..=cell(hi[1], field.z0) {
            for i in cell(lo[0], field.x0)..=cell(hi[0], field.x0) {
                let (x0, z0) = (field.x0 + i as f64 * STEP, field.z0 + j as f64 * STEP);
                let mut piece = clip(&tri, 0, x0, true);
                piece = clip(&piece, 0, x0 + STEP, false);
                piece = clip(&piece, 1, z0, true);
                piece = clip(&piece, 1, z0 + STEP, false);
                if piece.len() < 3 {
                    continue;
                }
                let ids: Vec<u32> = piece
                    .iter()
                    .map(|&p| {
                        let y = roof_y(p);
                        highest = highest.max(y);
                        vertex(mesh, p[0], y, p[1])
                    })
                    .collect();
                for k in 1..ids.len() - 1 {
                    push(mesh, [ids[0], ids[k], ids[k + 1]]);
                }
            }
        }
    }
    let roof = (mesh.triangles.len() - first_roof) as u32;

    // Floor, facing down.
    for [a, b, c] in &triangles {
        let ids = [*c, *b, *a].map(|k| vertex(mesh, footprint[k][0], floor, footprint[k][1]));
        push(mesh, ids);
    }
    crate::roof::Parts { walls, roof, top: highest }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns every 0.25 m over `[x0, x1] × [z0, z1]`, at `y(x, z)`.
    fn scan(x0: f64, x1: f64, z0: f64, z1: f64, y: impl Fn(f64, f64) -> f64) -> Vec<[f64; 4]> {
        let mut out = Vec::new();
        let mut z = z0 + 0.125;
        while z < z1 {
            let mut x = x0 + 0.125;
            while x < x1 {
                out.push([x, z, y(x, z), 0.0]);
                x += 0.25;
            }
            z += 0.25;
        }
        out
    }

    const SQUARE: [P; 4] = [[0.0, 0.0], [0.0, 20.0], [20.0, 20.0], [20.0, 0.0]];

    #[test]
    fn a_tower_on_a_podium_keeps_both_heights() {
        let points = scan(0.0, 20.0, 0.0, 20.0, |x, z| if (5.0..10.0).contains(&x) && (5.0..10.0).contains(&z) { 30.0 } else { 8.0 });
        let field = Field::measure(&SQUARE, &points).unwrap();
        assert!((field.height([7.5, 7.5]) - 30.0).abs() < 1e-6);
        assert!((field.height([15.0, 15.0]) - 8.0).abs() < 1e-6);
        assert_eq!(field.kind(), ROOF_FLAT);
    }

    #[test]
    fn gaps_in_the_scan_are_filled_from_around() {
        // A 4 m hole in the returns, as under a skylight.
        let points: Vec<[f64; 4]> = scan(0.0, 20.0, 0.0, 20.0, |_, _| 9.0).into_iter().filter(|p| !((8.0..12.0).contains(&p[0]) && (8.0..12.0).contains(&p[1]))).collect();
        let field = Field::measure(&SQUARE, &points).unwrap();
        assert!((field.height([10.0, 10.0]) - 9.0).abs() < 1e-6);
    }

    #[test]
    fn too_few_returns_are_not_a_roof() {
        let points = scan(0.0, 5.0, 0.0, 5.0, |_, _| 9.0);
        assert!(Field::measure(&SQUARE, &points).is_none());
    }

    #[test]
    fn a_pitched_roof_is_drawn_as_one() {
        let points = scan(0.0, 20.0, 0.0, 20.0, |x, _| 12.0 - 0.6 * (x - 10.0).abs());
        assert_eq!(Field::measure(&SQUARE, &points).unwrap().kind(), ROOF_GABLE);
    }

    #[test]
    fn the_mesh_covers_the_footprint_and_reaches_the_tower() {
        let points = scan(0.0, 20.0, 0.0, 20.0, |x, z| if (5.0..10.0).contains(&x) && (5.0..10.0).contains(&z) { 30.0 } else { 8.0 });
        let field = Field::measure(&SQUARE, &points).unwrap();
        let ccw: Vec<P> = SQUARE.iter().rev().copied().collect();
        let mut mesh = CollisionMesh::default();
        let parts = build(&mut mesh, &ccw, 0.0, &field);
        assert!((parts.top - 30.0).abs() < 1e-6);
        // The roof's triangles cover the footprint once, seen from above.
        let roof = &mesh.triangles[parts.walls as usize..(parts.walls + parts.roof) as usize];
        let area: f64 = roof
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|v| mesh.vertices[v as usize]).map(|v| [f64::from(v[0]) / 100.0, f64::from(v[2]) / 100.0]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0
            })
            .sum();
        assert!((area - 400.0).abs() < 0.5, "{area}");
        // And it cuts into cubes like any roof.
        let tri = |t: &[u32; 3]| {
            let [a, b, c] = t.map(|v| mesh.vertices[v as usize]);
            [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]]
        };
        let walls: Vec<_> = mesh.triangles[..parts.walls as usize].iter().map(tri).collect();
        let roofs: Vec<_> = roof.iter().map(tri).collect();
        let shell = shared::carve::Shell::new(&walls, &roofs).unwrap();
        assert_eq!(shell.cells().dims[1], 3000_u32.div_ceil(72));
    }
}
