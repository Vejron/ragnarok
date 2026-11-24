//! The people at the wheel of the traffic's cars (TRAFFIC.md).
//!
//! A driver at the wheel is a light entity, not a player: a kind, a place and some health, with no
//! body in the physics and nobody's input. There is one for every car of the traffic near enough a
//! player to see into, sitting at its wheel. The car's seat stays empty for all the server's vehicles
//! know, so a player who boards it takes the wheel, and the driver is put out on the road.
//!
//! A driver who has a gun pointed at their car, or whose car is shot, does one of three things by
//! their nerve: gives the car up, drives off as fast as it goes, or drives at whoever it was.
//! Out of the car a driver is a civilian (`civilians`): a soldier's body with a mind that runs.

use super::{LOOK_EVERY, PULL_OVER_FOR, Traffic, sound, ticks};
use crate::entities::{Component, Health, HurtBy, Hurtbox, NetKind, Transform};
use crate::game::{Noise, Player, World};
use crate::mechanics::bots::HEAR_BLAST;
use crate::mechanics::bots::clear_line;
use crate::mechanics::combat::look;
use crate::mechanics::vehicles::exit_position;
use shared::consts::*;
use shared::items::ItemKind;
use shared::quant::from_mrad;
use shared::vehicles::vehicle_def;
use sim::CarPose;

/// A shown car this near a player has its driver in it, metres, and one this far from every
/// player has not: nobody sees into a car from further.
const DRIVER_WITHIN: f32 = 150.0;
const DRIVER_BEYOND: f32 = 180.0;
/// A car of the traffic going faster than this is not boarded, m/s: its driver is dragged out of
/// one that stands or crawls.
pub const DRAG_SPEED: f32 = 3.0;
/// How near a gun must be for a driver to see it aimed at them, metres, and how long it must stay
/// on their car before they do something about it, seconds. Aimed is down its sights
/// (`BUTTON_ALT`): a gun that is only in hand and happens to point their way is nothing to them.
const AIM_WITHIN: f32 = 30.0;
const AIM_FOR: f32 = 0.4;
/// How long a driver who flees drives like it, seconds.
const FLEE_FOR: f32 = 20.0;
/// How near and how long a driver goes for the one who threatened them, metres and seconds.
const RAM_WITHIN: f32 = 40.0;
const RAM_FOR: f32 = 4.0;
/// A fleeing car that has stood this long is boxed in, seconds: its driver gets out and runs.
const BOXED_IN: f32 = 3.0;
/// How long a driver who gave the car up stands beside it with their hands up, seconds.
const HANDS_UP: f32 = 2.0;
/// How far above the middle of a driver at the wheel a hit on them is shown, metres: a soldier's chest.
const CHEST_Y: f32 = 0.9;
/// How long a driver who heard a blast hurries, seconds, and one who heard a shot.
const HURRY_BLAST: f32 = 12.0;
const HURRY_SHOT: f32 = 8.0;
/// A blast this near is not something to drive past, metres: the driver takes it as meant for them.
const BLAST_NEAR: f32 = 40.0;
/// A round that passes this near a car is a near miss, metres, as far along its line as this.
const NEAR_MISS: f32 = 4.0;
const NEAR_MISS_REACH: f32 = 200.0;
/// How long a driver who ducks sits there, seconds, at the least and at the most.
const DUCK: [f32; 2] = [2.0, 4.0];
/// How long a panic lasts, seconds, how far off its lane a driver in one aims, metres, and how
/// long before they throw the wheel over again, seconds.
const PANIC_FOR: f32 = 12.0;
const PANIC_SWERVE: [f32; 2] = [2.0, 5.0];
const PANIC_THROW: [f32; 2] = [0.4, 1.2];
/// A car in a panic that has got going, m/s, and then all but stopped for this long, seconds, has
/// hit something.
const PANIC_GOING: f32 = 4.0;
const PANIC_STOPPED: f32 = 1.5;
/// A dead driver's car runs on while it does more than this, m/s, for this long at most, seconds,
/// its wheel up to this share of full lock off straight and, half the time, this much throttle on.
const RUNAWAY_SPEED: f32 = 2.0;
const RUNAWAY_FOR: f32 = 8.0;
const RUNAWAY_STEER: f32 = 0.15;
const RUNAWAY_THROTTLE: f32 = 0.6;

