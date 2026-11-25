//! Who the police are after (EMERGENCY.md). A player who hurts a civilian, attacks the emergency
//! services, takes a car in front of the police or fires a gun near them is *wanted*: they carry
//! heat, shown to them as one to five stars, faint while no police see them. The more stars, the more police cars come, and from
//! three the police shoot to kill where before they only arrest.
//!
//! Heat is lost all at once, by not being seen by any police for long enough (longer the more
//! stars), by being arrested, or by dying.

use super::officers::Officer;
use super::{Emergency, distance_2d, ticks};
use crate::entities::{Component, With};
use crate::game::{Bot, Damaged, Died, Noise, Player, Seats, World};
use crate::mechanics::bots::clear_line;
use crate::mechanics::traffic::{Duty, Service};
use crate::mechanics::{Civilian, Driver, position_of};
use shared::consts::*;

/// The heat at which a player has one star, two, and so on up to five, and the most heat there is.
pub const HEAT: [f32; 5] = [10.0, 30.0, 60.0, 100.0, 150.0];
const HEAT_MAX: f32 = 200.0;
/// How many police cars come for a player with one star, two, and so on.
pub const CARS: [usize; 5] = [1, 2, 4, 6, 8];
/// From this many stars the police shoot to kill; below it they arrest.
pub const LETHAL_STARS: u8 = 3;
/// How long a player with one star, two, and so on must go unseen to be wanted no more, seconds.
/// The time counts only once they are `AWAY` metres from where the police last knew them to be, or
/// a police car has got there and not found them: waiting where the crime was done for the police
/// to come is not getting away.
const ESCAPE: [f32; 5] = [20.0, 30.0, 60.0, 90.0, 120.0];
const AWAY: f32 = 50.0;
/// The heat for hurting a civilian or a driver of the traffic, and for killing one. The same one
/// hurt again within `SAME_VICTIM` seconds is the same crime.
const HEAT_HURT: f32 = 10.0;
const HEAT_KILL: f32 = 25.0;
const SAME_VICTIM: f32 = 10.0;
/// The heat for hurting someone of the emergency services, and for killing one.
const HEAT_HURT_SERVICE: f32 = 30.0;
const HEAT_KILL_SERVICE: f32 = 50.0;
/// The heat for damaging a police car or an ambulance, at most once in `VEHICLE_EVERY` seconds, for
/// wrecking one, and for taking one.
const HEAT_VEHICLE: f32 = 15.0;
const VEHICLE_EVERY: f32 = 3.0;
const HEAT_WRECK: f32 = 40.0;
const HEAT_TAKE_SERVICE: f32 = 30.0;
/// The heat for taking a car of the traffic where the police see it, within `WITNESS_WITHIN` metres.
const HEAT_TAKE: f32 = 15.0;
const WITNESS_WITHIN: f32 = 60.0;
/// The heat for firing a gun in earshot of the police, at most once in `FIRE_EVERY` seconds, and
/// the stars that alone can come to.
const HEAT_FIRE: f32 = 10.0;
const FIRE_EVERY: f32 = 5.0;
const FIRE_STARS: usize = 2;
/// The heat for a round that passes within `NEAR_MISS` metres of the police, at most once in
/// `VEHICLE_EVERY` seconds.
const HEAT_NEAR_MISS: f32 = 20.0;
const NEAR_MISS: f32 = 4.0;
/// The heat for keeping moving faster than `FLEE_SPEED` m/s in sight of the police, every
/// `FLEE_EVERY` seconds, and the stars that alone can come to.
const HEAT_FLEE: f32 = 10.0;
const FLEE_EVERY: f32 = 20.0;
const FLEE_SPEED: f32 = 3.0;
const FLEE_STARS: usize = 3;
/// The police see a wanted player within this, metres, with nothing between them, looked for every
/// `LOOK_EVERY` ticks.
pub const SIGHT: f32 = 80.0;
pub const LOOK_EVERY: u32 = 6;
/// Anyone who seems to go faster than this between two looks has jumped there, m/s.
const HEADING_MAX: f32 = 80.0;

