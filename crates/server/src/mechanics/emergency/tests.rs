use super::*;
use crate::game::{Command, Game, Profile};
use crate::mechanics::life::{Harm, damage_entity};
use crate::mechanics::{Civilian, TrafficCall};
use shared::protocol::{LIMB_LEG_L, WEAPON_RIFLE};
use shared::world::{ROAD_MINOR, Road, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// A two-lane road along `points` (metres).
fn road(points: &[[f32; 2]]) -> Road {
    Road { width: 700, kind: ROAD_MINOR, points: points.iter().map(|p| [(p[0] * 100.0) as i32, (p[1] * 100.0) as i32]).collect(), lanes: 2, forward: 1, ..Default::default() }
}

/// A flat world 1200 m square with a block of streets 400 m by 300 m in its middle, a road across
/// the block, and a road out of the world from each of two corners: with ambulances and police if
/// `services`, and no traffic.
fn game(services: bool) -> Game {
    let roads = vec![
        road(&[[-200.0, -150.0], [0.0, -150.0], [200.0, -150.0]]),
        road(&[[200.0, -150.0], [200.0, 150.0]]),
        road(&[[200.0, 150.0], [0.0, 150.0], [-200.0, 150.0]]),
        road(&[[-200.0, 150.0], [-200.0, -150.0]]),
        road(&[[0.0, -150.0], [0.0, 150.0]]),
        road(&[[-600.0, -150.0], [-200.0, -150.0]]),
        road(&[[200.0, 150.0], [600.0, 150.0]]),
    ];
    let map = WorldFile { version: WORLD_VERSION, half_extent: 60_000, spawns: vec![[-55_000, 55_000]], roads, ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    (game.world.fleet.ambulance, game.world.fleet.police) = (services, services);
    game
}

/// Someone who joins and stands at `[x, z]`.
fn join(game: &mut Game, [x, z]: [f32; 2]) -> u32 {
    let (outbox, inbox) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    let id = response.try_recv().unwrap().unwrap();
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character = sim::Character { position: [x, sim::STANDING_Y, z], grounded: true, ..Default::default() };
    id
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        game.step();
    }
}

/// A civilian on their feet at `[x, z]`.
fn civilian(game: &mut Game, [x, z]: [f32; 2]) -> u32 {
    let id = game.world.alloc_id();
    game.world.traffic_calls.push(TrafficCall::Afoot { driver: id, feet: [x, 0.0, z], yaw: 0.0, hands_up: 0, threat: [x + 30.0, 0.0, z] });
    game.step();
    id
}

/// Kills `id`, by nobody's hand.
fn kill(game: &mut Game, id: u32) {
    assert!(damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, None, WEAPON_RIFLE).head(true)));
}

fn medics(game: &Game) -> Vec<u32> {
    game.world.entities.ids::<With<Medic>>()
}

/// The ambulances that are out, each with what it is doing.
fn ambulances(game: &Game) -> Vec<Duty> {
    game.world.fleet.units.iter().filter(|unit| unit.service == Service::Ambulance && unit.duty != Duty::Dormant).map(|unit| unit.duty).collect()
}

/// Steps `game` until `done` says so, for `seconds` at most. Whether it did.
fn until(game: &mut Game, seconds: f32, mut done: impl FnMut(&Game) -> bool) -> bool {
    for _ in 0..(seconds * TICK_RATE as f32) as u32 {
        if done(game) {
            return true;
        }
        game.step();
    }
    done(game)
}

#[test]
fn a_civilian_killed_lies_there_until_an_ambulance_has_come_and_its_medics_have_taken_them() {
    let mut game = game(true);
    join(&mut game, [240.0, 30.0]);
    let id = civilian(&mut game, [208.0, 0.0]);
    kill(&mut game, id);
    // No ambulance is called for the first few seconds, and then one is on its way.
    run(&mut game, 2.0);
    assert!(ambulances(&game).is_empty());
    run(&mut game, 5.0);
    assert_eq!(ambulances(&game), vec![Duty::Responding]);
    assert!(game.world.entities.of::<Civilian>(id).claimed);
    // It parks by the place, and two medics get out of it.
    assert!(until(&mut game, 240.0, |game| medics(game).len() == 2), "the medics are out");
    assert_eq!(ambulances(&game), vec![Duty::OnScene]);
    assert!(game.world.entities.has::<Player>(id), "the body has lain there all the while");
    for &medic in &medics(&game) {
        let player = game.world.entities.of::<Player>(medic);
        assert_eq!((player.team, player.held().item), (TEAM_SERVICES, shared::items::ITEM_NONE));
        assert!(crate::mechanics::is_noncombatant(&game.world, medic));
        let at = player.character.position;
        assert!((at[0] - 200.0).hypot(at[2]) < 15.0, "beside their ambulance: {at:?}");
    }
    assert!(game.seen_people(MEDIC_KIND).len() >= 2);
    // One of them walks to the body and kneels by it, and then it is gone from where it lay.
    assert!(until(&mut game, 60.0, |game| medics(game).iter().any(|&medic| matches!(game.world.entities.of::<Medic>(medic).task, medics::Task::Lifting(..)))), "a medic kneels by the body");
    let kneeling = medics(&game).into_iter().find(|&medic| matches!(game.world.entities.of::<Medic>(medic).task, medics::Task::Lifting(..))).unwrap();
    let (medic, body) = (game.world.entities.of::<Player>(kneeling).character.position, game.world.entities.of::<Player>(id).character.position);
    assert!(distance_2d(medic, body) < 2.0);
    assert!(until(&mut game, 5.0, |game| !game.world.entities.has::<Player>(id)), "the body is taken up");
    // They get back in, and the ambulance drives off.
    assert!(until(&mut game, 60.0, |game| medics(game).is_empty()), "the medics are back aboard");
    run(&mut game, 2.0);
    assert_eq!(ambulances(&game), vec![Duty::Leaving]);
    assert!(until(&mut game, 240.0, |game| ambulances(game).is_empty()), "and is gone once it is out of sight");
}

#[test]
fn in_a_world_with_no_ambulances_the_dead_are_gone_as_soon_as_ever_and_nobody_comes() {
    let mut game = game(false);
    join(&mut game, [240.0, 30.0]);
    let id = civilian(&mut game, [208.0, 0.0]);
    kill(&mut game, id);
    run(&mut game, 9.0);
    assert!(game.world.entities.has::<Player>(id));
    run(&mut game, 3.0);
    assert!(!game.world.entities.has::<Player>(id));
    assert!(game.world.fleet.units.is_empty() && game.world.fleet.orders.is_empty());
}

