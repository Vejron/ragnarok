use shared::vec3;
use super::*;
use shared::consts::TICK_DT;
use shared::protocol::{KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PAK, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER};
use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};

fn flat() -> WorldFile {
    WorldFile { version: WORLD_VERSION, half_extent: 20_000, ..Default::default() }
}

/// A 4 m tall box from x = -5..5 at z = -60..-58, straight ahead of a car at the origin.
fn world_with_wall() -> WorldFile {
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![
        v(-5.0, 0.0, -60.0), v(5.0, 0.0, -60.0), v(5.0, 0.0, -58.0), v(-5.0, 0.0, -58.0),
        v(-5.0, 4.0, -60.0), v(5.0, 4.0, -60.0), v(5.0, 4.0, -58.0), v(-5.0, 4.0, -58.0),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat() }
}

/// A `height` m tall slab from x = -8..8 and z = -40..-20, straight ahead of a vehicle at the origin
/// (a kerb, a low wall, a crate): wide and deep enough to drive up onto.
fn world_with_step(height: f32) -> WorldFile {
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![
        v(-8.0, 0.0, -40.0), v(8.0, 0.0, -40.0), v(8.0, 0.0, -20.0), v(-8.0, 0.0, -20.0),
        v(-8.0, height, -40.0), v(8.0, height, -40.0), v(8.0, height, -20.0), v(-8.0, height, -20.0),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat() }
}

fn with_car(world: &WorldFile) -> Physics {
    let mut physics = Physics::new(world).unwrap();
    physics.add_car(1, KIND_JEEP, [0.0, 1.0, 0.0], 0.0);
    physics
}

fn run(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
    for _ in 0..ticks {
        physics.drive_car(1, controls, TICK_DT);
        physics.step_cars(TICK_DT);
    }
    physics.car_pose(1).unwrap()
}

/// Heading of the car's forward axis: 0 faces -Z, positive turns left.
fn yaw(pose: &CarPose) -> f32 {
    pose.yaw()
}

const FORWARD: CarControls = CarControls { throttle: 1.0, ..CarControls::IDLE };
const COAST: CarControls = CarControls::IDLE;

/// A scripted drive of the cars and a tank on flat ground, for the fixture in `vehicle.spec.ts`.
fn scripted_controls(tick: u32) -> CarControls {
    let phase = tick / 60;
    match phase {
        0 => COAST,
        1 | 2 => FORWARD,
        3 | 4 => CarControls { throttle: 1.0, steer: 0.6, ..CarControls::IDLE },
        5 => CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE },
        6 => CarControls { throttle: -1.0, ..CarControls::IDLE },
        _ => CarControls { throttle: 0.5, handbrake: true, ..CarControls::IDLE },
    }
}

/// `(kind, tick, position)` after `scripted_controls`, mirrored in `vehicle.spec.ts`.
const SCRIPTED_CHECKPOINTS: [(u8, u32, [f32; 3]); 16] = [
    (KIND_JEEP, 90, [0.0, 0.8572, -0.9723]),
    (KIND_JEEP, 210, [2.5239, 0.8564, -22.4112]),
    (KIND_JEEP, 300, [30.9153, 0.8560, -23.3722]),
    (KIND_JEEP, 480, [64.3589, 0.8568, -59.5452]),
    (KIND_SPORTS, 90, [-0.0019, 0.3975, -1.9154]),
    (KIND_SPORTS, 210, [4.3494, 0.3961, -44.2191]),
    (KIND_SPORTS, 300, [56.9594, 0.3953, -54.7882]),
    (KIND_SPORTS, 480, [125.6157, 0.3959, -128.6126]),
    (KIND_TANK, 90, [0.0, 0.9400, -0.8340]),
    (KIND_TANK, 210, [0.5739, 0.9400, -17.5632]),
    (KIND_TANK, 300, [11.9063, 0.9400, -29.5214]),
    (KIND_TANK, 480, [26.5903, 0.9400, -40.3629]),
    (KIND_STRV, 90, [0.0000, 1.0798, -1.1357]),
    (KIND_STRV, 210, [0.8855, 1.0800, -23.6921]),
    (KIND_STRV, 300, [18.7068, 1.0800, -35.1709]),
    (KIND_STRV, 480, [47.1117, 1.0799, -41.0568]),
];

#[test]
fn scripted_drive_matches_fixture() {
    for kind in [KIND_JEEP, KIND_SPORTS, KIND_TANK, KIND_STRV] {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
        for tick in 0..480 {
            physics.drive_car(1, scripted_controls(tick), TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, _, expected)) = SCRIPTED_CHECKPOINTS.iter().find(|(k, t, _)| *k == kind && *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "kind {kind} tick {done}: {position:?}");
            }
        }
    }
}

/// A 12° slope rising towards -Z (the way `FORWARD` drives): 5 m cells over 200 m. `vehicle.spec.ts`
/// builds the same heights.
fn ramp() -> WorldFile {
    const SLOPE: f32 = 0.2125566;
    let side = 41u32;
    let heights = (0..side)
        .flat_map(|j| (0..side).map(move |_| j))
        .map(|j| ((100.0 - (j as f32 * 5.0 - 100.0)) * SLOPE * 100.0).round() as i32)
        .collect();
    WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights }, ..flat() }
}

/// `(tick, position)` of a car driven `FORWARD` up `ramp` from [0, _, 40], mirrored in `vehicle.spec.ts`.
const RAMP_CHECKPOINTS: [(u32, [f32; 3]); 3] = [
    (60, [0.0000, 13.8145, 39.1671]),
    (180, [0.0000, 17.2683, 22.8746]),
    (360, [0.0000, 29.7911, -36.0072]),
];

#[test]
fn a_car_climbs_a_slope_like_wasm() {
    let world = ramp();
    let mut physics = Physics::new(&world).unwrap();
    let ground = world.ground.height_at(0.0, 40.0);
    physics.add_car(1, KIND_JEEP, [0.0, ground + 1.5, 40.0], 0.0);
    for tick in 0..360 {
        physics.drive_car(1, FORWARD, TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = RAMP_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {done}: {position:?}");
        }
    }
    let pose = physics.car_pose(1).unwrap();
    let ground = world.ground.height_at(pose.position[0], pose.position[2]);
    assert!(pose.position[2] < 20.0, "it climbs: {pose:?}");
    assert!(pose.position[1] - ground < 1.2 && pose.position[1] - ground > 0.3, "rides on the slope: {pose:?} on {ground}");
    assert!(pose.up_y > 0.9);
}

#[test]
fn ride_heights_differ_between_cars() {
    let mut heights = vec![];
    for (kind, ride) in [(KIND_JEEP, JEEP_RIDE_HEIGHT), (KIND_SPORTS, SPORTS_RIDE_HEIGHT)] {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
        let pose = run(&mut physics, COAST, 240);
        assert!((pose.position[1] - ride).abs() < 0.01, "kind {kind} settles at {ride}: {pose:?}");
        heights.push(pose.position[1] - half_extents(kind)[1]);
    }
    // Ground clearance under the chassis: the jeep clears at least four times what the sports car does.
    assert!(heights[0] > 0.4 && heights[1] < 0.15 && heights[0] > 3.0 * heights[1], "{heights:?}");
}

#[test]
fn the_sports_car_is_quicker_than_the_jeep() {
    let mut reached = vec![];
    for kind in [KIND_JEEP, KIND_SPORTS] {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
        run(&mut physics, COAST, 60);
        let early = run(&mut physics, FORWARD, 60 * 3);
        let late = run(&mut physics, FORWARD, 60 * 12);
        assert!(late.up_y > 0.98 && late.position[0].abs() < 1.0, "kind {kind} drives straight: {late:?}");
        reached.push((early.forward_speed, late.forward_speed));
    }
    let [(jeep_early, jeep_top), (sports_early, sports_top)] = reached[..] else { unreachable!() };
    assert!(sports_early > jeep_early * 1.2, "quicker off the line: {reached:?}");
    assert!(sports_top > jeep_top * 1.3, "higher top speed: {reached:?}");
    assert!(sports_top <= SPORTS_MAX_SPEED + 1.0 && jeep_top <= JEEP_MAX_SPEED + 1.0, "{reached:?}");
}

#[test]
fn settles_on_its_suspension() {
    let mut physics = with_car(&flat());
    let pose = run(&mut physics, COAST, 180);
    assert!(pose.up_y > 0.99, "{pose:?}");
    assert!(pose.position[1] > 0.5 && pose.position[1] < 1.0, "ride height {pose:?}");
    assert!(pose.forward_speed.abs() < 0.1);
}

#[test]
fn accelerates_forward_to_a_sensible_speed() {
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 60);
    let pose = run(&mut physics, FORWARD, 60 * 6);
    assert!(pose.position[2] < -20.0, "should drive towards -Z: {pose:?}");
    assert!(pose.position[0].abs() < 1.0, "no drift sideways: {pose:?}");
    assert!(pose.forward_speed > 8.0 && pose.forward_speed <= JEEP_MAX_SPEED + 1.0, "{pose:?}");
    assert!(pose.up_y > 0.98, "{pose:?}");
}

#[test]
fn reverses() {
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 60);
    let pose = run(&mut physics, CarControls { throttle: -1.0, ..COAST }, 60 * 3);
    assert!(pose.position[2] > 3.0 && pose.forward_speed < -1.0, "{pose:?}");
}

#[test]
fn steering_right_turns_the_heading_clockwise() {
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 60);
    // Short of a half turn, so the yaw doesn't wrap.
    let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 60 * 2);
    assert!(yaw(&pose) < -0.3, "right turn lowers the yaw: {}", yaw(&pose));
    assert!(pose.position[0] > 1.0, "ends up to the right (+X): {pose:?}");
    assert!(pose.steer > 0.0);
}

