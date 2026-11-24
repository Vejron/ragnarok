//! The static scene built a tile at a time, as the client streams it, and the vehicles of a snapshot as
//! solids to walk into. The server builds the whole world at once (`Physics::new`); the client's
//! prediction builds the tiles around the player with the same collider code, so a building, a
//! trunk or a wall is the same shape on both sides.

use super::*;
use crate::vehicle::{Lift, Role, extra_solids, heavy, hull, spec, with_role};
use shared::world::{Manifest, Tile};

/// A vehicle as a snapshot shows it: a fixed body with the hull and the extra solids its kind has,
/// where the soldier bumps into it, and how fast it was going, for a soldier leaving its roof.
pub(crate) struct Obstacle {
    kind: u8,
    body: RigidBodyHandle,
    extra: Vec<ColliderHandle>,
    velocity: [f32; 3],
}

/// A hole the predicted vehicle broke in a building by ramming it, as a `Carved` message would tell
/// of it: for the client to draw at once, ahead of the server's word.
#[derive(Clone, Debug, PartialEq)]
pub struct Ram {
    pub building: shared::world::ObjId,
    /// Where the vehicle hit the wall, metres.
    pub point: [f32; 3],
    /// The cells broken out, and the cells left with nothing to hold them (`shared::carve::Cut`).
    pub blasted: Vec<u32>,
    pub fell: Vec<u32>,
}

/// A streamed tile: its colliders, and the tile alone as a world (what collides in it, under the
/// server's ids), from which its buildings are cut into cells when blasts bite them.
///
/// A tile's buildings are made solid a few at a time after it arrives, nearest the player first
/// (`build_pending`): working out a city tile's every look and collider at once held a frame up for
/// tens of milliseconds. Until a building is built its look is not worked out and it has no
/// collider; what is done to it meanwhile (cells carved out, gates opened) is kept and built in.
pub(crate) struct StreamedTile {
    /// Its colliders but its buildings': the ground, the trunks, the fences and walls, the tombstones.
    handles: Vec<ColliderHandle>,
    /// The tile's heightfield, one of `handles`; `None` when it has no ground.
    ground: Option<ColliderHandle>,
    world: WorldFile,
    /// The tile's look, from which each building's (`world.looks`) is worked out when it is built.
    look: shared::world::Look,
    /// Each building's middle, metres `[x, z]`, by its number in `world`: what the queue is ordered by.
    centres: Vec<[f32; 2]>,
    /// The buildings not built yet, by their number in `world`.
    pending: std::collections::BTreeSet<usize>,
    /// The gates of a building not built yet that were opened or shut meanwhile (`set_gates`), by its number in `world`.
    shut: std::collections::HashMap<usize, Vec<u32>>,
    /// Buildings already cut into cells, by their number in `world`, with their floors: unpacking a
    /// big one is most of the work of a hit, so it is done once. `None` for one that cannot be cut.
    shells: std::collections::HashMap<usize, Option<(shared::carve::Shell, Vec<shared::carve::Tri>)>>,
    /// The faces as they stand of each building carved here, kept between the server's cuts
    /// (`building_solid_kept`), by its number in `world`.
    kept: std::collections::HashMap<usize, Option<shared::facade::Standing>>,
    /// The cells gone from each carved building, by its number in `world`, as `carve_building` was
    /// last told: a gate that is opened or shut reshapes what is left of its building.
    carved: std::collections::HashMap<usize, Vec<u32>>,
    /// The cells the predicted vehicle broke out of each building ramming it (`ram_buildings`), by
    /// the building's number in `world`: gone here ahead of the server's word, until `carve_building`
    /// brings it or `forget_rams` gives up waiting.
    rammed: std::collections::HashMap<usize, Vec<u32>>,
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

/// Building `at` of `tile`'s shape with the cells the server says are gone (`StreamedTile::carved`)
/// taken out, its cells unpacked once and kept: `Some(None)` when nothing of it stands, `None` when
/// it cannot be cut into cells (or was never carved), and keeps the shape it has.
fn carved_shape(tile: &mut StreamedTile, at: usize) -> Option<Option<SharedShape>> {
    let removed = tile.carved.get(&at)?;
    let world = &tile.world;
    let (shell, floor) = tile.shells.entry(at).or_insert_with(|| building_shell(world, at)).as_ref()?;
    let mut gone = shell.untouched();
    for &cell in removed {
        if let Some(flag) = gone.get_mut(cell as usize) {
            *flag = true;
        }
    }
    Some(building_solid_kept(world, at, (shell, &gone), floor, tile.kept.entry(at).or_default()))
}

/// Works out building `at` of `tile`'s look (`BuildingLook::of`) from the tile's, if it is not yet,
/// with the gates opened or shut while it waited.
fn work_out_look(tile: &mut StreamedTile, at: usize) {
    let range = tile.look.buildings.get(at).copied().unwrap_or([0; 3]);
    let (Some(look), Some(building)) = (tile.world.looks.get_mut(at), tile.world.buildings.get(at)) else { return };
    if !look.is_empty() || range[1] + range[2] == 0 {
        return;
    }
    *look = shared::world::BuildingLook::of(&tile.look, range, building);
    if let Some(shut) = tile.shut.remove(&at) {
        look.shut = shut;
    }
}

impl Physics {
    /// A world of `manifest` whose tiles arrive one by one (`add_tile`): for now only the slab under
    /// everything and the walls round the square.
    pub fn streamed(manifest: &Manifest) -> Self {
        let world = bare(manifest);
        let mut physics = Self::empty(from_cm(manifest.half_extent));
        physics.stones = crate::cells::TileCells::new(manifest.half_extent, manifest.tile_size);
        physics.waters = crate::waters::Waters::new(manifest.half_extent, manifest.tile_size, manifest.ground_cell, manifest.frozen());
        physics.insert_static(slab(&world, manifest.ground_cell > 0), None);
        for wall in walls(&world) {
            physics.insert_static(wall, None);
        }
        // The lifts are whole in the manifest, not cut into tiles: they stand from the start.
        for lift in crate::lift_colliders(&manifest.lifts) {
            physics.insert_static(lift, None);
        }
        physics.step_pipeline();
        physics
    }

