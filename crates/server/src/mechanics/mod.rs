//! Game mechanics are small modules that hook into the game loop. Adding a feature (vehicles,
//! pickups, game modes) means adding a `Mechanic`, not editing the loop.

mod armor;
mod bots;
mod combat;
mod drones;
mod gunnery;
mod life;
mod movement;
mod npcs;
mod pickups;
mod projectiles;
mod spawn;
mod vehicles;

pub use bots::{Bots, DRIVER, GUNNER, nearest_waypoint, road_route};
pub use combat::{Combat, position_of};
#[cfg(test)]
pub use combat::damage_vehicle;
pub use drones::Drones;
pub use gunnery::Gunnery;
pub use life::Life;
pub use movement::Movement;
pub use npcs::{Npcs, calling};
pub use pickups::Pickups;
pub use spawn::{Spawn, usable as spawn_usable};
pub use vehicles::{Vehicles, add_tank, remove_vehicle, seat};

use crate::game::World;

pub trait Mechanic: Send {
    /// Its name, for timings and logs.
    fn name(&self) -> &'static str {
        let path = std::any::type_name::<Self>();
        path.rsplit("::").next().unwrap_or(path)
    }
    /// The room was just created, before any player joined.
    fn on_start(&mut self, _world: &mut World) {}
    /// A player entity was just created (at the origin, with no input yet).
    fn on_join(&mut self, _world: &mut World, _id: u32) {}
    /// A player entity is about to be removed.
    fn on_leave(&mut self, _world: &mut World, _id: u32) {}
    /// One fixed simulation tick. Each player's input for this tick is already in `Player::input`.
    fn on_tick(&mut self, _world: &mut World, _dt: f32) {}
}

/// Registers the components mechanics own with the room's ECS world (see `entities`).
pub fn register_components(ecs: &mut bevy_ecs::world::World) {
    ecs.register_component::<npcs::Wander>();
    ecs.register_component::<pickups::Pickup>();
    ecs.register_component::<crate::game::Vehicle>();
    ecs.register_component::<crate::game::Mounts>();
    ecs.register_component::<crate::game::Drone>();
    ecs.register_component::<crate::game::Wreck>();
    ecs.register_component::<crate::game::Seats>();
    ecs.register_component::<crate::game::Player>();
    ecs.register_component::<crate::game::Bot>();
    projectiles::register(ecs);
}

/// The mechanics a room runs, in tick order. `Bots` goes first: it writes the bots' input, which
/// everything after reads as it reads anyone's. `Life` brings the dead back before `Npcs` moves them.
pub fn default_mechanics() -> Vec<Box<dyn Mechanic>> {
    vec![Box::new(Bots), Box::new(Spawn), Box::new(Pickups), Box::new(Drones), Box::new(Vehicles), Box::new(Gunnery), Box::new(Movement), Box::new(Life), Box::new(Npcs), Box::new(Combat)]
}

/// What tests outside the mechanics reach in for: setting up a busy room without playing it.
#[cfg(test)]
pub(crate) mod for_tests {
    pub use super::combat::damage;
    pub use super::drones::launch as launch_drone;
    pub use super::projectiles::Flying;

    pub fn add_vehicle(world: &mut crate::game::World, kind: u8, position: [f32; 3], yaw: f32) -> u32 {
        super::vehicles::add_vehicle(world, kind, position, yaw)
    }
}
