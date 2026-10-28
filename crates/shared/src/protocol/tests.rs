use super::*;
use crate::consts::*;
use crate::items::*;
use crate::replicated::tests::generated;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

fn sample_input() -> ClientInput {
    ClientInput {
        tag: C_INPUT,
        version: PROTOCOL_VERSION,
        baseline: 1230,
        frames: vec![
            InputFrame { seq: 41, strafe: 127, forward: 0, yaw: 1571, buttons: 0, pitch: 300, view: 61_000, slot: 0 },
            InputFrame { seq: 42, strafe: -127, forward: 127, yaw: -1950, buttons: BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, pitch: -150, view: 7, slot: 3 },
        ],
    }
}

fn sample_snapshot() -> Snapshot {
    use crate::replicated::{self as wire, Replicated};
    let walker = |x, yaw, vy| Replicated {
        kind: Some(wire::Kind { kind: KIND_STICKMAN }),
        place: Some(wire::Place { x, y: 85, z: 100_000 }),
        heading: Some(wire::Heading { yaw }),
        motion: Some(wire::Motion { vy, knock_x: 0, knock_z: 0 }),
        flags: Some(wire::Flags { flags: FLAG_GROUNDED, stance: 0 }),
        ..Default::default()
    };
    let tank = Replicated {
        kind: Some(wire::Kind { kind: KIND_TANK }),
        place: Some(wire::Place { x: 300, y: 90, z: -2000 }),
        orientation: Some(wire::Orientation { qx: 0, qy: 700, qz: 0 }),
        steer: Some(wire::Steer { steer: 32 }),
        turret: Some(wire::Turret { yaw: -700, pitch: 250, shots: 3, range: 1234, flags: TURRET_STOWED }),
        health: Some(wire::Health { hp: 250 }),
        ..Default::default()
    };
    let hurt = Replicated { damage: Some(wire::Damage { parts: [0, 0, 40, 0, 0, 0, 255, 3, 0, 0, 0] }), ..tank.clone() };
    let updates = vec![
        // A stickman moving: differences. A dead one new to the receiver, whole. A tank, whole, and one repaired.
        // The receiver themself, with what only they are told, and the tank they drive.
        Replicated { vitals: Some(wire::Vitals { hp: 73, progress: 128, spent: 0, winded: 0 }), hotbar: Some(wire::Hotbar { slots: [ItemSlot { item: ITEM_AK5, count: 1, ammo: 17 }, ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 }, ItemSlot { item: ITEM_GRENADE, count: 2, ammo: 0 }, ItemSlot::default(), ItemSlot::default()] }), ..walker(25, -120, 650) }.update(1, Some(&walker(0, 0, 0))).unwrap(),
        Replicated { flags: Some(wire::Flags { flags: FLAG_GROUNDED | FLAG_DEAD, stance: 0 }), ..walker(0, 0, 0) }.update(7, None).unwrap(),
        Replicated { drive: Some(wire::Drive { vx: 120, vy: -5, vz: -2500, wx: 30, wy: -400, wz: 15, steer: 250, spin: -600 }), ..tank.clone() }.update(10, None).unwrap(),
        tank.update(12, Some(&hurt)).unwrap(),
        // Someone sitting in the tank they just got into: where they look, and no place.
        Replicated {
            kind: Some(wire::Kind { kind: KIND_STICKMAN }),
            seated: Some(wire::Seated { vehicle: 12, seat: 1 }),
            heading: Some(wire::Heading { yaw: 900 }),
            look: Some(wire::Look { pitch: -60 }),
            flags: Some(wire::Flags { flags: FLAG_CROUCHING, stance: 0 }),
            ..Default::default()
        }
        .update(13, Some(&walker(0, 0, 0)))
        .unwrap(),
    ];
    Snapshot {
        tag: S_SNAPSHOT,
        tick: 1234,
        baseline: 1230,
        ack: 42,
        ride: 9,
        updates,
        removed: vec![3, 11],
        shots: vec![ShotEvent { shooter: 7, weapon: WEAPON_RIFLE, x: 1500, y: 160, z: -4200 }, ShotEvent { shooter: 1, weapon: WEAPON_RIFLE, x: -20, y: 90, z: 31_000 }],
        bullets: vec![BulletSpawn { id: 77, shooter: 7, item: ITEM_AK5, x: 120, y: 165, z: -3000, vx: 1500, vy: -20, vz: -27000, tick: 1233 }],
    }
}

