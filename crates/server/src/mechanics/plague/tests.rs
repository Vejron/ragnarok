use super::brain::*;
use super::*;
use crate::game::{Command, Game, Outgoing};
use crate::physics::STANDING_Y;
use shared::protocol::KIND_PLAGUE;
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// No noise: every score is taken at face value.
fn steady() -> f32 {
    0.5
}

fn contact(id: u32, x: f32, z: f32, category: Category) -> Contact {
    Contact { id, position: [x, 0.0, z], category, crew: vec![id], visible: true }
}

/// A brain that has seen `contacts` once, at `tick`.
fn seen(tick: u32, contacts: &[Contact]) -> Brain {
    let mut brain = Brain::default();
    brain.perceive(tick, contacts);
    brain
}

#[test]
fn a_crowd_at_his_feet_gets_the_foot() {
    let crowd = [contact(1, 10.0, 0.0, Category::Infantry), contact(2, 12.0, 2.0, Category::Infantry), contact(3, 11.0, -2.0, Category::Infantry)];
    let mut brain = seen(0, &crowd);
    match brain.decide(0, [0.0; 3], &crowd, true, steady) {
        Decision::Stomp(point) => assert!(flat_distance(point, [11.0, 0.0, 0.0]) < 3.0, "{point:?}"),
        other => panic!("expected a stomp, got {other:?}"),
    }
}

#[test]
fn a_tank_out_of_reach_gets_an_airstrike() {
    let tank = [contact(5, 0.0, -120.0, Category::Armor)];
    let mut brain = seen(0, &tank);
    match brain.decide(0, [0.0; 3], &tank, true, steady) {
        Decision::Strike(point) => assert!(flat_distance(point, tank[0].position) < 1.0, "{point:?}"),
        other => panic!("expected an airstrike, got {other:?}"),
    }
}

#[test]
fn with_nobody_about_he_walks_on() {
    let mut brain = Brain::default();
    assert_eq!(brain.decide(0, [0.0; 3], &[], true, steady), Decision::Walk);
    // Nor at someone beyond the airstrike's range.
    let far = [contact(1, PLAGUE_STRIKE_RANGE + 50.0, 0.0, Category::Armor)];
    let mut brain = seen(0, &far);
    assert_eq!(brain.decide(0, [0.0; 3], &far, true, steady), Decision::Walk);
}

#[test]
fn an_attack_waits_out_its_cooldown() {
    let crowd = [contact(1, 10.0, 0.0, Category::Infantry)];
    let mut brain = seen(0, &crowd);
    assert!(matches!(brain.decide(0, [0.0; 3], &crowd, true, steady), Decision::Stomp(_)));
    assert_eq!(brain.decide(THINK_EVERY, [0.0; 3], &crowd, true, steady), Decision::Walk, "the foot is still on cooldown");
    let ready = PLAGUE_STOMP_WINDUP_TICKS + STOMP_COOLDOWN_TICKS;
    assert!(matches!(brain.decide(ready, [0.0; 3], &crowd, true, steady), Decision::Stomp(_)));
}

#[test]
fn hatred_fades() {
    let mut brain = Brain::default();
    brain.feel(&[PlagueWound { attacker: Some(7), amount: 1000 }], |_| Some(Category::Infantry));
    assert_eq!(brain.nemesis(), Some(7));
    // Ten half-lives on, a thousand is under one: forgotten.
    brain.forget(10 * 20 * TICK_RATE);
    assert_eq!(brain.nemesis(), None);
}

#[test]
fn whoever_hurt_him_is_who_he_goes_for() {
    // Two lone stickmen out of reach, one each side: the one who has been shooting him gets the strike.
    let pair = [contact(1, 0.0, -100.0, Category::Infantry), contact(2, 0.0, 100.0, Category::Infantry)];
    let mut brain = seen(0, &pair);
    brain.feel(&[PlagueWound { attacker: Some(2), amount: 500 }], |_| Some(Category::Infantry));
    match brain.decide(0, [0.0; 3], &pair, true, steady) {
        Decision::Strike(point) => assert!(flat_distance(point, pair[1].position) < 1.0, "{point:?}"),
        other => panic!("expected an airstrike, got {other:?}"),
    }
}

