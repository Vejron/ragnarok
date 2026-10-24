//! Cubes players build with (`ITEM_BRICKS`): one sparse layer for the whole world, on the same lattice
//! of `CELL` cubes as the buildings' cells (`carve`), so a cube put against a wall lines up with it.
//!
//! A placed cube is named by its lattice position (`Pos`, centimetres divided by `CELL`), not by a
//! building's cell number: a building's grid is its bounding box, fixed when the world was made, and
//! a build may go on past it as far as anyone likes, or stand on the ground on its own. The cubes are
//! kept in chunks of `CHUNK` cubes a side, each drawn and collided with on its own, so placing one
//! cube rebuilds one small chunk. The server decides what is placed, dug and falls and tells everyone
//! the positions (`Built`); clients keep the same layer and draw and collide with it through this code
//! (built to WebAssembly in `crates/sim-wasm`).
//!
//! Every cube is of a `Material` (`MATERIALS`), which says how tough it is: how much bullet damage it
//! takes to break and how far into a blast it holds. Bricks are the only one so far; a tougher or a
//! flimsier block is a row of `MATERIALS` and an item that places it.

use crate::carve::{CELL, CELL_HP, boxes_of, faces_of, Tri};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// What a cube is made of, and so how tough it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub name: &'static str,
    /// Bullet damage that breaks a cube of it: each round takes its damage off, as off a building's cell.
    pub hp: u8,
    /// How much of a blast's reach into walls takes a cube of it out: 1 as far as a wall, less for a
    /// tougher one. At most 1.
    pub blast_reach: f32,
}

/// Brick: as tough as a building's own wall (a few rifle rounds, `CELL_HP`) and blasted as far.
pub const MATERIAL_BRICK: u8 = 0;

/// Every material, by id (`Cube::material`).
pub const MATERIALS: &[Material] = &[Material { name: "Brick", hp: CELL_HP, blast_reach: 1.0 }];

/// The material `id` names; brick for an unknown one.
pub fn material(id: u8) -> &'static Material {
    MATERIALS.get(id as usize).unwrap_or(&MATERIALS[MATERIAL_BRICK as usize])
}

/// One placed cube: its colour (the builder's, `PLAYER_COLOR_COUNT` colours) and its material (`MATERIALS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cube {
    pub colour: u8,
    pub material: u8,
}

/// Cubes a chunk is along each side.
pub const CHUNK: i32 = 16;
/// Cubes in one chunk.
pub const CHUNK_CELLS: usize = (CHUNK * CHUNK * CHUNK) as usize;

/// A cube's place on the world lattice: its corner with the smallest coordinates is `pos * CELL`
/// centimetres.
pub type Pos = [i32; 3];

/// The six cubes that share a face with a cube, as steps.
pub const NEIGHBOURS: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// The lattice cube holding the point `cm` (centimetres).
pub fn cell_of(cm: [i32; 3]) -> Pos {
    cm.map(|v| v.div_euclid(CELL))
}

/// The lattice cube holding the point `at` (metres).
pub fn cell_at(at: [f32; 3]) -> Pos {
    at.map(|v| ((v * 100.0).floor() as i32).div_euclid(CELL))
}

/// The middle of cube `pos`, centimetres.
pub fn centre(pos: Pos) -> [i32; 3] {
    pos.map(|v| v * CELL + CELL / 2)
}

/// The chunk cube `pos` is in.
pub fn chunk_of(pos: Pos) -> Pos {
    pos.map(|v| v.div_euclid(CHUNK))
}

/// The corner of chunk `chunk` with the smallest coordinates, centimetres.
pub fn chunk_origin(chunk: Pos) -> [i32; 3] {
    chunk.map(|v| v * CHUNK * CELL)
}

