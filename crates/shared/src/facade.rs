//! A looked building's walls and roof as they stand, for its collider: the windows cut out of the
//! walls (`windows`), and the pieces lying in blasted cells gone, so what is collided with is what
//! the client draws (`client/src/render/facade.ts` and `lookfracture.ts` cut the same way). A whole
//! building is its own few triangles; only round a hole is it sliced along the cell grid.

use crate::carve::{Shell, Tri};
use crate::windows::Window;

pub type P = [f64; 3];
pub type Poly = Vec<P>;

const EPS: f64 = 1e-4;
/// Cells round a face's box within which a hole slices it.
const SLICE_REACH: i64 = 2;

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn lerp(a: P, b: P, t: f64) -> P {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A look triangle (nine centimetre numbers) as a polygon in metres, wound outwards (the look is wound inwards).
pub fn poly_of(t: &Tri) -> Poly {
    let p = |k: usize| [t[k] as f64 / 100.0, t[k + 1] as f64 / 100.0, t[k + 2] as f64 / 100.0];
    vec![p(0), p(6), p(3)]
}

/// The unit normal `poly` winds towards (Newell's method).
pub fn normal_of(poly: &[P]) -> P {
    let mut n = [0.0; 3];
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l < 1e-12 { [0.0, 1.0, 0.0] } else { [n[0] / l, n[1] / l, n[2] / l] }
}

pub fn centre_of(poly: &[P]) -> P {
    let mut c = [0.0; 3];
    for p in poly {
        for a in 0..3 {
            c[a] += p[a] / poly.len() as f64;
        }
    }
    c
}

/// `poly` cut by the plane `n · p = d` into the part behind and the part in front; either may be empty.
pub fn split_plane(poly: &[P], n: P, d: f64) -> (Poly, Poly) {
    let (mut behind, mut front) = (Vec::new(), Vec::new());
    let side = |p: P| {
        let s = dot(p, n) - d;
        if s > EPS { 1 } else if s < -EPS { -1 } else { 0 }
    };
    let mut all_on = true;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa != 0 {
            all_on = false;
        }
        if sa <= 0 {
            behind.push(a);
        }
        if sa >= 0 {
            front.push(a);
        }
        if sa * sb < 0 {
            let t = (d - dot(a, n)) / (dot(b, n) - dot(a, n));
            let cut = lerp(a, b, t);
            behind.push(cut);
            front.push(cut);
        }
    }
    if all_on {
        return (Vec::new(), poly.to_vec());
    }
    (if behind.len() >= 3 { behind } else { Vec::new() }, if front.len() >= 3 { front } else { Vec::new() })
}

/// `walls` with the windows (not the doors) of `windows` cut out of the ones in their planes.
pub fn cut_windows(mut walls: Vec<Poly>, windows: &[Window]) -> Vec<Poly> {
    for w in windows.iter().filter(|w| !w.door) {
        let [a, b, _, d] = w.corners.map(|c| c.map(f64::from));
        let n = w.normal.map(f64::from);
        let width = (b[0] - a[0]).hypot(b[2] - a[2]);
        let along = [(b[0] - a[0]) / width, 0.0, (b[2] - a[2]) / width];
        let plane_d = dot(a, n);
        let (u0, u1, v0, v1) = (dot(a, along), dot(b, along), a[1], d[1]);
        let mut out = Vec::with_capacity(walls.len());
        for wall in walls {
            let wn = normal_of(&wall);
            if dot(wn, n) < 0.999 || (dot(wall[0], n) - plane_d).abs() > 0.03 {
                out.push(wall);
                continue;
            }
            let mut pieces = vec![wall];
            for (pn, pd) in [(along, u0), (along, u1), ([0.0, 1.0, 0.0], v0), ([0.0, 1.0, 0.0], v1)] {
                let mut next = Vec::new();
                for piece in &pieces {
                    let (behind, front) = split_plane(piece, pn, pd);
                    for part in [behind, front] {
                        if part.len() >= 3 {
                            next.push(part);
                        }
                    }
                }
                pieces = next;
            }
            for piece in pieces {
                let c = centre_of(&piece);
                let (u, v) = (dot(c, along), c[1]);
                if u > u0 && u < u1 && v > v0 && v < v1 {
                    continue;
                }
                out.push(piece);
            }
        }
        walls = out;
    }
    walls
}

/// The building's grid, in metres.
struct Grid<'a> {
    shell: &'a Shell,
    cell: f64,
    origin: [f64; 3],
}

