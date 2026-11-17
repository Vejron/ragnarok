//! Awareness: a bot does not see someone the moment a ray reaches them. It takes them in over time,
//! quickly if they are near, on their feet, moving or carrying a lit lamp, slowly if they are far,
//! low, still or in the dark. Half way there it stops and stares, which is the sign you have to get
//! out of its way. Noises give only a rough idea of where they came from. Standing crops hide whoever
//! is low enough in them (`crop_cover`).

use super::*;
use shared::world::{CROP_NONE, THEME_WINTER};

/// Seconds to take in someone in plain view: near, on their feet, walking, in daylight.
const FILL: f32 = 0.2;
/// How fast it fades once they are gone, a second.
const FADE: f32 = 0.25;
/// Already on edge it takes people in this many times faster.
const ALERT_RATE: f32 = 3.0;
/// From here it stops and stares, on patrol.
pub(super) const SUSPICIOUS: f32 = 0.4;
/// They count in full up to this far, metres, and for `FAR` at `SIGHT_RANGE`.
const NEAR: f32 = 15.0;
const FAR: f32 = 0.15;
/// How much of them there is to see, crouched and lying down.
const CROUCHED: f32 = 0.6;
const LYING: f32 = 0.3;
/// How much their moving counts: standing still, and running.
const STILL: f32 = 0.7;
const RUNNING: f32 = 1.3;
/// How much of them shows at dead of night, and with a lamp lit, day or night.
const DARK: f32 = 0.35;
const LAMP: f32 = 1.2;
/// How much of someone shows in a field of crops: on their feet, head and shoulders over it; crouched
/// and on the move, the crop parting round them. Crouched and still, or lying down, nothing does.
const IN_CROP_STANDING: f32 = 0.5;
const IN_CROP_MOVING: f32 = 0.15;
/// The crop must stand this far from someone toward the looker to hide them, metres: at the field's
/// very edge they are in plain view from outside it.
const CROP_DEPTH: f32 = 2.0;
/// Feet further than this over the ground are not in the crop, metres: a jump, a canopy.
const CROP_CLEARANCE: f32 = 0.5;
/// A share of a day either side of sunrise and sunset, over which it goes dark.
const DUSK: f64 = 0.05;
/// A noise further off than this it hears only roughly, metres, off by this share of the distance.
const EXACT: f32 = 15.0;
const ROUGH: f32 = 0.12;

/// How dark it is at `tick`, 0 by day and 1 at night. The same clock the client's sky runs on.
pub(crate) fn darkness(tick: u32) -> f32 {
    let phase = (f64::from(tick) / f64::from(TICK_RATE) / f64::from(DAY_SECONDS) + f64::from(DAY_START_PHASE)).fract();
    (((phase - 0.5).abs() - 0.25) / DUSK).clamp(0.0, 1.0) as f32
}

/// How much of `player` the crop they are in leaves for a looker at `eye` to see: 1 out of it, in a
/// vehicle, off the ground or in a winter world, whose fields are stubble; 0 when it hides them
/// altogether. Nothing here knows of a looker high enough to see down into the field.
pub(in crate::mechanics) fn crop_cover(world: &World, player: &Player, eye: [f32; 3]) -> f32 {
    let character = &player.character;
    let [x, y, z] = character.position;
    if world.map.theme == THEME_WINTER || player.seat.is_some() || character.chute || world.physics.crop_at(x, z) == CROP_NONE {
        return 1.0;
    }
    let feet = y - if character.prone { PRONE_CENTRE } else { STAND_CENTRE };
    if feet - world.map.ground.height_at(x, z) > CROP_CLEARANCE {
        return 1.0;
    }
    let distance = distance_2d(eye, character.position).max(1e-3);
    let toward = [(eye[0] - x) / distance * CROP_DEPTH, (eye[2] - z) / distance * CROP_DEPTH];
    if world.physics.crop_at(x + toward[0], z + toward[1]) == CROP_NONE {
        return 1.0;
    }
    let walking = player.input.forward != 0 || player.input.strafe != 0;
    match (character.prone, player.crouched(), walking) {
        (true, ..) | (_, true, false) => 0.0,
        (_, true, true) => IN_CROP_MOVING,
        _ => IN_CROP_STANDING,
    }
}

/// How readily `player`, `distance` metres off, is taken in, 1 in plain view. `crop` is their `crop_cover`.
fn visibility(player: &Player, distance: f32, dark: f32, crop: f32) -> f32 {
    let far = lerp(1.0, FAR, ((distance - NEAR) / (SIGHT_RANGE - NEAR)).clamp(0.0, 1.0));
    let height = match (player.character.prone, player.crouched()) {
        (true, _) => LYING,
        (_, true) => CROUCHED,
        _ => 1.0,
    };
    let walking = player.input.forward != 0 || player.input.strafe != 0;
    let moving = match (walking, player.input.buttons() & BUTTON_RUN != 0) {
        (false, _) => STILL,
        (true, true) => RUNNING,
        (true, false) => 1.0,
    };
    let light = if player.lamp { LAMP } else { lerp(1.0, DARK, dark) };
    far * height * moving * light * crop
}

/// One look at `other`, who is in view at `at`: whether the bot has taken them in by now.
pub(super) fn notice(world: &World, bot: &mut Bot, other: u32, at: [f32; 3], eye: [f32; 3], alert: bool) -> bool {
    let tracking = bot.sight.sees && bot.sight.target == Some(other);
    let distance = distance_2d(eye, at);
    if tracking || distance <= CLOSE_IN {
        bot.sight.aware = 1.0;
        return true;
    }
    let Some(player) = world.entities.get::<Player>(other) else { return false };
    let rate = visibility(player, distance, darkness(world.tick), crop_cover(world, player, eye)) * if alert { ALERT_RATE } else { 1.0 };
    bot.sight.aware = (bot.sight.aware + rate * LOOK_EVERY as f32 / TICK_RATE as f32 / FILL).min(1.0);
    if bot.sight.aware < 1.0 {
        if bot.mode == BotMode::Patrol {
            bot.sight.last_seen = at;
        }
        return false;
    }
    true
}

/// It looked and there was no one.
pub(super) fn fade(bot: &mut Bot) {
    bot.sight.aware = (bot.sight.aware - FADE * LOOK_EVERY as f32 / TICK_RATE as f32).max(0.0);
}

/// Where bot `id`, at `at`, thinks a noise at `from` was: a little off, the further the more, and
/// always the same way off for one bot, so that a squad does not converge on one point.
pub(super) fn roughly(id: u32, at: [f32; 3], from: [f32; 3]) -> [f32; 3] {
    let distance = distance_2d(at, from);
    if distance <= EXACT {
        return from;
    }
    let (sin, cos) = (id as f32 * 2.4).sin_cos();
    [from[0] + cos * distance * ROUGH, from[1], from[2] + sin * distance * ROUGH]
}
