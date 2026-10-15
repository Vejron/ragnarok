use super::Mechanic;
use crate::game::{Pickup, World};
use shared::consts::BUTTON_USE;
use shared::items::*;
use shared::protocol::ItemSlot;
use shared::quant::from_cm;

/// Items lying on the ground. They are scattered once, around the spawn points, when the room
/// starts; a player who presses use within `PICKUP_RADIUS` of one takes it, and it comes back
/// `PICKUP_RESPAWN_TICKS` later.
///
/// Taking goes into a stack of the same item if there is room, else the first empty slot, else it
/// swaps with the slot in hand, leaving what was in hand on the ground. It runs before `Vehicles`
/// and consumes the use button, so a car parked next to a gun is not boarded by reaching for it.
pub struct Pickups;

/// How far from a spawn point an item may be dropped, metres.
const SCATTER: f32 = 25.0;
/// Attempts to find a free spot for each item before giving up on it.
const ATTEMPTS: usize = 12;
/// How high above the ground an item floats, metres.
const FLOAT: f32 = 0.5;
/// How far above or below a player's centre an item can be reached, metres.
const REACH_Y: f32 = 2.5;

fn pick_item(world: &mut World) -> u8 {
    let total: u32 = (1..ITEM_COUNT).filter_map(def).map(|d| d.weight).sum();
    let mut roll = (world.rng.next_u64() % total as u64) as u32;
    for item in 1..ITEM_COUNT {
        let weight = def(item).map_or(0, |d| d.weight);
        if roll < weight {
            return item;
        }
        roll -= weight;
    }
    ITEM_BANDAGE
}

/// A spot on open ground near a spawn point, or `None` if the item landed on a roof or a wall.
fn pick_spot(world: &mut World) -> Option<[f32; 3]> {
    let centre = if world.map.spawns.is_empty() {
        [0.0, 0.0]
    } else {
        let at = world.map.spawns[(world.rng.next_u64() % world.map.spawns.len() as u64) as usize];
        [from_cm(at[0]), from_cm(at[1])]
    };
    let mut offset = || (world.rng.next_f64() as f32 * 2.0 - 1.0) * SCATTER;
    let (x, z) = (centre[0] + offset(), centre[1] + offset());
    if !world.map.is_inside((x * 100.0) as i32, (z * 100.0) as i32) {
        return None;
    }
    let ground = world.map.ground.height_at(x, z);
    // Straight down from well above: if the first thing it meets is not the ground, this is under a roof.
    let top = ground + 40.0;
    let floor = world.physics.cast_ray([x, top, z], [0.0, -1.0, 0.0], 80.0, 0).map(|t| top - t)?;
    ((floor - ground).abs() < 0.5).then_some([x, floor + FLOAT, z])
}

/// Adds `count` of `item` to the hotbar; returns how many did not fit.
fn store(hotbar: &mut [ItemSlot], item: u8, count: u8) -> u8 {
    let Some(item_def) = def(item) else { return count };
    let mut left = count;
    for slot in hotbar.iter_mut().filter(|slot| slot.item == item && item_def.stack > 1) {
        let take = left.min(item_def.stack.saturating_sub(slot.count));
        slot.count += take;
        left -= take;
    }
    while left > 0 {
        let Some(slot) = hotbar.iter_mut().find(|slot| slot.item == ITEM_NONE) else { break };
        let take = left.min(item_def.stack);
        *slot = ItemSlot { item, count: take, ammo: item_def.mag };
        left -= take;
    }
    left
}

