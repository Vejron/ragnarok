//! Bots: players with no connection whose input is made up here. Each tick, before anything else
//! runs, `Bots` writes every bot's `InputFrame` (axes, facing, buttons) as a client would send it,
//! so movement, items, seats, damage, scores and replication need nothing new for them.
//!
//! A bot walks a route of waypoints. It looks a few times a second; when it sees someone not on its
//! team it runs at them, and when they get out of sight it runs to where they were last seen, looks
//! round there for a while and goes back to its route. While it can see them it fights: it picks a
//! gun for the distance, closes to where that gun is good, and once it has reacted it fires at where
//! they will be when the round gets there, never quite on the mark. A gunshot or a blast it hears,
//! or a hit it takes, sends it to look where that came from. Out of a fight it walks the way
//! `navigation` finds round buildings, walls and trees, with a little sidestepping for anything else
//! in the way, such as a vehicle (see BOTS.md). Each has a temper of its own, and being shot at wears
//! on its nerve until it gets down or breaks for cover (see `nerve` and `cover`). A crowd takes turns:
//! only a few at a time really attack, while the rest flank or keep heads down (see `tokens`).

mod cover;
mod crew;
mod evade;
mod nerve;
mod senses;
mod tokens;
mod wounded;

pub use crew::road_route;
pub(crate) use senses::darkness;
pub use crew::{DRIVER, GUNNER, nearest_waypoint};

use shared::vec3;
use super::Mechanic;
use bevy_ecs::message::MessageCursor;
use crate::entities::With;
use super::combat::same_team;
use super::life::open_ground;
use super::npcs::turn_towards;
use crate::game::{Bot, BotMode, Damaged, Noise, Player, Role, Threat, Walk, World};
use shared::consts::*;
use shared::items::{HOTBAR_SLOTS, ITEM_AK5, ITEM_AT_RIFLE, ITEM_CARL_GUSTAF, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER, ItemDef, ItemKind, def};
use shared::vehicles::{Exposure, vehicle_def};
use shared::protocol::{InputFrame, ItemSlot};
use shared::quant::to_mrad;
use std::f32::consts::{PI, TAU};

#[derive(Default)]
pub struct Bots {
    /// How far through the noises it has listened.
    heard: MessageCursor<Noise>,
}

/// How far a bot sees, metres.
pub const SIGHT_RANGE: f32 = 60.0;
/// Cosine of half its field of view while it is calm: 120° across. Once alert it sees all round.
const CALM_VIEW_COS: f32 = 0.5;
/// It looks every this many ticks (10 a second), each bot on its own beat.
const LOOK_EVERY: u32 = 6;
/// How close a pursuer comes before it stops and stares, metres.
pub const CLOSE_IN: f32 = 3.0;
/// A pursuit that takes it this far from its route (any of its waypoints, or home before it has one)
/// is given up, metres.
pub const LEASH: f32 = 150.0;
/// How long it looks round where it lost someone.
const SEARCH_TICKS: u32 = 6 * TICK_RATE;
/// How fast it turns, radians a second; and how fast while looking round.
const TURN_RATE: f32 = 6.0;
const SEARCH_TURN_RATE: f32 = 1.5;
/// It only walks off once it faces within this of where it is going, radians.
const WALK_WITHIN: f32 = 0.8;
/// Close enough to a waypoint, or to where someone was last seen, metres.
const REACHED: f32 = 1.2;
/// A goal it has not got `PROGRESS` metres closer to in `STALL_TICKS` is given up.
const PROGRESS: f32 = 0.5;
const STALL_TICKS: u32 = 3 * TICK_RATE;
/// A wall this near ahead, metres, sends it sidestepping.
const WALL_AHEAD: f32 = 1.2;
/// The headings tried, in turn, when the way ahead is blocked: radians off straight.
const DETOURS: [f32; 6] = [0.6, -0.6, 1.2, -1.2, 1.8, -1.8];
/// A made-up route: this many waypoints after home, between `ROUTE_MIN` and `ROUTE_RADIUS` metres
/// from home, each in a straight, open line from the one before.
const ROUTE_POINTS: usize = 3;
const ROUTE_MIN: f32 = 8.0;
const ROUTE_RADIUS: f32 = 25.0;
const ROUTE_ATTEMPTS: usize = 16;
/// The most grid cells looked at in one tick to find ways, among all the bots: the rest wait their
/// turn. About 2 ms.
const SEARCH_PER_TICK: usize = 8_000;
/// It looks for a way again when its goal moves this far, metres, or this often on the way.
const GOAL_MOVED: f32 = 2.0;
const REPLAN_TICKS: u32 = 2 * TICK_RATE;
/// Close enough to a corner of its way to head for the next one, metres.
const CORNER_REACHED: f32 = 0.8;
/// Height above the ground the line between waypoints is checked at, metres.
const ROUTE_LINE_Y: f32 = 1.0;
/// How far a gunshot carries, metres, and a blast or a cannon firing.
pub const HEAR_SHOT: f32 = 80.0;
pub const HEAR_BLAST: f32 = 150.0;
/// Already going to look somewhere, it goes to a noise instead only if that is this much nearer,
/// metres: in a busy fight the nearest shot jumps about from tick to tick.
const NEARER_NOISE: f32 = 10.0;
/// Bots nearer than this push each other apart, metres: the push is nothing at this distance and
/// 1 on top of each other. It weighs `PUSH_WEIGHT` against the pull of where the bot is going, which
/// is under 1 so that a crowd can slow a bot but never walk it backwards from its goal.
const SEPARATE: f32 = 3.0;
const PUSH_WEIGHT: f32 = 0.8;
/// Where on a body a bot looks at: about the chest, above the capsule centre.
const CHEST: f32 = 0.3;

