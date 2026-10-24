use super::*;
use crate::game::{Command, Game, Outgoing, World};
use shared::carve::CELL;
use shared::construct::{cell_at, centre};
use shared::items::{ITEM_BRICKS, def};
use shared::protocol::{Built, InputFrame, ItemSlot, S_BUILT, decode, peek_tag};
use shared::quant::{from_cm, to_mrad};
use shared::world::{Building, CollisionMesh, ObjId, WORLD_VERSION, WorldFile};
use sim::STANDING_Y;
use tokio::sync::{mpsc, oneshot};

/// A game on flat ground with a 10 x 10 m, 6 m tall house with its middle at (20, 0), as a closed box
/// mesh, in tile (1, 1): its west wall's cells are lattice column x = 20.
fn game() -> (Game, ObjId) {
    let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
    let vertices = vec![
        v(15.0, 0.0, -5.0), v(25.0, 0.0, -5.0), v(25.0, 0.0, 5.0), v(15.0, 0.0, 5.0),
        v(15.0, 6.0, -5.0), v(25.0, 6.0, -5.0), v(25.0, 6.0, 5.0), v(15.0, 6.0, 5.0),
    ];
    let triangles = vec![
        [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
        [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
    ];
    let building = Building { height: 600, footprint: vec![[1500, -500], [2500, -500], [2500, 500], [1500, 500]], base: 0, first: 0, walls: 8, roof: 2, ..Default::default() };
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
    (game, id)
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// A player standing at `(x, z)` facing -Z with bricks in slot 0.
fn builder(game: &mut Game, x: f32, z: f32) -> u32 {
    let (id, inbox) = join(game);
    std::mem::forget(inbox);
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.character.position = [x, STANDING_Y, z];
    player.hotbar[0] = ItemSlot { item: ITEM_BRICKS, count: 1, ammo: 0 };
    id
}

/// One tick of `buttons` with slot 0 in hand, facing -Z and looking `pitch` radians up.
fn press(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, pitch: f32) {
    *seq += 1;
    game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: to_mrad(pitch), view: 0, slot: 0 }]));
    game.step();
}

/// `buttons` held for a few ticks, fewer than the cooldown: one use, once the input has come through.
fn tap(game: &mut Game, id: u32, seq: &mut u32, buttons: u16) {
    for _ in 0..3 {
        press(game, id, seq, buttons, -0.5);
    }
}

/// Ticks of nothing pressed, so the next press is a fresh one and the cooldown has run out.
fn wait(game: &mut Game, id: u32, seq: &mut u32) {
    for _ in 0..def(ITEM_BRICKS).unwrap().cooldown_ticks + 1 {
        press(game, id, seq, 0, -0.5);
    }
}

