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

fn streamed(manifest: &Manifest, tiles: &[(u16, u16, Tile)]) -> Physics {
    let mut physics = Physics::streamed(manifest);
    for (col, row, tile) in tiles {
        physics.add_tile(manifest, *col, *row, tile).unwrap();
    }
    physics
}

/// Rays down onto a grid over the city and across it at a few heights.
fn probes(half: f32) -> Vec<([f32; 3], [f32; 3])> {
    let mut rays = Vec::new();
    let step = half / 12.0;
    let mut x = -half + step / 2.0;
    while x < half {
        let mut z = -half + step / 2.0;
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
fn a_stickman_walks_the_streamed_city_as_the_server_walks_it() {
    let (manifest, tiles) = fixture();
    let whole = Physics::new(&WorldFile::assemble(&manifest, tiles.clone())).unwrap();
    let streamed = streamed(&manifest, &tiles);
    let spawn = manifest.spawns[0].map(from_cm);
    let ground = whole.cast_ray_static([spawn[0], 80.0, spawn[1]], [0.0, -1.0, 0.0], 200.0).unwrap();
    let start = Character { position: [spawn[0], 80.0 - ground + STANDING_Y, spawn[1]], grounded: true, ..Default::default() };
    let (mut a, mut b) = (start, start);
    for tick in 0..600u32 {
        // Running and turning, now and then jumping: into walls, kerbs and slopes.
        let input = InputFrame { seq: tick + 1, forward: 127, strafe: if tick % 200 < 100 { 60 } else { -60 }, yaw: ((tick * 7) % 6283) as i16 - 3141, buttons: if tick % 90 == 0 { BUTTON_JUMP } else { BUTTON_RUN }, ..Default::default() };
        whole.step_character(&mut a, &input, TICK_DT, 1.0, None);
        streamed.step_character(&mut b, &input, TICK_DT, 1.0, None);
    }
    let travelled = (a.position[0] - start.position[0]).hypot(a.position[2] - start.position[2]);
    assert!(travelled > 10.0, "it hardly moved: {travelled} m");
    let apart = (0..3).map(|i| (a.position[i] - b.position[i]).powi(2)).sum::<f32>().sqrt();
    assert!(apart < 0.01, "after 10 s apart by {apart} m: {:?} and {:?}", a.position, b.position);
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