#[test]
fn a_badly_hurt_civilian_stays_down_for_the_medics_and_one_lightly_hurt_runs() {
    let mut game = game(true);
    join(&mut game, [240.0, 30.0]);
    let (badly, lightly, legless) = (civilian(&mut game, [208.0, 0.0]), civilian(&mut game, [212.0, 6.0]), civilian(&mut game, [210.0, -8.0]));
    let start = game.world.entities.of::<Player>(lightly).character.position;
    damage_entity(&mut game.world, badly, Harm::new(PLAYER_MAX_HP / 10 * 7, None, WEAPON_RIFLE));
    damage_entity(&mut game.world, lightly, Harm::new(PLAYER_MAX_HP / 10 * 3, None, WEAPON_RIFLE));
    damage_entity(&mut game.world, legless, Harm::new(LIMB_HP, None, WEAPON_RIFLE).limb(Some(LIMB_LEG_L)));
    run(&mut game, 10.0);
    for id in [badly, legless] {
        let (mind, player) = (game.world.entities.of::<Civilian>(id), game.world.entities.of::<Player>(id));
        assert!(mind.hurt && player.character.down > 0 && game.world.entities.alive(id), "down, and alive");
    }
    let lay = [badly, legless].map(|id| game.world.entities.of::<Player>(id).character.position);
    let ran = game.world.entities.of::<Player>(lightly).character.position;
    assert!(!game.world.entities.of::<Civilian>(lightly).hurt && distance_2d(start, ran) > 10.0, "the other ran {} m", distance_2d(start, ran));
    // They lie where they fell until the medics have them, one after the other in one ambulance.
    assert!(until(&mut game, 240.0, |game| medics(game).len() == 2), "the medics are out");
    for (id, lay) in [badly, legless].into_iter().zip(lay) {
        assert!(distance_2d(game.world.entities.of::<Player>(id).character.position, lay) < 1.0);
    }
    assert!(until(&mut game, 120.0, |game| ![badly, legless].iter().any(|&id| game.world.entities.has::<Player>(id))), "both are taken up");
    let back = until(&mut game, 60.0, |game| medics(game).is_empty());
    assert!(back, "and the medics are back aboard");
    run(&mut game, 2.0);
    assert_eq!(game.world.fleet.units.iter().filter(|unit| unit.service == Service::Ambulance && unit.tag != 0).count(), 0, "one ambulance came, and is let go");
}

#[test]
fn a_badly_hurt_civilian_nobody_comes_for_gets_up_after_a_while() {
    let mut game = game(true);
    game.world.fleet.ambulance = true;
    join(&mut game, [240.0, 30.0]);
    // Far from any road: no ambulance can be sent.
    let soldier = join(&mut game, [-420.0, 420.0]);
    let id = civilian(&mut game, [-400.0, 400.0]);
    damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP / 10 * 7, Some(soldier), WEAPON_RIFLE));
    run(&mut game, 60.0);
    assert!(game.world.entities.of::<Civilian>(id).hurt && game.world.entities.of::<Player>(id).character.down > 0);
    assert!(ambulances(&game).is_empty());
    let lay = game.world.entities.of::<Player>(id).character.position;
    run(&mut game, 45.0);
    assert!(!game.world.entities.of::<Civilian>(id).hurt);
    assert!(distance_2d(game.world.entities.of::<Player>(id).character.position, lay) > 3.0, "up and off");
}

#[test]
fn casualties_close_together_are_one_incident_with_one_ambulance_and_those_far_apart_are_two() {
    let mut game = game(true);
    join(&mut game, [240.0, 30.0]);
    join(&mut game, [-240.0, -30.0]);
    let near = [civilian(&mut game, [208.0, 0.0]), civilian(&mut game, [210.0, 20.0]), civilian(&mut game, [206.0, -25.0])];
    for id in near {
        kill(&mut game, id);
        run(&mut game, 1.0);
    }
    run(&mut game, 8.0);
    assert_eq!(ambulances(&game).len(), 1, "one ambulance for the three of them");
    // Another, the other side of the block: its own incident, and the second ambulance.
    let far = civilian(&mut game, [-208.0, 0.0]);
    kill(&mut game, far);
    run(&mut game, 8.0);
    assert_eq!(ambulances(&game).len(), 2);
    // And a third place has to wait: there are only the two.
    let third = civilian(&mut game, [150.0, 100.0]);
    kill(&mut game, third);
    run(&mut game, 8.0);
    assert_eq!(ambulances(&game).len(), 2);
    assert!(!game.world.entities.of::<Civilian>(third).claimed);
}

#[test]
fn never_more_than_the_cap_lie_waiting_however_many_are_killed() {
    let mut game = game(true);
    join(&mut game, [0.0, 60.0]);
    // In a field in the middle of the block, with no road within reach: nobody comes.
    let mut all = Vec::new();
    for n in 0..20 {
        let id = civilian(&mut game, [60.0 + (n % 5) as f32 * 3.0, 40.0 + (n / 5) as f32 * 3.0]);
        kill(&mut game, id);
        all.push(id);
    }
    run(&mut game, 3.0);
    let lying = |game: &Game| all.iter().filter(|&&id| game.world.entities.has::<Player>(id)).count();
    assert_eq!(lying(&game), CASUALTIES_MAX);
    assert!(all[..8].iter().all(|&id| !game.world.entities.has::<Player>(id)), "the first killed are the first gone");
}

#[test]
fn medics_stay_in_their_ambulance_while_shots_are_fired_near_it() {
    let mut game = game(true);
    let soldier = join(&mut game, [240.0, 30.0]);
    let id = civilian(&mut game, [208.0, 0.0]);
    kill(&mut game, id);
    assert!(until(&mut game, 240.0, |game| ambulances(game) == vec![Duty::OnScene]), "the ambulance is there");
    // Someone keeps firing by it: nobody gets out.
    for tick in 0..(20 * TICK_RATE) {
        if tick.is_multiple_of(TICK_RATE / 2) {
            crate::mechanics::bots::make_shot(&mut game.world, [215.0, 1.0, 10.0], Some(soldier), [0.0, 0.0, -1.0]);
        }
        game.step();
        assert!(medics(&game).is_empty());
    }
    // When it has been quiet a while they do.
    assert!(until(&mut game, 15.0, |game| medics(game).len() == 2), "the medics are out");
}

#[test]
fn medics_whose_ambulance_is_taken_are_gone_and_another_comes_for_who_is_left() {
    let mut game = game(true);
    let soldier = join(&mut game, [240.0, 30.0]);
    let ids = [civilian(&mut game, [208.0, 0.0]), civilian(&mut game, [235.0, -30.0])];
    for id in ids {
        kill(&mut game, id);
    }
    assert!(until(&mut game, 240.0, |game| medics(game).len() == 2), "the medics are out");
    let vid = game.world.fleet.units.iter().find(|unit| unit.duty == Duty::OnScene).and_then(|unit| unit.vid).expect("their ambulance");
    crate::mechanics::seat(&mut game.world, soldier, vid, 0);
    run(&mut game, 2.0);
    assert!(ambulances(&game).is_empty(), "the ambulance is the player's");
    assert!(until(&mut game, 50.0, |game| medics(game).is_empty()), "its medics are gone in the end");
    crate::mechanics::vehicles::alight(&mut game.world, soldier);
    if ids.iter().any(|&id| game.world.entities.has::<Player>(id)) {
        assert!(until(&mut game, 60.0, |game| !ambulances(game).is_empty()), "and another ambulance is sent");
    }
}

/// How many stars `id` has.
fn stars(game: &Game, id: u32) -> u8 {
    wanted::stars(&game.world, id)
}

fn officers(game: &Game) -> Vec<u32> {
    game.world.entities.ids::<With<Officer>>()
}