#[test]
fn what_has_been_hurting_him_is_what_he_aims_at() {
    // A vehicle is worth more than a stickman, all else equal...
    let pair = [contact(1, 0.0, -100.0, Category::Armor), contact(2, 0.0, 100.0, Category::Infantry)];
    let mut brain = seen(0, &pair);
    let Decision::Strike(point) = brain.decide(0, [0.0; 3], &pair, true, steady) else { panic!("expected an airstrike") };
    assert!(flat_distance(point, pair[0].position) < 1.0);
    // ...until infantry is what has been hurting him (someone neither of these is).
    let mut brain = seen(0, &pair);
    brain.feel(&[PlagueWound { attacker: Some(99), amount: 2000 }], |_| Some(Category::Infantry));
    assert_eq!(brain.worst(), Some(Category::Infantry));
    let Decision::Strike(point) = brain.decide(0, [0.0; 3], &pair, true, steady) else { panic!("expected an airstrike") };
    assert!(flat_distance(point, pair[1].position) < 1.0, "{point:?}");
}

#[test]
fn an_airstrike_leads_a_moving_target() {
    let mut brain = Brain::default();
    brain.perceive(0, &[contact(1, 100.0, 0.0, Category::Armor)]);
    // Half a second later it is 5 m further on: 10 m/s along +X.
    let now = [contact(1, 105.0, 0.0, Category::Armor)];
    brain.perceive(THINK_EVERY, &now);
    let Decision::Strike(point) = brain.decide(THINK_EVERY, [0.0; 3], &now, true, steady) else { panic!("expected an airstrike") };
    let expected = 105.0 + 10.0 * strike_lead_seconds();
    assert!((point[0] - expected).abs() < 0.5 && point[2].abs() < 0.5, "{point:?}, expected x {expected}");
}

#[test]
fn someone_who_hid_is_looked_for_where_they_were_last_seen() {
    let mut brain = Brain::default();
    brain.perceive(0, &[contact(1, 0.0, -100.0, Category::Infantry)]);
    // They ducked out of sight and ran on: he only knows where they went in.
    let mut hidden = contact(1, 0.0, -140.0, Category::Infantry);
    hidden.visible = false;
    brain.perceive(THINK_EVERY, std::slice::from_ref(&hidden));
    brain.feel(&[PlagueWound { attacker: Some(1), amount: 500 }], |_| Some(Category::Infantry));
    let Decision::Strike(point) = brain.decide(THINK_EVERY, [0.0; 3], &[hidden], true, steady) else { panic!("expected an airstrike") };
    assert!(flat_distance(point, [0.0, 0.0, -100.0]) < 1.0, "{point:?}");
}

#[test]
fn a_contact_never_seen_is_not_struck() {
    let mut hidden = contact(1, 0.0, -100.0, Category::Armor);
    hidden.visible = false;
    let mut brain = seen(0, std::slice::from_ref(&hidden));
    assert_eq!(brain.decide(0, [0.0; 3], &[hidden], true, steady), Decision::Walk);
}

fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[0, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.pickups.clear();
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox): (_, mpsc::Receiver<Outgoing>) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    response.try_recv().unwrap().unwrap()
}

/// A room with the Orange Plague just arrived, standing at the origin facing -Z, and one player at `[x, z]`.
fn with_plague(x: f32, z: f32) -> (Game, u32) {
    let mut game = game();
    game.add_mechanic(Box::new(OrangePlague::new(1)));
    let id = join(&mut game);
    game.step();
    let plague = game.world.plague.as_mut().expect("he arrives once someone is online");
    plague.position = [0.0, 0.0, 0.0];
    plague.yaw = 0.0;
    let player = game.world.players.get_mut(&id).unwrap();
    player.character.position = [x, STANDING_Y, z];
    (game, id)
}

