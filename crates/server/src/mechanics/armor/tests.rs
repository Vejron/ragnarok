use shared::vec3;
use super::*;
use crate::entities::Health;
use super::super::projectiles::Flying;
use shared::armor::{Part, PenKind};
use crate::game::{Command, Game, Seat};
use shared::protocol::{KIND_HELI, KIND_JEEP, KIND_PAK, KIND_WALKER, KIND_PLANE, KIND_STRV, PROJECTILE_SHELL, WEAPON_SHELL};
use shared::world::{Road, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

fn strike(normal: [f32; 3], point: [f32; 3]) -> Strike {
    Strike { distance: 1.0, solid: None, normal, point }
}

fn on_turret(normal: [f32; 3], point: [f32; 3]) -> Strike {
    Strike { solid: Some(0), ..strike(normal, point) }
}

/// A unit vector `degrees` off straight into the front (+Z in the hull frame, towards the tail), turned
/// sideways about the vertical.
fn from_front(degrees: f32) -> [f32; 3] {
    let a = degrees.to_radians();
    [a.sin(), 0.0, a.cos()]
}

/// A tank gun's solid shot, and a rifle bullet, close in.
const SHOT: f32 = 90.0;
const BULLET: f32 = 8.0;

fn pierced(outcome: Outcome) -> f32 {
    match outcome {
        Outcome::Pierced { scale, .. } => scale,
        other => panic!("expected it to go through, got {other:?}"),
    }
}

fn blocked(outcome: Outcome) -> f32 {
    match outcome {
        Outcome::Blocked { mm, .. } => mm,
        other => panic!("expected the plate to stop it, got {other:?}"),
    }
}

const GLACIS: [f32; 3] = [0.0, 0.0, -2.7];

#[test]
fn a_level_shot_into_the_strvs_glacis_meets_it_at_its_slope_and_a_shell_still_goes_through() {
    // 40 mm leaning back 55 degrees is 70 mm along a level shot.
    let mm = blocked(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], GLACIS), from_front(0.0), 60.0, 0.0));
    assert!((mm - 40.0 / 55f32.to_radians().cos()).abs() < 0.5, "{mm}");
    let scale = pierced(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], GLACIS), from_front(0.0), SHOT, 0.0));
    assert!(scale > 0.5 && scale < 0.75, "with little to spare: {scale}");
}

#[test]
fn a_shot_well_off_the_nose_glances_off_the_glacis() {
    let off = judge(KIND_STRV, &strike([0.0, 0.0, -1.0], GLACIS), from_front(35.0), SHOT, 0.0);
    assert!(matches!(off, Outcome::Ricochet { .. }), "{off:?}");
    // The same angle on a plain tank's gentler plate goes in.
    let tank = judge(KIND_TANK, &strike([0.0, 0.0, -1.0], [0.0, 0.0, -2.6]), from_front(35.0), SHOT, 0.0);
    assert!(matches!(tank, Outcome::Pierced { .. }), "{tank:?}");
}

#[test]
fn a_shot_from_above_has_less_glacis_to_get_through_than_a_level_one() {
    let level = blocked(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], [0.0; 3]), from_front(0.0), 1.0, 0.0));
    let (s, c) = 20f32.to_radians().sin_cos();
    let falling = blocked(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], [0.0; 3]), [0.0, -s, c], 1.0, 0.0));
    assert!(falling < level, "{falling} vs {level}");
}

#[test]
fn rifle_bullets_do_not_go_through_a_tank_anywhere_but_its_slits() {
    let faces = [
        ([0.0, 0.0, -1.0], [0.0, 0.0, -2.6], [0.0, 0.0, 1.0]),
        ([1.0, 0.0, 0.0], [1.3, 0.0, 0.0], [-1.0, 0.0, 0.0]),
        ([0.0, 0.0, 1.0], [0.0, 0.0, 2.6], [0.0, 0.0, -1.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.5, 0.0], [0.0, -1.0, 0.0]),
        ([0.0, 1.0, 0.0], [0.0, 0.5, 2.0], [0.0, -1.0, 0.0]),
    ];
    for (normal, point, dir) in faces {
        let outcome = judge(KIND_TANK, &strike(normal, point), dir, BULLET, 0.0);
        assert!(matches!(outcome, Outcome::Blocked { .. } | Outcome::Ricochet { .. }), "{point:?}: {outcome:?}");
        assert_eq!(outcome.share(shared::armor::PenKind::Ball), 0.0);
    }
    let visor = judge(KIND_TANK, &strike([0.0, 0.0, -1.0], [-0.55, 0.35, -2.6]), [0.0, 0.0, 1.0], BULLET, 0.0);
    assert!(matches!(visor, Outcome::Pierced { slit: true, plate: "driver's visor", .. }), "{visor:?}");
}

