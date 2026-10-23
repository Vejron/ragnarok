//! How a standing building is cut into cells, so a blast can take a bite out of it and what the
//! bite leaves hanging can fall.
//!
//! A building's walls and roof (the triangles `osm-import` made) are turned into cubes: the cells
//! of a regular grid they pass through. The building is drawn and collided with as those cubes.
//! The server decides which cells a blast removes and which are left unsupported, and tells
//! everyone the cell numbers; the clients cut the same cells with this same code, built to
//! WebAssembly (`crates/sim-wasm`: the physics through `ClientSim`, the drawing through the `carve_*`
//! functions that `client/src/sim/carve.ts` wraps), and draw and collide with what is left. It is
//! integer arithmetic on centimetres, and `cell_numbers_match_the_fixture` and `carve.spec.ts` pin
//! the numbers.
//!
//! The floor is not carved: it is never part of the cells.
//!
//! The importer cuts every building once (`Shell::new`) and bakes the cells into the tile
//! (`Shell::cells`, `world::Cells`), with the faces of the cubes in the collision mesh in place of
//! the walls and roof; the server and clients only read them back (`Shell::from_cells`).

use crate::world::{Bits, Cells};

/// Side of a cell, centimetres: every building is cut into cubes of this size, on one lattice for
/// the whole world (cell corners at multiples of it), so neighbouring buildings' cells line up.
pub const CELL: i32 = 72;
/// Most cells in one building's grid (its bounding box). A building bigger than this is not carved.
pub const MAX_CELLS: usize = 8_000_000;

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
    /// Whether the roof passes through the cell, for its colour. A cell filled to close a gap is
    /// roof when either cell it joins is.
    roof: Vec<bool>,
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

/// How a building's standing cells are held up (`Shell::hold`), one number per cell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hold {
    span: Vec<u16>,
    load: Vec<u32>,
}

