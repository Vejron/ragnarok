use super::*;
use crate::game::{Command, Game, Outgoing};
use crate::mechanics::for_tests::{Harm, damage_entity};
use shared::protocol::{InputFrame, WEAPON_RIFLE, decode, peek_tag};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// Where the backpack starts in these tests: well clear of the car parked on the spawn point.
const HOME: [f32; 2] = [40.0, 40.0];

/// A flat 400 m square with the backpack at `HOME`, not yet laid down.
fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game.world.rng = crate::rng::Rng::from_seed(3);
    game.world.backpack = Some(BackpackState::new(HOME));
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

fn state(game: &Game) -> &BackpackState {
    game.world.backpack.as_ref().unwrap()
}

/// Puts player `id` standing at `[x, z]`.
fn stand(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    let mut player = game.world.entities.of_mut::<Player>(id);
    player.character.position = [x, sim::STANDING_Y, z];
    player.character.vy = 0.0;
}

/// Steps `ticks` with player `id` walking forward (`forward` of 127) facing -Z.
fn walk(game: &mut Game, ids: &[u32], seq: &mut u32, forward: i8, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        for &id in ids {
            game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, forward, ..Default::default() }]));
        }
        game.step();
    }
}

/// The backpack's circles sent since the last look.
fn circles(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<Circle> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_BACKPACK) => decode::<Circle>(&bytes),
            _ => None,
        })
        .collect()
}

/// Player `id` with the backpack, standing at home.
fn carrying() -> (Game, u32, mpsc::Receiver<Outgoing>) {
    let mut game = game();
    let (id, inbox) = join(&mut game);
    game.step();
    stand(&mut game, id, HOME);
    game.step();
    assert_eq!(state(&game).carrier, Some(id));
    (game, id, inbox)
}

#[test]
fn a_world_without_the_backpack_has_none() {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    let (_, mut inbox) = join(&mut game);
    game.step();
    assert!(lying(&game.world).is_empty());
    assert!(circles(&mut inbox).is_empty());
}

#[test]
fn it_is_laid_at_home_and_everyone_is_told_a_circle_around_it() {
    let mut game = game();
    let (_, mut inbox) = join(&mut game);
    game.step();
    let lying = lying(&game.world);
    assert_eq!(lying.len(), 1);
    let [x, _, z] = lying[0].1;
    assert!((x - HOME[0]).abs() < 1e-3 && (z - HOME[1]).abs() < 1e-3);
    let circle = *circles(&mut inbox).last().expect("a circle");
    assert_eq!((circle.carrier, circle.radius), (0, (BACKPACK_WIDE * 50.0) as u16));
    let off = (x - circle.x as f32 / 100.0).hypot(z - circle.z as f32 / 100.0);
    assert!(off <= circle.radius as f32 / 100.0, "{off} m out");
}

#[test]
fn a_joiner_is_told_the_circle() {
    let mut game = game();
    join(&mut game);
    game.step();
    let (_, mut inbox) = join(&mut game);
    assert_eq!(circles(&mut inbox).len(), 1);
}

#[test]
fn walking_onto_it_takes_it() {
    let (game, id, _inbox) = carrying();
    assert!(carries(&game.world, id));
    assert!(lying(&game.world).is_empty());
    let shown = game.replicated();
    let entity = &shown.iter().find(|(eid, _)| *eid == id).unwrap().1;
    assert_eq!(entity.carry, Some(shared::replicated::Carry { what: CARRY_BACKPACK }));
}

#[test]
fn out_of_reach_it_stays_on_the_ground() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    game.step();
    stand(&mut game, id, [HOME[0] + BACKPACK_REACH + 1.0, HOME[1]]);
    game.step();
    assert_eq!(state(&game).carrier, None);
    assert_eq!(lying(&game.world).len(), 1);
}

#[test]
fn the_carrier_heals() {
    let (mut game, id, _inbox) = carrying();
    game.world.entities.get_mut::<Health>(id).unwrap().hp = 50;
    for _ in 0..TICK_RATE * 2 {
        game.step();
    }
    let hp = game.world.entities.of::<Health>(id).hp;
    assert_eq!(hp, 50 + (2.0 * BACKPACK_REGEN) as u8);
    for _ in 0..TICK_RATE * 30 {
        game.step();
    }
    assert_eq!(game.world.entities.of::<Health>(id).hp, PLAYER_MAX_HP, "no more than full");
}

#[test]
fn the_carrier_walks_twice_as_fast() {
    let (mut game, carrier, _inbox) = carrying();
    let (other, _other_inbox) = join(&mut game);
    stand(&mut game, carrier, [60.0, 60.0]);
    stand(&mut game, other, [80.0, 60.0]);
    let mut seq = 0;
    walk(&mut game, &[carrier, other], &mut seq, 0, 10);
    let start = |game: &Game, id| game.world.entities.of::<Player>(id).character.position[2];
    let (a, b) = (start(&game, carrier), start(&game, other));
    walk(&mut game, &[carrier, other], &mut seq, 127, TICK_RATE);
    let (da, db) = (a - start(&game, carrier), b - start(&game, other));
    assert!(db > 1.0, "the other walked {db} m");
    let ratio = da / db;
    assert!((ratio - BACKPACK_SPEED).abs() < 0.15, "{da} m against {db} m");
}