/// What `Built` messages `inbox` has been sent: `(live, placed, removed, fell)`.
fn built_told(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<(bool, usize, usize, usize)> {
    let mut out = Vec::new();
    while let Ok(message) = inbox.try_recv() {
        if let Outgoing::Reliable(bytes) = message
            && peek_tag(&bytes) == Some(S_BUILT)
        {
            let built = decode::<Built>(&bytes).unwrap();
            out.push((built.live, built.placed.len(), built.removed.len(), built.fell.len()));
        }
    }
    out
}

/// A brick of `colour`.
fn brick(colour: u8) -> Cube {
    Cube { colour, material: MATERIAL_BRICK }
}

/// A beam of cubes at height row `y` along -x from the house's west wall (lattice x 19 down to `19 - len + 1`), at z row 0.
fn beam(world: &mut World, y: i32, len: i32) -> Vec<Pos> {
    let cubes: Vec<Pos> = (0..len).map(|i| [19 - i, y, 0]).collect();
    for &pos in &cubes {
        assert!(world.place_cube(pos, brick(0)));
    }
    cubes
}

#[test]
fn fire_puts_a_cube_of_the_builders_colour_on_the_ground_where_the_crosshair_is() {
    let (mut game, _) = game();
    let id = builder(&mut game, 100.0, 0.0);
    game.world.entities.get_mut::<Player>(id).unwrap().color = 7;
    let (_, mut watcher) = join(&mut game);
    let mut seq = 0;
    wait(&mut game, id, &mut seq);
    built_told(&mut watcher);
    tap(&mut game, id, &mut seq, BUTTON_FIRE);
    let cubes: Vec<(Pos, Cube)> = game.world.construct.cubes().collect();
    assert_eq!(cubes.len(), 1);
    let (pos, colour) = cubes[0];
    assert_eq!(colour, brick(7));
    assert_eq!(pos[1], 0, "it sits on the ground");
    // The crosshair is right of the body (the camera's shoulder) and ahead of it.
    let c = cube_centre(pos);
    assert!(c[0] > 100.0 && c[2] < -3.0 && c[2] > -6.0, "{c:?}");
    assert_eq!(built_told(&mut watcher), vec![(true, 1, 0, 0)]);
    // It is solid from the end of the tick.
    let down = game.world.physics.cast_ray_static([c[0], 10.0, c[2]], [0.0, -1.0, 0.0], 20.0).unwrap();
    assert!((down - (10.0 - from_cm(CELL))).abs() < 0.01, "{down}");
}

#[test]
fn held_fire_keeps_building_towards_the_crosshair() {
    let (mut game, _) = game();
    let id = builder(&mut game, 100.0, 0.0);
    let mut seq = 0;
    wait(&mut game, id, &mut seq);
    let cooldown = def(ITEM_BRICKS).unwrap().cooldown_ticks;
    for _ in 0..3 * cooldown {
        press(&mut game, id, &mut seq, BUTTON_FIRE, -0.5);
    }
    assert_eq!(game.world.construct.len(), 3, "one cube every cooldown");
}

#[test]
fn aim_and_fire_digs_the_cube_out_again() {
    let (mut game, _) = game();
    let id = builder(&mut game, 100.0, 0.0);
    let mut seq = 0;
    wait(&mut game, id, &mut seq);
    tap(&mut game, id, &mut seq, BUTTON_FIRE);
    assert_eq!(game.world.construct.len(), 1);
    wait(&mut game, id, &mut seq);
    tap(&mut game, id, &mut seq, BUTTON_FIRE | BUTTON_ALT);
    assert!(game.world.construct.is_empty());
    // Aiming at the ground digs nothing.
    wait(&mut game, id, &mut seq);
    tap(&mut game, id, &mut seq, BUTTON_FIRE | BUTTON_ALT);
    assert!(game.world.construct.is_empty());
}

#[test]
fn a_cube_needs_something_to_rest_on_and_room() {
    let (mut game, _) = game();
    let world = &mut game.world;
    assert!(!may_place(world, [100, 10, 0]), "nothing holds a cube up in the air");
    assert!(may_place(world, [100, 0, 0]), "the ground does");
    assert!(!may_place(world, [100, -2, 0]), "buried");
    assert!(may_place(world, [19, 4, 0]), "against the house's wall");
    assert!(!may_place(world, [20, 4, 0]), "in the wall");
    world.place_cube([100, 0, 0], brick(0));
    assert!(!may_place(world, [100, 0, 0]), "on another");
    assert!(may_place(world, [100, 1, 0]), "a cube holds one up");
    builder(&mut game, from_cm(centre([60, 0, 0])[0]), 0.36);
    assert!(!may_place(&mut game.world, [60, 1, 0]), "where someone stands");
}

#[test]
fn a_build_goes_on_past_where_the_building_ends() {
    let (mut game, _) = game();
    // Up the wall and out over the street, far beyond the house's own grid.
    let mut pos = [19, 4, 0];
    for _ in 0..200 {
        assert!(may_place(&mut game.world, pos), "{pos:?}");
        game.world.place_cube(pos, brick(1));
        pos[0] -= 1;
    }
    game.step();
    assert_eq!(game.world.construct.len(), 200);
    let far = cube_centre([19 - 199, 4, 0]);
    assert!(game.world.physics.cast_ray_static([far[0], 10.0, far[2]], [0.0, -1.0, 0.0], 20.0).is_some(), "its far end is solid");
}

#[test]
fn digging_out_a_towers_foot_brings_it_down_on_whoever_is_under_it() {
    let (mut game, _) = game();
    for y in 0..8 {
        assert!(game.world.place_cube([100, y, 0], brick(0)));
    }
    game.step();
    assert!(game.world.dig_cube([100, 0, 0]));
    assert!(game.world.construct.is_empty(), "nothing held the rest");
    assert_eq!(game.world.fresh_built.fell.len(), 7);
    assert!(!game.world.rubble.is_empty(), "it comes down as rubble");
}

#[test]
fn a_beam_off_a_wall_falls_when_the_wall_under_it_is_blown_out() {
    let (mut game, house) = game();
    beam(&mut game.world, 4, 5);
    game.step();
    // The wall cell the beam rests on, and those round it.
    let wall = cube_centre([20, 4, 0]);
    game.world.carve(house, wall, 1.0).expect("the wall is bitten");
    assert!(game.world.construct.is_empty(), "the beam is left hanging: {:?}", game.world.construct.cubes().collect::<Vec<_>>());
    assert_eq!(game.world.fresh_built.fell.len(), 5);
}

#[test]
fn a_blast_takes_the_cubes_in_its_reach() {
    let (mut game, _) = game();
    for x in 0..20 {
        game.world.place_cube([100 + x, 0, 0], brick(0));
    }
    game.step();
    let point = cube_centre([100, 0, 0]);
    game.world.blast_cubes(point, 3.0);
    let left = game.world.construct.len();
    assert!(left < 20 && left > 10, "{left}");
    assert!(!game.world.construct.has([100, 0, 0]));
    assert!(game.world.construct.has([119, 0, 0]));
}

#[test]
fn a_joiner_is_told_every_cube_there_is() {
    let (mut game, _) = game();
    for y in 0..4 {
        game.world.place_cube([100, y, 0], brick(2));
    }
    game.step();
    let (_, mut inbox) = join(&mut game);
    assert_eq!(built_told(&mut inbox), vec![(false, 4, 0, 0)]);
}

/// A wall of bricks standing on the ground 10 m in front of a player at (100, 0), across where they look.
fn wall_ahead(world: &mut World) -> usize {
    let z = cell_at([0.0, 0.0, -10.0])[2];
    let mut count = 0;
    for x in 130..150 {
        for y in 0..6 {
            assert!(world.place_cube([x, y, z], brick(0)));
            count += 1;
        }
    }
    count
}

/// A player at (100, 0) facing -Z with a loaded AK5 in slot 0.
fn rifleman(game: &mut Game) -> u32 {
    let id = builder(game, 100.0, 0.0);
    let ak5 = def(shared::items::ITEM_AK5).unwrap();
    game.world.entities.get_mut::<Player>(id).unwrap().hotbar[0] = ItemSlot { item: shared::items::ITEM_AK5, count: 1, ammo: ak5.mag };
    id
}

#[test]
fn a_rifle_round_hurts_a_brick_and_a_few_break_it_out() {
    let (mut game, _) = game();
    let built = wall_ahead(&mut game.world);
    let id = rifleman(&mut game);
    let mut seq = 0;
    for _ in 0..3 {
        press(&mut game, id, &mut seq, 0, 0.0);
    }
    // One round: the brick it struck is hurt, not gone.
    for _ in 0..3 {
        press(&mut game, id, &mut seq, BUTTON_FIRE, 0.0);
    }
    for _ in 0..30 {
        press(&mut game, id, &mut seq, 0, 0.0);
    }
    assert_eq!(game.world.construct.len(), built, "one round breaks nothing");
    let hurt = game.world.construct.cubes().filter(|&(pos, _)| game.world.construct.hp(pos) < Some(shared::construct::material(MATERIAL_BRICK).hp)).count();
    assert_eq!(hurt, 1, "the brick it struck is hurt");
    // A burst breaks bricks out of the wall, and everyone is told.
    for _ in 0..60 {
        press(&mut game, id, &mut seq, BUTTON_FIRE, 0.0);
    }
    for _ in 0..30 {
        press(&mut game, id, &mut seq, 0, 0.0);
    }
    assert!(game.world.construct.len() < built, "the burst broke nothing");
}

#[test]
fn shooting_out_a_towers_foot_brings_it_down() {
    let (mut game, _) = game();
    for y in 0..6 {
        game.world.place_cube([100, y, 0], brick(0));
    }
    // Rebuilt at the end of one tick, solid from the start of the next.
    game.step();
    game.step();
    let foot = cube_centre([100, 0, 0]);
    let hp = shared::construct::material(MATERIAL_BRICK).hp;
    // Rounds level into the foot from the side, a wall chip each.
    for _ in 0..hp.div_ceil(12) {
        super::super::combat::hit_object(&mut game.world, [foot[0] - 5.0, foot[1], foot[2]], [1.0, 0.0, 0.0], 5.0 - HALF_CUBE, 12);
    }
    assert!(game.world.construct.is_empty(), "{:?}", game.world.construct.cubes().collect::<Vec<_>>());
    assert_eq!(game.world.fresh_built.removed, vec![[100, 0, 0]]);
    assert_eq!(game.world.fresh_built.fell.len(), 5);
}

#[test]
fn a_car_at_speed_breaks_the_bricks_it_drives_into() {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    // The car's heading is not worth working out: a ring of wall two bricks high stands 25 m off all round.
    let r = cell_at([25.0, 0.0, 0.0])[0];
    for i in -r..=r {
        for [x, z] in [[i, r], [i, -r], [r, i], [-r, i]] {
            for y in 0..2 {
                game.world.place_cube([x, y, z], brick(0));
            }
        }
    }
    let built = game.world.construct.len();
    let (vid, _) = game.world.entities.all::<crate::game::Vehicle>().next().expect("a car is placed at the spawn point");
    let (driver, inbox) = join(&mut game);
    std::mem::forget(inbox);
    game.world.entities.get_mut::<crate::game::Seats>(vid).unwrap()[0] = Some(driver);
    game.world.entities.get_mut::<Player>(driver).unwrap().seat = Some(crate::game::Seat { vehicle: vid, index: 0, remote: false });
    for seq in 1..=TICK_RATE * 6 {
        game.handle(Command::Input(driver, 0, vec![InputFrame { seq, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
    assert!(game.world.construct.len() < built, "the car drove 6 s into a wall and broke nothing");
}
