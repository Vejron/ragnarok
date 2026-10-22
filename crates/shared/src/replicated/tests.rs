use super::*;
use crate::protocol::{decode, encode};

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
    // v1's `EntityDelta` for the same change was 11 bytes.
    assert_eq!(encode(&update).len(), 7);
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

/// A field's TypeScript type, from its Rust one.
fn ts_type(rust: &str) -> String {
    match rust {
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => "number".into(),
        list if list.starts_with("Vec<") || list.starts_with('[') => {
            let inner = list.trim_start_matches("Vec<").trim_start_matches('[').split([';', '>', ']']).next().unwrap();
            format!("{}[]", ts_type(inner))
        }
        name => {
            assert!(STRUCTS.iter().any(|(known, _)| *known == name), "a field of type {name}, which STRUCTS does not list");
            name.into()
        }
    }
}

/// The client's half of the components: their types and the schema its decoder walks.
/// `client/src/net/components.generated.ts` holds the committed copy; `UPDATE_PARITY=1 cargo test -p
/// shared` rewrites it.
fn components_ts() -> String {
    let mut out = String::from("// GENERATED from crates/shared/src/replicated.rs by its tests. Do not edit: change the Rust and\n// run `UPDATE_PARITY=1 cargo test -p shared`. What each component means is documented there.\n\n");
    out += &format!("/** Bit 31 of an update's `set` mask: a `cleared` mask follows it. */\nexport const CLEARED = {CLEARED}\n\n");
    let units: Vec<String> = UNITS.iter().map(|unit| format!("'{unit}'")).collect();
    out += &format!("/** What a field's integer counts (`UNITS` in the Rust): how the client turns it into its own units, and blends it. */\nexport type Unit = {}\n\n", units.join(" | "));
    for (name, fields) in STRUCTS {
        let fields: Vec<String> = fields.iter().map(|(field, _)| format!("{field}: number")).collect();
        out += &format!("export interface {name} {{\n  {}\n}}\n\n", fields.join("\n  "));
    }
    let schema = schema();
    for (_, _, ty, _, fields) in &schema {
        let fields: Vec<String> = fields.iter().map(|(field, rust, _)| format!("{field}: {}", ts_type(&rust.replace(' ', "")))).collect();
        out += &format!("export interface {ty} {{\n  {}\n}}\n\n", fields.join("\n  "));
    }
    out += "/** An entity as it is replicated: the components it has. */\nexport interface Components {\n";
    for (_, name, ty, ..) in &schema {
        out += &format!("  {name}?: {ty}\n");
    }
    out += "}\n\nexport type ComponentName = keyof Components\n\n";
    out += "/** A field, in wire order: its unit if it has one, and the struct's fields if it holds a list of structs. */\nexport interface FieldSchema {\n  name: string\n  unit?: Unit\n  struct?: readonly FieldSchema[]\n}\n\n";
    out += "/** Every component in wire-id order: its bit, whether it is sent as a difference, and its fields. */\nexport const SCHEMA: readonly { id: number; name: ComponentName; diff: boolean; fields: readonly FieldSchema[] }[] = [\n";
    for (id, name, _, diff, fields) in &schema {
        let fields: Vec<String> = fields
            .iter()
            .map(|(field, rust, unit)| {
                let rust = rust.replace(' ', "");
                match STRUCTS.iter().find(|(known, _)| rust.contains(known)) {
                    Some((_, members)) => format!("{{ name: '{field}', struct: [{}] }}", members.iter().map(|(member, unit)| field_ts(member, unit)).collect::<Vec<_>>().join(", ")),
                    None => field_ts(field, unit),
                }
            })
            .collect();
        out += &format!("  {{ id: {id}, name: '{name}', diff: {diff}, fields: [{}] }},\n", fields.join(", "));
    }
    out += "]\n";
    out
}

/// What one of a unit's integers is worth in the client's units (`UNITS`).
fn scale(unit: &str) -> Option<u32> {
    match unit {
        "cm" => Some(100),
        "mrad" => Some(1000),
        "quat" => Some(32767),
        "axis" => Some(127),
        "byte" => Some(255),
        "dm" => Some(10),
        _ => None,
    }
}

fn struct_of(rust: &str) -> Option<&'static [(&'static str, &'static str)]> {
    let rust = rust.replace(' ', "");
    STRUCTS.iter().find(|(known, _)| rust.contains(known)).map(|(_, members)| *members)
}

