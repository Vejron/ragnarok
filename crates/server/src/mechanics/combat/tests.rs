use super::*;
use crate::entities::Health;
use crate::game::{Command, Game, Outgoing};
use sim::STANDING_Y;
use shared::protocol::{InputFrame, Snapshot, WEAPON_BULLET, WEAPON_FLARE, WEAPON_REVOLVER, WEAPON_PELLET, WEAPON_SHOTGUN, decode};
use shared::quant::to_mrad;
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
    // Loose items would take a test's use button.
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// The camera is above and to the right of the eye, so a shooter aims a little down to put the
/// crosshair on a target 20 m away at the same height.
const AIM_20M: f32 = -0.075;

/// How a player aims with `slot` in hand: a gun's crosshair on a target 20 m away, anything else level.
fn aim_for(game: &Game, id: u32, slot: usize) -> f32 {
    let item = game.world.players[&id].hotbar[slot].item;
    if def(item).is_some_and(|d| d.kind == ItemKind::Gun) { AIM_20M } else { 0.0 }
}

/// Feeds `ticks` frames of `buttons`, facing `yaw`.
fn hold(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, buttons: u16, ticks: u32) {
    let aim = aim_for(game, id, 0);
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: to_mrad(yaw), buttons, pitch: to_mrad(aim), view: 0, slot: 0 }]));
        game.step();
    }
}

/// Lets a bullet land: a round takes a few ticks to cross 20 m, so tests that fire look a moment later.
fn settle(game: &mut Game, id: u32, seq: &mut u32) {
    hold(game, id, seq, 0.0, 0, 12);
}

/// Two players 20 m apart on flat ground, clear of the car that parks on the spawn point, the
/// first facing the second (-Z), both settled. The target stands where the shooter's shoulder
/// camera looks, as a player turns until the crosshair is on someone.
fn duel() -> (Game, u32, u32, u32, u32) {
    let mut game = game();
    let (shooter, _a) = join(&mut game);
    let (target, _b) = join(&mut game);
    std::mem::forget((_a, _b));
    game.world.players.get_mut(&shooter).unwrap().character.position = [100.0, STANDING_Y, 0.0];
    game.world.players.get_mut(&target).unwrap().character.position = [100.0 + ON_FOOT_SHOULDER, STANDING_Y, -20.0];
    // The shooter carries an AK5 in slot 0, which is where `hold` keeps the selection.
    give(&mut game, shooter, 0, ITEM_AK5, 1);
    let (mut sa, mut sb) = (0, 0);
    hold(&mut game, shooter, &mut sa, 0.0, 0, 3);
    hold(&mut game, target, &mut sb, 0.0, 0, 1);
    (game, shooter, target, sa, sb)
}

fn give(game: &mut Game, id: u32, slot: usize, item: u8, count: u8) {
    let mag = def(item).map_or(0, |d| d.mag);
    game.world.players.get_mut(&id).unwrap().hotbar[slot] = ItemSlot { item, count, ammo: mag };
}

/// One tick of `buttons` with `slot` selected, looking `pitch` radians up.
fn press(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, slot: u8, pitch: f32) {
    *seq += 1;
    game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: to_mrad(pitch), view: 0, slot }]));
    game.step();
}

fn ak5() -> &'static ItemDef {
    def(ITEM_AK5).unwrap()
}

#[test]
fn a_ray_meets_a_capsule_on_its_side_and_misses_wide() {
    let centre = [0.0, 1.0, -10.0];
    let t = ray_capsule([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0).unwrap();
    assert!((t - 9.65).abs() < 1e-4, "{t}");
    assert_eq!(ray_capsule([0.5, 1.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0), None, "0.5 m to the side");
    assert_eq!(ray_capsule([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 5.0), None, "out of range");
    assert_eq!(ray_capsule([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], centre, 0.5, 0.35, 100.0), None, "behind");
    // Over the head is over the head; just inside the rounded cap is a hit.
    assert_eq!(ray_capsule([0.0, 2.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0), None);
    assert!(ray_capsule([0.0, 1.8, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0).is_some());
}

#[test]
fn the_ak5_hurts_what_it_points_at_and_credits_the_shooter() {
    let (mut game, shooter, target, mut seq, _) = duel();
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 3);
    settle(&mut game, shooter, &mut seq);
    let hp = game.world.players[&target].hp;
    assert_eq!(hp, PLAYER_MAX_HP - ak5().damage, "one shot while held for three ticks");
    assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP);

    let shots_needed = PLAYER_MAX_HP.div_ceil(ak5().damage) as u32;
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, shots_needed * ak5().cooldown_ticks);
    settle(&mut game, shooter, &mut seq);
    assert!(!game.world.players[&target].alive());
    assert_eq!(game.world.players[&shooter].kills, 1);
    assert_eq!(game.world.players[&target].deaths, 1);
}

#[test]
fn a_shot_at_the_ground_hits_nobody_and_ends_on_it() {
    let (mut game, shooter, target, _, _) = duel();
    game.world.players.get_mut(&shooter).unwrap().input = InputFrame { seq: 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: to_mrad(-0.8), view: 0, slot: 0 };
    game.world.shots.clear();
    fire_gun(&mut game.world, shooter, ak5());
    for _ in 0..10 {
        projectiles::step(&mut game.world, 1.0 / TICK_RATE as f32, Stage::Hands);
    }
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
    let end = game.world.shots.last().expect("a shot was recorded");
    assert!(end.y.abs() < 20, "ends on the ground, y = {} cm", end.y);
    assert!(end.z < 0 && end.z > -2000, "a little ahead of the shooter: z = {} cm", end.z);
}

#[test]
fn a_shot_goes_where_the_shoulder_camera_crosshair_is() {
    let (mut game, shooter, _, _, _) = duel();
    // The pivot is 0.9 m to the right; a target whose centre is on that line is under the crosshair,
    // and one 5 m off to the side is not.
    game.world.players.get_mut(&shooter).unwrap().input = InputFrame { seq: 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: 0, view: 0, slot: 0 };
    let c = game.world.players[&shooter].character;
    let pivot_y = c.position[1] + ON_FOOT_HEIGHT;
    let eye = [c.position[0], c.position[1] + EYE_HEIGHT, c.position[2]];
    let pivot = [c.position[0] + ON_FOOT_SHOULDER, pivot_y, c.position[2]];
    let dir = crosshair_dir(&game.world, shooter, 0, pivot, eye, look(0.0, 0.0), 100.0, 0);
    // Over 100 m the line from the eye crosses the camera's ray, which is 0.9 m to the right and 0.9 m above.
    let at = |t: f32| [eye[0] + dir[0] * t, eye[1] + dir[1] * t];
    assert!((at(100.0)[0] - (c.position[0] + ON_FOOT_SHOULDER)).abs() < 0.05, "x {}", at(100.0)[0]);
    assert!((at(100.0)[1] - pivot_y).abs() < 0.1, "y {}", at(100.0)[1]);
    assert!(dir[2] < -0.99, "still forwards: {dir:?}");
}

#[test]
fn a_hit_is_reported_to_the_shooter_and_the_victim_with_a_bearing() {
    let (mut game, shooter, target, _, _) = duel();
    game.world.players.get_mut(&shooter).unwrap().character.position = [0.0, STANDING_Y, 0.0];
    game.world.players.get_mut(&target).unwrap().character.position = [0.0, STANDING_Y, -20.0];
    game.world.hits.clear();
    damage(&mut game.world, target, 30, Some(shooter), WEAPON_RIFLE);
    let hits = &game.world.hits;
    assert_eq!(hits.len(), 2);
    // The shooter is straight ahead of nobody: from the victim at -20 the shooter is at +z.
    assert_eq!((hits[0].0, hits[0].1.role, hits[0].1.kind, hits[0].1.amount, hits[0].1.bearing, hits[0].1.target), (target, HIT_TAKEN, HIT_BODY, 30, to_mrad(std::f32::consts::PI), target));
    assert_eq!((hits[1].0, hits[1].1.role, hits[1].1.amount, hits[1].1.bearing, hits[1].1.target), (shooter, HIT_DEALT, 30, BEARING_NONE, target));
    // The number belongs at the target's chest.
    assert_eq!(hits[1].1.z, to_cm(-20.0));
    // The finishing blow is a kill, and amounts are what was actually lost.
    game.world.hits.clear();
    damage(&mut game.world, target, 255, Some(shooter), WEAPON_RIFLE);
    let last = &game.world.hits[1].1;
    assert_eq!((last.role, last.kind, last.amount), (HIT_DEALT, HIT_KILL, PLAYER_MAX_HP - 30));
}

#[test]
fn hurting_yourself_reports_no_dealt_hit() {
    let (mut game, shooter, _, _, _) = duel();
    game.world.hits.clear();
    damage(&mut game.world, shooter, 5, Some(shooter), WEAPON_RIFLE);
    assert_eq!(game.world.hits.len(), 1);
    assert_eq!(game.world.hits[0].1.role, HIT_TAKEN);
    assert_eq!(game.world.hits[0].1.bearing, BEARING_NONE);
}

#[test]
fn dead_players_do_not_fire_and_cannot_be_hit_until_they_respawn() {
    let (mut game, shooter, target, mut seq, mut seq_t) = duel();
    damage(&mut game.world, target, PLAYER_MAX_HP, Some(shooter), WEAPON_RIFLE);
    assert_eq!(game.world.kills.len(), 1);
    assert_eq!(game.world.kills[0].killer, shooter);
    // The dead hold fire at the shooter; nothing happens.
    hold(&mut game, target, &mut seq_t, std::f32::consts::PI, BUTTON_FIRE, 30);
    assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP);
    // And are not hit again while down.
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 30);
    assert_eq!(game.world.players[&target].deaths, 1);
    assert_eq!(game.world.players[&shooter].kills, 1);

    hold(&mut game, target, &mut seq_t, 0.0, 0, RESPAWN_TICKS);
    let target = &game.world.players[&target];
    assert!(target.alive());
    assert_eq!(target.hp, PLAYER_MAX_HP);
}

