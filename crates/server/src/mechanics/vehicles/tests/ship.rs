//! The pirate ship: placed on the water with its six guns, sailed from the helm, its deck walked and
//! its guns manned.

use super::*;
use shared::protocol::{KIND_SHIP, KIND_SHIP_GUN};
use shared::world::{NO_WATER, PlacedVehicle};

/// Flat ground at 0 over 600 m in 5 m cells, with a lake 5 m deep within 200 m of the middle and a
/// ship placed in the middle of it, heading -Z.
fn sea() -> Game {
    let side = 121u32;
    let wet = |i: u32, j: u32| {
        let (x, z) = (i as f32 * 5.0 - 300.0, j as f32 * 5.0 - 300.0);
        x.abs() <= 200.0 && z.abs() <= 200.0
    };
    let (mut heights, mut water) = (Vec::new(), Vec::new());
    for j in 0..side {
        for i in 0..side {
            heights.push(if wet(i, j) { -500 } else { 0 });
            water.push(if wet(i, j) { 0 } else { NO_WATER });
        }
    }
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 30_000,
        ground: shared::terrain::Ground { origin: -30_000, cell: 500, side, heights },
        water,
        spawns: vec![[-25_000, 0]],
        placed_vehicles: vec![PlacedVehicle { kind: KIND_SHIP, at: [0, 0], yaw: 0.0 }],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn ship(game: &Game) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_SHIP).map(|(id, _)| id).unwrap()
}

/// Feeds one frame of `axes`, `buttons` and a look of `yaw` / `pitch` (radians) to each of `players`, then steps.
fn frame(game: &mut Game, players: &mut [(u32, u32)], axes: (i8, i8), buttons: u16, look: [f32; 2]) {
    for (id, seq) in players.iter_mut() {
        *seq += 1;
        let [yaw, pitch] = look.map(shared::quant::to_mrad);
        game.handle(Command::Input(*id, 0, vec![InputFrame { seq: *seq, strafe: axes.0, forward: axes.1, yaw, buttons, pitch, view: 0, slot: 0 }]));
    }
    game.step();
}

/// Stands player `id` on the ship's deck at `local` (hull frame, the ground under their feet), and
/// lets them settle there.
fn on_deck(game: &mut Game, id: u32, seq: &mut u32, local: [f32; 2]) {
    let ship = ship(game);
    let pose = game.world.physics.car_pose(ship).unwrap();
    let at = pose.world_point([local[0], SHIP_DECK_Y + STANDING_Y + 0.05, local[1]]);
    game.world.entities.get_mut::<Player>(id).unwrap().character = Character { position: at, ..Default::default() };
    let mut players = [(id, *seq)];
    for _ in 0..30 {
        frame(game, &mut players, (0, 0), 0, [0.0, 0.0]);
    }
    *seq = players[0].1;
}

/// Where player `id` stands, in the ship's frame.
fn on_ship(game: &Game, id: u32) -> [f32; 3] {
    let pose = game.world.physics.car_pose(ship(game)).unwrap();
    let at = game.world.entities.of::<Player>(id).character.position;
    pose.local_dir(vec3::sub(at, pose.position))
}

#[test]
fn a_ship_is_placed_afloat_with_its_six_guns_on_deck() {
    let game = sea();
    let ship = ship(&game);
    let pose = game.world.physics.car_pose(ship).unwrap();
    assert!((pose.position[1] - (SHIP_HALF_EXTENTS[1] - SHIP_DRAFT)).abs() < 0.3, "afloat at {:?}", pose.position);
    let guns: Vec<u32> = game.world.entities.all::<Vehicle>().filter(|(_, vehicle)| vehicle.kind == KIND_SHIP_GUN).map(|(id, _)| id).collect();
    assert_eq!(guns.len(), 6);
    assert_eq!(game.world.physics.fittings_of(ship), guns);
    for gun in guns {
        let at = game.world.physics.car_pose(gun).unwrap().position;
        let local = pose.local_dir(vec3::sub(at, pose.position));
        assert!(SHIP_GUN_SLOTS.iter().any(|(slot, _)| vec3::distance(*slot, local) < 0.05), "gun {gun} stands at a slot, not {local:?}");
    }
}

