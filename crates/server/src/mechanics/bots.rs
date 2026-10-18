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
//! in the way, such as a vehicle (see BOTS.md).

mod crew;

pub use crew::road_route;
pub use crew::{DRIVER, GUNNER, nearest_waypoint};

use super::Mechanic;
use super::combat::same_team;
use super::npcs::{open_ground, turn_towards};
use crate::game::{Bot, BotMode, Noise, World};
use shared::consts::*;
use shared::items::{HOTBAR_SLOTS, ITEM_AK5, ITEM_REVOLVER, ITEM_SHOTGUN, ITEM_SNIPER, ItemDef, ItemKind, def};
use shared::protocol::{InputFrame, ItemSlot};
use shared::quant::to_mrad;
use std::f32::consts::{PI, TAU};

pub struct Bots;

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
/// It looks up or down no further than this, radians.
const PITCH_LIMIT: f32 = 1.4;
/// Fastest it believes anyone moves, m/s: past that a sighting is a respawn or a teleport, not speed.
const TOP_SPEED: f32 = 20.0;
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
/// From this skill up it sidesteps while it stands and fights, a way for 0.6 to 1.6 s at a time.
const STRAFE_SKILL: f32 = 0.3;
const STRAFE_TICKS: [f32; 2] = [0.6, 1.6];

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// How close it comes, metres, before it stands and fights with `item`.
fn hold_distance(item: u8) -> f32 {
    match item {
        ITEM_SHOTGUN => 5.0,
        ITEM_AK5 => 15.0,
        ITEM_SNIPER => 35.0,
        _ => 12.0,
    }
}

/// The slot to fight with at `distance`: the shotgun up close, else the AK5, the revolver or the
/// sniper, whichever it has, else any gun. `None` when it has no gun. Holding the shotgun it keeps it
/// a little past where it would take it up, so it does not swap back and forth at the edge.
fn pick_gun(hotbar: &[ItemSlot; HOTBAR_SLOTS], distance: f32, current: u8) -> Option<u8> {
    let slot_of = |item: u8| hotbar.iter().position(|slot| slot.item == item).map(|i| i as u8);
    let holds_shotgun = hotbar.get(current as usize).is_some_and(|slot| slot.item == ITEM_SHOTGUN);
    let close = distance < if holds_shotgun { SHOTGUN_OUT } else { SHOTGUN_IN };
    let order = if close { [ITEM_SHOTGUN, ITEM_AK5, ITEM_REVOLVER, ITEM_SNIPER] } else { [ITEM_AK5, ITEM_REVOLVER, ITEM_SNIPER, ITEM_SHOTGUN] };
    order.into_iter().find_map(slot_of).or_else(|| hotbar.iter().position(|slot| def(slot.item).is_some_and(|d| d.kind == ItemKind::Gun)).map(|i| i as u8))
}