/// How long it takes to fire on someone it has just seen, seconds: at skill 0, and at skill 1.
const REACT_POOR: f32 = 0.7;
const REACT_GOOD: f32 = 0.2;
/// How far off its aim is, radians, at skill 0 and at skill 1, once it has settled on a target. On
/// first sight it is `1 + UNSETTLED` times that, falling off over `SETTLE` seconds of watching.
const AIM_POOR: f32 = 0.1;
const AIM_GOOD: f32 = 0.008;
const UNSETTLED: f32 = 2.0;
const SETTLE: f32 = 1.0;
/// How much of the lead a moving target needs it gives, at skill 0 (at skill 1, all of it).
const LEAD_POOR: f32 = 0.3;
/// It fires once it faces within this of where it wants to, radians.
const FIRE_WITHIN: f32 = 0.05;
/// How wide of the target its first rounds go, metres, and how long they take to walk onto it,
/// seconds, at skill 0 and at skill 1. The misses are the warning that it has found you.
const MISS_BY: f32 = 2.0;
/// How wide of the target a bot with no token keeps its fire, metres.
const SUPPRESS_BY: f32 = 3.0;
const WALK_IN_POOR: f32 = 1.5;
const WALK_IN_GOOD: f32 = 0.6;
/// Fully shaken, its aim is off by this many times more; and its pauses between bursts longer.
const SHAKEN_AIM: f32 = 3.0;
const SHAKEN_PAUSE: f32 = 2.0;
/// Its aim is off by this share more while it moves, and per m/s the target moves, up to `FAST`.
const MOVING_AIM: f32 = 0.25;
const TARGET_SPEED_AIM: f32 = 0.04;
const FAST: f32 = 10.0;
/// A bot with less courage than this that has broken and found no cover runs.
const COWARD: f32 = 0.35;
/// Further off than this from whom it fights, metres, and short of where its gun is good, it closes
/// in in bounds: a run of `BOUND_RUN` seconds, then down to fire for `BOUND_FIRE`, at the least and
/// at the most.
const BOUND_FROM: f32 = 8.0;
const BOUND_RUN: [f32; 2] = [3.0, 5.0];
const BOUND_FIRE: [f32; 2] = [2.0, 3.0];
/// Standing off, it moves to another spot every `SHIFT_EVERY` seconds: this far round whom it
/// fights, metres, giving up the move after `SHIFT_FOR` seconds.
const SHIFT_EVERY: [f32; 2] = [5.0, 9.0];
const SHIFT_BY: f32 = 8.0;
const SHIFT_FOR: f32 = 4.0;
/// How far it stands off for its gun, as a share of `hold_distance`: from no aggression to full.
const HOLD_SCALE: [f32; 2] = [1.4, 0.5];
/// Caught on patrol, at skill 0 it can take this much longer to react, seconds.
const STARTLED: f32 = 0.6;
/// At a waypoint: the chance it stops to look about, for how long, seconds, the chance it cuts out
/// the next one, and the slowest it walks on, as an axis value.
const PAUSE_CHANCE: f64 = 0.5;
const PAUSE: [f32; 2] = [1.0, 4.0];
const SKIP_CHANCE: f64 = 0.2;
const SLOWEST_PACE: f32 = 80.0;
/// How fast it looks about, radians a second, swinging the other way every second.
const LOOK_ABOUT_RATE: f32 = 0.9;
/// It looks up or down no further than this, radians.
const PITCH_LIMIT: f32 = 1.4;
/// Fastest it believes anyone moves, m/s, on foot and in a vehicle: past that a sighting is a respawn
/// or a teleport, not speed.
const TOP_SPEED: f32 = 20.0;
const TOP_DRIVING: f32 = 60.0;
/// Against a vehicle it takes up an anti-tank weapon past this, metres, and a tank nearer than
/// `TOO_CLOSE` it gets away from even with one, and turns to fight again with one past `FIGHT_FROM`.
const ANTI_TANK_PAST: f32 = 15.0;
const TOO_CLOSE: f32 = 20.0;
const FIGHT_FROM: f32 = 30.0;
/// Rounds in a burst of a gun that fires while the trigger is held, and the pause after one at
/// skill 0 and at skill 1, seconds.
const BURST_SHOTS: [u32; 2] = [3, 6];
const BURST_PAUSE_POOR: f32 = 0.6;
const BURST_PAUSE_GOOD: f32 = 0.25;
/// It takes the shotgun under this distance, and keeps it until past the second, metres.
const SHOTGUN_IN: f32 = 10.0;
const SHOTGUN_OUT: f32 = 15.0;
/// The distance it picks its gun for when there is no one in sight.
const PATROL_DISTANCE: f32 = 30.0;
/// From this skill up it sidesteps while it stands and fights, a way for 0.6 to 1.6 s at a time with
/// as long standing still between, at this share of a walk.
const STRAFE_SKILL: f32 = 0.3;
const STRAFE_TICKS: [f32; 2] = [0.6, 1.6];
const SIDESTEP: f32 = 0.45;
/// Others closer than this push it sideways while it fights, at no more than this share of a walk.
const PUSHED_FROM: f32 = 0.3;
const PUSHED: f32 = 0.5;

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// How close it comes, metres, before it stands and fights with `item`.
fn hold_distance(item: u8) -> f32 {
    match item {
        ITEM_SHOTGUN => 5.0,
        ITEM_AK5 => 15.0,
        ITEM_SNIPER => 35.0,
        ITEM_CARL_GUSTAF | ITEM_AT_RIFLE => 60.0,
        _ => 12.0,
    }
}

/// Whether `slot` holds something that can hurt a tank, with a round left.
fn anti_tank(slot: &ItemSlot) -> bool {
    match slot.item {
        ITEM_CARL_GUSTAF => slot.count > 0,
        ITEM_AT_RIFLE => true,
        _ => false,
    }
}

/// The slot to fight `threat` with at `distance`. A tank only with an anti-tank weapon (`None` when it
/// has none); a vehicle a rifle can reach with one past `ANTI_TANK_PAST`, if it has one. Otherwise the
/// shotgun up close, else the AK5, the revolver or the sniper, whichever it has, else any gun but the
/// anti-tank ones. `None` when it has no gun. Holding the shotgun it keeps it a little past where it
/// would take it up, so it does not swap back and forth at the edge.
fn pick_gun(hotbar: &[ItemSlot; HOTBAR_SLOTS], distance: f32, current: u8, threat: Threat) -> Option<u8> {
    let anti = hotbar.iter().position(anti_tank).map(|i| i as u8);
    match threat {
        Threat::Armour => return anti,
        Threat::Soft if distance > ANTI_TANK_PAST && anti.is_some() => return anti,
        _ => {}
    }
    let slot_of = |item: u8| hotbar.iter().position(|slot| slot.item == item).map(|i| i as u8);
    let holds_shotgun = hotbar.get(current as usize).is_some_and(|slot| slot.item == ITEM_SHOTGUN);
    let close = distance < if holds_shotgun { SHOTGUN_OUT } else { SHOTGUN_IN };
    let order = if close { [ITEM_SHOTGUN, ITEM_AK5, ITEM_REVOLVER, ITEM_SNIPER] } else { [ITEM_AK5, ITEM_REVOLVER, ITEM_SNIPER, ITEM_SHOTGUN] };
    order.into_iter().find_map(slot_of).or_else(|| hotbar.iter().position(|slot| def(slot.item).is_some_and(|d| d.kind == ItemKind::Gun) && !anti_tank(slot)).map(|i| i as u8)).or(anti)
}

/// Where to put the shot so it meets a body now at `target` moving at `velocity`: ahead of it by
/// `lead` (0 to 1) of how far it goes while the round flies from `eye`, and above it by how far a
/// flying round drops on the way. A hitscan round arrives at once.
pub fn aim_point(eye: [f32; 3], target: [f32; 3], velocity: [f32; 3], gun: &ItemDef, lead: f32) -> [f32; 3] {
    let chest = [target[0], target[1] + CHEST, target[2]];
    if !gun.projectile {
        return chest;
    }
    let flight = vec3::distance(chest, eye) / BULLET_SPEED;
    let drop = 0.5 * GRAVITY * BULLET_GRAVITY_SCALE * flight * flight;
    let mut aim: [f32; 3] = vec3::along(chest, velocity, flight * lead);
    aim[1] += drop;
    aim
}

/// A random number in -1..1, more often near 0 than not.
fn wobble(world: &mut World) -> f32 {
    (world.rng.next_f64() + world.rng.next_f64() - 1.0) as f32
}

fn distance_2d(a: [f32; 3], b: [f32; 3]) -> f32 {
    (b[0] - a[0]).hypot(b[2] - a[2])
}