impl Mechanic for Pickups {
    fn on_start(&mut self, world: &mut World) {
        let mut placed = 0;
        for _ in 0..PICKUP_COUNT {
            for _ in 0..ATTEMPTS {
                let Some(position) = pick_spot(world) else { continue };
                let item = pick_item(world);
                let count = def(item).map_or(1, |d| if d.kind == ItemKind::Gun { 1 } else { d.stack.min(2) });
                let id = world.alloc_id();
                world.pickups.insert(id, Pickup { item, count, position, respawn_in: 0, dropped: false });
                placed += 1;
                break;
            }
        }
        tracing::info!(pickups = placed, "pickups placed");
    }

    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        for pickup in world.pickups.values_mut() {
            pickup.respawn_in = pickup.respawn_in.saturating_sub(1);
        }
        let reaching: Vec<u32> = world
            .players
            .iter()
            .filter(|(_, p)| p.alive() && p.seat.is_none() && p.input.buttons() & BUTTON_USE != 0)
            .map(|(&id, _)| id)
            .collect();
        for id in reaching {
            let Some(at) = world.players.get(&id).map(|player| player.character.position) else { continue };
            let nearest = world
                .pickups
                .iter()
                .filter(|(_, p)| p.respawn_in == 0 && (p.position[1] - at[1]).abs() <= REACH_Y)
                .map(|(&pid, p)| (pid, (p.position[0] - at[0]).hypot(p.position[2] - at[2])))
                .filter(|(_, d)| *d <= PICKUP_RADIUS)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((pid, _)) = nearest else { continue };
            let Some(pickup) = world.pickups.get(&pid).copied() else { continue };
            let Some(player) = world.players.get_mut(&id) else { continue };
            // The reach is spent: the car beside the gun stays unboarded.
            player.input.buttons &= !BUTTON_USE;
            let left = store(&mut player.hotbar, pickup.item, pickup.count);
            if left == 0 && pickup.dropped {
                // A dropped item is spent for good.
                world.pickups.remove(&pid);
                continue;
            }
            let Some(entry) = world.pickups.get_mut(&pid) else { continue };
            if left == 0 {
                entry.respawn_in = PICKUP_RESPAWN_TICKS;
            } else if left < pickup.count {
                entry.count = left;
            } else {
                // Nowhere to put it: trade it for what is in hand.
                // Combat has not yet seen this tick's selection, so read it from the input.
                let held = (player.input.slot() as usize).min(HOTBAR_SLOTS - 1);
                let slot = player.hotbar[held];
                let item_def = def(pickup.item);
                if let (true, Some(item_def)) = (slot.item != ITEM_NONE && slot.item != pickup.item, item_def) {
                    player.hotbar[held] = ItemSlot { item: pickup.item, count: pickup.count, ammo: item_def.mag };
                    player.interrupt();
                    entry.item = slot.item;
                    entry.count = slot.count;
                    entry.dropped = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Command, Game, Outgoing};
    use shared::consts::BUTTON_USE;
    use shared::protocol::InputFrame;
    use shared::world::{WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

    fn game() -> (Game, u32, mpsc::Receiver<Outgoing>) {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        let (outbox, inbox) = mpsc::channel(256);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply });
        let id = response.try_recv().unwrap().unwrap();
        game.world.pickups.clear();
        // These tests are about taking things, so they start with room to take them.
        game.world.players.get_mut(&id).unwrap().hotbar = Default::default();
        (game, id, inbox)
    }

    fn press_use(game: &mut Game, id: u32, seq: &mut u32, slot: u8) {
        // The client sends use for one frame per press.
        for tick in 0..3 {
            *seq += 1;
            let buttons = if tick == 0 { BUTTON_USE } else { 0 };
            game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot }]));
            game.step();
        }
    }

    fn drop_at_feet(game: &mut Game, id: u32, item: u8, count: u8) -> u32 {
        let at = game.world.players[&id].character.position;
        let pid = game.world.alloc_id();
        game.world.pickups.insert(pid, Pickup { item, count, position: at, respawn_in: 0, dropped: false });
        pid
    }

    #[test]
    fn scattering_places_items_on_the_ground() {
        let (mut game, ..) = game();
        Pickups.on_start(&mut game.world);
        assert!(!game.world.pickups.is_empty());
        assert!(game.world.pickups.len() <= PICKUP_COUNT);
        assert!(game.world.pickups.values().all(|p| def(p.item).is_some()));
    }

    #[test]
    fn the_fixture_world_gets_its_items_on_open_ground() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack");
        let (world, hash) = crate::read_world(&path).unwrap();
        let game = Game::new(world, hash).unwrap();
        let placed = game.world.pickups.len();
        assert!(placed >= PICKUP_COUNT * 3 / 4, "only {placed} of {PICKUP_COUNT} found a spot");
        for pickup in game.world.pickups.values() {
            let [x, y, z] = pickup.position;
            let ground = game.world.map.ground.height_at(x, z);
            assert!((y - ground - FLOAT).abs() < 0.6, "item at {x},{z} floats {} m above the ground", y - ground);
        }
        let kinds: std::collections::BTreeSet<u8> = game.world.pickups.values().map(|p| p.item).collect();
        assert!(kinds.len() >= 3, "a spread of items, got {kinds:?}");
    }

    #[test]
    fn use_takes_the_item_into_the_first_free_slot_and_it_comes_back_later() {
        let (mut game, id, _inbox) = game();
        let pid = drop_at_feet(&mut game, id, ITEM_AK5, 1);
        let mut seq = 0;
        press_use(&mut game, id, &mut seq, 0);
        let slot = game.world.players[&id].hotbar[0];
        assert_eq!(slot, ItemSlot { item: ITEM_AK5, count: 1, ammo: def(ITEM_AK5).unwrap().mag });
        assert!(game.world.pickups[&pid].respawn_in > 0);
        assert!(game.pickup_states().is_empty(), "a taken item is not shown");

        game.world.pickups.get_mut(&pid).unwrap().respawn_in = 1;
        game.step();
        assert_eq!(game.pickup_states().len(), 1);
    }

    #[test]
    fn stackable_items_stack_up_to_the_limit_and_the_rest_stay_on_the_ground() {
        let (mut game, id, _inbox) = game();
        game.world.players.get_mut(&id).unwrap().hotbar = [ItemSlot { item: ITEM_GRENADE, count: 2, ammo: 0 }, ItemSlot { item: ITEM_REVOLVER, count: 1, ammo: 6 }, ItemSlot { item: ITEM_AK5, count: 1, ammo: 30 }, ItemSlot { item: ITEM_BANDAGE, count: 1, ammo: 0 }, ItemSlot { item: ITEM_BANDAGE, count: 5, ammo: 0 }];
        let pid = drop_at_feet(&mut game, id, ITEM_GRENADE, 2);
        let mut seq = 0;
        press_use(&mut game, id, &mut seq, 0);
        let grenades = def(ITEM_GRENADE).unwrap().stack;
        assert_eq!(game.world.players[&id].hotbar[0].count, grenades);
        assert_eq!(game.world.pickups[&pid].count, 2 - (grenades - 2));
        assert_eq!(game.world.pickups[&pid].respawn_in, 0, "what did not fit is still there");
    }

    #[test]
    fn a_full_hotbar_swaps_with_the_slot_in_hand() {
        let (mut game, id, _inbox) = game();
        game.world.players.get_mut(&id).unwrap().hotbar =
            [ItemSlot { item: ITEM_REVOLVER, count: 1, ammo: 6 }, ItemSlot { item: ITEM_AK5, count: 1, ammo: 30 }, ItemSlot { item: ITEM_GRENADE, count: 3, ammo: 0 }, ItemSlot { item: ITEM_BANDAGE, count: 5, ammo: 0 }, ItemSlot { item: ITEM_REVOLVER, count: 1, ammo: 2 }];
        let pid = drop_at_feet(&mut game, id, ITEM_AK5, 1);
        game.world.pickups.get_mut(&pid).unwrap().count = 1;
        // Everything but the AK5 slot is full of other things; hold slot 2 (grenades) and swap.
        game.world.players.get_mut(&id).unwrap().hotbar[1] = ItemSlot { item: ITEM_BANDAGE, count: 5, ammo: 0 };
        let mut seq = 0;
        press_use(&mut game, id, &mut seq, 2);
        let player = &game.world.players[&id];
        assert_eq!(player.hotbar[2].item, ITEM_AK5);
        let on_ground = &game.world.pickups[&pid];
        assert_eq!((on_ground.item, on_ground.count, on_ground.respawn_in), (ITEM_GRENADE, 3, 0));
    }

    #[test]
    fn reaching_for_an_item_does_not_also_board_a_car() {
        let (mut game, id, _inbox) = game();
        drop_at_feet(&mut game, id, ITEM_BANDAGE, 1);
        let mut seq = 0;
        press_use(&mut game, id, &mut seq, 0);
        assert_eq!(game.world.players[&id].hotbar[0].item, ITEM_BANDAGE);
        assert!(game.world.players[&id].seat.is_none());
    }

    #[test]
    fn out_of_reach_items_are_left_alone() {
        let (mut game, id, _inbox) = game();
        let pid = drop_at_feet(&mut game, id, ITEM_AK5, 1);
        game.world.pickups.get_mut(&pid).unwrap().position[0] += PICKUP_RADIUS + 1.0;
        let mut seq = 0;
        press_use(&mut game, id, &mut seq, 0);
        assert_eq!(game.world.players[&id].hotbar[0].item, ITEM_NONE);
        assert_eq!(game.world.pickups[&pid].respawn_in, 0);
    }
}