fn mode(game: &Game) -> PlagueMode {
    game.world.plague.as_ref().unwrap().mode
}

#[test]
fn he_only_comes_while_someone_is_online() {
    let mut game = game();
    game.add_mechanic(Box::new(OrangePlague::new(10)));
    for _ in 0..30 {
        game.step();
    }
    assert!(game.world.plague.is_none(), "nobody to see him: he waits");
    join(&mut game);
    for _ in 0..10 {
        game.step();
    }
    let plague = game.world.plague.as_ref().expect("in town after ten ticks with someone online");
    assert_eq!(plague.hp, PLAGUE_HP);
    // Everyone is sent him, with his health full.
    let listed = game.entity_states().into_iter().find(|e| e.kind == KIND_PLAGUE).unwrap();
    assert_eq!(listed.vy, 1000);
}

#[test]
fn the_stomp_is_wound_up_before_it_lands() {
    let (mut game, id) = with_plague(0.0, -15.0);
    let mut wound_up = None;
    for tick in 0..2 * TICK_RATE {
        game.step();
        if mode(&game) == PlagueMode::Stomp {
            wound_up = Some(tick);
            break;
        }
    }
    let started = wound_up.expect("a stickman at his feet gets stomped");
    let target = game.world.plague.as_ref().unwrap().target;
    assert!(flat_distance(target, [0.0, 0.0, -15.0]) < 1.0, "aimed at them: {target:?}");
    // Not a scratch until the foot comes down; the player keeps still and takes it.
    for _ in 1..PLAGUE_STOMP_WINDUP_TICKS {
        game.step();
        assert_eq!(game.world.players[&id].hp, 100, "hurt during the wind-up");
        game.world.players.get_mut(&id).unwrap().character.position = [0.0, STANDING_Y, -15.0];
    }
    game.step();
    assert!(game.world.players[&id].hp < 100, "the stomp lands {} ticks after it began", PLAGUE_STOMP_WINDUP_TICKS + started);
}

#[test]
fn a_stickman_who_steps_out_of_the_shadow_is_spared() {
    let (mut game, id) = with_plague(0.0, -15.0);
    while mode(&game) != PlagueMode::Stomp {
        game.step();
    }
    // They run for it: well clear of the blast before the foot comes down.
    game.world.players.get_mut(&id).unwrap().character.position = [0.0, STANDING_Y, -15.0 - PLAGUE_STOMP_RADIUS - 5.0];
    for _ in 0..PLAGUE_STOMP_WINDUP_TICKS + 1 {
        game.step();
    }
    assert_eq!(game.world.players[&id].hp, 100);
}

#[test]
fn an_airstrike_comes_down_where_it_was_marked() {
    let (mut game, _) = with_plague(0.0, -120.0);
    while mode(&game) != PlagueMode::Strike {
        game.step();
    }
    let target = game.world.plague.as_ref().unwrap().target;
    assert!(flat_distance(target, [0.0, 0.0, -120.0]) < 1.0, "{target:?}");
    // Follow each rocket down: where it was last seen is where it burst.
    let mut last: std::collections::BTreeMap<u32, [f32; 3]> = Default::default();
    let mut landed = Vec::new();
    for _ in 0..PLAGUE_STRIKE_WINDUP_TICKS + PLAGUE_STRIKE_ROCKETS * PLAGUE_STRIKE_INTERVAL_TICKS + 5 * TICK_RATE {
        game.step();
        let now: std::collections::BTreeMap<u32, [f32; 3]> = game.world.rockets.iter().filter(|(_, r)| r.kind == PROJECTILE_STRIKE).map(|(&id, r)| (id, r.position)).collect();
        landed.extend(last.iter().filter(|(id, _)| !now.contains_key(id)).map(|(_, &at)| at));
        last = now;
    }
    assert_eq!(landed.len(), PLAGUE_STRIKE_ROCKETS as usize, "every rocket came down");
    for at in landed {
        // Within the marked circle, give or take the last tick's fall.
        assert!(flat_distance(at, target) <= PLAGUE_STRIKE_RADIUS + 1.5, "{at:?} is outside the circle at {target:?}");
    }
}

