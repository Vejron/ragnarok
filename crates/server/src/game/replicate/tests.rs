use super::*;
use crate::mechanics::for_tests::{Flying, add_vehicle, damage, launch_drone};
use shared::protocol::{KIND_DRONE, KIND_JEEP, PROJECTILE_ROCKET};
use shared::world::{WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

/// Whether the v2 scene says exactly what the v1 rows do, of everything in the room.
fn same_as_v1(game: &Game) -> (usize, usize, usize, usize) {
    let (entities, vehicles, rockets, pickups) = wire::v1_rows(&game.replicated());
    let mut v1_entities = game.entity_states();
    v1_entities.sort_by_key(|e| e.id);
    assert_eq!(entities, v1_entities, "stickmen and creatures, tick {}", game.world.tick);
    assert_eq!(vehicles, game.vehicle_states(), "vehicles, tick {}", game.world.tick);
    assert_eq!(rockets, game.projectile_states(), "projectiles, tick {}", game.world.tick);
    assert_eq!(pickups, game.pickup_states(), "pickups, tick {}", game.world.tick);
    (entities.len(), vehicles.len(), rockets.len(), pickups.len())
}

#[test]
fn the_v2_scene_says_what_the_v1_rows_do_in_a_busy_room() {
    // The committed city if it is there, for vehicles, buildings and spawn points; else open ground.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack");
    let map = crate::read_world(&fixture).map(|(map, _)| map).unwrap_or_else(|_| WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0], [3000, 0]], ..Default::default() });
    let mut game = Game::new(map, 0).unwrap();
    let mut inboxes = Vec::new();
    let mut ids = Vec::new();
    for _ in 0..12 {
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        ids.push(response.try_recv().unwrap().unwrap());
        inboxes.push(inbox);
    }
    let spawn = game.world.map.spawns.first().map_or([0.0, 0.0], |s| [s[0] as f32 / 100.0, s[1] as f32 / 100.0]);
    for n in 0..4 {
        game.add_bot([spawn[0] + n as f32 * 3.0, spawn[1]], TEAM_BOTS, Vec::new());
    }
    game.add_crew(["Driver".into(), "Gunner".into()], spawn, TEAM_BOTS, Vec::new());

    let mut seen = (0, 0, 0, 0);
    let mut most = |counts: (usize, usize, usize, usize)| seen = (seen.0.max(counts.0), seen.1.max(counts.1), seen.2.max(counts.2), seen.3.max(counts.3));
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
                game.world.entities.of_mut::<Vehicle>(jeep).damage = [10, 0, 200, 0, 0, 255, 0, 0];
                crate::mechanics::seat(&mut game.world, ids[2], jeep, 2);
            }
            70 => {
                let other = game.world.entities.ids_where::<Vehicle>(|_, vehicle| !shared::protocol::is_drone(vehicle.kind)).last().copied();
                if let Some(vid) = other {
                    crate::mechanics::damage_vehicle(&mut game.world, vid, 255, Some(ids[3]));
                }
            }
            80 => damage(&mut game.world, ids[4], PLAYER_MAX_HP, Some(ids[5]), shared::protocol::WEAPON_RIFLE),
            _ => {}
        }
        game.step();
        if t % 6 == 0 {
            most(same_as_v1(&game));
            let vehicles = game.vehicle_states();
            drones |= vehicles.iter().any(|v| v.thrust.is_some_and(|t| t.flags & THRUST_LINKED != 0));
            hurt |= vehicles.iter().any(|v| v.damage.is_some());
            wrecks |= vehicles.iter().any(|v| v.hp == 0);
            crews |= vehicles.iter().any(|v| v.crew.iter().any(|rider| rider.held != 0));
            dead |= game.entity_states().iter().any(|e| e.flags & FLAG_DEAD != 0);
        }
    }
    let (entities, vehicles, rockets, _) = seen;
    assert!(entities >= 12 && vehicles >= 2 && rockets >= 1, "the room was busy: {seen:?}");
    assert!(drones && hurt && wrecks && dead && crews, "every case was met: drone {drones}, hurt {hurt}, wreck {wrecks}, dead {dead}, crew {crews}");
    drop(inboxes);
}
