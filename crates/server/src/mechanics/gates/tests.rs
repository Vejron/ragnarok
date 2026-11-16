use crate::game::{Command, Game, Player};
use shared::carve::{Shell, Tri};
use shared::consts::*;
use shared::protocol::{Gates, InputFrame, S_GATES, decode, peek_tag};
use shared::world::{BUILDING_BARN, Building, BuildingLook, CollisionMesh, Look, ObjId, WORLD_VERSION, WorldFile};
use sim::Character;
use tokio::sync::{mpsc, oneshot};

const BARN: ObjId = ObjId { col: 0, row: 0, index: 0 };

/// A world with one barn on flat ground: 22 × 11 m with walls 4.5 m high, its corner at the origin
/// and its long walls along x.
fn barn() -> WorldFile {
    let face = |corners: &[[i32; 3]]| -> Vec<Tri> { (1..corners.len() - 1).map(|i| { let (a, b, c) = (corners[0], corners[i], corners[i + 1]); [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]] }).collect() };
    let (w, d, h) = (2200, 1100, 450);
    let walls: Vec<Tri> = [face(&[[0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]]), face(&[[w, 0, d], [0, 0, d], [0, h, d], [w, h, d]]), face(&[[0, 0, d], [0, 0, 0], [0, h, 0], [0, h, d]]), face(&[[w, 0, 0], [w, 0, d], [w, h, d], [w, h, 0]])].concat();
    let roof: Vec<Tri> = face(&[[0, h, 0], [0, h, d], [w, h, d], [w, h, 0]]);
    let floor: Vec<Tri> = face(&[[0, 0, 0], [0, 0, d], [w, 0, d], [w, 0, 0]]);
    let mut look = Look::default();
    for t in walls.iter().chain(&roof) {
        let base = look.vertices.len() as u32;
        look.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
        look.triangles.push([base, base + 1, base + 2]);
    }
    let windows = shared::windows::openings_of(&look, [0, walls.len() as u32, roof.len() as u32], BUILDING_BARN, [0, 0]);
    let footprint = vec![[0, 0], [w, 0], [w, d], [0, d]];
    let inside: Vec<Tri> = shared::interior::rooms(&footprint, &walls, &roof, &windows, BUILDING_BARN).iter().filter(|p| p.collided()).map(|p| p.tri).collect();
    assert!(!inside.is_empty(), "a barn has a floor");
    let mut collision = CollisionMesh::default();
    for t in &floor {
        let base = collision.vertices.len() as u32;
        collision.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
        collision.triangles.push([base, base + 1, base + 2]);
    }
    let building = Building { height: h, footprint, base: 0, first: 0, walls: 0, roof: 0, roof_kind: 0, kind: BUILDING_BARN, front: [0, 0], cells: Shell::new(&walls, &roof).unwrap().cells(), paint: None };
    let shut = shared::windows::gates(&windows);
    WorldFile {
        version: WORLD_VERSION,
        half_extent: 40_000,
        spawns: vec![[-3_000, -3_000]],
        buildings: vec![building],
        building_ids: vec![BARN],
        building_triangles: vec![[0, floor.len() as u32]],
        looks: vec![BuildingLook { walls, roof, windows, inside, shut }],
        collision,
        ..Default::default()
    }
}

fn game() -> Game {
    let mut game = Game::new(barn(), 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<crate::game::Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

fn stand(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    game.world.entities.of_mut::<Player>(id).character = Character { position: [x, STAND_CENTRE, z], grounded: true, ..Default::default() };
}

fn step(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }]));
        game.step();
    }
}

/// Presses USE once and lets the press through the jitter buffer and the reshape through a step.
fn press(game: &mut Game, id: u32, seq: &mut u32) {
    step(game, id, seq, BUTTON_USE, 1);
    step(game, id, seq, 0, 6);
}

/// The barn's one gate: its number among the openings, the middle of its sill and its wall's outward normal.
fn gate(game: &Game) -> (u32, [f32; 3], [f32; 3]) {
    let look = &game.world.map.looks[0];
    let gates = shared::windows::gates(&look.windows);
    assert_eq!(gates.len(), 1, "a barn has one gate");
    let w = &look.windows[gates[0] as usize];
    let [a, b, ..] = w.corners;
    (gates[0], [(a[0] + b[0]) / 2.0, a[1], (a[2] + b[2]) / 2.0], w.normal)
}

/// How far a round fired at the gate's middle from 3 m outside it flies before it meets something.
fn round(game: &Game) -> f32 {
    let (_, mid, n) = gate(game);
    game.world.physics.cast_ray_static([mid[0] + n[0] * 3.0, 1.2, mid[2] + n[2] * 3.0], [-n[0], 0.0, -n[2]], 100.0).unwrap_or(100.0)
}

