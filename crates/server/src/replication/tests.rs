use super::*;
use shared::replicated::{self as wire, Replicated};

fn person(id: u32, x_m: f32, z_m: f32) -> (u32, Replicated) {
    (
        id,
        Replicated {
            kind: Some(wire::Kind { kind: KIND_STICKMAN }),
            place: Some(wire::Place { x: (x_m * 100.0) as i32, y: 90, z: (z_m * 100.0) as i32 }),
            heading: Some(wire::Heading { yaw: 0 }),
            motion: Some(wire::Motion::default()),
            flags: Some(wire::Flags { flags: FLAG_GROUNDED, stance: 0 }),
            ..Default::default()
        },
    )
}

fn lying(id: u32, x_m: f32) -> (u32, Replicated) {
    (id, Replicated { pickup: Some(wire::Pickup { item: 2 }), place: Some(wire::Place { x: (x_m * 100.0) as i32, y: 5, z: 0 }), ..Default::default() })
}

fn flying(id: u32, x_m: f32) -> (u32, Replicated) {
    (id, Replicated { projectile: Some(wire::Projectile { kind: 0 }), place: Some(wire::Place { x: (x_m * 100.0) as i32, y: 2000, z: -99_999 }), velocity: Some(wire::Velocity { vx: -30_000, vy: 20_000, vz: 30_000 }), ..Default::default() })
}

fn sorted(mut scene: Scene) -> Scene {
    scene.sort_by_key(|(id, _)| *id);
    scene
}

fn snapshot(replica: &mut Replica, tick: u32, scene: &Scene) -> Snapshot {
    let world = Everything { scene, shots: &[], bullets: &[] };
    let bytes = replica.snapshot([0.0, 0.0], &world, Own { tick, ..Default::default() });
    assert!(bytes.len() <= MAX_PACKET);
    decode(&bytes).unwrap()
}

/// What a client rebuilds from `snapshot`, given the scenes it holds.
fn rebuild(held: &mut Vec<(u32, Scene)>, snapshot: &Snapshot) -> Scene {
    let base = held.iter().find(|(tick, _)| *tick == snapshot.baseline).map(|(_, s)| s.clone()).unwrap_or_default();
    let scene = apply(&base, &snapshot.updates, &snapshot.removed);
    held.push((snapshot.tick, scene.clone()));
    scene
}

#[test]
fn only_the_near_are_sent_and_the_far_come_in_as_you_approach() {
    let mut replica = Replica::default();
    let mut held = Vec::new();
    let near = person(1, 10.0, 0.0);
    let far = person(2, INTEREST_RADIUS + 20.0, 0.0);
    let first = snapshot(&mut replica, 2, &vec![near.clone(), far]);
    assert_eq!(rebuild(&mut held, &first), vec![near.clone()]);
    // Moves into range, but stays listed once in: the edge does not flicker.
    replica.acknowledge(2);
    let closer = person(2, INTEREST_RADIUS - 10.0, 0.0);
    let second = snapshot(&mut replica, 4, &vec![near.clone(), closer.clone()]);
    assert_eq!(second.baseline, 2, "diffed against the acknowledged snapshot");
    assert_eq!(rebuild(&mut held, &second), vec![near.clone(), closer]);
    replica.acknowledge(4);
    let edge = person(2, INTEREST_RADIUS + 10.0, 0.0);
    let third = snapshot(&mut replica, 6, &vec![near.clone(), edge.clone()]);
    assert_eq!(rebuild(&mut held, &third), vec![near.clone(), edge], "kept inside the hysteresis band");
    replica.acknowledge(6);
    let gone = person(2, INTEREST_RADIUS + INTEREST_HYSTERESIS + 10.0, 0.0);
    let fourth = snapshot(&mut replica, 8, &vec![near.clone(), gone]);
    assert_eq!(fourth.removed, vec![2]);
    assert_eq!(rebuild(&mut held, &fourth), vec![near]);
}

#[test]
fn a_standing_player_costs_nothing_once_acknowledged() {
    let mut replica = Replica::default();
    let people: Scene = (1..=20).map(|id| person(id, id as f32, 0.0)).collect();
    let full = snapshot(&mut replica, 2, &people);
    assert_eq!(full.updates.len(), 20);
    replica.acknowledge(2);
    let again = snapshot(&mut replica, 4, &people);
    assert_eq!(again.baseline, 2);
    assert!(again.updates.is_empty() && again.removed.is_empty());
}

#[test]
fn without_an_acknowledgement_every_snapshot_is_whole() {
    let mut replica = Replica::default();
    let people = vec![person(1, 1.0, 0.0), person(2, 2.0, 0.0)];
    snapshot(&mut replica, 2, &people);
    let second = snapshot(&mut replica, 4, &people);
    assert_eq!(second.baseline, 0);
    assert_eq!(second.updates.len(), 2);
    assert!(second.updates.iter().all(|update| update.kind.is_some()), "each comes whole");
}

#[test]
fn an_acknowledgement_of_something_never_sent_falls_back_to_whole() {
    let mut replica = Replica::default();
    replica.acknowledge(999);
    let snap = snapshot(&mut replica, 2, &vec![person(1, 1.0, 0.0)]);
    assert_eq!(snap.baseline, 0);
}

#[test]
fn an_old_baseline_falls_out_of_the_window() {
    let mut replica = Replica::default();
    let people = vec![person(1, 1.0, 0.0)];
    snapshot(&mut replica, 2, &people);
    replica.acknowledge(2);
    for i in 0..(BASELINE_WINDOW as u32 + 2) {
        snapshot(&mut replica, 4 + i * 2, &people);
    }
    let late = snapshot(&mut replica, 1000, &people);
    assert_eq!(late.baseline, 0, "the acknowledged snapshot is long gone: start over");
}