#[test]
fn falling_out_of_the_world_is_a_death_with_no_killer() {
    let (mut game, shooter, target, _, mut seq_t) = duel();
    game.world.players.get_mut(&target).unwrap().character.position[1] = KILL_Y - 1.0;
    hold(&mut game, target, &mut seq_t, 0.0, 0, 1);
    assert!(!game.world.players[&target].alive());
    assert_eq!(game.world.players[&target].deaths, 1);
    assert_eq!(game.world.players[&shooter].kills, 0);
}

#[test]
fn snapshots_carry_own_health_and_shot_events() {
    let mut game = game();
    let (shooter, mut inbox) = join(&mut game);
    game.world.players.get_mut(&shooter).unwrap().character.position = [100.0, STANDING_Y, 0.0];
    let mut seq = 0;
    give(&mut game, shooter, 0, ITEM_AK5, 1);
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 6);
    settle(&mut game, shooter, &mut seq);
    let snapshots: Vec<Snapshot> = std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Datagram(bytes) => decode(&bytes),
            Outgoing::Reliable(_) => None,
        })
        .collect();
    assert!(!snapshots.is_empty());
    assert!(snapshots.iter().all(|s| s.me.hp == PLAYER_MAX_HP));
    let last = snapshots.last().unwrap();
    assert_eq!(last.me.slots[0], ItemSlot { item: ITEM_AK5, count: 1, ammo: ak5().mag - 1 }, "own hotbar, with a round fired");
    let shots: Vec<_> = snapshots.iter().flat_map(|s| s.shots.iter()).collect();
    assert_eq!(shots.len(), 1, "one shot in six ticks at this cooldown, announced where it landed");
    assert_eq!(shots[0].weapon, WEAPON_BULLET);
    let bullets: Vec<_> = snapshots.iter().flat_map(|s| s.bullets.iter()).collect();
    assert!(!bullets.is_empty(), "a bullet leaving the gun is announced, once or twice");
    assert!(bullets.iter().all(|b| b.id == bullets[0].id && b.shooter == shooter && b.item == ITEM_AK5));
    assert_eq!(shots[0].shooter, bullets[0].id, "its end names the bullet");
}

/// The one car in the test world, and a player sitting in its driver seat.
fn driver_in_the_car(game: &mut Game) -> (u32, u32) {
    let (vid, _) = game.world.entities.all::<Vehicle>().next().expect("a car is placed at the spawn point");
    let (id, inbox) = join(game);
    std::mem::forget(inbox);
    game.world.entities.get_mut::<Vehicle>(vid).unwrap().seats[0] = Some(id);
    game.world.players.get_mut(&id).unwrap().seat = Some(crate::game::Seat { vehicle: vid, index: 0, remote: false });
    (vid, id)
}