/// Where to put the shot so it meets a body now at `target` moving at `velocity`: ahead of it by
/// `lead` (0 to 1) of how far it goes while the round flies from `eye`, and above it by how far a
/// flying round drops on the way. A hitscan round arrives at once.
pub fn aim_point(eye: [f32; 3], target: [f32; 3], velocity: [f32; 3], gun: &ItemDef, lead: f32) -> [f32; 3] {
    let chest = [target[0], target[1] + CHEST, target[2]];
    if !gun.projectile {
        return chest;
    }
    let flight = (0..3).map(|i| (chest[i] - eye[i]).powi(2)).sum::<f32>().sqrt() / BULLET_SPEED;
    let drop = 0.5 * GRAVITY * BULLET_GRAVITY_SCALE * flight * flight;
    let mut aim: [f32; 3] = std::array::from_fn(|i| chest[i] + velocity[i] * flight * lead);
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
fn push(id: u32, at: [f32; 3], crowd: &[(u32, [f32; 2])]) -> [f32; 2] {
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
fn bend(yaw: f32, push: [f32; 2]) -> f32 {
    let (x, z) = (-yaw.sin() + push[0] * PUSH_WEIGHT, -yaw.cos() + push[1] * PUSH_WEIGHT);
    if x.hypot(z) < 1e-3 { yaw } else { (-x).atan2(-z) }
}

/// The heading that faces from `from` to `to`. Yaw 0 faces -Z: forward is (-sin yaw, -cos yaw).
fn heading(from: [f32; 3], to: [f32; 3]) -> f32 {
    (-(to[0] - from[0])).atan2(-(to[2] - from[2]))
}

/// Whether nothing (the world, a building, a vehicle) stands between `from` and `to`.
pub fn clear_line(world: &World, from: [f32; 3], to: [f32; 3]) -> bool {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if length < 0.01 {
        return true;
    }
    let dir = d.map(|v| v / length);
    world.physics.cast_ray(from, dir, length, 0).is_none_or(|t| t >= length - 0.2)
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

/// The nearest living, standing player not on `id`'s team that it can see from `eye` looking along
/// `yaw`, and where they stand. A calm bot only sees what is in front of it.
fn spot(world: &World, id: u32, eye: [f32; 3], yaw: f32, alert: bool) -> Option<(u32, [f32; 3])> {
    let facing = [-yaw.sin(), -yaw.cos()];
    let mut best: Option<(u32, [f32; 3], f32)> = None;
    for (&other, player) in &world.players {
        if other == id || !player.alive() || !player.embodied() || same_team(world, id, other) {
            continue;
        }
        let at = player.character.position;
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
        if clear_line(world, eye, [at[0], at[1] + CHEST, at[2]]) {
            best = Some((other, at, distance));
        }
    }
    best.map(|(other, at, _)| (other, at))
}

/// Notes a noise at `at` made by `by` that carries `reach` metres, for the bots to hear on the next
/// tick. With no bots to hear it, it is not kept.
pub fn make_noise(world: &mut World, at: [f32; 3], by: Option<u32>, reach: f32) {
    if !world.bots.is_empty() {
        world.noises.push(Noise { at, by, reach });
    }
}

/// Where the nearest of `noises` that bot `id`, standing at `at`, hears and would go to look at came
/// from: not its own, and not so far from its route that it would give up on the way. A teammate's
/// gunfire counts: it goes to help.
fn hear(id: u32, bot: &Bot, at: [f32; 3], noises: &[Noise]) -> Option<[f32; 3]> {
    noises
        .iter()
        .map(|noise| (noise, (0..3).map(|i| (noise.at[i] - at[i]).powi(2)).sum::<f32>().sqrt()))
        .filter(|(noise, distance)| *distance <= noise.reach)
        .filter(|(noise, _)| off_route(bot, noise.at) < LEASH)
        .filter(|(noise, _)| noise.by != Some(id))
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
    bot.path.clear();
    bot.target = None;
    bot.sees = false;
    bot.next = (0..bot.route.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (bot.route[i][0] - at[0]).hypot(bot.route[i][1] - at[2]);
            d(a).total_cmp(&d(b))
        })
        .unwrap_or(0);
    new_goal(bot);
}

fn new_goal(bot: &mut Bot) {
    bot.best = f32::MAX;
    bot.stalled = 0;
}

/// The point to head for on the way to `goal`: the next corner of the way there. The way is looked
/// for when it has none, when the goal has moved, and every `REPLAN_TICKS` on the way, as long as
/// there is some of this tick's `budget` of cells left. With no way found it heads straight for the goal.
fn follow(world: &mut World, bot: &mut Bot, at: [f32; 3], goal: [f32; 3], budget: &mut usize) -> [f32; 3] {
    let (here, to) = ([at[0], at[2]], [goal[0], goal[2]]);
    let feet = |p: [f32; 3]| [p[0], p[1] - crate::physics::STANDING_Y, p[2]];
    bot.replan_in = bot.replan_in.saturating_sub(1);
    let moved = (to[0] - bot.path_to[0]).hypot(to[1] - bot.path_to[1]) > GOAL_MOVED;
    if (bot.path.is_empty() || moved || bot.replan_in == 0) && *budget > 0 {
        let was = bot.path.first().copied();
        bot.path = world.nav.find_path(&world.physics, &world.map, feet(at), feet(goal), budget).unwrap_or_else(|| vec![to]);
        bot.path_to = to;
        bot.replan_in = REPLAN_TICKS;
        // Only a new corner to head for is a fresh start: the same way again does not hide being stuck.
        if bot.path.first().copied() != was {
            new_goal(bot);
        }
    }
    while bot.path.len() > 1 && (bot.path[0][0] - here[0]).hypot(bot.path[0][1] - here[1]) < CORNER_REACHED {
        bot.path.remove(0);
        new_goal(bot);
    }
    let [x, z] = bot.path.first().copied().unwrap_or(to);
    [x, at[1], z]
}

/// Notes how much nearer `distance` is than the best so far; true once it has stopped getting nearer.
fn stalled(bot: &mut Bot, distance: f32) -> bool {
    stalled_by(bot, distance, PROGRESS, STALL_TICKS)
}

/// `stalled`, after `ticks` with no nearer by `progress` metres.
fn stalled_by(bot: &mut Bot, distance: f32, progress: f32, ticks: u32) -> bool {
    if distance < bot.best - progress {
        bot.best = distance;
        bot.stalled = 0;
    } else {
        bot.stalled += 1;
    }
    bot.stalled > ticks
}

/// The heading to walk on towards `want`: straight, or the first detour with no wall close ahead.
fn steer(world: &World, at: [f32; 3], want: f32) -> f32 {
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
fn fight(world: &mut World, id: u32, bot: &mut Bot, eye: [f32; 3], slot: u8, distance: f32, dt: f32, away: [f32; 2]) -> InputFrame {
    let Some(player) = world.players.get(&id) else { return InputFrame::default() };
    let held = player.hotbar[slot as usize];
    let (trigger_held, reloading) = (player.trigger_held, player.reload_left > 0);
    let Some(gun) = def(held.item).copied() else { return InputFrame::default() };
    bot.tracked += 1;
    bot.react = bot.react.saturating_sub(1);
    bot.pause = bot.pause.saturating_sub(1);
    new_goal(bot);

    // Where they are by now, from where they were at the last look and how fast they were going.
    let since = world.tick.wrapping_sub(bot.seen_tick) as f32 / TICK_RATE as f32;
    let now: [f32; 3] = std::array::from_fn(|i| bot.last_seen[i] + bot.velocity[i] * since);
    let aim = aim_point(eye, now, bot.velocity, &gun, lerp(LEAD_POOR, 1.0, bot.skill));
    let want_yaw = heading(eye, aim) + bot.error[0];
    let want_pitch = ((aim[1] - eye[1]).atan2(distance_2d(eye, aim)) + bot.error[1]).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    bot.yaw = turn_towards(bot.yaw, want_yaw, TURN_RATE * dt);
    bot.pitch = turn_towards(bot.pitch, want_pitch, TURN_RATE * dt);
    let on_aim = (turn_towards(bot.yaw, want_yaw, PI) - bot.yaw).abs() < FIRE_WITHIN && (want_pitch - bot.pitch).abs() < FIRE_WITHIN;

    let mut buttons = 0;
    if bot.react == 0 && on_aim && !reloading && held.ammo > 0 && distance <= gun.range {
        let pull = if !gun.auto {
            // A fresh press for every round.
            !trigger_held
        } else if bot.pause > 0 {
            false
        } else {
            if bot.burst == 0 {
                let shots = BURST_SHOTS[0] + (world.rng.next_f64() * f64::from(BURST_SHOTS[1] - BURST_SHOTS[0] + 1)) as u32;
                bot.burst = shots * gun.cooldown_ticks;
            }
            bot.burst -= 1;
            if bot.burst == 0 {
                let pause = lerp(BURST_PAUSE_POOR, BURST_PAUSE_GOOD, bot.skill) * (0.75 + world.rng.next_f64() as f32 * 0.5);
                bot.pause = (pause * TICK_RATE as f32) as u32;
            }
            true
        };
        if pull {
            buttons |= BUTTON_FIRE;
        }
    }

    let (mut forward, mut strafe) = (0, 0);
    if distance > hold_distance(held.item) {
        forward = 127;
    } else {
        // Where it wants to be: stand, aim down the sights, and, if it knows how, keep moving sideways.
        buttons |= BUTTON_ALT;
        if bot.skill >= STRAFE_SKILL {
            bot.strafe_left = bot.strafe_left.saturating_sub(1);
            if bot.strafe_left == 0 {
                bot.strafe = if bot.strafe == 0 { if world.rng.next_f64() < 0.5 { -1 } else { 1 } } else { -bot.strafe };
                bot.strafe_left = (lerp(STRAFE_TICKS[0], STRAFE_TICKS[1], world.rng.next_f64() as f32) * TICK_RATE as f32) as u32;
            }
            strafe = bot.strafe * 127;
        }
    }
    // Others close by push it sideways, whether it is closing in or standing off, so a squad
    // fanning out round one target does not stack up on one line.
    let lateral = away[0] * bot.yaw.cos() - away[1] * bot.yaw.sin();
    if lateral.abs() > 0.1 {
        strafe = (lateral * 127.0).clamp(-127.0, 127.0) as i8;
    }
    InputFrame { forward, strafe, buttons, yaw: to_mrad(bot.yaw), pitch: to_mrad(bot.pitch), view: world.tick as u16, slot, ..Default::default() }
}

/// Adds a reload to `input` when the gun in hand is empty, or is not full and there is no one in sight.
fn reload(world: &World, id: u32, bot: &Bot, mut input: InputFrame) -> InputFrame {
    let Some(player) = world.players.get(&id) else { return input };
    let held = player.hotbar[(input.slot as usize).min(HOTBAR_SLOTS - 1)];
    let Some(gun) = def(held.item).filter(|d| d.kind == ItemKind::Gun) else { return input };
    if player.reload_left == 0 && (held.ammo == 0 || (!bot.sees && held.ammo < gun.mag)) {
        input.buttons |= BUTTON_RELOAD;
        input.buttons &= !BUTTON_FIRE;
    }
    input
}

/// One tick of bot `id`'s thinking: what it does now, as the input a player would send.
fn think(world: &mut World, id: u32, bot: &mut Bot, dt: f32, noises: &[Noise], crowd: &[(u32, [f32; 2])], budget: &mut usize) -> InputFrame {
    let idle = InputFrame { yaw: to_mrad(bot.yaw), pitch: to_mrad(bot.pitch), view: world.tick as u16, slot: bot.slot, ..Default::default() };
    let Some(player) = world.players.get(&id) else { return idle };
    if !player.alive() {
        // Whatever it was doing died with it; it comes back at home, on its route.
        bot.mode = BotMode::Patrol;
        bot.target = None;
        bot.sees = false;
        bot.next = 0;
        bot.burst = 0;
        bot.pause = 0;
        bot.hit_from = None;
        bot.path.clear();
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
    let eye = [at[0], at[1] + EYE_HEIGHT, at[2]];
    let away = push(id, at, crowd);
    if bot.route.is_empty() {
        bot.route = make_route(world, bot.home);
    }

    // Hit, or hearing a fight, while it is not in one itself: it goes to look where that came from,
    // alert, so it sees all round on the way.
    let heard = bot.hit_from.take().or_else(|| {
        let busy = bot.mode == BotMode::Pursue;
        hear(id, bot, at, noises).filter(|&from| !busy || distance_2d(at, from) + NEARER_NOISE < distance_2d(at, bot.last_seen))
    });
    if let Some(from) = heard
        && !(bot.mode == BotMode::Pursue && bot.sees)
    {
        if bot.mode != BotMode::Pursue || distance_2d(from, bot.last_seen) > GOAL_MOVED {
            tracing::debug!(bot = id, ?from, "bot heard something");
            new_goal(bot);
        }
        bot.mode = BotMode::Pursue;
        bot.last_seen = from;
        bot.sees = false;
    }

    if (world.tick + id).is_multiple_of(LOOK_EVERY) {
        let alert = bot.mode != BotMode::Patrol;
        match spot(world, id, eye, bot.yaw, alert) {
            Some((other, seen_at)) => {
                let same = bot.mode == BotMode::Pursue && bot.target == Some(other);
                if !same {
                    tracing::debug!(bot = id, target = other, "bot spotted someone");
                    new_goal(bot);
                }
                // How fast they are going, from where they were at the last look.
                let since = world.tick.wrapping_sub(bot.seen_tick) as f32 / TICK_RATE as f32;
                let moved: [f32; 3] = std::array::from_fn(|i| (seen_at[i] - bot.last_seen[i]) / since.max(1e-3));
                let speed = moved.iter().map(|v| v * v).sum::<f32>().sqrt();
                bot.velocity = if same && bot.sees && speed < TOP_SPEED { std::array::from_fn(|i| (bot.velocity[i] + moved[i]) / 2.0) } else { [0.0; 3] };
                if !same || !bot.sees {
                    // Caught sight of them: it takes a moment to react, and its aim has to settle.
                    bot.react = (lerp(REACT_POOR, REACT_GOOD, bot.skill) * TICK_RATE as f32) as u32;
                    bot.tracked = 0;
                }
                let settled = 1.0 + UNSETTLED * (-(bot.tracked as f32) / (SETTLE * TICK_RATE as f32)).exp();
                let off = lerp(AIM_POOR, AIM_GOOD, bot.skill) * settled;
                bot.error = [wobble(world) * off, wobble(world) * off];
                bot.mode = BotMode::Pursue;
                bot.target = Some(other);
                bot.last_seen = seen_at;
                bot.seen_tick = world.tick;
                bot.sees = true;
            }
            None => bot.sees = false,
        }
    }
    let Some(player) = world.players.get(&id) else { return idle };
    let fighting = bot.mode == BotMode::Pursue && bot.sees;
    if !fighting {
        bot.pitch = turn_towards(bot.pitch, 0.0, TURN_RATE * dt);
    }
    let distance = if fighting { distance_2d(at, bot.last_seen) } else { PATROL_DISTANCE };
    let gun = pick_gun(&player.hotbar, distance, bot.slot);
    if let Some(slot) = gun {
        bot.slot = slot;
    }
    let idle = InputFrame { slot: bot.slot, ..idle };

    // A crewman on foot goes back to its tank, unless it is fighting someone it can see.
    let boarding = match bot.crew {
        Some(crew) if !(bot.mode == BotMode::Pursue && bot.sees) => crew::board(world, id, crew, at),
        _ => crew::Boarding::No,
    };
    let (goal, run) = match bot.mode {
        _ if matches!(boarding, crew::Boarding::Aboard) => return idle,
        _ if let crew::Boarding::Walk(goal) = boarding => (goal, true),
        BotMode::Patrol => {
            let Some(&[x, z]) = bot.route.get(bot.next) else { return idle };
            let goal = [x, at[1], z];
            if distance_2d(at, goal) < REACHED {
                bot.next = (bot.next + 1) % bot.route.len();
                new_goal(bot);
                return idle;
            }
            (goal, false)
        }
        BotMode::Pursue => {
            let goal = bot.last_seen;
            let distance = distance_2d(at, goal);
            if off_route(bot, at) > LEASH {
                resume_patrol(bot, at);
                return idle;
            }
            if bot.sees && let Some(slot) = gun {
                return fight(world, id, bot, eye, slot, distance, dt, away);
            }
            if bot.sees && distance < CLOSE_IN {
                // Unarmed and up close: stand and face them.
                bot.yaw = turn_towards(bot.yaw, heading(at, goal), TURN_RATE * dt);
                return InputFrame { yaw: to_mrad(bot.yaw), ..idle };
            }
            if !bot.sees && distance < REACHED {
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
    };

    let toward = follow(world, bot, at, goal, budget);
    if stalled(bot, distance_2d(at, toward)) {
        // Stuck: on patrol it moves on to the next waypoint, and in a pursuit it looks round here.
        bot.path.clear();
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
        forward: if facing { 127 } else { 0 },
        buttons: if run { BUTTON_RUN } else { 0 },
        ..InputFrame { yaw: to_mrad(bot.yaw), ..idle }
    }
}

impl Mechanic for Bots {
    fn on_leave(&mut self, world: &mut World, id: u32) {
        world.bots.remove(&id);
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        for area in world.physics.take_changed() {
            world.nav.forget(area);
        }
        let noises = std::mem::take(&mut world.noises);
        let mut budget = SEARCH_PER_TICK;
        let ids: Vec<u32> = world.bots.keys().copied().collect();
        // Where every bot on its feet stands as the tick begins, for them to steer clear of each other.
        let crowd: Vec<(u32, [f32; 2])> = ids
            .iter()
            .filter_map(|id| world.players.get(id).filter(|p| p.alive() && p.seat.is_none()).map(|p| (*id, [p.character.position[0], p.character.position[2]])))
            .collect();
        for id in ids {
            let Some(mut bot) = world.bots.remove(&id) else { continue };
            let input = think(world, id, &mut bot, dt, &noises, &crowd, &mut budget);
            let input = reload(world, id, &bot, input);
            world.bots.insert(id, bot);
            if let Some(player) = world.players.get_mut(&id) {
                player.input = input;
            }
        }
    }
}

#[cfg(test)]
mod tests;
