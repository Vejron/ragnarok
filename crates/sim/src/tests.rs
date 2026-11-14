use super::*;
use shared::terrain::Ground;
use shared::world::{Building, BuildingLook, CollisionMesh, Look, WORLD_VERSION};

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
        .map(|(i, j)| (f(i as f32 * 5.0 - 100.0, j as f32 * 5.0 - 100.0) * 100.0).round() as i32)
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
fn stony_ground_slows_a_walker_by_its_grade_on_the_server_and_streamed_alike() {
    use shared::world::{Manifest, STONES_DENSITY, Tile, TileRef};
    // One 200 m tile, its south half (z > 0) as stony as ISOM's "walk" (211).
    let grade = (0.7 * f32::from(STONES_DENSITY)).round() as u8;
    let stones: Vec<u8> = (0..100 * 100).map(|k| if k / 100 >= 50 { grade } else { 0 }).collect();
    let manifest = Manifest { version: WORLD_VERSION, half_extent: 10_000, tile_size: 20_000, tiles: vec![TileRef { col: 0, row: 0, hash: 0 }], ..Default::default() };
    let tile = Tile { stones, ..Default::default() };
    let server = Physics::new(&WorldFile::assemble(&manifest, [(0, 0, tile.clone())])).unwrap();
    let mut streamed = Physics::streamed(&manifest);
    streamed.add_tile(&manifest, 0, 0, &tile).unwrap();
    let forward = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 };
    let walked = |physics: &Physics, z: f32| {
        let mut character = Character { position: [0.0, STANDING_Y, z], grounded: true, ..Default::default() };
        for _ in 0..TICK_RATE {
            physics.move_character(&mut character, &forward, TICK_DT);
        }
        z - character.position[2]
    };
    assert!((walked(&server, -50.0) - WALK_SPEED).abs() < 0.1, "bare ground: {}", walked(&server, -50.0));
    let stony = walked(&server, 50.0);
    assert!((stony / WALK_SPEED - stones_scale(grade)).abs() < 0.03 && stony < WALK_SPEED * 0.7, "stony ground: {stony}");
    assert_eq!(walked(&streamed, 50.0), stony);
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
fn a_long_sprint_runs_out_of_stamina_and_slows_to_a_walk() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    let sprint = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
    let ticks = STAMINA_FULL.div_ceil(STAMINA_RUN_DRAIN);
    for _ in 0..ticks - 1 {
        physics.move_character(&mut character, &sprint, TICK_DT);
    }
    assert!(!character.winded);
    let before = character.position[2];
    physics.move_character(&mut character, &sprint, TICK_DT);
    assert!(character.winded && character.spent == STAMINA_FULL);
    // The tick it ran out is already a walk, and so is every one after it until it has recovered.
    assert!((before - character.position[2] - WALK_SPEED * TICK_DT).abs() < 1e-3);
    let before = character.position[2];
    physics.move_character(&mut character, &sprint, TICK_DT);
    assert!((before - character.position[2] - WALK_SPEED * TICK_DT).abs() < 1e-3);
}

