use shared::vec3;
use super::*;
use shared::consts::TICK_DT;
use shared::protocol::{KIND_AIR_AMBULANCE, KIND_AMV, KIND_BUS, KIND_DRONE, KIND_GOLF_CART, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MISSILE, KIND_MORTAR, KIND_PAK, KIND_PLANE, KIND_ROADSTER, KIND_SEAPLANE, KIND_SNOWMOBILE, KIND_SNOWRACER, KIND_SPORTS, KIND_STRF, KIND_STRV, KIND_TANK, KIND_TECHNICAL, KIND_TITAN, KIND_WALKER};
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
    (KIND_SPORTS, 90, [-0.0028, 0.3975, -1.9145]),
    (KIND_SPORTS, 210, [4.3484, 0.3961, -44.2180]),
    (KIND_SPORTS, 300, [56.9584, 0.3953, -54.7872]),
    (KIND_SPORTS, 480, [125.6134, 0.3959, -128.6123]),
    (KIND_TANK, 90, [0.0, 1.1300, -0.9490]),
    (KIND_TANK, 210, [0.5937, 1.1300, -18.9117]),
    (KIND_TANK, 300, [12.0193, 1.1300, -31.0185]),
    (KIND_TANK, 480, [25.7233, 1.1300, -41.9264]),
    (KIND_STRV, 90, [0.0000, 1.2000, -1.1390]),
    (KIND_STRV, 210, [0.8934, 1.2000, -23.7257]),
    (KIND_STRV, 300, [18.5748, 1.2000, -35.2314]),
    (KIND_STRV, 480, [43.6059, 1.2000, -43.3173]),
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
    for (kind, ride) in [(KIND_JEEP, JEEP_RIDE_HEIGHT), (KIND_SPORTS, SPORTS_RIDE_HEIGHT), (KIND_GOLF_CART, GOLF_CART_RIDE_HEIGHT), (KIND_TECHNICAL, TECHNICAL_RIDE_HEIGHT), (KIND_SNOWRACER, SNOWRACER_RIDE_HEIGHT), (KIND_BUS, BUS_RIDE_HEIGHT), (KIND_AMV, AMV_RIDE_HEIGHT)] {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
        let pose = run(&mut physics, COAST, 240);
        assert!((pose.position[1] - ride).abs() < 0.01, "kind {kind} settles at {ride}: {pose:?}");
        heights.push(pose.position[1] - half_extents(kind)[1]);
    }
    // Ground clearance under the chassis: the jeep clears at least four times what the sports car does.
    assert!(heights[0] > 0.4 && heights[1] < 0.15 && heights[0] > 3.0 * heights[1], "{heights:?}");
    // The technical clears more than the jeep, and the AMV nearly as much as the technical.
    assert!(heights[3] > heights[0] + 0.1, "{heights:?}");
    assert!(heights[6] > 0.4, "{heights:?}");
}

#[test]
fn the_sports_car_is_quicker_than_the_jeep() {
    let mut reached = vec![];
    for kind in [KIND_JEEP, KIND_SPORTS] {
        // A kilometre of flat ground: at full throttle for 15 s the cars cover more than `flat()` has.
        let mut physics = Physics::new(&WorldFile { version: WORLD_VERSION, half_extent: 100_000, ..Default::default() }).unwrap();
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
fn a_golf_cart_potters_along_and_stays_on_its_wheels_at_full_lock() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_GOLF_CART, [0.0, 1.5, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let early = run(&mut physics, FORWARD, 60 * 2);
    let top = run(&mut physics, FORWARD, 60 * 8);
    assert!(early.forward_speed > 5.0, "gets going: {early:?}");
    assert!(top.forward_speed > GOLF_CART_MAX_SPEED - 1.5 && top.forward_speed <= GOLF_CART_MAX_SPEED + 0.5, "tops out at its own speed: {top:?}");
    let start = yaw(&top);
    let mut worst = 1.0_f32;
    for _ in 0..60 * 3 {
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 1);
        worst = worst.min(pose.up_y);
    }
    let turned = (yaw(&physics.car_pose(1).unwrap()) - start).rem_euclid(std::f32::consts::TAU);
    assert!(worst > 0.8, "flat out at full lock it stays on its wheels: {worst}");
    assert!(turned > 0.5, "and turns: {turned}");
}

#[test]
fn a_bus_gets_going_slowly_and_stays_on_its_wheels_at_full_lock() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_BUS, [0.0, 1.5, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let early = run(&mut physics, FORWARD, 60 * 3);
    let top = run(&mut physics, FORWARD, 60 * 7);
    assert!(early.forward_speed > 4.0 && early.forward_speed < 10.0, "gets going, but slowly: {early:?}");
    assert!(top.forward_speed > BUS_MAX_SPEED - 1.5 && top.forward_speed <= BUS_MAX_SPEED + 0.5, "tops out at its own speed: {top:?}");
    let start = yaw(&top);
    let mut worst = 1.0_f32;
    for _ in 0..60 * 3 {
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 1);
        worst = worst.min(pose.up_y);
    }
    let turned = (yaw(&physics.car_pose(1).unwrap()) - start).rem_euclid(std::f32::consts::TAU);
    assert!(worst > 0.8, "flat out at full lock it stays on its wheels: {worst}");
    assert!(turned > 0.5, "and turns: {turned}");
}

#[test]
fn an_amv_gets_going_like_a_heavy_truck_and_stays_on_its_eight_wheels_at_full_lock() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_AMV, [0.0, 1.8, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let early = run(&mut physics, FORWARD, 60 * 3);
    let top = run(&mut physics, FORWARD, 60 * 5);
    assert!(early.forward_speed > 8.0 && early.forward_speed < 18.0, "gets going, but not as a car does: {early:?}");
    assert!(top.forward_speed > AMV_MAX_SPEED - 4.0 && top.forward_speed <= AMV_MAX_SPEED + 1.0, "tops out at its own speed: {top:?}");
    let start = yaw(&top);
    let mut worst = 1.0_f32;
    for _ in 0..60 * 3 {
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 1);
        worst = worst.min(pose.up_y);
    }
    let turned = (yaw(&physics.car_pose(1).unwrap()) - start).rem_euclid(std::f32::consts::TAU);
    assert!(worst > 0.8, "flat out at full lock it stays on its wheels: {worst}");
    assert!(turned > 0.5, "and turns: {turned}");
}

