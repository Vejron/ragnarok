//! Open water (`Tile::water`) as something to drive into: how high its surface is under a point, for
//! the vehicles that plane over it or wade through it (`vehicle::water`). Server and client read the
//! same levels, so a crossing is predicted. Frozen water is left out: winter drives on the bed as before.

use shared::quant::from_cm;
use shared::world::{NO_WATER, THEME_WINTER, WorldFile};
use std::collections::BTreeMap;

/// Water surface heights over a square of ground samples.
#[derive(Clone, Debug)]
struct Grid {
    /// Where its first sample is, metres.
    x0: f32,
    z0: f32,
    side: usize,
    /// Surface height at each ground sample, metres, row-major as `Ground::heights`; `NEG_INFINITY` where it is dry.
    levels: Vec<f32>,
    /// How deep the water is at each sample, metres; 0 where it is dry.
    depths: Vec<f32>,
}

/// The water's surface at a point, waves and all: its height (metres), its slope `[dh/dx, dh/dz]` and
/// how fast it is rising there (m/s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub height: f32,
    pub slope: [f32; 2],
    pub rise: f32,
}

/// Every tile's water surface, by the tile it came in. A world that is not cut into whole tiles is
/// one grid, under tile `(0, 0)`.
#[derive(Clone, Debug, Default)]
pub struct Waters {
    by_tile: BTreeMap<(u16, u16), Grid>,
    /// The world's half-extent, a tile's side (0 for a world in one grid) and a ground cell's, metres.
    half: f32,
    tile: f32,
    cell: f32,
    /// Winter: the water is ice and none of it is kept.
    frozen: bool,
    /// The waves are stilled (a test's flat water).
    calm: bool,
}

impl Waters {
    pub fn new(half_extent: i32, tile_size: i32, ground_cell: i32, frozen: bool) -> Waters {
        Waters { by_tile: BTreeMap::new(), half: from_cm(half_extent), tile: from_cm(tile_size), cell: from_cm(ground_cell), frozen, calm: false }
    }

    /// The water of a world's whole `ground`, as `WorldFile::water` holds it: cut back into its tiles
    /// when the ground is made of whole ones, else kept in one grid.
    pub fn of_world(world: &WorldFile) -> Waters {
        let ground = &world.ground;
        let per_tile = if ground.cell > 0 && world.tile_size > 0 && world.tile_size % ground.cell == 0 { (world.tile_size / ground.cell) as u32 } else { 0 };
        let cells = ground.side.saturating_sub(1);
        let tiled = per_tile > 0 && cells.is_multiple_of(per_tile) && ground.origin == -world.half_extent;
        let mut waters = Waters::new(world.half_extent, if tiled { world.tile_size } else { 0 }, ground.cell, world.theme == THEME_WINTER);
        if world.water.len() != ground.heights.len() {
            return waters;
        }
        if !tiled {
            waters.put((0, 0), from_cm(ground.origin), from_cm(ground.origin), &ground.heights, &world.water);
            return waters;
        }
        let n = per_tile + 1;
        for row in 0..cells / per_tile {
            for col in 0..cells / per_tile {
                let (i0, j0) = (col * per_tile, row * per_tile);
                let block = |of: &[i32]| -> Vec<i32> { (0..n).flat_map(|j| (0..n).map(move |i| ((j0 + j) * ground.side + i0 + i) as usize)).map(|at| of[at]).collect() };
                waters.set((col as u16, row as u16), &block(&ground.heights), &block(&world.water));
            }
        }
        waters
    }

    /// Tile `tile`'s water over its `ground`, both centimetres a sample. A sample whose water is not
    /// above its ground is dry.
    pub fn set(&mut self, tile: (u16, u16), ground: &[i32], water: &[i32]) {
        let (x0, z0) = (f32::from(tile.0) * self.tile - self.half, f32::from(tile.1) * self.tile - self.half);
        self.put(tile, x0, z0, ground, water);
    }

    fn put(&mut self, tile: (u16, u16), x0: f32, z0: f32, ground: &[i32], water: &[i32]) {
        if self.frozen || water.is_empty() || water.len() != ground.len() || !water.iter().zip(ground).any(|(&w, &g)| w != NO_WATER && w > g) {
            self.by_tile.remove(&tile);
            return;
        }
        let levels: Vec<f32> = water.iter().zip(ground).map(|(&w, &g)| if w != NO_WATER && w > g { from_cm(w) } else { f32::NEG_INFINITY }).collect();
        let depths = water.iter().zip(ground).map(|(&w, &g)| if w != NO_WATER && w > g { from_cm(w - g) } else { 0.0 }).collect();
        let side = (levels.len() as f32).sqrt().round() as usize;
        self.by_tile.insert(tile, Grid { x0, z0, side, levels, depths });
    }

    /// Stills the waves everywhere.
    pub fn calm(&mut self) {
        self.calm = true;
    }

    pub fn remove(&mut self, tile: (u16, u16)) {
        self.by_tile.remove(&tile);
    }

    /// The water surface height at `(x, z)`, metres, from the nearest ground sample; `None` where it
    /// is dry or the tile is not here.
    pub fn level_at(&self, x: f32, z: f32) -> Option<f32> {
        let (grid, i, j) = self.sample(x, z)?;
        let level = grid.levels[j * grid.side + i];
        level.is_finite().then_some(level)
    }

