use super::*;
use crate::entities::{Health, HurtBy};
use crate::game::{Command, Game, Outgoing};
use sim::STANDING_Y;
use shared::protocol::{InputFrame, KIND_DRONE, THRUST_AIR, THRUST_BOOST, THRUST_LIFT};
use shared::world::{Road, WORLD_VERSION};
use tokio::sync::{mpsc, oneshot};

/// A flat world with one road along X, and spawn points on it.
fn game() -> Game {
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 30_000,
        // Twelve spots in the middle for the cars, tanks, helicopter and walker, and one far out at each end
        // for the planes. Players start on the first six.
        spawns: [[-25_000, 0], [25_000, 0]].into_iter().chain((0..12).map(|i| [i * 2500 - 13_750, 0])).collect(),
        roads: vec![Road { width: 800, kind: 0, points: vec![[-30_000, 0], [30_000, 0]] }],
        ..Default::default()
    };
    // Loose items would take a test's use button.
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// Feeds one frame per tick, with the frame counter owned by the caller.
fn press(game: &mut Game, id: u32, seq: &mut u32, axes: (i8, i8), buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: axes.0, forward: axes.1, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
}

/// Settles the jitter buffer: two frames must queue before any is consumed.
fn idle(game: &mut Game, id: u32, seq: &mut u32, ticks: u32) {
    press(game, id, seq, (0, 0), 0, ticks);
}

fn move_player_next_to_a_car(game: &mut Game, id: u32) -> u32 {
    let (vid, _) = game.world.entities.all::<Vehicle>().next().unwrap();
    let pose = game.world.physics.car_pose(vid).unwrap();
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character.position = [pose.position[0] + 2.0, STANDING_Y, pose.position[2]];
    vid
}

#[test]
fn cars_are_placed_along_the_road() {
    let game = game();
    assert_eq!(game.world.entities.count::<Vehicle>(), CAR_COUNT + ROADSTER_COUNT + TANK_COUNT + STRV_COUNT + HELI_COUNT + WALKER_COUNT + PLANE_COUNT + game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_MORTAR).count());
    // The mortar only takes a spawn the others left, and this world has none spare.
    let mortars = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_MORTAR).count();
    assert!(mortars <= MORTAR_COUNT, "{mortars} mortars");
    let planes = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_PLANE).count();
    assert_eq!(planes, PLANE_COUNT);
    let tanks = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_TANK).count();
    assert_eq!(tanks, TANK_COUNT);
    let strvs = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_STRV).count();
    assert_eq!(strvs, STRV_COUNT);
    let helis = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_HELI).count();
    assert_eq!(helis, HELI_COUNT);
    let walkers = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_WALKER).count();
    assert_eq!(walkers, WALKER_COUNT);
    let count = |kind| game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == kind).count();
    assert_eq!(count(KIND_JEEP), CAR_COUNT / 2, "half the cars are jeeps");
    assert_eq!(count(KIND_SPORTS), CAR_COUNT / 2, "and half are sports cars");
    assert_eq!(count(KIND_ROADSTER), ROADSTER_COUNT);
    for vid in game.world.entities.ids::<With<Vehicle>>() {
        let pose = game.world.physics.car_pose(vid).unwrap();
        // The road runs along X, so the car faces +X or -X: |yaw| is a quarter turn.
        assert!((pose.yaw().abs() - std::f32::consts::FRAC_PI_2).abs() < 0.05, "{}", pose.yaw());
    }
}

#[test]
fn use_boards_the_nearest_car_and_use_again_leaves() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, id, &mut seq, 30);
    let vid = move_player_next_to_a_car(&mut game, id);

    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(id));
    assert!(game.seen_people(shared::protocol::KIND_STICKMAN).is_empty(), "a seated player is not a stickman entity");
    assert_eq!(game.seen(vid).unwrap().seats.unwrap().riders[0], id);

    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert!(game.world.entities.of::<Seats>(vid).vacant());
    let states = game.seen_people(shared::protocol::KIND_STICKMAN);
    assert_eq!(states.len(), 1);
    // The stickman stands beside the car, not inside it.
    let pose = game.world.physics.car_pose(vid).unwrap();
    let place = states[0].1.place.unwrap();
    let (dx, dz) = (place.x as f32 / 100.0 - pose.position[0], place.z as f32 / 100.0 - pose.position[2]);
    assert!(dx.hypot(dz) > 1.5, "{dx} {dz}");
}

#[test]
fn a_player_out_of_reach_does_not_board() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, id, &mut seq, 30);
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [0.0, STANDING_Y, 15.0];
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert!(game.world.entities.of::<Player>(id).seat.is_none());
}