#[test]
fn a_rocket_flies_and_bursts_on_the_target_and_hurts_it() {
    let (mut game, shooter, target, mut seq, _) = duel();
    // A rocket flies along the look direction from the eye, so it is the target that stands in its line.
    game.world.players.get_mut(&target).unwrap().character.position[0] = 100.0;
    game.world.players.get_mut(&shooter).unwrap().input = InputFrame { seq: 0, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 };
    fire_rocket(&mut game.world, shooter);
    // 20 m at 45 m/s is under half a second.
    for _ in 0..40 {
        seq += 1;
        game.handle(Command::Input(shooter, 0, vec![InputFrame { seq, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
        if game.world.players[&target].hp < PLAYER_MAX_HP {
            break;
        }
    }
    let hp = game.world.players[&target].hp;
    assert!(hp < PLAYER_MAX_HP, "the rocket landed");
    assert!(hp <= PLAYER_MAX_HP - 90, "a direct hit is nearly lethal: {hp}");
    assert!(game.world.in_flight().is_empty(), "a burst rocket is gone");
    assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP, "20 m away is outside the blast");
}

#[test]
fn a_rocket_into_a_wall_next_to_you_hurts_you_and_credits_nobody() {
    let (mut game, shooter, _, mut seq, _) = duel();
    // Straight down at the feet: bursts a metre or so away.
    game.world.players.get_mut(&shooter).unwrap().input = InputFrame { seq: 1, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: to_mrad(-1.4), view: 0, slot: 0 };
    fire_rocket(&mut game.world, shooter);
    for _ in 0..4 {
        seq += 1;
        game.handle(Command::Input(shooter, 0, vec![InputFrame { seq, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: to_mrad(-1.4), view: 0, slot: 0 }]));
        game.step();
    }
    assert!(game.world.players[&shooter].hp < PLAYER_MAX_HP, "hit by your own blast");
    assert_eq!(game.world.players[&shooter].kills, 0);
}

#[test]
fn the_ak5_chips_a_vehicle_and_a_wreck_kills_the_crew() {
    let mut game = game();
    let (vid, driver) = driver_in_the_car(&mut game);
    let (shooter, _inbox) = join(&mut game);
    std::mem::forget(_inbox);
    give(&mut game, shooter, 0, ITEM_AK5, 1);
    let pose = game.world.physics.car_pose(vid).unwrap();
    game.world.players.get_mut(&shooter).unwrap().character.position = [pose.position[0], STANDING_Y, pose.position[2] + 15.0];
    let mut seq = 0;
    hold(&mut game, shooter, &mut seq, 0.0, 0, 3);
    // The eye is above the roof; look a little down at the car.
    for _ in 0..3 {
        seq += 1;
        game.handle(Command::Input(shooter, 0, vec![InputFrame { seq, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: to_mrad(-0.05), view: 0, slot: 0 }]));
        game.step();
    }
    let max = shared::protocol::vehicle_hp(shared::protocol::KIND_JEEP);
    settle(&mut game, shooter, &mut seq);
    assert_eq!(game.world.entities.of::<Health>(vid).hp, max - (ak5().damage / 4).max(1));

    damage_vehicle(&mut game.world, vid, max, Some(shooter));
    assert!(game.world.entities.dead(vid));
    let vehicle = &game.world.entities.of::<Vehicle>(vid);
    assert_eq!(vehicle.seats, [None; CAR_SEATS], "the crew is out of their seats");
    assert!(!game.world.players[&driver].alive(), "and dead");
    assert_eq!(game.world.players[&shooter].kills, 1, "the credit goes to whoever wrecked it");
    assert!(game.world.shots.iter().any(|shot| shot.weapon == WEAPON_WRECK));
}

#[test]
fn a_wreck_tumbles_as_it_is_thrown_up() {
    let mut game = game();
    let (vid, _driver) = driver_in_the_car(&mut game);
    damage_vehicle(&mut game.world, vid, 255, None);
    let motion = game.world.physics.car_motion(vid).unwrap();
    let spin = motion.angvel.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!(spin > 3.0, "a wreck is sent turning over, not just straight up (spin {spin})");
    assert!(motion.linvel[1] > 5.0);
}

#[test]
fn a_wreck_cannot_be_boarded_and_comes_back_whole() {
    let mut game = game();
    let (vid, _driver) = driver_in_the_car(&mut game);
    damage_vehicle(&mut game.world, vid, 255, None);
    assert!(game.world.entities.dead(vid));
    let (id, inbox) = join(&mut game);
    std::mem::forget(inbox);
    let pose = game.world.physics.car_pose(vid).unwrap();
    game.world.players.get_mut(&id).unwrap().character.position = [pose.position[0] + 2.0, STANDING_Y, pose.position[2]];
    let mut seq = 0;
    hold(&mut game, id, &mut seq, 0.0, 0, 3);
    hold(&mut game, id, &mut seq, 0.0, BUTTON_USE, 1);
    hold(&mut game, id, &mut seq, 0.0, 0, 3);
    assert!(game.world.players[&id].seat.is_none(), "nobody boards a wreck");

    for _ in 0..WRECK_TICKS {
        game.step();
    }
    assert!(!game.world.entities.dead(vid));
    assert_eq!(game.world.entities.of::<Health>(vid).hp, shared::protocol::vehicle_hp(shared::protocol::KIND_JEEP));
    assert!(!game.world.entities.has::<Wreck>(vid), "and is no wreck any more");
}

#[test]
fn a_rocket_in_flight_is_in_the_snapshot() {
    let (mut game, shooter, _, mut seq, _) = duel();
    game.world.players.get_mut(&shooter).unwrap().input = InputFrame { seq: 0, strafe: 0, forward: 0, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 };
    fire_rocket(&mut game.world, shooter);
    hold(&mut game, shooter, &mut seq, 0.0, 0, 1);
    let rockets = game.projectile_states();
    assert_eq!(rockets.len(), 1);
    let shared::protocol::ProjectileState { kind, z, vz, .. } = rockets[0];
    assert_eq!(kind, PROJECTILE_ROCKET);
    assert!(z < 0, "flying towards -Z from the shooter at z = 0: {z}");
    // The motor has pushed it past its launch speed, straight ahead, and no further than its top speed.
    let speed = -(vz as f32) / 100.0;
    assert!(speed > ROCKET_SPEED && speed <= ROCKET_MAX_SPEED, "{vz}");
}

/// The target steps 5 m aside, then the shooter fires a frame whose picture was `lag` ticks old.
/// Returns the target's health afterwards.
fn shoot_at_a_target_that_has_moved(lag: u32) -> u8 {
    let (mut game, shooter, target, mut seq, _) = duel();
    for _ in 0..3 {
        game.step();
    }
    let tick_before = game.world.tick;
    game.world.players.get_mut(&target).unwrap().character.position = [105.0, STANDING_Y, -20.0];
    hold(&mut game, shooter, &mut seq, 0.0, 0, 4);
    // The picture on the shooter's screen was `lag` ticks behind the server.
    let view = game.world.tick.wrapping_sub(lag);
    assert!(view <= tick_before + 8, "the test means to look at a moment around the move");
    seq += 1;
    game.handle(Command::Input(shooter, 0, vec![InputFrame { seq, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: to_mrad(aim_for(&game, shooter, 0)), view: view as u16, slot: 0 }]));
    settle(&mut game, shooter, &mut seq);
    game.world.players[&target].hp
}

#[test]
fn a_shot_hits_where_the_shooter_saw_the_target() {
    // Seen 10 ticks ago, when the target still stood in line, and the round (about five ticks over 20 m)
    // was there before the target stepped aside as the shooter saw it.
    assert_eq!(shoot_at_a_target_that_has_moved(10), PLAYER_MAX_HP - ak5().damage);
    // Seen right now, the target is 5 m to the side.
    assert_eq!(shoot_at_a_target_that_has_moved(0), PLAYER_MAX_HP);
}

#[test]
fn rewinding_is_capped_and_never_forwards() {
    assert_eq!(rewind_ticks(1000, 1000), 0);
    assert_eq!(rewind_ticks(1000, 994), 6);
    assert_eq!(rewind_ticks(1000, 900), MAX_REWIND_TICKS, "a very old picture is capped");
    assert_eq!(rewind_ticks(1000, 1010), 0, "a picture from the future is no picture");
    // The wire carries the tick modulo 65536.
    assert_eq!(rewind_ticks(65_540, 65_534), 6);
    assert_eq!(rewind_ticks(70_000, (69_994 % 65_536) as u16), 6);
}

#[test]
fn an_empty_hand_does_nothing() {
    let (mut game, shooter, target, mut seq, _) = duel();
    give(&mut game, shooter, 4, ITEM_NONE, 0);
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 4, 10);
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
    assert!(game.world.shots.is_empty());
}

fn hold_slot(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, slot: u8, ticks: u32) {
    for _ in 0..ticks {
        press(game, id, seq, buttons, slot, aim_for(game, id, slot as usize));
    }
}

#[test]
fn the_revolver_needs_a_press_per_shot_and_hits_hard() {
    let (mut game, shooter, target, mut seq, _) = duel();
    give(&mut game, shooter, 1, ITEM_REVOLVER, 1);
    let revolver = def(ITEM_REVOLVER).unwrap();
    // Held down for a second: still one shot.
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 1, TICK_RATE);
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP - revolver.damage);
    assert_eq!(game.world.players[&shooter].hotbar[1].ammo, revolver.mag - 1);
    // Let go and press again once the hammer is back.
    hold_slot(&mut game, shooter, &mut seq, 0, 1, revolver.cooldown_ticks);
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 1, 1);
    hold_slot(&mut game, shooter, &mut seq, 0, 1, 12);
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP - 2 * revolver.damage);
    assert!(game.world.shots.iter().all(|shot| shot.weapon == WEAPON_REVOLVER) || game.world.shots.is_empty());
}

#[test]
fn a_scoped_sniper_shot_goes_true_along_the_look_direction() {
    let (mut game, shooter, target, mut seq, _) = duel();
    let sniper = def(ITEM_SNIPER).unwrap();
    give(&mut game, shooter, 1, ITEM_SNIPER, 1);
    // Straight ahead of the eye, not off to the side where the over-the-shoulder crosshair
    // would find a target.
    game.world.players.get_mut(&target).unwrap().character.position = [100.0, STANDING_Y, -20.0];
    // Long enough that no rewound picture still has it anywhere else.
    hold(&mut game, shooter, &mut seq, 0.0, 0, MAX_REWIND_TICKS + 1);
    press(&mut game, shooter, &mut seq, BUTTON_ALT | BUTTON_FIRE, 1, 0.0);
    // A round in flight, not an instant hit.
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
    assert!(game.world.in_flight().values().any(|r| r.kind == PROJECTILE_BULLET));
    // 20 m at twice a tank shell's speed is a handful of ticks.
    let mut landed = false;
    for _ in 0..TICK_RATE / 4 {
        press(&mut game, shooter, &mut seq, BUTTON_ALT, 1, 0.0);
        landed |= game.world.shots.iter().any(|shot| shot.weapon == WEAPON_BULLET);
    }
    assert!(landed);
    // Straight ahead of the eye is the head: double damage, and a headshot kill.
    let expected = PLAYER_MAX_HP.saturating_sub(sniper.damage * 2);
    assert_eq!(game.world.players[&target].hp, expected);
    assert_eq!(game.world.players[&shooter].hotbar[1].ammo, sniper.mag - 1);
}

