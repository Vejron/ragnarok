//! Game mechanics are small modules that hook into the game loop. Adding a feature (vehicles,
//! pickups, game modes) means adding a `Mechanic`, not editing the loop.

mod armor;
mod combat;
mod drones;
mod gunnery;
mod movement;
mod npcs;
mod pickups;
mod plague;
mod spawn;
mod vehicles;

pub use combat::Combat;
pub use drones::Drones;
pub use gunnery::Gunnery;
pub use movement::Movement;
pub use npcs::Npcs;
pub use pickups::Pickups;
pub use plague::{FIRST_ARRIVAL_TICKS as PLAGUE_FIRST_ARRIVAL_TICKS, OrangePlague, progress_byte as plague_progress, wire_mode as plague_wire_mode};
pub use spawn::{Spawn, usable as spawn_usable};
pub use vehicles::Vehicles;

use crate::game::World;

pub trait Mechanic: Send {
    /// The room was just created, before any player joined.
    fn on_start(&mut self, _world: &mut World) {}
    /// A player entity was just created (at the origin, with no input yet).
    fn on_join(&mut self, _world: &mut World, _id: u32) {}
    /// A player entity is about to be removed.
    fn on_leave(&mut self, _world: &mut World, _id: u32) {}
    /// One fixed simulation tick. Each player's input for this tick is already in `Player::input`.
    fn on_tick(&mut self, _world: &mut World, _dt: f32) {}
}

/// The mechanics a room runs, in tick order.
pub fn default_mechanics() -> Vec<Box<dyn Mechanic>> {
    vec![Box::new(Spawn), Box::new(Pickups), Box::new(Drones), Box::new(Vehicles), Box::new(Gunnery), Box::new(Movement), Box::new(Npcs), Box::new(Combat)]
}
