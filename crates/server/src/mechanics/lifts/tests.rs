use crate::game::{Command, Game, Player};
use shared::consts::*;
use shared::lifts::Path;
use shared::protocol::InputFrame;
use shared::world::{LIFT_CHAIR, Lift, WORLD_VERSION, WorldFile};
use sim::Character;
use tokio::sync::{mpsc, oneshot};

/// A lift 200 m long on flat ground, going north (-Z) from the origin over one 9 m pylon.
fn lift() -> Lift {
    Lift { kind: LIFT_CHAIR, seats: 2, points: vec![[0, 0], [0, -10_000], [0, -20_000]], base: vec![0, 0, 0], cable: vec![310, 900, 310] }
}

fn game() -> (Game, Vec<mpsc::Receiver<crate::game::Outgoing>>) {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[2_000, 2_000]], lifts: vec![lift()], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    (game, Vec::new())
}

fn join(game: &mut Game, inboxes: &mut Vec<mpsc::Receiver<crate::game::Outgoing>>) -> u32 {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    inboxes.push(inbox);
    response.try_recv().unwrap().unwrap()
}

/// Stands soldier `id` on the ground at `[x, z]`.
fn stand(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    game.world.entities.of_mut::<Player>(id).character = Character { position: [x, STAND_CENTRE, z], grounded: true, ..Default::default() };
}

fn input(game: &mut Game, id: u32, seq: &mut u32, buttons: u16) {
    *seq += 1;
    game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
}

fn step(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        input(game, id, seq, buttons);
        game.step();
    }
}

/// Frames of input the jitter buffer holds before it gives one out: a press is applied this many
/// ticks after it is sent.
const BUFFERED: u32 = 2;

/// Sets the clock so chair 0 is `along` metres round the loop when a press sent now is applied, and
/// returns where its first seat is then.
fn chair_at(game: &mut Game, along: f32) -> [f32; 3] {
    let path = Path::new(&lift());
    let tick = (along / LIFT_SPEED * TICK_RATE as f32).round() as u32;
    game.world.tick = tick - BUFFERED;
    path.place(&path.chair(0, tick), 0)
}

#[test]
fn a_soldier_boards_at_the_bottom_and_rides_round_and_round_until_they_jump() {
    let (mut game, mut inboxes) = game();
    let id = join(&mut game, &mut inboxes);
    let mut seq = 0;
    step(&mut game, id, &mut seq, 0, 10);
    let seat = chair_at(&mut game, 3.0);
    stand(&mut game, id, [seat[0], seat[2]]);
    step(&mut game, id, &mut seq, BUTTON_USE, 1);
    step(&mut game, id, &mut seq, 0, BUFFERED);
    let ride = game.world.entities.of::<Player>(id).lift.expect("sat on the chair");
    assert_eq!((ride.lift, ride.chair, ride.seat), (0, 0, 0));
    // Halfway up the chair is over the pylon, high off the ground.
    let path = Path::new(&lift());
    let ticks = |metres: f32| (metres / LIFT_SPEED * TICK_RATE as f32) as u32;
    step(&mut game, id, &mut seq, 0, ticks(path.up / 2.0 - 3.0));
    let character = game.world.entities.of::<Player>(id).character;
    assert!(character.position[1] > 6.0, "{character:?}");
    assert!(character.position[2] < -90.0, "carried north: {character:?}");
    assert!(game.world.entities.of::<Player>(id).crouched(), "sitting");
    // Round the top and back down: still aboard, and pressing USE on the way changes nothing.
    step(&mut game, id, &mut seq, 0, ticks(path.up / 2.0 + 15.0));
    step(&mut game, id, &mut seq, BUTTON_USE, 1);
    step(&mut game, id, &mut seq, 0, ticks(path.down - path.up - 20.0));
    let (lift, character) = { let player = game.world.entities.of::<Player>(id); (player.lift, player.character) };
    assert!(lift.is_some(), "rides on past the top station");
    assert!(character.position[0] < 0.0 && character.position[2] > -20.0, "coming down the left side near the bottom: {character:?}");
    // Round the bottom and up again.
    step(&mut game, id, &mut seq, 0, ticks(30.0));
    let (lift, character) = { let player = game.world.entities.of::<Player>(id); (player.lift, player.character) };
    assert!(lift.is_some() && character.position[0] > 0.0, "going up again: {character:?}");
    // A jump at the bottom station steps off onto its deck.
    step(&mut game, id, &mut seq, BUTTON_JUMP, 1);
    step(&mut game, id, &mut seq, 0, BUFFERED + 60);
    let (lift, character) = { let player = game.world.entities.of::<Player>(id); (player.lift, player.character) };
    assert!(lift.is_none(), "jumped off");
    // The station's deck is 10 cm over the ground.
    assert!(character.grounded && character.position[1] < STAND_CENTRE + 0.2, "on the station's deck: {character:?}");
    assert!(game.world.entities.alive(id));
}

