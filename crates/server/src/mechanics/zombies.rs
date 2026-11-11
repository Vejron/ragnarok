//! Zombies (ZOMBIES.md): players with no connection, like bots, whose input is made up here, so
//! they walk, fall, lose limbs and die exactly as a soldier does. A zombie has no gun and no fear.
//! It smells the nearest living soldier within `ZOMBIE_SMELL_RANGE`, walls or none, and shambles
//! after them along the way `navigation` finds, pushed apart from the rest of its horde. Close in, it
//! lunges, and in reach it claws: it reaches back for `ZOMBIE_CLAW_WINDUP` ticks, and then the blow
//! lands if they are still in reach. With no arms left it bites instead. With no legs it crawls on,
//! which the soldier's own legless walk does for it. With no one to smell it goes to the last
//! gunshot or blast it heard, and one that is hurt turns on whoever hurt it.
//!
//! When and where they rise is the scenario's (`scenario::Horde`); this is only their minds.

use super::Mechanic;
use super::bots::{bend, follow, fresh_goal, heading, push, steer};
use super::life::{Harm, damage_entity};
use super::npcs::turn_towards;
use crate::entities::{Component, With};
use crate::game::{Damaged, Noise, Player, Walk, World};
use bevy_ecs::message::MessageCursor;
use shared::consts::*;
use shared::protocol::{InputFrame, LIMB_ARMS, WEAPON_CLAW};
use shared::quant::to_mrad;
use std::f32::consts::PI;

#[derive(Default)]
pub struct Zombies {
    /// How far through the noises they have listened.
    heard: MessageCursor<Noise>,
}

/// The mind of a zombie. Its body is the `Player` with the same id.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Zombie {
    /// Facing, radians, turned at `TURN_RATE` rather than snapped.
    pub yaw: f32,
    /// Its own shambling pace, a forward axis value, and whether it runs instead.
    pub pace: i8,
    pub runner: bool,
    /// Whom it is after.
    pub prey: Option<u32>,
    /// Where it heard something, to go to while it smells no one.
    pub lure: Option<[f32; 3]>,
    pub walk: Walk,
    /// Ticks into a claw, 0 when it is not clawing: up to `ZOMBIE_CLAW_WINDUP` it reaches back, on that
    /// tick the blow lands, and then it recovers for `ZOMBIE_CLAW_RECOVER`.
    pub claw: u32,
}

impl Zombie {
    pub fn new(pace: i8, runner: bool, yaw: f32) -> Self {
        Zombie { yaw, pace, runner, prey: None, lure: None, walk: Walk::new(), claw: 0 }
    }

    /// Whether it is reaching back to strike, which the snapshot shows as `FLAG_CLAWING`.
    pub fn clawing(&self) -> bool {
        self.claw > 0 && self.claw <= ZOMBIE_CLAW_WINDUP
    }
}

/// It smells about every this many ticks (4 a second), each on its own beat.
const SMELL_EVERY: u32 = 15;
/// How fast it turns, radians a second.
const TURN_RATE: f32 = 3.5;
/// It only walks on once it faces within this of where it is going, radians.
const WALK_WITHIN: f32 = 1.0;
/// It starts a claw only facing within this of its prey, radians.
const CLAW_WITHIN: f32 = 0.6;
/// The blow lands on prey this much further off than `ZOMBIE_CLAW_REACH` too, metres, so that
/// stepping back as it winds up is not quite enough.
const CLAW_SLACK: f32 = 0.4;
/// Prey this far above or below it is out of its reach, metres.
const CLAW_HEIGHT: f32 = 1.6;
/// Close enough to where it heard something: it stands there groaning, metres.
const LURE_REACHED: f32 = 3.0;
/// The most grid cells looked at in one tick to find ways, among all the zombies: the rest head
/// straight on until their turn.
const SEARCH_PER_TICK: usize = 4_000;