#[test]
fn steering_left_mirrors_it() {
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 60);
    let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 60 * 2);
    assert!(yaw(&pose) > 0.3 && pose.position[0] < -1.0, "{pose:?}");
}

/// Angle between where the car points and where it is going, radians.
fn slip_angle(pose: &CarPose) -> f32 {
    let heading = pose.yaw();
    let [vx, _, vz] = pose.velocity;
    let travel = (-vx).atan2(-vz);
    let mut angle = (travel - heading).abs();
    if angle > std::f32::consts::PI {
        angle = 2.0 * std::f32::consts::PI - angle;
    }
    angle
}

#[test]
fn the_handbrake_swings_the_tail_out() {
    let mut slips = vec![];
    for handbrake in [false, true] {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        run(&mut physics, FORWARD, 60 * 2);
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, handbrake, ..CarControls::IDLE }, 40);
        assert!(pose.up_y > 0.9, "stays on its wheels: {pose:?}");
        slips.push(slip_angle(&pose));
    }
    assert!(slips[1] > slips[0] + 0.15 && slips[1] > 0.3, "slides with the handbrake: {slips:?}");
}

#[test]
fn a_hard_corner_at_speed_rolls_it_but_a_firm_one_does_not() {
    for kind in [KIND_JEEP, KIND_SPORTS] {
        let mut worst = [1.0_f32; 2];
        for (i, steer) in [0.35_f32, 1.0].into_iter().enumerate() {
            let mut physics = Physics::new(&flat()).unwrap();
            // Back from the middle, so the run-up stays inside the 200 m world.
            physics.add_car(1, kind, [0.0, 1.5, 190.0], 0.0);
            run(&mut physics, COAST, 60);
            let fast = run(&mut physics, FORWARD, 60 * 6);
            assert!(fast.forward_speed > 20.0, "kind {kind}: {fast:?}");
            for _ in 0..60 * 3 {
                let pose = run(&mut physics, CarControls { throttle: 1.0, steer, ..CarControls::IDLE }, 1);
                worst[i] = worst[i].min(pose.up_y);
            }
        }
        assert!(worst[0] > 0.8, "kind {kind} rolls in a firm corner: {worst:?}");
        assert!(worst[1] < 0.3, "kind {kind} stays up at full lock: {worst:?}");
    }
}

#[test]
fn a_wrecked_front_wheel_pulls_the_car_to_its_side() {
    for (wheels, side) in [([220, 0, 0, 0], -1.0), ([0, 220, 0, 0], 1.0)] {
        let mut physics = with_car(&flat());
        physics.set_car_condition(1, CarCondition { power: 1.0, wheels, ..CarCondition::NEW });
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, FORWARD, 60 * 3);
        assert!(pose.position[0] * side > 2.0, "{wheels:?} pulls towards {side}: {pose:?}");
        assert!(pose.up_y > 0.95, "{pose:?}");
    }
    let mut sound = with_car(&flat());
    run(&mut sound, COAST, 60);
    assert!(run(&mut sound, FORWARD, 60 * 3).position[0].abs() < 0.3, "a sound car runs straight");
}

#[test]
fn a_lost_wheel_drops_its_corner_and_drags() {
    let mut sound = with_car(&flat());
    run(&mut sound, COAST, 60);
    let sound = run(&mut sound, FORWARD, 60 * 3);
    let mut physics = with_car(&flat());
    physics.set_car_condition(1, CarCondition { power: 1.0, wheels: [0, 0, 0, WHEEL_GONE], ..CarCondition::NEW });
    let rest = run(&mut physics, COAST, 90);
    // The right rear corner is down: rolled right (the right side lower) and nose up.
    let rotation = Rotation::from_xyzw(rest.rotation[0], rest.rotation[1], rest.rotation[2], rest.rotation[3]);
    let right = rotation * Vector::X;
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    assert!(right.y < -0.05 && nose.y > 0.02, "tilted onto the lost wheel: right {right:?} nose {nose:?}");
    let pose = run(&mut physics, FORWARD, 60 * 3);
    assert!(pose.forward_speed < sound.forward_speed * 0.8, "drags: {} against {}", pose.forward_speed, sound.forward_speed);
}

#[test]
fn a_battered_engine_is_slower() {
    let mut speeds = vec![];
    for health in [100, 10] {
        let mut physics = with_car(&flat());
        physics.set_car_condition(1, CarCondition::of(KIND_JEEP, health, [0; shared::protocol::DAMAGE_SLOTS]));
        run(&mut physics, COAST, 60);
        speeds.push(run(&mut physics, FORWARD, 60 * 2).forward_speed);
    }
    assert!(speeds[1] < speeds[0] * 0.75, "{speeds:?}");
    assert_eq!(CarCondition::of(KIND_JEEP, 60, [0; shared::protocol::DAMAGE_SLOTS]).power, 1.0, "full power down to half health");
}

/// `(tick, position)` of a jeep with a bent front left wheel and no rear right one, driven by
/// `scripted_controls`, mirrored in `vehicle.spec.ts`.
const DAMAGED_CHECKPOINTS: [(u32, [f32; 3]); 3] = [(90, [0.1147, 0.7626, -0.3659]), (300, [9.1750, 0.7638, -15.2648]), (480, [13.0374, 0.7641, -30.9333])];
const DAMAGED_CONDITION: CarCondition = CarCondition { power: 0.8, wheels: [180, 0, 0, WHEEL_GONE], ..CarCondition::NEW };

#[test]
fn a_damaged_drive_matches_fixture() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_JEEP, [0.0, 1.5, 0.0], 0.0);
    physics.set_car_condition(1, DAMAGED_CONDITION);
    for tick in 0..480 {
        physics.drive_car(1, scripted_controls(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = DAMAGED_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {done}: {position:?}");
        }
    }
}

#[test]
fn brakes_to_a_stop_and_coasts_down() {
    let mut physics = with_car(&flat());
    run(&mut physics, FORWARD, 60 * 4);
    let braked = run(&mut physics, CarControls { throttle: -1.0, ..COAST }, 60 * 3);
    assert!(braked.forward_speed.abs() < 1.0 || braked.forward_speed < 0.0, "{braked:?}");
    let mut physics = with_car(&flat());
    let moving = run(&mut physics, FORWARD, 60 * 4);
    let coasted = run(&mut physics, COAST, 60 * 10);
    assert!(coasted.forward_speed < moving.forward_speed * 0.5, "{moving:?} -> {coasted:?}");
}

#[test]
fn a_wall_stops_the_car() {
    let mut physics = with_car(&world_with_wall());
    let pose = run(&mut physics, FORWARD, 60 * 12);
    assert!(pose.position[2] > -58.0 - 0.1, "stopped before the wall: {pose:?}");
    assert!(pose.position[2] < -50.0, "reached the wall: {pose:?}");
}

#[test]
fn spawn_checks_see_walls() {
    let physics = Physics::new(&world_with_wall()).unwrap();
    assert!(physics.car_fits(KIND_JEEP, [0.0, 1.0, 0.0], 0.0));
    assert!(!physics.car_fits(KIND_JEEP, [0.0, 1.0, -58.5], 0.0));
}

#[test]
fn cars_are_solid_to_characters() {
    use crate::{Character, STANDING_Y};
    use shared::protocol::InputFrame;
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 60);
    let mut character = Character { position: [-4.0, STANDING_Y, 0.0], grounded: true, ..Default::default() };
    // Yaw -π/2 faces +X, into the car's flank.
    let toward_car = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: -1571, buttons: 0, pitch: 0, view: 0, slot: 0 };
    for _ in 0..180 {
        physics.move_character(&mut character, &toward_car, TICK_DT);
    }
    let x = character.position[0];
    assert!(x < -JEEP_HALF_EXTENTS[0] - CAPSULE_RADIUS + 0.1 && x > -3.0, "stopped at the car's side: {x}");
    assert!(physics.car_hit(character.position).is_none(), "resting against a car is not a hit");
}

#[test]
fn a_moving_car_overlaps_a_character_in_its_path() {
    use crate::STANDING_Y;
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 60);
    let in_path = [0.0, STANDING_Y, -12.0];
    let mut hit = None;
    for _ in 0..180 {
        run(&mut physics, FORWARD, 1);
        hit = hit.or(physics.car_hit(in_path));
    }
    let hit = hit.expect("the car drives into the spot");
    assert_eq!(hit.car, 1);
    assert!(hit.velocity[2] < -3.0, "{hit:?}");
}

fn with_tank(world: &WorldFile) -> Physics {
    let mut physics = Physics::new(world).unwrap();
    physics.add_car(1, KIND_TANK, [0.0, 1.2, 0.0], 0.0);
    physics
}

#[test]
fn a_tank_settles_and_drives_slower_than_a_car() {
    let mut physics = with_tank(&flat());
    let rest = run(&mut physics, COAST, 180);
    assert!(rest.up_y > 0.99 && rest.position[1] > 0.6 && rest.position[1] < 1.3, "{rest:?}");
    let pose = run(&mut physics, FORWARD, 60 * 8);
    assert!(pose.position[2] < -20.0 && pose.position[0].abs() < 1.0, "{pose:?}");
    assert!(pose.forward_speed > 5.0 && pose.forward_speed <= TANK_MAX_SPEED + 0.5, "{pose:?}");
}

