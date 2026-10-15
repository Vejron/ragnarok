//! How a standing building is cut into cells, so a blast can take a bite out of it and what the
//! bite leaves hanging can fall.
//!
//! A building's walls and roof (the triangles `osm-import` made) are cut into small triangles, each
//! of which belongs to one cell of a regular grid laid over the building. The server decides which
//! cells a blast removes and which are left unsupported, and tells everyone the cell numbers; the
//! clients cut the same triangles the same way and draw and collide with what is left. Everything
//! that both sides must agree on is integer arithmetic on centimetres, so the cell a triangle is
//! in is the same in Rust and in `client/src/sim/carve.ts` (checked by `fixtures/parity`).
//!
//! The floor is not carved: it is never part of the cells.

/// Smallest cell, centimetres. A bigger building gets bigger cells so that it stays within
/// [`MAX_CELLS`] and [`MAX_TRIANGLES`].
pub const CELL_MIN: i32 = 100;
/// Most cells in one building's grid: they are named by a `u16` on the wire.
pub const MAX_CELLS: usize = 60_000;
/// Most small triangles, walls and roof together.
pub const MAX_TRIANGLES: usize = 6_000;
/// A building left with less than this percent of its cells is not worth keeping: it collapses.
pub const COLLAPSE_BELOW_PERCENT: usize = 30;

/// Health of one cell against bullets: a rifle needs a few rounds to break out a piece of wall.
/// Blasts take cells outright.
pub const CELL_HP: u8 = 40;

/// A triangle: three corners `[x, y, z]`, centimetres, flattened.
pub type Tri = [i32; 9];

/// Candidate cell sizes, centimetres, smallest first.
const CELL_SIZES: [i32; 8] = [100, 150, 200, 300, 400, 600, 900, 1400];

/// A building's walls and roof as small triangles in a grid of cells.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shell {
    /// Corner of the grid with the smallest coordinates, centimetres.
    pub origin: [i32; 3],
    /// Side of a cell, centimetres.
    pub cell: i32,
    /// Cells along x, y and z.
    pub dims: [usize; 3],
    /// Wall triangles then roof triangles, in the order of subdividing the source ones, and the cell of each.
    pub triangles: Vec<(Tri, u16)>,
    /// How many of `triangles` are wall; the rest are roof.
    pub walls: usize,
    /// Whether any small triangle lies in the cell.
    solid: Vec<bool>,
    /// Cells that hold the building up: its lowest layer, and any piece that never reached it.
    anchored: Vec<bool>,
}

/// What one blast did to a building: the cells it broke, and the cells that were left with nothing
/// to hold them and fell. Cell numbers are `Shell::index` (`(y * dims[2] + z) * dims[0] + x`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cut {
    pub blasted: Vec<u16>,
    pub fell: Vec<u16>,
}

impl Cut {
    pub fn is_empty(&self) -> bool {
        self.blasted.is_empty() && self.fell.is_empty()
    }
}

fn edge(a: &[i32], b: &[i32]) -> i64 {
    (0..3).map(|i| (a[i] as i64 - b[i] as i64).pow(2)).sum()
}

/// Cuts every triangle of `source` into pieces whose longest edge is at most `limit` centimetres:
/// the longest edge is bisected (ties to the lowest) until they fit. Integer midpoints, rounded
/// down, so any implementation of this gets the same pieces. Triangles with no area are dropped.
pub fn subdivide(source: &[Tri], limit: i32, out: &mut Vec<Tri>) {
    let limit_sq = limit as i64 * limit as i64;
    let mut stack: Vec<Tri> = Vec::new();
    for &tri in source {
        stack.push(tri);
        while let Some(t) = stack.pop() {
            let corner = |k: usize| [t[k * 3], t[k * 3 + 1], t[k * 3 + 2]];
            let (mut longest, mut length) = (0, -1i64);
            for i in 0..3 {
                let l = edge(&corner(i), &corner((i + 1) % 3));
                if l > length {
                    (longest, length) = (i, l);
                }
            }
            let (p, q, r) = (corner(longest), corner((longest + 1) % 3), corner((longest + 2) % 3));
            if length <= limit_sq {
                // No area: the corners are on a line.
                let (u, v) = ([q[0] - p[0], q[1] - p[1], q[2] - p[2]], [r[0] - p[0], r[1] - p[1], r[2] - p[2]]);
                let cross = [
                    u[1] as i64 * v[2] as i64 - u[2] as i64 * v[1] as i64,
                    u[2] as i64 * v[0] as i64 - u[0] as i64 * v[2] as i64,
                    u[0] as i64 * v[1] as i64 - u[1] as i64 * v[0] as i64,
                ];
                if cross != [0, 0, 0] {
                    out.push([p[0], p[1], p[2], q[0], q[1], q[2], r[0], r[1], r[2]]);
                }
                continue;
            }
            let m = [(p[0] + q[0]) >> 1, (p[1] + q[1]) >> 1, (p[2] + q[2]) >> 1];
            stack.push([p[0], p[1], p[2], m[0], m[1], m[2], r[0], r[1], r[2]]);
            stack.push([m[0], m[1], m[2], q[0], q[1], q[2], r[0], r[1], r[2]]);
        }
    }
}