#[test]
fn aiming_tightens_any_guns_spread() {
    let ak = ak5();
    assert_eq!(effective_spread(ak, 0), ak.spread);
    assert_eq!(effective_spread(ak, BUTTON_ALT | BUTTON_FIRE), ak.spread * AIM_SPREAD_SCALE);
    assert!(effective_spread(ak, BUTTON_ALT) < effective_spread(ak, 0));
}

#[test]
fn aim_without_a_scoped_gun_changes_nothing() {
    let (mut game, shooter, target, mut seq, _) = duel();
    // The AK5 has no scope: aim held leaves the over-the-shoulder crosshair, which finds the
    // target the duel sets up to the side.
    let pitch = aim_for(&game, shooter, 0);
    press(&mut game, shooter, &mut seq, BUTTON_ALT | BUTTON_FIRE, 0, pitch);
    settle(&mut game, shooter, &mut seq);
    assert!(game.world.players[&target].hp < PLAYER_MAX_HP);
}

#[test]
fn a_gun_runs_dry_reloads_and_fires_again() {
    let (mut game, shooter, target, mut seq, _) = duel();
    game.world.players.get_mut(&shooter).unwrap().hotbar[0].ammo = 1;
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 1);
    assert_eq!(game.world.players[&shooter].hotbar[0].ammo, 0);
    settle(&mut game, shooter, &mut seq);
    let hp = game.world.players[&target].hp;
    // Dry: pulling the trigger starts a reload instead of a shot.
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, ak5().cooldown_ticks + 1);
    assert_eq!(game.world.players[&target].hp, hp);
    assert!(game.world.players[&shooter].reload_left > 0);
    hold(&mut game, shooter, &mut seq, 0.0, 0, ak5().reload_ticks + 1);
    assert_eq!(game.world.players[&shooter].hotbar[0].ammo, ak5().mag);
    hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 1);
    settle(&mut game, shooter, &mut seq);
    assert_eq!(game.world.players[&target].hp, hp - ak5().damage);
}

#[test]
fn switching_slots_drops_a_reload() {
    let (mut game, shooter, _, mut seq, _) = duel();
    game.world.players.get_mut(&shooter).unwrap().hotbar[0].ammo = 3;
    hold_slot(&mut game, shooter, &mut seq, BUTTON_RELOAD, 0, 2);
    assert!(game.world.players[&shooter].reload_left > 0);
    hold_slot(&mut game, shooter, &mut seq, 0, 3, 1);
    assert_eq!(game.world.players[&shooter].reload_left, 0);
    assert_eq!(game.world.players[&shooter].hotbar[0].ammo, 3, "it never finished");
}

#[test]
fn a_grenade_arcs_falls_and_bursts_when_the_fuse_is_out() {
    let (mut game, shooter, target, mut seq, _) = duel();
    give(&mut game, shooter, 2, ITEM_GRENADE, 2);
    // Thrown level at a target 20 m away: it lands short of them, then the fuse takes it.
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 2, 1);
    assert_eq!(game.world.players[&shooter].hotbar[2].count, 1, "one thrown");
    assert_eq!(game.world.in_flight().len(), 1);
    let grenade = *game.world.in_flight().values().next().unwrap();
    assert_eq!(grenade.kind, PROJECTILE_GRENADE);
    hold_slot(&mut game, shooter, &mut seq, 0, 2, 20);
    let grenade = *game.world.in_flight().values().next().expect("still in flight");
    assert!(grenade.position[1] < 3.0, "gravity pulls it down: y = {}", grenade.position[1]);
    hold_slot(&mut game, shooter, &mut seq, 0, 2, GRENADE_FUSE_TICKS);
    assert!(game.world.in_flight().is_empty(), "burst");
    assert!(game.world.shots.iter().any(|s| s.weapon == WEAPON_GRENADE) || game.world.players[&target].hp <= PLAYER_MAX_HP);
}

#[test]
fn a_grenade_at_your_feet_hurts_and_one_beside_a_target_kills_them() {
    let (mut game, shooter, target, _, _) = duel();
    let at = game.world.players[&target].character.position;
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying { kind: PROJECTILE_GRENADE, owner: shooter, source: 0, position: [at[0] + 1.0, at[1], at[2]], velocity: [0.0; 3], ttl: 1, item: 0, back: 0, flown: 0.0 });
    game.step();
    assert!(game.world.in_flight().is_empty());
    assert!(game.world.players[&target].hp < PLAYER_MAX_HP - 50, "{}", game.world.players[&target].hp);
    assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP, "20 m clear of the blast");
}

#[test]
fn a_flare_gun_spends_a_flare_and_fires_it() {
    let (mut game, shooter, _, mut seq, _) = duel();
    give(&mut game, shooter, 2, ITEM_FLARE, 2);
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 2, 1);
    assert_eq!(game.world.players[&shooter].hotbar[2].count, 1, "one fired");
    let flare = *game.world.in_flight().values().next().expect("in flight");
    assert_eq!(flare.kind, PROJECTILE_FLARE);
    // Aimed level, it still goes up steeply.
    let speed = flare.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!((flare.velocity[1] / speed - FLARE_LOFT.sin()).abs() < 0.02, "{:?}", flare.velocity);
    assert!(game.world.shots.iter().any(|s| s.weapon == WEAPON_FLARE && s.shooter == shooter));
}

#[test]
fn a_flare_climbs_then_sinks_slowly_and_burns_out_in_the_air() {
    let (mut game, shooter, target, _, _) = duel();
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying { kind: PROJECTILE_FLARE, owner: shooter, source: 0, position: [100.0, 2.0, 10.0], velocity: [5.0, FLARE_SPEED, 0.0], ttl: FLARE_BURN_TICKS, item: 0, back: 0, flown: 0.0 });
    let mut top = 0.0f32;
    let mut ticks = 0;
    while game.world.in_flight()[&id].velocity[1] > 0.0 {
        game.step();
        ticks += 1;
        top = top.max(game.world.in_flight()[&id].position[1]);
    }
    assert!(top > 50.0, "it climbs high: {top}");
    // Under its parachute it sinks no faster than `FLARE_SINK` and its drift dies away.
    for _ in 0..2 * TICK_RATE {
        game.step();
        ticks += 1;
    }
    let hanging = game.world.in_flight()[&id];
    assert!((hanging.velocity[1] + FLARE_SINK).abs() < 1e-3, "sinks at {}", hanging.velocity[1]);
    assert!(hanging.velocity[0].abs() < 0.5, "drift dies away: {}", hanging.velocity[0]);
    assert!(hanging.position[1] > top - 2.0 * FLARE_SINK - 2.0, "slowly: {} from {top}", hanging.position[1]);
    for _ in ticks..FLARE_BURN_TICKS - 1 {
        game.step();
    }
    let last = game.world.in_flight()[&id];
    assert!(last.position[1] > 10.0 && last.velocity[1] < 0.0, "still hanging just before it burns out: {last:?}");
    game.step();
    assert!(!game.world.in_flight().contains_key(&id), "burnt out");
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP, "light, not harm");
}

#[test]
fn a_flare_that_comes_down_burns_where_it_lands() {
    let (mut game, shooter, _, _, _) = duel();
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying { kind: PROJECTILE_FLARE, owner: shooter, source: 0, position: [100.0, 3.0, 10.0], velocity: [0.0, -1.0, 0.0], ttl: FLARE_BURN_TICKS, item: 0, back: 0, flown: 0.0 });
    let mut ticks = 0;
    while game.world.in_flight()[&id].velocity != [0.0; 3] {
        game.step();
        ticks += 1;
        assert!(ticks < 3 * TICK_RATE, "still falling");
    }
    let landed = game.world.in_flight()[&id].position;
    assert!(landed[1] < 1.0, "on the ground: {landed:?}");
    for _ in 0..TICK_RATE {
        game.step();
    }
    assert_eq!(game.world.in_flight()[&id].position, landed, "it burns where it lies");
}

