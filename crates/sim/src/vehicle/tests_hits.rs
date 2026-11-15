//! What a round does to a vehicle where it strikes it, and what of the Titan a round can meet.

use super::*;
use shared::vec3;
use shared::consts::TICK_DT;
use shared::protocol::{KIND_TANK, KIND_TITAN};
use shared::world::{WORLD_VERSION, WorldFile};

fn flat() -> WorldFile {
    WorldFile { version: WORLD_VERSION, half_extent: 20_000, ..Default::default() }
}

/// A piloted vehicle that is asked for nothing.
const PILOTED: CarControls = CarControls { powered: true, ..CarControls::IDLE };

fn run(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
    for _ in 0..ticks {
        physics.drive_car(1, controls, TICK_DT);
        physics.step_cars(TICK_DT);
    }
    physics.car_pose(1).unwrap()
}

/// A Titan standing at the origin facing -Z, settled on its legs.
fn titan() -> Physics {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_TITAN, [0.0, TITAN_LEG_LENGTH + TITAN_HALF_EXTENTS[1], 0.0], 0.0);
    run(&mut physics, PILOTED, 300);
    physics
}

/// How far the hull leans from upright, radians.
fn tilt(pose: &CarPose) -> f32 {
    pose.up_y.clamp(-1.0, 1.0).acos()
}

/// Where a round from straight ahead of the Titan (from -Z) meets it at hull-frame height `y` and
/// sideways `x`: the distance flown from 60 m out and the vehicle it met, if any.
fn from_ahead(physics: &Physics, x: f32, y: f32) -> (Option<(f32, Option<u32>)>, Option<Strike>) {
    let pose = physics.car_pose(1).unwrap();
    let origin = pose.world_point([x, y, -60.0]);
    let dir = pose.world_dir([0.0, 0.0, 1.0]);
    (physics.cast_ray_hit(origin, dir, 120.0, 0), physics.armor_strike(origin, dir, 120.0, 1))
}

/// An armour-piercing shell's knock on the Titan's chest, from straight ahead, and how it moves after:
/// the most it leans, and how long until it is upright and still again, seconds. Fails if it falls.
fn rocked(controls: CarControls) -> (f32, f32) {
    let mut physics = titan();
    let pose = physics.car_pose(1).unwrap();
    let dir = pose.world_dir([0.0, 0.0, 1.0]);
    let origin = pose.world_point([0.0, 4.0, -60.0]);
    let strike = physics.armor_strike(origin, dir, 120.0, 1).expect("a round at the chest meets it");
    physics.strike_car(1, dir, pose.world_point(strike.point), AP_HIT_KNOCK);
    let (mut most, mut settled) = (0.0f32, 0.0);
    for tick in 1..=360 {
        let pose = run(&mut physics, controls, 1);
        assert!(pose.up_y > 0.9, "it never falls: {pose:?}");
        most = most.max(tilt(&pose));
        let spin = physics.car_motion(1).unwrap().angvel;
        let still = tilt(&pose) < 0.15f32.to_radians() && vec3::length(spin) < 0.01 && vec3::length(pose.velocity) < 0.1;
        if !still {
            settled = tick as f32 * TICK_DT;
        }
    }
    (most.to_degrees(), settled)
}

#[test]
fn an_ap_shell_on_the_titans_chest_rocks_it_and_it_stands_again() {
    let (most, settled) = rocked(PILOTED);
    println!("piloted titan rocked {most:.2} degrees, still again after {settled:.2} s");
    assert!((1.5..3.0).contains(&most), "the hit sways it, visibly but not far: {most} degrees");
    assert!(settled < 2.5, "it is upright and still again within a couple of seconds: {settled} s");
    // Nobody aboard, it stands up to it as well.
    let (most, settled) = rocked(CarControls::IDLE);
    println!("empty titan rocked {most:.2} degrees, still again after {settled:.2} s");
    assert!(most < 3.0 && settled < 2.5, "an empty one rocks and settles too: {most} degrees, {settled} s");
}

#[test]
fn a_shell_on_a_tanks_side_rocks_it_away_from_the_hit() {
    let mut physics = Physics::new(&flat()).unwrap();
    physics.add_car(1, KIND_TANK, [0.0, 2.0, 0.0], 0.0);
    let settled = run(&mut physics, CarControls::IDLE, 180);
    let [hx, hy, _] = spec(KIND_TANK).half_extents;
    // On its left side, high up, going right.
    physics.strike_car(1, [1.0, 0.0, 0.0], settled.world_point([-hx, 0.8 * hy, 0.0]), AP_HIT_KNOCK);
    let motion = physics.car_motion(1).unwrap();
    println!("tank struck: {motion:?}");
    assert!(motion.linvel[0] > 0.5 * AP_HIT_KNOCK, "pushed away from the hit: {motion:?}");
    assert!(motion.angvel[2] < -0.05, "rolled away from it, its top going right: {motion:?}");
    let rolled = run(&mut physics, CarControls::IDLE, 3);
    assert!(rolled.world_dir([0.0, 1.0, 0.0])[0] > 0.0, "it leans away from the hit: {rolled:?}");
    let after = run(&mut physics, CarControls::IDLE, 240);
    assert!(after.up_y > 0.999, "and settles back on its tracks: {after:?}");
}