/// The police cars that are out for `suspect`, each with what it is doing.
fn police(game: &Game, suspect: u32) -> Vec<Duty> {
    game.world.fleet.units.iter().filter(|unit| unit.service == Service::Police && unit.tag == suspect).map(|unit| unit.duty).collect()
}

/// `by` wounds `victim` with a rifle, taking `amount` of their health.
fn shoot(game: &mut Game, by: u32, victim: u32, amount: u8) {
    damage_entity(&mut game.world, victim, Harm::new(amount, Some(by), WEAPON_RIFLE));
    game.step();
}

/// Holds player `id` where they stand: a test's player has nobody at the controls.
fn hold(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character.position = [x, sim::STANDING_Y, z];
}

#[test]
fn a_player_who_hurts_a_civilian_is_wanted_and_one_police_car_comes_for_them() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    let bystander = join(&mut game, [225.0, 30.0]);
    let id = civilian(&mut game, [210.0, 0.0]);
    assert_eq!(stars(&game, soldier), 0);
    shoot(&mut game, soldier, id, 20);
    assert_eq!(stars(&game, soldier), 1);
    // The same one hit again at once is the same crime; another is not, and killing is worse.
    shoot(&mut game, soldier, id, 20);
    assert_eq!(game.world.entities.of::<Wanted>(soldier).heat, 10.0);
    assert_eq!(stars(&game, bystander), 0, "nobody else is wanted for it");
    run(&mut game, 2.0);
    assert_eq!(police(&game, soldier).len(), 1, "one car for one star");
    let car = game.world.fleet.units.iter().find(|unit| unit.tag == soldier).unwrap();
    assert!(distance_2d(car.at, [215.0, 0.0, 20.0]) < 460.0, "from near by: {:?}", car.at);
    assert!(until(&mut game, 240.0, |game| officers(game).len() == 2), "it parks by them and two officers get out");
    for &officer in &officers(&game) {
        let player = game.world.entities.of::<Player>(officer);
        assert_eq!(player.team, TEAM_SERVICES);
        assert_eq!(player.hands.hotbar[0].item, shared::items::ITEM_REVOLVER);
        assert_eq!(game.world.entities.of::<Officer>(officer).suspect, soldier);
    }
    assert!(game.seen_people(OFFICER_KIND).len() >= 2);
}

#[test]
fn each_worse_crime_is_another_star_and_more_police_cars_up_to_eight() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    let victims: Vec<u32> = (0..6).map(|n| civilian(&mut game, [205.0 + n as f32 * 2.0, 0.0])).collect();
    shoot(&mut game, soldier, victims[0], 20);
    shoot(&mut game, soldier, victims[1], 20);
    shoot(&mut game, soldier, victims[2], 20);
    assert_eq!(stars(&game, soldier), 2, "three wounded is 30");
    shoot(&mut game, soldier, victims[0], PLAYER_MAX_HP);
    assert_eq!(stars(&game, soldier), 2, "and one of them dead 55");
    shoot(&mut game, soldier, victims[1], PLAYER_MAX_HP);
    assert_eq!(stars(&game, soldier), 3);
    run(&mut game, 1.0);
    assert!(police(&game, soldier).len() <= 2, "two cars a second at most");
    run(&mut game, 5.0);
    assert_eq!(police(&game, soldier).len(), 4, "as many as the stars call for");
    for &victim in &victims[2..] {
        shoot(&mut game, soldier, victim, PLAYER_MAX_HP);
    }
    assert_eq!(stars(&game, soldier), 5);
    for _ in 0..(10 * TICK_RATE) {
        hold(&mut game, soldier, [215.0, 20.0]);
        // Alive for as long as it takes them all to come: the officers out have nothing to shoot with.
        for officer in officers(&game) {
            game.world.entities.get_mut::<Player>(officer).unwrap().hands.hotbar = crate::game::hotbar_of([shared::items::ITEM_NONE; shared::items::HOTBAR_SLOTS]);
        }
        game.step();
        assert!(game.world.entities.alive(soldier), "shot dead");
        assert!(police(&game, soldier).len() <= 8, "never more than eight");
    }
    assert_eq!(police(&game, soldier).len(), 8);
    assert!(game.world.entities.of::<Wanted>(soldier).heat <= 200.0);
}

#[test]
fn a_player_who_stands_still_by_an_officer_is_arrested_and_starts_again_with_a_fine() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    game.world.entities.get_mut::<Player>(soldier).unwrap().score.cash = 1000;
    game.world.entities.get_mut::<Player>(soldier).unwrap().hands.hotbar[0] = Default::default();
    let id = civilian(&mut game, [210.0, 0.0]);
    shoot(&mut game, soldier, id, 20);
    let deaths = game.world.entities.of::<Player>(soldier).score.deaths;
    // They stand there and wait for it.
    let mut out = false;
    for _ in 0..(300 * TICK_RATE) {
        if stars(&game, soldier) == 0 {
            break;
        }
        hold(&mut game, soldier, [215.0, 20.0]);
        game.step();
        out |= !officers(&game).is_empty();
        assert!(game.world.entities.alive(soldier), "nobody shoots at one star");
    }
    assert!(out, "officers got out");
    assert_eq!(stars(&game, soldier), 0, "arrested: wanted no more");
    let player = game.world.entities.of::<Player>(soldier);
    assert_eq!(player.score.cash, 800, "a fifth of their cash is gone");
    assert_eq!(player.score.deaths, deaths, "and it is no death");
    assert_eq!(player.hands.hotbar[0], player.hands.loadout[0], "with what a fresh life carries");
    let at = player.character.position;
    assert!(distance_2d(at, [215.0, 0.0, 20.0]) > 100.0, "back where a life starts: {at:?}");
    // The officers walk back to their car, and it drives off.
    assert!(until(&mut game, 60.0, |game| officers(game).is_empty()), "the officers are back aboard");
    run(&mut game, 2.0);
    assert!(police(&game, soldier).is_empty(), "and the car is let go");
}

#[test]
fn a_player_who_keeps_moving_is_not_arrested_and_fleeing_in_sight_of_the_police_is_another_star() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    let id = civilian(&mut game, [210.0, 0.0]);
    shoot(&mut game, soldier, id, 20);
    assert!(until(&mut game, 240.0, |game| !officers(game).is_empty()), "officers got out");
    // Round and round a circle 12 m across near the car, at 4 m/s: in their sight, never still.
    for tick in 0..(50 * TICK_RATE) {
        let turn = tick as f32 / TICK_RATE as f32 * 4.0 / 6.0;
        hold(&mut game, soldier, [215.0 + turn.cos() * 6.0, 20.0 + turn.sin() * 6.0]);
        game.step();
    }
    assert!(stars(&game, soldier) >= 2, "{} stars", stars(&game, soldier));
    assert!(game.world.entities.alive(soldier));
}