#[test]
fn a_bullet_in_the_head_counts_double() {
    let (mut game, id) = with_plague(0.0, -100.0);
    // Between his eyes, at the bridge of his nose.
    let [_, y, _] = shared::plague::HEAD;
    let origin = [0.0, y, -60.0];
    let (t, part) = first_plague_hit(&game.world, origin, [0.0, 0.0, 1.0], 100.0, id).expect("the ray meets his head");
    assert_eq!(part, PlaguePart::Head);
    assert!((t - (60.0 - shared::plague::HEAD_RADIUS)).abs() < 0.1, "{t}");
    let legs = first_plague_hit(&game.world, [0.0, 5.0, -60.0], [0.0, 0.0, 1.0], 100.0, id);
    assert!(legs.is_none(), "between his legs is open air");
    let leg = first_plague_hit(&game.world, [PLAGUE_FOOT_SPREAD, 5.0, -60.0], [0.0, 0.0, 1.0], 100.0, id).unwrap();
    assert_eq!(leg.1, PlaguePart::Leg);
    damage_plague(&mut game.world, 10, Some(id), PlaguePart::Head, 0, origin);
    damage_plague(&mut game.world, 10, Some(id), PlaguePart::Leg, 0, origin);
    assert_eq!(game.world.plague.as_ref().unwrap().hp, PLAGUE_HP - 30);
}

#[test]
fn blasts_hurt_him_but_his_own_do_not() {
    let (mut game, id) = with_plague(0.0, -100.0);
    let point = [0.0, 1.0, -3.0];
    blast(&mut game.world, Blast { point, radius: 5.0, damage: 100.0, knock: 0.0, dir: [0.0, 0.0, 1.0], attacker: Some(id), weapon: shared::protocol::WEAPON_SHELL });
    let hurt = PLAGUE_HP - game.world.plague.as_ref().unwrap().hp;
    assert!(hurt > 50, "a shell at his shins hurts: {hurt}");
    blast(&mut game.world, Blast { point, radius: 5.0, damage: 100.0, knock: 0.0, dir: [0.0, 0.0, 1.0], attacker: None, weapon: WEAPON_STOMP });
    assert_eq!(PLAGUE_HP - game.world.plague.as_ref().unwrap().hp, hurt, "his own stomp leaves him be");
}

#[test]
fn shooting_the_phone_hand_drops_the_call() {
    let (mut game, id) = with_plague(0.0, -120.0);
    while mode(&game) != PlagueMode::Strike {
        game.step();
    }
    damage_plague(&mut game.world, 5, Some(id), PlaguePart::Hand, 0, [9.0, 17.0, 0.0]);
    for _ in 0..PLAGUE_STRIKE_WINDUP_TICKS + PLAGUE_STRIKE_ROCKETS * PLAGUE_STRIKE_INTERVAL_TICKS {
        game.step();
        assert!(!game.world.rockets.values().any(|r| r.kind == PROJECTILE_STRIKE), "no rockets come");
    }
}

#[test]
fn shot_down_he_topples_and_leaves() {
    let (mut game, id) = with_plague(0.0, -200.0);
    damage_plague(&mut game.world, PLAGUE_HP, Some(id), PlaguePart::Body, 0, [0.0; 3]);
    assert_eq!(mode(&game), PlagueMode::Toppling);
    let listed = game.entity_states().into_iter().find(|e| e.kind == KIND_PLAGUE).unwrap();
    assert_ne!(listed.flags & shared::protocol::FLAG_DEAD, 0, "clients are told he is going down");
    // Nothing more hurts him on the way down.
    damage_plague(&mut game.world, 10, Some(id), PlaguePart::Body, 0, [0.0; 3]);
    for _ in 0..TOPPLE_TICKS {
        game.step();
    }
    assert!(game.world.plague.is_none(), "gone once he has hit the ground");
}

