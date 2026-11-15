//! What a Titan's fists and feet do (VEHICLES-XIV.md). The physics moves them (`sim`'s `limbs`): this
//! passes its pilot's hands on to it, and makes what they meet feel it.
//!
//! - **A punch.** A closed fist moving at `TITAN_PUNCH_SPEED` or more throws the soldiers it meets, hurting
//!   them by its speed, hurts the vehicles it meets (the physics shoves them), takes a bite out of a
//!   building or placed cubes, and fells trees. An open hand only shoves, as any solid does.
//! - **A grip.** A hand that closes on a soldier holds them: they ride on the fist, free to look about
//!   and shoot, until it opens, and then fly off as fast as it was moving. One that closes on a vehicle
//!   no heavier than `TITAN_LIFT_MASS` carries it off the same way.
//! - **A step.** Where a foot comes down, soldiers under it are crushed and those around it are thrown
//!   off their feet; vehicles, buildings, cubes and trees under it are hurt.
//! - **A slam.** Come down out of the air at `TITAN_SLAM_SPEED` or faster, it lands as a blast would,
//!   wider and harder the faster it fell: soldiers are thrown and hurt, vehicles thrown and broken,
//!   buildings bitten down and trees felled.
//! - **Its head** turns to where its pilot looks, as far as its neck allows (`Head`), and the searchlight
//!   in its visor goes on and off at its pilot's `BUTTON_LAMP`.
//!
//! Its guns (the autocannon in its right fist and the missile tubes on its shoulder) are mounts, worked
//! by `Gunnery`; the hand that holds the gun never grips.

use super::Mechanic;
use super::life::{Harm, damage_entity};
use crate::entities::{Component, With};
use crate::game::{Fist, Player, Seats, Vehicle, World};
use shared::consts::*;
use shared::mounts::Muzzle;
use shared::protocol::{ClientHands, HANDS_GRIP_LEFT, HANDS_GRIP_RIGHT, HANDS_REACH_LEFT, HANDS_REACH_RIGHT, WEAPON_TITAN};
use shared::quant::{from_cm, from_mrad};
use shared::vec3;
use shared::vehicles::vehicle_def;
use sim::{Hand, LimbsPose, Obj};
use std::collections::BTreeMap;

/// A pilot who has sent no hands for this many ticks has let go of the controls: the arms hang.
const HANDS_STALE_TICKS: u32 = TICK_RATE / 2;
/// How far beyond a closing fist's own radius a soldier can be and still be picked up, metres.
const GRAB_SLACK: f32 = 1.5;
/// How far beyond a moving fist's radius a soldier can be and still be hit, metres: about their own breadth.
const PUNCH_SLACK: f32 = 0.6;
/// How many ticks ahead of itself a fist hits what is in its way.
const PUNCH_AHEAD: f32 = 2.0;
/// A fist hurts the same vehicle, building or tree again only after this many ticks.
const STRIKE_COOLDOWN: u32 = TICK_RATE / 3;
/// A soldier a fist has thrown cannot be hit again for this many ticks.
const HIT_COOLDOWN: u32 = 20;
/// The upward speed a soldier hit or shaken off their feet gets on top of the blow's own, m/s.
const HOP: f32 = 3.0;
/// The fastest a soldier is thrown along the ground, m/s.
const THROW_MAX_SPEED: f32 = 45.0;
/// Let go slower than this, m/s, a soldier is only put down; faster, they are hurt by
/// `THROW_DAMAGE` for every m/s over it.
const THROW_SAFE_SPEED: f32 = 12.0;
const THROW_DAMAGE: f32 = 3.0;
/// How far a foot's crushing reaches around its middle, metres, and how high over its underside.
const CRUSH_RADIUS: f32 = 3.6;
const CRUSH_HEIGHT: f32 = 4.0;
/// How hard a foot coming down rams what is under it, m/s, as `ram_building` counts it.
const STOMP_DV: f32 = 6.0;
/// Tree damage per m/s of a fist: any punch fells a tree.
const TREE_DAMAGE_PER_SPEED: f32 = 60.0;
/// The share of the speed a slam throws a vehicle outward with that it also throws it up with.
const SLAM_TOSS: f32 = 0.6;
/// The share of a fist's speed another Titan it lands on is knocked by.
const GIANT_KNOCK: f32 = 0.06;