#[test]
fn the_driver_steers_and_passengers_ride_along() {
    let mut game = game();
    let (driver, _a) = join(&mut game);
    let (rider, _b) = join(&mut game);
    let (mut seq_a, mut seq_b) = (0, 0);
    idle(&mut game, driver, &mut seq_a, 3);
    idle(&mut game, rider, &mut seq_b, 3);
    let vid = move_player_next_to_a_car(&mut game, driver);
    move_player_next_to_a_car(&mut game, rider);

    // Drivers board first so they take seat 0.
    press(&mut game, driver, &mut seq_a, (0, 0), BUTTON_USE, 1);
    idle(&mut game, driver, &mut seq_a, 3);
    press(&mut game, rider, &mut seq_b, (0, 0), BUTTON_USE, 1);
    idle(&mut game, rider, &mut seq_b, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[..2], [Some(driver), Some(rider)]);

    let start = game.world.physics.car_pose(vid).unwrap().position;
    for _ in 0..180 {
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        // A passenger's stick does nothing.
        game.handle(Command::Input(rider, 0, vec![InputFrame { seq: seq_b, strafe: 127, forward: -127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    let end = game.world.physics.car_pose(vid).unwrap();
    let travelled = (end.position[0] - start[0]).hypot(end.position[2] - start[2]);
    assert!(travelled > 5.0, "the car drove {travelled} m");
    let rider_at = game.world.entities.of::<Player>(rider).character.position;
    assert_eq!(rider_at, end.position, "passengers move with the car");

    // Only the driver is told the car's exact motion, so they can predict it.
    let drive = game.drive_state(driver).expect("the driver gets the car's motion");
    let speed_cm = (drive.vx as f32).hypot(drive.vz as f32);
    assert!(speed_cm > 500.0, "driving at over 5 m/s: {drive:?}");
    assert_eq!(game.drive_state(rider), None);
}

#[test]
fn a_pilot_lifts_off_with_one_passenger() {
    let mut game = game();
    let (pilot, _a) = join(&mut game);
    let (rider, _b) = join(&mut game);
    let (third, _c) = join(&mut game);
    let (mut seq_a, mut seq_b, mut seq_c) = (0, 0, 0);
    for (id, seq) in [(pilot, &mut seq_a), (rider, &mut seq_b), (third, &mut seq_c)] {
        idle(&mut game, id, seq, 3);
    }
    let (vid, _) = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_HELI).expect("a helicopter is placed");
    let start = game.world.physics.car_pose(vid).unwrap();
    for id in [pilot, rider, third] {
        game.world.entities.get_mut::<Player>(id).unwrap().character.position = [start.position[0] + 2.5, STANDING_Y, start.position[2]];
    }
    for (id, seq) in [(pilot, &mut seq_a), (rider, &mut seq_b), (third, &mut seq_c)] {
        press(&mut game, id, seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, id, seq, 3);
    }
    assert_eq!(game.world.entities.of::<Seats>(vid)[..], [Some(pilot), Some(rider)], "two seats, and the pilot first");
    assert!(game.world.entities.of::<Player>(third).seat.is_none());

    // Jump climbs: a second of rotor spin-up is not needed, the lift is immediate.
    press(&mut game, pilot, &mut seq_a, (0, 0), BUTTON_JUMP, 120);
    let end = game.world.physics.car_pose(vid).unwrap();
    assert!(end.position[1] > start.position[1] + 3.0, "climbed from {} to {}", start.position[1], end.position[1]);
    assert_eq!(game.world.entities.of::<Player>(rider).character.position, end.position, "the passenger goes up with it");
    let drive = game.drive_state(pilot).expect("the pilot gets the motion");
    assert!(drive.vy > 100, "climbing at over 1 m/s: {drive:?}");

    // Let go of the stick: it hovers where it is.
    idle(&mut game, pilot, &mut seq_a, 240);
    let hover = game.world.physics.car_pose(vid).unwrap();
    idle(&mut game, pilot, &mut seq_a, 60);
    let later = game.world.physics.car_pose(vid).unwrap();
    assert!((later.position[1] - hover.position[1]).abs() < 0.5, "hovering: {} -> {}", hover.position[1], later.position[1]);
}

#[test]
fn leaving_the_game_frees_the_seat() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, id, &mut seq, 30);
    let vid = move_player_next_to_a_car(&mut game, id);
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(id));
    game.handle(Command::Leave(id));
    assert!(game.world.entities.of::<Seats>(vid).vacant());
}

#[test]
fn a_car_on_its_roof_is_put_back_on_its_wheels() {
    let mut game = game();
    let (vid, _) = game.world.entities.all::<Vehicle>().next().unwrap();
    let pose = game.world.physics.car_pose(vid).unwrap();
    // Flip it by resetting the body upside down.
    game.world.physics.set_car_roll(vid, std::f32::consts::PI, [pose.position[0], 1.5, pose.position[2]]);
    for _ in 0..(UNFLIP_TICKS + 240) {
        game.step();
    }
    assert!(game.world.physics.car_pose(vid).unwrap().up_y > 0.9);
}

#[test]
fn a_fallen_walker_is_stood_on_its_feet_and_unhurt() {
    let mut game = game();
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_WALKER).unwrap().0;
    let hp = game.world.entities.of::<Health>(vid).hp;
    let pose = game.world.physics.car_pose(vid).unwrap();
    // On its back, its hull on the ground, where its get-up pull cannot turn it.
    game.world.physics.set_car_roll(vid, std::f32::consts::PI, [pose.position[0], 1.3, pose.position[2]]);
    for _ in 0..(UNFLIP_TICKS + 5 * TICK_RATE) {
        game.step();
    }
    let after = game.world.physics.car_pose(vid).unwrap();
    let ground = game.world.map.ground.height_at(after.position[0], after.position[2]);
    assert!(after.up_y > 0.95, "back on its feet: {after:?}");
    assert!(after.position[1] > ground + WALKER_LEG_LENGTH - 1.0, "standing, not sunk: {after:?}");
    assert!(game.world.entities.of::<Health>(vid).hp + 10 >= hp, "standing up did not hurt it: {} of {hp}", game.world.entities.of::<Health>(vid).hp);
}