/// The `span` of a cell no chain of cells joins to an anchor.
const UNHELD: u16 = u16::MAX;
/// How much longer a cell's chain to an anchor may get than in the whole building before it lets
/// go: 5 cells sideways, about 3.6 m of roof or floor hanging out past its last support.
const SPAN_SLACK: u16 = 10;
/// A cell fails when it carries more than `LOAD_FACTOR` times what it did in the whole building,
/// plus `LOAD_SLACK`: the pillar left holding up a roof that a wall shared with it.
const LOAD_FACTOR: u32 = 2;
const LOAD_SLACK: u32 = 4;

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
            roof: vec![false; dims[0] * dims[1] * dims[2]],
            anchored: Vec::new(),
        };
        for (list, is_roof) in [(walls, false), (roof, true)] {
            for tri in list {
                sample(tri, |point| {
                    let id = shell.cell_at(point) as usize;
                    shell.solid[id] = true;
                    shell.roof[id] |= is_roof;
                });
            }
        }
        shell.close_gaps();
        shell.anchor();
        Some(shell)
    }

    /// The cells as the importer bakes them into the tile (`pack`).
    pub fn cells(&self) -> Cells {
        let layer = self.dims[0] * self.dims[2];
        Cells { origin: self.origin, dims: self.dims.map(|d| d as u32), solid: Bits(pack(&self.solid, layer)), roof: Bits(pack(&self.roof, layer)) }
    }

    /// The building cut as the importer baked it. `None` when it has no cells.
    pub fn from_cells(cells: &Cells) -> Option<Self> {
        let dims = cells.dims.map(|d| d as usize);
        let n = dims[0] * dims[1] * dims[2];
        if n == 0 || n > MAX_CELLS {
            return None;
        }
        let mut shell = Shell {
            origin: cells.origin,
            cell: CELL,
            dims,
            solid: unpack(&cells.solid.0, n, dims[0] * dims[2]),
            roof: unpack(&cells.roof.0, n, dims[0] * dims[2]),
            anchored: Vec::new(),
        };
        shell.anchor();
        Some(shell)
    }

    /// Whether the roof passes through cell `id`.
    pub fn is_roof(&self, id: u32) -> bool {
        self.roof.get(id as usize).copied().unwrap_or(false)
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
                    let here = self.index(x, y, z);
                    // Plane (x, z): towards +x+z and +x-z.
                    if x + 1 < nx && z + 1 < nz && at(x + 1, y, z + 1) && !at(x + 1, y, z) && !at(x, y, z + 1) {
                        fill.push((self.index(x + 1, y, z), here, self.index(x + 1, y, z + 1)));
                    }
                    if x + 1 < nx && z > 0 && at(x + 1, y, z - 1) && !at(x + 1, y, z) && !at(x, y, z - 1) {
                        fill.push((self.index(x + 1, y, z), here, self.index(x + 1, y, z - 1)));
                    }
                    // Plane (x, y): towards +x+y and +x-y.
                    if x + 1 < nx && y + 1 < ny && at(x + 1, y + 1, z) && !at(x + 1, y, z) && !at(x, y + 1, z) {
                        fill.push((self.index(x + 1, y, z), here, self.index(x + 1, y + 1, z)));
                    }
                    if x + 1 < nx && y > 0 && at(x + 1, y - 1, z) && !at(x + 1, y, z) && !at(x, y - 1, z) {
                        fill.push((self.index(x + 1, y, z), here, self.index(x + 1, y - 1, z)));
                    }
                    // Plane (z, y): towards +z+y and +z-y.
                    if z + 1 < nz && y + 1 < ny && at(x, y + 1, z + 1) && !at(x, y, z + 1) && !at(x, y + 1, z) {
                        fill.push((self.index(x, y, z + 1), here, self.index(x, y + 1, z + 1)));
                    }
                    if z + 1 < nz && y > 0 && at(x, y - 1, z + 1) && !at(x, y, z + 1) && !at(x, y - 1, z) {
                        fill.push((self.index(x, y, z + 1), here, self.index(x, y - 1, z + 1)));
                    }
                }
            }
        }
        for (id, a, b) in fill {
            self.solid[id as usize] = true;
            self.roof[id as usize] |= self.roof[a as usize] || self.roof[b as usize];
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
        cut.fell = self.unsupported_after(removed, &cut.blasted);
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
        let Some(id) = nearest_standing(self.origin, self.dims, at, |id| self.solid[id as usize] && !removed[id as usize]) else { return cut };
        let left = hp[id as usize].saturating_sub(amount);
        hp[id as usize] = left;
        if left > 0 {
            return cut;
        }
        removed[id as usize] = true;
        cut.blasted.push(id);
        cut.fell = self.unsupported_after(removed, &cut.blasted);
        for &lost in &cut.fell {
            removed[lost as usize] = true;
        }
        cut
    }

    /// As `unsupported`, for when everything was held before `taken` came out of `removed`: only the
    /// cells beside those can have lost their hold, so each is followed until it reaches an anchor
    /// (it and every cell on the way are held) or runs out of cells (all it reached fell). The work
    /// is the size of what is looked at, not of the building's grid. Sorted by cell number.
    fn unsupported_after(&self, removed: &[bool], taken: &[u32]) -> Vec<u32> {
        let standing = |id: u32| self.solid[id as usize] && !removed[id as usize];
        let mut held: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let mut fell: Vec<u32> = Vec::new();
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for &gone in taken {
            for seed in self.around(gone) {
                if !standing(seed) || held.contains(&seed) || seen.contains(&seed) {
                    continue;
                }
                let mut piece = vec![seed];
                let mut visited: std::collections::HashSet<u32> = std::collections::HashSet::from([seed]);
                let mut at = 0;
                let mut safe = false;
                while at < piece.len() && !safe {
                    let id = piece[at];
                    at += 1;
                    if self.anchored[id as usize] || held.contains(&id) {
                        safe = true;
                        break;
                    }
                    for next in self.around(id) {
                        if standing(next) && visited.insert(next) {
                            piece.push(next);
                        }
                    }
                }
                if safe {
                    held.extend(visited);
                } else {
                    seen.extend(visited);
                    fell.extend(piece);
                }
            }
        }
        fell.sort_unstable();
        fell
    }

    /// How the standing cells are held up, given `removed`: `Hold`.
    ///
    /// `span` is the cost of the cheapest chain of standing cells from an anchor: a step up costs
    /// nothing (a wall stands on what is under it), a step down 1 (it hangs), a step sideways 2
    /// (it is carried like a beam). `load` is the weight each cell carries: its own, and a share of
    /// every cell that leans on it, each cell giving all it carries in equal parts to the cells
    /// nearer an anchor that it touches. A cell no chain reaches has the span `UNHELD`.
    pub fn hold(&self, removed: &[bool]) -> Hold {
        use std::cmp::Reverse;
        let n = self.solid.len();
        let standing = |i: usize| self.solid[i] && !removed[i];
        let mut span = vec![UNHELD; n];
        let mut heap = std::collections::BinaryHeap::new();
        for i in (0..n).filter(|&i| self.anchored[i] && standing(i)) {
            span[i] = 0;
            heap.push(Reverse((0u16, i as u32)));
        }
        let mut held = Vec::new();
        while let Some(Reverse((d, id))) = heap.pop() {
            if d > span[id as usize] {
                continue;
            }
            held.push(id);
            let from = self.coords(id);
            for next in self.around(id).filter(|&next| standing(next as usize)) {
                let to = self.coords(next);
                let cost = if to[0] == from[0] && to[2] == from[2] { u16::from(to[1] < from[1]) } else { 2 };
                if d + cost < span[next as usize] {
                    span[next as usize] = d + cost;
                    heap.push(Reverse((d + cost, next)));
                }
            }
        }
        // Farthest from an anchor first, the higher first among equals: whatever leans on a cell has come before it.
        held.sort_unstable_by_key(|&id| Reverse((span[id as usize], self.coords(id)[1], id)));
        let mut load = vec![0u32; n];
        for &id in &held {
            load[id as usize] += 1;
            let (mine, y) = (span[id as usize], self.coords(id)[1]);
            let nearer = |next: &u32| standing(*next as usize) && (span[*next as usize] < mine || (span[*next as usize] == mine && self.coords(*next)[1] < y));
            let count = self.around(id).filter(nearer).count() as u32;
            if count == 0 {
                continue;
            }
            let (share, extra) = (load[id as usize] / count, (load[id as usize] % count) as usize);
            for (k, next) in self.around(id).filter(nearer).enumerate() {
                load[next as usize] += share + u32::from(k < extra);
            }
        }
        Hold { span, load }
    }

    /// What the building cannot go on holding: the standing cells that are held much more thinly
    /// or carry much more than they did when it was whole (`whole`, from `hold` of `untouched`),
    /// and whatever that leaves cut off. They are taken out of `removed` and returned as `fell`.
    /// Measured against the whole building, so one that nobody has touched never fails however
    /// it was built.
    pub fn strain(&self, removed: &mut [bool], whole: &Hold) -> Cut {
        let now = self.hold(removed);
        let weak: Vec<u32> = (0..self.solid.len())
            .filter(|&i| self.solid[i] && !removed[i])
            .filter(|&i| now.span[i] > whole.span[i].saturating_add(SPAN_SLACK) || now.load[i] > whole.load[i].saturating_mul(LOAD_FACTOR).saturating_add(LOAD_SLACK))
            .map(|i| i as u32)
            .collect();
        let mut cut = Cut::default();
        if weak.is_empty() {
            return cut;
        }
        for &id in &weak {
            removed[id as usize] = true;
        }
        cut.fell = weak;
        cut.fell.extend(self.unsupported_after(removed, &cut.fell));
        for &id in &cut.fell {
            removed[id as usize] = true;
        }
        cut.fell.sort_unstable();
        cut
    }

    /// The standing cells that no chain of standing cells joins to an anchor.
    #[cfg(test)]
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

    /// What the building is drawn as: the faces of its standing cubes (solid, not
    /// in `removed`) that look onto a cell with nothing standing in it, as triangles in centimetres,
    /// wound counter-clockwise seen from outside the cube. Neighbouring faces of wall cubes, and of
    /// roof cubes, are merged into rectangles as `greedy` does, so a plain wall is two triangles a
    /// side, and the clients draw the same faces in the wall's and the roof's colours (`faces_of`).
    pub fn faces(&self, removed: &[bool]) -> Vec<Tri> {
        let keys: Vec<i32> = (0..self.solid.len()).map(|i| if self.solid[i] && !removed[i] { i32::from(self.roof[i]) } else { -1 }).collect();
        let mut out = Vec::new();
        faces_of(self.origin, self.dims, &keys, |tri, _| out.push(tri));
        out
    }

    /// Whether each cell holds a piece of wall or roof, in cell number order.
    pub fn solid_flags(&self) -> &[bool] {
        &self.solid
    }

    /// Whether the roof passes through each cell, in cell number order.
    pub fn roof_flags(&self) -> &[bool] {
        &self.roof
    }

    /// What the building is collided with: its standing cubes (solid, not in `removed`) merged into
    /// boxes, each `[min x, min y, min z, max x, max y, max z]` in centimetres. Going through the
    /// cells in number order, each standing cell not yet in a box starts one, which grows first
    /// along x, then along z while the whole row is standing and free, then up while the whole layer
    /// is. A wall is then a box or two a floor, where its faces would be hundreds of triangles.
    pub fn boxes(&self, removed: &[bool]) -> Vec<[i32; 6]> {
        let [nx, ny, nz] = self.dims;
        let mut free: Vec<bool> = (0..self.solid.len()).map(|i| self.solid[i] && !removed[i]).collect();
        let mut out = Vec::new();
        for y in 0..ny {
            for z in 0..nz {
                for x in 0..nx {
                    if !free[self.index(x, y, z) as usize] {
                        continue;
                    }
                    let row = |free: &[bool], y: usize, z: usize, w: usize| (x..x + w).all(|x| free[self.index(x, y, z) as usize]);
                    let mut w = 1;
                    while x + w < nx && free[self.index(x + w, y, z) as usize] {
                        w += 1;
                    }
                    let mut d = 1;
                    while z + d < nz && row(&free, y, z + d, w) {
                        d += 1;
                    }
                    let mut h = 1;
                    while y + h < ny && (z..z + d).all(|z| row(&free, y + h, z, w)) {
                        h += 1;
                    }
                    for (yi, zi, xi) in (y..y + h).flat_map(|y| (z..z + d).flat_map(move |z| (x..x + w).map(move |x| (y, z, x)))) {
                        free[self.index(xi, yi, zi) as usize] = false;
                    }
                    let [ox, oy, oz] = self.origin;
                    let c = |v: usize| v as i32 * self.cell;
                    out.push([ox + c(x), oy + c(y), oz + c(z), ox + c(x + w), oy + c(y + h), oz + c(z + d)]);
                }
            }
        }
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

/// The standing cell nearest `at` (centimetres) of a grid of `dims` cells from `origin`, looking at the
/// cell `at` is in and those around it, if its middle is within a cell's reach: where a round that
/// landed there struck. The first in cell order wins a tie.
pub fn nearest_standing(origin: [i32; 3], dims: [usize; 3], at: [i32; 3], standing: impl Fn(u32) -> bool) -> Option<u32> {
    let cell = i64::from(CELL);
    let range = |a: usize| {
        let lo = ((i64::from(at[a]) - cell - i64::from(origin[a])).div_euclid(cell)).clamp(0, dims[a] as i64 - 1);
        let hi = ((i64::from(at[a]) + cell - i64::from(origin[a])).div_euclid(cell)).clamp(0, dims[a] as i64 - 1);
        lo as usize..=hi as usize
    };
    let mut nearest: Option<(i64, u32)> = None;
    for y in range(1) {
        for z in range(2) {
            for x in range(0) {
                let id = ((y * dims[2] + z) * dims[0] + x) as u32;
                if !standing(id) {
                    continue;
                }
                let centre = [x, y, z].map(|c| c as i64 * cell + cell / 2);
                let d = (0..3).map(|a| (i64::from(origin[a]) + centre[a] - i64::from(at[a])).pow(2)).sum::<i64>();
                if nearest.is_none_or(|(best, _)| d < best) {
                    nearest = Some((d, id));
                }
            }
        }
    }
    nearest.filter(|&(d, _)| d <= cell * cell * 3 / 2).map(|(_, id)| id)
}

/// What a grid of keyed cells is drawn as: the faces `greedy` merges (`keys` as it takes them), two
/// triangles each, in centimetres from the grid's corner at `origin`, wound counter-clockwise seen
/// from outside the cube. Calls `emit` with each triangle and its face's key.
pub fn faces_of(origin: [i32; 3], dims: [usize; 3], keys: &[i32], mut emit: impl FnMut(Tri, i32)) {
    greedy(dims, keys, |face| {
        let corner = |du: usize, dv: usize| {
            let mut p = [0i32; 3];
            p[face.d] = origin[face.d] + (face.i + usize::from(face.side > 0)) as i32 * CELL;
            p[face.u] = origin[face.u] + (face.a + du * face.w) as i32 * CELL;
            p[face.v] = origin[face.v] + (face.b + dv * face.h) as i32 * CELL;
            p
        };
        let [p0, p1, p2, p3] = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
        let tri = |a: [i32; 3], b: [i32; 3], c: [i32; 3]| [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]];
        if face.side > 0 {
            emit(tri(p0, p1, p2), face.key);
            emit(tri(p0, p2, p3), face.key);
        } else {
            emit(tri(p0, p2, p1), face.key);
            emit(tri(p0, p3, p2), face.key);
        }
    });
}