#[test]
fn attacking_the_police_makes_them_shoot_to_kill_and_they_go_home_when_the_player_is_dead() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    let id = civilian(&mut game, [210.0, 0.0]);
    shoot(&mut game, soldier, id, 20);
    assert!(until(&mut game, 240.0, |game| officers(game).len() == 2), "officers got out");
    // A round into one of them, and another: that is three stars and more.
    let officer = officers(&game)[0];
    shoot(&mut game, soldier, officer, 20);
    shoot(&mut game, soldier, officer, 20);
    assert!(stars(&game, soldier) >= 3, "{} stars", stars(&game, soldier));
    let mut fired = false;
    for _ in 0..(120 * TICK_RATE) {
        if !game.world.entities.alive(soldier) {
            break;
        }
        hold(&mut game, soldier, [215.0, 20.0]);
        game.step();
        fired |= officers(&game).iter().any(|&officer| game.world.entities.of::<Player>(officer).input.buttons() & BUTTON_FIRE != 0);
    }
    assert!(fired, "they fired");
    assert!(!game.world.entities.alive(soldier), "and the player is dead");
    game.step();
    assert_eq!(stars(&game, soldier), 0, "the dead are wanted no more");
    // Every car that came for them goes home.
    assert!(until(&mut game, 90.0, |game| officers(game).is_empty()), "the officers are back aboard");
    run(&mut game, 2.0);
    assert!(police(&game, soldier).is_empty(), "and the cars are let go");
    assert!(game.world.fleet.units.iter().all(|unit| unit.service != Service::Police || matches!(unit.duty, Duty::Dormant | Duty::Leaving)));
}

#[test]
fn an_officers_rounds_hurt_nobody_but_their_suspect() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    let bystander = join(&mut game, [216.0, 21.0]);
    let id = civilian(&mut game, [210.0, 0.0]);
    shoot(&mut game, soldier, id, 20);
    assert!(until(&mut game, 240.0, |game| officers(game).len() == 2), "officers got out");
    let officer = officers(&game)[0];
    let other = civilian(&mut game, [214.0, 22.0]);
    for victim in [bystander, other, officers(&game)[1]] {
        assert!(!damage_entity(&mut game.world, victim, Harm::new(50, Some(officer), WEAPON_RIFLE)), "{victim} is not theirs to hurt");
    }
    assert!(damage_entity(&mut game.world, soldier, Harm::new(50, Some(officer), WEAPON_RIFLE)) || !game.world.entities.alive(soldier) || game.world.entities.of::<crate::entities::Health>(soldier).hp < PLAYER_MAX_HP, "the suspect is");
    assert_eq!(stars(&game, bystander), 0);
}

#[test]
fn a_player_nobody_has_seen_for_long_enough_is_wanted_no_more_and_the_police_drive_off() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    let id = civilian(&mut game, [210.0, 0.0]);
    shoot(&mut game, soldier, id, 20);
    run(&mut game, 3.0);
    assert_eq!(police(&game, soldier).len(), 1);
    // Off into a field, far from where they were seen and from any road.
    hold(&mut game, soldier, [80.0, 60.0]);
    run(&mut game, 5.0);
    let wanted = game.world.entities.of::<Wanted>(soldier);
    assert!(wanted.unseen > 0 && distance_2d(wanted.seen_at, [215.0, 0.0, 20.0]) < 1.0, "the police know only where they were");
    assert_eq!(crate::mechanics::wanted_byte(&game.world, soldier), 0x81, "one star, unseen");
    assert!(until(&mut game, 30.0, |game| stars(game, soldier) == 0), "20 s unseen at one star");
    assert!(until(&mut game, 90.0, |game| officers(game).is_empty() && police(game, soldier).is_empty()), "and whoever came for them has gone home");
}

#[test]
fn the_police_stand_down_when_their_suspect_leaves_the_game() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    join(&mut game, [230.0, 30.0]);
    let id = civilian(&mut game, [210.0, 0.0]);
    shoot(&mut game, soldier, id, 20);
    assert!(until(&mut game, 240.0, |game| officers(game).len() == 2), "officers got out");
    game.handle(Command::Leave(soldier));
    assert!(until(&mut game, 90.0, |game| officers(game).is_empty() && police(game, soldier).is_empty()), "they go home");
}

#[test]
fn firing_a_gun_is_a_crime_only_in_earshot_of_the_police_and_never_more_than_two_stars() {
    let mut game = game(true);
    game.world.fleet.patrols = 0;
    let soldier = join(&mut game, [215.0, 20.0]);
    run(&mut game, 1.0);
    // With no police about, nothing comes of it.
    crate::mechanics::bots::make_shot(&mut game.world, [215.0, 1.5, 20.0], Some(soldier), [0.0, 1.0, 0.0]);
    run(&mut game, 1.0);
    assert_eq!(stars(&game, soldier), 0);
    // With a police car parked across the road, each burst is a little heat, up to two stars. (Into
    // the air: a round past an officer who has come for them is worse.)
    game.world.fleet.orders.push(FleetOrder::Send { service: Service::Police, to: [200.0, 0.0, 20.0], tag: TAG_INCIDENT | 77, ahead: None });
    assert!(until(&mut game, 240.0, |game| game.world.fleet.units.iter().any(|unit| unit.tag == TAG_INCIDENT | 77 && unit.duty == Duty::OnScene && unit.vid.is_some())), "a police car is there");
    // (The police that come for it arrest whoever stands still, so the most heat there was is what counts.)
    let mut most = 0.0f32;
    for _ in 0..8 {
        crate::mechanics::bots::make_shot(&mut game.world, [215.0, 1.5, 20.0], Some(soldier), [0.0, 1.0, 0.0]);
        game.step();
        most = most.max(game.world.entities.get::<Wanted>(soldier).map_or(0.0, |wanted| wanted.heat));
        hold(&mut game, soldier, [215.0, 20.0]);
        run(&mut game, 5.5);
    }
    assert!((10.0..=30.0).contains(&most), "{most}");
}

#[test]
fn taking_a_police_car_is_a_crime_and_taking_a_car_of_the_traffic_only_where_the_police_see_it() {
    let mut game = game(true);
    let soldier = join(&mut game, [215.0, 20.0]);
    game.world.fleet.orders.push(FleetOrder::Send { service: Service::Police, to: [200.0, 0.0, 20.0], tag: TAG_INCIDENT | 77, ahead: None });
    assert!(until(&mut game, 240.0, |game| game.world.fleet.units.iter().any(|unit| unit.tag == TAG_INCIDENT | 77 && unit.duty == Duty::OnScene && unit.vid.is_some())), "a police car is there");
    let vid = game.world.fleet.units.iter().find(|unit| unit.tag == TAG_INCIDENT | 77).and_then(|unit| unit.vid).unwrap();
    crate::mechanics::seat(&mut game.world, soldier, vid, 0);
    run(&mut game, 0.5);
    assert_eq!(stars(&game, soldier), 2, "30 for the police car");
}

#[test]
fn a_bot_that_kills_a_civilian_is_nobodys_suspect_but_the_ambulance_still_comes() {
    let mut game = game(true);
    join(&mut game, [240.0, 30.0]);
    let bot = game.add_bot([230.0, 10.0], 1, vec![]).unwrap();
    let id = civilian(&mut game, [208.0, 0.0]);
    damage_entity(&mut game.world, id, Harm::new(PLAYER_MAX_HP, Some(bot), WEAPON_RIFLE).head(true));
    run(&mut game, 8.0);
    assert!(game.world.entities.ids::<With<Wanted>>().is_empty());
    assert!(game.world.fleet.units.iter().all(|unit| unit.service != Service::Police || unit.duty == Duty::Dormant));
    assert_eq!(ambulances(&game), vec![Duty::Responding]);
}

