//! The people at the wheel of the traffic's cars (TRAFFIC.md).
//!
//! A driver is a light entity, not a player: a kind, a place and some health, with no body in the
//! physics and nobody's input. There is one for every car of the traffic near enough a player to
//! see into, sitting at its wheel. The car's seat stays empty for all the server's vehicles know,
//! so a player who boards it takes the wheel, and the driver is put out on the road.
//!
//! A driver who has a gun pointed at their car, or whose car is shot, does one of three things by
//! their nerve: gives the car up, drives off as fast as it goes, or drives at whoever it was.
//! On foot a driver runs from what frightened them until nobody is near enough to see them go.

use super::{LOOK_EVERY, PULL_OVER_FOR, Traffic, sound, ticks};
use crate::entities::{Component, Health, HurtBy, Hurtbox, NetKind, Transform, With};
use crate::game::{Player, World};
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
/// How near a gun must be for a driver to see it pointed at them, metres, and how long it must
/// stay on their car before they do something about it, seconds.
const AIM_WITHIN: f32 = 30.0;
const AIM_FOR: f32 = 0.5;
/// How long a driver who flees drives like it, seconds.
const FLEE_FOR: f32 = 20.0;
/// How near and how long a driver goes for the one who threatened them, metres and seconds.
const RAM_WITHIN: f32 = 40.0;
const RAM_FOR: f32 = 4.0;
/// A fleeing car that has stood this long is boxed in, seconds: its driver gets out and runs.
const BOXED_IN: f32 = 3.0;
/// How long a driver who gave the car up stands beside it with their hands up, seconds.
const HANDS_UP: f32 = 2.0;
/// How fast a driver runs on foot, m/s.
const RUN_SPEED: f32 = 5.0;
/// A driver on foot is gone once no player is within this of them, metres, or after this long,
/// seconds; a dead one lies this long.
const AFOOT_UNSEEN: f32 = 150.0;
const AFOOT_MAX: f32 = 120.0;
const LIES: f32 = 10.0;
/// How far ahead a running driver looks for something in their way, metres, and how much they
/// turn aside for it, radians.
const WAY_AHEAD: f32 = 2.5;
const TURN_ASIDE: f32 = 0.6;
/// How far above the middle of a driver on foot a hit on them is shown, metres: a soldier's chest.
const CHEST_Y: f32 = 0.9;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mood {
    /// Driving as traffic does.
    Calm,
    /// Stopping, to get out with their hands up.
    GivingUp,
    /// Driving off as fast as the car goes, for `left` more ticks.
    Fleeing { left: u32 },
    /// Driving at player `at`, for `left` more ticks.
    Ramming { left: u32, at: u32 },
    /// Taking a car too damaged to drive on off the road, onto the verge, for `left` more ticks
    /// at most.
    PullingOver { left: u32 },
}

/// What a round can meet of a driver: a soldier's body, standing on their feet when they are on
/// foot, and as a rider's when they sit at the wheel (`life::body`).
fn body() -> Hurtbox {
    Hurtbox { centre_y: sim::STANDING_Y, half_height: CAPSULE_HALF_HEIGHT, radius: HURT_RADIUS, head: Some(HEAD_ZONE_Y), mark_y: CHEST_Y, hit_kind: shared::protocol::HIT_BODY }
}

