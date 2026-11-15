use super::*;
use crate::game::{Command, Game, Profile};
use crate::mechanics::{Harm, damage_entity};
use shared::protocol::WEAPON_RIFLE;
use shared::world::{Road, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A line from (-200, -20) east to (200, -20), then south to (200, 100), with a stop at each end and
/// one 200 m along: 520 m, which two buses run.
fn line() -> BusLine {
    BusLine { points: vec![[-20_000, -2_000], [-10_000, -2_000], [0, -2_000], [10_000, -2_000], [20_000, -2_000], [20_000, 10_000]], stops: vec![0, 20_000, 52_000], signs: Vec::new() }
}

/// A flat 600 m square with the road of `lines` and those lines on it.
fn game(lines: Vec<BusLine>) -> Game {
    let roads = lines.iter().map(|line| Road { width: 1000, kind: 0, carried: false, points: line.points.clone() }).collect();
    let map = WorldFile { version: WORLD_VERSION, half_extent: 30_000, spawns: vec![[-25_000, -25_000]], roads, bus_lines: lines, ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    response.try_recv().unwrap().unwrap()
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

/// Runs until `done`, or fails after `seconds`.
fn run_until(game: &mut Game, seconds: f32, what: &str, mut done: impl FnMut(&Game) -> bool) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
        if done(game) {
            return;
        }
    }
    panic!("never {what} in {seconds} s");
}

/// Feeds `id` one frame a tick with `buttons` held.
fn press(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, buttons, ..Default::default() }]));
        game.step();
    }
}

/// The buses on their lines, and the one heading out or back.
fn buses(game: &Game) -> Vec<(u32, Bus)> {
    game.world.entities.all::<Bus>().map(|(vid, bus)| (vid, bus.clone())).collect()
}

fn bus(game: &Game, out: bool) -> u32 {
    buses(game).into_iter().find(|(_, bus)| bus.out == out).expect("a bus each way").0
}

fn pose(game: &Game, vid: u32) -> CarPose {
    game.world.physics.car_pose(vid).unwrap()
}

/// Stands `id` at the front door of bus `vid`.
fn to_the_door(game: &mut Game, id: u32, vid: u32) {
    let door = pose(game, vid).world_point([BUS_DOOR[0] + 1.0, 0.0, BUS_DOOR[2]]);
    let y = game.world.map.ground.height_at(door[0], door[2]) + sim::STANDING_Y;
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [door[0], y, door[2]];
}

fn seat_of(game: &Game, id: u32) -> Option<(u32, usize)> {
    game.world.entities.of::<Player>(id).seat.map(|seat| (seat.vehicle, seat.index))
}

#[test]
fn a_world_with_no_bus_lines_gets_no_buses() {
    let mut game = game(vec![]);
    run(&mut game, 1.0);
    assert_eq!(game.world.entities.count::<Bus>(), 0);
    assert_eq!(game.world.entities.count::<BusDriver>(), 0);
    assert!(game.world.entities.all::<Vehicle>().all(|(_, vehicle)| vehicle.kind != KIND_BUS));
}

#[test]
fn each_line_gets_its_buses_with_a_driver_in_each() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let buses = buses(&game);
    assert_eq!(buses.len(), 2, "a short line runs two");
    assert_eq!(buses.iter().filter(|(_, bus)| bus.out).count(), 1, "one each way");
    for (vid, bus) in &buses {
        let driver = bus.driver.expect("the room sent a driver");
        assert!(game.world.entities.has::<BusDriver>(driver));
        assert_eq!(seat_of(&game, driver), Some((*vid, 0)), "in the driver's seat");
        assert_eq!(game.world.entities.of::<Player>(driver).team, crate::game::TEAM_NONE, "a civilian");
    }
}

#[test]
fn a_long_world_of_lines_never_runs_more_than_the_most_buses() {
    // Five lines with a stop every 58 m want three buses each, fifteen between them.
    let long = |z: i32| BusLine { points: vec![[-29_000, z], [29_000, z]], stops: (0..=10).map(|n| n * 5_800).collect(), signs: Vec::new() };
    let mut game = game((-2..=2).map(|n| long(n * 10_000)).collect());
    run(&mut game, 1.0);
    assert_eq!(game.world.entities.count::<Bus>(), BUS_MAX);
}

