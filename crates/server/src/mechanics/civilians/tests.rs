use super::*;
use crate::game::{Command, Game, Profile};
use crate::mechanics::TrafficCall;
use shared::protocol::{LIMB_LEG_L, WEAPON_GRENADE, WEAPON_RIFLE};
use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A flat 200 m square with a solid 6 m tall block, a house, from (10, -6) to (22, 6) m.
fn game() -> Game {
    let v = |x: i32, y: i32, z: i32| [x * 100, y * 100, z * 100];
    let vertices = vec![v(10, 0, -6), v(22, 0, -6), v(22, 0, 6), v(10, 0, 6), v(10, 6, -6), v(22, 6, -6), v(22, 6, 6), v(10, 6, 6)];
    let triangles = vec![[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
    let map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[-8_000, -8_000]], collision: CollisionMesh { vertices, triangles }, ..Default::default() };
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

/// A driver who gets out at `[x, z]`, running from `threat`, with their hands up for `hands_up` ticks.
fn get_out(game: &mut Game, [x, z]: [f32; 2], threat: [f32; 2], hands_up: u32) -> u32 {
    let id = game.world.alloc_id();
    let feet = [x, game.world.map.ground.height_at(x, z), z];
    game.world.traffic_calls.push(TrafficCall::Afoot { driver: id, feet, yaw: 0.0, hands_up, threat: [threat[0], 0.0, threat[1]] });
    game.step();
    id
}

#[test]
fn a_driver_out_of_their_car_is_a_civilian_where_they_got_out_seen_as_one() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-20.0, 0.0]);
    let id = get_out(&mut game, [0.0, 0.0], [-20.0, 0.0], 0);
    let player = game.world.entities.of::<Player>(id);
    assert_eq!(player.team, TEAM_NONE_FOR_TESTS);
    assert!(!player.character.chute, "out of a car, not under a canopy");
    assert!(player.character.position[0].abs() < 0.5 && player.character.position[2].abs() < 0.5, "beside the car: {:?}", player.character.position);
    assert!(game.seen_people(KIND_CIVILIAN).iter().any(|(seen, _)| *seen == id));
    assert!(is_civilian(&game.world, id));
}

const TEAM_NONE_FOR_TESTS: u8 = crate::game::TEAM_NONE;

#[test]
fn a_civilian_runs_from_a_soldier_round_a_house_and_never_through_it() {
    let mut game = game();
    let soldier = join(&mut game);
    // The house stands straight on from the soldier, past the civilian.
    put(&mut game, soldier, [-10.0, 0.0]);
    let id = get_out(&mut game, [4.0, 0.0], [-10.0, 0.0], 0);
    for _ in 0..(8 * TICK_RATE) {
        put(&mut game, soldier, [-10.0, 0.0]);
        game.step();
        let [x, _, z] = at(&game, id);
        assert!(!((10.2..21.8).contains(&x) && (-5.8..5.8).contains(&z)), "inside the house at {x}, {z}");
    }
    let [x, _, z] = at(&game, id);
    let away = (x + 10.0).hypot(z);
    assert!(away > 30.0, "ran {away} m from the soldier");
}

#[test]
fn a_civilian_with_their_hands_up_stands_there_and_then_runs() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-15.0, -30.0]);
    let id = get_out(&mut game, [-15.0, -20.0], [-15.0, -30.0], 2 * TICK_RATE);
    let from = at(&game, id);
    run(&mut game, 1.0);
    assert!(game.world.entities.of::<Player>(id).surrendered, "hands up");
    let still = at(&game, id);
    assert!((still[0] - from[0]).hypot(still[2] - from[2]) < 0.2, "and standing");
    run(&mut game, 4.0);
    assert!(!game.world.entities.of::<Player>(id).surrendered);
    let gone = at(&game, id);
    assert!((gone[0] + 15.0).hypot(gone[2] + 30.0) > 20.0, "then they run: {gone:?}");
}

