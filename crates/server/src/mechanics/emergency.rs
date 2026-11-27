//! The emergency services (EMERGENCY.md): who is sent where, and what they do when they get there.
//!
//! `Traffic` drives the police cars and ambulances (`traffic::fleet`); this gives them their orders
//! through `World::fleet`, and is the people in them once they are out of the car.
//!
//! - **Casualties.** A civilian who is killed, or hurt so badly that they stay down
//!   (`Civilian::hurt`), is a casualty. Those near each other are one *incident*, and an incident
//!   gets one ambulance. It parks by the place, its two medics get out, walk to each casualty in
//!   turn, lift them and carry them back, and then the ambulance drives off. Where nobody is near
//!   enough to see any of it, the casualties are simply gone when the ambulance has arrived.
//! - **The wanted.** A player who does what the police will not have (`wanted`) is theirs to
//!   catch: police cars are sent to where they were last seen, more of them the more stars the
//!   player has, and follow by road as they are seen elsewhere. Parked near them, a car's two
//!   officers get out (`officers`) to arrest or to shoot. One rule ends every chase: a unit whose
//!   suspect is wanted no more goes home, whether they died, were arrested, got away or left.

mod medics;
mod officers;
mod wanted;

use super::Mechanic;
use super::traffic::{Duty, FleetOrder, Service, viewers};
use super::vehicles::exit_position;
use crate::entities::With;
use crate::game::{Noise, Player, Seats, Vehicle, World};
use bevy_ecs::message::MessageCursor;
pub use medics::Medic;
pub use officers::{Officer, officer_loadout};
pub use wanted::{Wanted, clear as clear_wanted, stars as wanted_stars, wanted_byte};
#[cfg(test)]
pub use wanted::charge as charge_wanted;
use shared::consts::*;
use std::collections::VecDeque;

/// The team of everyone who works for the emergency services: they do not hurt each other.
pub const TEAM_SERVICES: u8 = 254;
/// What the medics and officers are drawn as, `KIND_*`.
pub const MEDIC_KIND: u8 = KIND_MEDIC;
pub const OFFICER_KIND: u8 = KIND_OFFICER;
/// A police car parked within this of where its suspect was last seen puts its officers out,
/// metres; one further off is sent on first.
const DEPLOY_WITHIN: f32 = 60.0;
/// A police car on its way pulls over where it is once it is this near where it looks for its
/// suspect, metres, rather than go on to the one place on the road nearest it.
const PULL_WITHIN: f32 = 50.0;
/// Where its cars are and what they should do is looked at every this many ticks; more cars sent, and
/// those left behind called off, once a second.
const DISPATCH_EVERY: u32 = 6;
/// Officers do not get out for a suspect who is driving faster than this, m/s: the car follows.
const DEPLOY_SPEED: f32 = 5.0;
/// A police car on its way is told where its suspect is now every this many seconds.
const FOLLOW_EVERY: u32 = 2;
/// A police car this near a suspect the police can see leaves the road and goes straight for them, metres.
const CHASE_WITHIN: f32 = 150.0;
/// The most police cars sent for one suspect in a second.
const SENDS_PER_SECOND: usize = 2;
/// A police car further than this from where it looks for its suspect, metres, that has got no
/// `GAIN` metres nearer there in the last `LAG_FOR` seconds, and has nobody out of it, is called off:
/// another is sent from ahead of them in its place.
const BEHIND: f32 = 250.0;
const GAIN: f32 = 10.0;
const LAG_FOR: f32 = 8.0;
/// From `LETHAL_STARS`, police cars whose suspect has been out of their sight for `SEARCH_AFTER`
/// seconds search ahead of where they were going: as far on as they could have gone since, up to
/// `SEARCH_AHEAD` metres, each car spread out to its own side by up to `SEARCH_SPREAD` metres.
const SEARCH_AFTER: f32 = 3.0;
const SEARCH_AHEAD: f32 = 150.0;
const SEARCH_SPREAD: f32 = 60.0;
/// An incident's tags have this bit, so that no player's id is ever one.
const TAG_INCIDENT: u32 = 1 << 31;
/// Casualties within this of an incident's first are part of it, metres, for this long after it, seconds.
const MERGE_WITHIN: f32 = 60.0;
const MERGE_FOR: f32 = 120.0;
/// An ambulance is asked for this long after an incident's first casualty, seconds, and asked for
/// again every second until one comes or this long has passed.
const DISPATCH_AFTER: f32 = 4.0;
const GIVE_UP: f32 = 90.0;
/// An ambulance that was on its way and is lost is replaced after this long, seconds.
const REPLACE_AFTER: f32 = 20.0;
/// How many casualties one ambulance takes.
const CARRIES: usize = 4;
/// The most casualties that lie waiting at once: past it the dead who have lain longest are gone.
const CASUALTIES_MAX: usize = 12;
/// A shot or a blast within this of a place, metres, in the last this many seconds, makes it
/// dangerous: medics stay in their ambulance, and those out of it keep down.
const DANGER_WITHIN: f32 = 40.0;
const DANGER_FOR: f32 = 5.0;
/// Someone is out of sight when no player is within this of them, metres.
const UNSEEN: f32 = 150.0;