/// How tight a circle `kind` drives at full lock and a crawl, metres: its radius, from how far it goes
/// against how far round it turns.
fn turning_radius(kind: u8) -> f32 {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, kind, [0.0, 1.8, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let crawl = CarControls { throttle: 0.3, steer: -1.0, ..CarControls::IDLE };
    let mut pose = run(&mut physics, crawl, 60 * 3);
    let (mut along, mut turned) = (0.0, 0.0);
    for _ in 0..60 * 3 {
        let next = run(&mut physics, crawl, 1);
        along += (next.position[0] - pose.position[0]).hypot(next.position[2] - pose.position[2]);
        turned += (yaw(&next) - yaw(&pose) + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        pose = next;
    }
    along / turned.abs()
}

#[test]
fn an_amv_steers_two_axles_and_turns_tighter_than_a_bus() {
    let def = shared::vehicles::vehicle_def(KIND_AMV);
    assert_eq!(def.spec.steer_axles, 2);
    let (amv, bus) = (turning_radius(KIND_AMV), turning_radius(KIND_BUS));
    // On its front axle alone it went round 11.2 m, as wide as the bus.
    assert!(amv < 10.0 && amv < bus - 1.0, "a tighter circle than the bus's {bus} m: {amv} m");
    assert!(amv > turning_radius(KIND_JEEP), "but wider than a jeep's");
}

#[test]
fn a_technical_gets_going_and_stays_on_its_wheels_at_full_lock() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_TECHNICAL, [0.0, 1.5, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let early = run(&mut physics, FORWARD, 60 * 3);
    let top = run(&mut physics, FORWARD, 60 * 5);
    assert!(early.forward_speed > 10.0, "gets going: {early:?}");
    assert!(top.forward_speed > TECHNICAL_MAX_SPEED - 3.0 && top.forward_speed <= TECHNICAL_MAX_SPEED + 1.0, "tops out at its own speed: {top:?}");
    let start = yaw(&top);
    let mut worst = 1.0_f32;
    for _ in 0..60 * 3 {
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 1);
        worst = worst.min(pose.up_y);
    }
    let turned = (yaw(&physics.car_pose(1).unwrap()) - start).rem_euclid(std::f32::consts::TAU);
    assert!(worst > 0.8, "flat out at full lock it stays on its wheels: {worst}");
    assert!(turned > 0.5, "and turns: {turned}");
}

#[test]
fn a_technical_is_softer_sprung_than_the_jeep() {
    // Each dropped from half a metre over where it rests: the soft springs give further and swing slower.
    let mut dips = vec![];
    for (kind, ride) in [(KIND_JEEP, JEEP_RIDE_HEIGHT), (KIND_TECHNICAL, TECHNICAL_RIDE_HEIGHT)] {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, kind, [0.0, ride + 0.5, 0.0], 0.0);
        let mut lowest = f32::MAX;
        let mut bottomed = 0;
        for tick in 0..120 {
            let pose = run(&mut physics, COAST, 1);
            if pose.position[1] < lowest {
                lowest = pose.position[1];
                bottomed = tick;
            }
        }
        let rest = run(&mut physics, COAST, 240);
        assert!((rest.position[1] - ride).abs() < 0.01 && rest.up_y > 0.99, "kind {kind} settles again: {rest:?}");
        dips.push((ride - lowest, bottomed));
    }
    let [(jeep_dip, jeep_bottomed), (technical_dip, technical_bottomed)] = dips[..] else { unreachable!() };
    assert!(technical_dip > jeep_dip * 1.3, "it sinks further on landing: {dips:?}");
    assert!(technical_bottomed > jeep_bottomed, "and takes longer about it: {dips:?}");
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

/// A line of `kind` across the way of a car at the origin, at z = -20 m, in 3 m pieces.
fn world_with_line(kind: u8) -> WorldFile {
    let points: Vec<[i32; 2]> = (-3..=3).map(|i| [i * 300, -2000]).collect();
    let line = shared::world::Line { kind, base: vec![0; points.len()], points, end: true };
    WorldFile { lines: vec![(0, 0, line)], ..flat() }
}

#[test]
fn a_car_drives_through_a_fence_as_if_it_were_not_there() {
    let mut open = with_car(&flat());
    let mut fenced = with_car(&world_with_line(shared::world::LINE_FENCE));
    for physics in [&mut open, &mut fenced] {
        run(physics, COAST, 60);
    }
    let (free, through) = (run(&mut open, FORWARD, 60 * 5), run(&mut fenced, FORWARD, 60 * 5));
    assert!(through.position[2] < -25.0, "{through:?}");
    assert_eq!(through.position, free.position, "the wheels do not ride up on it either");
    assert!(fenced.car_tree_contacts().is_empty());
}

#[test]
fn a_car_stops_on_a_wall_which_a_tank_goes_through() {
    let world = world_with_line(shared::world::LINE_WALL_HIGH);
    let mut physics = with_car(&world);
    run(&mut physics, COAST, 60);
    let mut touched = false;
    for _ in 0..60 * 5 {
        run(&mut physics, FORWARD, 1);
        touched |= physics.car_tree_contacts().iter().any(|(_, object)| matches!(object, Obj::Line(_)));
    }
    let pose = physics.car_pose(1).unwrap();
    assert!(pose.position[2] > -20.0, "{pose:?}");
    assert!(touched, "the contact names the piece it ran into");
    let mut physics = Physics::new(&world).unwrap();
    physics.add_car(1, KIND_TANK, [0.0, 1.5, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let pose = run(&mut physics, FORWARD, 60 * 8);
    assert!(pose.position[2] < -25.0, "{pose:?}");
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

#[test]
fn a_snowmobiles_steering_has_no_delay() {
    assert_eq!(spec(shared::protocol::KIND_SNOWMOBILE).steer_time, 0.0);
}

#[test]
fn a_snowmobiles_lock_narrows_much_faster_with_speed_than_a_jeeps() {
    let sled = spec(shared::protocol::KIND_SNOWMOBILE);
    let jeep = spec(KIND_JEEP);
    let share = |spec: &Spec, speed: f32| lock(spec, speed) / lock(spec, 0.0);
    assert!(share(sled, 20.0) < 0.3, "{}", share(sled, 20.0));
    assert!(share(sled, 7.0) < 0.55 && share(sled, 7.0) > 0.45);
    assert!((share(jeep, 20.0) - 0.5).abs() < 1e-3);
    assert!(lock(sled, 0.0) > lock(jeep, 0.0));
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
const DAMAGED_CHECKPOINTS: [(u32, [f32; 3]); 3] = [(90, [0.1301, 0.7640, -0.3042]), (300, [8.7144, 0.7641, -14.3911]), (480, [12.9401, 0.7384, -28.2222])];
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
    let toward_car = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: -1571, buttons: 0, pitch: 0, view: 0, slot: 0, collective: 0 };
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

fn with_strf(world: &WorldFile) -> Physics {
    let mut physics = Physics::new(world).unwrap();
    physics.add_car(1, KIND_STRF, [0.0, 1.3, 0.0], 0.0);
    physics
}

#[test]
fn a_strf_settles_at_its_ride_height() {
    let mut physics = with_strf(&flat());
    let rest = run(&mut physics, COAST, 240);
    assert!(rest.up_y > 0.99 && (rest.position[1] - STRF_RIDE_HEIGHT).abs() < 0.01, "settles at {STRF_RIDE_HEIGHT}: {rest:?}");
}

#[test]
fn a_strf_outruns_the_tank_and_stays_upright_pivoting_flat_out() {
    let (mut tank, mut strf) = (with_tank(&flat()), with_strf(&flat()));
    run(&mut tank, COAST, 120);
    run(&mut strf, COAST, 120);
    let (tank_pose, strf_pose) = (run(&mut tank, FORWARD, 60 * 6), run(&mut strf, FORWARD, 60 * 6));
    assert!(strf_pose.forward_speed > tank_pose.forward_speed + 3.0, "{} vs {}", strf_pose.forward_speed, tank_pose.forward_speed);
    assert!(strf_pose.forward_speed <= STRF_MAX_SPEED + 0.5 && strf_pose.position[0].abs() < 1.0, "{strf_pose:?}");
    // Full stick over at speed: it slews round without rolling over.
    let turned = run(&mut strf, CarControls { steer: 1.0, ..FORWARD }, 60);
    assert!(turned.up_y > 0.9, "stays upright: {turned:?}");
    assert!(yaw(&turned) < -0.8, "turns right: {}", yaw(&turned));
}

fn with_pbv(world: &WorldFile) -> Physics {
    let mut physics = Physics::new(world).unwrap();
    physics.add_car(1, KIND_PBV, [0.0, 1.3, 0.0], 0.0);
    physics
}

#[test]
fn a_pbv_settles_at_its_ride_height() {
    let mut physics = with_pbv(&flat());
    let rest = run(&mut physics, COAST, 240);
    assert!(rest.up_y > 0.99 && (rest.position[1] - PBV_RIDE_HEIGHT).abs() < 0.01, "settles at {PBV_RIDE_HEIGHT}: {rest:?}");
}

#[test]
fn a_pbv_outruns_the_tank_but_not_the_strf_and_stays_upright_pivoting_flat_out() {
    let (mut tank, mut strf, mut pbv) = (with_tank(&flat()), with_strf(&flat()), with_pbv(&flat()));
    for physics in [&mut tank, &mut strf, &mut pbv] {
        run(physics, COAST, 120);
    }
    let (tank_pose, strf_pose, pbv_pose) = (run(&mut tank, FORWARD, 60 * 6), run(&mut strf, FORWARD, 60 * 6), run(&mut pbv, FORWARD, 60 * 6));
    assert!(pbv_pose.forward_speed > tank_pose.forward_speed + 3.0 && pbv_pose.forward_speed < strf_pose.forward_speed, "{} between {} and {}", pbv_pose.forward_speed, tank_pose.forward_speed, strf_pose.forward_speed);
    assert!(pbv_pose.forward_speed <= PBV_MAX_SPEED + 0.5 && pbv_pose.position[0].abs() < 1.0, "{pbv_pose:?}");
    // Full stick over at speed: it slews round without rolling over.
    let turned = run(&mut pbv, CarControls { steer: 1.0, ..FORWARD }, 60);
    assert!(turned.up_y > 0.9, "stays upright: {turned:?}");
    assert!(yaw(&turned) < -0.8, "turns right: {}", yaw(&turned));
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
fn tracked_vehicles_climb_a_step_and_the_strv_and_strf_a_taller_one() {
    // All three take a kerb, and a 0.8 m wall, riding up it on the plate under their nose.
    for kind in [KIND_TANK, KIND_STRV, KIND_STRF, KIND_PBV] {
        assert!(over_the_step(kind, 0.3) > 100.0, "kind {kind}");
        assert!(over_the_step(kind, 0.8) > 100.0, "kind {kind}: {}", over_the_step(kind, 0.8));
    }
    // 1.1 m: the higher noses of the Strv S and the Strf 90 get them over, the tank sits against it.
    assert!(over_the_step(KIND_STRV, 1.1) > 100.0, "{}", over_the_step(KIND_STRV, 1.1));
    assert!(over_the_step(KIND_STRF, 1.1) > 100.0, "{}", over_the_step(KIND_STRF, 1.1));
    assert!(over_the_step(KIND_TANK, 1.1) < 0.0, "{}", over_the_step(KIND_TANK, 1.1));
}

/// The hardest contact a `kind` meets when it is sent at `speed` m/s under a light throttle, nose first
/// or (`reversing`) tail first, at a `height` m kerb 20 m away and on over it, and how it ends up.
fn onto_a_kerb(kind: u8, height: f32, reversing: bool, speed: f32) -> (Option<CarImpact>, CarPose) {
    let mut physics = Physics::new(&world_with_step(height)).unwrap();
    physics.add_car(1, kind, [0.0, 1.2, 0.0], if reversing { std::f32::consts::PI } else { 0.0 });
    run(&mut physics, COAST, 120);
    physics.set_car_velocity(1, [0.0, 0.0, -speed]);
    let controls = CarControls { throttle: if reversing { -0.3 } else { 0.3 }, ..CarControls::IDLE };
    let mut hardest = None::<CarImpact>;
    // Until it is well up on the kerb, or has had the time to be.
    for _ in 0..300 {
        if physics.car_pose(1).unwrap().position[2] < -24.5 {
            break;
        }
        physics.drive_car(1, controls, TICK_DT);
        physics.step_cars(TICK_DT);
        for impact in physics.car_impacts() {
            if hardest.is_none_or(|known| impact.dv > known.dv) {
                hardest = Some(impact);
            }
        }
    }
    (hardest, physics.car_pose(1).unwrap())
}

#[test]
fn a_low_car_rides_up_a_kerb_forwards_and_in_reverse() {
    // The kerbs are well over what a sports car or a golf cart clears (0.1 m), and under its bumper.
    for kind in [KIND_SPORTS, KIND_ROADSTER, KIND_GOLF_CART] {
        for height in [0.2, 0.3] {
            for reversing in [false, true] {
                let (hardest, pose) = onto_a_kerb(kind, height, reversing, 8.0);
                assert!(pose.position[2] < -24.5 && pose.up_y > 0.8, "kind {kind} gets up onto a {height} m kerb: {pose:?}");
                assert!(speed(&pose) > 2.0, "kind {kind} is not stopped by a {height} m kerb: {}", speed(&pose));
                // What the kerb does to it is a knock from below, not a hit that stops it.
                let hardest = hardest.unwrap();
                assert!(hardest.dv < 5.0 && hardest.dv - hardest.lift < 1.0, "kind {kind}, {height} m: {hardest:?}");
            }
        }
    }
    // A kerb as high as its bumper is a wall to it.
    let (hardest, pose) = onto_a_kerb(KIND_SPORTS, 0.5, false, 8.0);
    assert!(pose.position[2] > -20.0, "{pose:?}");
    assert!(hardest.unwrap().dv - hardest.unwrap().lift > 6.0, "{hardest:?}");
}

#[test]
fn a_landing_pushes_a_car_up_and_a_wall_does_not() {
    // Dropped six metres, it bottoms out on its belly: all of the blow is from below.
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_SPORTS, [0.0, 7.0, 0.0], 0.0);
    let mut landing = None::<CarImpact>;
    for _ in 0..180 {
        physics.step_cars(TICK_DT);
        landing = physics.car_impacts().into_iter().chain(landing).max_by(|a, b| a.dv.total_cmp(&b.dv));
    }
    let landing = landing.unwrap();
    assert!(landing.dv > 6.0 && landing.lift > landing.dv * 0.95, "{landing:?}");
    // Driven into a wall, almost none of it is.
    let mut physics = Physics::new(&world_with_wall()).unwrap();
    physics.add_car(1, KIND_SPORTS, [0.0, 1.0, 0.0], 0.0);
    run(&mut physics, COAST, 120);
    physics.set_car_velocity(1, [0.0, 0.0, -20.0]);
    let mut crash = None::<CarImpact>;
    for _ in 0..240 {
        physics.step_cars(TICK_DT);
        crash = physics.car_impacts().into_iter().chain(crash).max_by(|a, b| a.dv.total_cmp(&b.dv));
    }
    let crash = crash.unwrap();
    assert!(crash.dv > 15.0 && crash.lift < crash.dv * 0.1, "{crash:?}");
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

/// A scripted flight for the fixture in `vehicle.spec.ts`, in the wind: spin up, lift off on full
/// collective, tip the nose down and fly ahead, turn towards a look to the right while rolling left, then
/// pull the nose up and sink.
fn scripted_flight(tick: u32) -> CarControls {
    let flying = |lift: f32, look: f32| CarControls { lift, aim: Some([look, 0.0]), ..PILOTED };
    match tick / 30 {
        0..=3 => flying(0.0, 0.0),
        4..=7 => flying(1.0, 0.0),
        8 => CarControls { throttle: 0.6, ..flying(0.7, 0.0) },
        9..=11 => flying(0.7, 0.0),
        12 => CarControls { steer: -0.5, ..flying(0.6, -1.0) },
        13 => flying(0.6, -1.0),
        14 => CarControls { throttle: -0.6, ..flying(0.4, -1.0) },
        _ => flying(0.4, -1.0),
    }
}

/// `(tick, position)` of a helicopter started at `[0, 3, 0]` in the wind, mirrored in `vehicle.spec.ts`.
const FLIGHT_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (90, [0.0717, 0.7999, 0.0029]),
    (210, [0.9357, 3.1621, 0.0317]),
    (330, [6.8222, 15.8349, -0.7983]),
    (480, [18.4246, 24.7133, -10.4861]),
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

/// `(tick, position)` of an air ambulance flown the same script from `[0, 3, 0]` in the wind, mirrored
/// in `vehicle.spec.ts`.
const AMBULANCE_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
    (90, [0.0502, 1.0999, 0.0014]),
    (210, [0.8792, 2.8737, 0.0216]),
    (330, [6.4967, 12.0760, -0.8170]),
    (480, [17.4981, 15.9223, -10.5615]),
];

#[test]
fn scripted_ambulance_flight_matches_fixture() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_AIR_AMBULANCE, [0.0, 3.0, 0.0], 0.0);
    for tick in 0..480 {
        physics.drive_car(1, scripted_flight(tick), TICK_DT);
        physics.step_cars(TICK_DT);
        let done = tick + 1;
        if let Some((_, expected)) = AMBULANCE_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
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
fn a_missile_fired_from_a_tube_climbs_steeply_then_pitches_over_to_the_look_at_its_speed() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_MISSILE, [0.0, 5.0, 100.0], 0.0);
    physics.set_car_dive(1, [0.0, 5.0, 100.0], 0.0, 1.55, SUB_LAUNCH_SPEED);
    // Looking level, a little to the left.
    let looking = CarControls { aim: Some([0.3, 0.0]), ..PILOTED };
    // Slow off the tube, it climbs almost straight up for the first couple of seconds.
    let early = run(&mut physics, looking, 60 * 3 / 2);
    let nose = early.world_dir([0.0, 0.0, -1.0]);
    assert!(nose[1] > 0.85, "still climbing steeply: nose {nose:?}");
    assert!(speed(&early) < MISSILE_SPEED * 0.6, "and still gathering speed: {}", speed(&early));
    let pose = run(&mut physics, looking, 60 * 3);
    let nose = pose.world_dir([0.0, 0.0, -1.0]);
    assert!(nose[1].abs() < 0.3, "then it pitched over: nose {nose:?}");
    assert!(pose.position[1] > 60.0, "after climbing out: {pose:?}");
    assert!(pose.up_y > 0.5, "the right way up: {pose:?}");
    assert!((speed(&pose) - MISSILE_SPEED).abs() < 5.0, "at its speed: {}", speed(&pose));
    // It heads where its flier looks: -Z, turned a little towards -X.
    let heading = (-nose[0]).atan2(-nose[2]);
    assert!((heading - 0.3).abs() < 0.25, "towards the look: {heading}");
}

#[test]
fn a_missile_pitching_over_towards_a_look_to_its_side_keeps_its_wings_about_level() {
    // Looked about, all round: it skids round with its nose rather than rolling onto its side.
    for aim in [[1.5, 0.0], [-2.5, 0.0], [1.0, 0.5], [0.3, -0.8]] {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, KIND_MISSILE, [0.0, 5.0, 100.0], 0.0);
        physics.set_car_dive(1, [0.0, 5.0, 100.0], 0.0, 1.55, SUB_LAUNCH_SPEED);
        let looking = CarControls { aim: Some(aim), ..PILOTED };
        for tick in 0..60 * 4 {
            let pose = run(&mut physics, looking, 1);
            let right = pose.world_dir([1.0, 0.0, 0.0]);
            assert!(right[1].abs() < 0.3, "{aim:?}, tick {tick}: rolled onto its side: right {right:?}");
        }
    }
}

#[test]
fn a_missile_at_speed_comes_about_to_a_look_behind_it_flat() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_MISSILE, [0.0, 60.0, 100.0], 0.0);
    physics.set_car_velocity(1, [0.0, 0.0, -MISSILE_SPEED]);
    // Straight behind it.
    let back = CarControls { aim: Some([std::f32::consts::PI, 0.0]), ..PILOTED };
    for _ in 0..60 * 3 {
        let pose = run(&mut physics, back, 1);
        assert!(pose.up_y > 0.7, "it skids round the right way up: {pose:?}");
    }
    let pose = run(&mut physics, back, 0);
    assert!(yaw(&pose).abs() > 2.9, "came about: {}", yaw(&pose));
    assert!((pose.position[1] - 60.0).abs() < 10.0, "about level: {pose:?}");
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