fn distance_2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    (b[0] - a[0]).hypot(b[2] - a[2])
}

/// Whether `id` is a zombie.
pub fn is_zombie(world: &World, id: u32) -> bool {
    world.entities.has::<Zombie>(id)
}

/// Whether zombie `id` is clawing (`Zombie::clawing`).
pub fn clawing(world: &World, id: u32) -> bool {
    world.entities.get::<Zombie>(id).is_some_and(Zombie::clawing)
}

/// The nearest living soldier on their feet within smell of `at`, and where they stand. Anyone in a
/// vehicle is out of its reach and of no interest.
fn smell(world: &World, at: [f32; 3]) -> Option<u32> {
    world
        .entities
        .all::<Player>()
        .filter(|(other, player)| world.entities.alive(*other) && player.embodied() && !is_zombie(world, *other))
        .map(|(other, player)| (other, distance_2d(at, player.character.position)))
        .filter(|&(_, distance)| distance <= ZOMBIE_SMELL_RANGE)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(other, _)| other)
}

/// Where `prey` stands, while they are still something to go after.
fn prey_at(world: &World, prey: u32) -> Option<[f32; 3]> {
    let player = world.entities.get::<Player>(prey).filter(|player| player.embodied())?;
    world.entities.alive(prey).then_some(player.character.position)
}

/// One tick of zombie `id`'s hunger: what it does now, as the input a player would send.
fn think(world: &mut World, id: u32, zombie: &mut Zombie, dt: f32, noises: &[Noise], crowd: &[(u32, [f32; 2])], budget: &mut usize) -> InputFrame {
    let idle = InputFrame { yaw: to_mrad(zombie.yaw), view: world.tick as u16, ..Default::default() };
    let Some(player) = world.entities.get::<Player>(id) else { return idle };
    if !world.entities.alive(id) {
        zombie.prey = None;
        zombie.lure = None;
        zombie.claw = 0;
        zombie.walk.path.clear();
        return idle;
    }
    let at = player.character.position;

    if (world.tick + id).is_multiple_of(SMELL_EVERY) || zombie.prey.is_some_and(|prey| prey_at(world, prey).is_none()) {
        let nearest = smell(world, at);
        if nearest != zombie.prey {
            fresh_goal(&mut zombie.walk);
        }
        zombie.prey = nearest;
    }
    if let Some(heard) = noises.iter().filter(|noise| noise.by != Some(id) && vec_distance(noise.at, at) <= noise.reach).min_by(|a, b| vec_distance(a.at, at).total_cmp(&vec_distance(b.at, at))) {
        zombie.lure = Some(heard.at);
    }

    let target = zombie.prey.and_then(|prey| prey_at(world, prey).map(|p| (prey, p)));
    if zombie.claw > 0 {
        zombie.claw += 1;
        if zombie.claw == ZOMBIE_CLAW_WINDUP + 1 {
            strike(world, id, at, target);
        }
        if zombie.claw > ZOMBIE_CLAW_WINDUP + ZOMBIE_CLAW_RECOVER {
            zombie.claw = 0;
        }
    }

    let goal = match target {
        Some((_, prey)) => {
            zombie.lure = None;
            prey
        }
        None => match zombie.lure {
            Some(lure) if distance_2d(at, lure) > LURE_REACHED => lure,
            _ => {
                zombie.lure = None;
                return idle;
            }
        },
    };
    let distance = distance_2d(at, goal);
    if let Some((_, prey)) = target {
        // In reach: it stops and claws, turning to keep facing them.
        let want = heading(at, prey);
        zombie.yaw = turn_towards(zombie.yaw, want, TURN_RATE * dt);
        let facing = (turn_towards(zombie.yaw, want, PI) - zombie.yaw).abs() < CLAW_WITHIN;
        let reach = distance <= ZOMBIE_CLAW_REACH && (prey[1] - at[1]).abs() < CLAW_HEIGHT;
        if reach && facing && zombie.claw == 0 {
            zombie.claw = 1;
        }
        if reach || zombie.clawing() {
            return InputFrame { yaw: to_mrad(zombie.yaw), ..idle };
        }
    }

    let toward = if distance < ZOMBIE_LUNGE { goal } else { follow(world, &mut zombie.walk, at, goal, budget) };
    let want = steer(world, at, bend(heading(at, toward), push(id, at, crowd)));
    zombie.yaw = turn_towards(zombie.yaw, want, TURN_RATE * dt);
    let facing = (turn_towards(zombie.yaw, want, PI) - zombie.yaw).abs() < WALK_WITHIN;
    let lunge = target.is_some() && distance < ZOMBIE_LUNGE;
    let run = zombie.runner && target.is_some();
    InputFrame {
        forward: if !facing { 0 } else if lunge || run { 127 } else { zombie.pace },
        buttons: if run { BUTTON_RUN } else { 0 },
        ..InputFrame { yaw: to_mrad(zombie.yaw), ..idle }
    }
}

