//! Public transport (BUSES.md): on a world with bus lines (`WorldFile::bus_lines`), buses run each
//! line from end to end and back, keeping to the right of the road's middle and halting at every
//! stop. A bus is an ordinary vehicle (`KIND_BUS`) with a driver in seat 0: a player with no
//! connection, whose input this writes as a player in that seat would, so `Vehicles` drives the real
//! bus. Anyone boards with USE into the first free seat, which is a passenger's while the driver
//! sits.
//!
//! A bus whose driver is dead or gone is no longer a line's bus: its driver's seat is free, so
//! whoever likes drives it off. A fresh bus takes its place on the line after `BUS_RETURN`, and the
//! old one is taken away once it has stood empty for `STRAY_EMPTY`.
//!
//! The room adds and removes the drivers, since a player joins and leaves through it: this asks for
//! them in `World::bus_calls`, which `Game::step` answers.

use super::Mechanic;
use super::vehicles::{add_vehicle, alight, remove_vehicle};
use crate::entities::With;
use crate::game::{Player, Seats, Vehicle, World};
use shared::consts::*;
use shared::protocol::{InputFrame, KIND_BUS};
use shared::quant::wrap_angle;
use shared::vehicles::vehicle_def;
use shared::world::BusLine;
use sim::CarPose;

/// The most buses a world runs, over all its lines: each is a vehicle the physics steps every tick.
pub const BUS_MAX: usize = 12;
/// The wait at a stop the number of buses on a line aims at, seconds.
const BUS_HEADWAY: f32 = 180.0;
/// How fast a bus drives between stops, m/s: under its top speed, so it has power in hand uphill.
const BUS_CRUISE: f32 = 13.0;
/// How far right of the road's middle a bus keeps, metres: buses going opposite ways pass each other
/// with a metre between them, and each keeps to its half of a carriageway seven metres wide.
const BUS_LANE: f32 = 1.75;
/// How long a bus stands at a stop, seconds.
const BUS_DWELL: f32 = 8.0;
/// How long after a line loses a bus a fresh one starts on it, seconds.
const BUS_RETURN: f32 = 60.0;
/// A bus halts within this of its stop, metres.
const STOP_WITHIN: f32 = 2.0;
/// The slowing a bus plans on, m/s², and the sideways pull it takes a bend at.
const BRAKING: f32 = 2.0;
const CORNERING: f32 = 2.0;
/// The slowest it takes any bend, and the speed it creeps up to a wall or a stop at, m/s.
const BEND_SPEED: f32 = 3.0;
const CREEP: f32 = 2.0;
/// The point it steers at is this far ahead along the line, metres, at a standstill and at most, and
/// this much further for every m/s.
const LOOK_AHEAD: [f32; 2] = [8.0, 16.0];
const LOOK_PER_SPEED: f32 = 0.5;
/// How far ahead it looks for bends, and the length the turn of the line is measured over, metres.
const BEND_SIGHT: f32 = 42.0;
const BEND_SPAN: f32 = 6.0;
/// A bend this far behind it still holds it to the bend's speed, metres: about its own length.
const BEND_BEHIND: f32 = 12.0;
/// It halts this far behind a vehicle in its way, and holds this far off a wall, metres.
const GAP_VEHICLE: f32 = 4.0;
const GAP_WALL: f32 = 2.0;
/// After this long held up by a vehicle it pushes on at a creep, seconds.
const HELD_MAX: f32 = 15.0;
/// A bus that has not got `STALL_PROGRESS` metres nearer its next stop in `STALL` seconds is stuck:
/// it backs out for `REVERSE`, and after `TRIES` of those it is put back on its line.
const STALL_PROGRESS: f32 = 3.0;
const STALL: f32 = 6.0;
const REVERSE: f32 = 2.0;
const TRIES: u32 = 3;
/// A bus this far from its line is lost, metres, and is put back on it.
const LOST: f32 = 25.0;
/// How far either side of where it last was a bus looks for itself on the line, metres: a line may
/// pass close by itself.
const SEARCH: f32 = 60.0;
/// A bus that has left its line is taken away after standing empty this long, seconds, and a wreck
/// after this long.
const STRAY_EMPTY: f32 = 60.0;
const STRAY_WRECK: f32 = 8.0;
/// A dead driver is taken away after this long, seconds.
const DRIVER_LIES: f32 = 10.0;
/// A bus is put down no nearer than this to another vehicle's middle, metres.
const CLEAR_OF_VEHICLES: f32 = 9.0;
/// Where the line's surface is looked for from, metres over the ground: above any bridge.
const SURFACE_FROM: f32 = 60.0;

