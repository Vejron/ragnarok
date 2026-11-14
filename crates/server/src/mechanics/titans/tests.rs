use super::*;
use crate::entities::Health;
use crate::game::{Command, Game};
use shared::protocol::{HANDS_FULL_RIGHT, InputFrame, KIND_JEEP, KIND_TITAN};
use shared::quant::to_cm_i16;
use shared::world::{WORLD_VERSION, WorldFile};
use sim::Character;
use tokio::sync::{mpsc, oneshot};

fn game() -> Game {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[30_000, 30_000]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> u32 {
    let (outbox, _inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    response.try_recv().unwrap().unwrap()
}

/// A Titan standing at the origin, facing -Z, settled on its legs.
fn titan(game: &mut Game) -> u32 {
    let vid = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_TITAN, [0.0, TITAN_LEG_LENGTH + TITAN_HALF_EXTENTS[1], 0.0], 0.0);
    for _ in 0..300 {
        game.step();
    }
    vid
}

/// Stands soldier `id` on the ground at `[x, z]`.
fn stand(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    game.world.entities.of_mut::<Player>(id).character = Character { position: [x, STAND_CENTRE, z], grounded: true, ..Default::default() };
}

/// `world`, a point in the world, in Titan `vid`'s hull frame, as a pilot sends it.
fn in_hull(game: &Game, vid: u32, world: [f32; 3]) -> [i16; 3] {
    let pose = game.world.physics.car_pose(vid).unwrap();
    pose.local_dir(vec3::sub(world, pose.position)).map(to_cm_i16)
}

/// One tick with `pilot` holding `buttons` and asking for its right fist at `right` (world), `closed` or not.
fn tick(game: &mut Game, vid: u32, pilot: u32, seq: &mut u32, buttons: u16, right: Option<[f32; 3]>, closed: bool) {
    *seq += 1;
    game.handle(Command::Input(pilot, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
    let [rx, ry, rz] = right.map_or([0; 3], |point| in_hull(game, vid, point));
    let flags = if right.is_some() { HANDS_REACH_RIGHT } else { 0 } | if closed { HANDS_GRIP_RIGHT } else { 0 };
    game.handle(Command::Hands(pilot, ClientHands { tag: shared::protocol::C_HANDS, lx: 0, ly: 0, lz: 0, rx, ry, rz, flags }));
    game.step();
}

fn hp(game: &Game, id: u32) -> u8 {
    game.world.entities.of::<Health>(id).hp
}

/// A game with a piloted Titan crouched at the origin: the Titan, its pilot and the pilot's input count.
fn crouched() -> (Game, u32, u32, u32) {
    let mut game = game();
    let pilot = join(&mut game);
    let vid = titan(&mut game);
    assert!(crate::mechanics::seat(&mut game.world, pilot, vid, 0));
    let mut seq = 0;
    for _ in 0..300 {
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, None, false);
    }
    let pose = game.world.physics.car_pose(vid).unwrap();
    assert!(pose.position[1] < TITAN_RIDE_HEIGHT - 0.8 * TITAN_CROUCH && pose.up_y > 0.85, "it crouches under its pilot: {pose:?}");
    (game, vid, pilot, seq)
}

#[test]
fn a_closed_hand_picks_a_soldier_up_carries_them_and_throws_them_when_it_opens() {
    let (mut game, vid, pilot, mut seq) = crouched();
    let victim = join(&mut game);
    let spot = [TITAN_SHOULDER[0], -12.0];
    stand(&mut game, victim, spot);
    let over = [spot[0], TITAN_FIST_RADIUS + 0.2, spot[1]];
    // The open hand comes down beside them and does them no harm.
    for _ in 0..150 {
        stand(&mut game, victim, spot);
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, Some(over), false);
    }
    assert_eq!(game.world.entities.of::<Player>(victim).held, None);
    assert_eq!(hp(&game, victim), PLAYER_MAX_HP, "an open hand coming down on them does not hurt");
    // It closes on them.
    for _ in 0..5 {
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, Some(over), true);
    }
    assert_eq!(game.world.entities.of::<Player>(victim).held, Some(Fist { titan: vid, hand: 1 }), "picked up in the right hand");
    assert_eq!(game.seen(victim).unwrap().held, Some(shared::replicated::Held { vehicle: vid, hand: 1 }));
    assert!(game.seen(vid).unwrap().titan.unwrap().flags & HANDS_FULL_RIGHT != 0, "everyone sees the hand is full");
    // The Titan stands, and lifts them high.
    for _ in 0..360 {
        let pose = game.world.physics.car_pose(vid).unwrap();
        let ahead = pose.world_point([TITAN_SHOULDER[0], 0.0, -15.0]);
        tick(&mut game, vid, pilot, &mut seq, 0, Some(ahead), true);
    }
    let carried = game.world.entities.of::<Player>(victim).character;
    assert!(carried.position[1] > 25.0 && carried.down == 0, "carried high, on their feet: {carried:?}");
    let fist = game.world.physics.limbs(vid).unwrap().fists_world[1];
    assert!(vec3::distance(carried.position, held_at(fist)) < 1.5, "on the fist: {carried:?} against {fist:?}");
    // They can still shoot from up there.
    let ammo = game.world.entities.of::<Player>(victim).hands.hotbar[0].ammo;
    for n in 0..30 {
        game.handle(Command::Input(victim, 0, vec![InputFrame { seq: n + 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: 0, view: 0, slot: 0 }]));
        let pose = game.world.physics.car_pose(vid).unwrap();
        tick(&mut game, vid, pilot, &mut seq, 0, Some(pose.world_point([TITAN_SHOULDER[0], 0.0, -15.0])), true);
    }
    assert!(game.world.entities.of::<Player>(victim).hands.hotbar[0].ammo < ammo, "a held soldier fires their rifle");
    // Swung forward and let go, they fly.
    for _ in 0..14 {
        let pose = game.world.physics.car_pose(vid).unwrap();
        tick(&mut game, vid, pilot, &mut seq, 0, Some(pose.world_point([TITAN_SHOULDER[0], 6.0, -40.0])), true);
    }
    let before = hp(&game, victim);
    let pose = game.world.physics.car_pose(vid).unwrap();
    tick(&mut game, vid, pilot, &mut seq, 0, Some(pose.world_point([TITAN_SHOULDER[0], 6.0, -40.0])), false);
    let thrown = game.world.entities.of::<Player>(victim);
    let speed = thrown.character.knock[0].hypot(thrown.character.knock[1]);
    assert!(thrown.held.is_none() && speed > 20.0 && thrown.character.down > 0, "thrown as fast as the fist went: {speed} m/s, {:?}", thrown.character);
    assert!(thrown.character.knock[1] < 0.0, "the way the fist was going");
    let _ = thrown;
    assert!(hp(&game, victim) < before, "a hard throw hurts");
    assert!(game.seen(victim).unwrap().held.is_none());
}