/// Cube `pos`'s number in its chunk: `(y * CHUNK + z) * CHUNK + x`, the order `carve` numbers a grid's cells.
fn slot(pos: Pos) -> usize {
    let [x, y, z] = pos.map(|v| v.rem_euclid(CHUNK) as usize);
    (y * CHUNK as usize + z) * CHUNK as usize + x
}

/// `pos` moved by `step`.
pub fn step(pos: Pos, step: Pos) -> Pos {
    [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]]
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Chunk {
    /// One byte a cube, in `slot` order: 0 for none, else its colour plus one.
    cells: Vec<u8>,
    /// Each cube's material, in `slot` order; 0 where there is none.
    materials: Vec<u8>,
    /// How many cubes it holds.
    count: u16,
}

/// Every placed cube in the world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Construct {
    /// Chunks with at least one cube, in order, so everything that walks them does it the same way.
    chunks: BTreeMap<Pos, Chunk>,
    count: usize,
    /// Health left in the cubes bullets have hurt; a cube not listed is whole (its material's `hp`).
    hurt: HashMap<Pos, u8>,
}

impl Construct {
    /// How many cubes are placed.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The cube at `pos`, if there is one.
    pub fn get(&self, pos: Pos) -> Option<Cube> {
        let chunk = self.chunks.get(&chunk_of(pos))?;
        let at = slot(pos);
        let byte = chunk.cells[at];
        (byte != 0).then(|| Cube { colour: byte - 1, material: chunk.materials[at] })
    }

    /// Whether a cube is at `pos`.
    pub fn has(&self, pos: Pos) -> bool {
        self.get(pos).is_some()
    }

    /// Puts `cube` at `pos`. False, and nothing changes, when one is there already.
    pub fn place(&mut self, pos: Pos, cube: Cube) -> bool {
        let chunk = self.chunks.entry(chunk_of(pos)).or_insert_with(|| Chunk { cells: vec![0; CHUNK_CELLS], materials: vec![0; CHUNK_CELLS], count: 0 });
        let at = slot(pos);
        if chunk.cells[at] != 0 {
            return false;
        }
        chunk.cells[at] = cube.colour.saturating_add(1);
        chunk.materials[at] = cube.material;
        chunk.count += 1;
        self.count += 1;
        true
    }

    /// Takes the cube at `pos` away. Returns it, or `None` when there was none.
    pub fn remove(&mut self, pos: Pos) -> Option<Cube> {
        let key = chunk_of(pos);
        let chunk = self.chunks.get_mut(&key)?;
        let at = slot(pos);
        if chunk.cells[at] == 0 {
            return None;
        }
        let cube = Cube { colour: chunk.cells[at] - 1, material: chunk.materials[at] };
        chunk.cells[at] = 0;
        chunk.materials[at] = 0;
        chunk.count -= 1;
        self.count -= 1;
        if chunk.count == 0 {
            self.chunks.remove(&key);
        }
        self.hurt.remove(&pos);
        Some(cube)
    }

    /// A round of `damage` hits the cube at `pos`: it loses that much health. Returns true when it has
    /// none left and breaks (it is still placed, for the caller to take out), false when it holds or
    /// there is no cube.
    pub fn chip(&mut self, pos: Pos, damage: u8) -> bool {
        let Some(cube) = self.get(pos) else { return false };
        let hp = self.hurt.get(&pos).copied().unwrap_or(material(cube.material).hp);
        let left = hp.saturating_sub(damage);
        if left == 0 {
            return true;
        }
        self.hurt.insert(pos, left);
        false
    }

    /// Health left in the cube at `pos`: its material's whole `hp` until bullets hurt it. `None` for no cube.
    pub fn hp(&self, pos: Pos) -> Option<u8> {
        let cube = self.get(pos)?;
        Some(self.hurt.get(&pos).copied().unwrap_or(material(cube.material).hp))
    }

