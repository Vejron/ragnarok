use super::*;
use crate::mechanics::for_tests::{Flying, Harm, add_vehicle, damage_entity, launch_drone};
use shared::protocol::{KIND_DRONE, KIND_JEEP, PROJECTILE_ROCKET};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

#[test]
fn a_client_rebuilds_exactly_what_the_server_has_near_it_in_a_busy_room() {
    // The committed city if it is there, for vehicles, buildings and spawn points; else open ground.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack");
    let map = crate::read_world(&fixture).map(|(map, _)| map).unwrap_or_else(|_| WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0], [3000, 0]], ..Default::default() });
    let mut game = Game::new(map, 0).unwrap();
    let mut inboxes = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..13 {
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        ids.push(response.try_recv().unwrap().unwrap());
        inboxes.push(inbox);
    }
    // The last to join only watches, and acknowledges what it rebuilt as a client does.
    let observer = ids.pop().unwrap();
    let mut watching = inboxes.pop().unwrap();
    let mut held: Vec<(u32, Scene)> = Vec::new();
    let mut rebuilt = 0u32;
    let (mut deltas, mut dropped) = (0, 0);
    let spawn = game.world.map.spawns.first().map_or([0.0, 0.0], |s| [s[0] as f32 / 100.0, s[1] as f32 / 100.0]);
    for n in 0..4 {
        game.add_bot([spawn[0] + n as f32 * 3.0, spawn[1]], TEAM_BOTS, Vec::new());
    }
    game.add_crew(["Driver".into(), "Gunner".into()], spawn, TEAM_BOTS, Vec::new());

    let mut seen = (0, 0, 0, 0);
    let (mut drones, mut hurt, mut wrecks, mut dead, mut crews) = (false, false, false, false, false);
    for t in 1..=360u32 {
        for (n, &id) in ids.iter().enumerate() {
            // Different items in hand, fired now and then, some crouching.
            let slot = (n % 3) as u8;
            let fire = if (t as usize + n * 7).is_multiple_of(40) { BUTTON_FIRE } else { 0 };
            let crouch = if n % 4 == 1 { BUTTON_CROUCH } else { 0 };
            let frame = InputFrame { seq: t, strafe: if n % 2 == 0 { 90 } else { -90 }, forward: 127, yaw: (t as i16 * 9 + n as i16 * 300) % 3000, buttons: fire | crouch, pitch: 100, view: t as u16, slot };
            game.handle(Command::Input(id, t.saturating_sub(2), vec![frame]));
        }
        game.handle(Command::Input(observer, rebuilt, vec![InputFrame { seq: t, ..Default::default() }]));
        match t {
            // A drone in the air, a rocket in flight, a hurt car with someone in the back, a wreck, a death.
            30 => assert!(launch_drone(&mut game.world, ids[0], KIND_DRONE), "a drone goes up"),
            40 => {
                let at = game.world.entities.of::<Player>(ids[1]).character.position;
                game.world.launch(Flying { kind: PROJECTILE_ROCKET, owner: ids[1], source: 0, position: [at[0], at[1] + 2.0, at[2]], velocity: [0.0, 0.0, -20.0], ttl: 200, item: 0, back: 0, flown: 0.0 });
            }
            50 => {
                let jeep = game.world.entities.ids_where::<Vehicle>(|_, vehicle| vehicle.kind == KIND_JEEP).first().copied().or_else(|| {
                    let at = game.world.entities.of::<Player>(ids[2]).character.position;
                    Some(add_vehicle(&mut game.world, KIND_JEEP, [at[0] + 6.0, at[1] + 1.0, at[2]], 0.0))
                });
                let jeep = jeep.unwrap();
                game.world.entities.of_mut::<Vehicle>(jeep).damage = [10, 0, 200, 0, 0, 255, 0, 0, 0, 0, 0];
                crate::mechanics::seat(&mut game.world, ids[2], jeep, 2);
            }
            70 => {
                let other = game.world.entities.ids_where::<Vehicle>(|_, vehicle| !shared::protocol::is_drone(vehicle.kind)).last().copied();
                if let Some(vid) = other {
                    crate::mechanics::damage_vehicle(&mut game.world, vid, 255, Some(ids[3]));
                }
            }
            80 => {
                damage_entity(&mut game.world, ids[4], Harm::new(PLAYER_MAX_HP, Some(ids[5]), shared::protocol::WEAPON_RIFLE));
            }
            _ => {}
        }
        game.step();
        while let Ok(out) = watching.try_recv() {
            let Outgoing::Datagram(bytes) = out else { continue };
            assert!(bytes.len() <= MAX_PACKET);
            let snapshot: Snapshot = decode(&bytes).expect("a snapshot decodes");
            let Some(base) = (snapshot.baseline == 0).then(Vec::new).or_else(|| held.iter().find(|(tick, _)| *tick == snapshot.baseline).map(|(_, scene)| scene.clone())) else {
                dropped += 1;
                continue;
            };
            deltas += usize::from(snapshot.baseline != 0);
            held.push((snapshot.tick, wire::apply(&base, &snapshot.updates, &snapshot.removed)));
            rebuilt = snapshot.tick;
        }
        if t >= 90 && t % SNAPSHOT_EVERY == 0 {
            // What a client holds now is exactly what the server has near it.
            let client = &held.last().expect("snapshots arrive").1;
            // As the observer is told it: all of what they own, only the public rest of everything else.
            let owned = game.owned(observer);
            let server: Vec<(u32, Replicated)> = game.replicated().into_iter().map(|(id, e)| (id, if owned.contains(&Some(id)) { e } else { e.public() })).collect();
            let at = game.world.entities.of::<Player>(observer).character.position;
            let near = |e: &Replicated| e.place.is_some_and(|p| ((p.x as f32 / 100.0 - at[0]).powi(2) + (p.z as f32 / 100.0 - at[2]).powi(2)).sqrt() <= INTEREST_RADIUS);
            for (id, entity) in server.iter().filter(|(_, e)| near(e) && e.pickup.is_none()) {
                let mine = client.iter().find(|(held, _)| held == id).map(|(_, e)| e);
                assert_eq!(mine, Some(entity), "entity {id} near the client, tick {t}");
            }
            for (id, entity) in client {
                assert_eq!(server.iter().find(|(had, _)| had == id).map(|(_, e)| e), Some(entity), "the client holds {id} as the server has it, tick {t}");
            }
            let count = |which: fn(&Replicated) -> bool| client.iter().filter(|(_, e)| which(e)).count();
            let entities = count(|e| e.orientation.is_none() && e.projectile.is_none() && e.pickup.is_none());
            let (vehicles, rockets, pickups) = (count(|e| e.orientation.is_some()), count(|e| e.projectile.is_some()), count(|e| e.pickup.is_some()));
            seen = (seen.0.max(entities), seen.1.max(vehicles), seen.2.max(rockets), seen.3.max(pickups));
        }
        if t % 6 == 0 {
            let vehicles = game.seen_vehicles();
            drones |= vehicles.iter().any(|(_, v)| v.thrust.as_ref().is_some_and(|t| t.flags & THRUST_LINKED != 0));
            hurt |= vehicles.iter().any(|(_, v)| v.damage.is_some());
            wrecks |= vehicles.iter().any(|(_, v)| v.health.as_ref().is_some_and(|h| h.hp == 0));
            crews |= vehicles.iter().any(|(vid, _)| game.seen_riders(*vid).iter().any(|(_, rider)| rider.look.is_some() && rider.flags.as_ref().is_some_and(|f| f.flags & FLAG_HELD_MASK != 0)));
            dead |= game.seen_people(KIND_SOLDIER).iter().any(|(_, e)| e.flags.as_ref().is_some_and(|f| f.flags & FLAG_DEAD != 0));
        }
    }
    let (entities, vehicles, rockets, _) = seen;
    assert!(entities >= 12 && vehicles >= 2 && rockets >= 1, "the client saw a busy room: {seen:?}");
    assert!(deltas > 150 && dropped == 0, "{deltas} snapshots came as deltas on what the client acknowledged, {dropped} named a baseline it lacked");
    assert!(drones && hurt && wrecks && dead && crews, "every case was met: drone {drones}, hurt {hurt}, wreck {wrecks}, dead {dead}, crew {crews}");
    drop(inboxes);
}