/// What a Titan's hands were doing last tick.
#[derive(Default)]
struct Hands {
    closed: [bool; 2],
    /// Ticks until each fist can hurt a vehicle or the scenery again.
    cooling: [u32; 2],
}

/// How a Titan's head is turned from its hull, radians: a component on its entity. It follows its
/// pilot's look as far as the neck allows (`TITAN_HEAD_TURN` either side, `TITAN_HEAD_DOWN` and
/// `TITAN_HEAD_UP`), and faces ahead when nobody pilots it. The searchlight in its visor shines the
/// way the head faces.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Head {
    /// + turns left.
    pub yaw: f32,
    /// + looks up.
    pub pitch: f32,
    /// The searchlight is on. It goes out when the pilot leaves.
    pub lamp: bool,
    /// The pilot's `BUTTON_LAMP` was down last tick: holding it switches the searchlight once.
    lamp_held: bool,
}

/// A tick of the head of Titan `vid` turning towards its `pilot`'s look, seen from the hull, so it
/// keeps its line while the hull leans and turns, and of its pilot switching its searchlight. The press
/// is taken from the pilot's input: it is not their own headlamp's too (`Movement`, which runs after).
fn turn_head(world: &mut World, vid: u32, pilot: Option<u32>, dt: f32) {
    let input = pilot.and_then(|pilot| world.entities.get_mut::<Player>(pilot)).map(|mut player| {
        let input = player.input;
        player.input.buttons &= !BUTTON_LAMP;
        input
    });
    let pressed = input.is_some_and(|input| input.buttons() & BUTTON_LAMP != 0);
    let look = input.map(|input| super::combat::look(from_mrad(input.yaw), from_mrad(input.pitch()).clamp(-super::combat::MAX_PITCH, super::combat::MAX_PITCH)));
    let (yaw, pitch) = match (look, world.physics.car_pose(vid)) {
        (Some(look), Some(pose)) => super::gunnery::angles(pose.local_dir(look)),
        _ => (0.0, 0.0),
    };
    if !world.entities.has::<Head>(vid) {
        world.entities.insert(vid, Head::default());
    }
    let mut head = world.entities.of_mut::<Head>(vid);
    head.lamp = pilot.is_some() && (head.lamp ^ (pressed && !head.lamp_held));
    head.lamp_held = pressed;
    let step = TITAN_HEAD_SLEW * dt;
    head.yaw += (yaw.clamp(-TITAN_HEAD_TURN, TITAN_HEAD_TURN) - head.yaw).clamp(-step, step);
    head.pitch += (pitch.clamp(-TITAN_HEAD_DOWN, TITAN_HEAD_UP) - head.pitch).clamp(-step, step);
}

/// Works every Titan's fists, feet and head.
#[derive(Default)]
pub struct Titans {
    hands: BTreeMap<u32, Hands>,
}

/// What `hands` asks of the physics.
fn asked(hands: &ClientHands) -> [Hand; 2] {
    let at = |[x, y, z]: [i16; 3]| Some([i32::from(x), i32::from(y), i32::from(z)].map(from_cm));
    [
        Hand { target: (hands.flags & HANDS_REACH_LEFT != 0).then(|| at([hands.lx, hands.ly, hands.lz])).flatten(), closed: hands.flags & HANDS_GRIP_LEFT != 0 },
        Hand { target: (hands.flags & HANDS_REACH_RIGHT != 0).then(|| at([hands.rx, hands.ry, hands.rz])).flatten(), closed: hands.flags & HANDS_GRIP_RIGHT != 0 },
    ]
}

/// The soldier held in hand `hand` of Titan `vid`, if any.
pub fn held_in(world: &World, vid: u32, hand: usize) -> Option<u32> {
    world.entities.ids_where::<Player>(|_, player| player.held == Some(Fist { titan: vid, hand: hand as u8 })).into_iter().next()
}

