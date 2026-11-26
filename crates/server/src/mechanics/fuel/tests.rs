use super::*;
use crate::game::{Command, Game, Outgoing, Player, Seats, TEAM_BOTS};
use shared::consts::*;
use shared::protocol::InputFrame;
use shared::vec3;
use shared::world::{Road, WORLD_VERSION, WorldFile};
use sim::STANDING_Y;
use tokio::sync::{mpsc, oneshot};

/// A flat 1200 m square with one road along X, a jeep and a sports car parked on it near the middle,
/// and `stations` (`[x, z]`, centimetres).
fn game(stations: Vec<[i32; 2]>) -> Game {
    let spawns: Vec<[i32; 2]> = [[-25_000, 0], [25_000, 0]].into_iter().chain((0..12).map(|i| [i * 2500 - 13_750, 0])).collect();
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 60_000,
        placed_vehicles: crate::mechanics::for_tests::parked(&spawns, &[KIND_JEEP, KIND_SPORTS]),
        spawns,
        roads: vec![Road { width: 800, kind: 0, carried: false, points: vec![[-60_000, 0], [60_000, 0]], ..Default::default() }],
        fuel_stations: stations,
        ..Default::default()
    };
    Game::new(map, 0).unwrap()
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// Feeds one frame per tick, with the frame counter owned by the caller.
fn press(game: &mut Game, id: u32, seq: &mut u32, forward: i8, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, forward, buttons, ..Default::default() }]));
        game.step();
    }
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

fn car(game: &Game, kind: u8) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == kind).unwrap().0
}

fn fuel(game: &Game, vid: u32) -> f32 {
    game.world.entities.of::<Vehicle>(vid).fuel
}

fn set_fuel(game: &mut Game, vid: u32, fuel: f32) {
    game.world.entities.of_mut::<Vehicle>(vid).fuel = fuel;
}

fn put(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [x, STANDING_Y, z];
}

/// Joins a player and sits them at the wheel of `vid`.
fn driver(game: &mut Game, vid: u32, seq: &mut u32) -> (u32, mpsc::Receiver<Outgoing>) {
    let (id, inbox) = join(game);
    press(game, id, seq, 0, 0, 30);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    put(game, id, [at[0] + 2.0, at[2]]);
    press(game, id, seq, 0, BUTTON_USE, 1);
    press(game, id, seq, 0, 0, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(id), "they are at the wheel");
    (id, inbox)
}

/// Seconds a full tank of `vid` lasts a player who holds `controls`, `boosting` or not.
fn lasts(game: &Game, vid: u32, controls: CarControls, boosting: bool) -> f32 {
    let mut vehicle = Vehicle { fuel: 1.0, ..clone(game.world.entities.of::<Vehicle>(vid)) };
    let mut ticks = 0;
    while !step(&mut vehicle, Some(Burn { controls, boosting }), false, true, TICK_DT) {
        ticks += 1;
        assert!(ticks < 24 * 3600 * TICK_RATE, "it never runs dry");
    }
    ticks as f32 * TICK_DT
}

fn clone(vehicle: &Vehicle) -> Vehicle {
    Vehicle { kind: vehicle.kind, controls: vehicle.controls, home: vehicle.home, stuck_ticks: 0, turret_hp: vehicle.turret_hp, damage: vehicle.damage, parts: vehicle.parts, scrape_in: 0, lamp: false, lamp_held: false, siren: false, doors: Default::default(), fuel: vehicle.fuel, dry_ticks: vehicle.dry_ticks }
}

const FLAT_OUT: CarControls = CarControls { throttle: 1.0, powered: true, ..CarControls::IDLE };

#[test]
fn a_jeep_at_full_throttle_runs_dry_in_about_ten_minutes_and_a_sports_car_sooner() {
    let game = game(Vec::new());
    let jeep = lasts(&game, car(&game, KIND_JEEP), FLAT_OUT, false);
    assert!((jeep - 600.0).abs() < 5.0, "a jeep's tank lasts {jeep} s flat out");
    let sports = lasts(&game, car(&game, KIND_SPORTS), FLAT_OUT, false);
    assert!((sports - 480.0).abs() < 5.0, "a sports car's lasts {sports} s");
    // Astern burns as ahead does, and sitting with the engine ticking over burns a twentieth of it.
    let astern = lasts(&game, car(&game, KIND_JEEP), CarControls { throttle: -1.0, ..FLAT_OUT }, false);
    assert!((astern - jeep).abs() < 1.0, "{astern} s astern");
    let idling = lasts(&game, car(&game, KIND_JEEP), CarControls { throttle: 0.0, ..FLAT_OUT }, false);
    assert!((idling - 600.0 / FUEL_IDLE_BURN).abs() < 60.0, "{idling} s at rest");
}