/// What the emergency services ask of the room, which only it can do: people to put on their feet
/// beside their vehicle, and to take away again.
#[derive(Clone, Debug, PartialEq)]
pub enum EmergencyCall {
    /// A medic gets out, under `id`, onto their feet at `feet` facing `yaw`.
    Medic { id: u32, feet: [f32; 3], yaw: f32, medic: Box<Medic> },
    /// An officer gets out, under `id`, onto their feet at `feet` facing `yaw`.
    Officer { id: u32, feet: [f32; 3], yaw: f32, officer: Box<Officer> },
    /// Medic or officer `id` is out of sight, or has lain dead long enough: they leave the game.
    Gone(u32),
    /// Medic or officer `id` is at the door of seat `seat` of their own `vehicle`: they get back in
    /// by it, its crew again (`traffic::Driver`, of `kind`), under the same id.
    Aboard { id: u32, vehicle: u32, seat: usize, kind: u8 },
}

impl EmergencyCall {
    /// The same call for `id`, out onto their feet at `feet` facing `yaw`.
    fn out_as(self, id: u32, feet: [f32; 3], yaw: f32) -> Self {
        match self {
            EmergencyCall::Medic { medic, .. } => EmergencyCall::Medic { id, feet, yaw, medic },
            EmergencyCall::Officer { officer, .. } => EmergencyCall::Officer { id, feet, yaw, officer },
            other => other,
        }
    }
}

/// Casualties near each other, and the ambulance that comes for them.
#[derive(Clone, Debug, PartialEq)]
struct Incident {
    /// What its ambulance is sent for (`Unit::tag`).
    tag: u32,
    /// Where its first casualty lay, and the tick they were found.
    at: [f32; 3],
    since: u32,
    /// The civilians who lie there, by id.
    casualties: Vec<u32>,
    /// Its ambulance, once one is on its way, and the tick one that was is found to be lost.
    unit: Option<usize>,
    lost: Option<u32>,
    /// Its medics are out of the ambulance, and who they are.
    out: bool,
    medics: Vec<u32>,
    /// How many casualties are aboard.
    loaded: usize,
    /// The ambulance has been let go.
    done: bool,
}

/// Runs a world's emergency services.
#[derive(Default)]
pub struct Emergency {
    incidents: Vec<Incident>,
    /// How many incidents there have been.
    count: u32,
    /// Where it has got to in the noises the world makes, and the shots and blasts of the last
    /// `DANGER_FOR` seconds: the tick and the place of each.
    heard: MessageCursor<Noise>,
    shots: VecDeque<(u32, [f32; 3])>,
    /// The officers each police car has out of it, by its unit, and whether it has been sent on
    /// once already from where it stands.
    crews: Vec<Vec<u32>>,
    sent_on: Vec<bool>,
    /// For each unit: whom it is after, the nearest it has got to them since, metres, and the tick
    /// it last got `GAIN` nearer.
    gaps: Vec<(u32, f32, u32)>,
    /// The crews climbing out of their vehicles by the doors: each one's id, their vehicle and seat,
    /// and what they are to be once they stand outside (`EmergencyCall::out_as`).
    alighting: Vec<(u32, u32, usize, EmergencyCall)>,
}