impl Shell {
    /// Cuts a building: its wall triangles and its roof triangles, in centimetres, wound however
    /// the collision mesh has them. `None` when there is nothing to carve.
    pub fn new(walls: &[Tri], roof: &[Tri]) -> Option<Self> {
        let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
        for tri in walls.iter().chain(roof) {
            for k in 0..3 {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(tri[k * 3 + axis]);
                    hi[axis] = hi[axis].max(tri[k * 3 + axis]);
                }
            }
        }
        if walls.is_empty() && roof.is_empty() {
            return None;
        }
        let mut best = None;
        for &cell in &CELL_SIZES {
            let dims = [0, 1, 2].map(|a| (((hi[a] - lo[a]) / cell) as usize + 1).max(1));
            let mut small_walls = Vec::new();
            let mut small_roof = Vec::new();
            subdivide(walls, cell, &mut small_walls);
            subdivide(roof, cell, &mut small_roof);
            let fits = dims[0] * dims[1] * dims[2] <= MAX_CELLS && small_walls.len() + small_roof.len() <= MAX_TRIANGLES;
            best = Some((cell, dims, small_walls, small_roof));
            if fits {
                break;
            }
        }
        let (cell, dims, small_walls, small_roof) = best?;
        let count = small_walls.len();
        let mut shell = Shell {
            origin: lo,
            cell,
            dims,
            triangles: Vec::with_capacity(count + small_roof.len()),
            walls: count,
            solid: vec![false; dims[0] * dims[1] * dims[2]],
            anchored: Vec::new(),
        };
        for tri in small_walls.into_iter().chain(small_roof) {
            let id = shell.cell_of(&tri);
            shell.solid[id as usize] = true;
            shell.triangles.push((tri, id));
        }
        shell.anchor();
        Some(shell)
    }

    /// The cell a triangle's centroid is in.
    fn cell_of(&self, tri: &Tri) -> u16 {
        let at = |axis: usize| {
            let centroid = (tri[axis] as i64 + tri[3 + axis] as i64 + tri[6 + axis] as i64).div_euclid(3);
            ((centroid - self.origin[axis] as i64).div_euclid(self.cell as i64)).clamp(0, self.dims[axis] as i64 - 1) as usize
        };
        self.index(at(0), at(1), at(2))
    }

    /// The number of the cell at `(x, y, z)`.
    pub fn index(&self, x: usize, y: usize, z: usize) -> u16 {
        ((y * self.dims[2] + z) * self.dims[0] + x) as u16
    }

    /// The `(x, y, z)` of cell `id`.
    pub fn coords(&self, id: u16) -> [usize; 3] {
        let id = id as usize;
        let x = id % self.dims[0];
        let z = (id / self.dims[0]) % self.dims[2];
        [x, id / (self.dims[0] * self.dims[2]), z]
    }

    /// The middle of cell `id`, centimetres.
    pub fn centre(&self, id: u16) -> [i32; 3] {
        let c = self.coords(id);
        [0, 1, 2].map(|a| self.origin[a] + c[a] as i32 * self.cell + self.cell / 2)
    }

    /// How many cells hold a piece of wall or roof.
    pub fn solid_count(&self) -> usize {
        self.solid.iter().filter(|&&s| s).count()
    }

    /// Whether cell `id` holds a piece of wall or roof.
    pub fn is_solid(&self, id: u16) -> bool {
        self.solid.get(id as usize).copied().unwrap_or(false)
    }

    /// Every cell number that holds something, ascending.
    pub fn solid_cells(&self) -> Vec<u16> {
        self.solid.iter().enumerate().filter(|&(_, &s)| s).map(|(i, _)| i as u16).collect()
    }

    /// The cells touching cell `id`, faces, edges and corners.
    fn around(&self, id: u16) -> impl Iterator<Item = u16> + '_ {
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
            let mut piece = vec![start as u16];
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
        let mut nearest: Option<(i64, u16)> = None;
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
        let mut nearest: Option<(i64, u16)> = None;
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
    fn unsupported(&self, removed: &[bool]) -> Vec<u16> {
        let n = self.solid.len();
        let mut held = vec![false; n];
        let mut queue: Vec<u16> = (0..n).filter(|&i| self.anchored[i] && !removed[i] && self.solid[i]).map(|i| i as u16).collect();
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
        (0..n).filter(|&i| self.solid[i] && !removed[i] && !held[i]).map(|i| i as u16).collect()
    }

    /// How many cells are still standing, given `removed`.
    pub fn standing(&self, removed: &[bool]) -> usize {
        (0..self.solid.len()).filter(|&i| self.solid[i] && !removed[i]).count()
    }

    /// Whether so little of the building is left that it should collapse outright.
    pub fn is_ruin(&self, removed: &[bool]) -> bool {
        self.standing(removed) * 100 < self.solid_count() * COLLAPSE_BELOW_PERCENT
    }

    /// The triangles still standing: the small wall and roof triangles whose cell is not in
    /// `removed`, as corner triples in centimetres.
    pub fn standing_triangles(&self, removed: &[bool]) -> Vec<Tri> {
        self.triangles.iter().filter(|&&(_, id)| !removed[id as usize]).map(|&(tri, _)| tri).collect()
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
    fn subdividing_keeps_the_area_and_the_limit() {
        let mut out = Vec::new();
        subdivide(&boxed(10, 8, 6), 100, &mut out);
        assert!(out.iter().all(|t| (0..3).all(|i| edge(&t[i * 3..i * 3 + 3], &t[((i + 1) % 3) * 3..((i + 1) % 3) * 3 + 3]) <= 100 * 100)));
        let area: f64 = out
            .iter()
            .map(|t| {
                let (u, v) = ([t[3] - t[0], t[4] - t[1], t[5] - t[2]], [t[6] - t[0], t[7] - t[1], t[8] - t[2]]);
                let c = [u[1] as f64 * v[2] as f64 - u[2] as f64 * v[1] as f64, u[2] as f64 * v[0] as f64 - u[0] as f64 * v[2] as f64, u[0] as f64 * v[1] as f64 - u[1] as f64 * v[0] as f64];
                (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt() / 2.0
            })
            .sum();
        // Two walls of 10 × 6 and two of 8 × 6, in square centimetres.
        assert!((area - 2.0 * (10.0 * 6.0 + 8.0 * 6.0) * 10_000.0).abs() < 1.0, "{area}");
    }

    #[test]
    fn a_blast_on_the_wall_makes_a_hole_and_leaves_the_rest() {
        let shell = Shell::new(&boxed(10, 8, 6), &roof(10, 8, 6)).unwrap();
        assert_eq!(shell.cell, CELL_MIN);
        let mut removed = shell.untouched();
        let cut = shell.blast(&mut removed, [500, 300, 0], 150);
        assert!(!cut.blasted.is_empty());
        assert!(cut.fell.is_empty(), "a hole in a wall drops nothing: {:?}", cut.fell);
        let before = shell.solid_count();
        assert!(shell.standing(&removed) < before);
        assert!(!shell.is_ruin(&removed));
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
        assert!(shell.is_ruin(&removed) || shell.standing(&removed) < shell.solid_count() / 2);
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
        assert_eq!(shell.standing_triangles(&removed).len(), shell.triangles.len());
    }

    /// Mirrored byte for byte by `client/src/__tests__/carve.spec.ts`.
    #[test]
    fn cell_numbers_match_the_fixture() {
        let shell = Shell::new(&boxed(5, 4, 3), &roof(5, 4, 3)).unwrap();
        assert_eq!(shell.cell, 100);
        assert_eq!(shell.dims, [6, 4, 5]);
        let sum: u64 = shell.triangles.iter().map(|&(_, id)| id as u64).sum();
        assert_eq!((shell.triangles.len(), shell.solid_count(), sum), (416, 71, 23_456));
    }
}