/// The push on the bot `id` at `at` from the other bots in `crowd` (their ids and spots on the
/// ground): away from each one within `SEPARATE`, harder the closer, at most 1 in all. This is the
/// separation rule of flocking: a bot steers clear of its neighbours, so a crowd spreads out instead
/// of piling onto one spot. Two on the very same spot are pushed apart along a line they agree on.
pub(super) fn push(id: u32, at: [f32; 3], crowd: &[(u32, [f32; 2])]) -> [f32; 2] {
    let mut total = [0.0f32; 2];
    for &(other, [x, z]) in crowd {
        if other == id {
            continue;
        }
        let (dx, dz) = (at[0] - x, at[2] - z);
        let d = dx.hypot(dz);
        if d >= SEPARATE {
            continue;
        }
        let away = if d > 1e-3 {
            [dx / d, dz / d]
        } else {
            let angle = id.min(other) as f32 * 2.4;
            let sign = if id < other { 1.0 } else { -1.0 };
            [angle.cos() * sign, angle.sin() * sign]
        };
        let strength = 1.0 - d / SEPARATE;
        total[0] += away[0] * strength;
        total[1] += away[1] * strength;
    }
    let size = total[0].hypot(total[1]);
    if size > 1.0 { [total[0] / size, total[1] / size] } else { total }
}

/// `yaw` bent away from `push`, which pulls less than the way the bot wants to go.
pub(super) fn bend(yaw: f32, push: [f32; 2]) -> f32 {
    let (x, z) = (-yaw.sin() + push[0] * PUSH_WEIGHT, -yaw.cos() + push[1] * PUSH_WEIGHT);
    if x.hypot(z) < 1e-3 { yaw } else { (-x).atan2(-z) }
}

/// The heading that faces from `from` to `to`. Yaw 0 faces -Z: forward is (-sin yaw, -cos yaw).
pub(super) fn heading(from: [f32; 3], to: [f32; 3]) -> f32 {
    (-(to[0] - from[0])).atan2(-(to[2] - from[2]))
}

/// Whether nothing (the world, a building, a vehicle) stands between `from` and `to`.
pub fn clear_line(world: &World, from: [f32; 3], to: [f32; 3]) -> bool {
    clear_line_past(world, from, to, None)
}

/// `clear_line`, looking past vehicle `own` (the one whoever looks from `from` sits in).
pub(crate) fn clear_line_past(world: &World, from: [f32; 3], to: [f32; 3], own: Option<u32>) -> bool {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if length < 0.01 {
        return true;
    }
    let dir = d.map(|v| v / length);
    world.physics.cast_ray(from, dir, length, own.unwrap_or(0)).is_none_or(|t| t >= length - 0.2)
}

/// A short loop of open, straight-line-connected waypoints around `home`, starting at it.
fn make_route(world: &mut World, home: [f32; 2]) -> Vec<[f32; 2]> {
    let mut route = vec![home];
    let line_point = |world: &World, [x, z]: [f32; 2]| [x, world.map.ground.height_at(x, z) + ROUTE_LINE_Y, z];
    for _ in 0..ROUTE_POINTS {
        let last = *route.last().unwrap_or(&home);
        for _ in 0..ROUTE_ATTEMPTS {
            let angle = world.rng.next_f64() as f32 * TAU;
            let distance = ROUTE_MIN + world.rng.next_f64() as f32 * (ROUTE_RADIUS - ROUTE_MIN);
            let at = [home[0] + angle.cos() * distance, home[1] + angle.sin() * distance];
            if open_ground(world, at[0], at[1]).is_some() && clear_line(world, line_point(world, last), line_point(world, at)) {
                route.push(at);
                break;
            }
        }
    }
    route
}

/// Someone seen: where they are (a soldier's centre, or their vehicle's), the point of them to aim
/// at, the vehicle they are in, and what they are to a soldier on foot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Seen {
    pub at: [f32; 3],
    pub mark: [f32; 3],
    pub vehicle: Option<u32>,
    pub threat: Threat,
}

/// Player `id` as seen from `eye`, if nothing stands between: on foot, their chest; in a vehicle with
/// glass or open sides, their head; in armour, its hull. The vehicle they are in does not hide them.
pub(crate) fn sighting(world: &World, eye: [f32; 3], id: u32) -> Option<Seen> {
    let player = world.entities.get::<Player>(id)?;
    if player.embodied() {
        let at = player.character.position;
        let mark = [at[0], at[1] + CHEST, at[2]];
        return clear_line(world, eye, mark).then_some(Seen { at, mark, vehicle: None, threat: Threat::Infantry });
    }
    let vid = player.seat.filter(|seat| !seat.remote)?.vehicle;
    let kind = world.entities.get::<crate::game::Vehicle>(vid)?.kind;
    let at = world.physics.car_pose(vid)?.position;
    let threat = if vehicle_def(kind).crew == Exposure::Sealed { Threat::Armour } else { Threat::Soft };
    let mark = match threat {
        Threat::Soft => world.seated_head(id).map_or(at, |(_, head)| head),
        _ => at,
    };
    let d = vec3::sub(mark, eye);
    let length = vec3::length(d);
    let clear = length < 0.01 || world.physics.cast_ray_hit(eye, d.map(|v| v / length), length, 0).is_none_or(|(t, hit)| t >= length - 0.2 || hit == Some(vid));
    clear.then_some(Seen { at, mark, vehicle: Some(vid), threat })
}

/// The nearest living player not on `id`'s team that it can see from `eye` looking along `yaw`, on
/// foot or in a vehicle, and how. A calm bot only sees what is in front of it, and none sees someone
/// hidden in a crop (`senses::crop_cover`).
fn spot(world: &World, id: u32, eye: [f32; 3], yaw: f32, alert: bool) -> Option<(u32, Seen)> {
    let facing = [-yaw.sin(), -yaw.cos()];
    let mut best: Option<(u32, Seen, f32)> = None;
    for (other, player) in world.entities.all::<Player>() {
        if other == id || !world.entities.alive(other) || player.surrendered || same_team(world, id, other) || super::is_noncombatant(world, other) {
            continue;
        }
        if !player.embodied() && player.seat.is_none_or(|seat| seat.remote) {
            continue;
        }
        // A police officer sees the one they are after and nobody else.
        if world.entities.get::<super::Officer>(id).is_some_and(|officer| officer.suspect != other) {
            continue;
        }
        let at = super::position_of(world, other).unwrap_or(player.character.position);
        let distance = distance_2d(eye, at);
        if distance > SIGHT_RANGE || best.is_some_and(|(_, _, nearest)| distance >= nearest) {
            continue;
        }
        if !alert && distance > CLOSE_IN {
            let cos = ((at[0] - eye[0]) * facing[0] + (at[2] - eye[2]) * facing[1]) / distance;
            if cos < CALM_VIEW_COS {
                continue;
            }
        }
        // Someone a crop hides is not there to be seen, even by a bot that was watching them: they are
        // gone as if behind a wall, and it goes to look where they were. Close in it finds them.
        if distance > CLOSE_IN && senses::crop_cover(world, player, eye) <= 0.0 {
            continue;
        }
        if let Some(seen) = sighting(world, eye, other) {
            best = Some((other, seen, distance));
        }
    }
    best.map(|(other, seen, _)| (other, seen))
}

/// Whether anything hears noises: a bot, a wild animal, the drivers of the traffic, or the emergency services.
fn listening(world: &World) -> bool {
    world.traffic > 0.0 || world.fleet.on() || world.entities.count::<Bot>() > 0 || world.entities.count::<super::wildlife::Wild>() > 0 || world.entities.count::<super::zombies::Zombie>() > 0
}