#[test]
fn a_shell_goes_through_the_side_easily_and_the_engine_grille_more_easily_than_the_roof() {
    let side = pierced(judge(KIND_STRV, &strike([1.0, 0.0, 0.0], [1.3, 0.0, 0.0]), [-1.0, 0.0, 0.0], SHOT, 0.0));
    let nose = pierced(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], GLACIS), from_front(0.0), SHOT, 0.0));
    assert!(side > nose, "{side} vs {nose}");
    // A heavy machine gun's 15 mm goes through the 10 mm grille but not the 12 mm roof beside it.
    let grille = judge(KIND_TANK, &strike([0.0, 1.0, 0.0], [0.0, 0.5, 2.0]), [0.0, -1.0, 0.0], 11.0, 0.0);
    let roof = judge(KIND_TANK, &strike([0.0, 1.0, 0.0], [0.0, 0.5, -1.5]), [0.0, -1.0, 0.0], 11.0, 0.0);
    assert!(matches!(grille, Outcome::Pierced { .. }) && matches!(roof, Outcome::Blocked { .. }), "{grille:?} {roof:?}");
}

#[test]
fn a_jeeps_sheet_steel_is_no_match_for_anything_and_a_plane_is_less() {
    let side = strike([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let toward = |degrees: f32| {
        let a = degrees.to_radians();
        [-a.cos(), 0.0, a.sin()]
    };
    assert!(matches!(judge(KIND_JEEP, &side, toward(60.0), BULLET, 0.0), Outcome::Pierced { .. }));
    // Only a graze keeps a bullet out, and a pistol's skids off.
    assert!(matches!(judge(KIND_JEEP, &side, toward(86.0), BULLET, 0.0), Outcome::Blocked { .. }));
    assert!(matches!(judge(KIND_JEEP, &side, toward(86.0), 2.0, 0.0), Outcome::Ricochet { .. }));
    assert!(matches!(judge(KIND_JEEP, &side, toward(86.0), SHOT, 0.0), Outcome::Pierced { .. }), "a shell bites in");
    assert!(matches!(judge(KIND_PLANE, &side, toward(89.0), BULLET, 0.0), Outcome::Pierced { .. }));
}

#[test]
fn a_turrets_plates_turn_with_it() {
    // From the left, square onto the drum: its 30 mm side with the gun ahead...
    // Low on the drum, under the mantlet once it has turned.
    let hit = on_turret([-1.0, 0.0, 0.0], [-1.0, 0.55, 0.3]);
    let ahead = judge(KIND_TANK, &hit, [1.0, 0.0, 0.0], 40.0, 0.0);
    assert!(matches!(ahead, Outcome::Pierced { turret: true, plate: "turret side", .. }), "{ahead:?}");
    // ...its 50 mm front once it has turned to face the shot.
    let facing = judge(KIND_TANK, &hit, [1.0, 0.0, 0.0], 40.0, std::f32::consts::FRAC_PI_2);
    assert!(matches!(facing, Outcome::Blocked { turret: true, plate: "turret front", .. }), "{facing:?}");
    let a = 80f32.to_radians();
    assert!(matches!(judge(KIND_TANK, &hit, [a.cos(), 0.0, a.sin()], 40.0, 0.0), Outcome::Ricochet { .. }), "a grazing shot turns off the drum");
}

#[test]
fn the_paks_shield_stops_a_rifle_round_but_not_through_its_sight_gap() {
    let shield = on_turret([0.0, 0.0, -1.0], [-0.4, 0.2, -0.98]);
    assert!(matches!(judge(KIND_PAK, &shield, [0.0, 0.0, 1.0], BULLET, 0.0), Outcome::Blocked { .. }));
    let gap = on_turret([0.0, 0.0, -1.0], [0.25, 0.3, -0.98]);
    assert!(matches!(judge(KIND_PAK, &gap, [0.0, 0.0, 1.0], BULLET, 0.0), Outcome::Pierced { slit: true, .. }));
    let sniper = judge(KIND_PAK, &shield, [0.0, 0.0, 1.0], 12.0, 0.0);
    assert!(matches!(sniper, Outcome::Pierced { .. }), "{sniper:?}");
}

#[test]
fn reflecting_mirrors_about_the_normal() {
    let out = reflect([0.6, 0.0, 0.8], [0.0, 0.0, -1.0]);
    assert!((out[0] - 0.6).abs() < 1e-6 && (out[2] + 0.8).abs() < 1e-6, "{out:?}");
}

// --- in the game ---

fn game() -> Game {
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 30_000,
        spawns: [[-25_000, 0], [25_000, 0]].into_iter().chain((0..11).map(|i| [i * 2700 - 12_000, 0])).collect(),
        roads: vec![Road { width: 800, kind: 0, points: vec![[-30_000, 0], [30_000, 0]] }],
        ..Default::default()
    };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    for _ in 0..90 {
        game.step();
    }
    game
}

fn of_kind(game: &Game, kind: u8) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, v)| v.kind == kind).expect("placed").0
}

