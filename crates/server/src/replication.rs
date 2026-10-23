//! What each player is sent: only what is near them, and only what changed since the snapshot
//! they last acknowledged. See design.md §5.8 and NETWORKING.md.
//!
//! The server keeps, per player, the scenes of its last `BASELINE_WINDOW` snapshots. The client
//! says in every input which snapshot it has rebuilt (`baseline`); the next snapshot is the
//! difference between that scene and the one in range now, one update per entity
//! (`shared::replicated`). The client applies the same updates, so what it holds is what the server
//! stored, with no echo needed.

use shared::consts::*;
use shared::items::{MAX_PICKUPS_PER_SNAPSHOT, def};
use shared::protocol::*;
use shared::replicated::{Replicated, Scene, Update, apply, diff};
use std::collections::VecDeque;

/// One player's replication state.
#[derive(Default)]
pub struct Replica {
    /// Newest snapshot tick the client says it holds; 0 for none.
    acked: u32,
    /// What the client held after each recent snapshot, oldest first.
    sent: VecDeque<(u32, Scene)>,
}

/// What a snapshot is built from: everything replicated (ordered by id), and this snapshot's effects.
pub struct Everything<'a> {
    pub scene: &'a Scene,
    pub shots: &'a [ShotEvent],
    pub bullets: &'a [BulletSpawn],
}

/// The parts of a snapshot that belong to one player rather than the world.
#[derive(Default)]
pub struct Own {
    pub tick: u32,
    pub ack: u32,
    pub ride: u32,
    pub drive: Option<DriveState>,
    pub me: SelfState,
}

/// How much an update is worth keeping when a snapshot will not fit, highest first: everything else,
/// then items lying on the ground, then projectiles, which a later snapshot catches up on soonest.
/// Bullets come between the first two, and effects (`shots`) last of all.
fn worth(entity: &Replicated) -> u8 {
    if entity.projectile.is_some() {
        0
    } else if entity.pickup.is_some() {
        1
    } else {
        3
    }
}
const BULLETS_WORTH: u8 = 2;

impl Replica {
    /// Notes the newest snapshot the client reports holding. Only ever moves forward.
    pub fn acknowledge(&mut self, tick: u32) {
        // Ticks wrap after about two years; compare as a signed distance.
        if tick.wrapping_sub(self.acked) as i32 > 0 {
            self.acked = tick;
        }
    }

    fn baseline(&self) -> Option<&(u32, Scene)> {
        if self.acked == 0 {
            return None;
        }
        self.sent.iter().rev().find(|(tick, _)| *tick == self.acked)
    }

