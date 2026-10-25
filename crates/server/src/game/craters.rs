//! Craters (CRATERS.md): where a tank shell, a mortar round or a bomb bursts on the ground, the ground
//! gives. The world's `Ground` is dug, the physics gets the new heightfield, and the crater is told to
//! every client (`Cratered`), who dig the same bowl in the tiles they hold.

use super::*;
use shared::protocol::CraterEntry;
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
        let crater = CraterEntry { x: to_cm(point[0]), z: to_cm(point[2]), radius: to_cm(radius), depth: to_cm(depth) };
        if !self.apply_crater(crater) {
            return false;
        }
        self.craters.push(crater);
        if self.craters.len() > CRATER_MAX_KEPT {
            self.craters.remove(0);
        }
        self.fresh_craters.push(crater);
        true
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
