use super::Mechanic;
use crate::game::World;
use crate::physics::{Character, STANDING_Y};
use shared::consts::KILL_Y;
use shared::quant::from_cm;

/// Drops new players at a random outdoor spawn point, and respawns anyone who falls out of the world.
/// Spawns are sorted centre-first by `osm-import`; picking among the first few keeps players close.
pub struct Spawn;

/// Spawns are slightly above ground so a player never starts intersecting it.
const SPAWN_DROP: f32 = 0.5;
/// How many of the most central spawn points new players are spread over.
const NEARBY_SPAWNS: usize = 6;

fn place(world: &mut World, id: u32) {
    let point = if world.map.spawns.is_empty() {
        [0, 0]
    } else {
        let nearby = world.map.spawns.len().min(NEARBY_SPAWNS);
        let index = (world.rng.next_u64() % nearby as u64) as usize;
        world.map.spawns[index]
    };
    if let Some(player) = world.players.get_mut(&id) {
        player.character = Character {
            position: [from_cm(point[0]), STANDING_Y + SPAWN_DROP, from_cm(point[1])],
            ..Default::default()
        };
    }
}

impl Mechanic for Spawn {
    fn on_join(&mut self, world: &mut World, id: u32) {
        place(world, id);
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        let fallen: Vec<u32> = world
            .players
            .iter()
            .filter(|(_, player)| player.character.position[1] < KILL_Y)
            .map(|(&id, _)| id)
            .collect();
        for id in fallen {
            place(world, id);
        }
    }
}