/// Where a soldier held in a fist at `fist` is: standing on top of it, clear of its ball.
fn held_at(fist: [f32; 3]) -> [f32; 3] {
    [fist[0], fist[1] + TITAN_FIST_RADIUS + STAND_CENTRE, fist[2]]
}

/// Whether hand `hand` of vehicle `vid` holds a gun (`Muzzle::Fist`).
fn holds_gun(world: &World, vid: u32, hand: usize) -> bool {
    world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle_def(vehicle.kind).mounts.iter().any(|mount| mount.muzzle == Muzzle::Fist { hand }))
}

/// Throws soldier `id` with `velocity`, knocking them down if it is hard enough to.
fn throw(world: &mut World, id: u32, velocity: [f32; 3]) {
    let Some(mut player) = world.entities.get_mut::<Player>(id) else { return };
    let flat = velocity[0].hypot(velocity[2]);
    let scale = if flat > THROW_MAX_SPEED { THROW_MAX_SPEED / flat } else { 1.0 };
    let power = vec3::length(velocity);
    let player = &mut *player;
    let character = &mut player.character;
    character.knock = [velocity[0] * scale, velocity[2] * scale];
    character.vy = velocity[1];
    character.grounded = false;
    character.ride = 0;
    if power >= KNOCKDOWN_POWER {
        world.physics.knock_down(character, shared::movement::knockdown_ticks(power.min(KNOCK_MAX_SPEED)));
        player.interrupt();
    }
    player.hit_cooldown = HIT_COOLDOWN;
}

/// Hand `hand` of Titan `vid` lets go of whatever it holds, which flies on at the fist's `velocity`.
fn let_go(world: &mut World, vid: u32, hand: usize, velocity: [f32; 3], pilot: Option<u32>) {
    world.physics.release_car(vid, hand);
    let Some(id) = held_in(world, vid, hand) else { return };
    world.entities.of_mut::<Player>(id).held = None;
    throw(world, id, velocity);
    let speed = vec3::length(velocity);
    if speed > THROW_SAFE_SPEED {
        let amount = ((speed - THROW_SAFE_SPEED) * THROW_DAMAGE).min(255.0) as u8;
        damage_entity(world, id, Harm::new(amount, pilot, WEAPON_TITAN));
    }
    tracing::debug!(id, titan = vid, hand, speed, "let go of");
}

/// Hand `hand` of Titan `vid` closes at `fist`: on the nearest soldier in it, or else on a vehicle.
fn grab(world: &mut World, vid: u32, hand: usize, fist: [f32; 3]) {
    let reach = TITAN_FIST_RADIUS + GRAB_SLACK;
    let nearest = world
        .entities
        .ids_where::<Player>(|id, player| world.entities.alive(id) && player.embodied() && player.seat.is_none() && player.held.is_none() && player.lift.is_none())
        .into_iter()
        .map(|id| (vec3::distance(world.entities.of::<Player>(id).character.position, fist), id))
        .filter(|(distance, _)| *distance <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, id)) = nearest {
        let mut player = world.entities.of_mut::<Player>(id);
        player.held = Some(Fist { titan: vid, hand: hand as u8 });
        player.character.prone = false;
        player.character.skis = false;
        player.character.down = 0;
        tracing::debug!(id, titan = vid, hand, "picked up");
        return;
    }
    for car in world.physics.cars_touching_ball(fist, reach, vid) {
        if !world.entities.dead(car) && world.physics.grab_car(vid, hand, car) {
            tracing::debug!(car, titan = vid, hand, "vehicle picked up");
            return;
        }
    }
}

