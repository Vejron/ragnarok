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
use shared::replicated::{OWNER_MASK, Replicated, Scene, Update, diff};
use std::collections::VecDeque;
use std::sync::Arc;

/// What a client holds after a snapshot, as the server keeps it: its entities shared with the world's
/// scene and every other receiver's, not copied into each.
type Held = Vec<(u32, Arc<Replicated>)>;

/// One player's replication state.
#[derive(Default)]
pub struct Replica {
    /// Newest snapshot tick the client says it holds; 0 for none.
    acked: u32,
    /// What the client held after each recent snapshot, oldest first.
    sent: VecDeque<(u32, Held)>,
}

/// One entity of the world, made ready to send: what anyone is told of it, and all of it, for its
/// owner, when it has `[owner]` components.
pub struct Ready {
    pub id: u32,
    pub public: Arc<Replicated>,
    pub whole: Option<Arc<Replicated>>,
    /// For a rider with no place of their own, their vehicle's index in the scene: they are sent to
    /// whoever is sent it.
    pub rides: Option<usize>,
}

/// The world's scene (ordered by id) made ready to send to everyone.
pub fn ready(scene: Scene) -> Vec<Ready> {
    let rides: Vec<Option<usize>> = scene
        .iter()
        .map(|(_, entity)| {
            let vehicle = entity.seated.as_ref().filter(|_| entity.place.is_none())?.vehicle;
            scene.binary_search_by_key(&vehicle, |(id, _)| *id).ok()
        })
        .collect();
    scene
        .into_iter()
        .zip(rides)
        .map(|((id, entity), rides)| match entity.mask() & OWNER_MASK {
            0 => Ready { id, public: Arc::new(entity), whole: None, rides },
            _ => Ready { id, public: Arc::new(entity.public()), whole: Some(Arc::new(entity)), rides },
        })
        .collect()
}

/// What a snapshot is built from: everything replicated (`ready`), and this snapshot's effects.
pub struct Everything<'a> {
    pub scene: &'a [Ready],
    pub shots: &'a [ShotEvent],
    pub bullets: &'a [BulletSpawn],
}

/// The parts of a snapshot that belong to one player rather than the world.
#[derive(Default)]
pub struct Own {
    pub tick: u32,
    pub ack: u32,
    pub ride: u32,
    /// The entities the player owns (themself, the vehicle they drive), whose `[owner]` components they
    /// alone are sent, and which they are sent wherever they are.
    pub owned: [Option<u32>; 2],
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

    fn baseline(&self) -> Option<&(u32, Held)> {
        if self.acked == 0 {
            return None;
        }
        self.sent.iter().rev().find(|(tick, _)| *tick == self.acked)
    }

    /// Builds the snapshot for the player standing at `at` (`[x, z]`, metres) and remembers what
    /// it leaves the client holding.
    ///
    /// What the player owns (`Own::owned`) is always sent, with its `[owner]` components; of everything
    /// else, only what anyone is told (`Replicated::public`). A rider with no place goes with their
    /// vehicle, at its distance. Everything within `INTEREST_RADIUS` is a candidate, and what was already held stays until it is `INTEREST_HYSTERESIS` further out; of items on the ground, only the nearest
    /// `MAX_PICKUPS_PER_SNAPSHOT`. If the encoded datagram would exceed `MAX_PACKET`, effects are left out
    /// first, then the least worth keeping (`worth`), farthest first; whatever did not fit simply
    /// arrives in a later snapshot.
    pub fn snapshot(&mut self, at: [f32; 2], world: &Everything, own: Own) -> Vec<u8> {
        let empty = Held::default();
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
        let owned = |id: u32| own.owned.contains(&Some(id));
        // Riders with no place (by index), with their vehicle's.
        let mut riding: Vec<(usize, usize)> = Vec::new();
        for (index, Ready { id, public: entity, rides, .. }) in world.scene.iter().enumerate() {
            if owned(*id) {
                near.push((index, entity.place.map_or(0.0, |place| distance(place.x, place.z))));
                continue;
            }
            let Some(place) = entity.place else {
                riding.extend(rides.map(|vehicle| (index, vehicle)));
                continue;
            };
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
        let riders: Vec<(usize, f32)> = riding.into_iter().filter_map(|(index, vehicle)| Some((index, near[near.binary_search_by_key(&vehicle, |(index, _)| *index).ok()?].1))).collect();
        if !riders.is_empty() {
            near.extend(riders);
            near.sort_unstable_by_key(|(index, _)| *index);
        }
        let now: Held = near
            .iter()
            .map(|&(index, _)| {
                let Ready { id, public, whole, .. } = &world.scene[index];
                (*id, Arc::clone(whole.as_ref().filter(|_| owned(*id)).unwrap_or(public)))
            })
            .collect();
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
            encode(&SnapshotOut { tag: S_SNAPSHOT, tick: own.tick, baseline: baseline_tick, ack: own.ack, ride: own.ride, updates, removed: &removed, shots, bullets })
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
        let held = if trimmed { apply_held(baseline, &updates, &removed) } else { now };
        self.sent.push_back((own.tick, held));
        while self.sent.len() > BASELINE_WINDOW {
            self.sent.pop_front();
        }
        bytes
    }
}

/// What `updates` and `removed` make of `base`, as `shared::replicated::apply` does, sharing what they
/// leave as it was.
fn apply_held(base: &Held, updates: &[Update], removed: &[u32]) -> Held {
    let mut scene: Held = base.iter().filter(|(id, _)| !removed.contains(id)).cloned().collect();
    for update in updates {
        match scene.binary_search_by_key(&update.id, |(id, _)| *id) {
            Ok(at) => scene[at].1 = Arc::new(Replicated::apply(Some(&scene[at].1), update)),
            Err(at) => scene.insert(at, (update.id, Arc::new(Replicated::apply(None, update)))),
        }
    }
    scene
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