/// `message` as Rust encodes it, after checking that it decodes to itself and that its tag can be read.
fn case<T: Serialize + DeserializeOwned + PartialEq + Debug>(name: &'static str, tag: u8, message: &T) -> (&'static str, Vec<u8>) {
    let bytes = encode(message);
    assert_eq!(decode::<T>(&bytes).as_ref(), Some(message), "{name} round-trips");
    assert_eq!(peek_tag(&bytes), Some(tag), "{name}'s tag");
    (name, bytes)
}

/// One of every message, as Rust encodes it: the client decodes these, and encodes its own to the same
/// bytes (`client/src/__tests__/codec.spec.ts`).
fn samples() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        case("input", C_INPUT, &sample_input()),
        case("ping", C_PING, &Ping { tag: C_PING, nonce: 5 }),
        case("rtcOut", C_RTC, &Rtc { tag: C_RTC, peer: 5, kind: RTC_ICE, json: "x".into() }),
        case("welcome", S_WELCOME, &Welcome { tag: S_WELCOME, version: PROTOCOL_VERSION, id: 70_000, tick_rate: 60, snapshot_every: 2, world_hash: 0xdead_beef }),
        case("snapshot", S_SNAPSHOT, &sample_snapshot()),
        case("pong", S_PONG, &Pong { tag: S_PONG, nonce: 5 }),
        case("leave", S_LEAVE, &Leave { tag: S_LEAVE, id: 9 }),
        case("kill", S_KILL, &Kill { tag: S_KILL, killer: 7, victim: 3, weapon: WEAPON_ROADKILL, killer_kills: 5, victim_deaths: 2, style: DEATH_ROADKILL, yaw: -1571, power: 100 }),
        case("scores", S_SCORES, &Scores { tag: S_SCORES, scores: vec![Score { id: 1, kills: 2, deaths: 3, held: 0 }, Score { id: 4, kills: 0, deaths: 900, held: 754 }] }),
        case("profiles", S_PROFILES, &Profiles { tag: S_PROFILES, players: vec![ProfileEntry { id: 7, name: "Åsa".into(), color: 3, team: 2, bot: true }, ProfileEntry { id: 8, name: String::new(), color: 0, team: 0, bot: false }] }),
        case("backpack", S_BACKPACK, &Backpack { tag: S_BACKPACK, x: -12_345, z: 67_890, radius: 7_500, carrier: 9, held: 301 }),
        case("notice", S_NOTICE, &Notice { tag: S_NOTICE, kind: NOTICE_OBJECTIVE, text: "Go".into() }),
        case("destroyed", S_DESTROYED, &Destroyed { tag: S_DESTROYED, objects: vec![DestroyedObject { kind: OBJ_TREE, col: 2, row: 3, index: 70_000, yaw: -1571, force: 51 }] }),
        case("damaged", S_DAMAGED, &Damaged { tag: S_DAMAGED, buildings: vec![DamagedBuilding { col: 2, row: 3, index: 70_000, stage: 3, mark: Some([1000, 20, -1000]) }, DamagedBuilding { col: 0, row: 1, index: 4, stage: 1, mark: None }] }),
        case("carved", S_CARVED, &Carved { tag: S_CARVED, live: true, buildings: vec![CarvedBuilding { col: 2, row: 3, index: 70_000, point: [1000, 20, -1000], blasted: vec![5, 300], fell: vec![9] }] }),
        case("built", S_BUILT, &Built { tag: S_BUILT, live: true, placed: vec![BuiltCube { x: -3, y: 40, z: 70_000, color: 11, material: 0 }], removed: vec![CubePos { x: 1, y: 2, z: -3 }], fell: vec![CubePos { x: 0, y: 41, z: 0 }, CubePos { x: -1, y: 41, z: 0 }] }),
        case("cratered", S_CRATERED, &Cratered { tag: S_CRATERED, live: false, craters: vec![CraterEntry { x: -70_000, z: 1200, radius: 560, depth: 100, age: 1799 }] }),
        case("dug", S_DUG, &Dug { tag: S_DUG, tiles: vec![DugTile { col: 2, row: 3, runs: vec![40, 3, -12, -300, -1, 70_000, 1, -5] }] }),
        case("hit", S_HIT, &Hit { tag: S_HIT, role: HIT_TAKEN, kind: HIT_BODY, amount: 25, weapon: WEAPON_RIFLE, bearing: Some(-1571), x: 100, y: 90, z: -250, target: 7 }),
        case("hitNowhere", S_HIT, &Hit { tag: S_HIT, role: HIT_DEALT, kind: HIT_KILL, amount: 100, weapon: WEAPON_SNIPER, bearing: None, x: 0, y: 0, z: 0, target: 0 }),
        case("rtc", S_RTC, &Rtc { tag: S_RTC, peer: 5, kind: RTC_ICE, json: "x".into() }),
    ]
}