#[test]
fn a_tank_turns_on_the_spot() {
    let mut physics = with_tank(&flat());
    run(&mut physics, COAST, 120);
    let start = physics.car_pose(1).unwrap();
    let right = run(&mut physics, CarControls { steer: 1.0, ..COAST }, 120);
    assert!(yaw(&right) < -0.8, "two seconds of right stick turns clockwise: {}", yaw(&right));
    let moved = (right.position[0] - start.position[0]).hypot(right.position[2] - start.position[2]);
    assert!(moved < 1.5, "pivots instead of driving a circle: {moved} m");
    let back = run(&mut physics, CarControls { steer: -1.0, ..COAST }, 120);
    assert!(yaw(&back) > yaw(&right) + 0.8, "left stick turns back: {} -> {}", yaw(&right), yaw(&back));
    // Let go and it stops turning.
    let held = run(&mut physics, COAST, 90);
    assert!((yaw(&run(&mut physics, COAST, 30)) - yaw(&held)).abs() < 0.05);
}

#[test]
fn a_strv_is_quicker_than_a_tank_and_pivots_faster() {
    let mut tank = with_tank(&flat());
    let mut strv = Physics::new(&flat()).unwrap();
    strv.add_car(1, KIND_STRV, [0.0, 1.2, 0.0], 0.0);
    run(&mut tank, COAST, 120);
    run(&mut strv, COAST, 120);
    let (tank_pose, strv_pose) = (run(&mut tank, FORWARD, 60 * 6), run(&mut strv, FORWARD, 60 * 6));
    assert!(strv_pose.forward_speed > tank_pose.forward_speed + 2.0, "{} vs {}", strv_pose.forward_speed, tank_pose.forward_speed);
    assert!(strv_pose.forward_speed <= STRV_MAX_SPEED + 0.5, "{strv_pose:?}");
    let spin = |physics: &mut Physics| yaw(&run(physics, CarControls { steer: 1.0, ..COAST }, 120));
    let (mut tank, mut strv) = (with_tank(&flat()), Physics::new(&flat()).unwrap());
    strv.add_car(1, KIND_STRV, [0.0, 1.2, 0.0], 0.0);
    run(&mut tank, COAST, 120);
    run(&mut strv, COAST, 120);
    assert!(spin(&mut strv) < spin(&mut tank) - 0.3);
}

/// How far past the slab's near edge (z = -20) a `kind` driven `FORWARD` from the origin gets in 20 s
/// over a `height` m step, in metres. Negative while it is still stuck in front of it.
fn over_the_step(kind: u8, height: f32) -> f32 {
    let mut physics = Physics::new(&world_with_step(height)).unwrap();
    physics.add_car(1, kind, [0.0, 1.2, 0.0], 0.0);
    run(&mut physics, COAST, 120);
    let pose = run(&mut physics, FORWARD, 60 * 20);
    -20.0 - pose.position[2]
}

#[test]
fn a_strv_climbs_a_step_that_stops_a_tank() {
    // Both take a kerb.
    assert!(over_the_step(KIND_TANK, 0.3) > 100.0);
    assert!(over_the_step(KIND_STRV, 0.3) > 100.0);
    // A 0.6 m wall: the bigger wheels and the higher belly get the Strv S over, the tank sits against it.
    assert!(over_the_step(KIND_STRV, 0.6) > 100.0, "{}", over_the_step(KIND_STRV, 0.6));
    assert!(over_the_step(KIND_TANK, 0.6) < 0.0, "{}", over_the_step(KIND_TANK, 0.6));
}

#[test]
fn a_tank_can_turn_and_drive_together() {
    let mut physics = with_tank(&flat());
    run(&mut physics, COAST, 60);
    let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 60 * 2);
    assert!(yaw(&pose) < -1.0 && pose.position[0] > 2.0 && pose.up_y > 0.95, "{pose:?}");
}

fn with_heli(height: f32) -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_HELI, [0.0, height, 0.0], 0.0);
    physics
}

const PILOTED: CarControls = CarControls { powered: true, ..CarControls::IDLE };

/// A scripted flight for the fixture in `vehicle.spec.ts`: hover, climb, lean forward, turn, sink.
fn scripted_flight(tick: u32) -> CarControls {
    match tick / 60 {
        0 => PILOTED,
        1 | 2 => CarControls { lift: 1.0, ..PILOTED },
        3 | 4 => CarControls { throttle: 1.0, ..PILOTED },
        5 | 6 => CarControls { throttle: 1.0, steer: 0.7, ..PILOTED },
        _ => CarControls { throttle: -0.5, steer: -1.0, lift: -1.0, ..PILOTED },
    }
}

/// `(tick, position)` of a helicopter started at `[0, 3, 0]`, mirrored in `vehicle.spec.ts`.
const FLIGHT_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (90, [0.0, 3.6099, 0.0]),
    (210, [0.0, 11.8079, -0.0842]),
    (330, [0.1181, 13.1328, -8.3432]),
    (480, [12.1913, 11.3589, -26.3039]),
];

#[test]
fn scripted_flight_matches_fixture() {
    let mut physics = with_heli(3.0);
    for tick in 0..480 {
        physics.drive_car(1, scripted_flight(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = FLIGHT_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {done}: {position:?}");
        }
    }
}

/// A scripted drone flight for the fixture in `vehicle.spec.ts`: hover, climb, fly ahead and down the
/// look, turn towards a new look while strafing, then back off and sink.
fn scripted_drone(tick: u32) -> CarControls {
    let looking = |yaw: f32, pitch: f32| CarControls { aim: Some([yaw, pitch]), ..PILOTED };
    match tick / 60 {
        0 => looking(0.0, 0.0),
        1 | 2 => CarControls { lift: 1.0, ..looking(0.0, 0.0) },
        3 | 4 => CarControls { throttle: 1.0, ..looking(0.0, -0.2) },
        5 | 6 => CarControls { throttle: 1.0, steer: 0.5, ..looking(1.0, 0.0) },
        _ => CarControls { throttle: -0.5, lift: -1.0, ..looking(1.0, 0.0) },
    }
}

/// `(tick, position)` of a drone started at `[0, 3, 0]`, mirrored in `vehicle.spec.ts`.
const DRONE_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (90, [0.0, 4.8267, 0.0]),
    (210, [0.0, 21.2060, -1.9828]),
    (330, [-0.9514, 18.6062, -42.9446]),
    (480, [-38.1789, 15.5587, -89.1635]),
];

#[test]
fn scripted_drone_matches_fixture() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_DRONE, [0.0, 3.0, 0.0], 0.0);
    for tick in 0..480 {
        physics.drive_car(1, scripted_drone(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = DRONE_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {done}: {position:?}");
        }
    }
}

/// A scripted loitering-drone flight for the fixture in `vehicle.spec.ts`: straight on, a turn
/// towards a new look, a dive down the look, a roll by hand, then a pull up by hand.
fn scripted_loiter(tick: u32) -> CarControls {
    let looking = |yaw: f32, pitch: f32| CarControls { aim: Some([yaw, pitch]), ..PILOTED };
    match tick / 60 {
        0 => looking(0.0, 0.0),
        1 | 2 => looking(1.0, 0.0),
        3 | 4 => looking(1.0, -0.4),
        5 => CarControls { steer: 1.0, ..looking(1.0, 0.0) },
        _ => CarControls { throttle: -1.0, ..looking(1.0, 0.0) },
    }
}

/// Starts a loitering drone at `[0, 60, 100]`, flying towards -Z, as a launch does. It stays clear of
/// the edge of the world, which is a wall.
fn with_loiter() -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_LOITER, [0.0, 60.0, 100.0], 0.0);
    physics.set_car_velocity(1, [0.0, 0.0, -LOITER_SPEED]);
    physics
}

/// `(tick, position)` of `scripted_loiter` from `with_loiter`, mirrored in `vehicle.spec.ts`.
const LOITER_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (90, [-1.9952, 60.7377, 48.0302]),
    (210, [-55.8591, 63.7674, 5.9786]),
    (330, [-109.7632, 38.6259, -29.9292]),
    (480, [-157.6974, 55.4860, -57.6976]),
];

