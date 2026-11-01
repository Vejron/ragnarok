//! Cover: a shaken bot, or one with an empty gun, breaks line of sight. It tries points around it,
//! furthest from the threat first, for one the threat cannot see, runs there, crouches, reloads and
//! calms down, then comes back out from a different side.
//!
//! A point counts as hidden if it would hide the bot crouching, as it does there, so a low wall will
//! do. It has to be a short run away too: the far side of a long wall is hidden, but getting there
//! takes it along the wall in full view.

use super::*;

/// The most rays cover looks use in one tick, among all the bots, and what one look costs.
pub(super) const RAYS_PER_TICK: usize = 48;
const POINTS: usize = 8;
const RAYS_A_LOOK: usize = POINTS * 2;
/// How far off it looks for cover, metres.
const NEAR: f32 = 3.0;
const FAR: f32 = 12.0;
/// The longest run to cover it will make, metres.
const LONGEST_RUN: f32 = 18.0;
/// How far nearer the threat than it is now a point may lie, metres.
const TOWARDS: f32 = 2.0;
/// After a look finds nothing, ticks before the next.
const RETRY_TICKS: u32 = TICK_RATE;
/// After it has come out of cover, ticks before it may go back.
const OUT_TICKS: u32 = 3 * TICK_RATE;
/// How long it stays at most, seconds, from no patience to full.
const STAY: [f32; 2] = [3.0, 10.0];
/// How far to the side of where it last saw them it comes out, metres.
const SIDE: [f32; 2] = [3.0, 6.0];

/// How long the way from `at` to `spot` is, metres, if there is one and this tick's `budget` allows.
fn run_to(world: &mut World, at: [f32; 3], spot: [f32; 3], budget: &mut usize) -> Option<f32> {
    let feet = |p: [f32; 3]| [p[0], p[1] - sim::STANDING_Y, p[2]];
    let path = world.nav.find_path(&world.physics, &world.map, feet(at), feet(spot), budget)?;
    let mut from = [at[0], at[2]];
    let mut length = 0.0;
    for corner in path {
        length += (corner[0] - from[0]).hypot(corner[1] - from[1]);
        from = corner;
    }
    Some(length)
}

/// Looks for a point near `at` hidden from `threat`, a short run away, and heads there. Stays as it
/// is if this tick's `rays` have run out, or there is none.
pub(super) fn seek(world: &mut World, bot: &mut Bot, at: [f32; 3], rays: &mut usize, budget: &mut usize) {
    bot.cover_wait = bot.cover_wait.saturating_sub(1);
    if bot.cover_wait > 0 || *rays < RAYS_A_LOOK {
        return;
    }
    *rays -= RAYS_A_LOOK;
    bot.cover_wait = RETRY_TICKS;
    let threat = bot.sight.last_seen;
    let eye = [threat[0], threat[1] + EYE_HEIGHT, threat[2]];
    let away = heading(threat, at);
    let far = distance_2d(at, threat);
    for n in 0..POINTS {
        // Straight away from the threat first, then wider and wider to either side.
        let turn = n.div_ceil(2) as f32 * if n % 2 == 0 { 1.0 } else { -1.0 } * TAU / POINTS as f32;
        let reach = lerp(NEAR, FAR, world.rng.next_f64() as f32);
        let (sin, cos) = (away + turn).sin_cos();
        let (x, z) = (at[0] - sin * reach, at[2] - cos * reach);
        let Some(floor) = open_ground(world, x, z) else { continue };
        let spot = [x, floor + sim::STANDING_Y, z];
        if distance_2d(spot, threat) + TOWARDS >= far && !clear_line(world, eye, [x, spot[1] + EYE_HEIGHT - CROUCH_EYE_DROP, z]) && run_to(world, at, spot, budget).is_some_and(|run| run < LONGEST_RUN) {
            tracing::debug!(target_id = ?bot.sight.target, ?spot, "bot takes cover");
            bot.mode = BotMode::Cover;
            bot.mode_left = (lerp(STAY[0], STAY[1], bot.temper.patience) * TICK_RATE as f32) as u32;
            bot.cover = Some(spot);
            bot.walk.path.clear();
            new_goal(bot);
            return;
        }
    }
}

/// Out of cover, to where it last saw them, or to the side of that so it does not come back the
/// same way it left.
pub(super) fn leave(world: &mut World, bot: &mut Bot, at: [f32; 3]) {
    bot.mode = BotMode::Pursue;
    bot.cover = None;
    bot.cover_wait = OUT_TICKS;
    bot.walk.path.clear();
    new_goal(bot);
    if bot.sight.sees {
        return;
    }
    let along = heading(at, bot.sight.last_seen) + if world.rng.next_f64() < 0.5 { PI / 2.0 } else { -PI / 2.0 };
    let side = lerp(SIDE[0], SIDE[1], world.rng.next_f64() as f32);
    bot.sight.last_seen[0] -= along.sin() * side;
    bot.sight.last_seen[2] -= along.cos() * side;
}
