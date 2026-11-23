//! Traffic (TRAFFIC.md): on a world that asks for it (`World::traffic`), that many cars drive the
//! roads, each on its own errand to nowhere in particular. Nobody can see more than a few dozen of
//! them at once, so a car is only as real as whoever is near needs it to be:
//!
//! - **On rails, unseen.** Far from every player a car is a row here and nothing else: a way of the
//!   road network (`graph`), how far along it, and a speed. It keeps its distance to the car ahead,
//!   slows for bends and gives way at junctions, all by arithmetic.
//! - **On rails, shown.** Within `SHOW_WITHIN` of a player it is a vehicle like any other to look at
//!   and to shoot at, but its body is put where its row says each tick (`Physics::set_car_rails`),
//!   with no wheels or springs to work out.
//! - **Physical.** Within `PHYSICAL_WITHIN` of a player, or once it is hurt or held up, the body is
//!   let go: it is an ordinary car, steered and throttled along its lane as a driver would
//!   (`Autopilot`), and whatever hits it moves it. At most `PHYSICAL_MAX` are, the rest stay on rails.
//!
//! A car a player takes, or that is wrecked, stops being traffic (`Abandoned`), and a fresh row
//! starts somewhere nobody is looking, so the roads stay as busy as they were.
//!
//! A car also has a place across its road (`Car::shift`): it pulls out to pass a slower car or to
//! go round a vehicle left standing in its lane, when nobody is coming (`decide`). And the bridges
//! are looked at in turn (`inspect`): a road whose deck is broken is shut, with the roads that only
//! led to it.

mod drivers;
mod graph;

use super::Mechanic;
use super::vehicles::{add_vehicle, remove_vehicle};
use crate::entities::{Health, With};
use crate::game::{Autopilot, Bot, Noise, Player, Seats, Vehicle, World};
use bevy_ecs::message::MessageCursor;
use crate::rng::Rng;
pub use graph::{Graph, Way};
pub use drivers::{DRAG_SPEED, Driver};
use drivers::{Mood, Nerve};
use graph::BRAKING;
use shared::consts::*;
use shared::quant::wrap_angle;
use shared::vehicles::vehicle_def;
use shared::world::ROAD_MAJOR;
use sim::{CarControls, Physics};
use std::collections::VecDeque;

/// The most cars a world's traffic has.
pub const TRAFFIC_MAX: usize = 4000;
/// A car this near a player is shown, metres, and one this far from every player is not: past the
/// range a player is told of vehicles at (`INTEREST_RADIUS`), so it is driving before it is seen.
const SHOW_WITHIN: f32 = INTEREST_RADIUS + 20.0;
const HIDE_BEYOND: f32 = INTEREST_RADIUS + INTEREST_HYSTERESIS + 10.0;
/// A shown car this near a player is physical, metres, and one this far from every player goes back
/// on rails once it is on its lane.
const PHYSICAL_WITHIN: f32 = 45.0;
const RAILS_BEYOND: f32 = 70.0;
/// The most cars shown at once, and the most of those that are physical: each of these is a vehicle
/// the physics steps every tick.
const SHOWN_MAX: usize = 384;
const PHYSICAL_MAX: usize = 96;
/// How a car picks up speed, m/s², and the hardest it brakes.
const ACCEL: f32 = 2.0;
const HARD_BRAKE: f32 = 7.0;
/// The gap a car leaves to what is ahead of it at a standstill, metres, and how much more for every
/// m/s it does, seconds.
const GAP: f32 = 3.0;
const HEADWAY: f32 = 1.0;
/// A car asks for a junction when it could stop this far short of its stop line, metres.
const ASK_AHEAD: f32 = 12.0;
/// It waits this far short of the junction's node, metres.
const STOP_BACK: f32 = 9.0;
/// A car is across a junction when it is this far into the road it left by, metres.
const CLEAR_AFTER: f32 = 9.0;
/// A junction belongs to one road into it at a time. The cars on that road follow each other across
/// for at most this long, seconds, before the other roads get their turn.
const GREEN_MAX: f32 = 15.0;
/// A car is taken to cross at no less than this speed, m/s, and is given this long on top, seconds.
const CROSSING_SPEED: f32 = 4.0;
const CROSSING_SPARE: f32 = 1.0;
/// A car gives way to one on a bigger road that is less than this from the junction, seconds.
const GIVE_WAY: f32 = 4.5;
/// A car that has stood this long at a junction with no room on the road beyond goes anyway,
/// seconds: round a small block of short roads, each car would else wait for the one ahead for good.
const PATIENCE: f32 = 5.0;
/// A car that has stood this long where nobody sees it is taken away and starts afresh elsewhere,
/// seconds: whatever knot it was in, the roads do not fill up with cars waiting for each other.
const STUCK: f32 = 60.0;
/// Every this many ticks a car's distance to the players is measured, and a shown car looks ahead.
const NEAR_EVERY: u32 = 8;
const LOOK_EVERY: u32 = 4;
/// A car that is not shown is moved once in this many ticks, by that many ticks' worth, and a
/// bridge is looked at as often.
const UNSEEN_EVERY: u32 = 4;
/// A car on rails held up this long is made physical, seconds: what is in its way may be pushed,
/// and it does not stand there for good.
const HELD_RAILS: f32 = 8.0;
/// A physical car that has not moved for `STALL` seconds though nothing holds it backs out for
/// `REVERSE`; after `TRIES` of those, with no player within `UNSEEN`, it is taken away.
const STALL: f32 = 5.0;
const REVERSE: f32 = 2.0;
const TRIES: u32 = 3;
const UNSEEN: f32 = 60.0;
/// A physical car this far from its lane is lost, metres.
const LOST: f32 = 20.0;
/// A physical car goes back on rails when it is within this of its lane, metres, and this of its
/// heading, radians.
const ON_LANE: f32 = 0.8;
const ON_HEADING: f32 = 0.12;
/// A fresh car starts at least this far from the others on its lane, metres.
const START_CLEAR: f32 = 18.0;
/// The point a physical car steers at is this far ahead at a standstill and at most, metres, and
/// this much further for every m/s.
const LOOK_AHEAD: [f32; 2] = [5.0, 14.0];
const LOOK_PER_SPEED: f32 = 0.5;
/// A wreck that was traffic is taken away after this long, seconds.
const ABANDON_WRECK: f32 = 8.0;
/// A car that was traffic is taken away after standing empty this long, seconds: one in the
/// traffic's way that nobody watches, and one that somebody does; and likewise one that is in
/// nobody's way.
const ABANDON_IN_WAY: [f32; 2] = [20.0, 90.0];
const ABANDON_ASIDE: [f32; 2] = [45.0, 300.0];
/// A car is in the traffic's way for this long after a car of the traffic last found it in its
/// path, seconds.
const IN_WAY_FOR: f32 = 10.0;
/// A player this near an empty car that is in nobody's way keeps it from being taken away,
/// metres: it may be theirs.
const ABANDON_THEIRS: f32 = 15.0;
/// The most cars that were traffic left standing at once: past it the one empty longest goes.
const ABANDONED_MAX: usize = 24;
/// A spot is watched by a player within this of it who looks its way, within `WATCH_HALF_CONE`
/// (the cosine of the angle off where they look), and by one this near whichever way they look, metres.
const WATCH_WITHIN: f32 = 150.0;
const WATCH_HALF_CONE: f32 = 0.5;
const WATCH_NEAR: f32 = 40.0;
/// A shown car that has stood this long starts afresh elsewhere if nobody watches it, seconds, and
/// one that has stood this long is left by its driver where it is, watched or not.
const STOOD_UNWATCHED: f32 = 45.0;
const STOOD_WATCHED: f32 = 120.0;
/// How fast a car pulls over onto the verge, m/s, how far past the road's edge its side is when it
/// is there, metres, and how long it tries for before it is left where it is, seconds.
const PULL_OVER_SPEED: f32 = 4.0;
const VERGE: f32 = 0.5;
const PULL_OVER_FOR: f32 = 15.0;
/// A bridge's deck is looked for from this far over the ground, up to this far over it, and its
/// roadway from this far over its underside, metres.
const DECK_FROM: f32 = 0.3;
const SURFACE_FROM: f32 = 60.0;
const DECK_OVER: f32 = 3.0;
/// A bridge's deck is looked at every this far along it, metres, and is broken where its surface
/// is this far below where it was, metres.
const DECK_STEP: f32 = 2.0;
const DECK_DROP: f32 = 1.0;
/// A junction one road's cars are still crossing is not another road's for this long after their
/// time is up, seconds.
const CLEARING: f32 = 6.0;
/// What a shown car looks for in its way is looked for every this far along its path, metres, as
/// far as it needs to stop in at `FIRM_BRAKE`, m/s², and no further than `SIGHT_MAX`, metres.
const SIGHT_STEP: f32 = 2.0;
const FIRM_BRAKE: f32 = 4.0;
const SIGHT_MAX: f32 = 90.0;
/// A car that has pulled out takes this long to be back in its lane, seconds.
const PULL_IN: f32 = 2.5;
/// How far out a car moves to pass, metres, where a lane of its road is not as wide or is wider.
const OUT: [f32; 2] = [2.6, 3.5];
/// A car pulls out no nearer the end of its road than this, metres, and is back in its lane by it.
const PASS_END: f32 = 45.0;
/// A car passes one that is this much slower than it would drive, m/s, once it is within this of
/// it, metres, and this much more for every m/s it does.
const PASS_FASTER: f32 = 2.5;
const PASS_FROM: f32 = 12.0;
/// It needs this much clear road ahead of the car it passes to come back into, metres.
const PASS_ROOM: f32 = 25.0;
/// A car on the wrong side of the road keeps this far and this many seconds from one coming the
/// other way, and pulls back in when one is nearer.
const MEET_CLEAR: f32 = 40.0;
const MEET_TIME: f32 = 2.0;
/// A car in its own lane stops this far short of one that is out on its side of the road, metres.
const MEET_ROOM: f32 = 20.0;
/// A car goes round a standing vehicle it sees within this, metres, and stays out for this far past it.
const SWERVE_SIGHT: f32 = 25.0;
const SWERVE_PAST: f32 = 11.0;
/// The fastest a car goes round a standing vehicle, m/s.
const SWERVE_SPEED: f32 = 4.0;
/// A car stops this far short of a standing vehicle, metres: room to steer out round it.
const SWERVE_ROOM: f32 = 14.0;
/// The speed a car eases out past what is still in the way of where it is, m/s, and how near it
/// gets to it doing so, metres.
const EASE: f32 = 2.0;
const EASE_GAP: f32 = 1.5;
/// How far straight ahead of itself a physical car that is steering out looks, metres.
const STEERING_SIGHT: f32 = 8.0;
/// How long a physical car backs up when it has eased as near as that and is still not clear, seconds.
const BACK_UP: f32 = 1.0;
/// How fast a car moves sideways, m/s: this at a standstill, and this much more for every m/s it does.
const SIDEWAYS: [f32; 2] = [0.3, 0.3];
/// How fast it moves out to go round a standing vehicle, m/s.
const SWERVE_SIDEWAYS: f32 = 3.0;
/// After passing, a car does not pull out again for this long, seconds.
const PASS_REST: f32 = 3.0;
/// The share of its kind's top speed a car cruises at.
const TOP_SHARE: f32 = 0.95;
/// How much faster than traffic does a driver who flees takes a bend or a turn.
const FLEE_BENDS: f32 = 1.3;
/// How much faster than their pace a driver who hurries drives, and than traffic takes a bend.
const HURRY: f32 = 1.4;
const HURRY_BENDS: f32 = 1.15;
/// How near the car ahead a driver who flees drives, metres.
const FLEE_GAP: f32 = 1.0;
/// How much harder than a driver in their senses one in a panic turns the wheel.
const PANIC_STEER: f32 = 1.8;
/// A car on rails that finds a vehicle this near ahead of it in its path is let off them, metres:
/// what is about to hit it then moves it.
const STRUCK_WITHIN: f32 = 1.0;
/// A car going round a standing vehicle whose body is this far out of its lane goes on round it
/// whoever comes, metres.
const OUT_FOR_GOOD: f32 = 0.8;
/// A car going round a standing vehicle is back in its lane this far short of a junction, metres.
const SWERVE_END: f32 = 12.0;
/// How far ahead along its road a car looks for the next junction and for cars coming the other way, metres.
const OPEN_SIGHT: f32 = 700.0;
/// How much faster or slower than its road's speed a car likes to drive: each car has its own pace
/// between these.
const PACE: [f32; 2] = [0.75, 1.15];
/// The kinds traffic is made of, each as often as it is listed: the everyday cars of 1980 to 1995 twice
/// each, and the Bronco, the Manta, the van and the EPA once (VEHICLES-XXIII.md).
const KINDS: [u8; 22] = [
    KIND_SAAB_900, KIND_SAAB_9000, KIND_VOLVO_240, KIND_VOLVO_740, KIND_GTI, KIND_KADETT, KIND_OMEGA, KIND_SIERRA, KIND_ESCORT,
    KIND_SAAB_900, KIND_SAAB_9000, KIND_VOLVO_240, KIND_VOLVO_740, KIND_GTI, KIND_KADETT, KIND_OMEGA, KIND_SIERRA, KIND_ESCORT,
    KIND_JEEP, KIND_SPORTS, KIND_TRANSPORTER, KIND_EPA,
];