/// What `Buses` asks of the room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusCall {
    /// A driver for this bus.
    Hire(u32),
    /// This driver is to leave the game.
    Dismiss(u32),
}

/// A bus on a line. A component on the vehicle's entity.
#[derive(crate::entities::Component, Clone, Debug)]
pub struct Bus {
    /// Which of the world's lines it runs.
    pub line: usize,
    /// Heading for the line's last stop, rather than back to its first.
    pub out: bool,
    /// How far along the line it is, from the line's start, metres.
    pub along: f32,
    /// The stop it is driving to, as an index into the line's stops.
    pub next: usize,
    /// Ticks left of its halt at a stop; 0 while it drives.
    pub wait: u32,
    /// Its driver, once the room has added one.
    pub driver: Option<u32>,
    /// The nearest it has been to its next stop, metres, and for how many ticks it has got no nearer.
    best: f32,
    stalled: u32,
    /// How many times it has backed out on the way to this stop, and ticks left of backing out now.
    tries: u32,
    reverse_left: u32,
    /// Ticks it has been held up by a vehicle in its way.
    held: u32,
}

/// The driver of a bus. A component on the player's entity.
#[derive(crate::entities::Component, Clone, Copy, Debug)]
pub struct BusDriver;

/// A bus that has lost its driver and its line, with the ticks it has stood empty and lain wrecked.
#[derive(crate::entities::Component, Clone, Copy, Debug, Default)]
pub struct Strayed {
    empty: u32,
    wrecked: u32,
}

/// A line laid out for driving: its points and stops in metres, and how far along each point is.
struct Path {
    points: Vec<[f32; 2]>,
    gone: Vec<f32>,
    stops: Vec<f32>,
}

impl Path {
    fn new(line: &BusLine) -> Self {
        let points: Vec<[f32; 2]> = line.points.iter().map(|p| [p[0] as f32 / 100.0, p[1] as f32 / 100.0]).collect();
        let mut gone = vec![0.0; points.len()];
        for i in 1..points.len() {
            gone[i] = gone[i - 1] + (points[i][0] - points[i - 1][0]).hypot(points[i][1] - points[i - 1][1]);
        }
        Path { points, gone, stops: line.stops.iter().map(|&stop| stop as f32 / 100.0).collect() }
    }

    fn length(&self) -> f32 {
        self.gone.last().copied().unwrap_or(0.0)
    }

    /// The point `along` metres from the start (held to the line's ends) and the unit direction the
    /// line runs there.
    fn at(&self, along: f32) -> ([f32; 2], [f32; 2]) {
        let along = along.clamp(0.0, self.length());
        let i = self.gone.partition_point(|&gone| gone <= along).clamp(1, self.points.len().max(2) - 1);
        let (Some(&a), Some(&b)) = (self.points.get(i - 1), self.points.get(i)) else { return (self.points.first().copied().unwrap_or_default(), [0.0, -1.0]) };
        let span = (self.gone[i] - self.gone[i - 1]).max(1e-3);
        let way = [(b[0] - a[0]) / span, (b[1] - a[1]) / span];
        let t = along - self.gone[i - 1];
        ([a[0] + way[0] * t, a[1] + way[1] * t], way)
    }

