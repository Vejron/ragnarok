use shared::vec3;
use super::*;
use shared::protocol::{KIND_JEEP, KIND_TANK};

/// The committed city: its manifest and its nine tiles.
fn fixture() -> (Manifest, Vec<(u16, u16, Tile)>) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world");
    let manifest = Manifest::decode(&std::fs::read(dir.join("world.msgpack")).unwrap()).unwrap();
    let tiles = manifest
        .tiles
        .iter()
        .map(|entry| (entry.col, entry.row, Tile::decode(&std::fs::read(dir.join(entry.path())).unwrap()).unwrap()))
        .collect();
    (manifest, tiles)
}

/// The world of `tiles` streamed in, every building made solid.
fn streamed(manifest: &Manifest, tiles: &[(u16, u16, Tile)]) -> Physics {
    let mut physics = Physics::streamed(manifest);
    for (col, row, tile) in tiles {
        physics.add_tile(manifest, *col, *row, tile.clone()).unwrap();
    }
    physics.build_pending(0.0, 0.0, f32::INFINITY, usize::MAX);
    physics.refresh();
    physics
}

/// The streamed fixture city's buildings: each one's id and middle, metres `[x, z]`.
fn buildings_of(physics: &Physics) -> Vec<(shared::world::ObjId, [f32; 2])> {
    physics.tiles.values().flat_map(|tile| tile.world.building_ids.iter().copied().zip(tile.centres.iter().copied())).collect()
}

#[test]
fn buildings_near_the_player_are_built_before_far_ones() {
    let (manifest, tiles) = fixture();
    let mut physics = Physics::streamed(&manifest);
    for (col, row, tile) in &tiles {
        physics.add_tile(&manifest, *col, *row, tile.clone()).unwrap();
    }
    let all = buildings_of(&physics);
    assert!(all.len() > 50, "only {} buildings", all.len());
    assert!(physics.handles.keys().all(|o| !matches!(o, Obj::Building(_))), "nothing is built before it is asked for");
    assert_eq!(physics.buildings_pending(), all.len());
    // From a corner of the city: everything within 40 m at once, then the five nearest past that.
    let (x, z) = (from_cm(-manifest.half_extent) + 30.0, from_cm(-manifest.half_extent) + 30.0);
    let near = all.iter().filter(|(_, [cx, cz])| (cx - x).hypot(cz - z) <= 40.0).count();
    assert_eq!(physics.build_pending(x, z, 40.0, 5), near.max(5));
    physics.refresh();
    let built: Vec<f32> = all.iter().filter(|(id, _)| physics.handles.contains_key(&Obj::Building(*id))).map(|(_, [cx, cz])| (cx - x).hypot(cz - z)).collect();
    let waiting: Vec<f32> = all.iter().filter(|(id, _)| !physics.handles.contains_key(&Obj::Building(*id))).map(|(_, [cx, cz])| (cx - x).hypot(cz - z)).collect();
    // A building without anything to collide with is built as nothing.
    assert!(built.len() <= near.max(5) && !built.is_empty());
    let furthest = built.iter().copied().fold(0.0, f32::max);
    assert!(waiting.iter().all(|&d| d >= furthest - 1e-3), "a far building was built before a nearer one");
    assert_eq!(physics.buildings_pending(), all.len() - near.max(5));
}

