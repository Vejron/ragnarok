//! The emergency fleet (EMERGENCY.md): the police cars and ambulances, which are cars of the traffic
//! with somewhere to be.
//!
//! A world with emergency services (`Fleet::on`) has `FLEET` rows ahead of its traffic's, one a
//! unit. A unit with nothing to do is `Dormant`: on no road, and nothing at all. A police car on
//! patrol drives as traffic does. One that is sent somewhere (`FleetOrder::Send`) plans the way
//! there by road (`route`), drives it fast with the traffic ahead pulling over for it, and parks on
//! the verge at the end of it (`OnScene`). Let go again (`FleetOrder::Release`) it drives off as
//! traffic, and is gone once nobody sees it.
//!
//! Who is sent where, and what the people in the cars do when they get there, is not traffic's to
//! say (`mechanics::emergency`): it gives the orders through `World::fleet` and reads there where
//! each unit is.

use super::drivers::Mood;
use super::graph::Way;
use super::route::{Route, Spot};
use super::{Car, HIDE_BEYOND, LOOK_EVERY, REVERSE, STALL, TOP_SHARE, TRIES, Traffic, ticks, yaw_of};
use shared::quant::wrap_angle;
use shared::vehicles::vehicle_def;
use crate::game::{Autopilot, World};
use shared::consts::*;
use sim::CarControls;
use std::collections::VecDeque;

/// How many units a world's emergency services have, and how many of them are police cars: the
/// first `POLICE` rows, with the ambulances after them.
pub const FLEET: usize = 22;
pub const POLICE: usize = 20;
/// A place is on the road nearest it within this, metres; one further than that from any road is
/// not driven to.
const REACH: f32 = 150.0;
/// A unit already on the road is sent if it can be there in this long, seconds; else one comes
/// from further off.
const NEAR: f32 = 90.0;
/// The longest drive a unit is sent on, seconds.
const LONGEST: f32 = 240.0;
/// A fresh unit starts at least this far from where it is sent, metres, as the crow flies: it is
/// seen to arrive.
const START_FROM: f32 = 250.0;
/// How many places are tried for a fresh unit to start from; the one with the shortest drive is taken.
const START_TRIES: usize = 16;
/// A fresh police car starts out of sight near where it is sent if it can: this far from it, metres,
/// and further by how far whoever it is sent after goes in `LEAD` seconds, up to `LEAD_MAX` metres,
/// ahead of them within `SPREAD` radians of the way they go when they go faster than `SPREAD_SPEED`
/// m/s; on a road no further than `RING_REACH` metres from the place looked at, and no more than
/// `RING_DRIVE` seconds' drive from where it is sent.
const RING: [f32; 2] = [120.0, 220.0];
const LEAD: f32 = 6.0;
const LEAD_MAX: f32 = 150.0;
const SPREAD: f32 = 0.7;
const SPREAD_SPEED: f32 = 5.0;
const RING_REACH: f32 = 60.0;
const RING_DRIVE: f32 = 45.0;
/// A unit this near the end of its way takes the turn it meant to, and plans from there, metres.
const COMMITTED: f32 = 20.0;
/// A unit makes for the verge from this far short of where it stops, metres.
pub(super) const PARK_FROM: f32 = 30.0;
/// It has arrived when it is within this of where it meant to stop, metres, going slower than
/// `PARK_ROLLING` m/s (its crew get out as it rolls to a stop), or has stood for `PARK_STOOD` seconds
/// within `PARK_SHORT` of it, held up by what is in the road there.
const PARK_NEAR: f32 = 6.0;
const PARK_ROLLING: f32 = 3.0;
const PARK_SHORT: f32 = 25.0;
const PARK_STOOD: f32 = 2.0;
/// Traffic this far ahead of a siren pulls over, metres, and drives on this long after it last
/// heard one behind it, seconds.
const YIELD_AHEAD: f32 = 80.0;
const YIELD_FOR: f32 = 2.0;
/// A place a unit is sent on to is the one it is already going to if it is within this of it
/// along the same road, metres.
const SAME_PLACE: f32 = 20.0;

