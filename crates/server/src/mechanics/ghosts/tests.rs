use super::*;
use crate::game::{Command, Game, TEAM_NONE};
use crate::mechanics::for_tests::{Harm, damage_entity};
use shared::protocol::{InputFrame, WEAPON_RIFLE};
use shared::world::{Building, NO_WATER, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// Flat ground at 0 over 200 m in 5 m cells, an entry point at (-80, 0) m, a 10 m house at (50, 50) m
/// and a pond around (-50, -50) m.
fn game() -> Game {
    let house = Building { height: 600, footprint: vec![[4_500, 4_500], [5_500, 4_500], [5_500, 5_500], [4_500, 5_500]], ..Default::default() };
    let side = 41u32;
    let pond = |i: u32, j: u32| (i as f32 * 5.0 - 100.0 + 50.0).abs() <= 10.0 && (j as f32 * 5.0 - 100.0 + 50.0).abs() <= 10.0;
    let water = (0..side).flat_map(|j| (0..side).map(move |i| if pond(i, j) { 0 } else { NO_WATER })).collect();
    let heights = (0..side).flat_map(|j| (0..side).map(move |i| if pond(i, j) { -300 } else { 0 })).collect();
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 10_000,
        ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights },
        water,
        buildings: vec![house],
        spawns: vec![[-8_000, 0]],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox) = mpsc::channel(1 << 16);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    response.try_recv().unwrap().unwrap()
}

/// Puts player `id` standing at `[x, z]` metres.
fn stand(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    game.world.entities.of_mut::<Player>(id).character = sim::Character { position: [x, STANDING_Y, z], grounded: true, ..Default::default() };
}

fn kill(game: &mut Game, id: u32) {
    damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, None, WEAPON_RIFLE));
}