#[test]
fn scripted_loiter_matches_fixture() {
    let mut physics = with_loiter();
    for tick in 0..480 {
        physics.drive_car(1, scripted_loiter(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = LOITER_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {done}: {position:?}");
        }
    }
}

fn speed(pose: &CarPose) -> f32 {
    vec3::length(pose.velocity)
}

#[test]
fn a_loitering_drone_left_alone_flies_straight_and_level() {
    let mut physics = with_loiter();
    let looking = CarControls { aim: Some([0.0, 0.0]), ..PILOTED };
    let pose = run(&mut physics, looking, 60 * 4);
    assert!(pose.position[2] < -10.0 && pose.position[0].abs() < 0.5, "it kept flying ahead: {pose:?}");
    assert!((pose.position[1] - 60.0).abs() < 0.5, "and held its height: {pose:?}");
    assert!((speed(&pose) - LOITER_SPEED).abs() < 0.5, "at its speed: {}", speed(&pose));
}

#[test]
fn a_loitering_drone_banks_into_a_turn_and_rolls_out_on_the_new_heading() {
    let mut physics = with_loiter();
    // Looking a quarter turn left.
    let turning = CarControls { aim: Some([std::f32::consts::FRAC_PI_2, 0.0]), ..PILOTED };
    let early = run(&mut physics, turning, 30);
    // Banked left: the right wingtip is up.
    let [x, y, z, w] = early.rotation;
    let right_y = 2.0 * (x * y + w * z);
    assert!(right_y > 0.3, "banked into the turn: {right_y}");
    let pose = run(&mut physics, turning, 150);
    assert!((yaw(&pose) - std::f32::consts::FRAC_PI_2).abs() < 0.1, "came round: {}", yaw(&pose));
    assert!(pose.up_y > 0.98, "and levelled out: {pose:?}");
    // Facing +PI/2 is heading -X: it flies where its nose points, at its speed.
    assert!(-pose.velocity[0] > LOITER_SPEED - 1.0 && pose.velocity[2].abs() < 1.0, "no slide: {:?}", pose.velocity);
    assert!((pose.position[1] - 60.0).abs() < 8.0, "about level through the turn: {pose:?}");
}

#[test]
fn a_loitering_drone_turns_quicker_than_the_plane() {
    let mut physics = with_loiter();
    // About turn: half a circle in a few seconds.
    let back = CarControls { aim: Some([3.0, 0.0]), ..PILOTED };
    let pose = run(&mut physics, back, 60 * 4);
    assert!(yaw(&pose).abs() > 2.8, "came about: {}", yaw(&pose));
}

#[test]
fn a_loitering_drone_keeps_its_speed_and_never_rolls_over() {
    // Hard turns while diving, pulling up and weaving: the mouse thrown about.
    let mut physics = with_loiter();
    for tick in 0..60 * 8 {
        let yaw = if (tick / 60) % 2 == 0 { 2.5 } else { -2.5 };
        let pitch = if (tick / 90) % 2 == 0 { -1.2 } else { 0.6 };
        let controls = CarControls { aim: Some([yaw, pitch]), ..PILOTED };
        physics.drive_car(1, controls, TICK_DT);
        physics.step_cars(TICK_DT);
        let pose = physics.car_pose(1).unwrap();
        assert!(pose.up_y > 0.0, "tick {tick}: upside down: {pose:?}");
        assert!((speed(&pose) - LOITER_SPEED).abs() < 3.0, "tick {tick}: speed {}", speed(&pose));
        if pose.position[1] < 15.0 {
            physics.reset_car(1, [pose.position[0], 60.0, pose.position[2]], pose.yaw());
            physics.set_car_velocity(1, pose.velocity);
        }
    }
}

#[test]
fn a_loitering_drone_pitches_and_rolls_by_hand() {
    let ahead = CarControls { aim: Some([0.0, 0.0]), ..PILOTED };
    // W pushes the nose down.
    let mut physics = with_loiter();
    let pose = run(&mut physics, CarControls { throttle: 1.0, ..ahead }, 30);
    assert!(pose.velocity[1] < -10.0, "diving: {pose:?}");
    // D rolls right: the right wingtip goes down.
    let mut physics = with_loiter();
    let pose = run(&mut physics, CarControls { steer: 1.0, ..ahead }, 20);
    let [x, y, z, w] = pose.rotation;
    let right_y = 2.0 * (x * y + w * z);
    assert!(right_y < -0.5, "rolled right: {right_y}");
}

#[test]
fn a_drone_with_its_motors_off_falls() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_DRONE, [0.0, 10.0, 0.0], 0.0);
    let pose = run(&mut physics, CarControls::IDLE, 60 * 3);
    assert!(pose.position[1] < 0.5, "{pose:?}");
}

#[test]
fn a_powered_helicopter_hovers() {
    let mut physics = with_heli(10.0);
    let pose = run(&mut physics, PILOTED, 60 * 5);
    assert!((pose.position[1] - 10.0).abs() < 0.2 && pose.up_y > 0.999, "{pose:?}");
    assert!(pose.velocity.iter().all(|v| v.abs() < 0.1), "{pose:?}");
}

#[test]
fn an_unpowered_helicopter_falls_and_rests() {
    let mut physics = with_heli(10.0);
    let pose = run(&mut physics, CarControls::IDLE, 60 * 4);
    assert!(pose.position[1] < 1.2 && pose.velocity[1].abs() < 0.5, "{pose:?}");
}

#[test]
fn it_climbs_and_sinks_on_the_lift_input() {
    let mut physics = with_heli(10.0);
    let up = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 2);
    assert!(up.position[1] > 14.0 && up.velocity[1] > 2.0, "{up:?}");
    // It still has the speed of the climb to shed first.
    let down = run(&mut physics, CarControls { lift: -1.0, ..PILOTED }, 60 * 6);
    assert!(down.position[1] < up.position[1] - 3.0 && down.velocity[1] < -3.0, "{up:?} -> {down:?}");
}

#[test]
fn leaning_forward_flies_forward_at_a_sensible_speed_without_losing_height() {
    let mut physics = with_heli(20.0);
    let pose = run(&mut physics, CarControls { throttle: 1.0, ..PILOTED }, 60 * 8);
    assert!(pose.position[2] < -60.0 && pose.position[0].abs() < 1.0, "flies towards -Z: {pose:?}");
    assert!(pose.forward_speed > 12.0 && pose.forward_speed < 30.0, "{pose:?}");
    assert!((pose.position[1] - 20.0).abs() < 2.0, "holds altitude while leaning: {pose:?}");
    assert!(pose.up_y > 0.85, "{pose:?}");
    let back = run(&mut physics, CarControls { throttle: -1.0, ..PILOTED }, 60 * 3);
    assert!(back.forward_speed < pose.forward_speed - 5.0, "{pose:?} -> {back:?}");
}

#[test]
fn the_stick_turns_it_like_a_tank_clockwise_on_the_right() {
    let mut physics = with_heli(10.0);
    let right = run(&mut physics, CarControls { steer: 1.0, ..PILOTED }, 60 * 2);
    assert!(yaw(&right) < -1.0, "{}", yaw(&right));
    let left = run(&mut physics, CarControls { steer: -1.0, ..PILOTED }, 60 * 4);
    assert!(yaw(&left) > yaw(&right) + 1.0, "{} -> {}", yaw(&right), yaw(&left));
}

#[test]
fn it_levels_out_after_a_knock() {
    let mut physics = with_heli(15.0);
    run(&mut physics, PILOTED, 30);
    physics.set_car_roll(1, 0.8, [0.0, 15.0, 0.0]);
    let pose = run(&mut physics, PILOTED, 60 * 3);
    assert!(pose.up_y > 0.99, "{pose:?}");
}

#[test]
fn the_ceiling_stops_the_climb() {
    let mut physics = with_heli(HELI_CEILING - 30.0);
    let pose = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 20);
    assert!(pose.position[1] > HELI_CEILING - 10.0 && pose.position[1] < HELI_CEILING + 8.0, "{pose:?}");
}

#[test]
fn the_ceiling_is_measured_from_the_ground_under_it() {
    // A plateau 600 m up: the helicopter climbs to its ceiling over it, not to 120 m above y = 0.
    let side = 41u32;
    let plateau = WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights: vec![60_000; (side * side) as usize] }, ..flat() };
    let mut physics = Physics::new(&plateau).unwrap();
    physics.add_car(1, KIND_HELI, [0.0, 610.0, 0.0], 0.0);
    let pose = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 20);
    let above = pose.position[1] - 600.0;
    assert!(above > HELI_CEILING - 10.0 && above < HELI_CEILING + 8.0, "{pose:?}");
}

/// Flat ground six kilometres across, so a plane has room.
fn airfield() -> WorldFile {
    WorldFile { version: WORLD_VERSION, half_extent: 300_000, ..Default::default() }
}

fn with_plane() -> Physics {
    let mut physics = Physics::new(&airfield()).unwrap();
    physics.add_car(1, KIND_PLANE, [0.0, 1.5, 600.0], 0.0);
    physics
}

/// A pilot with the throttle at `lift` (Space +1, Shift -1), the stick pulled `pull` (S is +1, nose
/// up; W is -1) and rolled `roll` (D is +1, right wing down).
fn pilot(lift: f32, pull: f32, roll: f32) -> CarControls {
    CarControls { lift, throttle: -pull, steer: roll, powered: true, ..CarControls::IDLE }
}

/// Takes off on full throttle with a little back stick, climbs to about `height` metres, and lets go.
fn airborne(physics: &mut Physics, height: f32) -> CarPose {
    let mut pose = run(physics, pilot(1.0, 0.3, 0.0), 60);
    while pose.position[1] < height {
        pose = run(physics, pilot(1.0, 0.3, 0.0), 30);
    }
    run(physics, pilot(0.0, 0.0, 0.0), 60 * 6)
}

/// Pitch of the nose, radians: + is up.
fn nose_pitch(pose: &CarPose) -> f32 {
    let [x, y, z, w] = pose.rotation;
    let nose = Rotation::from_xyzw(x, y, z, w) * Vector::new(0.0, 0.0, -1.0);
    nose.y.clamp(-1.0, 1.0).asin()
}

#[test]
fn a_parked_plane_sits_on_its_wheels_and_idles_when_a_pilot_climbs_in() {
    let mut physics = with_plane();
    let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 120);
    assert!(rest.up_y > 0.99 && rest.position[1] > 0.9 && rest.position[1] < 1.4, "{rest:?}");
    assert!(rest.velocity.iter().all(|v| v.abs() < 0.1), "{rest:?}");
    // With the stick released it idles on the ground, creeping at most.
    let idle = run(&mut physics, pilot(0.0, 0.0, 0.0), 120);
    assert!(idle.forward_speed < 3.0 && (600.0 - idle.position[2]) < 4.0, "{idle:?}");
}

#[test]
fn full_throttle_lifts_it_off_within_the_runway() {
    let mut physics = with_plane();
    let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 60);
    let mut ticks = 0;
    let pose = loop {
        let pose = run(&mut physics, pilot(1.0, 0.25, 0.0), 1);
        ticks += 1;
        if pose.position[1] > rest.position[1] + 1.0 || ticks > 600 {
            break pose;
        }
    };
    let run_length = rest.position[2] - pose.position[2];
    assert!(run_length < PLANE_RUNWAY, "off the ground after {run_length} m: {pose:?}");
    assert!(pose.forward_speed > PLANE_STALL_SPEED * 0.9 && pose.position[0].abs() < 0.5, "{pose:?}");
}

