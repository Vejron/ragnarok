use super::*;
use crate::game::{Command, Game, MAX_BOTS, Profile, Role, TEAM_BOTS, TEAM_NONE, Temper};
use crate::mechanics::life::{Harm, damage_entity};
use shared::protocol::{ItemSlot, KIND_SOLDIER};
use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A flat 200 m square. `wall` adds a long 10 m tall wall across x = 10..12 m, z = -30..30 m.
fn game(wall: bool) -> Game {
    let mut map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[-5_000, -5_000]], ..Default::default() };
    if wall {
        let v = |x: i32, y: i32, z: i32| [x * 100, y * 100, z * 100];
        let vertices = vec![
            v(10, 0, -30), v(12, 0, -30), v(12, 0, 30), v(10, 0, 30),
            v(10, 10, -30), v(12, 10, -30), v(12, 10, 30), v(10, 10, 30),
        ];
        let triangles = vec![
            [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
            [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
        ];
        map.collision = CollisionMesh { vertices, triangles };
    }
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

fn feet(game: &Game, id: u32) -> [f32; 3] {
    game.world.entities.of::<Player>(id).character.position
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

#[test]
fn a_bot_is_a_player_on_the_bot_team_seen_as_a_soldier_at_home() {
    let mut game = game(false);
    let id = game.add_bot([20.0, -10.0], 0, vec![]).unwrap();
    let player = &game.world.entities.of::<Player>(id);
    assert_eq!(player.team, TEAM_BOTS, "a bot is never on no team");
    let [x, _, z] = player.character.position;
    assert!((x - 20.0).abs() < 0.01 && (z + 10.0).abs() < 0.01, "spawned at {x}, {z}");
    assert!(game.seen_people(KIND_SOLDIER).iter().any(|(seen, _)| *seen == id));
}

#[test]
fn bots_have_their_own_room_and_never_keep_people_out() {
    let mut game = game(false);
    for n in 0..MAX_BOTS {
        assert!(game.add_bot([n as f32 - 32.0, 20.0], TEAM_BOTS, vec![]).is_some());
    }
    assert!(game.add_bot([0.0, 0.0], TEAM_BOTS, vec![]).is_none(), "no more than MAX_BOTS");
    for _ in 0..MAX_PLAYERS {
        join(&mut game);
    }
    let (outbox, _inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    assert_eq!(response.try_recv().unwrap(), None, "no more than MAX_PLAYERS people");
    assert_eq!(game.world.entities.count::<Player>(), MAX_BOTS + MAX_PLAYERS);
}

#[test]
fn it_walks_its_route_round_and_round() {
    let mut game = game(false);
    let id = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0], [12.0, 0.0], [12.0, 12.0]]).unwrap();
    let mut visited = [false; 3];
    for _ in 0..(40 * TICK_RATE) {
        game.step();
        let next = game.world.entities.of::<Bot>(id).next;
        visited[next] = true;
        assert_eq!(game.world.entities.of::<Bot>(id).mode, BotMode::Patrol);
    }
    assert_eq!(visited, [true; 3], "went to every waypoint");
    let [x, _, z] = feet(&game, id);
    assert!(x > -2.0 && x < 14.0 && z > -2.0 && z < 14.0, "stays on its route: {x}, {z}");
}

#[test]
fn it_makes_up_a_route_near_home_when_given_none() {
    let mut game = game(false);
    let id = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![]).unwrap();
    game.step();
    let route = &game.world.entities.of::<Bot>(id).route;
    assert!(route.len() > 1, "route {route:?}");
    assert!(route.iter().all(|p| p[0].hypot(p[1]) <= ROUTE_RADIUS + 0.01));
}

/// Takes every item off player `id`.
fn disarm(game: &mut Game, id: u32) {
    game.world.entities.get_mut::<Player>(id).unwrap().hands.hotbar = Default::default();
}

/// A bot guarding `[0, 0]`, facing -Z (yaw 0), as good a shot as `skill`.
fn guard(game: &mut Game, skill: f32) -> u32 {
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    let mut mind = game.world.entities.get_mut::<Bot>(bot).unwrap();
    mind.aim.skill = skill;
    // Tempers are rolled from the clock: these tests are about what every bot does.
    mind.temper = Temper::EVEN;
    bot
}

fn rounds_left(game: &Game, id: u32) -> u8 {
    game.world.entities.of::<Player>(id).held().ammo
}

#[test]
fn an_unarmed_bot_spots_someone_in_front_and_runs_at_them() {
    let mut game = game(false);
    // Facing -Z (yaw 0), guarding its spot.
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    disarm(&mut game, bot);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -30.0]);
    run(&mut game, 0.5);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue);
    assert_eq!(game.world.entities.of::<Bot>(bot).sight.target, Some(human));
    run(&mut game, 5.0);
    let gap = distance_2d(feet(&game, bot), feet(&game, human));
    assert!(gap < CLOSE_IN + 0.5, "closed in to {gap} m");
}

#[test]
fn a_calm_bot_does_not_see_behind_it_or_through_walls() {
    let mut game = game(true);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    let behind = join(&mut game);
    put(&mut game, behind, [0.0, 30.0]);
    let walled = join(&mut game);
    // East of the wall, which stands between them.
    put(&mut game, walled, [30.0, 0.0]);
    game.world.entities.get_mut::<Bot>(bot).unwrap().yaw = -PI / 2.0; // Facing +X, at the wall.
    run(&mut game, 1.0);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Patrol, "saw {:?}", game.world.entities.of::<Bot>(bot).sight.target);
}

#[test]
fn it_goes_to_where_it_lost_them_looks_round_and_goes_back_to_its_route() {
    let mut game = game(true);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    let human = join(&mut game);
    put(&mut game, human, [0.0, -20.0]);
    run(&mut game, 0.5);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue);
    // They slip behind the wall.
    put(&mut game, human, [40.0, 0.0]);
    run(&mut game, 6.0);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Search);
    let near_last_seen = distance_2d(feet(&game, bot), [0.0, 0.0, -20.0]);
    assert!(near_last_seen < 3.0, "{near_last_seen} m from where they were seen");
    run(&mut game, 8.0);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Patrol);
}