impl Grid<'_> {
    fn of(&self, v: f64, axis: usize) -> i64 {
        ((v - self.origin[axis]) / self.cell).floor() as i64
    }
    fn has(&self, c: [i64; 3]) -> bool {
        (0..3).all(|a| c[a] >= 0 && c[a] < self.shell.dims[a] as i64)
    }
    fn index(&self, c: [i64; 3]) -> u32 {
        self.shell.index(c[0] as usize, c[1] as usize, c[2] as usize)
    }
}

/// Whether the wall in cell `c` is left with nothing to hang from: the cell is not the building's own
/// and every cell of the building round it is gone (`removed`). It goes with them.
fn orphan(grid: &Grid, removed: &[bool], c: [i64; 3]) -> bool {
    if grid.has(c) && grid.shell.is_solid(grid.index(c)) {
        return false;
    }
    let mut lost = false;
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let at = [c[0] + dx, c[1] + dy, c[2] + dz];
                if !grid.has(at) {
                    continue;
                }
                let id = grid.index(at) as usize;
                if !grid.shell.is_solid(id as u32) {
                    continue;
                }
                if !removed.get(id).copied().unwrap_or(false) {
                    return false;
                }
                lost = true;
            }
        }
    }
    lost
}

/// The faces of a building whose look is `walls` and `roof` (nine centimetre numbers a triangle)
/// as they stand: the windows cut out, and with `cut` (its cells and which are gone) the pieces in
/// gone cells left out, the rest sliced along the cell grid near them. Three corners a triangle,
/// metres; nothing degenerate.
pub fn standing(walls: &[Tri], roof: &[Tri], windows: &[Window], cut: Option<(&Shell, &[bool])>) -> Vec<[f32; 3]> {
    let wall_polys = cut_windows(walls.iter().map(poly_of).collect(), windows);
    let polys: Vec<Poly> = wall_polys.into_iter().chain(roof.iter().map(poly_of)).collect();
    let mut out = Vec::new();
    let mut emit = |poly: &[P]| {
        for i in 1..poly.len() - 1 {
            let (a, b, c) = (poly[0], poly[i], poly[i + 1]);
            let (e, f) = (sub(b, a), sub(c, a));
            let n = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
            if dot(n, n) < 1e-10 {
                continue;
            }
            out.extend([a, b, c].map(|p| p.map(|v| v as f32)));
        }
    };
    let Some((shell, removed)) = cut else {
        for poly in &polys {
            emit(poly);
        }
        return out;
    };
    let grid = Grid { shell, cell: shell.cell as f64 / 100.0, origin: shell.origin.map(|v| v as f64 / 100.0) };
    let near: Vec<[i64; 3]> = (0..removed.len()).filter(|&id| removed[id]).map(|id| shell.coords(id as u32).map(|v| v as i64)).collect();
    for poly in &polys {
        let normal = normal_of(poly);
        let lo: Vec<i64> = (0..3).map(|a| grid.of(poly.iter().map(|p| p[a]).fold(f64::INFINITY, f64::min), a) - SLICE_REACH).collect();
        let hi: Vec<i64> = (0..3).map(|a| grid.of(poly.iter().map(|p| p[a]).fold(f64::NEG_INFINITY, f64::max), a) + SLICE_REACH).collect();
        let close = near.iter().any(|c| (0..3).all(|a| c[a] >= lo[a] && c[a] <= hi[a]));
        if !close {
            emit(poly);
            continue;
        }
        let mut pieces = vec![poly.clone()];
        for axis in 0..3 {
            let n = [(axis == 0) as i32 as f64, (axis == 1) as i32 as f64, (axis == 2) as i32 as f64];
            let (lo, hi) = (poly.iter().map(|p| p[axis]).fold(f64::INFINITY, f64::min), poly.iter().map(|p| p[axis]).fold(f64::NEG_INFINITY, f64::max));
            let mut k = ((lo - grid.origin[axis]) / grid.cell).floor() as i64 + 1;
            while grid.origin[axis] + k as f64 * grid.cell < hi {
                let at = grid.origin[axis] + k as f64 * grid.cell;
                let mut next = Vec::new();
                for piece in &pieces {
                    let (behind, front) = split_plane(piece, n, at);
                    for part in [behind, front] {
                        if part.len() >= 3 {
                            next.push(part);
                        }
                    }
                }
                pieces = next;
                k += 1;
            }
        }
        for piece in pieces {
            let c = centre_of(&piece);
            // A hair inside the wall, so a wall on a cell boundary is judged by the cell it is cut into.
            let at: [i64; 3] = std::array::from_fn(|a| grid.of(c[a] - normal[a] * 0.01, a));
            let gone = grid.has(at) && removed.get(grid.index(at) as usize).copied().unwrap_or(false);
            if gone || orphan(&grid, removed, at) {
                continue;
            }
            emit(&piece);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::openings_of;
    use crate::world::Look;

    fn quad(a: [i32; 3], b: [i32; 3], c: [i32; 3], e: [i32; 3]) -> [Tri; 2] {
        [[a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]], [a[0], a[1], a[2], c[0], c[1], c[2], e[0], e[1], e[2]]]
    }

    /// A box house 5 × 4 × 3 m, wound inwards.
    fn house() -> (Vec<Tri>, Vec<Tri>) {
        let (w, d, h) = (500, 400, 300);
        let walls: Vec<Tri> = [quad([0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]), quad([w, 0, 0], [w, 0, d], [w, h, d], [w, h, 0]), quad([w, 0, d], [0, 0, d], [0, h, d], [w, h, d]), quad([0, 0, d], [0, 0, 0], [0, h, 0], [0, h, d])].concat();
        let roof = quad([0, h, 0], [w, h, 0], [w, h, d], [0, h, d]).to_vec();
        (walls, roof)
    }

    fn look_of(walls: &[Tri], roof: &[Tri]) -> Look {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for t in walls.iter().chain(roof) {
            let base = vertices.len() as u32;
            vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
            triangles.push([base, base + 1, base + 2]);
        }
        Look { vertices, triangles, buildings: vec![[0, walls.len() as u32, roof.len() as u32]] }
    }

    /// Whether a ray straight at the front wall (z = 0) at `(x, y)` meets a triangle of `tris` in that plane.
    fn covered(tris: &[[f32; 3]], x: f32, y: f32) -> bool {
        tris.chunks_exact(3).any(|t| {
            if t.iter().any(|p| p[2].abs() > 1e-4) {
                return false;
            }
            let (a, b, c) = (t[0], t[1], t[2]);
            let d = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            let u = ((x - a[0]) * (c[1] - a[1]) - (y - a[1]) * (c[0] - a[0])) / d;
            let v = ((b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])) / d;
            u >= 0.0 && v >= 0.0 && u + v <= 1.0
        })
    }

    #[test]
    fn the_windows_are_holes_in_the_walls_and_the_rest_stands() {
        let (walls, roof) = house();
        let look = look_of(&walls, &roof);
        let windows = openings_of(&look, look.buildings[0]);
        let tris = standing(&walls, &roof, &windows, None);
        let pane = windows.iter().find(|o| !o.door && o.normal[2] < -0.9).unwrap();
        let [a, b, _, d] = pane.corners;
        assert!(!covered(&tris, (a[0] + b[0]) / 2.0, (a[1] + d[1]) / 2.0), "through the window");
        assert!(covered(&tris, a[0] - 0.2, (a[1] + d[1]) / 2.0), "the wall beside it");
        assert!(covered(&tris, 0.2, 0.2), "the wall's corner");
        let door = windows.iter().find(|o| o.door).unwrap();
        let [a, b, _, d] = door.corners;
        assert!(covered(&tris, (a[0] + b[0]) / 2.0, (a[1] + d[1]) / 2.0), "a door is no hole");
    }

    #[test]
    fn a_blasted_cell_takes_its_piece_of_wall_out_and_leaves_the_rest() {
        let (walls, roof) = house();
        let shell = Shell::new(&walls, &roof).unwrap();
        let mut removed = shell.untouched();
        // The cell in the front wall (z = 0) a little left of the middle, 1.5 m up: the solid one whose middle is nearest.
        let count = shell.dims[0] * shell.dims[1] * shell.dims[2];
        let hole = (0..count as u32).filter(|&id| shell.is_solid(id)).min_by_key(|&id| { let c = shell.centre(id); (c[0] - 80).pow(2) + (c[1] - 150).pow(2) + c[2].pow(2) }).unwrap();
        removed[hole as usize] = true;
        let tris = standing(&walls, &roof, &[], Some((&shell, &removed)));
        let c = shell.centre(hole).map(|v| v as f32 / 100.0);
        assert!(!covered(&tris, c[0], c[1]), "the hole");
        assert!(covered(&tris, 4.5, 0.3), "the far corner stands");
        let whole = standing(&walls, &roof, &[], None);
        assert!(tris.len() > whole.len(), "sliced round the hole, so more triangles");
    }
}
