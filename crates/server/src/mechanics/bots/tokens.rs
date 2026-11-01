//! Attack tokens: however many bots are after someone, only a few really try to kill them. Each
//! target has a few tokens, and a bot needs one to fight as it means it. The rest stay busy: a couple
//! run round to the target's sides, those who can see the target fire wide to keep their heads down,
//! and the others wait in a crouch for a place to open. A flanker that gets there is next in line, so
//! being flanked is dangerous. Anyone right next to the target fights regardless: a bot beside you
//! that does not shoot looks broken.
//!
//! Handed out every `ASSIGN_EVERY` ticks. A squad no bigger than its tokens is all attackers.

use super::*;
use crate::game::Role;

/// How often roles are handed out, ticks.
pub(super) const ASSIGN_EVERY: u32 = TICK_RATE / 2;
/// Tokens on one target, at skill 0 and at skill 1.
const TOKENS_POOR: f32 = 2.0;
const TOKENS_GOOD: f32 = 4.0;
/// How long a bot keeps its token before it has to compete for it again, ticks.
const TOKEN_TICKS: u32 = 4 * TICK_RATE;
/// Nearer the target than this it fights, token or no token, metres.
const POINT_BLANK: f32 = 8.0;
/// The fight goes on while someone on the bot's side has seen the target this recently.
const CONTACT_TICKS: u32 = 5 * TICK_RATE;
/// Flankers on one target, the most, and how long one has to get there, ticks.
const MAX_FLANKERS: usize = 2;
const FLANK_TICKS: u32 = 8 * TICK_RATE;
/// How far round the target it goes from where it is now, radians, and how far from it it ends up,
/// metres, the nearer if the first is not open ground.
const FLANK_ANGLE: f32 = 1.75;
const FLANK_REACH: [f32; 2] = [12.0, 8.0];
/// A flanker this near its point has got there, metres.
const FLANK_REACHED: f32 = 2.0;
/// One with no token and no sight of the target stops this far from where it was last seen, metres.
pub(super) const HOLD_OFF: f32 = 20.0;

/// A bot in a fight, as it stands.
struct Member {
    id: u32,
    distance: f32,
    sees: bool,
    /// Reloading or out of rounds: it gives its token back.
    busy: bool,
    kept: bool,
    flanking: bool,
    flanked: bool,
    skill: f32,
}

/// How far ahead of `target`, facing `yaw`, `point` is: a flanker wants this low, out of their view.
fn ahead_of(target: [f32; 3], yaw: f32, point: [f32; 3]) -> f32 {
    (point[0] - target[0]) * -yaw.sin() + (point[2] - target[2]) * -yaw.cos()
}

/// The point round `target` from `from`, on `side` (1 or -1), if there is open ground there.
fn flank_point(world: &World, target: [f32; 3], from: [f32; 3], side: f32) -> Option<[f32; 3]> {
    let (sin, cos) = (heading(target, from) + side * FLANK_ANGLE).sin_cos();
    FLANK_REACH.iter().find_map(|reach| {
        let (x, z) = (target[0] - sin * reach, target[2] - cos * reach);
        open_ground(world, x, z).map(|floor| [x, floor + sim::STANDING_Y, z])
    })
}

/// Goes on a way to a flank; true if there was somewhere to go. With another already on its way to
/// `taken` it takes the far side from them, and otherwise the side further out of the sight of
/// `target`, facing `yaw`.
pub(super) fn send_flanking(world: &World, bot: &mut Bot, target: [f32; 3], yaw: f32, from: [f32; 3], taken: Option<[f32; 3]>) -> bool {
    let badness = |point: [f32; 3]| match taken {
        Some(taken) => -distance_2d(point, taken),
        None => ahead_of(target, yaw, point),
    };
    let sides = [flank_point(world, target, from, 1.0), flank_point(world, target, from, -1.0)];
    let Some(point) = sides.into_iter().flatten().min_by(|&a, &b| badness(a).total_cmp(&badness(b))) else { return false };
    bot.flank = Some(point);
    bot.flank_left = FLANK_TICKS;
    bot.flanked = false;
    bot.walk.path.clear();
    new_goal(bot);
    true
}

/// Hands out the roles in the fights `ids` are in. A bot in none is an attacker with nothing kept,
/// which is how one on its own has always fought.
pub(super) fn assign(world: &mut World, ids: &[u32]) {
    let mut fights: Vec<(u32, Vec<Member>)> = Vec::new();
    let mut freed: Vec<u32> = Vec::new();
    for &id in ids {
        let (Some(player), Some(bot)) = (world.entities.get::<Player>(id), world.entities.get::<Bot>(id)) else { continue };
        let foe = bot.sight.target.filter(|_| bot.mode == BotMode::Pursue && world.entities.alive(id) && player.seat.is_none());
        let Some(foe_at) = foe.and_then(|foe| world.entities.get::<Player>(foe).filter(|p| world.entities.alive(foe) && p.embodied())).map(|p| p.character.position) else {
            freed.push(id);
            continue;
        };
        let flank_left = bot.flank_left.saturating_sub(ASSIGN_EVERY);
        let held = player.hands.hotbar[(bot.slot as usize).min(HOTBAR_SLOTS - 1)];
        let member = Member {
            id,
            distance: distance_2d(player.character.position, foe_at),
            sees: bot.sight.sees,
            busy: player.hands.reload_left > 0 || held.ammo == 0,
            kept: bot.role == Role::Attacker && bot.token_left > ASSIGN_EVERY,
            flanking: bot.flank.is_some() && flank_left > 0,
            flanked: bot.flanked,
            skill: bot.aim.skill,
        };
        let Some(foe) = foe else { continue };
        match fights.iter_mut().find(|(target, _)| *target == foe) {
            Some((_, members)) => members.push(member),
            None => fights.push((foe, vec![member])),
        }
    }
    for id in freed {
        if let Some(mut bot) = world.entities.get_mut::<Bot>(id) {
            bot.role = Role::Attacker;
            bot.token_left = 0;
            bot.flank = None;
            bot.flanked = false;
        }
    }
    for (target, members) in fights {
        run_fight(world, target, members);
    }
}