#[test]
fn a_snapshot_sent_borrowed_is_the_same_bytes() {
    let snapshot = sample_snapshot();
    let Snapshot { tag, tick, baseline, ack, ride, ref updates, ref removed, ref shots, ref bullets } = snapshot;
    let out = SnapshotOut { tag, tick, baseline, ack, ride, updates, removed, shots, bullets };
    assert_eq!(encode(&out), encode(&snapshot));
}

#[test]
fn every_message_has_one_tag_of_its_own() {
    let mut tags: Vec<&str> = MESSAGES.iter().flat_map(|message| message.tags.iter().map(|(name, _)| *name)).collect();
    let count = tags.len();
    tags.sort();
    tags.dedup();
    assert_eq!(tags.len(), count);
    for direction in ["C_", "S_"] {
        let mut values: Vec<u8> = MESSAGES.iter().flat_map(|message| message.tags.iter()).filter(|(name, _)| name.starts_with(direction)).map(|(_, tag)| *tag).collect();
        let count = values.len();
        values.sort();
        values.dedup();
        assert_eq!(values.len(), count, "two {direction} messages share a tag");
    }
}

#[test]
fn peek_rejects_garbage() {
    assert_eq!(peek_tag(&[]), None);
    assert_eq!(peek_tag(&[0x90]), None);
    assert_eq!(peek_tag(&[0x92, 0xcc, 0x01]), None);
}

/// A field's type, as the generator reads its Rust one.
#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Int,
    Bool,
    Text,
    /// A snapshot's entity updates, which `replicated.ts` decodes.
    Updates,
    List(Box<Ty>),
    Array(Box<Ty>, usize),
    Optional(Box<Ty>),
    Struct(String),
}

fn parse(rust: &str) -> Ty {
    let rust = rust.replace(' ', "");
    let inner = |prefix: &str| rust.strip_prefix(prefix).and_then(|rest| rest.strip_suffix('>')).map(parse);
    if rust == "Vec<Update>" {
        return Ty::Updates;
    }
    if let Some(item) = inner("Vec<") {
        return Ty::List(Box::new(item));
    }
    if let Some(item) = inner("Option<") {
        return Ty::Optional(Box::new(item));
    }
    if let Some(array) = rust.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
        let (item, length) = array.split_once(';').expect("an array has a length");
        return Ty::Array(Box::new(parse(item)), length.parse().expect("a literal length"));
    }
    match rust.as_str() {
        "i8" | "i16" | "i32" | "u8" | "u16" | "u32" => Ty::Int,
        "bool" => Ty::Bool,
        "String" => Ty::Text,
        name => {
            assert!(MESSAGES.iter().any(|message| message.name == name && message.tags.is_empty()), "a field of type {name}, which messages! does not list as a struct");
            Ty::Struct(name.into())
        }
    }
}

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map(|first| first.to_lowercase().chain(chars).collect()).unwrap_or_default()
}

/// What one of a unit's integers is worth in the client's units (`UNITS`).
fn scale(unit: &str) -> u32 {
    match unit {
        "cm" => 100,
        "mrad" => 1000,
        "byte" => 255,
        "fifth" => 5,
        unit => panic!("unit {unit}, which UNITS does not list"),
    }
}

fn ts_type(ty: &Ty) -> String {
    match ty {
        Ty::Int => "number".into(),
        Ty::Bool => "boolean".into(),
        Ty::Text => "string".into(),
        Ty::Updates => "Update[]".into(),
        Ty::List(item) => format!("{}[]", ts_type(item)),
        Ty::Array(item, length) => format!("[{}]", vec![ts_type(item); *length].join(", ")),
        Ty::Optional(item) => format!("{} | null", ts_type(item)),
        Ty::Struct(name) => name.clone(),
    }
}