    /// How far along the line the point of it nearest `p` is, looking only between `from` and `to`
    /// metres along it, and how far from the line `p` is there.
    fn nearest(&self, p: [f32; 2], from: f32, to: f32) -> (f32, f32) {
        let mut best = (from.max(0.0), f32::MAX);
        for i in 1..self.points.len() {
            if self.gone[i] < from || self.gone[i - 1] > to {
                continue;
            }
            let (a, b) = (self.points[i - 1], self.points[i]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let span = self.gone[i] - self.gone[i - 1];
            let t = if span < 1e-3 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / (span * span)).clamp(0.0, 1.0) };
            let off = (p[0] - a[0] - d[0] * t).hypot(p[1] - a[1] - d[1] * t);
            if off < best.1 {
                best = (self.gone[i - 1] + span * t, off);
            }
        }
        best
    }

    /// Where a bus `along` the line stands and faces, heading `out` or back: `BUS_LANE` to the right
    /// of the line as it goes, `[x, z]` and its yaw.
    fn lane(&self, along: f32, out: bool) -> ([f32; 2], f32) {
        let (p, way) = self.at(along);
        let way = if out { way } else { [-way[0], -way[1]] };
        // With the nose along -Z the right is +X: a quarter turn clockwise from the way it goes.
        let right = [-way[1], way[0]];
        ([p[0] + right[0] * BUS_LANE, p[1] + right[1] * BUS_LANE], (-way[0]).atan2(-way[1]))
    }

    /// The stop a bus `along` the line drives to next, heading `out` or back.
    fn next_stop(&self, along: f32, out: bool) -> usize {
        if out {
            self.stops.iter().position(|&stop| stop > along + STOP_WITHIN).unwrap_or(self.stops.len().saturating_sub(1))
        } else {
            self.stops.iter().rposition(|&stop| stop < along - STOP_WITHIN).unwrap_or(0)
        }
    }

    /// How long a bus takes there and back, seconds: the drive, and the halt and the slowing and
    /// getting away at each stop.
    fn round_trip(&self) -> f32 {
        2.0 * (self.length() / BUS_CRUISE + self.stops.len() as f32 * (BUS_DWELL + BUS_CRUISE / BRAKING))
    }
}

/// How many ticks `seconds` are.
fn ticks(seconds: f32) -> u32 {
    (seconds * TICK_RATE as f32) as u32
}

/// Runs the buses of a world that has bus lines.
#[derive(Default)]
pub struct Buses {
    /// Each of the world's lines, laid out.
    paths: Vec<Path>,
    /// How many buses each line runs.
    wanted: Vec<usize>,
    /// Lines owed a fresh bus, with the tick it is due.
    due: Vec<(u32, usize)>,
    /// Drivers to take away, with the tick to do it.
    gone: Vec<(u32, u32)>,
}

impl Buses {
    /// Lays the lines out and puts their buses on them, the first time and again if the world's lines
    /// changed (a test sets them after the start).
    fn lay_out(&mut self, world: &mut World) {
        if self.paths.len() == world.map.bus_lines.len() {
            return;
        }
        self.paths = world.map.bus_lines.iter().map(Path::new).collect();
        // Enough on each line for the wait at a stop, at least two, and fewer of each when that is
        // more than the world runs.
        let mut wanted: Vec<usize> = self.paths.iter().map(|path| ((path.round_trip() / BUS_HEADWAY).ceil() as usize).max(2)).collect();
        while wanted.iter().sum::<usize>() > BUS_MAX {
            let Some(most) = (0..wanted.len()).filter(|&line| wanted[line] > 1).max_by_key(|&line| wanted[line]) else { break };
            wanted[most] -= 1;
        }
        for (line, &count) in wanted.iter().enumerate() {
            let length = self.paths[line].length();
            for n in 0..count {
                // Spread evenly round the loop there and back.
                let round = 2.0 * length * (n as f32 + 0.5) / count as f32;
                let (along, out) = if round < length { (round, true) } else { (2.0 * length - round, false) };
                if put(world, &self.paths[line], line, along, out).is_none() {
                    tracing::warn!(line, along, "no room for a bus; it starts later");
                    self.due.push((world.tick + ticks(BUS_RETURN), line));
                }
            }
        }
        tracing::info!(lines = self.paths.len(), buses = world.entities.count::<Bus>(), "bus lines running");
        self.wanted = wanted;
    }
}

/// The height of the road's surface at `(x, z)`: the bridge deck where there is one, else the ground.
fn surface(world: &World, x: f32, z: f32) -> f32 {
    let ground = world.map.ground.height_at(x, z);
    let from = ground + SURFACE_FROM;
    world.physics.cast_ray([x, from, z], [0.0, -1.0, 0.0], SURFACE_FROM, 0).map_or(ground, |fall| from - fall)
}

