//! Bridges: carved as houses are, but standing on the ground under them and much harder to hurt.

use super::super::combat::{Blast, blast, hit_object};
use crate::entities::Health;
use crate::game::{Command, Game, Player};
use shared::consts::*;
use shared::protocol::{WEAPON_AUTOCANNON, WEAPON_SHELL};
use shared::world::{BUILDING_BRIDGE, Building, CollisionMesh, ObjId, WORLD_VERSION, WorldFile};
use sim::{Obj, STANDING_Y};
use tokio::sync::{mpsc, oneshot};

/// The six faces of a closed box from `lo` to `hi`, metres, as triangles of `vertices` (centimetres):
/// its walls, then its top.
fn cuboid(vertices: &mut Vec<[i32; 3]>, lo: [f32; 3], hi: [f32; 3]) -> (Vec<[u32; 3]>, Vec<[u32; 3]>) {
    let first = vertices.len() as u32;
    for k in 0..8 {
        let pick = |axis: usize, bit: usize| if k >> bit & 1 == 1 { hi[axis] } else { lo[axis] };
        vertices.push([pick(0, 0), pick(1, 1), pick(2, 2)].map(|v| (v * 100.0).round() as i32));
    }
    let c = |x: u32, y: u32, z: u32| first + (x | (y << 1) | (z << 2));
    let quad = |a, b, d, e| [[a, b, d], [a, d, e]];
    let mut walls = Vec::new();
    walls.extend(quad(c(0, 0, 0), c(1, 0, 0), c(1, 1, 0), c(0, 1, 0)));
    walls.extend(quad(c(0, 0, 1), c(0, 1, 1), c(1, 1, 1), c(1, 0, 1)));
    walls.extend(quad(c(0, 0, 0), c(0, 1, 0), c(0, 1, 1), c(0, 0, 1)));
    walls.extend(quad(c(1, 0, 0), c(1, 0, 1), c(1, 1, 1), c(1, 1, 0)));
    walls.extend(quad(c(0, 0, 0), c(0, 0, 1), c(1, 0, 1), c(1, 0, 0)));
    (walls, quad(c(0, 1, 0), c(0, 1, 1), c(1, 1, 1), c(1, 1, 0)).to_vec())
}

/// A game on flat ground with a 10 x 10 m, 6 m tall house with its middle at (35, 0), and a bridge
/// section: a deck 24 m long and 8 m wide from 6 m to 6.7 m up, its middle at the origin, on a pier
/// 1.8 m by 6 m standing in the ground under its middle. Returns the game, the house and the bridge.
fn game() -> (Game, ObjId, ObjId) {
    let mut vertices = Vec::new();
    let (house_walls, house_roof) = cuboid(&mut vertices, [30.0, 0.0, -5.0], [40.0, 6.0, 5.0]);
    let (deck_walls, deck_top) = cuboid(&mut vertices, [-12.0, 6.0, -4.0], [12.0, 6.7, 4.0]);
    let (pier_walls, pier_top) = cuboid(&mut vertices, [-0.9, -0.5, -3.0], [0.9, 6.0, 3.0]);
    let mut triangles: Vec<[u32; 3]> = house_walls.iter().chain(&house_roof).copied().collect();
    let bridge_walls: Vec<[u32; 3]> = deck_walls.into_iter().chain(pier_walls).chain(pier_top).collect();
    triangles.extend(&bridge_walls);
    triangles.extend(&deck_top);
    let house = Building { height: 600, footprint: vec![[3000, -500], [4000, -500], [4000, 500], [3000, 500]], base: 0, first: 0, walls: 10, roof: 2, ..Default::default() };
    let bridge = Building {
        height: 720,
        footprint: vec![[-1200, -400], [1200, -400], [1200, 400], [-1200, 400]],
        base: -50,
        first: 12,
        walls: bridge_walls.len() as u32,
        roof: deck_top.len() as u32,
        kind: BUILDING_BRIDGE,
        ..Default::default()
    };
    let ids = [ObjId { col: 1, row: 1, index: 0 }, ObjId { col: 1, row: 1, index: 1 }];
    let end = triangles.len() as u32;
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 20_000,
        spawns: vec![[-5000, -5000]],
        buildings: vec![house, bridge],
        building_ids: ids.to_vec(),
        building_triangles: vec![[0, 12], [12, end]],
        collision: CollisionMesh { vertices, triangles },
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    (game, ids[0], ids[1])
}

/// How many cells of building `id` are gone.
fn lost(game: &Game, id: ObjId) -> usize {
    game.world.carves.get(&id).map_or(0, |carving| carving.removed.iter().filter(|&&gone| gone).count())
}

fn shell(point: [f32; 3], radius: f32, damage: f32, weapon: u8) -> Blast {
    Blast { point, radius, damage, knock: 0.0, dir: [0.0, -1.0, 0.0], attacker: None, weapon }
}

/// The top of the deck, 5 m along it from the pier.
const ON_DECK: [f32; 3] = [5.0, 6.7, 0.0];

#[test]
fn rifle_and_machine_gun_rounds_leave_a_bridge_untouched() {
    let (mut game, _, bridge) = game();
    let (from, down) = ([5.0, 9.0, 0.0], [0.0, -1.0, 0.0]);
    let (distance, object) = game.world.physics.cast_ray_object(from, down, 10.0).expect("the deck is under the gun");
    assert_eq!(object, Some(Obj::Building(bridge)));
    for _ in 0..200 {
        hit_object(&mut game.world, from, down, distance, 255);
    }
    assert_eq!(lost(&game, bridge), 0);
    assert!(game.world.fresh.carved.is_empty() && game.world.object_hp.is_empty());
}