/// Notes a noise at `at` made by `by` that carries `reach` metres, for the bots to hear on the next
/// tick, and for wild animals to bolt from. With nobody to hear it, it is not kept.
pub fn make_noise(world: &mut World, at: [f32; 3], by: Option<u32>, reach: f32) {
    if listening(world) {
        world.send(Noise { at, by, reach, line: None });
    }
}

/// Notes a gunshot at `at` by `by`, its round going along `line`.
pub fn make_shot(world: &mut World, at: [f32; 3], by: Option<u32>, line: [f32; 3]) {
    if listening(world) {
        world.send(Noise { at, by, reach: HEAR_SHOT, line: Some(line) });
    }
}

/// Where the nearest of `noises` that bot `id`, standing at `at`, hears and would go to look at came
/// from: not its own, and not so far from its route that it would give up on the way. A teammate's
/// gunfire counts: it goes to help. Their engine does not.
fn hear(world: &World, id: u32, bot: &Bot, at: [f32; 3], noises: &[Noise]) -> Option<[f32; 3]> {
    noises
        .iter()
        .map(|noise| (noise, vec3::distance(noise.at, at)))
        .filter(|(noise, distance)| *distance <= noise.reach)
        .filter(|(noise, _)| off_route(bot, noise.at) < LEASH)
        .filter(|(noise, _)| noise.by != Some(id))
        // A police officer listens for the one they are after.
        .filter(|(noise, _)| world.entities.get::<super::Officer>(id).is_none_or(|officer| noise.by == Some(officer.suspect)))
        // An engine its own side drives is nothing to look at.
        .filter(|(noise, _)| noise.line.is_some() || noise.reach >= HEAR_BLAST || !noise.by.is_some_and(|by| same_team(world, id, by)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(noise, _)| noise.at)
}

/// How far `at` is from the bot's route: from the nearest of its waypoints, or from home while it has none.
fn off_route(bot: &Bot, at: [f32; 3]) -> f32 {
    let from = |[x, z]: [f32; 2]| (at[0] - x).hypot(at[2] - z);
    bot.route.iter().copied().map(from).fold(from(bot.home), f32::min)
}

/// Back to walking its route, from the waypoint nearest to where it is.
fn resume_patrol(bot: &mut Bot, at: [f32; 3]) {
    bot.mode = BotMode::Patrol;
    bot.walk.path.clear();
    bot.sight.target = None;
    bot.sight.sees = false;
    bot.next = (0..bot.route.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (bot.route[i][0] - at[0]).hypot(bot.route[i][1] - at[2]);
            d(a).total_cmp(&d(b))
        })
        .unwrap_or(0);
    new_goal(bot);
}

fn new_goal(bot: &mut Bot) {
    fresh_goal(&mut bot.walk);
}

/// Starts counting afresh how near it gets to where it is going.
pub(super) fn fresh_goal(walk: &mut Walk) {
    walk.best = f32::MAX;
    walk.stalled = 0;
}

/// The point to head for on the way to `goal`: the next corner of the way there. The way is looked
/// for when it has none, when the goal has moved, and every `REPLAN_TICKS` on the way, as long as
/// there is some of this tick's `budget` of cells left. With no way found it heads straight for the goal.
pub(super) fn follow(world: &mut World, walk: &mut Walk, at: [f32; 3], goal: [f32; 3], budget: &mut usize) -> [f32; 3] {
    let (here, to) = ([at[0], at[2]], [goal[0], goal[2]]);
    let feet = |p: [f32; 3]| [p[0], p[1] - sim::STANDING_Y, p[2]];
    walk.replan_in = walk.replan_in.saturating_sub(1);
    let moved = (to[0] - walk.path_to[0]).hypot(to[1] - walk.path_to[1]) > GOAL_MOVED;
    if (walk.path.is_empty() || moved || walk.replan_in == 0) && *budget > 0 {
        let was = walk.path.first().copied();
        walk.path = world.nav.find_path(&world.physics, &world.map, feet(at), feet(goal), budget).unwrap_or_else(|| vec![to]);
        walk.path_to = to;
        walk.replan_in = REPLAN_TICKS;
        // Only a new corner to head for is a fresh start: the same way again does not hide being stuck.
        if walk.path.first().copied() != was {
            fresh_goal(walk);
        }
    }
    while walk.path.len() > 1 && (walk.path[0][0] - here[0]).hypot(walk.path[0][1] - here[1]) < CORNER_REACHED {
        walk.path.remove(0);
        fresh_goal(walk);
    }
    let [x, z] = walk.path.first().copied().unwrap_or(to);
    [x, at[1], z]
}

/// Notes how much nearer `distance` is than the best so far; true once it has stopped getting nearer.
fn stalled(bot: &mut Bot, distance: f32) -> bool {
    stalled_by(bot, distance, PROGRESS, STALL_TICKS)
}

/// `stalled`, after `ticks` with no nearer by `progress` metres.
fn stalled_by(bot: &mut Bot, distance: f32, progress: f32, ticks: u32) -> bool {
    if distance < bot.walk.best - progress {
        bot.walk.best = distance;
        bot.walk.stalled = 0;
    } else {
        bot.walk.stalled += 1;
    }
    bot.walk.stalled > ticks
}

/// The heading to walk on towards `want`: straight, or the first detour with no wall close ahead.
pub(super) fn steer(world: &World, at: [f32; 3], want: f32) -> f32 {
    let free = |yaw: f32| world.physics.cast_ray(at, [-yaw.sin(), 0.0, -yaw.cos()], WALL_AHEAD, 0).is_none();
    if free(want) {
        return want;
    }
    DETOURS.iter().map(|offset| want + offset).find(|&yaw| free(yaw)).unwrap_or(want)
}