#[test]
fn a_closed_fist_swung_through_a_soldier_throws_and_hurts_them() {
    let (mut game, vid, pilot, mut seq) = crouched();
    let victim = join(&mut game);
    let spot = [TITAN_SHOULDER[0], -12.0];
    let level = TITAN_FIST_RADIUS + 0.3;
    // The fist is drawn back to the right of them, then swung through where they stand.
    for _ in 0..90 {
        stand(&mut game, victim, spot);
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, Some([spot[0] + 9.0, level, spot[1]]), false);
    }
    assert_eq!(hp(&game, victim), PLAYER_MAX_HP);
    let mut struck = None;
    for step in 0..40 {
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, Some([spot[0] - 9.0, level, spot[1]]), true);
        if struck.is_none() && hp(&game, victim) < PLAYER_MAX_HP {
            struck = Some((step, game.world.entities.of::<Player>(victim).character));
        }
    }
    let (step, character) = struck.expect("the fist hit them");
    assert!(character.knock[0] < -8.0 && character.vy > 0.0 && !character.grounded, "thrown the way the fist went, at tick {step}: {character:?}");
    assert!(character.down > 0 || !game.world.entities.alive(victim), "knocked down");
    assert!(PLAYER_MAX_HP - hp(&game, victim) >= 40, "it hurts: {} left", hp(&game, victim));
}

