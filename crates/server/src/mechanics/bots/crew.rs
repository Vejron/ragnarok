//! Tank crews: two bots in one tank. The driver (seat 0) drives it round its route, which is a
//! stretch of the nearest road unless it was given one. The gunner (seat 1) looks out all round
//! from the turret, and when it sees someone it lays the gun on them, allowing for the shell's fall
//! and their speed, and fires. What the gunner is doing decides where the driver goes: it halts
//! while the gunner fights, drives to where the gunner last saw or heard someone, and goes back to
//! its route once the gunner gives up. A driver that has stopped getting nearer to where it is
//! going backs out and tries again, and after a few tries it gives that waypoint up.
//!
//! Both write their input as a player in that seat would, so `Vehicles` and `Gunnery` drive the
//! real tank. A crewman on foot (respawned, or put out) walks back to its tank and gets in.

use super::*;
use crate::game::{Crew, Mounts, Player, Seats, Vehicle};
use crate::mechanics::combat::position_of;
use crate::mechanics::gunnery::aim;
use crate::mechanics::vehicles::seat;
use sim::CarPose;
use shared::protocol::{DAMAGE_TURRET, TURRET_GONE};
use shared::quant::wrap_angle;
use shared::world::WorldFile;
use std::collections::HashMap;

/// The seat that drives, and the one that works the gun.
pub const DRIVER: usize = 0;
pub const GUNNER: usize = 1;
/// How far the gunner sees, metres. It sees all round, and people in vehicles too.
const TANK_SIGHT: f32 = 120.0;
/// Where the gunner looks from, in the hull's frame: just above the turret.
const TURRET_EYE: [f32; 3] = [0.0, TANK_TURRET_PIVOT[1] + 0.6, TANK_TURRET_PIVOT[2]];
/// How long it takes to fire on someone just seen, seconds, at skill 0 and at skill 1.
const TANK_REACT_POOR: f32 = 1.2;
const TANK_REACT_GOOD: f32 = 0.4;
/// How far off its laying is, radians, at skill 0 and at skill 1, once settled. A shell bursts
/// `EXPLOSION_RADIUS` wide, so this is looser than a rifle's.
const TANK_AIM_POOR: f32 = 0.04;
const TANK_AIM_GOOD: f32 = 0.004;
/// It fires once the gun lies within this of where it wants it, radians.
const LAID_WITHIN: f32 = 0.02;
/// It does not fire on anyone nearer than this, metres: the burst would reach its own tank.
const MIN_RANGE: f32 = 12.0;
/// Fastest it believes anything moves, m/s: cars are quick.
const TANK_TOP_SPEED: f32 = 40.0;
/// How long it keeps after someone it has lost before looking round, seconds.
const CHASE: f32 = 20.0;
/// How fast it sweeps the turret round while it looks, radians a second.
const SWEEP_RATE: f32 = 0.6;
/// The driver stops this near where the gunner last saw someone, metres.
const CHASE_STOP: f32 = 15.0;
/// Close enough to a waypoint to head for the next one, metres.
const WAYPOINT_REACHED: f32 = 5.0;
/// Steering per radian off the heading it wants, and the error past which it turns on the spot.
const STEER_GAIN: f32 = 2.0;
const PIVOT: f32 = 0.7;
/// Coming up to a waypoint where the route turns by more than `SHARP` radians, it slows over the
/// last `SLOW_OVER` metres, so it does not run wide: to `CORNER_SPEED` m/s for a right angle, less
/// for a sharper turn, and to `HAIRPIN_SPEED` to turn right round on the spot.
const SHARP: f32 = 0.4;
const SLOW_OVER: f32 = 25.0;
const CORNER_SPEED: f32 = 3.0;
const HAIRPIN_SPEED: f32 = 0.5;
/// A goal it has not got `DRIVE_PROGRESS` metres nearer to in `DRIVE_STALL` seconds is a stuck
/// tank: it backs out for `REVERSE`, and after `TRIES` of those gives the waypoint up. Turning round
/// on the spot takes about three seconds, so this is longer than on foot; and a tank shoving a car
/// along at a crawl is stuck too.
const DRIVE_PROGRESS: f32 = 3.0;
const DRIVE_STALL: f32 = 5.0;
const REVERSE: f32 = 2.0;
const TRIES: u32 = 3;
/// A crewman on foot within this of its tank goes back to it, metres.
const BOARD_RANGE: f32 = 60.0;
/// A route made along a road runs this far each way from the nearest point of it, metres, with
/// waypoints at least `ROAD_SPACING` apart.
const ROAD_REACH: f32 = 300.0;
const ROAD_SPACING: f32 = 8.0;