#[test]
fn a_round_through_the_titans_arm_hits_it() {
    let physics = titan();
    let pose = physics.car_pose(1).unwrap();
    let bones = physics.hit_bones(1);
    assert_eq!(bones.len(), 8, "two arms and two legs of two bones each: {bones:?}");
    // The middle of each arm's bones, from straight ahead: the box it walks with is nowhere near them.
    for (index, &(from, to, radius)) in bones.iter().enumerate().filter(|(index, _)| *index < 4) {
        let middle = pose.local_dir(vec3::sub(std::array::from_fn(|i| (from[i] + to[i]) / 2.0), pose.position));
        assert!(middle[0].abs() > TITAN_HALF_EXTENTS[0] + 1.0, "bone {index} is clear of the box: {middle:?}");
        let (hit, strike) = from_ahead(&physics, middle[0], middle[1]);
        let (distance, car) = hit.expect("the round meets the arm");
        println!("bone {index} at {middle:?} met {distance} m out");
        assert_eq!(car, Some(1), "it is the Titan it meets");
        let near = 60.0 + middle[2];
        assert!((near - radius - 0.6..=near).contains(&distance), "at the arm's near side: {distance} m against its middle {near} m");
        let strike = strike.expect("its armour sees the same hit");
        assert!((strike.distance - distance).abs() < 1e-3 && strike.solid.is_none(), "{strike:?}");
    }
}

#[test]
fn a_round_past_the_titans_waist_misses() {
    let physics = titan();
    // Inside the box it walks with, but beside the narrow waist, above the thighs and below the flanks.
    let (hit, strike) = from_ahead(&physics, 5.0, -4.5);
    assert!(hit.is_none() && strike.is_none(), "it flies on past: {hit:?} {strike:?}");
    // The box itself is still there for it to stand on.
    let pose = physics.car_pose(1).unwrap();
    let ray = Ray::new(Vector::from(pose.world_point([5.0, -4.5, -60.0])), Vector::from(pose.world_dir([0.0, 0.0, 1.0])));
    let queries = physics.broad_phase.as_query_pipeline(physics.narrow_phase.query_dispatcher(), &physics.bodies, &physics.colliders, QueryFilter::default());
    assert!(queries.cast_ray(&ray, 120.0, true).is_some(), "the hull's box is in the physics as before");
    // A little further in, it meets the waist.
    let (hit, _) = from_ahead(&physics, 3.5, -4.5);
    assert_eq!(hit.and_then(|(_, car)| car), Some(1), "the waist itself is hit");
}

#[test]
fn a_round_at_the_titans_head_hits_it() {
    let physics = titan();
    // The visor, above the top of the box it walks with.
    let (hit, strike) = from_ahead(&physics, 0.0, 11.6);
    let (distance, car) = hit.expect("the round meets the head");
    assert_eq!(car, Some(1));
    assert!((distance - (60.0 - 3.25)).abs() < 0.05, "on the face of it: {distance} m");
    let strike = strike.expect("its armour sees it");
    assert!(strike.normal[2] < -0.99 && strike.solid.is_none(), "it faces the round, on the hull: {strike:?}");
    // The crest is the very top.
    let (hit, _) = from_ahead(&physics, 0.0, 14.0);
    assert_eq!(hit.and_then(|(_, car)| car), Some(1), "the crest is hit");
    let (hit, _) = from_ahead(&physics, 0.0, 14.5);
    assert!(hit.is_none(), "over the top of it is a miss: {hit:?}");
    // And the pauldron, out past the box's side.
    let (hit, _) = from_ahead(&physics, 11.5, 9.0);
    assert_eq!(hit.and_then(|(_, car)| car), Some(1), "the pauldron is hit");
}

#[test]
fn a_round_at_the_titans_legs_meets_them_where_they_bend() {
    let physics = titan();
    let pose = physics.car_pose(1).unwrap();
    let bones = physics.hit_bones(1);
    // Each thigh and shin, from ahead, at its middle.
    for (index, &(from, to, _)) in bones.iter().enumerate().filter(|(index, _)| *index >= 4) {
        let middle = pose.local_dir(vec3::sub(std::array::from_fn(|i| (from[i] + to[i]) / 2.0), pose.position));
        let (hit, _) = from_ahead(&physics, middle[0], middle[1]);
        assert_eq!(hit.and_then(|(_, car)| car), Some(1), "bone {index} at {middle:?} is hit");
    }
    // The knees bend forward, as they are drawn.
    let knee = pose.local_dir(vec3::sub(bones[4].1, pose.position));
    assert!(knee[2] < -1.0, "the knee is ahead of the hip: {knee:?}");
}