    /// The surface at `(x, z)` with its waves (`shared::waves`), `time` seconds into their loop: the
    /// waves die away towards the shore, as deep as the water is between the samples around it. `None`
    /// where it is dry or the tile is not here.
    pub fn surface(&self, x: f32, z: f32, time: f32) -> Option<Surface> {
        let level = self.level_at(x, z)?;
        let share = if self.calm { 0.0 } else { shared::waves::shore(self.depth_at(x, z)) };
        let wave = shared::waves::wave(x, z, time);
        Some(Surface { height: level + wave.height * share, slope: wave.slope.map(|slope| slope * share), rise: wave.rise * share })
    }

    /// How deep the water is at `(x, z)`, metres, between the four samples around it, as the water's
    /// mesh draws it; 0 off the grid.
    fn depth_at(&self, x: f32, z: f32) -> f32 {
        let Some((grid, _, _)) = self.sample(x, z) else { return 0.0 };
        let (u, v) = (((x - grid.x0) / self.cell).max(0.0), ((z - grid.z0) / self.cell).max(0.0));
        let last = grid.side - 1;
        let (i, j) = ((u as usize).min(last.saturating_sub(1)), (v as usize).min(last.saturating_sub(1)));
        let (fu, fv) = ((u - i as f32).clamp(0.0, 1.0), (v - j as f32).clamp(0.0, 1.0));
        let at = |i: usize, j: usize| grid.depths[j.min(last) * grid.side + i.min(last)];
        let near = at(i, j) + (at(i + 1, j) - at(i, j)) * fu;
        let far = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fu;
        near + (far - near) * fv
    }

    /// The grid under `(x, z)` and its nearest sample's column and row.
    fn sample(&self, x: f32, z: f32) -> Option<(&Grid, usize, usize)> {
        if self.by_tile.is_empty() || self.cell <= 0.0 {
            return None;
        }
        let tile = if self.tile > 0.0 {
            let (u, v) = ((x + self.half) / self.tile, (z + self.half) / self.tile);
            if u < 0.0 || v < 0.0 {
                return None;
            }
            (u as u16, v as u16)
        } else {
            (0, 0)
        };
        let grid = self.by_tile.get(&tile)?;
        let (i, j) = (((x - grid.x0) / self.cell).round(), ((z - grid.z0) / self.cell).round());
        let last = grid.side as f32 - 1.0;
        if i < 0.0 || j < 0.0 || i > last || j > last {
            return None;
        }
        Some((grid, i as usize, j as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_level_under_a_point() {
        // Tiles of 4 m, the world 8 m across, in 2 m cells: tile (1, 0) is x 0..4, z -4..0, 3 × 3 samples.
        let mut waters = Waters::new(400, 400, 200, false);
        let ground = vec![0; 9];
        waters.set((1, 0), &ground, &[NO_WATER, NO_WATER, NO_WATER, NO_WATER, 150, 150, NO_WATER, 150, -10]);
        assert_eq!(waters.level_at(2.1, -1.9), Some(1.5), "the middle sample");
        assert_eq!(waters.level_at(3.9, -2.1), Some(1.5));
        assert_eq!(waters.level_at(0.2, -3.8), None, "a dry sample");
        assert_eq!(waters.level_at(3.9, -0.1), None, "water under the ground is dry");
        assert_eq!(waters.level_at(-1.0, -1.0), None, "a tile without water");
        waters.remove((1, 0));
        assert_eq!(waters.level_at(2.1, -1.9), None);
    }

    #[test]
    fn a_world_is_cut_back_into_its_tiles() {
        // Two tiles of 4 m in 2 m cells: 5 × 5 samples, with a pool on the shared edge's middle sample.
        let mut world = WorldFile { half_extent: 400, tile_size: 400, ..Default::default() };
        world.ground = shared::terrain::Ground { origin: -400, cell: 200, side: 5, heights: vec![0; 25] };
        world.water = vec![NO_WATER; 25];
        world.water[2 * 5 + 2] = 120;
        let waters = Waters::of_world(&world);
        assert_eq!(waters.level_at(0.4, 0.3), Some(1.2));
        assert_eq!(waters.level_at(-0.4, -0.3), Some(1.2), "the edge sample is in both tiles");
        assert_eq!(waters.level_at(3.0, 3.0), None);
        world.theme = THEME_WINTER;
        assert_eq!(Waters::of_world(&world).level_at(0.4, 0.3), None, "winter water is ice");
    }

    #[test]
    fn waves_ride_on_deep_water_and_die_away_at_the_shore() {
        // One grid of 2 m cells: 3 m deep at x = 0, shallowing to the shore at x = 8.
        let mut world = WorldFile { half_extent: 400, ..Default::default() };
        let ground: Vec<i32> = (0..25).map(|k| [-300, -300, -150, -20, 10][k % 5]).collect();
        world.ground = shared::terrain::Ground { origin: 0, cell: 200, side: 5, heights: ground };
        world.water = vec![0; 25];
        let waters = Waters::of_world(&world);
        let time = 7.3;
        let deep = waters.surface(0.0, 4.0, time).unwrap();
        assert_eq!(deep.height, shared::waves::wave(0.0, 4.0, time).height, "deep water has the whole wave");
        let shallow = waters.surface(6.0, 4.0, time).unwrap();
        assert_eq!((shallow.height, shallow.rise), (0.0, 0.0), "20 cm of water lies flat");
    }

    #[test]
    fn frozen_water_is_not_kept() {
        let mut waters = Waters::new(400, 400, 200, true);
        waters.set((1, 0), &[0; 9], &[150; 9]);
        assert_eq!(waters.level_at(2.0, -2.0), None);
    }
}
