use super::*;
use shared::items::*;
use crate::game::{Command, Game, Outgoing};
use sim::STANDING_Y;
use shared::protocol::{InputFrame, ItemSlot, THRUST_LINKED, WEAPON_RIFLE};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[0, 0]], ..Default::default() };
    // Loose items would take a test's use button.
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, inbox): (_, mpsc::Receiver<Outgoing>) = mpsc::channel(4096);
    std::mem::forget(inbox);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    response.try_recv().unwrap().unwrap()
}

/// Feeds `ticks` frames of `buttons` and the forward axis, facing -Z and looking `pitch` up.
fn fly(game: &mut Game, id: u32, seq: &mut u32, forward: i8, pitch: f32, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        let frame = InputFrame { seq: *seq, strafe: 0, forward, yaw: 0, buttons, pitch: shared::quant::to_mrad(pitch), view: 0, slot: 0 };
        game.handle(Command::Input(id, 0, vec![frame]));
        game.step();
    }
}

fn idle(game: &mut Game, id: u32, seq: &mut u32, ticks: u32) {
    fly(game, id, seq, 0, 0.0, 0, ticks);
}

/// A pilot on open ground away from the parked car, holding a drone in slot 0, and settled.
fn pilot(game: &mut Game) -> (u32, u32) {
    let id = join(game);
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character.position = [100.0, STANDING_Y, 0.0];
    player.hands.hotbar[0] = ItemSlot { item: ITEM_DRONE, count: 1, ammo: 0 };
    let mut seq = 0;
    idle(game, id, &mut seq, 10);
    (id, seq)
}

/// Launches the pilot's drone and lets go of fire; returns the drone's id.
fn launch_drone(game: &mut Game, id: u32, seq: &mut u32) -> u32 {
    fly(game, id, seq, 0, 0.0, BUTTON_FIRE, 3);
    idle(game, id, seq, 3);
    game.world.entities.of::<Player>(id).seat.expect("the pilot is at the drone's controls").vehicle
}

fn drone(game: &Game, vid: u32) -> Option<Drone> {
    game.world.entities.get::<Drone>(vid).copied()
}

#[test]
fn a_launch_spends_the_drone_and_puts_the_player_at_its_controls() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let player = &game.world.entities.of::<Player>(id);
    assert_eq!(player.seat, Some(Seat { vehicle: vid, index: 0, remote: true }));
    assert_eq!(player.hands.hotbar[0].item, ITEM_NONE, "the drone left the hand");
    assert_eq!(game.world.entities.of::<Vehicle>(vid).kind, KIND_DRONE);
    assert!(drone(&game, vid).is_some_and(|drone| drone.linked && drone.owner == id));
    // The body is still out there, kneeling, and its drone is listed with a full, linked battery.
    let body = game.seen(id).expect("the pilot's body is listed");
    assert_ne!(body.flags.unwrap().flags & shared::protocol::FLAG_CROUCHING, 0);
    let listed = game.seen(vid).unwrap();
    let thrust = listed.thrust.expect("a drone sends its battery");
    assert!(thrust.air > 250 && thrust.flags & THRUST_LINKED != 0, "{thrust:?}");
}

#[test]
fn a_drone_left_alone_hovers() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let start = game.world.physics.car_pose(vid).unwrap().position;
    idle(&mut game, id, &mut seq, 120);
    let now = game.world.physics.car_pose(vid).unwrap().position;
    assert!((now[1] - start[1]).abs() < 0.5, "held its height: {start:?} -> {now:?}");
    assert!(drone(&game, vid).is_some_and(|drone| drone.linked));
}

#[test]
fn the_stick_flies_the_drone_and_the_body_stays_put() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let body = game.world.entities.of::<Player>(id).character.position;
    let vid = launch_drone(&mut game, id, &mut seq);
    let start = game.world.physics.car_pose(vid).unwrap().position;
    fly(&mut game, id, &mut seq, 127, 0.0, 0, 120);
    let now = game.world.physics.car_pose(vid).unwrap();
    assert!(start[2] - now.position[2] > 15.0, "flew ahead (-Z): {start:?} -> {:?}", now.position);
    let speed = now.velocity[0].hypot(now.velocity[2]);
    assert!(speed > 12.0, "and fast: {speed}");
    let still = game.world.entities.of::<Player>(id).character.position;
    assert!((0..3).all(|i| (still[i] - body[i]).abs() < 0.05), "the body stayed: {body:?} -> {still:?}");
}

#[test]
fn looking_down_with_the_stick_forward_dives() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    fly(&mut game, id, &mut seq, 0, 0.0, BUTTON_JUMP, 120);
    idle(&mut game, id, &mut seq, 90);
    let high = game.world.physics.car_pose(vid).unwrap().position;
    fly(&mut game, id, &mut seq, 127, -0.6, 0, 60);
    let low = game.world.physics.car_pose(vid).unwrap();
    assert!(low.velocity[1] < -5.0, "diving: {:?}", low.velocity);
    assert!(low.position[1] < high[1] - 3.0 && low.position[2] < high[2] - 5.0, "down and ahead: {high:?} -> {:?}", low.position);
}