#[test]
fn bots_ignore_their_own_team_and_cannot_hurt_it() {
    let mut game = game(false);
    let a = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    let b = game.add_bot([0.0, -10.0], TEAM_BOTS, vec![[0.0, -10.0]]).unwrap();
    run(&mut game, 1.0);
    assert_eq!(game.world.entities.of::<Bot>(a).mode, BotMode::Patrol);
    damage_entity(&mut game.world, b, Harm::new(50, Some(a), 0));
    assert_eq!(game.world.entities.of::<crate::entities::Health>(b).hp, PLAYER_MAX_HP);
    // People are on no team: they still hurt each other, and bots.
    let human = join(&mut game);
    let other = join(&mut game);
    assert_eq!(game.world.entities.of::<Player>(human).team, TEAM_NONE);
    damage_entity(&mut game.world, other, Harm::new(10, Some(human), 0));
    damage_entity(&mut game.world, b, Harm::new(10, Some(human), 0));
    assert_eq!(game.world.entities.of::<crate::entities::Health>(other).hp, PLAYER_MAX_HP - 10);
    assert_eq!(game.world.entities.of::<crate::entities::Health>(b).hp, PLAYER_MAX_HP - 10);
}

#[test]
fn a_bot_on_another_team_is_fair_game() {
    let mut game = game(false);
    let red = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    let blue = game.add_bot([0.0, -20.0], TEAM_BOTS + 1, vec![[0.0, -20.0]]).unwrap();
    run(&mut game, 0.5);
    assert_eq!(game.world.entities.of::<Bot>(red).sight.target, Some(blue));
}

#[test]
fn a_killed_bot_comes_back_at_home_on_patrol() {
    let mut game = game(false);
    let bot = game.add_bot([5.0, 5.0], TEAM_BOTS, vec![[5.0, 5.0], [15.0, 5.0]]).unwrap();
    let human = join(&mut game);
    put(&mut game, human, [5.0, -10.0]);
    run(&mut game, 0.5);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue);
    put(&mut game, human, [-80.0, 80.0]);
    damage_entity(&mut game.world, bot, Harm::new(PLAYER_MAX_HP, Some(human), 0));
    run(&mut game, RESPAWN_TICKS as f32 / TICK_RATE as f32 + 0.2);
    assert!(game.world.entities.alive(bot));
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Patrol);
    assert!(distance_2d(feet(&game, bot), [5.0, 0.0, 5.0]) < 2.0, "back home");
}


#[test]
fn it_shoots_someone_it_sees_dead() {
    let mut game = game(false);
    let bot = guard(&mut game, 1.0);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -25.0]);
    run(&mut game, 6.0);
    assert!(game.world.entities.of::<Player>(human).score.deaths >= 1, "still on {} hp", game.world.entities.of::<crate::entities::Health>(human).hp);
    assert_eq!(game.world.entities.of::<Player>(bot).score.kills, game.world.entities.of::<Player>(human).score.deaths);
}

#[test]
fn it_takes_a_moment_to_react() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.0);
    game.step();
    let full = rounds_left(&game, bot);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -25.0]);
    // Not fired on for REACT_POOR after it has seen them, and it takes that in over a moment too.
    run(&mut game, REACT_POOR - 0.15);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue);
    assert_eq!(rounds_left(&game, bot), full, "fired before it had reacted");
    run(&mut game, 2.5);
    assert!(rounds_left(&game, bot) < full, "never fired");
}

#[test]
fn a_better_shot_hits_more() {
    // Ten seconds against someone who cannot die, running back and forth across its front at 25 m.
    let damage_done = |skill: f32, seed: u64| {
        let mut game = game(false);
        game.world.rng = crate::rng::Rng::from_seed(seed);
        let bot = guard(&mut game, skill);
        let human = join(&mut game);
        let mut dealt = 0u32;
        for tick in 0..10 * TICK_RATE {
            let t = tick as f32 / TICK_RATE as f32;
            // A triangle wave between x = -10 and 10 at 4 m/s.
            let x = 10.0 - ((t * 4.0) % 40.0 - 20.0).abs();
            put(&mut game, human, [x, -25.0]);
            game.step();
            let hp = game.world.entities.of::<crate::entities::Health>(human).hp;
            dealt += u32::from(PLAYER_MAX_HP - hp);
            game.world.entities.get_mut::<crate::entities::Health>(human).unwrap().hp = PLAYER_MAX_HP;
        }
        let _ = bot;
        dealt
    };
    let mut hit_at_all = 0;
    let mut hit_well = 0;
    for seed in 1..=8 {
        hit_at_all += damage_done(0.0, seed);
        hit_well += damage_done(1.0, seed);
    }
    assert!(hit_well > hit_at_all * 2, "skill 1 did {hit_well}, skill 0 did {hit_at_all}");
    assert!(hit_at_all > 0, "skill 0 never hit at all");
}

#[test]
fn it_leads_a_moving_target_and_aims_over_the_drop() {
    let gun = shared::items::def(shared::items::ITEM_AK5).unwrap();
    let eye = [0.0, 1.0, 0.0];
    let still = aim_point(eye, [0.0, 0.7, -140.0], [0.0; 3], gun, 1.0);
    // Half a second's flight: a quarter of `GRAVITY * BULLET_GRAVITY_SCALE` metres of drop.
    let flight = 140.0 / BULLET_SPEED;
    assert!((still[1] - (0.7 + CHEST) - 0.5 * GRAVITY * BULLET_GRAVITY_SCALE * flight * flight).abs() < 0.02, "{still:?}");
    let running = aim_point(eye, [0.0, 0.7, -140.0], [5.0, 0.0, 0.0], gun, 1.0);
    assert!((running[0] - 5.0 * flight).abs() < 0.02, "{running:?}");
    let half = aim_point(eye, [0.0, 0.7, -140.0], [5.0, 0.0, 0.0], gun, 0.5);
    assert!((half[0] - 2.5 * flight).abs() < 0.02, "{half:?}");
    let shotgun = shared::items::def(shared::items::ITEM_SHOTGUN).unwrap();
    assert_eq!(aim_point(eye, [0.0, 0.7, -10.0], [5.0, 0.0, 0.0], shotgun, 1.0), [0.0, 0.7 + CHEST, -10.0], "a hitscan round needs no lead");
}