/// A shown car of the traffic. A component on the vehicle's entity.
#[derive(crate::entities::Component, Clone, Copy, Debug)]
pub struct TrafficCar;

/// A vehicle that was traffic until a player took it, its driver left it or it was wrecked, with
/// the ticks it has stood empty and lain wrecked, and the tick a car of the traffic last found it
/// in its path.
#[derive(crate::entities::Component, Clone, Copy, Debug, Default)]
pub struct Abandoned {
    empty: u32,
    wrecked: u32,
    in_way: Option<u32>,
}

/// One car of the traffic.
#[derive(Clone, Copy, Debug)]
struct Car {
    /// `KIND_*`.
    kind: u8,
    /// The way it is on, how far along it its middle is, metres, and its speed, m/s.
    way: Way,
    along: f32,
    speed: f32,
    /// The way it came by, and the one it goes on by: `None` where its way leaves the world.
    before: Option<Way>,
    after: Option<Way>,
    /// The junction it has been let across and is not yet clear of.
    holds: Option<u32>,
    /// Its vehicle, while it is shown.
    vid: Option<u32>,
    /// Its vehicle is off the rails.
    physical: bool,
    /// How far ahead of its nose what it last saw in its way is, metres; `f32::MAX` for nothing.
    blocked: f32,
    /// Ticks it has stood held up by that.
    held: u32,
    /// Ticks it has stood at a junction that is not yet its to cross.
    waiting: u32,
    /// Ticks it has stood, whatever for.
    stood: u32,
    /// How far the nearest player was when last measured, metres.
    nearest: f32,
    /// Physical: ticks it has stood though nothing holds it, ticks left of backing out, and how
    /// many times it has backed out.
    stalled: u32,
    reverse_left: u32,
    tries: u32,
    /// How fast it likes to drive, as a share of its road's speed.
    pace: f32,
    /// How far left of its lane it is, metres, and how far it means to be: out to pass, or 0.
    shift: f32,
    aside: f32,
    /// The car it is out to pass, or `None` when it is out to go round a standing vehicle, which it
    /// is past at `swerve_to` metres along its way.
    passing: Option<u32>,
    swerve_to: f32,
    /// How far along its way it is when its nose is up to the standing vehicle it goes round, metres.
    upon: f32,
    /// How far left of its lane its body really is, metres: `shift` on rails, and wherever its
    /// driver has got it to when it is physical.
    lateral: f32,
    /// What it saw is in the way of where it is but not of where it is steering to: it eases past.
    easing: bool,
    /// It has given the pass up, and drops back behind the car it was passing to pull in.
    back: bool,
    /// What it saw in its way is a vehicle standing still.
    still: bool,
    /// Ticks before it may pull out again.
    rest: u32,
    /// Its driver's entity, while a player is near enough to see one at the wheel.
    driver: Option<u32>,
    /// What its driver does when threatened, and what they are about now.
    nerve: Nerve,
    mood: Mood,
    /// Ticks a player's gun has been on it, and where what frightened its driver was, `[x, z]`.
    aimed: u32,
    threat: [f32; 2],
    /// Its vehicle's health when its driver last looked, and their own: less now is a hit.
    hp: u8,
    hurt: u8,
    /// Ticks it has stood while its driver flees.
    boxed: u32,
    /// Its driver loses their head when the car is hit, where another would do as their nerve has it.
    rash: bool,
    /// The vehicle it last saw in its way; 0 for none, or someone on foot.
    by: u32,
}

/// A vehicle that is not on rails: something a shown car may find in its way.
#[derive(Clone, Copy)]
struct Obstacle {
    vid: u32,
    pose: sim::CarPose,
    /// Half its width and half its length, metres.
    half: [f32; 2],
    /// It is a car of the traffic: one that stands is waiting for something, and is not gone round.
    traffic: bool,
}

/// How long a car takes to gain `gain` metres on another it is closing on at `closing` m/s, picking
/// up speed at `ACCEL` until it closes at `top`, seconds.
fn gaining(gain: f32, closing: f32, top: f32) -> f32 {
    let top = top.max(closing).max(0.5);
    let gathering = (top - closing) / ACCEL;
    let gathered = closing * gathering + 0.5 * ACCEL * gathering * gathering;
    if gathered >= gain { (-closing + (closing * closing + 2.0 * ACCEL * gain).sqrt()) / ACCEL } else { gathering + (gain - gathered) / top }
}

/// How many ticks `seconds` are.
fn ticks(seconds: f32) -> u32 {
    (seconds * TICK_RATE as f32) as u32
}

/// The speed that slows to `to` over `over` metres at `BRAKING`, m/s.
fn slowing(to: f32, over: f32) -> f32 {
    (to * to + 2.0 * BRAKING * over.max(0.0)).sqrt()
}

/// Half the length of a car of `kind`, metres.
fn half_length(kind: u8) -> f32 {
    vehicle_def(kind).spec.half_extents[2]
}

/// The yaw of a vehicle that drives along the unit direction `d`.
fn yaw_of(d: [f32; 3]) -> f32 {
    (-d[0]).atan2(-d[2])
}

/// Runs a world's traffic.
pub struct Traffic {
    graph: Graph,
    built: bool,
    cars: Vec<Car>,
    /// The cars on each way (`Way::index`), the furthest along first.
    lanes: Vec<VecDeque<u32>>,
    /// The way into each junction (`Way::index`) whose cars may cross it, the tick it became theirs
    /// and the tick it stops being.
    junctions: Vec<Option<(u32, u32, u32)>>,
    /// How many cars are crossing each junction.
    crossing: Vec<u16>,
    /// The edges a bridge carries that are not shut, each with the height of its roadway every
    /// `DECK_STEP` along it as it was when the network was made, and which of them is looked at next.
    decks: Vec<(u32, Vec<f32>)>,
    deck_at: usize,
    /// Where the players are, as of this tick.
    viewers: Vec<[f32; 3]>,
    /// The way each of them looks, radians, in the same order.
    looks: Vec<f32>,
    /// The vehicles the cars found standing in their paths this tick.
    in_way: Vec<u32>,
    /// Where it has got to in the noises the world makes: the blasts and shots its drivers hear.
    heard: MessageCursor<Noise>,
    /// The vehicles that are not on rails and everyone on foot, as of this tick: what a shown car
    /// may find in its way.
    obstacles: Vec<Obstacle>,
    people: Vec<[f32; 3]>,
    /// The tick a car that is out of its lane last had each way (`Way::index`) ahead of it as the
    /// lane of those who come towards it: only on such a way does a car look for who it meets.
    met: Vec<u32>,
    /// How many times a car has passed another.
    overtakes: u32,
    shown: usize,
    physical: usize,
    /// The world's tick, as of the last of its own.
    tick: u32,
    rng: Rng,
}

