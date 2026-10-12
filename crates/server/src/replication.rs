//! What each player is sent: only what is near them, and only what changed since the snapshot
//! they last acknowledged. See design.md §5.8 and NETWORKING.md.
//!
//! The server keeps, per player, the scenes of its last `BASELINE_WINDOW` snapshots. The client
//! says in every input which snapshot it has rebuilt (`baseline`); the next snapshot is the
//! difference between that scene and the one in range now. The client runs `delta::apply` over the
//! same lists, so what it holds is what the server stored, with no echo needed.

use shared::consts::*;
use shared::delta::{Delta, Scene, apply, diff};
use shared::items::MAX_PICKUPS_PER_SNAPSHOT;
use shared::protocol::*;
use std::collections::{HashMap, VecDeque};

/// One player's replication state.
#[derive(Default)]
pub struct Replica {
    /// Newest snapshot tick the client says it holds; 0 for none.
    acked: u32,
    /// What the client held after each recent snapshot, oldest first.
    sent: VecDeque<(u32, Scene)>,
}

/// What a snapshot is built from: the whole world, each list ordered by id.
pub struct Everything<'a> {
    pub entities: &'a [EntityState],
    pub vehicles: &'a [VehicleState],
    pub shots: &'a [ShotEvent],
    pub rockets: &'a [ProjectileState],
    pub pickups: &'a [PickupState],
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
    /// `INTEREST_HYSTERESIS` further out. If the encoded datagram would exceed `MAX_PACKET`, the
    /// farthest things are left out of it, cosmetic effects first; a thing that did not fit simply
    /// arrives in a later snapshot, nearest first.
    pub fn snapshot(&mut self, at: [f32; 2], world: &Everything, own: Own) -> Vec<u8> {
        let empty = Scene::default();
        let (baseline_tick, baseline) = match self.baseline() {
            Some((tick, scene)) => (*tick, scene),
            None => (0, &empty),
        };
        let held_entity = |id: u32| baseline.entities.binary_search_by_key(&id, |e| e.id).is_ok();
        let held_vehicle = |id: u32| baseline.vehicles.binary_search_by_key(&id, |v| v.id).is_ok();
        let distance = |x: i32, z: i32| ((x as f32 / 100.0 - at[0]).powi(2) + (z as f32 / 100.0 - at[1]).powi(2)).sqrt();
        let within = |held: bool, d: f32| d <= INTEREST_RADIUS + if held { INTEREST_HYSTERESIS } else { 0.0 };

        // Nearest first, so trimming the tail drops the farthest.
        let mut entities: Vec<(f32, EntityState)> = world
            .entities
            .iter()
            .map(|e| (distance(e.x, e.z), *e))
            .filter(|(d, e)| within(held_entity(e.id), *d))
            .collect();
        let mut vehicles: Vec<(f32, VehicleState)> = world
            .vehicles
            .iter()
            .map(|v| (distance(v.x, v.z), v.clone()))
            .filter(|(d, v)| within(held_vehicle(v.id), *d))
            .collect();
        entities.sort_by(|a, b| a.0.total_cmp(&b.0));
        vehicles.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut shots: Vec<ShotEvent> = world.shots.iter().filter(|s| distance(s.x, s.z) <= INTEREST_RADIUS).copied().collect();
        shots.truncate(MAX_SHOT_EVENTS);
        let mut rockets: Vec<ProjectileState> = world.rockets.iter().filter(|r| distance(r.x, r.z) <= INTEREST_RADIUS).copied().collect();
        // Items on the ground are static, so only the nearest few are listed: the rest arrive as the player walks up.
        let mut pickups: Vec<(f32, PickupState)> = world.pickups.iter().map(|p| (distance(p.x, p.z), *p)).filter(|(d, _)| *d <= INTEREST_RADIUS).collect();
        pickups.sort_by(|a, b| a.0.total_cmp(&b.0));
        pickups.truncate(MAX_PICKUPS_PER_SNAPSHOT);
        let mut pickups: Vec<PickupState> = pickups.into_iter().map(|(_, p)| p).collect();

        let by_id = |entities: &[(f32, EntityState)]| {
            let mut list: Vec<EntityState> = entities.iter().map(|(_, e)| *e).collect();
            list.sort_by_key(|e| e.id);
            list
        };
        let scene_of = |entities: &[(f32, EntityState)], vehicles: &[(f32, VehicleState)]| {
            let mut v: Vec<VehicleState> = vehicles.iter().map(|(_, v)| v.clone()).collect();
            v.sort_by_key(|v| v.id);
            Scene { entities: by_id(entities), vehicles: v }
        };

        let mut delta = diff(baseline, &scene_of(&entities, &vehicles));
        // Distances by id, to order the delta lists nearest-first.
        let entity_distances: HashMap<u32, f32> = entities.iter().map(|(d, e)| (e.id, *d)).collect();
        let vehicle_distances: HashMap<u32, f32> = vehicles.iter().map(|(d, v)| (v.id, *d)).collect();
        let entity_distance = |id: u32| entity_distances.get(&id).copied().unwrap_or(0.0);
        let vehicle_distance = |id: u32| vehicle_distances.get(&id).copied().unwrap_or(0.0);
        delta.new_entities.sort_by(|a, b| entity_distance(a.id).total_cmp(&entity_distance(b.id)));
        delta.entities.sort_by(|a, b| entity_distance(a.id).total_cmp(&entity_distance(b.id)));
        delta.new_vehicles.sort_by(|a, b| vehicle_distance(a.id).total_cmp(&vehicle_distance(b.id)));
        delta.vehicles.sort_by(|a, b| vehicle_distance(a.id).total_cmp(&vehicle_distance(b.id)));

        let encode_with = |delta: &Delta, shots: &[ShotEvent], rockets: &[ProjectileState], pickups: &[PickupState]| {
            encode(&Snapshot { tag: S_SNAPSHOT, tick: own.tick, baseline: baseline_tick, ack: own.ack, ride: own.ride, new_entities: delta.new_entities.clone(), entities: delta.entities.clone(), new_vehicles: delta.new_vehicles.clone(), vehicles: delta.vehicles.clone(), removed: delta.removed.clone(), drive: own.drive, me: own.me.clone(), shots: shots.to_vec(), rockets: rockets.to_vec(), pickups: pickups.to_vec() })
        };
        let mut bytes = encode_with(&delta, &shots, &rockets, &pickups);
        // Shed what can be shed: effects (cosmetic) first, then rockets, then the farthest
        // newcomers and changes. `removed` is never shed: stale things would linger.
        while bytes.len() > MAX_PACKET {
            let candidates = [
                delta.new_entities.last().map(|e| (entity_distance(e.id), 0)),
                delta.entities.last().map(|e| (entity_distance(e.id), 1)),
                delta.new_vehicles.last().map(|v| (vehicle_distance(v.id), 2)),
                delta.vehicles.last().map(|v| (vehicle_distance(v.id), 3)),
            ];
            if shots.pop().is_some() || rockets.pop().is_some() || pickups.pop().is_some() {
            } else if let Some((_, which)) = candidates.into_iter().flatten().max_by(|a, b| a.0.total_cmp(&b.0)) {
                match which {
                    0 => drop(delta.new_entities.pop()),
                    1 => drop(delta.entities.pop()),
                    2 => drop(delta.new_vehicles.pop()),
                    _ => drop(delta.vehicles.pop()),
                }
            } else {
                break;
            }
            bytes = encode_with(&delta, &shots, &rockets, &pickups);
        }

        // What the client holds now is the baseline with the (possibly trimmed) delta applied.
        let held = apply(baseline, &delta);
        self.sent.push_back((own.tick, held));
        while self.sent.len() > BASELINE_WINDOW {
            self.sent.pop_front();
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: u32, x_m: f32, z_m: f32) -> EntityState {
        EntityState { id, kind: KIND_STICKMAN, x: (x_m * 100.0) as i32, y: 90, z: (z_m * 100.0) as i32, yaw: 0, vy: 0, flags: FLAG_GROUNDED, knock_x: 0, knock_z: 0 }
    }

    fn snapshot(replica: &mut Replica, tick: u32, entities: &[EntityState]) -> Snapshot {
        let world = Everything { entities, vehicles: &[], shots: &[], rockets: &[], pickups: &[] };
        let bytes = replica.snapshot([0.0, 0.0], &world, Own { tick, ..Default::default() });
        assert!(bytes.len() <= MAX_PACKET);
        decode(&bytes).unwrap()
    }

    /// What a client rebuilds from `snapshot`, given the scenes it holds.
    fn rebuild(held: &mut Vec<(u32, Scene)>, snapshot: &Snapshot) -> Scene {
        let base = held.iter().find(|(tick, _)| *tick == snapshot.baseline).map(|(_, s)| s.clone()).unwrap_or_default();
        let delta = Delta {
            new_entities: snapshot.new_entities.clone(),
            entities: snapshot.entities.clone(),
            new_vehicles: snapshot.new_vehicles.clone(),
            vehicles: snapshot.vehicles.clone(),
            removed: snapshot.removed.clone(),
        };
        let scene = apply(&base, &delta);
        held.push((snapshot.tick, scene.clone()));
        scene
    }

    #[test]
    fn only_the_near_are_sent_and_the_far_come_in_as_you_approach() {
        let mut replica = Replica::default();
        let mut held = Vec::new();
        let near = person(1, 10.0, 0.0);
        let far = person(2, INTEREST_RADIUS + 20.0, 0.0);
        let first = snapshot(&mut replica, 2, &[near, far]);
        assert_eq!(rebuild(&mut held, &first).entities, vec![near]);

        // Moves into range, but stays listed once in: the edge does not flicker.
        replica.acknowledge(2);
        let closer = person(2, INTEREST_RADIUS - 10.0, 0.0);
        let second = snapshot(&mut replica, 4, &[near, closer]);
        assert_eq!(second.baseline, 2, "diffed against the acknowledged snapshot");
        assert_eq!(rebuild(&mut held, &second).entities, vec![near, closer]);

        replica.acknowledge(4);
        let edge = person(2, INTEREST_RADIUS + 10.0, 0.0);
        let third = snapshot(&mut replica, 6, &[near, edge]);
        assert_eq!(rebuild(&mut held, &third).entities, vec![near, edge], "kept inside the hysteresis band");

        replica.acknowledge(6);
        let gone = person(2, INTEREST_RADIUS + INTEREST_HYSTERESIS + 10.0, 0.0);
        let fourth = snapshot(&mut replica, 8, &[near, gone]);
        assert_eq!(fourth.removed, vec![2]);
        assert_eq!(rebuild(&mut held, &fourth).entities, vec![near]);
    }

    #[test]
    fn a_standing_player_costs_nothing_once_acknowledged() {
        let mut replica = Replica::default();
        let people: Vec<EntityState> = (1..=20).map(|id| person(id, id as f32, 0.0)).collect();
        let full = snapshot(&mut replica, 2, &people);
        assert_eq!(full.new_entities.len(), 20);
        replica.acknowledge(2);
        let again = snapshot(&mut replica, 4, &people);
        assert_eq!(again.baseline, 2);
        assert!(again.new_entities.is_empty() && again.entities.is_empty() && again.removed.is_empty());
    }

    #[test]
    fn without_an_acknowledgement_every_snapshot_is_whole() {
        let mut replica = Replica::default();
        let people = [person(1, 1.0, 0.0), person(2, 2.0, 0.0)];
        snapshot(&mut replica, 2, &people);
        let second = snapshot(&mut replica, 4, &people);
        assert_eq!(second.baseline, 0);
        assert_eq!(second.new_entities.len(), 2);
    }

    #[test]
    fn an_acknowledgement_of_something_never_sent_falls_back_to_whole() {
        let mut replica = Replica::default();
        replica.acknowledge(999);
        let snap = snapshot(&mut replica, 2, &[person(1, 1.0, 0.0)]);
        assert_eq!(snap.baseline, 0);
    }

    #[test]
    fn an_old_baseline_falls_out_of_the_window() {
        let mut replica = Replica::default();
        let people = [person(1, 1.0, 0.0)];
        snapshot(&mut replica, 2, &people);
        replica.acknowledge(2);
        for i in 0..(BASELINE_WINDOW as u32 + 2) {
            snapshot(&mut replica, 4 + i * 2, &people);
        }
        let late = snapshot(&mut replica, 1000, &people);
        assert_eq!(late.baseline, 0, "the acknowledged snapshot is long gone: start over");
    }

    #[test]
    fn a_crowded_room_is_trimmed_to_the_packet_nearest_first() {
        let mut replica = Replica::default();
        // Busy ones, far from the origin, so each takes its full width on the wire.
        let people: Vec<EntityState> = (1..=MAX_PLAYERS as u32)
            .map(|id| {
                let mut e = person(id, id as f32 * 3.0, -(id as f32));
                e.yaw = -2000;
                e.vy = -650;
                e.knock_x = 700;
                e.knock_z = -700;
                e
            })
            .collect();
        let first = snapshot(&mut replica, 2, &people);
        assert!(first.new_entities.len() < MAX_PLAYERS, "a packet cannot hold {MAX_PLAYERS} whole entities");
        let ids: Vec<u32> = first.new_entities.iter().map(|e| e.id).collect();
        assert!(ids.contains(&1), "the nearest got in");
        // The rest follow, each acknowledged snapshot bringing the next nearest.
        let mut held = Vec::new();
        let mut scene = rebuild(&mut held, &first);
        let mut tick = 2;
        for _ in 0..8 {
            replica.acknowledge(tick);
            tick += 2;
            let next = snapshot(&mut replica, tick, &people);
            scene = rebuild(&mut held, &next);
        }
        assert_eq!(scene.entities.len(), MAX_PLAYERS);
    }

    #[test]
    fn effects_out_of_range_are_not_sent() {
        let mut replica = Replica::default();
        let near = ShotEvent { shooter: 3, weapon: WEAPON_RIFLE, x: 500, y: 90, z: 0 };
        let far = ShotEvent { shooter: 3, weapon: WEAPON_RIFLE, x: 90_000, y: 90, z: 0 };
        let world = Everything { entities: &[], vehicles: &[], shots: &[near, far], rockets: &[], pickups: &[] };
        let bytes = replica.snapshot([0.0, 0.0], &world, Own { tick: 2, ..Default::default() });
        let snap: Snapshot = decode(&bytes).unwrap();
        assert_eq!(snap.shots, vec![near]);
    }

    /// Thirty players running about, snapshot after acknowledged snapshot.
    #[test]
    fn a_delta_is_a_good_deal_smaller_than_a_whole_snapshot() {
        let mut replica = Replica::default();
        let mut whole = Replica::default();
        let runners = |step: i32| -> Vec<EntityState> {
            (1..=30u32)
                .map(|id| {
                    let phase = id as i32 * 37;
                    // About 7.5 m/s at 30 snapshots a second is 25 cm a snapshot.
                    EntityState { id, kind: KIND_STICKMAN, x: 5_000 + phase * 20 + step * 25, y: 87, z: -3_000 + phase * 10 - step * 9, yaw: ((step + phase) % 6000 - 3000) as i16, vy: 0, flags: FLAG_GROUNDED, knock_x: 0, knock_z: 0 }
                })
                .collect()
        };
        let (mut delta_bytes, mut whole_bytes) = (0, 0);
        for step in 0..60 {
            let tick = 2 + step as u32 * 2;
            let people = runners(step);
            let world = Everything { entities: &people, vehicles: &[], shots: &[], rockets: &[], pickups: &[] };
            let own = || Own { tick, ..Default::default() };
            let a = replica.snapshot([0.0, 0.0], &world, own());
            let b = whole.snapshot([0.0, 0.0], &world, own());
            replica.acknowledge(tick);
            if step >= 1 {
                delta_bytes += a.len();
                whole_bytes += b.len();
            }
        }
        println!("30 runners: {} bytes a snapshot as deltas, {} whole", delta_bytes / 59, whole_bytes / 59);
        assert!(delta_bytes * 10 < whole_bytes * 7, "deltas {delta_bytes} vs whole {whole_bytes}");
    }
}