/// Packs one flag a cell (`Shell::index` order, `layer` cells to a layer of y) small for a tile
/// file: each flag is XORed with the one under it, so a wall, the same all the way up, leaves only
/// its bottom layer; the flags go eight to a byte, least significant bit first; and each run of
/// zero bytes is written as a 0 and the run's length less one, as an unsigned LEB128 number.
pub fn pack(flags: &[bool], layer: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; flags.len().div_ceil(8)];
    for i in 0..flags.len() {
        let below = i >= layer && flags[i - layer];
        if flags[i] != below {
            bytes[i / 8] |= 1 << (i % 8);
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != 0 {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] == 0 {
            i += 1;
        }
        out.push(0);
        let mut run = i - start - 1;
        while run >= 0x80 {
            out.push((run as u8 & 0x7f) | 0x80);
            run >>= 7;
        }
        out.push(run as u8);
    }
    out
}

/// `count` flags back from `pack`. Bytes missing at the end are zero.
pub fn unpack(packed: &[u8], count: usize, layer: usize) -> Vec<bool> {
    let mut bytes = Vec::with_capacity(count.div_ceil(8));
    let mut at = 0;
    while at < packed.len() {
        let byte = packed[at];
        at += 1;
        if byte != 0 {
            bytes.push(byte);
            continue;
        }
        let (mut run, mut shift) = (0usize, 0);
        while at < packed.len() {
            let part = packed[at];
            at += 1;
            run |= ((part & 0x7f) as usize) << shift;
            shift += 7;
            if part & 0x80 == 0 {
                break;
            }
        }
        bytes.resize(bytes.len() + run + 1, 0);
    }
    let mut flags = vec![false; count];
    for i in 0..count {
        let bit = bytes.get(i / 8).is_some_and(|byte| byte & (1 << (i % 8)) != 0);
        flags[i] = bit != (i >= layer && flags[i - layer]);
    }
    flags
}