/// One tick of a fight with the target it can see, at `distance`, with the gun in `slot`: it turns
/// to where the shot has to go, closes to where that gun is good (or stands and sidesteps there), and
/// fires once it has reacted and is on aim.
#[allow(clippy::too_many_arguments)]
fn fight(world: &mut World, id: u32, bot: &mut Bot, eye: [f32; 3], slot: u8, distance: f32, dt: f32, away: [f32; 2], advance: bool) -> InputFrame {
    let Some(player) = world.entities.get::<Player>(id) else { return InputFrame::default() };
    let held = player.hands.hotbar[slot as usize];
    let (trigger_held, reloading) = (player.hands.trigger_held, player.hands.reload_left > 0);
    let Some(gun) = def(held.item).copied() else { return InputFrame::default() };
    bot.aim.tracked += 1;
    bot.aim.react = bot.aim.react.saturating_sub(1);
    bot.aim.pause = bot.aim.pause.saturating_sub(1);
    new_goal(bot);

    // Where they are by now, from where they were at the last look and how fast they were going. In
    // a vehicle that is the point of them it saw (their head, or the hull), which `aim_point` takes
    // for a chest.
    let since = world.tick.wrapping_sub(bot.sight.seen_tick) as f32 / TICK_RATE as f32;
    let seen = if bot.sight.vehicle.is_some() { [bot.sight.mark[0], bot.sight.mark[1] - CHEST, bot.sight.mark[2]] } else { bot.sight.last_seen };
    let mut now: [f32; 3] = vec3::along(seen, bot.sight.velocity, since);
    // `aim_point` goes for a standing chest, over the back of someone lying down.
    if bot.sight.target.and_then(|target| world.entities.get::<Player>(target)).is_some_and(|target| target.character.prone) {
        now[1] -= CHEST;
    }
    let aim = aim_point(eye, now, bot.sight.velocity, &gun, lerp(LEAD_POOR, 1.0, bot.aim.skill));
    // Its first rounds go wide to one side and walk in.
    let walk_in = lerp(WALK_IN_POOR, WALK_IN_GOOD, bot.aim.skill) * TICK_RATE as f32;
    let wide = match bot.role {
        Role::Attacker => MISS_BY * (-(bot.aim.tracked as f32) / walk_in).exp(),
        _ => SUPPRESS_BY,
    } / distance.max(CLOSE_IN);
    let want_yaw = heading(eye, aim) + bot.aim.error[0] + bot.aim.side[0] * wide;
    let want_pitch = ((aim[1] - eye[1]).atan2(distance_2d(eye, aim)) + bot.aim.error[1] + bot.aim.side[1] * wide).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    bot.yaw = turn_towards(bot.yaw, want_yaw, TURN_RATE * dt);
    bot.pitch = turn_towards(bot.pitch, want_pitch, TURN_RATE * dt);
    let on_aim = (turn_towards(bot.yaw, want_yaw, PI) - bot.yaw).abs() < FIRE_WITHIN && (want_pitch - bot.pitch).abs() < FIRE_WITHIN;

    let mut buttons = 0;
    // A launcher's rounds are its stack; a gun's, its magazine.
    let loaded = if gun.kind == ItemKind::Launcher { held.count > 0 } else { held.ammo > 0 };
    if bot.aim.react == 0 && on_aim && !reloading && loaded && distance <= gun.range && !nerve::frozen(bot) {
        let pull = if !gun.auto {
            // A fresh press for every round.
            !trigger_held
        } else if bot.aim.pause > 0 {
            false
        } else {
            if bot.aim.burst == 0 {
                let shots = BURST_SHOTS[0] + (world.rng.next_f64() * f64::from(BURST_SHOTS[1] - BURST_SHOTS[0] + 1)) as u32;
                bot.aim.burst = shots * gun.cooldown_ticks;
            }
            bot.aim.burst -= 1;
            if bot.aim.burst == 0 {
                let shaken = 1.0 + (1.0 - bot.nerve) * SHAKEN_PAUSE;
                let pause = lerp(BURST_PAUSE_POOR, BURST_PAUSE_GOOD, bot.aim.skill) * (0.75 + world.rng.next_f64() as f32 * 0.5) * shaken;
                bot.aim.pause = (pause * TICK_RATE as f32) as u32;
            }
            true
        };
        if pull {
            buttons |= BUTTON_FIRE;
        }
    }

    let (mut forward, mut strafe) = (0, 0);
    if advance && distance > stand_off(bot, held.item) && !nerve::broken(bot) && bot.role == Role::Attacker {
        forward = 127;
    } else {
        // Where it wants to be: settle, aim down the sights, and, if it knows how, keep moving sideways.
        let pose = nerve::pose(world, bot, held.item, distance);
        buttons |= BUTTON_ALT | pose;
        // Kneeling or flat it keeps still to shoot; on its feet it sidesteps.
        if bot.aim.skill >= STRAFE_SKILL && pose == 0 {
            bot.strafe_left = bot.strafe_left.saturating_sub(1);
            if bot.strafe_left == 0 {
                // A step one way, a moment still, and a step the other.
                bot.strafe = match (bot.strafe, bot.strafe_last) {
                    (0, 1) => -1,
                    (0, _) => 1,
                    _ => 0,
                };
                if bot.strafe != 0 {
                    bot.strafe_last = bot.strafe;
                }
                bot.strafe_left = (lerp(STRAFE_TICKS[0], STRAFE_TICKS[1], world.rng.next_f64() as f32) * TICK_RATE as f32) as u32;
            }
            strafe = (f32::from(bot.strafe) * SIDESTEP * 127.0) as i8;
        }
    }
    // Others close by push it sideways, whether it is closing in or standing off, so a squad
    // fanning out round one target does not stack up on one line.
    let lateral = away[0] * bot.yaw.cos() - away[1] * bot.yaw.sin();
    if lateral.abs() > PUSHED_FROM {
        strafe = (lateral.clamp(-1.0, 1.0) * PUSHED * 127.0) as i8;
    }
    InputFrame { forward, strafe, buttons, yaw: to_mrad(bot.yaw), pitch: to_mrad(bot.pitch), view: world.tick as u16, slot, ..Default::default() }
}

/// How close it comes to fight with `item`, metres: nearer the more aggressive it is.
fn stand_off(bot: &Bot, item: u8) -> f32 {
    hold_distance(item) * lerp(HOLD_SCALE[0], HOLD_SCALE[1], bot.temper.aggression)
}

/// Ticks of a random length between `range[0]` and `range[1]` seconds.
fn ticks_between(world: &mut World, range: [f32; 2]) -> u32 {
    (lerp(range[0], range[1], world.rng.next_f64() as f32) * TICK_RATE as f32) as u32
}

/// A spot `SHIFT_BY` round `target` from `at`, keeping its distance, on whichever side is open and
/// less in front of them (facing `yaw`).
fn shift_spot(world: &World, at: [f32; 3], target: [f32; 3], yaw: f32) -> Option<[f32; 3]> {
    let reach = distance_2d(at, target).max(1.0);
    let turn = SHIFT_BY / reach;
    let ahead = |p: [f32; 3]| (p[0] - target[0]) * -yaw.sin() + (p[2] - target[2]) * -yaw.cos();
    [turn, -turn]
        .into_iter()
        .filter_map(|turn| {
            let (sin, cos) = (heading(target, at) + turn).sin_cos();
            let (x, z) = (target[0] - sin * reach, target[2] - cos * reach);
            open_ground(world, x, z).map(|floor| [x, floor + sim::STANDING_Y, z])
        })
        .min_by(|&a, &b| ahead(a).total_cmp(&ahead(b)))
}

/// Adds a reload to `input` when the gun in hand is empty, or is not full and there is no one in sight.
fn reload(world: &World, id: u32, bot: &Bot, mut input: InputFrame) -> InputFrame {
    let Some(player) = world.entities.get::<Player>(id) else { return input };
    let held = player.hands.hotbar[(input.slot as usize).min(HOTBAR_SLOTS - 1)];
    let Some(gun) = def(held.item).filter(|d| d.kind == ItemKind::Gun) else { return input };
    if player.hands.reload_left == 0 && (held.ammo == 0 || (!bot.sight.sees && held.ammo < gun.mag)) {
        input.buttons |= BUTTON_RELOAD;
        input.buttons &= !BUTTON_FIRE;
    }
    input
}