/// An expression decoding raw value `e`: the value, or `undefined` if it is malformed.
fn decode_ts(e: &str, ty: &Ty) -> String {
    match ty {
        Ty::Int => format!("w.int({e})"),
        Ty::Bool => format!("w.bool({e})"),
        Ty::Text => format!("w.text({e})"),
        Ty::Updates => format!("w.updates({e})"),
        Ty::List(item) if **item == Ty::Int => format!("w.ints({e})"),
        Ty::Array(item, length) if **item == Ty::Int => format!("w.ints({e}, {length}) as {} | undefined", ts_type(ty)),
        Ty::List(item) => match &**item {
            Ty::Struct(name) => format!("w.list({e}, decode{name})"),
            item => panic!("a list of {item:?}"),
        },
        Ty::Optional(item) => format!("w.optional({e}, (v) => {})", decode_ts("v", item)),
        Ty::Struct(name) => format!("decode{name}({e})"),
        ty => panic!("a field of {ty:?}"),
    }
}

/// Decoded value `e` in the client's units.
fn convert_ts(e: &str, ty: &Ty, unit: &str) -> String {
    if unit.is_empty() {
        return e.into();
    }
    let s = scale(unit);
    match ty {
        Ty::Int => format!("{e} / {s}"),
        Ty::List(item) if **item == Ty::Int => format!("{e}.map((v) => v / {s})"),
        Ty::Array(item, _) if **item == Ty::Int => format!("{e}.map((v) => v / {s}) as {}", ts_type(ty)),
        Ty::Optional(item) => format!("{e} === null ? null : {}", convert_ts(e, item, unit)),
        ty => panic!("a unit on a field of {ty:?}"),
    }
}

/// What the client writes for value `e`, a field of a message it sends.
fn write_ts(e: &str, ty: &Ty, unit: &str) -> String {
    assert!(unit.is_empty(), "the client sends its values already quantized");
    match ty {
        Ty::Struct(name) => format!("write{name}({e})"),
        Ty::List(item) => match &**item {
            Ty::Struct(name) => format!("{e}.map(write{name})"),
            _ => e.into(),
        },
        _ => e.into(),
    }
}

fn message(name: &str) -> &'static Message {
    MESSAGES.iter().find(|message| message.name == name).unwrap_or_else(|| panic!("no message {name}"))
}

fn sent_by(message: &Message, direction: &str) -> bool {
    message.tags.iter().any(|(tag, _)| tag.starts_with(direction))
}

/// The structs a message of `direction` holds, directly or in theirs.
fn reached(direction: &str) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    let mut todo: Vec<&'static Message> = MESSAGES.iter().filter(|message| sent_by(message, direction)).collect();
    while let Some(next) = todo.pop() {
        for (_, rust, _) in next.fields {
            let mut ty = parse(rust);
            while let Ty::List(item) | Ty::Array(item, _) | Ty::Optional(item) = ty {
                ty = *item;
            }
            if let Ty::Struct(name) = ty {
                let held = message(&name);
                if !found.contains(&held.name) {
                    found.push(held.name);
                    todo.push(held);
                }
            }
        }
    }
    found
}