#[test]
fn a_bus_keeps_to_its_side_of_the_road_and_halts_at_each_stop() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let vid = bus(&game, true);
    let path = Path::new(&line());
    // It starts half way along heading out: the next stop is the end of the line, round the corner.
    assert_eq!(game.world.entities.of::<Bus>(vid).next, 2);
    let mut fastest = 0.0_f32;
    run_until(&mut game, 90.0, "halted at the end stop", |game| {
        let pose = pose(game, vid);
        let bus = game.world.entities.of::<Bus>(vid);
        fastest = fastest.max(pose.forward_speed);
        // Never off the road, which is 10 m wide, and right of its middle as it goes, but for the
        // sharp corner at 400 m, which a 12 m bus swings wide of.
        let (p, way) = path.at(bus.along);
        let side = (pose.position[0] - p[0]) * -way[1] + (pose.position[2] - p[1]) * way[0];
        assert!(side.abs() < 5.0, "{side} m from the middle at {} m along", bus.along);
        assert!(side > 0.5 || (bus.along - 400.0).abs() < 30.0, "{side} m right of the middle at {} m along", bus.along);
        bus.wait > 0
    });
    let at_stop = pose(&game, vid);
    let along = game.world.entities.of::<Bus>(vid).along;
    assert!((along - 520.0).abs() <= STOP_WITHIN + 0.5, "halted at the stop: {along}");
    assert!(at_stop.forward_speed.abs() < 0.3, "standing: {at_stop:?}");
    assert!(fastest > 8.0 && fastest <= BUS_CRUISE + 1.0, "drove at a bus's pace: {fastest}");
}

#[test]
fn a_bus_turns_back_at_the_end_of_its_line_and_calls_at_the_next_stop() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let vid = bus(&game, true);
    run_until(&mut game, 90.0, "halted at the end stop", |game| game.world.entities.of::<Bus>(vid).wait > 0);
    let before = pose(&game, vid);
    run_until(&mut game, BUS_DWELL + 2.0, "turned round", |game| !game.world.entities.of::<Bus>(vid).out);
    let after = pose(&game, vid);
    assert!(wrap_angle(after.yaw() - before.yaw()).abs() > 3.0, "facing back the way it came");
    assert!((after.position[0] - before.position[0]).abs() > 3.0, "on the other side of the road: {after:?}");
    assert_eq!(game.world.entities.of::<Bus>(vid).next, 1, "for the stop in the middle");
    run_until(&mut game, 90.0, "halted at the middle stop", |game| {
        let bus = game.world.entities.of::<Bus>(vid);
        bus.wait > 0 && (bus.along - 200.0).abs() <= STOP_WITHIN + 0.5
    });
}

#[test]
fn a_player_boards_as_a_passenger_and_rides_to_the_next_stop() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let vid = bus(&game, true);
    let rider = join(&mut game);
    let mut seq = 0;
    press(&mut game, rider, &mut seq, 0, 4);
    to_the_door(&mut game, rider, vid);
    press(&mut game, rider, &mut seq, BUTTON_USE, 1);
    press(&mut game, rider, &mut seq, 0, 2);
    assert_eq!(seat_of(&game, rider), Some((vid, 1)), "the first passenger seat: the driver has seat 0");
    let from = pose(&game, vid).position;
    for _ in 0..90 * TICK_RATE {
        press(&mut game, rider, &mut seq, 0, 1);
        if game.world.entities.of::<Bus>(vid).wait > 0 {
            break;
        }
    }
    let to = pose(&game, vid).position;
    assert!(game.world.entities.of::<Bus>(vid).wait > 0, "it reached its stop");
    assert!((to[0] - from[0]).hypot(to[2] - from[2]) > 100.0, "and carried them there");
    // That stop is the end of the line, where everyone gets off.
    assert_eq!(seat_of(&game, rider), None, "set down at the end of the line");
}

#[test]
fn a_bus_whose_driver_is_killed_can_be_driven_and_is_replaced() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let vid = bus(&game, true);
    let driver = game.world.entities.of::<Bus>(vid).driver.unwrap();
    let thief = join(&mut game);
    let mut seq = 0;
    press(&mut game, thief, &mut seq, 0, 4);
    damage_entity(&mut game.world, driver, Harm::new(u8::MAX, None, WEAPON_RIFLE).unsparing());
    press(&mut game, thief, &mut seq, 0, 2);
    assert!(!game.world.entities.has::<Bus>(vid), "off its line");
    assert!(game.world.entities.has::<Strayed>(vid));
    assert_eq!(game.world.entities.of::<Seats>(vid).rider(0), None, "the driver's seat is free");
    // Whoever gets in now drives it.
    to_the_door(&mut game, thief, vid);
    press(&mut game, thief, &mut seq, BUTTON_USE, 1);
    press(&mut game, thief, &mut seq, 0, 2);
    assert_eq!(seat_of(&game, thief), Some((vid, 0)));
    // The dead driver is taken away, and a fresh bus joins the line.
    for _ in 0..(BUS_RETURN as u32 + 2) * TICK_RATE {
        press(&mut game, thief, &mut seq, 0, 1);
    }
    assert!(!game.world.entities.has::<Player>(driver), "the dead driver has gone");
    let running = buses(&game);
    assert_eq!(running.len(), 2, "the line has its two again");
    assert!(running.iter().all(|(other, bus)| *other != vid && bus.driver.is_some()));
    assert!(game.world.entities.has::<Vehicle>(vid), "the stolen one is still there while someone is in it");
}