/// How far to the side of the search unit `unit` takes, metres: spread evenly whatever the number
/// of units, by the golden ratio.
fn spread(unit: usize) -> f32 {
    ((unit as f32 * 0.618_034).fract() * 2.0 - 1.0) * SEARCH_SPREAD
}

/// Where to look for the suspect `wanted`, `side` metres to the side: where the police last knew
/// them to be, or, from `LETHAL_STARS` once they have been out of sight a while, ahead of there
/// as far as they could have got since.
fn search(wanted: &Wanted, side: f32) -> [f32; 3] {
    let at = wanted.seen_at;
    let unseen = wanted.unseen as f32 / TICK_RATE as f32;
    if wanted::stars_of(wanted.heat) < wanted::LETHAL_STARS || unseen < SEARCH_AFTER {
        return at;
    }
    let speed = wanted.heading[0].hypot(wanted.heading[1]);
    if speed < 1.0 {
        // Stood still when last seen: all round where that was.
        let angle = side * 2.4;
        return [at[0] + angle.cos() * side.abs(), at[1], at[2] + angle.sin() * side.abs()];
    }
    let (dir, along) = ([wanted.heading[0] / speed, wanted.heading[1] / speed], (speed * unseen).min(SEARCH_AHEAD));
    [at[0] + dir[0] * along - dir[1] * side, at[1], at[2] + dir[1] * along + dir[0] * side]
}

fn distance_2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    (b[0] - a[0]).hypot(b[2] - a[2])
}

/// How many ticks `seconds` are.
fn ticks(seconds: f32) -> u32 {
    (seconds * TICK_RATE as f32) as u32
}

/// Whether `id` works for the emergency services.
pub fn in_services(world: &World, id: u32) -> bool {
    world.entities.has::<Medic>(id) || world.entities.has::<Officer>(id)
}

/// Whether what the vehicle `car` runs into is spared its health: a police car or an ambulance
/// knocks down whoever it hits, and hurts only someone the police are after.
pub fn spares(world: &World, car: u32, victim: u32) -> bool {
    world.fleet.units.iter().any(|unit| unit.vid == Some(car)) && world.entities.get::<Seats>(car).is_some_and(|seats| seats.rider(0).is_none()) && !world.entities.has::<Wanted>(victim)
}

/// Whether what officer `by` does to `victim` comes to nothing: an officer's rounds hurt nobody
/// but their suspect, and no vehicle the suspect is not in.
pub fn misses(world: &World, by: u32, victim: u32) -> bool {
    let Some(officer) = world.entities.get::<Officer>(by) else { return false };
    let aboard = world.entities.get::<Seats>(victim).is_some_and(|seats| seats.riders().any(|rider| rider == officer.suspect));
    victim != officer.suspect && !aboard
}

/// Where someone getting out of seat `seat` of vehicle `vid` stands, on the ground, and the way
/// the vehicle points: by its door, for a seat that has one that can be got out of.
fn door(world: &World, vid: u32, seat: usize) -> Option<([f32; 3], f32)> {
    let pose = world.physics.car_pose(vid)?;
    let kind = world.entities.get::<Vehicle>(vid)?.kind;
    if let Some(spot) = shared::vehicles::door_of(kind, seat).and_then(|(_, door)| super::vehicles::doors::door_spot(world, vid, door)) {
        return Some(spot);
    }
    let spot = exit_position(world, kind, &pose, seat);
    let floor = world.physics.cast_ray([spot[0], spot[1] + 0.5, spot[2]], [0.0, -1.0, 0.0], 4.0, 0).map_or(world.map.ground.height_at(spot[0], spot[2]), |fall| spot[1] + 0.5 - fall);
    Some(([spot[0], floor, spot[2]], pose.yaw()))
}

/// How near the door of a seat someone of a crew must stand to get back in by it, metres.
const BY_DOOR: f32 = 1.8;