#[test]
fn there_are_never_more_police_cars_out_than_there_are_however_many_are_wanted() {
    let mut game = game(true);
    game.world.fleet.patrols = 2;
    let suspects: Vec<u32> = (0..4).map(|n| join(&mut game, [215.0 + n as f32 * 3.0, 20.0])).collect();
    let victims: Vec<u32> = (0..8).map(|n| civilian(&mut game, [205.0 + n as f32 * 2.0, 0.0])).collect();
    for (n, &suspect) in suspects.iter().enumerate() {
        shoot(&mut game, suspect, victims[n * 2], PLAYER_MAX_HP);
        shoot(&mut game, suspect, victims[n * 2 + 1], PLAYER_MAX_HP);
    }
    for _ in 0..(30 * TICK_RATE) {
        for (n, &suspect) in suspects.iter().enumerate() {
            hold(&mut game, suspect, [215.0 + n as f32 * 3.0, 20.0]);
        }
        game.step();
        assert!(game.world.fleet.units.iter().filter(|unit| unit.service == Service::Police && unit.duty != Duty::Dormant).count() <= crate::mechanics::traffic::POLICE);
        assert!(officers(&game).len() <= 2 * crate::mechanics::traffic::POLICE);
    }
    // Each suspect has their own cars, and each officer the suspect of their car.
    for &officer in &officers(&game) {
        let mind = game.world.entities.of::<Officer>(officer);
        assert_eq!(game.world.fleet.units[mind.unit].tag, mind.suspect);
    }
}

#[test]
fn a_police_car_leaves_the_road_after_a_suspect_in_a_field_and_its_officers_get_out_beside_them() {
    let mut game = game(true);
    // In the middle of a field, 100 m from the nearest road.
    let soldier = join(&mut game, [100.0, 0.0]);
    let id = civilian(&mut game, [104.0, 4.0]);
    shoot(&mut game, soldier, id, 20);
    let mut chased = false;
    for _ in 0..(300 * TICK_RATE) {
        if !officers(&game).is_empty() {
            break;
        }
        hold(&mut game, soldier, [100.0, 0.0]);
        game.step();
        chased |= police(&game, soldier).contains(&Duty::Chasing);
    }
    assert!(chased, "it went straight for them");
    assert_eq!(officers(&game).len(), 2, "and its officers got out");
    let car = game.world.fleet.units.iter().find(|unit| unit.tag == soldier).unwrap();
    assert_eq!(car.duty, Duty::OnScene);
    assert!(distance_2d(car.at, [100.0, 0.0, 0.0]) < 25.0, "parked {} m from them, off the road", distance_2d(car.at, [100.0, 0.0, 0.0]));
    assert!(game.world.entities.alive(soldier) && game.world.entities.of::<crate::entities::Health>(soldier).hp == PLAYER_MAX_HP, "without running them over");
    // Wanted no more, the car finds its way back to the road and is gone.
    wanted::clear(&mut game.world, soldier);
    assert!(until(&mut game, 60.0, |game| officers(game).is_empty()), "the officers are back aboard");
    hold(&mut game, soldier, [-550.0, 550.0]);
    assert!(until(&mut game, 120.0, |game| game.world.fleet.units.iter().all(|unit| unit.service != Service::Police || unit.duty == Duty::Dormant)), "{:?}", game.world.fleet.units);
}

#[test]
fn nobody_takes_a_police_car_with_its_crew_in_it_and_one_they_have_left_is_there_to_take() {
    let mut game = game(true);
    let soldier = suspect(&mut game, [215.0, 20.0], wanted::HEAT[0], [0.0; 2], 0.0);
    let thief = join(&mut game, [215.0, 30.0]);
    assert!(until(&mut game, 240.0, |game| game.world.fleet.units.iter().any(|unit| unit.tag == soldier && unit.vid.is_some_and(|vid| crate::mechanics::traffic::crewed(&game.world, vid)))), "a police car with its crew in it is in sight");
    let unit = game.world.fleet.units.iter().position(|unit| unit.tag == soldier && unit.vid.is_some_and(|vid| crate::mechanics::traffic::crewed(&game.world, vid))).unwrap();
    let vid = game.world.fleet.units[unit].vid.unwrap();
    // Its crew climb out by their doors once it parks.
    assert!(until(&mut game, 240.0, |game| game.world.entities.all::<Officer>().filter(|(_, officer)| officer.unit == unit).count() == 2), "its officers get out");
    assert!(!crate::mechanics::traffic::crewed(&game.world, vid), "parked, with nobody at the wheel");
    // Taken then, nobody is put out of it as a civilian.
    assert!(crate::mechanics::seat(&mut game.world, thief, vid, 0), "the wheel is free to take");
    run(&mut game, 1.0);
    assert!(game.world.entities.ids::<With<Civilian>>().is_empty());
}

#[test]
fn the_police_who_get_out_are_the_ones_who_drove_and_leave_their_doors_open_and_get_back_in_by_them() {
    let mut game = game(true);
    let soldier = suspect(&mut game, [215.0, 20.0], wanted::HEAT[0], [0.0; 2], 0.0);
    let crewed = |game: &Game| game.world.fleet.units.iter().position(|unit| unit.tag == soldier && unit.vid.is_some_and(|vid| crate::mechanics::traffic::crew_of(&game.world, vid).len() == 2));
    assert!(until(&mut game, 240.0, |game| crewed(game).is_some()), "a police car with its crew of two in it is in sight");
    let unit = crewed(&game).unwrap();
    let vid = game.world.fleet.units[unit].vid.unwrap();
    let mut crew: Vec<u32> = crate::mechanics::traffic::crew_of(&game.world, vid).into_iter().map(|(id, _)| id).collect();
    crew.sort_unstable();
    // Each is drawn in their own seat.
    let seats: Vec<u8> = crew.iter().filter_map(|&id| game.seen(id).and_then(|seen| seen.seated).map(|seated| seated.seat)).collect();
    assert_eq!(seats.len(), 2, "both are drawn in the car");
    assert_ne!(seats[0], seats[1], "in seats of their own");
    // Getting out, each is seen part way through it, by an open door.
    let mut stepping = false;
    let out = until(&mut game, 240.0, |game| {
        stepping |= crew.iter().any(|&id| game.seen(id).and_then(|seen| seen.seated).is_some_and(|seated| seated.step > 50 && seated.step < 200));
        game.world.entities.all::<Officer>().filter(|(_, officer)| officer.unit == unit).count() == 2
    });
    assert!(out, "its officers get out");
    assert!(stepping, "climbing out by their doors, not put beside the car at once");
    let mut officers: Vec<u32> = game.world.entities.all::<Officer>().filter(|(_, officer)| officer.unit == unit).map(|(id, _)| id).collect();
    officers.sort_unstable();
    assert_eq!(officers, crew, "the officers are the two who sat in the car");
    let doors = game.world.entities.of::<Vehicle>(vid).doors;
    assert_eq!(doors.open, 0b11, "both doors are left standing open");
    let seen = game.seen(vid).and_then(|seen| seen.doors).expect("its doors are told of");
    assert!(seen.left > 200 && seen.right > 200, "and are seen to be: {seen:?}");
    // Wanted no more, they walk back and climb in by their doors, its crew again, and shut them.
    wanted::clear(&mut game.world, soldier);
    assert!(until(&mut game, 60.0, |game| crate::mechanics::traffic::crew_of(&game.world, vid).len() == 2), "both are back in");
    let mut back: Vec<u32> = crate::mechanics::traffic::crew_of(&game.world, vid).into_iter().map(|(id, _)| id).collect();
    back.sort_unstable();
    assert_eq!(back, crew, "the same two");
    assert!(until(&mut game, 3.0, |game| game.world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle.doors.open == 0)), "with the doors pulled shut behind them");
}