/// How far `point` is from where a fist at `fist` moving at `velocity` is over the next `PUNCH_AHEAD`
/// ticks, metres. The fist is solid, and shoves a soldier along ahead of it: it has hit them when it is
/// about to reach them, not once it is in them, which it never is.
fn off_path(fist: [f32; 3], velocity: [f32; 3], point: [f32; 3]) -> f32 {
    let travel = velocity.map(|v| v * PUNCH_AHEAD * TICK_DT);
    let length = vec3::dot(travel, travel);
    let along = if length > 1e-6 { (vec3::dot(vec3::sub(point, fist), travel) / length).clamp(0.0, 1.0) } else { 0.0 };
    vec3::distance(point, [fist[0] + travel[0] * along, fist[1] + travel[1] * along, fist[2] + travel[2] * along])
}

/// A fist of Titan `vid` at `fist`, moving at `velocity`, hits the soldiers it meets.
fn punch_soldiers(world: &mut World, vid: u32, fist: [f32; 3], velocity: [f32; 3], pilot: Option<u32>) {
    let speed = vec3::length(velocity);
    let reach = TITAN_FIST_RADIUS + PUNCH_SLACK;
    let hit = world.entities.ids_where::<Player>(|id, player| {
        world.entities.alive(id) && player.embodied() && player.seat.is_none() && player.hit_cooldown == 0 && player.held.is_none_or(|held| held.titan != vid) && off_path(fist, velocity, player.character.position) <= reach
    });
    for id in hit {
        // Whoever another Titan holds is knocked out of its hand.
        world.entities.of_mut::<Player>(id).held = None;
        throw(world, id, [velocity[0], velocity[1].max(0.0) + HOP, velocity[2]]);
        let amount = (speed * TITAN_PUNCH_DAMAGE).min(255.0) as u8;
        damage_entity(world, id, Harm::new(amount, pilot, WEAPON_TITAN).from(fist));
        tracing::debug!(id, titan = vid, speed, "punched");
    }
}

/// A fist or a foot of Titan `vid` at `point` hits the vehicles, buildings, cubes and trees within
/// `radius` of it, as hard as its `velocity`; `amount` is what a vehicle loses.
fn strike(world: &mut World, vid: u32, point: [f32; 3], radius: f32, velocity: [f32; 3], amount: f32, pilot: Option<u32>) {
    let speed = vec3::length(velocity);
    let weight = vehicle_def(KIND_TITAN).body.ram;
    for car in world.physics.cars_touching_ball(point, radius, vid) {
        if world.entities.dead(car) || world.physics.held_car(vid, 0) == Some(car) || world.physics.held_car(vid, 1) == Some(car) {
            continue;
        }
        // Another Titan does not feel a fist as a car does: it is knocked where the fist lands.
        if world.entities.get::<Vehicle>(car).is_some_and(|vehicle| vehicle_def(vehicle.kind).spec.arms.is_some()) {
            world.physics.knock_car(car, velocity.map(|v| v * GIANT_KNOCK), point);
        }
        // What it is made of is for `damage_vehicle` to weigh (`Body::tough`).
        super::combat::damage_vehicle(world, car, amount.min(255.0) as u8, pilot);
    }
    for object in world.physics.objects_touching_ball(point, radius) {
        match object {
            Obj::Building(id) => super::combat::ram_building(world, id, point, speed, weight),
            Obj::Tree(_) | Obj::Line(_) => {
                super::combat::damage_object(world, object, (speed * TREE_DAMAGE_PER_SPEED) as u16, point);
            }
        }
    }
    super::combat::ram_cubes(world, point, speed, weight);
}