/// A player the police are after. A component on the player's entity, there while they have heat.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Wanted {
    pub heat: f32,
    /// Where the police last knew them to be, and the ticks since any of them saw them.
    pub seen_at: [f32; 3],
    pub unseen: u32,
    /// How fast and which way they were going when last seen, m/s over the ground (x, z).
    pub heading: [f32; 2],
    /// Where they were at the last look, to tell how fast they go.
    was_at: [f32; 3],
    /// The ticks they have fled in sight of the police since that last counted against them.
    fled: u32,
    /// The ticks of their last crime of each kind that counts only so often: firing, harming or
    /// taking a vehicle, and a near miss.
    fired: u32,
    vehicle: u32,
    missed: u32,
    /// Whom they hurt lately, and on which tick.
    victims: Vec<(u32, u32)>,
}

/// How many stars `heat` is.
pub fn stars_of(heat: f32) -> u8 {
    HEAT.iter().filter(|&&at| heat >= at).count() as u8
}

/// How many stars player `id` has: 0 for someone the police are not after.
pub fn stars(world: &World, id: u32) -> u8 {
    world.entities.get::<Wanted>(id).map_or(0, |wanted| stars_of(wanted.heat))
}

/// Whether `id` is someone the police can be after: a person, not a bot, a zombie, a civilian or
/// one of their own.
pub fn person(world: &World, id: u32) -> bool {
    world.entities.has::<Player>(id) && !world.entities.has::<Bot>(id) && !crate::mechanics::is_noncombatant(world, id) && !crate::mechanics::is_zombie(world, id)
}

/// What a wanted player is told of it, for the stars on their screen: the stars in the low three
/// bits, and bit 7 set while no police see them.
pub fn wanted_byte(world: &World, id: u32) -> u8 {
    world.entities.get::<Wanted>(id).map_or(0, |wanted| stars_of(wanted.heat) | if wanted.unseen > LOOK_EVERY { 0x80 } else { 0 })
}

/// Player `id` has done something that is worth `heat`. With `stars`, it cannot take them past
/// that many by itself. The police know where they were when they did it.
pub fn charge(world: &mut World, id: u32, heat: f32, stars: Option<usize>) {
    if !world.fleet.police || !person(world, id) || !world.entities.alive(id) {
        return;
    }
    let Some(at) = position_of(world, id) else { return };
    if !world.entities.has::<Wanted>(id) {
        world.entities.insert(id, Wanted { heat: 0.0, seen_at: at, unseen: 0, heading: [0.0; 2], was_at: at, fled: 0, fired: 0, vehicle: 0, missed: 0, victims: Vec::new() });
    }
    let mut wanted = world.entities.of_mut::<Wanted>(id);
    let most = stars.map_or(HEAT_MAX, |stars| HEAT[stars - 1]);
    wanted.heat = wanted.heat.max((wanted.heat + heat).min(most));
    (wanted.seen_at, wanted.unseen) = (at, 0);
}

/// Player `id` is wanted no more.
pub fn clear(world: &mut World, id: u32) {
    world.entities.remove::<Wanted>(id);
}

/// Whether the crime of the kind last done on tick `last` counts again now, `every` seconds on.
fn again(tick: u32, last: u32, every: f32) -> bool {
    last == 0 || tick.wrapping_sub(last) > ticks(every)
}

impl Emergency {
    /// Where the police are who can see or hear something: the officers on foot, and the police
    /// cars that are out and near enough a player to be there at all.
    pub(super) fn witnesses(&self, world: &World) -> Vec<[f32; 3]> {
        let officers = world.entities.all::<Officer>().filter(|&(id, _)| world.entities.alive(id)).filter_map(|(id, _)| world.entities.get::<Player>(id).map(|player| player.eye()));
        let cars = world.fleet.units.iter().filter(|unit| unit.service == Service::Police && unit.duty != Duty::Dormant && unit.vid.is_some()).map(|unit| [unit.at[0], unit.at[1] + 0.6, unit.at[2]]);
        officers.chain(cars).collect()
    }