/// One tick of bot `id`'s thinking: what it does now, as the input a player would send.
#[allow(clippy::too_many_arguments)]
fn think(world: &mut World, id: u32, bot: &mut Bot, dt: f32, noises: &[Noise], crowd: &[(u32, [f32; 2])], movers: &[evade::Mover], budget: &mut usize, rays: &mut usize) -> InputFrame {
    let idle = InputFrame { yaw: to_mrad(bot.yaw), pitch: to_mrad(bot.pitch), view: world.tick as u16, slot: bot.slot, ..Default::default() };
    let Some(player) = world.entities.get::<Player>(id) else { return idle };
    if !world.entities.alive(id) {
        // Whatever it was doing died with it; it comes back at home, on its route.
        bot.mode = BotMode::Patrol;
        bot.sight.target = None;
        bot.sight.sees = false;
        bot.next = 0;
        bot.aim.burst = 0;
        bot.aim.pause = 0;
        bot.hit_from = None;
        bot.walk.path.clear();
        bot.nerve = 1.0;
        bot.limbs = 0;
        bot.pose_left = 0;
        bot.cover = None;
        bot.cover_wait = 0;
        bot.role = Role::Attacker;
        bot.token_left = 0;
        bot.flank = None;
        bot.flanked = false;
        bot.pause_left = 0;
        bot.sight.aware = 0.0;
        bot.dodge = 0;
        bot.bound = 0;
        bot.shift = None;
        if let Some(mut player) = world.entities.get_mut::<Player>(id) {
            player.surrendered = false;
        }
        new_goal(bot);
        return idle;
    }
    if let Some(seat) = player.seat {
        // A crewman works its seat; anyone else just rides.
        return match bot.crew {
            Some(crew) if crew.vehicle == seat.vehicle && !seat.remote => crew::work(world, id, bot, seat.vehicle, seat.index, noises, dt),
            _ => idle,
        };
    }
    let at = player.character.position;
    let eye = player.eye();
    let stepping = player.input.forward != 0 || player.input.strafe != 0;
    let away = push(id, at, crowd);
    // A vehicle coming at it: out of the way first, whatever else it is about.
    if bot.dodge == 0
        && !matches!(bot.mode, BotMode::Surrender | BotMode::PlayDead)
        && (world.tick + id).is_multiple_of(LOOK_EVERY)
        && let Some(yaw) = evade::dodge(at, movers)
    {
        evade::start_dodge(bot, yaw);
    }
    if bot.dodge > 0 {
        bot.dodge -= 1;
        return evade::run_along(bot, bot.dodge_yaw, dt, idle);
    }
    if bot.route.is_empty() {
        bot.route = make_route(world, bot.home);
    }
    nerve::feel(world, id, bot, at, noises, dt);

    // Hit, or hearing a fight, while it is not in one itself: it goes to look where that came from,
    // alert, so it sees all round on the way. In cover it only notes where a hit came from.
    let hit = bot.hit_from.take();
    let shot_at = hit.is_some();
    if bot.mode.committed()
        && let Some(from) = hit
    {
        bot.sight.last_seen = from;
    }
    let heard = hit.or_else(|| {
        let busy = bot.mode == BotMode::Pursue;
        hear(world, id, bot, at, noises).map(|from| senses::roughly(id, at, from)).filter(|&from| !busy || distance_2d(at, from) + NEARER_NOISE < distance_2d(at, bot.sight.last_seen))
    });
    if let Some(from) = heard
        && !(bot.mode == BotMode::Pursue && bot.sight.sees)
        && !bot.mode.committed()
    {
        if bot.mode != BotMode::Pursue || distance_2d(from, bot.sight.last_seen) > GOAL_MOVED {
            tracing::debug!(bot = id, ?from, "bot heard something");
            new_goal(bot);
        }
        bot.mode = BotMode::Pursue;
        bot.sight.last_seen = from;
        bot.sight.sees = false;
    }

    if (world.tick + id).is_multiple_of(LOOK_EVERY) {
        // An officer knows whom they are looking for, and looks all round.
        let alert = bot.mode != BotMode::Patrol || world.entities.has::<super::Officer>(id);
        let in_view = spot(world, id, eye, bot.yaw, alert);
        if in_view.is_none() {
            senses::fade(bot);
        }
        match in_view.filter(|&(other, seen)| senses::notice(world, bot, other, seen.at, eye, alert)) {
            Some((other, seen)) => {
                let seen_at = seen.at;
                let same = (bot.mode == BotMode::Pursue || bot.mode.committed()) && bot.sight.target == Some(other);
                if !same {
                    tracing::debug!(bot = id, target = other, "bot spotted someone");
                    new_goal(bot);
                }
                // How fast they are going, from where they were at the last look.
                let since = world.tick.wrapping_sub(bot.sight.seen_tick) as f32 / TICK_RATE as f32;
                let moved: [f32; 3] = std::array::from_fn(|i| (seen_at[i] - bot.sight.last_seen[i]) / since.max(1e-3));
                let speed = vec3::length(moved);
                let top = if seen.vehicle.is_some() { TOP_DRIVING } else { TOP_SPEED };
                bot.sight.velocity = if same && bot.sight.sees && speed < top { std::array::from_fn(|i| (bot.sight.velocity[i] + moved[i]) / 2.0) } else { [0.0; 3] };
                if !same || !bot.sight.sees {
                    // Caught sight of them: it takes a moment to react, and its aim has to settle.
                    let startled = if bot.mode == BotMode::Patrol { world.rng.next_f64() as f32 * STARTLED * (1.0 - bot.aim.skill) } else { 0.0 };
                    bot.aim.react = ((lerp(REACT_POOR, REACT_GOOD, bot.aim.skill) + startled) * TICK_RATE as f32) as u32;
                    bot.aim.tracked = 0;
                    bot.aim.side = [if world.rng.next_f64() < 0.5 { -1.0 } else { 1.0 }, wobble(world) * 0.5];
                    // A bound starts with it down and firing.
                    (bot.bound, bot.bounding) = (0, true);
                }
                let settled = 1.0 + UNSETTLED * (-(bot.aim.tracked as f32) / (SETTLE * TICK_RATE as f32)).exp();
                let shaken = 1.0 + (1.0 - bot.nerve) * SHAKEN_AIM;
                let moving = 1.0 + if stepping { MOVING_AIM } else { 0.0 } + vec3::length(bot.sight.velocity).min(FAST) * TARGET_SPEED_AIM;
                let off = lerp(AIM_POOR, AIM_GOOD, bot.aim.skill) * settled * shaken * moving;
                bot.aim.error = [wobble(world) * off, wobble(world) * off];
                if !bot.mode.committed() {
                    bot.mode = BotMode::Pursue;
                }
                bot.sight.target = Some(other);
                (bot.sight.threat, bot.sight.vehicle, bot.sight.mark) = (seen.threat, seen.vehicle, seen.mark);
                bot.sight.last_seen = seen_at;
                bot.sight.seen_tick = world.tick;
                bot.sight.sees = true;
            }
            None => bot.sight.sees = false,
        }
    }
    let Some(player) = world.entities.get::<Player>(id) else { return idle };
    let fighting = bot.mode == BotMode::Pursue && bot.sight.sees;
    if !fighting {
        bot.pitch = turn_towards(bot.pitch, 0.0, TURN_RATE * dt);
    }
    let distance = if fighting { distance_2d(at, bot.sight.last_seen) } else { PATROL_DISTANCE };
    let threat = if fighting { bot.sight.threat } else { Threat::Infantry };
    // An officer keeps an anti-tank weapon for a tank.
    let officer = world.entities.has::<super::Officer>(id);
    let gun = pick_gun(&player.hands.hotbar, distance, bot.slot, if officer && threat == Threat::Soft { Threat::Infantry } else { threat });
    // What it has for anything that is not a tank, to fire on one it cannot hurt.
    let small_arm = pick_gun(&player.hands.hotbar, distance_2d(at, bot.sight.last_seen), bot.slot, Threat::Infantry).filter(|&slot| !anti_tank(&player.hands.hotbar[slot as usize]));
    let carries_anti_tank = player.hands.hotbar.iter().any(anti_tank);
    if let Some(slot) = gun {
        bot.slot = slot;
    }
    let idle = InputFrame { slot: bot.slot, ..idle };
    let out_of_rounds = fighting && player.hands.hotbar[bot.slot as usize].ammo == 0 && def(player.hands.hotbar[bot.slot as usize].item).is_some_and(|d| d.kind == ItemKind::Gun);
    // Hit by someone it cannot see: it gets its head down first and finds them after.
    let blind = shot_at && !bot.sight.sees;
    // A tank it cannot hurt, or one too close to fight: it gets away from it.
    if fighting && threat == Threat::Armour && (gun.is_none() || distance < TOO_CLOSE) && matches!(bot.mode, BotMode::Pursue | BotMode::Evade) {
        evade::start(world, bot, false);
    }
    // Far enough again from a tank it has something for: it turns to fight it with that.
    if bot.mode == BotMode::Evade && bot.sight.sees && bot.sight.threat == Threat::Armour && carries_anti_tank && distance_2d(at, bot.sight.last_seen) > FIGHT_FROM && !nerve::broken(bot) {
        (bot.mode, bot.cover) = (BotMode::Pursue, None);
        new_goal(bot);
    }
    if bot.mode == BotMode::Pursue && (nerve::broken(bot) || out_of_rounds || blind) {
        cover::seek(world, bot, at, rays, budget);
        // A coward that has broken and found nowhere to hide runs for it.
        if bot.mode == BotMode::Pursue && nerve::broken(bot) && !nerve::frozen(bot) && bot.temper.courage < COWARD && bot.cover_wait > 0 && fighting && !world.entities.has::<crate::mechanics::Officer>(id) {
            evade::start(world, bot, true);
        }
    }
    if bot.mode == BotMode::Pursue && fighting && nerve::frozen(bot) && bot.cover_wait > 0 && !world.entities.has::<super::Officer>(id) {
        wounded::give_in(world, id, bot, at, crowd);
    }
    let Some(player) = world.entities.get::<Player>(id) else { return idle };
    let reloading = player.hands.reload_left > 0;

    // A crewman on foot goes back to its tank, unless it is fighting someone it can see.
    let boarding = match bot.crew {
        Some(crew) if !(bot.mode == BotMode::Pursue && bot.sight.sees) => crew::board(world, id, crew, at),
        _ => crew::Boarding::No,
    };
    let (goal, run) = 'plan: { match bot.mode {
        _ if matches!(boarding, crew::Boarding::Aboard) => return idle,
        _ if let crew::Boarding::Walk(goal) = boarding => (goal, true),
        BotMode::Patrol => {
            let Some(&[x, z]) = bot.route.get(bot.next) else { return idle };
            let goal = [x, at[1], z];
            if bot.sight.aware >= senses::SUSPICIOUS {
                // Something there: it stops and stares.
                bot.yaw = turn_towards(bot.yaw, heading(at, bot.sight.last_seen), TURN_RATE * dt);
                return InputFrame { yaw: to_mrad(bot.yaw), ..idle };
            }
            if bot.pause_left > 0 {
                bot.pause_left -= 1;
                let swing = if (bot.pause_left / TICK_RATE).is_multiple_of(2) { 1.0 } else { -1.0 };
                bot.yaw = (bot.yaw + swing * LOOK_ABOUT_RATE * dt + PI).rem_euclid(TAU) - PI;
                return InputFrame { yaw: to_mrad(bot.yaw), ..idle };
            }
            if distance_2d(at, goal) < REACHED {
                let skip = if bot.route.len() > 2 && world.rng.next_f64() < SKIP_CHANCE { 2 } else { 1 };
                bot.next = (bot.next + skip) % bot.route.len();
                // A sentry keeps its eyes front.
                if bot.route.len() > 1 && world.rng.next_f64() < PAUSE_CHANCE {
                    bot.pause_left = (lerp(PAUSE[0], PAUSE[1], world.rng.next_f64() as f32) * TICK_RATE as f32) as u32;
                }
                bot.pace = lerp(SLOWEST_PACE, 127.0, world.rng.next_f64() as f32) as i8;
                new_goal(bot);
                return idle;
            }
            (goal, false)
        }
        BotMode::Pursue => {
            let goal = bot.sight.last_seen;
            let distance = distance_2d(at, goal);
            if off_route(bot, at) > LEASH {
                resume_patrol(bot, at);
                return idle;
            }
            if let Some(point) = tokens::flank_goal(bot, at) {
                break 'plan (point, true);
            }
            if bot.role == Role::Holder && !bot.sight.sees && distance < tokens::HOLD_OFF {
                // No place for it yet: it keeps down and watches.
                bot.yaw = turn_towards(bot.yaw, heading(at, goal), TURN_RATE * dt);
                return InputFrame { yaw: to_mrad(bot.yaw), buttons: BUTTON_CROUCH, ..idle };
            }
            if bot.sight.sees && let Some(slot) = gun {
                let item = world.entities.get::<Player>(id).map_or(0, |player| player.hands.hotbar[slot as usize].item);
                let far = distance > stand_off(bot, item);
                // Far off: it closes in bounds, running a few seconds by the way round things, then
                // down to fire a few, and on again.
                if far && distance > BOUND_FROM && bot.role == Role::Attacker && !nerve::broken(bot) {
                    bot.shift = None;
                    bot.bound = bot.bound.saturating_sub(1);
                    if bot.bound == 0 {
                        bot.bounding = !bot.bounding;
                        bot.bound = ticks_between(world, if bot.bounding { BOUND_RUN } else { BOUND_FIRE });
                        new_goal(bot);
                    }
                    if bot.bounding {
                        break 'plan (goal, true);
                    }
                    return fight(world, id, bot, eye, slot, distance, dt, away, false);
                }
                bot.bounding = false;
                // Standing off: now and then it moves to another spot round them before it fires again.
                if !far && distance > BOUND_FROM && bot.role == Role::Attacker && !nerve::broken(bot) {
                    if let Some(point) = bot.shift {
                        bot.shift_wait = bot.shift_wait.saturating_sub(1);
                        if distance_2d(at, point) > REACHED * 1.5 && bot.shift_wait > 0 {
                            break 'plan (point, true);
                        }
                        bot.shift = None;
                        bot.shift_wait = ticks_between(world, SHIFT_EVERY);
                    } else if bot.shift_wait == 0 {
                        let yaw = bot.sight.target.and_then(|target| world.entities.get::<Player>(target)).map_or(0.0, |target| target.character.yaw);
                        bot.shift = shift_spot(world, at, goal, yaw);
                        bot.shift_wait = if bot.shift.is_some() { (SHIFT_FOR * TICK_RATE as f32) as u32 } else { ticks_between(world, SHIFT_EVERY) };
                        new_goal(bot);
                    } else {
                        bot.shift_wait -= 1;
                    }
                }
                return fight(world, id, bot, eye, slot, distance, dt, away, true);
            }
            if bot.sight.sees && distance < CLOSE_IN {
                // Unarmed and up close: stand and face them.
                bot.yaw = turn_towards(bot.yaw, heading(at, goal), TURN_RATE * dt);
                return InputFrame { yaw: to_mrad(bot.yaw), ..idle };
            }
            if !bot.sight.sees && distance < REACHED {
                bot.mode = BotMode::Search;
                bot.mode_left = SEARCH_TICKS;
                return idle;
            }
            (goal, true)
        }
        BotMode::Search => {
            bot.mode_left = bot.mode_left.saturating_sub(1);
            if bot.mode_left == 0 {
                resume_patrol(bot, at);
                return idle;
            }
            bot.yaw = (bot.yaw + SEARCH_TURN_RATE * dt + PI).rem_euclid(TAU) - PI;
            return InputFrame { yaw: to_mrad(bot.yaw), ..idle };
        }
        BotMode::Cover => {
            let Some(goal) = bot.cover else {
                cover::leave(world, bot, at);
                return idle;
            };
            bot.mode_left = bot.mode_left.saturating_sub(1);
            let there = distance_2d(at, goal) < REACHED;
            if there
                && !bot.sight.sees
                && let Some(input) = wounded::bandage(world, id, idle)
            {
                bot.mode_left = bot.mode_left.max(1);
                return input;
            }
            // Seen at its spot, the spot is no good; and it does not run for ever.
            if bot.mode_left == 0 || (there && (bot.sight.sees || (nerve::steady(bot) && !reloading))) {
                cover::leave(world, bot, at);
                return idle;
            }
            if there {
                bot.yaw = turn_towards(bot.yaw, heading(at, bot.sight.last_seen), TURN_RATE * dt);
                return InputFrame { yaw: to_mrad(bot.yaw), buttons: BUTTON_CROUCH, ..idle };
            }
            (goal, true)
        }
        BotMode::Surrender => {
            bot.mode_left = bot.mode_left.saturating_sub(1);
            if bot.mode_left == 0 {
                wounded::rise(world, id, bot, at);
            }
            return InputFrame { yaw: to_mrad(bot.yaw), ..idle };
        }
        BotMode::PlayDead => {
            bot.mode_left = bot.mode_left.saturating_sub(1);
            if wounded::spring(bot, at) {
                return idle;
            }
            if bot.mode_left == 0 {
                wounded::rise(world, id, bot, at);
            }
            return InputFrame { buttons: BUTTON_PRONE, yaw: to_mrad(bot.yaw), ..idle };
        }
        BotMode::Evade => match evade::plan(world, id, bot, at, dt, rays, budget, idle) {
            Ok(plan) => plan,
            // Kept down out of the way of a tank, now and then it fires on it anyway.
            Err(_) if let Some(slot) = small_arm
                && evade::harass(world, bot, at) =>
            {
                bot.slot = slot;
                return fight(world, id, bot, eye, slot, distance_2d(at, bot.sight.last_seen), dt, away, false);
            }
            Err(input) => return input,
        },
    } };

    let toward = follow(world, &mut bot.walk, at, goal, budget);
    if stalled(bot, distance_2d(at, toward)) {
        // Stuck: on patrol it moves on to the next waypoint, and in a pursuit it looks round here.
        bot.walk.path.clear();
        new_goal(bot);
        if bot.mode == BotMode::Patrol {
            bot.next = (bot.next + 1) % bot.route.len();
        } else {
            bot.mode = BotMode::Search;
            bot.mode_left = SEARCH_TICKS;
        }
        return idle;
    }
    let want = steer(world, at, bend(heading(at, toward), away));
    bot.yaw = turn_towards(bot.yaw, want, TURN_RATE * dt);
    let facing = (turn_towards(bot.yaw, want, PI) - bot.yaw).abs() < WALK_WITHIN;
    InputFrame {
        forward: if !facing { 0 } else if run { 127 } else { bot.pace },
        buttons: if run { BUTTON_RUN } else { 0 },
        ..InputFrame { yaw: to_mrad(bot.yaw), ..idle }
    }
}

