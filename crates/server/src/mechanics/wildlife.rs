use super::Mechanic;
use super::life::{open_ground, place_at_home};
use super::npcs::{pick_target, random_between, turn_towards};
use crate::entities::{Component, Health, Home, Hurtbox, NetKind, Respawn, Respawned, Transform, With};
use crate::game::{Damaged, Died, Noise, Player, World};
use bevy_ecs::message::MessageCursor;
use shared::consts::*;
use shared::protocol::{HIT_DEER, KIND_DEER, KIND_STAG};
use shared::quant::from_cm;
use std::f32::consts::{PI, TAU};

/// Wild animals: herds of deer the map editor placed (`WorldFile::placed_animals`). They graze about
/// their pasture, look up at someone who comes near and bolt from someone who comes nearer, from a
/// gunshot or a blast they hear, and from being hurt. One that bolts takes its herd with it, and where
/// they stop is their pasture until they have grazed their way back home. Being shot, run over, dying
/// and coming back are `Life`'s, from their components, as for the rooster; unlike the rooster, a dead
/// one lies where it fell (`Fell`) until it comes back.
#[derive(Default)]
pub struct Wildlife {
    /// How far through the noises they have listened.
    heard: MessageCursor<Noise>,
}

/// They look about every this many ticks (10 a second), each on its own beat.
const LOOK_EVERY: u32 = 6;
/// Grazing in one spot lasts this long, ticks, and one in this many ends in a few steps elsewhere.
const GRAZE_MIN_TICKS: u32 = 4 * TICK_RATE;
const GRAZE_SPAN_TICKS: u32 = 6 * TICK_RATE;
/// How long it stands and stares at what it noticed, ticks.
const ALERT_TICKS: u32 = 2 * TICK_RATE;
/// A walk or a flight that has not got there by now is given up, ticks.
const WALK_TIMEOUT_TICKS: u32 = 15 * TICK_RATE;
const FLEE_TIMEOUT_TICKS: u32 = 14 * TICK_RATE;
/// Each time it has grazed, its pasture moves this far back towards home, metres.
const HOMING: f32 = 4.0;
/// A herd-mate this near the one that bolts goes too, after a moment (up to `STARTLE_SPAN` ticks).
const HERD_REACH: f32 = 2.0 * DEER_HERD_RADIUS;
const STARTLE_SPAN: u64 = 24;
/// Herd-mates run to the first one's spot give or take this, metres.
const HERD_SPREAD: f32 = 8.0;
/// Its flight goes this much to either side of straight away, radians, and these are the turns it
/// tries when that way is shut.
const FLEE_JITTER: f32 = 0.35;
const FLEE_TURNS: [f32; 7] = [0.0, 0.6, -0.6, 1.2, -1.2, 1.8, -1.8];
/// It checks its line of flight for water every this many metres.
const WET_STEP: f32 = 5.0;
/// A wall this far ahead (at body height) ends a walk or turns a flight, metres.
const WALL_AHEAD: f32 = 0.8;
const GALLOP_WALL_AHEAD: f32 = 2.0;
/// It gallops at full speed only once facing within this of its way, radians; turning, it slows to a trot.
const GALLOP_WITHIN: f32 = 1.0;
const TROT: f32 = 0.3;
const ARRIVED: f32 = 0.5;
/// How readily it notices someone who stands still (upright), as a share of the walking distance.
const STILL_SHARE: f32 = 0.6;
/// Someone in a vehicle is noticed at the running distance times this.
const VEHICLE_SHARE: f32 = 1.5;
/// Someone low and moving is noticed at the sneaking distance times this.
const CREEP_SHARE: f32 = 1.6;

/// What it is doing. `mode_left` counts ticks down to the next decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WildMode {
    /// Head down cropping grass (`FLAG_GRAZING`).
    Graze,
    /// A few steps to fresh grass.
    Walk,
    /// Head up, turned to stare at `threat`.
    Alert,
    /// Galloping away from `threat`.
    Flee,
}