#[test]
fn a_building_carved_before_it_was_built_is_built_carved() {
    let (manifest, tiles) = fixture();
    // The same building, carved once it stood and carved before it was built: the same shape.
    let mut first = streamed(&manifest, &tiles);
    let mut later = Physics::streamed(&manifest);
    for (col, row, tile) in &tiles {
        later.add_tile(&manifest, *col, *row, tile.clone()).unwrap();
    }
    let tile = first.tiles.values().find(|t| t.world.buildings.iter().any(|b| b.cells.dims[0] > 0)).unwrap();
    let at = tile.world.buildings.iter().position(|b| b.cells.dims[0] > 0).unwrap();
    let id = tile.world.building_ids[at];
    let [cx, cz] = tile.centres[at];
    let base = from_cm(tile.world.buildings[at].base);
    // A slice of cells through its middle, ground to roof.
    let dims = tile.world.buildings[at].cells.dims;
    let removed: Vec<u32> = (0..dims[1]).flat_map(|y| (0..dims[2]).map(move |z| (y * dims[2] + z) * dims[0] + dims[0] / 2)).collect();
    first.carve_building(id.col, id.row, id.index, &removed);
    first.refresh();
    later.carve_building(id.col, id.row, id.index, &removed);
    later.build_pending(0.0, 0.0, f32::INFINITY, usize::MAX);
    later.refresh();
    for dz in [-3.0, -1.0, 0.0, 1.0, 3.0] {
        for y in [0.5, 2.0] {
            let from = [cx - 30.0, base + y, cz + dz];
            assert_eq!(first.cast_ray_static(from, [1.0, 0.0, 0.0], 60.0), later.cast_ray_static(from, [1.0, 0.0, 0.0], 60.0), "ray from {from:?}");
        }
    }
}

#[test]
fn gates_told_before_their_building_was_built_are_as_told_once_it_is() {
    let (manifest, tiles) = fixture();
    let mut physics = Physics::streamed(&manifest);
    for (col, row, tile) in &tiles {
        physics.add_tile(&manifest, *col, *row, tile.clone()).unwrap();
    }
    // Every building with a look told its gates while none is built: the server's word, whatever it was before.
    let looked: Vec<shared::world::ObjId> = physics.tiles.values().flat_map(|tile| tile.world.building_ids.iter().zip(&tile.look.buildings).filter(|(_, range)| range[1] + range[2] > 0).map(|(&id, _)| id)).collect();
    assert!(!looked.is_empty(), "the city has no building with a look");
    for id in &looked {
        physics.set_gates(id.col, id.row, id.index, &[1, 4]);
    }
    physics.build_pending(0.0, 0.0, f32::INFINITY, usize::MAX);
    for id in &looked {
        let tile = &physics.tiles[&(id.col, id.row)];
        let at = tile.world.building_ids.iter().position(|known| known == id).unwrap();
        assert_eq!(tile.world.looks[at].shut, [1, 4], "building {id:?} lost the gates it was told while it waited");
    }
}

#[test]
fn a_tile_taken_out_drops_its_buildings_waiting_to_be_built() {
    let (manifest, tiles) = fixture();
    let mut physics = Physics::streamed(&manifest);
    for (col, row, tile) in &tiles {
        physics.add_tile(&manifest, *col, *row, tile.clone()).unwrap();
    }
    let total = physics.buildings_pending();
    let (col, row, _) = &tiles[4];
    let own = physics.tiles[&(*col, *row)].pending.len();
    physics.remove_tile(*col, *row);
    assert_eq!(physics.buildings_pending(), total - own);
    physics.build_pending(0.0, 0.0, f32::INFINITY, usize::MAX);
    assert!(physics.handles.keys().all(|o| !matches!(o, Obj::Building(id) if id.col == *col && id.row == *row)));
}

#[test]
fn a_carved_building_streamed_out_and_in_leaves_no_old_collider_behind() {
    let (manifest, tiles) = fixture();
    let mut physics = streamed(&manifest, &tiles);
    let colliders = physics.colliders.len();
    let (col, row, tile) = tiles.iter().find(|(_, _, t)| t.buildings.iter().any(|b| b.cells.dims[0] > 0)).unwrap();
    let index = tile.buildings.iter().position(|b| b.cells.dims[0] > 0).unwrap() as u32;
    // A cut gives the building a new collider in place of the one it was built with.
    physics.carve_building(*col, *row, index, &[0, 1, 2]);
    physics.refresh();
    physics.remove_tile(*col, *row);
    physics.add_tile(&manifest, *col, *row, tile.clone()).unwrap();
    physics.build_pending(0.0, 0.0, f32::INFINITY, usize::MAX);
    physics.refresh();
    assert_eq!(physics.colliders.len(), colliders, "the carved collider was left behind when its tile went");
}

