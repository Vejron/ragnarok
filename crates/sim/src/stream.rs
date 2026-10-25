//! The static scene built a tile at a time, as the client streams it, and the vehicles of a snapshot as
//! solids to walk into. The server builds the whole world at once (`Physics::new`); the client's
//! prediction builds the tiles around the player with the same collider code, so a building, a
//! trunk or a wall is the same shape on both sides.

use super::*;
use crate::vehicle::{Role, extra_solids, heavy, spec, with_role};
use shared::world::{Manifest, Tile};

/// A vehicle as a snapshot shows it: a fixed body with the hull and the extra solids its kind has,
/// where the stickman bumps into it, and how fast it was going, for a stickman leaving its roof.
pub(crate) struct Obstacle {
    kind: u8,
    body: RigidBodyHandle,
    extra: Vec<ColliderHandle>,
    velocity: [f32; 3],
}

/// A streamed tile: its colliders, and the tile alone as a world (what collides in it, under the
/// server's ids), from which its buildings are cut into cells when blasts bite them.
pub(crate) struct StreamedTile {
    handles: Vec<ColliderHandle>,
    /// The tile's heightfield, one of `handles`; `None` when it has no ground.
    ground: Option<ColliderHandle>,
    world: WorldFile,
    /// Buildings already cut into cells, by their number in `world`, with their floors: unpacking a
    /// big one is most of the work of a hit, so it is done once. `None` for one that cannot be cut.
    shells: std::collections::HashMap<usize, Option<(shared::carve::Shell, Vec<shared::carve::Tri>)>>,
}

/// The manifest's world with nothing in it and no terrain: enough for the slab, the walls and one
/// tile's buildings and trees (`WorldFile::assemble` would otherwise lay out the whole world's ground).
fn bare(manifest: &Manifest) -> WorldFile {
    WorldFile::assemble(&Manifest { ground_cell: 0, spawns: Vec::new(), ..manifest.clone() }, [])
}

/// Tile `(col, row)`'s ground as a heightfield of its own, laid out as `static_colliders` lays out the
/// whole world's: centred on its square, scaled to its width, heights column-major with rows along Z.
/// `None` when the world has no terrain or the tile carries none.
fn tile_ground(manifest: &Manifest, col: u16, row: u16, ground: &[i32]) -> Option<Collider> {
    let n = manifest.tile_samples() as usize;
    if n < 2 || ground.len() != n * n {
        return None;
    }
    let mut heights = Vec::with_capacity(n * n);
    for column in 0..n {
        for row in 0..n {
            heights.push(ground[row * n + column] as f32 / 100.0);
        }
    }
    let width = (n - 1) as f32 * from_cm(manifest.ground_cell);
    let x0 = from_cm(-manifest.half_extent) + f32::from(col) * from_cm(manifest.tile_size);
    let z0 = from_cm(-manifest.half_extent) + f32::from(row) * from_cm(manifest.tile_size);
    Some(ColliderBuilder::heightfield(Array2::new(n, n, heights), Vector::new(width, 1.0, width)).translation(Vector::new(x0 + width / 2.0, 0.0, z0 + width / 2.0)).build())
}

impl Physics {
    /// A world of `manifest` whose tiles arrive one by one (`add_tile`): for now only the slab under
    /// everything and the walls round the square.
    pub fn streamed(manifest: &Manifest) -> Self {
        let world = bare(manifest);
        let mut physics = Self::empty(from_cm(manifest.half_extent));
        physics.insert_static(slab(&world, manifest.ground_cell > 0), None);
        for wall in walls(&world) {
            physics.insert_static(wall, None);
        }
        physics.step_pipeline();
        physics
    }

    /// Adds tile `(col, row)` of `manifest`: its ground, buildings, bridges and trunks. A tile already
    /// there is replaced.
    pub fn add_tile(&mut self, manifest: &Manifest, col: u16, row: u16, tile: &Tile) -> Result<()> {
        self.remove_tile(col, row);
        let mut handles = Vec::new();
        let ground = tile_ground(manifest, col, row, &tile.ground).map(|ground| self.insert_static(ground, None));
        handles.extend(ground);
        // The tile alone, as the server's merged world holds it: what collides, under the same ids.
        let solid = Tile { buildings: tile.buildings.clone(), collision: tile.collision.clone(), trees: tile.trees.clone(), bridges: tile.bridges.clone(), ..Default::default() };
        let alone = Manifest { ground_cell: 0, tiles: Vec::new(), spawns: Vec::new(), ..manifest.clone() };
        let world = WorldFile::assemble(&alone, [(col, row, solid)]);
        for (collider, object) in object_colliders(&world)? {
            handles.push(self.insert_static(collider, object));
        }
        self.tiles.insert((col, row), StreamedTile { handles, ground, world, shells: Default::default() });
        self.refresh();
        Ok(())
    }

