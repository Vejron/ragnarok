//! Getting in and out of a car by its doors (DOORS.md).
//!
//! A seat with a door (`VehicleDef::doors`) is got out of in steps: its door swings open, the one
//! in it climbs out over `STEP_OUT_S`, and the door is pulled shut behind them, or left standing open
//! by a crew in a hurry. Getting in runs the same steps the other way. Whoever is stepping is
//! `Stepping`, and stays in their seat until they are out (`step` on their `seated` says how far):
//! so a player's own soldier, which their client predicts, never moves while they do, and is set
//! down at the door only once they stand there.
//!
//! A car going faster than `BAIL_SPEED` is not got out of by its door but bailed out of: whoever
//! leaves it falls out onto the road at its speed and lies there a moment, as a car would knock
//! them down (KNOCKDOWN.md).
//!
//! The doors themselves come off when they have taken their `DoorDef::hp`: from rounds that meet them,
//! from blasts beside them and from crashes on their side, and much sooner when they stand open.
//!
//! A door standing open that nobody holds swings freely on its hinge (`swing`): the car speeding up,
//! braking and leaning swings it, and the wind blows it shut driving forwards and open reversing. It
//! is no body of the physics and never moves the car, but rays along it and along the path of its
//! edge find what it snags on, and it is torn off by anything it meets faster than a walk.

use crate::entities::Component;
use crate::game::{Player, Vehicle, World};
use shared::consts::*;
use shared::vehicles::{DoorDef, door_of, vehicle_def};
use sim::CarPose;

/// Ticks in `seconds`, at least one.
fn ticks(seconds: f32) -> i32 {
    ((seconds * TICK_RATE as f32).round() as i32).max(1)
}

/// Someone getting out of, or into, seat `seat` of `vehicle` by its door `door`: a component on their
/// own entity, a player or a driver of the traffic.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Stepping {
    pub vehicle: u32,
    pub seat: usize,
    /// Which of the kind's doors, its index in `VehicleDef::doors`.
    pub door: usize,
    /// Getting out, rather than in.
    pub out: bool,
    /// Ticks since it began. Below zero they wait their turn outside the door, for whoever is in the
    /// seat to get out first.
    pub ticks: i32,
    /// The door is pulled shut behind them; a crew in a hurry leaves it standing open.
    pub shut: bool,
    /// Out and standing by the door, for whoever put them out to take on from there (a driver of
    /// the traffic; a player is set down at once).
    pub done: bool,
    /// Ticks the door takes to open: none for a door that has come off.
    pub opening: i32,
}

impl Stepping {
    /// How far they are through getting out, 0 in the seat and 1 standing outside the door.
    pub fn step(&self) -> f32 {
        let climbing = ((self.ticks - self.opening) as f32 / ticks(STEP_OUT_S) as f32).clamp(0.0, 1.0);
        if self.out { climbing } else { 1.0 - climbing }
    }

    /// Whether the steps are all taken.
    fn over(&self) -> bool {
        self.ticks >= self.opening + ticks(STEP_OUT_S)
    }
}

/// How far `id` is through getting in or out by a door, as the wire carries it: 0 for anyone sitting
/// still, and anyone who is not stepping.
pub fn step_byte(world: &World, id: u32) -> u8 {
    world.entities.get::<Stepping>(id).map_or(0, |stepping| (stepping.step() * 255.0).round() as u8)
}

/// Whether `id` is getting in or out by a door.
pub fn stepping(world: &World, id: u32) -> bool {
    world.entities.has::<Stepping>(id)
}

/// Whether `id` has got out by a door and stands there, for whoever put them out to take on from.
pub fn stepped_out(world: &World, id: u32) -> bool {
    world.entities.get::<Stepping>(id).is_some_and(|stepping| stepping.done)
}