/// A helicopter in the air over the airfield at `height` with its rotor at governed speed, flying ahead
/// (towards -Z) at `speed`, in still air.
fn flying_heli(height: f32, speed: f32) -> Physics {
    let mut physics = Physics::new(&airfield()).unwrap();
    physics.calm_wind();
    physics.add_car(1, KIND_HELI, [0.0, height, 0.0], 0.0);
    let motion = CarMotion { linvel: [0.0, 0.0, -speed], angvel: [0.0; 3], steer: 0.0, spin: 0.0 };
    physics.set_car_state(1, [0.0, height, 0.0], [0.0, 0.0, 0.0, 1.0], motion, Some(1.0));
    physics
}

/// A pilot holding the collective lever at `lever`, looking straight ahead, hands off the stick.
fn lever(lever: f32) -> CarControls {
    CarControls { lift: lever, aim: Some([0.0, 0.0]), ..PILOTED }
}

/// The helicopter's nose-up pitch and right-side-up roll, radians.
fn attitude(pose: &CarPose) -> (f32, f32) {
    let rotation = Rotation::from_xyzw(pose.rotation[0], pose.rotation[1], pose.rotation[2], pose.rotation[3]);
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let right = rotation * Vector::X;
    (nose.y.clamp(-1.0, 1.0).asin(), right.y.clamp(-1.0, 1.0).asin())
}

/// A test pilot: works the stick to hold the helicopter at `pitch` (+ nose up) and `roll` (+ right side
/// up), radians, on top of `controls`, as a person would by eye.
fn holding(physics: &Physics, controls: CarControls, pitch: f32, roll: f32) -> CarControls {
    let (pitch_now, roll_now) = attitude(&physics.car_pose(1).unwrap());
    let angvel = physics.car_motion(1).unwrap().angvel;
    let pose = physics.car_pose(1).unwrap();
    let rotation = Rotation::from_xyzw(pose.rotation[0], pose.rotation[1], pose.rotation[2], pose.rotation[3]);
    let local = rotation.inverse() * Vector::from(angvel);
    // The stick asks for a rate: towards the wanted lean, less the rate it already has.
    let throttle = (-((pitch - pitch_now) * 3.0 - local.x * 0.5) / 0.6).clamp(-1.0, 1.0);
    let steer = (-((roll - roll_now) * 3.0 - local.z * 0.5) / 0.9).clamp(-1.0, 1.0);
    CarControls { throttle, steer, ..controls }
}

/// Flies `ticks` with `pilot` deciding each tick's controls from the physics.
fn fly_by(physics: &mut Physics, ticks: u32, pilot: impl Fn(&Physics) -> CarControls) -> CarPose {
    for _ in 0..ticks {
        let controls = pilot(physics);
        physics.drive_car(1, controls, TICK_DT);
        physics.step_cars(TICK_DT);
    }
    physics.car_pose(1).unwrap()
}

/// Flies `ticks` holding the lever at `at` and the hull level.
fn hover(physics: &mut Physics, at: f32, ticks: u32) -> CarPose {
    fly_by(physics, ticks, |physics| holding(physics, lever(at), 0.0, 0.0))
}

/// The helicopter's rotor speed (share of governed) and its `THRUST_*` flags.
fn rotor(physics: &Physics) -> (f32, u8) {
    physics.car_thrust(1).unwrap()
}

#[test]
fn a_stopped_rotor_spins_up_before_it_lifts_off() {
    let mut physics = with_heli(1.0);
    assert_eq!(rotor(&physics).0, 0.0, "it starts with its rotor stopped");
    let early = run(&mut physics, lever(1.0), 60);
    assert!(early.position[1] < 1.0 && rotor(&physics).0 < 0.5, "{early:?} {:?}", rotor(&physics));
    let late = fly_by(&mut physics, 60 * 4, |physics| holding(physics, lever(1.0), 0.0, 0.0));
    assert!(late.position[1] > 3.0 && rotor(&physics).0 > 0.95, "{late:?} {:?}", rotor(&physics));
}

#[test]
fn the_hover_lever_holds_its_height_out_of_ground_effect() {
    let mut physics = flying_heli(40.0, 0.0);
    let pose = hover(&mut physics, HELI_HOVER_LEVER, 60 * 10);
    assert!((pose.position[1] - 40.0).abs() < 1.5 && pose.velocity[1].abs() < 0.2, "{pose:?}");
    assert!(rotor(&physics).0 > 0.99, "{:?}", rotor(&physics));
}

#[test]
fn the_tail_rotor_drifts_it_right_in_a_level_hover_and_a_little_bank_holds_it() {
    let mut level = flying_heli(40.0, 0.0);
    let drifted = hover(&mut level, HELI_HOVER_LEVER, 60 * 5);
    assert!(drifted.position[0] > 1.0, "translating tendency: {drifted:?}");
    let mut banked = flying_heli(40.0, 0.0);
    let held = fly_by(&mut banked, 60 * 5, |physics| holding(physics, lever(HELI_HOVER_LEVER), 0.0, 0.03));
    assert!(held.position[0].abs() < drifted.position[0] * 0.5, "{held:?} vs {drifted:?}");
}

#[test]
fn the_ground_holds_up_a_rotor_close_over_it_on_less_lever() {
    let low = HELI_HOVER_LEVER - 0.05;
    let mut high = flying_heli(40.0, 0.0);
    let aloft = hover(&mut high, low, 60 * 6);
    assert!(aloft.position[1] < 38.0, "out of ground effect it sinks: {aloft:?}");
    let mut near = flying_heli(2.5, 0.0);
    let cushion = hover(&mut near, low, 60 * 6);
    assert!(cushion.position[1] > 1.2 && cushion.velocity[1].abs() < 0.3, "the cushion holds it off the ground: {cushion:?}");
}

#[test]
fn an_unpowered_helicopter_falls_and_rests() {
    let mut physics = with_heli(10.0);
    let pose = run(&mut physics, CarControls::IDLE, 60 * 4);
    assert!(pose.position[1] < 1.2 && pose.velocity[1].abs() < 0.5, "{pose:?}");
}

#[test]
fn with_nobody_at_the_controls_the_rotor_spins_down() {
    let mut physics = flying_heli(0.8, 0.0);
    run(&mut physics, CarControls::IDLE, 60 * 5);
    let slowing = rotor(&physics).0;
    run(&mut physics, CarControls::IDLE, 60 * 10);
    assert!(slowing < 0.7 && rotor(&physics).0 == 0.0, "{slowing} then {:?}", rotor(&physics));
}

#[test]
fn full_collective_climbs_at_over_ten_metres_a_second() {
    let mut physics = flying_heli(20.0, 0.0);
    let pose = hover(&mut physics, 1.0, 60 * 6);
    assert!(pose.velocity[1] > 10.0 && pose.velocity[1] < 15.0, "inflow limits the climb: {pose:?}");
    assert!(rotor(&physics).0 > 0.97, "the engine has the power for it: {:?}", rotor(&physics));
}

#[test]
fn lowering_the_lever_sinks_it_at_a_steady_rate() {
    let mut physics = flying_heli(80.0, 0.0);
    let pose = hover(&mut physics, 0.4, 60 * 6);
    assert!(pose.velocity[1] < -2.0 && pose.velocity[1] > -6.0, "{pose:?}");
}

#[test]
fn nose_down_it_tops_out_near_fifty_metres_a_second_and_nose_up_slows_it() {
    let mut physics = flying_heli(40.0, 0.0);
    let pose = fly_by(&mut physics, 60 * 24, |physics| holding(physics, lever(0.7), -0.45, 0.0));
    assert!(pose.position[2] < -800.0, "flies towards -Z: {pose:?}");
    assert!(pose.forward_speed > 45.0 && pose.forward_speed < 60.0, "{pose:?}");
    assert!((pose.position[1] - 40.0).abs() < 40.0 && rotor(&physics).0 > 0.95, "{pose:?} {:?}", rotor(&physics));
    let back = fly_by(&mut physics, 60 * 4, |physics| holding(physics, lever(0.4), 0.35, 0.0));
    assert!(back.forward_speed < pose.forward_speed - 15.0, "pulling back flares it: {pose:?} -> {back:?}");
}

#[test]
fn a_nose_down_dash_takes_more_collective_than_a_hover() {
    let mut physics = flying_heli(60.0, 40.0);
    let pose = fly_by(&mut physics, 60 * 4, |physics| holding(physics, lever(HELI_HOVER_LEVER), -0.35, 0.0));
    assert!(pose.velocity[1] < -1.5, "it sinks on the hover lever: {pose:?}");
}

#[test]
fn clean_air_lifts_it_on_the_same_lever() {
    let mut still = flying_heli(40.0, 0.0);
    let hovering = hover(&mut still, HELI_HOVER_LEVER, 60 * 3);
    let mut moving = flying_heli(40.0, 25.0);
    let flying = hover(&mut moving, HELI_HOVER_LEVER, 60 * 3);
    assert!(flying.position[1] > hovering.position[1] + 1.5, "{hovering:?} {flying:?}");
}

