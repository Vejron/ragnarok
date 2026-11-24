//! Civilians (TRAFFIC.md): drivers of the traffic out of their cars. Each is a player with no
//! connection, as a bot or a zombie is, on no team and with empty hands, whose input is made up here,
//! so they run, fall, are thrown, lose limbs and die exactly as a soldier does.
//!
//! A civilian who gave their car up stands with their hands up a moment, and then runs. They run
//! from the nearest soldier within `SCARED_WITHIN`, or from whatever frightened them, by the way
//! `navigation` finds round buildings, each a little off straight away and pushed apart from the
//! others. Out of reach of anyone they slow to a walk. Knocked down, they lie there for longer the
//! worse they are hurt, and a while more by chance, then get up where they are before running on. They are gone once no player is near enough to see them,
//! and the dead after a while.

use super::Mechanic;
use super::bots::{bend, follow, fresh_goal, heading, push, steer};
use super::npcs::turn_towards;
use crate::entities::{Component, With};
use crate::game::{Damaged, Player, Walk, World};
use shared::consts::*;
use shared::protocol::InputFrame;
use shared::quant::to_mrad;
use std::f32::consts::PI;

/// The mind of a civilian. Their body is the `Player` with the same id.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Civilian {
    /// Facing, radians, turned at `TURN_RATE` rather than snapped.
    pub yaw: f32,
    /// What they run from.
    pub threat: [f32; 3],
    /// Ticks left standing with their hands up before they run.
    pub hands_up: u32,
    /// Ticks left getting up off the ground after a knockdown (`GETUP_TICKS`): they stay where they are.
    pub rising: u32,
    /// Ticks left lying there after the knockdown itself is over (`stunned`); `None` while they are
    /// not down, or before it is reckoned.
    pub lying: Option<u32>,
    /// Where they are running to, and how they get there.
    pub goal: Option<[f32; 3]>,
    pub walk: Walk,
    /// How far off straight away from the threat they run, radians: their own, so a crowd scatters.
    pub slant: f32,
    /// Ticks on their feet, and ticks lain dead.
    pub afoot: u32,
    pub lain: u32,
}

impl Civilian {
    pub fn new(yaw: f32, threat: [f32; 3], hands_up: u32, slant: f32) -> Self {
        Civilian { yaw, threat, hands_up, rising: 0, lying: None, goal: None, walk: Walk::new(), slant, afoot: 0, lain: 0 }
    }
}

/// Whether `id` is a civilian: on foot out of a car of the traffic, or driving a bus. Nobody's
/// enemy: bots leave them be, they never come back as ghosts, and killing one is no kill.
pub fn is_civilian(world: &World, id: u32) -> bool {
    world.entities.has::<Civilian>(id) || world.entities.has::<super::BusDriver>(id)
}

/// They look round for whom to run from about every this many ticks (4 a second), each on their own beat.
const LOOK_EVERY: u32 = 15;
/// How fast they turn, radians a second.
const TURN_RATE: f32 = 7.0;
/// They only run on once they face within this of where they are going, radians.
const RUN_WITHIN: f32 = 1.2;
/// A soldier within this of them, metres, is someone to run from.
const SCARED_WITHIN: f32 = 45.0;
/// How far each leg of their run goes, metres, and how near its end counts as there.
const LEG: f32 = 30.0;
const THERE: f32 = 4.0;
/// How far a slant goes off straight away from the threat, radians, either way.
pub const CIVILIAN_SLANT: f32 = 0.7;
/// Further than this from what they run from, metres, they walk.
const WALK_BEYOND: f32 = 70.0;
/// A civilian is gone once no player is within this of them, metres, or after this long on their
/// feet, seconds; a dead one lies this long.
const UNSEEN: f32 = 150.0;
const AFOOT_MAX: f32 = 120.0;
const LIES: f32 = 10.0;
/// How long a civilian knocked down lies there once the knockdown itself is over, seconds: at the
/// least, more by how badly they are hurt, and more by chance.
const STUNNED_MIN: f32 = 0.5;
const STUNNED_HURT: f32 = 8.0;
const STUNNED_CHANCE: f32 = 3.0;
/// The most grid cells looked at in one tick to find ways, among all the civilians.
const SEARCH_PER_TICK: usize = 2_000;