#[test]
fn it_takes_the_shotgun_up_close_and_a_rifle_further_out() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.5);
    // The standard loadout has no shotgun: one is carried in the last slot.
    game.world.entities.get_mut::<Player>(bot).unwrap().hands.hotbar[4] = ItemSlot { item: shared::items::ITEM_SHOTGUN, count: 1, ammo: shared::items::def(shared::items::ITEM_SHOTGUN).unwrap().mag };
    let human = join(&mut game);
    put(&mut game, human, [0.0, -6.0]);
    run(&mut game, 0.5);
    assert_eq!(game.world.entities.of::<Player>(bot).held().item, shared::items::ITEM_SHOTGUN);
    put(&mut game, human, [0.0, -40.0]);
    run(&mut game, 0.5);
    assert_eq!(game.world.entities.of::<Player>(bot).held().item, shared::items::ITEM_AK5);
}

#[test]
fn it_stands_off_at_the_distance_its_gun_is_good_at() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.5);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -45.0]);
    run(&mut game, 0.3);
    // Someone who cannot die, standing still.
    for _ in 0..8 * TICK_RATE {
        game.world.entities.get_mut::<crate::entities::Health>(human).unwrap().hp = PLAYER_MAX_HP;
        game.step();
    }
    let gap = distance_2d(feet(&game, bot), feet(&game, human));
    assert!(gap > 10.0 && gap < 20.0, "stood off at {gap} m");
}

#[test]
fn it_reloads_when_empty_and_tops_up_between_fights() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.5);
    game.step();
    let slot = game.world.entities.of::<Player>(bot).hands.selected;
    let mag = shared::items::def(game.world.entities.of::<Player>(bot).held().item).unwrap().mag;
    game.world.entities.get_mut::<Player>(bot).unwrap().hands.hotbar[slot].ammo = 3;
    run(&mut game, 4.0);
    assert_eq!(rounds_left(&game, bot), mag, "topped up with no one about");
    let human = join(&mut game);
    put(&mut game, human, [0.0, -25.0]);
    game.world.entities.get_mut::<Player>(bot).unwrap().hands.hotbar[slot].ammo = 0;
    run(&mut game, 0.5);
    assert!(game.world.entities.of::<Player>(bot).hands.reload_left > 0, "reloading in the middle of a fight");
}

#[test]
fn it_shoots_from_its_eye_along_its_look() {
    let mut game = game(false);
    let bot = guard(&mut game, 1.0);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -25.0]);
    let spawn = (0..TICK_RATE)
        .find_map(|_| {
            game.step();
            game.world.bullets.iter().find(|b| b.shooter == bot).copied()
        })
        .expect("fired");
    let eye = feet(&game, bot)[1] + EYE_HEIGHT;
    assert!((shared::quant::from_cm(spawn.y) - eye).abs() < 0.3, "from {} m, eye at {eye} m", shared::quant::from_cm(spawn.y));
    assert!(spawn.vx.unsigned_abs() < spawn.vz.unsigned_abs() / 10, "straight down its look: {spawn:?}");
}

#[test]
fn it_walks_round_a_wall_across_its_route() {
    let mut game = game(true);
    // Either side of the wall at x = 10..12, which runs from z = -30 to 30.
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0], [25.0, 0.0]]).unwrap();
    let mut furthest: f32 = 0.0;
    for _ in 0..(60 * TICK_RATE) {
        game.step();
        let [x, _, z] = feet(&game, bot);
        assert!(!(x > 9.5 && x < 12.5 && z.abs() < 29.5), "in the wall at {x}, {z}");
        furthest = furthest.max(x);
        if distance_2d(feet(&game, bot), [25.0, 0.0, 0.0]) < REACHED {
            return;
        }
    }
    panic!("never got round: furthest x {furthest}, at {:?}", feet(&game, bot));
}

#[test]
fn it_chases_round_a_wall_to_where_it_last_saw_someone() {
    let mut game = game(true);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap();
    let human = join(&mut game);
    put(&mut game, human, [80.0, -80.0]);
    game.step();
    // It last saw them just behind the wall, and has lost them.
    let y = feet(&game, bot)[1];
    let mut mind = game.world.entities.get_mut::<Bot>(bot).unwrap();
    mind.mode = BotMode::Pursue;
    mind.sight.target = Some(human);
    mind.sight.last_seen = [14.0, y, 0.0];
    for _ in 0..(25 * TICK_RATE) {
        game.step();
        if game.world.entities.of::<Bot>(bot).mode != BotMode::Pursue {
            assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Search, "gave up the chase");
            let gap = distance_2d(feet(&game, bot), [14.0, 0.0, 0.0]);
            assert!(gap < 2.0, "searching {gap} m from where they were last seen, at {:?}", feet(&game, bot));
            return;
        }
    }
    panic!("never got there: at {:?}", feet(&game, bot));
}

#[test]
fn a_calm_bot_shot_from_behind_turns_round_and_fights_back() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.5);
    let human = join(&mut game);
    put(&mut game, human, [0.0, 30.0]);
    run(&mut game, 1.0);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Patrol, "it does not see behind it");
    damage_entity(&mut game.world, bot, Harm::new(10, Some(human), 0));
    run(&mut game, 1.0);
    let mind = &game.world.entities.of::<Bot>(bot);
    assert_eq!((mind.mode, mind.sight.target, mind.sight.sees), (BotMode::Pursue, Some(human), true));
}

#[test]
fn it_goes_to_look_where_it_heard_a_gunshot_and_searches_there() {
    let mut game = game(true);
    let bot = guard(&mut game, 0.5);
    disarm(&mut game, bot);
    // Behind the wall, out of sight: only heard.
    let shot = [25.0, sim::STANDING_Y, 0.0];
    make_noise(&mut game.world, shot, None, HEAR_SHOT);
    game.step();
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue);
    assert_eq!(game.world.entities.of::<Bot>(bot).sight.target, None, "it has seen no one");
    for _ in 0..(25 * TICK_RATE) {
        game.step();
        if game.world.entities.of::<Bot>(bot).mode != BotMode::Pursue {
            assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Search);
            let gap = distance_2d(feet(&game, bot), shot);
            assert!(gap < 2.0 + 25.0 * 0.12, "searching {gap} m from the shot");
            return;
        }
    }
    panic!("never got there: at {:?}", feet(&game, bot));
}