#[test]
fn let_go_at_speed_flapback_lifts_the_nose() {
    let mut physics = flying_heli(60.0, 40.0);
    let pose = run(&mut physics, lever(0.6), 60);
    assert!(attitude(&pose).0 > 0.05, "{:?}", attitude(&pose));
}

#[test]
fn through_translational_lift_the_transverse_flow_rolls_it_right() {
    let mut physics = flying_heli(60.0, 0.6 * HELI_ETL_SPEED);
    let pose = run(&mut physics, lever(HELI_HOVER_LEVER), 45);
    assert!(attitude(&pose).1 < -0.03, "{:?}", attitude(&pose));
}

#[test]
fn past_its_blade_stall_speed_it_rolls_left_and_pitches_up() {
    let mut physics = flying_heli(80.0, 68.0);
    let pose = run(&mut physics, lever(0.6), 30);
    let (pitch, roll) = attitude(&pose);
    assert!(pitch > 0.1 && roll > 0.05, "{pitch} {roll}");
}

#[test]
fn hovering_it_turns_its_nose_towards_the_look() {
    let mut physics = flying_heli(40.0, 0.0);
    let pose = fly_by(&mut physics, 60 * 2, |physics| holding(physics, CarControls { aim: Some([-1.5, 0.0]), ..lever(HELI_HOVER_LEVER) }, 0.0, 0.03));
    assert!((yaw(&pose) + 1.5).abs() < 0.1, "{}", yaw(&pose));
    assert!(pose.position[0].abs() < 2.0 && pose.position[2].abs() < 2.0, "turns on the spot: {pose:?}");
}

#[test]
fn a_turn_at_speed_banks_and_keeps_the_nose_on_the_flight_path() {
    let mut physics = flying_heli(40.0, 30.0);
    let look_right = |physics: &Physics| {
        let controls = CarControls { aim: Some([-1.5, 0.0]), ..lever(0.6) };
        // The pilot holds the nose a little down; the bank is the pedals' and the turn's.
        CarControls { steer: 0.0, ..holding(physics, controls, -0.15, 0.0) }
    };
    let banking = fly_by(&mut physics, 60 * 2, look_right);
    assert!(banking.up_y < 0.85, "banks into the turn: {banking:?}");
    let pose = fly_by(&mut physics, 60 * 5, look_right);
    let track = (-pose.velocity[0]).atan2(-pose.velocity[2]);
    assert!(yaw(&pose) < -1.0 && pose.position[0] > 20.0, "turned right: {pose:?}");
    assert!(shared::quant::wrap_angle(yaw(&pose) - track).abs() < 0.15, "nose on the path: {} vs {track}", yaw(&pose));
}

#[test]
fn let_go_the_stick_leaves_it_at_the_bank_it_was_given() {
    let mut physics = flying_heli(40.0, 0.0);
    run(&mut physics, CarControls { steer: 1.0, ..lever(0.55) }, 30);
    let let_go = run(&mut physics, lever(0.55), 60 * 2);
    let (_, roll) = attitude(&let_go);
    assert!(roll < -0.2, "still banked right: {roll}");
    assert!(let_go.velocity[0] > 2.0, "and sliding right: {let_go:?}");
    let level = hover(&mut physics, 0.55, 60 * 2);
    assert!(attitude(&level).1.abs() < 0.05, "the pilot levels it: {:?}", attitude(&level));
}

#[test]
fn a_knock_is_damped_out_but_the_pilot_levels_it() {
    let mut physics = flying_heli(40.0, 0.0);
    run(&mut physics, lever(HELI_HOVER_LEVER), 30);
    physics.set_car_roll(1, 0.5, [0.0, 40.0, 0.0]);
    let hands_off = run(&mut physics, lever(HELI_HOVER_LEVER), 60);
    let spin = vec3::length(physics.car_motion(1).unwrap().angvel);
    assert!(spin < 0.1 && attitude(&hands_off).1 > 0.3, "stops turning, still tilted: {spin} {:?}", attitude(&hands_off));
    let level = hover(&mut physics, HELI_HOVER_LEVER, 60 * 2);
    assert!(level.up_y > 0.99, "{level:?}");
}

#[test]
fn asking_more_than_a_hurt_engine_gives_droops_the_rotor() {
    let mut physics = flying_heli(40.0, 0.0);
    physics.set_car_condition(1, CarCondition { power: 0.4, ..CarCondition::NEW });
    hover(&mut physics, 1.0, 60 * 3);
    let (speed, flags) = rotor(&physics);
    assert!(speed < 0.9 && flags & THRUST_LOW_ROTOR != 0, "{speed} {flags}");
}

#[test]
fn with_the_engine_dead_it_autorotates_and_a_flare_sets_it_down() {
    let mut physics = flying_heli(150.0, 20.0);
    physics.set_car_condition(1, CarCondition { power: 0.0, ..CarCondition::NEW });
    let mut slowest = 1.0f32;
    let mut fastest_descent = 0.0f32;
    let mut touchdown = None;
    for _ in 0..60 * 30 {
        let pose = physics.car_pose(1).unwrap();
        let height = pose.position[1] - 0.8;
        // Lever down and the nose a little down to keep its speed; near the ground the nose up to flare
        // off the speed, then level and the lever up to cushion the landing.
        let controls = if height > 25.0 {
            holding(&physics, lever(0.1), -0.08, 0.0)
        } else if height > 6.0 {
            holding(&physics, lever(0.1), 0.35, 0.0)
        } else {
            holding(&physics, lever(1.0), 0.0, 0.0)
        };
        if height > 25.0 {
            slowest = slowest.min(rotor(&physics).0);
            fastest_descent = fastest_descent.max(-pose.velocity[1]);
        }
        let after = run(&mut physics, controls, 1);
        if after.position[1] < 0.85 {
            touchdown = Some(pose);
            break;
        }
    }
    assert!(slowest > 0.85, "the upflow keeps the rotor turning: {slowest}");
    // Hardest just after the engine stops, before the air coming up through the disc holds it.
    assert!(fastest_descent < 18.0, "{fastest_descent}");
    let touchdown = touchdown.expect("it lands");
    assert!(touchdown.velocity[1] > -4.0 && touchdown.up_y > 0.9, "{touchdown:?}");
}

#[test]
fn sinking_fast_into_its_own_downwash_loses_lift_and_flying_ahead_escapes_it() {
    let mut still = flying_heli(80.0, 0.0);
    let vertical = hover(&mut still, 0.0, 60 * 3);
    let mut moving = flying_heli(80.0, 20.0);
    let ahead = hover(&mut moving, 0.0, 60 * 3);
    assert!(vertical.velocity[1] < ahead.velocity[1] - 3.0, "{vertical:?} {ahead:?}");
}

#[test]
fn hovering_level_in_the_wind_it_drifts_downwind() {
    let mut physics = flying_heli(40.0, 0.0);
    let mut windy = Physics::new(&airfield()).unwrap();
    windy.add_car(1, KIND_HELI, [0.0, 40.0, 0.0], 0.0);
    let still = CarMotion { linvel: [0.0; 3], angvel: [0.0; 3], steer: 0.0, spin: 0.0 };
    windy.set_car_state(1, [0.0, 40.0, 0.0], [0.0, 0.0, 0.0, 1.0], still, Some(1.0));
    let calm = hover(&mut physics, HELI_HOVER_LEVER, 60 * 8);
    let blown = hover(&mut windy, HELI_HOVER_LEVER, 60 * 8);
    let [wx, wz] = shared::wind::wind(0);
    let along = |pose: &CarPose| pose.position[0] * wx + pose.position[2] * wz;
    assert!(along(&blown) > along(&calm) + 3.0, "{calm:?} {blown:?}");
}

#[test]
fn the_ceiling_stops_the_climb() {
    let mut physics = flying_heli(HELI_CEILING - 30.0, 0.0);
    let pose = hover(&mut physics, 1.0, 60 * 20);
    // A full-collective climb carries it a little past before the drooping rotor gives it up.
    assert!(pose.position[1] > HELI_CEILING - 10.0 && pose.position[1] < HELI_CEILING + 15.0, "{pose:?}");
    assert!(rotor(&physics).1 & THRUST_LOW_ROTOR != 0, "the engine has run out of air: {:?}", rotor(&physics));
}

#[test]
fn the_ceiling_is_measured_from_the_ground_under_it() {
    // A plateau 600 m up: the helicopter climbs to its ceiling over it, not to 120 m above y = 0.
    let side = 41u32;
    let plateau = WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights: vec![60_000; (side * side) as usize] }, ..flat() };
    let mut physics = Physics::new(&plateau).unwrap();
    physics.calm_wind();
    physics.add_car(1, KIND_HELI, [0.0, 610.0, 0.0], 0.0);
    let still = CarMotion { linvel: [0.0; 3], angvel: [0.0; 3], steer: 0.0, spin: 0.0 };
    physics.set_car_state(1, [0.0, 610.0, 0.0], [0.0, 0.0, 0.0, 1.0], still, Some(1.0));
    // A little bank against the tail rotor's drift keeps it over the plateau.
    let pose = fly_by(&mut physics, 60 * 20, |physics| holding(physics, lever(1.0), 0.0, 0.03));
    let above = pose.position[1] - 600.0;
    assert!(above > HELI_CEILING - 10.0 && above < HELI_CEILING + 15.0, "{pose:?}");
}

/// An air ambulance `height` metres up over the airfield in still air, its rotor governed, flying
/// ahead at `speed` m/s.
fn flying_ambulance(height: f32, speed: f32) -> Physics {
    let mut physics = Physics::new(&airfield()).unwrap();
    physics.calm_wind();
    physics.add_car(1, KIND_AIR_AMBULANCE, [0.0, height, 0.0], 0.0);
    let motion = CarMotion { linvel: [0.0, 0.0, -speed], angvel: [0.0; 3], steer: 0.0, spin: 0.0 };
    physics.set_car_state(1, [0.0, height, 0.0], [0.0, 0.0, 0.0, 1.0], motion, Some(1.0));
    physics
}

#[test]
fn an_air_ambulance_stands_level_on_its_skids_with_its_tail_off_the_ground() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_AIR_AMBULANCE, [0.0, 1.3, 0.0], 0.0);
    let pose = run(&mut physics, CarControls::IDLE, 60 * 3);
    assert!((pose.position[1] - AIR_AMBULANCE_HALF_EXTENTS[1]).abs() < 0.05 && pose.up_y > 0.999, "{pose:?}");
}

#[test]
fn an_air_ambulance_spins_up_before_it_lifts_off() {
    let mut physics = Physics::new(&airfield()).unwrap();
    physics.calm_wind();
    physics.add_car(1, KIND_AIR_AMBULANCE, [0.0, 1.2, 0.0], 0.0);
    let early = run(&mut physics, lever(1.0), 60);
    assert!(early.position[1] < 1.3 && rotor(&physics).0 < 0.5, "{early:?}");
    let late = fly_by(&mut physics, 60 * 6, |physics| holding(physics, lever(1.0), 0.0, 0.0));
    assert!(late.position[1] > 5.0 && late.up_y > 0.99 && rotor(&physics).0 > 0.95, "{late:?}");
}

#[test]
fn the_air_ambulances_hover_lever_holds_it_out_of_ground_effect() {
    let mut physics = flying_ambulance(40.0, 0.0);
    let pose = hover(&mut physics, AIR_AMBULANCE_HOVER_LEVER, 60 * 10);
    assert!((pose.position[1] - 40.0).abs() < 1.5 && pose.velocity[1].abs() < 0.2, "{pose:?}");
    assert!(rotor(&physics).0 > 0.99, "{:?}", rotor(&physics));
}

#[test]
fn an_air_ambulance_climbs_at_about_eight_metres_a_second() {
    let mut physics = flying_ambulance(20.0, 0.0);
    let pose = hover(&mut physics, 1.0, 60 * 6);
    assert!(pose.velocity[1] > 7.0 && pose.velocity[1] < 9.0, "{pose:?}");
    assert!(rotor(&physics).0 > 0.97, "{:?}", rotor(&physics));
}

