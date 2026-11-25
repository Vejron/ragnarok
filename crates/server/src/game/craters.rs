//! Craters (CRATERS.md): where a blast bursts on the ground, the ground gives. The world's `Ground` is
//! dug, the physics gets the new heightfield, and the crater is told to every client (`Cratered`), who
//! dig the same bowl in the tiles they hold. A joiner is sent the dug ground itself (`Dug`), and the
//! craters of the last `CRATER_FADE_SECONDS` only for their marks ("Lasting ground"). Dug ground heals
//! back to the imported ground `CRATER_HEAL_SECONDS` after its last crater, once nobody is near enough
//! to see it and nothing stands on it to be buried (`Healed`, "Healing").

use super::*;
use shared::protocol::{CraterEntry, DugTile, HealedTile};
use shared::world::NO_WATER;

/// How a burst struck where it dug: what its crater's marks are painted from (`CraterEntry`'s `yaw`,
/// `dip`, `contact` and `solid`). The hole it digs is the same however it struck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    /// The way the shell was going. Straight up or down leaves a round mark with no heading.
    pub dir: [f32; 3],
    /// Whether it struck a vehicle and burst on that, not on the ground.
    pub direct: bool,
    /// Whether it was a solid shot (`Burst::solid`).
    pub solid: bool,
}

impl Strike {
    /// A charge that went off where it lay or fell straight down: a round, burnt mark.
    pub const DOWN: Strike = Strike { dir: [0.0, -1.0, 0.0], direct: false, solid: false };

    /// A shell going along `dir`, as the wire says it (`CraterEntry`, `ShotEvent`): the `yaw` of its
    /// heading over the ground (`look`'s convention: 0 faces -Z) and its `dip`, the share of its way
    /// that did not lie along the ground: 0 skimming it, 255 straight up or down, which has no heading.
    pub fn heading(dir: [f32; 3]) -> (i16, u8) {
        let [dx, dy, dz] = dir;
        let along = dx.hypot(dz) / (dx.hypot(dz).hypot(dy)).max(1e-6);
        let yaw = if along > 1e-3 { to_mrad((-dx).atan2(-dz)) } else { 0 };
        (yaw, to_byte(1.0 - along))
    }
}

/// The tile `(col, row)` that the point `(x, z)` (centimetres) is in, on a ground starting at `origin`
/// (centimetres) cut into tiles `tile_size` (centimetres) a side; the client's is `tileAt`.
fn tile_of(x: i32, z: i32, origin: i32, tile_size: i32) -> (u16, u16) {
    let at = |v: i32| (v - origin).div_euclid(tile_size.max(1)).clamp(0, i32::from(u16::MAX)) as u16;
    (at(x), at(z))
}

/// A share 0 to 1 as a wire byte (`u8 as byte`).
fn to_byte(share: f32) -> u8 {
    (share.clamp(0.0, 1.0) * 255.0).round() as u8
}

