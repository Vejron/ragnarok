//! Game mechanics are small modules that hook into the game loop. Adding a feature (vehicles,
//! pickups, game modes) means adding a `Mechanic`, not editing the loop.

mod movement;
mod spawn;

pub use movement::Movement;
pub use spawn::Spawn;

use crate::game::World;

pub trait Mechanic: Send {
    /// A player entity was just created (at the origin, with no input yet).
    fn on_join(&mut self, _world: &mut World, _id: u32) {}
    /// A player entity is about to be removed.
    fn on_leave(&mut self, _world: &mut World, _id: u32) {}
    /// One fixed simulation tick. Each player's input for this tick is already in `Player::input`.
    fn on_tick(&mut self, _world: &mut World, _dt: f32) {}
}

/// The mechanics a room runs, in tick order.
pub fn default_mechanics() -> Vec<Box<dyn Mechanic>> {
    vec![Box::new(Spawn), Box::new(Movement)]
}