    /// The crimes among what was hurt and who died this tick, and the heat off those who died.
    pub(super) fn judge(&mut self, world: &mut World) {
        let tick = world.tick;
        let hurt: Vec<Damaged> = world.happened::<Damaged>().cloned().collect();
        for damaged in hurt {
            let Some(by) = damaged.attacker.filter(|&by| by != damaged.victim && person(world, by)) else { continue };
            let victim = damaged.victim;
            let heat = if world.entities.has::<Civilian>(victim) || world.entities.has::<Driver>(victim) {
                // The same one hurt again a moment later is the same crime, until it kills them.
                let lately = world.entities.get::<Wanted>(by).is_some_and(|wanted| wanted.victims.iter().any(|&(other, at)| other == victim && tick.wrapping_sub(at) < ticks(SAME_VICTIM)));
                if damaged.killed {
                    HEAT_KILL
                } else if lately {
                    0.0
                } else {
                    HEAT_HURT
                }
            } else if super::in_services(world, victim) {
                if damaged.killed { HEAT_KILL_SERVICE } else { HEAT_HURT_SERVICE }
            } else if world.fleet.units.iter().any(|unit| unit.vid == Some(victim)) {
                let often = world.entities.get::<Wanted>(by).is_none_or(|wanted| again(tick, wanted.vehicle, VEHICLE_EVERY));
                if damaged.killed {
                    HEAT_WRECK
                } else if often {
                    HEAT_VEHICLE
                } else {
                    0.0
                }
            } else if damaged.killed && world.entities.has::<crate::mechanics::traffic::TrafficCar>(victim) {
                // A car of the traffic wrecked with its driver in it.
                HEAT_KILL
            } else {
                continue;
            };
            if heat > 0.0 {
                charge(world, by, heat, None);
                if let Some(mut wanted) = world.entities.get_mut::<Wanted>(by) {
                    wanted.victims.retain(|&(other, at)| other != victim && tick.wrapping_sub(at) < ticks(SAME_VICTIM));
                    wanted.victims.push((victim, tick));
                    wanted.vehicle = tick.max(1);
                }
            }
        }
        // The dead are wanted no more.
        let died: Vec<u32> = world.happened::<Died>().map(|died| died.victim).filter(|&id| world.entities.has::<Wanted>(id)).collect();
        for id in died {
            clear(world, id);
        }
    }

    /// The crimes among the shots fired since last tick: a gun fired in earshot of the police, and
    /// a round that went by them.
    pub(super) fn overhear(&mut self, world: &mut World, noises: &[Noise]) {
        let shots: Vec<&Noise> = noises.iter().filter(|noise| noise.line.is_some() && noise.by.is_some_and(|by| person(world, by))).collect();
        if shots.is_empty() || !world.fleet.police {
            return;
        }
        let witnesses = self.witnesses(world);
        let tick = world.tick;
        for noise in shots {
            let (Some(by), Some(line)) = (noise.by, noise.line) else { continue };
            let length = (line[0] * line[0] + line[1] * line[1] + line[2] * line[2]).sqrt().max(1e-6);
            let (mut heard, mut close) = (false, false);
            for witness in &witnesses {
                let to = [witness[0] - noise.at[0], witness[1] - noise.at[1], witness[2] - noise.at[2]];
                let off = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
                let along = (to[0] * line[0] + to[1] * line[1] + to[2] * line[2]) / length;
                heard |= off < noise.reach;
                close |= along > 0.0 && (off * off - along * along).max(0.0).sqrt() < NEAR_MISS;
            }
            let (fired, missed) = world.entities.get::<Wanted>(by).map_or((0, 0), |wanted| (wanted.fired, wanted.missed));
            if close && again(tick, missed, VEHICLE_EVERY) {
                charge(world, by, HEAT_NEAR_MISS, None);
                if let Some(mut wanted) = world.entities.get_mut::<Wanted>(by) {
                    wanted.missed = tick.max(1);
                }
            } else if heard && again(tick, fired, FIRE_EVERY) {
                charge(world, by, HEAT_FIRE, Some(FIRE_STARS));
                if let Some(mut wanted) = world.entities.get_mut::<Wanted>(by) {
                    wanted.fired = tick.max(1);
                }
            }
        }
    }