    /// Takes tile `(col, row)` out, with whatever of it is still standing.
    pub fn remove_tile(&mut self, col: u16, row: u16) {
        let Some(StreamedTile { handles, .. }) = self.tiles.remove(&(col, row)) else { return };
        for handle in handles {
            if let Some(object) = self.objects.remove(&handle) {
                self.handles.remove(&object);
            }
            self.colliders.remove(handle, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
        }
        self.removals.retain(|object| self.handles.contains_key(object));
        self.replacements.retain(|(object, _)| self.handles.contains_key(object));
        self.refresh();
    }

    /// Gives tile `(col, row)` the ground `heights` (centimetres, row-major, `tile_samples` a side): a crater
    /// dented it. Takes effect at once. Does nothing for a tile that is not there or has no ground.
    pub fn set_tile_ground(&mut self, manifest: &Manifest, col: u16, row: u16, heights: &[i32]) {
        let Some(tile) = self.tiles.get_mut(&(col, row)) else { return };
        let Some(new) = tile_ground(manifest, col, row, heights) else { return };
        let Some(old) = tile.ground else { return };
        tile.handles.retain(|&handle| handle != old);
        self.colliders.remove(old, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
        let handle = self.colliders.insert(new);
        let tile = self.tiles.get_mut(&(col, row)).expect("still there");
        tile.handles.push(handle);
        tile.ground = Some(handle);
        self.refresh();
    }

    /// Gives building `index` of streamed tile `(col, row)` the shape it has with the cells `removed`
    /// taken out (the cells a `Carved` message lists, blasted and fallen alike), as `World::carve` does
    /// on the server. Takes effect at the next `refresh`. Does nothing for a building that is not
    /// standing or cannot be cut into cells.
    pub fn carve_building(&mut self, col: u16, row: u16, index: u32, removed: &[u32]) {
        let Some(tile) = self.tiles.get_mut(&(col, row)) else { return };
        let Some(at) = tile.world.building_ids.iter().position(|id| id.index == index) else { return };
        let world = &tile.world;
        let Some((shell, floor)) = tile.shells.entry(at).or_insert_with(|| building_shell(world, at)).as_ref() else { return };
        let mut gone = shell.untouched();
        for &cell in removed {
            if let Some(flag) = gone.get_mut(cell as usize) {
                *flag = true;
            }
        }
        let shape = solid_of(&shell.boxes(&gone), floor);
        self.replace_building(Obj::Building(shared::world::ObjId { col, row, index }), shape);
    }

    /// Carries out queued destructions and reshapes (`destroy`, `replace_building`) and brings the
    /// broad-phase up to date, for a world whose vehicles are not stepped (the client's).
    ///
    /// While a vehicle is simulated here (the client's own, predicted) the pipeline is not stepped,
    /// which would move that vehicle on: its next `step_cars` brings the queries up to date instead.
    pub fn refresh(&mut self) {
        self.apply_removals();
        if self.cars.is_empty() {
            self.step_pipeline();
        }
    }

    /// Puts vehicle `id` of `kind` where a snapshot shows it, as a solid the stickman walks into,
    /// moving at `velocity` m/s. `turret` is whether a tank still has its turret.
    /// A vehicle simulated here (`add_car`) has no stand-in: it is solid as it is.
    pub fn set_obstacle(&mut self, id: u32, kind: u8, position: [f32; 3], rotation: [f32; 4], velocity: [f32; 3], turret: bool) {
        if self.cars.contains_key(&id) {
            return;
        }
        let pose = Pose::from_parts(Vector::from(position), Rotation::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]).normalize());
        if self.obstacles.get(&id).is_some_and(|obstacle| obstacle.kind != kind) {
            self.remove_obstacle(id);
        }
        if !self.obstacles.contains_key(&id) {
            let body = self.bodies.insert(RigidBodyBuilder::fixed().pose(pose).build());
            let [hx, hy, hz] = half_extents(kind);
            let mut hull = ColliderBuilder::cuboid(hx, hy, hz);
            if heavy(kind) {
                hull = hull.collision_groups(plows_trees());
            }
            self.colliders.insert_with_parent(hull.build(), body, &mut self.bodies);
            let extra = extra_solids(kind).into_iter().map(|solid| self.colliders.insert_with_parent(solid.build(), body, &mut self.bodies)).collect();
            self.obstacles.insert(id, Obstacle { kind, body, extra, velocity });
        }
        let obstacle = self.obstacles.get_mut(&id).expect("just made");
        obstacle.velocity = velocity;
        if let Some(body) = self.bodies.get_mut(obstacle.body) {
            body.set_position(pose, false);
        }
        // Only a turret can be shot off.
        for handle in with_role(spec(kind), &obstacle.extra, Role::Turret) {
            if let Some(collider) = self.colliders.get_mut(handle) {
                collider.set_enabled(turret);
            }
        }
    }

    /// Takes vehicle `id`'s stand-in away.
    pub fn remove_obstacle(&mut self, id: u32) {
        if let Some(obstacle) = self.obstacles.remove(&id) {
            self.bodies.remove(obstacle.body, &mut self.islands, &mut self.colliders, &mut self.impulse_joints, &mut self.multibody_joints, &mut self.soft_bodies, true);
        }
    }

    /// The vehicles with a stand-in, in id order.
    pub fn obstacle_ids(&self) -> Vec<u32> {
        self.obstacles.keys().copied().collect()
    }

    /// How fast vehicle `id` was going, m/s, as its stand-in was last placed.
    pub fn obstacle_velocity(&self, id: u32) -> Option<[f32; 3]> {
        self.obstacles.get(&id).map(|obstacle| obstacle.velocity)
    }
}

#[cfg(test)]
mod tests;