/// The seat of `id`'s own vehicle (`unit`, parked where it was sent) whose door they stand by and
/// could get back in by: the nearest that nobody sits in.
fn boarding(world: &World, id: u32, unit: usize) -> Option<(u32, usize)> {
    let vid = world.fleet.units.get(unit).filter(|unit| unit.duty == Duty::OnScene)?.vid?;
    let at = world.entities.get::<Player>(id).filter(|_| world.entities.alive(id))?.character.position;
    let crew = super::traffic::crew_of(world, vid);
    (0..2)
        .filter(|seat| !crew.iter().any(|&(_, taken)| taken == *seat))
        .filter_map(|seat| door(world, vid, seat).map(|(feet, _)| (seat, distance_2d(at, feet))))
        .filter(|&(_, off)| off < BY_DOOR)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(seat, _)| (vid, seat))
}

/// Whether `id` is of a vehicle's crew still: out of it as a medic or an officer, or getting out
/// of it or back in.
fn crewing<M: crate::entities::Component>(world: &World, id: u32) -> bool {
    world.entities.has::<M>(id) || super::vehicles::stepping(world, id)
}

impl Emergency {
    /// Puts whoever sits in seat `seat` of `vid` out of it, to be `call` once they stand outside:
    /// climbing out by its door, which they leave open, or out at once where it has none that can be
    /// got out of. With nobody in that seat (nobody was near enough to see into the car as it came),
    /// someone fresh gets out at once. Their id, or `None` where there is nowhere to get out.
    fn put_out(&mut self, world: &mut World, vid: u32, seat: usize, call: EmergencyCall) -> Option<u32> {
        let sitting = super::traffic::crew_of(world, vid).into_iter().find(|&(_, at)| at == seat).map(|(id, _)| id);
        if let Some(id) = sitting
            && super::vehicles::doors::start_out(world, id, vid, seat, false)
        {
            self.alighting.push((id, vid, seat, call));
            return Some(id);
        }
        let (feet, yaw) = door(world, vid, seat)?;
        let id = sitting.unwrap_or_else(|| world.alloc_id());
        world.emergency_calls.push(call.out_as(id, feet, yaw));
        Some(id)
    }

    /// Those of the crews who have climbed out and stand by their door: a medic or an officer from
    /// here, under the same id. One whose vehicle went from under them is gone with it.
    fn alight_crews(&mut self, world: &mut World) {
        if self.alighting.is_empty() {
            return;
        }
        let mut still = Vec::new();
        for (id, vid, seat, call) in std::mem::take(&mut self.alighting) {
            if !super::vehicles::stepping(world, id) {
                if world.entities.has::<super::traffic::Driver>(id) {
                    world.entities.despawn(id);
                }
                continue;
            }
            if !super::vehicles::stepped_out(world, id) {
                still.push((id, vid, seat, call));
                continue;
            }
            match door(world, vid, seat) {
                Some((feet, yaw)) => world.emergency_calls.push(call.out_as(id, feet, yaw)),
                None => {
                    world.entities.despawn(id);
                }
            }
        }
        self.alighting = still;
    }

