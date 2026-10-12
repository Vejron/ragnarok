use super::*;
use shared::terrain::Ground;
use shared::world::{CollisionMesh, WORLD_VERSION};

fn flat_world() -> WorldFile {
    WorldFile { version: WORLD_VERSION, half_extent: 10_000, ..Default::default() }
}

/// A 2 m tall box from (5, 0, -1) to (6, 2, 1), as a closed triangle mesh.
fn world_with_wall() -> WorldFile {
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![
        v(5.0, 0.0, -1.0), v(6.0, 0.0, -1.0), v(6.0, 0.0, 1.0), v(5.0, 0.0, 1.0),
        v(5.0, 2.0, -1.0), v(6.0, 2.0, -1.0), v(6.0, 2.0, 1.0), v(5.0, 2.0, 1.0),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat_world() }
}

/// 200 m square of 5 m cells, height `f(x, z)` metres.
fn terrain_world(f: impl Fn(f32, f32) -> f32) -> WorldFile {
    let side = 41u32;
    let heights = (0..side)
        .flat_map(|j| (0..side).map(move |i| (i, j)))
        .map(|(i, j)| (f(i as f32 * 5.0 - 100.0, j as f32 * 5.0 - 100.0) * 100.0).round() as i16)
        .collect();
    WorldFile { ground: Ground { origin: -10_000, cell: 500, side, heights }, ..flat_world() }
}

fn standing() -> Character {
    Character { position: [0.0, STANDING_Y, 0.0], grounded: true, ..Default::default() }
}

#[test]
fn falls_and_lands_on_the_ground() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = Character { position: [0.0, 3.0, 0.0], ..Default::default() };
    for _ in 0..120 {
        physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
    }
    assert!(character.grounded);
    assert_eq!(character.vy, 0.0);
    assert!((character.position[1] - STANDING_Y).abs() < 0.05, "{:?}", character.position);
}

#[test]
fn walks_forward_at_walk_speed() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    let forward = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 };
    for _ in 0..TICK_RATE {
        physics.move_character(&mut character, &forward, TICK_DT);
    }
    assert!((character.position[2] + WALK_SPEED).abs() < 0.1, "{:?}", character.position);
    assert!(character.grounded);
}

#[test]
fn sliding_carries_a_runner_much_further_than_crouch_walking() {
    let physics = Physics::new(&flat_world()).unwrap();
    let held = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN | BUTTON_CROUCH, pitch: 0, view: 0, slot: 0 };
    let press = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN | BUTTON_CROUCH | BUTTON_SLIDE, pitch: 0, view: 0, slot: 0 };
    let distance = |first: &InputFrame| {
        let mut character = standing();
        physics.move_character(&mut character, first, TICK_DT);
        for _ in 0..TICK_RATE {
            physics.move_character(&mut character, &held, TICK_DT);
        }
        -character.position[2]
    };
    assert!(distance(&press) > distance(&held) + 3.0, "{} vs {}", distance(&press), distance(&held));
}

#[test]
fn jumps_and_comes_back_down() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    physics.move_character(&mut character, &InputFrame { seq: 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_JUMP, pitch: 0, view: 0, slot: 0 }, TICK_DT);
    assert!(!character.grounded);
    let mut peak: f32 = 0.0;
    for _ in 0..120 {
        physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
        peak = peak.max(character.position[1]);
    }
    assert!(peak > STANDING_Y + 0.8, "peak {peak}");
    assert!(character.grounded);
}

#[test]
fn walls_block_movement() {
    let physics = Physics::new(&world_with_wall()).unwrap();
    let mut character = standing();
    // Yaw -π/2 faces +X, towards the wall.
    let toward_wall = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: -1571, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
    for _ in 0..TICK_RATE * 2 {
        physics.move_character(&mut character, &toward_wall, TICK_DT);
    }
    let x = character.position[0];
    assert!(x < 5.0 - CAPSULE_RADIUS + 0.05 && x > 4.0, "x = {x}");
}