    /// Builds the snapshot for the player standing at `at` (`[x, z]`, metres) and remembers what
    /// it leaves the client holding.
    ///
    /// Everything within `INTEREST_RADIUS` is a candidate, and what was already held stays until it is
    /// `INTEREST_HYSTERESIS` further out; of items on the ground, only the nearest
    /// `MAX_PICKUPS_PER_SNAPSHOT`. If the encoded datagram would exceed `MAX_PACKET`, effects are left out
    /// first, then the least worth keeping (`worth`), farthest first; whatever did not fit simply
    /// arrives in a later snapshot.
    pub fn snapshot(&mut self, at: [f32; 2], world: &Everything, own: Own) -> Vec<u8> {
        let empty = Scene::default();
        let (baseline_tick, baseline) = match self.baseline() {
            Some((tick, scene)) => (*tick, scene),
            None => (0, &empty),
        };
        let held = |id: u32| baseline.binary_search_by_key(&id, |(held, _)| *held).is_ok();
        let distance = |x: i32, z: i32| ((x as f32 / 100.0 - at[0]).powi(2) + (z as f32 / 100.0 - at[1]).powi(2)).sqrt();
        let within = |held: bool, d: f32| d <= INTEREST_RADIUS + if held { INTEREST_HYSTERESIS } else { 0.0 };

        // What is in range (by its index in the world's scene), with how far it is. Items on the ground
        // are static and many, so only the nearest few are listed: the rest arrive as the player walks up.
        let mut near: Vec<(usize, f32)> = Vec::new();
        let mut pickups: Vec<(usize, f32)> = Vec::new();
        for (index, (id, entity)) in world.scene.iter().enumerate() {
            let Some(place) = entity.place else { continue };
            let d = distance(place.x, place.z);
            if entity.pickup.is_some() {
                if d <= INTEREST_RADIUS {
                    pickups.push((index, d));
                }
            } else if within(held(*id), d) {
                near.push((index, d));
            }
        }
        pickups.sort_by(|a, b| a.1.total_cmp(&b.1));
        near.extend(pickups.into_iter().take(MAX_PICKUPS_PER_SNAPSHOT));
        near.sort_unstable_by_key(|(index, _)| *index);
        let now: Scene = near.iter().map(|&(index, _)| world.scene[index].clone()).collect();
        // `now` is ordered by id, as the world's scene is; this is how far each of it is.
        let distances: Vec<f32> = near.iter().map(|&(_, d)| d).collect();

        let mut shots: Vec<ShotEvent> = world.shots.iter().filter(|s| distance(s.x, s.z) <= INTEREST_RADIUS).copied().collect();
        shots.truncate(MAX_SHOT_EVENTS);
        // A bullet is wanted by everyone whose view its line crosses, not only by those near the muzzle:
        // it flies up to its gun's range, mostly level.
        let mut bullets: Vec<(f32, BulletSpawn)> = world
            .bullets
            .iter()
            .filter_map(|b| {
                let range = def(b.item).map_or(0.0, |d| d.range);
                let (ox, oz) = (b.x as f32 / 100.0, b.z as f32 / 100.0);
                let (vx, vz) = (b.vx as f32 / 100.0, b.vz as f32 / 100.0);
                let flat = (vx * vx + vz * vz).sqrt().max(0.001);
                let (dx, dz) = (vx / flat * range, vz / flat * range);
                let length2 = dx * dx + dz * dz;
                let along = if length2 > 0.0 { (((at[0] - ox) * dx + (at[1] - oz) * dz) / length2).clamp(0.0, 1.0) } else { 0.0 };
                let d = ((ox + dx * along - at[0]).powi(2) + (oz + dz * along - at[1]).powi(2)).sqrt();
                (d <= INTEREST_RADIUS).then_some((d, *b))
            })
            .collect();
        bullets.sort_by(|a, b| a.0.total_cmp(&b.0));
        bullets.truncate(MAX_BULLET_SPAWNS);
        let mut bullets: Vec<BulletSpawn> = bullets.into_iter().map(|(_, b)| b).collect();

        let (mut updates, removed) = diff(baseline, &now);
        // Nearest first, so what is kept when trimming is the nearest. Each distance is looked up once:
        // they are never negative, and the bits of a positive float sort as the float does.
        let distance_of = |id: u32| now.binary_search_by_key(&id, |(held, _)| *held).map_or(0.0, |at| distances[at]);
        updates.sort_by_cached_key(|update| distance_of(update.id).to_bits());
        let encode_with = |updates: &[Update], shots: &[ShotEvent], bullets: &[BulletSpawn]| {
            encode(&Snapshot { tag: S_SNAPSHOT, tick: own.tick, baseline: baseline_tick, ack: own.ack, ride: own.ride, updates: updates.to_vec(), removed: removed.clone(), drive: own.drive, me: own.me.clone(), shots: shots.to_vec(), bullets: bullets.to_vec() })
        };
        let mut bytes = encode_with(&updates, &shots, &bullets);
        let mut trimmed = false;
        // Effects are cosmetic: they go first, as many at a time as the excess calls for.
        while bytes.len() > MAX_PACKET && !shots.is_empty() {
            let excess = bytes.len() - MAX_PACKET;
            shots.truncate(shots.len() - shots.len().min(excess.div_ceil(10)).max(1));
            bytes = encode_with(&updates, &shots, &bullets);
        }
        // Then whatever is worth least, farthest first. Each list goes on the wire as a MessagePack
        // array, its header and then its items one after another, so what fits is worked out from each
        // item's size and the snapshot with the lists empty: two encodes, rather than one per thing shed.
        // `removed` is never shed: stale things would linger.
        if bytes.len() > MAX_PACKET {
            let mut buffer = Vec::new();
            let worth_of = |id: u32| now.binary_search_by_key(&id, |(held, _)| *held).map_or(3, |at| worth(&now[at].1));
            // (worth, distance, an update's index or a bullet's, bytes on the wire)
            let mut order: Vec<(u8, f32, Option<usize>, usize)> = Vec::new();
            order.extend(updates.iter().enumerate().map(|(i, update)| (worth_of(update.id), distance_of(update.id), Some(i), wire_size(update, &mut buffer))));
            order.extend(bullets.iter().map(|bullet| (BULLETS_WORTH, 0.0, None, wire_size(bullet, &mut buffer))));
            // Most worth first, then nearest; stable, so bullets keep their nearest-first order.
            order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)));
            let mut total = encode_with(&[], &shots, &[]).len();
            let (mut kept, mut kept_bullets) = (Vec::new(), 0);
            for &(_, _, index, size) in &order {
                let count = if index.is_some() { kept.len() } else { kept_bullets };
                let grown = total + size + array_header(count + 1) - array_header(count);
                if grown > MAX_PACKET {
                    break;
                }
                total = grown;
                match index {
                    Some(i) => kept.push(i),
                    None => kept_bullets += 1,
                }
            }
            kept.sort_unstable();
            trimmed = kept.len() < updates.len();
            updates = kept.into_iter().map(|i| updates[i].clone()).collect();
            bullets.truncate(kept_bullets);
            bytes = encode_with(&updates, &shots, &bullets);
            debug_assert_eq!(bytes.len(), total, "the worked-out size is the encoded size");
        }
        // What the client holds now is the baseline with the updates applied: what is in range, unless
        // some were trimmed.
        let held = if trimmed { apply(baseline, &updates, &removed) } else { now };
        self.sent.push_back((own.tick, held));
        while self.sent.len() > BASELINE_WINDOW {
            self.sent.pop_front();
        }
        bytes
    }
}

/// What `item` takes up inside an encoded message.
fn wire_size<T: serde::Serialize>(item: &T, buffer: &mut Vec<u8>) -> usize {
    buffer.clear();
    rmp_serde::encode::write(buffer, item).expect("protocol messages always serialize");
    buffer.len()
}

/// The bytes MessagePack spends saying an array holds `len` items.
fn array_header(len: usize) -> usize {
    match len {
        0..16 => 1,
        16..=0xFFFF => 3,
        _ => 5,
    }
}

#[cfg(test)]
mod tests;
