//! The submarine's weapons officer: the periscope, the tubes and the missiles flown from them.

use super::*;
use crate::game::{Command, Drone, Game, Outgoing, Seat};
use crate::mechanics::seat;
use shared::protocol::{InputFrame, SUB_MAST_UP};
use shared::world::{NO_WATER, PlacedVehicle, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// Flat ground at 0 over 600 m in 5 m cells, with a lake 10 m deep within 200 m of the middle and a
/// submarine placed in the middle of it, heading -Z.
fn sea() -> Game {
    let side = 121u32;
    let wet = |i: u32, j: u32| {
        let (x, z) = (i as f32 * 5.0 - 300.0, j as f32 * 5.0 - 300.0);
        x.abs() <= 200.0 && z.abs() <= 200.0
    };
    let (mut heights, mut water) = (Vec::new(), Vec::new());
    for j in 0..side {
        for i in 0..side {
            heights.push(if wet(i, j) { -1000 } else { 0 });
            water.push(if wet(i, j) { 0 } else { NO_WATER });
        }
    }
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 30_000,
        ground: shared::terrain::Ground { origin: -30_000, cell: 500, side, heights },
        water,
        spawns: vec![[-25_000, 0]],
        placed_vehicles: vec![PlacedVehicle { kind: KIND_SUBMARINE, at: [0, 0], yaw: 0.0 }],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox): (_, mpsc::Receiver<Outgoing>) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    response.try_recv().unwrap().unwrap()
}

fn sub(game: &Game) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_SUBMARINE).map(|(id, _)| id).unwrap()
}

/// Feeds `ticks` frames of `buttons` and the lift (jump or run) to each of `players`, all looking
/// `yaw`, then steps.
fn frames(game: &mut Game, players: &mut [(u32, u32)], buttons: u16, yaw: f32, ticks: u32) {
    for _ in 0..ticks {
        for (id, seq) in players.iter_mut() {
            *seq += 1;
            game.handle(Command::Input(*id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: shared::quant::to_mrad(yaw), buttons, pitch: 0, view: 0, slot: 0 }]));
        }
        game.step();
    }
}

/// A crewed submarine: its captain in seat 0 and its officer in seat 1, settled on the surface.
fn crewed() -> (Game, u32, [(u32, u32); 2]) {
    let mut game = sea();
    let vid = sub(&game);
    let (captain, officer) = (join(&mut game), join(&mut game));
    assert!(seat(&mut game.world, captain, vid, 0) && seat(&mut game.world, officer, vid, 1));
    let mut crew = [(captain, 0), (officer, 0)];
    frames(&mut game, &mut crew, 0, 0.0, 60);
    (game, vid, crew)
}

/// How deep the submarine's middle is under the surface, metres.
fn depth(game: &Game, vid: u32) -> f32 {
    -game.world.physics.car_pose(vid).unwrap().position[1]
}

fn missile(game: &Game) -> Option<u32> {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_MISSILE).map(|(id, _)| id)
}

#[test]
fn a_submarine_is_placed_floating_with_its_tubes_loaded_and_the_periscope_down() {
    let mut game = sea();
    game.step();
    let vid = sub(&game);
    assert!((depth(&game, vid) - SUB_SURFACE_DEPTH).abs() < 0.3, "afloat at {}", depth(&game, vid));
    let seen = game.seen(vid).unwrap().sub.expect("a submarine sends its tubes and periscope");
    assert_eq!(seen.tubes, 0b1111);
    assert_eq!(seen.flags & SUB_MAST_UP, 0, "nobody is at the periscope");
}

#[test]
fn the_officer_raises_the_periscope_and_turns_it_with_their_look() {
    let (mut game, vid, mut crew) = crewed();
    frames(&mut game, &mut crew[1..], 0, 1.0, 5);
    let seen = game.seen(vid).unwrap().sub.unwrap();
    assert_ne!(seen.flags & SUB_MAST_UP, 0);
    assert!((from_mrad(seen.scope) - 1.0).abs() < 0.05, "turned to the look: {}", from_mrad(seen.scope));
}