    /// Every cube, chunk by chunk and in `slot` order within a chunk.
    pub fn cubes(&self) -> impl Iterator<Item = (Pos, Cube)> + '_ {
        self.chunks.iter().flat_map(|(chunk, data)| {
            let origin = chunk.map(|v| v * CHUNK);
            data.cells.iter().enumerate().filter(|(_, byte)| **byte != 0).map(move |(i, byte)| {
                let n = CHUNK as usize;
                let (x, z, y) = (i % n, (i / n) % n, i / (n * n));
                ([origin[0] + x as i32, origin[1] + y as i32, origin[2] + z as i32], Cube { colour: byte - 1, material: data.materials[i] })
            })
        })
    }

    /// Chunk `chunk`'s cubes, one byte each in cell order (0 for none, else colour plus one), or
    /// `None` when it holds none.
    pub fn chunk_cells(&self, chunk: Pos) -> Option<&[u8]> {
        self.chunks.get(&chunk).map(|data| data.cells.as_slice())
    }

    /// What chunk `chunk` is collided with: its cubes merged into boxes (`chunk_boxes`).
    pub fn boxes(&self, chunk: Pos) -> Vec<[i32; 6]> {
        self.chunk_cells(chunk).map_or_else(Vec::new, |cells| chunk_boxes(chunk, cells))
    }

    /// The cubes a blast that bites walls `radius` centimetres round `at` (centimetres) takes out: those
    /// whose middles are within their material's share of it (`Material::blast_reach`), in order.
    pub fn blasted(&self, at: [i32; 3], radius: i32) -> Vec<Pos> {
        let reach = |pos: Pos| self.get(pos).map_or(0.0, |cube| material(cube.material).blast_reach.clamp(0.0, 1.0));
        self.near(at, radius)
            .into_iter()
            .filter(|&pos| {
                let c = centre(pos);
                let d2 = (0..3).map(|a| f64::from(c[a] - at[a]).powi(2)).sum::<f64>();
                d2 <= (f64::from(radius) * f64::from(reach(pos))).powi(2)
            })
            .collect()
    }

    /// The cube whose middle is nearest `at` (centimetres), if one is within `radius` centimetres: the
    /// one a contact on its face at `at` touched. The first in order wins a tie.
    pub fn nearest(&self, at: [i32; 3], radius: i32) -> Option<Pos> {
        let d2 = |pos: Pos| {
            let c = centre(pos);
            (0..3).map(|a| i64::from(c[a] - at[a]).pow(2)).sum::<i64>()
        };
        self.near(at, radius).into_iter().min_by_key(|&pos| d2(pos))
    }

    /// The cubes whose middles are within `radius` centimetres of `at` (centimetres), in order.
    pub fn near(&self, at: [i32; 3], radius: i32) -> Vec<Pos> {
        if self.is_empty() || radius <= 0 {
            return Vec::new();
        }
        let lo = cell_of(at.map(|v| v - radius));
        let hi = cell_of(at.map(|v| v + radius));
        let r2 = i64::from(radius).pow(2);
        let mut out = Vec::new();
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let pos = [x, y, z];
                    if !self.has(pos) {
                        continue;
                    }
                    let c = centre(pos);
                    if (0..3).map(|a| i64::from(c[a] - at[a]).pow(2)).sum::<i64>() <= r2 {
                        out.push(pos);
                    }
                }
            }
        }
        out
    }

    /// Whether a cube could go at `pos`: nothing is there, and it would rest on something, a cube
    /// beside, above or below it, or whatever `held` says holds it from outside the layer (the ground,
    /// a building's wall).
    pub fn can_rest(&self, pos: Pos, held: impl FnOnce(Pos) -> bool) -> bool {
        !self.has(pos) && (NEIGHBOURS.iter().any(|&s| self.has(step(pos, s))) || held(pos))
    }

    /// The cubes that nothing holds up any more, among those joined to `seeds`: each piece of cubes
    /// sharing faces is looked through until a cube `held` says rests on something outside the layer
    /// (the ground, a standing building cell). A piece with none falls whole. A piece bigger than
    /// `most` cubes is taken as held without looking further, so a huge build never costs a long tick.
    /// Returns them in order; they are still placed, for the caller to take away.
    pub fn unsupported(&self, seeds: &[Pos], mut held: impl FnMut(Pos) -> bool, most: usize) -> Vec<Pos> {
        let mut seeds = seeds.to_vec();
        seeds.sort_unstable();
        seeds.dedup();
        let mut seen: HashSet<Pos> = HashSet::new();
        let mut out = Vec::new();
        for seed in seeds {
            if !self.has(seed) || !seen.insert(seed) {
                continue;
            }
            let mut piece = vec![seed];
            let mut queue = VecDeque::from([seed]);
            let mut holds = false;
            while let Some(pos) = queue.pop_front() {
                if held(pos) || piece.len() > most {
                    holds = true;
                    break;
                }
                for &s in &NEIGHBOURS {
                    let next = step(pos, s);
                    if self.has(next) && seen.insert(next) {
                        piece.push(next);
                        queue.push_back(next);
                    }
                }
            }
            if !holds {
                out.extend(piece);
            }
        }
        out.sort_unstable();
        out
    }
}