#[test]
fn forty_millimetre_rounds_only_wear_a_bridge_down_slowly() {
    let (mut game, _, bridge) = game();
    let round = || shell(ON_DECK, CANNON_RADIUS, CANNON_DAMAGE, WEAPON_AUTOCANNON);
    for hit in 0..11 {
        blast(&mut game.world, round());
        assert_eq!(lost(&game, bridge), 0, "a cell broke after {} rounds", hit + 1);
    }
    blast(&mut game.world, round());
    assert_eq!(lost(&game, bridge), 1, "the twelfth round breaks one cell");
}

#[test]
fn a_tank_shell_bites_a_bridge_less_than_a_house() {
    let (mut game, house, bridge) = game();
    blast(&mut game.world, shell(ON_DECK, EXPLOSION_RADIUS, EXPLOSION_DAMAGE, WEAPON_SHELL));
    blast(&mut game.world, shell([30.0, 3.0, 0.0], EXPLOSION_RADIUS, EXPLOSION_DAMAGE, WEAPON_SHELL));
    assert!(!game.world.object_hp.contains_key(&Obj::Building(bridge)), "a bridge has no health to lose");
    let (bridge, house) = (lost(&game, bridge), lost(&game, house));
    assert!(bridge > 0, "a tank shell bites a bridge");
    assert!(bridge * 4 < house, "but far less than a house: {bridge} cells to {house}");
}

#[test]
fn a_deck_cut_off_from_its_pier_falls_and_the_rest_stands() {
    let (mut game, _, bridge) = game();
    // A cut right across the deck 4 m from the pier.
    for z in [-3.5, -1.5, 0.5, 2.5, 4.0] {
        game.world.carve(bridge, [4.0, 6.35, z], 1.2);
    }
    let carving = &game.world.carves[&bridge];
    let standing = |id: u32| !carving.removed[id as usize];
    let solid: Vec<u32> = (0..carving.removed.len() as u32).filter(|&id| carving.shell.is_solid(id)).collect();
    let far = solid.iter().filter(|&&id| carving.shell.centre(id)[0] > 600).filter(|&&id| standing(id)).count();
    let near = solid.iter().filter(|&&id| carving.shell.centre(id)[0] < 200).filter(|&&id| standing(id)).count();
    assert_eq!(far, 0, "the end of the deck past the cut fell");
    assert!(near > 100, "the deck on the pier stands: {near}");
    assert!(!game.world.rubble.is_empty());
}

#[test]
fn a_bridge_whose_pier_is_blown_drops_its_deck_on_whoever_is_under_it() {
    let (mut game, _, bridge) = game();
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    let under = response.try_recv().unwrap().unwrap();
    std::mem::forget(inbox);
    game.world.entities.get_mut::<Player>(under).unwrap().character.position = [8.0, STANDING_Y, 0.0];
    // Bites right across the pier, 2.5 m up.
    let fell: usize = [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0].into_iter().filter_map(|z| game.world.carve(bridge, [0.0, 2.5, z], 2.0)).map(|cut| cut.fell.len()).sum();
    let carving = &game.world.carves[&bridge];
    let left: Vec<[i32; 3]> = (0..carving.removed.len() as u32).filter(|&id| carving.shell.is_solid(id) && !carving.removed[id as usize]).map(|id| carving.shell.centre(id)).filter(|c| c[1] > 200 && c[1] < 300).collect();
    assert!(left.is_empty(), "pier left at {left:?}");
    assert!(fell > 200, "the deck has nothing left under it: {fell} fell");
    // About as long as 7 m of fall takes, and a little more.
    for _ in 0..2 * TICK_RATE {
        game.step();
    }
    assert!(game.world.rubble.is_empty(), "it has all landed");
    assert!(game.world.entities.of::<Health>(under).hp < PLAYER_MAX_HP, "the deck came down on them");
}

#[test]
fn a_rocket_holing_a_pier_leaves_the_deck_standing() {
    let (mut game, _, bridge) = game();
    // A Carl Gustaf round's bite (a fifth of a rocket's blast, `blast_bridge`) in the pier's side, 2.5 m up.
    let cut = game.world.carve(bridge, [0.9, 2.5, 1.5], ROCKET_BLAST_RADIUS * 0.2).expect("a bite");
    let bite = cut.blasted.len() + cut.fell.len();
    // A bridge is not weighed as a house is (`Shell::strain`): on the real viaduct, every cell of a
    // pier carrying the whole deck, a hole in its side brought the section down.
    assert!(!game.world.strained.contains_key(&bridge), "a bridge is never queued for a structure check");
    for _ in 0..10 * TICK_RATE {
        game.world.tick = game.world.tick.wrapping_add(1);
        game.world.settle();
    }
    assert_eq!(lost(&game, bridge), bite, "only the bite goes");
}

#[test]
fn the_fixture_worlds_bridge_is_solid_and_can_be_hit() {
    // As the importer bakes it: a section has no triangles left in the mesh, only its cells and look.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack");
    let (map, _) = crate::read_world(&path).unwrap();
    let sections: Vec<(ObjId, [f32; 3])> = map
        .buildings
        .iter()
        .zip(&map.building_ids)
        .filter(|(building, _)| building.kind == BUILDING_BRIDGE)
        .map(|(building, &id)| {
            let [x, z] = building.centre();
            (id, [x as f32 / 100.0, (building.base + building.height) as f32 / 100.0 + 5.0, z as f32 / 100.0])
        })
        .collect();
    assert!(sections.len() >= 2, "the fixture has a bridge in sections");
    let game = Game::new(map, 0).unwrap();
    for (id, above) in sections {
        let (_, object) = game.world.physics.cast_ray_object(above, [0.0, -1.0, 0.0], 50.0).expect("the deck is under the gun");
        assert_eq!(object, Some(Obj::Building(id)), "a round dropped on section {id:?} meets it");
    }
}
