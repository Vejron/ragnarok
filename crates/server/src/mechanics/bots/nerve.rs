//! Nerve: how shaken a bot is, from 1 (calm) to 0. Hits, lost limbs, blasts close by, rounds passing
//! close and a squadmate dying nearby all take some away, the more the less `courage` it has, and
//! time gives it back. Below its breaking point it stops advancing, keeps its head down and looks
//! for cover; all the way down, it shakes too much to shoot well or at all.

use super::*;
use crate::mechanics::limbs::Wounds;

/// What each thing takes from a bot of no courage.
const HIT_SHOCK: f32 = 0.25;
const LIMB_SHOCK: f32 = 0.4;
const BLAST_SHOCK: f32 = 0.15;
const NEAR_MISS_SHOCK: f32 = 0.12;
const SQUADMATE_SHOCK: f32 = 0.3;
/// How much of a shock a bot of full courage shrugs off.
const COURAGE_SHIELD: f32 = 0.7;
/// A blast this near shakes it, metres; a round passing this near; a death on its side this near.
const BLAST_NEAR: f32 = 15.0;
const MISS_NEAR: f32 = 2.5;
const SQUADMATE_NEAR: f32 = 20.0;
/// Nerve back per second, and on top of that in cover.
const RECOVER: f32 = 0.06;
const RECOVER_IN_COVER: f32 = 0.1;
/// Where it breaks at no courage: full courage never does.
const BREAK_POINT: f32 = 0.55;
/// It is steady again this far above its breaking point.
const STEADY_ABOVE: f32 = 0.3;
/// Under this it holds its fire, frozen.
const FROZEN: f32 = 0.08;
/// How long it keeps a pose before thinking again, seconds.
const POSE_TICKS: [f32; 2] = [1.5, 4.0];
/// Past this it may crouch to shoot, and with a sniper may lie down, metres.
const CROUCH_PAST: f32 = 10.0;
const PRONE_PAST: f32 = 25.0;

/// Takes `shock` from `bot`, softened by its courage.
pub(super) fn shake(bot: &mut Bot, shock: f32) {
    bot.nerve = (bot.nerve - shock * (1.0 - bot.temper.courage * COURAGE_SHIELD)).max(0.0);
}

/// What a hit that took `lost` health does to a bot's nerve.
pub(super) fn hit_shock(lost: u8) -> f32 {
    HIT_SHOCK + f32::from(lost) / 100.0
}

pub(super) fn near_squadmate(from: [f32; 3], at: [f32; 3]) -> bool {
    distance_2d(from, at) < SQUADMATE_NEAR
}

pub(super) const fn squadmate_shock() -> f32 {
    SQUADMATE_SHOCK
}

/// Whether a round that left `noise.at` along `noise.line` passes within `MISS_NEAR` of `at`.
fn passes_near(noise: &Noise, at: [f32; 3]) -> bool {
    let Some(line) = noise.line else { return false };
    let to: [f32; 3] = std::array::from_fn(|i| at[i] - noise.at[i]);
    let along = to[0] * line[0] + to[1] * line[1] + to[2] * line[2];
    along > 0.0 && vec3::distance(vec3::along(noise.at, line, along), at) < MISS_NEAR
}

/// One tick of how bot `id`, at `at`, feels: `noises` close to it and limbs newly gone shake it,
/// and otherwise it calms down.
pub(super) fn feel(world: &World, id: u32, bot: &mut Bot, at: [f32; 3], noises: &[Noise], dt: f32) {
    let limbs = world.entities.get::<Wounds>(id).map_or(0, |wounds| wounds.lost.count_ones());
    if limbs > bot.limbs {
        shake(bot, LIMB_SHOCK);
    }
    bot.limbs = limbs;
    for noise in noises {
        if noise.by == Some(id) || noise.by.is_some_and(|by| same_team(world, id, by)) {
            continue;
        }
        if noise.reach >= HEAR_BLAST && vec3::distance(noise.at, at) < BLAST_NEAR {
            shake(bot, BLAST_SHOCK);
        }
        if passes_near(noise, at) {
            shake(bot, NEAR_MISS_SHOCK);
        }
    }
    let recover = RECOVER + if bot.mode == BotMode::Cover { RECOVER_IN_COVER } else { 0.0 };
    bot.nerve = (bot.nerve + recover * dt).min(1.0);
}

/// The nerve under which it has broken.
fn breaking_point(bot: &Bot) -> f32 {
    BREAK_POINT * (1.0 - bot.temper.courage)
}

pub(super) fn broken(bot: &Bot) -> bool {
    bot.nerve < breaking_point(bot)
}

pub(super) fn steady(bot: &Bot) -> bool {
    bot.nerve >= breaking_point(bot) + STEADY_ABOVE
}

pub(super) fn frozen(bot: &Bot) -> bool {
    bot.nerve < FROZEN && bot.temper.courage < 1.0
}

/// The stance it fights in, as buttons: flat when it has broken, or when it is a patient sniper a
/// long way off; crouched more often the more patient it is, further out; otherwise on its feet.
/// It thinks again every few seconds, so its height changes between bursts.
pub(super) fn pose(world: &mut World, bot: &mut Bot, item: u8, distance: f32) -> u16 {
    bot.pose_left = bot.pose_left.saturating_sub(1);
    let pinned = broken(bot);
    if bot.pose_left == 0 || (pinned && bot.pose != BUTTON_PRONE) {
        let roll = world.rng.next_f64() as f32;
        bot.pose = if pinned || (item == ITEM_SNIPER && distance > PRONE_PAST && roll < 0.5 + bot.temper.patience / 2.0) {
            BUTTON_PRONE
        } else if distance > CROUCH_PAST && roll < 0.3 + bot.temper.patience / 2.0 {
            BUTTON_CROUCH
        } else {
            0
        };
        bot.pose_left = (lerp(POSE_TICKS[0], POSE_TICKS[1], world.rng.next_f64() as f32) * TICK_RATE as f32) as u32;
    }
    bot.pose
}