/// Where someone who gets out of door `door` of vehicle `vid` stands: on the ground there, and the
/// way the car points. `None` when the car is gone, or the spot is in a wall or another car, so
/// that the door cannot be got out of.
pub fn door_spot(world: &World, vid: u32, door: &DoorDef) -> Option<([f32; 3], f32)> {
    let pose = world.physics.car_pose(vid)?;
    spot_at(world, &pose, door)
}

fn spot_at(world: &World, pose: &CarPose, door: &DoorDef) -> Option<([f32; 3], f32)> {
    let spot = pose.world_point(door.spot());
    let floor = world.physics.cast_ray([spot[0], spot[1] + 1.0, spot[2]], [0.0, -1.0, 0.0], 3.0, 0).map_or(world.map.ground.height_at(spot[0], spot[2]), |fall| spot[1] + 1.0 - fall);
    let feet = [spot[0], floor, spot[2]];
    world.physics.character_fits([feet[0], feet[1] + sim::STANDING_Y, feet[2]]).then_some((feet, pose.yaw()))
}

/// The door of seat `seat` of vehicle `vid`, with its index, if it has one that can be got out of now.
fn usable_door(world: &World, vid: u32, seat: usize) -> Option<(usize, &'static DoorDef)> {
    let kind = world.entities.get::<Vehicle>(vid)?.kind;
    let (index, door) = door_of(kind, seat)?;
    door_spot(world, vid, door).map(|_| (index, door))
}

/// Starts `id`, who sits in seat `seat` of `vid`, getting out by its door, which they pull shut
/// behind them or, with `shut` false, leave open. False when the seat has no door that can be
/// got out of, and they must get out another way.
pub fn start_out(world: &mut World, id: u32, vid: u32, seat: usize, shut: bool) -> bool {
    let Some((door, _)) = usable_door(world, vid, seat) else { return false };
    let opening = if world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle.doors.off(door)) { 0 } else { ticks(DOOR_OPEN_S) };
    world.entities.insert(id, Stepping { vehicle: vid, seat, door, out: true, ticks: 0, shut, done: false, opening });
    true
}

/// Starts `id`, who has just taken seat `seat` of `vid`, getting in by its door, after waiting
/// `wait` ticks for its door to be clear. False when the seat has no door.
pub fn start_in(world: &mut World, id: u32, vid: u32, seat: usize, wait: i32) -> bool {
    let Some(kind) = world.entities.get::<Vehicle>(vid).map(|vehicle| vehicle.kind) else { return false };
    let Some((door, _)) = door_of(kind, seat) else { return false };
    let off = world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle.doors.off(door));
    // A door someone left standing open need not be opened again.
    let open = world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle.doors.ajar(door));
    let opening = if off || open { 0 } else { ticks(DOOR_OPEN_S) };
    world.entities.insert(id, Stepping { vehicle: vid, seat, door, out: false, ticks: -wait, shut: true, done: false, opening });
    true
}

/// How long someone getting out of a seat by its door takes, from first touching it to standing
/// outside it, ticks: what someone waiting to get in by the same door waits.
pub fn out_ticks() -> i32 {
    ticks(DOOR_OPEN_S) + ticks(STEP_OUT_S)
}