impl Default for Traffic {
    fn default() -> Self {
        // Its own dice, so that a world with traffic rolls the same for everything else as one without.
        Traffic { graph: Graph::default(), built: false, cars: Vec::new(), lanes: Vec::new(), junctions: Vec::new(), crossing: Vec::new(), decks: Vec::new(), deck_at: 0, viewers: Vec::new(), looks: Vec::new(), in_way: Vec::new(), heard: MessageCursor::default(), obstacles: Vec::new(), people: Vec::new(), met: Vec::new(), overtakes: 0, shown: 0, physical: 0, tick: 0, rng: Rng::from_seed(0x7a3f_19c4_55d1_e2b7) }
    }
}

/// The height of the roadway of the bridge over `(x, z)`, or of the ground where there is none.
/// The deck is looked for from below, up from the ground to its underside, and its top from just
/// over that: looked for from the sky, a stay cable, a pylon or a tree over the road would be found
/// first.
fn deck_at(world: &World, x: f32, z: f32) -> f32 {
    let ground = world.map.ground.height_at(x, z);
    let under = ground + DECK_FROM;
    let under = world.physics.cast_ray_static([x, under, z], [0.0, 1.0, 0.0], SURFACE_FROM).map_or(ground, |rise| under + rise);
    let from = under + DECK_OVER;
    world.physics.cast_ray_static([x, from, z], [0.0, -1.0, 0.0], from - ground).map_or(ground, |fall| from - fall)
}

/// The players traffic is real for: people, not bots, zombies or bus drivers.
fn viewers(world: &World) -> Vec<[f32; 3]> {
    world.entities.all::<Player>().filter(|&(id, _)| human(world, id)).map(|(_, player)| player.character.position).collect()
}

/// Whether vehicle `vid` is fit to drive on as traffic: it has half its health or more, and all
/// its wheels.
fn sound(world: &World, vid: u32) -> bool {
    let whole = world.entities.get::<Health>(vid).is_some_and(|health| u16::from(health.hp) * 2 >= u16::from(health.max));
    let wheels = world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle.damage[shared::protocol::DAMAGE_WHEELS..shared::protocol::DAMAGE_WHEELS + 4].iter().all(|&wheel| wheel != shared::protocol::WHEEL_GONE));
    whole && wheels
}

/// Whether player `id` is a person: not a bot, a zombie or a bus driver.
fn human(world: &World, id: u32) -> bool {
    !world.entities.has::<Bot>(id) && !world.entities.has::<super::BusDriver>(id) && !super::is_zombie(world, id)
}

impl Traffic {
    /// Builds the network the first time traffic is asked for, and brings the number of cars to
    /// what the world wants.
    fn lay_out(&mut self, world: &mut World) {
        if !self.built {
            self.built = true;
            let map = &world.map;
            let surface = |x: f32, z: f32, carried: bool| if carried { deck_at(world, x, z) } else { map.ground.height_at(x, z) };
            self.graph = Graph::build(&map.roads, |p| map.is_inside((p[0] * 100.0) as i32, (p[1] * 100.0) as i32), surface);
            self.lanes = vec![VecDeque::new(); self.graph.edges.len() * 2];
            self.met = vec![u32::MAX; self.graph.edges.len() * 2];
            self.junctions = vec![None; self.graph.nodes.len()];
            self.crossing = vec![0; self.graph.nodes.len()];
            self.decks = (0..self.graph.edges.len() as u32)
                .filter(|&edge| self.graph.edges[edge as usize].carried)
                .map(|index| {
                    let edge = &self.graph.edges[index as usize];
                    let steps = (edge.length() / DECK_STEP).ceil() as usize;
                    (index, (0..=steps).map(|step| edge.at(step as f32 * DECK_STEP).0).map(|p| deck_at(world, p[0], p[2])).collect())
                })
                .collect();
            tracing::info!(nodes = self.graph.nodes.len(), edges = self.graph.edges.len(), driven_km = self.graph.driven() / 1000.0, entries = self.graph.entries.len(), "road network laid out");
        }
        // A world with next to no road to drive has no traffic.
        let wanted = if self.graph.driven() < 200.0 { 0 } else { ((self.graph.driven() / 1000.0 * world.traffic).round() as usize).min(TRAFFIC_MAX) };
        while self.cars.len() > wanted {
            let i = self.cars.len() - 1;
            self.leave(i);
            self.unshow(i, world);
            self.cars.pop();
        }
        let before = self.cars.len();
        while self.cars.len() < wanted {
            let kind = KINDS[(self.rng.next_u64() % KINDS.len() as u64) as usize];
            let pace = PACE[0] + (PACE[1] - PACE[0]) * self.rng.next_f64() as f32;
            self.cars.push(Car { kind, way: Way { edge: 0, back: false }, along: 0.0, speed: 0.0, before: None, after: None, holds: None, vid: None, physical: false, blocked: f32::MAX, held: 0, waiting: 0, stood: 0, nearest: f32::MAX, stalled: 0, reverse_left: 0, tries: 0, pace, shift: 0.0, aside: 0.0, passing: None, swerve_to: 0.0, upon: 0.0, lateral: 0.0, easing: false, back: false, still: false, rest: 0, driver: None, nerve: Nerve::Flees, mood: Mood::Calm, aimed: 0, threat: [0.0; 2], hp: 0, hurt: 0, boxed: 0, rash: false, by: 0 });
            self.start(self.cars.len() - 1, false);
        }
        if self.cars.len() != before {
            tracing::info!(cars = self.cars.len(), "traffic on the roads");
        }
    }

    /// Where car `i` is and the unit direction it drives: on its lane, or as far left of it as it
    /// has moved out.
    fn pose(&self, i: usize) -> ([f32; 3], [f32; 3]) {
        self.pose_ahead(i, 0.0)
    }

    /// Where car `i` will be `ahead` metres further on, as far left of its lane as it is now.
    fn pose_ahead(&self, i: usize, ahead: f32) -> ([f32; 3], [f32; 3]) {
        self.pose_out(i, ahead, self.cars[i].shift)
    }

    /// Where car `i` would be `ahead` metres further on and `shift` metres left of its lane.
    fn pose_out(&self, i: usize, ahead: f32, shift: f32) -> ([f32; 3], [f32; 3]) {
        let car = &self.cars[i];
        let length = self.graph.length(car.way);
        let (p, d) = match car.after {
            Some(after) if car.along + ahead > length => self.graph.pose(Some(car.way), after, None, (car.along + ahead - length).min(self.graph.length(after))),
            _ => self.graph.pose(car.before, car.way, car.after, (car.along + ahead).min(length)),
        };
        // With the nose along -Z the right is +X: a quarter turn clockwise from the way it goes.
        let flat = d[0].hypot(d[2]).max(1e-6);
        ([p[0] + d[2] / flat * shift, p[1], p[2] - d[0] / flat * shift], d)
    }

    /// The way a car that came down `way` goes on by: any of the ways out, a main road and the way
    /// straight on more often than a side road and a sharp turn. `None` where `way` leaves the world.
    fn choose(&mut self, way: Way) -> Option<Way> {
        let (_, heading) = self.graph.pose(None, way, None, self.graph.length(way));
        let exits: Vec<(Way, f32)> = self
            .graph
            .exits(way)
            .map(|out| {
                let (_, d) = self.graph.pose(None, out, None, 0.0);
                let straight = (heading[0] * d[0] + heading[2] * d[2]).max(0.0);
                let class = if self.graph.edge(out).kind == ROAD_MAJOR { 3.0 } else { 1.0 };
                (out, class * (0.25 + straight))
            })
            .collect();
        let total: f32 = exits.iter().map(|&(_, weight)| weight).sum();
        let mut pick = self.rng.next_f64() as f32 * total;
        for &(out, weight) in &exits {
            pick -= weight;
            if pick <= 0.0 {
                return Some(out);
            }
        }
        exits.last().map(|&(out, _)| out)
    }

    /// Car `i` is done with the junction it was crossing. The junction stays its road's until the
    /// time it was given for is up, for the cars behind it.
    fn release(&mut self, i: usize) {
        if let Some(node) = self.cars[i].holds.take() {
            self.crossing[node as usize] = self.crossing[node as usize].saturating_sub(1);
        }
    }

    /// Takes car `i` off its lane and out of its junction.
    fn leave(&mut self, i: usize) {
        let way = self.cars[i].way;
        self.lanes[way.index()].retain(|&car| car != i as u32);
        self.release(i);
    }

    /// Puts car `i` on `way`, `along` metres down it, having come by `before`.
    fn enter(&mut self, i: usize, way: Way, along: f32, before: Option<Way>) {
        let after = self.choose(way);
        // Onto the next stretch of the same road (no other road meets it here) a car that is out
        // stays out; at a junction, and afresh, it is back to its own lane from wherever it was.
        let onward = before.is_some_and(|before| self.graph.nodes[self.graph.start(way) as usize].edges == 2 && before == self.cars[i].way);
        let gone = self.graph.length(self.cars[i].way);
        let car = &mut self.cars[i];
        (car.way, car.along, car.before, car.after) = (way, along, before, after);
        if onward {
            (car.swerve_to, car.upon) = (car.swerve_to - gone, car.upon - gone);
        } else {
            (car.aside, car.passing, car.back) = (0.0, None, false);
        }
        let lane = &self.lanes[way.index()];
        let at = lane.iter().position(|&other| self.cars[other as usize].along < along).unwrap_or(lane.len());
        self.lanes[way.index()].insert(at, i as u32);
    }

    /// Whether `along` metres down `way` is at least `START_CLEAR` from every car on that lane.
    fn clear(&self, way: Way, along: f32) -> bool {
        self.lanes[way.index()].iter().all(|&other| (self.cars[other as usize].along - along).abs() > START_CLEAR)
    }