/// Rays down onto a grid over the city and across it at a few heights.
fn probes(half: f32) -> Vec<([f32; 3], [f32; 3])> {
    let mut rays = Vec::new();
    let step = half / 12.0;
    // Off the lattice the buildings' cubes lie on, so no ray runs along a seam between two boxes.
    let mut x = -half + step / 2.0 + 0.07;
    while x < half {
        let mut z = -half + step / 2.0 + 0.07;
        while z < half {
            rays.push(([x, 80.0, z], [0.0, -1.0, 0.0]));
            z += step;
        }
        for y in [0.5, 2.0, 8.0] {
            rays.push(([x, y, -half + 1.0], [0.0, 0.0, 1.0]));
            rays.push(([-half + 1.0, y, x], [1.0, 0.0, 0.0]));
        }
        x += step;
    }
    rays
}

#[test]
fn a_world_streamed_tile_by_tile_collides_as_the_whole_world_does() {
    let (manifest, tiles) = fixture();
    let whole = Physics::new(&WorldFile::assemble(&manifest, tiles.clone())).unwrap();
    let streamed = streamed(&manifest, &tiles);
    let half = from_cm(manifest.half_extent);
    let mut hits = 0;
    for (origin, dir) in probes(half) {
        let a = whole.cast_ray_static(origin, dir, 2.0 * half);
        let b = streamed.cast_ray_normal(origin, dir, 2.0 * half).map(|(t, _)| t);
        match (a, b) {
            (Some(a), Some(b)) => {
                assert!((a - b).abs() < 1e-3, "ray from {origin:?} along {dir:?}: whole world {a}, streamed {b}");
                hits += 1;
            }
            (None, None) => {}
            _ => panic!("ray from {origin:?} along {dir:?}: whole world {a:?}, streamed {b:?}"),
        }
    }
    assert!(hits > 100, "only {hits} rays hit anything");
}

#[test]
fn a_soldier_walks_the_streamed_city_as_the_server_walks_it() {
    // The first spawn does not get far on this route.
    const SPAWN: usize = 1;
    let (manifest, tiles) = fixture();
    let whole = Physics::new(&WorldFile::assemble(&manifest, tiles.clone())).unwrap();
    let streamed = streamed(&manifest, &tiles);
    let spawn = manifest.spawns[SPAWN].map(from_cm).map(|v| v + 0.31);
    let ground = whole.cast_ray_static([spawn[0], 80.0, spawn[1]], [0.0, -1.0, 0.0], 200.0).unwrap();
    let start = Character { position: [spawn[0], 80.0 - ground + STANDING_Y, spawn[1]], grounded: true, ..Default::default() };
    let (mut a, mut b) = (start, start);
    for tick in 0..600u32 {
        // Running and turning, now and then jumping: into walls, kerbs and slopes.
        let input = InputFrame { seq: tick + 1, forward: 127, strafe: if tick % 200 < 100 { 60 } else { -60 }, yaw: ((tick * 7) % 6283) as i16 - 3141, buttons: if tick % 90 == 0 { BUTTON_JUMP } else { BUTTON_RUN }, ..Default::default() };
        whole.step_character(&mut a, &input, TICK_DT, 1.0, false, None);
        streamed.step_character(&mut b, &input, TICK_DT, 1.0, false, None);
    }
    let travelled = (a.position[0] - start.position[0]).hypot(a.position[2] - start.position[2]);
    assert!(travelled > 4.0, "it hardly moved: {travelled} m");
    let apart = vec3::distance(a.position, b.position);
    // Contacts resolve a centimetre or two apart on tiles and on the whole world; a missing wall would be metres.
    assert!(apart < 0.05, "after 10 s apart by {apart} m: {:?} and {:?}", a.position, b.position);
}

#[test]
fn a_tile_taken_out_leaves_only_the_slab_under_it() {
    let (manifest, tiles) = fixture();
    let mut physics = streamed(&manifest, &tiles);
    let (col, row, _) = &tiles[4];
    let size = from_cm(manifest.tile_size);
    let centre = [from_cm(-manifest.half_extent) + (f32::from(*col) + 0.5) * size, from_cm(-manifest.half_extent) + (f32::from(*row) + 0.5) * size];
    physics.remove_tile(*col, *row);
    let (t, _) = physics.cast_ray_normal([centre[0], 80.0, centre[1]], [0.0, -1.0, 0.0], 200.0).expect("the slab is still there");
    assert!((80.0 - t - (if manifest.ground_cell > 0 { -BED_FLOOR } else { 0.0 })).abs() < 1e-3, "hit at {}", 80.0 - t);
}