#[test]
fn a_driven_car_knocks_back_a_player_in_its_way() {
    let mut game = game();
    let (driver, _a) = join(&mut game);
    let (victim, _b) = join(&mut game);
    let (mut seq_a, mut seq_b) = (0, 0);
    idle(&mut game, driver, &mut seq_a, 3);
    idle(&mut game, victim, &mut seq_b, 3);
    let vid = move_player_next_to_a_car(&mut game, driver);
    press(&mut game, driver, &mut seq_a, (0, 0), BUTTON_USE, 1);
    idle(&mut game, driver, &mut seq_a, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(driver));

    // Stand the victim 12 m ahead of the nose.
    let pose = game.world.physics.car_pose(vid).unwrap();
    let (sin, cos) = pose.yaw().sin_cos();
    let ahead = [-sin, -cos];
    let spot = [pose.position[0] + ahead[0] * 12.0, STANDING_Y, pose.position[2] + ahead[1] * 12.0];
    game.world.entities.get_mut::<Player>(victim).unwrap().character.position = spot;

    let mut peak_knock: f32 = 0.0;
    let mut hopped = false;
    let mut lowest_hp = PLAYER_MAX_HP;
    for _ in 0..240 {
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.handle(Command::Input(victim, 0, vec![InputFrame { seq: seq_b, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
        let character = game.world.entities.of::<Player>(victim).character;
        peak_knock = peak_knock.max(character.knock[0].hypot(character.knock[1]));
        hopped |= character.position[1] > STANDING_Y + 0.3;
        lowest_hp = lowest_hp.min(game.world.entities.of::<Player>(victim).hp);
    }
    assert!(lowest_hp < PLAYER_MAX_HP, "a car at speed hurts: lowest hp {lowest_hp}");
    assert!(peak_knock > KNOCK_MIN_PUSH, "peak knockback {peak_knock}");
    assert!(hopped, "a hit sends the player up");
    let character = game.world.entities.of::<Player>(victim).character;
    let moved = (character.position[0] - spot[0]) * ahead[0] + (character.position[2] - spot[2]) * ahead[1];
    assert!(moved > 2.0, "thrown {moved} m along the car's heading");
    assert!(game.world.entities.of::<Player>(victim).seat.is_none());
    // And it shows on the wire.
    assert!(game.seen(victim).is_some());
}

/// A driver in a car and a second player standing on its roof.
struct RoofScene {
    vid: u32,
    driver: u32,
    rider: u32,
    /// Where the car started, `[x, z]`.
    start: [f32; 2],
    /// The next input sequence numbers of the driver and the rider.
    seqs: (u32, u32),
    _inboxes: (mpsc::Receiver<Outgoing>, mpsc::Receiver<Outgoing>),
}

fn driver_and_roof_rider(game: &mut Game) -> RoofScene {
    let (driver, a) = join(game);
    let (rider, b) = join(game);
    let (mut seq_a, mut seq_b) = (0, 0);
    idle(game, driver, &mut seq_a, 3);
    idle(game, rider, &mut seq_b, 3);
    let vid = move_player_next_to_a_car(game, driver);
    press(game, driver, &mut seq_a, (0, 0), BUTTON_USE, 1);
    idle(game, driver, &mut seq_a, 3);
    let pose = game.world.physics.car_pose(vid).unwrap();
    // Drop the rider on the roof; it settles a tick or two later.
    let top = pose.position[1] + JEEP_HALF_EXTENTS[1] + STANDING_Y + 0.05;
    game.world.entities.get_mut::<Player>(rider).unwrap().character.position = [pose.position[0], top, pose.position[2]];
    for _ in 0..30 {
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.handle(Command::Input(rider, 0, vec![InputFrame { seq: seq_b, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    RoofScene { vid, driver, rider, start: [pose.position[0], pose.position[2]], seqs: (seq_a, seq_b), _inboxes: (a, b) }
}

#[test]
fn a_player_on_the_roof_rides_along() {
    let mut game = game();
    let RoofScene { vid, driver, rider, start, seqs: (mut seq_a, mut seq_b), .. } = driver_and_roof_rider(&mut game);
    assert_eq!(game.world.entities.of::<Player>(rider).character.ride, vid, "standing on the roof");
    for _ in 0..240 {
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.handle(Command::Input(rider, 0, vec![InputFrame { seq: seq_b, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    let car = game.world.physics.car_pose(vid).unwrap();
    let on = game.world.entities.of::<Player>(rider).character;
    let travelled = (car.position[0] - start[0]).hypot(car.position[2] - start[1]);
    assert!(travelled > 8.0, "the car drove {travelled} m");
    assert_eq!(on.ride, vid);
    let apart = (on.position[0] - car.position[0]).hypot(on.position[2] - car.position[2]);
    assert!(apart < 1.5, "the rider is {apart} m from the car's centre");
    assert!(on.position[1] > car.position[1] + 0.8, "still on top: {:?} vs {:?}", on.position, car.position);
    assert_eq!(game.world.entities.of::<Player>(rider).hit_cooldown, 0, "a rider is never run over by their own car");
}

#[test]
fn jumping_off_a_moving_car_keeps_its_speed() {
    let mut game = game();
    let RoofScene { vid, driver, rider, seqs: (mut seq_a, mut seq_b), .. } = driver_and_roof_rider(&mut game);
    for _ in 0..180 {
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.handle(Command::Input(rider, 0, vec![InputFrame { seq: seq_b, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    let speed = game.world.physics.car_pose(vid).unwrap().velocity;
    let speed = speed[0].hypot(speed[2]);
    assert!(speed > 6.0, "the car is moving at {speed}");
    seq_a += 1;
    seq_b += 1;
    game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
    game.handle(Command::Input(rider, 0, vec![InputFrame { seq: seq_b, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_JUMP, pitch: 0, view: 0, slot: 0 }]));
    game.step();
    // The jitter buffer holds one frame back, so the jump is applied on the next tick.
    seq_a += 1;
    seq_b += 1;
    game.handle(Command::Input(driver, 0, vec![InputFrame { seq: seq_a, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
    game.handle(Command::Input(rider, 0, vec![InputFrame { seq: seq_b, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
    game.step();
    let left = game.world.entities.of::<Player>(rider).character;
    assert_eq!(left.ride, 0);
    assert!(!left.grounded);
    let knock = left.knock[0].hypot(left.knock[1]);
    assert!((knock - speed).abs() < 2.0, "carries {knock} m/s of the car's {speed}");
}

#[test]
fn parked_cars_in_the_fixture_world_settle_and_stay_put() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack");
    let (world, hash) = crate::read_world(&path).unwrap();
    let mut game = Game::new(world, hash).unwrap();
    assert!(game.world.entities.count::<Vehicle>() >= 3, "only {} cars fit", game.world.entities.count::<Vehicle>());
    let start: Vec<_> = game.world.entities.ids::<With<Vehicle>>().into_iter().map(|id| (id, game.world.physics.car_pose(id).unwrap())).collect();
    for _ in 0..5 * TICK_RATE {
        game.step();
    }
    for (id, before) in start {
        let after = game.world.physics.car_pose(id).unwrap();
        let moved = (after.position[0] - before.position[0]).hypot(after.position[2] - before.position[2]);
        let ground = game.world.map.ground.height_at(after.position[0], after.position[2]);
        // A plane stands taller on its undercarriage than any car, and a walker's or a hen's hull is up on its legs.
        let ride = match game.world.entities.of::<Vehicle>(id).kind {
            KIND_PLANE => 1.3,
            KIND_STRV => 1.2,
            KIND_WALKER | KIND_HEN => WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1],
            _ => 1.0,
        };
        assert!(after.up_y > 0.99 && after.position[1] < ground + ride && moved < 0.5, "car {id}: {before:?} -> {after:?}");
    }
    let planes = game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|vehicle| vehicle.kind == KIND_PLANE).count();
    assert_eq!(planes, PLANE_COUNT, "both planes find somewhere to stand");
    for (id, _) in game.world.entities.all::<Vehicle>().filter(|(_, vehicle)| vehicle.kind == KIND_PLANE) {
        let pose = game.world.physics.car_pose(id).unwrap();
        let runway = game.world.physics.runway(&game.world.map.ground, pose.position, pose.yaw(), RUNWAY_SEARCH);
        assert!(runway >= PLANE_RUNWAY, "plane {id} has {runway} m to take off in");
    }
}

#[test]
fn a_tank_has_a_driver_and_a_gunner_and_no_third_seat() {
    let mut game = game();
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_TANK).unwrap().0;
    let pose = game.world.physics.car_pose(vid).unwrap();
    let mut crew = Vec::new();
    for _ in 0..3 {
        let (id, inbox) = join(&mut game);
        crew.push((id, inbox, 0));
    }
    for (id, _, seq) in &mut crew {
        idle(&mut game, *id, seq, 3);
    }
    for (id, _, seq) in &mut crew {
        game.world.entities.get_mut::<Player>(*id).unwrap().character.position = [pose.position[0] + 2.5, STANDING_Y, pose.position[2]];
        press(&mut game, *id, seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, *id, seq, 3);
    }
    let seated: Vec<_> = crew.iter().map(|(id, ..)| game.world.entities.of::<Player>(*id).seat.map(|seat| (seat.vehicle, seat.index))).collect();
    assert_eq!(seated, [Some((vid, 0)), Some((vid, 1)), None], "two seats, the third stays outside");
    let state = game.seen(vid).unwrap();
    assert_eq!(state.seats.unwrap().riders.len(), 2);
    assert!(state.turret.is_some(), "a tank reports its turret");
}

#[test]
fn a_lone_crewman_can_switch_between_driver_and_gunner() {
    let mut game = game();
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_TANK).unwrap().0;
    let pose = game.world.physics.car_pose(vid).unwrap();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, id, &mut seq, 3);
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [pose.position[0] + 2.5, STANDING_Y, pose.position[2]];
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[..2], [Some(id), None]);

    press(&mut game, id, &mut seq, (0, 0), BUTTON_RELOAD, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[..2], [None, Some(id)]);
    assert_eq!(game.world.entities.of::<Player>(id).seat, Some(Seat { vehicle: vid, index: 1, remote: false }));

    press(&mut game, id, &mut seq, (0, 0), BUTTON_RELOAD, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[..2], [Some(id), None]);
}

#[test]
fn a_full_tank_has_nowhere_to_switch_to() {
    let mut game = game();
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_TANK).unwrap().0;
    let pose = game.world.physics.car_pose(vid).unwrap();
    let mut crew = Vec::new();
    for _ in 0..2 {
        let (id, inbox) = join(&mut game);
        crew.push((id, inbox, 0));
    }
    for (id, _, seq) in &mut crew {
        idle(&mut game, *id, seq, 3);
        game.world.entities.get_mut::<Player>(*id).unwrap().character.position = [pose.position[0] + 2.5, STANDING_Y, pose.position[2]];
        press(&mut game, *id, seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, *id, seq, 3);
    }
    let (first, _, seq) = &mut crew[0];
    press(&mut game, *first, seq, (0, 0), BUTTON_RELOAD, 1);
    idle(&mut game, *first, seq, 3);
    let seats = game.world.entities.of::<Seats>(vid);
    assert_eq!(seats[..2], [Some(crew[0].0), Some(crew[1].0)]);
}

/// Two cars off the road, `a` facing -Z towards the flank of `b` 12 m ahead, settled.
fn two_cars(game: &mut Game) -> (u32, u32) {
    let mut cars = game.world.entities.all::<Vehicle>().filter(|(_, v)| v.kind == KIND_JEEP).map(|(id, _)| id);
    let (a, b) = (cars.next().unwrap(), cars.next().unwrap());
    game.world.physics.reset_car(a, [15.0, 1.0, 30.0], 0.0);
    game.world.physics.reset_car(b, [15.0, 1.0, 18.0], std::f32::consts::FRAC_PI_2);
    for _ in 0..60 {
        Vehicles.on_tick(&mut game.world, TICK_DT);
    }
    (a, b)
}

/// Steps the vehicles until `ticks` have passed, and returns the crash and scrape events seen.
fn crash_events(game: &mut Game, ticks: u32) -> Vec<ShotEvent> {
    let mut seen = vec![];
    for _ in 0..ticks {
        Vehicles.on_tick(&mut game.world, TICK_DT);
        seen.extend(game.world.shots.drain(..).filter(|shot| shot.weapon == WEAPON_CRASH || shot.weapon == WEAPON_SCRAPE));
    }
    seen
}

#[test]
fn ramming_a_car_hurts_both_and_blames_the_driver() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let (a, b) = two_cars(&mut game);
    game.world.entities.get_mut::<Seats>(a).unwrap()[0] = Some(id);
    game.world.entities.get_mut::<Player>(id).unwrap().seat = Some(Seat { vehicle: a, index: 0, remote: false });
    game.world.physics.kick_car(a, [0.0, 0.0, -22.0]);
    let events = crash_events(&mut game, 60);
    let (rammer, rammed) = (&game.world.entities.of::<Vehicle>(a), &game.world.entities.of::<Vehicle>(b));
    let hp = |vid| game.world.entities.of::<Health>(vid).hp;
    assert!(hp(a) < vehicle_hp(KIND_JEEP) && hp(b) < vehicle_hp(KIND_JEEP), "{} {}", hp(a), hp(b));
    assert!(rammer.damage[DAMAGE_FRONT] > 50, "the rammer's nose: {:?}", rammer.damage);
    // `b` faces -X, so its left side faces +Z, where `a` came from.
    assert!(rammed.damage[DAMAGE_LEFT] > 50 && rammed.damage[DAMAGE_RIGHT] == 0, "the rammed car's flank: {:?}", rammed.damage);
    assert_eq!(game.world.entities.get::<HurtBy>(b), Some(&HurtBy(id)));
    assert!(events.iter().any(|shot| shot.shooter == a && shot.weapon == WEAPON_CRASH), "{events:?}");
    let hit = events.iter().find(|shot| shot.weapon == WEAPON_CRASH).unwrap();
    assert!((hit.z as f32 / 100.0 - 19.0).abs() < 2.0, "where they met: {hit:?}");
}

#[test]
fn a_nudge_does_no_harm() {
    let mut game = game();
    let (a, b) = two_cars(&mut game);
    game.world.physics.kick_car(a, [0.0, 0.0, -3.0]);
    let events = crash_events(&mut game, 300);
    assert_eq!((game.world.entities.of::<Health>(a).hp, game.world.entities.of::<Health>(b).hp), (vehicle_hp(KIND_JEEP), vehicle_hp(KIND_JEEP)));
    assert_eq!(game.world.entities.of::<Vehicle>(b).damage, [0; 8]);
    assert!(events.iter().all(|shot| shot.weapon != WEAPON_CRASH), "{events:?}");
}

#[test]
fn a_wheel_takes_a_hit_beside_it_and_a_wreck_comes_back_whole() {
    let mut damage = [0; 8];
    // Just behind the front left wheel, on the left side.
    dent(KIND_JEEP, &mut damage, [-JEEP_HALF_EXTENTS[0], 0.0, -JEEP_AXLE_Z + 0.4], 15.0);
    assert!(damage[DAMAGE_LEFT] > 100 && damage[DAMAGE_RIGHT] == 0, "{damage:?}");
    assert!(damage[DAMAGE_WHEELS] > 100, "the front left wheel: {damage:?}");
    assert_eq!(damage[DAMAGE_WHEELS + 3], 0, "the far rear wheel is untouched: {damage:?}");
    dent(KIND_JEEP, &mut damage, [-JEEP_HALF_EXTENTS[0], 0.0, -JEEP_AXLE_Z], 30.0);
    assert_eq!(damage[DAMAGE_WHEELS], shared::protocol::WHEEL_GONE, "a second hit takes it off: {damage:?}");

    let mut game = game();
    let (a, _) = two_cars(&mut game);
    game.world.entities.get_mut::<Vehicle>(a).unwrap().damage = damage;
    game.world.entities.insert(a, HurtBy(7));
    super::super::combat::damage_vehicle(&mut game.world, a, 255, None);
    for _ in 0..WRECK_TICKS + 2 {
        Vehicles.on_tick(&mut game.world, TICK_DT);
    }
    assert_eq!((game.world.entities.of::<Health>(a).hp, game.world.entities.of::<Vehicle>(a).damage), (vehicle_hp(KIND_JEEP), [0; 8]));
    assert!(game.world.entities.get::<HurtBy>(a).is_none(), "nobody is blamed for a fresh one");
}

#[test]
fn gunfire_at_a_wheel_bends_it_and_a_second_round_takes_it_off_but_the_middle_is_safe() {
    let mut game = game();
    let (a, b) = two_cars(&mut game);
    let pose = game.world.physics.car_pose(a).unwrap();
    let kind = game.world.entities.of::<Vehicle>(a).kind;
    let front_left = wheel_spots(kind)[0];
    // A revolver round (35) on the front left wheel bends it; two sniper rounds (90) take it off.
    let at = pose.world_point([front_left[0], 0.0, front_left[1]]);
    shoot_wheels(&mut game.world, a, at, 35.0, BULLET_WHEEL_REACH);
    let first = game.world.entities.of::<Vehicle>(a).damage[DAMAGE_WHEELS];
    assert!(first > 40, "one round bends it: {first}");
    assert_eq!(game.world.entities.of::<Vehicle>(a).damage[DAMAGE_WHEELS + 3], 0);
    shoot_wheels(&mut game.world, a, at, 90.0, BULLET_WHEEL_REACH);
    shoot_wheels(&mut game.world, a, at, 90.0, BULLET_WHEEL_REACH);
    assert_eq!(game.world.entities.of::<Vehicle>(a).damage[DAMAGE_WHEELS], shared::protocol::WHEEL_GONE);
    // Fire into the middle of the car: no wheel is within a bullet's reach.
    let middle = game.world.physics.car_pose(b).unwrap().position;
    shoot_wheels(&mut game.world, b, middle, 90.0, BULLET_WHEEL_REACH);
    assert_eq!(game.world.entities.of::<Vehicle>(b).damage[DAMAGE_WHEELS..], [0; 4]);
}

#[test]
fn a_plane_is_never_parked_on_another_ones_runway() {
    let game = game();
    let planes: Vec<CarPose> = game.world.entities.all::<Vehicle>().filter(|(_, v)| v.kind == KIND_PLANE).map(|(id, _)| game.world.physics.car_pose(id).unwrap()).collect();
    let [a, b] = &planes[..] else { panic!("two planes: {planes:?}") };
    assert!(!on_runway(a.position, a.yaw(), PLANE_RUNWAY, b.position) && !on_runway(b.position, b.yaw(), PLANE_RUNWAY, a.position), "{a:?} {b:?}");
    // Each faces in along the road, towards the middle, where it has room.
    assert!(a.position[0] * b.position[0] < 0.0, "one at each end: {a:?} {b:?}");
    for pose in [a, b] {
        let inward = -pose.position[0].signum();
        let (sin, _) = pose.yaw().sin_cos();
        assert!(-sin * inward > 0.99, "{pose:?}");
    }
    // Nothing parked in the way either.
    for (vid, _) in game.world.entities.all::<Vehicle>().filter(|(_, v)| v.kind != KIND_PLANE) {
        let other = game.world.physics.car_pose(vid).unwrap().position;
        assert!(!on_runway(a.position, a.yaw(), PLANE_RUNWAY, other) && !on_runway(b.position, b.yaw(), PLANE_RUNWAY, other), "{other:?}");
    }
}

#[test]
fn a_pilot_and_a_gunner_take_a_plane_off_and_a_third_stays_behind() {
    let mut game = game();
    let (vid, _) = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).expect("a plane is placed");
    let start = game.world.physics.car_pose(vid).unwrap();
    let mut crew = Vec::new();
    for _ in 0..3 {
        let (id, inbox) = join(&mut game);
        crew.push((id, inbox, 0));
    }
    for (id, _, seq) in &mut crew {
        idle(&mut game, *id, seq, 3);
        game.world.entities.get_mut::<Player>(*id).unwrap().character.position = [start.position[0], STANDING_Y, start.position[2] + 3.0];
        press(&mut game, *id, seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, *id, seq, 3);
    }
    let seated: Vec<_> = crew.iter().map(|(id, ..)| game.world.entities.of::<Player>(*id).seat.map(|seat| seat.index)).collect();
    assert_eq!(seated, [Some(0), Some(1), None], "a pilot, a gunner, and nobody else");
    let state = game.seen(vid).unwrap();
    assert_eq!(state.arms, Some(shared::replicated::Arms { shots: 0, heat: 0, rear_heat: 0, bombs: PLANE_BOMBS, flags: 0 }), "the plane reports its weapons");

    // Space for full throttle and a little back stick (S) to lift off, along the road.
    let (pilot, gunner, mut seq) = (crew[0].0, crew[1].0, crew[0].2);
    let yaw = shared::quant::to_mrad(start.yaw());
    for _ in 0..60 * 7 {
        seq += 1;
        game.handle(Command::Input(pilot, 0, vec![InputFrame { seq, strafe: 0, forward: -60, yaw, buttons: BUTTON_JUMP, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    let flying = game.world.physics.car_pose(vid).unwrap();
    assert!(flying.position[1] > start.position[1] + 5.0 && flying.forward_speed > PLANE_STALL_SPEED, "{start:?} -> {flying:?}");
    assert_eq!(game.world.entities.of::<Player>(gunner).character.position, flying.position, "the gunner goes up with it");
    assert!(game.drive_state(pilot).is_some(), "the pilot predicts the plane");
}

#[test]
fn a_gentle_landing_does_no_harm() {
    let mut game = game();
    let (vid, _) = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).unwrap();
    let pose = game.world.physics.car_pose(vid).unwrap();
    // Coming in at 22 m/s and sinking at a metre and a half a second.
    game.world.physics.set_car_dive(vid, [pose.position[0], 3.0, pose.position[2]], pose.yaw(), -0.07, 22.0);
    let events = crash_events(&mut game, 180);
    assert_eq!(game.world.entities.of::<Health>(vid).hp, vehicle_hp(KIND_PLANE), "{events:?}");
    let landed = game.world.physics.car_pose(vid).unwrap();
    assert!(landed.up_y > 0.95 && landed.position[1] < 1.5, "{landed:?}");
}

#[test]
fn flying_a_plane_into_the_ground_wrecks_it() {
    let mut game = game();
    let (vid, _) = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).unwrap();
    let pose = game.world.physics.car_pose(vid).unwrap();
    // Diving at 45° and 30 m/s, a few metres up.
    game.world.physics.set_car_dive(vid, [pose.position[0], 6.0, pose.position[2]], pose.yaw(), -0.8, 30.0);
    for _ in 0..120 {
        Vehicles.on_tick(&mut game.world, TICK_DT);
    }
    assert!(game.world.entities.dead(vid), "hp {}", game.world.entities.of::<Health>(vid).hp);
}

#[test]
fn a_roadster_driver_boosts_on_run_hops_on_jump_and_brakes_on_crouch() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, id, &mut seq, 30);
    let (vid, _) = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_ROADSTER).expect("a roadster is placed");
    let pose = game.world.physics.car_pose(vid).unwrap();
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [pose.position[0], STANDING_Y, pose.position[2] + 2.0];
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(id));

    press(&mut game, id, &mut seq, (0, 0), BUTTON_RUN, 30);
    let controls = game.world.entities.of::<Vehicle>(vid).controls;
    assert!(controls.boost && !controls.hop && !controls.handbrake && controls.lift == 0.0, "{controls:?}");
    let thrust = game.seen(vid).unwrap().thrust.expect("a roadster sends its tank");
    assert_eq!(thrust.flags, THRUST_BOOST);
    assert!(thrust.air < 255, "boosting drains the tank: {}", thrust.air);
    assert!(game.world.physics.car_pose(vid).unwrap().velocity.iter().map(|v| v * v).sum::<f32>().sqrt() > 5.0);

    let ground = game.world.physics.car_pose(vid).unwrap().position[1];
    press(&mut game, id, &mut seq, (0, 0), BUTTON_JUMP, 30);
    assert!(game.world.entities.of::<Vehicle>(vid).controls.hop && !game.world.entities.of::<Vehicle>(vid).controls.handbrake);
    assert!(game.world.physics.car_pose(vid).unwrap().position[1] > ground + 2.0, "it hopped");

    press(&mut game, id, &mut seq, (0, 0), BUTTON_CROUCH, 3);
    assert!(game.world.entities.of::<Vehicle>(vid).controls.handbrake);
    // Nothing else carries a tank, but a hen's stamina and a drone's battery.
    assert!(game.seen_vehicles().iter().filter(|(_, v)| !matches!(v.kind.as_ref().unwrap().kind, KIND_ROADSTER | KIND_HEN | KIND_DRONE)).all(|(_, v)| v.thrust.is_none()));
}

/// A game with one pine just in front of where a vehicle of `kind` is set down facing it, and the
/// vehicle's speed (m/s) one tick before, and the tick the tree fell.
fn drive_into_a_pine(kind: u8, speed: f32) -> Option<(f32, f32)> {
    use shared::world::{ObjId, TREE_PINE, Tree};
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 30_000,
        spawns: (0..14).map(|i| [i * 2500 - 15_000, 20_000]).collect(),
        trees: vec![Tree { x: 1500, z: 100, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true }],
        tree_ids: vec![ObjId { col: 2, row: 3, index: 0 }],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == kind).unwrap().0;
    game.world.physics.reset_car(vid, [15.0, 1.0, 5.5], 0.0);
    for _ in 0..60 {
        Vehicles.on_tick(&mut game.world, TICK_DT);
    }
    game.world.physics.kick_car(vid, [0.0, 0.0, -speed]);
    for _ in 0..120 {
        let before = game.world.physics.car_pose(vid)?.velocity;
        Vehicles.on_tick(&mut game.world, TICK_DT);
        if !game.world.destroyed.is_empty() {
            let after = game.world.physics.car_pose(vid)?.velocity;
            return Some((before[0].hypot(before[2]), after[0].hypot(after[2])));
        }
    }
    None
}

#[test]
fn a_tank_plows_through_a_tree_without_slowing() {
    let (before, after) = drive_into_a_pine(KIND_TANK, 10.0).expect("the tank felled the pine");
    assert!(after > before * 0.98, "{before} -> {after}");
}

#[test]
fn a_jeep_is_jolted_by_the_tree_it_fells() {
    let (before, after) = drive_into_a_pine(KIND_JEEP, 20.0).expect("the jeep felled the pine");
    assert!(after < before * 0.8, "{before} -> {after}");
}

#[test]
fn a_hen_driver_leaps_on_jump_and_the_snapshot_carries_its_stamina() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, id, &mut seq, 30);
    let ground = game.world.map.ground.height_at(0.0, 60.0);
    let vid = add_vehicle(&mut game.world, KIND_HEN, [0.0, ground + drop_height(KIND_HEN), 60.0], 0.0);
    idle(&mut game, id, &mut seq, 60);
    let stand = game.world.physics.car_pose(vid).unwrap().position[1];
    let state = game.seen(vid).unwrap();
    assert_eq!(state.thrust.map(|thrust| thrust.air), Some(255), "a hen sends its stamina, full");
    assert_eq!(state.arms.map(|arms| arms.bombs), Some(HEN_EGGS), "and its clutch");

    let pose = game.world.physics.car_pose(vid).unwrap();
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [pose.position[0], STANDING_Y + ground, pose.position[2] + 3.0];
    press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(id));

    press(&mut game, id, &mut seq, (0, 0), BUTTON_JUMP, 20);
    let controls = game.world.entities.of::<Vehicle>(vid).controls;
    assert!(controls.hop && !controls.handbrake && controls.lift == 0.0 && !controls.boost, "{controls:?}");
    assert!(game.world.physics.car_pose(vid).unwrap().position[1] > stand + 2.0, "it leapt");
    let thrust = game.seen(vid).unwrap().thrust.unwrap();
    assert!(thrust.flags & THRUST_AIR != 0 && thrust.flags & THRUST_LIFT != 0 && thrust.air < 255, "{thrust:?}");
}