#[test]
fn an_air_ambulance_outruns_the_gunship_at_about_sixty_two_metres_a_second() {
    let mut physics = flying_ambulance(50.0, 0.0);
    let pose = fly_by(&mut physics, 60 * 40, |physics| {
        // The pilot works the lever to hold fifty metres.
        let pose = physics.car_pose(1).unwrap();
        let at = 0.78 + (50.0 - pose.position[1]) * 0.02 - pose.velocity[1] * 0.04;
        holding(physics, lever(at.clamp(0.0, 1.0)), -0.39, 0.0)
    });
    // Over the ground: the nose is well down, so it is more than the speed along the nose.
    let speed = pose.velocity[0].hypot(pose.velocity[2]);
    assert!(speed > 59.0 && speed < 66.0, "{pose:?}");
    let mut gunship = flying_heli(50.0, 0.0);
    let slower = fly_by(&mut gunship, 60 * 24, |physics| holding(physics, lever(0.7), -0.45, 0.0));
    assert!(speed > slower.velocity[0].hypot(slower.velocity[2]) + 5.0, "{pose:?} against {slower:?}");
    assert!((pose.position[1] - 50.0).abs() < 12.0 && rotor(&physics).0 > 0.95, "level, with power to spare: {pose:?} {:?}", rotor(&physics));
}

#[test]
fn an_air_ambulance_with_its_engines_dead_autorotates_and_a_flare_sets_it_down() {
    let mut physics = flying_ambulance(150.0, 20.0);
    physics.set_car_condition(1, CarCondition { power: 0.0, ..CarCondition::NEW });
    let mut slowest = 1.0f32;
    let mut touchdown = None;
    for _ in 0..60 * 40 {
        let pose = physics.car_pose(1).unwrap();
        let height = pose.position[1] - AIR_AMBULANCE_HALF_EXTENTS[1];
        // All the way down with the lever: it sinks slower than the gunship for the same lever, and needs
        // the air coming up through the disc.
        let controls = if height > 25.0 {
            holding(&physics, lever(0.0), -0.08, 0.0)
        } else if height > 6.0 {
            holding(&physics, lever(0.0), 0.35, 0.0)
        } else {
            holding(&physics, lever(1.0), 0.0, 0.0)
        };
        if height > 25.0 {
            slowest = slowest.min(rotor(&physics).0);
        }
        let after = run(&mut physics, controls, 1);
        if after.position[1] < AIR_AMBULANCE_HALF_EXTENTS[1] + 0.05 {
            touchdown = Some(pose);
            break;
        }
    }
    assert!(slowest > 0.85, "the upflow keeps the rotor turning: {slowest}");
    let touchdown = touchdown.expect("it lands");
    assert!(touchdown.velocity[1] > -4.0 && touchdown.up_y > 0.9, "{touchdown:?}");
}

#[test]
fn an_air_ambulances_tail_is_solid_behind_its_cabin() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_AIR_AMBULANCE, [0.0, 1.1, 0.0], 0.0);
    physics.step_cars(TICK_DT);
    for (what, at) in [("boom", AIR_AMBULANCE_BOOM_CENTRE), ("fin", AIR_AMBULANCE_FIN_CENTRE)] {
        let pose = physics.car_pose(1).unwrap();
        let [x, y, z] = pose.world_point(at);
        let strike = physics.armor_strike([x + 4.0, y, z], [-1.0, 0.0, 0.0], 8.0, 1);
        assert!(strike.is_some_and(|strike| strike.solid.is_some()), "a round meets the {what}");
    }
}

#[test]
fn a_pilot_taking_an_air_ambulance_moves_its_lever_as_the_gunships() {
    let input = shared::protocol::InputFrame { collective: 255, ..Default::default() };
    let controls = CarControls::driven(KIND_AIR_AMBULANCE, &input);
    assert!(controls.lift == 1.0 && controls.powered && !controls.handbrake, "{controls:?}");
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
    (120, [-0.0028, 0.3968, -8.9719], 1.0000),
    (180, [-0.0028, 0.3992, -46.4499], 0.6700),
    (240, [0.8566, 4.5892, -100.0662], 0.2742),
    (420, [20.8949, 0.3971, -188.5134], 0.6808),
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
fn a_helicopters_collective_is_the_lever_its_input_carries_not_jump_or_run() {
    let set = |collective: u8, buttons: u16| CarControls::driven(KIND_HELI, &shared::protocol::InputFrame { collective, buttons, ..Default::default() });
    assert_eq!(set(255, 0).lift, 1.0);
    assert_eq!(set(0, BUTTON_JUMP).lift, 0.0);
    assert!((set(128, BUTTON_RUN).lift - 128.0 / 255.0).abs() < 1e-6);
    assert!(set(0, BUTTON_JUMP).powered && !set(0, BUTTON_JUMP).handbrake);
}

#[test]
fn only_aircraft_fly_towards_the_look() {
    let input = shared::protocol::InputFrame { forward: 127, strafe: 127, yaw: 500, pitch: -300, ..Default::default() };
    for kind in [KIND_PLANE, KIND_SEAPLANE, KIND_DRONE, KIND_LOITER, KIND_HELI] {
        let controls = CarControls::driven(kind, &input);
        assert_eq!(controls.aim, Some([0.5, -0.3]), "kind {kind}");
        assert_eq!((controls.throttle, controls.steer), (1.0, 1.0));
    }
    for kind in [KIND_JEEP, KIND_TANK, KIND_HEN] {
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


fn with_seaplane() -> Physics {
    let mut physics = Physics::new(&airfield()).unwrap();
    physics.add_car(1, KIND_SEAPLANE, [0.0, SEAPLANE_RIDE_HEIGHT + 0.3, 600.0], 0.0);
    physics
}

#[test]
fn a_seaplane_stands_on_the_wheels_in_its_pontoons_at_its_ride_height() {
    let mut physics = with_seaplane();
    let rest = run(&mut physics, CarControls { handbrake: true, ..CarControls::IDLE }, 240);
    assert!((rest.position[1] - SEAPLANE_RIDE_HEIGHT).abs() < 0.01 && rest.up_y > 0.999, "it settles at {SEAPLANE_RIDE_HEIGHT}: {rest:?}");
    // The pontoons' undersides are clear of the road.
    assert!(rest.position[1] - SEAPLANE_KEEL > 0.1, "{rest:?}");
    let idle = run(&mut physics, pilot(0.0, 0.0, 0.0), 120);
    assert!(idle.forward_speed < 3.0 && (600.0 - idle.position[2]) < 4.0, "it idles where it stands: {idle:?}");
}

#[test]
fn a_seaplane_flies_from_a_road_slower_than_the_triplane_and_rolls_slower() {
    // Its cruise and its top speed in level flight, and how far it banks in half a second of full stick.
    let measure = |physics: &mut Physics| {
        airborne(physics, 40.0);
        run(physics, pilot(0.0, 0.0, 0.0), 60 * 10);
        let cruise = run(physics, pilot(0.0, 0.0, 0.0), 60 * 3);
        assert!(cruise.up_y > 0.99 && cruise.velocity[1].abs() < 0.5, "it cruises level: {cruise:?}");
        let flat_out = run(physics, pilot(1.0, 0.0, 0.0), 60 * 20);
        run(physics, pilot(0.0, 0.0, 0.0), 60 * 8);
        let banked = run(physics, pilot(0.0, 0.0, 1.0), 30);
        (cruise.forward_speed, flat_out.forward_speed, banked.up_y.clamp(-1.0, 1.0).acos())
    };
    let (cruise, top, bank) = measure(&mut with_seaplane());
    let (plane_cruise, plane_top, plane_bank) = measure(&mut with_plane());
    assert!((cruise - SEAPLANE_CRUISE_SPEED).abs() < 3.0 && (top - 38.0).abs() < 3.0, "it cruises at {cruise} and tops out at {top} m/s");
    assert!(cruise < plane_cruise - 3.0 && top < plane_top - 4.0, "slower than the triplane: {cruise} against {plane_cruise}, {top} against {plane_top}");
    assert!(bank > 0.3 && bank < plane_bank * 0.85, "it rolls slower: {bank} rad against {plane_bank}");
}

#[test]
fn a_seaplane_lands_on_a_road_and_rolls_to_a_stop() {
    let mut physics = with_seaplane();
    airborne(&mut physics, 15.0);
    // Throttle cut, it glides down and its wheels bring it to a stop.
    for _ in 0..60 * 60 {
        let pose = run(&mut physics, pilot(-1.0, 0.0, 0.0), 1);
        if pose.forward_speed < 0.5 && pose.position[1] < SEAPLANE_RIDE_HEIGHT + 0.2 {
            break;
        }
    }
    let settled = run(&mut physics, pilot(-1.0, 0.0, 0.0), 120);
    assert!((settled.position[1] - SEAPLANE_RIDE_HEIGHT).abs() < 0.05 && settled.up_y > 0.99, "down on its wheels: {settled:?}");
    assert!(settled.forward_speed.abs() < 0.5, "and stopped: {settled:?}");
}

/// Flat ground for z > 0, then a `deg` slope rising towards -Z, on 2 m cells.
fn hill(deg: f32) -> WorldFile {
    let cell = 200;
    let side = (20_000 / cell + 1) as u32;
    let tan = deg.to_radians().tan();
    let heights = (0..side)
        .flat_map(|j| (0..side).map(move |_| j))
        .map(|j| ((-((-10_000 + j as i32 * cell) as f32 / 100.0)).max(0.0) * tan * 100.0).round() as i32)
        .collect();
    WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell, side, heights }, ..flat() }
}

/// Rolling ground from z = -5 on towards -Z: `amp` m high humps every `wave` m, on 1 m cells, with a
/// `cross` degree slope down towards -X under them.
fn humps(amp: f32, wave: f32, cross: f32) -> WorldFile {
    let cell = 100;
    let side = (20_000 / cell + 1) as u32;
    let tan_x = cross.to_radians().tan();
    let heights = (0..side)
        .flat_map(|j| (0..side).map(move |i| (i, j)))
        .map(|(i, j)| {
            let (x, z) = ((-10_000 + i as i32 * cell) as f32 / 100.0, (-10_000 + j as i32 * cell) as f32 / 100.0);
            let hump = if z < -5.0 { amp * (0.5 - 0.5 * (std::f32::consts::TAU * z / wave).cos()) } else { 0.0 };
            ((hump + x * tan_x) * 100.0).round() as i32
        })
        .collect();
    WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell, side, heights }, ..flat() }
}

#[test]
fn tracked_vehicles_ride_over_metre_high_humps_without_stopping() {
    for kind in [KIND_TANK, KIND_STRV, KIND_STRF, KIND_PBV] {
        let mut physics = Physics::new(&humps(1.0, 10.0, 0.0)).unwrap();
        physics.add_car(1, kind, [0.0, 1.4, 0.0], 0.0);
        run(&mut physics, COAST, 60);
        let mut slowest = f32::MAX;
        let mut pose = run(&mut physics, FORWARD, 60 * 2);
        for _ in 0..40 {
            pose = run(&mut physics, FORWARD, 10);
            slowest = slowest.min(pose.forward_speed);
        }
        // The nose rides up each hump on the plate under it instead of ramming it. The tank is long
        // enough to touch a crest with its belly between two humps, which slows it more than the others.
        let floor = if kind == KIND_TANK { 2.5 } else { 4.0 };
        assert!(pose.position[2] < -60.0 && slowest > floor && pose.up_y > 0.9, "kind {kind}: {pose:?}, slowest {slowest}");
    }
}

#[test]
fn tracked_vehicles_grind_up_a_35_degree_slope_from_a_standing_start() {
    let deg: f32 = 35.0;
    for kind in [KIND_TANK, KIND_STRV, KIND_STRF, KIND_PBV] {
        let mut physics = Physics::new(&hill(deg)).unwrap();
        physics.add_car(1, kind, [0.0, 20.0 * deg.to_radians().tan() + 1.6, -20.0], 0.0);
        physics.set_car_dive(1, [0.0, 20.0 * deg.to_radians().tan() + 1.6, -20.0], 0.0, deg.to_radians(), 0.0);
        let start = run(&mut physics, COAST, 60);
        let pose = run(&mut physics, FORWARD, 60 * 6);
        assert!(pose.position[2] < start.position[2] - 10.0 && pose.forward_speed > 5.0, "kind {kind}: {start:?} to {pose:?}");
    }
}

