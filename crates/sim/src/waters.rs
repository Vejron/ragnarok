//! Open water (`Tile::water`) as something to drive into: how high its surface is under a point, for
//! the vehicles that plane over it or wade through it (`vehicle::water`). Server and client read the
//! same levels, so a crossing is predicted. Frozen water is left out: winter drives on the bed as before.

use shared::quant::from_cm;
use shared::world::{FLOW_STEP, NO_WATER, REACH_STEP, WorldFile};
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
    /// How wide the open water is around each sample, metres (`Tile::reach`).
    reaches: Vec<f32>,
    /// The water's velocity at each sample, `[x, z]` m/s (`Tile::flow`); empty where it is all still.
    flows: Vec<[f32; 2]>,
}

/// The water's surface at a point, waves and all: its height (metres), its slope `[dh/dx, dh/dz]`,
/// how fast it is rising there (m/s), the current that carries what floats in it, `[x, z]` m/s, and
/// how the waves swing the water there across, `[x, z]` m/s (forwards on a crest, back in a trough).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub height: f32,
    pub slope: [f32; 2],
    pub rise: f32,
    pub flow: [f32; 2],
    pub sway: [f32; 2],
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
        let mut waters = Waters::new(world.half_extent, if tiled { world.tile_size } else { 0 }, ground.cell, world.frozen());
        if world.water.len() != ground.heights.len() {
            return waters;
        }
        if !tiled {
            waters.put((0, 0), from_cm(ground.origin), from_cm(ground.origin), &ground.heights, &world.water, &world.reach, &world.flow);
            return waters;
        }
        let n = per_tile + 1;
        for row in 0..cells / per_tile {
            for col in 0..cells / per_tile {
                let (i0, j0) = (col * per_tile, row * per_tile);
                let at = || (0..n).flat_map(|j| (0..n).map(move |i| ((j0 + j) * ground.side + i0 + i) as usize));
                let block = |of: &[i32]| -> Vec<i32> { at().map(|at| of[at]).collect() };
                let reach: Vec<u8> = if world.reach.len() == world.water.len() { at().map(|at| world.reach[at]).collect() } else { Vec::new() };
                let flow: Vec<i8> = if world.flow.len() == 2 * world.water.len() { at().flat_map(|at| [world.flow[2 * at], world.flow[2 * at + 1]]).collect() } else { Vec::new() };
                waters.set((col as u16, row as u16), &block(&ground.heights), &block(&world.water), &reach, &flow);
            }
        }
        waters
    }

    /// Tile `tile`'s water over its `ground`, both centimetres a sample, with how wide the open water
    /// is around each (`Tile::reach`; empty is none, a calm pond) and which way it runs
    /// (`Tile::flow`; empty is still water). A sample whose water is not above its ground is dry.
    pub fn set(&mut self, tile: (u16, u16), ground: &[i32], water: &[i32], reach: &[u8], flow: &[i8]) {
        let (x0, z0) = (f32::from(tile.0) * self.tile - self.half, f32::from(tile.1) * self.tile - self.half);
        self.put(tile, x0, z0, ground, water, reach, flow);
    }

    #[allow(clippy::too_many_arguments)]
    fn put(&mut self, tile: (u16, u16), x0: f32, z0: f32, ground: &[i32], water: &[i32], reach: &[u8], flow: &[i8]) {
        if self.frozen || water.is_empty() || water.len() != ground.len() || !water.iter().zip(ground).any(|(&w, &g)| w != NO_WATER && w > g) {
            self.by_tile.remove(&tile);
            return;
        }
        let levels: Vec<f32> = water.iter().zip(ground).map(|(&w, &g)| if w != NO_WATER && w > g { from_cm(w) } else { f32::NEG_INFINITY }).collect();
        let depths = water.iter().zip(ground).map(|(&w, &g)| if w != NO_WATER && w > g { from_cm(w - g) } else { 0.0 }).collect();
        let reaches = if reach.len() == water.len() { reach.iter().map(|&steps| f32::from(steps) * REACH_STEP).collect() } else { vec![0.0; water.len()] };
        let flows = if flow.len() == 2 * water.len() { flow.chunks_exact(2).map(|v| [f32::from(v[0]) * FLOW_STEP, f32::from(v[1]) * FLOW_STEP]).collect() } else { Vec::new() };
        let side = (levels.len() as f32).sqrt().round() as usize;
        self.by_tile.insert(tile, Grid { x0, z0, side, levels, depths, reaches, flows });
    }

    /// Stills the waves everywhere.
    pub fn calm(&mut self) {
        self.calm = true;
    }

    pub fn remove(&mut self, tile: (u16, u16)) {
        self.by_tile.remove(&tile);
    }

    /// The still water surface's height at `(x, z)`, metres; `None` where the nearest ground sample
    /// is dry or the tile is not here. A river's surface falls along it and is shaped by its
    /// current, so the height is read between the wet samples around the point, as the water's
    /// mesh draws it.
    pub fn level_at(&self, x: f32, z: f32) -> Option<f32> {
        self.still_at(x, z).map(|(level, _)| level)
    }

    /// The still surface at `(x, z)`: its height, metres, and its slope `[dh/dx, dh/dz]`. The
    /// height is blended from the wet samples of the four round the point; the slope is that of
    /// the cell they make, and level where one of them is dry (the shore).
    fn still_at(&self, x: f32, z: f32) -> Option<(f32, [f32; 2])> {
        let (grid, near_i, near_j) = self.sample(x, z)?;
        if !grid.levels[near_j * grid.side + near_i].is_finite() {
            return None;
        }
        let (u, v) = (((x - grid.x0) / self.cell).max(0.0), ((z - grid.z0) / self.cell).max(0.0));
        let last = grid.side - 1;
        let (i, j) = ((u as usize).min(last.saturating_sub(1)), (v as usize).min(last.saturating_sub(1)));
        let (fu, fv) = ((u - i as f32).clamp(0.0, 1.0), (v - j as f32).clamp(0.0, 1.0));
        let at = |i: usize, j: usize| grid.levels[j.min(last) * grid.side + i.min(last)];
        let corners = [(at(i, j), (1.0 - fu) * (1.0 - fv)), (at(i + 1, j), fu * (1.0 - fv)), (at(i, j + 1), (1.0 - fu) * fv), (at(i + 1, j + 1), fu * fv)];
        let (mut sum, mut weight) = (0.0, 0.0);
        for (level, share) in corners {
            if level.is_finite() {
                sum += level * share;
                weight += share;
            }
        }
        let [a, b, c, d] = corners.map(|(level, _)| level);
        let slope = if [a, b, c, d].iter().all(|level| level.is_finite()) {
            [((b - a) * (1.0 - fv) + (d - c) * fv) / self.cell, ((c - a) * (1.0 - fu) + (d - b) * fu) / self.cell]
        } else {
            [0.0; 2]
        };
        // The nearest sample is wet, so it is one of the four and their shares are not all nothing.
        Some((if weight > 0.0 { sum / weight } else { grid.levels[near_j * grid.side + near_i] }, slope))
    }

    /// How wide the open water is at `(x, z)`, metres, from the nearest ground sample; 0 off the grid.
    fn reach_at(&self, x: f32, z: f32) -> f32 {
        self.sample(x, z).map_or(0.0, |(grid, i, j)| grid.reaches[j * grid.side + i])
    }

    /// The highest still water surface at `(x, z)` or `reach` metres either way along x or z, metres:
    /// water near enough to be standing in at the edge of the grid's samples.
    pub fn level_near(&self, x: f32, z: f32, reach: f32) -> Option<f32> {
        [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)].into_iter().filter_map(|(dx, dz)| self.level_at(x + dx, z + dz)).reduce(f32::max)
    }

    /// The surface at `(x, z)` with its waves (`shared::waves`), `time` seconds into their loop: they
    /// are as big as the water is wide, and die away towards the shore, as deep as the water is
    /// between the samples around it. Its slope is the waves' and the still surface's own (a river
    /// falls, and stands in waves where it runs fast). `None` where it is dry or the tile is not here.
    pub fn surface(&self, x: f32, z: f32, time: f32) -> Option<Surface> {
        let (level, lie) = self.still_at(x, z)?;
        let share = if self.calm { 0.0 } else { shared::waves::shore(self.depth_at(x, z)) };
        let flow = self.flow_at(x, z);
        let swell = shared::waves::wave(x, z, time, self.reach_at(x, z), share);
        // The surge of a rapid's standing waves, on top of whatever chop the water's width raises.
        let churn = if self.calm { shared::waves::Wave::default() } else { shared::waves::churn(x, z, time, flow) };
        let wave = shared::waves::Wave {
            height: swell.height + churn.height,
            slope: [lie[0] + swell.slope[0] + churn.slope[0], lie[1] + swell.slope[1] + churn.slope[1]],
            rise: swell.rise + churn.rise,
            orbit: swell.orbit,
        };
        Some(Surface { height: level + wave.height, slope: wave.slope, rise: wave.rise, flow, sway: wave.orbit })
    }

    /// The current at `(x, z)`, `[x, z]` m/s, between the four samples around it; still off the grid
    /// and on still water.
    pub fn flow_at(&self, x: f32, z: f32) -> [f32; 2] {
        let Some((grid, _, _)) = self.sample(x, z) else { return [0.0; 2] };
        if grid.flows.is_empty() {
            return [0.0; 2];
        }
        let (u, v) = (((x - grid.x0) / self.cell).max(0.0), ((z - grid.z0) / self.cell).max(0.0));
        let last = grid.side - 1;
        let (i, j) = ((u as usize).min(last.saturating_sub(1)), (v as usize).min(last.saturating_sub(1)));
        let (fu, fv) = ((u - i as f32).clamp(0.0, 1.0), (v - j as f32).clamp(0.0, 1.0));
        let at = |i: usize, j: usize| grid.flows[j.min(last) * grid.side + i.min(last)];
        let mix = |a: [f32; 2], b: [f32; 2], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        mix(mix(at(i, j), at(i + 1, j), fu), mix(at(i, j + 1), at(i + 1, j + 1), fu), fv)
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
        waters.set((1, 0), &ground, &[NO_WATER, NO_WATER, NO_WATER, NO_WATER, 150, 150, NO_WATER, 150, -10], &[], &[]);
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
        world.theme = shared::world::THEME_WINTER;
        assert_eq!(Waters::of_world(&world).level_at(0.4, 0.3), None, "winter water is ice");
        world.open_water = true;
        assert_eq!(Waters::of_world(&world).level_at(0.4, 0.3), Some(1.2), "unless it was left open");
    }

    #[test]
    fn waves_ride_on_deep_water_and_die_away_at_the_shore() {
        // One grid of 2 m cells: 3 m deep at x = 0, shallowing to the shore at x = 8.
        let mut world = WorldFile { half_extent: 400, ..Default::default() };
        let ground: Vec<i32> = (0..25).map(|k| [-300, -300, -150, -20, 10][k % 5]).collect();
        world.ground = shared::terrain::Ground { origin: 0, cell: 200, side: 5, heights: ground };
        world.water = vec![0; 25];
        world.reach = vec![u8::MAX; 25];
        let waters = Waters::of_world(&world);
        let time = 7.3;
        let deep = waters.surface(0.0, 4.0, time).unwrap();
        assert_eq!(deep.height, shared::waves::wave(0.0, 4.0, time, shared::waves::OPEN_SEA, 1.0).height, "deep water has the whole wave");
        let shallow = waters.surface(6.0, 4.0, time).unwrap();
        assert_eq!((shallow.height, shallow.rise), (0.0, 0.0), "20 cm of water lies flat");
    }

    #[test]
    fn waves_are_smaller_on_small_water() {
        // The same deep water, once as the sea and once as a pond 20 m across.
        let mut world = WorldFile { half_extent: 400, ..Default::default() };
        world.ground = shared::terrain::Ground { origin: 0, cell: 200, side: 5, heights: vec![-400; 25] };
        world.water = vec![0; 25];
        let crest = |world: &WorldFile| {
            let waters = Waters::of_world(world);
            (0..shared::waves::LOOP_TICKS).step_by(5).map(|tick| waters.surface(4.0, 4.0, shared::waves::time(tick)).unwrap().height).fold(0.0, f32::max)
        };
        world.reach = vec![u8::MAX; 25];
        let sea = crest(&world);
        world.reach = vec![(20.0 / REACH_STEP) as u8; 25];
        let pond = crest(&world);
        assert!(sea > 1.0, "the sea heaves {sea} m");
        assert!(pond < 0.05, "a pond ripples, not {pond} m");
    }

    #[test]
    fn a_falling_rivers_level_is_read_between_its_samples_and_slopes_as_it_falls() {
        // One grid of 2 m cells, 2 m deep: the surface falls 10 cm a sample towards +x.
        let mut world = WorldFile { half_extent: 400, ..Default::default() };
        world.ground = shared::terrain::Ground { origin: 0, cell: 200, side: 5, heights: (0..25).map(|k| -200 - 10 * (k % 5)).collect() };
        world.water = (0..25).map(|k| -10 * (k % 5)).collect();
        let mut waters = Waters::of_world(&world);
        waters.calm();
        assert_eq!(waters.level_at(2.0, 4.0), Some(-0.1), "on a sample");
        let between = waters.level_at(3.0, 4.0).unwrap();
        assert!((between + 0.15).abs() < 1e-5, "half way to the next: {between}");
        let surface = waters.surface(3.0, 4.0, 0.0).unwrap();
        assert!((surface.slope[0] + 0.05).abs() < 1e-5 && surface.slope[1].abs() < 1e-5, "it falls 5 cm a metre along x: {:?}", surface.slope);
    }

    #[test]
    fn beside_a_dry_sample_the_level_is_the_wet_ones_and_lies_flat() {
        // Tile (1, 0) again: the samples at x = 2 and 4, z = -2 are wet, at different heights.
        let mut waters = Waters::new(400, 400, 200, false);
        waters.set((1, 0), &[0; 9], &[NO_WATER, NO_WATER, NO_WATER, NO_WATER, 150, 170, NO_WATER, NO_WATER, NO_WATER], &[], &[]);
        waters.calm();
        let surface = waters.surface(2.5, -1.5, 0.0).unwrap();
        assert!((surface.height - 1.55).abs() < 1e-5, "a quarter of the way from 1.5 to 1.7, whatever the dry ones are: {}", surface.height);
        assert_eq!(surface.slope, [0.0; 2], "the shore's cell has no slope");
    }

    #[test]
    fn frozen_water_is_not_kept() {
        let mut waters = Waters::new(400, 400, 200, true);
        waters.set((1, 0), &[0; 9], &[150; 9], &[], &[]);
        assert_eq!(waters.level_at(2.0, -2.0), None);
    }
}