/// Fires a shell at the middle of vehicle `vid` from 12 m away in the hull-frame direction `local_from`.
fn shoot(game: &mut Game, vid: u32, local_from: [f32; 3]) -> Vec<ShotEvent> {
    shoot_at(game, vid, [0.0; 3], local_from)
}

/// Fires a shell at hull-frame point `target` on vehicle `vid` from 12 m away in the direction `local_from`,
/// and steps until it is gone. Returns the shot events it caused.
fn shoot_at(game: &mut Game, vid: u32, target: [f32; 3], local_from: [f32; 3]) -> Vec<ShotEvent> {
    let pose = game.world.physics.car_pose(vid).unwrap();
    let at = pose.world_point(target);
    let from = std::array::from_fn(|i| at[i] + pose.world_dir(local_from)[i] * 12.0);
    let flat = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
    let len = vec3::length(flat);
    let dir = flat.map(|v| v / len);
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying { kind: PROJECTILE_SHELL, owner: 0, source: 0, position: from, velocity: dir.map(|v| v * SHELL_SPEED), ttl: SHELL_TTL_TICKS, item: 0, back: 0, flown: 0.0 });
    let mut events = Vec::new();
    for _ in 0..40 {
        game.step();
        events.extend(game.world.shots.iter().copied());
        if !game.world.in_flight().contains_key(&id) {
            break;
        }
    }
    events
}

#[test]
fn a_shell_into_the_side_of_a_strv_hurts_and_one_into_its_glacis_off_axis_glances_off() {
    let mut game = game();
    let vid = of_kind(&game, KIND_STRV);
    let full = game.world.entities.of::<Health>(vid).hp;
    // Square onto its left side.
    shoot(&mut game, vid, [-1.0, 0.0, 0.0]);
    let after_side = game.world.entities.of::<Health>(vid).hp;
    let side_damage = full - after_side;
    assert!(side_damage >= 40, "a square side hit hurts: {side_damage}");
    // 40 degrees off the nose at the middle of the glacis: a glancing blow.
    let (s, c) = 40f32.to_radians().sin_cos();
    let events = shoot_at(&mut game, vid, [0.0, 0.0, -STRV_HALF_EXTENTS[2] + 0.05], [s, 0.0, -c]);
    assert!(!events.iter().any(|e| matches!(e.weapon, WEAPON_SHELL | shared::protocol::WEAPON_AP)), "a ricochet does not burst: {events:?}");
    assert_eq!(game.world.entities.of::<Health>(vid).hp, after_side);
}

#[test]
fn a_hit_from_behind_costs_more_than_one_into_the_nose() {
    let mut game = game();
    let vid = of_kind(&game, KIND_STRV);
    let full = game.world.entities.of::<Health>(vid).hp;
    shoot(&mut game, vid, [0.0, 0.0, -1.0]);
    let nose = full - game.world.entities.of::<Health>(vid).hp;
    let before = game.world.entities.of::<Health>(vid).hp;
    shoot(&mut game, vid, [0.0, 0.0, 1.0]);
    let tail = before - game.world.entities.of::<Health>(vid).hp;
    assert!(nose > 0 && u32::from(tail) * 3 > u32::from(nose) * 4, "nose {nose}, tail {tail}");
}