/// What a driver does when threatened: four in ten give the car up, four flee and two go for
/// whoever it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Nerve {
    GivesUp,
    Flees,
    Rams,
}

impl Nerve {
    /// The nerve of one driver in ten, `roll` being which of the ten.
    pub(super) fn of(roll: u64) -> Self {
        match roll % 10 {
            0..=3 => Nerve::GivesUp,
            4..=7 => Nerve::Flees,
            _ => Nerve::Rams,
        }
    }
}

/// What a car's driver is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mood {
    /// Driving as traffic does.
    Calm,
    /// Stopping, to get out with their hands up.
    GivingUp,
    /// Driving off as fast as the car goes, for `left` more ticks.
    Fleeing { left: u32 },
    /// Driving at player `at`, for `left` more ticks.
    Ramming { left: u32, at: u32 },
    /// Driving faster than they would, for `left` more ticks: something went off, or someone is shooting.
    Hurrying { left: u32 },
    /// Down behind the wheel with the brake on for `left` more ticks, and then off.
    Ducking { left: u32 },
    /// Out of their mind for `left` more ticks: flat out, braking for nothing, steering for a
    /// point `swerve` metres left of their lane, which is thrown again in `next` ticks. `going`
    /// once the car has got up speed, and `stopped` the ticks it has since stood.
    Panicking { left: u32, swerve: f32, next: u32, going: bool, stopped: u32 },
    /// Dead at the wheel of a car that runs on for `left` more ticks at most, its wheel at `steer`
    /// and its throttle at `throttle`; `stopped` the ticks it has all but stood.
    Runaway { left: u32, steer: f32, throttle: f32, stopped: u32 },
    /// Taking a car too damaged to drive on off the road, onto the verge, for `left` more ticks
    /// at most.
    PullingOver { left: u32 },
}

/// What a round can meet of a driver: a soldier's body, standing on their feet when they are on
/// foot, and as a rider's when they sit at the wheel (`life::body`).
fn body() -> Hurtbox {
    Hurtbox { centre_y: sim::STANDING_Y, half_height: CAPSULE_HALF_HEIGHT, radius: HURT_RADIUS, head: Some(HEAD_ZONE_Y), mark_y: CHEST_Y, hit_kind: shared::protocol::HIT_BODY }
}

/// What frightened a driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fright {
    /// A gun kept on their car.
    Aimed,
    /// Their car was hit.
    Hit,
    /// They were hit themselves.
    Wounded,
    /// A blast right by them.
    Blast,
    /// A round went past.
    NearMiss,
}

/// A driver of the traffic at the wheel. A component on their own entity.
#[derive(Component, Clone, Copy, Debug)]
pub struct Driver {
    /// The vehicle whose wheel they sit at.
    pub vehicle: u32,
}

/// What `Traffic` asks of the room, which only it can do: drivers to put on their feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrafficCall {
    /// Driver `driver` gets out, onto their feet at `feet` facing `yaw`: a civilian from now on, with
    /// their hands up for `hands_up` ticks first, running from `threat` (`civilians`). One dead at
    /// the wheel lies there.
    Afoot { driver: u32, feet: [f32; 3], yaw: f32, hands_up: u32, threat: [f32; 3] },
    /// Civilian `id` is out of sight, or has lain dead long enough: they leave the game.
    Gone(u32),
}

/// Whether a gun at `eye` pointed along the unit vector `dir` is on the vehicle of `kind` at
/// `pose`, within `AIM_WITHIN` and with nothing between them.
fn aims(world: &World, eye: [f32; 3], dir: [f32; 3], pose: &CarPose, kind: u8) -> bool {
    let half = vehicle_def(kind).spec.half_extents.map(|h| h + 0.2);
    let from = pose.local_dir([eye[0] - pose.position[0], eye[1] - pose.position[1], eye[2] - pose.position[2]]);
    let along = pose.local_dir(dir);
    // Where the line goes into and comes out of the hull's box, in the hull's own frame.
    let (mut enters, mut leaves) = (0.0f32, AIM_WITHIN);
    for axis in 0..3 {
        if along[axis].abs() < 1e-6 {
            if from[axis].abs() > half[axis] {
                return false;
            }
            continue;
        }
        let (a, b) = ((-half[axis] - from[axis]) / along[axis], (half[axis] - from[axis]) / along[axis]);
        (enters, leaves) = (enters.max(a.min(b)), leaves.min(a.max(b)));
        if enters > leaves {
            return false;
        }
    }
    let reach = (enters - 0.4).max(0.0);
    clear_line(world, eye, [eye[0] + dir[0] * reach, eye[1] + dir[1] * reach, eye[2] + dir[2] * reach])
}