#[test]
fn a_winded_jump_is_lower_than_a_rested_one() {
    let physics = Physics::new(&flat_world()).unwrap();
    let jump = InputFrame { seq: 1, buttons: BUTTON_JUMP, ..Default::default() };
    let peak = |mut character: Character| {
        physics.move_character(&mut character, &jump, TICK_DT);
        let mut top = character.position[1];
        for _ in 0..TICK_RATE {
            physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
            top = top.max(character.position[1]);
        }
        top - STANDING_Y
    };
    let rested = peak(standing());
    let winded = peak(Character { spent: STAMINA_FULL, winded: true, ..standing() });
    assert!((rested / winded - 1.6).abs() < 0.1, "{rested} vs {winded}");
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
fn a_fence_stops_a_soldier_and_a_power_pole_stops_them_and_a_ray() {
    use shared::world::{LINE_FENCE, LINE_POWER, Line};
    // A fence across x = 5 m, and a pole at (0, -5) m with a line going on past the world's edge.
    let fence = Line { kind: LINE_FENCE, points: vec![[500, -1000], [500, 1000]], base: vec![0, 0], end: true };
    let power = Line { kind: LINE_POWER, points: vec![[0, -500], [0, -9000]], base: vec![0, 0], end: false };
    let world = WorldFile { lines: vec![(0, 0, fence), (0, 0, power)], ..flat_world() };
    let physics = Physics::new(&world).unwrap();
    let toward = |yaw| InputFrame { seq: 1, strafe: 0, forward: 127, yaw, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
    let mut character = standing();
    for _ in 0..TICK_RATE * 2 {
        physics.move_character(&mut character, &toward(-1571), TICK_DT);
    }
    let x = character.position[0];
    assert!(x < 5.0 - CAPSULE_RADIUS && x > 4.0, "x = {x}");
    // Along it is open: the stop slides.
    let mut along = Character { position: [4.0, STANDING_Y, 0.0], ..standing() };
    for _ in 0..TICK_RATE {
        physics.move_character(&mut along, &toward(0), TICK_DT);
    }
    assert!(along.position[2] < -3.0, "{:?}", along.position);
    // A bullet goes through the wire, but not through the pole.
    assert_eq!(physics.cast_ray_static([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 10.0), None);
    assert!(physics.cast_ray_static([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], 10.0).is_some_and(|d| (d - 4.86).abs() < 0.02));
    let mut walker = standing();
    for _ in 0..TICK_RATE * 2 {
        physics.move_character(&mut walker, &toward(0), TICK_DT);
    }
    // A thin pole is walked round, not through: the capsule slides off it.
    assert!(walker.position[0].abs() > CAPSULE_RADIUS, "{:?}", walker.position);
}

#[test]
fn a_ski_lifts_pylon_and_station_are_solid_on_the_server_and_streamed_alike() {
    use shared::world::{LIFT_CHAIR, Lift, Manifest};
    // A lift north (-Z) from (0, -10) m over a pylon at (0, -50) m to (0, -90) m, on flat ground.
    let lift = Lift { kind: LIFT_CHAIR, seats: 2, points: vec![[0, -1_000], [0, -5_000], [0, -9_000]], base: vec![0, 0, 0], cable: vec![310, 900, 310] };
    let world = WorldFile { lifts: vec![lift.clone()], ..flat_world() };
    let manifest = Manifest { version: WORLD_VERSION, half_extent: 10_000, tile_size: 20_000, lifts: vec![lift], ..Default::default() };
    for physics in [Physics::new(&world).unwrap(), Physics::streamed(&manifest)] {
        // A round fired at the pylon from 5 m off meets its 0.35 m tube.
        let hit = physics.cast_ray_static([0.0, 1.0, -45.0], [0.0, 0.0, -1.0], 10.0);
        assert!(hit.is_some_and(|d| (d - 4.65).abs() < 0.02), "{hit:?}");
        // The bottom station's deck is underfoot: a soldier dropped on it stands on its top, 10 cm up.
        let mut character = Character { position: [0.0, 2.0, -12.0], ..Default::default() };
        for _ in 0..TICK_RATE {
            physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
        }
        assert!(character.grounded && (character.position[1] - STANDING_Y - 0.1).abs() < 0.05, "{:?}", character.position);
        // Beside the line, nothing.
        assert_eq!(physics.cast_ray_static([20.0, 1.0, -45.0], [0.0, 0.0, -1.0], 10.0), None);
    }
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

/// A building that is only cubes: a platform 72 cm high, 7.92 m wide at z -5.04..-2.88 and 4.32 m
/// wide at z -2.88..5.04, so it is two boxes meeting along z = -3. Its floor is two triangles at y = 0.
fn world_with_platform() -> WorldFile {
    let (origin, dims) = ([-216, 0, -504], [11usize, 1, 14]);
    let solid: Vec<bool> = (0..dims[0] * dims[1] * dims[2]).map(|id| (id / dims[0]) % dims[2] < 3 || id % dims[0] < 6).collect();
    let cells = shared::world::Cells {
        origin,
        dims: dims.map(|d| d as u32),
        solid: shared::world::Bits(shared::carve::pack(&solid, dims[0] * dims[2])),
        roof: shared::world::Bits(shared::carve::pack(&vec![false; solid.len()], dims[0] * dims[2])),
    };
    let vertices = vec![[-216, 0, -504], [576, 0, -504], [576, 0, 504], [-216, 0, 504]];
    WorldFile {
        collision: CollisionMesh { vertices, triangles: vec![[0, 2, 1], [0, 3, 2]] },
        buildings: vec![shared::world::Building { cells, ..Default::default() }],
        building_ids: vec![ObjId { col: 0, row: 0, index: 0 }],
        building_triangles: vec![[0, 2]],
        ..flat_world()
    }
}

#[test]
fn walking_over_the_cubes_stays_on_top_across_where_two_boxes_meet() {
    let world = world_with_platform();
    let (shell, _) = building_shell(&world, 0).unwrap();
    assert_eq!(shell.boxes(&shell.untouched()).len(), 2);
    let physics = Physics::new(&world).unwrap();
    let mut character = Character { position: [0.0, 0.72 + STANDING_Y, 4.5], grounded: true, ..Default::default() };
    // Forward is towards -z: 2 s at walking speed, over the seam at z = -2.88.
    for tick in 0..TICK_RATE * 2 {
        physics.move_character(&mut character, &held(0, 127), TICK_DT);
        assert!(character.grounded, "tick {tick}: {:?}", character.position);
        assert!((character.position[1] - (0.72 + STANDING_Y)).abs() < 0.01, "tick {tick}: sank or rose to {:?}", character.position);
    }
    assert!((character.position[2] - (4.5 - 2.0 * WALK_SPEED)).abs() < 0.2, "caught on the seam: {:?}", character.position);
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

/// A scripted run against the wall box: walking, wall sliding and jumps, the last of which clears it. `predict.spec.ts` replays
/// the same script in WASM Rapier and checks it against `SCRIPTED_CHECKPOINTS`.
pub fn scripted_input(seq: u32) -> InputFrame {
    let yaw = -1571 + ((seq as i32 * 13) % 1200 - 600) as i16;
    let buttons = if seq % 90 == 30 { BUTTON_JUMP } else if seq > 150 { BUTTON_RUN } else { 0 };
    InputFrame { seq, strafe: (seq % 7) as i8 * 10 - 30, forward: 127, yaw, buttons, pitch: 0, view: 0, slot: 0 }
}

/// `(tick, position)` after `scripted_input`, mirrored in `predict.spec.ts`.
const SCRIPTED_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (30, [1.8138, 1.0009, 0.7480]),
    (60, [3.7758, 2.3609, 0.7701]),
    (90, [4.6299, 0.8701, 0.0624]),
    (300, [21.9144, 1.0038, 0.5827]),
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

/// Whether a soldier running at a step in 1 m ground gets on top of it: `rise` metres over `width`,
/// the shape an orienteering map's cliff is cut into the terrain with.
fn scales_step(rise: f32, width: f32) -> bool {
    let side = 201u32;
    let height = |z: f32| ((10.0 - z) / width).clamp(0.0, 1.0) * rise;
    let heights = (0..side).flat_map(|j| (0..side).map(move |i| (i, j))).map(|(_, j)| (height(j as f32 - 100.0) * 100.0).round() as i32).collect();
    let world = WorldFile { ground: Ground { origin: -10_000, cell: 100, side, heights }, ..flat_world() };
    let physics = Physics::new(&world).unwrap();
    let mut character = Character { position: [0.0, STANDING_Y, 20.0], grounded: true, ..Default::default() };
    for seq in 0..TICK_RATE * 8 {
        physics.move_character(&mut character, &InputFrame { seq, forward: 127, buttons: BUTTON_RUN, ..Default::default() }, TICK_DT);
    }
    character.position[2] < 10.0 - width - 1.0
}

#[test]
fn a_sheer_step_stops_a_soldier_and_a_broken_one_does_not() {
    assert!(!scales_step(3.0, 1.0));
    assert!(scales_step(1.8, 2.0));
}

#[test]
fn a_hill_is_slower_up_than_down() {
    let slope = 0.2;
    let world = terrain_world(|_, z| (z + 100.0) * slope);
    let physics = Physics::new(&world).unwrap();
    let walked = |forward: i8| {
        let ground = world.ground.height_at(0.0, 0.0);
        let mut character = Character { position: [0.0, ground + STANDING_Y, 0.0], grounded: true, ..Default::default() };
        for seq in 0..TICK_RATE {
            let input = InputFrame { seq, strafe: 0, forward, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 };
            physics.move_character(&mut character, &input, TICK_DT);
        }
        character.position[2].abs()
    };
    // Yaw 0 faces -Z, so forward is downhill.
    assert!((walked(127) - WALK_SPEED).abs() < 0.2, "down {}", walked(127));
    assert!((walked(-127) - WALK_SPEED * 0.7).abs() < 0.2, "up {}", walked(-127));
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
    (60, [-0.0733, 50.7774, 32.5448]),
    (120, [-0.2956, 46.4280, 25.0937]),
    (240, [-0.4372, 45.1150, 21.4867]),
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

/// Skis on, standing on the ground of `world` at `[x, z]`.
fn on_skis(world: &WorldFile, x: f32, z: f32) -> Character {
    Character { position: [x, world.ground.height_at(x, z) + STANDING_Y, z], grounded: true, skis: true, ..Default::default() }
}

#[test]
fn skis_run_down_a_slope_with_nobody_pushing_and_stay_on_the_ground() {
    let world = terrain_world(|_, z| z * 0.17);
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 0.0, 60.0);
    let mut airborne = 0;
    for seq in 0..360 {
        physics.move_character(&mut character, &InputFrame { seq, ..Default::default() }, TICK_DT);
        airborne += u32::from(!character.grounded);
    }
    assert!(character.position[2] < 50.0 && -character.knock[1] > 4.0, "{character:?}");
    assert!(airborne < 3, "{airborne}");
    assert_eq!(character.down, 0);
}

#[test]
fn skis_climb_slowly_and_faster_with_the_poles_going_hard() {
    let world = terrain_world(|_, z| -z * 0.18);
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 0.0, 60.0);
    for seq in 0..600 {
        physics.move_character(&mut character, &InputFrame { seq, forward: 127, ..Default::default() }, TICK_DT);
    }
    assert!((40.0..58.0).contains(&character.position[2]), "{:?}", character.position);
}

#[test]
fn a_crest_at_speed_throws_a_skier_and_a_flat_landing_after_a_big_one_is_a_fall() {
    // Up a ramp towards -Z that ends in a 6 m drop onto the flat.
    let world = terrain_world(|_, z| if z > 40.0 { 0.0 } else if z > 10.0 { (40.0 - z) * 0.2 } else { 0.0 });
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 0.0, 42.0);
    character.knock = [0.0, -20.0];
    let (mut flew, mut fell) = (0, false);
    for seq in 0..240 {
        physics.move_character(&mut character, &InputFrame { seq, forward: 127, buttons: BUTTON_RUN, ..Default::default() }, TICK_DT);
        flew += u32::from(!character.grounded);
        fell |= character.down > 0;
    }
    assert!(flew > 20, "{flew}");
    assert!(fell, "{character:?}");
}

#[test]
fn a_skier_exactly_on_a_ground_line_still_feels_the_slope() {
    let world = ramp_world();
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 0.0, 60.0);
    physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
    assert!(character.knock[1] < 0.0, "{character:?}");
}

#[test]
fn a_wall_at_speed_is_a_crash() {
    let physics = Physics::new(&world_with_wall()).unwrap();
    let mut character = Character { skis: true, position: [-6.0, STANDING_Y, 0.0], grounded: true, ..Default::default() };
    character.knock = [14.0, 0.0];
    let east = shared::quant::to_mrad(-std::f32::consts::FRAC_PI_2);
    let mut fell = false;
    for seq in 0..120 {
        physics.move_character(&mut character, &InputFrame { seq, forward: 127, yaw: east, ..Default::default() }, TICK_DT);
        fell |= character.down > 0;
    }
    assert!(fell, "{character:?}");
}

#[test]
fn a_fall_tumbles_on_and_stops() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = Character { skis: true, ..standing() };
    character.knock = [0.0, -15.0];
    character.ski_strain = SKI_STRAIN_FALL - 1;
    let start = character.position[2];
    let input = InputFrame { yaw: 1500, ..Default::default() };
    physics.move_character(&mut character, &input, TICK_DT);
    assert!(character.down > 0, "{character:?}");
    while character.down > 0 {
        physics.move_character(&mut character, &input, TICK_DT);
    }
    assert!(start - character.position[2] > 6.0, "{:?}", character.position);
}

#[test]
fn a_track_is_faster_than_the_snow_beside_it() {
    let mut world = flat_world();
    world.ski.push(shared::world::SkiTrail { width: 400, points: vec![[0, 2_000], [0, -8_000]] });
    let physics = Physics::new(&world).unwrap();
    let run = |x: f32| {
        let mut character = Character { position: [x, STANDING_Y, 15.0], grounded: true, skis: true, ..Default::default() };
        for seq in 0..240 {
            physics.move_character(&mut character, &InputFrame { seq, forward: 127, ..Default::default() }, TICK_DT);
        }
        15.0 - character.position[2]
    };
    assert!(run(0.0) > run(10.0) * 1.1, "{} {}", run(0.0), run(10.0));
}

/// `(tick, position)` skiing `ramp_world`'s slope, mirrored in `predict.spec.ts`.
const SKI_CHECKPOINTS: [(u32, [f32; 3]); 3] = [
    (60, [1.2015, 59.3078, 60.4792]),
    (120, [0.8157, 58.3169, 57.7560]),
    (240, [-0.3172, 55.2653, 49.3722]),
];

/// Skiing `ramp_world`: down the slope turning a little, a jump, a tuck, a snowplow.
pub fn ski_input(seq: u32) -> InputFrame {
    let buttons = if seq == 70 { BUTTON_JUMP } else if (100..180).contains(&seq) { BUTTON_CROUCH } else { 0 };
    InputFrame { seq, strafe: 0, forward: if seq >= 180 { -127 } else { 0 }, yaw: (seq as i16 * 2) % 300, buttons, pitch: 0, view: 0, slot: 0 }
}

#[test]
fn ski_run_matches_fixture() {
    let world = ramp_world();
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 1.3, 61.7);
    for seq in 1..=240 {
        physics.move_character(&mut character, &ski_input(seq), TICK_DT);
        if let Some((_, expected)) = SKI_CHECKPOINTS.iter().find(|(tick, _)| *tick == seq) {
            let close = character.position.iter().zip(expected).all(|(actual, expected)| (actual - expected).abs() < 0.001);
            assert!(close, "tick {seq}: {:?} {:?}", character.position, character);
        }
    }
}