    /// Sees to the police cars: those on their way told where their suspect is now, or to pull over
    /// near them, officers put out of one that has parked by them, and every car whose suspect is
    /// wanted no more let go. Each `full` second also more sent for each of the wanted who has fewer
    /// than their stars call for, and those left behind called off.
    fn dispatch(&mut self, world: &mut World, full: bool) {
        if !world.fleet.police {
            return;
        }
        let tick = world.tick;
        let units = world.fleet.units.clone();
        self.crews.resize(units.len(), Vec::new());
        self.sent_on.resize(units.len(), false);
        self.gaps.resize(units.len(), (0, 0.0, 0));
        let mut hunted: Vec<(u32, f32, [f32; 3])> = world.entities.all::<Wanted>().map(|(id, wanted)| (id, wanted.heat, search(wanted, 0.0))).collect();
        let in_sight: Vec<u32> = world.entities.all::<Wanted>().filter(|(_, wanted)| wanted.unseen == 0).map(|(id, _)| id).collect();
        hunted.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let follow = full && (tick / TICK_RATE).is_multiple_of(FOLLOW_EVERY);
        let mut called_off: Vec<usize> = Vec::new();
        for (unit, at) in units.iter().enumerate() {
            self.crews[unit].retain(|&id| crewing::<Officer>(world, id));
            if at.service != Service::Police || at.tag == 0 || at.tag & TAG_INCIDENT != 0 {
                continue;
            }
            let out = !self.crews[unit].is_empty();
            let Some(&(suspect, ..)) = hunted.iter().find(|&&(id, ..)| id == at.tag) else {
                // Its suspect is wanted no more: it goes, once its officers are back in it.
                if !out {
                    world.fleet.orders.push(FleetOrder::Release { unit });
                }
                continue;
            };
            // Where this car looks for them: where they are seen, or its own part of the search.
            let seen_at = world.entities.get::<Wanted>(suspect).map_or(at.at, |wanted| search(wanted, spread(unit)));
            let now = crate::mechanics::position_of(world, suspect);
            // Left behind and getting no nearer where it looks for them: called off, for one from
            // ahead of them.
            let gap = distance_2d(at.at, seen_at);
            let lag = &mut self.gaps[unit];
            if lag.0 != suspect {
                *lag = (suspect, gap, tick);
            } else if gap < lag.1 - GAIN {
                (lag.1, lag.2) = (gap, tick);
            }
            if full && !out && gap > BEHIND && tick.wrapping_sub(lag.2) > ticks(LAG_FOR) {
                world.fleet.orders.push(FleetOrder::Recall { unit });
                called_off.push(unit);
                continue;
            }
            // In sight and near: the car goes straight for them, across whatever is between.
            let near = at.vid.is_some() && in_sight.contains(&suspect) && now.is_some_and(|p| distance_2d(at.at, p) < CHASE_WITHIN);
            match at.duty {
                Duty::Chasing => {
                    self.sent_on[unit] = false;
                    if !near {
                        world.fleet.orders.push(FleetOrder::Redirect { unit, to: seen_at });
                    }
                }
                Duty::Responding if near && distance_2d(at.at, seen_at) > DEPLOY_WITHIN / 2.0 => {
                    world.fleet.orders.push(FleetOrder::Chase { unit, quarry: suspect });
                }
                // Near where it looks for them: off the road where it is, and out.
                Duty::Responding if distance_2d(at.at, seen_at) < PULL_WITHIN => {
                    world.fleet.orders.push(FleetOrder::Pull { unit });
                }
                Duty::Responding => {
                    self.sent_on[unit] = false;
                    if follow {
                        world.fleet.orders.push(FleetOrder::Redirect { unit, to: seen_at });
                    }
                }
                Duty::OnScene if !out => {
                    // A suspect who drives is followed by car, and one who is far off is driven
                    // nearer first, if the road goes nearer.
                    let driving = world.entities.get::<Player>(suspect).and_then(|player| player.seat).and_then(|seat| world.physics.car_pose(seat.vehicle)).is_some_and(|pose| pose.forward_speed.abs() > DEPLOY_SPEED);
                    let far = distance_2d(at.at, seen_at) > DEPLOY_WITHIN;
                    if near && (driving || far) {
                        world.fleet.orders.push(FleetOrder::Chase { unit, quarry: suspect });
                    } else if driving || (far && !self.sent_on[unit]) {
                        world.fleet.orders.push(FleetOrder::Redirect { unit, to: seen_at });
                        self.sent_on[unit] = true;
                    } else if let Some(vid) = at.vid {
                        // Both of the crew at once, by their own doors.
                        let yaw = world.physics.car_pose(vid).map_or(0.0, |pose| pose.yaw());
                        for seat in 0..2 {
                            let call = EmergencyCall::Officer { id: 0, feet: [0.0; 3], yaw, officer: Box::new(Officer::new(unit, suspect, seat, yaw)) };
                            if let Some(id) = self.put_out(world, vid, seat, call) {
                                self.crews[unit].push(id);
                            }
                        }
                        self.sent_on[unit] = false;
                    }
                }
                _ => {}
            }
        }
        if !full {
            return;
        }
        // More cars for whoever has fewer than their stars call for, the most wanted first, from
        // ahead of where they are going.
        for &(id, heat, to) in &hunted {
            let star = wanted::stars_of(heat).clamp(1, 5) as usize;
            let have = units.iter().enumerate().filter(|&(unit, at)| at.tag == id && !called_off.contains(&unit)).count();
            let ahead = Some(world.entities.get::<Wanted>(id).map_or([0.0; 2], |wanted| wanted.heading));
            for _ in 0..wanted::CARS[star - 1].saturating_sub(have).min(SENDS_PER_SECOND) {
                world.fleet.orders.push(FleetOrder::Send { service: Service::Police, to, tag: id, ahead });
            }
        }
    }