fn join(game: &mut Game) -> u32 {
    let (outbox, _inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    std::mem::forget(_inbox);
    response.try_recv().unwrap().unwrap()
}

#[test]
fn enough_damage_to_the_turret_takes_it_off_and_kills_the_gunner() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let gunner = join(&mut game);
    game.world.entities.get_mut::<Seats>(vid).unwrap()[1] = Some(gunner);
    game.world.entities.get_mut::<Player>(gunner).unwrap().seat = Some(Seat { vehicle: vid, index: 1, remote: false });
    damage_turret(&mut game.world, vid, TANK_TURRET_HP / 2, None);
    assert_eq!(game.world.entities.of::<Vehicle>(vid).damage[DAMAGE_TURRET], 0, "still on");
    assert_eq!(game.world.entities.of::<Seats>(vid)[1], Some(gunner));
    damage_turret(&mut game.world, vid, TANK_TURRET_HP, None);
    let vehicle = &game.world.entities.of::<Vehicle>(vid);
    assert_eq!(vehicle.damage[DAMAGE_TURRET], TURRET_GONE);
    assert_eq!(game.world.entities.of::<Seats>(vid).rider(1), None);
    assert!(!game.world.entities.alive(gunner), "the gunner went with it");
    assert!(game.world.shots.iter().any(|shot| shot.weapon == WEAPON_TURRET));
    assert!(!game.world.entities.dead(vid), "the tank itself is not wrecked");
}

#[test]
fn the_machine_gunner_goes_with_the_turret_too() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let (gunner, top) = (join(&mut game), join(&mut game));
    for (id, index) in [(gunner, 1), (top, 2)] {
        game.world.entities.get_mut::<Seats>(vid).unwrap()[index] = Some(id);
        game.world.entities.get_mut::<Player>(id).unwrap().seat = Some(Seat { vehicle: vid, index, remote: false });
    }
    damage_turret(&mut game.world, vid, 255, None);
    assert!(!game.world.entities.alive(gunner) && !game.world.entities.alive(top));
    assert_eq!(game.world.entities.of::<Seats>(vid).rider(2), None);
}

#[test]
fn a_tank_without_its_turret_cannot_fire_and_gets_one_back_on_respawn() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    damage_turret(&mut game.world, vid, 255, None);
    assert_eq!(game.world.entities.of::<Vehicle>(vid).damage[DAMAGE_TURRET], TURRET_GONE);
    // Wrecked and put back: the turret is on again.
    super::super::combat::damage_vehicle(&mut game.world, vid, 255, None);
    for _ in 0..(WRECK_TICKS + 5) {
        game.step();
    }
    let vehicle = &game.world.entities.of::<Vehicle>(vid);
    assert_eq!(vehicle.damage[DAMAGE_TURRET], 0);
    assert_eq!(vehicle.turret_hp, TANK_TURRET_HP);
}

#[test]
fn a_shell_into_a_tanks_turret_damages_the_turret() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let pose = game.world.physics.car_pose(vid).unwrap();
    // A shell from the side, at the height of the turret drum.
    let target = pose.world_point([0.0, 0.85, 0.3]);
    let from = pose.world_point([-12.0, 0.85, 0.3]);
    let dir = [target[0] - from[0], target[1] - from[1], target[2] - from[2]];
    let len = vec3::length(dir);
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying { kind: PROJECTILE_SHELL, owner: 0, source: 0, position: from, velocity: dir.map(|v| v / len * SHELL_SPEED), ttl: SHELL_TTL_TICKS, item: 0, back: 0, flown: 0.0 });
    for _ in 0..40 {
        game.step();
    }
    assert!(game.world.entities.of::<Vehicle>(vid).turret_hp < TANK_TURRET_HP, "turret took the hit");
}

/// Fires an AK5 round at hull-frame point `target` on vehicle `vid` from 12 m away in the direction
/// `local_from`, as a hitscan (a round in flight lands the same way). Returns the shot events it caused.
fn rifle_at(game: &mut Game, vid: u32, target: [f32; 3], local_from: [f32; 3]) -> Vec<ShotEvent> {
    gun_at(game, shared::items::ITEM_AK5, vid, target, local_from)
}