    /// Adds tile `(col, row)` of `manifest`: its ground, bridges, trunks, fences and tombstones at once,
    /// and its buildings to the queue `build_pending` makes them solid from. A tile already there is replaced.
    pub fn add_tile(&mut self, manifest: &Manifest, col: u16, row: u16, mut tile: Tile) -> Result<()> {
        self.remove_tile(col, row);
        self.add_ski_trails(&tile.ski);
        let mut handles = Vec::new();
        let ground = tile_ground(manifest, col, row, &tile.ground).map(|ground| self.insert_static(ground, None));
        handles.extend(ground);
        self.stones.set((col, row), std::mem::take(&mut tile.stones));
        self.waters.set((col, row), &tile.ground, &tile.water, &tile.reach, &tile.flow);
        // The look is kept aside, so assembling works out no building's look: each is worked out as it is built.
        let look = std::mem::take(&mut tile.look);
        // The tile alone, as the server's merged world holds it: what collides, under the same ids.
        let solid = Tile {
            buildings: std::mem::take(&mut tile.buildings),
            collision: std::mem::take(&mut tile.collision),
            trees: std::mem::take(&mut tile.trees),
            bridges: std::mem::take(&mut tile.bridges),
            lines: std::mem::take(&mut tile.lines),
            graves: std::mem::take(&mut tile.graves),
            ..Default::default()
        };
        let alone = Manifest { ground_cell: 0, tiles: Vec::new(), spawns: Vec::new(), ..manifest.clone() };
        let world = WorldFile::assemble(&alone, [(col, row, solid)]);
        for (collider, object) in static_object_colliders(&world)? {
            handles.push(self.insert_static(collider, object));
        }
        let centres = world
            .buildings
            .iter()
            .map(|building| {
                let n = building.footprint.len().max(1) as f32;
                let (x, z) = building.footprint.iter().fold((0.0, 0.0), |(x, z), p| (x + from_cm(p[0]) / n, z + from_cm(p[1]) / n));
                [x, z]
            })
            .collect();
        let pending = (0..world.building_ids.len().min(world.building_triangles.len())).collect();
        self.tiles.insert((col, row), StreamedTile { handles, ground, world, look, centres, pending, shut: Default::default(), shells: Default::default(), kept: Default::default(), carved: Default::default(), rammed: Default::default() });
        self.refresh();
        Ok(())
    }