    /// Whether a shot or a blast went off near `at` a moment ago.
    fn danger(&self, at: [f32; 3]) -> bool {
        self.shots.iter().any(|&(_, shot)| distance_2d(shot, at) < DANGER_WITHIN)
    }

    /// Finds the casualties, sorts them into incidents, and sees to each incident: an ambulance
    /// asked for, its medics put out when it has arrived, and it let go when they are back.
    fn survey(&mut self, world: &mut World) {
        let tick = world.tick;
        // Everyone who lies waiting, with where and for how long, the longest first.
        let mut lying: Vec<(u32, [f32; 3], bool, u32)> = world
            .entities
            .all::<super::Civilian>()
            .filter_map(|(id, civilian)| {
                let dead = !world.entities.alive(id);
                let at = world.entities.get::<Player>(id)?.character.position;
                (dead || civilian.hurt).then_some((id, at, dead, civilian.lain + civilian.waited))
            })
            .collect();
        lying.sort_by_key(|&(id, _, _, lain)| (std::cmp::Reverse(lain), id));
        // Too many: the dead who have lain longest are gone, but for those a medic is on the way to.
        let spoken_for: Vec<u32> = self.incidents.iter().filter(|incident| incident.out).flat_map(|incident| incident.casualties.iter().copied()).collect();
        let mut over = lying.len().saturating_sub(CASUALTIES_MAX);
        lying.retain(|&(id, _, dead, _)| {
            if over > 0 && dead && !spoken_for.contains(&id) {
                over -= 1;
                world.traffic_calls.push(super::TrafficCall::Gone(id));
                return false;
            }
            true
        });
        for incident in &mut self.incidents {
            incident.casualties.retain(|id| lying.iter().any(|&(other, ..)| other == *id));
        }
        for &(id, at, ..) in &lying {
            if self.incidents.iter().any(|incident| incident.casualties.contains(&id)) {
                continue;
            }
            let near = self.incidents.iter_mut().find(|incident| !incident.done && distance_2d(incident.at, at) < MERGE_WITHIN && tick.wrapping_sub(incident.since) < ticks(MERGE_FOR));
            match near {
                Some(incident) => incident.casualties.push(id),
                None => {
                    self.count += 1;
                    self.incidents.push(Incident { tag: TAG_INCIDENT | self.count, at, since: tick, casualties: vec![id], unit: None, lost: None, out: false, medics: Vec::new(), loaded: 0, done: false });
                }
            }
        }
        for index in 0..self.incidents.len() {
            self.attend(index, world);
        }
        self.incidents.retain(|incident| !incident.done);
    }