/// A unit that has left the road is lost only when it is this far from it, metres.
pub(super) const REJOIN: f32 = 400.0;
/// A unit chasing someone stops this far short of one who is on foot or all but standing, metres,
/// and takes one who goes slower than `CHASE_SLOW` m/s to be standing.
const CHASE_STOP: f32 = 10.0;
const CHASE_SLOW: f32 = 3.0;
/// Its chase is over when it is within this of where it means to stop, metres, going slower than
/// `HALT_SPEED` m/s: its crew jump out as it rolls to a stop.
const HALT_SHORT: f32 = 8.0;
const HALT_SPEED: f32 = 6.0;
/// It brakes this hard to stop short of one on foot, m/s².
const CHASE_BRAKING: f32 = 6.0;
/// A unit told to pull over stops on the verge this far ahead at the least, metres, or as far as it
/// takes to brake from its speed at `PULL_BRAKING` m/s².
const PULL_AHEAD: f32 = 12.0;
const PULL_BRAKING: f32 = 7.0;
/// While it chases, the road nearest it is looked up every this many ticks.
const CHASE_LOCATE: u32 = 30;
/// A unit chasing someone who drives boosts while it is further than this from them, metres, and
/// heads within `BOOST_BEARING` radians of the way it wants to go.
const BOOST_FROM: f32 = 20.0;
const BOOST_BEARING: f32 = 0.3;
/// Within this of whom it chases, metres, it finds its way round buildings by the soldiers' grid,
/// once a second, looking at no more than `PATH_CELLS` cells; and heads for the corner of that way
/// at least `CORNER_AHEAD` metres off.
const PATH_WITHIN: f32 = 80.0;
const PATH_CELLS: usize = 3_000;
/// It heads for the furthest corner of that way within this, metres, that it has room to drive
/// straight to, looking at this many of them, the furthest first.
const CORNER_WITHIN: f32 = 30.0;
const CORNERS: usize = 5;

/// Which of the emergency services a unit is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Police,
    Ambulance,
}

impl Service {
    /// The service of the unit in row `slot`.
    pub fn of(slot: usize) -> Self {
        if slot < POLICE { Service::Police } else { Service::Ambulance }
    }

    /// The rows of its units.
    fn slots(self) -> std::ops::Range<usize> {
        match self {
            Service::Police => 0..POLICE,
            Service::Ambulance => POLICE..FLEET,
        }
    }

    /// The vehicle it drives, `KIND_*`.
    pub fn kind(self) -> u8 {
        match self {
            Service::Police => KIND_POLICE_CAR,
            Service::Ambulance => KIND_AMBULANCE,
        }
    }
}

/// What a car of the traffic is on the road for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Duty {
    /// Nothing: it is traffic.
    Civil,
    /// A unit that is nowhere: on no road, waiting to be sent.
    Dormant,
    /// A police car driving about as traffic does.
    Patrol,
    /// On its way to where it was sent.
    Responding,
    /// Off the road after someone, wherever they go (`FleetOrder::Chase`).
    Chasing,
    /// Parked where it was sent, or where its chase ended.
    OnScene,
    /// Let go: driving off as traffic, to be gone once nobody sees it.
    Leaving,
}

/// Where a unit is and what it is doing, for whoever gives the fleet its orders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unit {
    pub service: Service,
    pub duty: Duty,
    /// Where it is, metres, and the way it points, radians. Of no meaning while it is dormant.
    pub at: [f32; 3],
    pub yaw: f32,
    /// Its vehicle, while a player is near enough for it to have one.
    pub vid: Option<u32>,
    /// What it was sent for, as whoever sent it named it; 0 for nothing.
    pub tag: u32,
    /// Its siren is on.
    pub siren: bool,
}

/// What the fleet is told to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FleetOrder {
    /// A unit of `service` goes to `to`, for `tag`: the nearest one on the road, or a fresh one from
    /// out of sight. With `ahead`, it is after someone there who goes at that (m/s over the ground,
    /// x and z), and a fresh one starts near them, ahead of them; without, it is seen to arrive from
    /// far off. Nothing comes of it when none is free or no road leads there.
    Send { service: Service, to: [f32; 3], tag: u32, ahead: Option<[f32; 2]> },
    /// Unit `unit`, which was sent somewhere, goes on to `to` instead.
    Redirect { unit: usize, to: [f32; 3] },
    /// Unit `unit`, on its way, pulls onto the verge just ahead of where it is and stops there.
    Pull { unit: usize },
    /// Unit `unit`, which has a vehicle and is near, goes straight for player `quarry`, road or no
    /// road. It parks when it is up to one who has stopped, or is stuck.
    Chase { unit: usize, quarry: u32 },
    /// Unit `unit` is done: it drives off.
    Release { unit: usize },
    /// Unit `unit`, which has nobody out of it and is no use where it is, is gone at once if nobody
    /// sees it go, and else drives off.
    Recall { unit: usize },
}

/// A world's emergency fleet, as `Traffic` and whoever commands it share it.
#[derive(Clone, Debug, Default)]
pub struct Fleet {
    /// It has ambulances, which come for casualties, and police.
    pub ambulance: bool,
    pub police: bool,
    /// How many police cars are out on patrol when nothing is going on.
    pub patrols: usize,
    /// The units, as of this tick. Empty until the roads are laid out.
    pub units: Vec<Unit>,
    /// What is asked of them. Drained every tick.
    pub orders: Vec<FleetOrder>,
}

