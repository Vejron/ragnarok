//! Craters (CRATERS.md): where a blast bursts on the ground, the ground gives. The world's `Ground` is
//! dug, the physics gets the new heightfield, and the crater is told to every client (`Cratered`), who
//! dig the same bowl in the tiles they hold. A joiner is sent the dug ground itself (`Dug`), and the
//! craters of the last `CRATER_FADE_SECONDS` only for their marks ("Lasting ground").

use super::*;
use shared::protocol::{CraterEntry, DugTile};
use shared::world::NO_WATER;

impl World {
    /// Digs a crater `radius` metres across and `depth` metres deep where a shell burst at `point`, if it
    /// burst on the terrain: within `CRATER_REACH` of the ground, and not over water. Whatever was
    /// standing on the ground there is told it moved. Returns whether the ground changed.
    pub fn dig_crater(&mut self, point: [f32; 3], radius: f32, depth: f32) -> bool {
        let ground = &self.map.ground;
        if ground.is_flat() || radius <= 0.0 || depth <= 0.0 || point[1] > ground.height_at(point[0], point[2]) + CRATER_REACH {
            return false;
        }
        if self.over_water(point[0], point[2]) {
            return false;
        }
        let crater = CraterEntry { x: to_cm(point[0]), z: to_cm(point[2]), radius: to_cm(radius), depth: to_cm(depth), age: 0 };
        if !self.apply_crater(crater) {
            return false;
        }
        self.craters.push_back((self.tick, crater));
        if self.craters.len() > CRATER_MAX_KEPT {
            self.craters.pop_front();
        }
        self.fresh_craters.push(crater);
        true
    }

    /// The craters a joiner is to mark, oldest first, each with its age: those dug in the last
    /// `CRATER_FADE_SECONDS`. Older ones are forgotten here.
    pub fn crater_marks(&mut self) -> Vec<CraterEntry> {
        let fade = CRATER_FADE_SECONDS * TICK_RATE;
        while self.craters.front().is_some_and(|&(dug, _)| self.tick.wrapping_sub(dug) >= fade) {
            self.craters.pop_front();
        }
        self.craters.iter().map(|&(dug, crater)| CraterEntry { age: (self.tick.wrapping_sub(dug) / TICK_RATE) as u16, ..crater }).collect()
    }

    /// Every tile whose ground craters changed, as its difference from the pristine ground (`DugTile`),
    /// in the order of `dug_tiles`.
    pub fn dug_ground(&self) -> Vec<DugTile> {
        let ground = &self.map.ground;
        let Some(per_tile) = self.samples_per_tile() else { return Vec::new() };
        if self.pristine.len() != ground.heights.len() {
            return Vec::new();
        }
        let n = per_tile + 1;
        let mut tiles = Vec::with_capacity(self.dug_tiles.len());
        for &(col, row) in &self.dug_tiles {
            let (i0, j0) = (u32::from(col) * per_tile, u32::from(row) * per_tile);
            let (mut runs, mut skip, mut start) = (Vec::new(), 0i32, None::<usize>);
            for j in 0..n {
                for i in 0..n {
                    let at = ((j0 + j) * ground.side + i0 + i) as usize;
                    let delta = ground.heights[at] - self.pristine[at];
                    if delta == 0 {
                        start = None;
                        skip += 1;
                        continue;
                    }
                    match start {
                        Some(count) => runs[count] += 1,
                        None => {
                            runs.extend([skip, 1]);
                            start = Some(runs.len() - 1);
                            skip = 0;
                        }
                    }
                    runs.push(delta);
                }
            }
            if !runs.is_empty() {
                tiles.push(DugTile { col, row, runs });
            }
        }
        tiles
    }

    /// Ground samples along a tile's side, less one (a tile's last row is its neighbour's first), or
    /// None when the tiles' ground does not line up with the world's.
    fn samples_per_tile(&self) -> Option<u32> {
        let (ground, size) = (&self.map.ground, self.map.tile_size);
        if ground.is_flat() || size <= 0 || size % ground.cell != 0 {
            return None;
        }
        let per_tile = (size / ground.cell) as u32;
        ((ground.side - 1) % per_tile == 0).then_some(per_tile)
    }

    /// Notes the tiles that samples `(i0, j0)` to `(i1, j1)` of the ground fall in, both on a seam.
    fn note_dug(&mut self, (i0, j0, i1, j1): (u32, u32, u32, u32)) {
        let Some(per_tile) = self.samples_per_tile() else { return };
        let last = (self.map.ground.side - 1) / per_tile - 1;
        let tiles = |a: u32, b: u32| (a.div_ceil(per_tile).saturating_sub(1))..=(b / per_tile).min(last);
        for row in tiles(j0, j1) {
            for col in tiles(i0, i1) {
                self.dug_tiles.insert((col as u16, row as u16));
            }
        }
    }

    /// Digs `crater` into the world's ground and queues the new heightfield. False when nothing moved.
    fn apply_crater(&mut self, crater: CraterEntry) -> bool {
        // However many land, no sample goes more than `CRATER_MAX_DEPTH` under what the importer left.
        if self.pristine.is_empty() {
            self.pristine = self.map.ground.heights.clone();
        }
        let (pristine, cap) = (&self.pristine, to_cm(CRATER_MAX_DEPTH));
        let Some(changed) = self.map.ground.dig(crater.x, crater.z, crater.radius, crater.depth, &|at| (pristine[at] - cap).max(0)) else { return false };
        self.physics.dig_ground(&self.map.ground, changed);
        self.note_dug(changed);
        true
    }

    /// Whether the ground sample nearest `(x, z)` has water over it.
    fn over_water(&self, x: f32, z: f32) -> bool {
        let ground = &self.map.ground;
        if self.map.water.len() != ground.heights.len() {
            return false;
        }
        let at = |v: f32| (((to_cm(v) - ground.origin) as f32 / ground.cell as f32).round() as i64).clamp(0, ground.side as i64 - 1);
        self.map.water[(at(z) * i64::from(ground.side) + at(x)) as usize] != NO_WATER
    }
}
