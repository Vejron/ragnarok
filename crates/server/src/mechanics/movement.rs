use super::Mechanic;
use crate::game::World;
use crate::physics::Character;
use shared::consts::*;

/// Walks every standing stickman by its input through the character controller, and keeps track of
/// the car it stands on, if any.
///
/// Riders are carried by `Vehicles` (before this runs), so here they only walk about on the roof.
/// Leaving a car by jumping or stepping off keeps its speed as knockback, so a stickman does not
/// drop away from a moving car as if it had stopped.
pub struct Movement;

/// Adds a car's horizontal velocity to a character's knockback, within the wire's limit.
fn inherit(character: &mut Character, velocity: [f32; 3]) {
    let mut knock = [character.knock[0] + velocity[0], character.knock[1] + velocity[2]];
    let speed = knock[0].hypot(knock[1]);
    if speed > KNOCK_MAX_SPEED {
        knock = [knock[0] / speed * KNOCK_MAX_SPEED, knock[1] / speed * KNOCK_MAX_SPEED];
    }
    character.knock = knock;
}

impl Mechanic for Movement {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        for player in world.players.values_mut().filter(|player| player.seat.is_none() && player.alive()) {
            let character = &mut player.character;
            let was_riding = character.ride;
            let velocity = world.physics.car_pose(was_riding).map(|pose| pose.velocity);
            let jumped = was_riding != 0 && character.grounded && player.input.buttons() & BUTTON_JUMP != 0;
            if let (true, Some(velocity)) = (jumped, velocity) {
                inherit(character, velocity);
            }
            world.physics.move_character(character, &player.input, dt);
            character.ride = if character.grounded { world.physics.car_under(character.position).map_or(0, |hit| hit.car) } else { 0 };
            if let (false, false, Some(velocity)) = (jumped, character.grounded, velocity) {
                // Stepped off the edge of the roof.
                if was_riding != 0 {
                    inherit(character, velocity);
                }
            }
        }
    }
}