    /// Starts car `i` afresh, off every lane as it is: coming into the world by one of its roads
    /// (`entering`) when there is room on one, else anywhere on the network that no player is near.
    fn start(&mut self, i: usize, entering: bool) {
        let mut found = None;
        if entering && !self.graph.entries.is_empty() {
            for _ in 0..8 {
                let way = self.graph.entries[(self.rng.next_u64() % self.graph.entries.len() as u64) as usize];
                let along = 1.0f32.min(self.graph.length(way) / 2.0);
                if self.clear(way, along) {
                    found = Some((way, along, self.graph.edge(way).speed * 0.8));
                    break;
                }
            }
        }
        for attempt in 0..40 {
            if found.is_some() {
                break;
            }
            let Some((way, along)) = self.graph.place(self.rng.next_f64() as f32 * self.graph.driven()) else { break };
            let (p, _) = self.graph.pose(None, way, None, along);
            // After enough tries a place in sight will have to do.
            let unseen = attempt >= 30 || self.viewers.iter().all(|v| (v[0] - p[0]).hypot(v[2] - p[2]) > HIDE_BEYOND);
            if unseen && self.clear(way, along) {
                found = Some((way, along, 0.0));
            }
        }
        let (way, along, speed) = found.unwrap_or((Way { edge: 0, back: false }, 0.0, 0.0));
        let car = &mut self.cars[i];
        (car.speed, car.blocked, car.held, car.waiting, car.stood, car.nearest, car.stalled, car.reverse_left, car.tries) = (speed, f32::MAX, 0, 0, 0, f32::MAX, 0, 0, 0);
        (car.shift, car.lateral, car.easing, car.still, car.rest) = (0.0, 0.0, false, false, 0);
        // Another driver, with a nerve of their own.
        let roll = self.rng.next_u64();
        (car.nerve, car.mood, car.aimed, car.boxed) = (Nerve::of(roll), Mood::Calm, 0, 0);
        // The same roll's next digit, so that one in two of each nerve is rash.
        (car.rash, car.by) = ((roll / 10).is_multiple_of(2), 0);
        self.enter(i, way, along, None);
        let car = &mut self.cars[i];
        car.speed = car.speed.min(self.graph.cap(way, along));
    }

    /// Takes car `i`'s vehicle out of the world, if it is shown.
    fn unshow(&mut self, i: usize, world: &mut World) {
        self.unman(i, world);
        let car = &mut self.cars[i];
        let Some(vid) = car.vid.take() else { return };
        self.shown -= 1;
        if std::mem::take(&mut car.physical) {
            self.physical -= 1;
        }
        remove_vehicle(world, vid);
    }

    /// Car `i` is gone from where it was, vehicle and all, and starts afresh elsewhere.
    fn reborn(&mut self, i: usize, world: &mut World, entering: bool) {
        self.leave(i);
        self.unshow(i, world);
        self.start(i, entering);
    }

    /// Car `i`'s vehicle is no longer traffic: a player has it, or it is a wreck. It stays where it
    /// is, its driver gets out of it, and the car starts afresh elsewhere.
    fn abandon(&mut self, i: usize, world: &mut World) {
        self.put_out(i, world);
        let car = &mut self.cars[i];
        if let Some(vid) = car.vid.take() {
            self.shown -= 1;
            if std::mem::take(&mut car.physical) {
                self.physical -= 1;
            } else {
                // Off its rails where it stands, for the physics to have.
                world.physics.set_car_rails(vid, false, [0.0; 3]);
            }
            world.entities.remove::<(TrafficCar, Autopilot)>(vid);
            world.entities.insert(vid, Abandoned::default());
        }
        self.leave(i);
        self.start(i, false);
    }

    /// The car nearest ahead of car `i` on its lane. The car it is out beside, passing, is not ahead
    /// of it, and nor is a car that is out while it is not: that one is in another lane.
    fn ahead_of(&self, i: usize) -> Option<u32> {
        let car = &self.cars[i];
        let beside = car.passing.filter(|_| car.aside > 0.0);
        self.lanes[car.way.index()]
            .iter()
            .copied()
            .filter(|&other| other != i as u32 && Some(other) != beside)
            .filter(|&other| {
                let other = &self.cars[other as usize];
                other.along > car.along && (car.aside > 0.0 || !(other.aside > 0.0 && other.shift > 1.0))
            })
            .min_by(|&a, &b| self.cars[a as usize].along.total_cmp(&self.cars[b as usize].along))
    }

    /// The gap from car `i`'s nose to the tail of the car ahead of it, on its own way or the next,
    /// metres, and that car's speed.
    fn leader(&self, i: usize) -> Option<(f32, f32)> {
        let car = &self.cars[i];
        let ahead = |other: &Car, along: f32| (along - half_length(other.kind) - car.along - half_length(car.kind), other.speed.max(0.0));
        if let Some(other) = self.ahead_of(i) {
            let other = &self.cars[other as usize];
            return Some(ahead(other, other.along));
        }
        let after = car.after?;
        let beside = car.passing.filter(|_| car.aside > 0.0);
        let other = &self.cars[*self.lanes[after.index()].iter().rev().find(|&&other| Some(other) != beside)? as usize];
        Some(ahead(other, self.graph.length(car.way) + other.along))
    }

    /// Whether car `i` may cross the junction at `node` to go on by `after`: it is nobody's or its
    /// own road's (and has not been for `GREEN_MAX`), there is room for it on the road beyond (or it has waited `PATIENCE` for some), and no car on
    /// a bigger road is about to arrive.
    fn may_cross(&self, i: usize, node: u32, after: Way, tick: u32) -> bool {
        let car = &self.cars[i];
        let mine = car.way.index() as u32;
        match self.junctions[node as usize] {
            Some((way, since, until)) if tick <= until => {
                if way != mine || tick.wrapping_sub(since) > ticks(GREEN_MAX) {
                    return false;
                }
            }
            // Another road's time is up, but its cars are still on the junction.
            Some((way, _, until)) if way != mine && self.crossing[node as usize] > 0 && tick <= until + ticks(CLEARING) => return false,
            _ => {}
        }
        let room = car.waiting > ticks(PATIENCE) || self.lanes[after.index()].back().is_none_or(|&last| {
            let last = &self.cars[last as usize];
            last.along - half_length(last.kind) > 2.0 * half_length(car.kind) + GAP
        });
        let rank = self.graph.edge(car.way).kind;
        let bigger = self.graph.nodes[node as usize].into.iter().filter(|&&way| way != car.way && self.graph.edge(way).kind < rank).any(|&way| {
            self.lanes[way.index()].front().is_some_and(|&first| {
                let first = &self.cars[first as usize];
                first.speed > 1.0 && (self.graph.length(way) - first.along) / first.speed < GIVE_WAY
            })
        });
        room && !bigger
    }

    /// How fast car `i` may go now, m/s: its road's speed, less for a bend or a turn ahead, for the
    /// car in front, for what it saw in its way, and for a junction that is not yet its to cross.
    fn limit(&mut self, i: usize, tick: u32, steps: u32) -> f32 {
        let car = self.cars[i];
        let left = self.graph.length(car.way) - car.along;
        // No faster than its kind goes: an EPA traktor holds everyone up.
        let top = vehicle_def(car.kind).spec.max_speed * TOP_SHARE;
        let mut limit = (self.graph.edge(car.way).speed * car.pace).min(self.graph.cap(car.way, car.along)).min(top);
        // A driver who has been threatened: stopping to give the car up, going for whoever it was
        // whatever is in the way, or off as fast as the car goes and the bends allow.
        let fleeing = matches!(car.mood, Mood::Fleeing { .. });
        // How near the car ahead it drives: a driver who flees sits on its tail, and one who
        // hurries keeps half the time behind it.
        let (keep, headway) = match car.mood {
            Mood::Fleeing { .. } => (FLEE_GAP, 0.0),
            Mood::Hurrying { .. } => (GAP, HEADWAY / 2.0),
            _ => (GAP, HEADWAY),
        };
        match car.mood {
            Mood::Calm => {}
            Mood::Hurrying { .. } => {
                // Faster than they would on the straight, and a little faster through a bend.
                let (cap, road) = (self.graph.cap(car.way, car.along), self.graph.edge(car.way).speed);
                let hurry = (road * car.pace * HURRY).min(top);
                limit = if cap > road * 0.95 { hurry } else { hurry.min(cap * HURRY_BENDS) };
            }
            // Down behind the wheel, standing on the brake.
            Mood::GivingUp | Mood::Ducking { .. } => return 0.0,
            // Out of their mind, or dead: nothing holds the car back.
            Mood::Panicking { .. } | Mood::Runaway { .. } => return top,
            // Slowly onto the verge, and no further.
            Mood::PullingOver { .. } if self.pulled_over(i) => return 0.0,
            Mood::PullingOver { .. } => limit = limit.min(PULL_OVER_SPEED),
            Mood::Ramming { .. } => return top,
            Mood::Fleeing { .. } => {
                // Flat out where the road is straight, and round a bend faster than traffic takes it.
                let (cap, road) = (self.graph.cap(car.way, car.along), self.graph.edge(car.way).speed);
                limit = if cap > road * 0.95 { top } else { top.min(cap * FLEE_BENDS) };
            }
        }
        if let Some((gap, speed)) = self.leader(i) {
            limit = limit.min(slowing(speed, gap - keep - headway * car.speed.max(0.0)));
            // Closer than it should ever be (it came onto the road beside the other): it falls back.
            if gap < GAP / 2.0 {
                limit = limit.min((speed - 1.5).max(0.0));
            }
        }
        // Dropping back behind the car it gave up passing.
        if let Some(other) = car.passing.filter(|_| car.back) {
            limit = limit.min((self.cars[other as usize].speed - 3.0).max(0.0));
        }
        // Round a standing vehicle it goes slowly.
        if car.passing.is_none() && car.aside > 0.0 {
            limit = limit.min(SWERVE_SPEED);
        }
        // One of two cars that meet is on the other's side of the road: each stops in its half of
        // what is between them, but for one that is out going round a standing vehicle, which goes on.
        let met = self.met[car.way.index()] == tick;
        if let Some((gap, speed, Some(round))) = Some(()).filter(|()| met && car.shift < 0.5).and_then(|()| self.oncoming(i)) {
            // A car coming the other way is out on this one's side of the road. One that is going
            // round a standing vehicle is waited for. One that is passing has seen to it that it
            // will be back in time, and is braked for only if they are about to meet all the same.
            if round || gap < MEET_CLEAR + MEET_TIME * (speed + car.speed) {
                limit = limit.min(slowing(0.0, (gap - MEET_ROOM) / 2.0));
            }
        }
        // Calling a pass off, with someone coming: it stops in its half of what is between them.
        if let Some((gap, ..)) = Some(()).filter(|()| car.back && car.shift > 0.5).and_then(|()| self.oncoming(i)) {
            limit = limit.min(slowing(0.0, gap / 2.0 - GAP));
        }
        if car.blocked < f32::MAX {
            let stop = slowing(0.0, car.blocked - if car.still && car.aside == 0.0 { SWERVE_ROOM } else { GAP });
            limit = limit.min(if car.easing && car.blocked > EASE_GAP {
                stop.max(EASE)
            } else {
                stop
            });
        }
        // A road that leads nowhere any more (its bridge is down) is driven to its end and no further.
        let Some(after) = car.after else { return if self.graph.live[car.way.index()] { limit } else { limit.min(slowing(0.0, left - STOP_BACK)) } };
        limit = limit.min(slowing(self.graph.turn_speed(car.way, after) * if fleeing { FLEE_BENDS } else { 1.0 }, left - graph::TURN_REACH));
        let node = self.graph.end(car.way);
        if self.graph.nodes[node as usize].edges >= 3 && car.holds != Some(node) {
            let stop = left - STOP_BACK;
            let asking = stop < car.speed * car.speed / (2.0 * BRAKING) + ASK_AHEAD;
            if asking && self.may_cross(i, node, after, tick) {
                // Its road's for as long as it takes this car to get across, and no less than it was.
                let until = tick + ticks((left + CLEAR_AFTER) / car.speed.max(CROSSING_SPEED) + CROSSING_SPARE);
                self.junctions[node as usize] = Some(match self.junctions[node as usize] {
                    Some((way, since, known)) if tick <= known && way == car.way.index() as u32 => (way, since, known.max(until)),
                    _ => (car.way.index() as u32, tick, until),
                });
                self.release(i);
                self.crossing[node as usize] += 1;
                (self.cars[i].holds, self.cars[i].waiting) = (Some(node), 0);
            } else if fleeing {
                // It waits for nobody: across it goes, whoever's the junction is.
                if asking {
                    self.release(i);
                    self.crossing[node as usize] += 1;
                    self.cars[i].holds = Some(node);
                }
            } else {
                limit = limit.min(slowing(0.0, stop));
                if car.speed < 0.5 {
                    self.cars[i].waiting += steps;
                }
            }
        }
        limit
    }