/// One tick of everyone getting in and out, and of the doors: each door opens as someone starts
/// through it and is pulled shut after them. A player who has got out is set down by the door, and
/// one getting out of a car that has got going bails out instead.
pub fn step(world: &mut World) {
    // Doors pulled shut once their time comes.
    for vid in world.entities.ids::<crate::entities::With<Vehicle>>() {
        let mut vehicle = world.entities.of_mut::<Vehicle>(vid);
        for n in 0..2 {
            if vehicle.doors.shut_in[n] > 0 {
                vehicle.doors.shut_in[n] -= 1;
                if vehicle.doors.shut_in[n] == 0 {
                    vehicle.doors.pulling |= 1 << n;
                }
            }
        }
    }
    let steppers: Vec<(u32, Stepping)> = world.entities.all::<Stepping>().map(|(id, stepping)| (id, *stepping)).collect();
    for (id, mut stepping) in steppers {
        let vid = stepping.vehicle;
        let pose = world.physics.car_pose(vid).filter(|_| world.entities.alive(vid));
        let player = world.entities.has::<Player>(id);
        let seated = world.entities.get::<Player>(id).is_none_or(|player| player.seat.is_some_and(|seat| seat.vehicle == vid));
        // The car is gone or wrecked, or a player was put out of it some other way: no more steps.
        let Some(pose) = pose.filter(|_| seated) else {
            world.entities.remove::<Stepping>(id);
            if player {
                super::alight(world, id);
            }
            continue;
        };
        if stepping.done {
            continue;
        }
        // Climbing out of a car that has got going: they fall out of it instead.
        if player && stepping.out && vec3_speed(&pose) > BAIL_SPEED {
            world.entities.remove::<Stepping>(id);
            bail(world, id);
            continue;
        }
        stepping.ticks += 1;
        // Their first step, once any wait is over, is the door swinging open.
        if stepping.ticks == 1 {
            open(world, vid, stepping.door);
        }
        if stepping.over() {
            let mut vehicle = world.entities.of_mut::<Vehicle>(vid);
            if stepping.shut && !vehicle.doors.off(stepping.door) {
                vehicle.doors.shut_in[stepping.door] = ticks(DOOR_SHUT_S) as u16;
            }
            if !stepping.out {
                world.entities.remove::<Stepping>(id);
                continue;
            }
            if player {
                world.entities.remove::<Stepping>(id);
                let def = vehicle_def(world.entities.of::<Vehicle>(vid).kind);
                let spot = def.doors.get(stepping.door).and_then(|door| spot_at(world, &pose, door));
                super::alight_at(world, id, spot.map(|(feet, _)| [feet[0], feet[1] + sim::STANDING_Y, feet[2]]));
                continue;
            }
            stepping.done = true;
        }
        world.entities.insert(id, stepping);
    }
}

/// Door `n` of `vid` swings open, and stays so until someone pulls it shut.
fn open(world: &mut World, vid: u32, n: usize) {
    if let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid)
        && !vehicle.doors.off(n)
    {
        vehicle.doors.open |= 1 << n;
        vehicle.doors.pulling &= !(1 << n);
        vehicle.doors.shut_in[n] = 0;
    }
}

fn vec3_speed(pose: &CarPose) -> f32 {
    shared::vec3::length(pose.velocity)
}

/// How fast someone who bails out of a car is pushed away from its side as well, m/s.
const BAIL_PUSH: f32 = 2.0;
/// How fast up someone who bails out is thrown, m/s.
const BAIL_HOP: f32 = 1.5;
/// Bailing out of a car faster than this hurts, m/s, and takes all a soldier has at `BAIL_LETHAL`.
const BAIL_HURT_FROM: f32 = 10.0;
const BAIL_LETHAL: f32 = 40.0;

/// Player `id` throws themself out of the car they sit in, which is going too fast to climb out of:
/// they land beside it going as fast as it was, and lie knocked down, hurt the more the faster it went.
pub fn bail(world: &mut World, id: u32) {
    let Some(vid) = world.entities.get::<Player>(id).and_then(|player| player.seat).map(|seat| seat.vehicle) else { return };
    let Some(pose) = world.physics.car_pose(vid) else { return };
    super::alight(world, id);
    let Some(mut player) = world.entities.get_mut::<Player>(id) else { return };
    let speed = vec3_speed(&pose);
    let out = [player.character.position[0] - pose.position[0], player.character.position[2] - pose.position[2]];
    let length = out[0].hypot(out[1]).max(1e-4);
    let character = &mut player.character;
    character.knock = [pose.velocity[0] + out[0] / length * BAIL_PUSH, pose.velocity[2] + out[1] / length * BAIL_PUSH];
    character.vy = BAIL_HOP;
    character.grounded = false;
    world.physics.knock_down(character, shared::movement::knockdown_ticks(speed.max(KNOCKDOWN_POWER)));
    player.interrupt();
    let share = ((speed - BAIL_HURT_FROM) / (BAIL_LETHAL - BAIL_HURT_FROM)).clamp(0.0, 1.0);
    let amount = (share * share * f32::from(PLAYER_MAX_HP)).round() as u8;
    if amount > 0 {
        crate::mechanics::life::damage_entity(world, id, crate::mechanics::life::Harm::new(amount, None, shared::protocol::WEAPON_ROADKILL));
    }
    tracing::debug!(id, vid, speed, "bailed out");
}