#[test]
fn a_felled_tree_and_a_fallen_building_stop_colliding() {
    let (manifest, tiles) = fixture();
    let mut physics = streamed(&manifest, &tiles);
    let objects: Vec<Obj> = physics.handles.keys().copied().collect();
    assert!(objects.iter().any(|o| matches!(o, Obj::Building(_))) && objects.iter().any(|o| matches!(o, Obj::Tree(_))));
    for object in &objects {
        assert!(physics.destroy(*object));
    }
    physics.refresh();
    assert!(physics.handles.is_empty());
}

#[test]
fn vehicles_are_solid_where_the_snapshot_puts_them_and_a_tank_loses_its_turret() {
    let manifest = Manifest { half_extent: 5_000, tile_size: 10_000, ..Default::default() };
    let mut physics = Physics::streamed(&manifest);
    let upright = [0.0, 0.0, 0.0, 1.0];
    physics.set_obstacle(7, KIND_JEEP, [0.0, 1.0, 0.0], upright, [3.0, 0.0, 0.0], true);
    physics.set_obstacle(8, KIND_TANK, [20.0, 1.2, 0.0], upright, [0.0; 3], true);
    physics.refresh();
    assert!(physics.cast_ray_normal([0.0, 10.0, 0.0], [0.0, -1.0, 0.0], 20.0).is_some_and(|(t, _)| t < 9.5), "the jeep's roof");
    assert_eq!(physics.obstacle_velocity(7), Some([3.0, 0.0, 0.0]));
    let turret_top = |physics: &Physics| physics.cast_ray_normal([20.0 + TANK_TURRET_PIVOT[0], 10.0, TANK_TURRET_PIVOT[2]], [0.0, -1.0, 0.0], 20.0).map(|(t, _)| 10.0 - t);
    let with = turret_top(&physics).unwrap();
    physics.set_obstacle(8, KIND_TANK, [20.0, 1.2, 0.0], upright, [0.0; 3], false);
    physics.refresh();
    let without = turret_top(&physics).unwrap();
    assert!(with > without + 0.3, "turret top {with}, hull top {without}");
    physics.remove_obstacle(7);
    physics.refresh();
    assert_eq!(physics.obstacle_ids(), vec![8]);
    assert!(physics.cast_ray_normal([0.0, 10.0, 0.0], [0.0, -1.0, 0.0], 20.0).is_some_and(|(t, _)| (t - 10.0).abs() < 1e-3), "only the slab where the jeep was");
}

#[test]
fn a_predicted_vehicle_is_not_moved_on_by_a_refresh_and_has_no_stand_in() {
    let manifest = Manifest { half_extent: 5_000, tile_size: 10_000, ..Default::default() };
    let mut physics = Physics::streamed(&manifest);
    physics.add_car(3, KIND_JEEP, [0.0, 3.0, 0.0], 0.0);
    let before = physics.car_pose(3).unwrap().position;
    for _ in 0..10 {
        physics.refresh();
    }
    assert_eq!(physics.car_pose(3).unwrap().position, before, "refreshing must not drop the car");
    physics.set_obstacle(3, KIND_JEEP, [20.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0], [0.0; 3], true);
    assert!(physics.obstacle_ids().is_empty());
    physics.step_cars(TICK_DT);
    assert!(physics.car_pose(3).unwrap().position[1] < before[1], "stepping does");
}