    /// Moves car `i` on to the next way where it has run off the end of its own, and gives up its
    /// junction once it is clear of it. False when its way left the world and it started afresh.
    fn settle(&mut self, i: usize, world: &mut World) -> bool {
        loop {
            let car = self.cars[i];
            let length = self.graph.length(car.way);
            if car.along < length {
                break;
            }
            let Some(next) = car.after else {
                self.reborn(i, world, true);
                return false;
            };
            self.lanes[car.way.index()].retain(|&other| other != i as u32);
            self.enter(i, next, (car.along - length).min(self.graph.length(next) - 0.01), Some(car.way));
        }
        let car = self.cars[i];
        if let Some(node) = car.holds {
            let behind = node == self.graph.start(car.way) && car.along > CLEAR_AFTER.min(self.graph.length(car.way) / 2.0);
            if behind || (node != self.graph.start(car.way) && node != self.graph.end(car.way)) {
                self.release(i);
            }
        }
        true
    }

    /// Where the body of car `i` belongs on its rails: its chassis centre and its rotation.
    fn body(&self, i: usize) -> ([f32; 3], [f32; 4]) {
        let (p, d) = self.pose(i);
        let (yaw, pitch) = (yaw_of(d), d[1].clamp(-1.0, 1.0).asin());
        let ((sy, cy), (sx, cx)) = ((yaw / 2.0).sin_cos(), (pitch / 2.0).sin_cos());
        ([p[0], p[1] + Physics::car_ride_height(self.cars[i].kind), p[2]], [cy * sx, sy * cx, -sy * sx, cy * cx])
    }

    /// `steps` ticks of a car on rails, taken as one.
    fn roll(&mut self, i: usize, world: &mut World, dt: f32, steps: u32) {
        let dt = dt * steps as f32;
        self.consider(i, dt, steps);
        let limit = self.limit(i, world.tick, steps);
        let car = &mut self.cars[i];
        car.speed = (car.speed + (limit - car.speed).clamp(-HARD_BRAKE * dt, ACCEL * dt)).max(0.0);
        car.along += car.speed * dt;
        car.stood = if car.speed < 0.5 { car.stood + steps } else { 0 };
        if car.stood > ticks(STUCK) && car.vid.is_none() {
            self.reborn(i, world, false);
            return;
        }
        if !self.settle(i, world) {
            return;
        }
        if let Some(vid) = self.cars[i].vid {
            let (position, rotation) = self.body(i);
            world.physics.place_car(vid, position, rotation);
        }
    }

    /// One tick of a physical car: where it is on the network, and what its driver asks of it.
    fn drive(&mut self, i: usize, world: &mut World) {
        let car = self.cars[i];
        let Some(vid) = car.vid else { return };
        // Taken by a player, or wrecked: no longer traffic.
        let taken = world.entities.get::<Seats>(vid).is_none_or(|seats| seats.rider(0).is_some());
        let pose = world.physics.car_pose(vid).filter(|_| !taken && !world.entities.dead(vid));
        let Some(pose) = pose else {
            self.abandon(i, world);
            return;
        };
        let [x, _, z] = pose.position;
        let (mut along, mut off) = self.graph.nearest(car.way, [x, z], car.along - 15.0, car.along + 30.0);
        let length = self.graph.length(car.way);
        if along >= length - 0.5 {
            let Some(next) = car.after else {
                // Out of the world by the road it was on.
                self.reborn(i, world, true);
                return;
            };
            self.lanes[car.way.index()].retain(|&other| other != i as u32);
            (along, off) = self.graph.nearest(next, [x, z], 0.0, 30.0);
            self.enter(i, next, along, Some(car.way));
        }
        let (speed, held) = (pose.forward_speed, self.cars[i].held);
        (self.cars[i].along, self.cars[i].speed) = (along, speed);
        self.cars[i].stood = if speed.abs() < 0.5 { car.stood + 1 } else { 0 };
        // How far left of its lane it really is.
        let (lane, way) = self.pose_out(i, 0.0, 0.0);
        let flat = way[0].hypot(way[2]).max(1e-6);
        self.cars[i].lateral = ((x - lane[0]) * way[2] - (z - lane[2]) * way[0]) / flat;
        self.settle(i, world);
        self.consider(i, 1.0 / TICK_RATE as f32, 1);
        let unseen = car.nearest > UNSEEN;
        // The one its driver is driving at, wherever off the road that takes it.
        let ramming = match car.mood {
            Mood::Ramming { at, .. } => self.target(world, at),
            _ => None,
        };
        // Fallen out of the world: gone, to start afresh.
        if pose.position[1] < KILL_Y + 5.0 {
            self.reborn(i, world, false);
            return;
        }
        // A driver in a panic, or a dead one, takes the car wherever it goes.
        let wild = matches!(car.mood, Mood::Panicking { .. } | Mood::Runaway { .. });
        if let Mood::Runaway { steer, throttle, .. } = car.mood {
            if pose.up_y < 0.3 {
                self.abandon(i, world);
            } else if let Some(mut autopilot) = world.entities.get_mut::<Autopilot>(vid) {
                autopilot.0 = CarControls { throttle, steer, powered: true, ..CarControls::IDLE };
            }
            return;
        }
        if (off > LOST && ramming.is_none() && !wild) || pose.up_y < 0.3 || (car.tries >= TRIES && !wild) {
            // Off the road, on its roof or stuck for good: gone if nobody is there to see, and
            // else left where it is.
            if unseen { self.reborn(i, world, false) } else { self.abandon(i, world) }
            return;
        }
        let limit = self.limit(i, world.tick, 1);

        // Steering: at a point ahead on its lane, turning as the arc through it asks.
        let spec = &vehicle_def(car.kind).spec;
        let ahead = (LOOK_AHEAD[0] + LOOK_PER_SPEED * speed.max(0.0)).min(LOOK_AHEAD[1]);
        let goal = match car.mood {
            // All over the road: for a point thrown to one side of its lane or the other.
            Mood::Panicking { swerve, .. } => self.pose_out(i, ahead, car.shift + swerve).0,
            _ => ramming.unwrap_or(self.pose_ahead(i, ahead).0),
        };
        let reach = (goal[0] - x).hypot(goal[2] - z).max(1.0);
        let bearing = wrap_angle((-(goal[0] - x)).atan2(-(goal[2] - z)) - pose.yaw());
        let wheelbase = spec.axles.iter().copied().fold(f32::MIN, f32::max) - spec.axles.iter().copied().fold(f32::MAX, f32::min);
        let lock = (2.0 * wheelbase * bearing.sin() / reach).atan();
        // A positive steer turns the hull clockwise seen from above, which takes its yaw down.
        let steer = (-lock / spec.steer_lock.max(0.1) * if wild { PANIC_STEER } else { 1.0 }).clamp(-1.0, 1.0);

        let car = &mut self.cars[i];
        let controls = |throttle: f32, steer: f32, handbrake: bool| CarControls { throttle, steer, handbrake, powered: true, ..CarControls::IDLE };
        // Steering out round something it has come too near to clear: it backs up a little for another go.
        if car.easing && car.blocked <= EASE_GAP && speed.abs() < 0.3 && car.reverse_left == 0 {
            car.reverse_left = ticks(BACK_UP);
        }
        let asked = if wild {
            // Flat out, braking for nothing.
            controls(1.0, steer, false)
        } else if car.reverse_left > 0 {
            // Backing out of where it stuck.
            car.reverse_left -= 1;
            controls(if car.easing { -0.5 } else { -1.0 }, -steer, false)
        } else {
            // Stuck: standing though nothing it waits for holds it.
            car.stalled = if limit > 1.0 && speed.abs() < 0.3 && held == 0 { car.stalled + 1 } else { 0 };
            if car.stalled > ticks(STALL) {
                (car.stalled, car.tries, car.reverse_left) = (0, car.tries + 1, ticks(REVERSE));
            } else if speed > 3.0 {
                car.tries = 0;
            }
            // The throttle for that speed: on for more, against the way it rolls to brake for less.
            let over = speed - limit;
            if limit < 0.3 && speed < 0.6 {
                controls(0.0, steer, true)
            } else {
                let throttle = if over < 0.0 { (-over * 0.6).min(1.0) } else if speed > 1.2 { (-over * 0.5).max(-1.0) } else { 0.0 };
                controls(throttle, steer, over > 3.0)
            }
        };
        if let Some(mut autopilot) = world.entities.get_mut::<Autopilot>(vid) {
            autopilot.0 = asked;
        }
    }