#[test]
fn it_cruises_level_and_tops_out_on_full_throttle() {
    let mut physics = with_plane();
    airborne(&mut physics, 40.0);
    let before = run(&mut physics, pilot(0.0, 0.0, 0.0), 60 * 10);
    let cruise = run(&mut physics, pilot(0.0, 0.0, 0.0), 60 * 3);
    assert!((cruise.forward_speed - PLANE_CRUISE_SPEED).abs() < 3.0, "{cruise:?}");
    assert!((cruise.position[1] - before.position[1]).abs() < 1.0 && cruise.up_y > 0.99, "level: {before:?} -> {cruise:?}");
    assert!(cruise.position[0].abs() < 1.0, "straight: {cruise:?}");
    let flat_out = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 20);
    assert!(flat_out.forward_speed > 42.0 && flat_out.forward_speed < 46.0, "{flat_out:?}");
}

#[test]
fn pulled_up_with_the_throttle_cut_it_stalls_drops_its_nose_and_recovers() {
    let mut physics = with_plane();
    airborne(&mut physics, 80.0);
    let (mut slowest, mut dropped) = (f32::MAX, false);
    for _ in 0..60 * 8 {
        let pose = run(&mut physics, pilot(-1.0, 1.0, 0.0), 1);
        slowest = slowest.min(pose.forward_speed);
        // The stick is still back, but the nose falls through the horizon.
        dropped |= slowest < PLANE_STALL_SPEED && nose_pitch(&pose) < -0.2;
    }
    assert!(slowest < PLANE_STALL_SPEED * 0.7, "it stalls: {slowest}");
    assert!(dropped, "and the nose drops");
    let recovered = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 8);
    assert!(recovered.forward_speed > 25.0 && recovered.up_y > 0.9 && recovered.position[1] > 10.0, "{recovered:?}");
}

#[test]
fn s_pulls_the_nose_up_and_w_pushes_it_down_and_letting_go_levels_it() {
    let mut physics = with_plane();
    let level = airborne(&mut physics, 60.0);
    let up = run(&mut physics, pilot(0.0, 1.0, 0.0), 45);
    assert!(nose_pitch(&up) > nose_pitch(&level) + 0.3, "{} -> {}", nose_pitch(&level), nose_pitch(&up));
    // Let go and the nose settles back to the horizon.
    let settled = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 6);
    assert!(nose_pitch(&settled).abs() < 0.1, "{}", nose_pitch(&settled));
    let down = run(&mut physics, pilot(0.0, -1.0, 0.0), 45);
    assert!(nose_pitch(&down) < nose_pitch(&settled) - 0.3, "{} -> {}", nose_pitch(&settled), nose_pitch(&down));
}

#[test]
fn space_and_shift_are_the_throttle() {
    let mut speeds = vec![];
    for lift in [-1.0, 0.0, 1.0] {
        let mut physics = with_plane();
        airborne(&mut physics, 40.0);
        speeds.push(run(&mut physics, pilot(lift, 0.0, 0.0), 60 * 8).forward_speed);
    }
    assert!(speeds[0] < speeds[1] - 5.0 && speeds[1] < speeds[2] - 3.0, "Shift slows, nothing cruises, Space speeds up: {speeds:?}");
}

#[test]
fn rolling_banks_and_turns_it_and_letting_go_levels_the_wings() {
    for (roll, side) in [(1.0, -1.0), (-1.0, 1.0)] {
        let mut physics = with_plane();
        let start = airborne(&mut physics, 50.0);
        // Holding D rolls right, until the bank stops at its limit, and a banked plane turns.
        let banked = run(&mut physics, pilot(1.0, 0.0, roll), 90);
        assert!(banked.up_y < 0.7 && banked.up_y > 0.1, "banked: {banked:?}");
        let turning = run(&mut physics, pilot(1.0, 0.0, roll), 60 * 3);
        assert!(turning.yaw() * side > 0.5 || (turning.yaw() - start.yaw()) * side > 0.5, "turns to the {side} side: {}", turning.yaw());
        let released = run(&mut physics, pilot(1.0, 0.0, 0.0), 60 * 4);
        assert!(released.up_y > 0.97, "wings level again: {released:?}");
    }
}

#[test]
fn the_mouse_turns_it_towards_where_the_pilot_points_banking_into_the_turn() {
    for turn in [1.2_f32, -1.2] {
        let mut physics = with_plane();
        let start = airborne(&mut physics, 60.0);
        let aim = Some([shared::quant::wrap_angle(start.yaw() + turn), 0.0]);
        let mouse = CarControls { aim, ..pilot(0.0, 0.0, 0.0) };
        let banking = run(&mut physics, mouse, 60);
        assert!(banking.up_y < 0.95, "it banks into the turn: {banking:?}");
        let arrived = run(&mut physics, mouse, 60 * 8);
        let error = shared::quant::wrap_angle(arrived.yaw() - shared::quant::wrap_angle(start.yaw() + turn)).abs();
        assert!(error < 0.2, "it points where the mouse does: {error} {arrived:?}");
        assert!(arrived.up_y > 0.97, "and levels its wings there: {arrived:?}");
    }
}

#[test]
fn the_mouse_climbs_and_dives_it() {
    let mut physics = with_plane();
    let start = airborne(&mut physics, 80.0);
    let up = run(&mut physics, CarControls { aim: Some([start.yaw(), 0.4]), ..pilot(1.0, 0.0, 0.0) }, 60 * 4);
    assert!(up.position[1] > start.position[1] + 10.0, "climbs: {start:?} -> {up:?}");
    let down = run(&mut physics, CarControls { aim: Some([up.yaw(), -0.4]), ..pilot(0.0, 0.0, 0.0) }, 60 * 4);
    assert!(down.position[1] < up.position[1] - 10.0, "dives: {up:?} -> {down:?}");
}

#[test]
fn without_a_pilot_it_comes_down() {
    let mut physics = with_plane();
    let flying = airborne(&mut physics, 60.0);
    let falling = run(&mut physics, CarControls::IDLE, 60 * 6);
    assert!(falling.position[1] < flying.position[1] - 10.0, "{flying:?} -> {falling:?}");
}

#[test]
fn the_ceiling_stops_a_climbing_plane() {
    let mut physics = with_plane();
    run(&mut physics, pilot(1.0, 0.4, 0.0), 60 * 60);
    let pose = run(&mut physics, pilot(1.0, 0.4, 0.0), 60 * 20);
    assert!(pose.position[1] < PLANE_CEILING + 10.0 && pose.position[1] > PLANE_CEILING - 40.0, "{pose:?}");
}

#[test]
fn it_turns_back_before_the_edge_of_the_world() {
    // Facing east, the wall 3 km out. The pilot just holds full throttle.
    let mut physics = Physics::new(&airfield()).unwrap();
    physics.add_car(1, KIND_PLANE, [0.0, 1.5, 600.0], -std::f32::consts::FRAC_PI_2);
    airborne(&mut physics, 40.0);
    let mut furthest: f32 = 0.0;
    for _ in 0..60 * 120 {
        let pose = run(&mut physics, pilot(1.0, 0.0, 0.0), 1);
        furthest = furthest.max(pose.position[0]);
        assert!(pose.position[1] > 5.0, "it does not fly into the ground: {pose:?}");
    }
    assert!(furthest > 2900.0 && furthest < 3000.0 - PLANE_WING_HALF_EXTENTS[0] - 1.0, "{furthest}");
}

#[test]
fn the_wings_are_solid_out_to_their_tips() {
    let mut physics = with_plane();
    let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 120);
    let [_, wy, wz] = PLANE_WING_CENTRE;
    let at = [rest.position[0] - 10.0, rest.position[1] + wy, rest.position[2] + wz];
    let hit = physics.cast_ray_hit(at, [1.0, 0.0, 0.0], 20.0, 99).expect("the wing stops it");
    assert!((hit.0 - (10.0 - PLANE_WING_HALF_EXTENTS[0])).abs() < 0.1 && hit.1 == Some(1), "{hit:?}");
}

#[test]
fn the_runway_ends_at_a_wall() {
    let world = world_with_wall();
    let physics = Physics::new(&world).unwrap();
    // The wall is 58 m ahead (-Z) and nothing is behind, all the way to the world's edge.
    let ahead = physics.runway(&world.ground, [0.0, 1.5, 0.0], 0.0, 150.0);
    let behind = physics.runway(&world.ground, [0.0, 1.5, 0.0], std::f32::consts::PI, 150.0);
    assert!((ahead - 58.0).abs() < 0.5, "{ahead}");
    assert_eq!(behind, 150.0);
    let edge = physics.runway(&world.ground, [0.0, 1.5, 0.0], std::f32::consts::PI, 250.0);
    assert!((edge - 200.0).abs() < 1.0, "the boundary wall: {edge}");
    // Past the end of the wall, the wingtip still clips it.
    let beside = physics.runway(&world.ground, [7.0, 1.5, 0.0], 0.0, 150.0);
    assert!((beside - 58.0).abs() < 0.5, "{beside}");
    assert!(physics.runway(&world.ground, [9.5, 1.5, 0.0], 0.0, 150.0) > 99.0);
}

/// A scripted flight for the fixture in `vehicle.spec.ts`: parked, a take-off, a climbing turn,
/// the blip switch in a turn, and a roll on the stick.
fn scripted_plane(tick: u32) -> CarControls {
    match tick / 60 {
        0 => pilot(0.0, 0.0, 0.0),
        1..7 => pilot(1.0, 0.3, 0.0),
        7..9 => pilot(0.0, 0.0, 0.8),
        9 | 10 => pilot(-1.0, 0.5, 0.8),
        _ => pilot(1.0, 0.1, -0.5),
    }
}