/// The mind of a wild animal. `Home` is its own spot in its herd, where it comes back after dying.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Wild {
    /// Its noticing distances are the hind's times this: the stag lets people closer.
    pub nerve: f32,
    pub herd: u32,
    pub mode: WildMode,
    pub mode_left: u32,
    /// Where it is walking or running to, `[x, z]`.
    pub target: [f32; 2],
    /// Where it grazes about: home, or where its last flight ended on the way back.
    pub pasture: [f32; 2],
    /// What it stares at or runs from, `[x, z]`.
    pub threat: [f32; 2],
    /// Ticks until it bolts after a herd-mate, to `target`; zero when it is not about to.
    pub startle: u32,
}

/// How a dead wild animal was killed (`DEATH_*`): it lies where it fell, drawn so, until it respawns.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fell(pub u8);

/// A wild animal of `kind` with its spot at `home`, in `herd`, grazing about `pasture`.
pub fn animal(kind: u8, home: [f32; 2], herd: u32, pasture: [f32; 2]) -> impl crate::entities::Bundle {
    let stag = kind == KIND_STAG;
    let (hp, centre, half_height, radius) =
        if stag { (STAG_HP, STAG_CAPSULE_CENTRE, STAG_CAPSULE_HALF_HEIGHT, STAG_CAPSULE_RADIUS) } else { (DEER_HP, DEER_CAPSULE_CENTRE, DEER_CAPSULE_HALF_HEIGHT, DEER_CAPSULE_RADIUS) };
    (
        NetKind(if stag { KIND_STAG } else { KIND_DEER }),
        Transform { position: [home[0], 0.0, home[1]], yaw: 0.0 },
        Health::full(hp),
        Hurtbox { centre_y: centre, half_height, radius, head: None, mark_y: half_height, hit_kind: HIT_DEER },
        Home(home),
        Respawn { delay: DEER_RESPAWN_TICKS, left: 0 },
        Wild { nerve: if stag { STAG_NERVE } else { 1.0 }, herd, mode: WildMode::Graze, mode_left: GRAZE_MIN_TICKS, target: home, pasture, threat: home, startle: 0 },
        crate::entities::Trail::default(),
    )
}

/// How entity `id` died, if it is a wild animal lying dead.
pub fn fell(world: &World, id: u32) -> Option<u8> {
    world.entities.get::<Fell>(id).map(|fell| fell.0)
}

/// Whether entity `id` is grazing, which the snapshot shows as `FLAG_GRAZING`.
pub fn grazing(world: &World, id: u32) -> bool {
    world.entities.get::<Wild>(id).is_some_and(|wild| wild.mode == WildMode::Graze)
}

fn distance([ax, az]: [f32; 2], [bx, bz]: [f32; 2]) -> f32 {
    (bx - ax).hypot(bz - az)
}

fn wet(world: &World, [x, z]: [f32; 2]) -> bool {
    world.map.wet((x * 100.0) as i32, (z * 100.0) as i32)
}

/// Open, dry ground at `to`, and no water on the straight way there from `from`.
fn dry_way(world: &World, from: [f32; 2], to: [f32; 2]) -> bool {
    if open_ground(world, to[0], to[1]).is_none() {
        return false;
    }
    let steps = (distance(from, to) / WET_STEP).ceil() as usize;
    (1..=steps).all(|step| {
        let t = step as f32 / steps as f32;
        !wet(world, [from[0] + (to[0] - from[0]) * t, from[1] + (to[1] - from[1]) * t])
    })
}

/// Somewhere about `DEER_FLEE_DISTANCE` from `at`, away from `threat`: straight away give or take a
/// little, else turned off to one side or the other, else half as far.
fn flee_target(world: &mut World, at: [f32; 2], threat: [f32; 2]) -> [f32; 2] {
    let away = if distance(at, threat) < 0.1 { random_between(world, 0.0, TAU) } else { (at[1] - threat[1]).atan2(at[0] - threat[0]) };
    let jitter = random_between(world, -FLEE_JITTER, FLEE_JITTER);
    for reach in [DEER_FLEE_DISTANCE, DEER_FLEE_DISTANCE / 2.0] {
        for turn in FLEE_TURNS {
            let angle = away + jitter + turn;
            let to = [at[0] + angle.cos() * reach, at[1] + angle.sin() * reach];
            if dry_way(world, at, to) {
                return to;
            }
        }
    }
    [at[0] + away.cos() * 10.0, at[1] + away.sin() * 10.0]
}

