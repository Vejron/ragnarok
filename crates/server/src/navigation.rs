//! Where a stickman can walk, and the way from one place to another: a grid of 1 m cells over the
//! world, each with the height of its floor or blocked, searched with A*. Bots use it to go round
//! buildings, walls and trees instead of walking into them.
//!
//! A cell's floor is the topmost static surface under its middle: the ground, a bridge, or a roof.
//! It is blocked when a 1 m box over it, from a step's height above that floor to above a standing
//! body's head, touches anything static: a wall, a tree, a rock, the world's edge, or ground too
//! steep to climb. Buildings are hollow meshes, so the box test is what makes their walls, which are
//! thin, block the cells they pass through. Two cells next to each other join only when their
//! floors are within `MAX_RISE`, so a roof is an island of its own and a bridge joins the road at
//! its ends.
//!
//! The grid is built a chunk of 16 x 16 cells at a time, when a search first looks there, and a
//! chunk is built again after a building in it is bitten or comes down (`forget`).

use sim::Physics;
use shared::consts::CAPSULE_RADIUS;
use shared::world::WorldFile;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// The side of a cell, metres.
pub const CELL: f32 = 1.0;
/// Cells along each side of a chunk.
const CHUNK: i32 = 16;
/// The blocking box runs from this far above the floor (anything lower is stepped over or walked
/// up) to `BODY_TOP`, metres.
const STEP_CLEAR: f32 = 0.5;
const BODY_TOP: f32 = 1.9;
/// The most two cells' floors may differ by and still join, metres: about the steepest slope a
/// stickman walks up.
const MAX_RISE: f32 = 1.0;
/// A cell's floor is looked for from this far above the ground down to this far below it, metres.
const FLOOR_FROM: f32 = 60.0;
const FLOOR_BELOW: f32 = 20.0;
/// One search looks at no more than this many cells: about an 80 m square of them.
pub const MAX_SEARCH: usize = 6_000;
/// Before searching, the open ground round each end is flooded up to this many cells: an end on an
/// island smaller than that (a roof, a walled yard) is found out cheaply, not by searching the
/// whole of `MAX_SEARCH` for it.
const ISLAND: usize = 400;
/// How far round an end that is not open (someone standing against a wall) a search looks for an
/// open cell to start or finish at, cells.
const NEAR_OPEN: i32 = 2;
/// What building a chunk costs, counted in the cells a search looks at: about as long.
const CHUNK_COST: usize = 200;
/// A straight line is checked every this many metres along it.
const LINE_STEP: f32 = 0.5;

type Cell = (i32, i32);