/// How far open a door must stand for what it meets to snag it, radians.
const SNAG_FROM: f32 = 0.1;
/// The heights along a door, as shares of its height up from its bottom, that rays look along for
/// what it snags on: below the waist and below the top of the window.
const SNAG_HEIGHTS: [f32; 2] = [0.4, 0.85];
/// Above this, m/s², a change in the car's velocity from one tick to the next is a crash, which
/// `crash_doors` deals with, not something that swings a door.
const SWING_ACCEL_MAX: f32 = 60.0;

/// One tick of the doors on their hinges, `dt` seconds: those held by whoever climbs through them
/// swing open, those being pulled shut swing shut and latch, and the rest swing freely with the
/// car's motion and the wind and are snagged by what they meet.
pub fn swing(world: &mut World, dt: f32) {
    let held: Vec<(u32, usize)> = world.entities.all::<Stepping>().filter(|(_, stepping)| stepping.ticks >= 1 && !stepping.over()).map(|(_, stepping)| (stepping.vehicle, stepping.door)).collect();
    let mut hurts: Vec<(u32, usize, f32)> = Vec::new();
    for vid in world.entities.ids::<crate::entities::With<Vehicle>>() {
        let kind = world.entities.of::<Vehicle>(vid).kind;
        let def = vehicle_def(kind);
        if def.doors.is_empty() {
            continue;
        }
        let Some(pose) = world.physics.car_pose(vid) else { continue };
        let mut doors = world.entities.of::<Vehicle>(vid).doors;
        let mut accel = [0.0; 3];
        for (axis, value) in accel.iter_mut().enumerate() {
            *value = (pose.velocity[axis] - doors.velocity[axis]) / dt;
        }
        doors.velocity = pose.velocity;
        if doors.open & !doors.gone == 0 {
            world.entities.of_mut::<Vehicle>(vid).doors = doors;
            continue;
        }
        if shared::vec3::length(accel) > SWING_ACCEL_MAX {
            accel = [0.0; 3];
        }
        // What swings a door, in the hull frame: gravity less the car's own acceleration, per kilogram,
        // and the wind of its going.
        let pull = pose.local_dir([-accel[0], -CAR_GRAVITY - accel[1], -accel[2]]);
        let wind = pose.local_dir([-pose.velocity[0], -pose.velocity[1], -pose.velocity[2]]);
        for (n, door) in def.doors.iter().enumerate() {
            if !doors.ajar(n) || doors.off(n) {
                doors.tip[n] = None;
                continue;
            }
            let pace = door.swing / DOOR_OPEN_S;
            let mut hurt = 0.0;
            if held.contains(&(vid, n)) {
                // Swung open by hand.
                doors.angle[n] = (doors.angle[n] + pace * dt).min(door.swing);
                doors.rate[n] = 0.0;
            } else if doors.pulling & (1 << n) != 0 {
                // Pulled shut by hand, and latched.
                doors.angle[n] = (doors.angle[n] - pace * dt).max(0.0);
                doors.rate[n] = 0.0;
                if doors.angle[n] == 0.0 {
                    latch(&mut doors, n);
                    continue;
                }
            } else {
                hurt += free_swing(&mut doors, n, door, pull, wind, dt);
                if !doors.ajar(n) {
                    continue;
                }
                hurt += snag(world, vid, &pose, &mut doors, n, door, dt);
            }
            doors.tip[n] = Some(pose.world_point(tip_of(door, doors.angle[n], SNAG_HEIGHTS[0])));
            if hurt > 0.0 {
                hurts.push((vid, n, hurt));
            }
        }
        world.entities.of_mut::<Vehicle>(vid).doors = doors;
    }
    for (vid, n, amount) in hurts {
        hurt_door(world, vid, n, amount);
    }
}