/// Joins a player whose messages are kept, to read what was said.
fn join_listening(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// The barks among what `inbox` has been sent: `(trigger, subject)`.
fn barks(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<(u8, u32)> {
    use shared::protocol::{Bark, S_BARK, decode, peek_tag};
    let mut out = Vec::new();
    while let Ok(message) = inbox.try_recv() {
        if let Outgoing::Reliable(bytes) = message
            && peek_tag(&bytes) == Some(S_BARK)
        {
            let Bark(_, trigger, _, subject) = decode(&bytes).unwrap();
            out.push((trigger, subject));
        }
    }
    out
}

#[test]
fn he_announces_himself_and_fires_whoever_he_stomps() {
    use shared::protocol::{BARK_ARRIVE, BARK_FIRED};
    let mut game = game();
    game.add_mechanic(Box::new(OrangePlague::new(1)));
    let (id, mut inbox) = join_listening(&mut game);
    game.step();
    assert_eq!(barks(&mut inbox), vec![(BARK_ARRIVE, 0)]);
    let plague = game.world.plague.as_mut().unwrap();
    plague.position = [0.0, 0.0, 0.0];
    game.world.players.get_mut(&id).unwrap().character.position = [0.0, STANDING_Y, -15.0];
    for _ in 0..3 * TICK_RATE {
        game.step();
    }
    assert!(!game.world.players[&id].alive(), "the stomp killed them");
    assert!(barks(&mut inbox).contains(&(BARK_FIRED, id)), "and he said so, by name");
}

/// Where his face part `bit` is in the world now, and which way he faces it from in front.
fn face_point(game: &Game, bit: u8) -> [f32; 3] {
    let plague = game.world.plague.as_ref().unwrap();
    let part = body_of(plague).into_iter().find(|c| c.part == PlaguePart::Face(bit)).unwrap();
    std::array::from_fn(|i| (part.a[i] + part.b[i]) / 2.0)
}

#[test]
fn a_shot_in_the_eye_hits_the_eye_and_puts_it_out() {
    use shared::protocol::FACE_EYE_LEFT;
    let (mut game, id) = with_plague(0.0, -100.0);
    let [x, y, _] = face_point(&game, FACE_EYE_LEFT);
    let (_, part) = first_plague_hit(&game.world, [x, y, -60.0], [0.0, 0.0, 1.0], 100.0, id).unwrap();
    assert_eq!(part, PlaguePart::Face(FACE_EYE_LEFT));
    damage_plague(&mut game.world, 100, Some(id), part, 0, [x, y, -3.0]);
    let plague = game.world.plague.as_ref().unwrap();
    assert_eq!(plague.hp, PLAGUE_HP - 100 * FACE_SCALE[0], "an eye counts four times over");
    assert_eq!(plague.face_lost, 0, "still there");
    damage_plague(&mut game.world, FACE_HP[0], Some(id), part, 0, [x, y, -3.0]);
    let plague = game.world.plague.as_ref().unwrap();
    assert_eq!(plague.face_lost, FACE_EYE_LEFT, "shot out");
    // Clients are told, in the top bits of his health.
    let listed = game.entity_states().into_iter().find(|e| e.kind == KIND_PLAGUE).unwrap();
    assert_eq!(listed.vy / shared::protocol::PLAGUE_FACE_SCALE, i16::from(FACE_EYE_LEFT));
    // A gone eye is just more head.
    let before = game.world.plague.as_ref().unwrap().hp;
    damage_plague(&mut game.world, 10, Some(id), part, 0, [x, y, -3.0]);
    assert_eq!(before - game.world.plague.as_ref().unwrap().hp, 10 * HEAD_SCALE);
}

#[test]
fn with_his_left_eye_out_he_cannot_see_to_his_left() {
    use shared::protocol::FACE_EYE_LEFT;
    let (mut game, _) = with_plague(-60.0, -60.0);
    let ahead_left = |game: &Game| contacts(&game.world, game.world.plague.as_ref().unwrap()).iter().any(|c| c.visible);
    assert!(ahead_left(&game), "seen with both eyes");
    game.world.plague.as_mut().unwrap().face_lost = FACE_EYE_LEFT;
    assert!(!ahead_left(&game), "not with the left one gone");
    game.world.players.values_mut().for_each(|p| p.character.position = [60.0, STANDING_Y, -60.0]);
    assert!(ahead_left(&game), "the right side he still sees");
}

#[test]
fn shot_from_out_of_sight_he_hears_where_it_came_from_unless_deaf() {
    use shared::protocol::{FACE_EAR_LEFT, FACE_EAR_RIGHT, FACE_EYE_LEFT, FACE_EYE_RIGHT};
    let (mut game, id) = with_plague(0.0, -120.0);
    // Blind, so the shooter is out of sight; they hurt him.
    let plague = game.world.plague.as_mut().unwrap();
    plague.face_lost = FACE_EYE_LEFT | FACE_EYE_RIGHT;
    let strikes = |game: &mut Game| {
        let mut seen = None;
        for _ in 0..2 * TICK_RATE {
            game.world.plague.as_mut().unwrap().wounds.push(PlagueWound { attacker: Some(id), amount: 200 });
            game.step();
            if mode(game) == PlagueMode::Strike {
                seen = Some(game.world.plague.as_ref().unwrap().target);
                break;
            }
        }
        seen
    };
    let target = strikes(&mut game).expect("he heard them and called it in");
    assert!(flat_distance(target, [0.0, 0.0, -120.0]) < 2.0, "{target:?}");
    // Deaf as well: nothing to go on.
    let (mut game, id2) = with_plague(0.0, -120.0);
    assert_eq!(id, id2);
    game.world.plague.as_mut().unwrap().face_lost = FACE_EYE_LEFT | FACE_EYE_RIGHT | FACE_EAR_LEFT | FACE_EAR_RIGHT;
    assert_eq!(strikes(&mut game), None);
}

#[test]
fn with_his_mouth_shot_shut_he_calls_no_airstrikes_and_says_so() {
    use shared::protocol::{BARK_GAGGED, FACE_MOUTH};
    let tank = [contact(5, 0.0, -120.0, Category::Armor)];
    let mut brain = seen(0, &tank);
    assert_eq!(brain.decide(0, [0.0; 3], &tank, false, steady), Decision::Walk);

    let mut game = game();
    game.add_mechanic(Box::new(OrangePlague::new(1)));
    let (id, mut inbox) = join_listening(&mut game);
    game.step();
    game.world.plague.as_mut().unwrap().position = [0.0, 0.0, 0.0];
    let mouth = face_point(&game, FACE_MOUTH);
    damage_plague(&mut game.world, FACE_HP[4], Some(id), PlaguePart::Face(FACE_MOUTH), 0, mouth);
    for _ in 0..THINK_EVERY + 1 {
        game.step();
    }
    assert!(barks(&mut inbox).contains(&(BARK_GAGGED, id)));
}

#[test]
fn he_is_solid_to_walk_into_but_not_to_rays() {
    let (mut game, id) = with_plague(0.0, -100.0);
    game.step();
    // A ray through his shin meets nothing in the physics world: his hit capsules decide.
    let shin = [PLAGUE_FOOT_SPREAD, 5.0, -30.0];
    assert_eq!(game.world.physics.cast_ray(shin, [0.0, 0.0, 1.0], 60.0, 0), None);
    // A stickman put inside his leg is put back outside it.
    game.world.players.get_mut(&id).unwrap().character.position = [PLAGUE_FOOT_SPREAD + 0.5, STANDING_Y, 0.0];
    game.step();
    let at = game.world.players[&id].character.position;
    let leg = body_of(game.world.plague.as_ref().unwrap()).into_iter().find(|c| c.part == PlaguePart::Leg).unwrap();
    assert!(shared::plague::distance_to(&leg, at) > 0.5, "out of his leg: {at:?}");
    assert!(game.world.physics.has_giant(), "he is in the physics world as solid matter");
}
