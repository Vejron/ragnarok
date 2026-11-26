use super::*;
use crate::game::{Command, Game, Outgoing, Player};
use crate::mechanics::emergency::{charge_wanted, wanted_stars};
use shared::consts::*;
use shared::protocol::{InputFrame, KIND_VOLVO_240, S_WASHED, Washed, decode, peek_tag};
use shared::world::{FuelStation, WORLD_VERSION, WorldFile};
use sim::STANDING_Y;
use tokio::sync::{mpsc, oneshot};

/// A flat 600 m square with a fuel station in the middle, unturned (its wash lane runs along +X at
/// z = `WASH_BAY_Z`), the police on duty, and a car in the lead-in before the wash's -X door, facing it.
fn game() -> (Game, u32) {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 30_000, spawns: vec![[0, -25_000]], fuel_stations: vec![FuelStation { at: [0, 0], yaw: 0, base: 0 }], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.fleet.police = true;
    let car = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_VOLVO_240, [WASH_BAY_X[0] - 4.0, 1.2, WASH_BAY_Z], -std::f32::consts::FRAC_PI_2);
    (game, car)
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

fn press(game: &mut Game, id: u32, seq: &mut u32, forward: i8, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, forward, buttons, ..Default::default() }]));
        game.step();
    }
}

/// Joins a player and sits them at the wheel of `vid`.
fn driver(game: &mut Game, vid: u32, seq: &mut u32) -> (u32, mpsc::Receiver<Outgoing>) {
    let (id, inbox) = join(game);
    press(game, id, seq, 0, 0, 30);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [at[0], STANDING_Y, at[2] - 2.0];
    press(game, id, seq, 0, BUTTON_USE, 1);
    press(game, id, seq, 0, 0, 3);
    assert_eq!(game.world.entities.of::<Seats>(vid)[0], Some(id), "they are at the wheel");
    (id, inbox)
}

/// The vehicles every `Washed` in `inbox` named.
fn washed(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<u32> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_WASHED) => decode::<Washed>(&bytes).map(|washed| washed.vehicles),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Knocks `vid` about: its health down to a third, its body sides dented, a wheel hurt, a door off.
fn batter(game: &mut Game, vid: u32) {
    let mut health = game.world.entities.of_mut::<Health>(vid);
    health.hp = health.max / 3;
    let mut vehicle = game.world.entities.of_mut::<Vehicle>(vid);
    vehicle.damage[..4].copy_from_slice(&[180, 90, 200, 40]);
    vehicle.damage[5] = 120;
    vehicle.doors.gone = 0b01;
}

#[test]
fn a_battered_wanted_car_driven_through_the_wash_comes_out_whole_and_its_driver_clean() {
    let (mut game, car) = game();
    let mut seq = 0;
    let (id, mut inbox) = driver(&mut game, car, &mut seq);
    batter(&mut game, car);
    charge_wanted(&mut game.world, id, 60.0, None);
    assert!(wanted_stars(&game.world, id) >= 2, "they are wanted");
    washed(&mut inbox);
    // Floored all the way: the wash drives it through at its own pace.
    press(&mut game, id, &mut seq, 127, 0, 2 * TICK_RATE);
    assert!(washed(&mut inbox).is_empty(), "nothing yet, half way in");
    assert!(game.world.entities.of::<Health>(car).hp < game.world.entities.of::<Health>(car).max);
    press(&mut game, id, &mut seq, 127, 0, 6 * TICK_RATE);
    let x = game.world.physics.car_pose(car).unwrap().position[0];
    assert!(x > WASH_BAY_X[1], "it came out of the far door: {x}");
    assert_eq!(washed(&mut inbox), [car], "everyone was told it came out washed");
    let health = *game.world.entities.of::<Health>(car);
    assert_eq!(health.hp, health.max, "whole again");
    let vehicle = game.world.entities.of::<Vehicle>(car);
    assert!(vehicle.damage[..8].iter().all(|&slot| slot == 0), "no dents and sound wheels: {:?}", vehicle.damage);
    assert_eq!(vehicle.doors.gone, 0, "its door back on");
    assert_eq!(vehicle.fuel, 1.0);
    assert_eq!(wanted_stars(&game.world, id), 0, "and its driver is wanted no more");
}

#[test]
fn a_car_driven_past_the_wash_is_not_healed() {
    let (mut game, car) = game();
    let mut seq = 0;
    // Beside the bay, not in its lane: it drives along the outside of the wall.
    game.world.physics.reset_car(car, [WASH_BAY_X[0] - 4.0, 1.2, WASH_BAY_Z - WASH_BAY_WIDTH / 2.0 - 3.0], -std::f32::consts::FRAC_PI_2);
    let (id, mut inbox) = driver(&mut game, car, &mut seq);
    batter(&mut game, car);
    press(&mut game, id, &mut seq, 127, 0, 5 * TICK_RATE);
    let x = game.world.physics.car_pose(car).unwrap().position[0];
    assert!(x > WASH_BAY_X[1], "it went past the bay: {x}");
    assert!(washed(&mut inbox).is_empty());
    assert_eq!(game.world.entities.of::<Health>(car).hp, game.world.entities.of::<Health>(car).max / 3);
}