#[test]
fn a_titans_arms_hang_unless_its_own_pilot_moves_them_and_when_the_pilot_stops() {
    let mut game = game();
    let (pilot, other) = (join(&mut game), join(&mut game));
    let vid = titan(&mut game);
    let rest = TITAN_FIST_REST;
    let ahead = ClientHands { tag: shared::protocol::C_HANDS, lx: 0, ly: 0, lz: 0, rx: 850, ry: 600, rz: -2400, flags: HANDS_REACH_RIGHT | HANDS_GRIP_RIGHT };
    // Somebody standing about asks for its fists: nothing.
    for _ in 0..60 {
        game.handle(Command::Hands(other, ahead));
        game.step();
    }
    let limbs = game.world.physics.limbs(vid).unwrap();
    assert!(vec3::distance(limbs.fists[1], rest) < 0.05 && limbs.closed == [false, false], "nobody aboard: {limbs:?}");
    // Its pilot does.
    assert!(crate::mechanics::seat(&mut game.world, pilot, vid, 0));
    for _ in 0..90 {
        game.handle(Command::Hands(pilot, ahead));
        game.step();
    }
    let limbs = game.world.physics.limbs(vid).unwrap();
    assert!(vec3::distance(limbs.fists[1], [8.5, 6.0, -24.0]) < 0.3 && limbs.closed == [false, true], "its pilot's fist goes out: {limbs:?}");
    let shown = game.seen(vid).unwrap().titan.unwrap();
    assert!((i32::from(shown.rz) + 2400).abs() < 30 && shown.flags & HANDS_GRIP_RIGHT != 0 && shown.flags & HANDS_FULL_RIGHT == 0, "and everyone sees it: {shown:?}");
    assert!((i32::from(shown.fly) + (TITAN_RIDE_HEIGHT * 100.0) as i32).abs() < 30 && shown.flx < 0 && shown.frx > 0, "with its feet on the ground under it: {shown:?}");
    // The pilot stops sending: the arm comes back to hang.
    for _ in 0..(HANDS_STALE_TICKS + 120) {
        game.step();
    }
    let limbs = game.world.physics.limbs(vid).unwrap();
    assert!(vec3::distance(limbs.fists[1], rest) < 0.05 && limbs.closed == [false, false], "let go of, it hangs: {limbs:?}");
}

#[test]
fn a_foot_coming_down_crushes_who_is_under_it_and_throws_those_around_it() {
    let mut game = game();
    let (pilot, under, near, far) = (join(&mut game), join(&mut game), join(&mut game), join(&mut game));
    let vid = titan(&mut game);
    let foot = [60.0, 0.0, 40.0];
    stand(&mut game, under, [foot[0] + 1.0, foot[2] - 1.0]);
    stand(&mut game, near, [foot[0] + 8.0, foot[2]]);
    stand(&mut game, far, [foot[0] + TITAN_STOMP_RADIUS + 3.0, foot[2]]);
    stomp(&mut game.world, vid, foot, Some(pilot));
    assert!(!game.world.entities.alive(under), "crushed");
    let thrown = game.world.entities.of::<Player>(near).character;
    assert!(thrown.knock[0] > 3.0 && thrown.vy > 0.0 && !thrown.grounded, "thrown outwards: {thrown:?}");
    assert_eq!(hp(&game, near), PLAYER_MAX_HP, "but not hurt");
    let untouched = game.world.entities.of::<Player>(far).character;
    assert!(untouched.knock == [0.0, 0.0] && untouched.grounded, "out of its reach: {untouched:?}");
}

#[test]
fn a_foot_wrecks_a_jeep_it_comes_down_on() {
    let mut game = game();
    let pilot = join(&mut game);
    let vid = titan(&mut game);
    let jeep = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_JEEP, [60.0, 1.0, 40.0], 0.0);
    for _ in 0..60 {
        game.step();
    }
    stomp(&mut game.world, vid, [61.0, 0.0, 40.0], Some(pilot));
    assert!(game.world.entities.dead(jeep), "flattened: {} left", hp(&game, jeep));
}

#[test]
fn a_titan_feels_a_tenth_of_a_shell_and_nothing_of_a_rifle() {
    let mut game = game();
    let vid = titan(&mut game);
    let full = hp(&game, vid);
    crate::mechanics::combat::damage_vehicle(&mut game.world, vid, 4, None);
    assert_eq!(hp(&game, vid), full, "small arms do nothing");
    crate::mechanics::combat::damage_vehicle(&mut game.world, vid, 100, None);
    assert_eq!(hp(&game, vid), full - 10, "a shell's hundred is ten to it");
    let jeep = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_JEEP, [60.0, 1.0, 40.0], 0.0);
    let before = hp(&game, jeep);
    crate::mechanics::combat::damage_vehicle(&mut game.world, jeep, 4, None);
    assert_eq!(hp(&game, jeep), before - 4, "a jeep feels all of it, as before");
}