/// A route along the road nearest `at`: from `ROAD_REACH` metres one way along it to as far the
/// other, and back, as a loop. Roads that meet end to end, or share a point, are one network, and
/// at a junction it goes on along whichever road runs straightest on. Just `at` when the world has
/// no roads.
pub fn road_route(map: &WorldFile, at: [f32; 2]) -> Vec<[f32; 2]> {
    // The network: every point of every road, those within a metre of each other being one, and
    // the links between points that follow each other along a road.
    let mut index: HashMap<[i32; 2], usize> = HashMap::new();
    let mut points: Vec<[f32; 2]> = Vec::new();
    let mut links: Vec<Vec<usize>> = Vec::new();
    let mut node = |p: [i32; 2]| {
        let key = [p[0].div_euclid(100), p[1].div_euclid(100)];
        *index.entry(key).or_insert_with(|| {
            points.push([p[0] as f32 / 100.0, p[1] as f32 / 100.0]);
            links.push(Vec::new());
            points.len() - 1
        })
    };
    let mut pairs = Vec::new();
    for road in &map.roads {
        let nodes: Vec<usize> = road.points.iter().map(|&p| node(p)).collect();
        pairs.extend(nodes.windows(2).filter(|w| w[0] != w[1]).map(|w| (w[0], w[1])));
    }
    for (a, b) in pairs {
        if !links[a].contains(&b) {
            links[a].push(b);
            links[b].push(a);
        }
    }
    let far = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
    let Some(start) = (0..points.len()).min_by(|&a, &b| far(points[a], at).total_cmp(&far(points[b], at))) else { return vec![at] };
    let Some(&first) = links[start].first() else { return vec![points[start]] };
    let unit = |a: [f32; 2], b: [f32; 2]| {
        let d = far(a, b).max(1e-3);
        [(b[0] - a[0]) / d, (b[1] - a[1]) / d]
    };
    // The other way out of the start: the link most nearly opposite the first.
    let ahead = unit(points[start], points[first]);
    let back = links[start].iter().copied().filter(|&n| n != first).min_by(|&a, &b| {
        let dot = |n: usize| {
            let u = unit(points[start], points[n]);
            u[0] * ahead[0] + u[1] * ahead[1]
        };
        dot(a).total_cmp(&dot(b))
    });
    // Out along the network from the start through `via`, straightest on at every junction, never
    // coming back to a point it has passed, until `ROAD_REACH` or a dead end.
    let walk = |via: usize, taken: &[usize]| {
        let mut seen: Vec<usize> = taken.to_vec();
        let (mut previous, mut current, mut gone) = (start, via, far(points[start], points[via]));
        let mut kept = vec![current];
        seen.push(current);
        while gone < ROAD_REACH {
            let heading = unit(points[previous], points[current]);
            let next = links[current].iter().copied().filter(|n| !seen.contains(n)).max_by(|&a, &b| {
                let dot = |n: usize| {
                    let u = unit(points[current], points[n]);
                    u[0] * heading[0] + u[1] * heading[1]
                };
                dot(a).total_cmp(&dot(b))
            });
            let Some(next) = next else { break };
            gone += far(points[current], points[next]);
            (previous, current) = (current, next);
            seen.push(current);
            kept.push(current);
        }
        kept
    };
    let out = walk(first, &[start]);
    let home = match back {
        Some(back) => walk(back, &[&[start][..], &out].concat()),
        None => Vec::new(),
    };
    // One end to the other, a point every `ROAD_SPACING` at least, then back without doing either
    // end twice.
    let mut there: Vec<[f32; 2]> = Vec::new();
    for n in home.into_iter().rev().chain(std::iter::once(start)).chain(out) {
        if there.last().is_none_or(|&last| far(last, points[n]) >= ROAD_SPACING) {
            there.push(points[n]);
        }
    }
    let back_again: Vec<[f32; 2]> = there.iter().rev().skip(1).take(there.len().saturating_sub(2)).copied().collect();
    there.extend(back_again);
    there
}

