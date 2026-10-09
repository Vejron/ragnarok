use super::Mechanic;
use crate::game::World;

/// Walks every stickman by its input through the character controller.
pub struct Movement;

impl Mechanic for Movement {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        for player in world.players.values_mut() {
            world.physics.move_character(&mut player.character, &player.input, dt);
        }
    }
}