/// Where a bus stands `along` the line heading `out` or back, if it fits there: nudged on along the
/// line a few times when it does not.
fn standing(world: &World, path: &Path, along: f32, out: bool, own: Option<u32>) -> Option<(f32, [f32; 3], f32)> {
    let step = if out { 15.0 } else { -15.0 };
    (0..6).map(|n| (along + step * n as f32).clamp(0.0, path.length())).find_map(|along| {
        let ([x, z], yaw) = path.lane(along, out);
        let position = [x, surface(world, x, z) + vehicle_def(KIND_BUS).body.drop_y, z];
        // Clear of the buildings and walls, and of every vehicle but itself.
        let clear = world.entities.ids::<With<Vehicle>>().into_iter().filter(|&vid| Some(vid) != own).filter_map(|vid| world.physics.car_pose(vid)).all(|pose| (pose.position[0] - x).hypot(pose.position[2] - z) > CLEAR_OF_VEHICLES);
        let fits = clear && world.physics.car_fits(KIND_BUS, position, yaw);
        fits.then_some((along, position, yaw))
    })
}

/// Puts a fresh bus on `line` at `along` heading `out` or back, and asks for its driver. `None` when
/// the world is full of vehicles or it does not fit there.
fn put(world: &mut World, path: &Path, line: usize, along: f32, out: bool) -> Option<u32> {
    if world.entities.count::<Vehicle>() >= MAX_VEHICLES {
        return None;
    }
    let (along, position, yaw) = standing(world, path, along, out, None)?;
    let vid = add_vehicle(world, KIND_BUS, position, yaw);
    world.entities.insert(vid, Bus { line, out, along, next: path.next_stop(along, out), wait: 0, driver: None, best: f32::MAX, stalled: 0, tries: 0, reverse_left: 0, held: 0 });
    world.bus_calls.push(BusCall::Hire(vid));
    Some(vid)
}

/// Whether anyone but its driver is aboard bus `vid`.
fn carrying(world: &World, vid: u32) -> bool {
    world.entities.get::<Seats>(vid).is_some_and(|seats| seats.riders().any(|id| !world.entities.has::<BusDriver>(id)))
}

/// Throttle and steering as a driver's axes, with the handbrake on when `halt`.
fn controls(world: &World, throttle: f32, steer: f32, halt: bool) -> InputFrame {
    let axis = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8;
    InputFrame { forward: axis(throttle), strafe: axis(steer), buttons: if halt { BUTTON_JUMP } else { 0 }, view: world.tick as u16, ..Default::default() }
}