#[test]
fn a_blast_carries_further_than_a_gunshot_and_its_own_shots_are_not_news() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.5);
    let far = [0.0, sim::STANDING_Y, 120.0];
    make_noise(&mut game.world, far, None, HEAR_SHOT);
    make_noise(&mut game.world, [0.0, 0.0, 0.0], Some(bot), HEAR_SHOT);
    game.step();
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Patrol);
    crate::mechanics::combat::blast(
        &mut game.world,
        crate::mechanics::combat::Blast { point: far, radius: 1.0, damage: 0.0, knock: 0.0, dir: [0.0, 0.0, 1.0], attacker: None, weapon: 0 },
    );
    game.step();
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue);
    let rough = 0.13 * distance_2d(feet(&game, bot), far);
    assert!(distance_2d(game.world.entities.of::<Bot>(bot).sight.last_seen, far) < rough);
}

#[test]
fn a_bot_in_a_fight_is_not_drawn_off_by_noises() {
    let mut game = game(false);
    let bot = guard(&mut game, 0.5);
    disarm(&mut game, bot);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -20.0]);
    run(&mut game, 0.5);
    assert!(game.world.entities.of::<Bot>(bot).sight.sees);
    for _ in 0..TICK_RATE {
        make_noise(&mut game.world, [0.0, sim::STANDING_Y, 30.0], None, HEAR_BLAST);
        game.step();
    }
    let mind = &game.world.entities.of::<Bot>(bot);
    assert_eq!((mind.sight.target, mind.sight.sees), (Some(human), true));
    assert!(distance_2d(mind.sight.last_seen, feet(&game, human)) < 0.5);
}

#[test]
fn a_teammate_firing_behind_a_wall_brings_it_running() {
    let mut game = game(true);
    // One bot fights someone in front of it; another, past the wall and facing away, cannot see either.
    let fighter = guard(&mut game, 1.0);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -25.0]);
    let helper = game.add_bot([30.0, 0.0], TEAM_BOTS, vec![[30.0, 0.0]]).unwrap();
    game.world.entities.get_mut::<Bot>(helper).unwrap().yaw = -PI / 2.0;
    let mut stood = Vec::new();
    for _ in 0..(2 * TICK_RATE) {
        stood.push(feet(&game, fighter));
        game.step();
    }
    let mind = &game.world.entities.of::<Bot>(helper);
    assert_eq!(mind.mode, BotMode::Pursue, "heard nothing");
    assert!(stood.iter().any(|&at| distance_2d(mind.sight.last_seen, at) < 5.0), "went to look at {:?}, not where the fighter fired from", mind.sight.last_seen);
}

#[test]
fn a_bot_sent_far_from_home_fights_at_the_end_of_its_route() {
    let mut game = game(false);
    // Home at one side, the one waypoint 170 m off at the other: an attacker's route.
    let bot = game.add_bot([-90.0, 0.0], TEAM_BOTS, vec![[80.0, 0.0]]).unwrap();
    game.world.entities.get_mut::<Bot>(bot).unwrap().aim.skill = 1.0;
    put(&mut game, bot, [80.0, 0.0]);
    let human = join(&mut game);
    put(&mut game, human, [80.0, -25.0]);
    run(&mut game, 6.0);
    assert!(game.world.entities.of::<Player>(human).score.deaths >= 1, "still on {} hp", game.world.entities.of::<crate::entities::Health>(human).hp);
}

#[test]
fn others_close_by_push_a_bot_away_and_far_ones_do_not() {
    let crowd = [(1, [0.0, 0.0]), (2, [1.0, 0.0]), (3, [50.0, 0.0])];
    let [x, z] = push(1, [0.0, 0.0, 0.0], &crowd);
    assert!(x < -0.5 && z.abs() < 1e-6, "pushed west by the one at 1 m east: {x}, {z}");
    assert_eq!(push(3, [50.0, 0.0, 0.0], &crowd), [0.0, 0.0], "nobody near");
    // Two on one spot are pushed in opposite directions.
    let same = [(1, [5.0, 5.0]), (2, [5.0, 5.0])];
    let (a, b) = (push(1, [5.0, 0.0, 5.0], &same), push(2, [5.0, 0.0, 5.0], &same));
    assert!(a[0].hypot(a[1]) > 0.9 && (a[0] + b[0]).abs() < 1e-6 && (a[1] + b[1]).abs() < 1e-6, "{a:?} {b:?}");
    // However many crowd round, the push is never more than 1.
    let many: Vec<(u32, [f32; 2])> = (0..9).map(|i| (10 + i, [0.5, 0.0])).collect();
    let [x, z] = push(1, [0.0, 0.0, 0.0], &many);
    assert!(x.hypot(z) <= 1.0 + 1e-6);
}

/// The smallest gap between any two of `ids`.
fn closest(game: &Game, ids: &[u32]) -> f32 {
    let mut least = f32::MAX;
    for (n, &a) in ids.iter().enumerate() {
        for &b in &ids[n + 1..] {
            least = least.min(distance_2d(feet(game, a), feet(game, b)));
        }
    }
    least
}

#[test]
fn bots_that_start_on_one_spot_spread_out_instead_of_walking_as_one() {
    let mut game = game(false);
    let route = vec![[0.0, 0.0], [30.0, 0.0], [30.0, 30.0]];
    let ids: Vec<u32> = (0..3).map(|_| game.add_bot([0.0, 0.0], TEAM_BOTS, route.clone()).unwrap()).collect();
    run(&mut game, 8.0);
    let gap = closest(&game, &ids);
    assert!(gap > 1.0, "the closest two are {gap} m apart");
}

#[test]
fn a_squad_fighting_one_target_does_not_stack_up_on_one_line() {
    let mut game = game(false);
    let ids: Vec<u32> = (0..3).map(|_| game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0]]).unwrap()).collect();
    let human = join(&mut game);
    put(&mut game, human, [0.0, -40.0]);
    for _ in 0..10 * TICK_RATE {
        game.world.entities.get_mut::<crate::entities::Health>(human).unwrap().hp = PLAYER_MAX_HP;
        game.step();
    }
    let gap = closest(&game, &ids);
    assert!(gap > 1.5, "the closest two are {gap} m apart");
}