impl Traffic {
    /// Puts a driver at the wheel of shown car `i` when a player is near enough to see one, and
    /// takes them away again when nobody is.
    pub(super) fn man(&mut self, i: usize, world: &mut World) {
        let car = self.cars[i];
        let Some(vid) = car.vid else { return };
        if car.driver.is_none() && car.nearest < DRIVER_WITHIN {
            let (position, d) = self.pose(i);
            let id = world.alloc_id();
            world.entities.spawn(id, (NetKind(KIND_CIVILIAN), Transform { position, yaw: super::yaw_of(d) }, Health::full(PLAYER_MAX_HP), body(), Driver { vehicle: vid }));
            let car = &mut self.cars[i];
            (car.driver, car.hp, car.hurt) = (Some(id), world.entities.get::<Health>(vid).map_or(0, |health| health.hp), PLAYER_MAX_HP);
        } else if car.driver.is_some() && car.nearest > DRIVER_BEYOND && car.mood == Mood::Calm {
            self.unman(i, world);
        }
    }

    /// Takes the driver of car `i` out of the world with their car.
    pub(super) fn unman(&mut self, i: usize, world: &mut World) {
        if let Some(id) = self.cars[i].driver.take() {
            world.entities.despawn(id);
        }
    }

    /// The driver of car `i` gets out and is a civilian from now on (`TrafficCall::Afoot`): with
    /// their hands up first if they gave the car up, and then running from what frightened them, or
    /// from whoever is nearest if nothing did. One shot dead at the wheel falls out and lies beside
    /// the car. A wreck's driver is gone with it.
    pub(super) fn put_out(&mut self, i: usize, world: &mut World) {
        let car = self.cars[i];
        let Some(id) = self.cars[i].driver.take() else { return };
        let pose = car.vid.filter(|&vid| !world.entities.dead(vid)).and_then(|vid| world.physics.car_pose(vid));
        let Some(pose) = pose else {
            world.entities.despawn(id);
            return;
        };
        let spot = exit_position(world, car.kind, &pose, 0);
        let feet = world.physics.cast_ray([spot[0], spot[1] + 0.5, spot[2]], [0.0, -1.0, 0.0], 4.0, 0).map_or(world.map.ground.height_at(spot[0], spot[2]), |fall| spot[1] + 0.5 - fall);
        let nearest = self.viewers.iter().copied().min_by(|a, b| (a[0] - spot[0]).hypot(a[2] - spot[2]).total_cmp(&(b[0] - spot[0]).hypot(b[2] - spot[2])));
        let threat = if car.mood == Mood::Calm { nearest.map_or([pose.position[0], pose.position[2]], |v| [v[0], v[2]]) } else { car.threat };
        let hands_up = if car.mood == Mood::GivingUp && world.entities.alive(id) { ticks(HANDS_UP) } else { 0 };
        world.traffic_calls.push(TrafficCall::Afoot { driver: id, feet: [spot[0], feet, spot[2]], yaw: pose.yaw(), hands_up, threat: [threat[0], feet, threat[1]] });
    }