/// One tick of a bus's driving: what its driver asks of it. It also keeps the bus's place on its
/// line, counts down its halts, turns it round at the ends and puts it back when it is stuck or lost.
fn drive(world: &mut World, path: &Path, bus: &mut Bus, vid: u32, pose: &CarPose) -> InputFrame {
    let halt = controls(world, 0.0, 0.0, true);
    let [x, _, z] = pose.position;
    let speed = pose.forward_speed;
    let (along, off) = path.nearest([x, z], bus.along - SEARCH, bus.along + SEARCH);
    bus.along = along;
    let way = if bus.out { 1.0 } else { -1.0 };
    let Some(&stop) = path.stops.get(bus.next) else { return halt };
    let last = bus.next == if bus.out { path.stops.len() - 1 } else { 0 };

    // Standing at a stop.
    if bus.wait > 0 {
        bus.wait -= 1;
        if bus.wait == 0 {
            if last {
                // The end of the line: it is turned to face back along it, on the other side of the road.
                if let Some((along, position, yaw)) = standing(world, path, stop, !bus.out, Some(vid)) {
                    world.physics.reset_car(vid, position, yaw);
                    bus.out = !bus.out;
                    bus.along = along;
                } else {
                    bus.wait = ticks(1.0);
                    return halt;
                }
            }
            bus.next = path.next_stop(bus.along, bus.out);
            (bus.best, bus.stalled, bus.tries) = (f32::MAX, 0, 0);
        }
        return halt;
    }

    // Lost, or stuck for good: back onto the line where it was, unless it carries anyone.
    let lost = off > LOST || bus.tries >= TRIES;
    if lost && !carrying(world, vid) {
        if let Some((along, position, yaw)) = standing(world, path, bus.along, bus.out, Some(vid)) {
            tracing::debug!(vid, off, tries = bus.tries, "bus put back on its line");
            world.physics.reset_car(vid, position, yaw);
            bus.along = along;
        }
        (bus.best, bus.stalled, bus.tries, bus.reverse_left) = (f32::MAX, 0, 0, 0);
        return halt;
    }

    // Steering: at a point ahead on its side of the road, turning as the arc through it asks.
    let ahead = (LOOK_AHEAD[0] + LOOK_PER_SPEED * speed.max(0.0)).min(LOOK_AHEAD[1]);
    let (goal, _) = path.lane(along + way * ahead, bus.out);
    let reach = (goal[0] - x).hypot(goal[1] - z).max(1.0);
    let bearing = wrap_angle((-(goal[0] - x)).atan2(-(goal[1] - z)) - pose.yaw());
    let lock = (2.0 * 2.0 * BUS_AXLE_Z * bearing.sin() / reach).atan();
    // A positive steer turns the hull clockwise seen from above, which takes its yaw down.
    let steer = -lock / BUS_MAX_STEER;

    // Backing out of where it stuck.
    if bus.reverse_left > 0 {
        bus.reverse_left -= 1;
        return controls(world, -1.0, -steer, false);
    }

    // How fast it may go: its cruising speed, less for a bend ahead, for its stop and for what is in its way.
    let mut limit = BUS_CRUISE;
    let slowing = |to: f32, over: f32| (to * to + 2.0 * BRAKING * over.max(0.0)).sqrt();
    // From a little behind it too: it is not round a bend until its tail is.
    let mut sight = -BEND_BEHIND;
    while sight <= BEND_SIGHT {
        let (_, before) = path.at(along + way * (sight - BEND_SPAN));
        let (_, after) = path.at(along + way * sight);
        let turn = (before[0] * after[1] - before[1] * after[0]).atan2(before[0] * after[0] + before[1] * after[1]).abs();
        if turn > 0.01 {
            let bend = (CORNERING * BEND_SPAN / turn).sqrt().clamp(BEND_SPEED, BUS_CRUISE);
            limit = limit.min(slowing(bend, sight - BEND_SPAN - GAP_VEHICLE));
        }
        sight += BEND_SPAN;
    }
    let left = (stop - along) * way;
    if left <= STOP_WITHIN {
        if speed.abs() < 0.3 {
            // Arrived. At the end of the line everyone gets off.
            bus.wait = ticks(BUS_DWELL);
            if last {
                let riders: Vec<u32> = world.entities.get::<Seats>(vid).map(|seats| seats.riders().filter(|&id| !world.entities.has::<BusDriver>(id)).collect()).unwrap_or_default();
                for id in riders {
                    alight(world, id);
                }
            }
        }
        return halt;
    }
    limit = limit.min(slowing(0.0, left - STOP_WITHIN).max(CREEP));

    // What is in the way: a ray ahead from its nose, as far as it needs to stop in.
    let nose = pose.world_point([0.0, 0.6, -BUS_HALF_EXTENTS[2] - 0.3]);
    let range = (speed * speed / (2.0 * BRAKING) + 6.0).clamp(8.0, 45.0);
    let mut held = false;
    match world.physics.cast_ray_hit(nose, pose.world_dir([0.0, 0.0, -1.0]), range, vid) {
        Some((gap, Some(_))) => {
            let pushing = bus.held > ticks(HELD_MAX);
            held = gap < GAP_VEHICLE + 1.0 && !pushing;
            limit = limit.min(if pushing { CREEP } else { slowing(0.0, gap - GAP_VEHICLE) });
        }
        Some((gap, None)) => limit = limit.min(slowing(0.0, gap - GAP_WALL).max(CREEP)),
        None => {}
    }
    bus.held = if held || bus.held > ticks(HELD_MAX) && speed < 0.5 { bus.held + 1 } else { 0 };

    // Stuck: no nearer its stop for a while, though nothing it waits for holds it.
    if !held {
        if left < bus.best - STALL_PROGRESS {
            (bus.best, bus.stalled) = (left, 0);
        } else {
            bus.stalled += 1;
        }
        if bus.stalled > ticks(STALL) {
            bus.tries += 1;
            (bus.best, bus.stalled) = (f32::MAX, 0);
            bus.reverse_left = ticks(REVERSE);
            tracing::debug!(vid, tries = bus.tries, "bus stuck; backing out");
            return halt;
        }
    }

    // The throttle for that speed: on for more, against the way it rolls to brake for less.
    let over = speed - limit;
    if limit < 0.3 && speed < 0.6 {
        return halt;
    }
    let throttle = if over < 0.0 { (-over * 0.6).min(1.0) } else if speed > 1.2 { (-over * 0.5).max(-1.0) } else { 0.0 };
    controls(world, throttle, steer, over > 3.0)
}

