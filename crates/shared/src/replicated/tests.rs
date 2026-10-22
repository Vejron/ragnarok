use super::*;
use crate::protocol::{EntityDelta, decode, encode};

fn stickman(x: i32, yaw: i16) -> Replicated {
    Replicated {
        kind: Some(Kind { kind: 0 }),
        place: Some(Place { x, y: 90, z: -400 }),
        heading: Some(Heading { yaw }),
        motion: Some(Motion::default()),
        flags: Some(Flags { flags: 1, stance: 0 }),
        ..Default::default()
    }
}

fn tank(x: i32, turret_yaw: i16) -> Replicated {
    Replicated {
        kind: Some(Kind { kind: 2 }),
        place: Some(Place { x, y: 120, z: 0 }),
        orientation: Some(Orientation { qx: 0, qy: 1200, qz: 0 }),
        steer: Some(Steer::default()),
        seats: Some(Seats { riders: vec![7, 0] }),
        turret: Some(Turret { yaw: turret_yaw, pitch: 40, shots: 3, range: 3000, flags: 0 }),
        health: Some(Health { hp: 250 }),
        ..Default::default()
    }
}

fn round_trip(update: &Update) -> Update {
    decode(&encode(update)).expect("an update decodes")
}

#[test]
fn every_component_has_its_own_bit_in_wire_order() {
    let ids: Vec<u8> = schema().iter().map(|(id, ..)| *id).collect();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "listed in ascending wire-id order: {ids:?}");
    assert!(ids.iter().all(|&id| id < 31), "bit 31 is CLEARED");
}

#[test]
fn a_new_entity_brings_everything_it_has_and_decodes_back() {
    let everything = Replicated {
        place: Some(Place { x: -12_345, y: 678, z: 90_000 }),
        heading: Some(Heading { yaw: -3141 }),
        motion: Some(Motion { vy: -500, knock_x: 120, knock_z: -80 }),
        flags: Some(Flags { flags: 0b1000_0101, stance: 1 }),
        orientation: Some(Orientation { qx: -32767, qy: 5, qz: 32767 }),
        steer: Some(Steer { steer: -127 }),
        turret: Some(Turret { yaw: 100, pitch: -50, shots: 255, range: 65535, flags: 1 }),
        crew: Some(Crew { riders: vec![Rider { yaw: 1, pitch: -2, held: 3 }, Rider::default()] }),
        thrust: Some(Thrust { air: 200, flags: 3 }),
        arms: Some(Arms { shots: 9, heat: 10, rear_heat: 11, bombs: 4, flags: 2 }),
        seats: Some(Seats { riders: vec![1, 0, 70_000] }),
        health: Some(Health { hp: 70 }),
        damage: Some(Damage { parts: [1, 2, 3, 4, 5, 6, 7, 255] }),
        velocity: Some(Velocity { vx: 30_000, vy: -30_000, vz: 0 }),
        kind: Some(Kind { kind: 6 }),
        projectile: Some(Projectile { kind: 2 }),
        pickup: Some(Pickup { item: 7 }),
    };
    let update = everything.update(42, None).expect("a new entity is always sent");
    assert_eq!(update.set(), everything.mask(), "everything it has is set");
    assert_eq!(update.cleared, 0);
    let decoded = round_trip(&update);
    assert_eq!(decoded, update);
    assert_eq!(Replicated::apply(None, &decoded), everything);
}

#[test]
fn nothing_changed_is_no_update() {
    let now = tank(100, 0);
    assert_eq!(now.update(5, Some(&now)), None);
}

#[test]
fn a_walking_stickman_sends_only_its_moves_as_differences_and_costs_no_more_than_before() {
    let (base, now) = (stickman(1000, 200), stickman(1012, 230));
    let update = now.update(3, Some(&base)).unwrap();
    assert_eq!(update.set(), 1 << Place::ID | 1 << Heading::ID);
    assert_eq!(update.place, Some([12, 0, 0]));
    assert_eq!(update.heading, Some([30]));
    assert_eq!(Replicated::apply(Some(&base), &round_trip(&update)), now);
    let before = encode(&EntityDelta { id: 3, dx: 12, dy: 0, dz: 0, dyaw: 30, dvy: 0, flags: 1, stance: 0, dknock_x: 0, dknock_z: 0 });
    let after = encode(&update);
    assert!(after.len() <= before.len(), "{} bytes now, {} before", after.len(), before.len());
}