/// A field's value `w` (a TypeScript expression) in the client's units.
fn convert_field_ts(w: &str, rust: &str, unit: &str) -> String {
    if let Some(members) = struct_of(rust) {
        let fields: Vec<String> = members.iter().map(|(member, unit)| format!("{member}: {}", convert_field_ts(&format!("r.{member}"), "", unit))).collect();
        return format!("{w}.map((r) => ({{ {} }}))", fields.join(", "));
    }
    let list = rust.starts_with("Vec<") || rust.starts_with('[');
    let one = |v: &str| match (unit, scale(unit)) {
        ("axis", Some(s)) => format!("Math.max(-1, Math.min(1, {v} / {s}))"),
        (_, Some(s)) => format!("{v} / {s}"),
        _ => v.to_string(),
    };
    match (list, scale(unit)) {
        (_, None) => w.to_string(),
        (true, Some(_)) => format!("{w}.map((v) => {})", one("v")),
        (false, Some(_)) => one(w),
    }
}

/// How a field of two snapshots, `a` and `b` (TypeScript expressions), blends a fraction `t` of the way,
/// as its unit says (`UNITS`): positions and steering are lerped, angles turned the short way round, and
/// everything else is the nearer snapshot's.
fn blend_field_ts(a: &str, b: &str, rust: &str, unit: &str) -> String {
    if let Some(members) = struct_of(rust) {
        let fields: Vec<String> = members.iter().map(|(member, unit)| format!("{member}: {}", blend_field_ts(&format!("p.{member}"), &format!("q.{member}"), "", unit))).collect();
        return format!("items({a}, {b}, (p, q) => ({{ {} }}))", fields.join(", "));
    }
    match unit {
        "cm" | "axis" => format!("lerp({a}, {b}, t)"),
        "mrad" => format!("angle({a}, {b}, t)"),
        _ => format!("t < 0.5 ? {a} : {b}"),
    }
}

/// The client's conversion of a replicated entity into its units, and its blend of two snapshots of
/// one. Generated rather than walked from the schema: they run for every entity every snapshot and
/// every frame, and code that builds objects of one fixed shape is several times faster than code that
/// builds them field by field from names. Every entity comes out with every component as a property,
/// `undefined` for one it does not have, so all of them share one shape.
fn units_ts() -> String {
    let mut out = String::from("// GENERATED from crates/shared/src/replicated.rs by its tests. Do not edit: change the Rust and\n// run `UPDATE_PARITY=1 cargo test -p shared`. What each unit means, and how it blends, is documented\n// at `UNITS` there.\n\n");
    out += "import { angle, items, lerp, rotation, rotationOf } from './blending'\nimport type { Components } from '@/net/components.generated'\nimport type { Entity } from './units'\n\n";
    let schema = schema();
    let has_unit = |fields: &[Field]| fields.iter().any(|(_, rust, unit)| !unit.is_empty() || struct_of(rust).is_some_and(|members| members.iter().any(|(_, unit)| !unit.is_empty())));
    out += "/** Entity `w`, as the wire has it, in the game's units. A component with no units is `w`'s own. */\nexport function toGame(w: Components): Entity {\n  return {\n";
    for (_, name, _, _, fields) in &schema {
        let converted = if fields.iter().all(|(_, _, unit)| *unit == "quat") {
            let parts: Vec<String> = fields.iter().map(|(field, _, _)| format!("w.{name}.{field}")).collect();
            format!("rotationOf({})", parts.join(", "))
        } else if !has_unit(fields) {
            out += &format!("    {name}: w.{name},\n");
            continue;
        } else {
            let parts: Vec<String> = fields.iter().map(|(field, rust, unit)| format!("{field}: {}", convert_field_ts(&format!("w.{name}.{field}"), rust, unit))).collect();
            format!("{{ {} }}", parts.join(", "))
        };
        out += &format!("    {name}: w.{name} && {converted},\n");
    }
    out += "  }\n}\n\n";
    out += "/**\n * `a` and `b`, two snapshots of one entity, a fraction `t` of the way from `a` to `b`. A component only\n * one of them has is the nearer snapshot's to have or not.\n */\n";
    out += "export function blend(a: Entity, b: Entity, t: number): Entity {\n  if (a === b) return a\n  const near = t < 0.5 ? a : b\n  return {\n";
    for (_, name, _, _, fields) in &schema {
        let moves = |(_, rust, unit): &&Field| matches!(*unit, "cm" | "axis" | "mrad" | "quat") || struct_of(rust).is_some();
        if !fields.iter().any(|field| moves(&field)) {
            // Nothing in it moves: the nearer snapshot's, whole.
            out += &format!("    {name}: near.{name},\n");
            continue;
        }
        // Unchanged, as a moving entity's other components mostly are, it is the same object: no new one.
        let rotation = fields.iter().all(|(_, _, unit)| *unit == "quat");
        // A rotation's `w` is rebuilt on the client, and is part of what it is.
        let parts = fields.iter().filter(|(_, rust, _)| struct_of(rust).is_none()).map(|(field, _, _)| *field).chain(rotation.then_some("qw"));
        let same: Vec<String> = parts.map(|field| format!("a.{name}.{field} === b.{name}.{field}")).collect();
        let same = if same.len() >= fields.len() { format!("a.{name} === b.{name} || ({})", same.join(" && ")) } else { format!("a.{name} === b.{name}") };
        let both = format!("a.{name} && b.{name} ? ({same} ? b.{name} : ");
        let blended = if rotation {
            format!("rotation(a.{name}, b.{name}, t)")
        } else {
            let parts: Vec<String> = fields.iter().map(|(field, rust, unit)| format!("{field}: {}", blend_field_ts(&format!("a.{name}.{field}"), &format!("b.{name}.{field}"), rust, unit))).collect();
            format!("{{ {} }}", parts.join(", "))
        };
        out += &format!("    {name}: {both}{blended}) : near.{name},\n");
    }
    out += "  }\n}\n";
    out
}