/// `on_skis`, up a ramp of `degrees` rising towards -Z, for six seconds of pushing: how far it got, metres.
fn climbed(degrees: f32, buttons: u16) -> f32 {
    let rise = degrees.to_radians().tan();
    let world = terrain_world(|_, z| (100.0 - z) * rise);
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 0.3, 40.0);
    for seq in 0..360 {
        physics.move_character(&mut character, &InputFrame { seq, forward: 127, buttons, ..Default::default() }, TICK_DT);
    }
    40.0 - character.position[2]
}

#[test]
fn run_held_gets_a_herringbone_up_forty_degrees_and_a_walk_only_up_thirty() {
    assert!(climbed(40.0, BUTTON_RUN) > 3.0, "{}", climbed(40.0, BUTTON_RUN));
    assert!(climbed(28.0, 0) > 3.0, "{}", climbed(28.0, 0));
    assert!(climbed(40.0, 0) < 0.0, "{}", climbed(40.0, 0));
}

/// Ticks in the air coasting at `speed` over a ridge 1.5 m high, its sides 7.5 m long.
fn air_over_ridge(speed: f32) -> u32 {
    let world = terrain_world(|_, z| (1.5 - (z - 30.0).abs() * 0.2).max(0.0));
    let physics = Physics::new(&world).unwrap();
    let mut character = on_skis(&world, 0.3, 50.0);
    character.knock = [0.0, -speed];
    let mut flew = 0;
    for seq in 0..180 {
        physics.move_character(&mut character, &InputFrame { seq, buttons: BUTTON_CROUCH, ..Default::default() }, TICK_DT);
        flew += u32::from(!character.grounded);
    }
    flew
}