/// A bot of temper `temper` at the origin, and someone 20 m off that it has seen.
fn in_a_fight(wall: bool, temper: Temper) -> (Game, u32, u32) {
    let mut game = game(wall);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![]).unwrap();
    game.world.entities.get_mut::<Bot>(bot).unwrap().temper = temper;
    let human = join(&mut game);
    put(&mut game, human, [0.0, -20.0]);
    run(&mut game, 0.5);
    (game, bot, human)
}

fn shoot_at(game: &mut Game, victim: u32, attacker: u32) {
    let harm = Harm::new(10, Some(attacker), 0);
    damage_entity(&mut game.world, victim, harm);
    game.step();
}

#[test]
fn a_round_passing_close_shakes_it_but_one_going_elsewhere_does_not() {
    let (mut game, bot, human) = in_a_fight(false, Temper::EVEN);
    let at = feet(&game, bot);
    let eye = [at[0], at[1], at[2] - 20.0];
    make_shot(&mut game.world, eye, Some(human), [1.0, 0.0, 0.0]);
    game.step();
    assert!(game.world.entities.of::<Bot>(bot).nerve >= 0.99, "wide of it");
    make_shot(&mut game.world, eye, Some(human), [0.0, 0.0, 1.0]);
    game.step();
    assert!(game.world.entities.of::<Bot>(bot).nerve < 0.95, "right past it");
}

#[test]
fn a_teammates_round_does_not_shake_it() {
    let (mut game, bot, _) = in_a_fight(false, Temper::EVEN);
    let friend = game.add_bot([0.0, -40.0], TEAM_BOTS, vec![]).unwrap();
    make_shot(&mut game.world, [0.0, 1.0, -20.0], Some(friend), [0.0, 0.0, 1.0]);
    game.step();
    assert!(game.world.entities.of::<Bot>(bot).nerve >= 0.99);
}

#[test]
fn nerve_comes_back_with_time() {
    let (mut game, bot, human) = in_a_fight(false, Temper::EVEN);
    put(&mut game, human, [0.0, 90.0]);
    game.world.entities.get_mut::<Bot>(bot).unwrap().nerve = 0.5;
    run(&mut game, 2.0);
    assert!(game.world.entities.of::<Bot>(bot).nerve > 0.55);
}

#[test]
fn broken_with_no_cover_it_stops_and_goes_flat() {
    let (mut game, bot, human) = in_a_fight(false, Temper::EVEN);
    put(&mut game, human, [0.0, -50.0]);
    game.world.entities.get_mut::<Bot>(bot).unwrap().nerve = 0.1;
    let start = feet(&game, bot);
    run(&mut game, 0.5);
    assert!(game.world.entities.of::<Player>(bot).character.prone, "flat on open ground");
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Pursue, "nowhere to go");
    assert!(distance_2d(start, feet(&game, bot)) < 1.5, "it does not advance");
}

#[test]
fn a_bot_with_no_fear_never_breaks() {
    let (mut game, bot, _) = in_a_fight(false, Temper { courage: 1.0, aggression: 0.5, patience: 0.5 });
    game.world.entities.get_mut::<Bot>(bot).unwrap().nerve = 0.0;
    game.step();
    let mind = &game.world.entities.of::<Bot>(bot);
    assert!(!nerve::broken(mind) && !nerve::frozen(mind));
}

#[test]
fn the_same_fire_breaks_a_coward_and_not_a_brave_soldier() {
    let shaken = |courage: f32| {
        let mut bot = Bot::new([0.0; 2], vec![]);
        bot.temper.courage = courage;
        for _ in 0..3 {
            nerve::shake(&mut bot, 0.2);
        }
        nerve::broken(&bot)
    };
    assert!(shaken(0.0) && !shaken(0.8));
}

/// A broken bot of full patience at `[x, z]`, with one round left, and someone unarmed at `threat`.
fn broken_bot(game: &mut Game, at: [f32; 2], threat: [f32; 2]) -> u32 {
    let bot = game.add_bot(at, TEAM_BOTS, vec![]).unwrap();
    game.world.entities.get_mut::<Bot>(bot).unwrap().temper = Temper { courage: 0.0, aggression: 0.5, patience: 1.0 };
    let human = join(game);
    put(game, human, threat);
    game.world.entities.get_mut::<Player>(human).unwrap().hands.hotbar = Default::default();
    // Calm, it sees only what is in front of it.
    game.world.entities.get_mut::<Bot>(bot).unwrap().yaw = heading([at[0], 0.0, at[1]], [threat[0], 0.0, threat[1]]);
    run(game, 0.7);
    assert!(game.world.entities.of::<Bot>(bot).sight.sees, "it has seen them");
    game.world.entities.get_mut::<Bot>(bot).unwrap().nerve = 0.1;
    let slot = game.world.entities.of::<Bot>(bot).slot as usize;
    game.world.entities.get_mut::<Player>(bot).unwrap().hands.hotbar[slot].ammo = 1;
    bot
}

#[test]
fn broken_by_the_end_of_a_wall_it_gets_behind_it_and_reloads_there() {
    // The wall runs across x = 10..12 and ends at z = 30, a few steps away.
    for seed in 1..=8 {
        let mut game = game(true);
        game.world.rng = crate::rng::Rng::from_seed(seed);
        let bot = broken_bot(&mut game, [8.0, 27.0], [-15.0, 27.0]);
        let mut hidden = false;
        for _ in 0..(8 * TICK_RATE) {
            game.step();
            let mind = game.world.entities.of::<Bot>(bot);
            hidden |= mind.mode == BotMode::Cover && feet(&game, bot)[0] > 12.0 && feet(&game, bot)[2] < 30.0;
            if hidden {
                break;
            }
        }
        assert!(hidden, "seed {seed}: ended at {:?}, {:?}", feet(&game, bot), game.world.entities.of::<Bot>(bot).mode);
        run(&mut game, 3.0);
        assert!(game.world.entities.of::<Player>(bot).held().ammo > 1, "seed {seed}: reloaded behind the wall");
    }
}