#[test]
fn a_tracked_vehicle_holds_its_heading_across_a_side_slope() {
    for kind in [KIND_TANK, KIND_STRV, KIND_STRF, KIND_PBV] {
        let mut physics = Physics::new(&humps(0.3, 6.0, 15.0)).unwrap();
        physics.add_car(1, kind, [0.0, 1.4, 0.0], 0.0);
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, FORWARD, 60 * 6);
        // It does not swing round downhill (towards -X), so it ends up about where it pointed.
        assert!(yaw(&pose).abs() < 0.1 && pose.position[0].abs() < 3.0, "kind {kind}: yaw {} at {pose:?}", yaw(&pose));
    }
}

/// A ski slope: flat ground for z > 0, then a `deg` slope rising towards -Z for `length` m (along the
/// ground), then flat again at its top, on 4 m cells.
fn piste(deg: f32, length: f32) -> WorldFile {
    let cell = 400;
    let side = (100_000 / cell + 1) as u32;
    let (tan, run) = (deg.to_radians().tan(), length * deg.to_radians().cos());
    let heights = (0..side)
        .flat_map(|j| (0..side).map(move |_| j))
        .map(|j| ((-((-50_000 + j as i32 * cell) as f32 / 100.0)).clamp(0.0, run) * tan * 100.0).round() as i32)
        .collect();
    WorldFile { half_extent: 50_000, ground: shared::terrain::Ground { origin: -50_000, cell, side, heights }, ..flat() }
}

/// Puts vehicle 1, a `kind`, at the top of a `piste(deg, length)`, `back` m short of its edge,
/// facing down it, and lets it settle there.
fn atop(kind: u8, deg: f32, length: f32, back: f32) -> Physics {
    let across = length * deg.to_radians().cos();
    let top = across * deg.to_radians().tan();
    let mut physics = Physics::new(&piste(deg, length)).unwrap();
    // Facing +Z, down the slope.
    physics.add_car(1, kind, [0.0, top + 1.0, -across - back], std::f32::consts::PI);
    run(&mut physics, CarControls { handbrake: true, ..COAST }, 60);
    physics
}

/// Runs vehicle 1 under `controls` until it reaches the foot of the slope (z = 0) or `seconds` pass,
/// and returns its pose there.
fn to_the_foot(physics: &mut Physics, controls: CarControls, seconds: f32) -> CarPose {
    let mut pose = physics.car_pose(1).unwrap();
    for _ in 0..(seconds * 60.0) as u32 {
        pose = run(physics, controls, 1);
        if pose.position[2] > 0.0 {
            break;
        }
    }
    pose
}

#[test]
fn down_a_piste_a_coasting_snowracer_runs_about_as_fast_as_a_snowmobile_flat_out() {
    for deg in [10.0, 20.0, 30.0] {
        // A second of its motor takes the sled over the edge; from there it coasts.
        let mut physics = atop(KIND_SNOWRACER, deg, 300.0, 2.0);
        run(&mut physics, FORWARD, 60);
        let sled = to_the_foot(&mut physics, COAST, 60.0);
        let snowmobile = to_the_foot(&mut atop(KIND_SNOWMOBILE, deg, 300.0, 2.0), FORWARD, 60.0);
        assert!(sled.position[2] > 0.0 && sled.up_y > 0.95, "{deg}°: the sled reaches the foot upright: {sled:?}");
        // Gravity takes it far past its motor's top speed, to within a fifth of the snowmobile's.
        let share = sled.forward_speed / snowmobile.forward_speed;
        assert!(sled.forward_speed > 3.0 * SNOWRACER_MAX_SPEED && (0.8..1.1).contains(&share), "{deg}°: sled {sled:?}, snowmobile {snowmobile:?}");
    }
}

#[test]
fn a_snowracers_little_motor_creeps_up_a_25_degree_piste_but_not_a_30_degree_one() {
    let climb = |deg: f32| {
        let mut physics = Physics::new(&hill(deg)).unwrap();
        physics.add_car(1, KIND_SNOWRACER, [0.0, 1.0, 10.0], 0.0);
        run(&mut physics, COAST, 60);
        run(&mut physics, FORWARD, 60 * 12)
    };
    let (gentle, steep) = (climb(25.0), climb(30.0));
    assert!(gentle.position[2] < -25.0 && (2.5..5.0).contains(&gentle.forward_speed), "it creeps up 25°: {gentle:?}");
    assert!(steep.forward_speed < 0.5, "and stalls on 30°: {steep:?}");
}

#[test]
fn on_the_flat_a_snowracer_tops_out_at_its_motors_speed_and_slides_to_a_stop() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_SNOWRACER, [0.0, 1.0, 0.0], 0.0);
    run(&mut physics, COAST, 60);
    let top = run(&mut physics, FORWARD, 60 * 6);
    assert!((top.forward_speed - SNOWRACER_MAX_SPEED).abs() < 0.5, "{top:?}");
    // The snow's friction stops it, in about the time `mu·g` takes.
    let sliding = run(&mut physics, COAST, 60 * 6);
    assert!(sliding.forward_speed > 1.5, "still sliding after six seconds: {sliding:?}");
    let stopped = run(&mut physics, COAST, 60 * 8);
    assert!(stopped.forward_speed.abs() < 0.05, "and stopped by fourteen: {stopped:?}");
}

#[test]
fn a_snowracer_carves_and_spins_but_stays_upright_at_any_lock_down_a_piste() {
    for steer in [0.5, 1.0] {
        let mut physics = atop(KIND_SNOWRACER, 20.0, 300.0, 2.0);
        run(&mut physics, FORWARD, 60);
        let fast = run(&mut physics, COAST, 60 * 6);
        assert!(fast.forward_speed > 15.0, "{fast:?}");
        let mut pose = fast;
        for _ in 0..60 * 3 {
            pose = run(&mut physics, CarControls { steer, ..COAST }, 1);
            assert!(pose.up_y > 0.5, "lock {steer}: it stays on its runners: {pose:?}");
        }
        assert!(yaw(&pose).abs() > 0.3, "lock {steer}: and turns: {pose:?}");
    }
}

#[test]
fn a_snowracer_rests_still_on_a_gentle_slope_and_slides_off_a_steeper_one() {
    let rest = |deg: f32| {
        let mut physics = Physics::new(&hill(deg)).unwrap();
        // Up the slope, facing down it, held on its brake while it settles, then let go.
        let at = [0.0, 20.0 * deg.to_radians().tan() + 0.4, -20.0];
        physics.add_car(1, KIND_SNOWRACER, at, std::f32::consts::PI);
        physics.set_car_dive(1, at, std::f32::consts::PI, -deg.to_radians(), 0.0);
        let settled = run(&mut physics, CarControls { handbrake: true, ..COAST }, 60 * 2);
        let after = run(&mut physics, COAST, 60 * 3);
        after.position[2] - settled.position[2]
    };
    let (gentle, steep) = (rest(2.0), rest(10.0));
    assert!(gentle.abs() < 0.1, "friction holds it on 2°: it moved {gentle} m");
    assert!(steep > 3.0, "but not on 10°: it moved {steep} m");
}

/// A kicker: flat ground, then a ramp rising towards -Z from z = -10 to `height` m over `length` m,
/// ending in a drop back to flat ground, on 0.5 m cells.
fn kicker(height: f32, length: f32) -> WorldFile {
    let cell = 50;
    let side = (20_000 / cell + 1) as u32;
    let heights = (0..side)
        .flat_map(|j| (0..side).map(move |_| j))
        .map(|j| {
            let z = (-10_000 + j as i32 * cell) as f32 / 100.0;
            let up = -10.0 - z;
            if (0.0..=length).contains(&up) { (up / length * height * 100.0).round() as i32 } else { 0 }
        })
        .collect();
    WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell, side, heights }, ..flat() }
}

/// What a jump off a kicker came to.
#[derive(Debug)]
struct Jump {
    /// How far past the lip it first came down, metres.
    landed: f32,
    /// How long it was off the ground, seconds.
    air: f32,
    /// The most its nose pointed above (+) and below (-) the line it flew, radians.
    above: f32,
    below: f32,
    /// How far it turned in the air, radians, + left.
    turned: f32,
    /// Where it was two seconds after landing.
    after: CarPose,
}

/// Sends vehicle 1, a `kind`, at `speed` m/s along -Z over a `kicker(height, length)` under
/// `controls` once it is off the lip.
fn jump(kind: u8, speed: f32, height: f32, length: f32, controls: CarControls) -> Jump {
    let mut physics = Physics::new(&kicker(height, length)).unwrap();
    physics.add_car(1, kind, [0.0, 1.0, 20.0], 0.0);
    run(&mut physics, COAST, 60);
    physics.set_car_dive(1, physics.car_pose(1).unwrap().position, 0.0, 0.0, speed);
    let lip = -10.0 - length;
    let ground = |z: f32| { let up = -10.0 - z; if (0.0..=length).contains(&up) { up / length * height } else { 0.0 } };
    let rest = spec(kind).half_extents[1] + 0.25;
    let mut jump = Jump { landed: f32::NAN, air: 0.0, above: 0.0, below: 0.0, turned: 0.0, after: physics.car_pose(1).unwrap() };
    let (mut flying, mut start) = (false, 0.0);
    for _ in 0..60 * 6 {
        let past = pose_past(&physics, lip);
        let pose = run(&mut physics, if past { controls } else { COAST }, 1);
        let off = pose.position[1] - ground(pose.position[2]) > rest + 0.3;
        if off && pose.position[2] < lip && jump.landed.is_nan() {
            if !flying {
                (flying, start) = (true, yaw(&pose));
            }
            jump.air += TICK_DT;
            let [vx, vy, vz] = pose.velocity;
            let line = (vy / (vx * vx + vy * vy + vz * vz).sqrt()).asin();
            jump.above = jump.above.max(nose_pitch(&pose) - line);
            jump.below = jump.below.min(nose_pitch(&pose) - line);
            jump.turned = yaw(&pose) - start;
        } else if flying && jump.landed.is_nan() {
            jump.landed = lip - pose.position[2];
            jump.after = run(&mut physics, COAST, 120);
            break;
        }
    }
    jump
}

/// Whether vehicle 1 is past the lip at `lip` (z).
fn pose_past(physics: &Physics, lip: f32) -> bool {
    physics.car_pose(1).is_some_and(|pose| pose.position[2] < lip)
}

#[test]
fn off_a_kicker_a_snowracer_sails_nose_along_its_line_further_than_a_snowmobile_and_lands_upright() {
    let sled = jump(KIND_SNOWRACER, 20.0, 1.0, 5.0, COAST);
    let snowmobile = jump(KIND_SNOWMOBILE, 20.0, 1.0, 5.0, COAST);
    assert!(sled.landed > snowmobile.landed && sled.air > 1.0, "it flies further: {sled:?} against {snowmobile:?}");
    // Its nose neither dives nor rears off the line it flies.
    assert!(sled.above < 0.25 && sled.below > -0.25, "{sled:?}");
    assert!(sled.after.up_y > 0.9 && sled.after.forward_speed > 10.0, "and it rides on: {sled:?}");
}

#[test]
fn in_the_air_the_rider_turns_a_snowracer_and_leans_it_forward_or_back() {
    let turned = jump(KIND_SNOWRACER, 20.0, 1.0, 5.0, CarControls { steer: -1.0, ..COAST });
    assert!(turned.turned > 0.6, "full stick left turns it left in the air: {turned:?}");
    assert!(turned.after.up_y > 0.9, "and it lands upright: {turned:?}");
    let forward = jump(KIND_SNOWRACER, 20.0, 1.0, 5.0, FORWARD);
    let back = jump(KIND_SNOWRACER, 20.0, 1.0, 5.0, CarControls { throttle: -1.0, ..COAST });
    assert!(forward.below < -0.25 && back.above > 0.2, "W leans the nose down, S lifts it: {forward:?}, {back:?}");
}