#[test]
fn a_ray_names_the_building_and_destroying_it_opens_the_wall() {
    let id = ObjId { col: 3, row: 4, index: 7 };
    let mut world = world_with_wall();
    world.building_ids = vec![id];
    world.building_triangles = vec![[0, 12]];
    let mut physics = Physics::new(&world).unwrap();
    let (distance, hit) = physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 20.0).expect("the wall is there");
    assert!((distance - 5.0).abs() < 1e-3, "{distance}");
    assert_eq!(hit, Some(Obj::Building(id)));

    assert!(physics.destroy(Obj::Building(id)));
    assert!(!physics.destroy(Obj::Building(id)), "already going");
    // Still there until the step that removes it.
    assert!(physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 20.0).is_some());
    physics.step_cars(TICK_DT);
    assert!(physics.cast_ray_object([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 20.0).is_none());

    let mut character = standing();
    let toward_wall = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: -1571, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
    for _ in 0..TICK_RATE * 2 {
        physics.move_character(&mut character, &toward_wall, TICK_DT);
    }
    assert!(character.position[0] > 6.0, "walked through: x = {}", character.position[0]);
}

#[test]
fn world_triangles_no_building_claims_stay_one_collider() {
    // Same wall, but a building claims only half of it: the other half must still block.
    let id = ObjId { col: 0, row: 0, index: 0 };
    let mut world = world_with_wall();
    world.building_ids = vec![id];
    world.building_triangles = vec![[0, 4]];
    let mut physics = Physics::new(&world).unwrap();
    physics.destroy(Obj::Building(id));
    physics.step_cars(TICK_DT);
    let still_solid = (0..20).any(|k| physics.cast_ray_object([0.0, 1.0, -0.9 + k as f32 * 0.09], [1.0, 0.0, 0.0], 20.0).is_some());
    assert!(still_solid, "the unclaimed triangles vanished with the building");
}

#[test]
fn boundary_walls_keep_players_in() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    let forward = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
    for _ in 0..TICK_RATE * 20 {
        physics.move_character(&mut character, &forward, TICK_DT);
    }
    assert!(character.position[2] > -100.0, "{:?}", character.position);
}

/// A scripted run against the wall box: walking, wall sliding and jumps. `predict.spec.ts` replays
/// the same script in WASM Rapier and checks it against `SCRIPTED_CHECKPOINTS`.
pub fn scripted_input(seq: u32) -> InputFrame {
    let yaw = -1571 + ((seq as i32 * 13) % 1200 - 600) as i16;
    let buttons = if seq % 90 == 30 { BUTTON_JUMP } else if seq > 150 { BUTTON_RUN } else { 0 };
    InputFrame { seq, strafe: (seq % 7) as i8 * 10 - 30, forward: 127, yaw, buttons, pitch: 0, view: 0, slot: 0 }
}

/// `(tick, position)` after `scripted_input`, mirrored in `predict.spec.ts`.
const SCRIPTED_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (30, [1.8138, 0.9722, 0.7480]),
    (60, [3.7758, 1.4722, 0.7701]),
    (90, [4.6299, 0.8649, 0.0827]),
    (300, [4.6295, 0.9728, 0.4872]),
];

#[test]
fn scripted_run_matches_fixture() {
    let physics = Physics::new(&world_with_wall()).unwrap();
    let mut character = standing();
    for seq in 1..=300 {
        physics.move_character(&mut character, &scripted_input(seq), TICK_DT);
        if let Some((_, expected)) = SCRIPTED_CHECKPOINTS.iter().find(|(tick, _)| *tick == seq) {
            let close = character.position.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 0.001);
            assert!(close, "tick {seq}: {:?}", character.position);
        }
    }
    assert!(!character.grounded);
}

#[test]
fn height_at_is_the_surface_the_collider_has() {
    let hills = |x: f32, z: f32| 8.0 + 6.0 * (x * 0.07).sin() * (z * 0.05).cos();
    let world = terrain_world(hills);
    let physics = Physics::new(&world).unwrap();
    for k in 0..200 {
        let (x, z) = (-95.0 + (k * 37 % 190) as f32 + 0.37, -95.0 + (k * 53 % 190) as f32 + 0.61);
        let hit = physics.cast_ray([x, 100.0, z], [0.0, -1.0, 0.0], 200.0, 0).expect("terrain below");
        let surface = 100.0 - hit;
        let sampled = world.ground.height_at(x, z);
        assert!((surface - sampled).abs() < 0.01, "({x}, {z}): ray {surface}, sampler {sampled}");
    }
}