#[test]
fn a_crater_lowers_the_ground_the_server_collides_with() {
    let (manifest, tiles) = fixture();
    let mut world = WorldFile::assemble(&manifest, tiles);
    let mut physics = Physics::new(&world).unwrap();
    // Open ground (nothing built over it) on the seam between two tiles, x = -50 m.
    let x = -50.0;
    let open = |physics: &Physics, z: f32| physics.cast_ray_static([x, 80.0, z], [0.0, -1.0, 0.0], 200.0).is_some_and(|t| (80.0 - t - world.ground.height_at(x, z)).abs() < 0.01);
    let z = (-120..120).map(|k| k as f32 * 2.0 + 0.3).find(|&z| (-12..=12).all(|d| open(&physics, z + d as f32))).expect("a clear stretch of seam");
    let before = physics.cast_ray_static([x, 80.0, z], [0.0, -1.0, 0.0], 200.0).unwrap();
    let changed = world.ground.dig((x * 100.0) as i32, (z * 100.0) as i32, 600, 150, &|_| -1000).expect("the ground is dug");
    physics.dig_ground(&world.ground, changed);
    physics.apply_removals();
    let after = physics.cast_ray_static([x, 80.0, z], [0.0, -1.0, 0.0], 200.0).unwrap();
    // The samples are 5 m apart, so the bowl is cut where they are: less than its 1.5 m at the middle.
    assert!(after - before > 0.5, "the ground fell only {} m", after - before);
    assert!((world.ground.height_at(x, z) - (80.0 - after)).abs() < 0.02, "the collider and height_at agree");
    // Either side of the seam, the tiles' own rows moved together.
    for dx in [-0.5, 0.5] {
        let hit = physics.cast_ray_static([x + dx, 80.0, z], [0.0, -1.0, 0.0], 200.0).unwrap();
        assert!((world.ground.height_at(x + dx, z) - (80.0 - hit)).abs() < 0.02);
    }
}

#[test]
fn a_streamed_tile_given_dented_ground_collides_with_it() {
    let (manifest, tiles) = fixture();
    let mut physics = streamed(&manifest, &tiles);
    let mut world = WorldFile::assemble(&manifest, tiles.clone());
    let x = -50.0;
    let open = |physics: &Physics, z: f32| physics.cast_ray_normal([x, 80.0, z], [0.0, -1.0, 0.0], 200.0).is_some_and(|(t, _)| (80.0 - t - world.ground.height_at(x, z)).abs() < 0.01);
    let z = (-120..120).map(|k| k as f32 * 2.0 + 0.3).find(|&z| (-12..=12).all(|d| open(&physics, z + d as f32))).expect("a clear stretch of seam");
    let changed = world.ground.dig((x * 100.0) as i32, (z * 100.0) as i32, 600, 150, &|_| -1000).unwrap();
    // Every tile the crater touched, with its own block of the dug ground.
    let n = manifest.tile_samples() as usize;
    let cells = n as u32 - 1;
    for (col, row, _) in &tiles {
        let (i0, j0) = (u32::from(*col) * cells, u32::from(*row) * cells);
        if changed.0 > i0 + cells || changed.2 < i0 || changed.1 > j0 + cells || changed.3 < j0 {
            continue;
        }
        physics.set_tile_ground(&manifest, *col, *row, &world.ground.block(i0, j0, n as u32));
    }
    for dx in [-1.0, 0.0, 1.0] {
        let (t, _) = physics.cast_ray_normal([x + dx, 80.0, z], [0.0, -1.0, 0.0], 200.0).unwrap();
        assert!((world.ground.height_at(x + dx, z) - (80.0 - t)).abs() < 0.02, "at {dx}: ground {} collider {}", world.ground.height_at(x + dx, z), 80.0 - t);
    }
}