/// The client's codec: every message's type, a decoder for each the server sends and a writer for each
/// the client sends. Generated as straight-line code, like `units.generated.ts`, since snapshots are
/// decoded 30 times a second.
fn messages_ts() -> String {
    let mut out = String::from("// GENERATED from crates/shared/src/protocol.rs by its tests. Do not edit: change the Rust and\n// run `UPDATE_PARITY=1 cargo test -p shared`. What each message means is documented there.\n\n");
    out += "import type { Update } from './replicated'\nimport * as w from './wire'\n\n";
    let mut tags: Vec<(&str, u8)> = MESSAGES.iter().flat_map(|message| message.tags.iter().copied()).collect();
    tags.sort_by_key(|(name, tag)| (!name.starts_with("C_"), *tag));
    for (name, tag) in tags {
        out += &format!("export const {name} = {tag}\n");
    }
    out += "\n";
    let server: Vec<&Message> = MESSAGES.iter().filter(|message| sent_by(message, "S_")).collect();
    for message in MESSAGES {
        let mut fields: Vec<String> = message.fields.iter().map(|(field, rust, _)| format!("{}: {}", camel(field), ts_type(&parse(rust)))).collect();
        if sent_by(message, "S_") {
            fields.insert(0, format!("type: '{}'", lower_first(message.name)));
        }
        out += &format!("export interface {} {{\n  {}\n}}\n\n", message.name, fields.join("\n  "));
    }
    out += &format!("export type ServerMessage = {}\n\n", server.iter().map(|message| message.name).collect::<Vec<_>>().join(" | "));

    let decoder = |message: &Message, offset: usize| {
        let names: Vec<String> = message.fields.iter().map(|(field, _, _)| camel(field)).collect();
        assert!(!names.iter().any(|name| name == "r" || name == "w" || name == "type"), "{}: a field named r, w or type", message.name);
        let mut body = String::new();
        for (at, ((_, rust, _), name)) in message.fields.iter().zip(&names).enumerate() {
            body += &format!("  const {name} = {}\n", decode_ts(&format!("r[{}]", offset + at), &parse(rust)));
        }
        body += &format!("  if ({}) return undefined\n", names.iter().map(|name| format!("{name} === undefined")).collect::<Vec<_>>().join(" || "));
        let mut values: Vec<String> = message.fields.iter().zip(&names).map(|((_, rust, unit), name)| {
            let value = convert_ts(name, &parse(rust), unit);
            if value == *name { name.clone() } else { format!("{name}: {value}") }
        }).collect();
        if offset == 1 {
            values.insert(0, format!("type: '{}'", lower_first(message.name)));
        }
        body += &format!("  return {{ {} }}\n", values.join(", "));
        body
    };
    let decoded = reached("S_");
    for message in MESSAGES.iter().filter(|message| decoded.contains(&message.name)) {
        let name = message.name;
        out += &format!("function decode{name}(r: unknown): {name} | undefined {{\n  if (!Array.isArray(r) || r.length !== {}) return undefined\n{}}}\n\n", message.fields.len(), decoder(message, 0));
    }
    for message in &server {
        out += &format!("function decode{}(r: readonly unknown[]): {} | undefined {{\n  if (r.length !== {}) return undefined\n{}}}\n\n", message.name, message.name, message.fields.len() + 1, decoder(message, 1));
    }
    out += "/** A server message, from its MessagePack array: undefined if it is malformed or not one. */\nexport function decodeMessage(r: readonly unknown[]): ServerMessage | undefined {\n  switch (r[0]) {\n";
    for message in &server {
        let (tag, _) = message.tags.iter().find(|(tag, _)| tag.starts_with("S_")).unwrap();
        out += &format!("    case {tag}:\n      return decode{}(r)\n", message.name);
    }
    out += "    default:\n      return undefined\n  }\n}\n";

    let written = reached("C_");
    for message in MESSAGES.iter().filter(|message| written.contains(&message.name)) {
        let name = message.name;
        let values: Vec<String> = message.fields.iter().map(|(field, rust, unit)| write_ts(&format!("m.{}", camel(field)), &parse(rust), unit)).collect();
        out += &format!("\nexport const write{name} = (m: {name}): unknown[] => [{}]\n", values.join(", "));
    }
    for message in MESSAGES.iter().filter(|message| sent_by(message, "C_")) {
        let (tag, _) = message.tags.iter().find(|(tag, _)| tag.starts_with("C_")).unwrap();
        let values: Vec<String> = message.fields.iter().map(|(field, rust, unit)| write_ts(&format!("m.{}", camel(field)), &parse(rust), unit)).collect();
        let input = if sent_by(message, "S_") { format!("Omit<{}, 'type'>", message.name) } else { message.name.into() };
        out += &format!("\nexport const write{} = (m: {input}): unknown[] => [{tag}, {}]\n", message.name, values.join(", "));
    }
    out
}

fn fixture_json() -> String {
    let cases: Vec<String> = samples().iter().map(|(name, bytes)| format!("  \"{name}\": [{}]", bytes.iter().map(u8::to_string).collect::<Vec<_>>().join(", "))).collect();
    format!("{{\n{}\n}}\n", cases.join(",\n"))
}

#[test]
fn the_clients_codec_and_fixture_are_generated_from_these() {
    generated("client/src/net/messages.generated.ts", &messages_ts());
    generated("client/src/__tests__/messages.fixture.json", &fixture_json());
}

#[test]
fn every_field_with_a_unit_names_a_known_one() {
    for message in MESSAGES {
        for (field, _, unit) in message.fields {
            assert!(unit.is_empty() || UNITS.contains(unit), "{}.{field}: unit {unit}", message.name);
        }
    }
}