#[test]
fn a_bandage_heals_after_a_pause_and_is_used_up() {
    let (mut game, shooter, _, mut seq, _) = duel();
    give(&mut game, shooter, 3, ITEM_BANDAGE, 1);
    game.world.players.get_mut(&shooter).unwrap().hp = 30;
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 3, 1);
    assert_eq!(game.world.players[&shooter].hp, 30, "not yet");
    assert!(game.world.players[&shooter].channel_left > 0);
    hold_slot(&mut game, shooter, &mut seq, 0, 3, BANDAGE_TICKS + 1);
    assert_eq!(game.world.players[&shooter].hp, 30 + def(ITEM_BANDAGE).unwrap().damage);
    assert_eq!(game.world.players[&shooter].hotbar[3], ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 });
}

#[test]
fn being_hit_spoils_a_bandage_and_full_health_does_not_start_one() {
    let (mut game, shooter, target, mut seq, _) = duel();
    give(&mut game, shooter, 3, ITEM_BANDAGE, 2);
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 3, 1);
    assert_eq!(game.world.players[&shooter].channel_left, 0, "nothing to heal");

    game.world.players.get_mut(&shooter).unwrap().hp = 50;
    hold_slot(&mut game, shooter, &mut seq, 0, 3, 5);
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 3, 1);
    assert!(game.world.players[&shooter].channel_left > 0);
    damage(&mut game.world, shooter, 5, Some(target), WEAPON_RIFLE);
    assert_eq!(game.world.players[&shooter].channel_left, 0);
    hold_slot(&mut game, shooter, &mut seq, 0, 3, BANDAGE_TICKS + 1);
    assert_eq!(game.world.players[&shooter].hp, 45);
    assert_eq!(game.world.players[&shooter].hotbar[3].count, 2, "not spent");
}

#[test]
fn a_new_life_starts_with_the_standard_loadout() {
    let (mut game, shooter, target, mut seq, mut seq_t) = duel();
    give(&mut game, target, 4, ITEM_BANDAGE, 2);
    damage(&mut game.world, target, PLAYER_MAX_HP, Some(shooter), WEAPON_RIFLE);
    hold(&mut game, target, &mut seq_t, 0.0, 0, RESPAWN_TICKS);
    assert!(game.world.players[&target].alive());
    let hotbar = game.world.players[&target].hotbar;
    assert_eq!(hotbar, starting_hotbar());
    let items: Vec<u8> = hotbar.iter().map(|slot| slot.item).collect();
    assert_eq!(items, [ITEM_AK5, ITEM_SNIPER, ITEM_FLARE, ITEM_DRONE, ITEM_LOITER]);
    assert_eq!(hotbar[0].ammo, def(ITEM_AK5).unwrap().mag, "magazines are full");
    assert_eq!(hotbar[2].count, def(ITEM_FLARE).unwrap().stack, "a full stack of flares");
    let _ = &mut seq;
}
/// A game on a world with one solid pine at (10, 0), identified as tile (2, 3) index 0.
fn game_with_tree() -> (Game, Obj) {
    use shared::world::{ObjId, TREE_PINE, Tree};
    let tree = Tree { x: 1000, z: 0, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true };
    let id = ObjId { col: 2, row: 3, index: 0 };
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], trees: vec![tree], tree_ids: vec![id], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    (game, Obj::Tree(id))
}

#[test]
fn a_rock_blocks_bullets_but_cannot_be_felled() {
    use shared::world::{ObjId, TREE_ROCK_BOULDER, Tree};
    let rock = Tree { x: 1000, z: 0, base: 0, height: 150, crown: 100, kind: TREE_ROCK_BOULDER, solid: true };
    let id = ObjId { col: 0, row: 0, index: 0 };
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], trees: vec![rock], tree_ids: vec![id], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    // The rock is in the way of a shot along the ground, so there is a collider...
    let hit = game.world.physics.cast_ray_object([0.0, 0.5, 0.0], [1.0, 0.0, 0.0], 30.0);
    let (distance, object) = hit.expect("the rock is solid");
    assert!((distance - 9.2).abs() < 0.1, "{distance}");
    // ...but it is no object, so a shot, a blast or a car cannot fell it.
    assert_eq!(object, None);
    assert!(game.world.physics.objects_near([10.0, 0.5, 0.0], 5.0).is_empty());
    hit_object(&mut game.world, [0.0, 0.5, 0.0], [1.0, 0.0, 0.0], 30.0, 255);
    assert!(game.world.destroyed.is_empty());
}

#[test]
fn rifle_rounds_wear_a_tree_down_until_it_falls_away_from_the_shooter() {
    let (mut game, tree) = game_with_tree();
    let hp = game.world.tree(match tree { Obj::Tree(id) => id, _ => unreachable!() }).unwrap().hp();
    let rounds = hp.div_ceil(12);
    for _ in 0..rounds - 1 {
        damage_object(&mut game.world, tree, 12, [0.0, 1.5, 0.0]);
    }
    assert!(game.world.destroyed.is_empty(), "fell after {} rounds, wanted {rounds}", rounds - 1);
    damage_object(&mut game.world, tree, 12, [0.0, 1.5, 0.0]);
    assert_eq!(game.world.destroyed.len(), 1);
    // The shooter is at x = 0 and the tree at x = +10, so it falls towards +X: yaw -90°.
    let yaw = from_mrad(game.world.destroyed[0].1);
    assert!((yaw + std::f32::consts::FRAC_PI_2).abs() < 0.01, "{yaw}");
    // Nothing happens twice.
    damage_object(&mut game.world, tree, 255, [0.0, 1.5, 0.0]);
    assert_eq!(game.world.destroyed.len(), 1);
}

#[test]
fn a_car_at_speed_fells_the_tree_it_drives_into() {
    use shared::world::{ObjId, TREE_PINE, Tree};
    // The car's heading is not worth working out: trees stand 30 m off on every side.
    let trees: Vec<Tree> = [[3000, 0], [-3000, 0], [0, 3000], [0, -3000]]
        .into_iter()
        .map(|[x, z]| Tree { x, z, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true })
        .collect();
    let tree_ids = (0..4).map(|index| ObjId { col: 2, row: 3, index }).collect();
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], trees, tree_ids, ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let (_, driver) = driver_in_the_car(&mut game);
    for seq in 1..=TICK_RATE * 6 {
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    assert!(!game.world.destroyed.is_empty(), "the car drove 6 s and felled nothing");
}

#[test]
fn a_blast_fells_a_tree_in_reach_and_everyone_hears_of_it() {
    let (mut game, _) = game_with_tree();
    let (id, mut inbox) = join(&mut game);
    while inbox.try_recv().is_ok() {}
    let point = [10.0, 1.0, 1.5];
    assert_eq!(game.world.physics.cast_ray_object([10.0, 1.0, 5.0], [0.0, 0.0, -1.0], 10.0).map(|(_, o)| o.is_some()), Some(true));
    blast(&mut game.world, Blast { point, radius: 5.0, damage: 100.0, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: Some(id), weapon: WEAPON_ROCKET });
    game.step();
    assert_eq!(game.world.destroyed.len(), 1);
    let mut told = false;
    while let Ok(Outgoing::Reliable(bytes)) = inbox.try_recv() {
        if shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DESTROYED) {
            let message: shared::protocol::Destroyed = decode(&bytes).unwrap();
            assert_eq!(message.1.len(), 1);
            assert_eq!((message.1[0].0, message.1[0].1, message.1[0].2, message.1[0].3), (shared::protocol::OBJ_TREE, 2, 3, 0));
            told = true;
        }
    }
    assert!(told, "no Destroyed message");
    // Its trunk no longer stops a ray, and a player who joins now is told it is gone.
    assert!(game.world.physics.cast_ray_object([10.0, 1.0, 5.0], [0.0, 0.0, -1.0], 10.0).is_none());
    let (_, mut late) = join(&mut game);
    let mut late_told = false;
    while let Ok(Outgoing::Reliable(bytes)) = late.try_recv() {
        late_told |= shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DESTROYED);
    }
    assert!(late_told, "a late joiner was not told");
}