/// `(tick, position)` of a plane started at `[0, 1.5, 600]` on the airfield, mirrored in `vehicle.spec.ts`.
const PLANE_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (120, [0.0000, 1.0504, 595.1553]),
    (420, [0.0000, 12.9997, 500.8397]),
    (660, [17.7795, 11.9774, 408.6677]),
    (900, [78.6062, 2.9332, 305.7454]),
];

#[test]
fn scripted_plane_flight_matches_fixture() {
    let mut physics = with_plane();
    for tick in 0..900 {
        physics.drive_car(1, scripted_plane(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = PLANE_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {done}: {position:?}");
        }
    }
}

#[test]
fn rays_see_walls_and_ignore_their_own_vehicle() {
    let mut physics = with_car(&world_with_wall());
    run(&mut physics, COAST, 60);
    let origin = [0.0, 1.5, 0.0];
    // Starting inside the car's own box, the ray still reaches the wall at z = -58.
    let distance = physics.cast_ray(origin, [0.0, 0.0, -1.0], 300.0, 1).expect("the wall is in range");
    assert!((distance - 58.0).abs() < 0.1, "{distance}");
    assert!(physics.cast_ray(origin, [0.0, 0.0, 1.0], 100.0, 1).is_none(), "nothing behind");
    assert!(physics.cast_ray(origin, [0.0, 0.0, -1.0], 30.0, 1).is_none(), "out of range");
    // Aimed at the car from outside, the ray stops at its side.
    let at_car = physics.cast_ray([-10.0, 0.5, 0.0], [1.0, 0.0, 0.0], 50.0, 99).expect("the car is solid to shells");
    assert!((at_car - (10.0 - JEEP_HALF_EXTENTS[0])).abs() < 0.2, "{at_car}");
}

#[test]
fn a_blast_shoves_a_nearby_vehicle_away() {
    let mut physics = with_car(&flat());
    run(&mut physics, COAST, 120);
    physics.blast_cars([-3.0, 0.0, 0.0], EXPLOSION_RADIUS);
    // Measured in the hop it gives: once down, the tyres' side grip soon stops the slide.
    let after = run(&mut physics, COAST, 5);
    assert!(after.velocity[0] > 1.0, "pushed towards +X: {:?}", after.velocity);
    let mut far = with_car(&flat());
    run(&mut far, COAST, 120);
    far.blast_cars([-30.0, 0.0, 0.0], EXPLOSION_RADIUS);
    assert!(run(&mut far, COAST, 20).velocity[0].abs() < 0.2);
}

#[test]
fn a_tanks_turret_is_solid_above_the_hull() {
    let mut physics = with_tank(&flat());
    let rest = run(&mut physics, COAST, 180);
    let [px, py, pz] = TANK_TURRET_PIVOT;
    // Above the hull's top, so only the turret can stop it. The ring sits off the hull's centre.
    let height = rest.position[1] + py;
    let hit = physics.cast_ray_hit([-10.0, height, rest.position[2] + pz], [1.0, 0.0, 0.0], 30.0, 99).expect("the turret is solid");
    assert!((hit.0 - (10.0 + px - TANK_TURRET_RADIUS)).abs() < 0.2, "{hit:?}");
    assert_eq!(hit.1, Some(1));
    // The gun's own tank does not stop its shell.
    assert!(physics.cast_ray([-10.0, height, rest.position[2] + pz], [1.0, 0.0, 0.0], 30.0, 1).is_none());
    // A standing character on the roof of the turret is carried by the tank.
    let on_top = [rest.position[0] + px, rest.position[1] + py + TANK_TURRET_HALF_HEIGHT + crate::STANDING_Y - 0.05, rest.position[2] + pz];
    assert_eq!(physics.car_under(on_top).map(|hit| hit.car), Some(1));
}

fn walker() -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_WALKER, [0.0, 4.9, 0.0], 0.0);
    physics
}

const DRIVEN: CarControls = CarControls { powered: true, ..CarControls::IDLE };

#[test]
fn a_walker_stands_on_its_legs_and_stays_upright() {
    let mut physics = walker();
    let pose = run(&mut physics, COAST, 240);
    let stand = WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1];
    assert!(pose.up_y > 0.99, "upright: {pose:?}");
    assert!((pose.position[1] - stand).abs() < 0.6, "hull rides about {stand} m up: {pose:?}");
    assert!(pose.velocity.iter().all(|v| v.abs() < 0.2), "at rest: {pose:?}");
}

#[test]
fn a_walker_walks_and_turns_under_a_driver() {
    let mut physics = walker();
    run(&mut physics, COAST, 120);
    let pose = run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 420);
    assert!(pose.forward_speed > 0.8 * WALKER_MAX_SPEED, "up to speed: {pose:?}");
    assert!(-pose.position[2] > 15.0, "went forward: {pose:?}");
    assert!(pose.up_y > 0.97, "stays upright walking: {pose:?}");
    let before = pose.yaw();
    let turned = run(&mut physics, CarControls { steer: -1.0, ..DRIVEN }, 120);
    assert!((turned.yaw() - before).abs() > 0.5, "turns: {before} -> {}", turned.yaw());
}

#[test]
fn a_walker_shrugs_off_a_nudge_but_a_close_blast_tips_it_over() {
    let mut physics = walker();
    run(&mut physics, COAST, 120);
    physics.spin_car(1, [0.0, 0.0, 0.4]);
    let pose = run(&mut physics, COAST, 240);
    assert!(pose.up_y > 0.99, "recovers from a nudge: {pose:?}");

    // A shell bursting at its side, a few metres off, is a shove it can stay on its feet through.
    let reached = physics.blast_cars([-9.0, 4.5, 0.0], 12.0);
    assert_eq!(reached.len(), 1);
    let pose = run(&mut physics, COAST, 240);
    assert!(pose.up_y > 0.9, "a far blast does not tip it: {pose:?}");

    // One right beside it goes over, and a fallen walker cannot be driven off.
    let at = physics.car_pose(1).unwrap().position;
    physics.blast_cars([at[0] - 1.2, at[1] - 0.8, at[2]], 12.0);
    let pose = run(&mut physics, COAST, 90);
    assert!(pose.up_y < WALKER_FALLEN_UP, "tipped over: {pose:?}");
    let fallen = run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 30);
    assert!(fallen.forward_speed.abs() < 2.0, "does not walk when down: {fallen:?}");
}

#[test]
fn a_walker_catches_itself_when_it_tips_forwards_or_back() {
    // The lowest `up_y` over `ticks` ticks.
    fn lowest(physics: &mut Physics, controls: CarControls, ticks: u32) -> f32 {
        (0..ticks).map(|_| run(physics, controls, 1).up_y).fold(1.0, f32::min)
    }
    // Standing, shoved hard forwards and then back about its hip axis (x is the hull's right).
    for spin in [1.0_f32, -1.0] {
        let mut physics = walker();
        run(&mut physics, COAST, 120);
        physics.spin_car(1, [spin, 0.0, 0.0]);
        let low = lowest(&mut physics, COAST, 180);
        assert!(low > 0.8, "a {spin} rad/s pitch shove does not take it over: {low}");
        let pose = run(&mut physics, COAST, 180);
        assert!(pose.up_y > 0.99, "and it stands again: {pose:?}");
    }
    // Walking, then pitched forwards as if by something in the way: it stays up.
    let mut physics = walker();
    run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 240);
    physics.spin_car(1, [-1.0, 0.0, 0.0]);
    let low = lowest(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 120);
    assert!(low > 0.8, "pitched forwards at a walk: {low}");
}

#[test]
fn a_walkers_legs_are_solid() {
    let mut physics = walker();
    physics.add_car(2, KIND_TANK, [-14.0, 1.0, 0.0], -std::f32::consts::FRAC_PI_2);
    let tank = CarControls { throttle: 1.0, ..DRIVEN };
    for _ in 0..300 {
        physics.drive_car(1, COAST, TICK_DT);
        physics.drive_car(2, tank, TICK_DT);
        physics.step_cars(TICK_DT);
    }
    // The walker's hull is higher than a tank, so only its legs can have been hit. It is shoved (and
    // the tank may well ride under it), so it is no longer where it stood.
    let walker = physics.car_pose(1).unwrap();
    assert!(walker.position[0] > 1.0, "the walker was shoved: {walker:?}");
}

#[test]
fn a_wrecked_walker_does_not_stand() {
    let mut physics = walker();
    physics.set_car_condition(1, CarCondition { power: 0.5, wheels: [0; 4], ..CarCondition::NEW });
    let pose = run(&mut physics, COAST, 120);
    // It slumps onto the boxes of its legs, which stop short of the ground when it stands.
    assert!(pose.position[1] < WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1] - 0.2, "sagged onto its legs: {pose:?}");
}

/// Same as `walked` in `vehicle.spec.ts`.
fn scripted_walk(tick: u32) -> CarControls {
    match tick / 60 {
        0 => DRIVEN,
        1..=3 => CarControls { throttle: 1.0, ..DRIVEN },
        4 | 5 => CarControls { throttle: 1.0, steer: 0.7, ..DRIVEN },
        _ => CarControls { throttle: -1.0, ..DRIVEN },
    }
}

/// `(tick, position)` of a walker started at [0, 4.9, 0] under `scripted_walk`, mirrored in `vehicle.spec.ts`.
const WALK_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (90, [0.0, 5.2644, -0.5608]),
    (210, [0.0, 5.2194, -14.7150]),
    (330, [3.3595, 5.2467, -32.3642]),
    (480, [9.8452, 5.1868, -36.6408]),
];