#[test]
fn a_bump_at_speed_gives_air_and_at_a_walk_it_does_not() {
    assert!(air_over_ridge(13.0) > 20, "{}", air_over_ridge(13.0));
    assert_eq!(air_over_ridge(4.0), 0);
}

/// A flat world with a track that runs along -Z, then bends 20° towards -X.
fn bent_track() -> WorldFile {
    let mut world = flat_world();
    world.ski.push(shared::world::SkiTrail { width: 400, points: vec![[0, 9_000], [0, 0], [-1_700, -4_700]] });
    world
}

/// Coasting from `start` along -Z at 10 m/s, looking `yaw` radians off, for `ticks`.
fn coast(physics: &Physics, start: [f32; 2], yaw: f32, ticks: u32) -> Character {
    let mut character = Character { position: [start[0], STANDING_Y, start[1]], grounded: true, skis: true, knock: [0.0, -10.0], ..Default::default() };
    let yaw = shared::quant::to_mrad(yaw);
    for seq in 0..ticks {
        physics.move_character(&mut character, &InputFrame { seq, yaw, ..Default::default() }, TICK_DT);
    }
    character
}

#[test]
fn a_groove_draws_a_skier_in_and_carries_them_round_a_bend() {
    let physics = Physics::new(&bent_track()).unwrap();
    // Starting between the grooves, looking straight on the first leg, then down the bend's way is within reach.
    let straight = coast(&physics, [-0.2, 60.0], 0.1, 120);
    assert!((straight.position[0] + 0.8).abs() < 0.1, "{:?}", straight.position);
    let round = coast(&physics, [-0.8, 5.0], 0.2, 150);
    let [x, z] = [round.position[0], round.position[2]];
    let off_centre = (x - z * (17.0 / 47.0)).abs() * (47.0 / 50.0);
    assert!(z < -5.0 && (off_centre - 0.8).abs() < 0.3, "{:?}", round.position);
    assert_eq!(round.down, 0);
}

#[test]
fn turning_well_away_leaves_a_groove() {
    let physics = Physics::new(&bent_track()).unwrap();
    let out = coast(&physics, [-0.8, 60.0], 0.8, 120);
    assert!(out.position[0] < -4.0, "{:?}", out.position);
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

/// A slab over the origin, 1 m off the ground: too low to stand under, high enough to lie under.
fn world_with_low_roof() -> WorldFile {
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let (lo, hi) = ([-1.0, 1.0, -1.0], [1.0, 1.2, 1.0]);
    let vertices = vec![
        v(lo[0], lo[1], lo[2]), v(hi[0], lo[1], lo[2]), v(hi[0], lo[1], hi[2]), v(lo[0], lo[1], hi[2]),
        v(lo[0], hi[1], lo[2]), v(hi[0], hi[1], lo[2]), v(hi[0], hi[1], hi[2]), v(lo[0], hi[1], hi[2]),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat_world() }
}

fn held(buttons: u16, forward: i8) -> InputFrame {
    InputFrame { seq: 1, strafe: 0, forward, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }
}

#[test]
fn lying_down_keeps_the_feet_on_the_ground() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    for _ in 0..30 {
        physics.move_character(&mut character, &held(BUTTON_PRONE, 0), TICK_DT);
    }
    assert!(character.prone && character.grounded);
    assert!((character.position[1] - PRONE_CENTRE).abs() < 0.05, "{:?}", character.position);
}

#[test]
fn letting_go_of_prone_on_open_ground_stands_it_back_up() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    for _ in 0..30 {
        physics.move_character(&mut character, &held(BUTTON_PRONE, 0), TICK_DT);
    }
    assert!(character.prone);
    for _ in 0..30 {
        physics.move_character(&mut character, &held(0, 0), TICK_DT);
    }
    assert!(!character.prone && character.grounded, "{character:?}");
    assert!((character.position[1] - STAND_CENTRE).abs() < 0.05, "{:?}", character.position);
}

#[test]
fn crawling_is_slower_than_walking() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = standing();
    physics.move_character(&mut character, &held(BUTTON_PRONE, 127), TICK_DT);
    let start = character.position[2];
    for _ in 0..TICK_RATE {
        physics.move_character(&mut character, &held(BUTTON_PRONE, 127), TICK_DT);
    }
    assert!(((start - character.position[2]) - PRONE_SPEED).abs() < 0.1, "{:?}", character.position);
}

#[test]
fn getting_up_is_refused_under_a_low_roof() {
    let physics = Physics::new(&world_with_low_roof()).unwrap();
    let mut character = Character { position: [0.0, PRONE_CENTRE, 0.0], grounded: true, prone: true, ..Default::default() };
    for _ in 0..30 {
        physics.move_character(&mut character, &held(0, 0), TICK_DT);
    }
    assert!(character.prone, "stood up into the roof: {:?}", character.position);
    assert!(character.position[1] < 1.0, "rose through the roof: {:?}", character.position);
}