/// [`rifle_at`] with gun `item`.
fn gun_at(game: &mut Game, item: u8, vid: u32, target: [f32; 3], local_from: [f32; 3]) -> Vec<ShotEvent> {
    use super::super::combat::{Round, hitscan};
    let pose = game.world.physics.car_pose(vid).unwrap();
    let at = pose.world_point(target);
    let from: [f32; 3] = std::array::from_fn(|i| at[i] + pose.world_dir(local_from)[i] * 12.0);
    let flat = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
    let len = vec3::length(flat);
    let gun = shared::items::def(item).unwrap();
    let round = Round { damage: gun.damage, vehicle: gun.damage / 4, plane: gun.damage / 4, weapon: gun.weapon, range: 50.0, falloff: gun.falloff, pen: gun.pen };
    game.world.shots.clear();
    game.world.marks.clear();
    hitscan(&mut game.world, 0, from, flat.map(|v| v / len), &round, 0, 0);
    game.world.shots.clone()
}

#[test]
fn a_rifle_round_strikes_sparks_off_a_tank_and_does_it_no_harm() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let full = game.world.entities.of::<Health>(vid).hp;
    for from in [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
        rifle_at(&mut game, vid, [0.0; 3], from);
        assert!(game.world.marks.iter().any(|(mark, _)| mark.vehicle == vid && matches!(mark.style, shared::protocol::MARK_GLANCE | shared::protocol::MARK_SCUFF)), "{:?}", game.world.marks);
    }
    assert_eq!(game.world.entities.of::<Health>(vid).hp, full);
}

#[test]
fn a_rifle_round_still_holes_a_jeep() {
    let mut game = game();
    let vid = of_kind(&game, KIND_JEEP);
    let full = game.world.entities.of::<Health>(vid).hp;
    rifle_at(&mut game, vid, [0.0; 3], [-1.0, 0.0, 0.0]);
    assert!(game.world.entities.of::<Health>(vid).hp < full);
}

#[test]
fn rifle_rounds_into_a_jeeps_bonnet_kill_its_engine_and_it_cannot_drive() {
    let mut game = game();
    let vid = of_kind(&game, KIND_JEEP);
    let engine = [0.0, 0.05, -1.4];
    rifle_at(&mut game, vid, engine, [-1.0, 0.0, 0.0]);
    let (mark, _) = game.world.marks[0];
    assert_eq!(mark.style, shared::protocol::MARK_HOLE | shared::protocol::MARK_VITAL, "the hole its smoke comes out of");
    assert!(game.world.armor_hits.iter().any(|hit| hit.inside.hurt == 1 << Part::Engine.slot().unwrap()), "called out");
    while part(&game, vid, Part::Engine) > 0 {
        rifle_at(&mut game, vid, engine, [-1.0, 0.0, 0.0]);
    }
    assert_eq!(condition(&game, vid).power, 0.0);
}

#[test]
fn a_rifle_round_through_a_jeep_that_hits_nothing_inside_leaves_a_plain_hole_and_no_callout() {
    let mut game = game();
    let vid = of_kind(&game, KIND_JEEP);
    game.world.armor_hits.clear();
    rifle_at(&mut game, vid, [0.0, 0.2, 0.3], [-1.0, 0.0, 0.0]);
    assert_eq!(game.world.marks[0].0.style, shared::protocol::MARK_HOLE);
    assert!(game.world.armor_hits.is_empty());
}

#[test]
fn rifle_rounds_into_a_jeeps_fuel_tank_set_it_alight() {
    let mut game = game();
    let vid = of_kind(&game, KIND_JEEP);
    for _ in 0..4 {
        rifle_at(&mut game, vid, [0.0, -0.15, 1.6], [-1.0, 0.0, 0.0]);
    }
    assert!(game.world.entities.has::<Burning>(vid));
}

#[test]
fn a_helicopter_with_its_engine_shot_dead_has_no_power_to_fly() {
    let mut game = game();
    let vid = of_kind(&game, KIND_HELI);
    while part(&game, vid, Part::Engine) > 0 {
        gun_at(&mut game, shared::items::ITEM_SNIPER, vid, [0.0, 0.55, 0.5], [-1.0, 0.0, 0.0]);
    }
    assert_eq!(condition(&game, vid).power, 0.0);
}

#[test]
fn a_round_through_a_walkers_cab_hits_its_pilot() {
    let mut game = game();
    let vid = of_kind(&game, KIND_WALKER);
    let pilot = seat(&mut game, vid, 0);
    let full = game.world.entities.of::<Health>(pilot).hp;
    gun_at(&mut game, shared::items::ITEM_SNIPER, vid, [0.0, 0.4, -0.9], [-1.0, 0.0, 0.0]);
    assert!(game.world.entities.get::<Health>(pilot).is_none_or(|health| health.hp < full));
}