#[test]
fn scripted_walk_matches_fixture() {
    let mut physics = walker();
    for tick in 0..480 {
        physics.drive_car(1, scripted_walk(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        if let Some((_, expected)) = WALK_CHECKPOINTS.iter().find(|(t, _)| *t == tick + 1) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
            assert!(close, "tick {}: {position:?}", tick + 1);
        }
    }
}

#[test]
fn a_hitched_mortar_is_pulled_along_behind_the_jeep_until_it_is_let_go() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_JEEP, [0.0, 1.0, 0.0], 0.0);
    // Parked just behind the jeep, facing the same way, with its tongue almost at the tow point.
    physics.add_car(2, KIND_MORTAR, [0.0, 1.0, 4.0], 0.0);
    assert!(physics.tow_reach(1, 2).unwrap() < HITCH_REACH, "the tongue starts in reach");
    physics.hitch(1, 2);
    assert_eq!(physics.towing(1), Some(2));
    assert_eq!(physics.towed_by(2), Some(1));

    let start = physics.car_pose(2).unwrap().position[2];
    for _ in 0..3 * 60 {
        physics.drive_car(1, FORWARD, TICK_DT);
        physics.drive_car(2, COAST, TICK_DT);
        physics.step_cars(TICK_DT);
    }
    let towed = physics.car_pose(2).unwrap().position[2];
    assert!(towed < start - 5.0, "the mortar came along behind: {start} -> {towed}");
    assert!(physics.tow_reach(1, 2).unwrap() < HITCH_REACH, "and it stayed hitched");

    physics.unhitch(2);
    assert_eq!(physics.towing(1), None);
    assert_eq!(physics.towed_by(2), None);
}

#[test]
fn a_pak_is_hitched_by_its_trails_and_travels_muzzle_last() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_JEEP, [0.0, 1.0, 0.0], 0.0);
    // Parked behind the jeep turned round, its trails' eye at the jeep's tow point and its muzzle away.
    physics.add_car(2, KIND_PAK, [0.0, 1.0, 4.0], std::f32::consts::PI);
    assert!(physics.tow_reach(1, 2).unwrap() < HITCH_REACH, "the trails' eye starts in reach");
    physics.hitch(1, 2);

    let start = physics.car_pose(2).unwrap().position[2];
    for _ in 0..3 * 60 {
        physics.drive_car(1, FORWARD, TICK_DT);
        physics.drive_car(2, COAST, TICK_DT);
        physics.step_cars(TICK_DT);
    }
    let pose = physics.car_pose(2).unwrap();
    assert!(pose.position[2] < start - 5.0, "the Pak came along behind: {start} -> {}", pose.position[2]);
    assert!(physics.tow_reach(1, 2).unwrap() < HITCH_REACH, "and it stayed hitched");
    assert!(pose.world_dir([0.0, 0.0, -1.0])[2] > 0.9, "its muzzle still points back, away from the jeep");
}


fn roadster() -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_ROADSTER, [0.0, 1.5, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    physics
}

fn tank(physics: &Physics) -> f32 {
    physics.car_thrust(1).unwrap().0
}

#[test]
fn only_a_roadster_has_thrusters() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_SPORTS, [0.0, 1.5, 0.0], 0.0);
    assert_eq!(physics.car_thrust(1), None);
    assert_eq!(roadster().car_thrust(1), Some((ROADSTER_AIR, 0)));
}

#[test]
fn the_roadster_boosts_past_a_sports_car() {
    let mut sports = Physics::new(&flat()).unwrap();
    sports.add_car(1, KIND_SPORTS, [0.0, 1.5, 0.0], 0.0);
    run(&mut sports, COAST, 60);
    let plain = run(&mut sports, FORWARD, 90).forward_speed;
    let mut physics = roadster();
    let boosted = run(&mut physics, CarControls { boost: true, ..FORWARD }, 90).forward_speed;
    assert!(boosted > plain + 15.0, "boost {boosted} vs {plain}");
    assert_eq!(physics.car_thrust(1).unwrap().1, THRUST_BOOST);
    assert!(tank(&physics) < 0.6, "boosting drains the tank: {}", tank(&physics));
}

#[test]
fn a_hop_clears_a_row_of_cars_and_costs_air() {
    let mut physics = roadster();
    let start = physics.car_pose(1).unwrap().position[1];
    let mut top = start;
    // One tick of jump, then let go: a single hop, no hover.
    run(&mut physics, CarControls { hop: true, ..COAST }, 1);
    assert!((tank(&physics) - (ROADSTER_AIR - ROADSTER_HOP_COST)).abs() < 1e-4);
    for _ in 0..120 {
        top = top.max(run(&mut physics, COAST, 1).position[1]);
    }
    assert!(top > start + 3.0, "hopped to {top} from {start}");
    let landed = physics.car_pose(1).unwrap().position[1];
    assert!((landed - start).abs() < 0.1, "landed at {landed}");
}

#[test]
fn holding_jump_hovers_until_the_tank_runs_dry() {
    let mut physics = roadster();
    let start = physics.car_pose(1).unwrap().position[1];
    let hold = CarControls { hop: true, ..COAST };
    // Up, then holding a slow climb on the lift nozzles.
    let up = run(&mut physics, hold, 120).position[1];
    let later = run(&mut physics, hold, 60).position[1];
    assert!(up > start + 3.0 && later > up - 0.5, "hovering at {up} then {later}");
    assert_eq!(physics.car_thrust(1).unwrap().1, THRUST_LIFT);
    // The tank lasts about four seconds; then it falls and lands.
    run(&mut physics, hold, 60 * 5);
    assert!(tank(&physics) < 0.01);
    let landed = physics.car_pose(1).unwrap().position[1];
    assert!((landed - start).abs() < 0.2, "fell back to {landed}");
}

#[test]
fn the_tank_refills_on_the_ground() {
    let mut physics = roadster();
    run(&mut physics, CarControls { boost: true, ..CarControls::IDLE }, 60);
    let drained = tank(&physics);
    run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 60 * 2);
    assert!(tank(&physics) > drained + 0.3, "refilled from {drained} to {}", tank(&physics));
    physics.reset_car(1, [0.0, 1.5, 0.0], 0.0);
    assert_eq!(tank(&physics), ROADSTER_AIR);
}

/// A scripted Roadster run with every nozzle, for the fixture in `vehicle.spec.ts`.
fn scripted_thrust(tick: u32) -> CarControls {
    match tick / 60 {
        0 => COAST,
        1 => FORWARD,
        2 => CarControls { boost: true, ..FORWARD },
        3 => CarControls { hop: true, steer: 0.5, ..FORWARD },
        _ => CarControls { handbrake: true, ..CarControls::IDLE },
    }
}

/// `(tick, position, tank)` after `scripted_thrust`, mirrored in `vehicle.spec.ts`.
const THRUST_CHECKPOINTS: [(u32, [f32; 3], f32); 4] = [
    (120, [-0.0019, 0.3968, -8.9728], 1.0000),
    (180, [-0.0019, 0.3992, -46.4509], 0.6700),
    (240, [0.8576, 4.5892, -100.0672], 0.2742),
    (420, [13.2106, 0.3972, -179.1133], 0.6742),
];

#[test]
fn scripted_thrust_matches_fixture() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_ROADSTER, [0.0, 1.5, 0.0], 0.0);
    for tick in 0..420 {
        physics.drive_car(1, scripted_thrust(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected, air)) = THRUST_CHECKPOINTS.iter().find(|(t, _, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001) && (tank(&physics) - air).abs() < 0.001;
            assert!(close, "tick {done}: {position:?} tank {}", tank(&physics));
        }
    }
}

fn hen() -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_HEN, [0.0, 5.4, 0.0], 0.0);
    run(&mut physics, DRIVEN, 120);
    physics
}

fn stamina(physics: &Physics) -> f32 {
    physics.car_thrust(1).unwrap().0
}

/// How much higher than it stood the hen got, and how many ticks it had no foot down, over `ticks`
/// ticks of `controls` (then none) out of 700.
fn flight(controls: CarControls, ticks: u32) -> (f32, u32, Physics) {
    let mut physics = hen();
    let start = physics.car_pose(1).unwrap().position[1];
    let (mut top, mut airborne) = (start, 0);
    for tick in 0..700 {
        let pose = run(&mut physics, if tick < ticks { controls } else { DRIVEN }, 1);
        top = top.max(pose.position[1]);
        assert!(pose.up_y > 0.9, "the wings keep it level: {pose:?}");
        if physics.car_thrust(1).unwrap().1 & THRUST_AIR != 0 {
            airborne += 1;
        }
    }
    (top - start, airborne, physics)
}

#[test]
fn a_hen_stands_on_the_walkers_legs_with_full_stamina() {
    let physics = hen();
    let pose = physics.car_pose(1).unwrap();
    assert!(pose.up_y > 0.99 && (pose.position[1] - (WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1])).abs() < 0.6, "{pose:?}");
    assert_eq!(physics.car_thrust(1), Some((HEN_STAMINA, 0)));
}

#[test]
fn a_tap_of_jump_is_one_leap_that_lands_on_its_feet() {
    let (rise, airborne, physics) = flight(CarControls { hop: true, ..DRIVEN }, 1);
    assert!((5.0..7.5).contains(&rise), "leapt {rise} m");
    assert!((100..160).contains(&airborne), "{airborne} ticks in the air");
    let pose = physics.car_pose(1).unwrap();
    assert!(pose.up_y > 0.99 && pose.velocity.iter().all(|v| v.abs() < 0.2), "landed and stood: {pose:?}");
    assert_eq!(stamina(&physics), HEN_STAMINA, "and got its breath back");
}