    /// How far ahead of car `i`'s nose the first thing in its way is, were it `shift` metres left of
    /// its lane, and whether that is a vehicle standing still that is not traffic: a vehicle that is
    /// not on rails, or someone on foot, on the path the car will drive, as far as it needs to stop
    /// in. `f32::MAX` for nothing. A car on rails is not looked for: the car behind it keeps its
    /// distance by the network.
    fn sight(&self, i: usize, shift: f32) -> (f32, bool, u32) {
        let speed = self.cars[i].speed;
        let range = (speed * speed / (2.0 * FIRM_BRAKE) + 10.0).clamp(12.0, SIGHT_MAX);
        self.sight_along(i, range, SIGHT_STEP, |ahead| self.pose_out(i, ahead, shift).0)
    }

    /// `sight`, straight ahead of where the body of physical car `i` is and points: what it would
    /// run into as it steers, for a few metres.
    fn sight_ahead(&self, i: usize) -> (f32, bool, u32) {
        let Some(own) = self.obstacles.iter().find(|other| Some(other.vid) == self.cars[i].vid) else { return (f32::MAX, false, 0) };
        self.sight_along(i, STEERING_SIGHT, 1.0, |ahead| own.pose.world_point([0.0, 0.0, -ahead]))
    }

    /// The first of the points `at(ahead)`, from car `i`'s nose to `range` metres past it every
    /// `step`, that is in someone on foot or in a vehicle that is not on rails: how far ahead of the
    /// nose, and whether it is a vehicle standing still that is not traffic.
    fn sight_along(&self, i: usize, range: f32, step: f32, at: impl Fn(f32) -> [f32; 3]) -> (f32, bool, u32) {
        let car = &self.cars[i];
        let [hx, _, hz] = vehicle_def(car.kind).spec.half_extents;
        let here = at(0.0);
        let near = |p: [f32; 3]| (p[0] - here[0]).hypot(p[2] - here[2]) < range + 16.0 && (p[1] - here[1]).abs() < 3.0;
        let vehicles: Vec<&Obstacle> = self.obstacles.iter().filter(|other| Some(other.vid) != car.vid && near(other.pose.position)).collect();
        let people: Vec<&[f32; 3]> = self.people.iter().filter(|&&person| near(person)).collect();
        if vehicles.is_empty() && people.is_empty() {
            return (f32::MAX, false, 0);
        }
        let mut ahead = hz + 0.5;
        while ahead <= hz + range {
            let p = at(ahead);
            if people.iter().any(|person| (person[0] - p[0]).hypot(person[2] - p[2]) < hx + 0.7) {
                return (ahead - hz, false, 0);
            }
            for other in &vehicles {
                // The point of the path in the other vehicle's own frame, against its box widened by this car's.
                let [x, _, z] = other.pose.local_dir([p[0] - other.pose.position[0], 0.0, p[2] - other.pose.position[2]]);
                if x.abs() < other.half[0] + hx + 0.25 && z.abs() < other.half[1] + 0.5 {
                    return (ahead - hz, !other.traffic && other.pose.forward_speed.abs() < 0.5, other.vid);
                }
            }
            ahead += step;
        }
        (f32::MAX, false, 0)
    }

    /// What shown car `i` sees in its way, on the path it means to drive.
    fn look(&mut self, i: usize) {
        let car = self.cars[i];
        let to = if car.aside > 0.0 { car.aside } else { car.shift };
        let (mut blocked, mut still, mut by) = self.sight(i, to);
        // Not yet where it is steering to: what is ahead of where its body is counts too.
        let mut easing = false;
        if (car.lateral - to).abs() > 0.6 {
            let here = if car.physical { self.sight_ahead(i) } else { self.sight(i, car.lateral) };
            if here.0 < blocked {
                // Only round a standing vehicle does it ease past what it has not yet cleared.
                (blocked, still, by, easing) = (here.0, here.1, here.2, car.aside > 0.0 && car.passing.is_none() && here.1);
            }
        }
        // A vehicle standing in its path is in the traffic's way, gone round or not.
        if still && blocked < SWERVE_SIGHT {
            self.in_way.push(by);
        }
        let car = &mut self.cars[i];
        (car.blocked, car.still, car.easing, car.by) = (blocked, still, easing, by);
        if blocked == f32::MAX {
            car.held = 0;
        }
    }