impl Mechanic for Bots {
    fn on_leave(&mut self, world: &mut World, id: u32) {
        world.entities.remove::<Bot>(id);
    }

    /// A bot hurt, or in a vehicle that was, turns to where it came from.
    fn react(&mut self, world: &mut World) {
        let hurt: Vec<(u32, [f32; 3], u8)> = world
            .happened::<Damaged>()
            .filter_map(|damaged| Some((damaged, damaged.from?)))
            .flat_map(|(damaged, from)| std::iter::once(damaged.victim).chain(damaged.aboard.iter().copied()).map(move |id| (id, from, damaged.lost)))
            .collect();
        for (id, from, lost) in hurt {
            // An officer turns only to their suspect's fire.
            let suspect = world.entities.get::<super::Officer>(id).map(|officer| officer.suspect);
            if suspect.is_some() && !world.happened::<Damaged>().any(|damaged| (damaged.victim == id || damaged.aboard.contains(&id)) && damaged.attacker == suspect) {
                continue;
            }
            if let Some(mut bot) = world.entities.get_mut::<Bot>(id) {
                bot.hit_from = Some(from);
                nerve::shake(&mut bot, nerve::hit_shock(lost));
            }
        }
        // A soldier killed shakes the rest of its side nearby.
        let killed: Vec<(u32, [f32; 3])> = world.happened::<Damaged>().filter(|damaged| damaged.killed).map(|damaged| (damaged.victim, damaged.at)).collect();
        for (victim, at) in killed {
            for id in world.entities.ids::<With<Bot>>() {
                let near = world.entities.get::<Player>(id).is_some_and(|player| nerve::near_squadmate(player.character.position, at));
                if id != victim && near && same_team(world, id, victim)
                    && let Some(mut bot) = world.entities.get_mut::<Bot>(id)
                {
                    nerve::shake(&mut bot, nerve::squadmate_shock());
                }
            }
        }
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        for area in world.physics.take_changed() {
            world.nav.forget(area);
        }
        let noises: Vec<Noise> = self.heard.read(world.events::<Noise>()).copied().collect();
        let mut budget = SEARCH_PER_TICK;
        let mut rays = cover::RAYS_PER_TICK;
        let ids = world.entities.ids::<With<Bot>>();
        let movers = if ids.is_empty() { Vec::new() } else { evade::movers(world) };
        if world.tick.is_multiple_of(tokens::ASSIGN_EVERY) {
            tokens::assign(world, &ids);
        }
        // Where every bot on its feet stands as the tick begins, for them to steer clear of each other.
        let crowd: Vec<(u32, [f32; 2])> = ids
            .iter()
            .filter_map(|id| world.entities.get::<Player>(*id).filter(|p| world.entities.alive(*id) && p.seat.is_none()).map(|p| (*id, [p.character.position[0], p.character.position[2]])))
            .collect();
        for id in ids {
            // Taken out to think with the world in hand, an empty mind in its place: what a bot thinks
            // reads only other bots' minds (a driver its gunner's).
            // A police officer is a soldier only when it has come to shooting; the rest of the time
            // the police's own mind has them (`emergency::officers`).
            if world.entities.get::<super::Officer>(id).is_some_and(|officer| !officer.armed) {
                continue;
            }
            let Some(mut mind) = world.entities.get_mut::<Bot>(id) else { continue };
            let mut bot = std::mem::replace(&mut *mind, Bot::new([0.0; 2], Vec::new()));
            let input = think(world, id, &mut bot, dt, &noises, &crowd, &movers, &mut budget, &mut rays);
            let input = if matches!(bot.mode, BotMode::Surrender | BotMode::PlayDead) { input } else { reload(world, id, &bot, input) };
            *world.entities.of_mut::<Bot>(id) = bot;
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.input = input;
            }
        }
    }
}

#[cfg(test)]
mod tests;