// --- inside ---

/// Seats a fresh player in seat `index` of vehicle `vid`.
fn seat(game: &mut Game, vid: u32, index: usize) -> u32 {
    let id = join(game);
    game.world.entities.get_mut::<Seats>(vid).unwrap()[index] = Some(id);
    game.world.entities.get_mut::<Player>(id).unwrap().seat = Some(Seat { vehicle: vid, index, remote: false });
    id
}

fn condition(game: &Game, vid: u32) -> sim::CarCondition {
    let vehicle = game.world.entities.of::<Vehicle>(vid);
    sim::CarCondition::of(vehicle.kind, game.world.entities.of::<Health>(vid).hp, vehicle.damage)
}

fn part(game: &Game, vid: u32, part: Part) -> u8 {
    game.world.entities.of::<Vehicle>(vid).parts[part.slot().unwrap()]
}

#[test]
fn a_rifle_round_through_the_drivers_visor_hits_the_driver_and_not_the_tank() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let driver = seat(&mut game, vid, 0);
    let full = game.world.entities.of::<Health>(vid).hp;
    let before = game.world.entities.of::<Health>(driver).hp;
    rifle_at(&mut game, vid, [-0.55, 0.35, -2.6], [0.0, 0.0, -1.0]);
    assert!(game.world.entities.of::<Health>(driver).hp < before, "the driver is hit");
    assert_eq!(game.world.entities.of::<Health>(vid).hp, full, "the tank is not");
    // A hand's breadth to the side, the glacis stops it.
    let after = game.world.entities.of::<Health>(driver).hp;
    rifle_at(&mut game, vid, [-0.1, 0.35, -2.6], [0.0, 0.0, -1.0]);
    assert_eq!(game.world.entities.of::<Health>(driver).hp, after);
}

#[test]
fn a_shell_into_the_rear_wrecks_the_engine_and_the_tank_cannot_drive() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    assert_eq!(condition(&game, vid).power, 1.0);
    shoot_at(&mut game, vid, [0.0, 0.0, 2.6], [0.0, 0.0, 1.0]);
    assert!(!game.world.entities.dead(vid), "it is not a wreck");
    assert_eq!(part(&game, vid, Part::Engine), 0, "{:?}", game.world.entities.of::<Vehicle>(vid).parts);
    let now = condition(&game, vid);
    assert_eq!((now.power, now.turn), (0.0, 0.0), "{now:?}");
}

#[test]
fn a_shell_low_on_the_flank_breaks_that_track() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    shoot_at(&mut game, vid, [-1.3, -0.3, 1.0], [-1.0, 0.0, 0.0]);
    assert_eq!(part(&game, vid, Part::TrackLeft), 0);
    assert!(part(&game, vid, Part::TrackRight) > 0);
    let now = condition(&game, vid);
    assert_eq!(now.sides, [0.0, 1.0]);
    assert!(now.turn < 0.5 && now.speed < 0.5 && now.power > 0.0, "it crawls: {now:?}");
}

#[test]
fn a_shell_into_the_ammunition_cooks_it_off() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let crew = seat(&mut game, vid, 0);
    shoot_at(&mut game, vid, [1.3, 0.0, 0.2], [1.0, 0.0, 0.0]);
    assert!(game.world.entities.dead(vid), "wrecked");
    assert!(!game.world.entities.alive(crew), "with its crew");
}

#[test]
fn a_shell_through_the_mantlet_smashes_the_breech_and_kills_the_gunner() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let gunner = seat(&mut game, vid, 1);
    let driver = seat(&mut game, vid, 0);
    shoot_at(&mut game, vid, [0.0, 0.85, -0.7], [0.0, 0.0, -1.0]);
    assert!(!game.world.entities.dead(vid));
    assert!(part_gone(&game.world, vid, Part::Breech));
    assert!(!game.world.entities.alive(gunner), "the gunner was behind it");
    assert!(game.world.entities.alive(driver), "the driver was not");
}

