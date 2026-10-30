//! The score: every player's kills and deaths, counted from the tick's `Died` events.

use super::Mechanic;
use crate::game::{Died, Player, World};

pub struct Scores;

impl Mechanic for Scores {
    /// A player's death counts against them, and for whoever killed them, if a player. A creature's
    /// counts for nobody.
    fn react(&mut self, world: &mut World) {
        let died: Vec<Died> = world.happened::<Died>().copied().collect();
        for death in died {
            let Some(mut victim) = world.entities.get_mut::<Player>(death.victim) else { continue };
            victim.score.deaths = victim.score.deaths.saturating_add(1);
            if let Some(mut killer) = death.killer.and_then(|id| world.entities.get_mut::<Player>(id)) {
                killer.score.kills = killer.score.kills.saturating_add(1);
            }
        }
    }
}
