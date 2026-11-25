//! Keeping out of the way of vehicles, and away from what a bot cannot fight (BOTS.md).
//!
//! - **Dodging.** A vehicle coming at a bot on foot fast enough to run it down, whose path passes
//!   near it, has it sprint off the vehicle's line for a moment, whatever else it was doing.
//! - **Evading.** A bot that sees a tank and has nothing that can hurt one, or one that has broken and
//!   found no cover, gets away: to a spot the threat cannot see if there is one close by, and else
//!   off in long runs until it is far enough. Then it keeps down and watches, and once it has not
//!   seen the threat for a while it goes back to looking for whoever it was after. Kept down out of
//!   a tank's way, it fires on it now and then with what it has, for all the good that does.

use super::*;
use crate::game::Threat;

/// A vehicle going faster than this, m/s, is one to get out of the way of.
const DODGE_SPEED: f32 = 6.0;
/// It is dodged if it is within this, metres, and its path will pass within `DODGE_NEAR` of the bot
/// in the next `DODGE_AHEAD` seconds.
const DODGE_WITHIN: f32 = 30.0;
const DODGE_NEAR: f32 = 4.0;
const DODGE_AHEAD: f32 = 3.0;
/// How long a dodge is run for, seconds.
const DODGE_FOR: f32 = 1.2;
/// How long a bot evades a tank for after it last saw it, seconds; and one that has broken, from
/// whatever broke it, at the least and at the most.
const EVADE_FOR: f32 = 10.0;
const FLEE_FOR: [f32; 2] = [6.0, 10.0];
/// It is far enough from a tank at this distance, metres.
const KEEP_AWAY: f32 = 50.0;
/// A run to get away goes about this far, metres, a little to one side of straight away (`slant`).
const SLANT: f32 = 0.5;
/// At its spot it is there within this, metres.
const THERE: f32 = 2.0;
/// Kept down out of a tank's way, it fires on it for this long, seconds at the least and the most,
/// and waits this long between.
const HARASS_FIRE: [f32; 2] = [1.5, 3.0];
const HARASS_WAIT: [f32; 2] = [3.0, 6.0];

/// A vehicle on the move: its id, where it is and how fast it goes which way, m/s.
pub(super) type Mover = (u32, [f32; 3], [f32; 3]);

/// The vehicles going fast enough to run someone down, as the tick begins.
pub(super) fn movers(world: &World) -> Vec<Mover> {
    world
        .entities
        .ids::<With<crate::game::Vehicle>>()
        .into_iter()
        .filter_map(|vid| {
            let pose = world.physics.car_pose(vid)?;
            let (sin, cos) = pose.yaw().sin_cos();
            (pose.forward_speed.abs() > DODGE_SPEED).then(|| (vid, pose.position, [-sin * pose.forward_speed, 0.0, -cos * pose.forward_speed]))
        })
        .collect()
}

/// Which way bot at `at` should run to get out of the way of one of `movers`, as a heading: off the
/// vehicle's line, on the side of it the bot is already on. `None` when nothing is coming at it.
pub(super) fn dodge(at: [f32; 3], movers: &[Mover]) -> Option<f32> {
    movers.iter().find_map(|&(_, p, v)| {
        let rel = [at[0] - p[0], at[2] - p[2]];
        if rel[0].hypot(rel[1]) > DODGE_WITHIN || (at[1] - p[1]).abs() > 3.0 {
            return None;
        }
        let speed2 = v[0] * v[0] + v[2] * v[2];
        // When it is nearest, and how near.
        let t = (rel[0] * v[0] + rel[1] * v[2]) / speed2;
        if !(0.0..DODGE_AHEAD).contains(&t) {
            return None;
        }
        let miss = [rel[0] - v[0] * t, rel[1] - v[2] * t];
        if miss[0].hypot(miss[1]) > DODGE_NEAR {
            return None;
        }
        // Square off its line, on the side the bot is on, or the left of it when right on it.
        let speed = speed2.sqrt();
        let across = [-v[2] / speed, v[0] / speed];
        let side = if miss[0] * across[0] + miss[1] * across[1] < 0.0 { -1.0 } else { 1.0 };
        Some((-(across[0] * side)).atan2(-(across[1] * side)))
    })
}

/// The input that runs bot `bot` towards the heading `want` at once, turning to face it as it goes:
/// by the forward and sideways axes, so it need not turn round first, and less sideways the more it
/// has turned.
pub(super) fn run_along(bot: &mut Bot, want: f32, dt: f32, idle: InputFrame) -> InputFrame {
    bot.yaw = turn_towards(bot.yaw, want, TURN_RATE * dt);
    let (sin, cos) = bot.yaw.sin_cos();
    let d = [-want.sin(), -want.cos()];
    let forward = d[0] * -sin + d[1] * -cos;
    let strafe = d[0] * cos - d[1] * sin;
    InputFrame { forward: (forward * 127.0).round() as i8, strafe: (strafe * 127.0).round() as i8, buttons: BUTTON_RUN, yaw: to_mrad(bot.yaw), ..idle }
}