#[test]
fn a_civilian_knocked_down_lies_there_and_gets_up_where_they_are_before_running_on() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-40.0, -30.0]);
    let id = get_out(&mut game, [-30.0, -30.0], [-40.0, -30.0], 0);
    run(&mut game, 0.5);
    let feet = at(&game, id);
    let blast = crate::mechanics::combat::Blast { point: [feet[0] - 1.0, feet[1] - 0.8, feet[2]], radius: 3.0, damage: 0.0, knock: 16.0, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_GRENADE };
    crate::mechanics::combat::blast(&mut game.world, blast);
    assert!(game.world.entities.of::<Player>(id).character.down > 0, "knocked down");
    // Until the knockdown is over.
    let mut ticks = 0;
    while game.world.entities.of::<Player>(id).character.down > 0 {
        game.step();
        ticks += 1;
        assert!(ticks < 10 * TICK_RATE, "never got up");
    }
    let up = at(&game, id);
    for _ in 0..GETUP_TICKS - 2 {
        game.step();
    }
    let rising = at(&game, id);
    assert!((rising[0] - up[0]).hypot(rising[2] - up[2]) < 0.3, "getting up, they stay where they lie: {up:?} to {rising:?}");
    run(&mut game, 2.0);
    let off = at(&game, id);
    assert!((off[0] - up[0]).hypot(off[2] - up[2]) > 4.0, "and then run on");
}

#[test]
fn a_civilian_loses_a_leg_and_dies_as_a_soldier_does_and_killing_one_is_no_kill() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-40.0, 30.0]);
    let id = get_out(&mut game, [-30.0, 30.0], [-40.0, 30.0], 0);
    let harm = crate::mechanics::life::Harm::new(LIMB_HP, Some(soldier), WEAPON_RIFLE).limb(Some(LIMB_LEG_L));
    crate::mechanics::life::damage_entity(&mut game.world, id, harm);
    assert_eq!(crate::mechanics::limbs::lost(&game.world, id), LIMB_LEG_L, "the leg is off, and they live");
    let harm = crate::mechanics::life::Harm::new(HEAD_HP, Some(soldier), WEAPON_RIFLE).head(true);
    assert!(crate::mechanics::life::damage_entity(&mut game.world, id, harm), "the head's health is all it takes");
    game.step();
    assert_eq!(game.world.entities.of::<Player>(soldier).score.kills, 0, "no kill");
    assert!(game.world.entities.has::<Player>(id), "the body lies there");
    run(&mut game, 11.0);
    assert!(!game.world.entities.has::<Player>(id), "and is taken away");
}

#[test]
fn a_civilian_nobody_can_see_is_gone() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-90.0, -90.0]);
    let id = get_out(&mut game, [0.0, 0.0], [-90.0, -90.0], 0);
    game.step();
    assert!(game.world.entities.has::<Player>(id), "one in sight stays");
    put(&mut game, soldier, [90.0, 90.0]);
    let far = game.world.alloc_id();
    let feet = [-95.0, 0.0, 95.0];
    game.world.traffic_calls.push(TrafficCall::Afoot { driver: far, feet, yaw: 0.0, hands_up: 0, threat: [0.0; 3] });
    run(&mut game, 0.2);
    assert!(!game.world.entities.has::<Player>(far), "one out of anyone's sight goes");
}

/// How many ticks civilian `id` lies knocked down by a blast beside them, until they start getting up.
fn knocked_for(game: &mut Game, id: u32) -> u32 {
    let feet = at(game, id);
    let blast = crate::mechanics::combat::Blast { point: [feet[0] - 1.0, feet[1] - 0.8, feet[2]], radius: 3.0, damage: 0.0, knock: 16.0, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_GRENADE };
    crate::mechanics::combat::blast(&mut game.world, blast);
    let mut ticks = 0;
    while game.world.entities.of::<Player>(id).character.down > 0 {
        game.step();
        ticks += 1;
        assert!(ticks < 30 * TICK_RATE, "never got up");
    }
    ticks
}

#[test]
fn a_civilian_knocked_down_lies_longer_the_worse_they_are_hurt_and_by_chance() {
    let mut game = game();
    let soldier = join(&mut game);
    put(&mut game, soldier, [-60.0, 0.0]);
    let whole = get_out(&mut game, [-50.0, -30.0], [-60.0, 0.0], 0);
    let hurt = get_out(&mut game, [-50.0, 30.0], [-60.0, 0.0], 0);
    game.world.entities.get_mut::<crate::entities::Health>(hurt).unwrap().hp = 15;
    run(&mut game, 0.3);
    let (whole_for, hurt_for) = (knocked_for(&mut game, whole), knocked_for(&mut game, hurt));
    assert!(hurt_for > whole_for + 4 * TICK_RATE, "the badly hurt lie {hurt_for} ticks, the whole {whole_for}");
    // The same knock again lies a different while.
    let again = (0..3).map(|_| {
        run(&mut game, 3.0);
        knocked_for(&mut game, whole)
    });
    let times: Vec<u32> = std::iter::once(whole_for).chain(again).collect();
    assert!(times.iter().any(|&t| t.abs_diff(times[0]) > TICK_RATE / 4), "by chance: {times:?}");
}