/// A 2 × 2 grid of 100 m tiles where the south-east tile is not part of the world (an L).
fn l_shaped_world() -> WorldFile {
    WorldFile { version: WORLD_VERSION, half_extent: 10_000, tile_size: 10_000, inside: vec![true, true, true, false], ..Default::default() }
}

#[test]
fn the_world_can_be_any_shape() {
    let world = l_shaped_world();
    assert!(world.is_inside(-5_000, -5_000));
    assert!(world.is_inside(5_000, -5_000), "north-east tile");
    assert!(world.is_inside(-5_000, 5_000), "south-west tile");
    assert!(!world.is_inside(5_000, 5_000), "the missing tile");
    assert!(!world.is_inside(10_001, 0), "beyond the square");
}

#[test]
fn walls_follow_the_edge_of_an_l_shaped_world() {
    // Standing in the south-west tile and walking east (+X) reaches the notch's wall at x = 0.
    let physics = Physics::new(&l_shaped_world()).unwrap();
    let mut character = standing();
    character.position = [-50.0, 1.0, 50.0];
    let east = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: -1571, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
    for _ in 0..TICK_RATE * 20 {
        physics.move_character(&mut character, &east, TICK_DT);
    }
    assert!(character.position[0] < 0.0, "walked into the missing tile: {:?}", character.position);
    // Walking east in the north row is free until the outer wall at x = 100.
    let mut character = standing();
    character.position = [-50.0, 1.0, -50.0];
    for _ in 0..TICK_RATE * 20 {
        physics.move_character(&mut character, &east, TICK_DT);
    }
    assert!(character.position[0] > 50.0 && character.position[0] < 100.0, "{:?}", character.position);
}

#[test]
fn one_legged_it_hops_along_slower_than_a_walk_and_cannot_run() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = Character { lost: shared::protocol::LIMB_LEG_L, ..standing() };
    let sprint = InputFrame { seq: 1, forward: 127, buttons: BUTTON_RUN, ..Default::default() };
    let (mut landings, mut was_grounded) = (0, true);
    for _ in 0..2 * TICK_RATE {
        physics.move_character(&mut character, &sprint, TICK_DT);
        if character.grounded && !was_grounded {
            landings += 1;
        }
        was_grounded = character.grounded;
    }
    assert!(landings >= 2, "it hops: landed {landings} times");
    let gone = -character.position[2];
    assert!(gone > 2.0 * WALK_SPEED * HOP_SCALE * 0.8 && gone < 2.0 * WALK_SPEED * 0.7, "{gone} m in 2 s");
    assert_eq!(character.spent, 0, "hopping is not running");
}

#[test]
fn with_no_legs_it_lies_down_for_good_and_crawls() {
    let physics = Physics::new(&flat_world()).unwrap();
    let mut character = Character { lost: shared::protocol::LIMB_LEGS, ..standing() };
    let stand_and_go = InputFrame { seq: 1, forward: 127, buttons: BUTTON_RUN | BUTTON_JUMP, ..Default::default() };
    for _ in 0..TICK_RATE {
        physics.move_character(&mut character, &stand_and_go, TICK_DT);
    }
    assert!(character.prone && character.grounded);
    assert!((-character.position[2] - PRONE_SPEED * LEGLESS_CRAWL_SCALE).abs() < 0.1, "{:?}", character.position);
}

/// A gabled house 6 × 4 m with 3 m walls and a ridge 4.5 m up along x at z = 2, baked into cells with
/// its look kept, as the importer leaves one: its walls and roof in `looks`, only its floor in the
/// collision mesh. Returns the world, its shell and its floor triangles.
fn looked_house() -> (WorldFile, Shell, Vec<Tri>) {
    let face = |corners: &[[i32; 3]]| -> Vec<Tri> { (1..corners.len() - 1).map(|i| { let (a, b, c) = (corners[0], corners[i], corners[i + 1]); [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]] }).collect() };
    let (w, d, h, r) = (600, 400, 300, 450);
    // Wound inwards, as the importer winds them.
    let walls: Vec<Tri> = [
        face(&[[0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]]),
        face(&[[w, 0, d], [0, 0, d], [0, h, d], [w, h, d]]),
        face(&[[0, 0, d], [0, 0, 0], [0, h, 0], [0, r, d / 2], [0, h, d]]),
        face(&[[w, 0, 0], [w, 0, d], [w, h, d], [w, r, d / 2], [w, h, 0]]),
    ]
    .concat();
    let roof: Vec<Tri> = [face(&[[0, h, 0], [0, r, d / 2], [w, r, d / 2], [w, h, 0]]), face(&[[0, r, d / 2], [0, h, d], [w, h, d], [w, r, d / 2]])].concat();
    let floor: Vec<Tri> = face(&[[0, 0, 0], [0, 0, d], [w, 0, d], [w, 0, 0]]);
    let mut look = Look::default();
    for t in walls.iter().chain(&roof) {
        let base = look.vertices.len() as u32;
        look.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
        look.triangles.push([base, base + 1, base + 2]);
    }
    let range = [0, walls.len() as u32, roof.len() as u32];
    let shell = Shell::new(&walls, &roof).unwrap();
    let mut mesh = CollisionMesh::default();
    for t in &floor {
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
        mesh.triangles.push([base, base + 1, base + 2]);
    }
    let building = Building { height: r, footprint: vec![[0, 0], [w, 0], [w, d], [0, d]], base: 0, first: 0, walls: 0, roof: 0, roof_kind: 2, cells: shell.cells(), paint: None };
    let world = WorldFile {
        buildings: vec![building],
        building_ids: vec![ObjId { col: 0, row: 0, index: 0 }],
        building_triangles: vec![[0, floor.len() as u32]],
        looks: vec![BuildingLook { walls: walls.clone(), roof: roof.clone(), windows: shared::windows::openings_of(&look, range), inside: Vec::new() }],
        collision: mesh,
        ..flat_world()
    };
    (world, shell, floor)
}