#[test]
fn boosting_burns_the_tank_faster() {
    let game = game(Vec::new());
    let boosted = lasts(&game, car(&game, KIND_JEEP), CarControls { boost: true, ..FLAT_OUT }, true);
    assert!((boosted - 600.0 / FUEL_BOOST_BURN).abs() < 5.0, "a jeep's tank lasts {boosted} s on the boost");
}

#[test]
fn a_helicopters_collective_burns_its_fuel() {
    let mut game = game(Vec::new());
    let heli = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_HELI, [0.0, 3.0, 50.0], 0.0);
    let hovering = lasts(&game, heli, CarControls { lift: 1.0, powered: true, ..CarControls::IDLE }, false);
    assert!((hovering - 480.0).abs() < 5.0, "a helicopter's tank lasts {hovering} s with the lever up");
}

#[test]
fn a_player_driving_uses_fuel_and_more_of_it_on_the_boost() {
    let mut game = game(Vec::new());
    let (jeep, mut seq) = (car(&game, KIND_JEEP), 0);
    let (id, _inbox) = driver(&mut game, jeep, &mut seq);
    press(&mut game, id, &mut seq, 127, 0, 2 * TICK_RATE);
    let used = 1.0 - fuel(&game, jeep);
    assert!((used - 2.0 / 600.0).abs() < 0.3 / 600.0, "two seconds flat out used {used} of the tank");
    set_fuel(&mut game, jeep, 1.0);
    press(&mut game, id, &mut seq, 127, BUTTON_RUN, 2 * TICK_RATE);
    let boosted = 1.0 - fuel(&game, jeep);
    assert!(boosted > used * 1.5, "two seconds on the boost used {boosted}, against {used} off it");
    // The parked car beside it has used none.
    assert_eq!(fuel(&game, car(&game, KIND_SPORTS)), 1.0);
}

#[test]
fn a_dry_car_does_not_answer_the_throttle_and_is_seen_to_be_empty() {
    let mut game = game(Vec::new());
    let (jeep, mut seq) = (car(&game, KIND_JEEP), 0);
    let (id, _inbox) = driver(&mut game, jeep, &mut seq);
    set_fuel(&mut game, jeep, 0.0);
    press(&mut game, id, &mut seq, 127, BUTTON_RUN, TICK_RATE);
    let speed = vec3::length(game.world.physics.car_pose(jeep).unwrap().velocity);
    assert!(speed < 0.5, "it does not move ({speed} m/s)");
    assert_eq!(byte(fuel(&game, jeep)), 0);
    // The last drop still reads as some, so that a client stops its engine only when the server does.
    assert_eq!(byte(0.0001), 1);
    assert_eq!(byte(1.0), 255);
    // With fuel in it, it drives off.
    set_fuel(&mut game, jeep, 0.5);
    press(&mut game, id, &mut seq, 127, 0, TICK_RATE);
    let speed = vec3::length(game.world.physics.car_pose(jeep).unwrap().velocity);
    assert!(speed > 2.0, "it drives ({speed} m/s)");
}

#[test]
fn a_bot_at_the_wheel_burns_nothing_and_drives_on_empty() {
    let mut game = game(Vec::new());
    let (tank, _crew) = game.add_crew(["Driver".into(), "Gunner".into()], [0.0, 40.0], TEAM_BOTS, vec![[150.0, 40.0]]).expect("room for a crew");
    run(&mut game, 6.0);
    let start = game.world.physics.car_pose(tank).unwrap().position;
    assert_eq!(fuel(&game, tank), 1.0, "its bot driver has burnt nothing");
    set_fuel(&mut game, tank, 0.0);
    run(&mut game, 6.0);
    let moved = vec3::distance(start, game.world.physics.car_pose(tank).unwrap().position);
    assert!(moved > 10.0, "it drives on with an empty tank ({moved} m)");
}

#[test]
fn a_holed_tank_leaks_while_parked_and_a_destroyed_one_faster() {
    let mut game = game(Vec::new());
    let (jeep, sports) = (car(&game, KIND_JEEP), car(&game, KIND_SPORTS));
    let slot = Part::Fuel.slot().unwrap();
    // The jeep's fuel tank is hurt (half of what it had), the sports car's is gone.
    let full = vehicle_def(KIND_JEEP).armor.module(slot).expect("a jeep has a fuel tank").hp;
    game.world.entities.of_mut::<Vehicle>(jeep).parts[slot] = full / 2;
    game.world.entities.of_mut::<Vehicle>(sports).parts[slot] = 0;
    run(&mut game, 10.0);
    let (hurt, gone) = (1.0 - fuel(&game, jeep), 1.0 - fuel(&game, sports));
    assert!((hurt - 10.0 / FUEL_LEAK_HURT_SECONDS).abs() < 0.005, "a holed tank lost {hurt} in ten seconds");
    assert!((gone - 10.0 / FUEL_LEAK_GONE_SECONDS).abs() < 0.005, "a destroyed one lost {gone}");
    run(&mut game, FUEL_LEAK_GONE_SECONDS);
    assert_eq!(fuel(&game, sports), 0.0, "and is empty soon after");
}