/// A foot of Titan `vid` comes down at `foot` (its underside).
fn stomp(world: &mut World, vid: u32, foot: [f32; 3], pilot: Option<u32>) {
    let near = world.entities.ids_where::<Player>(|id, player| {
        let at = player.character.position;
        world.entities.alive(id) && player.embodied() && player.seat.is_none() && player.held.is_none() && (at[0] - foot[0]).hypot(at[2] - foot[2]) <= TITAN_STOMP_RADIUS && (at[1] - foot[1]).abs() <= CRUSH_HEIGHT
    });
    for id in near {
        let at = world.entities.of::<Player>(id).character.position;
        let away = [at[0] - foot[0], at[2] - foot[2]];
        let distance = away[0].hypot(away[1]);
        if distance <= CRUSH_RADIUS {
            damage_entity(world, id, Harm::new(255, pilot, WEAPON_TITAN).from(foot));
            continue;
        }
        if !world.entities.of::<Player>(id).character.grounded {
            continue;
        }
        // The ground jumps under them: thrown outwards, harder the nearer.
        let power = TITAN_STOMP_KNOCK * (1.0 - (distance - CRUSH_RADIUS) / (TITAN_STOMP_RADIUS - CRUSH_RADIUS));
        throw(world, id, [away[0] / distance * power, HOP, away[1] / distance * power]);
    }
    let middle = [foot[0], foot[1] + CRUSH_RADIUS * 0.5, foot[2]];
    strike(world, vid, middle, CRUSH_RADIUS, [0.0, -STOMP_DV, 0.0], TITAN_STOMP_VEHICLE_DAMAGE, pilot);
}

/// Titan `vid` comes down between its `feet` at `impact` m/s, hard enough to be a slam: a blast without
/// fire, wider and harder the faster it fell, which it does not feel itself. Vehicles are thrown further
/// than a blast's shove takes them.
fn slam(world: &mut World, vid: u32, feet: [[f32; 3]; 2], impact: f32, pilot: Option<u32>) {
    let power = ((impact - TITAN_SLAM_SPEED) / (TITAN_SLAM_FULL - TITAN_SLAM_SPEED)).clamp(0.0, 1.0);
    let radius = TITAN_SLAM_RADIUS + (TITAN_SLAM_RADIUS_FULL - TITAN_SLAM_RADIUS) * power;
    let knock = TITAN_SLAM_KNOCK * (0.5 + 0.5 * power);
    let point = [0, 1, 2].map(|axis| (feet[0][axis] + feet[1][axis]) * 0.5);
    let near = world.physics.cars_touching_ball(point, radius, vid);
    super::combat::blast_sparing(
        world,
        super::combat::Blast { point, radius, damage: TITAN_SLAM_DAMAGE * (0.4 + 0.6 * power), knock, dir: [0.0, 1.0, 0.0], attacker: pilot, weapon: WEAPON_TITAN },
        vid,
    );
    for car in near {
        // Another Titan is shaken, not thrown; what this one holds comes down with it.
        if world.entities.get::<Vehicle>(car).is_some_and(|vehicle| vehicle_def(vehicle.kind).spec.arms.is_some()) || world.physics.held_car(vid, 0) == Some(car) || world.physics.held_car(vid, 1) == Some(car) {
            continue;
        }
        let Some(pose) = world.physics.car_pose(car) else { continue };
        let away = [pose.position[0] - point[0], pose.position[2] - point[2]];
        let distance = away[0].hypot(away[1]).max(0.1);
        let falloff = (1.0 - distance / radius).max(0.0);
        let dv = [away[0] / distance * knock * falloff, knock * falloff * SLAM_TOSS, away[1] / distance * knock * falloff];
        world.physics.knock_car(car, dv, pose.position);
        super::combat::damage_vehicle(world, car, (TITAN_SLAM_VEHICLE_DAMAGE * (0.4 + 0.6 * power) * falloff).min(255.0) as u8, pilot);
    }
    tracing::debug!(titan = vid, impact, radius, "slammed");
}