    /// Every `LOOK_EVERY` ticks: who has taken a car they should not have, which of the wanted the
    /// police can see, who flees them, and who has got away.
    pub(super) fn watch(&mut self, world: &mut World) {
        if !world.fleet.police {
            return;
        }
        let tick = world.tick;
        let witnesses = self.witnesses(world);
        let seen_from = |world: &World, at: [f32; 3], within: f32| witnesses.iter().any(|&witness| distance_2d(witness, at) < within && clear_line(world, witness, [at[0], at[1] + 0.5, at[2]]));
        // A police car or an ambulance with someone at its wheel who is not its crew is stolen,
        // whoever sees it; a car of the traffic only where the police do.
        let mut thieves: Vec<(u32, f32)> = world.fleet.units.iter().filter_map(|unit| unit.vid).filter_map(|vid| world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(0))).map(|id| (id, HEAT_TAKE_SERVICE)).collect();
        if !witnesses.is_empty() {
            for vid in world.entities.ids::<With<crate::mechanics::traffic::TrafficCar>>() {
                if world.fleet.units.iter().any(|unit| unit.vid == Some(vid)) {
                    continue;
                }
                let Some(id) = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(0)) else { continue };
                if position_of(world, id).is_some_and(|at| seen_from(world, at, WITNESS_WITHIN)) {
                    thieves.push((id, HEAT_TAKE));
                }
            }
        }
        for (id, heat) in thieves {
            if world.entities.get::<Wanted>(id).is_none_or(|wanted| again(tick, wanted.vehicle, VEHICLE_EVERY)) {
                charge(world, id, heat, None);
                if let Some(mut wanted) = world.entities.get_mut::<Wanted>(id) {
                    wanted.vehicle = tick.max(1);
                }
            }
        }
        for id in world.entities.ids::<With<Wanted>>() {
            let Some(at) = position_of(world, id).filter(|_| world.entities.alive(id)) else {
                clear(world, id);
                continue;
            };
            let seen = seen_from(world, at, SIGHT);
            let searched = world.fleet.units.iter().any(|unit| unit.tag == id && unit.duty == Duty::OnScene);
            let mut wanted = world.entities.of_mut::<Wanted>(id);
            let look = LOOK_EVERY as f32 / TICK_RATE as f32;
            let speed = distance_2d(wanted.was_at, at) / look;
            // Faster than anything goes is a jump: a new life, or carried off by an arrest.
            let heading = if speed < HEADING_MAX { [(at[0] - wanted.was_at[0]) / look, (at[2] - wanted.was_at[2]) / look] } else { [0.0; 2] };
            wanted.was_at = at;
            let mut fled = false;
            if seen {
                (wanted.seen_at, wanted.unseen, wanted.heading) = (at, 0, heading);
                wanted.fled = if speed > FLEE_SPEED { wanted.fled + LOOK_EVERY } else { 0 };
                if wanted.fled > ticks(FLEE_EVERY) {
                    (wanted.fled, fled) = (0, true);
                }
            } else if searched || distance_2d(wanted.seen_at, at) > AWAY {
                wanted.unseen += LOOK_EVERY;
            }
            let star = stars_of(wanted.heat).clamp(1, 5) as usize;
            let escaped = wanted.unseen > ticks(ESCAPE[star - 1]);
            if escaped {
                clear(world, id);
            } else if fled {
                charge(world, id, HEAT_FLEE, Some(FLEE_STARS));
            }
        }
    }
}