impl Fleet {
    /// The world has emergency services of some kind.
    pub fn on(&self) -> bool {
        self.ambulance || self.police
    }
}

/// Where a unit was sent, and the way it has planned there.
#[derive(Clone, Debug)]
pub(super) struct Plan {
    pub(super) tag: u32,
    /// Where it is going; `None` when it has nowhere to be.
    pub(super) to: Option<Spot>,
    /// The ways it has yet to turn into, in order.
    pub(super) ways: VecDeque<Way>,
    /// The way it stops on, and how far along it, metres.
    pub(super) last: Way,
    pub(super) stop: f32,
    /// Whom it chases, and where they were a tick ago.
    pub(super) quarry: u32,
    pub(super) quarry_at: [f32; 3],
    /// Its way round what is between it and them, the tick it was found, and the side it last went
    /// round something on (`steering::avoid`).
    pub(super) path: Vec<[f32; 2]>,
    pub(super) path_at: u32,
    pub(super) lean: f32,
    /// It was told to pull over where it is (`FleetOrder::Pull`).
    pub(super) pulled: bool,
}

impl Default for Plan {
    fn default() -> Self {
        Plan { tag: 0, to: None, ways: VecDeque::new(), last: Way { edge: 0, back: false }, stop: 0.0, quarry: 0, quarry_at: [0.0; 3], path: Vec::new(), path_at: 0, lean: 0.0, pulled: false }
    }
}

impl Traffic {
    /// Puts the fleet's rows ahead of the traffic's, all dormant. Whatever names a row by its
    /// number is moved up with them.
    pub(super) fn enlist(&mut self) {
        if self.fleet > 0 {
            return;
        }
        for lane in &mut self.lanes {
            lane.iter_mut().for_each(|car| *car += FLEET as u32);
        }
        for car in &mut self.cars {
            car.passing = car.passing.map(|other| other + FLEET as u32);
        }
        self.cars.splice(0..0, (0..FLEET).map(|slot| Car { duty: Duty::Dormant, ..Car::new(Service::of(slot).kind(), 1.0) }));
        self.plans = vec![Plan::default(); FLEET];
        self.fleet = FLEET;
    }

    /// How far along its way unit `i` stops: only once it is on the last way of its plan.
    pub(super) fn stop_of(&self, i: usize) -> Option<f32> {
        if i >= self.fleet || self.cars[i].duty != Duty::Responding {
            return None;
        }
        let plan = &self.plans[i];
        (plan.to.is_some() && plan.ways.is_empty() && plan.last == self.cars[i].way).then_some(plan.stop)
    }

    /// Whether row `i` is a unit parked where it was sent.
    pub(super) fn parked(&self, vid: u32) -> bool {
        self.cars[..self.fleet].iter().any(|car| car.duty == Duty::OnScene && car.vid == Some(vid))
    }

    /// Unit `i` drives `route`, which starts on `from`.
    fn set_route(&mut self, i: usize, from: Way, route: Route) {
        let plan = &mut self.plans[i];
        (plan.last, plan.stop) = (route.ways.last().copied().unwrap_or(from), route.stop);
        plan.ways = route.ways.into();
    }

    /// The way car `i` goes on by from `way`, which it is coming onto: the next of its plan if it
    /// has one, planned afresh if it is not where the plan had it, and else any way out (`choose`).
    pub(super) fn next_way(&mut self, i: usize, way: Way) -> Option<Way> {
        if i < self.fleet
            && let Some(to) = self.plans[i].to
        {
            if self.plans[i].ways.front() == Some(&way) {
                self.plans[i].ways.pop_front();
            }
            match self.plans[i].ways.front() {
                Some(&next) if self.graph.exits(way).any(|out| out == next) => return Some(next),
                // On the last of its ways: it stops before the end of it.
                None if way == self.plans[i].last => {}
                // Not where its plan had it, or the road ahead is shut: another plan from here.
                _ => {
                    if let Some(route) = self.graph.route(way, 0.0, to, LONGEST) {
                        self.set_route(i, way, route);
                        if let Some(&next) = self.plans[i].ways.front() {
                            return Some(next);
                        }
                    }
                }
            }
        }
        self.choose(way)
    }