/// How wrong a driver's client is while it rams a parked jeep at full throttle, metres: the largest
/// correction a snapshot brings, and the furthest its prediction is from where the server had the
/// car at the same tick. The server's world runs both jeeps; the client's runs the driven one
/// `LATENCY` ticks ahead of the snapshots it gets every other tick, replaying its inputs over each,
/// with the parked jeep as a fixed stand-in or, with `beside`, simulated beside it.
fn ram_corrections(beside: bool) -> (f32, f32) {
    /// Ticks from an input being made to the snapshot that answers it being replayed: 100 ms.
    const LATENCY: usize = 6;
    const TICKS: usize = 300;
    let forward = CarControls { throttle: 1.0, ..CarControls::IDLE };
    let parked = CarControls { handbrake: true, ..CarControls::IDLE };
    let state = |physics: &Physics, id: u32| (physics.car_pose(id).unwrap(), physics.car_motion(id).unwrap());

    let mut server = Physics::new(&WorldFile { version: shared::world::WORLD_VERSION, half_extent: 20_000, ..Default::default() }).unwrap();
    server.add_car(1, KIND_JEEP, [0.0, 1.0, 0.0], 0.0);
    server.add_car(2, KIND_JEEP, [0.0, 1.0, -30.0], 0.0);
    // The server's two jeeps after each tick.
    let mut truth = Vec::new();
    for _ in 0..TICKS {
        server.drive_car(1, forward, TICK_DT);
        server.drive_car(2, parked, TICK_DT);
        server.step_cars(TICK_DT);
        truth.push((state(&server, 1), state(&server, 2)));
    }
    let end = truth[TICKS - 1];
    assert!(end.0.0.position[2] < -27.0 && end.1.0.position[2] < -31.0, "the server's jeep shoved the parked one: {:?} and {:?}", end.0.0.position, end.1.0.position);

    let mut client = Physics::streamed(&Manifest { half_extent: 20_000, tile_size: 40_000, ..Default::default() });
    client.add_car(1, KIND_JEEP, [0.0, 1.0, 0.0], 0.0);
    let step = |client: &mut Physics| {
        client.drive_car(1, forward, TICK_DT);
        client.drive_neighbours(TICK_DT);
        client.step_cars(TICK_DT);
    };
    let (mut largest, mut furthest) = (0.0f32, 0.0f32);
    for tick in 0..TICKS {
        if tick >= LATENCY && tick % 2 == 0 {
            // The snapshot of `LATENCY` ticks ago arrives: rewind to it and replay what was done since.
            let before = client.car_pose(1).unwrap().position;
            let ((pose, motion), (other, other_motion)) = truth[tick - LATENCY];
            // Of another's vehicle the client knows only its speed over the ground.
            let velocity = [other_motion.linvel[0], 0.0, other_motion.linvel[2]];
            if beside {
                assert!(client.set_neighbour(2, KIND_JEEP, other.position, other.rotation, velocity, 0.0, parked, CarCondition::of(KIND_JEEP, 255, [0; shared::protocol::DAMAGE_SLOTS]), true));
            } else {
                client.set_obstacle(2, KIND_JEEP, other.position, other.rotation, velocity, true);
                client.refresh();
            }
            client.set_car_state(1, pose.position, pose.rotation, motion, None);
            for _ in 0..LATENCY {
                step(&mut client);
            }
            largest = largest.max(vec3::distance(before, client.car_pose(1).unwrap().position));
        }
        step(&mut client);
        furthest = furthest.max(vec3::distance(client.car_pose(1).unwrap().position, truth[tick].0.0.position));
    }
    (largest, furthest)
}

#[test]
fn a_predicted_car_shoves_a_parked_one_as_the_server_does() {
    // Against a stand-in nothing moves, the predicted jeep stops dead and waits for the server.
    let (largest, furthest) = ram_corrections(false);
    assert!(largest > 0.3 && furthest > 1.0, "a fixed stand-in: corrected by {largest} m, {furthest} m from the server");
    let (largest, furthest) = ram_corrections(true);
    assert!(largest < 0.15 && furthest < 0.4, "simulated beside it: corrected by {largest} m, {furthest} m from the server");
}