#[test]
fn fire_sets_the_charge_off_and_hurts_who_is_near() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let target = join(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    // Beside the drone, out of its proximity fuse.
    game.world.entities.get_mut::<Player>(target).unwrap().character.position = [at[0] + 2.0, STANDING_Y, at[2]];
    fly(&mut game, id, &mut seq, 0, 0.0, BUTTON_FIRE, 3);
    assert!(!game.world.entities.has::<Vehicle>(vid), "the drone is gone");
    assert_eq!(game.world.entities.of::<Player>(id).seat, None, "the pilot is back in their body");
    assert!(game.world.entities.of::<crate::entities::Health>(target).hp < PLAYER_MAX_HP, "the blast reached the bystander");
}

#[test]
fn flying_into_someone_wounded_kills_them_and_credits_the_pilot() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let target = join(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    game.world.entities.get_mut::<Player>(target).unwrap().character.position = [at[0], STANDING_Y, at[2] - 15.0];
    // A soldier at full health lives through it, knocked flat; one already hurt does not.
    game.world.entities.get_mut::<crate::entities::Health>(target).unwrap().hp = 60;
    // Down to head height, then straight at them.
    for _ in 0..240 {
        if !game.world.entities.has::<Vehicle>(vid) {
            break;
        }
        let y = game.world.physics.car_pose(vid).unwrap().position[1];
        let pitch = if y > STANDING_Y + 0.6 { -0.15 } else { 0.0 };
        fly(&mut game, id, &mut seq, 127, pitch, 0, 1);
    }
    assert!(!game.world.entities.has::<Vehicle>(vid), "it went off");
    assert_eq!(game.world.entities.of::<crate::entities::Health>(target).hp, 0, "right on them, the blast kills");
    assert_eq!(game.world.entities.of::<Player>(id).score.kills, 1);
}

#[test]
fn use_recalls_the_drone_without_a_blast() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let vehicles = game.world.entities.count::<Vehicle>();
    fly(&mut game, id, &mut seq, 0, 0.0, BUTTON_USE, 1);
    idle(&mut game, id, &mut seq, 3);
    assert!(!game.world.entities.has::<Vehicle>(vid));
    assert_eq!(game.world.entities.count::<Vehicle>(), vehicles - 1);
    let player = &game.world.entities.of::<Player>(id);
    assert_eq!(player.seat, None, "back in the body, and not in some other vehicle");
    assert_eq!(game.world.entities.of::<crate::entities::Health>(id).hp, PLAYER_MAX_HP);
    assert!(game.seen(vid).is_none(), "no longer listed");
}

#[test]
fn a_flat_battery_drops_the_drone_which_goes_off_where_it_lands() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    // Well away from the pilot first, and up a little.
    fly(&mut game, id, &mut seq, 127, 0.1, 0, 90);
    game.world.entities.of_mut::<Drone>(vid).energy = 2;
    idle(&mut game, id, &mut seq, 3);
    assert_eq!(game.world.entities.of::<Player>(id).seat, None, "control is back with the pilot");
    assert!(drone(&game, vid).is_some_and(|drone| !drone.linked), "and the drone falls");
    idle(&mut game, id, &mut seq, DRONE_FALL_TICKS + 10);
    assert!(!game.world.entities.has::<Vehicle>(vid), "it went off");
    assert_eq!(game.world.entities.of::<crate::entities::Health>(id).hp, PLAYER_MAX_HP, "far from the pilot");
}

#[test]
fn one_hit_pops_the_drone_and_frees_the_pilot() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let shooter = join(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    fly(&mut game, id, &mut seq, 127, 0.0, 0, 60);
    super::super::combat::damage_vehicle(&mut game.world, vid, 1, Some(shooter));
    assert!(!game.world.entities.has::<Vehicle>(vid));
    assert_eq!(game.world.entities.of::<Player>(id).seat, None);
    assert_eq!(game.world.entities.of::<crate::entities::Health>(id).hp, PLAYER_MAX_HP, "nobody in the drone died with it");
}

#[test]
fn a_hurt_pilot_loses_the_link() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    super::super::life::damage_entity(&mut game.world, id, super::super::life::Harm::new(10, None, WEAPON_RIFLE));
    idle(&mut game, id, &mut seq, 1);
    assert_eq!(game.world.entities.of::<Player>(id).seat, None);
    assert!(drone(&game, vid).is_some_and(|drone| !drone.linked));
}

#[test]
fn the_link_does_not_reach_past_the_range() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let body = game.world.entities.of::<Player>(id).character.position;
    game.world.physics.reset_car(vid, [body[0], 20.0, body[2] - DRONE_RANGE - 5.0], 0.0);
    idle(&mut game, id, &mut seq, 1);
    assert_eq!(game.world.entities.of::<Player>(id).seat, None);
    assert!(drone(&game, vid).is_some_and(|drone| !drone.linked));
}

#[test]
fn a_pilot_who_leaves_takes_the_drone_with_them() {
    let mut game = game();
    let (id, mut seq) = pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    game.handle(Command::Leave(id));
    game.step();
    assert!(!game.world.entities.has::<Vehicle>(vid));
}