    /// Plans unit `i`'s way from where it is to where it was sent. False when no road leads there.
    pub(super) fn replan(&mut self, i: usize) -> bool {
        let car = self.cars[i];
        let Some(to) = self.plans.get(i).and_then(|plan| plan.to) else { return false };
        // Too near the end of its way to change its mind about the turn, or already let across
        // the junction: it plans from the way it goes on by.
        let left = self.graph.length(car.way) - car.along;
        let committed = car.after.filter(|_| car.duty != Duty::OnScene && (left < COMMITTED || car.holds.is_some()));
        let (from, along) = committed.map_or((car.way, car.along), |after| (after, 0.0));
        let Some(route) = self.graph.route(from, along, to, LONGEST) else { return false };
        self.set_route(i, from, route);
        if committed.is_none()
            && let Some(&next) = self.plans[i].ways.front()
        {
            self.cars[i].after = Some(next);
        }
        true
    }

    /// Unit `i` is where it was sent: it stands there, off its lane, until it is told otherwise.
    fn arrive(&mut self, i: usize) {
        self.leave(i);
        let car = &mut self.cars[i];
        (car.duty, car.mood, car.speed, car.stood) = (Duty::OnScene, Mood::Parked, 0.0, 0);
    }

    /// Unit `i`, parked, is back on its lane to drive on.
    fn resume(&mut self, i: usize) {
        let car = self.cars[i];
        self.enter(i, car.way, car.along, car.before);
    }

    /// Unit `i` is gone from the roads, vehicle and all, and has nowhere to be.
    pub(super) fn retire(&mut self, i: usize, world: &mut World) {
        if !matches!(self.cars[i].duty, Duty::OnScene | Duty::Chasing) {
            self.leave(i);
        }
        self.unshow(i, world);
        let car = &mut self.cars[i];
        (car.duty, car.mood) = (Duty::Dormant, Mood::Calm);
        self.plans[i] = Plan::default();
    }

    /// One tick of a parked unit: its brake is on, and nobody is at its wheel (its crew is out of
    /// it, `mechanics::emergency`).
    pub(super) fn stand(&mut self, i: usize, world: &mut World) {
        self.crew_out(i, world);
        let car = self.cars[i];
        if let Some(mut autopilot) = car.vid.filter(|_| car.physical).and_then(|vid| world.entities.get_mut::<Autopilot>(vid)) {
            autopilot.0 = CarControls { handbrake: true, ..CarControls::IDLE };
        }
    }

