//! How a standing building is cut into cells, so a blast can take a bite out of it and what the
//! bite leaves hanging can fall.
//!
//! A building's walls and roof (the triangles `osm-import` made) are turned into cubes: the cells
//! of a regular grid they pass through. The building is drawn and collided with as those cubes.
//! The server decides which cells a blast removes and which are left unsupported, and tells
//! everyone the cell numbers; the clients cut the same cells the same way and draw and collide
//! with what is left. Everything that both sides must agree on is integer arithmetic on
//! centimetres, so a cell and the faces of the cubes are the same in Rust and in
//! `client/src/sim/carve.ts` (pinned by `cell_numbers_match_the_fixture` and `carve.spec.ts`).
//!
//! The floor is not carved: it is never part of the cells.

/// Side of a cell, centimetres: every building is cut into cubes of this size, on one lattice for
/// the whole world (cell corners at multiples of it), so neighbouring buildings' cells line up.
pub const CELL: i32 = 100;
/// Most cells in one building's grid (its bounding box). A building bigger than this is not carved.
pub const MAX_CELLS: usize = 4_000_000;

/// Health of one cell against bullets: a rifle needs a few rounds to break out a piece of wall.
/// Blasts take cells outright.
pub const CELL_HP: u8 = 40;

/// A triangle: three corners `[x, y, z]`, centimetres, flattened.
pub type Tri = [i32; 9];

/// A building's walls and roof as a grid of cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    /// Corner of the grid with the smallest coordinates, centimetres.
    pub origin: [i32; 3],
    /// Side of a cell, centimetres.
    pub cell: i32,
    /// Cells along x, y and z.
    pub dims: [usize; 3],
    /// Whether the cell is part of the building: a wall or the roof passes through it (see `Shell::new`).
    solid: Vec<bool>,
    /// Cells that hold the building up: its lowest layer, and any piece that never reached it.
    anchored: Vec<bool>,
}

/// What one blast did to a building: the cells it broke, and the cells that were left with nothing
/// to hold them and fell. Cell numbers are `Shell::index` (`(y * dims[2] + z) * dims[0] + x`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cut {
    pub blasted: Vec<u32>,
    pub fell: Vec<u32>,
}

impl Cut {
    pub fn is_empty(&self) -> bool {
        self.blasted.is_empty() && self.fell.is_empty()
    }
}

/// Longest gap between the points a wall or roof is sampled at to find the cells it passes
/// through, centimetres: half a cell, so it misses no cell it crosses by much.
const SAMPLE: i64 = CELL as i64 / 2;

/// Calls `visit` with points spread over triangle `tri`: `a + (i (b - a) + j (c - a)) / n` (rounded
/// down) for every `i, j >= 0` with `i + j <= n`, where `a` is the corner facing the longest edge
/// (the widest angle, so the rows of points are not drawn out along a sliver; the first such
/// corner on a tie), `b` and `c` the next ones round, and `n` the fewest steps that bring the
/// longest edge to within `SAMPLE` a step. Integer arithmetic only, so every implementation gets
/// the same points.
fn sample(tri: &Tri, mut visit: impl FnMut([i64; 3])) {
    let p = |k: usize| [tri[k * 3] as i64, tri[k * 3 + 1] as i64, tri[k * 3 + 2] as i64];
    let corners = [p(0), p(1), p(2)];
    let length = |k: usize| (0..3).map(|i| (corners[k][i] - corners[(k + 1) % 3][i]).pow(2)).sum::<i64>();
    let (mut edge, mut longest) = (0, length(0));
    for k in 1..3 {
        if length(k) > longest {
            (edge, longest) = (k, length(k));
        }
    }
    let (a, b, c) = (corners[(edge + 2) % 3], corners[edge], corners[(edge + 1) % 3]);
    let mut n = 1i64;
    while (n * SAMPLE).pow(2) < longest {
        n += 1;
    }
    let (ab, ac) = ([0, 1, 2].map(|i| b[i] - a[i]), [0, 1, 2].map(|i| c[i] - a[i]));
    for i in 0..=n {
        for j in 0..=n - i {
            visit([0, 1, 2].map(|k| a[k] + (i * ab[k] + j * ac[k]).div_euclid(n)));
        }
    }
}