#[derive(Default)]
pub struct Civilians;

fn distance_2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    (b[0] - a[0]).hypot(b[2] - a[2])
}

/// The nearest soldier on their feet within `SCARED_WITHIN` of `at`: anyone alive and armed or not,
/// who is not a civilian.
fn scariest(world: &World, at: [f32; 3]) -> Option<[f32; 3]> {
    world
        .entities
        .all::<Player>()
        .filter(|&(other, player)| world.entities.alive(other) && player.embodied() && !is_civilian(world, other))
        .map(|(_, player)| player.character.position)
        .filter(|&p| distance_2d(at, p) <= SCARED_WITHIN)
        .min_by(|a, b| distance_2d(at, *a).total_cmp(&distance_2d(at, *b)))
}

/// Somewhere `LEG` metres off from `at`, away from `threat` and `slant` aside, that a soldier can
/// stand: straight there first, then wider and wider to either side.
fn away(world: &World, at: [f32; 3], threat: [f32; 3], slant: f32) -> Option<[f32; 3]> {
    let straight = heading(threat, at) + slant;
    (0..9u32).find_map(|n| {
        let turn = n.div_ceil(2) as f32 * if n % 2 == 0 { 0.4 } else { -0.4 };
        let (sin, cos) = (straight + turn).sin_cos();
        let (x, z) = (at[0] - sin * LEG, at[2] - cos * LEG);
        let floor = super::life::open_ground(world, x, z)?;
        Some([x, floor + sim::STANDING_Y, z])
    })
}

/// How many ticks civilian `id`, knocked down, lies there after the knockdown itself: `STUNNED_MIN`
/// seconds, up to `STUNNED_HURT` more the worse they are hurt (the health they have lost, and a whole
/// share more for each leg gone), and up to `STUNNED_CHANCE` more by chance.
fn stunned(world: &mut World, id: u32) -> u32 {
    let health = world.entities.get::<crate::entities::Health>(id).map_or(1.0, |health| f32::from(health.hp) / f32::from(health.max.max(1)));
    let legs = (super::limbs::lost(world, id) & shared::protocol::LIMB_LEGS).count_ones() as f32;
    let hurt = ((1.0 - health) + legs * 0.5).min(1.0);
    let chance = world.rng.next_f64() as f32;
    ((STUNNED_MIN + STUNNED_HURT * hurt + STUNNED_CHANCE * chance) * TICK_RATE as f32) as u32
}