impl Titans {
    /// One tick of Titan `vid`, whose limbs are at `limbs`.
    fn work(&mut self, world: &mut World, vid: u32, limbs: LimbsPose, dt: f32) {
        let wrecked = world.entities.dead(vid);
        let pilot = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(0)).filter(|_| !wrecked);
        turn_head(world, vid, pilot, dt);
        let asked = pilot.and_then(|pilot| world.entities.get::<Player>(pilot)).and_then(|player| player.fists).map_or([Hand::default(); 2], |hands| asked(&hands));
        world.physics.set_hands(vid, asked);
        let state = self.hands.entry(vid).or_default();
        let was = state.closed;
        state.closed = asked.map(|hand| hand.closed);
        for cooling in &mut state.cooling {
            *cooling = cooling.saturating_sub(1);
        }
        let cooling = state.cooling;
        for hand in 0..2 {
            let (fist, velocity) = (limbs.fists_world[hand], limbs.fist_velocity[hand]);
            let full = held_in(world, vid, hand).is_some() || world.physics.held_car(vid, hand).is_some();
            if !asked[hand].closed && full {
                let_go(world, vid, hand, velocity, pilot);
                continue;
            }
            // A hand that holds a gun closes on nothing: closed, it is only a fist to punch with.
            if asked[hand].closed && !was[hand] && !full && !holds_gun(world, vid, hand) {
                grab(world, vid, hand, fist);
                continue;
            }
            // Only a closed, empty fist hits: an open hand comes down beside a soldier and does no harm.
            let speed = vec3::length(velocity);
            if full || !asked[hand].closed || speed < TITAN_PUNCH_SPEED {
                continue;
            }
            punch_soldiers(world, vid, fist, velocity, pilot);
            if cooling[hand] == 0 {
                let before = (world.physics.cars_touching_ball(fist, TITAN_FIST_RADIUS + 0.3, vid).len(), world.physics.objects_touching_ball(fist, TITAN_FIST_RADIUS).len());
                strike(world, vid, fist, TITAN_FIST_RADIUS + 0.3, velocity, speed * TITAN_PUNCH_VEHICLE_DAMAGE, pilot);
                if before != (0, 0)
                    && let Some(state) = self.hands.get_mut(&vid)
                {
                    state.cooling[hand] = STRIKE_COOLDOWN;
                }
            }
        }
        for foot in 0..2 {
            if limbs.landed[foot] && !wrecked {
                stomp(world, vid, limbs.feet[foot], pilot);
            }
        }
        if limbs.impact >= TITAN_SLAM_SPEED && !wrecked {
            slam(world, vid, limbs.feet, limbs.impact, pilot);
        }
    }
}

impl Mechanic for Titans {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        // A pilot's hands grow stale when no more come.
        for id in world.entities.ids_where::<Player>(|_, player| player.fists.is_some()) {
            let mut player = world.entities.of_mut::<Player>(id);
            player.fists_age += 1;
            if player.fists_age > HANDS_STALE_TICKS || player.seat.is_none() {
                player.fists = None;
            }
        }
        let titans: Vec<u32> = world.entities.ids::<With<Vehicle>>().into_iter().filter(|&vid| world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle_def(vehicle.kind).spec.arms.is_some())).collect();
        self.hands.retain(|vid, _| titans.contains(vid));
        for &vid in &titans {
            if let Some(limbs) = world.physics.limbs(vid) {
                self.work(world, vid, limbs, dt);
            }
        }
        // Whoever is held rides on their fist; the dead, and those whose Titan is gone, are let go of.
        for id in world.entities.ids_where::<Player>(|_, player| player.held.is_some()) {
            let Some(held) = world.entities.of::<Player>(id).held else { continue };
            let limbs = world.physics.limbs(held.titan).filter(|_| titans.contains(&held.titan));
            let (Some(limbs), true) = (limbs, world.entities.alive(id)) else {
                world.entities.of_mut::<Player>(id).held = None;
                continue;
            };
            let hand = usize::from(held.hand.min(1));
            let mut player = world.entities.of_mut::<Player>(id);
            let player = &mut *player;
            // A held soldier's USE is nobody's: they board nothing from up there.
            player.input.buttons &= !BUTTON_USE;
            let character = &mut player.character;
            character.position = held_at(limbs.fists_world[hand]);
            character.yaw = shared::quant::from_mrad(player.input.yaw);
            character.vy = 0.0;
            character.knock = [0.0, 0.0];
            character.grounded = true;
            character.ride = 0;
            character.chute = false;
            character.swimming = false;
        }
    }

    fn on_leave(&mut self, world: &mut World, id: u32) {
        if let Some(mut player) = world.entities.get_mut::<Player>(id) {
            player.held = None;
            player.fists = None;
        }
    }
}

#[cfg(test)]
mod tests;