fn vec_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    shared::vec3::distance(a, b)
}

/// The blow of zombie `id`, standing at `at`, at `target` if they are still in its reach: its claws,
/// or its teeth once it has no arms.
fn strike(world: &mut World, id: u32, at: [f32; 3], target: Option<(u32, [f32; 3])>) {
    let Some((prey, where_)) = target else { return };
    if distance_2d(at, where_) > ZOMBIE_CLAW_REACH + CLAW_SLACK || (where_[1] - at[1]).abs() >= CLAW_HEIGHT {
        return;
    }
    let armless = super::limbs::lost(world, id) & LIMB_ARMS == LIMB_ARMS;
    let amount = if armless { ZOMBIE_BITE_DAMAGE } else { ZOMBIE_CLAW_DAMAGE };
    damage_entity(world, prey, Harm::new(amount, Some(id), WEAPON_CLAW).from(at));
}

impl Mechanic for Zombies {
    fn on_leave(&mut self, world: &mut World, id: u32) {
        world.entities.remove::<Zombie>(id);
    }

    /// A zombie hurt turns on whoever hurt it, if it can get at them.
    fn react(&mut self, world: &mut World) {
        let hurt: Vec<(u32, u32)> = world.happened::<Damaged>().filter_map(|damaged| Some((damaged.victim, damaged.attacker?))).collect();
        for (victim, attacker) in hurt {
            if attacker == victim || is_zombie(world, attacker) || prey_at(world, attacker).is_none() {
                continue;
            }
            if let Some(mut zombie) = world.entities.get_mut::<Zombie>(victim)
                && zombie.prey != Some(attacker)
            {
                zombie.prey = Some(attacker);
                fresh_goal(&mut zombie.walk);
            }
        }
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let noises: Vec<Noise> = self.heard.read(world.events::<Noise>()).copied().collect();
        let ids = world.entities.ids::<With<Zombie>>();
        if ids.is_empty() {
            return;
        }
        let mut budget = SEARCH_PER_TICK;
        // Where every zombie on its feet stands as the tick begins, for them to spread out.
        let crowd: Vec<(u32, [f32; 2])> = ids
            .iter()
            .filter_map(|id| world.entities.get::<Player>(*id).filter(|_| world.entities.alive(*id)).map(|p| (*id, [p.character.position[0], p.character.position[2]])))
            .collect();
        for id in ids {
            let Some(mut mind) = world.entities.get_mut::<Zombie>(id) else { continue };
            let mut zombie = std::mem::replace(&mut *mind, Zombie::new(0, false, 0.0));
            let input = think(world, id, &mut zombie, dt, &noises, &crowd, &mut budget);
            *world.entities.of_mut::<Zombie>(id) = zombie;
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.input = input;
            }
        }
    }
}

#[cfg(test)]
mod tests;