const SIDES: [Cell; 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const AROUND: [Cell; 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// The walking grid of a world, built as it is needed.
#[derive(Default)]
pub struct NavGrid {
    /// Built chunks by chunk coordinates: each cell's floor height, row by row, NaN if blocked.
    chunks: HashMap<Cell, Box<[f32]>>,
    /// Chunks built since `take_built` was last called.
    fresh: usize,
}

/// The cell `[x, z]` (metres) is in.
pub fn cell_of([x, z]: [f32; 2]) -> Cell {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

/// The middle of `cell`, metres.
pub fn centre((i, j): Cell) -> [f32; 2] {
    [(i as f32 + 0.5) * CELL, (j as f32 + 0.5) * CELL]
}

impl NavGrid {
    /// How many chunks have been built.
    #[cfg(test)]
    pub fn built(&self) -> usize {
        self.chunks.len()
    }

    /// The height of `cell`'s floor, if a standing stickman can be there.
    pub fn floor(&mut self, physics: &Physics, map: &WorldFile, (i, j): Cell) -> Option<f32> {
        let key = (i.div_euclid(CHUNK), j.div_euclid(CHUNK));
        let cells = self.chunks.entry(key).or_insert_with(|| {
            self.fresh += 1;
            build_chunk(physics, map, key)
        });
        let floor = cells[(j.rem_euclid(CHUNK) * CHUNK + i.rem_euclid(CHUNK)) as usize];
        (!floor.is_nan()).then_some(floor)
    }

    /// Whether a standing stickman can be in `cell`.
    pub fn open(&mut self, physics: &Physics, map: &WorldFile, cell: Cell) -> bool {
        self.floor(physics, map, cell).is_some()
    }

    /// Whether a stickman can step from cell `a` to cell `b`: both open, and their floors near enough.
    fn joins(&mut self, physics: &Physics, map: &WorldFile, a: Cell, b: Cell) -> bool {
        match (self.floor(physics, map, a), self.floor(physics, map, b)) {
            (Some(fa), Some(fb)) => (fa - fb).abs() <= MAX_RISE,
            _ => false,
        }
    }

    /// Forgets the chunks over `[min x, min z, max x, max z]` (metres), and a cell round it, so they
    /// are built again from the colliders as they are now.
    pub fn forget(&mut self, [x0, z0, x1, z1]: [f32; 4]) {
        let (lo, hi) = (cell_of([x0 - CELL, z0 - CELL]), cell_of([x1 + CELL, z1 + CELL]));
        let (lo, hi) = ((lo.0.div_euclid(CHUNK), lo.1.div_euclid(CHUNK)), (hi.0.div_euclid(CHUNK), hi.1.div_euclid(CHUNK)));
        self.chunks.retain(|&(ci, cj), _| ci < lo.0 || ci > hi.0 || cj < lo.1 || cj > hi.1);
    }

    /// Whether a stickman can walk straight from `from` to `to` (metres): every cell its body
    /// passes over is open, and each joins the one before.
    pub fn clear_line(&mut self, physics: &Physics, map: &WorldFile, from: [f32; 2], to: [f32; 2]) -> bool {
        let d = [to[0] - from[0], to[1] - from[1]];
        let length = d[0].hypot(d[1]);
        let mut last = cell_of(from);
        if length < 1e-3 {
            return self.open(physics, map, last);
        }
        // The body's two sides as well as its middle.
        let side = [-d[1] / length * CAPSULE_RADIUS, d[0] / length * CAPSULE_RADIUS];
        let steps = (length / LINE_STEP).ceil() as usize;
        for n in 0..=steps {
            let t = n as f32 / steps as f32;
            let p = [from[0] + d[0] * t, from[1] + d[1] * t];
            let middle = cell_of(p);
            if !self.joins(physics, map, last, middle) {
                return false;
            }
            for s in [1.0, -1.0] {
                let beside = cell_of([p[0] + side[0] * s, p[1] + side[1] * s]);
                if !self.joins(physics, map, middle, beside) {
                    return false;
                }
            }
            last = middle;
        }
        true
    }

    /// The nearest open cell to `cell` with its floor within `MAX_RISE` of `level`, `cell` itself if
    /// it is one, within `NEAR_OPEN`.
    fn near_open(&mut self, physics: &Physics, map: &WorldFile, cell: Cell, level: f32) -> Option<Cell> {
        (0..=NEAR_OPEN).find_map(|ring| {
            let mut around: Vec<Cell> = (-ring..=ring)
                .flat_map(|di| (-ring..=ring).map(move |dj| (di, dj)))
                .filter(|&(di, dj)| di.abs().max(dj.abs()) == ring)
                .map(|(di, dj)| (cell.0 + di, cell.1 + dj))
                .collect();
            around.sort_by_key(|&(i, j)| (i - cell.0).pow(2) + (j - cell.1).pow(2));
            around.into_iter().find(|&c| self.floor(physics, map, c).is_some_and(|floor| (floor - level).abs() <= MAX_RISE))
        })
    }

    /// The cells joined to `cell`, if there are fewer than `ISLAND` of them; `None` if there are more.
    fn island(&mut self, physics: &Physics, map: &WorldFile, cell: Cell) -> Option<HashSet<Cell>> {
        let mut seen = HashSet::from([cell]);
        let mut todo = vec![cell];
        while let Some(at) = todo.pop() {
            for (di, dj) in SIDES {
                let next = (at.0 + di, at.1 + dj);
                if !seen.contains(&next) && self.joins(physics, map, at, next) {
                    if seen.len() >= ISLAND {
                        return None;
                    }
                    seen.insert(next);
                    todo.push(next);
                }
            }
        }
        Some(seen)
    }

    /// A way to walk from feet at `from` to feet at `to` (`[x, y, z]`, metres): the corners `[x, z]`
    /// to head for in turn, ending at `to`, each in a clear straight line from the one before (the
    /// first from `from`). `None` when there is no way, or none found within `MAX_SEARCH` cells or
    /// what is left of `budget`, from which the cells it looks at are taken, and `CHUNK_COST` for
    /// every chunk it has to build.
    pub fn find_path(&mut self, physics: &Physics, map: &WorldFile, from: [f32; 3], to: [f32; 3], budget: &mut usize) -> Option<Vec<[f32; 2]>> {
        let fresh = self.fresh;
        let found = self.search(physics, map, from, to, budget);
        *budget = budget.saturating_sub((self.fresh - fresh) * CHUNK_COST);
        found
    }

    fn search(&mut self, physics: &Physics, map: &WorldFile, from: [f32; 3], to: [f32; 3], budget: &mut usize) -> Option<Vec<[f32; 2]>> {
        let (level_from, level_to) = (from[1], to[1]);
        let (from, to) = ([from[0], from[2]], [to[0], to[2]]);
        if self.clear_line(physics, map, from, to) {
            return Some(vec![to]);
        }
        let start = self.near_open(physics, map, cell_of(from), level_from)?;
        let goal = self.near_open(physics, map, cell_of(to), level_to)?;
        // Either end on a small island the other is not on: no way, found out cheaply.
        for (end, other) in [(goal, start), (start, goal)] {
            let island = self.island(physics, map, end);
            *budget = budget.saturating_sub(island.as_ref().map_or(ISLAND, HashSet::len));
            if island.is_some_and(|cells| !cells.contains(&other)) {
                return None;
            }
        }
        let limit = (*budget).min(MAX_SEARCH);
        // Costs in tenths of a cell: 10 along a side, 14 across a corner.
        let guess = |(i, j): Cell| {
            let (di, dj) = ((i - goal.0).abs(), (j - goal.1).abs());
            10 * di.max(dj) + 4 * di.min(dj)
        };
        let mut open = BinaryHeap::from([Reverse((guess(start), 0, start))]);
        let mut cost: HashMap<Cell, i32> = HashMap::from([(start, 0)]);
        let mut came: HashMap<Cell, Cell> = HashMap::new();
        let mut searched = 0;
        let found = loop {
            let Some(Reverse((_, so_far, cell))) = open.pop() else { break false };
            if cell == goal {
                break true;
            }
            if so_far > cost[&cell] {
                continue;
            }
            searched += 1;
            if searched > limit {
                break false;
            }
            for (di, dj) in AROUND {
                let next = (cell.0 + di, cell.1 + dj);
                // Across a corner only when both cells beside it join too: no cutting a wall's end.
                let diagonal = di != 0 && dj != 0;
                if !self.joins(physics, map, cell, next)
                    || diagonal && !(self.joins(physics, map, cell, (cell.0 + di, cell.1)) && self.joins(physics, map, cell, (cell.0, cell.1 + dj)))
                {
                    continue;
                }
                let reached = so_far + if diagonal { 14 } else { 10 };
                if cost.get(&next).is_none_or(|&known| reached < known) {
                    cost.insert(next, reached);
                    came.insert(next, cell);
                    open.push(Reverse((reached + guess(next), reached, next)));
                }
            }
        };
        *budget = budget.saturating_sub(searched);
        found.then(|| self.smooth(physics, map, from, to, trace(&came, start, goal)))
    }

    /// The cells of a found way cut down to its corners: from each corner, on to the furthest
    /// cell of the way still in a clear straight line.
    fn smooth(&mut self, physics: &Physics, map: &WorldFile, from: [f32; 2], to: [f32; 2], cells: Vec<Cell>) -> Vec<[f32; 2]> {
        let mut points: Vec<[f32; 2]> = cells.into_iter().skip(1).map(centre).collect();
        // Finish where it was asked to, if that can be walked to from the last cell, or is just past
        // it: a cell is blocked when anything is in it, and against a wall there is still room to stand.
        let reach = (NEAR_OPEN as f32 + 0.5) * CELL;
        match points.last().copied() {
            Some(last) if (to[0] - last[0]).hypot(to[1] - last[1]) < reach || self.clear_line(physics, map, last, to) => points.push(to),
            None => points.push(to),
            _ => {}
        }
        let mut corners = Vec::new();
        let mut anchor = from;
        let mut i = 0;
        while i < points.len() {
            let mut j = i;
            while j + 1 < points.len() && self.clear_line(physics, map, anchor, points[j + 1]) {
                j += 1;
            }
            corners.push(points[j]);
            anchor = points[j];
            i = j + 1;
        }
        corners
    }
}

/// The cells from `start` to `goal`, both included, from what each was reached from.
fn trace(came: &HashMap<Cell, Cell>, start: Cell, goal: Cell) -> Vec<Cell> {
    let mut cells = vec![goal];
    while let Some(&before) = came.get(cells.last().expect("never empty")) {
        cells.push(before);
        if before == start {
            break;
        }
    }
    cells.reverse();
    cells
}

/// The floor of every cell of chunk `key`, NaN where it is blocked.
fn build_chunk(physics: &Physics, map: &WorldFile, (ci, cj): Cell) -> Box<[f32]> {
    let mut cells = vec![f32::NAN; (CHUNK * CHUNK) as usize].into_boxed_slice();
    for row in 0..CHUNK {
        for column in 0..CHUNK {
            let [x, z] = centre((ci * CHUNK + column, cj * CHUNK + row));
            if !map.is_inside((x * 100.0) as i32, (z * 100.0) as i32) {
                continue;
            }
            let top = map.ground.height_at(x, z) + FLOOR_FROM;
            let Some(down) = physics.cast_ray_static([x, top, z], [0.0, -1.0, 0.0], FLOOR_FROM + FLOOR_BELOW) else { continue };
            let floor = top - down;
            let half = [CELL / 2.0, (BODY_TOP - STEP_CLEAR) / 2.0, CELL / 2.0];
            if !physics.box_touches_static([x, floor + (BODY_TOP + STEP_CLEAR) / 2.0, z], half) {
                cells[(row * CHUNK + column) as usize] = floor;
            }
        }
    }
    cells
}

#[cfg(test)]
mod tests;