fn set(world: &mut World, id: u32, wild: Wild) {
    if let Some(mut slot) = world.entities.get_mut::<Wild>(id) {
        *slot = wild;
    }
}

fn graze(world: &mut World, wild: &mut Wild) {
    wild.mode = WildMode::Graze;
    wild.mode_left = GRAZE_MIN_TICKS + (world.rng.next_u64() % GRAZE_SPAN_TICKS as u64) as u32;
}

/// Animal `id` bolts from `threat`, and the rest of its herd nearby after it, to about where it runs.
pub fn bolt(world: &mut World, id: u32, threat: [f32; 2]) {
    let (Some(mut wild), Some(transform)) = (world.entities.get::<Wild>(id).copied(), world.entities.get::<Transform>(id).copied()) else { return };
    if wild.mode == WildMode::Flee || !world.entities.alive(id) {
        return;
    }
    let at = [transform.position[0], transform.position[2]];
    wild.mode = WildMode::Flee;
    wild.mode_left = FLEE_TIMEOUT_TICKS;
    wild.threat = threat;
    wild.startle = 0;
    wild.target = flee_target(world, at, threat);
    set(world, id, wild);
    tracing::debug!(animal = id, ?threat, "animal bolted");
    for mate in world.entities.ids::<With<Wild>>() {
        let (Some(mut other), Some(place)) = (world.entities.get::<Wild>(mate).copied(), world.entities.get::<Transform>(mate).copied()) else { continue };
        let near = distance(at, [place.position[0], place.position[2]]) < HERD_REACH;
        if mate == id || other.herd != wild.herd || other.mode == WildMode::Flee || other.startle > 0 || !near || !world.entities.alive(mate) {
            continue;
        }
        let angle = random_between(world, 0.0, TAU);
        let spread = random_between(world, 0.0, HERD_SPREAD);
        let to = [wild.target[0] + angle.cos() * spread, wild.target[1] + angle.sin() * spread];
        other.target = if dry_way(world, [place.position[0], place.position[2]], to) { to } else { wild.target };
        other.threat = threat;
        other.startle = 1 + (world.rng.next_u64() % STARTLE_SPAN) as u32;
        set(world, mate, other);
    }
}

/// How near `player` comes before an animal with `nerve` notices them, metres, by how they go about.
fn notice_distance(world: &World, id: u32, player: &Player, nerve: f32) -> f32 {
    let walking = player.input.forward != 0 || player.input.strafe != 0;
    let running = walking && player.input.buttons() & BUTTON_RUN != 0;
    let low = player.character.prone || player.crouched();
    let distance = if world.seated_head(id).is_some() {
        DEER_NOTICE_RUN * VEHICLE_SHARE
    } else if low {
        DEER_NOTICE_SNEAK * if walking { CREEP_SHARE } else { 1.0 }
    } else if running {
        DEER_NOTICE_RUN
    } else if walking {
        DEER_NOTICE_WALK
    } else {
        DEER_NOTICE_WALK * STILL_SHARE
    };
    distance * nerve
}

/// The player that worries an animal at `at` most, where they are, and how near they are as a share
/// of the distance it notices them at: below 1 it has noticed them.
fn worry(world: &World, at: [f32; 2], nerve: f32) -> Option<([f32; 2], f32)> {
    let mut worst: Option<([f32; 2], f32)> = None;
    for (other, player) in world.entities.all::<Player>() {
        if !world.entities.alive(other) {
            continue;
        }
        let place = match world.seated_head(other) {
            Some((_, head)) => head,
            None if player.embodied() => player.character.position,
            None => continue,
        };
        let place = [place[0], place[2]];
        let share = distance(at, place) / notice_distance(world, other, player, nerve);
        if share < 1.0 && worst.is_none_or(|(_, least)| share < least) {
            worst = Some((place, share));
        }
    }
    worst
}