    /// Whether a driver has been threatened: a player's gun has stayed on their car, or the car or
    /// they themselves have been hurt. They take it as their nerve has it. One shot dead at the wheel
    /// drives no further, and a car too damaged to drive on is taken off the road.
    pub(super) fn threats(&mut self, world: &mut World) {
        let gunmen: Vec<(u32, [f32; 3], [f32; 3])> = world
            .entities
            .all::<Player>()
            .filter(|&(id, player)| super::human(world, id) && world.entities.alive(id) && player.seat.is_none() && player.input.buttons() & BUTTON_ALT != 0 && shared::items::def(player.held().item).is_some_and(|item| item.kind == ItemKind::Gun))
            .map(|(id, player)| (id, player.eye(), look(from_mrad(player.input.yaw), from_mrad(player.input.pitch()))))
            .collect();
        for i in 0..self.cars.len() {
            let car = self.cars[i];
            let Some(vid) = car.vid else { continue };
            if !sound(world, vid) && !world.entities.dead(vid) && !matches!(car.mood, Mood::PullingOver { .. } | Mood::GivingUp) {
                self.cars[i].mood = Mood::PullingOver { left: ticks(PULL_OVER_FOR) };
                continue;
            }
            let Some(driver) = car.driver else { continue };
            if world.entities.dead(driver) {
                // Dead at the wheel of a car that is going: it runs on, with nobody to stop it.
                if matches!(car.mood, Mood::Runaway { .. }) {
                    continue;
                }
                if car.speed > RUNAWAY_SPEED {
                    self.let_go(i, world);
                    let steer = (self.rng.next_f64() as f32 * 2.0 - 1.0) * RUNAWAY_STEER;
                    let throttle = if self.rng.next_u64().is_multiple_of(2) { RUNAWAY_THROTTLE } else { 0.0 };
                    self.cars[i].mood = Mood::Runaway { left: ticks(RUNAWAY_FOR), steer, throttle, stopped: 0 };
                } else {
                    self.abandon(i, world);
                }
                continue;
            }
            if !matches!(car.mood, Mood::Calm | Mood::Hurrying { .. }) {
                // What the car and its driver take meanwhile is not news once they have calmed down.
                (self.cars[i].hp, self.cars[i].hurt) = (world.entities.get::<Health>(vid).map_or(0, |health| health.hp), world.entities.get::<Health>(driver).map_or(0, |health| health.hp));
                continue;
            }
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let mut by: Option<(Option<u32>, [f32; 3], Fright)> = None;
            // Shot, and still alive: whoever did it is who they take it out on, or run from.
            let hurt = world.entities.get::<Health>(driver).map_or(0, |health| health.hp);
            if hurt < car.hurt {
                let shooter = world.entities.get::<HurtBy>(driver).map(|&HurtBy(shooter)| shooter).filter(|&shooter| self.target(world, shooter).is_some());
                by = Some((shooter, shooter.and_then(|shooter| self.target(world, shooter)).unwrap_or(pose.position), Fright::Wounded));
            }
            // Hurt since it was last looked at, by someone: a round, a blast, or a car driven into
            // it. A knock from another car of the traffic, which nobody is behind, is only a knock.
            let hp = world.entities.get::<Health>(vid).map_or(0, |health| health.hp);
            if hp < car.hp
                && by.is_none()
                && let Some(&HurtBy(who)) = world.entities.get::<HurtBy>(vid)
            {
                let from = crate::mechanics::position_of(world, who).unwrap_or(pose.position);
                by = Some((Some(who).filter(|&who| self.target(world, who).is_some()), from, Fright::Hit));
            }
            let aimed = if car.nearest < AIM_WITHIN + 5.0 { gunmen.iter().find(|&&(_, eye, dir)| aims(world, eye, dir, &pose, car.kind)) } else { None };
            let car = &mut self.cars[i];
            (car.hp, car.hurt) = (hp, hurt);
            car.aimed = if aimed.is_some() { car.aimed + LOOK_EVERY } else { 0 };
            if let Some(&(who, eye, _)) = aimed.filter(|_| car.aimed >= ticks(AIM_FOR) && by.is_none()) {
                by = Some((Some(who), eye, Fright::Aimed));
            }
            if let Some((who, at, fright)) = by {
                self.frighten(i, world, who, at, fright);
            }
        }
    }