#[test]
fn a_bus_left_empty_off_its_line_is_taken_away() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let vid = bus(&game, false);
    let driver = game.world.entities.of::<Bus>(vid).driver.unwrap();
    damage_entity(&mut game.world, driver, Harm::new(u8::MAX, None, WEAPON_RIFLE).unsparing());
    run(&mut game, STRAY_EMPTY + 2.0);
    assert!(!game.world.entities.has::<Vehicle>(vid));
    assert!(game.world.physics.car_pose(vid).is_none());
}

#[test]
fn a_bus_far_off_its_line_is_put_back_when_empty() {
    let mut game = game(vec![line()]);
    run(&mut game, 1.0);
    let vid = bus(&game, true);
    let path = Path::new(&line());
    let there = pose(&game, vid);
    game.world.physics.reset_car(vid, [there.position[0], there.position[1], there.position[2] - 80.0], there.yaw());
    run(&mut game, 0.5);
    let back = pose(&game, vid);
    let (_, off) = path.nearest([back.position[0], back.position[2]], 0.0, path.length());
    assert!(off < 5.0, "back on its line: {off} m off");
    assert!(game.world.entities.has::<Bus>(vid), "and still its line's bus");
}

/// The buses of a real world, run for `MINUTES` (default 10) with nobody watching: how far each got,
/// how often it backed out or was put back, and what a tick of them costs.
/// `WORLD=worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture buses_on_a_real_world`
#[test]
#[ignore]
fn buses_on_a_real_world() {
    let path = std::env::var("WORLD").expect("WORLD names a world.msgpack");
    let (mut map, _) = crate::read_world(std::path::Path::new(&path)).unwrap();
    // `NO_BUSES=1` runs the same world with no lines, to see what the buses add to a tick.
    if std::env::var("NO_BUSES").is_ok() {
        map.bus_lines.clear();
    }
    let lines = map.bus_lines.len();
    let mut game = Game::new(map, 0).unwrap();
    let minutes: f32 = std::env::var("MINUTES").ok().and_then(|text| text.parse().ok()).unwrap_or(10.0);
    let mut seen: std::collections::BTreeMap<u32, (f32, u32, u32, f32)> = Default::default();
    let mut tries: std::collections::BTreeMap<u32, u32> = Default::default();
    let started = std::time::Instant::now();
    let steps = (minutes * 60.0 * TICK_RATE as f32) as u32;
    for _ in 0..steps {
        game.step();
        for (vid, bus) in buses(&game) {
            let Some(pose) = game.world.physics.car_pose(vid) else { continue };
            let entry = seen.entry(vid).or_insert((0.0, 0, 0, 0.0));
            entry.0 += pose.forward_speed.max(0.0) / TICK_RATE as f32;
            entry.3 = entry.3.max(pose.forward_speed);
            let before = tries.insert(vid, bus.tries).unwrap_or(0);
            if bus.tries > before {
                entry.1 += 1;
                eprintln!("bus {vid} on line {} backs out at {:.0} m along ({:.0}, {:.0}), heading {}", bus.line, bus.along, pose.position[0], pose.position[2], if bus.out { "out" } else { "back" });
            }
            if bus.tries == 0 && before >= TRIES {
                entry.2 += 1;
            }
        }
    }
    let spent = started.elapsed();
    eprintln!("{lines} lines, {} buses, {minutes} min in {spent:.1?}: {:.3} ms a tick for the whole room", seen.len(), spent.as_secs_f64() * 1000.0 / f64::from(steps));
    for (vid, (driven, backed, put_back, fastest)) in seen {
        eprintln!("bus {vid}: drove {driven:.0} m, fastest {fastest:.1} m/s, backed out {backed} times, put back {put_back} times");
    }
}