#[test]
fn what_a_player_owns_is_told_to_them_alone_and_wherever_they_sit() {
    let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0], [3000, 0]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    let mut join = || {
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        (response.try_recv().unwrap().unwrap(), inbox)
    };
    let ((driver, mut theirs), (other, mut others)) = (join(), join());
    let at = game.world.entities.of::<Player>(driver).character.position;
    let jeep = add_vehicle(&mut game.world, KIND_JEEP, [at[0] + 4.0, at[1] + 1.0, at[2]], 0.0);
    crate::mechanics::seat(&mut game.world, driver, jeep, 0);
    for _ in 0..SNAPSHOT_EVERY * 3 {
        game.step();
    }
    let snapshots = |inbox: &mut mpsc::Receiver<Outgoing>| -> Vec<Snapshot> {
        std::iter::from_fn(|| inbox.try_recv().ok()).filter_map(|out| if let Outgoing::Datagram(bytes) = out { decode(&bytes) } else { None }).collect()
    };
    let (theirs, others) = (snapshots(&mut theirs), snapshots(&mut others));
    let last = theirs.last().expect("the driver is sent snapshots");
    let me = last.updates.iter().find(|update| update.id == driver).expect("seated, the driver is still told of themself");
    assert_eq!(me.vitals.as_ref().map(|vitals| vitals.hp), Some(PLAYER_MAX_HP));
    assert!(me.hotbar.is_some() && me.place.is_none(), "their health and hotbar, and no body: they sit in the jeep");
    assert!(last.updates.iter().any(|update| update.id == jeep && update.drive.is_some()), "the jeep's motion, to its driver");
    for update in others.iter().flat_map(|snapshot| &snapshot.updates) {
        let private = update.vitals.is_some() || update.hotbar.is_some() || update.drive.is_some();
        assert!(!private || update.id == other, "{} is told only what is theirs: {update:?}", other);
    }
    assert!(others.iter().flat_map(|snapshot| &snapshot.updates).any(|update| update.id == jeep), "the jeep itself is public");
}