#[test]
fn a_fist_fells_a_tree_and_takes_a_bite_out_of_a_house() {
    use shared::world::{Building, CollisionMesh, ObjId, TREE_PINE, Tree};
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![v(55.0, 0.0, -5.0), v(65.0, 0.0, -5.0), v(65.0, 0.0, 5.0), v(55.0, 0.0, 5.0), v(55.0, 6.0, -5.0), v(65.0, 6.0, -5.0), v(65.0, 6.0, 5.0), v(55.0, 6.0, 5.0)];
    let triangles = vec![[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
    let house = Building { height: 600, footprint: vec![[5500, -500], [6500, -500], [6500, 500], [5500, 500]], base: 0, first: 0, walls: 8, roof: 2, ..Default::default() };
    let (house_id, tree_id) = (ObjId { col: 1, row: 1, index: 0 }, ObjId { col: 2, row: 3, index: 0 });
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 40_000,
        spawns: vec![[30_000, 30_000]],
        buildings: vec![house],
        building_ids: vec![house_id],
        building_triangles: vec![[0, 12]],
        collision: CollisionMesh { vertices, triangles },
        trees: vec![Tree { x: 6000, z: 4000, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true }],
        tree_ids: vec![tree_id],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let vid = titan(&mut game);
    strike(&mut game.world, vid, [60.0, 3.0, 40.0 - TITAN_FIST_RADIUS + 0.5], TITAN_FIST_RADIUS, [0.0, 0.0, 20.0], 80.0, None);
    assert!(game.world.felled.contains(&Obj::Tree(tree_id)), "the tree goes over");
    assert!(!game.world.carves.contains_key(&house_id), "the house is out of its reach");
    strike(&mut game.world, vid, [54.0, 3.0, 0.0], TITAN_FIST_RADIUS, [20.0, 0.0, 0.0], 80.0, None);
    assert!(game.world.carves.contains_key(&house_id), "a hole is knocked in the house");
}

#[test]
fn a_placed_titan_stands_is_boarded_from_between_its_feet_walks_and_sets_its_pilot_down() {
    use shared::world::PlacedVehicle;
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[30_000, 30_000]], placed_vehicles: vec![PlacedVehicle { kind: KIND_TITAN, at: [0, 0], yaw: 0.0 }], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_TITAN).map(|(id, _)| id).expect("placed");
    for _ in 0..300 {
        game.step();
    }
    let stood = game.world.physics.car_pose(vid).unwrap();
    assert!((stood.position[1] - TITAN_RIDE_HEIGHT).abs() < 0.2 && stood.up_y > 0.99, "it stands where it was placed: {stood:?}");
    // A soldier on the ground between its feet climbs in.
    let pilot = join(&mut game);
    stand(&mut game, pilot, [0.5, 0.5]);
    let mut seq = 0;
    let mut press = |game: &mut Game, buttons: u16, forward: i8, ticks: u32| {
        for _ in 0..ticks {
            seq += 1;
            game.handle(Command::Input(pilot, 0, vec![InputFrame { seq, strafe: 0, forward, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
            game.step();
        }
    };
    press(&mut game, BUTTON_USE, 0, 1);
    press(&mut game, 0, 0, 4);
    assert_eq!(game.world.entities.of::<Player>(pilot).seat.map(|seat| (seat.vehicle, seat.index)), Some((vid, 0)), "boarded from the ground");
    press(&mut game, 0, 127, 360);
    let walked = game.world.physics.car_pose(vid).unwrap();
    assert!(walked.position[2] < -30.0 && walked.forward_speed > 0.8 * TITAN_MAX_SPEED && walked.up_y > 0.97, "it walks under its pilot: {walked:?}");
    press(&mut game, 0, 0, 240);
    press(&mut game, BUTTON_USE, 0, 1);
    press(&mut game, 0, 0, 30);
    let player = game.world.entities.of::<Player>(pilot);
    let at = game.world.physics.car_pose(vid).unwrap().position;
    assert!(player.seat.is_none() && player.character.position[1] < 3.0, "set down on the ground: {:?}", player.character);
    assert!((player.character.position[0] - at[0]).hypot(player.character.position[2] - at[2]) < 16.0, "by its feet: {:?} against {at:?}", player.character.position);
}