/// The index of the waypoint of `route` nearest `at`.
pub fn nearest_waypoint(route: &[[f32; 2]], at: [f32; 2]) -> usize {
    (0..route.len()).min_by(|&a, &b| (route[a][0] - at[0]).hypot(route[a][1] - at[1]).total_cmp(&(route[b][0] - at[0]).hypot(route[b][1] - at[1]))).unwrap_or(0)
}

/// What a crewman on foot does about its tank.
pub(super) enum Boarding {
    /// It got in, this tick.
    Aboard,
    /// It is on its way to it, which is here.
    Walk([f32; 3]),
    /// It has no tank to go back to: wrecked, gone, too far, or someone else in its seat.
    No,
}

/// Whether crewman `id`, on foot at `at`, gets back into its tank, walks to it, or does without.
pub(super) fn board(world: &mut World, id: u32, crew: Crew, at: [f32; 3]) -> Boarding {
    let Some(seats) = world.entities.get::<Seats>(crew.vehicle) else { return Boarding::No };
    if world.entities.dead(crew.vehicle) || seats.rider(crew.seat).is_some() {
        return Boarding::No;
    }
    let Some(pose) = world.physics.car_pose(crew.vehicle) else { return Boarding::No };
    let distance = (0..3).map(|i| (pose.position[i] - at[i]).powi(2)).sum::<f32>().sqrt();
    if distance <= ENTER_RADIUS - 0.5 && seat(world, id, crew.vehicle, crew.seat) {
        return Boarding::Aboard;
    }
    if distance > BOARD_RANGE {
        return Boarding::No;
    }
    Boarding::Walk(pose.position)
}

/// One tick of crewman `id`'s work in seat `index` of vehicle `vid`: driving or gunnery.
pub(super) fn work(world: &mut World, id: u32, bot: &mut Bot, vid: u32, index: usize, noises: &[Noise], dt: f32) -> InputFrame {
    let idle = InputFrame { view: world.tick as u16, slot: bot.slot, ..Default::default() };
    let Some(pose) = world.physics.car_pose(vid) else { return idle };
    match index {
        DRIVER => drive(world, bot, vid, &pose, idle),
        GUNNER => gun(world, id, bot, vid, &pose, noises, dt, idle),
        _ => idle,
    }
}

/// Throttle and steering as a driver's axes, with the handbrake on when `halt`.
fn controls(throttle: f32, steer: f32, halt: bool, idle: InputFrame) -> InputFrame {
    let axis = |v: f32| (v.clamp(-1.0, 1.0) * 127.0).round() as i8;
    InputFrame { forward: axis(throttle), strafe: axis(steer), buttons: if halt { BUTTON_JUMP } else { 0 }, ..idle }
}