    /// The stretches of road ahead of car `i` up to the next junction (where a third road meets
    /// it) or the road's end, as far as `OPEN_SIGHT`: each way with how far ahead of the car it starts.
    fn road_ahead(&self, i: usize) -> impl Iterator<Item = (Way, f32)> + '_ {
        let car = &self.cars[i];
        let mut next = Some((car.way, -car.along, car.after));
        std::iter::from_fn(move || {
            let (way, from, after) = next?;
            let end = from + self.graph.length(way);
            next = after.filter(|_| end < OPEN_SIGHT && self.graph.nodes[self.graph.end(way) as usize].edges == 2).map(|after| {
                // Beyond the stretch it has chosen it goes on by the only way there is, if there is but one.
                let mut exits = self.graph.exits(after);
                let only = exits.next().filter(|_| exits.next().is_none());
                (after, end, only)
            });
            Some((way, from))
        })
    }

    /// How far ahead of car `i` the next junction or the end of its road is, metres.
    fn open_road(&self, i: usize) -> f32 {
        self.road_ahead(i).last().map_or(0.0, |(way, from)| from + self.graph.length(way))
    }

    /// The nearest car coming the other way on car `i`'s road this side of the next junction: how
    /// far off, its speed, and whether it is out of its lane, and if so whether that is to go round
    /// a standing vehicle (`Some(true)`) or to pass (`Some(false)`).
    fn oncoming(&self, i: usize) -> Option<(f32, f32, Option<bool>)> {
        let car = &self.cars[i];
        self.road_ahead(i)
            .flat_map(|(way, from)| {
                let length = self.graph.length(way);
                self.lanes[way.reversed().index()].iter().map(move |&other| (&self.cars[other as usize], from + length))
            })
            .map(|(other, end)| (end - other.along - half_length(other.kind) - half_length(car.kind), other.speed.max(0.0), (other.shift > 0.5).then_some(other.passing.is_none() && other.aside > 0.0)))
            .filter(|&(gap, ..)| gap > -4.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// How far ahead of car `i` car `other` is along the road, metres (behind it when negative), if
    /// it is on the same stretch, the next or the last.
    fn ahead_by(&self, i: usize, other: u32) -> Option<f32> {
        let (car, other) = (&self.cars[i], &self.cars[other as usize]);
        if other.way == car.way {
            Some(other.along - car.along)
        } else if Some(other.way) == car.after {
            Some(self.graph.length(car.way) - car.along + other.along)
        } else if Some(other.way) == car.before {
            Some(other.along - self.graph.length(other.way) - car.along)
        } else {
            None
        }
    }

    /// How far right of its lane car `i` stands when it has pulled over, metres: its side `VERGE`
    /// past the edge of the road.
    fn verge(&self, i: usize) -> f32 {
        let car = &self.cars[i];
        self.graph.edge(car.way).width / 2.0 - self.graph.offset(car.way) + vehicle_def(car.kind).spec.half_extents[0] + VERGE
    }

    /// Whether car `i` has pulled over as far as it means to.
    fn pulled_over(&self, i: usize) -> bool {
        (self.cars[i].lateral + self.verge(i)).abs() < 0.8
    }

    /// Whether a player is watching the spot `p`: near it and looking its way, or right beside it.
    fn watched(&self, p: [f32; 3]) -> bool {
        self.viewers.iter().zip(&self.looks).any(|(v, &yaw)| {
            let to = [p[0] - v[0], p[2] - v[2]];
            let reach = to[0].hypot(to[1]);
            reach < WATCH_NEAR || (reach < WATCH_WITHIN && to[0] * -yaw.sin() + to[1] * -yaw.cos() > reach * WATCH_HALF_CONE)
        })
    }

    /// Whether car `i` pulls out, to pass a slower car or go round a standing vehicle, or comes
    /// back in; and moves it sideways towards where it means to be.
    fn consider(&mut self, i: usize, dt: f32, steps: u32) {
        // Pulling over, it has nobody to pass: it makes for the verge on its right.
        let pulling = matches!(self.cars[i].mood, Mood::PullingOver { .. });
        let verge = self.verge(i);
        if pulling {
            let car = &mut self.cars[i];
            (car.aside, car.passing, car.back) = (0.0, None, false);
        } else {
            self.decide(i, steps);
        }
        let car = &mut self.cars[i];
        // Round a standing vehicle it is from close by and slowly: it steers out at once.
        let rate = if car.passing.is_none() && car.aside > 0.0 { SWERVE_SIDEWAYS * dt } else { (SIDEWAYS[0] + SIDEWAYS[1] * car.speed.max(0.0)) * dt };
        let to = if pulling { -verge } else { car.aside };
        car.shift += (to - car.shift).clamp(-rate, rate);
        if !car.physical {
            car.lateral = car.shift;
        }
    }

    /// Whether car `i` pulls out or comes back in (`Car::aside`): out to pass a slower car where the
    /// road ahead is straight and clear of oncoming cars for as long as the pass takes, or to go
    /// round a vehicle standing in its lane; back in once it is past, or when it has to give it up.
    fn decide(&mut self, i: usize, steps: u32) {
        let car = self.cars[i];
        let edge = self.graph.edge(car.way);
        let (ours, theirs) = (edge.lanes[usize::from(car.way.back)], edge.lanes[usize::from(!car.way.back)]);
        let (width, speed, no_passing) = (edge.width, edge.speed, edge.no_passing);
        // What it would drive at with nothing ahead: its pace, and no more than its kind goes.
        let top = vehicle_def(car.kind).spec.max_speed * TOP_SHARE;
        // How much it cares who is coming and what the road allows: a driver who flees not at
        // all, and one who hurries half as much as they should.
        let (desired, care) = match car.mood {
            Mood::Fleeing { .. } => (top, 0.0),
            Mood::Hurrying { .. } => ((speed * car.pace * HURRY).min(top), 0.5),
            _ => ((speed * car.pace).min(top), 1.0),
        };
        let reckless = care == 0.0;
        let from_junction = self.graph.nodes[self.graph.start(car.way) as usize].edges != 2;
        // Out is the next lane its own way where the road has one, and else the other side of the road.
        let across = ours < 2;
        if car.aside > 0.0 {
            // Past the car it passes, or behind it again having given up; or past the standing vehicle.
            let (past, behind) = match car.passing {
                Some(other) => match self.ahead_by(i, other) {
                    Some(ahead) => {
                        let clear = half_length(self.cars[other as usize].kind) + half_length(car.kind) + GAP;
                        (ahead < -clear - car.speed, ahead > clear - 1.0)
                    }
                    // It has turned off, or been left far behind.
                    None => (true, false),
                },
                None => (car.along > car.swerve_to, false),
            };
            // A car that is going round a standing vehicle goes on round it: it cannot come back in
            // beside the thing, and whoever comes the other way waits for it.
            let meeting = across
                && match car.passing {
                    // Someone is coming who would be there before it is past and back in its lane.
                    Some(other) => {
                        let clear = half_length(self.cars[other as usize].kind) + half_length(car.kind) + GAP;
                        let to_gain = self.ahead_by(i, other).map_or(0.0, |ahead| (ahead + clear + car.speed).max(0.0));
                        let slower = self.cars[other as usize].speed;
                        let time = gaining(to_gain, (car.speed - slower).max(0.0), desired - slower) + PULL_IN;
                        self.oncoming(i).is_some_and(|(gap, speed, _)| gap < (speed + car.speed) * time + MEET_ROOM)
                    }
                    // Not yet up to the thing nor out of its lane, it stays in behind it for a car
                    // that would be there before it was round. Once its body is out it goes on: back
                    // in it could not get, and it would stand across the other lane for good.
                    None => {
                        let round = car.swerve_to - car.along;
                        car.along < car.upon - 2.0 && car.lateral < OUT_FOR_GOOD && self.oncoming(i).is_some_and(|(gap, speed, _)| gap < round + speed * (round / SWERVE_SPEED + PULL_IN) + MEET_ROOM)
                    }
                };
            // How far it is to the next junction: it is back in its lane by then.
            let give_up = !reckless && !past && (meeting || self.open_road(i) < if car.passing.is_some() { PASS_END } else { SWERVE_END });
            if give_up && car.passing.is_some() && !behind {
                // Beside the other car with no time to get by: it drops back to pull in behind it.
                self.cars[i].back = true;
            } else if past || give_up {
                if past && car.passing.is_some() {
                    self.overtakes += 1;
                }
                let car = &mut self.cars[i];
                (car.aside, car.passing, car.back, car.rest) = (0.0, None, false, ticks(PASS_REST));
                // It is wherever it has got to, which is past the car that was ahead.
                let lane = &mut self.lanes[car.way.index()];
                let cars = &self.cars;
                lane.make_contiguous().sort_by(|&a, &b| cars[b as usize].along.total_cmp(&cars[a as usize].along));
            }
            return;
        }
        if car.rest > 0 {
            self.cars[i].rest = car.rest.saturating_sub(steps);
            return;
        }
        let out = (width / f32::from((ours + theirs).max(1))).clamp(OUT[0], OUT[1]);
        // Not near either end of its road, and not where being out would be off the asphalt.
        if car.shift > 0.05 || (from_junction && car.along < 8.0) || (self.graph.offset(car.way) - out).abs() > width / 2.0 - 1.0 {
            return;
        }
        let (need, passing) = if car.blocked < SWERVE_SIGHT && car.still {
            // A vehicle standing in its lane: round it, if the way round is clear.
            let need = car.blocked + SWERVE_PAST;
            if self.sight(i, out).0 < need {
                return;
            }
            (need, None)
        } else {
            // A slower car ahead on its lane, with room ahead of it to come back into.
            let Some(index) = self.ahead_of(i) else { return };
            let other = &self.cars[index as usize];
            let gap = other.along - car.along - half_length(other.kind) - half_length(car.kind);
            let room = self.ahead_of(index as usize).is_none_or(|next| self.cars[next as usize].along - other.along > PASS_ROOM);
            let straight = |along: f32| self.graph.cap(car.way, along) > speed * 0.95;
            // A driver who flees passes whatever is slower, a standing queue too, wherever they are.
            let faster = if care < 1.0 { 1.0 } else { PASS_FASTER };
            if other.aside > 0.0 || other.shift > 0.05 || gap > PASS_FROM + car.speed || other.speed > desired - faster {
                return;
            }
            if !reckless && (other.speed < 1.0 || !room || (across && no_passing)) {
                return;
            }
            // How long it takes to gain the length of the pass on it, picking up speed as it goes,
            // and how far it drives meanwhile.
            // It comes back in a second's drive ahead of it.
            let (gain, closing, top) = (gap + 2.0 * half_length(other.kind) + 2.0 * GAP + desired, (car.speed - other.speed).max(0.0), desired - other.speed);
            // It gains on it faster and faster until it is at its own pace, and steadily from then.
            let need = other.speed * gaining(gain, closing, top) + gain;
            if !reckless && (!straight(car.along) || !straight(car.along + need)) {
                return;
            }
            (need, Some(index))
        };
        // How far it is to the next junction: it is back in its lane by then.
        let open = self.open_road(i);
        if !reckless && open - need < if passing.is_some() { PASS_END + 10.0 } else { SWERVE_END } {
            return;
        }
        if reckless {
            // Out it goes, whoever is coming.
        } else if across && passing.is_some() {
            // Nobody coming the other way within what it takes and what they cover meanwhile, nor
            // able to come onto the road at the next junction and get that near.
            let time = need / car.speed.max(SWERVE_SPEED) + PULL_IN;
            let fastest = speed * PACE[1];
            if open < need + (fastest * time + MEET_CLEAR) * care || self.oncoming(i).is_some_and(|(gap, speed, _)| gap < need + (speed.max(fastest) * time + MEET_CLEAR) * care) {
                return;
            }
        } else if across {
            // Round a standing vehicle it goes slowly, and whoever comes the other way waits for it
            // (`MEET_ROOM`): nobody so near that they could not stop.
            if self.oncoming(i).is_some_and(|(gap, speed, _)| gap < need + MEET_ROOM + speed * speed / (2.0 * FIRM_BRAKE) + speed) {
                return;
            }
        }
        let car = &mut self.cars[i];
        (car.aside, car.passing, car.swerve_to, car.upon) = (out, passing, car.along + need, car.along + car.blocked);
    }

    /// Looks at one bridge's deck, and shuts its road if the deck is broken: somewhere along it the
    /// roadway is `DECK_DROP` below where it was.
    fn inspect(&mut self, world: &mut World) {
        if self.decks.is_empty() {
            return;
        }
        self.deck_at = (self.deck_at + 1) % self.decks.len();
        let (index, was) = &self.decks[self.deck_at];
        let (index, edge) = (*index, &self.graph.edges[*index as usize]);
        let broken = was.iter().enumerate().any(|(step, &was)| {
            let (p, _) = edge.at(step as f32 * DECK_STEP);
            deck_at(world, p[0], p[2]) < was - DECK_DROP
        });
        if broken {
            self.shut(index, world);
        }
    }

    /// The bridge that carries edge `edge` is down: nothing drives it any more, nor the roads that
    /// only led to it. A car on rails that nobody sees starts afresh elsewhere; one that is shown
    /// is left to the physics if it is on the bridge, and else drives to the end of its road and stands.
    fn shut(&mut self, edge: u32, world: &mut World) {
        self.graph.shut(edge);
        self.decks.retain(|&(deck, _)| deck != edge);
        tracing::info!(edge, driven_km = self.graph.driven() / 1000.0, "a bridge is down: its road is shut");
        for i in 0..self.cars.len() {
            let car = self.cars[i];
            if self.graph.live[car.way.index()] {
                if car.after.is_some_and(|after| !self.graph.live[after.index()]) {
                    self.cars[i].after = self.choose(car.way);
                }
            } else if car.vid.is_none() {
                self.reborn(i, world, false);
            } else if car.way.edge == edge {
                self.abandon(i, world);
            } else {
                self.cars[i].after = None;
            }
        }
    }

    /// Lets shown car `i` off its rails at the speed it has: it is physical from now on.
    fn let_go(&mut self, i: usize, world: &mut World) {
        let car = self.cars[i];
        let Some(vid) = car.vid.filter(|_| !car.physical) else { return };
        let (_, d) = self.pose(i);
        world.physics.set_car_rails(vid, false, d.map(|v| v * car.speed));
        let car = &mut self.cars[i];
        (car.physical, car.stalled, car.reverse_left, car.tries) = (true, 0, 0, 0);
        self.physical += 1;
    }

    /// Measures how near the players car `i` is, and shows, hides, lets go or puts back on rails
    /// its vehicle by that.
    fn rank(&mut self, i: usize, world: &mut World) {
        let (p, d) = self.pose(i);
        let nearest = self.viewers.iter().map(|v| (v[0] - p[0]).hypot(v[2] - p[2])).fold(f32::MAX, f32::min);
        self.cars[i].nearest = nearest;
        let car = self.cars[i];
        let Some(vid) = car.vid else {
            if nearest < SHOW_WITHIN && self.shown < SHOWN_MAX && world.entities.count::<Vehicle>() < MAX_VEHICLES {
                let (position, rotation) = self.body(i);
                let vid = add_vehicle(world, car.kind, position, yaw_of(d));
                world.physics.set_car_rails(vid, true, [0.0; 3]);
                world.physics.place_car(vid, position, rotation);
                world.entities.insert(vid, (TrafficCar, Autopilot(CarControls { handbrake: true, ..CarControls::IDLE })));
                self.cars[i].vid = Some(vid);
                self.shown += 1;
            }
            return;
        };
        if nearest > HIDE_BEYOND {
            self.unshow(i, world);
            return;
        }
        self.man(i, world);
        // A player has dragged its driver out and taken the wheel, or it is a wreck.
        if world.entities.dead(vid) || world.entities.get::<Seats>(vid).is_some_and(|seats| seats.rider(0).is_some()) {
            self.abandon(i, world);
            return;
        }
        // Stood too long, whatever for: gone to start afresh if nobody is looking, and else its
        // driver gives it up where it is in the end.
        if car.stood > ticks(STOOD_WATCHED) {
            self.abandon(i, world);
            return;
        }
        if car.stood > ticks(STOOD_UNWATCHED) && !self.watched(p) {
            self.reborn(i, world, false);
            return;
        }
        let car = self.cars[i];
        let sound = sound(world, vid);
        // A driver who only hurries drives as the rails would have it; any other mood needs the car.
        let steady = matches!(car.mood, Mood::Calm | Mood::Hurrying { .. });
        if !car.physical {
            let hurt = !sound;
            let taken = world.entities.get::<Seats>(vid).is_some_and(|seats| !seats.vacant());
            if (nearest < PHYSICAL_WITHIN || hurt || taken || car.held > ticks(HELD_RAILS) || !steady) && self.physical < PHYSICAL_MAX {
                self.let_go(i, world);
            }
        } else if nearest > RAILS_BEYOND
            && let Some(pose) = world.physics.car_pose(vid)
        {
            // Back on rails once it drives its lane as the rails would have it.
            // A scratched car drives on as traffic; a badly damaged one is on its way off the road.
            let off = (pose.position[0] - p[0]).hypot(pose.position[2] - p[2]);
            if sound && steady && pose.up_y > 0.97 && off < ON_LANE && wrap_angle(pose.yaw() - yaw_of(d)).abs() < ON_HEADING && car.blocked == f32::MAX {
                world.physics.set_car_rails(vid, true, [0.0; 3]);
                let car = &mut self.cars[i];
                (car.physical, car.speed) = (false, pose.forward_speed.max(0.0));
                self.physical -= 1;
            }
        }
    }

    /// How many cars there are, how many of them are shown and how many of those are physical.
    #[cfg(test)]
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.cars.len(), self.shown, self.physical)
    }

    /// The network and the traffic in a line each, for a measuring run to print.
    #[cfg(test)]
    pub fn report(&self) -> String {
        let graph = &self.graph;
        let all: f32 = graph.edges.iter().map(|edge| edge.length()).sum();
        let live = graph.live.iter().filter(|&&live| live).count();
        let speed = self.cars.iter().map(|car| car.speed).sum::<f32>() / self.cars.len().max(1) as f32;
        let standing = self.cars.iter().filter(|car| car.speed < 0.5).count();
        // Why the standing cars stand: behind another, at a junction that is another road's, with no
        // room beyond one, held up by what they saw, or none of those.
        let mut why = [0usize; 5];
        for (i, car) in self.cars.iter().enumerate().filter(|(_, car)| car.speed < 0.5) {
            let (left, node) = (graph.length(car.way) - car.along, graph.end(car.way));
            let at_junction = graph.nodes[node as usize].edges >= 3 && car.holds != Some(node) && left < STOP_BACK + 3.0;
            let reason = if car.blocked < f32::MAX {
                3
            } else if self.leader(i).is_some_and(|(gap, _)| gap < GAP + 2.0) {
                0
            } else if !at_junction {
                4
            } else if self.junctions[node as usize].is_some_and(|(way, _, until)| way != car.way.index() as u32 && self.tick <= until) {
                1
            } else {
                2
            };
            why[reason] += 1;
        }
        format!(
            "network: {} nodes, {} edges, {:.1} km of road, {live} of {} ways driven over {:.1} km, {} ways in from the world's edge, {} points where roads pass over each other, {} of {} bridge roads shut\ntraffic: {} cars, {} shown, {} physical, mean speed {speed:.1} m/s, {} passes so far, {} out passing, {standing} standing ({} behind a car, {} at another road's junction, {} with no room beyond one, {} held up, {} other)",
            graph.nodes.len(),
            graph.edges.len(),
            all / 1000.0,
            graph.live.len(),
            graph.driven() / 1000.0,
            graph.entries.len(),
            graph.split,
            graph.edges.iter().filter(|edge| edge.shut).count(),
            graph.edges.iter().filter(|edge| edge.carried).count(),
            self.cars.len(),
            self.shown,
            self.physical,
            self.overtakes,
            self.cars.iter().filter(|car| car.aside > 0.0).count(),
            why[0],
            why[1],
            why[2],
            why[3],
            why[4]
        )
    }
}