#[test]
fn the_far_side_of_a_wall_too_long_to_get_round_is_no_cover() {
    for seed in 1..=8 {
        let mut game = game(true);
        game.world.rng = crate::rng::Rng::from_seed(seed);
        let bot = broken_bot(&mut game, [8.0, 0.0], [-10.0, 0.0]);
        run(&mut game, 2.0);
        assert_ne!(game.world.entities.of::<Bot>(bot).mode, BotMode::Cover, "seed {seed}");
        assert!(game.world.entities.of::<Player>(bot).character.prone, "seed {seed}: flat where it is");
    }
}

#[test]
fn it_comes_out_of_cover_once_steady() {
    let mut game = game(false);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![]).unwrap();
    {
        let mut mind = game.world.entities.get_mut::<Bot>(bot).unwrap();
        mind.mode = BotMode::Cover;
        mind.mode_left = 1000;
        mind.cover = Some([0.0, 0.0, 0.0]);
        mind.sight.last_seen = [20.0, 1.0, 0.0];
    }
    run(&mut game, 0.2);
    let mind = game.world.entities.of::<Bot>(bot);
    assert_eq!(mind.mode, BotMode::Pursue);
    assert!(mind.cover.is_none());
    assert!((mind.sight.last_seen[2]).abs() >= 3.0, "comes out to one side: {:?}", mind.sight.last_seen);
}

#[test]
fn its_first_rounds_go_wide_and_walk_in() {
    let (mut game, bot, human) = in_a_fight(false, Temper { courage: 1.0, aggression: 0.0, patience: 0.5 });
    game.world.entities.get_mut::<Bot>(bot).unwrap().aim.skill = 1.0;
    let off = |game: &Game| {
        let mind = game.world.entities.of::<Bot>(bot);
        (shared::quant::wrap_angle(mind.yaw - heading(feet(game, bot), feet(game, human)))).abs()
    };
    {
        let mut mind = game.world.entities.get_mut::<Bot>(bot).unwrap();
        mind.aim.side = [1.0, 0.0];
        mind.aim.tracked = 0;
    }
    run(&mut game, 0.2);
    let early = off(&game);
    run(&mut game, 3.0);
    let late = off(&game);
    assert!(early > 0.03, "wide at first: {early}");
    assert!(late < 0.02, "on them later: {late}");
}

#[test]
fn it_dies_calm_of_mind() {
    let (mut game, bot, human) = in_a_fight(false, Temper::EVEN);
    game.world.entities.get_mut::<Bot>(bot).unwrap().nerve = 0.1;
    shoot_at(&mut game, bot, human);
    game.world.entities.get_mut::<Bot>(bot).unwrap().cover = Some([1.0, 0.0, 1.0]);
    damage_entity(&mut game.world, bot, Harm::new(250, Some(human), 0));
    game.step();
    game.step();
    let mind = game.world.entities.of::<Bot>(bot);
    assert_eq!((mind.nerve, mind.cover), (1.0, None));
}

#[test]
fn tempers_are_rolled_and_some_are_fanatics() {
    let mut game = game(false);
    let ids: Vec<u32> = (0..MAX_BOTS).filter_map(|_| game.add_bot([0.0, 0.0], TEAM_BOTS, vec![])).collect();
    let tempers: Vec<Temper> = ids.iter().map(|&id| game.world.entities.of::<Bot>(id).temper).collect();
    assert!(tempers.iter().any(|t| t.courage < 0.3) && tempers.iter().any(|t| t.courage > 0.6));
    assert!(tempers.iter().all(|t| t.aggression >= 0.0 && t.aggression < 1.0));
}

#[test]
fn it_can_hit_someone_lying_down() {
    let mut game = game(false);
    game.world.rng = crate::rng::Rng::from_seed(5);
    let bot = guard(&mut game, 1.0);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -20.0]);
    let mut struck = 0;
    for seq in 1..=6 * TICK_RATE {
        game.handle(Command::Input(human, 0, vec![InputFrame { seq, buttons: BUTTON_PRONE, ..Default::default() }]));
        game.step();
        let health = game.world.entities.of::<crate::entities::Health>(human).hp;
        struck = struck.max(u32::from(PLAYER_MAX_HP - health));
        game.world.entities.get_mut::<crate::entities::Health>(human).unwrap().hp = PLAYER_MAX_HP;
    }
    assert!(game.world.entities.of::<Player>(human).character.prone && game.world.entities.of::<Bot>(bot).sight.sees);
    assert!(struck > 0, "never hit them");
}

/// `count` steady bots of skill 0 in a line 30 m from someone unarmed at the origin, all facing them.
fn crowd_on(game: &mut Game, count: usize) -> (u32, Vec<u32>) {
    game.world.rng = crate::rng::Rng::from_seed(9);
    let human = join(game);
    put(game, human, [0.0, 0.0]);
    game.world.entities.get_mut::<Player>(human).unwrap().hands.hotbar = Default::default();
    let bots: Vec<u32> = (0..count)
        .map(|n| {
            let id = guard(game, 0.0);
            put(game, id, [(n as f32 - count as f32 / 2.0) * 4.0, -30.0]);
            let mut mind = game.world.entities.get_mut::<Bot>(id).unwrap();
            mind.home = [(n as f32 - count as f32 / 2.0) * 4.0, -30.0];
            mind.route = vec![mind.home];
            mind.yaw = std::f32::consts::PI;
            id
        })
        .collect();
    (human, bots)
}

fn keep_alive(game: &mut Game, human: u32, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.world.entities.get_mut::<crate::entities::Health>(human).unwrap().hp = PLAYER_MAX_HP;
        game.step();
    }
}

fn with_role(game: &Game, bots: &[u32], role: Role) -> Vec<u32> {
    bots.iter().copied().filter(|&id| game.world.entities.of::<Bot>(id).role == role).collect()
}

#[test]
fn only_a_few_of_a_crowd_attack_and_the_rest_keep_busy() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 8);
    keep_alive(&mut game, human, 2.0);
    let attackers = with_role(&game, &bots, Role::Attacker);
    let distant = attackers.iter().filter(|&&id| distance_2d(feet(&game, id), feet(&game, human)) > 8.0).count();
    assert!(distant <= 2, "{distant} attackers at skill 0");
    assert_eq!(with_role(&game, &bots, Role::Flanker).len(), 2);
    assert!(!with_role(&game, &bots, Role::Suppressor).is_empty());
}