thread_local! {
    /// The sequence number of the last input sent.
    static SEQ: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Steps `ticks` ticks with player `id` sending `input` (a fresh sequence number each).
fn hold(game: &mut Game, id: u32, input: InputFrame, ticks: u32) {
    for _ in 0..ticks {
        let seq = SEQ.with(|seq| {
            seq.set(seq.get() + 1);
            seq.get()
        });
        game.handle(Command::Input(id, 0, vec![InputFrame { seq, ..input }]));
        game.step();
    }
}

fn press(game: &mut Game, id: u32, buttons: u16) {
    hold(game, id, InputFrame { buttons, ..Default::default() }, 1);
    hold(game, id, InputFrame::default(), 1);
}

/// Kills player `id` standing at `at` and waits for them to come back as a ghost, then long enough
/// that they may take shape.
fn haunting(game: &mut Game, id: u32, at: [f32; 2]) {
    stand(game, id, at);
    kill(game, id);
    hold(game, id, InputFrame::default(), RESPAWN_TICKS + GHOST_MIN_TICKS + 1);
    assert!(ghost(&game.world, id).is_some(), "a ghost");
}

fn position(game: &Game, id: u32) -> [f32; 3] {
    game.world.entities.of::<Player>(id).character.position
}

#[test]
fn a_killed_player_rises_as_a_ghost_after_the_death_pause_and_keeps_their_hotbar() {
    let mut game = game();
    let id = join(&mut game);
    stand(&mut game, id, [10.0, 10.0]);
    game.world.entities.of_mut::<Player>(id).hands.hotbar[0].ammo = 3;
    let hotbar = game.world.entities.of::<Player>(id).hands.hotbar;
    kill(&mut game, id);
    hold(&mut game, id, InputFrame::default(), RESPAWN_TICKS - 1);
    assert!(ghost(&game.world, id).is_none(), "still lying dead");
    hold(&mut game, id, InputFrame::default(), 2);
    assert!(ghost(&game.world, id).is_some());
    let [x, y, z] = position(&game, id);
    assert!((x - 10.0).abs() < 0.5 && (z - 10.0).abs() < 0.5 && y > STANDING_Y, "rose out of the body: {x} {y} {z}");
    assert!(!game.world.entities.alive(id), "a ghost is dead");
    assert_eq!(shown(&game.world, id).map(|(stance, _, block)| (stance, block)), Some((STANCE_GHOST, GHOST_BLOCK_SOON)));
    assert_eq!(game.world.entities.of::<Player>(id).hands.hotbar, hotbar);
}

#[test]
fn a_ghost_flies_where_it_looks_through_a_house() {
    let mut game = game();
    let id = join(&mut game);
    haunting(&mut game, id, [30.0, 50.0]);
    // Yaw -π/2 faces +X, through the house at (50, 50).
    hold(&mut game, id, InputFrame { forward: 127, yaw: -1571, ..Default::default() }, 3 * TICK_RATE);
    assert!(position(&game, id)[0] > 70.0, "{:?}", position(&game, id));
}

#[test]
fn a_bot_still_respawns_at_a_spawn_point() {
    let mut game = game();
    let bot = game.add_bot([20.0, 0.0], TEAM_NONE, Vec::new()).unwrap();
    kill(&mut game, bot);
    for _ in 0..RESPAWN_TICKS + 2 {
        game.step();
    }
    assert!(ghost(&game.world, bot).is_none());
    assert!(game.world.entities.alive(bot));
}

#[test]
fn a_ghost_cannot_take_shape_before_the_least_time() {
    let mut game = game();
    let id = join(&mut game);
    stand(&mut game, id, [10.0, 10.0]);
    kill(&mut game, id);
    hold(&mut game, id, InputFrame::default(), RESPAWN_TICKS + 10);
    press(&mut game, id, BUTTON_USE);
    assert_eq!(ghost(&game.world, id).map(|ghost| ghost.forming), Some(0));
}

#[test]
fn a_ghost_cannot_take_shape_near_a_living_enemy_but_may_beside_a_team_mate() {
    let mut game = game();
    let id = join(&mut game);
    let enemy = game.add_bot([20.0, 0.0], TEAM_NONE, Vec::new()).unwrap();
    stand(&mut game, enemy, [20.0, 0.0]);
    haunting(&mut game, id, [0.0, 0.0]);
    assert_eq!(shown(&game.world, id).map(|(_, _, block)| block), Some(GHOST_BLOCK_ENEMY));
    press(&mut game, id, BUTTON_USE);
    assert_eq!(ghost(&game.world, id).map(|ghost| ghost.forming), Some(0), "20 m from an enemy");

    // On the same team, the bot is no enemy.
    game.world.entities.of_mut::<Player>(id).team = 1;
    game.world.entities.of_mut::<Player>(enemy).team = 1;
    press(&mut game, id, BUTTON_USE);
    assert!(ghost(&game.world, id).is_some_and(|ghost| ghost.forming > 0));
}

#[test]
fn a_dead_enemy_does_not_keep_a_ghost_out() {
    let mut game = game();
    let id = join(&mut game);
    let enemy = game.add_bot([20.0, 0.0], TEAM_NONE, Vec::new()).unwrap();
    haunting(&mut game, id, [0.0, 0.0]);
    stand(&mut game, enemy, [GHOST_KEEPOUT + 1.0, 0.0]);
    assert_eq!(shown(&game.world, id).map(|(_, _, block)| block), Some(GHOST_BLOCK_NONE), "just outside");
    stand(&mut game, enemy, [5.0, 0.0]);
    kill(&mut game, enemy);
    assert_eq!(shown(&game.world, id).map(|(_, _, block)| block), Some(GHOST_BLOCK_NONE));
}

#[test]
fn a_ghost_cannot_take_shape_over_water_or_in_a_house() {
    let mut game = game();
    let id = join(&mut game);
    haunting(&mut game, id, [-50.0, -50.0]);
    assert_eq!(shown(&game.world, id).map(|(_, _, block)| block), Some(GHOST_BLOCK_GROUND), "over the pond");
    let [_, y, _] = position(&game, id);
    game.world.entities.of_mut::<Player>(id).character.position = [50.0, y, 50.0];
    assert_eq!(shown(&game.world, id).map(|(_, _, block)| block), Some(GHOST_BLOCK_GROUND), "in the house");
}

#[test]
fn taking_shape_lasts_the_forming_time_and_stands_the_soldier_on_the_ground_whole() {
    let mut game = game();
    let id = join(&mut game);
    haunting(&mut game, id, [10.0, 10.0]);
    hold(&mut game, id, InputFrame { buttons: BUTTON_JUMP, ..Default::default() }, TICK_RATE);
    assert!(position(&game, id)[1] > 5.0, "flew up");
    press(&mut game, id, BUTTON_USE);
    assert_eq!(shown(&game.world, id).map(|(stance, _, _)| stance), Some(STANCE_FORM));
    assert!(position(&game, id)[1] < STANDING_Y + 1.0, "sank to the ground: {:?}", position(&game, id));
    // Forming, it is still dead: nothing can hurt it, and it cannot act.
    assert!(!game.world.entities.alive(id));
    let left = ghost(&game.world, id).unwrap().forming;
    assert!(left + 3 >= GHOST_FORM_TICKS, "{left}");
    hold(&mut game, id, InputFrame { forward: 127, ..Default::default() }, left - 1);
    assert!(!game.world.entities.alive(id));
    hold(&mut game, id, InputFrame::default(), 1);
    assert!(game.world.entities.alive(id));
    assert!(ghost(&game.world, id).is_none());
    assert_eq!(game.world.entities.get::<Health>(id).map(|health| health.hp), Some(PLAYER_MAX_HP));
    let player = game.world.entities.of::<Player>(id);
    assert!(!player.character.ghost && !player.character.chute);
    let [x, _, z] = player.character.position;
    assert!((x - 10.0).abs() < 1.0 && (z - 10.0).abs() < 1.0, "where the ghost was: {x} {z}");
}

#[test]
fn a_ghost_cannot_be_hurt() {
    let mut game = game();
    let id = join(&mut game);
    haunting(&mut game, id, [10.0, 10.0]);
    kill(&mut game, id);
    assert!(ghost(&game.world, id).is_some_and(|ghost| ghost.age > GHOST_MIN_TICKS), "untouched");
}

#[test]
fn a_ghost_whose_time_runs_out_where_it_may_not_stand_drops_at_an_entry_point() {
    let mut game = game();
    let id = join(&mut game);
    haunting(&mut game, id, [-50.0, -50.0]);
    hold(&mut game, id, InputFrame::default(), GHOST_TICKS);
    assert!(ghost(&game.world, id).is_none());
    assert!(game.world.entities.alive(id));
    let character = game.world.entities.of::<Player>(id).character;
    assert!(character.chute && (character.position[0] + 80.0).abs() < 1.0, "{:?}", character.position);
}

#[test]
fn a_ghost_whose_time_runs_out_where_it_may_stand_takes_shape_there() {
    let mut game = game();
    let id = join(&mut game);
    haunting(&mut game, id, [10.0, 10.0]);
    let age = ghost(&game.world, id).unwrap().age;
    hold(&mut game, id, InputFrame::default(), GHOST_TICKS - age - 1);
    assert_eq!(shown(&game.world, id).map(|(stance, _, _)| stance), Some(STANCE_GHOST));
    hold(&mut game, id, InputFrame::default(), 1);
    assert_eq!(shown(&game.world, id).map(|(stance, _, _)| stance), Some(STANCE_FORM));
}

#[test]
fn the_chute_button_gives_up_the_ghost_for_a_parachute_drop() {
    let mut game = game();
    let id = join(&mut game);
    stand(&mut game, id, [10.0, 10.0]);
    game.world.entities.of_mut::<Player>(id).hands.hotbar[0].ammo = 3;
    kill(&mut game, id);
    hold(&mut game, id, InputFrame::default(), RESPAWN_TICKS + 2);
    press(&mut game, id, BUTTON_CHUTE);
    assert!(ghost(&game.world, id).is_none());
    assert!(game.world.entities.alive(id));
    let character = game.world.entities.of::<Player>(id).character;
    assert!(character.chute && (character.position[0] + 80.0).abs() < 1.0, "{:?}", character.position);
    let player = game.world.entities.of::<Player>(id);
    assert_eq!(player.hands.hotbar, player.hands.loadout, "a fresh loadout");
}

#[test]
fn joining_still_drops_by_parachute() {
    let mut game = game();
    let id = join(&mut game);
    assert!(ghost(&game.world, id).is_none());
    assert!(game.world.entities.of::<Player>(id).character.chute);
}