/// One tick of civilian `id`'s fear: what they do now, as the input a player would send, and
/// whether they are to go.
fn think(world: &mut World, id: u32, civilian: &mut Civilian, dt: f32, viewers: &[[f32; 3]], crowd: &[(u32, [f32; 2])], budget: &mut usize) -> (InputFrame, bool) {
    let idle = InputFrame { yaw: to_mrad(civilian.yaw), view: world.tick as u16, ..Default::default() };
    let Some(player) = world.entities.get::<Player>(id) else { return (idle, true) };
    let (at, down) = (player.character.position, player.character.down);
    if !world.entities.alive(id) {
        civilian.lain += 1;
        return (idle, civilian.lain > (LIES * TICK_RATE as f32) as u32);
    }
    civilian.afoot += 1;
    let unseen = viewers.iter().all(|v| distance_2d(*v, at) > UNSEEN);
    if unseen || civilian.afoot > (AFOOT_MAX * TICK_RATE as f32) as u32 {
        return (idle, true);
    }
    // Knocked down: they lie there for as long as it takes them, and then get up where they are, as
    // the getting up is drawn. Kept down meanwhile, which everyone sees as the knockdown going on.
    if down > 0 {
        let lying = *civilian.lying.get_or_insert_with(|| stunned(world, id));
        if down <= 1 && lying > 0 {
            civilian.lying = Some(lying - 1);
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.character.down = 2;
            }
        }
        civilian.rising = GETUP_TICKS;
        return (idle, false);
    }
    civilian.lying = None;
    if civilian.rising > 0 {
        civilian.rising -= 1;
        return (idle, false);
    }
    if (world.tick + id).is_multiple_of(LOOK_EVERY)
        && let Some(threat) = scariest(world, at)
    {
        // A threat that has come round to the other side of them sends them another way.
        if distance_2d(threat, civilian.threat) > LEG * 0.5 {
            civilian.goal = None;
        }
        civilian.threat = threat;
    }
    if civilian.hands_up > 0 {
        civilian.hands_up -= 1;
        civilian.yaw = turn_towards(civilian.yaw, heading(at, civilian.threat), TURN_RATE * dt);
        return (InputFrame { yaw: to_mrad(civilian.yaw), ..idle }, false);
    }
    // A fresh leg once there, or when the way there leads back towards the threat.
    let towards = civilian.goal.is_some_and(|goal| distance_2d(goal, civilian.threat) < distance_2d(at, civilian.threat));
    if civilian.goal.is_none_or(|goal| distance_2d(at, goal) < THERE) || towards {
        civilian.goal = away(world, at, civilian.threat, civilian.slant);
        civilian.walk.path.clear();
        fresh_goal(&mut civilian.walk);
    }
    let Some(goal) = civilian.goal else { return (idle, false) };
    let toward = follow(world, &mut civilian.walk, at, goal, budget);
    let want = steer(world, at, bend(heading(at, toward), push(id, at, crowd)));
    civilian.yaw = turn_towards(civilian.yaw, want, TURN_RATE * dt);
    let facing = (turn_towards(civilian.yaw, want, PI) - civilian.yaw).abs() < RUN_WITHIN;
    let scared = distance_2d(at, civilian.threat) < WALK_BEYOND;
    let input = InputFrame { forward: if facing { 127 } else { 0 }, buttons: if scared { BUTTON_RUN } else { 0 }, ..InputFrame { yaw: to_mrad(civilian.yaw), ..idle } };
    (input, false)
}

impl Mechanic for Civilians {
    fn on_leave(&mut self, world: &mut World, id: u32) {
        world.entities.remove::<Civilian>(id);
    }

    /// A civilian hurt runs from whoever hurt them, hands up or not.
    fn react(&mut self, world: &mut World) {
        let hurt: Vec<(u32, [f32; 3])> = world.happened::<Damaged>().filter(|damaged| world.entities.has::<Civilian>(damaged.victim)).map(|damaged| (damaged.victim, damaged.from.unwrap_or(damaged.at))).collect();
        for (id, from) in hurt {
            if let Some(mut civilian) = world.entities.get_mut::<Civilian>(id) {
                (civilian.threat, civilian.hands_up, civilian.goal) = (from, 0, None);
            }
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.surrendered = false;
            }
        }
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let ids = world.entities.ids::<With<Civilian>>();
        if ids.is_empty() {
            return;
        }
        let viewers = super::traffic::viewers(world);
        let crowd: Vec<(u32, [f32; 2])> = ids.iter().filter_map(|&id| world.entities.get::<Player>(id).filter(|_| world.entities.alive(id)).map(|p| (id, [p.character.position[0], p.character.position[2]]))).collect();
        let mut budget = SEARCH_PER_TICK;
        for id in ids {
            let Some(mut mind) = world.entities.get_mut::<Civilian>(id) else { continue };
            let mut civilian = std::mem::replace(&mut *mind, Civilian::new(0.0, [0.0; 3], 0, 0.0));
            let (input, gone) = think(world, id, &mut civilian, dt, &viewers, &crowd, &mut budget);
            let hands_up = civilian.hands_up > 0;
            *world.entities.of_mut::<Civilian>(id) = civilian;
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.input = input;
                player.surrendered = hands_up;
            }
            if gone {
                world.traffic_calls.push(super::TrafficCall::Gone(id));
            }
        }
    }
}

#[cfg(test)]
mod tests;