/// Who does what in the fight of `members` against `target`.
fn run_fight(world: &mut World, target: u32, mut members: Vec<Member>) {
    let (target_at, target_yaw) = {
        let target = &world.entities.of::<Player>(target).character;
        (target.position, target.yaw)
    };
    // What one of them has seen they have all been told, and the fight is on while anyone has.
    let latest = members.iter().filter_map(|m| world.entities.get::<Bot>(m.id).map(|bot| (bot.sight.seen_tick, bot.sight.last_seen))).max_by_key(|&(tick, _)| tick);
    let Some((seen_tick, last_seen)) = latest else { return };
    let contact = world.tick.wrapping_sub(seen_tick) < CONTACT_TICKS;
    let skill = members.iter().map(|m| m.skill).sum::<f32>() / members.len() as f32;
    let mut tokens = lerp(TOKENS_POOR, TOKENS_GOOD, skill).round() as usize;
    if !contact || members.len() <= tokens {
        // No one is waiting for a place, or the fight is over and they are all looking.
        tokens = members.len();
        members.iter_mut().for_each(|member| member.busy = false);
    }

    // Nearest first, those who have been round the side ahead of them, those who can see ahead of
    // those who cannot.
    members.sort_by(|a, b| (!a.flanked, !a.sees, a.distance).partial_cmp(&(!b.flanked, !b.sees, b.distance)).unwrap_or(std::cmp::Ordering::Equal).then(a.id.cmp(&b.id)));
    let mut roles: Vec<(u32, Role)> = Vec::with_capacity(members.len());
    let mut rest: Vec<&Member> = Vec::new();
    for member in &members {
        if member.distance < POINT_BLANK {
            roles.push((member.id, Role::Attacker));
        } else if member.kept && !member.busy && tokens > 0 {
            tokens -= 1;
            roles.push((member.id, Role::Attacker));
        } else {
            rest.push(member);
        }
    }
    let mut flankers = rest.iter().filter(|m| m.flanking).count().min(MAX_FLANKERS);
    let mut taken = rest.iter().filter(|m| m.flanking).find_map(|m| world.entities.get::<Bot>(m.id).and_then(|bot| bot.flank));
    for member in rest {
        let role = if member.flanking {
            Role::Flanker
        } else if tokens > 0 && !member.busy {
            tokens -= 1;
            Role::Attacker
        } else if flankers < MAX_FLANKERS && !member.busy && !member.flanked {
            let from = world.entities.of::<Player>(member.id).character.position;
            let Some(mut bot) = world.entities.get_mut::<Bot>(member.id) else { continue };
            let mut mind = std::mem::replace(&mut *bot, Bot::new([0.0; 2], Vec::new()));
            let sent = send_flanking(world, &mut mind, target_at, target_yaw, from, taken);
            taken = mind.flank.or(taken);
            *world.entities.of_mut::<Bot>(member.id) = mind;
            if sent {
                flankers += 1;
                Role::Flanker
            } else if member.sees {
                Role::Suppressor
            } else {
                Role::Holder
            }
        } else if member.sees {
            Role::Suppressor
        } else {
            Role::Holder
        };
        roles.push((member.id, role));
    }

    for (id, role) in roles {
        let Some(mut bot) = world.entities.get_mut::<Bot>(id) else { continue };
        if !bot.sight.sees && contact {
            bot.sight.last_seen = last_seen;
        }
        bot.token_left = match (role, bot.role) {
            (Role::Attacker, Role::Attacker) => bot.token_left.saturating_sub(ASSIGN_EVERY),
            (Role::Attacker, _) => TOKEN_TICKS,
            _ => 0,
        };
        if role == Role::Attacker && bot.token_left == 0 {
            bot.token_left = TOKEN_TICKS;
        }
        bot.flank_left = bot.flank_left.saturating_sub(ASSIGN_EVERY);
        if role != Role::Flanker {
            bot.flank = None;
        }
        if role == Role::Attacker {
            bot.flanked = false;
        }
        bot.role = role;
    }
}

/// The point a flanker is running for, once it has noted whether it has got there.
pub(super) fn flank_goal(bot: &mut Bot, at: [f32; 3]) -> Option<[f32; 3]> {
    let point = bot.flank.filter(|_| bot.role == Role::Flanker)?;
    if distance_2d(at, point) < FLANK_REACHED {
        bot.flank = None;
        bot.flanked = true;
        bot.role = Role::Suppressor;
        bot.aim.tracked = 0;
        return None;
    }
    Some(point)
}