/// A pilot like [`pilot`]'s, holding a loitering drone instead.
fn loiter_pilot(game: &mut Game) -> (u32, u32) {
    let (id, seq) = pilot(game);
    game.world.entities.get_mut::<Player>(id).unwrap().hands.hotbar[0] = ItemSlot { item: ITEM_LOITER, count: 1, ammo: 0 };
    (id, seq)
}

#[test]
fn a_loitering_drone_starts_high_up_behind_its_pilot_already_flying() {
    let mut game = game();
    let (id, mut seq) = loiter_pilot(&mut game);
    let body = game.world.entities.of::<Player>(id).character.position;
    // Facing -Z: behind is +Z. It has flown a metre or two by the time it is looked at.
    fly(&mut game, id, &mut seq, 0, 0.0, BUTTON_FIRE, 3);
    let vid = game.world.entities.of::<Player>(id).seat.expect("the pilot is at its controls").vehicle;
    assert_eq!(game.world.entities.of::<Vehicle>(vid).kind, KIND_LOITER);
    assert_eq!(game.world.entities.of::<Player>(id).hands.hotbar[0].item, ITEM_NONE, "it left the hand");
    let pose = game.world.physics.car_pose(vid).unwrap();
    assert!((pose.position[0] - body[0]).abs() < 0.5, "{pose:?}");
    assert!((pose.position[1] - body[1] - LOITER_LAUNCH_UP).abs() < 3.0, "{pose:?}");
    assert!((pose.position[2] - body[2] - LOITER_LAUNCH_BACK).abs() < 3.0, "{pose:?}");
    assert!(pose.velocity[2] < -LOITER_SPEED * 0.8, "already cruising along the look: {pose:?}");
}

#[test]
fn a_loitering_drone_left_alone_keeps_flying() {
    let mut game = game();
    let (id, mut seq) = loiter_pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let start = game.world.physics.car_pose(vid).unwrap().position;
    idle(&mut game, id, &mut seq, 120);
    let now = game.world.physics.car_pose(vid).unwrap().position;
    assert!(start[2] - now[2] > 40.0, "it cannot hover: {start:?} -> {now:?}");
    assert!((now[1] - start[1]).abs() < 2.0, "but holds its height: {start:?} -> {now:?}");
    assert!(drone(&game, vid).is_some_and(|drone| drone.linked));
    let listed = game.seen(vid).unwrap();
    assert!(listed.thrust.as_ref().is_some_and(|thrust| thrust.air > 240), "its own, longer battery: {:?}", listed.thrust);
}

#[test]
fn a_loitering_drones_warhead_reaches_far() {
    let mut game = game();
    let (id, mut seq) = loiter_pilot(&mut game);
    let target = join(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    // Brought down to the ground, with someone well outside an FPV drone's blast.
    game.world.physics.reset_car(vid, [100.0, 2.0, -60.0], 0.0);
    game.world.entities.get_mut::<Player>(target).unwrap().character.position = [108.0, STANDING_Y, -60.0];
    fly(&mut game, id, &mut seq, 0, 0.0, BUTTON_FIRE, 3);
    assert!(!game.world.entities.has::<Vehicle>(vid), "it went off");
    assert_eq!(game.world.entities.of::<Player>(id).seat, None);
    assert!(game.world.entities.of::<crate::entities::Health>(target).hp < PLAYER_MAX_HP, "8 m out, the blast still hurts");
}

#[test]
fn a_loitering_drone_takes_more_than_one_hit() {
    let mut game = game();
    let (id, mut seq) = loiter_pilot(&mut game);
    let shooter = join(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    super::super::combat::damage_vehicle(&mut game.world, vid, 30, Some(shooter));
    assert!(game.world.entities.has::<Vehicle>(vid), "one round does not bring it down");
    super::super::combat::damage_vehicle(&mut game.world, vid, 30, Some(shooter));
    assert!(!game.world.entities.has::<Vehicle>(vid), "two do");
    assert_eq!(game.world.entities.of::<Player>(id).seat, None, "and the pilot is back in their body");
}

#[test]
fn a_loitering_drone_reaches_much_further_than_an_fpv_drone() {
    let mut game = game();
    let (id, mut seq) = loiter_pilot(&mut game);
    let vid = launch_drone(&mut game, id, &mut seq);
    let body = game.world.entities.of::<Player>(id).character.position;
    game.world.physics.reset_car(vid, [body[0], 100.0, body[2] - 450.0], 0.0);
    idle(&mut game, id, &mut seq, 1);
    assert!(drone(&game, vid).is_some_and(|drone| drone.linked), "450 m out it is still flown");
    game.world.physics.reset_car(vid, [body[0], 100.0, body[2] - LOITER_RANGE - 5.0], 0.0);
    idle(&mut game, id, &mut seq, 1);
    assert_eq!(game.world.entities.of::<Player>(id).seat, None, "past its range the link is lost");
    assert!(drone(&game, vid).is_some_and(|drone| !drone.linked));
}