#[test]
fn a_looked_house_is_collided_with_as_its_own_roof_and_walls_with_the_windows_open() {
    let (world, shell, floor) = looked_house();
    let mut physics = Physics::new(&world).unwrap();
    // Straight down onto the roof halfway up the front slope: the slope itself, not the top of a cell.
    let t = physics.cast_ray([3.0, 10.0, 1.0], [0.0, -1.0, 0.0], 20.0, 0).expect("the roof");
    assert!((10.0 - t - 3.75).abs() < 0.02, "the roof at z = 1 is 3.75 m up, found {}", 10.0 - t);
    // Through a window of the front wall: nothing until the far side of the house.
    let pane = world.looks[0].windows.iter().find(|w| !w.door && w.normal[2] < -0.9).expect("a window on the front wall");
    let [a, b, _, d] = pane.corners;
    let centre = [(a[0] + b[0]) / 2.0, (a[1] + d[1]) / 2.0];
    // (The back wall's window is in line with it, so it may go out through that too and meet nothing.)
    let through = physics.cast_ray([centre[0], centre[1], -5.0], [0.0, 0.0, 1.0], 20.0, 0);
    assert!(through.is_none_or(|t| t > 8.0), "the round went through the window and on, {through:?}");
    // Beside the window, the wall stops it.
    let beside = physics.cast_ray([centre[0] + 0.9, centre[1], -5.0], [0.0, 0.0, 1.0], 20.0, 0).expect("the wall");
    assert!((beside - 5.0).abs() < 0.02, "{beside}");
    // A cell blasted out of the front wall opens it there and nowhere else.
    let count = shell.dims[0] * shell.dims[1] * shell.dims[2];
    let hole = (0..count as u32).filter(|&id| shell.is_solid(id)).min_by_key(|&id| { let c = shell.centre(id); (c[0] - 500).pow(2) + (c[1] - 100).pow(2) + c[2].pow(2) }).unwrap();
    let mut removed = shell.untouched();
    removed[hole as usize] = true;
    physics.replace_building(Obj::Building(ObjId { col: 0, row: 0, index: 0 }), building_solid(&world, 0, Some((&shell, &removed)), &floor));
    physics.step_cars(TICK_DT);
    let c = shell.centre(hole).map(|v| v as f32 / 100.0);
    let open = physics.cast_ray([c[0], c[1], -5.0], [0.0, 0.0, 1.0], 20.0, 0);
    assert!(open.is_none_or(|t| t > 8.0), "through the hole, {open:?}");
    let still = physics.cast_ray([0.3, 0.3, -5.0], [0.0, 0.0, 1.0], 20.0, 0).expect("the wall");
    assert!((still - 5.0).abs() < 0.02, "{still}");
}

/// A house 12 × 8 m with 6 m walls under a flat roof, big enough for rooms (`shared::interior`): two
/// storeys and the stairs between them, its door open. Baked into cells with its look kept, as
/// `looked_house`. Returns the world and every part of its inside, steps and all.
fn furnished_house() -> (WorldFile, Vec<shared::interior::Part>) {
    let face = |corners: &[[i32; 3]]| -> Vec<Tri> { (1..corners.len() - 1).map(|i| { let (a, b, c) = (corners[0], corners[i], corners[i + 1]); [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]] }).collect() };
    let (w, d, h) = (1200, 800, 600);
    let walls: Vec<Tri> = [face(&[[0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]]), face(&[[w, 0, d], [0, 0, d], [0, h, d], [w, h, d]]), face(&[[0, 0, d], [0, 0, 0], [0, h, 0], [0, h, d]]), face(&[[w, 0, 0], [w, 0, d], [w, h, d], [w, h, 0]])].concat();
    let roof: Vec<Tri> = face(&[[0, h, 0], [0, h, d], [w, h, d], [w, h, 0]]);
    let floor: Vec<Tri> = face(&[[0, 0, 0], [0, 0, d], [w, 0, d], [w, 0, 0]]);
    let mut look = Look::default();
    for t in walls.iter().chain(&roof) {
        let base = look.vertices.len() as u32;
        look.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
        look.triangles.push([base, base + 1, base + 2]);
    }
    let windows = shared::windows::openings_of(&look, [0, walls.len() as u32, roof.len() as u32]);
    let footprint = vec![[0, 0], [w, 0], [w, d], [0, d]];
    let parts = shared::interior::rooms(&footprint, &walls, &roof, &windows);
    let mut mesh = CollisionMesh::default();
    for t in &floor {
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
        mesh.triangles.push([base, base + 1, base + 2]);
    }
    let building = Building { height: h, footprint, base: 0, first: 0, walls: 0, roof: 0, roof_kind: 0, cells: Shell::new(&walls, &roof).unwrap().cells(), paint: None };
    let inside = parts.iter().filter(|p| p.collided()).map(|p| p.tri).collect();
    let world = WorldFile {
        buildings: vec![building],
        building_ids: vec![ObjId { col: 0, row: 0, index: 0 }],
        building_triangles: vec![[0, floor.len() as u32]],
        looks: vec![BuildingLook { walls, roof, windows, inside }],
        collision: mesh,
        ..flat_world()
    };
    (world, parts)
}

/// Walks `character` straight towards `[x, z]` for `ticks` ticks, or until it is there.
fn walk_towards(physics: &Physics, character: &mut Character, [x, z]: [f32; 2], ticks: u32) {
    for _ in 0..ticks {
        let (dx, dz) = (x - character.position[0], z - character.position[2]);
        if dx.hypot(dz) < 0.15 {
            break;
        }
        let yaw = shared::quant::to_mrad((-dx).atan2(-dz));
        physics.move_character(character, &InputFrame { seq: 1, forward: 127, yaw, ..Default::default() }, TICK_DT);
    }
}