impl Shell {
    /// Cuts a building: its wall triangles and its roof triangles, in centimetres, wound however
    /// the collision mesh has them. `None` when there is nothing to carve, or it is too big.
    ///
    /// A cell is solid when a wall or the roof passes through it: when a point the wall or roof is
    /// sampled at (`sample`) is in it. Where two solid cells then touch only along an edge, with both
    /// cells beside them empty, the one with the greater x (else the greater z) is filled as well, so
    /// there is no seeing through between them. That is decided from the cells before any are
    /// filled, so the order of filling does not matter.
    pub fn new(walls: &[Tri], roof: &[Tri]) -> Option<Self> {
        if walls.is_empty() && roof.is_empty() {
            return None;
        }
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for tri in walls.iter().chain(roof) {
            for k in 0..3 {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(tri[k * 3 + axis]);
                    hi[axis] = hi[axis].max(tri[k * 3 + axis]);
                }
            }
        }
        let origin = lo.map(|v| v.div_euclid(CELL) * CELL);
        // A face on a cell's boundary belongs to the cell below it (see `cell_at`): a box 5 m wide is 5 cells.
        let dims = [0, 1, 2].map(|a| ((hi[a] - 1 - origin[a]).div_euclid(CELL)).max(0) as usize + 1);
        if dims[0] * dims[1] * dims[2] > MAX_CELLS {
            return None;
        }
        let mut shell = Shell {
            origin,
            cell: CELL,
            dims,
            solid: vec![false; dims[0] * dims[1] * dims[2]],
            anchored: Vec::new(),
        };
        for tri in walls.iter().chain(roof) {
            sample(tri, |point| {
                let id = shell.cell_at(point);
                shell.solid[id as usize] = true;
            });
        }
        shell.close_gaps();
        shell.anchor();
        Some(shell)
    }

    /// Fills one of the two empty cells beside each pair of solid cells that touch only along an
    /// edge, in each of the three planes. See `Shell::new`.
    fn close_gaps(&mut self) {
        let was = self.solid.clone();
        let [nx, ny, nz] = self.dims;
        let at = |x: usize, y: usize, z: usize| was[self.index(x, y, z) as usize];
        let mut fill = Vec::new();
        for y in 0..ny {
            for z in 0..nz {
                for x in 0..nx {
                    if !at(x, y, z) {
                        continue;
                    }
                    // Plane (x, z): towards +x+z and +x-z.
                    if x + 1 < nx && z + 1 < nz && at(x + 1, y, z + 1) && !at(x + 1, y, z) && !at(x, y, z + 1) {
                        fill.push(self.index(x + 1, y, z));
                    }
                    if x + 1 < nx && z > 0 && at(x + 1, y, z - 1) && !at(x + 1, y, z) && !at(x, y, z - 1) {
                        fill.push(self.index(x + 1, y, z));
                    }
                    // Plane (x, y): towards +x+y and +x-y.
                    if x + 1 < nx && y + 1 < ny && at(x + 1, y + 1, z) && !at(x + 1, y, z) && !at(x, y + 1, z) {
                        fill.push(self.index(x + 1, y, z));
                    }
                    if x + 1 < nx && y > 0 && at(x + 1, y - 1, z) && !at(x + 1, y, z) && !at(x, y - 1, z) {
                        fill.push(self.index(x + 1, y, z));
                    }
                    // Plane (z, y): towards +z+y and +z-y.
                    if z + 1 < nz && y + 1 < ny && at(x, y + 1, z + 1) && !at(x, y, z + 1) && !at(x, y + 1, z) {
                        fill.push(self.index(x, y, z + 1));
                    }
                    if z + 1 < nz && y > 0 && at(x, y - 1, z + 1) && !at(x, y, z + 1) && !at(x, y - 1, z) {
                        fill.push(self.index(x, y, z + 1));
                    }
                }
            }
        }
        for id in fill {
            self.solid[id as usize] = true;
        }
    }

    /// The cell point `p` (centimetres) is in, clamped to the grid. A point on the boundary between
    /// two cells is in the lower one, so a wall that ends flush with a cell does not reach into the next.
    fn cell_at(&self, p: [i64; 3]) -> u32 {
        let at = |axis: usize| ((p[axis] - 1 - self.origin[axis] as i64).div_euclid(self.cell as i64)).clamp(0, self.dims[axis] as i64 - 1) as usize;
        self.index(at(0), at(1), at(2))
    }

    /// The number of the cell at `(x, y, z)`.
    pub fn index(&self, x: usize, y: usize, z: usize) -> u32 {
        ((y * self.dims[2] + z) * self.dims[0] + x) as u32
    }

    /// The `(x, y, z)` of cell `id`.
    pub fn coords(&self, id: u32) -> [usize; 3] {
        let id = id as usize;
        let x = id % self.dims[0];
        let z = (id / self.dims[0]) % self.dims[2];
        [x, id / (self.dims[0] * self.dims[2]), z]
    }

    /// The middle of cell `id`, centimetres.
    pub fn centre(&self, id: u32) -> [i32; 3] {
        let c = self.coords(id);
        [0, 1, 2].map(|a| self.origin[a] + c[a] as i32 * self.cell + self.cell / 2)
    }

    /// How many cells hold a piece of wall or roof.
    pub fn solid_count(&self) -> usize {
        self.solid.iter().filter(|&&s| s).count()
    }

    /// Whether cell `id` holds a piece of wall or roof.
    pub fn is_solid(&self, id: u32) -> bool {
        self.solid.get(id as usize).copied().unwrap_or(false)
    }

    /// Every cell number that holds something, ascending.
    pub fn solid_cells(&self) -> Vec<u32> {
        self.solid.iter().enumerate().filter(|&(_, &s)| s).map(|(i, _)| i as u32).collect()
    }

    /// The cells touching cell `id`, faces, edges and corners.
    fn around(&self, id: u32) -> impl Iterator<Item = u32> + '_ {
        let [x, y, z] = self.coords(id);
        (-1i32..=1).flat_map(move |dy| (-1i32..=1).flat_map(move |dz| (-1i32..=1).map(move |dx| (dx, dy, dz)))).filter_map(move |(dx, dy, dz)| {
            if (dx, dy, dz) == (0, 0, 0) {
                return None;
            }
            let (nx, ny, nz) = (x as i32 + dx, y as i32 + dy, z as i32 + dz);
            let inside = nx >= 0 && ny >= 0 && nz >= 0 && (nx as usize) < self.dims[0] && (ny as usize) < self.dims[1] && (nz as usize) < self.dims[2];
            inside.then(|| self.index(nx as usize, ny as usize, nz as usize))
        })
    }

    /// Marks the cells that hold the building up: those in its lowest layer, and every cell of a
    /// piece that does not touch that layer at all (which the importer made, and nobody cut).
    fn anchor(&mut self) {
        let n = self.solid.len();
        self.anchored = vec![false; n];
        let mut seen = vec![false; n];
        for start in 0..n {
            if !self.solid[start] || seen[start] {
                continue;
            }
            let mut piece = vec![start as u32];
            seen[start] = true;
            let mut at = 0;
            while at < piece.len() {
                let id = piece[at];
                at += 1;
                for next in self.around(id) {
                    if self.solid[next as usize] && !seen[next as usize] {
                        seen[next as usize] = true;
                        piece.push(next);
                    }
                }
            }
            let grounded = piece.iter().any(|&id| self.coords(id)[1] == 0);
            for id in piece {
                self.anchored[id as usize] = !grounded || self.coords(id)[1] == 0;
            }
        }
    }

    /// Takes the cells within `radius` centimetres of `at` out of `removed` (one flag per cell),
    /// roughened so the edge of the hole is ragged, then everything that is no longer connected to
    /// the ground. Cells already removed are left alone. However small the radius, the standing
    /// cell nearest `at` goes if it is within a cell's reach: a bullet always makes its hole.
    pub fn blast(&self, removed: &mut [bool], at: [i32; 3], radius: i32) -> Cut {
        let mut cut = Cut::default();
        let reach = (radius as i64 * 5 / 4).max(self.cell as i64);
        let range = |a: usize| {
            let lo = ((at[a] as i64 - reach - self.origin[a] as i64).div_euclid(self.cell as i64)).clamp(0, self.dims[a] as i64 - 1);
            let hi = ((at[a] as i64 + reach - self.origin[a] as i64).div_euclid(self.cell as i64)).clamp(0, self.dims[a] as i64 - 1);
            lo as usize..=hi as usize
        };
        let mut nearest: Option<(i64, u32)> = None;
        for y in range(1) {
            for z in range(2) {
                for x in range(0) {
                    let id = self.index(x, y, z);
                    if !self.solid[id as usize] || removed[id as usize] {
                        continue;
                    }
                    let c = self.centre(id);
                    let d = (0..3).map(|a| (c[a] as i64 - at[a] as i64).pow(2)).sum::<i64>();
                    if nearest.is_none_or(|(best, _)| d < best) {
                        nearest = Some((d, id));
                    }
                    // 0.75..1.25 of the radius, by the cell's number: the same bite always has the same edge.
                    let rough = 75 + id as i64 * 2654435761 % 51;
                    let r = radius as i64 * rough / 100;
                    if d <= r * r {
                        removed[id as usize] = true;
                        cut.blasted.push(id);
                    }
                }
            }
        }
        if cut.blasted.is_empty() {
            // Within a cell's reach: the cell the point is in, or the one next to it.
            match nearest {
                Some((d, id)) if d <= (self.cell as i64) * (self.cell as i64) * 3 / 2 => {
                    removed[id as usize] = true;
                    cut.blasted.push(id);
                }
                _ => return cut,
            }
        }
        cut.fell = self.unsupported(removed);
        for &id in &cut.fell {
            removed[id as usize] = true;
        }
        cut.blasted.sort_unstable();
        cut
    }

    /// A round of `amount` damage lands at `at` (centimetres): the standing cell nearest it, if it
    /// is within a cell's reach, loses that much of its `hp` (one number per cell) and breaks out
    /// at zero, taking with it whatever that leaves unsupported. A cell that is only hurt cuts
    /// nothing, and the result is empty.
    pub fn chip(&self, removed: &mut [bool], hp: &mut [u8], at: [i32; 3], amount: u8) -> Cut {
        let mut cut = Cut::default();
        let range = |a: usize| {
            let lo = ((at[a] as i64 - self.cell as i64 - self.origin[a] as i64).div_euclid(self.cell as i64)).clamp(0, self.dims[a] as i64 - 1);
            let hi = ((at[a] as i64 + self.cell as i64 - self.origin[a] as i64).div_euclid(self.cell as i64)).clamp(0, self.dims[a] as i64 - 1);
            lo as usize..=hi as usize
        };
        let mut nearest: Option<(i64, u32)> = None;
        for y in range(1) {
            for z in range(2) {
                for x in range(0) {
                    let id = self.index(x, y, z);
                    if !self.solid[id as usize] || removed[id as usize] {
                        continue;
                    }
                    let c = self.centre(id);
                    let d = (0..3).map(|a| (c[a] as i64 - at[a] as i64).pow(2)).sum::<i64>();
                    if nearest.is_none_or(|(best, _)| d < best) {
                        nearest = Some((d, id));
                    }
                }
            }
        }
        let Some((d, id)) = nearest else { return cut };
        if d > (self.cell as i64) * (self.cell as i64) * 3 / 2 {
            return cut;
        }
        let left = hp[id as usize].saturating_sub(amount);
        hp[id as usize] = left;
        if left > 0 {
            return cut;
        }
        removed[id as usize] = true;
        cut.blasted.push(id);
        cut.fell = self.unsupported(removed);
        for &lost in &cut.fell {
            removed[lost as usize] = true;
        }
        cut
    }

    /// The standing cells that no chain of standing cells joins to an anchor.
    fn unsupported(&self, removed: &[bool]) -> Vec<u32> {
        let n = self.solid.len();
        let mut held = vec![false; n];
        let mut queue: Vec<u32> = (0..n).filter(|&i| self.anchored[i] && !removed[i] && self.solid[i]).map(|i| i as u32).collect();
        for &id in &queue {
            held[id as usize] = true;
        }
        let mut at = 0;
        while at < queue.len() {
            let id = queue[at];
            at += 1;
            for next in self.around(id) {
                let i = next as usize;
                if self.solid[i] && !removed[i] && !held[i] {
                    held[i] = true;
                    queue.push(next);
                }
            }
        }
        (0..n).filter(|&i| self.solid[i] && !removed[i] && !held[i]).map(|i| i as u32).collect()
    }

    /// How many cells are still standing, given `removed`.
    pub fn standing(&self, removed: &[bool]) -> usize {
        (0..self.solid.len()).filter(|&i| self.solid[i] && !removed[i]).count()
    }

    /// What the building is collided with: the faces of its standing cubes (solid, not in `removed`)
    /// that look onto a cell with nothing standing in it, as triangles in centimetres, wound
    /// counter-clockwise seen from outside the cube. Neighbouring faces are merged into rectangles
    /// as `greedy` does, so a plain wall is two triangles a side. Mirrored by `Shell.faces`.
    pub fn faces(&self, removed: &[bool]) -> Vec<Tri> {
        let keys: Vec<i32> = (0..self.solid.len()).map(|i| if self.solid[i] && !removed[i] { 0 } else { -1 }).collect();
        let mut out = Vec::new();
        greedy(self.dims, &keys, |face| {
            let corner = |du: usize, dv: usize| {
                let mut p = [0i32; 3];
                p[face.d] = self.origin[face.d] + (face.i + usize::from(face.side > 0)) as i32 * self.cell;
                p[face.u] = self.origin[face.u] + (face.a + du * face.w) as i32 * self.cell;
                p[face.v] = self.origin[face.v] + (face.b + dv * face.h) as i32 * self.cell;
                p
            };
            let [p0, p1, p2, p3] = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
            let tri = |a: [i32; 3], b: [i32; 3], c: [i32; 3]| [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]];
            if face.side > 0 {
                out.extend([tri(p0, p1, p2), tri(p0, p2, p3)]);
            } else {
                out.extend([tri(p0, p2, p1), tri(p0, p3, p2)]);
            }
        });
        out
    }

    /// A fresh `hp`: every cell whole.
    pub fn fresh_hp(&self) -> Vec<u8> {
        vec![CELL_HP; self.solid.len()]
    }

    /// A fresh `removed`: nothing taken out.
    pub fn untouched(&self) -> Vec<bool> {
        vec![false; self.solid.len()]
    }
}