    /// Takes tile `(col, row)` out, with whatever of it is still standing.
    pub fn remove_tile(&mut self, col: u16, row: u16) {
        self.stones.remove((col, row));
        self.waters.remove((col, row));
        let Some(StreamedTile { handles, world, .. }) = self.tiles.remove(&(col, row)) else { return };
        // A building's collider is found by what it is, not by the handle it was built with: a cut
        // gives it a new one (`apply_removals`).
        let buildings: Vec<ColliderHandle> = world.building_ids.iter().filter_map(|&id| self.handles.get(&Obj::Building(id)).copied()).collect();
        for handle in handles.into_iter().chain(buildings) {
            if let Some(object) = self.objects.remove(&handle) {
                self.handles.remove(&object);
            }
            self.colliders.remove(handle, &mut self.islands, &mut self.bodies, &mut self.soft_bodies, false);
        }
        self.removals.retain(|object| self.handles.contains_key(object));
        self.replacements.retain(|(object, _)| self.handles.contains_key(object));
        self.refresh();
    }

    /// Makes solid the buildings waiting for it (`add_tile`) within `near` metres of `(x, z)`, all of
    /// them, and then up to `most` more, nearest first. Returns how many it made. They collide from
    /// the next `refresh`.
    pub fn build_pending(&mut self, x: f32, z: f32, near: f32, most: usize) -> usize {
        let mut waiting: Vec<(f32, (u16, u16), usize)> = Vec::new();
        for (&key, tile) in &self.tiles {
            for &at in &tile.pending {
                let [cx, cz] = tile.centres[at];
                waiting.push(((cx - x).hypot(cz - z), key, at));
            }
        }
        waiting.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut built = 0;
        for (distance, key, at) in waiting {
            if distance > near && built >= most {
                break;
            }
            self.build_building(key, at);
            built += 1;
        }
        built
    }

    /// How many buildings of the streamed tiles are waiting to be made solid.
    pub fn buildings_pending(&self) -> usize {
        self.tiles.values().map(|tile| tile.pending.len()).sum()
    }

    /// Makes building `at` of streamed tile `key` solid, as what was done to it meanwhile left it.
    fn build_building(&mut self, key: (u16, u16), at: usize) {
        let Some(tile) = self.tiles.get_mut(&key) else { return };
        if !tile.pending.remove(&at) {
            return;
        }
        work_out_look(tile, at);
        let id = tile.world.building_ids[at];
        let collider = match carved_shape(tile, at) {
            Some(shape) => shape.map(|shape| ColliderBuilder::new(shape).build()),
            None => building_collider(&tile.world, at).ok().flatten(),
        };
        if let Some(collider) = collider {
            self.insert_static(collider, Some(Obj::Building(id)));
        }
    }

    /// Whether building `id` is one of a streamed tile's still waiting to be built, which then never
    /// is: it is gone (`destroy`).
    pub(crate) fn forget_unbuilt(&mut self, id: shared::world::ObjId) -> bool {
        let Some(tile) = self.tiles.get_mut(&(id.col, id.row)) else { return false };
        let Some(at) = tile.world.building_ids.iter().position(|known| known.index == id.index) else { return false };
        tile.pending.remove(&at)
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
        tile.carved.insert(at, removed.to_vec());
        // The server's word on it replaces what a ram was predicted to break.
        tile.rammed.remove(&at);
        // One not built yet is built with these cells gone when its turn comes.
        if tile.pending.contains(&at) {
            return;
        }
        let Some(shape) = carved_shape(tile, at) else { return };
        self.replace_building(Obj::Building(shared::world::ObjId { col, row, index }), shape);
    }