#[test]
fn the_captain_takes_it_down_and_deep_the_periscope_is_under_and_nothing_fires() {
    let (mut game, vid, mut crew) = crewed();
    // Run dives it.
    frames(&mut game, &mut crew[..1], BUTTON_RUN, 0.0, 4 * TICK_RATE);
    frames(&mut game, &mut crew, 0, 0.0, TICK_RATE);
    assert!(depth(&game, vid) > SUB_MAST_HEIGHT, "it dived to {}", depth(&game, vid));
    assert_eq!(game.seen(vid).unwrap().sub.unwrap().flags & SUB_MAST_UP, 0, "the periscope is under");
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 3);
    assert!(missile(&game).is_none(), "nothing fires from that deep");
    assert_eq!(game.seen(vid).unwrap().sub.unwrap().tubes, 0b1111);
}

#[test]
fn fire_launches_a_missile_the_officer_flies_and_its_tube_reloads() {
    let (mut game, vid, mut crew) = crewed();
    let officer = crew[1].0;
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 2);
    let rocket = missile(&game).expect("a missile went up");
    assert_eq!(game.world.entities.of::<Player>(officer).seat, Some(Seat { vehicle: rocket, index: 0, remote: true, from: Some((vid, 1)) }));
    // Their body stays in their seat, out of the world, and the tube is empty.
    assert_eq!(game.world.entities.of::<Seats>(vid).rider(1), Some(officer));
    assert!(!game.world.entities.of::<Player>(officer).embodied());
    assert_eq!(game.seen(vid).unwrap().sub.unwrap().tubes, 0b1110);
    // Holding fire on does not set it off: the press went on the launch.
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 10);
    assert!(game.world.entities.get::<Drone>(rocket).is_some_and(|drone| drone.linked));
    // It went up and out over the water.
    let pose = game.world.physics.car_pose(rocket).unwrap();
    assert!(pose.position[1] > 3.0, "it climbed out: {:?}", pose.position);
    // The tube loads again.
    frames(&mut game, &mut crew, 0, 0.0, SUB_TUBE_RELOAD);
    assert_eq!(game.seen(vid).unwrap().sub.unwrap().tubes, 0b1111);
}

#[test]
fn once_its_missile_goes_off_the_officer_is_back_at_the_periscope() {
    let (mut game, vid, mut crew) = crewed();
    let officer = crew[1].0;
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 2);
    let rocket = missile(&game).unwrap();
    frames(&mut game, &mut crew[1..], 0, 0.0, 30);
    // A fresh press sets it off.
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 2);
    assert!(!game.world.entities.has::<Vehicle>(rocket), "it went off");
    assert_eq!(game.world.entities.of::<Player>(officer).seat, Some(Seat { vehicle: vid, index: 1, remote: false, from: None }));
    // Still holding fire, nothing else goes up until it is pressed again.
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 5);
    assert!(missile(&game).is_none());
    frames(&mut game, &mut crew[1..], 0, 0.0, 2);
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 2);
    assert!(missile(&game).is_some(), "a second one goes up from the next tube");
    assert_eq!(game.seen(vid).unwrap().sub.unwrap().tubes, 0b1100);
}

#[test]
fn giving_a_missile_up_brings_the_officer_back_too() {
    let (mut game, vid, mut crew) = crewed();
    let officer = crew[1].0;
    frames(&mut game, &mut crew[1..], BUTTON_FIRE, 0.0, 2);
    frames(&mut game, &mut crew[1..], 0, 0.0, 10);
    frames(&mut game, &mut crew[1..], BUTTON_USE, 0.0, 1);
    frames(&mut game, &mut crew[1..], 0, 0.0, 3);
    assert!(missile(&game).is_none());
    assert_eq!(game.world.entities.of::<Player>(officer).seat, Some(Seat { vehicle: vid, index: 1, remote: false, from: None }));
}