#[test]
fn a_turret_that_did_not_move_is_not_resent() {
    let update = tank(130, 0).update(9, Some(&tank(100, 0))).unwrap();
    assert!(update.turret.is_none() && update.seats.is_none() && update.health.is_none());
    assert_eq!(update.place, Some([30, 0, 0]));
    let turned = tank(100, 15).update(9, Some(&tank(100, 0))).unwrap();
    assert_eq!(turned.set(), 1 << Turret::ID);
    assert_eq!(turned.turret.unwrap().yaw, 15);
}

#[test]
fn a_repair_clears_damage_and_hurt_brings_it_back() {
    let hurt = Replicated { damage: Some(Damage { parts: [0, 0, 90, 0, 0, 0, 0, 0] }), ..tank(0, 0) };
    let repaired = tank(0, 0);
    let update = repaired.update(9, Some(&hurt)).unwrap();
    assert_eq!(update.cleared, 1 << Damage::ID);
    assert_eq!(update.set(), CLEARED, "nothing set, only the clearing");
    let decoded = round_trip(&update);
    assert_eq!(decoded.cleared, 1 << Damage::ID);
    assert_eq!(Replicated::apply(Some(&hurt), &decoded), repaired);
    let again = hurt.update(9, Some(&repaired)).unwrap();
    assert_eq!(Replicated::apply(Some(&repaired), &round_trip(&again)), hurt);
}

#[test]
fn a_component_the_baseline_lacks_is_sent_whole_even_one_sent_as_differences() {
    let base = Replicated { velocity: None, ..tank(0, 0) };
    let now = Replicated { velocity: Some(Velocity { vx: 500, vy: -20, vz: 3 }), ..tank(0, 0) };
    let update = now.update(1, Some(&base)).unwrap();
    assert_eq!(update.velocity, Some([500, -20, 3]));
    assert_eq!(Replicated::apply(Some(&base), &update), now);
}

#[test]
fn differences_go_through_the_whole_range_of_a_field() {
    let base = Replicated { heading: Some(Heading { yaw: 3141 }), steer: Some(Steer { steer: -127 }), orientation: Some(Orientation { qx: -32767, qy: 0, qz: 32767 }), ..Default::default() };
    let now = Replicated { heading: Some(Heading { yaw: -3142 }), steer: Some(Steer { steer: 127 }), orientation: Some(Orientation { qx: 32767, qy: 0, qz: -32767 }), ..Default::default() };
    let update = now.update(1, Some(&base)).unwrap();
    assert_eq!(update.orientation, Some([65534, 0, -65534]));
    assert_eq!(Replicated::apply(Some(&base), &round_trip(&update)), now);
}

#[test]
fn diff_and_apply_rebuild_the_new_scene() {
    let base: Scene = vec![(1, stickman(0, 0)), (2, stickman(500, 0)), (4, tank(0, 0)), (9, stickman(9, 9))];
    let now: Scene = vec![(1, stickman(0, 0)), (3, stickman(-20, 7)), (4, tank(40, 12)), (9, stickman(10, 9)), (12, tank(1, 1))];
    let (updates, removed) = diff(&base, &now);
    assert_eq!(removed, vec![2]);
    assert_eq!(updates.iter().map(|update| update.id).collect::<Vec<_>>(), vec![3, 4, 9, 12], "the unchanged one is not listed");
    let decoded: Vec<Update> = updates.iter().map(round_trip).collect();
    assert_eq!(apply(&base, &decoded, &removed), now);
    let (all, none) = diff(&[], &now);
    assert!(none.is_empty());
    assert_eq!(apply(&[], &all, &[]), now, "against nothing, everything is new");
}

/// The components as the client decodes them, as JSON. `client/src/net/schema.generated.json` holds
/// the committed copy; `UPDATE_PARITY=1 cargo test -p shared` rewrites it.
fn schema_json() -> String {
    let components: Vec<String> = schema()
        .iter()
        .map(|(id, name, diff, fields)| {
            let fields: Vec<String> = fields.iter().map(|(field, ty)| format!("[\"{field}\", \"{}\"]", ty.replace(' ', ""))).collect();
            format!("    {{\"id\": {id}, \"name\": \"{name}\", \"diff\": {diff}, \"fields\": [{}]}}", fields.join(", "))
        })
        .collect();
    format!("{{\n  \"cleared\": {CLEARED},\n  \"components\": [\n{}\n  ]\n}}\n", components.join(",\n"))
}

#[test]
fn the_client_schema_matches_the_components() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/src/net/schema.generated.json");
    let actual = schema_json();
    if std::env::var_os("UPDATE_PARITY").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    let committed = std::fs::read_to_string(&path).expect("client/src/net/schema.generated.json; regenerate with UPDATE_PARITY=1");
    assert_eq!(committed, actual, "replicated components changed: regenerate client/src/net/schema.generated.json with UPDATE_PARITY=1");
}