    /// The driver of shown car `i` has been frightened by `fright`, which `who` did from `at`, and
    /// does what their nerve has them do about it.
    fn frighten(&mut self, i: usize, world: &mut World, who: Option<u32>, at: [f32; 3], fright: Fright) {
        let Some(pose) = self.cars[i].vid.and_then(|vid| world.physics.car_pose(vid)) else { return };
        // Ahead of the car and near: one with the nerve for it drives at them.
        let (to, forward) = ([at[0] - pose.position[0], at[2] - pose.position[2]], [-pose.yaw().sin(), -pose.yaw().cos()]);
        let reach = to[0].hypot(to[1]);
        let ahead = reach < RAM_WITHIN && to[0] * forward[0] + to[1] * forward[1] > 0.5 * reach;
        let car = self.cars[i];
        let flee = Mood::Fleeing { left: ticks(FLEE_FOR) };
        let panic = Mood::Panicking { left: ticks(PANIC_FOR), swerve: 0.0, next: 0, going: false, stopped: 0 };
        let mood = match (fright, car.nerve, who) {
            // Hit themselves, or with a driver who loses their head when the car is: a panic.
            (Fright::Wounded, ..) => panic,
            (Fright::Hit, ..) if car.rash => panic,
            // A blast right by them: the bold lose their heads, the timid get out and run.
            (Fright::Blast, Nerve::Rams, _) => panic,
            (Fright::Blast, Nerve::GivesUp, _) => Mood::GivingUp,
            (Fright::Blast, Nerve::Flees, _) => flee,
            // A round past the car: the timid get down behind the wheel, and then drive off.
            (Fright::NearMiss, Nerve::GivesUp, _) => Mood::Ducking { left: ticks(DUCK[0] + (DUCK[1] - DUCK[0]) * self.rng.next_f64() as f32) },
            (Fright::NearMiss, ..) => flee,
            (_, Nerve::GivesUp, _) => Mood::GivingUp,
            (_, Nerve::Rams, Some(who)) if car.physical && ahead => Mood::Ramming { left: ticks(RAM_FOR), at: who },
            _ => flee,
        };
        // A panic is the car's to have out: it is let off its rails for it.
        if matches!(mood, Mood::Panicking { .. }) {
            self.let_go(i, world);
        }
        let car = &mut self.cars[i];
        (car.threat, car.aimed, car.boxed, car.mood) = ([at[0], at[2]], 0, 0, mood);
        tracing::debug!(car = i, ?fright, mood = ?car.mood, "a driver is frightened");
    }

    /// What the drivers of the shown cars make of the noises since last tick: a blast or gunfire
    /// in earshot makes them hurry, and a blast right by them or a round going past the car
    /// frightens them.
    pub(super) fn hear(&mut self, world: &mut World) {
        let noises: Vec<Noise> = self.heard.read(world.events::<Noise>()).copied().collect();
        if noises.is_empty() || self.shown == 0 {
            return;
        }
        for i in 0..self.cars.len() {
            let car = self.cars[i];
            if car.vid.is_none() || !matches!(car.mood, Mood::Calm | Mood::Hurrying { .. }) {
                continue;
            }
            let (p, _) = self.pose(i);
            let mut hurry = 0;
            let mut fright: Option<(Option<u32>, [f32; 3], Fright)> = None;
            for noise in &noises {
                let to = [p[0] - noise.at[0], p[1] - noise.at[1], p[2] - noise.at[2]];
                let off = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
                match noise.line {
                    // A blast: something to get away from, or right by them.
                    None if noise.reach >= HEAR_BLAST && off < BLAST_NEAR => fright = Some((noise.by, noise.at, Fright::Blast)),
                    None if noise.reach >= HEAR_BLAST && off < noise.reach => hurry = hurry.max(ticks(HURRY_BLAST)),
                    None => {}
                    // A shot: how near the car its round goes by, if it goes by it at all.
                    Some(line) => {
                        let length = (line[0] * line[0] + line[1] * line[1] + line[2] * line[2]).sqrt().max(1e-6);
                        let along = (to[0] * line[0] + to[1] * line[1] + to[2] * line[2]) / length;
                        let wide = (off * off - along * along).max(0.0).sqrt();
                        if along > 0.0 && along < NEAR_MISS_REACH && wide < NEAR_MISS {
                            fright = fright.or(Some((noise.by, noise.at, Fright::NearMiss)));
                        } else if off < noise.reach {
                            hurry = hurry.max(ticks(HURRY_SHOT));
                        }
                    }
                }
            }
            if let Some((who, at, fright)) = fright {
                self.frighten(i, world, who.filter(|&who| self.target(world, who).is_some()), at, fright);
            } else if hurry > 0 {
                let left = if let Mood::Hurrying { left } = car.mood { left.max(hurry) } else { hurry };
                self.cars[i].mood = Mood::Hurrying { left };
            }
        }
    }

