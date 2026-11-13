use super::*;
use crate::game::{Command, Game, Outgoing};
use crate::mechanics::Wounds;
use crate::mechanics::for_tests::{Harm, damage_entity};
use shared::protocol::{LIMB_ARM_L, WEAPON_RIFLE};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A soldier standing on a flat 400 m square.
fn soldier() -> (Game, u32, mpsc::Receiver<Outgoing>) {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let (outbox, inbox) = mpsc::channel(1 << 16);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    let id = response.try_recv().unwrap().unwrap();
    let mut player = game.world.entities.of_mut::<Player>(id);
    player.character.position = [40.0, sim::STANDING_Y, 40.0];
    player.character.vy = 0.0;
    (game, id, inbox)
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32).round() as u32 {
        game.step();
    }
}

fn hp(game: &Game, id: u32) -> u8 {
    game.world.entities.of::<Health>(id).hp
}

#[test]
fn a_hurt_soldier_waits_then_heals_to_full_in_a_minute() {
    let (mut game, id, _inbox) = soldier();
    game.world.entities.get_mut::<Health>(id).unwrap().hp = 1;
    run(&mut game, MEND_DELAY_SECONDS - 0.1);
    assert_eq!(hp(&game, id), 1, "no healing while the hurt is fresh");
    run(&mut game, 0.1 + MEND_SECONDS / 2.0);
    let half = hp(&game, id) as i32;
    assert!((half - (1 + PLAYER_MAX_HP as i32 / 2)).abs() <= 1, "half way after half a minute: {half}");
    run(&mut game, MEND_SECONDS / 2.0 + 0.1);
    assert_eq!(hp(&game, id), PLAYER_MAX_HP);
}

#[test]
fn the_head_mends_but_the_limbs_do_not() {
    let (mut game, id, _inbox) = soldier();
    game.world.entities.insert(id, Wounds { lost: LIMB_ARM_L, bleeding: 0, taken: [0, 0, 20, 0, HEAD_HP - 1] });
    run(&mut game, MEND_DELAY_SECONDS + MEND_SECONDS + 0.1);
    let wounds = game.world.entities.of::<Wounds>(id);
    assert_eq!(wounds.taken[4], 0, "the head is whole");
    assert_eq!(wounds.taken[2], 20, "the leg is still hurt");
    assert_eq!(wounds.lost, LIMB_ARM_L, "the arm is still gone");
}

#[test]
fn being_hurt_again_starts_the_wait_over() {
    let (mut game, id, _inbox) = soldier();
    game.world.entities.get_mut::<Health>(id).unwrap().hp = 50;
    run(&mut game, MEND_DELAY_SECONDS + 10.0);
    let healing = hp(&game, id);
    assert!(healing > 50, "healing under way");
    damage_entity(&mut game.world, id, Harm::new(10, None, WEAPON_RIFLE));
    let hurt = hp(&game, id);
    run(&mut game, MEND_DELAY_SECONDS - 0.5);
    assert_eq!(hp(&game, id), hurt, "the wait begins again");
}