#[test]
fn holding_jump_flaps_higher_then_glides_down_slowly() {
    let hold = CarControls { hop: true, ..DRIVEN };
    let (leap, leap_air, _) = flight(hold, 1);
    let (rise, airborne, _) = flight(hold, 700);
    assert!(rise > leap + 2.0, "flapping climbs past the leap: {rise} vs {leap}");
    assert!(airborne > leap_air * 2, "and stays up much longer: {airborne} vs {leap_air}");

    let mut physics = hen();
    run(&mut physics, hold, 60);
    assert_eq!(physics.car_thrust(1).unwrap().1, THRUST_AIR | THRUST_LIFT, "flapping");
    run(&mut physics, hold, 200);
    assert_eq!(stamina(&physics), 0.0, "until it runs out of breath");
    let pose = run(&mut physics, hold, 60);
    assert_eq!(physics.car_thrust(1).unwrap().1, THRUST_AIR | THRUST_GLIDE, "then gliding");
    assert!((pose.velocity[1] + HEN_GLIDE_SINK).abs() < 0.3, "at the glide's sink: {pose:?}");
}

#[test]
fn the_wings_carry_it_forward_and_stamina_comes_back_on_the_ground() {
    let (_, _, physics) = flight(CarControls { hop: true, throttle: 1.0, ..DRIVEN }, 420);
    let pose = physics.car_pose(1).unwrap();
    assert!(-pose.position[2] > 60.0, "flew forward: {pose:?}");
    let mut physics = hen();
    run(&mut physics, CarControls { hop: true, ..DRIVEN }, 200);
    let drained = stamina(&physics);
    run(&mut physics, DRIVEN, 300);
    assert!(stamina(&physics) > drained + 0.5, "refilled from {drained} to {}", stamina(&physics));
    physics.reset_car(1, [0.0, 5.4, 0.0], 0.0);
    assert_eq!(stamina(&physics), HEN_STAMINA);
}

#[test]
fn an_empty_or_wrecked_hen_does_not_leap() {
    let mut physics = hen();
    run(&mut physics, CarControls { hop: true, ..CarControls::IDLE }, 30);
    assert!(physics.car_pose(1).unwrap().velocity[1].abs() < 0.2, "nobody aboard");
    physics.set_car_condition(1, CarCondition { power: 0.5, ..CarCondition::NEW });
    run(&mut physics, CarControls { hop: true, ..DRIVEN }, 30);
    assert_eq!(stamina(&physics), HEN_STAMINA, "a wreck");
}

/// A scripted hen run (walk, leap, flap, glide, land), for the fixture in `vehicle.spec.ts`.
fn scripted_hen(tick: u32) -> CarControls {
    match tick / 60 {
        0 | 1 => DRIVEN,
        2 => CarControls { throttle: 1.0, ..DRIVEN },
        3..=7 => CarControls { hop: true, throttle: 1.0, steer: 0.3, ..DRIVEN },
        _ => DRIVEN,
    }
}

/// `(tick, position, stamina)` after `scripted_hen`, mirrored in `vehicle.spec.ts`.
const HEN_CHECKPOINTS: [(u32, [f32; 3], f32); 4] = [
    (180, [0.0000, 5.2071, -2.2903], 1.0000),
    (240, [0.0279, 11.1432, -13.4641], 0.7835),
    (420, [4.6358, 13.9028, -50.7188], 0.0000),
    (600, [20.7118, 5.2639, -82.4246], 0.4608),
];

#[test]
fn scripted_hen_matches_fixture() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_HEN, [0.0, 5.4, 0.0], 0.0);
    for tick in 0..600 {
        physics.drive_car(1, scripted_hen(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected, left)) = HEN_CHECKPOINTS.iter().find(|(t, _, _)| *t == done) {
            let position = physics.car_pose(1).unwrap().position;
            let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001) && (stamina(&physics) - left).abs() < 0.001;
            assert!(close, "tick {done}: {position:?} stamina {}", stamina(&physics));
        }
    }
}

fn frame(buttons: u16) -> shared::protocol::InputFrame {
    shared::protocol::InputFrame { seq: 1, buttons, ..Default::default() }
}

#[test]
fn a_roadster_boosts_on_run_hops_on_jump_and_brakes_on_crouch() {
    let roadster = |buttons| CarControls::driven(KIND_ROADSTER, &frame(buttons));
    assert!(roadster(BUTTON_RUN).boost && !roadster(BUTTON_RUN).hop && !roadster(BUTTON_RUN).handbrake);
    assert!(roadster(BUTTON_JUMP).hop && !roadster(BUTTON_JUMP).boost && roadster(BUTTON_JUMP).lift == 0.0);
    assert!(roadster(BUTTON_CROUCH).handbrake);
    let sports = CarControls::driven(KIND_SPORTS, &frame(BUTTON_JUMP));
    assert!(sports.handbrake && !sports.hop && !sports.boost);
}

#[test]
fn a_helicopter_climbs_on_jump_and_sinks_on_run() {
    assert_eq!(CarControls::driven(KIND_HELI, &frame(BUTTON_JUMP)).lift, 1.0);
    assert_eq!(CarControls::driven(KIND_HELI, &frame(BUTTON_RUN)).lift, -1.0);
    assert_eq!(CarControls::driven(KIND_HELI, &frame(BUTTON_JUMP | BUTTON_RUN)).lift, 0.0);
    assert!(CarControls::driven(KIND_HELI, &frame(0)).powered);
}

#[test]
fn only_planes_and_drones_fly_towards_the_look() {
    let input = shared::protocol::InputFrame { forward: 127, strafe: 127, yaw: 500, pitch: -300, ..Default::default() };
    for kind in [KIND_PLANE, KIND_DRONE, KIND_LOITER] {
        let controls = CarControls::driven(kind, &input);
        assert_eq!(controls.aim, Some([0.5, -0.3]), "kind {kind}");
        assert_eq!((controls.throttle, controls.steer), (1.0, 1.0));
    }
    for kind in [KIND_JEEP, KIND_TANK, KIND_HELI, KIND_HEN] {
        assert_eq!(CarControls::driven(kind, &input).aim, None, "kind {kind}");
    }
}

#[test]
fn a_hen_leaps_on_jump_with_no_handbrake_or_climb() {
    let jump = CarControls::driven(KIND_HEN, &frame(BUTTON_JUMP));
    assert!(jump.hop && !jump.handbrake && !jump.boost && jump.lift == 0.0);
    let run = CarControls::driven(KIND_HEN, &frame(BUTTON_RUN));
    assert!(!run.hop && !run.boost && run.lift == 0.0);
}

#[test]
fn a_car_put_in_a_reported_state_is_in_it() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_ROADSTER, [0.0, 1.0, 0.0], 0.0);
    let motion = CarMotion { linvel: [1.0, -2.0, 3.0], angvel: [0.1, 0.2, -0.3], steer: 0.2, spin: 0.0 };
    let turned = [0.0, 0.38268343, 0.0, 0.9238795];
    physics.set_car_state(1, [5.0, 2.0, -7.0], turned, motion, Some(0.25));
    let pose = physics.car_pose(1).unwrap();
    assert_eq!(pose.position, [5.0, 2.0, -7.0]);
    assert!((pose.rotation[1] - turned[1]).abs() < 1e-6);
    assert_eq!(physics.car_motion(1), Some(motion));
    assert_eq!(physics.car_thrust(1).map(|(tank, _)| tank), Some(0.25));
}

#[test]
fn a_cars_wheels_are_read_off_the_damage_and_its_engine_off_its_health() {
    assert_eq!(CarCondition::of(KIND_JEEP, 100, [9, 9, 9, 9, 1, 2, 3, 4, 0, 0, 0]), CarCondition { power: 1.0, wheels: [1, 2, 3, 4], ..CarCondition::NEW });
    assert!((CarCondition::of(KIND_JEEP, 10, [0; shared::protocol::DAMAGE_SLOTS]).power - 0.6).abs() < 1e-6);
    assert_eq!(CarCondition::of(KIND_TANK, 125, [0; shared::protocol::DAMAGE_SLOTS]).power, 1.0);
}

#[test]
fn a_tanks_turret_flag_is_not_mistaken_for_a_lost_wheel() {
    let damage = [0, 0, 0, 0, shared::protocol::TURRET_GONE, 0, 0, 0, 0, 0, 0];
    assert_eq!(CarCondition::of(KIND_TANK, 250, damage).wheels, [0; 4]);
    // A car's four wheels are in the same slots.
    assert_eq!(CarCondition::of(KIND_JEEP, 100, damage).wheels, [shared::protocol::TURRET_GONE, 0, 0, 0]);
}

#[test]
fn a_tanks_parts_take_its_power_speed_and_turn() {
    use shared::armor::{PARTS, Part, State, pack};
    use shared::protocol::{DAMAGE_PARTS, KIND_TANK};
    let with = |part: Part, state: State| {
        let mut states = [State::Sound; PARTS];
        states[part.slot().unwrap()] = state;
        let [low, high] = pack(states);
        let mut damage = [0; shared::protocol::DAMAGE_SLOTS];
        damage[DAMAGE_PARTS] = low;
        damage[DAMAGE_PARTS + 1] = high;
        CarCondition::of(KIND_TANK, 250, damage)
    };
    assert_eq!(CarCondition::of(KIND_TANK, 250, [0; shared::protocol::DAMAGE_SLOTS]), CarCondition::NEW);
    let hurt = with(Part::Engine, State::Hurt);
    assert!(hurt.power < 1.0 && hurt.speed < 1.0 && hurt.turn == 1.0, "{hurt:?}");
    let dead = with(Part::Engine, State::Gone);
    assert_eq!((dead.power, dead.turn), (0.0, 0.0));
    assert_eq!(with(Part::Transmission, State::Gone).power, 0.0);
    let track = with(Part::TrackRight, State::Gone);
    assert_eq!(track.sides, [1.0, 0.0]);
    assert!(track.power > 0.0 && track.turn < 1.0);
    // The gun's parts do not move it.
    assert_eq!(with(Part::Breech, State::Gone), CarCondition::NEW);
}