#[test]
fn a_blast_out_of_reach_leaves_the_tree_standing() {
    let (mut game, _) = game_with_tree();
    blast(&mut game.world, Blast { point: [10.0, 1.0, 30.0], radius: 5.0, damage: 100.0, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_ROCKET });
    game.step();
    assert!(game.world.destroyed.is_empty());
}
/// A 10 x 10 m, 6 m tall house with its middle at (20, 0), as a closed box mesh, in tile (1, 1).
fn game_with_house() -> (Game, Obj) {
    use shared::world::{Building, CollisionMesh, ObjId};
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![
        v(15.0, 0.0, -5.0), v(25.0, 0.0, -5.0), v(25.0, 0.0, 5.0), v(15.0, 0.0, 5.0),
        v(15.0, 6.0, -5.0), v(25.0, 6.0, -5.0), v(25.0, 6.0, 5.0), v(15.0, 6.0, 5.0),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    let building = Building {
        height: 600,
        footprint: vec![[1500, -500], [2500, -500], [2500, 500], [1500, 500]],
        base: 0,
        first: 0,
        walls: 8,
        roof: 2,
        ..Default::default()
    };
    let id = ObjId { col: 1, row: 1, index: 0 };
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 20_000,
        spawns: vec![[0, 0]],
        buildings: vec![building],
        building_ids: vec![id],
        building_triangles: vec![[0, 12]],
        collision: CollisionMesh { vertices, triangles },
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    (game, Obj::Building(id))
}

fn rocket_at(game: &mut Game, point: [f32; 3]) {
    blast(&mut game.world, Blast { point, radius: ROCKET_BLAST_RADIUS, damage: ROCKET_DAMAGE, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_ROCKET });
}

#[test]
fn a_house_takes_any_beating_and_is_still_standing() {
    let (mut game, house) = game_with_house();
    let Obj::Building(id) = house else { unreachable!() };
    assert_eq!(game.world.building(id).unwrap().hp(), 150, "a 10 x 10 x 6 m house");
    for _ in 0..5 {
        rocket_at(&mut game, [14.0, 1.0, 0.0]);
    }
    damage_object(&mut game.world, house, u16::MAX, [0.0, 1.5, 0.0]);
    game.step();
    assert!(game.world.destroyed.is_empty(), "the house came down");
    assert_eq!(game.world.object_hp.get(&house), Some(&1), "it is left at its last stage");
    assert_eq!(game.world.damage_stage(house), Some(DAMAGE_STAGES));
    assert!(game.world.physics.cast_ray_object([12.0, 1.0, 0.0], [1.0, 0.0, 0.0], 40.0).is_some(), "the walls stop nothing");
}

#[test]
fn a_rocket_on_the_wall_hurts_but_one_across_the_street_does_not() {
    let (mut game, house) = game_with_house();
    rocket_at(&mut game, [30.0, 1.0, 0.0]);
    game.step();
    assert!(game.world.object_hp.is_empty() && game.world.destroyed.is_empty(), "5 m from the wall is out of reach: {:?}", game.world.object_hp);
    rocket_at(&mut game, [27.0, 1.0, 0.0]);
    assert!(game.world.object_hp.get(&house).is_some_and(|&hp| hp < 150));
}

/// What `Damaged` entries (stage, marked, x, y, z in cm) `inbox` has been sent.
fn damaged_told(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<(u8, u8, i32, i32, i32)> {
    let mut out = Vec::new();
    while let Ok(Outgoing::Reliable(bytes)) = inbox.try_recv() {
        if shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DAMAGED) {
            out.extend(decode::<shared::protocol::Damaged>(&bytes).unwrap().1.into_iter().map(|e| (e.3, e.4, e.5, e.6, e.7)));
        }
    }
    out
}

#[test]
fn a_shell_leaves_a_mark_and_each_stage_is_announced_once() {
    let (mut game, house) = game_with_house();
    let (_, mut inbox) = join(&mut game);
    while inbox.try_recv().is_ok() {}
    // 150 HP: stages begin at 99, 49.5 and 22.5 left. A 30-damage blast leaves a mark but no stage.
    damage_object(&mut game.world, house, 30, [14.0, 1.0, 0.0]);
    game.step();
    assert_eq!(damaged_told(&mut inbox), vec![(0, 1, 1400, 100, 0)]);
    // 30 more: 90 left, stage 1; a mark too.
    damage_object(&mut game.world, house, 30, [14.0, 1.0, 0.0]);
    game.step();
    assert_eq!(damaged_told(&mut inbox), vec![(1, 1, 1400, 100, 0)]);
    // Rifle chips of 4 leave no mark, and say nothing until they cross into stage 2 (46 left).
    for _ in 0..11 {
        damage_object(&mut game.world, house, 4, [0.0, 1.5, 0.0]);
    }
    game.step();
    assert_eq!(damaged_told(&mut inbox), vec![(2, 0, 0, 0, 0)]);
}

#[test]
fn a_joiner_is_told_how_hurt_a_standing_house_is() {
    let (mut game, house) = game_with_house();
    damage_object(&mut game.world, house, 140, [14.0, 1.0, 0.0]);
    let (_, mut late) = join(&mut game);
    assert_eq!(damaged_told(&mut late), vec![(3, 0, 0, 0, 0)]);
}

#[test]
fn a_killing_blow_on_a_house_is_its_last_stage() {
    let (mut game, house) = game_with_house();
    game.world.fresh_damaged.clear();
    damage_object(&mut game.world, house, 150, [14.0, 1.0, 0.0]);
    assert_eq!(game.world.fresh_damaged.len(), 1);
    assert_eq!(game.world.fresh_damaged[0].1, DAMAGE_STAGES);
}

/// The target is moved to `distance` metres in front of the shooter, and left long enough that no
/// rewound picture still has it anywhere else.
fn stand_at(game: &mut Game, id: u32, distance: f32) {
    game.world.players.get_mut(&id).unwrap().character.position = [100.0 + ON_FOOT_SHOULDER, STANDING_Y, -distance];
    for _ in 0..=MAX_REWIND_TICKS {
        game.step();
    }
}

/// One ray of `item` from `distance` metres away, straight down -Z at the target at z = -20.
fn one_round_from(game: &mut Game, shooter: u32, item: u8, distance: f32) {
    let gun = def(item).unwrap();
    let round = Round { damage: gun.damage, vehicle: 1, plane: 1, weapon: gun.weapon, range: gun.range, falloff: gun.falloff };
    let origin = [100.0 + ON_FOOT_SHOULDER, STANDING_Y, -20.0 + distance];
    hitscan(&mut game.world, shooter, origin, [0.0, 0.0, -1.0], &round, 0, 0);
}

#[test]
fn a_shotgun_kills_at_point_blank_and_credits_the_shooter() {
    let (mut game, shooter, target, mut seq, _) = duel();
    give(&mut game, shooter, 4, ITEM_SHOTGUN, 1);
    stand_at(&mut game, target, 3.0);
    // The shoulder camera is 1.5 m above the target's centre, so the crosshair on it at three metres is well down.
    // Aimed, so the spread is tight enough that every pellet is on the body and the kill is not down to luck.
    for _ in 0..6 {
        press(&mut game, shooter, &mut seq, BUTTON_FIRE | BUTTON_ALT, 4, -0.46);
    }
    assert_eq!(game.world.players[&target].hp, 0, "ten pellets at three metres are lethal");
    assert_eq!(game.world.players[&shooter].kills, 1);
}

#[test]
fn a_shotgun_pull_announces_every_pellet() {
    let (mut game, shooter, _, _, _) = duel();
    game.world.players.get_mut(&shooter).unwrap().input = InputFrame { seq: 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: to_mrad(AIM_20M), view: 0, slot: 4 };
    game.world.shots.clear();
    fire_gun(&mut game.world, shooter, def(ITEM_SHOTGUN).unwrap());
    let gun = def(ITEM_SHOTGUN).unwrap();
    let shots = &game.world.shots;
    assert_eq!(shots.len(), usize::from(gun.pellets));
    assert_eq!(shots[0].weapon, WEAPON_SHOTGUN);
    assert!(shots[1..].iter().all(|s| s.weapon == WEAPON_PELLET));
}

#[test]
fn a_shotgun_pellet_is_a_scratch_from_far_away() {
    let gun = def(ITEM_SHOTGUN).unwrap();
    let (mut near, shooter, target, ..) = duel();
    one_round_from(&mut near, shooter, ITEM_SHOTGUN, 5.0);
    assert_eq!(near.world.players[&target].hp, PLAYER_MAX_HP - gun.damage);
    let (mut far, shooter, target, ..) = duel();
    one_round_from(&mut far, shooter, ITEM_SHOTGUN, 38.0);
    let lost = PLAYER_MAX_HP - far.world.players[&target].hp;
    assert!(lost > 0 && lost < gun.damage / 2, "{lost}");
}

#[test]
fn a_shotgun_pull_costs_one_round_and_needs_a_fresh_press() {
    let (mut game, shooter, target, mut seq, _) = duel();
    give(&mut game, shooter, 4, ITEM_SHOTGUN, 1);
    stand_at(&mut game, target, 35.0);
    // Held for two seconds: the cooldown is a second, but a pump gun still wants a fresh press.
    hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 4, 2 * TICK_RATE);
    let gun = def(ITEM_SHOTGUN).unwrap();
    assert_eq!(game.world.players[&shooter].hotbar[4].ammo, gun.mag - 1);
}