/// Busy ones, far from the origin, so each takes its full width on the wire.
fn crowd() -> Scene {
    (1..=MAX_PLAYERS as u32)
        .map(|id| {
            let (id, mut e) = person(id, id as f32 * 3.0, -(id as f32));
            e.heading = Some(wire::Heading { yaw: -2000 });
            e.motion = Some(wire::Motion { vy: -650, knock_x: 700, knock_z: -700 });
            (id, e)
        })
        .collect()
}

#[test]
fn a_crowded_room_is_trimmed_to_the_packet_nearest_first() {
    let mut replica = Replica::default();
    let people = crowd();
    let first = snapshot(&mut replica, 2, &people);
    assert!(first.updates.len() < MAX_PLAYERS, "a packet cannot hold {MAX_PLAYERS} whole entities");
    let ids: Vec<u32> = first.updates.iter().map(|update| update.id).collect();
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
    assert_eq!(scene, people);
}

#[test]
fn when_it_will_not_fit_projectiles_and_items_wait_before_people_do() {
    let mut replica = Replica::default();
    // The rockets and items are nearer than anyone, but worth less.
    let mut scene = crowd();
    scene.extend((200..220).map(|id| flying(id, 0.5)));
    scene.extend((300..320).map(|id| lying(id, 0.5)));
    let scene = sorted(scene);
    let first = snapshot(&mut replica, 2, &scene);
    let kept = |range: std::ops::Range<u32>| first.updates.iter().filter(|update| range.contains(&update.id)).count();
    assert!(kept(1..65) > 0 && kept(200..220) == 0 && kept(300..320) == 0, "people first: {} people, {} rockets, {} items", kept(1..65), kept(200..220), kept(300..320));
}

#[test]
fn only_the_nearest_items_on_the_ground_are_listed() {
    let mut replica = Replica::default();
    let items: Scene = (1..=40).map(|id| lying(id, id as f32)).collect();
    let snap = snapshot(&mut replica, 2, &items);
    let ids: Vec<u32> = snap.updates.iter().map(|update| update.id).collect();
    assert_eq!(ids, (1..=MAX_PICKUPS_PER_SNAPSHOT as u32).collect::<Vec<_>>());
}

#[test]
fn effects_out_of_range_are_not_sent() {
    let mut replica = Replica::default();
    let near = ShotEvent { shooter: 3, weapon: WEAPON_RIFLE, x: 500, y: 90, z: 0 };
    let far = ShotEvent { shooter: 3, weapon: WEAPON_RIFLE, x: 90_000, y: 90, z: 0 };
    let world = Everything { scene: &Vec::new(), shots: &[near, far], bullets: &[] };
    let bytes = replica.snapshot([0.0, 0.0], &world, Own { tick: 2, ..Default::default() });
    let snap: Snapshot = decode(&bytes).unwrap();
    assert_eq!(snap.shots, vec![near]);
}

#[test]
fn a_bullet_goes_to_whoever_its_line_crosses() {
    use shared::items::ITEM_AK5;
    let mut replica = Replica::default();
    // Fired from the origin along -Z, out to the AK5's 200 m.
    let bullet = BulletSpawn { id: 9, shooter: 3, item: ITEM_AK5, x: 0, y: 165, z: 0, vx: 0, vy: 0, vz: -27_900, tick: 10 };
    let nothing = Vec::new();
    let world = Everything { scene: &nothing, shots: &[], bullets: &[bullet] };
    // A player 150 m down its line sees it.
    let down_the_line: Snapshot = decode(&replica.snapshot([0.0, -150.0], &world, Own { tick: 12, ..Default::default() })).unwrap();
    assert_eq!(down_the_line.bullets, vec![bullet]);
    // Beside it, 400 m off to the side, it never comes near.
    let mut replica = Replica::default();
    let aside: Snapshot = decode(&replica.snapshot([400.0, -100.0], &world, Own { tick: 12, ..Default::default() })).unwrap();
    assert!(aside.bullets.is_empty());
    // Behind the muzzle, out of reach.
    let mut replica = Replica::default();
    let behind: Snapshot = decode(&replica.snapshot([0.0, 500.0], &world, Own { tick: 12, ..Default::default() })).unwrap();
    assert!(behind.bullets.is_empty());
}

/// Thirty players running about, snapshot after acknowledged snapshot.
#[test]
fn a_delta_is_a_good_deal_smaller_than_a_whole_snapshot() {
    let mut replica = Replica::default();
    let mut whole = Replica::default();
    let runners = |step: i32| -> Scene {
        (1..=30u32)
            .map(|id| {
                let phase = id as i32 * 37;
                // About 7.5 m/s at 30 snapshots a second is 25 cm a snapshot.
                let (id, mut e) = person(id, 0.0, 0.0);
                e.place = Some(wire::Place { x: 5_000 + phase * 20 + step * 25, y: 87, z: -3_000 + phase * 10 - step * 9 });
                e.heading = Some(wire::Heading { yaw: ((step + phase) % 6000 - 3000) as i16 });
                (id, e)
            })
            .collect()
    };
    let (mut delta_bytes, mut whole_bytes) = (0, 0);
    for step in 0..60 {
        let tick = 2 + step as u32 * 2;
        let people = runners(step);
        let world = Everything { scene: &people, shots: &[], bullets: &[] };
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
    // v1 sent these 30 as about 320 bytes a snapshot (NETWORKING.md).
    assert!(delta_bytes / 59 <= 320, "{} bytes a snapshot", delta_bytes / 59);
}