/// One `FieldSchema`, as TypeScript.
fn field_ts(name: &str, unit: &str) -> String {
    if unit.is_empty() { format!("{{ name: '{name}' }}") } else { format!("{{ name: '{name}', unit: '{unit}' }}") }
}

fn scene_json(scene: &[(u32, Replicated)]) -> String {
    format!("[{}]", scene.iter().map(|(id, entity)| format!("[{id}, {}]", entity.json())).collect::<Vec<_>>().join(", "))
}

/// Byte-for-byte cases for the client's decoder: a baseline scene, the encoded updates against it, the
/// ids removed, and the scene they must make.
fn fixture_json() -> String {
    let all = Replicated {
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
    let first: Scene = vec![(1, stickman(1000, 200)), (4, tank(0, 0)), (70_000, all.clone())];
    let hurt = Replicated { damage: Some(Damage { parts: [0, 0, 90, 0, 0, 0, 0, 0] }), ..tank(40, 12) };
    let second: Scene = vec![(1, stickman(1012, 230)), (3, stickman(-20, 7)), (4, hurt), (70_000, Replicated { heading: Some(Heading { yaw: 3141 }), steer: Some(Steer { steer: 127 }), ..all.clone() })];
    let third: Scene = vec![(1, stickman(1012, 230)), (3, Replicated { velocity: Some(Velocity { vx: 9, vy: -9, vz: 1 }), ..stickman(-20, 7) }), (4, tank(40, 12))];
    let steps = [("everything new, against nothing", Vec::new(), first.clone()), ("moves, an arrival and new damage", first, second.clone()), ("a repair, a departure and a component gained whole", second, third)];
    let cases: Vec<String> = steps
        .iter()
        .map(|(name, base, now)| {
            let (updates, removed) = diff(base, now);
            assert_eq!(&apply(base, &updates, &removed), now);
            let bytes: Vec<String> = encode(&updates).iter().map(u8::to_string).collect();
            format!("    {{\"name\": \"{name}\", \"base\": {}, \"bytes\": [{}], \"removed\": {:?}, \"expected\": {}}}", scene_json(base), bytes.join(", "), removed, scene_json(now))
        })
        .collect();
    format!("{{\n  \"cases\": [\n{}\n  ]\n}}\n", cases.join(",\n"))
}

/// Writes `contents` to `path` (from the repository root) when `UPDATE_PARITY` is set, then fails while
/// the committed copy differs.
fn generated(path: &str, contents: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
    if std::env::var_os("UPDATE_PARITY").is_some() {
        std::fs::write(&path, contents).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(committed, contents, "{} is stale: regenerate it with UPDATE_PARITY=1 cargo test -p shared", path.display());
}

/// A scene with every family and the edge cases of each, for the client's conversion into its units.
fn scene_fixture_json() -> String {
    let vehicle = |kind: u8| Replicated { kind: Some(Kind { kind }), place: Some(Place { x: 500, y: 80, z: -900 }), orientation: Some(Orientation { qx: 10, qy: -2000, qz: 30 }), steer: Some(Steer { steer: 40 }), seats: Some(Seats { riders: vec![0, 0] }), health: Some(Health { hp: 100 }), ..Default::default() };
    let scene: Scene = vec![
        // Prone, knocked, crouching, holding a gun (flags carry the held item).
        (1, Replicated { motion: Some(Motion { vy: -320, knock_x: 150, knock_z: -40 }), flags: Some(Flags { flags: 0b0001_0101, stance: 1 }), ..stickman(-700, -1500) }),
        // A rooster: no motion, crowing.
        (2, Replicated { kind: Some(Kind { kind: 5 }), place: Some(Place { x: 10, y: 0, z: 20 }), heading: Some(Heading { yaw: 900 }), flags: Some(Flags { flags: 0b1000_0001, stance: 0 }), ..Default::default() }),
        (4, tank(300, -700)),
        // A field mortar, hitched: its turret stowed, its crew showing and empty.
        (5, Replicated { turret: Some(Turret { yaw: 0, pitch: 600, shots: 0, range: 1500, flags: 1 }), crew: Some(Crew { riders: vec![Rider::default()] }), seats: Some(Seats { riders: vec![0] }), ..vehicle(9) }),
        // A plane: crew showing, arms, hurt.
        (6, Replicated { turret: Some(Turret { yaw: 3000, pitch: -100, shots: 17, range: 0, flags: 0 }), crew: Some(Crew { riders: vec![Rider { yaw: 100, pitch: 50, held: 0 }, Rider { yaw: -3000, pitch: -40, held: 2 }] }), arms: Some(Arms { shots: 200, heat: 128, rear_heat: 3, bombs: 4, flags: 2 }), damage: Some(Damage { parts: [0, 9, 0, 0, 0, 0, 0, 0] }), seats: Some(Seats { riders: vec![1, 0] }), ..vehicle(6) }),
        // A drone, linked, its battery as air; a wreck.
        (7, Replicated { thrust: Some(Thrust { air: 99, flags: 4 }), seats: Some(Seats { riders: vec![1] }), ..vehicle(11) }),
        (8, Replicated { health: Some(Health { hp: 0 }), ..vehicle(1) }),
        (9, Replicated { projectile: Some(Projectile { kind: 0 }), place: Some(Place { x: 1, y: 2000, z: 3 }), velocity: Some(Velocity { vx: -4000, vy: 50, vz: 0 }), ..Default::default() }),
        (10, Replicated { pickup: Some(Pickup { item: 7 }), place: Some(Place { x: -5, y: 30, z: 5 }), ..Default::default() }),
    ];
    format!("{{\n  \"scene\": {}\n}}\n", scene_json(&scene))
}

#[test]
fn the_clients_components_and_fixture_are_generated_from_these() {
    generated("client/src/net/components.generated.ts", &components_ts());
    generated("client/src/entities/units.generated.ts", &units_ts());
    generated("client/src/__tests__/replicated.fixture.json", &fixture_json());
    generated("client/src/__tests__/scene.fixture.json", &scene_fixture_json());
}

#[test]
fn a_struct_in_a_list_goes_on_the_wire_as_its_fields_in_order() {
    let bytes = encode(&Rider { yaw: 1, pitch: 2, held: 3 });
    assert_eq!(decode::<(i16, i16, u8)>(&bytes), Some((1, 2, 3)), "STRUCTS says Rider is [yaw, pitch, held]");
}

#[test]
fn every_field_with_a_unit_names_a_known_one() {
    let fields = schema().into_iter().flat_map(|(_, component, _, _, fields)| fields.iter().map(move |(field, _, unit)| (component, *field, *unit)));
    let members = STRUCTS.iter().flat_map(|(name, members)| members.iter().map(move |(member, unit)| (*name, *member, *unit)));
    for (owner, field, unit) in fields.chain(members) {
        assert!(unit.is_empty() || UNITS.contains(&unit), "{owner}.{field} counts in {unit}, which UNITS does not list");
    }
}