/// The driver's tick: where the gunner's state sends it, then steering there.
fn drive(world: &World, bot: &mut Bot, vid: u32, pose: &CarPose, idle: InputFrame) -> InputFrame {
    let at = pose.position;
    let halt = controls(0.0, 0.0, true, idle);
    let gunner = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(GUNNER)).and_then(|id| world.entities.get::<Bot>(id));
    // How fast it may go: full speed, unless it is coming up to a sharp turn.
    let mut limit = TANK_MAX_SPEED;
    let goal = match gunner.map(|g| (g.mode, g.sees, g.last_seen)) {
        // It holds still while the gunner fights, and while it looks round.
        Some((BotMode::Pursue, true, _)) | Some((BotMode::Search, ..)) => {
            bot.mode = BotMode::Pursue;
            return halt;
        }
        Some((BotMode::Pursue, false, seen)) => {
            if bot.mode != BotMode::Pursue || distance_2d(seen, bot.last_seen) > GOAL_MOVED {
                bot.tries = 0;
                new_goal(bot);
            }
            bot.mode = BotMode::Pursue;
            bot.last_seen = seen;
            if distance_2d(at, seen) < CHASE_STOP || bot.tries >= TRIES {
                return halt;
            }
            seen
        }
        _ => {
            if bot.mode != BotMode::Patrol {
                // Back to the route, at the waypoint nearest to where the chase left it.
                bot.mode = BotMode::Patrol;
                bot.next = nearest_waypoint(&bot.route, [at[0], at[2]]);
                bot.tries = 0;
                new_goal(bot);
            }
            let Some(&[x, z]) = bot.route.get(bot.next) else { return halt };
            if distance_2d(at, [x, at[1], z]) < WAYPOINT_REACHED {
                next_waypoint(bot);
                if bot.route.len() <= 1 {
                    return halt;
                }
            }
            let [x, z] = bot.route[bot.next];
            let goal = [x, at[1], z];
            let [ax, az] = bot.route[(bot.next + 1) % bot.route.len()];
            let turn = wrap_angle(heading(goal, [ax, at[1], az]) - heading(at, goal)).abs();
            if turn > SHARP {
                let corner = (2.0 * CORNER_SPEED * (1.0 - turn / PI)).max(HAIRPIN_SPEED);
                limit = lerp(corner, TANK_MAX_SPEED, (distance_2d(at, goal) - WAYPOINT_REACHED).max(0.0) / SLOW_OVER).min(TANK_MAX_SPEED);
            }
            goal
        }
    };

    let off = wrap_angle(heading(at, goal) - pose.yaw());
    // A positive steer turns the hull clockwise seen from above, which takes its yaw down.
    let steer = -off * STEER_GAIN;
    if bot.reverse_left > 0 {
        bot.reverse_left -= 1;
        if bot.reverse_left == 0 {
            new_goal(bot);
        }
        return controls(-1.0, steer, false, idle);
    }
    if stalled_by(bot, distance_2d(at, goal), DRIVE_PROGRESS, (DRIVE_STALL * TICK_RATE as f32) as u32) {
        bot.tries += 1;
        tracing::debug!(vid, tries = bot.tries, "tank stuck; backing out");
        if bot.tries >= TRIES && bot.mode == BotMode::Patrol {
            next_waypoint(bot);
        } else {
            bot.reverse_left = (REVERSE * TICK_RATE as f32) as u32;
        }
        new_goal(bot);
        return halt;
    }
    // Full ahead when facing the way, easing off as the heading is further out, and turning on the
    // spot when it is well off.
    let throttle = if off.abs() > PIVOT { 0.0 } else { 1.0 - 0.6 * off.abs() / PIVOT };
    // Too fast for the turn ahead: off the throttle, and on the brake when well over.
    let over = pose.forward_speed - limit;
    if over > 0.0 {
        return controls(0.0, steer, over > 2.0, idle);
    }
    controls(throttle, steer, false, idle)
}

fn next_waypoint(bot: &mut Bot) {
    bot.next = (bot.next + 1) % bot.route.len().max(1);
    bot.tries = 0;
    new_goal(bot);
}

/// The nearest living enemy the gunner can see from `eye`, all round and out to `TANK_SIGHT`: on
/// foot, or in a vehicle (but not its own). With where to lay the gun on them.
fn spot_from_turret(world: &World, id: u32, vid: u32, eye: [f32; 3]) -> Option<(u32, [f32; 3])> {
    let mut best: Option<(u32, [f32; 3], f32)> = None;
    for (other, player) in world.entities.all::<Player>() {
        if other == id || !world.entities.alive(other) || same_team(world, id, other) {
            continue;
        }
        let riding = player.seat.filter(|seat| !seat.remote).map(|seat| seat.vehicle);
        if riding == Some(vid) {
            continue;
        }
        let Some(at) = position_of(world, other) else { continue };
        let mark = if riding.is_some() { at } else { [at[0], at[1] + CHEST, at[2]] };
        let distance = (0..3).map(|i| (mark[i] - eye[i]).powi(2)).sum::<f32>().sqrt();
        if distance > TANK_SIGHT || best.is_some_and(|(.., nearest)| distance >= nearest) {
            continue;
        }
        if clear_shot(world, eye, mark, vid, riding) {
            best = Some((other, mark, distance));
        }
    }
    best.map(|(other, at, _)| (other, at))
}