#[test]
fn the_helmsman_sails_her_and_whoever_walks_the_deck_is_carried_along() {
    let mut game = sea();
    let ship = ship(&game);
    let (helmsman, _a) = join(&mut game);
    let (walker, _b) = join(&mut game);
    let (mut seq_a, mut seq_b) = (0, 0);
    idle(&mut game, walker, &mut seq_b, 3);
    on_deck(&mut game, walker, &mut seq_b, [0.0, 1.5]);
    assert_eq!(game.world.entities.of::<Player>(walker).character.ride, ship, "standing on the deck");
    // At the helm, E takes it rather than the gun a few metres forward of it. From here on only the
    // helmsman's input comes in; the walker stands still.
    idle(&mut game, helmsman, &mut seq_a, 3);
    on_deck(&mut game, helmsman, &mut seq_a, [0.0, SHIP_HELM[2]]);
    press(&mut game, helmsman, &mut seq_a, (0, 0), BUTTON_USE, 1);
    idle(&mut game, helmsman, &mut seq_a, 2);
    let seat = game.world.entities.of::<Player>(helmsman).seat.expect("at the helm");
    assert_eq!((seat.vehicle, seat.index), (ship, 0));

    // The wind blows towards +X, so heading -Z is a beam reach.
    let start = game.world.physics.car_pose(ship).unwrap().position;
    press(&mut game, helmsman, &mut seq_a, (0, 127), 0, 20 * TICK_RATE);
    let end = game.world.physics.car_pose(ship).unwrap().position;
    let sailed = (end[0] - start[0]).hypot(end[2] - start[2]);
    assert!(sailed > 60.0, "she sailed {sailed} m");
    let walker_at = on_ship(&game, walker);
    assert_eq!(game.world.entities.of::<Player>(walker).character.ride, ship, "still aboard");
    assert!(walker_at[0].abs() < 1.0 && (walker_at[2] - 1.5).abs() < 1.0, "where they stood on deck: {walker_at:?}");

    // Leaving the helm puts them on the deck, carried along with it.
    press(&mut game, helmsman, &mut seq_a, (0, 0), BUTTON_USE, 1);
    idle(&mut game, helmsman, &mut seq_a, 20);
    assert!(game.world.entities.of::<Player>(helmsman).seat.is_none());
    let at = on_ship(&game, helmsman);
    assert!(at[0].abs() < SHIP_HALF_EXTENTS[0] && at[2].abs() < SHIP_HALF_EXTENTS[2] && at[1] > SHIP_DECK_Y, "on deck: {at:?}");
    assert_eq!(game.world.entities.of::<Player>(helmsman).character.ride, ship);
}

#[test]
fn a_gun_on_deck_is_in_the_way_of_whoever_walks_into_it() {
    let mut game = sea();
    let (walker, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, walker, &mut seq, 3);
    // In the gangway abreast of the middle starboard gun, walking to the right, into it.
    on_deck(&mut game, walker, &mut seq, [0.0, 0.5]);
    press(&mut game, walker, &mut seq, (127, 0), 0, 2 * TICK_RATE);
    let at = on_ship(&game, walker);
    let gun_inboard = SHIP_GUN_SLOTS[1].0[0] - SHIP_GUN_HALF_EXTENTS[2];
    assert!(at[0] > 0.2, "they walked across the gangway: {at:?}");
    assert!(at[0] < gun_inboard - CAPSULE_RADIUS + 0.1, "the gun stopped them at {at:?}");
    // Between two guns there is room to the bulwark.
    on_deck(&mut game, walker, &mut seq, [0.0, (SHIP_GUN_Z[1] + SHIP_GUN_Z[2]) / 2.0]);
    press(&mut game, walker, &mut seq, (127, 0), 0, 2 * TICK_RATE);
    let at = on_ship(&game, walker);
    assert!(at[0] > gun_inboard + 0.3, "they walked out between the guns: {at:?}");
    assert!(at[0] < SHIP_HALF_EXTENTS[0], "the bulwark kept them aboard: {at:?}");
}