    /// A unit of `service` is sent to `to` for `tag`: the one that can be there soonest of those on
    /// the road near it and a fresh one, which with `ahead` starts out of sight near the place, ahead
    /// of whoever is there going at that.
    fn send(&mut self, service: Service, to: [f32; 3], tag: u32, ahead: Option<[f32; 2]>, world: &World) {
        let Some(spot) = self.graph.locate(to, REACH) else { return };
        // One that is already out, if it is near by road.
        let near = service
            .slots()
            .filter(|&i| matches!(self.cars[i].duty, Duty::Patrol | Duty::Leaving))
            .filter_map(|i| self.graph.route(self.cars[i].way, self.cars[i].along, spot, NEAR).map(|route| (route.cost, i)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let dormant = service.slots().find(|&i| self.cars[i].duty == Duty::Dormant);
        let fresh = ahead.filter(|_| dormant.is_some()).and_then(|ahead| self.start_near(to, spot, ahead, world));
        if let Some((cost, i)) = near
            && fresh.as_ref().is_none_or(|(.., route)| cost <= route.cost)
        {
            self.plans[i] = Plan { tag, to: Some(spot), ..Plan::default() };
            if self.replan(i) {
                (self.cars[i].duty, self.cars[i].mood) = (Duty::Responding, Mood::Responding);
                return;
            }
            self.plans[i] = Plan::default();
        }
        let Some(i) = dormant else { return };
        let Some((way, along, route)) = fresh.or_else(|| self.start_far(to, spot)) else { return };
        self.reset(i, 0.0);
        self.plans[i] = Plan { tag, to: Some(spot), ..Plan::default() };
        self.set_route(i, way, route);
        (self.cars[i].duty, self.cars[i].mood) = (Duty::Responding, Mood::Responding);
        self.enter(i, way, along, None);
    }

    /// A place out of everyone's sight within the `RING` round `to`, ahead of whoever is there going
    /// at `ahead`, for a fresh unit to start from, and its way to `spot`: of those tried, the one with
    /// the shortest drive.
    fn start_near(&mut self, to: [f32; 3], spot: Spot, ahead: [f32; 2], world: &World) -> Option<(Way, f32, Route)> {
        let speed = ahead[0].hypot(ahead[1]);
        let (heading, lead) = (ahead[1].atan2(ahead[0]), (speed * LEAD).min(LEAD_MAX));
        let mut found: Option<(Way, f32, Route)> = None;
        for _ in 0..START_TRIES {
            let angle = if speed > SPREAD_SPEED { heading + (self.rng.next_f64() as f32 * 2.0 - 1.0) * SPREAD } else { self.rng.next_f64() as f32 * std::f32::consts::TAU };
            let far = RING[0] + self.rng.next_f64() as f32 * (RING[1] - RING[0]) + lead;
            let Some(near) = self.graph.locate([to[0] + angle.cos() * far, to[1], to[2] + angle.sin() * far], RING_REACH) else { continue };
            let (p, _) = self.graph.pose(None, near.way, None, near.along);
            if !super::hidden(world, p) {
                continue;
            }
            // It may set off either way along the road.
            for (way, along) in [(near.way, near.along), (near.way.reversed(), self.graph.length(near.way) - near.along)] {
                if !self.graph.live[way.index()] || !self.clear(way, along) {
                    continue;
                }
                if let Some(route) = self.graph.route(way, along, spot, RING_DRIVE).filter(|route| found.as_ref().is_none_or(|(.., known)| route.cost < known.cost)) {
                    found = Some((way, along, route));
                }
            }
        }
        found
    }

    /// A place anywhere on the network where nobody is looking and not right beside `to`, for a
    /// fresh unit to start from, and its way to `spot`: of those tried, the one with the shortest drive.
    fn start_far(&mut self, to: [f32; 3], spot: Spot) -> Option<(Way, f32, Route)> {
        let mut found: Option<(Way, f32, Route)> = None;
        for attempt in 0..START_TRIES {
            // One try in four is a road in from the world's edge.
            let place = if attempt % 4 == 3 && !self.graph.entries.is_empty() {
                let way = self.graph.entries[(self.rng.next_u64() % self.graph.entries.len() as u64) as usize];
                Some((way, 1.0f32.min(self.graph.length(way) / 2.0)))
            } else {
                self.graph.place(self.rng.next_f64() as f32 * self.graph.driven())
            };
            let Some((way, along)) = place else { break };
            let (p, _) = self.graph.pose(None, way, None, along);
            let unseen = self.viewers.iter().all(|v| (v[0] - p[0]).hypot(v[2] - p[2]) > HIDE_BEYOND);
            if !unseen || (p[0] - to[0]).hypot(p[2] - to[2]) < START_FROM || !self.clear(way, along) {
                continue;
            }
            if let Some(route) = self.graph.route(way, along, spot, LONGEST).filter(|route| found.as_ref().is_none_or(|(.., known)| route.cost < known.cost)) {
                found = Some((way, along, route));
            }
        }
        found
    }

    /// Unit `i`, which was sent somewhere, goes on to `to`.
    fn redirect(&mut self, i: usize, to: [f32; 3]) {
        if i >= self.fleet || !matches!(self.cars[i].duty, Duty::Responding | Duty::OnScene | Duty::Chasing) {
            return;
        }
        let Some(spot) = self.graph.locate(to, REACH) else { return };
        let chasing = self.cars[i].duty == Duty::Chasing;
        let old = self.plans[i].to;
        // The same place, or as good as: on the same road and no further along it than matters.
        let same = old.is_some_and(|old| {
            let along = if old.way == spot.way { Some(old.along) } else if old.way == spot.way.reversed() { Some(self.graph.length(old.way) - old.along) } else { None };
            along.is_some_and(|along| (along - spot.along).abs() < SAME_PLACE)
        });
        if same && !chasing {
            return;
        }
        self.plans[i].to = Some(spot);
        if !self.replan(i) {
            self.plans[i].to = old;
            return;
        }
        self.plans[i].pulled = false;
        if matches!(self.cars[i].duty, Duty::OnScene | Duty::Chasing) {
            (self.cars[i].duty, self.cars[i].mood) = (Duty::Responding, Mood::Responding);
            self.resume(i);
        }
    }

    /// Unit `i`, on its way, stops on the verge just ahead of where it is: as far on as it takes to
    /// brake, on its way or the next it has taken.
    fn pull_over(&mut self, i: usize) {
        if i >= self.fleet || self.cars[i].duty != Duty::Responding || self.plans[i].pulled {
            return;
        }
        let car = self.cars[i];
        let ahead = (car.speed * car.speed / (2.0 * PULL_BRAKING)).max(PULL_AHEAD);
        let left = self.graph.length(car.way) - car.along;
        let (way, along) = match car.after {
            Some(after) if ahead > left - 1.0 => (after, (ahead - left).min(self.graph.length(after) - 1.0)),
            _ => (car.way, (car.along + ahead).min(self.graph.length(car.way) - 1.0)),
        };
        let plan = &mut self.plans[i];
        plan.to = Some(Spot { way, along, off: 0.0 });
        plan.ways = if way == car.way { VecDeque::new() } else { VecDeque::from([way]) };
        (plan.last, plan.stop, plan.pulled) = (way, along, true);
    }

    /// Unit `i` leaves the road after player `quarry`.
    fn pursue(&mut self, i: usize, quarry: u32) {
        if i >= self.fleet || self.cars[i].vid.is_none() || !matches!(self.cars[i].duty, Duty::Responding | Duty::OnScene) {
            return;
        }
        if self.cars[i].duty == Duty::Responding {
            self.leave(i);
        }
        let car = &mut self.cars[i];
        (car.duty, car.mood, car.stalled, car.reverse_left, car.tries) = (Duty::Chasing, Mood::Responding, 0, 0, 0);
        let plan = &mut self.plans[i];
        (plan.quarry, plan.quarry_at, plan.path, plan.lean) = (quarry, [f32::MAX; 3], Vec::new(), 0.0);
    }

    /// How many times the unit with vehicle `vid` has had to back out of being stuck.
    #[cfg(test)]
    pub fn tries_of(&self, vid: u32) -> u32 {
        self.cars[..self.fleet].iter().find(|car| car.vid == Some(vid)).map_or(0, |car| car.tries)
    }

    /// Unit `i`'s chase is over where it stands: it is parked there.
    fn halt(&mut self, i: usize) {
        let car = &mut self.cars[i];
        (car.duty, car.mood, car.speed, car.stood) = (Duty::OnScene, Mood::Parked, 0.0, 0);
    }

    /// One tick of a unit that chases someone: straight for them, wherever that takes it. It stops
    /// short of one who is on foot or has stopped, and where it is stuck for good; either way its
    /// crew can go on from there on foot.
    pub(super) fn chase(&mut self, i: usize, world: &mut World) {
        let car = self.cars[i];
        let Some(vid) = car.vid else {
            // Nobody near enough for it to be a car: back to the road it is nearest.
            self.halt(i);
            return;
        };
        let taken = world.entities.get::<crate::game::Seats>(vid).is_none_or(|seats| seats.rider(0).is_some());
        let Some(pose) = world.physics.car_pose(vid).filter(|_| !taken && !world.entities.dead(vid) && car.physical) else {
            if car.physical || taken {
                self.abandon(i, world);
            }
            return;
        };
        if pose.position[1] < KILL_Y + 5.0 || pose.up_y < 0.3 {
            self.abandon(i, world);
            return;
        }
        let quarry = self.plans[i].quarry;
        let Some(target) = crate::mechanics::position_of(world, quarry).filter(|_| world.entities.alive(quarry)) else {
            self.halt(i);
            return;
        };
        let seated = world.entities.get::<crate::game::Player>(quarry).is_some_and(|player| player.seat.is_some());
        let was = std::mem::replace(&mut self.plans[i].quarry_at, target);
        let fast = seated && was[0] != f32::MAX && (target[0] - was[0]).hypot(target[2] - was[2]) * TICK_RATE as f32 > CHASE_SLOW;
        // Where it is on the network, for when it goes back to the road.
        if (i as u32).wrapping_add(world.tick).is_multiple_of(CHASE_LOCATE)
            && let Some(spot) = self.graph.locate(pose.position, REACH)
        {
            let car = &mut self.cars[i];
            (car.way, car.along, car.before, car.after) = (spot.way, spot.along, None, None);
        }
        let [x, _, z] = pose.position;
        let (reach, speed) = ((target[0] - x).hypot(target[2] - z), pose.forward_speed);
        self.cars[i].speed = speed;
        let spec = &vehicle_def(car.kind).spec;
        // On the boost after one who drives and is not yet caught up with; flat out after one who
        // drives; and so as to stop short of one who does not.
        let boosting = fast && reach > BOOST_FROM && spec.boost.is_some();
        let top = if boosting { spec.max_speed * spec.boost.map_or(1.0, |boost| boost.speed) } else { spec.cruise_speed() * TOP_SHARE };
        let stop = reach - CHASE_STOP - spec.half_extents[2];
        let mut limit = if fast { top } else { top.min((2.0 * CHASE_BRAKING * stop.max(0.0)).sqrt()) };
        if !fast && stop < HALT_SHORT && speed.abs() < HALT_SPEED {
            self.halt(i);
            return;
        }
        // The way it wants: round what the soldiers' grid knows is between, when it is near, and
        // straight at them otherwise; and then round whatever it feels ahead.
        let plan = &mut self.plans[i];
        if reach < PATH_WITHIN && world.tick.wrapping_sub(plan.path_at) >= TICK_RATE {
            let feet = |p: [f32; 3]| [p[0], world.map.ground.height_at(p[0], p[2]), p[2]];
            let mut cells = PATH_CELLS;
            plan.path = world.nav.find_path(&world.physics, &world.map, feet(pose.position), feet(target), &mut cells).unwrap_or_default();
            plan.path_at = world.tick;
        } else if reach >= PATH_WITHIN {
            plan.path.clear();
        }
        let path: Vec<[f32; 2]> = plan.path.iter().copied().filter(|c| (c[0] - x).hypot(c[1] - z) < CORNER_WITHIN).collect();
        let open = |c: &[f32; 2]| {
            let reach = (c[0] - x).hypot(c[1] - z);
            crate::mechanics::steering::clear(world, vid, car.kind, &pose, (-(c[0] - x)).atan2(-(c[1] - z)), reach) >= reach - 0.5
        };
        let corner = path.iter().rev().take(CORNERS).find(|c| open(c)).or(path.first()).or(plan.path.first()).map_or([target[0], target[2]], |&c| c);
        let want = (-(corner[0] - x)).atan2(-(corner[1] - z));
        let (yaw, cap) = crate::mechanics::steering::avoid(world, vid, car.kind, &pose, want, speed, &mut plan.lean);
        let bearing = wrap_angle(yaw - pose.yaw());
        // Slow for what is ahead, and for a sharp turn.
        limit = limit.min(cap.max(2.0)).min(crate::mechanics::steering::turning(bearing, top));
        // It turns as for a point a few metres ahead that way, more the faster it goes.
        let ahead = (super::LOOK_AHEAD[0] + super::LOOK_PER_SPEED * speed.max(0.0)).min(super::LOOK_AHEAD[1]);
        let wheelbase = spec.axles.iter().copied().fold(f32::MIN, f32::max) - spec.axles.iter().copied().fold(f32::MAX, f32::min);
        let lock = (2.0 * wheelbase * bearing.sin() / ahead).atan();
        let steer = (-lock / spec.steer_lock.max(0.1)).clamp(-1.0, 1.0);
        let car = &mut self.cars[i];
        let controls = |throttle: f32, steer: f32, handbrake: bool| CarControls { throttle, steer, handbrake, powered: true, ..CarControls::IDLE };
        let asked = if car.reverse_left > 0 {
            car.reverse_left -= 1;
            controls(-1.0, -steer, false)
        } else {
            // Standing though it means to go: it backs out, and after a few goes it is stuck.
            car.stalled = if limit > 1.0 && speed.abs() < 0.3 { car.stalled + 1 } else { 0 };
            if car.stalled > ticks(STALL) {
                (car.stalled, car.tries, car.reverse_left) = (0, car.tries + 1, ticks(REVERSE));
            } else if speed > 3.0 {
                car.tries = 0;
            }
            let over = speed - limit;
            let throttle = if over < 0.0 { (-over * 0.6).min(1.0) } else if speed > 1.2 { (-over * 0.5).max(-1.0) } else { 0.0 };
            // Nearly straight at them with the way clear, and short of its speed: the boost on.
            let boost = boosting && bearing.abs() < BOOST_BEARING && over < 0.0;
            CarControls { boost, ..controls(throttle, steer, over > 3.0) }
        };
        let stuck = car.tries >= TRIES;
        if let Some(mut autopilot) = world.entities.get_mut::<Autopilot>(vid) {
            autopilot.0 = asked;
        }
        if stuck {
            self.halt(i);
        }
    }

    /// Unit `i` is done with what it was sent for: it drives off as traffic.
    fn dismiss(&mut self, i: usize) {
        if i >= self.fleet || !matches!(self.cars[i].duty, Duty::Responding | Duty::OnScene | Duty::Chasing) {
            return;
        }
        let parked = self.cars[i].duty != Duty::Responding;
        self.plans[i] = Plan::default();
        (self.cars[i].duty, self.cars[i].mood) = (Duty::Leaving, Mood::Calm);
        if parked {
            self.resume(i);
        }
    }

    /// Unit `i` is no use where it is: gone at once where nobody sees it go and nobody has taken
    /// it, and else let go to drive off.
    fn recall(&mut self, i: usize, world: &mut World) {
        if i >= self.fleet || !matches!(self.cars[i].duty, Duty::Responding | Duty::OnScene | Duty::Chasing) {
            return;
        }
        let car = self.cars[i];
        let at = car.vid.and_then(|vid| world.physics.car_pose(vid)).map_or_else(|| self.pose(i).0, |pose| pose.position);
        let taken = car.vid.and_then(|vid| world.entities.get::<crate::game::Seats>(vid)).is_some_and(|seats| seats.riders().next().is_some());
        if !taken && (car.vid.is_none() || super::hidden(world, at)) {
            self.retire(i, world);
        } else {
            self.dismiss(i);
        }
    }

    /// The traffic ahead of unit `i`, which has its siren on, pulls over: on its way as far as
    /// `YIELD_AHEAD`, and on the way it turns into.
    fn clear_road(&mut self, i: usize) {
        let car = self.cars[i];
        let left = self.graph.length(car.way) - car.along;
        let mut ahead: Vec<u32> = self.lanes[car.way.index()].iter().copied().filter(|&other| (0.0..YIELD_AHEAD).contains(&(self.cars[other as usize].along - car.along)) && other != i as u32).collect();
        if let Some(after) = car.after.filter(|_| left < YIELD_AHEAD) {
            ahead.extend(self.lanes[after.index()].iter().copied().filter(|&other| self.cars[other as usize].along < YIELD_AHEAD - left));
        }
        for other in ahead {
            let other = &mut self.cars[other as usize];
            if other.duty == Duty::Civil && matches!(other.mood, Mood::Calm | Mood::Hurrying { .. } | Mood::Yielding { .. }) {
                other.mood = Mood::Yielding { left: ticks(YIELD_FOR) };
            }
        }
    }

    /// One tick of the fleet: what it was told to do, who has arrived and who is gone, a patrol
    /// car to make up their number, and where every unit is for whoever commands them.
    pub(super) fn command(&mut self, world: &mut World) {
        if self.fleet == 0 {
            world.fleet.orders.clear();
            return;
        }
        for order in std::mem::take(&mut world.fleet.orders) {
            match order {
                FleetOrder::Send { service, to, tag, ahead } => self.send(service, to, tag, ahead, world),
                FleetOrder::Redirect { unit, to } => self.redirect(unit, to),
                FleetOrder::Pull { unit } => self.pull_over(unit),
                FleetOrder::Chase { unit, quarry } => self.pursue(unit, quarry),
                FleetOrder::Release { unit } => self.dismiss(unit),
                FleetOrder::Recall { unit } => self.recall(unit, world),
            }
        }
        let tick = world.tick;
        let wanted = if world.fleet.police { world.fleet.patrols.min(POLICE) } else { 0 };
        for i in 0..self.fleet {
            let car = self.cars[i];
            match car.duty {
                Duty::Responding => {
                    let there = self.stop_of(i).is_some_and(|stop| (car.along > stop - PARK_NEAR && car.speed.abs() < PARK_ROLLING) || (stop - car.along < PARK_SHORT && car.stood > ticks(PARK_STOOD)));
                    if there {
                        self.arrive(i);
                    } else if (i as u32).wrapping_add(tick).is_multiple_of(LOOK_EVERY) {
                        self.clear_road(i);
                    }
                }
                // Out of everyone's sight: back on patrol if the patrols are short of a car, and
                // else gone.
                Duty::Leaving if car.vid.is_none() && car.nearest > HIDE_BEYOND => {
                    let patrols = self.cars[..POLICE].iter().filter(|car| car.duty == Duty::Patrol).count();
                    if Service::of(i) == Service::Police && patrols < wanted {
                        self.cars[i].duty = Duty::Patrol;
                    } else {
                        self.retire(i, world);
                    }
                }
                _ => {}
            }
        }
        // A patrol car short: one comes into the world, or starts where nobody is looking.
        if tick.is_multiple_of(TICK_RATE)
            && self.cars[..POLICE].iter().filter(|car| car.duty == Duty::Patrol).count() < wanted
            && let Some(i) = (0..POLICE).find(|&i| self.cars[i].duty == Duty::Dormant)
        {
            self.cars[i].duty = Duty::Patrol;
            self.start(i, true);
        }
        // A unit's siren is on while it is on its way somewhere, for everyone to hear and see.
        for car in &self.cars[..self.fleet] {
            if let Some(mut vehicle) = car.vid.and_then(|vid| world.entities.get_mut::<crate::game::Vehicle>(vid)) {
                let on = car.mood == Mood::Responding;
                if vehicle.siren != on {
                    vehicle.siren = on;
                }
            }
        }
        world.fleet.units = (0..self.fleet)
            .map(|i| {
                let car = &self.cars[i];
                let (at, yaw) = match car.vid.and_then(|vid| world.physics.car_pose(vid)) {
                    Some(pose) => (pose.position, pose.yaw()),
                    None => {
                        let (p, d) = self.pose(i);
                        (p, yaw_of(d))
                    }
                };
                Unit { service: Service::of(i), duty: car.duty, at, yaw, vid: car.vid, tag: self.plans[i].tag, siren: car.mood == Mood::Responding }
            })
            .collect();
    }
}