/// A driver of the traffic. A component on their own entity.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Driver {
    /// The vehicle whose wheel they sit at; `None` once they are on foot.
    pub vehicle: Option<u32>,
    /// On foot: ticks left of standing with their hands up before they run.
    pub hands_up: u32,
    /// Ticks they have been on foot, and ticks they have lain dead.
    afoot: u32,
    lain: u32,
    /// What they run from, `[x, z]`.
    threat: [f32; 2],
    /// How far they have turned aside from straight away for what is in their way, radians.
    turn: f32,
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
            world.entities.spawn(id, (NetKind(KIND_CIVILIAN), Transform { position, yaw: super::yaw_of(d) }, Health::full(PLAYER_MAX_HP), body(), Driver { vehicle: Some(vid), ..Default::default() }));
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

    /// The driver of car `i` gets out and is on foot from now on: with their hands up first if
    /// they gave the car up, and then running from what frightened them, or from whoever is
    /// nearest if nothing did. One shot dead at the wheel falls out and lies beside the car. A
    /// wreck's driver is gone with it.
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
        world.entities.insert(id, (Transform { position: [spot[0], feet, spot[2]], yaw: pose.yaw() }, Driver { vehicle: None, hands_up, threat, ..Default::default() },));
    }

    /// Whether a driver has been threatened: a player's gun has stayed on their car, or the car or
    /// they themselves have been hurt. They take it as their nerve has it. One shot dead at the wheel
    /// drives no further, and a car too damaged to drive on is taken off the road.
    pub(super) fn threats(&mut self, world: &mut World) {
        let gunmen: Vec<(u32, [f32; 3], [f32; 3])> = world
            .entities
            .all::<Player>()
            .filter(|&(id, player)| super::human(world, id) && world.entities.alive(id) && player.seat.is_none() && shared::items::def(player.held().item).is_some_and(|item| item.kind == ItemKind::Gun))
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
                self.abandon(i, world);
                continue;
            }
            if car.mood != Mood::Calm {
                continue;
            }
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let mut by: Option<(Option<u32>, [f32; 3])> = None;
            // Shot, and still alive: whoever did it is who they take it out on, or run from.
            let hurt = world.entities.get::<Health>(driver).map_or(0, |health| health.hp);
            if hurt < car.hurt {
                let shooter = world.entities.get::<HurtBy>(driver).map(|&HurtBy(shooter)| shooter).filter(|&shooter| self.target(world, shooter).is_some());
                by = Some((shooter, shooter.and_then(|shooter| self.target(world, shooter)).unwrap_or(pose.position)));
            }
            // Hurt since it was last looked at: whoever is nearest did it, for all the driver knows.
            let hp = world.entities.get::<Health>(vid).map_or(0, |health| health.hp);
            if hp < car.hp && by.is_none() {
                let nearest = self.viewers.iter().copied().min_by(|a, b| (a[0] - pose.position[0]).hypot(a[2] - pose.position[2]).total_cmp(&(b[0] - pose.position[0]).hypot(b[2] - pose.position[2])));
                by = Some((None, nearest.unwrap_or(pose.position)));
            }
            let aimed = if car.nearest < AIM_WITHIN + 5.0 { gunmen.iter().find(|&&(_, eye, dir)| aims(world, eye, dir, &pose, car.kind)) } else { None };
            let car = &mut self.cars[i];
            (car.hp, car.hurt) = (hp, hurt);
            car.aimed = if aimed.is_some() { car.aimed + LOOK_EVERY } else { 0 };
            if let Some(&(who, eye, _)) = aimed.filter(|_| car.aimed >= ticks(AIM_FOR)) {
                by = Some((Some(who), eye));
            }
            if let Some((who, at)) = by {
                // Ahead of the car and near: one with the nerve for it drives at them.
                let (to, forward) = ([at[0] - pose.position[0], at[2] - pose.position[2]], [-pose.yaw().sin(), -pose.yaw().cos()]);
                let reach = to[0].hypot(to[1]);
                let ahead = reach < RAM_WITHIN && to[0] * forward[0] + to[1] * forward[1] > 0.5 * reach;
                car.threat = [at[0], at[2]];
                car.aimed = 0;
                car.boxed = 0;
                car.mood = match (car.nerve, who) {
                    (Nerve::GivesUp, _) => Mood::GivingUp,
                    (Nerve::Rams, Some(who)) if car.physical && ahead => Mood::Ramming { left: ticks(RAM_FOR), at: who },
                    _ => Mood::Fleeing { left: ticks(FLEE_FOR) },
                };
                tracing::debug!(car = i, mood = ?car.mood, "a driver is threatened");
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

/// One tick of every driver who is on foot: they stand with their hands up, then run from what
/// frightened them, round what is in their way, and are gone when nobody is near enough to see.
pub(super) fn afoot(world: &mut World, viewers: &[[f32; 3]], dt: f32) {
    for id in world.entities.ids::<With<Driver>>() {
        let (mut driver, mut transform) = (*world.entities.of::<Driver>(id), *world.entities.of::<Transform>(id));
        if driver.vehicle.is_some() {
            continue;
        }
        let [x, y, z] = transform.position;
        let unseen = viewers.iter().all(|v| (v[0] - x).hypot(v[2] - z) > AFOOT_UNSEEN);
        let inside = world.map.is_inside((x * 100.0) as i32, (z * 100.0) as i32);
        if world.entities.dead(id) {
            driver.lain += 1;
        } else {
            driver.afoot += 1;
        }
        if driver.lain > ticks(LIES) || unseen || !inside || driver.afoot > ticks(AFOOT_MAX) {
            world.entities.despawn(id);
            continue;
        }
        if world.entities.dead(id) {
            // Lying where they fell.
        } else if driver.hands_up > 0 {
            driver.hands_up -= 1;
        } else {
            // Straight away from it, turned aside for as long as something is in the way.
            let away = [x - driver.threat[0], z - driver.threat[1]];
            let yaw = (-away[0]).atan2(-away[1]) + driver.turn;
            let forward = [-yaw.sin(), -yaw.cos()];
            if world.physics.cast_ray([x, y + 1.0, z], [forward[0], 0.0, forward[1]], WAY_AHEAD, 0).is_some() {
                driver.turn += if driver.turn < 0.0 { -TURN_ASIDE } else { TURN_ASIDE };
            } else {
                driver.turn *= 0.98;
                let (nx, nz) = (x + forward[0] * RUN_SPEED * dt, z + forward[1] * RUN_SPEED * dt);
                // On whatever is underfoot: the ground, or a bridge's deck.
                let floor = world.physics.cast_ray([nx, y + 1.2, nz], [0.0, -1.0, 0.0], 3.0, 0).map_or(world.map.ground.height_at(nx, nz), |fall| y + 1.2 - fall);
                transform.position = [nx, floor, nz];
            }
            transform.yaw = yaw;
        }
        *world.entities.of_mut::<Driver>(id) = driver;
        *world.entities.of_mut::<Transform>(id) = transform;
    }
}