#[test]
fn walking_down_and_up_a_slope_stays_grounded() {
    // A 20° ramp rising towards +Z.
    let slope = 20.0f32.to_radians().tan();
    let world = terrain_world(|_, z| (z + 100.0) * slope);
    let physics = Physics::new(&world).unwrap();
    for (start_z, input_z) in [(40.0, 127), (-40.0, -127)] {
        let ground = world.ground.height_at(0.0, start_z);
        let mut character = Character { position: [0.0, ground + STANDING_Y, start_z], grounded: true, ..Default::default() };
        let mut airborne = 0;
        for seq in 0..TICK_RATE * 4 {
            let input = InputFrame { seq, strafe: 0, forward: input_z, yaw: 0, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
            physics.move_character(&mut character, &input, TICK_DT);
            airborne += !character.grounded as u32;
        }
        let ground = world.ground.height_at(character.position[0], character.position[2]);
        assert!((character.position[1] - ground - STANDING_Y).abs() < 0.1, "{:?} on {ground}", character.position);
        assert!(airborne <= TICK_RATE / 10, "airborne {airborne} of {} ticks from z = {start_z}", TICK_RATE * 4);
    }
}

/// The slope the cross-language fixture runs on: a 20° ramp rising towards +Z, in 5 m cells.
/// `predict.spec.ts` builds the same heights.
fn ramp_world() -> WorldFile {
    let slope = 20.0f32.to_radians().tan();
    terrain_world(|_, z| (z + 100.0) * slope)
}

/// Running downhill with strafing, a jump and a stop: `ramp_world`'s fixture input.
pub fn ramp_input(seq: u32) -> InputFrame {
    let buttons = if seq == 50 { BUTTON_RUN | BUTTON_JUMP } else if seq < 150 { BUTTON_RUN } else { 0 };
    InputFrame { seq, strafe: (seq % 5) as i8 * 10 - 20, forward: if seq < 150 { 127 } else { 0 }, yaw: (seq as i16 / 3) % 200, buttons, pitch: 0, view: 0, slot: 0 }
}

/// `(tick, position)` after `ramp_input` from `RAMP_START`, mirrored in `predict.spec.ts`.
const RAMP_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (30, [-0.0180, 50.4969, 36.2721]),
    (60, [-0.0733, 50.4621, 32.5448]),
    (120, [-0.2956, 46.4279, 25.0934]),
    (240, [-0.4475, 45.1151, 21.4869]),
];
const RAMP_START: [f32; 2] = [0.0, 40.0];

#[test]
fn ramp_run_matches_fixture() {
    let world = ramp_world();
    let physics = Physics::new(&world).unwrap();
    let ground = world.ground.height_at(RAMP_START[0], RAMP_START[1]);
    let mut character = Character { position: [RAMP_START[0], ground + STANDING_Y, RAMP_START[1]], grounded: true, ..Default::default() };
    for seq in 1..=240 {
        physics.move_character(&mut character, &ramp_input(seq), TICK_DT);
        if let Some((_, expected)) = RAMP_CHECKPOINTS.iter().find(|(tick, _)| *tick == seq) {
            let close = character.position.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 0.001);
            assert!(close, "tick {seq}: {:?}", character.position);
        }
    }
}

#[test]
fn simulation_is_deterministic() {
    let physics = Physics::new(&world_with_wall()).unwrap();
    let run = || {
        let mut character = standing();
        for seq in 0..300u32 {
            let input = InputFrame { seq, strafe: 90, forward: 127, yaw: (seq as i16 * 7) % 3000, buttons: if seq % 50 == 0 { BUTTON_JUMP } else { 0 }, pitch: 0, view: 0, slot: 0 };
            physics.move_character(&mut character, &input, TICK_DT);
        }
        character
    };
    assert_eq!(run(), run());
}