    /// Breaks the walls vehicle `car` of `kind` rammed in the last step, as the server will when it
    /// steps the same input (`sim::breach`, `ram_building` there): each building it lost enough speed
    /// to gets a hole where it was hit, from the next step, and through a hole the vehicle goes on
    /// with part of the velocity `before` it had coming in. Returns whether any wall broke. The holes
    /// are a prediction: the server's `Carved` message replaces each (`carve_building`), and
    /// `forget_rams` closes those it never confirms.
    pub fn ram_buildings(&mut self, car: u32, kind: u8, before: [f32; 3]) -> bool {
        let mut kept = None;
        for impact in self.car_impacts() {
            let (true, Some(Obj::Building(id))) = (impact.car == car, impact.object) else { continue };
            let Some(breach) = crate::breach(kind, impact.dv) else { continue };
            let Some(tile) = self.tiles.get_mut(&(id.col, id.row)) else { continue };
            let Some(at) = tile.world.building_ids.iter().position(|known| known.index == id.index) else { continue };
            // Nothing that drives breaks a bridge.
            if tile.world.buildings.get(at).is_none_or(|building| building.kind == shared::world::BUILDING_BRIDGE) {
                continue;
            }
            let world = &tile.world;
            let Some((shell, floor)) = tile.shells.entry(at).or_insert_with(|| building_shell(world, at)).as_ref() else { continue };
            let mut gone = shell.untouched();
            for &cell in tile.carved.get(&at).into_iter().chain(tile.rammed.get(&at)).flatten() {
                if let Some(flag) = gone.get_mut(cell as usize) {
                    *flag = true;
                }
            }
            let cut = shell.blast(&mut gone, impact.point.map(shared::quant::to_cm), shared::quant::to_cm(breach.radius));
            if cut.is_empty() {
                continue;
            }
            tile.rammed.entry(at).or_default().extend(cut.blasted.iter().chain(&cut.fell));
            let shape = building_solid(world, at, Some((shell, &gone)), floor);
            self.replace_building(Obj::Building(id), shape);
            self.rams.push(Ram { building: id, point: impact.point, blasted: cut.blasted, fell: cut.fell });
            kept = kept.or(Some(breach.keep));
        }
        if let (Some(Some(keep)), Some(now)) = (kept, self.car_pose(car)) {
            self.set_car_velocity(car, crate::breach_velocity(before, now.velocity, keep));
        }
        kept.is_some()
    }

    /// The holes `ram_buildings` has broken since this was last asked, oldest first.
    pub fn take_rams(&mut self) -> Vec<Ram> {
        std::mem::take(&mut self.rams)
    }

    /// Closes the holes `ram_buildings` predicted that the server has not confirmed: each such
    /// building is as the server last said (`carve_building`), or whole. Takes effect at the next
    /// `refresh`.
    pub fn forget_rams(&mut self) {
        let waiting: Vec<(u16, u16, usize)> = self.tiles.iter().flat_map(|(&(col, row), tile)| tile.rammed.keys().map(move |&at| (col, row, at))).collect();
        for (col, row, at) in waiting {
            let Some(tile) = self.tiles.get_mut(&(col, row)) else { continue };
            tile.rammed.remove(&at);
            let Some(index) = tile.world.building_ids.get(at).map(|id| id.index) else { continue };
            match tile.carved.get(&at).cloned() {
                Some(removed) => self.carve_building(col, row, index, &removed),
                None => {
                    let shape = building_shape(&tile.world, at);
                    self.replace_building(Obj::Building(shared::world::ObjId { col, row, index }), shape);
                }
            }
        }
    }

    /// Shuts the gates `shut` of building `index` of streamed tile `(col, row)` (their numbers among
    /// its openings, `BuildingLook::shut`) and opens its others, as the server has them: a shut gate
    /// is wall. Takes effect at the next `refresh`. Does nothing for a building that is not there.
    pub fn set_gates(&mut self, col: u16, row: u16, index: u32, shut: &[u32]) {
        let Some(tile) = self.tiles.get_mut(&(col, row)) else { return };
        let Some(at) = tile.world.building_ids.iter().position(|id| id.index == index) else { return };
        // One not built yet has its look worked out, and these gates shut, when its turn comes.
        if tile.pending.contains(&at) {
            tile.shut.insert(at, shut.to_vec());
            return;
        }
        let Some(look) = tile.world.looks.get_mut(at) else { return };
        if look.shut == shut {
            return;
        }
        look.shut = shut.to_vec();
        match tile.carved.get(&at).cloned() {
            Some(removed) => self.carve_building(col, row, index, &removed),
            None => {
                let shape = building_shape(&tile.world, at);
                self.replace_building(Obj::Building(shared::world::ObjId { col, row, index }), shape);
            }
        }
    }