#[test]
fn burning_fuel_eats_the_tank_until_it_is_a_wreck_and_a_fresh_one_is_whole() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let entry = Entry { at: [-1.0, -0.25, 1.15], dir: [1.0, 0.0, 0.0], residual: 50.0, calibre: 75.0, kind: PenKind::Ap, slit: true, crew: 100, weapon: WEAPON_SHELL, attacker: None };
    let done = inside(&mut game.world, vid, entry);
    let fuel = 1 << Part::Fuel.slot().unwrap();
    assert_eq!(done.gone & fuel, fuel, "{done:?}");
    assert!(game.world.entities.has::<crate::game::Burning>(vid));
    let full = game.world.entities.of::<Health>(vid).hp;
    for _ in 0..60 {
        game.step();
    }
    assert!(game.world.entities.of::<Health>(vid).hp < full, "it burns");
    assert_eq!(game.world.entities.of::<Vehicle>(vid).damage[shared::protocol::DAMAGE_FIRE], shared::protocol::BURNING);
    super::super::combat::damage_vehicle(&mut game.world, vid, 255, None);
    for _ in 0..(WRECK_TICKS + 5) {
        game.step();
    }
    assert_eq!(game.world.entities.of::<Vehicle>(vid).parts, Vehicle::fresh_parts(KIND_TANK));
    assert!(!game.world.entities.has::<crate::game::Burning>(vid));
    assert_eq!(condition(&game, vid), sim::CarCondition::of(KIND_TANK, game.world.entities.of::<Health>(vid).hp, [0; shared::protocol::DAMAGE_SLOTS]));
}

#[test]
fn a_spray_of_fragments_is_the_same_every_time() {
    let run = || {
        let mut game = game();
        let vid = of_kind(&game, KIND_TANK);
        let entry = Entry { at: [1.3, 0.1, -1.0], dir: [-1.0, 0.0, 0.0], residual: 60.0, calibre: 75.0, kind: PenKind::Ap, slit: false, crew: 100, weapon: WEAPON_SHELL, attacker: None };
        inside(&mut game.world, vid, entry);
        game.world.entities.of::<Vehicle>(vid).parts
    };
    assert_eq!(run(), run());
}

#[test]
fn an_at_rifle_goes_through_a_tanks_sides_and_rear_but_not_its_front() {
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let full = game.world.entities.of::<Health>(vid).hp;
    gun_at(&mut game, shared::items::ITEM_AT_RIFLE, vid, [0.0, 0.0, -2.6], [0.0, 0.0, -1.0]);
    assert_eq!(game.world.entities.of::<Health>(vid).hp, full, "the glacis stops it");
    gun_at(&mut game, shared::items::ITEM_AT_RIFLE, vid, [0.0, 0.0, 2.6], [0.0, 0.0, 1.0]);
    assert!(game.world.entities.of::<Health>(vid).hp < full, "the rear plate does not");
    assert!(part(&game, vid, Part::Engine) < Vehicle::fresh_parts(KIND_TANK)[Part::Engine.slot().unwrap()], "and it hits the engine behind it");
    let before = game.world.entities.of::<Health>(vid).hp;
    gun_at(&mut game, shared::items::ITEM_AT_RIFLE, vid, [-1.3, 0.0, 0.5], [-1.0, 0.0, -0.3]);
    assert!(game.world.entities.of::<Health>(vid).hp < before, "nor the side plate, a little off square");
    let before = game.world.entities.of::<Health>(vid).hp;
    gun_at(&mut game, shared::items::ITEM_AT_RIFLE, vid, [-1.0, 0.6, 0.3], [-1.0, 0.0, 0.0]);
    assert!(game.world.entities.of::<Health>(vid).hp < before, "nor the turret's side");
}

/// Fires a high-explosive shell, as [`shoot_at`] does an armour-piercing one.
fn he_at(game: &mut Game, vid: u32, target: [f32; 3], local_from: [f32; 3]) {
    let pose = game.world.physics.car_pose(vid).unwrap();
    let at = pose.world_point(target);
    let from: [f32; 3] = std::array::from_fn(|i| at[i] + pose.world_dir(local_from)[i] * 12.0);
    let flat = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
    let len = vec3::length(flat);
    let kind = shared::protocol::PROJECTILE_HE;
    let id = game.world.launch(Flying { kind, owner: 0, source: 0, position: from, velocity: flat.map(|v| v / len * SHELL_SPEED), ttl: SHELL_TTL_TICKS, item: 0, back: 0, flown: 0.0 });
    for _ in 0..40 {
        game.step();
        if !game.world.in_flight().contains_key(&id) {
            break;
        }
    }
}