#[test]
fn a_soldier_walks_in_at_the_door_of_a_house_with_rooms_and_up_its_stairs() {
    let (world, parts) = furnished_house();
    let physics = Physics::new(&world).unwrap();
    // In at the door: from 2 m outside it to 1.2 m inside, up onto the plinth.
    let door = world.looks[0].windows.iter().find(|w| w.door).expect("a door");
    let mid = [(door.corners[0][0] + door.corners[1][0]) / 2.0, (door.corners[0][2] + door.corners[1][2]) / 2.0];
    let out = |by: f32| [mid[0] + door.normal[0] * by, mid[1] + door.normal[2] * by];
    let mut character = Character { position: [out(2.0)[0], STANDING_Y, out(2.0)[1]], grounded: true, ..Default::default() };
    walk_towards(&physics, &mut character, out(-1.2), 4 * TICK_RATE);
    let plinth = shared::interior::PLINTH as f32 / 100.0;
    assert!((character.position[0] - out(-1.2)[0]).hypot(character.position[2] - out(-1.2)[1]) < 0.3, "inside, at {:?}", character.position);
    assert!((character.position[1] - STANDING_Y - plinth).abs() < 0.08, "on the ground floor, at {:?}", character.position);
    // A window is still a hole and the wall beside the door still a wall.
    let beside = physics.cast_ray([out(3.0)[0] + door.normal[2] * 1.2, 1.0, out(3.0)[1] - door.normal[0] * 1.2], [-door.normal[0], 0.0, -door.normal[2]], 6.0, 0).expect("the wall");
    assert!((beside - 3.0).abs() < 0.05, "{beside}");

    // Up the first flight: from the landing at its foot to the floor above.
    let ramp: Vec<[f32; 3]> = parts.iter().filter(|p| p.kind == shared::interior::PART_RAMP).take(2).flat_map(|p| [[p.tri[0], p.tri[1], p.tri[2]], [p.tri[3], p.tri[4], p.tri[5]], [p.tri[6], p.tri[7], p.tri[8]]]).map(|c| c.map(|v| v as f32 / 100.0)).collect();
    let (low, high) = (ramp.iter().map(|p| p[1]).fold(f32::MAX, f32::min), ramp.iter().map(|p| p[1]).fold(f32::MIN, f32::max));
    let end = |y: f32| { let at: Vec<&[f32; 3]> = ramp.iter().filter(|p| (p[1] - y).abs() < 0.01).collect(); [at.iter().map(|p| p[0]).sum::<f32>() / at.len() as f32, at.iter().map(|p| p[2]).sum::<f32>() / at.len() as f32] };
    let (foot, head) = (end(low), end(high));
    let along = [(head[0] - foot[0]) / (head[0] - foot[0]).hypot(head[1] - foot[1]), (head[1] - foot[1]) / (head[0] - foot[0]).hypot(head[1] - foot[1])];
    assert!((low - plinth).abs() < 0.06 && high > 2.7, "from the ground floor to the next, {low} to {high}");
    let mut character = Character { position: [foot[0] - along[0] * 0.5, low + STANDING_Y, foot[1] - along[1] * 0.5], grounded: true, ..Default::default() };
    walk_towards(&physics, &mut character, [head[0] + along[0] * 0.5, head[1] + along[1] * 0.5], 6 * TICK_RATE);
    assert!((character.position[1] - STANDING_Y - high).abs() < 0.08 && character.grounded, "upstairs, at {:?}", character.position);
}

#[test]
fn a_hole_blown_in_a_floor_is_fallen_through_and_the_rest_of_it_holds() {
    let (world, parts) = furnished_house();
    let mut physics = Physics::new(&world).unwrap();
    let (shell, floor) = building_shell(&world, 0).unwrap();
    // A piece of the upper floor, well clear of its edge.
    let upper = parts.iter().filter(|p| p.kind == shared::interior::PART_FLOOR).map(|p| p.tri[1]).max().unwrap();
    let at = [300, upper, 500];
    let down = |physics: &Physics, x: f32, z: f32| 5.0 - physics.cast_ray([x, 5.0, z], [0.0, -1.0, 0.0], 10.0, 0).expect("a floor");
    assert!((down(&physics, 3.0, 5.0) - upper as f32 / 100.0).abs() < 0.02, "the upper floor");
    // The floor's cells are the building's own: one blast takes them, as it takes a wall's.
    let hole = shared::carve::nearest_standing(shell.origin, shell.dims, at, |id| shell.is_solid(id)).expect("the floor is cut into cells");
    assert!((shell.centre(hole)[1] - upper).abs() <= shared::carve::CELL / 2 && (shell.centre(hole)[0] - 300).abs() <= shared::carve::CELL / 2, "{:?}", shell.centre(hole));
    let mut removed = shell.untouched();
    let cut = shell.blast(&mut removed, at, 90);
    assert!(!cut.blasted.is_empty() && cut.fell.is_empty(), "a hole, and nothing hung from it: {cut:?}");
    physics.replace_building(Obj::Building(ObjId { col: 0, row: 0, index: 0 }), building_solid(&world, 0, Some((&shell, &removed)), &floor));
    physics.step_cars(TICK_DT);
    assert!((down(&physics, 3.0, 5.0) - shared::interior::PLINTH as f32 / 100.0).abs() < 0.06, "through the hole to the ground floor, {}", down(&physics, 3.0, 5.0));
    assert!((down(&physics, 9.0, 2.0) - upper as f32 / 100.0).abs() < 0.02 || (down(&physics, 9.0, 6.0) - upper as f32 / 100.0).abs() < 0.02, "the floor stands elsewhere");
}

/// 200 m square of 1 m cells: dry at 0 north of z = -10, then a lake whose bed shelves down at 1 in 5 to
/// 4 m deep, its still surface at 0. East of x = 20 the lake ends in a 3 m bank, steeper than walking climbs.
fn lake_world() -> WorldFile {
    let side = 201u32;
    let bed = |x: f32, z: f32| -> f32 {
        if z > -10.0 || x >= 20.0 {
            0.0
        } else if x >= 19.0 {
            -3.0
        } else {
            -((-z - 10.0) * 0.2).min(4.0)
        }
    };
    let mut heights = Vec::new();
    let mut water = Vec::new();
    for j in 0..side {
        for i in 0..side {
            let ground = bed(i as f32 - 100.0, j as f32 - 100.0);
            heights.push((ground * 100.0).round() as i32);
            water.push(if ground < 0.0 { 0 } else { shared::world::NO_WATER });
        }
    }
    WorldFile { ground: Ground { origin: -10_000, cell: 100, side, heights }, water, ..flat_world() }
}

/// The lake with its waves stilled, for exact heights.
fn calm_lake() -> Physics {
    let mut physics = Physics::new(&lake_world()).unwrap();
    physics.calm_waters();
    physics
}

/// Input facing yaw (radians; 0 faces -Z) and looking `pitch` up, pushing `forward`, with `buttons`.
fn swim_input(forward: i8, yaw: f32, pitch: f32, buttons: u16) -> InputFrame {
    InputFrame { forward, yaw: shared::quant::to_mrad(yaw), pitch: shared::quant::to_mrad(pitch), buttons, ..Default::default() }
}

fn run(physics: &Physics, character: &mut Character, input: &InputFrame, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        physics.move_character(character, input, TICK_DT);
    }
}

/// A swimmer at the float height over deep water at `z`.
fn floating(x: f32, z: f32) -> Character {
    Character { position: [x, -SWIM_FLOAT, z], swimming: true, ..Default::default() }
}