    /// Carries out queued destructions and reshapes (`destroy`, `replace_building`) and brings the
    /// broad-phase up to date, for a world whose vehicles are not stepped (the client's).
    ///
    /// While a vehicle is simulated here (the client's own, predicted) the pipeline is not stepped,
    /// which would move that vehicle on: its next `step_cars` brings the queries up to date instead.
    pub fn refresh(&mut self) {
        // Carrying out what was queued already brings the queries up to date.
        let stepped = self.apply_removals();
        if self.cars.is_empty() && !stepped {
            self.step_pipeline();
        }
    }

    /// Puts vehicle `id` of `kind` where a snapshot shows it, as a solid the soldier walks into,
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
            // The same hull as the vehicle the server simulates.
            let mut shape = hull(kind);
            if heavy(kind) {
                shape = shape.collision_groups(plows_trees());
            }
            // A ship's gun touches nothing, as on the server, so it never fights the ship this client
            // sails under it; it is still in the way of a soldier.
            let groups = (spec(kind).lift == Lift::Mounted).then(InteractionGroups::none);
            let fit = |collider: ColliderBuilder| match groups {
                Some(groups) => collider.collision_groups(groups),
                None => collider,
            };
            self.colliders.insert_with_parent(fit(shape).build(), body, &mut self.bodies);
            let extra = extra_solids(kind).into_iter().map(|solid| self.colliders.insert_with_parent(fit(solid).build(), body, &mut self.bodies)).collect();
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

    /// Whether a vehicle of `kind` can be simulated beside the one the client predicts (`set_neighbour`):
    /// one that stays on its wheels or tracks with nobody at its controls. An aircraft, a boat or a
    /// walker would fall or drift without what its pilot does, and a towed gun hangs on a hitch that
    /// is not here.
    pub fn can_neighbour(kind: u8) -> bool {
        spec(kind).lift == Lift::Wheels && !shared::vehicles::vehicle_def(kind).body.towed
    }

    /// Puts vehicle `id` of `kind` where a snapshot shows it as a vehicle of its own, in place of its
    /// stand-in, so that the vehicle the client predicts shoves it as the server's does rather than
    /// stopping dead on a solid nothing moves. It is moving at `velocity` m/s with its front wheels
    /// at `steer` of their lock, worn as `condition` says, and is held to `controls` by every
    /// `drive_neighbours` until it is set again. `turret` is whether a tank still has its turret.
    /// Whether it was done: not for a kind that is not one that `can_neighbour`, nor for a vehicle already
    /// simulated here in its own right.
    #[allow(clippy::too_many_arguments)]
    pub fn set_neighbour(&mut self, id: u32, kind: u8, position: [f32; 3], rotation: [f32; 4], velocity: [f32; 3], steer: f32, controls: CarControls, condition: CarCondition, turret: bool) -> bool {
        if !Self::can_neighbour(kind) || (self.cars.contains_key(&id) && !self.neighbours.contains_key(&id)) {
            return false;
        }
        if self.neighbours.get(&id).is_some_and(|&(known, _)| known != kind) {
            self.remove_neighbour(id);
        }
        if !self.cars.contains_key(&id) {
            self.remove_obstacle(id);
            self.add_car(id, kind, position, 0.0);
        }
        self.neighbours.insert(id, (kind, controls));
        // What a snapshot does not say of another's vehicle (its spin, a tank's turning) starts at rest.
        let motion = CarMotion { linvel: velocity, angvel: [0.0; 3], steer: steer * spec(kind).steer_lock, spin: 0.0 };
        self.set_car_state(id, position, rotation, motion, None);
        self.set_car_condition(id, condition);
        self.set_turret(id, turret);
        true
    }

    /// Stops simulating vehicle `id` beside the predicted one. Its next `set_obstacle` makes it a solid again.
    pub fn remove_neighbour(&mut self, id: u32) {
        if self.neighbours.remove(&id).is_some() {
            self.remove_car(id);
        }
    }

    /// The vehicles simulated beside the predicted one, in id order.
    pub fn neighbour_ids(&self) -> Vec<u32> {
        self.neighbours.keys().copied().collect()
    }

    /// Applies each neighbour's controls for the next `step_cars`, as the server drives every vehicle
    /// before it steps them.
    pub fn drive_neighbours(&mut self, dt: f32) {
        let neighbours: Vec<(u32, CarControls)> = self.neighbours.iter().map(|(&id, &(_, controls))| (id, controls)).collect();
        for (id, controls) in neighbours {
            self.drive_car(id, controls, dt);
        }
    }
}

#[cfg(test)]
mod tests;
