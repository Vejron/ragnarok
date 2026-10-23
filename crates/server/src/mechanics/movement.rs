use super::Mechanic;
use crate::entities::With;
use crate::game::{Player, World};
use shared::items::{aim_speed_scale, HOTBAR_SLOTS};

/// Walks every standing stickman by its input through the character controller, and keeps track of
/// the car it stands on, if any.
///
/// Riders are carried by `Vehicles` (before this runs), so here they only walk about on the roof.
/// Leaving a car by jumping or stepping off keeps its speed as knockback, so a stickman does not
/// drop away from a moving car as if it had stopped.
pub struct Movement;

impl Mechanic for Movement {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        for id in world.entities.ids::<With<Player>>() {
            let alive = world.entities.alive(id);
            let mut player = world.entities.of_mut::<Player>(id);
            if player.seat.is_some() || !alive {
                continue;
            }
            let player = &mut *player;
            let character = &mut player.character;
            let velocity = world.physics.car_pose(character.ride).map(|pose| pose.velocity);
            let slot = (player.input.slot() as usize).min(HOTBAR_SLOTS - 1);
            let scale = aim_speed_scale(player.hotbar[slot].item, player.input.buttons());
            world.physics.step_character(character, &player.input, dt, scale, velocity);
            character.ride = if character.grounded { world.physics.car_under(character.position).map_or(0, |hit| hit.car) } else { 0 };
        }
    }
}
