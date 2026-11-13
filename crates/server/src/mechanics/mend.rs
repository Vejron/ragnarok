//! Mending by oneself: a soldier who has gone `MEND_DELAY_SECONDS` without being hurt heals their torso and
//! their head evenly, from nothing to full in `MEND_SECONDS`. Limbs do not mend; that is a bandage's work.

use super::{Mechanic, is_zombie, limbs};
use crate::entities::{Health, With};
use crate::game::{Player, World};
use shared::consts::*;
use std::collections::HashMap;

/// What each soldier's healing has to go on: their torso's health and their head's damage as of the
/// last tick, to tell a fresh hurt by, and the ticks since that hurt.
#[derive(Default)]
pub struct Mend {
    seen: HashMap<u32, Seen>,
}

#[derive(Clone, Copy)]
struct Seen {
    hp: u8,
    head: u8,
    /// Ticks unhurt.
    quiet: u32,
}

/// Whole points of `full` healed on the `quiet`th tick of mending, so a second's ticks add up to an even
/// share of it without keeping fractions.
fn share(full: u8, quiet: u32) -> u8 {
    let per_tick = full as f32 / (MEND_SECONDS * TICK_RATE as f32);
    ((quiet + 1) as f32 * per_tick).floor() as u8 - (quiet as f32 * per_tick).floor() as u8
}

impl Mechanic for Mend {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        let delay = (MEND_DELAY_SECONDS * TICK_RATE as f32) as u32;
        let mut seen = HashMap::with_capacity(self.seen.len());
        for id in world.entities.ids::<With<Player>>() {
            let Some(health) = world.entities.get::<Health>(id).copied() else { continue };
            if !health.alive() || is_zombie(world, id) {
                continue;
            }
            let head = limbs::head_taken(world, id);
            let quiet = match self.seen.get(&id) {
                Some(last) if health.hp >= last.hp && head <= last.head => last.quiet + 1,
                _ => 0,
            };
            if quiet > delay {
                let n = quiet - delay - 1;
                let torso = share(health.max, n);
                if torso > 0 && health.hp < health.max {
                    let mut health = world.entities.get_mut::<Health>(id).unwrap();
                    health.hp = health.hp.saturating_add(torso).min(health.max);
                }
                let heal = share(HEAD_HP, n);
                if heal > 0 && head > 0 {
                    limbs::heal_head(world, id, heal);
                }
            }
            let hp = world.entities.get::<Health>(id).map_or(0, |health| health.hp);
            seen.insert(id, Seen { hp, head: limbs::head_taken(world, id), quiet });
        }
        self.seen = seen;
    }
}

#[cfg(test)]
mod tests;