/// One merged face of `greedy`: `w × h` cells from `(a, b)` of layer `i` along axis `d`, facing
/// `side` (1 towards +d, -1 towards -d). `u` and `v` are the next axes round from `d`, so `u × v`
/// points along +d.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
/// in a fixed order: by axis, side and layer, then by where it starts, row by row.
///
/// Only the standing cells are visited after the first pass over `keys`, as a big building's grid
/// is mostly empty: each layer's exposed faces are bucketed and sorted, rather than every cell of
/// every layer looked at.
pub fn greedy(dims: [usize; 3], keys: &[i32], mut emit: impl FnMut(Face)) {
    let [nx, _, nz] = dims;
    let ids: Vec<usize> = (0..keys.len()).filter(|&id| keys[id] >= 0).collect();
    // Each standing cell's x, y and z.
    let at: [Vec<usize>; 3] = [ids.iter().map(|&id| id % nx).collect(), ids.iter().map(|&id| id / (nx * nz)).collect(), ids.iter().map(|&id| (id / nx) % nz).collect()];
    // Step in the cell number for one cell along x, y and z.
    let stride = [1, nx * nz, nx];
    let (mut layer, mut place, mut sorted) = (vec![0; ids.len()], vec![0; ids.len()], vec![0; ids.len()]);
    for d in 0..3 {
        let (u, v) = ((d + 1) % 3, (d + 2) % 3);
        let (nu, nv, nd) = (dims[u], dims[v], dims[d]);
        let (su, sv, sd) = (stride[u], stride[v], stride[d]);
        let mut mask = vec![-1i32; nu * nv];
        for side in [1i32, -1] {
            // The faces looking out, as `b * nu + a`, bucketed by layer and then sorted.
            let mut starts = vec![0; nd + 1];
            let mut exposed = 0;
            for (n, &id) in ids.iter().enumerate() {
                let i = at[d][n];
                let j = i as i64 + i64::from(side);
                if j >= 0 && (j as usize) < nd && keys[(id as i64 + i64::from(side) * sd as i64) as usize] >= 0 {
                    continue;
                }
                layer[exposed] = i;
                place[exposed] = at[v][n] * nu + at[u][n];
                exposed += 1;
                starts[i + 1] += 1;
            }
            for i in 0..nd {
                starts[i + 1] += starts[i];
            }
            let mut next = starts[..nd].to_vec();
            for n in 0..exposed {
                sorted[next[layer[n]]] = place[n];
                next[layer[n]] += 1;
            }
            for i in 0..nd {
                let faces = &mut sorted[starts[i]..starts[i + 1]];
                if faces.is_empty() {
                    continue;
                }
                faces.sort_unstable();
                let plane = i * sd;
                for &p in faces.iter() {
                    mask[p] = keys[plane + (p % nu) * su + (p / nu) * sv];
                }
                for &p in faces.iter() {
                    let key = mask[p];
                    if key < 0 {
                        continue;
                    }
                    let (a, b) = (p % nu, p / nu);
                    let mut w = 1;
                    while a + w < nu && mask[p + w] == key {
                        w += 1;
                    }
                    let mut h = 1;
                    while b + h < nv && (0..w).all(|k| mask[p + h * nu + k] == key) {
                        h += 1;
                    }
                    for r in 0..h {
                        mask[p + r * nu..p + r * nu + w].fill(-1);
                    }
                    emit(Face { d, u, v, side, i, a, b, w, h, key });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `greedy` as it was, visiting every cell of every layer: the oracle the sparse one is held to.
    fn dense_greedy(dims: [usize; 3], keys: &[i32], mut emit: impl FnMut(Face)) {
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

    /// A grid of `dims` with cells standing at random, keyed 0 to `kinds - 1`, from `seed`.
    fn random_keys(dims: [usize; 3], fill: u64, kinds: i32, seed: u64) -> Vec<i32> {
        let mut state = seed;
        (0..dims[0] * dims[1] * dims[2])
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                if (state >> 33) % 100 < fill { ((state >> 20) % kinds as u64) as i32 } else { -1 }
            })
            .collect()
    }

    /// A way of meshing a grid: `greedy` or `dense_greedy`.
    type Mesher = fn([usize; 3], &[i32], &mut dyn FnMut(Face));

    fn faces_of(dims: [usize; 3], keys: &[i32], mesh: Mesher) -> Vec<Face> {
        let mut out = Vec::new();
        mesh(dims, keys, &mut |face| out.push(face));
        out
    }

    #[test]
    fn sparse_greedy_makes_the_dense_ones_faces_in_its_order() {
        let shapes = [[1, 1, 1], [3, 1, 1], [1, 4, 1], [1, 1, 5], [4, 3, 5], [7, 2, 3], [9, 9, 9], [16, 5, 11]];
        for (n, &dims) in shapes.iter().enumerate() {
            for (fill, kinds) in [(10, 1), (50, 1), (50, 3), (90, 2), (100, 1), (0, 1)] {
                let keys = random_keys(dims, fill, kinds, n as u64 * 31 + fill);
                let sparse = faces_of(dims, &keys, |d, k, e| greedy(d, k, e));
                let dense = faces_of(dims, &keys, |d, k, e| dense_greedy(d, k, e));
                assert_eq!(sparse, dense, "{dims:?} {fill}% {kinds} keys");
            }
        }
        // A real building, roofed and blasted.
        let shell = Shell::new(&boxed(10, 8, 6), &roof(10, 8, 6)).unwrap();
        let mut removed = shell.untouched();
        shell.blast(&mut removed, [500, 300, 0], 150);
        let keys: Vec<i32> = (0..shell.solid.len()).map(|i| if shell.solid[i] && !removed[i] { i32::from(shell.roof[i]) } else { -1 }).collect();
        assert_eq!(faces_of(shell.dims, &keys, |d, k, e| greedy(d, k, e)), faces_of(shell.dims, &keys, |d, k, e| dense_greedy(d, k, e)));
    }

    /// Not a regression test: what one structure check costs. `cargo test --release -p shared -- --ignored --nocapture strain_cost`
    #[test]
    #[ignore]
    fn strain_cost() {
        for (w, d, h) in [(10, 8, 6), (30, 12, 8), (120, 80, 40)] {
            let shell = Shell::new(&boxed(w, d, h), &roof(w, d, h)).unwrap();
            let whole = shell.hold(&shell.untouched());
            let mut removed = shell.untouched();
            shell.blast(&mut removed, [300, 100, 0], 150);
            let start = std::time::Instant::now();
            for _ in 0..10 {
                let mut again = removed.clone();
                shell.strain(&mut again, &whole);
            }
            println!("{w}x{d}x{h} m: {:?} for {} standing cells", start.elapsed() / 10, shell.standing(&removed));
        }
    }

    /// Not a regression test: the two on a big hollow building. `cargo test --release -p shared -- --ignored --nocapture greedy_cost`
    #[test]
    #[ignore]
    fn greedy_cost() {
        let shell = Shell::new(&boxed(120, 80, 40), &roof(120, 80, 40)).unwrap();
        let keys: Vec<i32> = (0..shell.solid.len()).map(|i| if shell.solid[i] { i32::from(shell.roof[i]) } else { -1 }).collect();
        for (name, mesh) in [("sparse", (|d, k, e| greedy(d, k, e)) as Mesher), ("dense", |d, k, e| dense_greedy(d, k, e))] {
            let start = std::time::Instant::now();
            let mut faces = 0;
            for _ in 0..10 {
                faces = faces_of(shell.dims, &keys, mesh).len();
            }
            println!("{name}: {:?} a building of {} cells ({} standing), {faces} faces", start.elapsed() / 10, keys.len(), shell.solid_count());
        }
    }

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

    /// The cells that fall are the ones the whole-grid search finds, blast after blast.
    #[test]
    fn what_falls_is_what_the_whole_grid_search_finds() {
        let shell = Shell::new(&boxed(20, 12, 8), &roof(20, 12, 8)).unwrap();
        let mut removed = shell.untouched();
        let mut state = 7u64;
        let mut next = |m: i32| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) % m as u64) as i32
        };
        let mut fallen = 0;
        for _ in 0..80 {
            let at = [next(2000), next(800), if next(2) == 0 { 0 } else { 1200 }];
            let before = removed.clone();
            let cut = shell.blast(&mut removed, at, 100 + next(300));
            let mut without = before.clone();
            for &id in &cut.blasted {
                without[id as usize] = true;
            }
            assert_eq!(cut.fell, shell.unsupported(&without), "blast at {at:?}");
            fallen += cut.fell.len();
        }
        assert!(fallen > 0, "nothing ever fell");
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
    fn a_building_nobody_touched_is_never_strained() {
        for (w, d, h) in [(6, 6, 4), (10, 8, 6), (30, 12, 8), (60, 9, 20)] {
            let shell = Shell::new(&boxed(w, d, h), &roof(w, d, h)).unwrap();
            let whole = shell.hold(&shell.untouched());
            let mut removed = shell.untouched();
            assert!(shell.strain(&mut removed, &whole).is_empty(), "{w}x{d}x{h}");
        }
    }

    #[test]
    fn a_hole_in_a_wall_is_no_strain() {
        let shell = Shell::new(&boxed(10, 8, 6), &roof(10, 8, 6)).unwrap();
        let whole = shell.hold(&shell.untouched());
        let mut removed = shell.untouched();
        shell.blast(&mut removed, [500, 300, 0], 150);
        assert!(shell.strain(&mut removed, &whole).is_empty());
    }

    /// Takes the walls along both long sides out, leaving the roof on the two ends.
    fn without_long_walls(shell: &Shell, removed: &mut [bool]) {
        for id in shell.solid_cells() {
            let [_, _, z] = shell.coords(id);
            if !shell.is_roof(id) && (z == 0 || z == shell.dims[2] - 1) {
                removed[id as usize] = true;
            }
        }
    }

    #[test]
    fn a_roof_with_its_long_walls_gone_comes_down_in_the_middle_and_then_holds() {
        let shell = Shell::new(&boxed(30, 12, 6), &roof(30, 12, 6)).unwrap();
        let whole = shell.hold(&shell.untouched());
        let mut removed = shell.untouched();
        without_long_walls(&shell, &mut removed);
        let before = shell.standing(&removed);
        let cut = shell.strain(&mut removed, &whole);
        assert!(!cut.fell.is_empty(), "the roof hangs 12 m from its ends");
        assert!(cut.blasted.is_empty());
        assert!(cut.fell.iter().filter(|&&id| shell.is_roof(id)).count() > 100, "the roof comes down");
        assert_eq!(shell.standing(&removed), before - cut.fell.len());
        // It keeps shedding what it cannot hold, a round at a time, and ends standing.
        let mut rounds = 0;
        while !shell.strain(&mut removed, &whole).is_empty() {
            rounds += 1;
            assert!(rounds < 50, "never settles");
        }
        assert!(shell.standing(&removed) > 0, "the ends and what hangs from them are left");
    }

    #[test]
    fn a_wall_standing_on_a_cut_foot_leans_on_the_rest() {
        // A tall tower with all but one corner column of its lowest rows gone: the load is far above
        // what that column carried when the tower was whole.
        let shell = Shell::new(&boxed(4, 4, 30), &roof(4, 4, 30)).unwrap();
        let whole = shell.hold(&shell.untouched());
        let mut removed = shell.untouched();
        for id in shell.solid_cells() {
            let [x, y, z] = shell.coords(id);
            if y < 3 && (x, z) != (0, 0) {
                removed[id as usize] = true;
            }
        }
        let cut = shell.strain(&mut removed, &whole);
        assert!(cut.fell.len() > 10, "{}", cut.fell.len());
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
        assert_eq!((lo, hi), ([0, 0, 0], [648, 432, 648]));
        // The top: the whole roof, one rectangle at its height.
        let top = faces.iter().filter(|t| t[1] == 432 && t[4] == 432 && t[7] == 432).count();
        assert_eq!(top, 2);
        // A hole lets you in: more faces, the hole's sides.
        let mut removed = shell.untouched();
        shell.blast(&mut removed, [300, 150, 0], 100);
        assert!(shell.faces(&removed).len() > faces.len());
    }

    #[test]
    fn the_boxes_cover_each_standing_cube_once() {
        let shell = Shell::new(&boxed(6, 6, 4), &roof(6, 6, 4)).unwrap();
        let mut removed = shell.untouched();
        shell.blast(&mut removed, [300, 150, 0], 100);
        for removed in [shell.untouched(), removed] {
            let boxes = shell.boxes(&removed);
            let mut covered = vec![0; removed.len()];
            for b in &boxes {
                for y in (b[1]..b[4]).step_by(CELL as usize) {
                    for z in (b[2]..b[5]).step_by(CELL as usize) {
                        for x in (b[0]..b[3]).step_by(CELL as usize) {
                            covered[shell.cell_at([x as i64 + 1, y as i64 + 1, z as i64 + 1]) as usize] += 1;
                        }
                    }
                }
            }
            for id in 0..removed.len() {
                assert_eq!(covered[id], usize::from(shell.solid[id] && !removed[id]), "cell {id}");
            }
            assert!(boxes.len() < shell.standing(&removed) / 4, "{} boxes for {} cubes", boxes.len(), shell.standing(&removed));
        }
    }

    /// Checked number for number through WebAssembly by `client/src/__tests__/carve.spec.ts`.
    #[test]
    fn cell_numbers_match_the_fixture() {
        let shell = Shell::new(&boxed(5, 4, 3), &roof(5, 4, 3)).unwrap();
        assert_eq!(shell.cell, 72);
        assert_eq!(shell.dims, [7, 5, 6]);
        let faces = shell.faces(&shell.untouched());
        let sum: i64 = faces.iter().flatten().enumerate().map(|(i, &v)| v as i64 * (i % 9 + 1) as i64).sum();
        assert_eq!((faces.len(), shell.solid_count(), sum), (36, 130, 393_120));
        let diagonal = Shell::new(&slanted(), &[]).unwrap();
        let cells: u64 = diagonal.solid_cells().iter().map(|&id| id as u64).sum();
        assert_eq!((diagonal.origin, diagonal.dims, diagonal.solid_count(), cells), ([-72, 0, 0], [11, 5, 8], 90, 19_505));
        let faces = diagonal.faces(&diagonal.untouched());
        let sum: i64 = faces.iter().flatten().enumerate().map(|(i, &v)| v as i64 * (i % 9 + 1) as i64).sum();
        assert_eq!((faces.len(), sum), (92, 1_179_144));
        // The boxes, summed the same way: each number by its place in its box.
        let boxes = |shell: &Shell| {
            let boxes = shell.boxes(&shell.untouched());
            let sum: i64 = boxes.iter().flatten().enumerate().map(|(i, &v)| v as i64 * (i % 6 + 1) as i64).sum();
            (boxes.len(), sum)
        };
        assert_eq!((boxes(&shell), boxes(&diagonal)), ((5, 30_024), (8, 52_704)));
    }

    #[test]
    fn baked_cells_come_back_as_they_were() {
        for shell in [Shell::new(&boxed(5, 4, 3), &roof(5, 4, 3)).unwrap(), Shell::new(&slanted(), &[]).unwrap()] {
            let back = Shell::from_cells(&shell.cells()).unwrap();
            assert_eq!((back.origin, back.dims, &back.solid, &back.roof), (shell.origin, shell.dims, &shell.solid, &shell.roof));
        }
        // A long run of zeros takes a two-byte length.
        let mut flags = vec![false; 8 * 300];
        flags[0] = true;
        flags[8 * 299 + 7] = true;
        let packed = pack(&flags, 8 * 300);
        assert_eq!(packed, vec![1, 0, 169, 2, 128]);
        assert_eq!(unpack(&packed, flags.len(), 8 * 300), flags);
    }

    /// `client/src/__tests__/carve.spec.ts` unpacks these bytes too, through WebAssembly.
    #[test]
    fn packed_cells_match_the_fixture() {
        let shell = Shell::new(&boxed(5, 4, 3), &roof(5, 4, 3)).unwrap();
        let cells = shell.cells();
        assert_eq!(cells.solid.0, vec![255, 96, 48, 24, 252, 3, 0, 15, 159, 207, 231, 3, 0, 0]);
        // The roof is the top layer: twenty-one bytes of nothing, then forty-nine cells of roof.
        assert_eq!(cells.roof.0, vec![0, 20, 255, 255, 255, 255, 255, 3]);
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