/// Door `n` swings shut and latches there.
fn latch(doors: &mut crate::game::DoorState, n: usize) {
    doors.open &= !(1 << n);
    doors.pulling &= !(1 << n);
    doors.angle[n] = 0.0;
    doors.rate[n] = 0.0;
    doors.tip[n] = None;
}

/// The outward face of `door` standing `angle` open, a unit vector in the hull frame: it faces out of
/// the side shut, and turns forwards as the door opens.
fn face_of(door: &DoorDef, angle: f32) -> [f32; 3] {
    [door.side() * angle.cos(), 0.0, -angle.sin()]
}

/// The outer edge of `door` standing `angle` open, `share` of the way up it, in the hull frame.
fn tip_of(door: &DoorDef, angle: f32, share: f32) -> [f32; 3] {
    [door.hinge[0] + door.side() * door.length * angle.sin(), door.hinge[1] + door.height * share, door.hinge[2] + door.length * angle.cos()]
}

/// Door `n`, a flat plate hung on its front edge, swings for `dt` seconds under `pull` (what the car's
/// motion and gravity do to each kilogram of it, m/s², in the hull frame) and the `wind` across it
/// (m/s, in the hull frame). It latches if it swings shut. What it takes from slamming onto or being
/// bent past its open stop.
fn free_swing(doors: &mut crate::game::DoorState, n: usize, door: &DoorDef, pull: [f32; 3], wind: [f32; 3], dt: f32) -> f32 {
    let face = face_of(door, doors.angle[n]);
    // A uniform plate turning on one edge: its middle, where the push acts, is half its length out,
    // and it turns a third as easily as its length squared.
    let across = shared::vec3::dot(wind, face);
    let push = 1.5 * (shared::vec3::dot(pull, face) + DOOR_AIR * across * across.abs()) / door.length;
    let mut rate = doors.rate[n];
    if rate.abs() < 0.05 && push.abs() < DOOR_HINGE_HOLD {
        rate = 0.0;
    } else {
        rate = (rate + push * dt) * (1.0 - DOOR_DAMPING * dt).max(0.0);
    }
    let mut angle = doors.angle[n] + rate * dt;
    let mut hurt = 0.0;
    if angle <= 0.0 {
        latch(doors, n);
        return 0.0;
    }
    if angle >= door.swing {
        angle = door.swing;
        if rate > DOOR_STOP_TEAR {
            hurt += (rate - DOOR_STOP_TEAR) * DOOR_STOP_HURT;
        }
        if push > DOOR_STOP_STRAIN {
            hurt += (push - DOOR_STOP_STRAIN) * DOOR_STRAIN_HURT * dt;
        }
        if rate > 0.0 {
            rate = -rate * DOOR_STOP_BOUNCE;
        }
    }
    doors.angle[n] = angle;
    doors.rate[n] = rate;
    hurt
}