#[test]
fn a_neighbour_takes_its_stand_ins_place_and_gives_it_back() {
    let manifest = Manifest { half_extent: 5_000, tile_size: 10_000, ..Default::default() };
    let mut physics = Physics::streamed(&manifest);
    let upright = [0.0, 0.0, 0.0, 1.0];
    let whole = |kind| CarCondition::of(kind, 255, [0; shared::protocol::DAMAGE_SLOTS]);
    physics.set_obstacle(7, KIND_JEEP, [0.0, 1.0, 0.0], upright, [0.0; 3], true);
    assert!(physics.set_neighbour(7, KIND_JEEP, [0.0, 1.0, 0.0], upright, [0.0; 3], 0.0, CarControls::IDLE, whole(KIND_JEEP), true));
    assert!(physics.obstacle_ids().is_empty() && physics.neighbour_ids() == vec![7]);
    // A snapshot's stand-ins leave a neighbour alone.
    physics.set_obstacle(7, KIND_JEEP, [0.0, 1.0, 0.0], upright, [0.0; 3], true);
    assert!(physics.obstacle_ids().is_empty());
    // What flies is not simulated without its pilot, and neither is the player's own vehicle.
    assert!(!physics.set_neighbour(8, shared::protocol::KIND_HELI, [20.0, 1.0, 0.0], upright, [0.0; 3], 0.0, CarControls::IDLE, whole(shared::protocol::KIND_HELI), true));
    physics.add_car(9, KIND_JEEP, [-20.0, 1.0, 0.0], 0.0);
    assert!(!physics.set_neighbour(9, KIND_JEEP, [-20.0, 1.0, 0.0], upright, [0.0; 3], 0.0, CarControls::IDLE, whole(KIND_JEEP), true));
    physics.remove_neighbour(9);
    assert!(physics.car_pose(9).is_some(), "the player's own vehicle is not a neighbour to remove");
    physics.remove_neighbour(7);
    assert!(physics.car_pose(7).is_none() && physics.neighbour_ids().is_empty());
    physics.set_obstacle(7, KIND_JEEP, [0.0, 1.0, 0.0], upright, [0.0; 3], true);
    assert_eq!(physics.obstacle_ids(), vec![7]);
}

#[test]
fn a_hard_ram_breaks_a_hole_the_vehicle_fits_and_a_heavier_one_keeps_more_speed() {
    use shared::protocol::KIND_HELI;
    assert_eq!(crate::breach(KIND_JEEP, 8.0), None, "a jeep at under 30 km/h only chips the wall");
    let jeep = crate::breach(KIND_JEEP, 10.0).unwrap();
    let [half_width, half_height, _] = crate::half_extents(KIND_JEEP);
    assert!(jeep.radius > half_width.hypot(half_height), "the hole takes in the corners of its nose: {jeep:?}");
    assert_eq!(jeep.keep, Some(0.5));
    let tank = crate::breach(KIND_TANK, 4.0).unwrap();
    assert!(tank.keep.unwrap() > 0.8 && tank.radius > jeep.radius, "{tank:?}");
    // What flies bites the wall and stops on it.
    assert_eq!(crate::breach(KIND_HELI, 12.0).map(|breach| breach.keep), Some(None));
    assert_eq!(crate::breach_velocity([0.0, -1.0, -12.0], [0.0, 0.2, -1.0], 0.5), [0.0, 0.2, -6.0]);
    assert_eq!(crate::breach_velocity([0.0, 0.0, -12.0], [0.0, 0.0, -9.0], 0.5), [0.0, 0.0, -9.0], "never slower than the wall left it");
}

/// A wall of the committed city a jeep can drive straight at over open, nearly level ground: the
/// building, a point 12 m out from the wall a metre over the ground, and the unit direction `[x, z]`
/// from there to it.
fn open_wall(physics: &Physics, world: &WorldFile) -> (shared::world::ObjId, [f32; 3], [f32; 2]) {
    let ground = |x: f32, z: f32| physics.cast_ray_normal([x, 200.0, z], [0.0, -1.0, 0.0], 400.0).map(|(t, _)| 200.0 - t);
    for (index, building) in world.buildings.iter().enumerate() {
        if building.kind == shared::world::BUILDING_BRIDGE || building_shell(world, index).is_none() {
            continue;
        }
        let xs = building.footprint.iter().map(|p| from_cm(p[0]));
        let zs = building.footprint.iter().map(|p| from_cm(p[1]));
        let (x0, x1) = (xs.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max));
        let (z0, z1) = (zs.clone().fold(f32::MAX, f32::min), zs.fold(f32::MIN, f32::max));
        let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
        for (at, dir) in [([x0 - 12.0, cz], [1.0, 0.0]), ([x1 + 12.0, cz], [-1.0, 0.0]), ([cx, z0 - 12.0], [0.0, 1.0]), ([cx, z1 + 12.0], [0.0, -1.0])] {
            let Some(floor) = ground(at[0], at[1]) else { continue };
            let from = [at[0], floor + 1.0, at[1]];
            // The wall is the first thing ahead, a hull's width either side, and the ground hardly rises or falls up to it.
            let side = [-dir[1], dir[0]];
            let clear = [-1.2f32, 0.0, 1.2].iter().all(|&off| {
                let origin = [from[0] + side[0] * off, from[1], from[2] + side[1] * off];
                let ahead = physics.cast_ray_normal(origin, [dir[0], 0.0, dir[1]], 20.0).is_some_and(|(t, _)| (t - 12.0).abs() < 0.3);
                let level = (0..12).all(|step| ground(origin[0] + dir[0] * step as f32, origin[2] + dir[1] * step as f32).is_some_and(|y| (y - floor).abs() < 0.25));
                ahead && level
            });
            if clear {
                return (world.building_ids[index], from, dir);
            }
        }
    }
    panic!("no wall of the city stands open on level ground");
}