impl Mechanic for Traffic {
    #[cfg(test)]
    fn inside(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    #[cfg(test)]
    fn inside_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        if world.traffic == 0.0 && self.cars.is_empty() {
            return;
        }
        self.viewers = viewers(world);
        self.looks = world.entities.all::<Player>().filter(|&(id, _)| human(world, id)).map(|(_, player)| shared::quant::from_mrad(player.input.yaw)).collect();
        self.lay_out(world);
        let tick = world.tick;
        self.tick = tick;
        if tick.is_multiple_of(UNSEEN_EVERY) {
            self.inspect(world);
        }
        // The lanes of those who come towards a car that is out of its own.
        for i in 0..self.cars.len() {
            if self.cars[i].shift > 0.5 || self.cars[i].aside > 0.0 {
                let ways: Vec<Way> = self.road_ahead(i).map(|(way, _)| way.reversed()).collect();
                for way in ways {
                    self.met[way.index()] = tick;
                }
            }
        }
        // What a shown car may find in its way: the vehicles that are not on rails, and everyone on foot.
        self.obstacles.clear();
        self.people.clear();
        if self.shown > 0 {
            for (vid, vehicle) in world.entities.all::<Vehicle>() {
                if let Some(pose) = world.physics.car_pose(vid).filter(|_| !world.physics.car_on_rails(vid)) {
                    let [hx, _, hz] = vehicle_def(vehicle.kind).spec.half_extents;
                    self.obstacles.push(Obstacle { vid, pose, half: [hx, hz], traffic: world.entities.has::<TrafficCar>(vid) });
                }
            }
            self.people.extend(world.entities.all::<Player>().filter(|&(id, player)| player.seat.is_none() && world.entities.alive(id)).map(|(_, player)| player.character.position));
        }
        if tick.is_multiple_of(LOOK_EVERY) {
            self.threats(world);
        }
        self.hear(world);
        for i in 0..self.cars.len() {
            if (i as u32).wrapping_add(tick).is_multiple_of(NEAR_EVERY) {
                self.rank(i, world);
            }
            if self.cars[i].mood != Mood::Calm {
                self.nerves(i, world);
            }
            let car = &mut self.cars[i];
            if car.vid.is_some() {
                // Held up: standing behind what it saw.
                car.held = if car.blocked < GAP + 2.0 && car.speed.abs() < 0.5 { car.held + 1 } else { 0 };
                if (i as u32).wrapping_add(tick).is_multiple_of(LOOK_EVERY) {
                    self.look(i);
                    // About to be hit by something that is not on rails: off them, to be moved by it.
                    let car = &self.cars[i];
                    if !car.physical && car.by != 0 && car.blocked < STRUCK_WITHIN {
                        self.let_go(i, world);
                    }
                }
            }
            if self.cars[i].physical {
                // A physical car is wherever it has got to, which may be past the one that was ahead.
                let lane = &mut self.lanes[self.cars[i].way.index()];
                let cars = &self.cars;
                lane.make_contiguous().sort_by(|&a, &b| cars[b as usize].along.total_cmp(&cars[a as usize].along));
                self.drive(i, world);
            } else if self.cars[i].vid.is_some() {
                self.roll(i, world, dt, 1);
            } else if (i as u32).wrapping_add(tick).is_multiple_of(UNSEEN_EVERY) {
                self.roll(i, world, dt, UNSEEN_EVERY);
            }
        }

        drivers::afoot(world, &self.viewers, dt);

        // What was traffic once is taken away: a wreck soon, and a car left empty after a while,
        // sooner where it is in the traffic's way and sooner still where nobody watches it go.
        let mut standing: Vec<(u32, u32)> = Vec::new();
        for vid in world.entities.ids::<With<Abandoned>>() {
            let occupied = world.entities.get::<Seats>(vid).is_some_and(|seats| !seats.vacant());
            let wrecked = world.entities.dead(vid);
            let place = world.physics.car_pose(vid).map(|pose| pose.position);
            // Fallen out of the world: gone, where `Vehicles` would put it back where it was first shown.
            if place.is_some_and(|p| p[1] < KILL_Y + 5.0) {
                remove_vehicle(world, vid);
                continue;
            }
            let watched = place.is_some_and(|p| self.watched(p));
            let theirs = place.is_some_and(|p| self.viewers.iter().any(|v| (v[0] - p[0]).hypot(v[2] - p[2]) < ABANDON_THEIRS));
            let found = self.in_way.contains(&vid);
            let (empty, gone) = {
                let mut abandoned = world.entities.of_mut::<Abandoned>(vid);
                if found {
                    abandoned.in_way = Some(tick);
                }
                let in_way = abandoned.in_way.is_some_and(|at| tick.wrapping_sub(at) < ticks(IN_WAY_FOR));
                // A player in it has it; one beside it may, unless it blocks the road.
                abandoned.empty = if occupied {
                    0
                } else if theirs && !in_way {
                    abandoned.empty
                } else {
                    abandoned.empty + 1
                };
                abandoned.wrecked = if wrecked { abandoned.wrecked + 1 } else { 0 };
                let after = if in_way { ABANDON_IN_WAY } else { ABANDON_ASIDE }[usize::from(watched)];
                (abandoned.empty, abandoned.empty > ticks(after) || abandoned.wrecked > ticks(ABANDON_WRECK))
            };
            if gone {
                remove_vehicle(world, vid);
            } else if !occupied {
                standing.push((empty, vid));
            }
        }
        self.in_way.clear();
        // Too many left standing: those empty longest go.
        if standing.len() > ABANDONED_MAX {
            standing.sort_unstable_by_key(|&(empty, vid)| (std::cmp::Reverse(empty), vid));
            for &(_, vid) in &standing[..standing.len() - ABANDONED_MAX] {
                remove_vehicle(world, vid);
            }
        }
    }
}

#[cfg(test)]
mod tests;