fn lethal(game: &mut Game) -> (u32, Vec<u32>) {
    let soldier = join(game, [215.0, 20.0]);
    let victims: Vec<u32> = (0..2).map(|n| civilian(game, [205.0 + n as f32 * 3.0, 0.0])).collect();
    for &victim in &victims {
        shoot(game, soldier, victim, PLAYER_MAX_HP);
    }
    let third = civilian(game, [212.0, 3.0]);
    shoot(game, soldier, third, 20);
    assert!(stars(game, soldier) >= 3, "{} stars", stars(game, soldier));
    assert!(until(game, 240.0, |game| officers(game).len() >= 2), "officers got out");
    (soldier, officers(game))
}

#[test]
fn officers_after_a_lethal_suspect_fight_as_soldiers_one_going_round_the_side() {
    let mut game = game(true);
    let (soldier, out) = lethal(&mut game);
    let (mut roles, mut knelt, mut armed) = (Vec::new(), false, false);
    for _ in 0..(10 * TICK_RATE) {
        if !game.world.entities.alive(soldier) {
            break;
        }
        hold(&mut game, soldier, [215.0, 20.0]);
        game.world.entities.get_mut::<crate::entities::Health>(soldier).unwrap().hp = PLAYER_MAX_HP;
        game.step();
        for &officer in &out {
            let Some(mind) = game.world.entities.get::<crate::game::Bot>(officer) else { continue };
            armed |= game.world.entities.of::<Officer>(officer).armed;
            roles.push(mind.role);
            let buttons = game.world.entities.of::<Player>(officer).input.buttons();
            knelt |= buttons & BUTTON_CROUCH != 0;
        }
    }
    assert!(armed, "a soldier's mind took over");
    assert!(roles.iter().any(|&role| role != crate::game::Role::Attacker), "one of the two flanked or kept their head down: {roles:?}");
    assert!(knelt, "they got down to fire");
}

#[test]
fn an_officer_never_takes_anyone_but_their_suspect_for_a_target_however_near() {
    let mut game = game(true);
    let (soldier, out) = lethal(&mut game);
    // Someone else stands right by the officers, with a gun.
    let at = game.world.entities.of::<Player>(out[0]).character.position;
    let bystander = join(&mut game, [at[0] + 3.0, at[2]]);
    for _ in 0..(5 * TICK_RATE) {
        hold(&mut game, soldier, [215.0, 20.0]);
        hold(&mut game, bystander, [at[0] + 3.0, at[2]]);
        game.world.entities.get_mut::<crate::entities::Health>(soldier).unwrap().hp = PLAYER_MAX_HP;
        crate::mechanics::bots::make_shot(&mut game.world, [at[0] + 3.0, 1.5, at[2]], Some(bystander), [1.0, 0.0, 0.0]);
        game.step();
        for &officer in &out {
            if let Some(mind) = game.world.entities.get::<crate::game::Bot>(officer) {
                assert!(mind.sight.target.is_none_or(|target| target == soldier), "an officer turned on someone else");
            }
        }
    }
}

#[test]
fn officers_do_not_count_against_the_bot_limit_and_a_dead_one_does_not_come_back() {
    let mut game = game(true);
    let (soldier, out) = lethal(&mut game);
    let room = (0..crate::game::MAX_BOTS).filter(|&n| game.add_bot([-400.0 + n as f32 * 2.0, -400.0], 1, vec![[-400.0, -400.0]]).is_some()).count();
    assert_eq!(room, crate::game::MAX_BOTS, "every bot a scenario wants still has its place");
    let officer = out[0];
    damage_entity(&mut game.world, officer, Harm::new(PLAYER_MAX_HP, Some(soldier), WEAPON_RIFLE).head(true));
    run(&mut game, 15.0);
    assert!(!game.world.entities.has::<Player>(officer), "the dead officer is gone, not back at home");
}

