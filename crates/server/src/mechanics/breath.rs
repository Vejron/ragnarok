//! Holding the breath under water, and drowning once it runs out (SWIMMING.md).

use super::Mechanic;
use super::life::{Harm, damage_entity};
use crate::entities::With;
use crate::game::{Player, World};
use shared::consts::*;
use shared::protocol::WEAPON_DROWN;
use shared::swim::breathe;

/// Counts down every soldier's air while their eyes are under the water and gives it back above it.
/// With none left, they drown: `DROWN_DAMAGE` every `DROWN_EVERY` ticks, killed by nobody.
pub struct Breath;

impl Mechanic for Breath {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        let mut drowning = Vec::new();
        for id in world.entities.ids::<With<Player>>() {
            let alive = world.entities.alive(id);
            let mut player = world.entities.of_mut::<Player>(id);
            if !alive || !player.embodied() {
                player.breath = BREATH_FULL;
                player.drowning = 0;
                continue;
            }
            let under = world.physics.submerged(&player.character);
            player.breath = breathe(player.breath, under);
            if under && player.breath == 0 {
                player.drowning += 1;
                if player.drowning.is_multiple_of(DROWN_EVERY) {
                    drowning.push(id);
                }
            } else {
                player.drowning = 0;
            }
        }
        for id in drowning {
            damage_entity(world, id, Harm::new(DROWN_DAMAGE, None, WEAPON_DROWN));
        }
    }
}

#[cfg(test)]
mod tests;