#[test]
fn a_squad_no_bigger_than_its_tokens_all_attack() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 2);
    keep_alive(&mut game, human, 2.0);
    assert_eq!(with_role(&game, &bots, Role::Attacker).len(), 2);
}

#[test]
fn an_attacker_dying_hands_its_token_on() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 8);
    keep_alive(&mut game, human, 1.5);
    let waiting = with_role(&game, &bots, Role::Suppressor);
    for id in with_role(&game, &bots, Role::Attacker) {
        damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, Some(human), 0));
    }
    keep_alive(&mut game, human, 1.0);
    assert!(waiting.iter().any(|&id| game.world.entities.of::<Bot>(id).role == Role::Attacker), "someone stepped up");
}

#[test]
fn a_flanker_gets_round_the_side_and_is_next_for_a_token() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 8);
    keep_alive(&mut game, human, 1.5);
    let flankers = with_role(&game, &bots, Role::Flanker);
    assert_eq!(flankers.len(), 2);
    keep_alive(&mut game, human, 9.0);
    let round = flankers.iter().filter(|&&id| feet(&game, id)[2] > -12.0 || game.world.entities.of::<Bot>(id).role == Role::Attacker).count();
    assert!(round > 0, "at {:?}", flankers.iter().map(|&id| feet(&game, id)).collect::<Vec<_>>());
}

#[test]
fn right_next_to_the_target_a_bot_fights_token_or_no_token() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 8);
    keep_alive(&mut game, human, 1.0);
    let waiting = with_role(&game, &bots, Role::Suppressor)[0];
    put(&mut game, waiting, [3.0, 3.0]);
    keep_alive(&mut game, human, 0.8);
    assert_eq!(game.world.entities.of::<Bot>(waiting).role, Role::Attacker);
}

#[test]
fn a_suppressor_does_little_harm_for_its_noise() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 8);
    keep_alive(&mut game, human, 1.5);
    for id in with_role(&game, &bots, Role::Attacker).into_iter().chain(with_role(&game, &bots, Role::Flanker)) {
        game.world.entities.remove::<Bot>(id);
        game.world.entities.get_mut::<Player>(id).unwrap().hands.hotbar = Default::default();
    }
    let mut taken = 0u32;
    for _ in 0..3 * TICK_RATE {
        game.world.entities.get_mut::<crate::entities::Health>(human).unwrap().hp = PLAYER_MAX_HP;
        game.step();
        taken += u32::from(PLAYER_MAX_HP - game.world.entities.of::<crate::entities::Health>(human).hp);
    }
    assert!(taken < PLAYER_MAX_HP as u32, "took {taken}");
}

/// How many seconds a calm bot at the origin, facing -Z, takes to see someone at 45 m ahead holding
/// `buttons`, at `tick`; none within 10 s.
fn time_to_see(buttons: u16, lamp: bool, tick: u32) -> Option<f32> {
    let mut game = game(false);
    game.world.tick = tick;
    let bot = guard(&mut game, 0.5);
    let human = join(&mut game);
    put(&mut game, human, [0.0, -45.0]);
    game.world.entities.get_mut::<Player>(human).unwrap().character.chute = false;
    game.world.entities.get_mut::<Player>(human).unwrap().lamp = lamp;
    (1..=10 * TICK_RATE).find_map(|step| {
        game.handle(Command::Input(human, 0, vec![InputFrame { seq: step, buttons, ..Default::default() }]));
        game.step();
        put(&mut game, human, [0.0, -45.0]);
        game.world.entities.of::<Bot>(bot).sight.sees.then_some(step as f32 / TICK_RATE as f32)
    })
}

const MIDNIGHT: u32 = (1800.0 * 0.64 * TICK_RATE as f32) as u32;

#[test]
fn it_takes_someone_in_slower_the_harder_they_are_to_see() {
    let standing = time_to_see(0, false, 0).expect("standing");
    let crouched = time_to_see(BUTTON_CROUCH, false, 0).expect("crouched");
    let lying = time_to_see(BUTTON_PRONE, false, 0).expect("lying");
    let night = time_to_see(0, false, MIDNIGHT).expect("at night");
    let lamp = time_to_see(0, true, MIDNIGHT).expect("with a lamp");
    assert!(standing < crouched && crouched < lying, "{standing} {crouched} {lying}");
    assert!(standing < night, "{standing} {night}");
    assert!(lamp < standing, "a lamp gives them away: {lamp} {standing}");
}

#[test]
fn it_is_dark_at_midnight_and_light_at_noon() {
    assert_eq!(senses::darkness(MIDNIGHT), 1.0);
    assert_eq!(senses::darkness((1800.0 * 0.14 * TICK_RATE as f32) as u32), 0.0);
}

#[test]
fn half_aware_on_patrol_it_stops_and_stares() {
    let mut game = game(false);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0], [0.0, 30.0]]).unwrap();
    {
        let mut mind = game.world.entities.get_mut::<Bot>(bot).unwrap();
        mind.next = 1;
        mind.sight.aware = 0.9;
        mind.sight.last_seen = [40.0, 1.0, 0.0];
    }
    run(&mut game, 0.5);
    assert!(feet(&game, bot)[2].abs() < 0.2, "it stopped");
    let mind = game.world.entities.of::<Bot>(bot);
    assert!(shared::quant::wrap_angle(mind.yaw - heading([0.0; 3], [40.0, 0.0, 0.0])).abs() < 0.2, "and looks that way");
}

#[test]
fn far_noises_are_rough_and_each_bot_is_off_its_own_way() {
    let at = [0.0, 1.0, 0.0];
    let far = [60.0, 1.0, 0.0];
    assert_eq!(senses::roughly(1, at, [10.0, 1.0, 0.0]), [10.0, 1.0, 0.0], "near is exact");
    assert_ne!(senses::roughly(1, at, far), far);
    assert_ne!(senses::roughly(1, at, far), senses::roughly(2, at, far));
}