#[test]
fn a_police_car_chasing_across_a_field_goes_round_a_house_in_its_way() {
    // A house 16 m square in the field between the road on the east side of the block and the player.
    let mut game = game(true);
    let v = |x: i32, y: i32, z: i32| [x * 100, y * 100, z * 100];
    let vertices = vec![v(140, 0, -8), v(156, 0, -8), v(156, 0, 8), v(140, 0, 8), v(140, 6, -8), v(156, 6, -8), v(156, 6, 8), v(140, 6, 8)];
    let triangles = vec![[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
    let mut map = game.world.map.clone();
    map.collision = shared::world::CollisionMesh { vertices, triangles };
    game = Game::new(map, 0).unwrap();
    (game.world.fleet.ambulance, game.world.fleet.police) = (true, true);
    let soldier = join(&mut game, [110.0, 0.0]);
    let id = civilian(&mut game, [114.0, 4.0]);
    shoot(&mut game, soldier, id, 20);
    let (mut chased, mut tries) = (false, 0);
    for _ in 0..(300 * TICK_RATE) {
        if !officers(&game).is_empty() {
            break;
        }
        hold(&mut game, soldier, [110.0, 0.0]);
        game.step();
        chased |= police(&game, soldier).contains(&Duty::Chasing);
        if let Some(vid) = game.world.fleet.units.iter().find(|unit| unit.tag == soldier).and_then(|unit| unit.vid) {
            let [x, _, z] = game.world.physics.car_pose(vid).unwrap().position;
            // Its middle no nearer the walls than half its width: a scrape, at the most.
            assert!(!((139.5..156.5).contains(&x) && (-8.5..8.5).contains(&z)), "into the house at {x}, {z}");
            tries = tries.max(game.mechanic::<crate::mechanics::Traffic>().map_or(0, |traffic| traffic.tries_of(vid)));
        }
    }
    assert!(chased, "it left the road after them");
    assert_eq!(officers(&game).len(), 2, "and got close enough for its officers to get out");
    let car = game.world.fleet.units.iter().find(|unit| unit.tag == soldier).unwrap();
    assert!(distance_2d(car.at, [110.0, 0.0, 0.0]) < 25.0, "beside them: {:?}", car.at);
    assert_eq!(tries, 0, "without having to back out of anything");
}

/// A player at `[x, z]` the police are after with `heat`, seen there, going at `heading` (m/s over
/// the ground) and looking at `yaw` (radians, 0 along -z).
fn suspect(game: &mut Game, [x, z]: [f32; 2], heat: f32, heading: [f32; 2], yaw: f32) -> u32 {
    let id = join(game, [x, z]);
    game.world.entities.get_mut::<Player>(id).unwrap().input.yaw = shared::quant::to_mrad(yaw);
    wanted::charge(&mut game.world, id, heat, None);
    game.world.entities.get_mut::<Wanted>(id).unwrap().heading = heading;
    id
}

/// Moves suspect `id` to `[x, z]` and has the police see them there.
fn seen(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    hold(game, id, [x, z]);
    let mut wanted = game.world.entities.get_mut::<Wanted>(id).unwrap();
    (wanted.seen_at, wanted.unseen) = ([x, sim::STANDING_Y, z], 0);
}

#[test]
fn at_five_stars_eight_police_cars_are_after_you_within_five_seconds() {
    let mut game = game(true);
    let soldier = suspect(&mut game, [215.0, 20.0], 150.0, [0.0; 2], 0.0);
    for _ in 0..(5 * TICK_RATE) {
        hold(&mut game, soldier, [215.0, 20.0]);
        game.step();
    }
    assert_eq!(police(&game, soldier).len(), 8);
}

#[test]
fn a_fresh_police_car_starts_ahead_of_a_suspect_going_away_and_out_of_their_view() {
    let mut game = game(true);
    // On the road along the north of the block, going east at 20 m/s and looking north.
    let soldier = suspect(&mut game, [100.0, 150.0], 30.0, [20.0, 0.0], 0.0);
    assert!(until(&mut game, 3.0, |game| !police(game, soldier).is_empty()), "a car was sent");
    let car = game.world.fleet.units.iter().find(|unit| unit.tag == soldier).unwrap();
    let off = distance_2d(car.at, [100.0, 0.0, 150.0]);
    assert!(car.at[0] > 200.0 && (100.0..460.0).contains(&off), "ahead of them, near: {:?}", car.at);
    assert!(crate::mechanics::traffic::hidden(&game.world, car.at), "where they cannot see it");
    // Looking the way they go, across open ground, it cannot start in front of them.
    let mut game = self::game(true);
    let soldier = suspect(&mut game, [100.0, 150.0], 30.0, [20.0, 0.0], -std::f32::consts::FRAC_PI_2);
    assert!(until(&mut game, 3.0, |game| !police(game, soldier).is_empty()), "a car was sent");
    let car = game.world.fleet.units.iter().find(|unit| unit.tag == soldier).unwrap();
    assert!(crate::mechanics::traffic::hidden(&game.world, car.at), "{:?}", car.at);
}

#[test]
fn a_police_car_left_far_behind_is_called_off_and_another_comes_from_near_them() {
    let mut game = game(true);
    let soldier = suspect(&mut game, [100.0, 150.0], 30.0, [0.0; 2], 0.0);
    for _ in 0..(5 * TICK_RATE) {
        seen(&mut game, soldier, [100.0, 150.0]);
        game.step();
    }
    let first: Vec<usize> = (0..game.world.fleet.units.len()).filter(|&unit| game.world.fleet.units[unit].tag == soldier).collect();
    assert_eq!(first.len(), 2);
    // Off to a field no road comes near: the cars can get no nearer, and are called off.
    for _ in 0..(15 * TICK_RATE) {
        seen(&mut game, soldier, [-500.0, 500.0]);
        game.step();
    }
    assert!(first.iter().all(|&unit| game.world.fleet.units[unit].tag != soldier), "{:?}", game.world.fleet.units);
    assert!(officers(&game).is_empty(), "nobody stayed behind on foot");
    // Back by a road far from where they were, cars come for them from near by.
    for _ in 0..(3 * TICK_RATE) {
        seen(&mut game, soldier, [-400.0, -150.0]);
        game.step();
    }
    let near = game.world.fleet.units.iter().filter(|unit| unit.tag == soldier).map(|unit| distance_2d(unit.at, [-400.0, 0.0, -150.0])).collect::<Vec<f32>>();
    assert!(!near.is_empty() && near.iter().all(|&off| off < 460.0), "{near:?}");
}

#[test]
fn officers_left_far_behind_where_nobody_sees_them_are_gone() {
    let mut game = game(true);
    let (soldier, out) = lethal(&mut game);
    for _ in 0..(2 * TICK_RATE) {
        seen(&mut game, soldier, [-400.0, -150.0]);
        game.step();
    }
    assert!(out.iter().all(|&officer| !game.world.entities.has::<Officer>(officer)), "they were gone at once");
}

#[test]
fn a_chasing_police_car_boosts_after_a_suspect_driving_off() {
    let mut game = game(true);
    // Driving east across the open, south of the block.
    let jeep = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [-500.0, 1.0, -100.0], -std::f32::consts::FRAC_PI_2);
    let soldier = join(&mut game, [-500.0, -100.0]);
    assert!(crate::mechanics::seat(&mut game.world, soldier, jeep, 0));
    wanted::charge(&mut game.world, soldier, 60.0, None);
    let (mut chased, mut boosted) = (false, false);
    for _ in 0..(30 * TICK_RATE) {
        let seq = game.world.tick + 1;
        let frame = shared::protocol::InputFrame { seq, forward: 127, view: seq as u16, yaw: shared::quant::to_mrad(-std::f32::consts::FRAC_PI_2), ..Default::default() };
        game.handle(Command::Input(soldier, seq.saturating_sub(2), vec![frame]));
        game.step();
        for unit in game.world.fleet.units.iter().filter(|unit| unit.tag == soldier && unit.duty == Duty::Chasing) {
            chased = true;
            boosted |= unit.vid.and_then(|vid| game.world.entities.get::<crate::game::Autopilot>(vid)).is_some_and(|autopilot| autopilot.0.boost);
        }
        if boosted {
            break;
        }
    }
    assert!(chased, "a police car went after them");
    assert!(boosted, "on the boost");
}

#[test]
fn from_three_stars_unseen_police_cars_search_ahead_of_where_the_suspect_was_going_and_spread_out() {
    let mut game = game(true);
    let soldier = suspect(&mut game, [0.0, 0.0], 30.0, [10.0, 0.0], 0.0);
    let look = |game: &Game, unit: usize| search(game.world.entities.of::<Wanted>(soldier), spread(unit));
    game.world.entities.get_mut::<Wanted>(soldier).unwrap().unseen = 10 * TICK_RATE;
    assert!(distance_2d(look(&game, 1), [0.0; 3]) < 0.01, "at two stars, where they were seen");
    game.world.entities.get_mut::<Wanted>(soldier).unwrap().heat = 60.0;
    let spots: Vec<[f32; 3]> = (0..4).map(|unit| look(&game, unit)).collect();
    for spot in &spots {
        assert!((spot[0] - 100.0).abs() < 0.01 && spot[2].abs() <= SEARCH_SPREAD, "as far on as they could have gone in 10 s: {spot:?}");
    }
    let sides: Vec<f32> = spots.iter().map(|spot| spot[2]).collect();
    assert!(sides.iter().enumerate().all(|(n, a)| sides[n + 1..].iter().all(|b| (a - b).abs() > 10.0)), "each to its own side: {sides:?}");
    game.world.entities.get_mut::<Wanted>(soldier).unwrap().unseen = 100 * TICK_RATE;
    assert!((look(&game, 0)[0] - SEARCH_AHEAD).abs() < 0.01, "and no further than that");
}

#[test]
fn at_five_stars_a_suspect_must_stay_out_of_sight_for_two_minutes() {
    let mut game = game(true);
    let soldier = suspect(&mut game, [215.0, 20.0], 150.0, [0.0; 2], 0.0);
    // Away in a field no road comes near.
    let hide = |game: &mut Game, seconds: f32| {
        for _ in 0..(seconds * TICK_RATE as f32) as u32 {
            hold(game, soldier, [-500.0, 500.0]);
            game.step();
        }
    };
    hide(&mut game, 100.0);
    assert_eq!(stars(&game, soldier), 5, "still wanted after 100 s");
    hide(&mut game, 25.0);
    assert_eq!(stars(&game, soldier), 0);
}

/// What a tick costs on a real world with one player driving about for a minute in a jeep with
/// traffic about: wanted by nobody, and with five stars.
/// `WORLD=$PWD/worlds/home/world.msgpack cargo test --release -p server -- --ignored --nocapture police_on_a_real_world`
#[test]
#[ignore]
fn police_on_a_real_world() {
    let path = std::env::var_os("WORLD").map(std::path::PathBuf::from).expect("WORLD");
    for heat in [0.0, 150.0] {
        let (map, _) = crate::read_world(&path).unwrap();
        let [sx, sz] = map.spawns[0].map(|v| v as f32 / 100.0);
        let mut game = Game::new(map, 0).unwrap();
        game.world.traffic = 1.0;
        (game.world.fleet.ambulance, game.world.fleet.police) = (true, true);
        let y = game.world.map.ground.height_at(sx, sz) + 2.0;
        let jeep = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_JEEP, [sx, y, sz], 0.0);
        let id = join(&mut game, [sx, sz]);
        crate::mechanics::seat(&mut game.world, id, jeep, 0);
        let (mut sum, mut worst, mut cars, mut officers_out) = (std::time::Duration::ZERO, std::time::Duration::ZERO, 0, 0);
        let ticks = 60 * TICK_RATE;
        for tick in 0..ticks {
            if heat > 0.0 {
                if game.world.entities.has::<Wanted>(id) {
                    game.world.entities.get_mut::<Wanted>(id).unwrap().heat = heat;
                } else {
                    wanted::charge(&mut game.world, id, heat, None);
                }
            }
            let seq = game.world.tick + 1;
            let strafe = ((tick as f32 / TICK_RATE as f32 * 0.4).sin() * 80.0) as i8;
            let frame = shared::protocol::InputFrame { seq, forward: 127, strafe, view: seq as u16, ..Default::default() };
            game.handle(Command::Input(id, seq.saturating_sub(2), vec![frame]));
            let start = std::time::Instant::now();
            game.step();
            let took = start.elapsed();
            (sum, worst) = (sum + took, worst.max(took));
            cars = cars.max(game.world.fleet.units.iter().filter(|unit| unit.tag == id && unit.vid.is_some()).count());
            officers_out = officers_out.max(officers(&game).len());
        }
        println!("heat {heat}: {:?} a tick on average, {worst:?} the worst; at most {cars} police cars in sight and {officers_out} officers out", sum / ticks);
    }
}