/// What a chunk's cubes are collided with: `cells` (one byte a cube in cell order, 0 for none) merged
/// into boxes by `carve::boxes_of`, centimetres. The client and the server build the same boxes.
pub fn chunk_boxes(chunk: Pos, cells: &[u8]) -> Vec<[i32; 6]> {
    if cells.len() != CHUNK_CELLS {
        return Vec::new();
    }
    let n = CHUNK as usize;
    boxes_of(chunk_origin(chunk), [n, n, n], cells.iter().map(|&byte| byte != 0).collect())
}

/// What a chunk's cubes are drawn as: the faces that look onto no cube of the chunk, those of one
/// colour merged into rectangles (`carve::faces_of`). Calls `emit` with each triangle and the colour.
pub fn chunk_faces(chunk: Pos, cells: &[u8], mut emit: impl FnMut(Tri, u8)) {
    if cells.len() != CHUNK_CELLS {
        return;
    }
    let n = CHUNK as usize;
    let keys: Vec<i32> = cells.iter().map(|&byte| i32::from(byte) - 1).collect();
    faces_of(chunk_origin(chunk), [n, n, n], &keys, |tri, key| emit(tri, key as u8));
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Construct {
        /// A brick of `colour` at `pos`.
        fn place_colour(&mut self, pos: Pos, colour: u8) -> bool {
            self.place(pos, Cube { colour, material: MATERIAL_BRICK })
        }
    }

    #[test]
    fn cubes_are_found_where_they_were_put_on_either_side_of_zero() {
        let mut built = Construct::default();
        for pos in [[0, 0, 0], [-1, 0, 0], [15, 3, -16], [16, 3, -17], [-100, 40, 7]] {
            assert!(built.place_colour(pos, 3));
        }
        assert!(!built.place_colour([0, 0, 0], 5), "a cube is never put on another");
        assert_eq!(built.len(), 5);
        assert_eq!(built.get([-1, 0, 0]), Some(Cube { colour: 3, material: MATERIAL_BRICK }));
        assert_eq!(built.get([1, 0, 0]), None);
        assert_eq!(built.remove([15, 3, -16]).map(|cube| cube.colour), Some(3));
        assert_eq!(built.remove([15, 3, -16]), None);
        assert_eq!(built.len(), 4);
        let listed: Vec<Pos> = built.cubes().map(|(pos, _)| pos).collect();
        assert_eq!(listed.len(), 4);
        for pos in listed {
            assert!(built.has(pos));
        }
    }

    #[test]
    fn an_empty_chunk_is_forgotten() {
        let mut built = Construct::default();
        built.place_colour([40, 1, 40], 0);
        built.remove([40, 1, 40]);
        assert!(built.is_empty());
        assert_eq!(built.chunk_cells(chunk_of([40, 1, 40])), None);
    }

    #[test]
    fn a_row_of_cubes_is_one_box() {
        let mut built = Construct::default();
        for x in 0..4 {
            built.place_colour([x, 2, -1], 0);
        }
        let chunk = chunk_of([0, 2, -1]);
        assert_eq!(chunk, [0, 0, -1]);
        assert_eq!(built.boxes(chunk), vec![[0, 2 * CELL, -CELL, 4 * CELL, 3 * CELL, 0]]);
    }

    #[test]
    fn a_lone_cube_has_twelve_triangles() {
        let mut built = Construct::default();
        built.place_colour([3, 3, 3], 7);
        let mut count = 0;
        chunk_faces([0, 0, 0], built.chunk_cells([0, 0, 0]).unwrap(), |_, colour| {
            assert_eq!(colour, 7);
            count += 1;
        });
        assert_eq!(count, 12);
    }

    #[test]
    fn a_cube_rests_on_a_neighbour_or_on_what_holds_it() {
        let mut built = Construct::default();
        built.place_colour([0, 0, 0], 0);
        assert!(built.can_rest([0, 1, 0], |_| false));
        assert!(!built.can_rest([0, 2, 0], |_| false));
        assert!(built.can_rest([0, 2, 0], |_| true));
        assert!(!built.can_rest([0, 0, 0], |_| true), "never where a cube is");
    }

    #[test]
    fn a_tower_falls_when_its_foot_goes() {
        let mut built = Construct::default();
        for y in 0..10 {
            built.place_colour([0, y, 0], 0);
        }
        // Only the bottom cube stands on the ground.
        let ground = |pos: Pos| pos[1] == 0;
        assert!(built.unsupported(&[[0, 5, 0]], ground, 1000).is_empty());
        built.remove([0, 0, 0]);
        let fell = built.unsupported(&[[0, 1, 0]], ground, 1000);
        assert_eq!(fell, (1..10).map(|y| [0, y, 0]).collect::<Vec<_>>());
    }

    #[test]
    fn a_piece_too_big_to_look_through_is_taken_as_held() {
        let mut built = Construct::default();
        for x in 0..50 {
            built.place_colour([x, 10, 0], 0);
        }
        assert!(built.unsupported(&[[0, 10, 0]], |_| false, 20).is_empty());
        assert_eq!(built.unsupported(&[[0, 10, 0]], |_| false, 100).len(), 50);
    }

    #[test]
    fn near_finds_the_cubes_in_reach() {
        let mut built = Construct::default();
        for x in -5..5 {
            built.place_colour([x, 0, 0], 0);
        }
        let found = built.near(centre([0, 0, 0]), CELL * 2);
        assert_eq!(found, vec![[-2, 0, 0], [-1, 0, 0], [0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    }

    #[test]
    fn bullets_wear_a_brick_down_to_nothing() {
        let mut built = Construct::default();
        built.place_colour([0, 0, 0], 0);
        let hp = material(MATERIAL_BRICK).hp;
        assert_eq!(built.hp([0, 0, 0]), Some(hp));
        assert!(!built.chip([0, 0, 0], hp - 1));
        assert_eq!(built.hp([0, 0, 0]), Some(1));
        assert!(built.chip([0, 0, 0], 1), "the last of it breaks it");
        assert!(!built.chip([5, 0, 0], 255), "no cube there");
        built.remove([0, 0, 0]);
        built.place_colour([0, 0, 0], 0);
        assert_eq!(built.hp([0, 0, 0]), Some(hp), "a new cube in the same place is whole");
    }

    #[test]
    fn every_material_is_blasted_no_further_than_a_wall() {
        assert!(MATERIALS.iter().all(|m| m.hp > 0 && m.blast_reach > 0.0 && m.blast_reach <= 1.0));
        assert_eq!(material(200).name, "Brick");
    }
}