#[test]
fn at_a_waypoint_it_sometimes_stops_and_looks_about() {
    let mut game = game(false);
    game.world.rng = crate::rng::Rng::from_seed(3);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![[0.0, 0.0], [6.0, 0.0], [6.0, 6.0]]).unwrap();
    let (mut paused, mut paces) = (false, Vec::new());
    for _ in 0..40 * TICK_RATE {
        game.step();
        let mind = game.world.entities.of::<Bot>(bot);
        paused |= mind.pause_left > 0;
        paces.push(mind.pace);
    }
    paces.dedup();
    assert!(paused && paces.len() > 1, "paused {paused}, paces {paces:?}");
}

#[test]
fn hurt_in_cover_it_bandages_itself() {
    let mut game = game(false);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![]).unwrap();
    {
        let mut player = game.world.entities.get_mut::<Player>(bot).unwrap();
        player.hands.hotbar[3] = ItemSlot { item: shared::items::ITEM_BANDAGE, ammo: 0, count: 2 };
    }
    game.world.entities.get_mut::<crate::entities::Health>(bot).unwrap().hp = 50;
    {
        let mut mind = game.world.entities.get_mut::<Bot>(bot).unwrap();
        mind.mode = BotMode::Cover;
        mind.mode_left = 30 * TICK_RATE;
        mind.cover = Some(feet_of([0.0, 0.0]));
        mind.sight.last_seen = [30.0, 1.0, 0.0];
    }
    run(&mut game, 8.0);
    assert!(game.world.entities.of::<crate::entities::Health>(bot).hp > 50, "bandaged");
}

fn feet_of([x, z]: [f32; 2]) -> [f32; 3] {
    [x, sim::STANDING_Y, z]
}

/// A frozen bot of `temper`, alone and in the open, 15 m from someone unarmed.
fn cornered(temper: Temper) -> (Game, u32, u32) {
    let mut game = game(false);
    game.world.rng = crate::rng::Rng::from_seed(11);
    let bot = game.add_bot([0.0, 0.0], TEAM_BOTS, vec![]).unwrap();
    game.world.entities.get_mut::<Bot>(bot).unwrap().temper = temper;
    let human = join(&mut game);
    put(&mut game, human, [0.0, -15.0]);
    game.world.entities.get_mut::<Player>(human).unwrap().hands.hotbar = Default::default();
    run(&mut game, 0.7);
    assert!(game.world.entities.of::<Bot>(bot).sight.sees);
    game.world.entities.get_mut::<Bot>(bot).unwrap().nerve = 0.0;
    (game, bot, human)
}

#[test]
fn cornered_a_timid_bot_puts_its_hands_up_stops_firing_and_later_goes_on() {
    let (mut game, bot, _) = cornered(Temper { courage: 0.0, aggression: 0.5, patience: 0.0 });
    run(&mut game, 1.5);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::Surrender);
    assert!(game.world.entities.of::<Player>(bot).surrendered);
    let rounds = rounds_left(&game, bot);
    run(&mut game, 3.0);
    assert_eq!(rounds_left(&game, bot), rounds, "it does not fire");
    run(&mut game, 20.0);
    assert!(!game.world.entities.of::<Player>(bot).surrendered, "it does not keep it up for ever");
}

#[test]
fn other_bots_leave_one_that_has_surrendered_alone() {
    let (mut game, bot, _) = cornered(Temper { courage: 0.0, aggression: 0.5, patience: 0.0 });
    run(&mut game, 1.5);
    let foe = game.add_bot([0.0, 10.0], TEAM_BOTS + 1, vec![]).unwrap();
    game.world.entities.get_mut::<Bot>(foe).unwrap().yaw = PI;
    run(&mut game, 2.0);
    assert_ne!(game.world.entities.of::<Bot>(foe).sight.target, Some(bot));
}

#[test]
fn a_bot_playing_dead_lies_still_until_someone_comes_close() {
    let (mut game, bot, human) = cornered(Temper { courage: 0.6, aggression: 0.5, patience: 1.0 });
    game.world.entities.get_mut::<Bot>(bot).unwrap().temper.courage = 0.5;
    run(&mut game, 1.5);
    assert_eq!(game.world.entities.of::<Bot>(bot).mode, BotMode::PlayDead);
    assert!(game.world.entities.of::<Player>(bot).character.prone);
    put(&mut game, human, [0.0, -4.0]);
    run(&mut game, 0.5);
    let mind = game.world.entities.of::<Bot>(bot);
    assert_eq!(mind.mode, BotMode::Pursue);
    assert!(mind.nerve >= 0.5);
}

#[test]
fn with_friends_by_it_a_timid_bot_does_not_surrender() {
    let (mut game, bot, _) = cornered(Temper { courage: 0.0, aggression: 0.5, patience: 0.0 });
    game.add_bot([3.0, 3.0], TEAM_BOTS, vec![]).unwrap();
    run(&mut game, 1.5);
    assert_ne!(game.world.entities.of::<Bot>(bot).mode, BotMode::Surrender);
}

#[test]
fn flankers_go_round_opposite_sides() {
    let mut game = game(false);
    let (human, bots) = crowd_on(&mut game, 8);
    keep_alive(&mut game, human, 1.5);
    let points: Vec<[f32; 3]> = with_role(&game, &bots, Role::Flanker).iter().filter_map(|&id| game.world.entities.of::<Bot>(id).flank).collect();
    assert_eq!(points.len(), 2);
    assert!(points[0][0] * points[1][0] < 0.0, "{points:?}");
}

#[test]
fn the_first_flanker_goes_round_the_side_out_of_their_view() {
    let game = game(false);
    // Coming from the north at someone looking east, the west side is behind them.
    let mut bot = Bot::new([0.0; 2], vec![]);
    assert!(tokens::send_flanking(&game.world, &mut bot, [0.0, sim::STANDING_Y, 0.0], -PI / 2.0, [0.0, sim::STANDING_Y, -30.0], None));
    assert!(bot.flank.unwrap()[0] < 0.0, "{:?}", bot.flank);
    let mut other = Bot::new([0.0; 2], vec![]);
    assert!(tokens::send_flanking(&game.world, &mut other, [0.0, sim::STANDING_Y, 0.0], PI / 2.0, [0.0, sim::STANDING_Y, -30.0], None));
    assert!(other.flank.unwrap()[0] > 0.0, "{:?}", other.flank);
}
