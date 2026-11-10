use crate::entities::Health;
use crate::game::{Command, Game, Player};
use shared::consts::*;
use shared::protocol::InputFrame;
use shared::world::{NO_WATER, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// Flat ground at 0 over 200 m in 5 m cells, with a lake 5 m deep within 50 m of the middle.
fn lake() -> (Game, u32, mpsc::Receiver<crate::game::Outgoing>) {
    let side = 41u32;
    let wet = |i: u32, j: u32| (i as f32 * 5.0 - 100.0).abs() <= 50.0 && (j as f32 * 5.0 - 100.0).abs() <= 50.0;
    let (mut heights, mut water) = (Vec::new(), Vec::new());
    for j in 0..side {
        for i in 0..side {
            heights.push(if wet(i, j) { -500 } else { 0 });
            water.push(if wet(i, j) { 0 } else { NO_WATER });
        }
    }
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 10_000,
        ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights },
        water,
        spawns: vec![[-8_000, 0]],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let (outbox, inbox) = mpsc::channel(1 << 16);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    let id = response.try_recv().unwrap().unwrap();
    let character = &mut game.world.entities.of_mut::<Player>(id).character;
    *character = sim::Character { position: [0.0, -SWIM_FLOAT, 0.0], swimming: true, ..Default::default() };
    (game, id, inbox)
}

fn hold(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, buttons, slot: 0, ..Default::default() }]));
        game.step();
    }
}

fn hp(game: &Game, id: u32) -> u8 {
    game.world.entities.get::<Health>(id).map_or(0, |health| health.hp)
}

#[test]
fn a_diver_runs_out_of_air_after_thirty_seconds_and_drowns() {
    let (mut game, id, _inbox) = lake();
    let mut seq = 0;
    hold(&mut game, id, &mut seq, BUTTON_CROUCH, 30.5);
    assert!(game.world.physics.submerged(&game.world.entities.of::<Player>(id).character), "down on the bed");
    assert_eq!(hp(&game, id), PLAYER_MAX_HP, "thirty seconds of air");
    hold(&mut game, id, &mut seq, BUTTON_CROUCH, 1.0);
    assert_eq!(hp(&game, id), PLAYER_MAX_HP - DROWN_DAMAGE, "a second out of air");
    hold(&mut game, id, &mut seq, BUTTON_CROUCH, 9.0);
    assert!(!game.world.entities.alive(id), "drowned from full health in ten seconds");
}

#[test]
fn surfacing_refills_breath() {
    let (mut game, id, _inbox) = lake();
    let mut seq = 0;
    hold(&mut game, id, &mut seq, BUTTON_CROUCH, 25.0);
    assert!(game.world.entities.of::<Player>(id).breath < BREATH_FULL / 4);
    hold(&mut game, id, &mut seq, BUTTON_JUMP, 10.0);
    assert_eq!(game.world.entities.of::<Player>(id).breath, BREATH_FULL);
    hold(&mut game, id, &mut seq, BUTTON_CROUCH, 25.0);
    assert_eq!(hp(&game, id), PLAYER_MAX_HP, "a fresh breath lasts again");
}

#[test]
fn a_swimmer_cannot_fire_and_shows_no_gun() {
    let (mut game, id, _inbox) = lake();
    let mut seq = 0;
    let ammo = game.world.entities.of::<Player>(id).hands.hotbar[0].ammo;
    hold(&mut game, id, &mut seq, BUTTON_FIRE, 1.0);
    assert!(game.world.entities.of::<Player>(id).character.swimming);
    assert_eq!(game.world.entities.of::<Player>(id).hands.hotbar[0].ammo, ammo);
    let scene = game.replicated();
    let (_, me) = scene.iter().find(|(entity, _)| *entity == id).unwrap();
    let flags = me.flags.as_ref().unwrap();
    assert_eq!(flags.flags & shared::protocol::FLAG_HELD_MASK, 0, "the gun is put away");
    assert_eq!(flags.stance, STANCE_SWIM);
}

#[test]
fn a_diver_shows_diving() {
    let (mut game, id, _inbox) = lake();
    let mut seq = 0;
    hold(&mut game, id, &mut seq, BUTTON_CROUCH, 2.0);
    let scene = game.replicated();
    let (_, me) = scene.iter().find(|(entity, _)| *entity == id).unwrap();
    assert_eq!(me.flags.as_ref().unwrap().stance, STANCE_DIVE);
    assert!(me.vitals.as_ref().unwrap().breath < 255);
}

