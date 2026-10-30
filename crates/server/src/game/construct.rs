//! The cubes players build with bricks (`shared::construct`), on the world's side: placing and digging
//! them, what blasts take, what falls when what held it goes, and their colliders. Who may place what,
//! and where they aim, is the `Building` mechanic's (`mechanics/building.rs`).

use super::*;
use shared::carve::CELL;
use shared::construct::{Cube, NEIGHBOURS, Pos, cell_of, centre, chunk_of, step};

/// Half a cube's side, metres.
pub const HALF_CUBE: f32 = CELL as f32 / 200.0;

/// The middle of cube `pos`, metres.
pub fn cube_centre(pos: Pos) -> [f32; 3] {
    centre(pos).map(from_cm)
}

impl World {
    /// Whether cube `pos` rests on something outside the placed cubes: the ground under it (its bottom
    /// at or below the terrain there), or a standing cell of a building beside, above or below it.
    pub fn held_from_outside(&mut self, pos: Pos) -> bool {
        let middle = cube_centre(pos);
        let bottom = from_cm(pos[1] * CELL);
        if bottom <= self.map.ground.height_at(middle[0], middle[2]) + 0.05 {
            return true;
        }
        let near: Vec<ObjId> = self.physics.objects_near(middle, HALF_CUBE + 0.15).into_iter().filter_map(|object| if let Obj::Building(id) = object { Some(id) } else { None }).collect();
        near.into_iter().any(|id| NEIGHBOURS.iter().any(|&s| self.building_cell_standing(id, step(pos, s))))
    }

    /// Whether building `id` has a standing cell at lattice place `pos`. Cuts the building into cells
    /// the first time it is asked, as a blast does.
    fn building_cell_standing(&mut self, id: ObjId, pos: Pos) -> bool {
        let Some(carving) = self.carving(id) else { return false };
        let shell = &carving.shell;
        let at: [i64; 3] = std::array::from_fn(|a| (i64::from(pos[a]) * i64::from(CELL) - i64::from(shell.origin[a])).div_euclid(i64::from(CELL)));
        if (0..3).any(|a| at[a] < 0 || at[a] >= shell.dims[a] as i64) {
            return false;
        }
        let cell = shell.index(at[0] as usize, at[1] as usize, at[2] as usize);
        shell.is_solid(cell) && !carving.removed[cell as usize]
    }

    /// Puts `cube` at `pos`, to be told and collided with from the end of this tick. False when one is
    /// there already. Whether it may go there at all is for the caller to check.
    pub fn place_cube(&mut self, pos: Pos, cube: Cube) -> bool {
        if !self.construct.place(pos, cube) {
            return false;
        }
        self.fresh.built.placed.push((pos, cube));
        self.construct_dirty.insert(chunk_of(pos));
        true
    }

    /// Takes the placed cube at `pos` out (dug or shot), and brings down whatever only it held up. False when there was none.
    pub fn dig_cube(&mut self, pos: Pos) -> bool {
        if self.construct.remove(pos).is_none() {
            return false;
        }
        self.fresh.built.removed.push(pos);
        self.construct_dirty.insert(chunk_of(pos));
        self.drop_cubes(&NEIGHBOURS.map(|s| step(pos, s)));
        true
    }

    /// A round of `damage` strikes the placed cube at `pos`: it loses that much health, and breaks out
    /// when it has none (`Construct::chip`), bringing down whatever only it held up. Returns whether it broke.
    pub fn chip_cube(&mut self, pos: Pos, damage: u8) -> bool {
        self.construct.chip(pos, damage) && self.dig_cube(pos)
    }

    /// A blast that bites walls `radius` metres round `point` takes out the placed cubes in its reach
    /// (`Construct::blasted`: less for a tougher material), and whatever only they held up.
    pub fn blast_cubes(&mut self, point: [f32; 3], radius: f32) {
        let gone = self.construct.blasted(point.map(to_cm), (radius * 100.0).round() as i32);
        if gone.is_empty() {
            return;
        }
        let mut seeds = Vec::new();
        for &pos in &gone {
            self.construct.remove(pos);
            self.construct_dirty.insert(chunk_of(pos));
            seeds.extend(NEIGHBOURS.map(|s| step(pos, s)));
        }
        self.fresh.built.removed.extend(gone);
        self.drop_cubes(&seeds);
    }

    /// Cells were cut from building `id`: the placed cubes that rested on them may have nothing left to hold them.
    pub(super) fn loosen_beside_cells(&mut self, id: ObjId, cut: &Cut) {
        if self.construct.is_empty() {
            return;
        }
        let Some(carving) = self.carves.get(&id) else { return };
        let seeds: Vec<Pos> = cut
            .blasted
            .iter()
            .chain(&cut.fell)
            .flat_map(|&cell| {
                let pos = cell_of(carving.shell.centre(cell));
                NEIGHBOURS.map(|s| step(pos, s))
            })
            .filter(|&pos| self.construct.has(pos))
            .collect();
        self.drop_cubes(&seeds);
    }

    /// Takes out every placed cube joined to `seeds` that nothing holds up any more (`Construct::unsupported`),
    /// as `fell`, and drops it on whoever is under it as rubble.
    fn drop_cubes(&mut self, seeds: &[Pos]) {
        let seeds: Vec<Pos> = seeds.iter().copied().filter(|&pos| self.construct.has(pos)).collect();
        if seeds.is_empty() {
            return;
        }
        // The layer is set aside while it is looked through, so asking what holds a cube can cut buildings into cells.
        let construct = std::mem::take(&mut self.construct);
        let fell = construct.unsupported(&seeds, |pos| self.held_from_outside(pos), BUILD_SUPPORT_SEARCH);
        self.construct = construct;
        if fell.is_empty() {
            return;
        }
        let mut columns: BTreeMap<(i32, i32), (f32, u32)> = BTreeMap::new();
        for &pos in &fell {
            self.construct.remove(pos);
            self.construct_dirty.insert(chunk_of(pos));
            let column = columns.entry((pos[0], pos[2])).or_insert((f32::MIN, 0));
            column.0 = column.0.max(cube_centre(pos)[1]);
            column.1 += 1;
        }
        for ((x, z), (top, cells)) in columns {
            let at = [from_cm(x * CELL + CELL / 2), from_cm(z * CELL + CELL / 2)];
            let ground = self.map.ground.height_at(at[0], at[1]);
            let fall = (top - ground).max(0.0);
            let due = self.tick.wrapping_add(((2.0 * fall / 9.8).sqrt() * TICK_RATE as f32).ceil() as u32 + 1);
            self.rubble.push(Rubble { due, at, top, ground, cells });
        }
        self.fresh.built.fell.extend(fell);
    }

    /// Gives every chunk of cubes that changed this tick its new collider.
    pub fn flush_construct(&mut self) {
        for chunk in std::mem::take(&mut self.construct_dirty) {
            let shape = sim::solid_of(&self.construct.boxes(chunk), &[]);
            self.physics.set_construct(chunk, shape);
        }
    }
}
