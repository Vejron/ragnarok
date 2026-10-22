use super::*;
use crate::entities::Health;
use super::super::projectiles::Flying;
use crate::game::{Command, Game, Seat};
use shared::protocol::{KIND_JEEP, KIND_STRV, PROJECTILE_SHELL, WEAPON_SHELL};
use shared::world::{Road, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

fn strike(normal: [f32; 3], point: [f32; 3]) -> Strike {
    Strike { distance: 1.0, turret: false, normal, point }
}

/// A unit vector `degrees` off straight into the front (+Z in the hull frame, towards the tail), turned
/// sideways about the vertical.
fn from_front(degrees: f32) -> [f32; 3] {
    let a = degrees.to_radians();
    [a.sin(), 0.0, a.cos()]
}

fn scale(outcome: Outcome) -> f32 {
    match outcome {
        Outcome::Hit { scale, .. } => scale,
        other => panic!("expected a hit, got {other:?}"),
    }
}

#[test]
fn a_level_shot_into_the_strvs_glacis_lands_at_its_slope_angle() {
    // The plate leans back 55 degrees, so a level shot meets it at 55 degrees from its normal.
    let hit = judge(KIND_STRV, &strike([0.0, 0.0, -1.0], [0.0, 0.0, -2.7]), from_front(0.0));
    assert!((scale(hit) - 0.96f32.cos()).abs() < 0.01, "{hit:?}");
}

#[test]
fn a_shot_well_off_the_nose_glances_off_the_glacis() {
    let off = judge(KIND_STRV, &strike([0.0, 0.0, -1.0], [0.0, 0.0, -2.7]), from_front(35.0));
    assert!(matches!(off, Outcome::Ricochet { .. }), "{off:?}");
    // The same angle on a plain tank's gentler plate is still a hit.
    let tank = judge(KIND_TANK, &strike([0.0, 0.0, -1.0], [0.0, 0.0, -2.6]), from_front(35.0));
    assert!(matches!(tank, Outcome::Hit { .. }), "{tank:?}");
}

#[test]
fn a_shot_from_above_goes_in_straighter_than_a_level_one() {
    let level = scale(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], [0.0; 3]), from_front(0.0)));
    let (s, c) = 20f32.to_radians().sin_cos();
    let falling = scale(judge(KIND_STRV, &strike([0.0, 0.0, -1.0], [0.0; 3]), [0.0, -s, c]));
    assert!(falling > level, "{falling} vs {level}");
}

#[test]
fn the_side_takes_a_square_hit_in_full_and_the_back_deck_is_a_weak_spot() {
    let side = judge(KIND_STRV, &strike([1.0, 0.0, 0.0], [1.3, 0.0, 0.0]), [-1.0, 0.0, 0.0]);
    assert!((scale(side) - 1.0).abs() < 1e-4);
    let back = judge(KIND_STRV, &strike([0.0, 0.0, 1.0], [0.0, 0.2, 2.6]), [0.0, 0.0, -1.0]);
    let off_deck = judge(KIND_STRV, &strike([0.0, 0.0, 1.0], [0.0, -0.4, 2.6]), [0.0, 0.0, -1.0]);
    assert!(scale(back) > scale(off_deck) * 1.5, "{back:?} {off_deck:?}");
    assert!(scale(back) > 1.0);
}

#[test]
fn a_jeep_only_turns_away_the_most_grazing_hits_and_a_plane_none() {
    let side = strike([1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    let toward = |degrees: f32| {
        let a = degrees.to_radians();
        [-a.cos(), 0.0, a.sin()]
    };
    assert!(matches!(judge(KIND_JEEP, &side, toward(60.0)), Outcome::Hit { .. }));
    assert!(matches!(judge(KIND_JEEP, &side, toward(86.0)), Outcome::Ricochet { .. }));
    assert!(matches!(judge(KIND_PLANE, &side, toward(89.0)), Outcome::Hit { .. }));
}

#[test]
fn a_turret_hit_is_marked_and_the_drum_turns_away_a_grazing_shot() {
    let mut turret = strike([1.0, 0.0, 0.0], [1.0, 1.0, 0.3]);
    turret.turret = true;
    assert_eq!(judge(KIND_TANK, &turret, [-1.0, 0.0, 0.0]), Outcome::Hit { scale: 1.0, turret: true });
    let a = 80f32.to_radians();
    assert!(matches!(judge(KIND_TANK, &turret, [-a.cos(), 0.0, a.sin()]), Outcome::Ricochet { .. }));
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
    let len = flat.iter().map(|v| v * v).sum::<f32>().sqrt();
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
    assert!(side_damage >= 90, "a square side hit hurts: {side_damage}");
    // 40 degrees off the nose at the middle of the glacis: a glancing blow.
    let (s, c) = 40f32.to_radians().sin_cos();
    let events = shoot_at(&mut game, vid, [0.0, 0.0, -STRV_HALF_EXTENTS[2] + 0.05], [s, 0.0, -c]);
    assert!(events.iter().any(|e| e.weapon == WEAPON_RICOCHET && e.shooter == vid), "{events:?}");
    assert!(!events.iter().any(|e| e.weapon == WEAPON_SHELL), "a ricochet does not burst: {events:?}");
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
    assert!(nose > 0 && tail > nose * 2, "nose {nose}, tail {tail}");
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
    assert!(!game.world.entities.of::<Player>(gunner).alive(), "the gunner went with it");
    assert!(game.world.shots.iter().any(|shot| shot.weapon == WEAPON_TURRET));
    assert!(!game.world.entities.dead(vid), "the tank itself is not wrecked");
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
    let len = dir.iter().map(|v| v * v).sum::<f32>().sqrt();
    let id = game.world.alloc_id();
    game.world.launch_as(id, Flying { kind: PROJECTILE_SHELL, owner: 0, source: 0, position: from, velocity: dir.map(|v| v / len * SHELL_SPEED), ttl: SHELL_TTL_TICKS, item: 0, back: 0, flown: 0.0 });
    for _ in 0..40 {
        game.step();
    }
    assert!(game.world.entities.of::<Vehicle>(vid).turret_hp < TANK_TURRET_HP, "turret took the hit");
}