#[test]
fn a_predicted_jeep_breaks_through_a_wall_and_the_hole_closes_if_the_server_never_confirms_it() {
    let (manifest, tiles) = fixture();
    let world = WorldFile::assemble(&manifest, tiles.clone());
    let mut physics = streamed(&manifest, &tiles);
    let (id, from, dir) = open_wall(&physics, &world);
    let wall = |physics: &Physics| physics.cast_ray_normal(from, [dir[0], 0.0, dir[1]], 30.0).map(|(t, _)| t);
    physics.add_car(1, KIND_JEEP, [from[0], from[1] + 0.2, from[2]], (-dir[0]).atan2(-dir[1]));
    let along = |physics: &Physics| {
        let pose = physics.car_pose(1).unwrap();
        ((pose.position[0] - from[0]) * dir[0] + (pose.position[2] - from[2]) * dir[1], pose.velocity[0] * dir[0] + pose.velocity[2] * dir[1])
    };
    let forward = CarControls { throttle: 1.0, ..CarControls::IDLE };
    let step = |physics: &mut Physics| {
        physics.drive_car(1, forward, TICK_DT);
        let before = physics.car_pose(1).unwrap().velocity;
        physics.step_cars(TICK_DT);
        (before, physics.ram_buildings(1, KIND_JEEP, before))
    };
    for _ in 0..30 {
        step(&mut physics);
    }
    physics.set_car_velocity(1, [dir[0] * 15.0, 0.0, dir[1] * 15.0]);
    let mut broke = None;
    for _ in 0..60 {
        let (before, rammed) = step(&mut physics);
        if rammed {
            broke = Some(before[0] * dir[0] + before[2] * dir[1]);
            break;
        }
    }
    let came_in = broke.expect("it rammed the wall hard enough to break it");
    // The hole is handed over once, for drawing: which building, where it was hit, and the cells gone.
    let rams = physics.take_rams();
    assert!(rams.len() == 1 && rams[0].building == id && rams[0].blasted.len() > 4, "{rams:?}");
    assert!(vec3::distance(rams[0].point, [from[0] + dir[0] * 12.0, rams[0].point[1], from[2] + dir[1] * 12.0]) < 1.5, "it hit the wall 12 m off: {:?}", rams[0].point);
    assert!(physics.take_rams().is_empty());
    let (_, speed) = along(&physics);
    assert!((speed - came_in * 0.5).abs() < 0.5, "it came in at {came_in} m/s and goes on at {speed}");
    for _ in 0..30 {
        step(&mut physics);
    }
    // A wall that held would have stopped its nose there, its middle two metres short; what stands
    // in the rooms behind stops it in the end.
    let (gone, _) = along(&physics);
    assert!(gone > 12.5, "its middle is past the face of the wall 12 m off: {gone} m");
    assert!(wall(&physics).is_none_or(|t| t > 12.5), "the wall is open where it went: {:?}", wall(&physics));
    // The server's word on the building replaces the prediction, and is not forgotten with it.
    physics.carve_building(id.col, id.row, id.index, &[]);
    physics.forget_rams();
    physics.step_cars(TICK_DT);
    assert!(wall(&physics).is_some_and(|t| (t - 12.0).abs() < 0.3), "the server said nothing is cut: {:?}", wall(&physics));
}