#[test]
fn guns_lose_damage_with_distance_and_keep_it_up_close() {
    for item in [ITEM_AK5, ITEM_REVOLVER] {
        let gun = def(item).unwrap();
        let (mut close, shooter, target, ..) = duel();
        one_round_from(&mut close, shooter, item, gun.falloff.start);
        assert_eq!(close.world.players[&target].hp, PLAYER_MAX_HP - gun.damage, "{}", gun.name);
        let (mut far, shooter, target, ..) = duel();
        one_round_from(&mut far, shooter, item, gun.range - 1.0);
        let lost = PLAYER_MAX_HP - far.world.players[&target].hp;
        assert!(lost < gun.damage && lost >= gun.damage / 2, "{}: {lost}", gun.name);
    }
}

#[test]
fn a_gun_round_flies_and_is_announced_once_instead_of_listed_every_snapshot() {
    let (mut game, shooter, target, mut seq, _) = duel();
    let aim = aim_for(&game, shooter, 0);
    press(&mut game, shooter, &mut seq, BUTTON_FIRE, 0, aim);
    assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP, "it has to get there first");
    let flying = game.world.in_flight();
    let bullet = flying.iter().find(|(_, r)| r.kind == PROJECTILE_BULLET).expect("the round is in flight");
    assert_eq!(bullet.1.item, ITEM_AK5);
    assert_eq!(game.world.bullets.len(), 1);
    let spawn = game.world.bullets[0];
    assert_eq!((spawn.id, spawn.shooter, spawn.item), (*bullet.0, shooter, ITEM_AK5));
    assert!((spawn.vz as f32 / 100.0 + BULLET_SPEED).abs() < 1.0, "fired along -Z at the bullet speed: {}", spawn.vz);
    assert!(game.projectile_states().is_empty(), "flown by the clients, not listed in every snapshot");
}

#[test]
fn a_round_that_has_flown_far_hits_for_less() {
    for (flown, full) in [(0.0, true), (190.0, false)] {
        let (mut game, shooter, target, ..) = duel();
        let gun = ak5();
        let id = game.world.alloc_id();
        game.world.launch_as(
            id,
            Flying {
                kind: PROJECTILE_BULLET,
                owner: shooter,
                source: 0,
                position: [100.0 + ON_FOOT_SHOULDER, STANDING_Y, -15.0],
                velocity: [0.0, 0.0, -BULLET_SPEED],
                ttl: 10,
                item: ITEM_AK5,
                back: 0,
                flown,
            },
        );
        projectiles::step(&mut game.world, 1.0 / TICK_RATE as f32, Stage::Hands);
        let lost = PLAYER_MAX_HP - game.world.players[&target].hp;
        if full {
            assert_eq!(lost, gun.damage);
        } else {
            assert!(lost < gun.damage && lost >= gun.damage / 2, "{lost}");
        }
        assert!(game.world.shots.iter().any(|s| s.weapon == WEAPON_BULLET && s.shooter == id), "its end names the bullet");
    }
}

/// What `Carved` entries `(live, blasted, fell)` `inbox` has been sent.
fn carved_told(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<(u8, usize, usize)> {
    let mut out = Vec::new();
    while let Ok(Outgoing::Reliable(bytes)) = inbox.try_recv() {
        if shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_CARVED) {
            let message = decode::<shared::protocol::Carved>(&bytes).unwrap();
            out.extend(message.2.into_iter().map(|e| (message.1, e.6.len(), e.7.len())));
        }
    }
    out
}

#[test]
fn a_rocket_bites_a_hole_in_the_wall_that_a_ray_goes_through() {
    let (mut game, house) = game_with_house();
    let (_, mut inbox) = join(&mut game);
    while inbox.try_recv().is_ok() {}
    let (hit, _) = game.world.physics.cast_ray_object([12.0, 1.0, 0.0], [1.0, 0.0, 0.0], 40.0).unwrap();
    assert!((hit - 3.0).abs() < 0.1, "the near wall is at x = 15: {hit}");
    rocket_at(&mut game, [14.5, 1.0, 0.0]);
    game.step();
    // The wall is open at the hole, and the ray carries on to the far wall: its cubes, x = 24 to 25.
    let (hit, _) = game.world.physics.cast_ray_object([12.0, 1.0, 0.0], [1.0, 0.0, 0.0], 40.0).unwrap();
    assert!((hit - 12.0).abs() < 0.1, "the bite did not open the wall: {hit}");
    assert!(game.world.destroyed.is_empty(), "a bite is not a collapse");
    assert!(game.world.object_hp.get(&house).is_some_and(|&hp| hp < 150), "and it still hurts");
    let told = carved_told(&mut inbox);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].0, 1, "it is news");
    assert!(told[0].1 > 5, "{told:?}");
    // Next to it the wall is still there.
    let (hit, _) = game.world.physics.cast_ray_object([12.0, 1.0, 4.0], [1.0, 0.0, 0.0], 40.0).unwrap();
    assert!((hit - 3.0).abs() < 0.1, "the wall beside the hole went too: {hit}");
}

#[test]
fn a_joiner_is_told_which_cells_are_gone() {
    let (mut game, _) = game_with_house();
    rocket_at(&mut game, [14.5, 1.0, 0.0]);
    game.step();
    let (_, mut late) = join(&mut game);
    let told = carved_told(&mut late);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].0, 0, "it is state, not news");
    assert!(told[0].1 > 5 && told[0].2 == 0);
}

#[test]
fn cutting_the_foot_of_a_house_drops_what_stood_on_it() {
    let (mut game, house) = game_with_house();
    let Obj::Building(id) = house else { unreachable!() };
    let mut fell = 0;
    // A band of blasts a metre up, all the way round the walls.
    for step in 0..=10 {
        let t = step as f32;
        for point in [[15.0 + t, 0.5, -5.0], [15.0 + t, 0.5, 5.0], [15.0, 0.5, -5.0 + t], [25.0, 0.5, -5.0 + t]] {
            if let Some(cut) = game.world.carve(id, point, 1.4) {
                fell += cut.fell.len();
            }
        }
    }
    assert!(fell > 0, "nothing came down");
}

#[test]
fn a_piece_of_wall_takes_a_few_rounds_and_then_breaks_out() {
    let (mut game, _) = game_with_house();
    let ray = |game: &Game, z: f32| game.world.physics.cast_ray_object([12.0, 1.3, z], [1.0, 0.0, 0.0], 40.0).unwrap().0;
    // Rounds from the west at the wall, 3 m away.
    for round in 1..=3 {
        hit_object(&mut game.world, [12.0, 1.3, 0.2], [1.0, 0.0, 0.0], 3.0, 12);
        game.step();
        assert!((ray(&game, 0.2) - 3.0).abs() < 0.1, "round {round} already broke it");
    }
    hit_object(&mut game.world, [12.0, 1.3, 0.2], [1.0, 0.0, 0.0], 3.0, 12);
    game.step();
    assert!(ray(&game, 0.2) > 11.0, "the fourth round left no hole");
    // It is a small one: two metres to the side the wall stands.
    assert!((ray(&game, 2.4) - 3.0).abs() < 0.1);
    assert!(game.world.destroyed.is_empty());
}