fn step(world: &mut World, id: u32, noises: &[Noise], dt: f32) {
    let (Some(mut wild), Some(mut transform), Some(Home(home))) =
        (world.entities.get::<Wild>(id).copied(), world.entities.get::<Transform>(id).copied(), world.entities.get::<Home>(id).copied())
    else {
        return;
    };
    let [x, y, z] = transform.position;
    let at = [x, z];

    // Bolting after a herd-mate.
    if wild.startle > 0 {
        wild.startle -= 1;
        if wild.startle == 0 {
            wild.mode = WildMode::Flee;
            wild.mode_left = FLEE_TIMEOUT_TICKS;
            set(world, id, wild);
            return;
        }
    }
    if wild.mode != WildMode::Flee && wild.startle == 0 {
        if let Some(noise) = noises.iter().find(|noise| distance(at, [noise.at[0], noise.at[2]]) < noise.reach) {
            bolt(world, id, [noise.at[0], noise.at[2]]);
            return;
        }
        if (world.tick + id).is_multiple_of(LOOK_EVERY)
            && let Some((place, share)) = worry(world, at, wild.nerve)
        {
            if share < DEER_BOLT_SHARE {
                bolt(world, id, place);
                return;
            }
            if wild.mode != WildMode::Alert {
                tracing::trace!(animal = id, "animal looked up");
            }
            wild.mode = WildMode::Alert;
            wild.mode_left = wild.mode_left.max(ALERT_TICKS);
            wild.threat = place;
        }
    }

    wild.mode_left = wild.mode_left.saturating_sub(1);
    let face = |transform: &mut Transform, to: [f32; 2], rate: f32| {
        let want = (-(to[0] - x)).atan2(-(to[1] - z));
        transform.yaw = turn_towards(transform.yaw, want, rate * dt);
        (turn_towards(transform.yaw, want, PI) - transform.yaw).abs()
    };
    match wild.mode {
        WildMode::Graze => {
            if wild.mode_left == 0 {
                // A little nearer home each time, so a scattered herd finds its way back.
                let back = distance(wild.pasture, home);
                if back > 0.1 {
                    let k = (HOMING / back).min(1.0);
                    wild.pasture = [wild.pasture[0] + (home[0] - wild.pasture[0]) * k, wild.pasture[1] + (home[1] - wild.pasture[1]) * k];
                }
                match pick_target(world, wild.pasture, DEER_HERD_RADIUS).filter(|&to| dry_way(world, at, to)) {
                    Some(to) => {
                        wild.mode = WildMode::Walk;
                        wild.mode_left = WALK_TIMEOUT_TICKS;
                        wild.target = to;
                    }
                    None => graze(world, &mut wild),
                }
            }
        }
        WildMode::Alert => {
            face(&mut transform, wild.threat, DEER_TURN_RATE);
            if wild.mode_left == 0 {
                graze(world, &mut wild);
            }
        }
        WildMode::Walk | WildMode::Flee => {
            let fleeing = wild.mode == WildMode::Flee;
            let (speed, rate, ahead) = if fleeing { (DEER_GALLOP_SPEED, DEER_FLEE_TURN_RATE, GALLOP_WALL_AHEAD) } else { (DEER_WALK_SPEED, DEER_TURN_RATE, WALL_AHEAD) };
            let left = distance(at, wild.target);
            let off = face(&mut transform, wild.target, rate);
            let forward = [-transform.yaw.sin(), -transform.yaw.cos()];
            let body = world.entities.get::<Hurtbox>(id).map_or(1.0, |hurtbox| hurtbox.centre_y);
            let blocked = off < GALLOP_WITHIN && world.physics.cast_ray_hit([x, y + body, z], [forward[0], 0.0, forward[1]], ahead, 0).is_some();
            if left < ARRIVED || wild.mode_left == 0 || (blocked && !fleeing) {
                if fleeing {
                    // It stops to look back at what it ran from, and grazes here for now.
                    wild.mode = WildMode::Alert;
                    wild.mode_left = ALERT_TICKS;
                    wild.pasture = at;
                } else {
                    graze(world, &mut wild);
                }
            } else if blocked {
                // A wall across its flight: off along it, away from the threat.
                wild.target = flee_target(world, at, [x - forward[0] * 5.0, z - forward[1] * 5.0]);
            } else {
                // A walk sets off only facing its way; a gallop slows to a trot round a turn.
                let pace = match (fleeing, off < GALLOP_WITHIN) {
                    (_, true) => 1.0,
                    (true, false) => TROT,
                    (false, false) => 0.0,
                };
                let stride = (speed * pace * dt).min(left);
                let (nx, nz) = (x + forward[0] * stride, z + forward[1] * stride);
                if stride > 0.0 && open_ground(world, nx, nz).is_some() && !wet(world, [nx, nz]) {
                    transform.position = [nx, world.map.ground.height_at(nx, nz), nz];
                } else if stride > 0.0 {
                    wild.mode_left = 0;
                }
            }
        }
    }
    set(world, id, wild);
    if let Some(mut slot) = world.entities.get_mut::<Transform>(id) {
        *slot = transform;
    }
}

