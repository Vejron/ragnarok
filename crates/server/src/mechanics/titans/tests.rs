use super::*;
use crate::entities::Health;
use crate::game::{Command, Game};
use shared::protocol::{HANDS_FULL_LEFT, HANDS_FULL_RIGHT, InputFrame, KIND_JEEP, KIND_MISSILE, KIND_TITAN, PROJECTILE_CANNON_AP, PROJECTILE_MORTAR, PROJECTILE_ROTARY};
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
    tick_hand(game, vid, pilot, seq, buttons, 1, right, closed);
}

/// One tick with `pilot` holding `buttons` and asking for fist `hand` (0 the left, 1 the right) at `at`
/// (world), `closed` or not; the other hangs.
#[allow(clippy::too_many_arguments)]
fn tick_hand(game: &mut Game, vid: u32, pilot: u32, seq: &mut u32, buttons: u16, hand: usize, at: Option<[f32; 3]>, closed: bool) {
    *seq += 1;
    game.handle(Command::Input(pilot, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
    let [x, y, z] = at.map_or([0; 3], |point| in_hull(game, vid, point));
    let (reach, grip) = [(HANDS_REACH_LEFT, HANDS_GRIP_LEFT), (HANDS_REACH_RIGHT, HANDS_GRIP_RIGHT)][hand];
    let flags = if at.is_some() { reach } else { 0 } | if closed { grip } else { 0 };
    let mut hands = ClientHands { tag: shared::protocol::C_HANDS, lx: 0, ly: 0, lz: 0, rx: 0, ry: 0, rz: 0, flags };
    if hand == 0 {
        (hands.lx, hands.ly, hands.lz) = (x, y, z);
    } else {
        (hands.rx, hands.ry, hands.rz) = (x, y, z);
    }
    game.handle(Command::Hands(pilot, hands));
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
    let spot = [-TITAN_SHOULDER[0], -12.0];
    stand(&mut game, victim, spot);
    let over = [spot[0], TITAN_FIST_RADIUS + 0.2, spot[1]];
    // The open hand comes down beside them and does them no harm.
    for _ in 0..150 {
        stand(&mut game, victim, spot);
        tick_hand(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, 0, Some(over), false);
    }
    assert_eq!(game.world.entities.of::<Player>(victim).held, None);
    assert_eq!(hp(&game, victim), PLAYER_MAX_HP, "an open hand coming down on them does not hurt");
    // It closes on them.
    for _ in 0..5 {
        tick_hand(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, 0, Some(over), true);
    }
    assert_eq!(game.world.entities.of::<Player>(victim).held, Some(Fist { titan: vid, hand: 0 }), "picked up in the left hand");
    assert_eq!(game.seen(victim).unwrap().held, Some(shared::replicated::Held { vehicle: vid, hand: 0 }));
    assert!(game.seen(vid).unwrap().titan.unwrap().flags & HANDS_FULL_LEFT != 0, "everyone sees the hand is full");
    // The Titan stands, and lifts them high.
    for _ in 0..360 {
        let pose = game.world.physics.car_pose(vid).unwrap();
        let ahead = pose.world_point([-TITAN_SHOULDER[0], 0.0, -15.0]);
        tick_hand(&mut game, vid, pilot, &mut seq, 0, 0, Some(ahead), true);
    }
    let carried = game.world.entities.of::<Player>(victim).character;
    assert!(carried.position[1] > 25.0 && carried.down == 0, "carried high, on their feet: {carried:?}");
    let fist = game.world.physics.limbs(vid).unwrap().fists_world[0];
    assert!(vec3::distance(carried.position, held_at(fist)) < 1.5, "on the fist: {carried:?} against {fist:?}");
    // They can still shoot from up there.
    let ammo = game.world.entities.of::<Player>(victim).hands.hotbar[0].ammo;
    for n in 0..30 {
        game.handle(Command::Input(victim, 0, vec![InputFrame { seq: n + 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_FIRE, pitch: 0, view: 0, slot: 0 }]));
        let pose = game.world.physics.car_pose(vid).unwrap();
        tick_hand(&mut game, vid, pilot, &mut seq, 0, 0, Some(pose.world_point([-TITAN_SHOULDER[0], 0.0, -15.0])), true);
    }
    assert!(game.world.entities.of::<Player>(victim).hands.hotbar[0].ammo < ammo, "a held soldier fires their rifle");
    // Swung forward and let go, they fly.
    for _ in 0..14 {
        let pose = game.world.physics.car_pose(vid).unwrap();
        tick_hand(&mut game, vid, pilot, &mut seq, 0, 0, Some(pose.world_point([-TITAN_SHOULDER[0], 6.0, -40.0])), true);
    }
    let before = hp(&game, victim);
    let pose = game.world.physics.car_pose(vid).unwrap();
    tick_hand(&mut game, vid, pilot, &mut seq, 0, 0, Some(pose.world_point([-TITAN_SHOULDER[0], 6.0, -40.0])), false);
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

#[test]
fn a_slam_throws_and_hurts_soldiers_throws_a_jeep_fells_trees_and_brings_down_a_house() {
    use shared::world::{Building, CollisionMesh, ObjId, TREE_PINE, Tree};
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![v(8.0, 0.0, -5.0), v(18.0, 0.0, -5.0), v(18.0, 0.0, 5.0), v(8.0, 0.0, 5.0), v(8.0, 6.0, -5.0), v(18.0, 6.0, -5.0), v(18.0, 6.0, 5.0), v(8.0, 6.0, 5.0)];
    let triangles = vec![[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
    let house = Building { height: 600, footprint: vec![[800, -500], [1800, -500], [1800, 500], [800, 500]], base: 0, first: 0, walls: 8, roof: 2, ..Default::default() };
    let (house_id, near_tree, far_tree) = (ObjId { col: 1, row: 1, index: 0 }, ObjId { col: 2, row: 3, index: 0 }, ObjId { col: 2, row: 3, index: 1 });
    let tree = |x: i32, z: i32| Tree { x, z, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true };
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 40_000,
        spawns: vec![[30_000, 30_000]],
        buildings: vec![house],
        building_ids: vec![house_id],
        building_triangles: vec![[0, 12]],
        collision: CollisionMesh { vertices, triangles },
        trees: vec![tree(-1500, 1000), tree(-9000, 0)],
        tree_ids: vec![near_tree, far_tree],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    let pilot = join(&mut game);
    let (near, edge, far) = (join(&mut game), join(&mut game), join(&mut game));
    // The Titan stands well away: only where it is said to land matters here.
    let vid = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_TITAN, [300.0, TITAN_LEG_LENGTH + TITAN_HALF_EXTENTS[1], 300.0], 0.0);
    let jeep = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_JEEP, [0.0, 1.0, -22.0], 0.0);
    for _ in 0..120 {
        game.step();
    }
    stand(&mut game, near, [0.0, 12.0]);
    stand(&mut game, edge, [0.0, 70.0]);
    stand(&mut game, far, [0.0, 110.0]);
    let (full, titan_hp) = (hp(&game, jeep), hp(&game, vid));
    // As hard as it lands: its feet either side of the origin.
    slam(&mut game.world, vid, [[-5.0, 0.0, 0.0], [5.0, 0.0, 0.0]], TITAN_SLAM_FULL, Some(pilot));
    let thrown = game.world.entities.of::<Player>(near).character;
    assert!(thrown.knock[1] > 10.0 && thrown.vy > 0.0 && thrown.down > 0, "thrown outwards and knocked down: {thrown:?}");
    assert!(hp(&game, near) < PLAYER_MAX_HP || !game.world.entities.alive(near), "and hurt");
    // Past where it hurts it still knocks them off their feet, and further out it does nothing.
    let shaken = game.world.entities.of::<Player>(edge).character;
    assert!(shaken.knock[1] > 1.0 && hp(&game, edge) == PLAYER_MAX_HP, "shaken but not hurt: {shaken:?}");
    let untouched = game.world.entities.of::<Player>(far).character;
    assert!(untouched.knock == [0.0, 0.0] && untouched.grounded, "out of its reach: {untouched:?}");
    assert!(hp(&game, jeep) < full / 2 || game.world.entities.dead(jeep), "the jeep is broken: {} of {full}", hp(&game, jeep));
    assert!(game.world.felled.contains(&Obj::Tree(near_tree)) && !game.world.felled.contains(&Obj::Tree(far_tree)), "the tree by it goes over, the far one stands");
    assert!(game.world.carves.contains_key(&house_id), "the house by it is broken open");
    assert_eq!(hp(&game, vid), titan_hp, "it does not feel its own landing");
    // The jeep flies.
    let before = game.world.physics.car_pose(jeep).unwrap().position;
    for _ in 0..30 {
        game.step();
    }
    let after = game.world.physics.car_pose(jeep).unwrap().position;
    assert!(after[2] < before[2] - 2.0 && after[1] > 1.5, "the jeep is thrown away from it and up: {before:?} -> {after:?}");
}

#[test]
fn a_titan_that_leaps_and_falls_slams_where_it_lands_and_one_set_down_gently_does_not() {
    for gentle in [false, true] {
        let mut game = game();
        let pilot = join(&mut game);
        let (bystander, held) = (join(&mut game), join(&mut game));
        let vid = titan(&mut game);
        assert!(crate::mechanics::seat(&mut game.world, pilot, vid, 0));
        let mut seq = 0;
        // Someone in its right fist goes up with it.
        game.world.entities.of_mut::<Player>(held).held = Some(Fist { titan: vid, hand: 1 });
        // Up on its thrusters for three seconds, then let go.
        for _ in 0..180 {
            tick(&mut game, vid, pilot, &mut seq, BUTTON_JUMP, None, true);
        }
        let high = game.world.physics.car_pose(vid).unwrap();
        assert!(high.position[1] > TITAN_RIDE_HEIGHT + 20.0, "it flew: {high:?}");
        stand(&mut game, bystander, [high.position[0] + 15.0, high.position[2]]);
        let mut impact: f32 = 0.0;
        for _ in 0..600 {
            // Set down gently, the thrusters are fired whenever it falls fast.
            let falling = game.world.physics.car_pose(vid).unwrap().velocity[1] < -4.0 && game.world.physics.limbs(vid).unwrap().step == shared::vehicles::Step::Airborne;
            tick(&mut game, vid, pilot, &mut seq, if gentle && falling { BUTTON_JUMP } else { 0 }, None, true);
            // The bystander waits where they were put until it lands.
            if impact == 0.0 {
                stand(&mut game, bystander, [high.position[0] + 15.0, high.position[2]]);
            }
            impact = impact.max(game.world.physics.limbs(vid).unwrap().impact);
        }
        let seen = game.world.entities.of::<Player>(bystander).character;
        println!("titan landed (gentle: {gentle}) at {impact} m/s; bystander {seen:?}, hp {}", hp(&game, bystander));
        if gentle {
            assert!(impact > 0.0 && impact < TITAN_SLAM_SPEED, "set down: {impact} m/s");
            // Its feet coming down still shake the ground under them, as any step of its does.
            assert!(hp(&game, bystander) == PLAYER_MAX_HP && (seen.position[0] - high.position[0] - 15.0).abs() < 5.0, "the bystander is unhurt and where they were, near enough: {seen:?}");
        } else {
            assert!(impact > TITAN_SLAM_SPEED + 5.0, "it came down hard: {impact} m/s");
            assert!(hp(&game, bystander) < PLAYER_MAX_HP || !game.world.entities.alive(bystander), "the bystander is hurt");
            assert!(seen.position[0] > high.position[0] + 17.0 || !game.world.entities.alive(bystander), "and thrown away from it: {seen:?}");
        }
        assert_eq!(hp(&game, held), PLAYER_MAX_HP, "whoever it holds comes down with it unhurt");
        assert!(game.world.entities.of::<Player>(held).held.is_some(), "and still in its fist");
        assert_eq!(hp(&game, vid), vehicle_def(KIND_TITAN).hp, "and it takes no harm itself");
    }
}

#[test]
fn a_blocked_ap_shell_still_knocks_the_vehicle() {
    use crate::mechanics::armor::{Outcome, armor_of};
    use crate::mechanics::projectiles::{Flying, pen_of};
    use shared::protocol::PROJECTILE_SHELL;
    let mut game = game();
    let vid = titan(&mut game);
    let pose = game.world.physics.car_pose(vid).unwrap();
    // At its chest from 12 m out, twenty degrees off its nose: too oblique for a shell to get through.
    let at = pose.world_point([0.0, 4.0, -4.0]);
    let off = 20f32.to_radians();
    let dir = pose.world_dir([-off.sin(), 0.0, off.cos()]);
    let from = vec3::along(at, dir, -12.0);
    let outcome = armor_of(&game.world, vid, from, dir, 20.0, pen_of(PROJECTILE_SHELL).near);
    assert!(matches!(outcome, Some(Outcome::Blocked { .. })), "the chest plate stops it: {outcome:?}");
    let still = game.world.physics.car_motion(vid).unwrap().angvel;
    assert!(vec3::length(still) < 0.01, "it stands still before: {still:?}");
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying::new(PROJECTILE_SHELL, 0, from, dir.map(|v| v * SHELL_SPEED), SHELL_TTL_TICKS));
    for _ in 0..10 {
        game.step();
    }
    assert!(!game.world.in_flight().contains_key(&id), "the shell is spent");
    let motion = game.world.physics.car_motion(vid).unwrap();
    let rocked = game.world.physics.car_pose(vid).unwrap();
    println!("titan after a blocked shell: {motion:?}, up {:?}", rocked.world_dir([0.0, 1.0, 0.0]));
    assert!(vec3::length(motion.angvel) > 0.02, "the hit rocks it although it did not get in: {motion:?}");
    assert!(rocked.world_dir([0.0, 1.0, 0.0])[2] > 0.0, "back, away from the hit: {rocked:?}");
}

/// A game with a piloted Titan standing at the origin: the Titan, its pilot and the pilot's input count.
fn piloted() -> (Game, u32, u32, u32) {
    let mut game = game();
    let pilot = join(&mut game);
    let vid = titan(&mut game);
    assert!(crate::mechanics::seat(&mut game.world, pilot, vid, 0));
    (game, vid, pilot, 0)
}

/// The shells in the air that Titan `vid` fired.
fn shells_from(game: &Game, vid: u32) -> Vec<crate::mechanics::projectiles::Flying> {
    game.world.in_flight().into_values().filter(|flying| flying.source == vid).collect()
}

/// Where the right fist is asked to be to hold the gun up ahead of the Titan at `vid`: out in front of
/// the right shoulder, a little low.
fn gun_up(game: &Game, vid: u32) -> [f32; 3] {
    game.world.physics.car_pose(vid).unwrap().world_point([TITAN_SHOULDER[0], TITAN_SHOULDER[1] - 4.0, -18.0])
}

#[test]
fn a_titans_fist_cannon_fires_along_its_forearm_from_the_muzzle() {
    let (mut game, vid, pilot, mut seq) = piloted();
    for _ in 0..120 {
        let at = gun_up(&game, vid);
        tick(&mut game, vid, pilot, &mut seq, 0, Some(at), false);
    }
    assert!(shells_from(&game, vid).is_empty(), "nothing fires until the trigger is pulled");
    let (elbow, fist) = game.world.physics.forearm(vid, TITAN_GUN_HAND).unwrap();
    // An input reaches the server's tick after it is sent.
    for _ in 0..2 {
        let at = gun_up(&game, vid);
        tick(&mut game, vid, pilot, &mut seq, BUTTON_ALT, Some(at), false);
    }
    let shells = shells_from(&game, vid);
    assert_eq!(shells.len(), 1, "one round: {shells:?}");
    let shell = shells[0];
    assert_eq!(shell.kind, PROJECTILE_ROTARY, "its heavy round");
    let along = vec3::sub(fist, elbow).map(|v| v / vec3::distance(fist, elbow));
    let heading = shell.velocity.map(|v| v / vec3::length(shell.velocity));
    assert!(vec3::dot(heading, along) > 0.99, "along the forearm: {heading:?} against {along:?}");
    let out = vec3::distance(shell.position, fist);
    assert!(out > TITAN_GUN_LENGTH - 0.5 && out < TITAN_GUN_LENGTH + 5.0, "from the muzzle, {out} m out of the fist");
    assert_eq!(game.seen(vid).unwrap().turret.unwrap().shots, 1, "everyone sees it fire");
    assert!((vec3::length(shell.velocity) - TITAN_ROUND_SPEED).abs() < 2.0, "{:?}", shell.velocity);
    // Held, it keeps firing, two and a half rounds a second.
    for _ in 0..2 * TICK_RATE {
        let at = gun_up(&game, vid);
        tick(&mut game, vid, pilot, &mut seq, BUTTON_ALT, Some(at), false);
    }
    let shots = game.seen(vid).unwrap().turret.unwrap().shots;
    assert!((5..=6).contains(&shots), "{shots} rounds in two seconds and a tick");
}

#[test]
fn a_titans_cannon_holds_fire_while_shift_throws_its_fist() {
    let (mut game, vid, pilot, mut seq) = piloted();
    for _ in 0..60 {
        let at = gun_up(&game, vid);
        tick(&mut game, vid, pilot, &mut seq, BUTTON_ALT | BUTTON_RUN, Some(at), true);
    }
    assert!(shells_from(&game, vid).is_empty(), "a punch is not a burst");
    assert_eq!(game.seen(vid).unwrap().turret.unwrap().shots, 0);
}

#[test]
fn a_titans_gun_hand_punches_but_never_grips() {
    let (mut game, vid, pilot, mut seq) = crouched();
    let victim = join(&mut game);
    let spot = [TITAN_SHOULDER[0], -12.0];
    stand(&mut game, victim, spot);
    let over = [spot[0], TITAN_FIST_RADIUS + 0.2, spot[1]];
    for _ in 0..150 {
        stand(&mut game, victim, spot);
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, Some(over), false);
    }
    for _ in 0..10 {
        tick(&mut game, vid, pilot, &mut seq, BUTTON_CROUCH, Some(over), true);
    }
    assert_eq!(game.world.entities.of::<Player>(victim).held, None, "the hand with the gun closes on nobody");
    assert_eq!(game.seen(vid).unwrap().titan.unwrap().flags & HANDS_FULL_RIGHT, 0);
}

/// One tick of `pilot` holding `buttons` and the stick `forward`, with no hands sent.
fn drive(game: &mut Game, pilot: u32, seq: &mut u32, buttons: u16, forward: i8) {
    *seq += 1;
    game.handle(Command::Input(pilot, 0, vec![InputFrame { seq: *seq, strafe: 0, forward, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
    game.step();
}

#[test]
fn a_titans_shoulder_tube_launches_a_missile_its_pilot_flies_while_it_stands() {
    use crate::game::Seat;
    let (mut game, vid, pilot, mut seq) = piloted();
    assert_eq!(game.seen(vid).unwrap().arms.unwrap().bombs, TITAN_TUBES, "its tubes are loaded");
    // An input reaches the server's tick after it is sent.
    drive(&mut game, pilot, &mut seq, BUTTON_CHUTE, 0);
    drive(&mut game, pilot, &mut seq, BUTTON_CHUTE, 0);
    let missile = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_MISSILE).map(|(id, _)| id).expect("a missile goes up");
    assert_eq!(game.world.entities.of::<Player>(pilot).seat, Some(Seat { vehicle: missile, index: 0, remote: true, from: Some((vid, 0)) }));
    assert_eq!(game.world.entities.of::<Seats>(vid).rider(0), Some(pilot), "their body stays in the Titan");
    assert_eq!(game.seen(vid).unwrap().arms.unwrap().bombs, TITAN_TUBES - 1);
    let pose = game.world.physics.car_pose(vid).unwrap();
    let up = pose.local_dir(vec3::sub(game.world.physics.car_pose(missile).unwrap().position, pose.position));
    assert!(up[0] < -6.0 && up[1] > 11.0, "out of the left shoulder: {up:?}");
    // Flying it, stick forward and the gun's trigger held, the Titan neither walks nor fires.
    let stood = pose.position;
    for _ in 0..90 {
        drive(&mut game, pilot, &mut seq, BUTTON_ALT, 127);
    }
    let moved = game.world.physics.car_pose(vid).unwrap().position;
    assert!(vec3::distance([moved[0], 0.0, moved[2]], [stood[0], 0.0, stood[2]]) < 1.0, "it stood: {stood:?} to {moved:?}");
    assert!(shells_from(&game, vid).is_empty(), "the missile's stick and trigger are not the Titan's");
    let flown = game.world.physics.car_pose(missile).unwrap().position;
    assert!(flown[1] > stood[1] + 20.0, "the missile climbed: {flown:?}");
    // A fresh press sets it off, and the pilot is back in the Titan.
    drive(&mut game, pilot, &mut seq, 0, 0);
    drive(&mut game, pilot, &mut seq, BUTTON_FIRE, 0);
    drive(&mut game, pilot, &mut seq, 0, 0);
    drive(&mut game, pilot, &mut seq, 0, 0);
    assert!(!game.world.entities.has::<Vehicle>(missile), "it went off");
    assert_eq!(game.world.entities.of::<Player>(pilot).seat, Some(Seat { vehicle: vid, index: 0, remote: false, from: None }));
    // The tube loads again.
    for _ in 0..TITAN_TUBE_RELOAD {
        game.step();
    }
    assert_eq!(game.seen(vid).unwrap().arms.unwrap().bombs, TITAN_TUBES, "loaded again");
}


/// A game with a house 10 m square and 6 m high from x = 55 to 65 and z = -5 to 5, and its id.
fn with_house() -> (Game, shared::world::ObjId) {
    use shared::world::{Building, CollisionMesh, ObjId};
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![v(55.0, 0.0, -5.0), v(65.0, 0.0, -5.0), v(65.0, 0.0, 5.0), v(55.0, 0.0, 5.0), v(55.0, 6.0, -5.0), v(65.0, 6.0, -5.0), v(65.0, 6.0, 5.0), v(55.0, 6.0, 5.0)];
    let triangles = vec![[0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2]];
    let house = Building { height: 600, footprint: vec![[5500, -500], [6500, -500], [6500, 500], [5500, 500]], base: 0, first: 0, walls: 8, roof: 2, ..Default::default() };
    let house_id = ObjId { col: 1, row: 1, index: 0 };
    let map = WorldFile { version: WORLD_VERSION, half_extent: 40_000, spawns: vec![[30_000, 30_000]], buildings: vec![house], building_ids: vec![house_id], building_triangles: vec![[0, 12]], collision: CollisionMesh { vertices, triangles }, ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    (game, house_id)
}

/// How many of `house`'s cells a round of `kind` flying at `speed` takes out of its wall at x = 55, met
/// square on 3 m up.
fn cells_taken(kind: u8, speed: f32) -> usize {
    use crate::mechanics::projectiles::Flying;
    let (mut game, house) = with_house();
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying::new(kind, 0, [35.0, 3.0, 0.0], [speed, 0.0, 0.0], SHELL_TTL_TICKS));
    for _ in 0..30 {
        game.step();
    }
    game.world.carves.get(&house).map_or(0, |carving| carving.removed.iter().filter(|&&removed| removed).count())
}

#[test]
fn a_40mm_armour_piercing_round_punches_a_hole_in_a_house_wall() {
    use crate::mechanics::projectiles::burst_of;
    assert!(burst_of(PROJECTILE_CANNON_AP).unwrap().radius * crate::mechanics::combat::CARVE_REACH < 0.8, "its charge alone bites nothing");
    let gone = cells_taken(PROJECTILE_CANNON_AP, SHELL_SPEED);
    assert!(gone >= 1, "a piece of wall is gone: {gone} cells");
}

#[test]
fn a_titans_heavy_round_bites_a_wall_but_less_than_a_mortar_bomb() {
    let heavy = cells_taken(PROJECTILE_ROTARY, TITAN_ROUND_SPEED);
    let mortar = cells_taken(PROJECTILE_MORTAR, TITAN_ROUND_SPEED);
    assert!(heavy >= 8, "a hole in the wall: {heavy} cells");
    assert!(heavy * 4 < mortar, "far smaller than a mortar bomb's: {heavy} cells to {mortar}");
}