impl Mechanic for Buses {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        self.lay_out(world);
        if self.paths.is_empty() {
            return;
        }
        for vid in world.entities.ids::<With<Bus>>() {
            let mut bus = world.entities.of::<Bus>(vid).clone();
            // Still waiting for the room to send its driver.
            let Some(driver) = bus.driver else { continue };
            let seated = world.entities.alive(vid) && world.entities.alive(driver) && world.entities.get::<Player>(driver).and_then(|player| player.seat).is_some_and(|seat| seat.vehicle == vid && seat.index == 0);
            let pose = world.physics.car_pose(vid);
            let (Some(pose), true, Some(path)) = (pose, seated, self.paths.get(bus.line)) else {
                // Its driver is dead or gone: the bus is anyone's, and its line gets another.
                tracing::info!(vid, line = bus.line, "bus lost its driver; out of service");
                world.entities.remove::<Bus>(vid);
                world.entities.insert(vid, Strayed::default());
                self.due.push((world.tick + ticks(BUS_RETURN), bus.line));
                if world.entities.has::<Player>(driver) {
                    self.gone.push((world.tick + ticks(DRIVER_LIES), driver));
                }
                continue;
            };
            let input = drive(world, path, &mut bus, vid, &pose);
            world.entities.insert(vid, bus);
            if let Some(mut player) = world.entities.get_mut::<Player>(driver) {
                player.input = input;
            }
        }

        // The buses that left their lines: taken away once empty for a while, or wrecked.
        for vid in world.entities.ids::<With<Strayed>>() {
            let empty = world.entities.get::<Seats>(vid).is_none_or(Seats::vacant);
            let wrecked = world.entities.dead(vid);
            let mut strayed = world.entities.of_mut::<Strayed>(vid);
            strayed.empty = if empty { strayed.empty + 1 } else { 0 };
            strayed.wrecked = if wrecked { strayed.wrecked + 1 } else { 0 };
            if strayed.empty > ticks(STRAY_EMPTY) || strayed.wrecked > ticks(STRAY_WRECK) {
                remove_vehicle(world, vid);
            }
        }

        // Fresh buses for the lines that lost one, starting from whichever end is free.
        let tick = world.tick;
        let mut later = Vec::new();
        for (when, line) in std::mem::take(&mut self.due) {
            if tick < when {
                later.push((when, line));
                continue;
            }
            let Some(path) = self.paths.get(line) else { continue };
            let running = world.entities.all::<Bus>().filter(|(_, bus)| bus.line == line).count();
            if running >= self.wanted.get(line).copied().unwrap_or(0) {
                continue;
            }
            if put(world, path, line, 0.0, true).or_else(|| put(world, path, line, path.length(), false)).is_none() {
                later.push((tick + ticks(5.0), line));
            }
        }
        self.due = later;

        // The drivers who lie dead long enough leave.
        let (leaving, staying): (Vec<_>, Vec<_>) = std::mem::take(&mut self.gone).into_iter().partition(|&(when, _)| tick >= when);
        self.gone = staying;
        world.bus_calls.extend(leaving.into_iter().map(|(_, id)| BusCall::Dismiss(id)));
    }
}

#[cfg(test)]
mod tests;
