//! The score: every player's kills and deaths, counted from the tick's `Died` events.

use super::Mechanic;
use crate::game::{Died, Player, World};

pub struct Scores;

impl Mechanic for Scores {
    /// A death counts against the one who died, and for whoever killed them, if a player.
    fn react(&mut self, world: &mut World) {
        let died: Vec<Died> = world.happened::<Died>().copied().collect();
        for death in died {
            if let Some(mut victim) = world.entities.get_mut::<Player>(death.victim) {
                victim.deaths = victim.deaths.saturating_add(1);
            }
            if let Some(mut killer) = death.killer.and_then(|id| world.entities.get_mut::<Player>(id)) {
                killer.kills = killer.kills.saturating_add(1);
            }
        }
    }
}