/// The `Gates` messages waiting in `inbox`.
fn told(inbox: &mut mpsc::Receiver<crate::game::Outgoing>) -> Vec<Gates> {
    let mut found = Vec::new();
    while let Ok(out) = inbox.try_recv() {
        if let crate::game::Outgoing::Reliable(bytes) = out
            && peek_tag(&bytes) == Some(S_GATES)
        {
            found.extend(decode::<Gates>(&bytes));
        }
    }
    found
}

#[test]
fn a_gate_is_shut_until_a_soldier_beside_it_opens_it_and_then_a_round_flies_in() {
    let mut game = game();
    let (id, mut inbox) = join(&mut game);
    let mut seq = 0;
    let (opening, mid, n) = gate(&game);
    step(&mut game, id, &mut seq, 0, 5);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(true));
    assert!((round(&game) - 3.0).abs() < 0.1, "a shut gate stops a round: {}", round(&game));
    // From across the yard the press does nothing.
    stand(&mut game, id, [mid[0] + n[0] * 8.0, mid[2] + n[2] * 8.0]);
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(true));
    assert!(told(&mut inbox).is_empty());
    // From beside it, it opens: the round flies on to the back wall, 11 m further.
    stand(&mut game, id, [mid[0] + n[0] * 1.5, mid[2] + n[2] * 1.5]);
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(false));
    assert!((round(&game) - 14.0).abs() < 0.1, "through the gateway to the far wall: {}", round(&game));
    let news = told(&mut inbox);
    assert_eq!(news.len(), 1);
    assert!(news[0].live && news[0].gates.len() == 1 && news[0].gates[0].open && u32::from(news[0].gates[0].door) == opening);
    // And shuts again.
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(true));
    assert!((round(&game) - 3.0).abs() < 0.1, "shut again: {}", round(&game));
    assert!(!told(&mut inbox)[0].gates[0].open);
}

#[test]
fn a_gate_does_not_shut_on_a_soldier_standing_in_it() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let (other, _other_inbox) = join(&mut game);
    let mut seq = 0;
    let (opening, mid, n) = gate(&game);
    stand(&mut game, id, [mid[0] + n[0] * 1.5, mid[2] + n[2] * 1.5]);
    stand(&mut game, other, [-20.0, -20.0]);
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(false));
    // Someone stands on the sill: the gate stays up.
    stand(&mut game, other, [mid[0], mid[2]]);
    stand(&mut game, id, [mid[0] + n[0] * 1.5, mid[2] + n[2] * 1.5]);
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(false));
    stand(&mut game, other, [-20.0, -20.0]);
    stand(&mut game, id, [mid[0] + n[0] * 1.5, mid[2] + n[2] * 1.5]);
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(true));
}

#[test]
fn a_joiner_is_told_which_gates_stand_open() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    let (opening, mid, n) = gate(&game);
    let (_, mut early) = join(&mut game);
    assert!(told(&mut early).is_empty(), "every gate is shut: nothing to tell");
    stand(&mut game, id, [mid[0] + n[0] * 1.5, mid[2] + n[2] * 1.5]);
    press(&mut game, id, &mut seq);
    let (_, mut late) = join(&mut game);
    let state = told(&mut late);
    assert_eq!(state.len(), 1);
    assert!(!state[0].live && state[0].gates.len() == 1 && state[0].gates[0].open && u32::from(state[0].gates[0].door) == opening);
}

#[test]
fn a_driver_drawn_up_to_a_shut_gate_opens_it_from_the_seat_and_gets_out_at_the_next_press() {
    let mut game = game();
    let (id, _inbox) = join(&mut game);
    let mut seq = 0;
    let (opening, mid, n) = gate(&game);
    let vid = crate::mechanics::add_tank(&mut game.world, [mid[0] + n[0] * 6.0, mid[2] + n[2] * 6.0], 0.0).expect("room for a tank in the yard");
    step(&mut game, id, &mut seq, 0, 30);
    assert!(crate::mechanics::seat(&mut game.world, id, vid, 0));
    press(&mut game, id, &mut seq);
    assert_eq!(game.world.gate_shut(BARN, opening), Some(false), "the gate went up");
    assert!(game.world.entities.of::<Player>(id).seat.is_some(), "and the driver kept their seat");
    press(&mut game, id, &mut seq);
    assert!(game.world.entities.of::<Player>(id).seat.is_none(), "an open gate takes no press: out they get");
    assert_eq!(game.world.gate_shut(BARN, opening), Some(false));
    // Getting out right at the sill does not shut the gate with the same press.
    assert!(crate::mechanics::seat(&mut game.world, id, vid, 0));
    game.world.entities.of_mut::<Player>(id).character.position = [mid[0] + n[0] * 1.5, STAND_CENTRE, mid[2] + n[2] * 1.5];
    press(&mut game, id, &mut seq);
    assert!(game.world.entities.of::<Player>(id).seat.is_none());
    assert_eq!(game.world.gate_shut(BARN, opening), Some(false));
}