/// `world`, a point in the world, in the hull frame of vehicle 1.
fn in_hull(physics: &Physics, world: [f32; 3]) -> [f32; 3] {
    let pose = physics.car_pose(1).unwrap();
    pose.local_dir(vec3::sub(world, pose.position))
}

fn titan() -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_TITAN, [0.0, TITAN_LEG_LENGTH + TITAN_HALF_EXTENTS[1], 0.0], 0.0);
    physics
}

#[test]
fn a_titan_stands_fifty_metres_high_on_its_legs() {
    let mut physics = titan();
    let pose = run(&mut physics, COAST, 600);
    println!("titan stands: {pose:?}");
    assert!(pose.up_y > 0.995, "upright: {pose:?}");
    assert!((pose.position[1] - TITAN_RIDE_HEIGHT).abs() < 0.1, "its hull rides {TITAN_RIDE_HEIGHT} m up: {pose:?}");
    assert!(pose.velocity.iter().all(|v| v.abs() < 0.1), "at rest: {pose:?}");
    // The top of its head is about fifty metres up.
    let top = pose.position[1] + TITAN_EYE[1] + 3.0;
    assert!((45.0..55.0).contains(&top), "about fifty metres high: {top}");
    let limbs = physics.limbs(1).unwrap();
    for (index, foot) in limbs.feet.iter().enumerate() {
        let side = if index == 0 { -1.0 } else { 1.0 };
        assert!(foot[1].abs() < 0.05 && (foot[0] - side * TITAN_FOOT_HALF).abs() < 0.5 && foot[2].abs() < 0.5, "foot {index} stands under its hip: {foot:?}");
    }
}

#[test]
fn a_titan_walks_on_one_foot_at_a_time_rising_over_each_and_turns_on_the_spot() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    let mut landed = [0; 2];
    let mut pose = physics.car_pose(1).unwrap();
    let (mut low, mut high, mut slow, mut fast) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    let mut planted = [[0.0; 3]; 2];
    for tick in 0..900 {
        pose = run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 1);
        let limbs = physics.limbs(1).unwrap();
        for (index, count) in landed.iter_mut().enumerate() {
            *count += u32::from(limbs.landed[index]);
            let away = (limbs.feet[index][0] - pose.position[0]).hypot(limbs.feet[index][2] - pose.position[2]);
            assert!(away < TITAN_STRIDE, "foot {index} keeps up with the hull: {away} m off");
            // A foot on the ground stays where it was put, and one of the two always is.
            if limbs.step.swinging().is_none_or(|(foot, _)| foot != index) && !limbs.landed[index] && tick > 0 {
                assert_eq!(limbs.planted[index], planted[index], "foot {index} does not slide");
            }
        }
        planted = limbs.planted;
        // Once it is in its stride: how high it rides and how fast it goes.
        if tick >= 500 {
            (low, high) = (low.min(pose.position[1]), high.max(pose.position[1]));
            (slow, fast) = (slow.min(pose.forward_speed), fast.max(pose.forward_speed));
        }
    }
    println!("titan walks: {pose:?}, steps {landed:?}, rides {low}..{high} m at {slow}..{fast} m/s");
    assert!(slow > 0.85 * TITAN_MAX_SPEED && fast < 1.05 * TITAN_MAX_SPEED, "up to speed: {slow}..{fast}");
    assert!(pose.up_y > 0.97, "stays upright walking: {pose:?}");
    assert!(landed[0] >= 3 && landed[1] >= 3 && landed[0].abs_diff(landed[1]) <= 1, "its feet step in turn: {landed:?}");
    // About 1.65 s a step: nine in fifteen seconds, not the twenty a quick patter would be.
    assert!(landed[0] + landed[1] <= 10, "it takes long, slow steps: {landed:?}");
    assert!(high - low > 0.4 && high < TITAN_RIDE_HEIGHT, "it rises over each foot and sinks between them: {low}..{high}");
    assert!(fast - slow > 0.3, "and goes slowest over a foot: {slow}..{fast}");
    let stopped = run(&mut physics, DRIVEN, 300);
    assert!(stopped.forward_speed.abs() < 0.3, "stops when the stick is let go: {stopped:?}");
    let before = stopped.yaw();
    let turned = run(&mut physics, CarControls { steer: -1.0, ..DRIVEN }, 180);
    assert!((turned.yaw() - before).abs() > 1.0, "turns on the spot: {before} -> {}", turned.yaw());
    let settled = run(&mut physics, DRIVEN, 400);
    let limbs = physics.limbs(1).unwrap();
    let feet_apart = (limbs.feet[0][0] - limbs.feet[1][0]).hypot(limbs.feet[0][2] - limbs.feet[1][2]);
    assert!((feet_apart - 2.0 * TITAN_FOOT_HALF).abs() < 2.0 * 2.5, "its feet came round with it: {feet_apart} m apart");
    assert!((settled.position[1] - TITAN_RIDE_HEIGHT).abs() < 0.3 && limbs.step.swinging().is_none(), "and it stands again on both: {settled:?}");
}

#[test]
fn a_titans_feet_put_back_where_a_snapshot_has_them_walk_on_the_same() {
    // What its pilot's client does at every snapshot: the hull and the feet are put back, and it is walked on.
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 400);
    let (pose, motion, limbs) = (physics.car_pose(1).unwrap(), physics.car_motion(1).unwrap(), physics.limbs(1).unwrap());
    let ahead = run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 30);
    // Its feet as the wire carries them: to the centimetre, the step and the weight a byte each.
    let cm = |point: [f32; 3]| point.map(|value| (value * 100.0).round() / 100.0);
    physics.set_car_state(1, pose.position, pose.rotation, motion, None);
    physics.set_feet(1, limbs.planted.map(cm), shared::vehicles::Step::from_byte(limbs.step.to_byte()), (limbs.weight * 255.0).round() / 255.0);
    let again = run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 30);
    let off = shared::vec3::distance(ahead.position, again.position);
    println!("replayed {off} m off");
    assert!(off < 0.15, "half a second on, it is where it was the first time: {off} m off");
}

#[test]
fn a_titan_crouches_to_bring_a_fist_to_the_ground_and_stands_again() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    // Standing, a fist asked for at the ground ahead stops at the end of its arm, far above it.
    let ahead = [TITAN_SHOULDER[0], -TITAN_RIDE_HEIGHT + TITAN_FIST_RADIUS, -12.0];
    physics.set_hands(1, [Hand::default(), Hand { target: Some(ahead), closed: false }]);
    run(&mut physics, DRIVEN, 180);
    let high = physics.limbs(1).unwrap().fists_world[1];
    assert!(high[1] > 8.0, "standing, it cannot reach the ground: {high:?}");
    // Crouched, the same fist comes down to it. The target is given in the hull frame, which has come down too.
    let crouched = run(&mut physics, CarControls { lift: -1.0, ..DRIVEN }, 300);
    println!("titan crouched: {crouched:?}");
    assert!(crouched.position[1] < TITAN_RIDE_HEIGHT - 0.8 * TITAN_CROUCH, "its hull came down: {crouched:?}");
    assert!(crouched.up_y > 0.85, "it leans, but does not fall: {crouched:?}");
    let low = [TITAN_SHOULDER[0], -crouched.position[1] + TITAN_FIST_RADIUS, -12.0];
    physics.set_hands(1, [Hand::default(), Hand { target: Some(low), closed: false }]);
    run(&mut physics, CarControls { lift: -1.0, ..DRIVEN }, 120);
    let fist = physics.limbs(1).unwrap().fists_world[1];
    println!("titan fist at the ground: {fist:?}");
    assert!(fist[1] < TITAN_FIST_RADIUS + 1.0, "crouched, its fist is on the ground: {fist:?}");
    let stood = run(&mut physics, DRIVEN, 300);
    assert!((stood.position[1] - TITAN_RIDE_HEIGHT).abs() < 0.5 && stood.up_y > 0.99, "it stands up again: {stood:?}");
}

#[test]
fn a_titans_fist_goes_where_it_is_asked_within_its_reach_and_no_faster_than_its_arm() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    let shoulder = [TITAN_SHOULDER[0], TITAN_SHOULDER[1], TITAN_SHOULDER[2]];
    // Far beyond its reach, straight ahead of the shoulder.
    physics.set_hands(1, [Hand::default(), Hand { target: Some([shoulder[0], shoulder[1], -200.0]), closed: true }]);
    let mut fastest: f32 = 0.0;
    for _ in 0..120 {
        run(&mut physics, DRIVEN, 1);
        let limbs = physics.limbs(1).unwrap();
        fastest = fastest.max(vec3::length(limbs.fist_velocity[1]));
        assert!(vec3::distance(limbs.fists[1], shoulder) <= TITAN_REACH + 1e-2, "within reach: {:?}", limbs.fists[1]);
    }
    let limbs = physics.limbs(1).unwrap();
    println!("titan fist: {limbs:?}, fastest {fastest}");
    assert!((limbs.fists[1][2] + TITAN_REACH).abs() < 0.2 && limbs.closed == [false, true], "at the end of its arm, closed: {limbs:?}");
    assert!(fastest > 0.8 * TITAN_FIST_SPEED && fastest < TITAN_FIST_SPEED + 3.0, "it swings at its arm's speed: {fastest}");
    assert!(vec3::length(limbs.fist_velocity[1]) < 1.0, "and stops there: {limbs:?}");
    // The other one hangs where it rests, mirrored.
    assert!(vec3::distance(limbs.fists[0], [-TITAN_FIST_REST[0], TITAN_FIST_REST[1], TITAN_FIST_REST[2]]) < 0.05, "the left hangs: {limbs:?}");
    // Asked for inside its own chest, it stays outside it.
    physics.set_hands(1, [Hand::default(), Hand { target: Some([0.0, 0.0, 0.0]), closed: false }]);
    run(&mut physics, DRIVEN, 120);
    let [x, y, z] = physics.limbs(1).unwrap().fists[1];
    let [hx, hy, hz] = TITAN_HALF_EXTENTS;
    assert!(x.abs() >= hx + TITAN_FIST_RADIUS - 0.05 || y.abs() >= hy + TITAN_FIST_RADIUS - 0.05 || z.abs() >= hz + TITAN_FIST_RADIUS - 0.05, "outside its torso: {:?}", [x, y, z]);
    // With nobody aboard, it hangs again.
    run(&mut physics, COAST, 120);
    let limbs = physics.limbs(1).unwrap();
    assert!(vec3::distance(limbs.fists[1], TITAN_FIST_REST) < 0.05, "an empty one's arms hang: {limbs:?}");
}