/// A dodge starts: for `DODGE_FOR`, along `yaw`.
pub(super) fn start_dodge(bot: &mut Bot, yaw: f32) {
    bot.dodge = (DODGE_FOR * TICK_RATE as f32) as u32;
    bot.dodge_yaw = yaw;
}

/// It gets away from where it last saw its threat: from a tank it cannot fight, for as long as it
/// keeps seeing one; or, broken, for a few seconds.
pub(super) fn start(world: &mut World, bot: &mut Bot, broken: bool) {
    let left = if broken { lerp(FLEE_FOR[0], FLEE_FOR[1], world.rng.next_f64() as f32) } else { EVADE_FOR };
    if bot.mode != BotMode::Evade {
        // It waits a spell before it fires on what it gets away from.
        (bot.bound, bot.bounding) = (ticks_between(world, HARASS_WAIT), false);
        bot.cover = None;
        bot.flank = None;
        bot.walk.path.clear();
        new_goal(bot);
        tracing::debug!(target_id = ?bot.sight.target, broken, "bot gets away");
    }
    bot.mode = BotMode::Evade;
    bot.mode_left = (left * TICK_RATE as f32) as u32;
}

/// One tick of getting away: where to run to, or the input for keeping down where it is.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan(world: &mut World, id: u32, bot: &mut Bot, at: [f32; 3], dt: f32, rays: &mut usize, budget: &mut usize, idle: InputFrame) -> Result<([f32; 3], bool), InputFrame> {
    let threat = bot.sight.last_seen;
    let armour = bot.sight.threat == Threat::Armour;
    if armour && bot.sight.sees {
        bot.mode_left = bot.mode_left.max((EVADE_FOR * TICK_RATE as f32) as u32);
    }
    bot.mode_left = bot.mode_left.saturating_sub(1);
    if bot.mode_left == 0 {
        // Long enough: it looks for whoever it was after where it last knew of them.
        bot.cover = None;
        bot.mode = BotMode::Search;
        bot.mode_left = SEARCH_TICKS;
        return Err(idle);
    }
    let far = distance_2d(at, threat);
    if bot.cover.is_none() {
        bot.cover = cover::find(world, bot, at, threat, rays, budget);
        if bot.cover.is_none() && (far < KEEP_AWAY || bot.sight.sees) {
            let slant = if id.is_multiple_of(2) { SLANT } else { -SLANT };
            bot.cover = crate::mechanics::civilians::away(world, at, threat, slant);
        }
        bot.walk.path.clear();
        new_goal(bot);
    }
    let Some(spot) = bot.cover else {
        // Nowhere to go and far enough: down where it is, watching.
        bot.yaw = turn_towards(bot.yaw, heading(at, threat), TURN_RATE * dt);
        return Err(InputFrame { yaw: to_mrad(bot.yaw), buttons: if armour { BUTTON_PRONE } else { BUTTON_CROUCH }, ..idle });
    };
    if distance_2d(at, spot) < THERE {
        // There: down, watching, unless it is in plain sight of what it runs from close by, when it
        // runs on.
        let eye = [threat[0], threat[1] + EYE_HEIGHT, threat[2]];
        let seen = (world.tick + id).is_multiple_of(LOOK_EVERY) && far < KEEP_AWAY && clear_line_past(world, eye, [at[0], at[1] + EYE_HEIGHT - CROUCH_EYE_DROP, at[2]], bot.sight.vehicle);
        if seen {
            bot.cover = None;
            return Err(idle);
        }
        bot.yaw = turn_towards(bot.yaw, heading(at, threat), TURN_RATE * dt);
        return Err(InputFrame { yaw: to_mrad(bot.yaw), buttons: BUTTON_CROUCH, ..idle });
    }
    Ok((spot, true))
}

/// Whether bot `bot`, at `at`, kept down out of the way of a tank it can see and is not too near,
/// fires on it this tick: in spells now and then, with whatever else it has.
pub(super) fn harass(world: &mut World, bot: &mut Bot, at: [f32; 3]) -> bool {
    let holding = bot.cover.is_none_or(|spot| distance_2d(at, spot) < THERE);
    if bot.mode != BotMode::Evade || bot.sight.threat != Threat::Armour || !bot.sight.sees || !holding || distance_2d(at, bot.sight.last_seen) < TOO_CLOSE {
        return false;
    }
    bot.bound = bot.bound.saturating_sub(1);
    if bot.bound == 0 {
        bot.bounding = !bot.bounding;
        bot.bound = ticks_between(world, if bot.bounding { HARASS_FIRE } else { HARASS_WAIT });
    }
    bot.bounding
}
