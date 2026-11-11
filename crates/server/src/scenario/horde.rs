//! A scenario's zombies (ZOMBIES.md): when and where they rise. At night, every `every` ticks, a
//! horde of up to `size` rises in one of the scenario's zombie areas, at a spot clear of anyone
//! living by `KEEP_AWAY`, as long as fewer than `max` are up. Its zombies come up one at a time over
//! `ZOMBIE_RISE_STAGGER`, each from a grave of its own, and take `ZOMBIE_RISE_TICKS` to climb out. At dawn, unless they rise by day as well, those still up fall where they stand, a few a tick.
//! The fallen leave once their bodies would have got up, as a squad's that does not respawn do.

use super::{Area, Progress, Scenario};
use crate::game::{Game, Player};
use crate::mechanics::{Harm, damage_entity, darkness, is_zombie, open_ground, start_rising};
use shared::consts::ZOMBIE_RISE_STAGGER;
use shared::protocol::WEAPON_CLAW;

/// A scenario's zombies, checked.
pub(super) struct Horde {
    /// Indices of the areas they rise in.
    pub areas: Vec<usize>,
    /// The most up at once, how many rise together, and ticks between hordes.
    pub max: u32,
    pub size: u32,
    pub every: u32,
    /// They rise by day as well.
    pub day: bool,
}

/// Darker than this is night, for zombies (0 day to 1 night, `darkness`).
const NIGHT: f32 = 0.5;
/// A horde rises at least this far from anyone living, metres.
const KEEP_AWAY: f32 = 35.0;
/// Its zombies stand about this far apart round the spot it rises at, metres.
const SPREAD: f32 = 4.0;
/// Spots tried for a horde, and for each zombie in it, before giving up until the next.
const TRIES: usize = 16;
/// At dawn this many fall a tick, so that the horde goes down over a moment, not all at once.
const DAWN_PER_TICK: usize = 2;
/// Enough to kill any soldier outright, the torso's whole health.
const FELLED: u8 = u8::MAX;

impl Scenario {
    /// One tick of the zombies: the fallen leave, and then by night a horde rises when it is due, or
    /// at dawn the rest fall. Nothing rises once the scenario has ended.
    pub(super) fn haunt(&mut self, game: &mut Game) {
        let Some(horde) = &self.zombies else { return };
        self.progress.zombies.retain(|&id| {
            let world = &game.world;
            let stays = world.entities.has::<Player>(id) && (world.entities.alive(id) || world.entities.get::<crate::entities::Respawn>(id).is_some_and(|respawn| respawn.left > 1));
            if !stays {
                game.remove_zombie(id);
            }
            stays
        });
        if self.progress.ended.is_some() {
            self.progress.graves.clear();
            return;
        }
        dig(&mut self.progress, game);
        let night = darkness(game.world.tick) > NIGHT;
        if !night && !horde.day {
            let up: Vec<u32> = self.progress.zombies.iter().copied().filter(|&id| game.world.entities.alive(id)).take(DAWN_PER_TICK).collect();
            for id in up {
                damage_entity(&mut game.world, id, Harm::new(FELLED, None, WEAPON_CLAW).unsparing());
            }
            self.progress.next_horde = 0;
            self.progress.graves.clear();
            return;
        }
        if self.progress.next_horde > 0 {
            self.progress.next_horde -= 1;
            return;
        }
        self.progress.next_horde = horde.every;
        let up = self.progress.zombies.iter().filter(|&&id| game.world.entities.alive(id)).count() as u32 + self.progress.graves.len() as u32;
        let count = horde.size.min(horde.max.saturating_sub(up));
        if count == 0 {
            return;
        }
        let area = &self.areas[horde.areas[(game.world.rng.next_u64() % horde.areas.len() as u64) as usize]];
        let Some(centre) = rising_spot(game, area) else {
            tracing::debug!("no spot clear of the living for a horde to rise at");
            return;
        };
        for n in 0..count {
            let mut spot = None;
            for _ in 0..TRIES {
                let p = around(game, centre, SPREAD);
                if open_ground(&game.world, p[0], p[1]).is_some() {
                    spot = Some(p);
                    break;
                }
            }
            let Some(at) = spot else { continue };
            // The first comes up at once, and the rest one by one after it, at moments of their own.
            let after = if n == 0 { 0 } else { (game.world.rng.next_f64() * f64::from(ZOMBIE_RISE_STAGGER)) as u32 };
            self.progress.graves.push((at, game.world.tick + after));
        }
        tracing::info!(x = centre[0], z = centre[1], count, "a horde of zombies is rising");
    }
}

/// The zombies whose moment has come start climbing out of their graves.
fn dig(progress: &mut Progress, game: &mut Game) {
    let now = game.world.tick;
    let mut i = 0;
    while i < progress.graves.len() {
        let (at, due) = progress.graves[i];
        if due > now {
            i += 1;
            continue;
        }
        progress.graves.swap_remove(i);
        let Some(id) = game.add_zombie(at) else { continue };
        start_rising(&mut game.world, id);
        progress.zombies.push(id);
    }
}

/// A random point within `radius` of `centre`, `[x, z]` metres.
fn around(game: &mut Game, centre: [f32; 2], radius: f32) -> [f32; 2] {
    let angle = game.world.rng.next_f64() as f32 * std::f32::consts::TAU;
    let distance = radius * (game.world.rng.next_f64() as f32).sqrt();
    [centre[0] + angle.cos() * distance, centre[1] + angle.sin() * distance]
}

/// A spot in `area` on open ground and at least `KEEP_AWAY` from anyone living, if one turns up.
fn rising_spot(game: &mut Game, area: &Area) -> Option<[f32; 2]> {
    for _ in 0..TRIES {
        let at = around(game, area.at, area.radius);
        let world = &game.world;
        let seen = world.entities.all::<Player>().any(|(id, player)| {
            let p = player.character.position;
            !is_zombie(world, id) && world.entities.alive(id) && (p[0] - at[0]).hypot(p[2] - at[1]) < KEEP_AWAY
        });
        if !seen && open_ground(world, at[0], at[1]).is_some() {
            return Some(at);
        }
    }
    None
}
