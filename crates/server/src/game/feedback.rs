//! What the players are told about this tick's harm, built from its `Damaged` and `Died` events once
//! the mechanics have reacted to them: a hit marker (`Hit`) for whoever took or dealt each hit, and a
//! `Kill` for the feed. Nothing that hurts anything builds these itself.

use super::*;
use crate::entities::Hurtbox;
use crate::mechanics::bearing;

/// The hits this tick's damage makes, each with the player it is for, in the order it happened: the
/// one hurt (a player) and whoever rode in it (a vehicle) took it, coming from where it came from;
/// whoever did it, if not themself or aboard, dealt it. A hit's kind says what was struck: a soldier
/// (a kill when it was the last of their health), a vehicle, or a creature as its `Hurtbox` says.
pub fn hits(world: &World) -> Vec<(u32, Hit)> {
    let mut hits = Vec::new();
    for damaged in world.happened::<Damaged>() {
        let &Damaged { victim, lost, attacker, weapon, from, at, killed, .. } = damaged;
        let kind = if world.entities.has::<Player>(victim) {
            if killed { HIT_KILL } else { HIT_BODY }
        } else if let Some(hurtbox) = world.entities.get::<Hurtbox>(victim) {
            hurtbox.hit_kind
        } else {
            HIT_VEHICLE
        };
        let [x, y, z] = at.map(to_cm);
        let hit = |role, bearing| Hit { tag: S_HIT, role, kind, amount: lost, weapon, bearing, x, y, z, target: victim };
        let towards = from.map(|from| bearing(at, from));
        if world.entities.has::<Player>(victim) {
            hits.push((victim, hit(HIT_TAKEN, towards)));
        }
        hits.extend(damaged.aboard.iter().map(|&rider| (rider, hit(HIT_TAKEN, towards))));
        if let Some(by) = attacker.filter(|&by| by != victim && !damaged.aboard.contains(&by)) {
            hits.push((by, hit(HIT_DEALT, None)));
        }
    }
    hits
}

/// How hard (m/s) a killing blow throws the body when nothing else was pushing it: a bullet's.
const BULLET_DEATH_PUSH: f32 = 4.0;

/// A `Kill` for each player who died this tick, in the order they died, with the killer's kills and
/// the victim's deaths as they stood just after it: the scores are already credited for the whole
/// tick, so a later kill of the same tick is taken back off. The body is thrown the way a shove
/// already on it (a blast, a car) goes, or else away from where the blow came from.
pub fn kills(world: &World) -> Vec<Kill> {
    let died: Vec<Died> = world.happened::<Died>().filter(|death| world.entities.has::<Player>(death.victim)).copied().collect();
    let score = |id: u32, of: fn(&Player) -> u16| world.entities.get::<Player>(id).map_or(0, of);
    died.iter()
        .enumerate()
        .map(|(i, death)| {
            let later = &died[i + 1..];
            let killer = death.killer.filter(|&id| world.entities.has::<Player>(id));
            let killer_kills = killer.map_or(0, |id| score(id, |p| p.kills).saturating_sub(later.iter().filter(|d| d.killer == Some(id)).count() as u16));
            let victim_deaths = score(death.victim, |p| p.deaths).saturating_sub(later.iter().filter(|d| d.victim == death.victim).count() as u16);
            let body = world.entities.get::<Player>(death.victim).map(|player| player.character).unwrap_or_default();
            let [kx, kz] = body.knock;
            let pushed = kx.hypot(kz);
            let (yaw, power) = if pushed > 0.5 {
                (to_mrad((-kx).atan2(-kz)), pushed)
            } else {
                let toward = death.from.map_or(0, |from| bearing(body.position, from));
                (toward.wrapping_add(3142), BULLET_DEATH_PUSH)
            };
            let power = (power * 5.0).round().min(255.0) as u8;
            Kill { tag: S_KILL, killer: killer.unwrap_or(0), victim: death.victim, weapon: death.weapon, killer_kills, victim_deaths, style: death.style, yaw, power }
        })
        .collect()
}
