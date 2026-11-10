use super::*;
use crate::game::{Command, Game, Profile};
use crate::mechanics::bots::{HEAR_SHOT, make_noise};
use crate::mechanics::life::{Harm, damage_entity};
use shared::protocol::{DEATH_BLAST, DEATH_BODY, FLAG_DEAD, FLAG_GRAZING, InputFrame, WEAPON_GRENADE, WEAPON_RIFLE};
use shared::world::{PlacedAnimal, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A flat 400 m square with a herd of `count` of `kind` about (50, 0), and its spawn far off in a corner.
fn game(kind: u8, count: u8) -> Game {
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 20_000,
        spawns: vec![[-15_000, -15_000]],
        placed_animals: vec![PlacedAnimal { kind, at: [5_000, 0], count }],
        ..Default::default()
    };
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

/// Puts player `id` standing at `[x, z]`.
fn put(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    let y = game.world.map.ground.height_at(x, z) + sim::STANDING_Y;
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [x, y, z];
}

/// Steps a tick with player `id` at `[x, z]`, as they hold `forward` and `buttons`: walking on the
/// spot, so to speak, for them to be seen as they go about it.
fn hold(game: &mut Game, id: u32, place: [f32; 2], forward: i8, buttons: u16) {
    let seq = game.world.tick + 1;
    game.handle(Command::Input(id, 0, vec![InputFrame { seq, forward, buttons, ..Default::default() }]));
    put(game, id, place);
    game.step();
}

fn animals(game: &Game) -> Vec<u32> {
    game.world.entities.ids::<With<Wild>>()
}

fn wild(game: &Game, id: u32) -> Wild {
    *game.world.entities.of::<Wild>(id)
}

fn at(game: &Game, id: u32) -> [f32; 2] {
    let [x, _, z] = game.world.entities.of::<Transform>(id).position;
    [x, z]
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

#[test]
fn a_placed_herd_stands_about_its_spot_as_deer_seen_by_clients() {
    let game = game(KIND_DEER, 5);
    let herd = animals(&game);
    assert_eq!(herd.len(), 5);
    for &id in &herd {
        assert!(distance(at(&game, id), [50.0, 0.0]) <= DEER_HERD_RADIUS / 2.0 + 0.01);
        assert_eq!(game.world.entities.of::<Health>(id).hp, DEER_HP);
    }
    assert_eq!(game.seen_people(KIND_DEER).len(), 5);
    let stags = self::game(KIND_STAG, 1);
    assert_eq!(stags.seen_people(KIND_STAG).len(), 1);
    assert_eq!(wild(&stags, animals(&stags)[0]).nerve, STAG_NERVE);
}

#[test]
fn left_alone_it_grazes_and_wanders_about_its_pasture() {
    let mut game = game(KIND_DEER, 1);
    let id = animals(&game)[0];
    let (mut grazed, mut walked, mut flagged) = (false, false, false);
    for _ in 0..(90 * TICK_RATE) {
        game.step();
        let w = wild(&game, id);
        assert!(matches!(w.mode, WildMode::Graze | WildMode::Walk), "calm, not {:?}", w.mode);
        // Its own spot is up to half a herd radius from the herd's, and it grazes up to a radius from that.
        assert!(distance(at(&game, id), [50.0, 0.0]) <= DEER_HERD_RADIUS * 1.5 + 1.0, "strayed");
        grazed |= w.mode == WildMode::Graze;
        walked |= w.mode == WildMode::Walk;
        flagged |= game.seen_people(KIND_DEER).iter().any(|(_, seen)| seen.flags.as_ref().is_some_and(|f| f.flags & FLAG_GRAZING != 0));
    }
    assert!(grazed && walked && flagged, "grazed {grazed}, walked {walked}, shown grazing {flagged}");
}

#[test]
fn it_looks_up_at_someone_walking_near_and_bolts_from_them_coming_nearer() {
    let mut game = game(KIND_DEER, 1);
    let id = animals(&game)[0];
    let player = join(&mut game);
    let deer = at(&game, id);
    let (far, near) = ([deer[0] - DEER_NOTICE_WALK * 0.8, deer[1]], [deer[0] - DEER_NOTICE_WALK * 0.4, deer[1]]);
    for _ in 0..LOOK_EVERY * 2 {
        hold(&mut game, player, far, 1, 0);
    }
    assert_eq!(wild(&game, id).mode, WildMode::Alert, "it looks up");
    for _ in 0..LOOK_EVERY * 2 {
        hold(&mut game, player, near, 1, 0);
    }
    assert_eq!(wild(&game, id).mode, WildMode::Flee, "it bolts");
    let before = distance(at(&game, id), near);
    for _ in 0..3 * TICK_RATE {
        hold(&mut game, player, near, 0, 0);
    }
    let after = distance(at(&game, id), near);
    assert!(after > before + 15.0, "it ran off: {before} m to {after} m");
}

#[test]
fn it_does_not_notice_someone_crouched_and_still_at_fifteen_metres() {
    let mut game = game(KIND_DEER, 1);
    let id = animals(&game)[0];
    let player = join(&mut game);
    for _ in 0..(10 * TICK_RATE) {
        let deer = at(&game, id);
        hold(&mut game, player, [deer[0] - 15.0, deer[1]], 0, BUTTON_CROUCH);
        assert!(matches!(wild(&game, id).mode, WildMode::Graze | WildMode::Walk), "calm, not {:?}", wild(&game, id).mode);
    }
}

#[test]
fn a_gunshot_it_hears_scatters_the_whole_herd_and_they_settle_again_further_off() {
    let mut game = game(KIND_DEER, 4);
    let herd = animals(&game);
    let shot = [0.0, 1.0, 0.0];
    make_noise(&mut game.world, shot, None, HEAR_SHOT);
    run(&mut game, 1.0);
    for &id in &herd {
        assert_eq!(wild(&game, id).mode, WildMode::Flee, "deer {id} runs");
    }
    run(&mut game, 20.0);
    for &id in &herd {
        let gap = distance(at(&game, id), [shot[0], shot[2]]);
        assert!(gap > 50.0 + DEER_FLEE_DISTANCE * 0.5, "deer {id} only {gap} m from the shot");
        assert_ne!(wild(&game, id).mode, WildMode::Flee, "and stopped");
    }
}

#[test]
fn a_noise_beyond_its_reach_leaves_it_grazing() {
    let mut game = game(KIND_DEER, 1);
    let id = animals(&game)[0];
    make_noise(&mut game.world, [-150.0, 1.0, 0.0], None, HEAR_SHOT);
    run(&mut game, 1.0);
    assert_ne!(wild(&game, id).mode, WildMode::Flee);
}

#[test]
fn a_shot_deer_bolts_and_one_shot_dead_sends_its_herd_off_and_comes_back_at_its_spot() {
    let mut game = game(KIND_DEER, 2);
    let [first, second] = animals(&game)[..] else { panic!("two deer") };
    let home = game.world.entities.of::<Home>(first).0;
    let mut harm = Harm::new(10, None, WEAPON_RIFLE);
    harm.from = Some([0.0, 1.0, 0.0]);
    assert!(!damage_entity(&mut game.world, first, harm));
    game.step();
    assert_eq!(wild(&game, first).mode, WildMode::Flee);
    let struck = game.seen_people(KIND_DEER).into_iter().find(|(seen, _)| *seen == first).and_then(|(_, seen)| seen.struck);
    assert_eq!(struck.map(|struck| (struck.count, struck.amount)), Some((1, 10)), "everyone sees it struck");

    let mut game = self::game(KIND_DEER, 2);
    let mut harm = Harm::new(DEER_HP, None, WEAPON_RIFLE);
    harm.from = Some([0.0, 1.0, 0.0]);
    assert!(damage_entity(&mut game.world, first, harm));
    game.step();
    assert_eq!(wild(&game, second).mode, WildMode::Flee, "its herd-mate runs");
    let seen = |game: &Game| game.seen_people(KIND_DEER).into_iter().find(|(seen, _)| *seen == first).map(|(_, seen)| seen);
    let dead = seen(&game).expect("the dead one lies where it fell");
    assert_ne!(dead.flags.unwrap().flags & FLAG_DEAD, 0);
    assert_eq!(dead.fell.map(|fell| fell.style), Some(DEATH_BODY));
    assert_eq!(dead.struck.map(|struck| struck.amount), Some(DEER_HP), "with the hit that killed it");
    run(&mut game, DEER_RESPAWN_TICKS as f32 / TICK_RATE as f32 + 0.5);
    assert!(game.world.entities.alive(first));
    assert!(distance(at(&game, first), home) < DEER_HERD_RADIUS + 1.0, "back about its spot");
    let back = seen(&game).expect("drawn again");
    assert_eq!(back.flags.unwrap().flags & FLAG_DEAD, 0);
    assert!(back.fell.is_none() && back.struck.is_none(), "whole again");
}

#[test]
fn it_never_runs_into_water() {
    let mut game = game(KIND_DEER, 1);
    let id = animals(&game)[0];
    // A lake from x = 60 m on, straight away from a shot at (0, 0), over flat ground sampled every 5 m.
    let (side, cell, origin) = (81, 500, -20_000);
    game.world.map.ground = shared::terrain::Ground { origin, cell, side: side as u32, heights: vec![0; side * side] };
    game.world.map.water = (0..side * side).map(|at| if origin + (at % side) as i32 * cell >= 6_000 { 100 } else { shared::world::NO_WATER }).collect();
    make_noise(&mut game.world, [0.0, 1.0, 0.0], None, HEAR_SHOT);
    for _ in 0..(15 * TICK_RATE) {
        game.step();
        assert!(!wet(&game.world, at(&game, id)), "in the water at {:?}", at(&game, id));
    }
    assert!(distance(at(&game, id), [0.0, 0.0]) > 50.0, "it still got away");
}

#[test]
fn a_deer_blown_up_lies_as_a_blast_left_it() {
    let mut game = game(KIND_DEER, 1);
    let id = animals(&game)[0];
    assert!(damage_entity(&mut game.world, id, Harm::new(DEER_HP, None, WEAPON_GRENADE)));
    game.step();
    assert_eq!(game.world.entities.get::<Fell>(id).copied(), Some(Fell(DEATH_BLAST)));
}