#[test]
fn a_titans_fist_shoves_a_jeep_and_a_closed_one_carries_it_off() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    let crouch = CarControls { lift: -1.0, ..DRIVEN };
    let crouched = run(&mut physics, crouch, 300);
    // A jeep ahead of the right fist, which sweeps through where it stands.
    let jeep = [crouched.position[0] + TITAN_SHOULDER[0], 1.0, crouched.position[2] - 12.0];
    physics.add_car(2, KIND_JEEP, jeep, 0.0);
    let level = TITAN_FIST_RADIUS + 0.3;
    physics.set_hands(1, [Hand::default(), Hand { target: Some(in_hull(&physics, [jeep[0] + 8.0, level, jeep[2]])), closed: false }]);
    run(&mut physics, crouch, 60);
    physics.set_hands(1, [Hand::default(), Hand { target: Some(in_hull(&physics, [jeep[0] - 8.0, level, jeep[2]])), closed: false }]);
    run(&mut physics, crouch, 60);
    let shoved = physics.car_pose(2).unwrap();
    println!("jeep shoved: {shoved:?}");
    assert!(shoved.position[0] < jeep[0] - 3.0, "the fist swept it aside: {shoved:?}");
    let titan_now = physics.car_pose(1).unwrap();
    assert!(titan_now.up_y > 0.85 && vec3::distance(titan_now.position, crouched.position) < 0.5, "the Titan does not feel it: {titan_now:?}");

    // Closed on it, the fist lifts it.
    for _ in 0..90 {
        let at = physics.car_pose(2).unwrap().position;
        physics.set_hands(1, [Hand::default(), Hand { target: Some(in_hull(&physics, at)), closed: true }]);
        run(&mut physics, crouch, 1);
    }
    let fist = physics.limbs(1).unwrap().fists_world[1];
    assert_eq!(physics.cars_touching_ball(fist, TITAN_FIST_RADIUS + 0.5, 1), vec![2], "the fist is on the jeep");
    assert!(physics.grab_car(1, 1, 2) && physics.held_car(1, 1) == Some(2));
    assert!(!physics.grab_car(1, 1, 2), "a full hand takes nothing more");
    physics.set_hands(1, [Hand::default(), Hand { target: Some([TITAN_SHOULDER[0], 0.0, -15.0]), closed: true }]);
    let stood = run(&mut physics, DRIVEN, 300);
    let carried = physics.car_pose(2).unwrap();
    println!("jeep carried: {carried:?}, titan {stood:?}");
    assert!(carried.position[1] > 25.0, "carried high: {carried:?}");
    assert!(stood.up_y > 0.98, "the Titan stands with it: {stood:?}");
    // Thrown: let go while the fist swings.
    physics.set_hands(1, [Hand::default(), Hand { target: Some([TITAN_SHOULDER[0], 5.0, -30.0]), closed: true }]);
    run(&mut physics, DRIVEN, 12);
    assert_eq!(physics.release_car(1, 1), Some(2));
    let thrown = physics.car_pose(2).unwrap();
    assert!(vec3::length(thrown.velocity) > 15.0 && physics.held_car(1, 1).is_none(), "it flies off as fast as the fist went: {thrown:?}");
    // A tank is not too heavy, but another Titan is not a thing to pick up.
    physics.add_car(3, KIND_TITAN, [80.0, TITAN_RIDE_HEIGHT, 0.0], 0.0);
    assert!(!physics.grab_car(1, 0, 3));
}

#[test]
fn a_titan_shrugs_off_a_shell_but_a_great_blow_fells_it_and_it_gets_up() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    // A blast against its chest that would throw a walker over.
    physics.blast_cars([0.0, TITAN_RIDE_HEIGHT, -5.0], 12.0);
    let pose = run(&mut physics, COAST, 300);
    assert!(pose.up_y > 0.99 && pose.position[2].abs() < 1.0, "a shell's blast barely moves it: {pose:?}");
    // Thrown over bodily.
    physics.spin_car(1, [0.0, 0.0, 6.0]);
    let mut lowest: f32 = 1.0;
    for _ in 0..240 {
        lowest = lowest.min(run(&mut physics, COAST, 1).up_y);
    }
    println!("titan felled: lowest up {lowest}");
    assert!(lowest < TITAN_GAIT.fallen_up, "a great blow fells it: {lowest}");
    let pose = run(&mut physics, COAST, 1800);
    println!("titan got up: {pose:?}");
    assert!(pose.up_y > 0.98 && (pose.position[1] - TITAN_RIDE_HEIGHT).abs() < 1.0, "it gets up again: {pose:?}");
}

/// Fills a Titan's tank where it is, as a snapshot would.
fn refuel(physics: &mut Physics) {
    let (pose, motion) = (physics.car_pose(1).unwrap(), physics.car_motion(1).unwrap());
    physics.set_car_state(1, pose.position, pose.rotation, motion, Some(TITAN_FUEL));
}

#[test]
fn a_titan_leaps_and_its_thrusters_carry_it_up_and_along_while_the_fuel_lasts() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    let hop = CarControls { hop: true, ..DRIVEN };
    // A leap takes it off its feet, which hang under it.
    let leapt = run(&mut physics, hop, 30);
    let limbs = physics.limbs(1).unwrap();
    let (fuel, flags) = physics.car_thrust(1).unwrap();
    println!("titan leapt: {leapt:?}, {limbs:?}, fuel {fuel}");
    assert!(leapt.position[1] > TITAN_RIDE_HEIGHT + 4.0 && limbs.step == shared::vehicles::Step::Airborne, "off the ground: {leapt:?}");
    assert!(limbs.feet.iter().all(|foot| foot[1] > 3.0 && leapt.position[1] - foot[1] < TITAN_RIDE_HEIGHT), "its feet hang under it, drawn up: {:?}", limbs.feet);
    assert!(flags & THRUST_AIR != 0 && fuel < TITAN_FUEL, "it cost fuel: {fuel}");
    // Held, the thrusters climb and carry it along; it turns, and stays level.
    let before = leapt.yaw();
    let flown = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..hop }, 240);
    println!("titan flown: {flown:?}");
    assert!((flown.velocity[1] - TITAN_JET_CLIMB).abs() < 1.0, "climbing at its pace: {flown:?}");
    assert!(flown.forward_speed > 5.0 && flown.up_y > 0.97, "carried along, level: {flown:?}");
    assert!((flown.yaw() - before).abs() > 1.0, "turned in the air: {before} -> {}", flown.yaw());
    assert!(physics.car_thrust(1).unwrap().1 & THRUST_LIFT != 0, "thrusters lit");
    // With the fuel gone it falls, whatever is held.
    let spent = run(&mut physics, hop, 240);
    let (fuel, flags) = physics.car_thrust(1).unwrap();
    assert!(fuel == 0.0 && flags & THRUST_LIFT == 0 && spent.velocity[1] < -5.0, "out of fuel, falling: {spent:?}, fuel {fuel}");
    // Refuelled in the air and held, it stops under its ceiling.
    for _ in 0..40 {
        refuel(&mut physics);
        run(&mut physics, hop, 60);
    }
    let high = physics.car_pose(1).unwrap();
    println!("titan at its ceiling: {high:?}");
    assert!((high.position[1] - TITAN_CEILING).abs() < 3.0 && high.velocity[1].abs() < 0.5, "it hangs at its ceiling: {high:?}");
}

#[test]
fn a_titan_that_comes_down_hard_lands_on_its_feet_and_its_thrusters_break_a_fall() {
    // Up to its ceiling, twice: once let fall, once with the thrusters fired whenever it falls fast.
    let mut hardest = [0.0_f32; 2];
    for (case, held) in [false, true].into_iter().enumerate() {
        let mut physics = titan();
        run(&mut physics, COAST, 300);
        let hop = CarControls { hop: true, ..DRIVEN };
        for _ in 0..12 {
            refuel(&mut physics);
            run(&mut physics, hop, 60);
        }
        refuel(&mut physics);
        let mut lowest = f32::MAX;
        let mut landed = false;
        for _ in 0..1800 {
            // The fuel is kept up, so that this is about the thrusters alone.
            if held {
                refuel(&mut physics);
            }
            let falling = physics.limbs(1).unwrap().step == shared::vehicles::Step::Airborne && physics.car_pose(1).unwrap().velocity[1] < -5.0;
            let pose = run(&mut physics, if held && falling { CarControls { hop: true, ..DRIVEN } } else { DRIVEN }, 1);
            let limbs = physics.limbs(1).unwrap();
            if limbs.impact > 0.0 {
                hardest[case] = hardest[case].max(limbs.impact);
                assert!(limbs.landed == [true; 2] && limbs.feet.iter().all(|foot| foot[1].abs() < 0.05), "both feet are planted on the ground: {limbs:?}");
                landed = true;
            }
            if landed {
                lowest = lowest.min(pose.position[1]);
                assert!(pose.up_y > 0.9, "it stays on its feet: {pose:?}");
            }
        }
        let rest = physics.car_pose(1).unwrap();
        println!("titan landed (thrusters held: {held}): hit at {} m/s, sank to {lowest} m, rests {rest:?}", hardest[case]);
        assert!(landed && (rest.position[1] - TITAN_RIDE_HEIGHT).abs() < 0.3 && rest.velocity[1].abs() < 0.2, "it stands again at its height: {rest:?}");
        // Its hull, whose underside is `TITAN_HALF_EXTENTS[1]` below its middle, never met the ground.
        assert!(lowest > TITAN_HALF_EXTENTS[1] + 6.0, "its legs took the landing: sank to {lowest} m");
    }
    assert!(hardest[0] >= TITAN_SLAM_FULL * 0.8, "let fall from its ceiling it slams: {hardest:?}");
    assert!(hardest[1] < TITAN_SLAM_SPEED, "with its thrusters held it sets down: {hardest:?}");
}

#[test]
fn a_titan_put_back_in_the_air_as_a_snapshot_has_it_flies_on_the_same() {
    let mut physics = titan();
    run(&mut physics, COAST, 300);
    let fly = CarControls { hop: true, throttle: 1.0, ..DRIVEN };
    run(&mut physics, fly, 120);
    let (pose, motion, limbs, (fuel, _)) = (physics.car_pose(1).unwrap(), physics.car_motion(1).unwrap(), physics.limbs(1).unwrap(), physics.car_thrust(1).unwrap());
    assert_eq!(limbs.step, shared::vehicles::Step::Airborne);
    let ahead = run(&mut physics, fly, 30);
    let cm = |point: [f32; 3]| point.map(|value| (value * 100.0).round() / 100.0);
    physics.set_car_state(1, pose.position, pose.rotation, motion, Some((fuel * 255.0).round() / 255.0));
    physics.set_feet(1, limbs.planted.map(cm), shared::vehicles::Step::from_byte(limbs.step.to_byte()), limbs.weight);
    let again = run(&mut physics, fly, 30);
    let off = shared::vec3::distance(ahead.position, again.position);
    println!("replayed in the air {off} m off");
    assert!(off < 0.05 && physics.limbs(1).unwrap().step == shared::vehicles::Step::Airborne, "half a second on, it is where it was the first time: {off} m off");
}

/// 400 m square of 2 m cells: dry at 0 north of z = 0, then a lake whose bed drops at 1 in 4 to 10 m
/// deep, its still surface at 0.
fn deep_lake() -> WorldFile {
    let side = 201u32;
    let mut heights = Vec::new();
    let mut water = Vec::new();
    for j in 0..side {
        for _ in 0..side {
            let z = j as f32 * 2.0 - 200.0;
            let ground = if z > 0.0 { 0.0 } else { -(-z * 0.25).min(10.0) };
            heights.push((ground * 100.0).round() as i32);
            water.push(if ground < 0.0 { 0 } else { shared::world::NO_WATER });
        }
    }
    WorldFile { ground: shared::terrain::Ground { origin: -20_000, cell: 200, side, heights }, water, ..flat() }
}

#[test]
fn a_titan_walks_into_a_lake_on_its_bed_at_its_full_speed() {
    let mut physics = Physics::new(&deep_lake()).unwrap();
    physics.add_car(1, KIND_TITAN, [0.0, TITAN_LEG_LENGTH + TITAN_HALF_EXTENTS[1] + 0.5, 40.0], 0.0);
    run(&mut physics, COAST, 300);
    // Walked from the shore to where the lake is 10 m deep.
    let mut slowest = f32::MAX;
    let mut pose = physics.car_pose(1).unwrap();
    while pose.position[2] > -120.0 {
        pose = run(&mut physics, CarControls { throttle: 1.0, ..DRIVEN }, 1);
        if pose.position[2] < -10.0 {
            slowest = slowest.min(pose.forward_speed);
        }
    }
    let pose = run(&mut physics, COAST, 300);
    let limbs = physics.limbs(1).unwrap();
    println!("titan in the lake: {pose:?}, feet {:?}, slowest {slowest} m/s", limbs.feet);
    assert!(slowest > 0.8 * TITAN_MAX_SPEED, "the water does not hold it back: {slowest} m/s");
    assert!(limbs.feet.iter().all(|foot| (foot[1] + 10.0).abs() < 0.1), "both feet stand on the bed: {:?}", limbs.feet);
    // Its feet are a stride apart, so it stands a little lower than on both under its hips.
    assert!((pose.position[1] - (TITAN_RIDE_HEIGHT - 10.0)).abs() < 1.5, "its hull rides its height over the bed, not the surface: {pose:?}");
}


