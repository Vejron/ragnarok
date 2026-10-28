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
    (240, [-0.4475, 45.1152, 21.4872]),
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
