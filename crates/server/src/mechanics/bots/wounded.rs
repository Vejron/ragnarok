//! What a bot does when it is hurt or has had enough: it bandages itself in cover, and one that has
//! frozen with nowhere to go gives in. A timid one puts its hands up; a patient one lies still and
//! waits for you to come close.

use super::*;
use crate::entities::Health;
use shared::items::ITEM_BANDAGE;

/// Under this courage it surrenders, and from this patience it plays dead.
const SURRENDER_COURAGE: f32 = 0.35;
const PLAY_DEAD_PATIENCE: f32 = 0.6;
/// It gives in only to someone this near, metres, and surrenders only with no friend this near.
const GIVE_IN_NEAR: f32 = 25.0;
const FRIEND_NEAR: f32 = 15.0;
/// How long it keeps it up, seconds.
const SURRENDER_TICKS: u32 = 20 * TICK_RATE;
const PLAY_DEAD_TICKS: u32 = 15 * TICK_RATE;
/// Playing dead, it springs on anyone this near, metres, with this much nerve.
const SPRING_NEAR: f32 = 6.0;
const SPRING_NERVE: f32 = 0.6;
/// Its aim on springing, as if it had been watching this long.
const SPRING_TRACKED: u32 = 3 * TICK_RATE;
/// The nerve it gets up with.
const RISEN_NERVE: f32 = 0.5;

/// The input that bandages bot `id`, if it is hurt and has a bandage or already has one half on.
pub(super) fn bandage(world: &World, id: u32, idle: InputFrame) -> Option<InputFrame> {
    let player = world.entities.get::<Player>(id)?;
    let (on, hurt) = (player.hands.channel_left > 0, world.entities.get::<Health>(id).is_some_and(|health| health.hp < PLAYER_MAX_HP));
    if !on && !hurt {
        return None;
    }
    let slot = player.hands.hotbar.iter().position(|slot| slot.item == ITEM_BANDAGE)?;
    let press = if on || player.hands.trigger_held { 0 } else { BUTTON_FIRE };
    Some(InputFrame { slot: slot as u8, buttons: BUTTON_CROUCH | press, ..idle })
}

/// Whether bot `id`, at `at`, has friends near enough to stand by it.
fn has_friends(id: u32, at: [f32; 3], crowd: &[(u32, [f32; 2])]) -> bool {
    crowd.iter().any(|&(other, [x, z])| other != id && (x - at[0]).hypot(z - at[2]) < FRIEND_NEAR)
}

/// Frozen with nowhere to go: it gives in, if it is the sort to.
pub(super) fn give_in(world: &mut World, id: u32, bot: &mut Bot, at: [f32; 3], crowd: &[(u32, [f32; 2])]) {
    if distance_2d(at, bot.sight.last_seen) > GIVE_IN_NEAR {
        return;
    }
    if bot.temper.courage < SURRENDER_COURAGE && !has_friends(id, at, crowd) {
        tracing::debug!(bot = id, "bot surrenders");
        bot.mode = BotMode::Surrender;
        bot.mode_left = SURRENDER_TICKS;
        if let Some(mut player) = world.entities.get_mut::<Player>(id) {
            player.surrendered = true;
        }
    } else if bot.temper.patience >= PLAY_DEAD_PATIENCE {
        tracing::debug!(bot = id, "bot plays dead");
        bot.mode = BotMode::PlayDead;
        bot.mode_left = PLAY_DEAD_TICKS;
    }
}

/// It is over: back on its feet and to its route, shaken still.
pub(super) fn rise(world: &mut World, id: u32, bot: &mut Bot, at: [f32; 3]) {
    if let Some(mut player) = world.entities.get_mut::<Player>(id) {
        player.surrendered = false;
    }
    bot.nerve = bot.nerve.max(RISEN_NERVE);
    resume_patrol(bot, at);
}

/// Whether it is time to stop playing dead and go for them, which it then does, its aim all ready.
pub(super) fn spring(bot: &mut Bot, at: [f32; 3]) -> bool {
    if !bot.sight.sees || distance_2d(at, bot.sight.last_seen) >= SPRING_NEAR {
        return false;
    }
    bot.mode = BotMode::Pursue;
    bot.nerve = SPRING_NERVE;
    bot.aim.react = 0;
    bot.aim.tracked = SPRING_TRACKED;
    true
}