/// Door `n` of `vid`, standing open, looks along itself and along the path its edge swept since last
/// tick for anything it meets, which shoves it aside, or tears it off going past faster than
/// `DOOR_SNAG_TEAR`. What it takes.
fn snag(world: &World, vid: u32, pose: &CarPose, doors: &mut crate::game::DoorState, n: usize, door: &DoorDef, dt: f32) -> f32 {
    let angle = doors.angle[n];
    if angle < SNAG_FROM {
        return 0.0;
    }
    let edge = pose.world_point(tip_of(door, angle, SNAG_HEIGHTS[0]));
    // Where it meets something, and how far out from the hinge.
    let mut met: Option<(f32, Option<u32>)> = None;
    for share in SNAG_HEIGHTS {
        let from = pose.world_point([door.hinge[0], door.hinge[1] + door.height * share, door.hinge[2]]);
        let to = pose.world_point(tip_of(door, angle, share));
        if let Some(hit) = ray_between(world, from, to, vid) {
            met = Some(met.map_or(hit, |seen: (f32, Option<u32>)| if hit.0 < seen.0 { hit } else { seen }));
        }
    }
    if let Some(last) = doors.tip[n]
        && let Some((_, car)) = ray_between(world, last, edge, vid)
    {
        met = Some((door.length, car));
    }
    let Some((out, car)) = met else { return 0.0 };
    // How fast the door goes past what it met, m/s.
    let mut past = doors.tip[n].map_or(pose.velocity, |last| [(edge[0] - last[0]) / dt, (edge[1] - last[1]) / dt, (edge[2] - last[2]) / dt]);
    if let Some(other) = car.and_then(|car| world.physics.car_pose(car)) {
        past = shared::vec3::sub(past, other.velocity);
    }
    past[1] = 0.0;
    // The edge's going is the car's and the door's own swing; where it met something, nearer the
    // hinge, the swing carries it less far.
    let face = pose.world_dir(face_of(door, angle));
    let out = out.clamp(0.3, door.length);
    let rate = doors.rate[n];
    let at = shared::vec3::sub(past, [face[0] * rate * (door.length - out), 0.0, face[2] * rate * (door.length - out)]);
    let speed = shared::vec3::length(at);
    if speed >= DOOR_SNAG_TEAR {
        tracing::debug!(vid, door = n, speed, "an open door snagged and was torn off");
        return f32::from(door.hp);
    }
    // Shoved aside: it swings so that where it met something goes no further into it.
    let carried = shared::vec3::dot(past, face) - rate * door.length;
    doors.rate[n] = -carried / out;
    speed * DOOR_SNAG_HURT * dt
}

/// What a ray from `from` to `to` meets first, other than vehicle `ignore`: how far along it, and the
/// vehicle it met if it was one.
fn ray_between(world: &World, from: [f32; 3], to: [f32; 3], ignore: u32) -> Option<(f32, Option<u32>)> {
    let line = shared::vec3::sub(to, from);
    let length = shared::vec3::length(line);
    if length < 1e-3 {
        return None;
    }
    let dir = [line[0] / length, line[1] / length, line[2] / length];
    world.physics.cast_ray_hit(from, dir, length, ignore)
}

/// What a door on the side a blow lands on takes from a crash, per m/s of the blow over the least
/// that dents; a door standing open takes `OPEN_DOOR_FRAILTY` times as much, and strikes whatever it
/// swings into.
const CRASH_DOOR_PER_DV: f32 = 14.0;
const OPEN_DOOR_FRAILTY: f32 = 4.0;
/// How near a door a crash's point must land to be taken by it, metres either side of its skin.
const CRASH_DOOR_REACH: f32 = 0.6;
/// What a door takes from a round that meets it, per point of the round's damage.
const ROUND_DOOR_SHARE: f32 = 0.5;
/// How near a door a round must strike to be taken by it, metres.
const ROUND_DOOR_REACH: f32 = 0.15;

/// Door `n` of `vid` takes `amount`, and comes off at its `hp`.
pub fn hurt_door(world: &mut World, vid: u32, n: usize, amount: f32) {
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return };
    let Some(door) = vehicle_def(vehicle.kind).doors.get(n) else { return };
    if vehicle.doors.off(n) || amount <= 0.0 {
        return;
    }
    let taken = vehicle.doors.damage[n] + amount;
    vehicle.doors.damage[n] = taken;
    if taken >= f32::from(door.hp) {
        let doors = &mut vehicle.doors;
        doors.gone |= 1 << n;
        doors.open &= !(1 << n);
        doors.pulling &= !(1 << n);
        doors.shut_in[n] = 0;
        doors.angle[n] = 0.0;
        doors.rate[n] = 0.0;
        doors.tip[n] = None;
        tracing::debug!(vid, door = n, "a door came off");
    }
}