/// A vehicle of `kind` parked away from the spawn point, with a player in seat `index`, settled.
fn rider_in(kind: u8, index: usize) -> (Game, u32, u32) {
    let mut game = game();
    let vid = crate::mechanics::vehicles::add_vehicle(&mut game.world, kind, [60.0, 2.0, 0.0], 0.0);
    let (rider, inbox) = join(&mut game);
    std::mem::forget(inbox);
    assert!(crate::mechanics::vehicles::seat(&mut game.world, rider, vid, index));
    for _ in 0..60 {
        game.step();
    }
    (game, vid, rider)
}

/// A round of 20 damage that does 5 to a vehicle, out to 50 m.
fn round() -> Round {
    Round { damage: 20, vehicle: 5, plane: 5, weapon: WEAPON_RIFLE, range: 50.0, falloff: Falloff::NONE }
}

/// Fires `round` from 10 m out on the vehicle's right (+X), level, at height `y`, through its centre's
/// depth `z`; returns what the rider and the vehicle lost.
fn shoot_from_the_side(game: &mut Game, vid: u32, rider: u32, y: f32, z: f32) -> (u8, u8) {
    let (shooter, inbox) = join(game);
    std::mem::forget(inbox);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    let (hp, car) = (game.world.players[&rider].hp, game.world.entities.of::<Health>(vid).hp);
    hitscan(&mut game.world, shooter, [at[0] + 10.0, y, z], [-1.0, 0.0, 0.0], &round(), 0, 0);
    (hp - game.world.players[&rider].hp, car - game.world.entities.of::<Health>(vid).hp)
}

#[test]
fn a_jeep_passengers_head_shows_above_the_hull_and_can_be_shot() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_JEEP, 1);
    let (_, head) = game.world.seated_head(rider).expect("a jeep's crew show");
    let (lost, chipped) = shoot_from_the_side(&mut game, vid, rider, head[1], head[2]);
    assert_eq!(lost, (20.0 * HEADSHOT_MULT) as u8, "a round at the head is a headshot");
    assert_eq!(chipped, 0, "and goes over the hull");
}

#[test]
fn the_hull_stops_a_round_aimed_at_a_seated_players_legs() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_JEEP, 1);
    let (_, head) = game.world.seated_head(rider).unwrap();
    let centre = game.world.physics.car_pose(vid).unwrap().position;
    let (lost, chipped) = shoot_from_the_side(&mut game, vid, rider, centre[1], head[2]);
    assert_eq!(lost, 0, "the door is in the way");
    assert_eq!(chipped, 5, "and takes the round");
}

#[test]
fn a_tanks_crew_cannot_be_shot() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_TANK, 0);
    assert!(game.world.seated_head(rider).is_none());
    let top = game.world.physics.car_pose(vid).unwrap().position[1] + TANK_HALF_EXTENTS[1] + 0.2;
    let (lost, _) = shoot_from_the_side(&mut game, vid, rider, top, 0.0);
    assert_eq!(lost, 0);
}

#[test]
fn a_helicopters_gunner_is_shot_through_the_glass() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_HELI, 1);
    let (_, head) = game.world.seated_head(rider).unwrap();
    // The gunner sits inside the hull box, so the round meets the airframe first.
    let (lost, chipped) = shoot_from_the_side(&mut game, vid, rider, head[1] - 0.3, head[2]);
    assert_eq!(lost, 20, "the round goes on through to the body");
    assert_eq!(chipped, 0, "and the airframe is unhurt");
}

#[test]
fn a_seated_players_head_is_rewound_like_a_standing_ones() {
    let (game, _, rider) = rider_in(shared::protocol::KIND_JEEP, 1);
    let (_, now) = game.world.seated_head(rider).unwrap();
    let then = game.world.head_at(rider, 1).expect("the record has them seated");
    assert!((0..3).all(|i| (now[i] - then[i]).abs() < 0.05), "a parked jeep has not moved: {now:?} {then:?}");
    assert!(game.world.position_at(rider, 1).is_none(), "and not standing");
}

#[test]
fn a_passenger_shot_dead_falls_out_of_the_seat() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_JEEP, 1);
    damage(&mut game.world, rider, PLAYER_MAX_HP, None, WEAPON_RIFLE);
    assert!(!game.world.players[&rider].alive());
    assert!(game.world.players[&rider].seat.is_none());
    assert_eq!(game.world.entities.of::<Vehicle>(vid).seats[1], None);
}

#[test]
fn a_jeep_passenger_fires_their_own_gun_out_of_the_car() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_JEEP, 1);
    give(&mut game, rider, 0, ITEM_AK5, 1);
    let (_, head) = game.world.seated_head(rider).unwrap();
    // A target ahead, where the passenger's shoulder camera looks.
    let (target, inbox) = join(&mut game);
    std::mem::forget(inbox);
    let pivot = [head[0] + PASSENGER_SHOULDER, head[1] + PASSENGER_HEIGHT, head[2]];
    game.world.players.get_mut(&target).unwrap().character.position = [pivot[0], STANDING_Y, head[2] - 20.0];
    let pitch = ((STANDING_Y - pivot[1]) / 20.0).atan();
    let mut seq = 0;
    for _ in 0..3 {
        press(&mut game, rider, &mut seq, BUTTON_FIRE, 0, pitch);
    }
    for _ in 0..12 {
        press(&mut game, rider, &mut seq, 0, 0, pitch);
    }
    assert!(game.world.players[&target].hp < PLAYER_MAX_HP, "the round landed");
    assert_eq!(game.world.entities.of::<Health>(vid).hp, vehicle_def(shared::protocol::KIND_JEEP).hp, "and left through the car without hurting it");
    assert!(game.world.players[&rider].hotbar[0].ammo < ak5().mag);
}

#[test]
fn a_driver_does_not_fire_their_gun() {
    let (mut game, _, rider) = rider_in(shared::protocol::KIND_JEEP, 0);
    give(&mut game, rider, 0, ITEM_AK5, 1);
    let mut seq = 0;
    for _ in 0..10 {
        press(&mut game, rider, &mut seq, BUTTON_FIRE, 0, 0.0);
    }
    assert_eq!(game.world.players[&rider].hotbar[0].ammo, ak5().mag);
}

#[test]
fn a_passenger_reloads_a_gun_that_is_not_full_and_otherwise_changes_seat() {
    let (mut game, vid, rider) = rider_in(shared::protocol::KIND_JEEP, 1);
    give(&mut game, rider, 0, ITEM_AK5, 1);
    game.world.players.get_mut(&rider).unwrap().hotbar[0].ammo = 3;
    let mut seq = 0;
    press(&mut game, rider, &mut seq, BUTTON_RELOAD, 0, 0.0);
    // Frames wait a moment in the jitter buffer before they are played.
    for _ in 0..6 {
        press(&mut game, rider, &mut seq, 0, 0, 0.0);
    }
    assert_eq!(game.world.players[&rider].seat.map(|seat| seat.index), Some(1), "still in their seat");
    assert!(game.world.players[&rider].reload_left > 0, "reloading");
    for _ in 0..ak5().reload_ticks + 2 {
        press(&mut game, rider, &mut seq, 0, 0, 0.0);
    }
    assert_eq!(game.world.players[&rider].hotbar[0].ammo, ak5().mag);
    press(&mut game, rider, &mut seq, BUTTON_RELOAD, 0, 0.0);
    for _ in 0..6 {
        press(&mut game, rider, &mut seq, 0, 0, 0.0);
    }
    let seat = game.world.players[&rider].seat.unwrap();
    assert_eq!(seat.vehicle, vid);
    assert_ne!(seat.index, 1, "with a full gun, reload moves them on");
}