#[test]
fn letting_go_halfway_up_drops_the_rider() {
    let (mut game, mut inboxes) = game();
    let id = join(&mut game, &mut inboxes);
    let mut seq = 0;
    step(&mut game, id, &mut seq, 0, 10);
    let seat = chair_at(&mut game, 3.0);
    stand(&mut game, id, [seat[0], seat[2]]);
    step(&mut game, id, &mut seq, BUTTON_USE, 1);
    step(&mut game, id, &mut seq, 0, 15 * TICK_RATE);
    assert!(game.world.entities.of::<Player>(id).lift.is_some());
    step(&mut game, id, &mut seq, BUTTON_JUMP, 1);
    step(&mut game, id, &mut seq, 0, BUFFERED);
    let character = game.world.entities.of::<Player>(id).character;
    assert!(game.world.entities.of::<Player>(id).lift.is_none());
    assert!(character.knock[1] < -3.0, "keeps the cable's speed: {character:?}");
    step(&mut game, id, &mut seq, 0, 3 * TICK_RATE);
    let character = game.world.entities.of::<Player>(id).character;
    assert!(character.position[1] < STAND_CENTRE + 0.1, "fell to the ground: {character:?}");
}

#[test]
fn nobody_can_reach_a_chair_high_over_their_head() {
    let (mut game, mut inboxes) = game();
    let id = join(&mut game, &mut inboxes);
    let mut seq = 0;
    step(&mut game, id, &mut seq, 0, 10);
    let seat = chair_at(&mut game, 100.0);
    assert!(seat[1] > 5.0);
    stand(&mut game, id, [seat[0], seat[2]]);
    step(&mut game, id, &mut seq, BUTTON_USE, 1);
    step(&mut game, id, &mut seq, 0, BUFFERED);
    assert!(game.world.entities.of::<Player>(id).lift.is_none());
}

#[test]
fn a_chair_for_two_takes_two_and_not_a_third() {
    let (mut game, mut inboxes) = game();
    let ids: Vec<u32> = (0..3).map(|_| join(&mut game, &mut inboxes)).collect();
    let mut seqs = [0u32; 3];
    let all = |game: &mut Game, seqs: &mut [u32; 3], buttons: u16| {
        for (i, &id) in ids.iter().enumerate() {
            input(game, id, &mut seqs[i], buttons);
        }
        game.step();
    };
    for _ in 0..10 {
        all(&mut game, &mut seqs, 0);
    }
    let seat = chair_at(&mut game, 3.0);
    for &id in &ids {
        stand(&mut game, id, [seat[0] + 0.3, seat[2]]);
    }
    all(&mut game, &mut seqs, BUTTON_USE);
    for _ in 0..BUFFERED {
        all(&mut game, &mut seqs, 0);
    }
    let rides: Vec<_> = ids.iter().filter_map(|&id| game.world.entities.of::<Player>(id).lift).collect();
    assert_eq!(rides.len(), 2, "{rides:?}");
    assert_ne!(rides[0].seat, rides[1].seat);
    assert_eq!(rides[0].chair, rides[1].chair);
}