/// A crash at `local` (the hull frame) with `over` m/s over the least that dents: the door it lands
/// on takes it, and a door standing open on the side it lands on takes it many times over.
pub fn crash_doors(world: &mut World, vid: u32, local: [f32; 3], over: f32) {
    let Some(vehicle) = world.entities.get::<Vehicle>(vid) else { return };
    let doors = vehicle.doors;
    let def = vehicle_def(vehicle.kind);
    for (n, door) in def.doors.iter().enumerate() {
        let ajar = doors.ajar(n);
        let on_side = (local[0] - door.hinge[0]) * door.side() > -CRASH_DOOR_REACH;
        let along = local[2] > door.hinge[2] - CRASH_DOOR_REACH && local[2] < door.hinge[2] + door.length * if ajar { 1.5 } else { 1.0 } + CRASH_DOOR_REACH;
        if !(on_side && along) {
            continue;
        }
        let amount = over * CRASH_DOOR_PER_DV * if ajar { OPEN_DOOR_FRAILTY } else { 1.0 };
        hurt_door(world, vid, n, amount);
    }
}

/// Where `point` in the world is in the hull frame of `vid`.
fn local_of(world: &World, vid: u32, point: [f32; 3]) -> Option<[f32; 3]> {
    let pose = world.physics.car_pose(vid)?;
    Some(pose.local_dir([point[0] - pose.position[0], point[1] - pose.position[1], point[2] - pose.position[2]]))
}

/// A round of `damage` striking `vid` at `point` in the world: the door it meets, if any, takes its share.
pub fn shoot_doors(world: &mut World, vid: u32, point: [f32; 3], damage: f32) {
    let Some(kind) = world.entities.get::<Vehicle>(vid).map(|vehicle| vehicle.kind) else { return };
    if vehicle_def(kind).doors.is_empty() {
        return;
    }
    let Some(local) = local_of(world, vid, point) else { return };
    for (n, door) in vehicle_def(kind).doors.iter().enumerate() {
        if door.holds(local, ROUND_DOOR_REACH) {
            hurt_door(world, vid, n, damage * ROUND_DOOR_SHARE);
        }
    }
}

/// A blast of `damage` at `point` in the world, reaching `radius` metres: every door within reach
/// takes it, the more the nearer.
pub fn blast_doors(world: &mut World, vid: u32, point: [f32; 3], damage: f32, radius: f32) {
    let Some(kind) = world.entities.get::<Vehicle>(vid).map(|vehicle| vehicle.kind) else { return };
    if vehicle_def(kind).doors.is_empty() {
        return;
    }
    let Some(local) = local_of(world, vid, point) else { return };
    for (n, door) in vehicle_def(kind).doors.iter().enumerate() {
        let middle = [door.hinge[0], door.hinge[1] + door.height * 0.4, door.hinge[2] + door.length * 0.5];
        let off = shared::vec3::length([local[0] - middle[0], local[1] - middle[1], local[2] - middle[2]]);
        if off < radius {
            hurt_door(world, vid, n, damage * (1.0 - off / radius) * 2.0);
        }
    }
}

/// Whether someone standing at `at` is by a free door of `vid` they could get in by: the door's
/// seat and how far they stand from where it is got in, metres.
pub fn door_within(world: &World, vid: u32, at: [f32; 3], reach: f32) -> Option<(usize, f32)> {
    let kind = world.entities.get::<Vehicle>(vid)?.kind;
    let pose = world.physics.car_pose(vid)?;
    let seats = world.entities.get::<crate::game::Seats>(vid)?;
    vehicle_def(kind)
        .doors
        .iter()
        .filter(|door| door.seat < seats.len() && seats.rider(door.seat).is_none())
        .map(|door| {
            let spot = pose.world_point(door.spot());
            (door.seat, (spot[0] - at[0]).hypot(spot[2] - at[2]))
        })
        .filter(|&(_, off)| off <= reach)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}