#[test]
fn one_officer_of_a_car_carries_a_carl_gustaf_and_fires_it_at_a_suspect_who_takes_to_a_tank() {
    let mut game = game(true);
    let (soldier, out) = lethal(&mut game);
    let tube = |game: &Game, officer: u32| game.world.entities.get::<Player>(officer).and_then(|player| player.hands.hotbar.iter().find(|slot| slot.item == shared::items::ITEM_CARL_GUSTAF).map(|slot| slot.count));
    let carriers: Vec<u32> = out.iter().copied().filter(|&officer| tube(&game, officer).is_some()).collect();
    assert_eq!(carriers.len(), 1, "one of the two carries one");
    let rounds = tube(&game, carriers[0]).unwrap();
    let y = game.world.map.ground.height_at(215.0, 20.0) + 1.0;
    let tank = crate::mechanics::for_tests::add_vehicle(&mut game.world, KIND_TANK, [215.0, y, 20.0], 0.0);
    assert!(crate::mechanics::seat(&mut game.world, soldier, tank, 0));
    let mut fired = false;
    for _ in 0..(30 * TICK_RATE) {
        game.step();
        if let Some(mut health) = game.world.entities.get_mut::<crate::entities::Health>(tank) {
            health.hp = health.max;
        }
        if tube(&game, carriers[0]).is_some_and(|left| left < rounds) {
            fired = true;
            break;
        }
    }
    assert!(fired, "they fired it at the tank");
}

#[test]
fn police_cars_arriving_together_pull_over_where_they_are_and_their_officers_get_out_at_once() {
    let mut game = game(true);
    // Stood on the road along the north of the block, looking north.
    let soldier = suspect(&mut game, [100.0, 150.0], 150.0, [0.0; 2], 0.0);
    let (mut out_by, mut most) = (None, 0);
    // For each unit: when it came within 60 m of them, and when its officers were out.
    let mut came: Vec<(Option<u32>, Option<u32>)> = vec![(None, None); crate::mechanics::traffic::POLICE];
    for tick in 0..(40 * TICK_RATE) {
        for (unit, at) in game.world.fleet.units.iter().enumerate() {
            if at.tag != soldier {
                continue;
            }
            if came[unit].0.is_none() && distance_2d(at.at, [100.0, 0.0, 150.0]) < 60.0 {
                came[unit].0 = Some(tick);
            }
            let crew = game.world.entities.all::<Officer>().any(|(_, officer)| officer.unit == unit);
            if came[unit].1.is_none() && crew {
                came[unit].1 = Some(tick);
            }
        }
        // The officers out have nothing to shoot with: this is about how fast they get out.
        for officer in officers(&game) {
            game.world.entities.get_mut::<Player>(officer).unwrap().hands.hotbar = crate::game::hotbar_of([shared::items::ITEM_NONE; shared::items::HOTBAR_SLOTS]);
        }
        seen(&mut game, soldier, [100.0, 150.0]);
        game.step();
        most = most.max(officers(&game).len());
        if most >= 12 && out_by.is_none() {
            out_by = Some(tick / TICK_RATE);
        }
    }
    let waits: Vec<f32> = came.iter().filter_map(|&(near, out)| Some(out?.saturating_sub(near?) as f32 / TICK_RATE as f32)).collect();
    assert!(out_by.is_some_and(|seconds| seconds < 20), "twelve officers out within 20 s: {most} at the most, {out_by:?}");
    // Stopped within about 3.5 s, and a second more to climb out by the doors (DOORS.md).
    assert!(waits.len() >= 6 && waits.iter().all(|&wait| wait < 5.5), "out within 5.5 s of coming within 60 m: {waits:?}");
    // Parked each where it pulled over, not queued for one spot.
    let parked: Vec<[f32; 3]> = game.world.fleet.units.iter().filter(|unit| unit.tag == soldier && unit.duty == Duty::OnScene).map(|unit| unit.at).collect();
    assert!(parked.iter().all(|at| distance_2d(*at, [100.0, 0.0, 150.0]) < 70.0), "{parked:?}");
}