#[test]
fn a_station_fills_an_empty_tank_in_ten_seconds_and_only_near_it() {
    let mut game = game(Vec::new());
    let (jeep, sports) = (car(&game, KIND_JEEP), car(&game, KIND_SPORTS));
    // A station a little off the road from the jeep; the sports car is moved well out of its reach.
    let at = game.world.physics.car_pose(jeep).unwrap().position;
    game.world.map.fuel_stations = vec![[(at[0] * 100.0) as i32, (at[2] * 100.0) as i32 + 1_500]];
    game.world.physics.reset_car(sports, [at[0] + FUEL_STATION_REACH + 20.0, at[1], at[2]], 0.0);
    set_fuel(&mut game, jeep, 0.0);
    set_fuel(&mut game, sports, 0.0);
    run(&mut game, FUEL_FILL_SECONDS / 2.0);
    let half = fuel(&game, jeep);
    assert!((half - 0.5).abs() < 0.02, "half full after half the time: {half}");
    run(&mut game, FUEL_FILL_SECONDS / 2.0 + 0.2);
    assert_eq!(fuel(&game, jeep), 1.0, "full after ten seconds");
    assert_eq!(fuel(&game, sports), 0.0, "the one out of reach got none");
}

#[test]
fn a_stranded_car_stays_while_someone_is_near_and_goes_home_full_once_they_are_gone() {
    let mut game = game(Vec::new());
    let jeep = car(&game, KIND_JEEP);
    let (id, _inbox) = join(&mut game);
    let home = game.world.entities.of::<Vehicle>(jeep).home.0;
    // It ran dry 150 m down the road from where it was placed, and has stood there empty for almost long enough.
    let out = [home[0] + 150.0, home[1], home[2]];
    game.world.physics.reset_car(jeep, out, 0.0);
    {
        let mut vehicle = game.world.entities.of_mut::<Vehicle>(jeep);
        vehicle.fuel = 0.0;
        vehicle.dry_ticks = FUEL_STRANDED_SECONDS * TICK_RATE - 1;
    }
    put(&mut game, id, [out[0] + 100.0, out[2]]);
    run(&mut game, 5.0);
    let stays = game.world.physics.car_pose(jeep).unwrap().position;
    assert!(vec3::distance(stays, out) < 2.0, "it stays while a player is within sight of it");
    assert_eq!(fuel(&game, jeep), 0.0);

    put(&mut game, id, [out[0] - HEAL_CLEAR - 50.0, out[2] + 300.0]);
    run(&mut game, 5.0);
    let back = game.world.physics.car_pose(jeep).unwrap().position;
    assert!(vec3::distance(back, home) < 2.0, "it is back where it was placed, {} m from it", vec3::distance(back, home));
    assert_eq!(fuel(&game, jeep), 1.0, "with a full tank");
}

#[test]
fn a_dry_car_someone_sits_in_is_not_counted_as_stranded() {
    let mut game = game(Vec::new());
    let (jeep, mut seq) = (car(&game, KIND_JEEP), 0);
    let (id, _inbox) = driver(&mut game, jeep, &mut seq);
    set_fuel(&mut game, jeep, 0.0);
    press(&mut game, id, &mut seq, 0, 0, 2 * TICK_RATE);
    assert_eq!(game.world.entities.of::<Vehicle>(jeep).dry_ticks, 0);
    // Once they get out, it counts.
    press(&mut game, id, &mut seq, 0, BUTTON_USE, 1);
    press(&mut game, id, &mut seq, 0, 0, 2 * TICK_RATE);
    assert!(game.world.entities.of::<Vehicle>(jeep).dry_ticks > TICK_RATE);
}

#[test]
fn every_engine_burns_fuel_but_the_pirate_ship_the_snow_racer_and_what_has_none() {
    let without = [KIND_SHIP, KIND_SNOWRACER, KIND_MORTAR, KIND_PAK, KIND_SHIP_GUN, KIND_HEN, KIND_TUPP, KIND_DRONE, KIND_LOITER, KIND_MISSILE];
    for def in shared::vehicles::VEHICLES {
        let burns = def.spec.fuel.is_some();
        assert_eq!(burns, !without.contains(&def.kind), "{}", def.name);
        if let Some(lasts) = def.spec.fuel {
            assert!((400.0..=900.0).contains(&lasts), "{} runs {lasts} s on a tank: about ten minutes", def.name);
        }
    }
    // A kind that burns none never runs dry, whatever its driver does.
    let mut game = game(Vec::new());
    let sled = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_SNOWRACER, [0.0, 2.0, 60.0], 0.0);
    let mut vehicle = clone(game.world.entities.of::<Vehicle>(sled));
    vehicle.fuel = 0.0;
    assert!(!step(&mut vehicle, Some(Burn { controls: FLAT_OUT, boosting: false }), false, true, TICK_DT));
}