#[test]
fn dying_drops_it_where_the_body_lies() {
    let (mut game, id, _inbox) = carrying();
    stand(&mut game, id, [70.0, 50.0]);
    game.step();
    let at = game.world.entities.of::<Player>(id).character.position;
    damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, None, WEAPON_RIFLE));
    game.step();
    assert_eq!(state(&game).carrier, None);
    assert!(!carries(&game.world, id));
    let lying = lying(&game.world);
    assert_eq!(lying.len(), 1);
    let [x, y, z] = lying[0].1;
    assert!((x - at[0]).abs() < 0.5 && (z - at[2]).abs() < 0.5, "at {x}, {z}, not {at:?}");
    assert!(y.abs() < 0.1, "on the ground, not {y}");
}

#[test]
fn leaving_drops_it() {
    let (mut game, id, _inbox) = carrying();
    stand(&mut game, id, [70.0, 50.0]);
    game.handle(Command::Leave(id));
    assert_eq!(state(&game).carrier, None);
    let [x, _, z] = lying(&game.world)[0].1;
    assert!((x - 70.0).abs() < 0.1 && (z - 50.0).abs() < 0.1);
}

#[test]
fn dropped_off_the_world_it_goes_home() {
    let (mut game, id, _inbox) = carrying();
    stand(&mut game, id, [500.0, 0.0]);
    game.handle(Command::Leave(id));
    let [x, _, z] = lying(&game.world)[0].1;
    assert_eq!([x, z], HOME);
}

#[test]
fn left_lying_it_goes_home() {
    let (mut game, id, _inbox) = carrying();
    stand(&mut game, id, [70.0, 50.0]);
    game.handle(Command::Leave(id));
    let away = lying(&game.world)[0].0;
    game.world.backpack.as_mut().unwrap().since = game.world.tick.wrapping_sub((BACKPACK_RETURN_SECONDS * TICK_RATE as f32) as u32 - 1);
    game.step();
    assert_eq!(lying(&game.world)[0].0, away, "not yet");
    game.step();
    let lying = lying(&game.world);
    assert_eq!(lying.len(), 1);
    assert_eq!([lying[0].1[0], lying[0].1[2]], HOME);
    // At home it stays put, however long.
    let home = lying[0].0;
    game.world.backpack.as_mut().unwrap().since = 0;
    game.world.tick += (BACKPACK_RETURN_SECONDS * TICK_RATE as f32) as u32 * 2;
    game.step();
    assert_eq!(super::lying(&game.world)[0].0, home);
}

#[test]
fn the_circle_narrows_in_one_pair_of_hands() {
    assert_eq!(radius_after(0), BACKPACK_WIDE / 2.0);
    let full = (BACKPACK_NARROW_SECONDS * TICK_RATE as f32) as u32;
    assert!((radius_after(full / 2) - (BACKPACK_WIDE + BACKPACK_NARROW) / 4.0).abs() < 1e-3);
    assert_eq!(radius_after(full), BACKPACK_NARROW / 2.0);
    assert_eq!(radius_after(full * 3), BACKPACK_NARROW / 2.0);

    let (mut game, id, mut inbox) = carrying();
    game.world.backpack.as_mut().unwrap().since = game.world.tick.wrapping_sub(full);
    for _ in 0..TICK_RATE {
        game.step();
    }
    let circle = *circles(&mut inbox).last().unwrap();
    assert_eq!((circle.carrier, circle.radius), (id, (BACKPACK_NARROW * 50.0) as u16));

    // A new pair of hands starts it wide again.
    let (other, _other_inbox) = join(&mut game);
    let at = position_of(&game.world, id).unwrap();
    damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, None, WEAPON_RIFLE));
    game.step();
    stand(&mut game, other, [at[0], at[2]]);
    game.step();
    assert_eq!(state(&game).carrier, Some(other));
    assert_eq!(state(&game).radius, BACKPACK_WIDE / 2.0);
}

#[test]
fn the_backpack_is_always_inside_the_circle() {
    let (mut game, id, _inbox) = carrying();
    let full = (BACKPACK_NARROW_SECONDS * TICK_RATE as f32) as u32;
    game.world.backpack.as_mut().unwrap().since = game.world.tick.wrapping_sub(full) + TICK_RATE * 5;
    let mut moves = 0;
    let mut last = state(&game).centre;
    for tick in 0..TICK_RATE * 20 {
        let seq = tick + 1;
        // Running in a wide circle, so it leaves the circle's middle every way.
        let yaw = shared::quant::to_mrad(tick as f32 * 0.01);
        game.handle(Command::Input(id, 0, vec![InputFrame { seq, forward: 127, yaw, ..Default::default() }]));
        game.step();
        let state = state(&game);
        let at = position_of(&game.world, id).unwrap();
        let out = (at[0] - state.centre[0]).hypot(at[2] - state.centre[1]);
        assert!(out <= state.radius, "{out} m from the middle of a {} m circle", state.radius);
        if state.centre != last {
            moves += 1;
            last = state.centre;
        }
    }
    assert!(moves >= 2, "the circle moved {moves} times");
}

#[test]
fn the_carrier_scores_a_point_a_second() {
    let (mut game, id, mut inbox) = carrying();
    for _ in 0..TICK_RATE * 3 {
        game.step();
    }
    assert_eq!(game.world.entities.of::<Player>(id).held, 3);
    assert_eq!(circles(&mut inbox).last().unwrap().held, 3);
}