#[test]
fn high_explosive_does_little_to_a_tanks_front_and_wrecks_a_jeep() {
    let mut game = game();
    let tank = of_kind(&game, KIND_TANK);
    let full = game.world.entities.of::<Health>(tank).hp;
    he_at(&mut game, tank, [0.0, 0.0, -2.6], [0.0, 0.0, -1.0]);
    let lost = full - game.world.entities.of::<Health>(tank).hp;
    assert!(lost > 0 && lost < 25, "the blast scorches it: {lost}");
    let jeep = of_kind(&game, KIND_JEEP);
    he_at(&mut game, jeep, [0.0; 3], [-1.0, 0.0, 0.0]);
    assert!(game.world.entities.dead(jeep), "a jeep is no match for it");
}

#[test]
fn the_shooter_and_the_crew_are_told_what_a_round_did() {
    use shared::protocol::{ARMOR_BLOCKED, ARMOR_PIERCED, ARMOR_SLIT};
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    seat(&mut game, vid, 0);
    game.world.armor_hits.clear();
    rifle_at(&mut game, vid, [0.0, 0.0, -2.6], [0.0, 0.0, -1.0]);
    assert_eq!(game.world.armor_hits.iter().map(|hit| hit.result).collect::<Vec<_>>(), [ARMOR_BLOCKED]);
    game.world.armor_hits.clear();
    rifle_at(&mut game, vid, [-0.55, 0.35, -2.6], [0.0, 0.0, -1.0]);
    let slit = game.world.armor_hits[0];
    assert_eq!(slit.result, ARMOR_SLIT);
    assert_ne!(slit.inside.crew & 1, 0, "the driver was hit: {slit:?}");
    // An AT rifle through the rear plate, into the engine.
    game.world.armor_hits.clear();
    gun_at(&mut game, shared::items::ITEM_AT_RIFLE, vid, [0.0, 0.0, 2.6], [0.0, 0.0, 1.0]);
    let engine = 1 << Part::Engine.slot().unwrap();
    assert!(game.world.armor_hits.iter().any(|hit| hit.result == ARMOR_PIERCED && hit.inside.hurt & engine != 0), "{:?}", game.world.armor_hits);
}

#[test]
fn a_hail_of_rifle_rounds_on_a_tank_is_one_scuff_a_snapshot_but_every_one_is_called_out() {
    use super::super::combat::{Round, hitscan};
    use shared::protocol::{MARK_GLANCE, MARK_HOLE, MARK_SCUFF};
    let mut game = game();
    let vid = of_kind(&game, KIND_TANK);
    let pose = game.world.physics.car_pose(vid).unwrap();
    let gun = shared::items::def(shared::items::ITEM_AK5).unwrap();
    let round = Round { damage: gun.damage, vehicle: 3, plane: 3, weapon: gun.weapon, range: 50.0, falloff: gun.falloff, pen: gun.pen };
    let fire_at = |game: &mut Game, target: [f32; 3]| {
        let at = pose.world_point(target);
        let from: [f32; 3] = std::array::from_fn(|i| at[i] + pose.world_dir([0.0, 0.0, -1.0])[i] * 12.0);
        let dir = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
        let len = vec3::length(dir);
        hitscan(&mut game.world, 0, from, dir.map(|v| v / len), &round, 0, 0);
    };
    game.world.marks.clear();
    game.world.armor_hits.clear();
    for k in 0..10 {
        fire_at(&mut game, [-0.3 + k as f32 * 0.06, 0.0, -2.6]);
    }
    let scuffs = game.world.marks.iter().filter(|(mark, _)| mark.vehicle == vid && matches!(mark.style, MARK_GLANCE | MARK_SCUFF)).count();
    assert_eq!(scuffs, 1, "{:?}", game.world.marks);
    assert_eq!(game.world.armor_hits.len(), 10, "the shooter hears of every one");
    // A round through the visor leaves its mark even so: in the hull's frame, where the visor is, going back along the hull.
    fire_at(&mut game, [-0.55, 0.35, -2.6]);
    let (hole, _) = *game.world.marks.iter().find(|(mark, _)| mark.style == MARK_HOLE).expect("a hole");
    assert_eq!((hole.x, hole.y, hole.z), (-55, 35, -260));
    assert!(hole.dz > 120, "{hole:?}");
}