#[test]
fn a_soldier_walking_into_deep_water_starts_swimming_and_floats_with_head_out() {
    let physics = calm_lake();
    let mut character = standing();
    run(&physics, &mut character, &swim_input(127, 0.0, 0.0, 0), 4.0);
    assert!(!character.swimming, "wading in the shallows: {character:?}");
    run(&physics, &mut character, &swim_input(127, 0.0, 0.0, 0), 12.0);
    assert!(character.swimming, "{character:?}");
    assert!(character.position[2] < -30.0, "out over the deep: {character:?}");
    assert!((character.position[1] + SWIM_FLOAT).abs() < 0.05, "at the float height: {character:?}");
    assert!(!physics.submerged(&character), "the head is out");
}

#[test]
fn looking_down_and_swimming_forward_dives_and_space_brings_the_diver_back_up() {
    let physics = calm_lake();
    let mut character = floating(0.0, -60.0);
    run(&physics, &mut character, &swim_input(127, 0.0, -1.1, 0), 1.5);
    assert!(physics.submerged(&character), "{character:?}");
    assert!(character.position[1] < -1.5, "well down: {character:?}");
    run(&physics, &mut character, &swim_input(0, 0.0, 0.0, BUTTON_JUMP), 3.0);
    assert!(!physics.submerged(&character), "{character:?}");
    assert!((character.position[1] + SWIM_FLOAT).abs() < 0.1, "back at the float height: {character:?}");
}

#[test]
fn crouch_sinks_a_swimmer_to_the_bed() {
    let physics = calm_lake();
    let mut character = floating(0.0, -60.0);
    run(&physics, &mut character, &swim_input(0, 0.0, 0.0, BUTTON_CROUCH), 4.0);
    assert!(character.grounded, "on the bottom: {character:?}");
    assert!((character.position[1] - (-4.0 + STANDING_Y)).abs() < 0.1, "{character:?}");
    assert!(character.swimming);
}

#[test]
fn a_swimmer_walks_out_where_the_bank_shallows() {
    let physics = calm_lake();
    let mut character = floating(0.0, -40.0);
    run(&physics, &mut character, &swim_input(127, -std::f32::consts::PI, 0.0, 0), 30.0);
    assert!(!character.swimming, "{character:?}");
    assert!(character.grounded, "{character:?}");
    assert!(character.position[2] > -10.0, "out on the shore: {character:?}");
}

#[test]
fn a_swimmer_climbs_out_over_a_bank_too_steep_to_walk_up() {
    let physics = calm_lake();
    let mut character = floating(15.0, -50.0);
    run(&physics, &mut character, &swim_input(127, -std::f32::consts::FRAC_PI_2, 0.0, 0), 10.0);
    assert!(character.position[0] > 20.5, "up on the bank: {character:?}");
    assert!(!character.swimming, "{character:?}");
}

#[test]
fn falling_into_deep_water_is_braked_before_the_bed_and_floats_up() {
    let physics = calm_lake();
    let mut character = Character { position: [0.0, 12.0, -60.0], ..Default::default() };
    let mut deepest = f32::MAX;
    for _ in 0..10 * TICK_RATE {
        physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
        deepest = deepest.min(character.position[1]);
    }
    assert!(character.swimming, "{character:?}");
    assert!((character.position[1] + SWIM_FLOAT).abs() < 0.1, "floating: {character:?}");
    assert!(deepest < -1.5, "it went in deep: {deepest}");
}

#[test]
fn a_parachute_closes_on_landing_in_water() {
    let physics = calm_lake();
    let mut character = Character { position: [0.0, 6.0, -60.0], chute: true, ..Default::default() };
    run(&physics, &mut character, &InputFrame::default(), 3.0);
    assert!(character.swimming && !character.chute, "{character:?}");
}

#[test]
fn a_swimmer_cannot_lie_down() {
    let physics = calm_lake();
    let mut character = floating(0.0, -60.0);
    run(&physics, &mut character, &swim_input(0, 0.0, 0.0, BUTTON_PRONE), 0.5);
    assert!(!character.prone && character.swimming, "{character:?}");
}

#[test]
fn lying_in_a_shallow_pool_puts_the_face_under() {
    let physics = calm_lake();
    // Half a metre deep, at z = -12.5.
    let mut character = Character { position: [0.0, -0.5 + STANDING_Y, -12.5], grounded: true, ..Default::default() };
    run(&physics, &mut character, &swim_input(0, 0.0, 0.0, BUTTON_PRONE), 0.5);
    assert!(character.prone && !character.swimming, "{character:?}");
    assert!(physics.submerged(&character));
}

#[test]
fn swimming_along_looking_a_little_down_stays_at_the_surface() {
    let physics = calm_lake();
    let mut character = floating(0.0, -60.0);
    for _ in 0..5 * TICK_RATE {
        physics.move_character(&mut character, &swim_input(127, -std::f32::consts::FRAC_PI_2, -0.75, 0), TICK_DT);
        assert!(!physics.submerged(&character), "{character:?}");
    }
    assert!((character.position[1] + SWIM_FLOAT).abs() < 0.05, "{character:?}");
}

#[test]
fn a_dive_goes_well_under_and_letting_go_floats_back_up() {
    let physics = calm_lake();
    let mut character = floating(0.0, -60.0);
    run(&physics, &mut character, &swim_input(127, -std::f32::consts::FRAC_PI_2, -1.1, 0), 1.0);
    assert!(character.position[1] < -1.5, "well under, not just below the surface: {character:?}");
    run(&physics, &mut character, &swim_input(0, 0.0, 0.0, 0), 8.0);
    assert!((character.position[1] + SWIM_FLOAT).abs() < 0.05, "{character:?}");
}

#[test]
fn a_floating_swimmer_bobs_with_the_waves() {
    let mut physics = Physics::new(&lake_world()).unwrap();
    let mut character = floating(0.0, -60.0);
    let (mut low, mut high) = (f32::MAX, f32::MIN);
    for tick in 0..20 * TICK_RATE {
        physics.set_tick(tick);
        physics.move_character(&mut character, &InputFrame::default(), TICK_DT);
        let surface = physics.waters.surface(0.0, -60.0, shared::waves::time(tick)).unwrap().height;
        if tick > 2 * TICK_RATE {
            assert!((character.position[1] - (surface - SWIM_FLOAT)).abs() < 0.12, "riding the surface: {character:?} on {surface}");
            assert!(!physics.submerged(&character), "the head stays out on the swell");
        }
        low = low.min(character.position[1]);
        high = high.max(character.position[1]);
    }
    assert!(high - low > 0.1, "it bobs: {low}..{high}");
}