/// Whether nothing stands between `from` and `to` but `own` (the tank looking) and `target` (the
/// vehicle at `to`, if it is one).
fn clear_shot(world: &World, from: [f32; 3], to: [f32; 3], own: u32, target: Option<u32>) -> bool {
    let d: [f32; 3] = std::array::from_fn(|i| to[i] - from[i]);
    let length = d.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length < 0.01 {
        return true;
    }
    match world.physics.cast_ray_hit(from, d.map(|v| v / length), length, own) {
        None => true,
        Some((t, hit)) => t >= length - 0.2 || (hit.is_some() && hit == target),
    }
}

/// The world elevation, radians, that lands a shell from `from` on `to`: the low one of the two.
/// Out of reach, the 45° that goes furthest.
pub fn lay(from: [f32; 3], to: [f32; 3]) -> f32 {
    let (d, h) = (distance_2d(from, to), to[1] - from[1]);
    let (v2, g) = (SHELL_SPEED * SHELL_SPEED, GRAVITY);
    let disc = v2 * v2 - g * (g * d * d + 2.0 * h * v2);
    if d < 0.01 {
        return h.signum() * PI / 2.0;
    }
    if disc < 0.0 {
        return PI / 4.0;
    }
    ((v2 - disc.sqrt()) / (g * d)).atan()
}

