use super::*;
use crate::game::{Command, Game, Profile};
use shared::protocol::{FLAG_CLAWING, KIND_ZOMBIE, LIMB_ARM_L, LIMB_LEGS, LIMB_LEG_L, LIMB_LEG_R, WEAPON_RIFLE};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A flat 200 m square.
fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[-5_000, -5_000]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game.world.rng = crate::rng::Rng::from_seed(7);
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    response.try_recv().unwrap().unwrap()
}

/// Puts player `id` standing still at `[x, z]`, on the ground, out of their parachute.
fn put(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    let y = game.world.map.ground.height_at(x, z) + sim::STANDING_Y;
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character = sim::Character { position: [x, y, z], grounded: true, ..Default::default() };
}

fn at(game: &Game, id: u32) -> [f32; 3] {
    game.world.entities.of::<Player>(id).character.position
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

fn hp(game: &Game, id: u32) -> u8 {
    game.world.entities.get::<crate::entities::Health>(id).map_or(0, |h| h.hp)
}

#[test]
fn a_zombie_is_a_soldiers_body_on_the_zombies_side_seen_as_a_zombie() {
    let mut game = game();
    let id = game.add_zombie([10.0, 10.0]).unwrap();
    let player = &game.world.entities.of::<Player>(id);
    assert_eq!(player.team, TEAM_ZOMBIES);
    assert!(!player.character.chute, "it rises from the ground, not under a canopy");
    let [x, _, z] = player.character.position;
    assert!((x - 10.0).abs() < 0.01 && (z - 10.0).abs() < 0.01, "rose at {x}, {z}");
    assert!(game.seen_people(KIND_ZOMBIE).iter().any(|(seen, _)| *seen == id));
}

#[test]
fn a_zombie_walks_to_the_nearest_soldier_and_claws_him() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [0.0, 0.0]);
    let zombie = game.add_zombie([12.0, 0.0]).unwrap();
    let full = hp(&game, soldier);
    let mut clawed = false;
    for _ in 0..20 * TICK_RATE {
        game.step();
        put(&mut game, soldier, [0.0, 0.0]);
        clawed |= game.replicated().iter().any(|(id, seen)| *id == zombie && seen.flags.as_ref().is_some_and(|f| f.flags & FLAG_CLAWING != 0));
        if hp(&game, soldier) < full {
            break;
        }
    }
    let [x, _, z] = at(&game, zombie);
    assert!(x.hypot(z) < ZOMBIE_CLAW_REACH + 0.5, "it came up to him, to {x}, {z}");
    assert!(clawed, "it was seen reaching back to claw");
    assert_eq!(hp(&game, soldier), full - ZOMBIE_CLAW_DAMAGE, "the claw landed on his body");
}

#[test]
fn zombies_never_claw_each_other() {
    let mut game = game();
    let a = game.add_zombie([0.0, 0.0]).unwrap();
    let b = game.add_zombie([1.0, 0.0]).unwrap();
    run(&mut game, 5.0);
    assert_eq!(hp(&game, a), PLAYER_MAX_HP);
    assert_eq!(hp(&game, b), PLAYER_MAX_HP);
}

#[test]
fn a_legless_zombie_still_crawls_after_its_prey() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [0.0, 0.0]);
    let zombie = game.add_zombie([10.0, 0.0]).unwrap();
    run(&mut game, 0.5);
    for leg in [LIMB_LEG_L, LIMB_LEG_R] {
        damage_entity(&mut game.world, zombie, Harm::new(200, Some(soldier), WEAPON_RIFLE).limb(Some(leg)));
    }
    assert!(game.world.entities.alive(zombie), "losing legs kills no one");
    assert_eq!(super::super::limbs::lost(&game.world, zombie) & LIMB_LEGS, LIMB_LEGS);
    let before = at(&game, zombie);
    for _ in 0..6 * TICK_RATE {
        game.step();
        put(&mut game, soldier, [0.0, 0.0]);
    }
    let after = at(&game, zombie);
    assert!(game.world.entities.of::<Player>(zombie).character.prone, "it is down on its belly");
    assert!(after[0] < before[0] - 1.0, "it crawled on, from {before:?} to {after:?}");
}

#[test]
fn shooting_off_a_zombies_arm_leaves_it_alive_but_a_headshot_kills_it() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [0.0, 0.0]);
    let zombie = game.add_zombie([30.0, 0.0]).unwrap();
    damage_entity(&mut game.world, zombie, Harm::new(200, Some(soldier), WEAPON_RIFLE).limb(Some(LIMB_ARM_L)));
    assert!(game.world.entities.alive(zombie));
    assert_ne!(super::super::limbs::lost(&game.world, zombie) & LIMB_ARM_L, 0, "the arm came off");
    damage_entity(&mut game.world, zombie, Harm::new(200, Some(soldier), WEAPON_RIFLE).head(true));
    assert!(!game.world.entities.alive(zombie));
}