    /// One tick of what the driver of shown car `i` is about, when it is not driving as traffic does.
    pub(super) fn nerves(&mut self, i: usize, world: &mut World) {
        let car = self.cars[i];
        match car.mood {
            Mood::Calm => {}
            // Stopped: out they get, and the car is anyone's.
            Mood::GivingUp => {
                if car.speed.abs() < 0.5 {
                    self.abandon(i, world);
                }
            }
            Mood::Fleeing { left } => {
                let boxed = if car.speed.abs() < 0.5 { car.boxed + 1 } else { 0 };
                if boxed > ticks(BOXED_IN) {
                    self.abandon(i, world);
                    return;
                }
                let car = &mut self.cars[i];
                car.boxed = boxed;
                car.mood = if left <= 1 { Mood::Calm } else { Mood::Fleeing { left: left - 1 } };
            }
            // On the verge and stopped, or it cannot get there: the driver leaves it.
            Mood::PullingOver { left } => {
                if left <= 1 || (self.pulled_over(i) && car.speed.abs() < 0.5) {
                    self.abandon(i, world);
                } else {
                    self.cars[i].mood = Mood::PullingOver { left: left - 1 };
                }
            }
            Mood::Hurrying { left } => self.cars[i].mood = if left <= 1 { Mood::Calm } else { Mood::Hurrying { left: left - 1 } },
            Mood::Ducking { left } => self.cars[i].mood = if left <= 1 { Mood::Fleeing { left: ticks(FLEE_FOR) } } else { Mood::Ducking { left: left - 1 } },
            Mood::Panicking { left, swerve, next, going, stopped } => {
                let going = going || car.speed > PANIC_GOING;
                let stopped = if going && car.speed.abs() < 1.0 { stopped + 1 } else { 0 };
                if stopped > ticks(PANIC_STOPPED) {
                    // It has hit something: out and away, if they can.
                    self.abandon(i, world);
                } else if left <= 1 || !car.physical {
                    self.cars[i].mood = Mood::Fleeing { left: ticks(FLEE_FOR) };
                } else {
                    // The wheel thrown over, one way or the other.
                    let (swerve, next) = if next == 0 {
                        let far = PANIC_SWERVE[0] + (PANIC_SWERVE[1] - PANIC_SWERVE[0]) * self.rng.next_f64() as f32;
                        let side = if self.rng.next_u64().is_multiple_of(2) { 1.0 } else { -1.0 };
                        (far * side, ticks(PANIC_THROW[0] + (PANIC_THROW[1] - PANIC_THROW[0]) * self.rng.next_f64() as f32))
                    } else {
                        (swerve, next - 1)
                    };
                    self.cars[i].mood = Mood::Panicking { left: left - 1, swerve, next, going, stopped };
                }
            }
            Mood::Runaway { left, steer, throttle, stopped } => {
                let stopped = if car.speed.abs() < 1.0 { stopped + 1 } else { 0 };
                if left <= 1 || stopped > TICK_RATE || !car.physical {
                    self.abandon(i, world);
                } else {
                    self.cars[i].mood = Mood::Runaway { left: left - 1, steer, throttle, stopped };
                }
            }
            Mood::Ramming { left, at } => {
                // Past them, or they are gone, or time is up: it drives off.
                let target = self.target(world, at);
                let pose = car.vid.and_then(|vid| world.physics.car_pose(vid));
                let past = match (target, pose) {
                    (Some(target), Some(pose)) => (target[0] - pose.position[0]) * -pose.yaw().sin() + (target[2] - pose.position[2]) * -pose.yaw().cos() < 0.0,
                    _ => true,
                };
                self.cars[i].mood = if left <= 1 || past || !car.physical { Mood::Fleeing { left: ticks(FLEE_FOR) } } else { Mood::Ramming { left: left - 1, at } };
            }
        }
    }

    /// Where player `at` is, if they are alive and on foot: what a driver who rams steers at.
    pub(super) fn target(&self, world: &World, at: u32) -> Option<[f32; 3]> {
        world.entities.get::<Player>(at).filter(|player| world.entities.alive(at) && player.seat.is_none()).map(|player| player.character.position)
    }
}