/// The gunner's tick: hearing, looking, then laying the gun and firing, or turning it to where it
/// is going to look, or sweeping it round.
#[allow(clippy::too_many_arguments)]
fn gun(world: &mut World, id: u32, bot: &mut Bot, vid: u32, pose: &CarPose, noises: &[Noise], dt: f32, idle: InputFrame) -> InputFrame {
    let at = pose.position;
    let eye = pose.world_point(TURRET_EYE);
    let chase = (CHASE * TICK_RATE as f32) as u32;
    let Some(vehicle) = world.entities.get::<Vehicle>(vid) else { return idle };
    let turret = world.entities.get::<Mounts>(vid).and_then(Mounts::turret).copied().unwrap_or_default();
    let turret_on = vehicle.damage[DAMAGE_TURRET] != TURRET_GONE;

    // A hit on the tank, or a fight heard, sends it after where that came from.
    let heard = bot.hit_from.take().or_else(|| hear(id, bot, at, noises).filter(|&from| bot.mode != BotMode::Pursue || distance_2d(at, from) + NEARER_NOISE < distance_2d(at, bot.last_seen)));
    if let Some(from) = heard
        && !(bot.mode == BotMode::Pursue && bot.sees)
    {
        bot.mode = BotMode::Pursue;
        bot.last_seen = from;
        bot.sees = false;
        bot.mode_left = chase;
    }

    if (world.tick + id).is_multiple_of(LOOK_EVERY) {
        match spot_from_turret(world, id, vid, eye) {
            Some((other, seen_at)) => {
                let same = bot.mode == BotMode::Pursue && bot.target == Some(other);
                let since = world.tick.wrapping_sub(bot.seen_tick) as f32 / TICK_RATE as f32;
                let moved: [f32; 3] = std::array::from_fn(|i| (seen_at[i] - bot.last_seen[i]) / since.max(1e-3));
                let speed = moved.iter().map(|v| v * v).sum::<f32>().sqrt();
                bot.velocity = if same && bot.sees && speed < TANK_TOP_SPEED { std::array::from_fn(|i| (bot.velocity[i] + moved[i]) / 2.0) } else { [0.0; 3] };
                if !same || !bot.sees {
                    tracing::debug!(bot = id, target = other, "tank gunner spotted someone");
                    bot.react = (lerp(TANK_REACT_POOR, TANK_REACT_GOOD, bot.skill) * TICK_RATE as f32) as u32;
                    bot.tracked = 0;
                }
                let settled = 1.0 + UNSETTLED * (-(bot.tracked as f32) / (SETTLE * TICK_RATE as f32)).exp();
                let off = lerp(TANK_AIM_POOR, TANK_AIM_GOOD, bot.skill) * settled;
                bot.error = [wobble(world) * off, wobble(world) * off];
                bot.mode = BotMode::Pursue;
                bot.target = Some(other);
                bot.last_seen = seen_at;
                bot.seen_tick = world.tick;
                bot.sees = true;
                bot.mode_left = chase;
            }
            None => bot.sees = false,
        }
    }

    // Where it wants the gun, as a world heading and elevation.
    let (yaw, pitch, fire) = match bot.mode {
        BotMode::Pursue if bot.sees => {
            bot.tracked += 1;
            bot.react = bot.react.saturating_sub(1);
            let since = world.tick.wrapping_sub(bot.seen_tick) as f32 / TICK_RATE as f32;
            let now: [f32; 3] = std::array::from_fn(|i| bot.last_seen[i] + bot.velocity[i] * since);
            let flight = (0..3).map(|i| (now[i] - eye[i]).powi(2)).sum::<f32>().sqrt() / SHELL_SPEED;
            let lead = lerp(LEAD_POOR, 1.0, bot.skill);
            let mark: [f32; 3] = std::array::from_fn(|i| now[i] + bot.velocity[i] * flight * lead);
            let far_enough = distance_2d(at, mark) >= MIN_RANGE;
            (heading(eye, mark) + bot.error[0], lay(eye, mark) + bot.error[1], bot.react == 0 && far_enough)
        }
        BotMode::Pursue => {
            bot.mode_left = bot.mode_left.saturating_sub(1);
            if off_route(bot, at) > LEASH {
                resume_patrol(bot, at);
            } else if bot.mode_left == 0 || distance_2d(at, bot.last_seen) < CHASE_STOP {
                bot.mode = BotMode::Search;
                bot.mode_left = SEARCH_TICKS;
            }
            bot.yaw = heading(eye, bot.last_seen);
            (bot.yaw, 0.0, false)
        }
        BotMode::Search => {
            bot.mode_left = bot.mode_left.saturating_sub(1);
            if bot.mode_left == 0 {
                resume_patrol(bot, at);
            }
            bot.yaw = wrap_angle(bot.yaw + SWEEP_RATE * dt);
            (bot.yaw, 0.0, false)
        }
        BotMode::Patrol => {
            bot.yaw = pose.yaw();
            (bot.yaw, 0.0, false)
        }
    };

    // The gun is laid in the hull's frame (see `Gunnery`): turn the world direction into it.
    let (sin, cos) = yaw.sin_cos();
    let local = pose.local_dir([-sin * pitch.cos(), pitch.sin(), -cos * pitch.cos()]);
    let local_yaw = (-local[0]).atan2(-local[2]);
    let local_pitch = local[1].clamp(-1.0, 1.0).asin();
    let laid = wrap_angle(turret.yaw - local_yaw).abs() < LAID_WITHIN && (turret.pitch - local_pitch.clamp(TANK_BARREL_MIN, TANK_BARREL_MAX)).abs() < LAID_WITHIN;
    let in_reach = (TANK_BARREL_MIN..=TANK_BARREL_MAX).contains(&local_pitch);
    let mut buttons = 0;
    if fire && laid && in_reach && turret_on && turret.cooldown == 0 {
        let (muzzle, _) = aim(pose, turret.yaw, turret.pitch);
        let target = bot.target.and_then(|t| world.entities.get::<Player>(t)).and_then(|p| p.seat).filter(|s| !s.remote).map(|s| s.vehicle);
        if clear_shot(world, muzzle, bot.last_seen, vid, target) {
            buttons |= BUTTON_FIRE;
        }
    }
    InputFrame { yaw: to_mrad(wrap_angle(local_yaw + pose.yaw())), pitch: to_mrad(local_pitch), buttons, ..idle }
}

#[cfg(test)]
mod tests;