/// One merged face of `greedy`: `w × h` cells from `(a, b)` of layer `i` along axis `d`, facing
/// `side` (1 towards +d, -1 towards -d). `u` and `v` are the next axes round from `d`, so `u × v`
/// points along +d.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub d: usize,
    pub u: usize,
    pub v: usize,
    pub side: i32,
    pub i: usize,
    pub a: usize,
    pub b: usize,
    pub w: usize,
    pub h: usize,
    pub key: i32,
}

/// Greedy meshing of a grid of `dims` cells: `keys` holds a key for each cell (`Shell::index`
/// order), or -1 for an empty one. For each axis, each side and each layer of cells, the faces of
/// cells that look onto an empty cell (or out of the grid) are merged into rectangles of one key:
/// each grows first along `u`, then along `v` while the whole row matches. Calls `emit` with each,
/// in a fixed order, so the clients (`greedyFaces` in `client/src/sim/carve.ts`) make the same ones.
pub fn greedy(dims: [usize; 3], keys: &[i32], mut emit: impl FnMut(Face)) {
    // Step in the cell number for one cell along x, y and z.
    let stride = [1, dims[0] * dims[2], dims[0]];
    for d in 0..3 {
        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
        let (nu, nv, nd) = (dims[u], dims[v], dims[d]);
        let (su, sv, sd) = (stride[u], stride[v], stride[d]);
        let mut mask = vec![-1i32; nu * nv];
        for side in [1i32, -1] {
            for i in 0..nd {
                let j = i as i64 + side as i64;
                let beyond = j >= 0 && j < nd as i64;
                let mut any = false;
                for b in 0..nv {
                    for a in 0..nu {
                        let id = i * sd + a * su + b * sv;
                        let key = keys[id];
                        let covered = beyond && keys[(id as i64 + side as i64 * sd as i64) as usize] >= 0;
                        mask[b * nu + a] = if key >= 0 && !covered { key } else { -1 };
                        any |= mask[b * nu + a] >= 0;
                    }
                }
                if !any {
                    continue;
                }
                for b in 0..nv {
                    let mut a = 0;
                    while a < nu {
                        let key = mask[b * nu + a];
                        if key < 0 {
                            a += 1;
                            continue;
                        }
                        let mut w = 1;
                        while a + w < nu && mask[b * nu + a + w] == key {
                            w += 1;
                        }
                        let mut h = 1;
                        while b + h < nv && (0..w).all(|k| mask[(b + h) * nu + a + k] == key) {
                            h += 1;
                        }
                        for r in 0..h {
                            mask[(b + r) * nu + a..(b + r) * nu + a + w].fill(-1);
                        }
                        emit(Face { d, u, v, side, i, a, b, w, h, key });
                        a += w;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box `w × d × h` metres, walls only (two triangles a face side), corner at the origin.
    fn boxed(w: i32, d: i32, h: i32) -> Vec<Tri> {
        let (w, d, h) = (w * 100, d * 100, h * 100);
        let quad = |a: [i32; 3], b: [i32; 3], c: [i32; 3], e: [i32; 3]| -> [Tri; 2] {
            [[a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]], [a[0], a[1], a[2], c[0], c[1], c[2], e[0], e[1], e[2]]]
        };
        let mut out = Vec::new();
        out.extend(quad([0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]));
        out.extend(quad([w, 0, 0], [w, 0, d], [w, h, d], [w, h, 0]));
        out.extend(quad([w, 0, d], [0, 0, d], [0, h, d], [w, h, d]));
        out.extend(quad([0, 0, d], [0, 0, 0], [0, h, 0], [0, h, d]));
        out
    }

    fn roof(w: i32, d: i32, h: i32) -> Vec<Tri> {
        let (w, d, h) = (w * 100, d * 100, h * 100);
        vec![[0, h, 0, w, h, 0, w, h, d], [0, h, 0, w, h, d, 0, h, d]]
    }

    #[test]
    fn a_blast_on_the_wall_makes_a_hole_and_leaves_the_rest() {
        let shell = Shell::new(&boxed(10, 8, 6), &roof(10, 8, 6)).unwrap();
        assert_eq!(shell.cell, CELL);
        let mut removed = shell.untouched();
        let cut = shell.blast(&mut removed, [500, 300, 0], 150);
        assert!(!cut.blasted.is_empty());
        assert!(cut.fell.is_empty(), "a hole in a wall drops nothing: {:?}", cut.fell);
        let before = shell.solid_count();
        assert!(shell.standing(&removed) < before);
        // Every removed cell is within about 1.25 radius of the blast.
        for &id in &cut.blasted {
            let c = shell.centre(id);
            let d = (0..3).map(|a| (c[a] as f64 - [500.0, 300.0, 0.0][a]).powi(2)).sum::<f64>().sqrt();
            assert!(d <= 150.0 * 1.25 + 1.0, "{d}");
        }
        // Blasting the same place again takes nothing more.
        assert!(shell.blast(&mut removed, [500, 300, 0], 150).is_empty());
    }

    #[test]
    fn cutting_a_wall_through_drops_what_it_held() {
        let shell = Shell::new(&boxed(10, 8, 6), &roof(10, 8, 6)).unwrap();
        let mut removed = shell.untouched();
        // A band all round, a metre up: everything above is left on nothing.
        let mut fell = 0;
        for x in (0..=1000).step_by(100) {
            for z in [0, 800] {
                fell += shell.blast(&mut removed, [x, 150, z], 160).fell.len();
            }
        }
        for z in (0..=800).step_by(100) {
            for x in [0, 1000] {
                fell += shell.blast(&mut removed, [x, 150, z], 160).fell.len();
            }
        }
        assert!(fell > 0, "the top should have come down");
        assert!(shell.standing(&removed) < shell.solid_count() / 2);
    }

    #[test]
    fn a_cell_takes_a_few_rounds_before_it_breaks() {
        let shell = Shell::new(&boxed(10, 8, 6), &roof(10, 8, 6)).unwrap();
        let (mut removed, mut hp) = (shell.untouched(), shell.fresh_hp());
        let mut rounds = 0;
        loop {
            rounds += 1;
            let cut = shell.chip(&mut removed, &mut hp, [500, 300, 0], 12);
            if !cut.is_empty() {
                assert_eq!(cut.blasted.len(), 1);
                break;
            }
            assert!(rounds < 20);
        }
        assert_eq!(rounds, 4, "40 health, 12 a round");
        assert!(shell.chip(&mut removed, &mut hp, [500, 300, 0], 12).is_empty() || shell.standing(&removed) < shell.solid_count());
    }

    #[test]
    fn an_untouched_building_stands_whole() {
        let shell = Shell::new(&boxed(6, 6, 4), &roof(6, 6, 4)).unwrap();
        let removed = shell.untouched();
        assert!(shell.unsupported(&removed).is_empty());
    }

    #[test]
    fn a_box_collides_as_its_cubes() {
        // A box 6 × 6 × 4 m with a flat roof is a hollow of cubes: its outside is the box itself,
        // merged into one rectangle a side, and its inside is a metre in from each wall.
        let shell = Shell::new(&boxed(6, 6, 4), &roof(6, 6, 4)).unwrap();
        let faces = shell.faces(&shell.untouched());
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for tri in &faces {
            for k in 0..3 {
                for a in 0..3 {
                    lo[a] = lo[a].min(tri[k * 3 + a]);
                    hi[a] = hi[a].max(tri[k * 3 + a]);
                }
            }
        }
        assert_eq!((lo, hi), ([0, 0, 0], [600, 400, 600]));
        // The top: the whole roof, one rectangle at its height.
        let top = faces.iter().filter(|t| t[1] == 400 && t[4] == 400 && t[7] == 400).count();
        assert_eq!(top, 2);
        // A hole lets you in: more faces, the hole's sides.
        let mut removed = shell.untouched();
        shell.blast(&mut removed, [300, 150, 0], 100);
        assert!(shell.faces(&removed).len() > faces.len());
    }

    /// Mirrored byte for byte by `client/src/__tests__/carve.spec.ts`.
    #[test]
    fn cell_numbers_match_the_fixture() {
        let shell = Shell::new(&boxed(5, 4, 3), &roof(5, 4, 3)).unwrap();
        assert_eq!(shell.cell, 100);
        assert_eq!(shell.dims, [5, 3, 4]);
        let faces = shell.faces(&shell.untouched());
        let sum: i64 = faces.iter().flatten().enumerate().map(|(i, &v)| v as i64 * (i % 9 + 1) as i64).sum();
        assert_eq!((faces.len(), shell.solid_count(), sum), (28, 48, 267_900));
        let diagonal = Shell::new(&slanted(), &[]).unwrap();
        let cells: u64 = diagonal.solid_cells().iter().map(|&id| id as u64).sum();
        assert_eq!((diagonal.origin, diagonal.dims, diagonal.solid_count(), cells), ([-100, 0, 0], [9, 3, 6], 42, 3309));
        let faces = diagonal.faces(&diagonal.untouched());
        let sum: i64 = faces.iter().flatten().enumerate().map(|(i, &v)| v as i64 * (i % 9 + 1) as i64).sum();
        assert_eq!((faces.len(), sum), (72, 960_300));
    }

    /// A wall 3 m high from (-37, 0, 12) to (712, 0, 530) cm, slanting across the grid.
    fn slanted() -> Vec<Tri> {
        let (a, b, h) = ([-37, 12], [712, 530], 300);
        vec![[a[0], 0, a[1], b[0], 0, b[1], b[0], h, b[1]], [a[0], 0, a[1], b[0], h, b[1], a[0], h, a[1]]]
    }

    #[test]
    fn a_slanting_wall_has_no_gaps_between_its_cells() {
        let shell = Shell::new(&slanted(), &[]).unwrap();
        let [nx, ny, nz] = shell.dims;
        let solid = |x: usize, y: usize, z: usize| shell.is_solid(shell.index(x, y, z));
        for y in 0..ny {
            for z in 0..nz {
                for x in 0..nx - 1 {
                    for (dz, side) in [(1i32, 1i32), (-1, -1)] {
                        let w = z as i32 + dz;
                        if w < 0 || w >= nz as i32 {
                            continue;
                        }
                        let w = w as usize;
                        let edge_only = solid(x, y, z) && solid(x + 1, y, w) && !solid(x + 1, y, z) && !solid(x, y, w);
                        assert!(!edge_only, "a gap at ({x}, {y}, {z}) towards {side}");
                    }
                }
            }
        }
        // Every cell the wall's line runs well into is there: all but those it only clips a corner of.
        for step in 0..=100 {
            let (x, z) = (-37 + (712 + 37) * step / 100, 12 + (530 - 12) * step / 100);
            let inset = |v: i32, o: i32| (v - o).rem_euclid(CELL).min(CELL - (v - o).rem_euclid(CELL));
            if inset(x, shell.origin[0]) < 30 || inset(z, shell.origin[2]) < 30 {
                continue;
            }
            let cell = |v: i32, o: i32| ((v - o).div_euclid(CELL)) as usize;
            assert!(solid(cell(x, shell.origin[0]), 1, cell(z, shell.origin[2])), "({x}, {z})");
        }
    }
}