#[test]
fn a_gunner_mans_a_gun_fires_round_shot_into_the_sea_and_steps_back_onto_the_deck() {
    let mut game = sea();
    let ship = ship(&game);
    let (gunner, _inbox) = join(&mut game);
    let mut seq = 0;
    idle(&mut game, gunner, &mut seq, 3);
    // Behind the middle starboard gun's breech.
    on_deck(&mut game, gunner, &mut seq, [SHIP_GUN_SLOTS[1].0[0] - SHIP_GUN_EXIT[2], SHIP_GUN_SLOTS[1].0[2]]);
    press(&mut game, gunner, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, gunner, &mut seq, 2);
    let seat = game.world.entities.of::<Player>(gunner).seat.expect("manning a gun");
    let gun = seat.vehicle;
    assert_eq!(game.world.entities.of::<Vehicle>(gun).kind, KIND_SHIP_GUN);
    assert_eq!(game.world.physics.mounted_on(gun), Some(ship));
    let start = game.world.physics.car_pose(gun).unwrap().position;

    // Laid out to starboard (+X is a yaw of -π/2), a little up, and fired.
    let mut players = [(gunner, seq)];
    let look = [-std::f32::consts::FRAC_PI_2, 0.1];
    for _ in 0..2 * TICK_RATE {
        frame(&mut game, &mut players, (0, 0), 0, look);
    }
    // Shot events go out with the snapshots, so follow the shot itself to where it ends.
    let flying = |game: &Game| game.world.entities.all::<crate::entities::Transform>().find(|&(id, _)| game.world.entities.has::<crate::mechanics::projectiles::Projectile>(id)).map(|(_, transform)| transform.position);
    let mut last = None;
    for tick in 0..5 * TICK_RATE {
        frame(&mut game, &mut players, (0, 0), if tick < 3 { BUTTON_FIRE } else { 0 }, look);
        match flying(&game) {
            Some(at) => last = Some(at),
            None if last.is_some() => break,
            None => {}
        }
    }
    assert!(flying(&game).is_none(), "the shot came down");
    let [x, y, z] = last.expect("the gun fired");
    assert!(x > start[0] + 30.0 && (z - start[2]).abs() < 30.0 && y.abs() < 3.0, "it came down in the sea out to starboard, at {:?}", [x, y, z]);
    let ground = game.world.map.ground.height_at(x, z);
    assert!((ground + 5.0).abs() < 0.01, "and dug nothing on the bed: {ground}");
    // The gun did not go anywhere, and nor did the ship much.
    assert!(vec3::distance(game.world.physics.car_pose(gun).unwrap().position, start) < 1.0);

    let mut seq = players[0].1;
    press(&mut game, gunner, &mut seq, (0, 0), BUTTON_USE, 1);
    idle(&mut game, gunner, &mut seq, 20);
    assert!(game.world.entities.of::<Player>(gunner).seat.is_none());
    let at = on_ship(&game, gunner);
    assert!(at[1] > SHIP_DECK_Y && at[0].abs() < SHIP_GUN_SLOTS[1].0[0], "back on the deck, inboard of the gun: {at:?}");
    assert_eq!(game.world.entities.of::<Player>(gunner).character.ride, ship);
}

#[test]
fn a_wrecked_ship_takes_her_guns_down_with_her() {
    let mut game = sea();
    let ship = ship(&game);
    let start = game.world.physics.car_pose(ship).unwrap().position;
    crate::mechanics::combat::damage_vehicle(&mut game.world, ship, u8::MAX, None);
    let guns = game.world.physics.fittings_of(ship);
    assert!(game.world.entities.dead(ship));
    assert!(guns.iter().all(|&gun| game.world.entities.dead(gun)), "every gun is wrecked with her");
    for _ in 0..8 * TICK_RATE {
        game.step();
    }
    let sunk = game.world.physics.car_pose(ship).unwrap().position;
    assert!(sunk[1] < start[1] - 2.5, "she went down: {start:?} to {sunk:?}");
    for &gun in &guns {
        let pose = game.world.physics.car_pose(gun).unwrap();
        assert!(pose.position[1] < start[1], "gun {gun} went down with her");
    }
    // Once the wreck has burnt out, a fresh one is back afloat where she was, with her guns.
    for _ in 0..(WRECK_TICKS + 5 * TICK_RATE) {
        game.step();
    }
    let back = game.world.physics.car_pose(ship).unwrap().position;
    assert!(game.world.entities.alive(ship) && guns.iter().all(|&gun| game.world.entities.alive(gun)));
    assert!((back[1] - start[1]).abs() < 0.5 && vec3::distance(back, start) < 2.0, "afloat again at {back:?}");
}