impl Mechanic for Wildlife {
    fn on_start(&mut self, world: &mut World) {
        for (herd, placed) in world.map.placed_animals.clone().into_iter().enumerate() {
            let centre = placed.at.map(from_cm);
            for _ in 0..placed.count {
                // Each its own spot about the herd's, on open ground if there is any near.
                let home = pick_target(world, centre, DEER_HERD_RADIUS / 2.0).unwrap_or(centre);
                let id = world.alloc_id();
                world.entities.spawn(id, animal(placed.kind, home, herd as u32, centre));
                place_at_home(world, id);
                world.entities.remove::<Respawned>(id);
                world.entities.get_mut::<Transform>(id).expect("just placed").yaw = random_between(world, -PI, PI);
            }
            tracing::info!(kind = placed.kind, count = placed.count, x = centre[0], z = centre[1], "herd placed");
        }
    }

    /// An animal hurt bolts from where it came from; so does one whose herd-mate was. One killed lies
    /// where it fell, as it was killed.
    fn react(&mut self, world: &mut World) {
        let fallen: Vec<(u32, u8)> = world.happened::<Died>().filter(|died| world.entities.has::<Wild>(died.victim)).map(|died| (died.victim, died.style)).collect();
        for (id, style) in fallen {
            world.entities.insert(id, Fell(style));
        }
        let hurt: Vec<(u32, [f32; 2])> = world
            .happened::<Damaged>()
            .filter(|damaged| world.entities.get::<Wild>(damaged.victim).is_some())
            .map(|damaged| (damaged.victim, damaged.from.map_or([damaged.at[0], damaged.at[2]], |from| [from[0], from[2]])))
            .collect();
        for (id, from) in hurt {
            if world.entities.alive(id) {
                bolt(world, id, from);
                continue;
            }
            // Dead: its herd-mates nearby run in its stead.
            let (Some(wild), Some(transform)) = (world.entities.get::<Wild>(id).copied(), world.entities.get::<Transform>(id).copied()) else { continue };
            let at = [transform.position[0], transform.position[2]];
            let mate = world.entities.ids::<With<Wild>>().into_iter().find(|&mate| {
                mate != id
                    && world.entities.alive(mate)
                    && world.entities.get::<Wild>(mate).is_some_and(|other| other.herd == wild.herd)
                    && world.entities.get::<Transform>(mate).is_some_and(|place| distance(at, [place.position[0], place.position[2]]) < HERD_REACH)
            });
            if let Some(mate) = mate {
                bolt(world, mate, from);
            }
        }
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let noises: Vec<Noise> = self.heard.read(world.events::<Noise>()).copied().collect();
        for id in world.entities.ids::<With<Wild>>() {
            // Back from the dead this tick (`Life` put it home): it grazes there, calm and whole.
            if world.entities.get::<Respawned>(id).is_some() {
                world.entities.remove::<Respawned>(id);
                world.entities.remove::<Fell>(id);
                world.entities.remove::<super::Struck>(id);
                if let (Some(mut wild), Some(Home(home))) = (world.entities.get::<Wild>(id).copied(), world.entities.get::<Home>(id).copied()) {
                    wild.pasture = home;
                    wild.startle = 0;
                    graze(world, &mut wild);
                    set(world, id, wild);
                }
                continue;
            }
            if world.entities.alive(id) {
                step(world, id, &noises, dt);
            }
        }
    }
}

#[cfg(test)]
mod tests;
