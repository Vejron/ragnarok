use super::Mechanic;
use crate::game::{Bot, Player, World};
use sim::{Character, STANDING_Y};
use shared::consts::SPAWN_ALTITUDE;
use shared::quant::from_cm;
use shared::world::{Building, WorldFile};

/// Drops new human players in under an open parachute from `SPAWN_ALTITUDE` above a spawn point. `Combat` calls `place` again when a dead player respawns.
/// A player who chose their own spawn points (`Player::spawns`) uses them in turn; anyone else gets
/// a random outdoor one of the world's. Those are sorted centre-first by `osm-import`; picking among
/// the first few keeps players close.
pub struct Spawn;

/// How many of the most central spawn points new players are spread over.
const NEARBY_SPAWNS: usize = 6;
/// Bots spawn slightly above ground so they never start intersecting it.
const SPAWN_DROP: f32 = 0.5;
/// A chosen spawn point must be at least this far from any wall, centimetres.
const WALL_CLEARANCE: i32 = 100;

/// Whether `[x, z]`, centimetres, is somewhere a player may choose to spawn: in the world, on dry
/// ground, and clear of every building.
pub fn usable(map: &WorldFile, [x, z]: [i32; 2]) -> bool {
    map.is_inside(x, z) && !map.wet(x, z) && !map.buildings.iter().any(|building| near_building(building, [x, z]))
}

/// Whether `point` is inside `building`'s footprint or within `WALL_CLEARANCE` of a wall.
fn near_building(building: &Building, point: [i32; 2]) -> bool {
    let [min_x, min_z, max_x, max_z] = building.bounds();
    let [x, z] = point;
    if x < min_x - WALL_CLEARANCE || x > max_x + WALL_CLEARANCE || z < min_z - WALL_CLEARANCE || z > max_z + WALL_CLEARANCE {
        return false;
    }
    let (px, pz) = (x as f64, z as f64);
    let footprint = &building.footprint;
    let mut inside = false;
    for (i, a) in footprint.iter().enumerate() {
        let b = footprint[(i + 1) % footprint.len()];
        let (ax, az, bx, bz) = (a[0] as f64, a[1] as f64, b[0] as f64, b[1] as f64);
        if (az > pz) != (bz > pz) && px < ax + (pz - az) * (bx - ax) / (bz - az) {
            inside = !inside;
        }
        // Distance from the point to this wall.
        let (dx, dz) = (bx - ax, bz - az);
        let t = if dx == 0.0 && dz == 0.0 { 0.0 } else { (((px - ax) * dx + (pz - az) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0) };
        if (px - ax - t * dx).hypot(pz - az - t * dz) < WALL_CLEARANCE as f64 {
            return true;
        }
    }
    inside
}

pub(super) fn place(world: &mut World, id: u32) {
    let own = world.entities.get_mut::<Player>(id).filter(|player| !player.spawns.is_empty()).map(|mut player| {
        let point = player.spawns[player.next_spawn % player.spawns.len()];
        player.next_spawn = player.next_spawn.wrapping_add(1);
        point
    });
    let point = if let Some(point) = own {
        point
    } else if world.map.spawns.is_empty() {
        [0, 0]
    } else {
        let nearby = world.map.spawns.len().min(NEARBY_SPAWNS);
        let index = (world.rng.next_u64() % nearby as u64) as usize;
        world.map.spawns[index]
    };
    // Bots cannot steer a canopy, so they start on the ground.
    let drop = !world.entities.has::<Bot>(id);
    if let Some(mut player) = world.entities.get_mut::<Player>(id) {
        let (x, z) = (from_cm(point[0]), from_cm(point[1]));
        let ground = world.map.ground.height_at(x, z);
        player.character = Character {
            position: [x, ground + STANDING_Y + if drop { SPAWN_ALTITUDE } else { SPAWN_DROP }, z],
            chute: drop,
            ..Default::default()
        };
    }
}

impl Mechanic for Spawn {
    fn on_join(&mut self, world: &mut World, id: u32) {
        place(world, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Command, Game, Profile};
    use shared::world::{NO_WATER, WORLD_VERSION};
    use tokio::sync::{mpsc, oneshot};

    /// A 200 m square with a 10 m house at (50, 50) m and a pond sample at (-50, -50) m.
    fn map() -> WorldFile {
        let house = Building { height: 600, footprint: vec![[4_500, 4_500], [5_500, 4_500], [5_500, 5_500], [4_500, 5_500]], ..Default::default() };
        let mut map = WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[0, 0]], buildings: vec![house], ..Default::default() };
        // A 5 × 5 ground grid of 50 m cells; sample (1, 1) is at (-50, -50) m.
        map.ground = shared::terrain::Ground { origin: -10_000, cell: 5_000, side: 5, heights: vec![0; 25] };
        map.water = vec![NO_WATER; 25];
        map.water[6] = 80;
        map
    }

    #[test]
    fn usable_refuses_outside_the_world_in_water_and_in_or_against_a_building() {
        let map = map();
        assert!(usable(&map, [0, 0]));
        assert!(!usable(&map, [10_500, 0]), "outside the square");
        assert!(!usable(&map, [-5_000, -5_000]), "in the pond");
        assert!(!usable(&map, [5_000, 5_000]), "inside the house");
        assert!(!usable(&map, [4_450, 5_000]), "half a metre from its wall");
        assert!(usable(&map, [4_300, 5_000]), "two metres from its wall");
    }

    fn join(game: &mut Game, spawns: Vec<[i32; 2]>) -> u32 {
        let (outbox, inbox) = mpsc::channel(4096);
        std::mem::forget(inbox);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Profile { spawns, ..Default::default() } });
        response.try_recv().unwrap().unwrap()
    }

    fn at(game: &Game, id: u32) -> [i32; 2] {
        let [x, _, z] = game.world.entities.of::<Player>(id).character.position;
        [(x * 100.0).round() as i32, (z * 100.0).round() as i32]
    }

    #[test]
    fn own_spawns_are_used_in_turn_and_unusable_ones_dropped() {
        let mut game = Game::new(map(), 0).unwrap();
        let (a, b) = ([2_000, 0], [-2_000, 3_000]);
        let id = join(&mut game, vec![a, [5_000, 5_000], b]);
        assert_eq!(game.world.entities.of::<Player>(id).spawns, vec![a, b], "the one in the house is dropped");
        assert_eq!(at(&game, id), a);
        place(&mut game.world, id);
        assert_eq!(at(&game, id), b);
        place(&mut game.world, id);
        assert_eq!(at(&game, id), a);
    }

    #[test]
    fn a_player_with_no_usable_spawn_gets_the_worlds() {
        let mut game = Game::new(map(), 0).unwrap();
        let id = join(&mut game, vec![[-5_000, -5_000]]);
        assert!(game.world.entities.of::<Player>(id).spawns.is_empty());
        assert_eq!(at(&game, id), [0, 0]);
    }

    #[test]
    fn a_player_drops_in_under_an_open_chute_and_a_bot_starts_on_the_ground() {
        let mut game = Game::new(map(), 0).unwrap();
        let id = join(&mut game, vec![]);
        let character = game.world.entities.of::<Player>(id).character;
        assert!(character.chute && !character.grounded);
        assert!((character.position[1] - (STANDING_Y + SPAWN_ALTITUDE)).abs() < 1e-3);
        let bot = game.add_bot([0.0, 0.0], 1, vec![]).unwrap();
        let character = game.world.entities.of::<Player>(bot).character;
        assert!(!character.chute && character.position[1] < 2.0);
    }
}