#[test]
fn a_zombie_goes_to_a_gunshot_it_hears() {
    let mut game = game();
    let zombie = game.add_zombie([0.0, 0.0]).unwrap();
    run(&mut game, 0.5);
    super::super::bots::make_shot(&mut game.world, [0.0, 1.0, 40.0], None, [0.0, 0.0, 1.0]);
    run(&mut game, 6.0);
    let [_, _, z] = at(&game, zombie);
    assert!(z > 5.0, "it shambled towards the shot, to z {z}");
}

#[test]
fn a_hurt_zombie_turns_on_whoever_hurt_it() {
    let mut game = game();
    let near = join(&mut game);
    let shooter = join(&mut game);
    put(&mut game, near, [5.0, 0.0]);
    put(&mut game, shooter, [-30.0, 0.0]);
    let zombie = game.add_zombie([0.0, 0.0]).unwrap();
    run(&mut game, 0.5);
    damage_entity(&mut game.world, zombie, Harm::new(10, Some(shooter), WEAPON_RIFLE).from([-30.0, 1.0, 0.0]));
    game.step();
    assert_eq!(game.world.entities.of::<Zombie>(zombie).prey, Some(shooter));
}

#[test]
fn a_zombie_far_from_what_it_heard_heads_straight_for_it_without_looking_for_a_way() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-90.0, -90.0]);
    let id = game.add_zombie([-60.0, 0.0]).unwrap();
    run(&mut game, 1.0);
    let from = at(&game, id);
    crate::mechanics::bots::make_noise(&mut game.world, [60.0, 0.0, 0.0], Some(soldier), 150.0);
    run(&mut game, 10.0);
    let now = at(&game, id);
    assert!(now[0] > from[0] + 3.0, "it went towards the noise: from {from:?} to {now:?}");
    assert!(now[0] < 0.0, "it is still far from it: {now:?}");
    assert_eq!(game.world.nav.built(), 0, "no way was looked for");
}

/// Not a regression test: what a horde costs a tick on a real world, with someone to chase and with
/// only blasts to go to. `RAGNAROK_WORLD` picks the world.
/// `RAGNAROK_WORLD=worlds/bussjobanan/world.msgpack cargo test -p server -- --ignored --nocapture horde_tick_cost`
#[test]
#[ignore]
fn horde_tick_cost() {
    use std::time::{Duration, Instant};
    let Some(path) = std::env::var_os("RAGNAROK_WORLD") else { return println!("set RAGNAROK_WORLD") };
    let count: usize = std::env::var("ZOMBIES").ok().and_then(|n| n.parse().ok()).unwrap_or(20);
    for case in ["nobody", "prey 60 m off", "blasts 120 m off", "blasts and prey far off"] {
        let (map, _) = crate::read_world(std::path::Path::new(&path)).unwrap();
        let mut game = Game::new(map, 0).unwrap();
        game.world.clear_pickups();
        let [sx, sz] = game.world.map.spawns[0].map(|v| v as f32 / 100.0);
        for n in 0..count {
            game.add_zombie([sx + (n % 5) as f32 * 2.0, sz + (n / 5) as f32 * 2.0]);
        }
        let soldier = join(&mut game);
        let far = if case == "prey 60 m off" { 60.0 } else { 2000.0 };
        put(&mut game, soldier, [sx + far, sz]);
        let (mut total, mut worst, mut late) = (Duration::ZERO, Duration::ZERO, 0);
        let ticks = 30 * TICK_RATE;
        for tick in 0..ticks {
            if case.starts_with("blasts") && tick % TICK_RATE == 0 {
                let angle = tick as f32 * 0.37;
                let (x, z) = (sx + 120.0 * angle.cos(), sz + 120.0 * angle.sin());
                let y = game.world.map.ground.height_at(x, z);
                crate::mechanics::bots::make_noise(&mut game.world, [x, y, z], Some(soldier), 150.0);
            }
            let start = Instant::now();
            game.step();
            let took = start.elapsed();
            total += took;
            worst = worst.max(took);
            late += usize::from(took > Duration::from_micros(16_667));
        }
        println!("{count} zombies, {case}: mean {:?} a tick, worst {worst:?}, {late} of {ticks} ticks over 16.7 ms, {} nav chunks", total / ticks, game.world.nav.built());
    }
}