    /// One look at incident `index`.
    fn attend(&mut self, index: usize, world: &mut World) {
        let tick = world.tick;
        let danger = self.danger(self.incidents[index].at);
        let incident = &mut self.incidents[index];
        let age = tick.wrapping_sub(incident.since);
        let unit = world.fleet.units.iter().position(|unit| unit.tag == incident.tag);
        let Some(unit) = unit else {
            if incident.unit.take().is_some() {
                // Its ambulance was wrecked, taken or lost on the way. Medics who were out are on
                // their own (`medics`), and what they had not yet seen to waits for the next.
                (incident.lost, incident.out) = (Some(tick), false);
                incident.medics.clear();
            }
            let waited = incident.lost.map_or(age > ticks(DISPATCH_AFTER), |lost| tick.wrapping_sub(lost) > ticks(REPLACE_AFTER));
            if incident.casualties.is_empty() || age > ticks(GIVE_UP + MERGE_FOR) || (incident.lost.is_none() && age > ticks(GIVE_UP)) {
                incident.done = true;
            } else if waited && world.fleet.ambulance {
                world.fleet.orders.push(FleetOrder::Send { service: Service::Ambulance, to: incident.at, tag: incident.tag, ahead: None });
            }
            return;
        };
        incident.unit = Some(unit);
        // Help is coming: they lie there until it has.
        for &id in &incident.casualties {
            if let Some(mut civilian) = world.entities.get_mut::<super::Civilian>(id) {
                civilian.claimed = true;
            }
        }
        let at = world.fleet.units[unit];
        if at.duty != Duty::OnScene {
            return;
        }
        if incident.out {
            // Everyone is back aboard, or will not be coming.
            incident.medics.retain(|&id| crewing::<Medic>(world, id));
            if incident.medics.is_empty() {
                world.fleet.orders.push(FleetOrder::Release { unit });
                incident.done = true;
            }
            return;
        }
        if incident.casualties.is_empty() {
            world.fleet.orders.push(FleetOrder::Release { unit });
            incident.done = true;
            return;
        }
        if danger {
            return;
        }
        let Some(vid) = at.vid else {
            // Nobody is near enough to see: the casualties are aboard, and it is off.
            for &id in incident.casualties.iter().take(CARRIES) {
                world.traffic_calls.push(super::TrafficCall::Gone(id));
            }
            world.fleet.orders.push(FleetOrder::Release { unit });
            incident.done = true;
            return;
        };
        let yaw = world.physics.car_pose(vid).map_or(0.0, |pose| pose.yaw());
        let tag = incident.tag;
        incident.out = true;
        for seat in 0..2 {
            let call = EmergencyCall::Medic { id: 0, feet: [0.0; 3], yaw, medic: Box::new(Medic::new(unit, tag, seat, yaw)) };
            if let Some(id) = self.put_out(world, vid, seat, call) {
                self.incidents[index].medics.push(id);
            }
        }
    }
}

impl Mechanic for Emergency {
    #[cfg(test)]
    fn inside(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    #[cfg(test)]
    fn inside_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn on_leave(&mut self, world: &mut World, id: u32) {
        world.entities.remove::<Medic>(id);
        world.entities.remove::<Officer>(id);
        world.entities.remove::<Wanted>(id);
    }

    /// Who did what to whom this tick, and what the police make of it.
    fn react(&mut self, world: &mut World) {
        if world.fleet.police {
            self.judge(world);
        }
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        if !world.fleet.on() {
            return;
        }
        let tick = world.tick;
        // Shots and blasts, by anyone.
        let noises: Vec<Noise> = self.heard.read(world.events::<Noise>()).copied().collect();
        // An engine going by is no danger: only shots and blasts are.
        self.shots.extend(noises.iter().filter(|noise| noise.line.is_some() || noise.reach >= crate::mechanics::bots::HEAR_BLAST).map(|noise| (tick, noise.at)));
        self.overhear(world, &noises);
        if tick.is_multiple_of(wanted::LOOK_EVERY) {
            self.watch(world);
        }
        while self.shots.front().is_some_and(|&(at, _)| tick.wrapping_sub(at) > ticks(DANGER_FOR)) {
            self.shots.pop_front();
        }
        self.alight_crews(world);
        if tick.is_multiple_of(TICK_RATE) {
            self.survey(world);
            self.dispatch(world, true);
        } else if tick.is_multiple_of(DISPATCH_EVERY) {
            self.dispatch(world, false);
        }
        if !world.entities.ids::<With<Medic>>().is_empty() || !world.entities.ids::<With<Officer>>().is_empty() {
            let viewers = viewers(world);
            self.tend(world, dt, &viewers);
            self.police(world, dt, &viewers);
        }
    }
}

#[cfg(test)]
mod tests;