impl World {
    /// Digs a crater `radius` metres across and `depth` metres deep where a shell burst at `point`, if it
    /// burst on the terrain: within `CRATER_REACH` of the ground, and not over water. Whatever was
    /// standing on the ground there is told it moved. `strike` is how it arrived, for the crater's
    /// marks only. Returns whether the ground changed.
    pub fn dig_crater(&mut self, point: [f32; 3], radius: f32, depth: f32, strike: Strike) -> bool {
        let ground = &self.map.ground;
        let above = point[1] - ground.height_at(point[0], point[2]);
        if ground.is_flat() || radius <= 0.0 || depth <= 0.0 || above > CRATER_REACH {
            return false;
        }
        if self.over_water(point[0], point[2]) {
            return false;
        }
        let (yaw, dip) = Strike::heading(strike.dir);
        let contact = (1.0 - above / CRATER_REACH) * if strike.direct { CRATER_DIRECT_CONTACT } else { 1.0 };
        let crater = CraterEntry {
            x: to_cm(point[0]),
            z: to_cm(point[2]),
            radius: to_cm(radius),
            depth: to_cm(depth),
            yaw,
            dip,
            contact: to_byte(contact),
            solid: strike.solid,
            age: 0,
        };
        if !self.apply_crater(crater) {
            return false;
        }
        self.craters.push_back((self.tick, crater));
        if self.craters.len() > CRATER_MAX_KEPT {
            self.craters.pop_front();
        }
        self.fresh.craters.push(crater);
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
        for &(col, row) in self.dug_tiles.keys() {
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

    /// Notes the tiles that samples `(i0, j0)` to `(i1, j1)` of the ground fall in, both on a seam, as
    /// dug now.
    fn note_dug(&mut self, (i0, j0, i1, j1): (u32, u32, u32, u32)) {
        let Some(per_tile) = self.samples_per_tile() else { return };
        let last = (self.map.ground.side - 1) / per_tile - 1;
        let tiles = |a: u32, b: u32| (a.div_ceil(per_tile).saturating_sub(1))..=(b / per_tile).min(last);
        for row in tiles(j0, j1) {
            for col in tiles(i0, i1) {
                self.dug_tiles.insert((col as u16, row as u16), self.tick);
            }
        }
    }

    /// Heals the dug ground that is ready to (CRATERS.md, "Healing"): back to the pristine ground, in
    /// patches of dug tiles joined by a dug sample on the edge or corner they share, so that a crater
    /// over a seam heals whole. A patch heals once every tile of it was last dug `CRATER_HEAL_SECONDS`
    /// ago, no eye in `eyes` (`(x, z)`, metres: the players) is within `HEAL_CLEAR` of it, and
    /// nothing on it (`things`, `(x, z)`, metres, and the placed cubes) is within `CRATER_HEAL_MARGIN`
    /// of ground that is dug, which it would be buried in.
    /// The craters centred on healed tiles are forgotten. Returns the tiles healed.
    pub fn heal_craters(&mut self, eyes: &[[f32; 2]], things: &[[f32; 2]]) -> Vec<HealedTile> {
        let Some(per_tile) = self.samples_per_tile() else { return Vec::new() };
        if self.dug_tiles.is_empty() || self.pristine.len() != self.map.ground.heights.len() {
            return Vec::new();
        }
        let wait = CRATER_HEAL_SECONDS * TICK_RATE;
        let mut cubes: Option<Vec<[f32; 2]>> = None;
        let mut seen = std::collections::BTreeSet::new();
        let mut healed = Vec::new();
        let tiles: Vec<(u16, u16)> = self.dug_tiles.keys().copied().collect();
        for start in tiles {
            if !seen.insert(start) {
                continue;
            }
            let patch = self.patch_of(start, per_tile);
            seen.extend(patch.iter().copied());
            if patch.iter().any(|tile| self.tick.wrapping_sub(self.dug_tiles[tile]) < wait) {
                continue;
            }
            let rects: Vec<[f32; 4]> = patch.iter().map(|&tile| self.tile_rect(tile, per_tile)).collect();
            let within = |at: &[f32; 2], reach: f32| rects.iter().any(|r| (r[0] - at[0]).max(at[0] - r[2]).max(0.0).hypot((r[1] - at[1]).max(at[1] - r[3]).max(0.0)) < reach);
            let buried = |at: &[f32; 2]| within(at, CRATER_HEAL_MARGIN) && self.on_dug_ground(*at);
            if eyes.iter().any(|eye| within(eye, HEAL_CLEAR)) || things.iter().any(buried) {
                continue;
            }
            let cubes = cubes.get_or_insert_with(|| {
                self.construct.cubes().map(|(pos, _)| shared::construct::centre(pos)).map(|[x, _, z]| [from_cm(x), from_cm(z)]).collect()
            });
            if cubes.iter().any(buried) {
                continue;
            }
            for tile in patch {
                self.heal_tile(tile, per_tile);
                healed.push(HealedTile { col: tile.0, row: tile.1 });
            }
        }
        if !healed.is_empty() {
            let gone: std::collections::BTreeSet<(u16, u16)> = healed.iter().map(|t| (t.col, t.row)).collect();
            let tile_size = self.map.tile_size;
            let origin = self.map.ground.origin;
            self.craters.retain(|(_, crater)| !gone.contains(&tile_of(crater.x, crater.z, origin, tile_size)));
        }
        healed
    }

    /// The dug tiles joined to `start` through dug samples on the edges and corners they share.
    fn patch_of(&self, start: (u16, u16), per_tile: u32) -> Vec<(u16, u16)> {
        let mut patch = vec![start];
        let mut next = 0;
        while next < patch.len() {
            let (col, row) = patch[next];
            next += 1;
            for (dc, dr) in [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (c, r) = (i32::from(col) + dc, i32::from(row) + dr);
                let (Ok(c), Ok(r)) = (u16::try_from(c), u16::try_from(r)) else { continue };
                let other = (c, r);
                if patch.contains(&other) || !self.dug_tiles.contains_key(&other) {
                    continue;
                }
                // The samples both tiles hold: an edge, or a corner.
                let span = |a: u16, b: u16| {
                    let (a0, b0) = (u32::from(a) * per_tile, u32::from(b) * per_tile);
                    a0.max(b0)..=(a0 + per_tile).min(b0 + per_tile)
                };
                let side = self.map.ground.side;
                let dug = span(row, other.1).any(|j| span(col, other.0).any(|i| {
                    let at = (j * side + i) as usize;
                    self.map.ground.heights[at] != self.pristine[at]
                }));
                if dug {
                    patch.push(other);
                }
            }
        }
        patch
    }

    /// Whether ground within `CRATER_HEAL_MARGIN` of `(x, z)`, metres, is dug: what stands there would
    /// be buried if it healed.
    pub(super) fn on_dug_ground(&self, [x, z]: [f32; 2]) -> bool {
        let ground = &self.map.ground;
        // No crater has landed yet (`pristine` is copied with the first), or there is no ground to dig.
        if ground.is_flat() || self.pristine.len() != ground.heights.len() {
            return false;
        }
        let last = i64::from(ground.side) - 1;
        let at = |v: f32, by: f32| (((to_cm(v + by) - ground.origin) as f32 / ground.cell as f32).floor() as i64).clamp(0, last);
        let (i0, i1, j0, j1) = (at(x, -CRATER_HEAL_MARGIN), at(x, CRATER_HEAL_MARGIN) + 1, at(z, -CRATER_HEAL_MARGIN), at(z, CRATER_HEAL_MARGIN) + 1);
        (j0..=j1.min(last)).any(|j| {
            (i0..=i1.min(last)).any(|i| {
                let k = (j * (last + 1) + i) as usize;
                ground.heights[k] != self.pristine[k]
            })
        })
    }

    /// Tile `(col, row)`'s ground as `[x0, z0, x1, z1]`, metres.
    fn tile_rect(&self, (col, row): (u16, u16), per_tile: u32) -> [f32; 4] {
        let ground = &self.map.ground;
        let at = |i: u32| from_cm(ground.origin + (i * per_tile) as i32 * ground.cell);
        [at(u32::from(col)), at(u32::from(row)), at(u32::from(col) + 1), at(u32::from(row) + 1)]
    }

    /// Lays the pristine ground back on tile `(col, row)`, its edges too, and gives the physics it.
    fn heal_tile(&mut self, (col, row): (u16, u16), per_tile: u32) {
        let ground = &mut self.map.ground;
        let (i0, j0) = (u32::from(col) * per_tile, u32::from(row) * per_tile);
        for j in j0..=j0 + per_tile {
            let from = (j * ground.side + i0) as usize;
            let to = from + per_tile as usize + 1;
            ground.heights[from..to].copy_from_slice(&self.pristine[from..to]);
        }
        self.physics.dig_ground(&self.map.ground, (i0, j0, i0 + per_tile, j0 + per_tile));
        self.dug_tiles.remove(&(col, row));
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
