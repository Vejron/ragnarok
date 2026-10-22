use crate::entities::With;
use super::Mechanic;
use crate::game::{Mounts, Seat, Vehicle, World};
use sim::{CarCondition, CarControls, CarImpact, CarPose, Character, Obj, half_extents};
use shared::consts::*;
use shared::vehicles::{VehicleDef, vehicle_def};
use shared::protocol::{
    DAMAGE_FRONT, DAMAGE_LEFT, DAMAGE_REAR, DAMAGE_RIGHT, DAMAGE_WHEELS, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER, ShotEvent, WEAPON_CRASH, is_drone,
    WEAPON_ROADKILL, WEAPON_SCRAPE, seat_count, vehicle_hp,
};
use shared::quant::{to_cm, wrap_angle};
use shared::world::WorldFile;
use std::collections::BTreeMap;

/// Cars parked around the map, seats, and driving them.
///
/// `BUTTON_USE` is one press per input frame: it boards the nearest vehicle with a free seat, or
/// leaves the one the player sits in. A driver's axes and `BUTTON_JUMP` (handbrake) steer the car;
/// passengers only ride, except the gunner of a tank or a helicopter (see `Gunnery`). A helicopter's pilot leans it with the axes and climbs on `BUTTON_JUMP`,
/// sinks on `BUTTON_RUN`. Cars are dynamic bodies stepped here, once per tick, after all input
/// is in. `BUTTON_RELOAD` moves a seated player to a free seat of the same vehicle.
pub struct Vehicles;

/// Cars (half jeeps, half sports cars), tanks and helicopters placed at start. Fewer appear when the world has too few clear spawn points.
const CAR_COUNT: usize = 6;
const TANK_COUNT: usize = 2;
const HELI_COUNT: usize = 1;
/// Roadsters, placed right after the other cars.
const ROADSTER_COUNT: usize = 1;
/// Strv S tanks, placed with the other tanks.
const STRV_COUNT: usize = 1;
/// Walkers, placed with the tanks and the helicopter on the road's spawn points.
const WALKER_COUNT: usize = 1;
/// Hens, placed after the mortars on whichever spawn points are left.
const HEN_COUNT: usize = 1;
/// Field mortars, placed after the planes on whichever spawn points are left. A car hitches one (see `hitch`).
const MORTAR_COUNT: usize = 1;
/// Planes, placed after everything else where they have room to take off.
const PLANE_COUNT: usize = 2;
/// The longest runway worth looking for, metres: a plane is off the ground in about 40.
const RUNWAY_SEARCH: f32 = 150.0;
/// Spawn points `Spawn` hands to players first; cars prefer the others.
const PLAYER_SPAWNS: usize = 6;
/// Spawn height of the chassis centre, a little above its resting height.
const CAR_DROP_Y: f32 = 1.0;
const PLANE_DROP_Y: f32 = 1.5;
/// Cars at least this fast (or any overlap at all) hurl a standing player away.
/// Knockback speed is the car's speed times this, within the limits below.
const KNOCK_SPEED_FACTOR: f32 = 1.1;
const KNOCK_MIN_PUSH: f32 = 2.5;
/// Hop given to a player who is hit: `KNOCK_HOP + KNOCK_HOP_PER_SPEED * speed`, at most `KNOCK_HOP_MAX`.
const KNOCK_HOP: f32 = 2.0;
const KNOCK_HOP_PER_SPEED: f32 = 0.25;
const KNOCK_HOP_MAX: f32 = 8.0;
/// A car slower than this (m/s) only shoves a stickman. Faster, it hurts by this much per m/s over.
const ROADKILL_MIN_SPEED: f32 = 3.0;
const ROADKILL_DAMAGE_PER_SPEED: f32 = 8.0;
/// Ticks before the same player can be hit again, so one pass is one hit.
const HIT_COOLDOWN: u32 = 12;
/// A car on its side or roof this long, and not moving, is set back on its wheels.
const UNFLIP_TICKS: u32 = 3 * TICK_RATE;

/// How high above the ground a fresh vehicle of `kind` starts, at its chassis centre: a walker's or a
/// hen's hull as high as its legs hold it up.
fn drop_height(kind: u8) -> f32 {
    match kind {
        KIND_WALKER | KIND_HEN => WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1],
        KIND_PLANE => PLANE_DROP_Y,
        _ => CAR_DROP_Y,
    }
}

/// Heading of the nearest road segment to `(x, z)`, in the stickman yaw convention, or 0.
fn road_heading(map: &WorldFile, x: f32, z: f32) -> f32 {
    let mut best = (f32::MAX, 0.0);
    for road in &map.roads {
        for pair in road.points.windows(2) {
            let [ax, az] = pair[0].map(|v| v as f32 / 100.0);
            let [bx, bz] = pair[1].map(|v| v as f32 / 100.0);
            let (dx, dz) = (bx - ax, bz - az);
            let length_sq = dx * dx + dz * dz;
            if length_sq < 1e-6 {
                continue;
            }
            let t = (((x - ax) * dx + (z - az) * dz) / length_sq).clamp(0.0, 1.0);
            let distance = (x - (ax + dx * t)).hypot(z - (az + dz * t));
            if distance < best.0 {
                best = (distance, (-dx).atan2(-dz));
            }
        }
    }
    best.1
}

/// Whether `point` (`[x, z]`) lies on the strip a plane at `start` facing `yaw` would take off along.
fn on_runway(start: [f32; 3], yaw: f32, length: f32, point: [f32; 3]) -> bool {
    let (sin, cos) = yaw.sin_cos();
    let (dx, dz) = (point[0] - start[0], point[2] - start[2]);
    let along = -sin * dx - cos * dz;
    let across = cos * dx - sin * dz;
    along > -PLANE_HALF_EXTENTS[2] && along < length && across.abs() < PLANE_WING_HALF_EXTENTS[0] + 2.0
}

/// Parks the planes at the spawn points nothing else took, facing along the road whichever way
/// has the longer clear run, longest first. A plane is never parked on another one's runway. If no
/// spot has a full `PLANE_RUNWAY`, the best there is will do. Returns how many were placed.
fn place_planes(world: &mut World, candidates: &[[i32; 2]], used: &[bool]) -> usize {
    // Vehicles already parked are in the way as much as walls are.
    let parked_vehicles: Vec<[f32; 3]> = world.entities.ids::<With<Vehicle>>().into_iter().filter_map(|vid| world.physics.car_pose(vid)).map(|pose| pose.position).collect();
    let clear_of_vehicles = |position: [f32; 3], yaw: f32, run: f32| {
        let (sin, cos) = yaw.sin_cos();
        parked_vehicles
            .iter()
            .filter(|&&other| on_runway(position, yaw, run, other))
            .map(|other| (-sin * (other[0] - position[0]) - cos * (other[2] - position[2]) - 3.0).max(0.0))
            .fold(run, f32::min)
    };
    let mut spots: Vec<(f32, [f32; 3], f32)> = Vec::new();
    for (&[cx, cz], _) in candidates.iter().zip(used).filter(|(_, used)| !**used) {
        let (x, z) = (cx as f32 / 100.0, cz as f32 / 100.0);
        let road = road_heading(&world.map, x, z);
        let position = [x, world.map.ground.height_at(x, z) + PLANE_DROP_Y, z];
        let best = [road, wrap_angle(road + std::f32::consts::PI)]
            .into_iter()
            .filter(|&yaw| world.physics.car_fits(KIND_PLANE, position, yaw))
            .map(|yaw| (clear_of_vehicles(position, yaw, world.physics.runway(&world.map.ground, position, yaw, RUNWAY_SEARCH)), yaw))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((run, yaw)) = best {
            spots.push((run, position, yaw));
        }
    }
    spots.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut parked: Vec<([f32; 3], f32, f32)> = Vec::new();
    for (run, position, yaw) in spots {
        if parked.len() == PLANE_COUNT || world.entities.count::<Vehicle>() >= MAX_VEHICLES {
            break;
        }
        // Only the stretch each needs to get off the ground has to be clear of the other.
        let need = |run: f32| run.min(PLANE_RUNWAY);
        let crossing = parked.iter().any(|&(other, other_yaw, other_run)| on_runway(other, other_yaw, need(other_run), position) || on_runway(position, yaw, need(run), other));
        if crossing {
            continue;
        }
        if run < PLANE_RUNWAY {
            tracing::warn!(run, "no full runway for a plane; parking it on the best there is");
        }
        add_vehicle(world, KIND_PLANE, position, yaw);
        parked.push((position, yaw, run));
    }
    parked.len()
}

pub(super) fn add_vehicle(world: &mut World, kind: u8, position: [f32; 3], yaw: f32) -> u32 {
    let id = world.alloc_id();
    world.physics.add_car(id, kind, position, yaw);
    world.entities.spawn(
        id,
        Vehicle {
            kind,
            seats: [None; CAR_SEATS],
            controls: CarControls::default(),
            home: (position, yaw),
            stuck_ticks: 0,
            hp: vehicle_hp(kind),
            wreck_in: 0,
            last_attacker: None,
            turret_hp: TANK_TURRET_HP,
            damage: [0; 8],
            scrape_in: 0,
        },
    );
    if let Some(mounts) = Mounts::fresh(vehicle_def(kind).mounts) {
        world.entities.insert(id, mounts);
    }
    id
}

/// Horizontal unit vectors `(right, back)` for a heading.
fn axes(yaw: f32) -> ([f32; 2], [f32; 2]) {
    let (sin, cos) = yaw.sin_cos();
    ([cos, -sin], [sin, cos])
}

/// The standing spot next to a car for a player leaving `seat`, or above it when it is boxed in.
fn exit_position(world: &World, kind: u8, pose: &CarPose, seat: usize) -> [f32; 3] {
    let half = half_extents(kind);
    let yaw = pose.yaw();
    let (right, back) = axes(yaw);
    let side = if seat.is_multiple_of(2) { -1.0 } else { 1.0 };
    let reach_side = half[0] + CAPSULE_RADIUS + 0.6;
    let reach_end = half[2] + CAPSULE_RADIUS + 0.6;
    let y = pose.position[1] + 0.3;
    let around = [(side * reach_side, 0.0), (-side * reach_side, 0.0), (0.0, reach_end), (0.0, -reach_end)];
    for (lx, lz) in around {
        let spot = [pose.position[0] + right[0] * lx + back[0] * lz, y, pose.position[2] + right[1] * lx + back[1] * lz];
        if world.physics.character_fits(spot) {
            return spot;
        }
    }
    [pose.position[0], pose.position[1] + 2.5, pose.position[2]]
}

/// Hitches the free mortar within `HITCH_REACH` of the car its driver drives, or lets go of the one that car
/// tows. Both need the car at a near standstill, and a mortar with no one aboard it.
fn hitch(world: &mut World, id: u32) {
    let Some(seat) = world.players.get(&id).and_then(|player| player.seat) else { return };
    let car = seat.vehicle;
    if seat.index != 0 || world.entities.get::<Vehicle>(car).is_none_or(|vehicle| vehicle.wrecked() || !vehicle_def(vehicle.kind).car_wheels) {
        return;
    }
    let Some(pose) = world.physics.car_pose(car) else { return };
    let speed = pose.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
    if speed > HITCH_MAX_SPEED {
        return;
    }
    if let Some(mortar) = world.physics.towing(car) {
        world.physics.unhitch(mortar);
        tracing::debug!(id, car, mortar, "unhitched");
        return;
    }
    let nearest = world.entities.all::<Vehicle>()
        .filter(|(_, vehicle)| vehicle.kind == KIND_MORTAR && !vehicle.wrecked() && vehicle.seats.iter().all(Option::is_none))
        .filter_map(|(mortar, _)| world.physics.tow_reach(car, mortar).map(|reach| (reach, mortar)))
        .filter(|&(reach, mortar)| reach <= HITCH_REACH && world.physics.towed_by(mortar).is_none())
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, mortar)) = nearest {
        world.physics.hitch(car, mortar);
        tracing::debug!(id, car, mortar, "hitched");
    }
}

/// Where a vehicle of `kind` is boarded from: its hull, but a walker's or a hen's from between its feet,
/// a leg's length below its hull, which is out of reach above a stickman's head. Mirrored in `game.ts`.
fn boarding_point(kind: u8, pose: &CarPose) -> [f32; 3] {
    let [x, y, z] = pose.position;
    if matches!(kind, KIND_WALKER | KIND_HEN) { [x, y - WALKER_LEG_LENGTH, z] } else { [x, y, z] }
}

fn board(world: &mut World, id: u32) {
    let Some(player) = world.players.get(&id) else { return };
    let here = player.character.position;
    let mut nearest: Option<(f32, u32, usize)> = None;
    for (vid, vehicle) in world.entities.all::<Vehicle>() {
        // A towed mortar is being pulled, and no one climbs on while it is. A drone is flown from afar.
        if vehicle.wrecked() || world.physics.towed_by(vid).is_some() || is_drone(vehicle.kind) {
            continue;
        }
        let Some(free) = vehicle.seats[..seat_count(vehicle.kind)].iter().position(Option::is_none) else { continue };
        let Some(pose) = world.physics.car_pose(vid) else { continue };
        let distance = boarding_point(vehicle.kind, &pose).iter().zip(here).map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
        if distance <= ENTER_RADIUS && nearest.is_none_or(|(best, ..)| distance < best) {
            nearest = Some((distance, vid, free));
        }
    }
    if let Some((_, vehicle, index)) = nearest {
        seat(world, id, vehicle, index);
    }
}

/// Sits player `id` in seat `index` of `vehicle`, if it is free. A bot of a crew takes its own seat
/// this way, where `BUTTON_USE` would take the first one free.
pub fn seat(world: &mut World, id: u32, vehicle: u32, index: usize) -> bool {
    let Some(mut car) = world.entities.get_mut::<Vehicle>(vehicle) else { return false };
    if index >= seat_count(car.kind) || car.seats[index].is_some() || car.wrecked() {
        return false;
    }
    let Some(player) = world.players.get_mut(&id) else { return false };
    if player.seat.is_some() {
        return false;
    }
    car.seats[index] = Some(id);
    player.seat = Some(Seat { vehicle, index, remote: false });
    player.character.vy = 0.0;
    player.character.ride = 0;
    tracing::debug!(id, vehicle, index, "boarded");
    true
}

/// Parks a fresh tank at `[x, z]` facing `yaw`, unless there are `MAX_VEHICLES` already or it does
/// not fit there.
pub fn add_tank(world: &mut World, [x, z]: [f32; 2], yaw: f32) -> Option<u32> {
    let position = [x, world.map.ground.height_at(x, z) + CAR_DROP_Y, z];
    if world.entities.count::<Vehicle>() >= MAX_VEHICLES || !world.physics.car_fits(KIND_TANK, position, yaw) {
        return None;
    }
    Some(add_vehicle(world, KIND_TANK, position, yaw))
}

/// Takes vehicle `vid` out of the world. Anyone aboard is set down beside it first.
pub fn remove_vehicle(world: &mut World, vid: u32) {
    let aboard: Vec<u32> = world.entities.get::<Vehicle>(vid).map(|vehicle| vehicle.seats.iter().flatten().copied().collect()).unwrap_or_default();
    for id in aboard {
        alight(world, id);
    }
    world.physics.detach(vid);
    world.entities.despawn(vid);
    world.physics.remove_car(vid);
}

/// Empties the player's seat. Returns where they sat.
fn vacate(world: &mut World, id: u32) -> Option<Seat> {
    let seat = world.players.get_mut(&id)?.seat.take()?;
    if let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(seat.vehicle) {
        vehicle.seats[seat.index] = None;
        if seat.index == 0 {
            vehicle.controls = CarControls::default();
        }
    }
    Some(seat)
}

/// Moves a seated player to the nearest free seat of the same vehicle, if there is one. Seat 0
/// drives and a tank's seat 1 works the gun, so a lone crewman can take either job.
fn switch_seat(world: &mut World, id: u32) {
    let Some(seat) = world.players.get(&id).and_then(|player| player.seat) else { return };
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(seat.vehicle) else { return };
    let seats = seat_count(vehicle.kind);
    let Some(free) = (1..seats).map(|step| (seat.index + step) % seats).find(|&index| vehicle.seats[index].is_none()) else { return };
    vehicle.seats[seat.index] = None;
    vehicle.seats[free] = Some(id);
    if let Some(player) = world.players.get_mut(&id) {
        player.seat = Some(Seat { vehicle: seat.vehicle, index: free, remote: false });
    }
    tracing::debug!(id, vehicle = seat.vehicle, from = seat.index, to = free, "switched seat");
}

/// Takes player `id` out of their seat and stands them beside the vehicle.
pub fn alight(world: &mut World, id: u32) {
    let Some(seat) = vacate(world, id) else { return };
    let Some(pose) = world.physics.car_pose(seat.vehicle) else { return };
    let kind = world.entities.get::<Vehicle>(seat.vehicle).map_or(KIND_JEEP, |vehicle| vehicle.kind);
    let position = exit_position(world, kind, &pose, seat.index);
    if let Some(player) = world.players.get_mut(&id) {
        player.character = Character { position, yaw: pose.yaw(), ..Default::default() };
    }
    tracing::debug!(id, vehicle = seat.vehicle, "alighted");
}

/// A car slower than this (m/s) only nudges a tree.
const TREE_RAM_MIN_SPEED: f32 = 4.0;
/// A tank barely has to be moving to push a tree over.
const TANK_TREE_RAM_MIN_SPEED: f32 = 0.3;
/// Tree damage per m/s of speed at the moment of impact: ten m/s fells a sapling, a broad trunk wants a run-up.
const TREE_RAM_DAMAGE_PER_SPEED: f32 = 6.0;
/// A tank's weight against a car's.
const TANK_RAM_FACTOR: f32 = 3.0;
/// A tank's weight against a tree: even a broad trunk goes over in one hit at a walking pace.
const TANK_TREE_FACTOR: f32 = 5.0;
/// Share of its speed a car keeps through a tree it fells, as the trunk gives way.
const CAR_PLOW_KEEP: f32 = 0.6;
/// How far (metres) a tank's hull reaches for a trunk: along its length, and out from each of the three points.
const TANK_PLOW_REACH: f32 = 1.9;
const TANK_PLOW_SPREAD: f32 = 1.8;

/// Fells the trees a car is driving into. Speed is taken from before the step, as the impact
/// itself has already slowed the car by now; a tree that goes down gives some of it back.
/// (A tank never touches a tree: see `plow_trees`.)
fn run_down_trees(world: &mut World, before: &BTreeMap<u32, CarPose>) {
    for (vid, object) in world.physics.car_tree_contacts() {
        let (Some(pose), Some(vehicle)) = (before.get(&vid), world.entities.get::<Vehicle>(vid)) else { continue };
        let speed = pose.velocity[0].hypot(pose.velocity[2]);
        let (kind, wrecked) = (vehicle.kind, vehicle.wrecked());
        if speed < TREE_RAM_MIN_SPEED || wrecked || is_drone(kind) {
            continue;
        }
        let amount = (speed * TREE_RAM_DAMAGE_PER_SPEED) as u16;
        if super::combat::damage_object(world, object, amount, pose.position) && !matches!(kind, KIND_PLANE | KIND_HELI) {
            // Only what the trunk took off the ground speed; falling and climbing are left to the physics.
            let now = world.physics.car_pose(vid).map_or(pose.velocity, |now| now.velocity);
            let kept = [pose.velocity[0] * CAR_PLOW_KEEP, now[1], pose.velocity[2] * CAR_PLOW_KEEP];
            if now[0].hypot(now[2]) < kept[0].hypot(kept[2]) {
                world.physics.set_car_velocity(vid, kept);
            }
        }
    }
}

/// A tank or Strv drives through destructible trunks without being slowed (they are not in what its
/// hull touches, and the client's prediction leaves them out the same way), and fells every one its
/// hull overlaps, hard, in proportion to its speed.
fn plow_trees(world: &mut World, before: &BTreeMap<u32, CarPose>) {
    let tanks: Vec<u32> = world.entities.all::<Vehicle>().filter(|(_, vehicle)| matches!(vehicle.kind, KIND_TANK | KIND_STRV) && !vehicle.wrecked()).map(|(vid, _)| vid).collect();
    for vid in tanks {
        let (Some(was), Some(now)) = (before.get(&vid), world.physics.car_pose(vid)) else { continue };
        let speed = was.velocity[0].hypot(was.velocity[2]);
        if speed < TANK_TREE_RAM_MIN_SPEED {
            continue;
        }
        // The hull is long: look at its middle and a point towards each end.
        let (fx, fz) = (-now.yaw().sin(), -now.yaw().cos());
        let mut trees: Vec<Obj> = [-TANK_PLOW_REACH, 0.0, TANK_PLOW_REACH]
            .into_iter()
            .flat_map(|along| world.physics.objects_near([now.position[0] + fx * along, now.position[1], now.position[2] + fz * along], TANK_PLOW_SPREAD))
            .filter(|object| matches!(object, Obj::Tree(_)))
            .collect();
        trees.dedup();
        let amount = (speed * TREE_RAM_DAMAGE_PER_SPEED * TANK_TREE_FACTOR) as u16;
        for object in trees {
            super::combat::damage_object(world, object, amount, now.position);
        }
    }
}

/// Speed (m/s) a contact must take out of a vehicle in one tick to break a wall.
const RAM_MIN_DV: f32 = 2.5;
/// Speed (m/s) a contact must take out of a vehicle in one tick to hurt it.
const CRASH_MIN_DV: f32 = 5.0;
/// Health lost per m/s over that: a head-on hit into a wall at 25 m/s wrecks a jeep.
const CRASH_HP_PER_DV: f32 = 5.0;
/// Damage to the side that took the hit, and to a wheel right at it, per m/s over the threshold.
const CRASH_DENT_PER_DV: f32 = 12.0;
const CRASH_WHEEL_PER_DV: f32 = 28.0;
/// How near (metres) a hit must be to a wheel to hurt it.
pub const WHEEL_REACH: f32 = 1.8;
/// Wheel damage per point of weapon damage, right on the wheel: a few revolver rounds or two
/// sniper rounds take one off.
const SHOT_WHEEL_PER_DAMAGE: f32 = 1.8;
/// How near a bullet must land to a wheel to hurt it; a blast uses `WHEEL_REACH`.
pub const BULLET_WHEEL_REACH: f32 = 1.0;
/// A tank's armour takes this share of what a car would.
const TANK_CRASH_FACTOR: f32 = 0.2;
/// A plane is wood and canvas: it takes this much more, so flying it into anything at speed is the end of it.
const PLANE_CRASH_FACTOR: f32 = 2.0;
/// A tree gives way, so hitting one hurts half as much.
const TREE_CRASH_FACTOR: f32 = 0.5;
/// A lighter contact than a crash throws sparks if the vehicle is this fast (m/s), at most every few ticks.
const SCRAPE_MIN_DV: f32 = 0.03;
const SCRAPE_SPEED: f32 = 4.0;
const SCRAPE_EVERY: u8 = 4;

/// Whether a vehicle of `kind` has a car's four wheels, which can be bent and lost.
fn has_car_wheels(kind: u8) -> bool {
    vehicle_def(kind).car_wheels
}

/// Where a car's wheels touch, in its own frame (`x`, `z`): front left, front right, rear left, rear right.
fn wheel_spots(kind: u8) -> [[f32; 2]; 4] {
    let VehicleDef { track_half: track, axle_z: axle, .. } = *vehicle_def(kind);
    [[-track, -axle], [track, -axle], [-track, axle], [track, axle]]
}

/// Adds the damage of a hit `over` m/s past the threshold at `local` (the vehicle's frame) to
/// `damage`: mostly to the side it came from, shared at a corner, and to any wheel near it.
fn dent(kind: u8, damage: &mut [u8; 8], local: [f32; 3], over: f32) {
    let [hx, _, hz] = half_extents(kind);
    let (nx, nz) = (local[0] / hx, local[2] / hz);
    let sides = [(DAMAGE_FRONT, -nz), (DAMAGE_REAR, nz), (DAMAGE_LEFT, -nx), (DAMAGE_RIGHT, nx)].map(|(side, toward)| (side, toward.max(0.0).powi(2)));
    let total: f32 = sides.iter().map(|(_, weight)| weight).sum();
    let add = |value: &mut u8, amount: f32| *value = (f32::from(*value) + amount).round().min(255.0) as u8;
    if total > 0.0 {
        for (side, weight) in sides {
            add(&mut damage[side], over * CRASH_DENT_PER_DV * weight / total);
        }
    }
    if !has_car_wheels(kind) {
        return;
    }
    hurt_wheels(kind, damage, local, over * CRASH_WHEEL_PER_DV, WHEEL_REACH);
}

/// Adds up to `amount` damage to each wheel within `reach` metres of `local` (the vehicle's frame),
/// scaled by how close it is. A tank, a helicopter and a plane have no wheels to lose.
fn hurt_wheels(kind: u8, damage: &mut [u8; 8], local: [f32; 3], amount: f32, reach: f32) {
    if !has_car_wheels(kind) {
        return;
    }
    for (wheel, [x, z]) in wheel_spots(kind).into_iter().enumerate() {
        let near = 1.0 - (local[0] - x).hypot(local[2] - z) / reach;
        if near > 0.0 {
            let value = &mut damage[DAMAGE_WHEELS + wheel];
            *value = (f32::from(*value) + amount * near).round().min(255.0) as u8;
        }
    }
}

/// Weapons fire at vehicle `vid` that hit at `point` (world) for `damage` points of weapon damage:
/// bends or knocks off a wheel it landed near. The hull's own health is handled by the caller.
pub fn shoot_wheels(world: &mut World, vid: u32, point: [f32; 3], damage: f32, reach: f32) {
    let Some(pose) = world.physics.car_pose(vid) else { return };
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return };
    if vehicle.wrecked() {
        return;
    }
    let offset = std::array::from_fn(|i| point[i] - pose.position[i]);
    hurt_wheels(vehicle.kind, &mut vehicle.damage, pose.local_dir(offset), damage * SHOT_WHEEL_PER_DAMAGE, reach);
}

/// Hurts every vehicle that hit something hard in the last step, and makes sparks fly from those
/// grinding along something. Each vehicle counts its hardest contact.
fn crash(world: &mut World, before: &BTreeMap<u32, CarPose>) {
    let mut hardest: BTreeMap<u32, CarImpact> = BTreeMap::new();
    for impact in world.physics.car_impacts() {
        if hardest.get(&impact.car).is_none_or(|known| impact.dv > known.dv) {
            hardest.insert(impact.car, impact);
        }
    }
    for vid in world.entities.ids::<With<Vehicle>>() {
        let mut vehicle = world.entities.of_mut::<Vehicle>(vid);
        vehicle.scrape_in = vehicle.scrape_in.saturating_sub(1);
    }
    // Every wall it hit gives way, not just the hardest contact.
    let rams: Vec<_> = world.physics.car_impacts().into_iter().filter(|i| i.dv > RAM_MIN_DV).collect();
    for impact in rams {
        let Some(Obj::Building(id)) = impact.object else { continue };
        let Some(vehicle) = world.entities.get::<Vehicle>(impact.car) else { continue };
        if vehicle.wrecked() || is_drone(vehicle.kind) {
            continue;
        }
        let weight = if matches!(vehicle.kind, KIND_TANK | KIND_STRV) { TANK_RAM_FACTOR } else { 1.0 };
        super::combat::ram_building(world, id, impact.point, impact.dv, weight);
    }
    for (vid, impact) in hardest {
        let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { continue };
        // A drone's own collisions are what set it off (see `Drones`).
        if vehicle.wrecked() || is_drone(vehicle.kind) {
            continue;
        }
        let [x, y, z] = impact.point.map(to_cm);
        let mut over = impact.dv - CRASH_MIN_DV;
        if over <= 0.0 {
            let speed = before.get(&vid).map_or(0.0, |pose| pose.velocity.iter().map(|v| v * v).sum::<f32>().sqrt());
            if impact.dv > SCRAPE_MIN_DV && speed > SCRAPE_SPEED && vehicle.scrape_in == 0 {
                vehicle.scrape_in = SCRAPE_EVERY;
                world.shots.push(ShotEvent { shooter: vid, weapon: WEAPON_SCRAPE, x, y, z });
            }
            continue;
        }
        if matches!(vehicle.kind, KIND_TANK | KIND_STRV) {
            over *= TANK_CRASH_FACTOR;
        }
        if vehicle.kind == KIND_PLANE {
            over *= PLANE_CRASH_FACTOR;
        }
        if matches!(impact.object, Some(Obj::Tree(_))) {
            over *= TREE_CRASH_FACTOR;
        }
        dent(vehicle.kind, &mut vehicle.damage, impact.local, over);
        // Whoever drove the other vehicle into it is to blame, should it go up.
        let attacker = impact.other_car.and_then(|other| world.entities.get::<Vehicle>(other)?.seats[0]);
        world.shots.push(ShotEvent { shooter: vid, weapon: WEAPON_CRASH, x, y, z });
        super::combat::damage_vehicle(world, vid, (over * CRASH_HP_PER_DV).round().min(255.0) as u8, attacker);
    }
}

/// Throws every standing player a car overlaps. The direction leans on the car's heading with a
/// little outward push, so a car drives a player ahead of it and off to the side.
fn knock_players(world: &mut World) {
    let mut run_down = Vec::new();
    for (&id, player) in world.players.iter_mut() {
        player.hit_cooldown = player.hit_cooldown.saturating_sub(1);
        if !player.embodied() || !player.alive() || player.character.ride != 0 || player.hit_cooldown > 0 {
            continue;
        }
        let Some(hit) = world.physics.car_hit(player.character.position) else { continue };
        // A drone is too light to throw anyone; it goes off instead (see `Drones`).
        if world.entities.get::<Vehicle>(hit.car).is_some_and(|vehicle| is_drone(vehicle.kind)) {
            continue;
        }
        let speed = hit.velocity[0].hypot(hit.velocity[2]);
        let outward = [player.character.position[0] - hit.position[0], player.character.position[2] - hit.position[2]];
        let unit = |v: [f32; 2]| {
            let length = v[0].hypot(v[1]);
            if length < 1e-4 { [0.0, 0.0] } else { [v[0] / length, v[1] / length] }
        };
        let (outward, ahead) = (unit(outward), unit([hit.velocity[0], hit.velocity[2]]));
        let direction = if speed > 1.0 { unit([ahead[0] * 0.75 + outward[0] * 0.25, ahead[1] * 0.75 + outward[1] * 0.25]) } else { outward };
        let power = (speed * KNOCK_SPEED_FACTOR).clamp(KNOCK_MIN_PUSH, KNOCK_MAX_SPEED);
        let character = &mut player.character;
        character.knock = [direction[0] * power, direction[1] * power];
        character.vy = (KNOCK_HOP + KNOCK_HOP_PER_SPEED * speed).min(KNOCK_HOP_MAX);
        character.grounded = false;
        player.hit_cooldown = HIT_COOLDOWN;
        tracing::debug!(player = ?player.character.position, car = hit.car, speed, "player knocked");
        if speed > ROADKILL_MIN_SPEED {
            let amount = ((speed - ROADKILL_MIN_SPEED) * ROADKILL_DAMAGE_PER_SPEED).min(f32::from(PLAYER_MAX_HP)).round() as u8;
            run_down.push((id, amount, hit.car, hit.position));
        }
    }
    // Whoever is driving is to blame.
    for (id, amount, car, at) in run_down {
        let driver = world.entities.get::<Vehicle>(car).and_then(|vehicle| vehicle.seats[0]);
        super::combat::damage_from(world, id, amount, driver, WEAPON_ROADKILL, Some(at));
    }
}

impl Mechanic for Vehicles {
    fn on_start(&mut self, world: &mut World) {
        // Vehicles placed in the map editor replace the scatter over the spawn points.
        if !world.map.placed_vehicles.is_empty() {
            let mut placed = 0;
            for spot in world.map.placed_vehicles.clone() {
                let (x, z) = (spot.at[0] as f32 / 100.0, spot.at[1] as f32 / 100.0);
                let position = [x, world.map.ground.height_at(x, z) + drop_height(spot.kind), z];
                if world.entities.count::<Vehicle>() >= MAX_VEHICLES {
                    tracing::warn!(kind = spot.kind, x, z, max = MAX_VEHICLES, "too many placed vehicles; skipped");
                } else if world.physics.car_fits(spot.kind, position, spot.yaw) {
                    add_vehicle(world, spot.kind, position, spot.yaw);
                    placed += 1;
                } else {
                    tracing::warn!(kind = spot.kind, x, z, "placed vehicle does not fit; skipped");
                }
            }
            tracing::info!(vehicles = placed, "vehicles placed from the editor");
            return;
        }
        let spawns = world.map.spawns.clone();
        // Prefer spawn points players do not start on, then fall back to those.
        let split = PLAYER_SPAWNS.min(spawns.len());
        let candidates: Vec<[i32; 2]> = spawns[split..].iter().chain(spawns[..split].iter().rev()).copied().collect();
        let mut used = vec![false; candidates.len()];
        let mut placed = 0;
        for (index, &[cx, cz]) in candidates.iter().enumerate() {
            const CARS: usize = CAR_COUNT + ROADSTER_COUNT;
            if placed == CARS + TANK_COUNT + STRV_COUNT + HELI_COUNT + WALKER_COUNT {
                break;
            }
            // Cars first (jeeps and sports cars take turns, then the Roadster), then the tanks, then the helicopter.
            let kind = match placed {
                0..CAR_COUNT if placed % 2 == 0 => KIND_JEEP,
                0..CAR_COUNT => KIND_SPORTS,
                _ if placed < CARS => KIND_ROADSTER,
                _ if placed < CARS + TANK_COUNT => KIND_TANK,
                _ if placed < CARS + TANK_COUNT + STRV_COUNT => KIND_STRV,
                _ if placed < CARS + TANK_COUNT + STRV_COUNT + HELI_COUNT => KIND_HELI,
                _ => KIND_WALKER,
            };
            let (x, z) = (cx as f32 / 100.0, cz as f32 / 100.0);
            let yaw = road_heading(&world.map, x, z);
            let position = [x, world.map.ground.height_at(x, z) + drop_height(kind), z];
            if world.physics.car_fits(kind, position, yaw) && world.entities.count::<Vehicle>() < MAX_VEHICLES {
                add_vehicle(world, kind, position, yaw);
                used[index] = true;
                placed += 1;
            }
        }
        placed += place_planes(world, &candidates, &used);
        // The mortars, then the hens, take whichever spawn points the planes left, so they never push a
        // plane off one.
        let runways: Vec<CarPose> = world.entities.all::<Vehicle>().filter(|(_, v)| v.kind == KIND_PLANE).filter_map(|(id, _)| world.physics.car_pose(id)).collect();
        for (kind, count) in [(KIND_MORTAR, MORTAR_COUNT), (KIND_HEN, HEN_COUNT)] {
            let mut added = 0;
            for (index, &[cx, cz]) in candidates.iter().enumerate() {
                if added == count {
                    break;
                }
                if used[index] {
                    continue;
                }
                let (x, z) = (cx as f32 / 100.0, cz as f32 / 100.0);
                let yaw = road_heading(&world.map, x, z);
                let position = [x, world.map.ground.height_at(x, z) + drop_height(kind), z];
                // Not on a plane's runway either: a plane needs the strip clear to take off.
                let on_a_runway = runways.iter().any(|plane| on_runway(plane.position, plane.yaw(), PLANE_RUNWAY, position));
                if !on_a_runway && world.physics.car_fits(kind, position, yaw) && world.entities.count::<Vehicle>() < MAX_VEHICLES {
                    add_vehicle(world, kind, position, yaw);
                    used[index] = true;
                    added += 1;
                    placed += 1;
                }
            }
        }
        tracing::info!(vehicles = placed, "vehicles placed");
    }

    fn on_leave(&mut self, world: &mut World, id: u32) {
        vacate(world, id);
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        // Boarding and leaving.
        let pressed: Vec<u32> =
            world.players.iter().filter(|(_, player)| player.alive() && player.input.buttons() & BUTTON_USE != 0).map(|(&id, _)| id).collect();
        for id in pressed {
            if world.players[&id].seat.is_some() {
                alight(world, id);
            } else {
                board(world, id);
            }
        }
        // A driver hitches or unhitches the mortar behind the car.
        let hitching: Vec<u32> = world.players.iter().filter(|(_, player)| player.alive() && player.input.buttons() & BUTTON_HITCH != 0).map(|(&id, _)| id).collect();
        for id in hitching {
            hitch(world, id);
        }
        // Reload moves to the other seat, unless it is a passenger's to reload their own gun with.
        let switching: Vec<u32> = world
            .players
            .iter()
            .filter(|&(&id, player)| {
                player.alive() && player.seat.is_some() && player.input.buttons() & (BUTTON_RELOAD | BUTTON_USE) == BUTTON_RELOAD && !super::combat::reloads_in_seat(world, id)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in switching {
            switch_seat(world, id);
        }

        // The driver's input becomes the car's controls; an empty car holds its handbrake.
        for vid in world.entities.ids::<With<Vehicle>>() {
            let mut vehicle = world.entities.of_mut::<Vehicle>(vid);
            let towed = world.physics.towed_by(vid).is_some();
            vehicle.controls = if vehicle.kind == KIND_MORTAR {
                // Nothing drives a mortar: it rolls when towed, and holds its brake when it is not.
                CarControls { handbrake: !towed, ..Default::default() }
            } else {
                match vehicle.seats[0].and_then(|driver| world.players.get(&driver)) {
                    Some(driver) => CarControls::driven(vehicle.kind, &driver.input),
                    None => CarControls { handbrake: true, ..Default::default() },
                }
            };
            world.physics.set_car_condition(vid, CarCondition::of(vehicle.kind, vehicle.hp, vehicle.damage));
            world.physics.drive_car(vid, vehicle.controls, dt);
        }
        let before: BTreeMap<u32, CarPose> = world.entities.ids::<With<Vehicle>>().into_iter().filter_map(|vid| Some((vid, world.physics.car_pose(vid)?))).collect();
        world.physics.step_cars(dt);
        // Riders keep their place on the roof as it moves.
        for player in world.players.values_mut().filter(|player| player.seat.is_none() && player.character.ride != 0) {
            let ride = player.character.ride;
            if let (Some(old), Some(new)) = (before.get(&ride), world.physics.car_pose(ride)) {
                player.character.position = old.carry(&new, player.character.position);
            }
        }
        knock_players(world);
        run_down_trees(world, &before);
        plow_trees(world, &before);
        crash(world, &before);

        // Riders travel with their car; stuck or fallen cars are put right.
        for vid in world.entities.ids::<With<Vehicle>>() {
            let mut vehicle = world.entities.of_mut::<Vehicle>(vid);
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            // Put back as new: its weapons are restocked once it is let go of.
            let mut fresh = false;
            let [x, y, z] = pose.position;
            let slow = pose.forward_speed.abs() < 1.5;
            vehicle.stuck_ticks = if pose.up_y < 0.2 && slow { vehicle.stuck_ticks + 1 } else { 0 };
            if vehicle.wrecked() {
                // A wreck lets go of whatever it tows, and whatever tows it.
                world.physics.detach(vid);
                // A wreck burns where it lies, then the room puts a fresh one back.
                vehicle.wreck_in = vehicle.wreck_in.saturating_sub(1);
                if vehicle.wreck_in == 0 {
                    vehicle.hp = vehicle_hp(vehicle.kind);
                    vehicle.damage = [0; 8];
                    vehicle.turret_hp = TANK_TURRET_HP;
                    vehicle.last_attacker = None;
                    fresh = true;
                    world.physics.reset_car(vid, vehicle.home.0, vehicle.home.1);
                }
            } else if y < KILL_Y {
                // Fallen out of the world: a fresh one at home.
                vehicle.hp = vehicle_hp(vehicle.kind);
                vehicle.damage = [0; 8];
                vehicle.turret_hp = TANK_TURRET_HP;
                fresh = true;
                world.physics.reset_car(vid, vehicle.home.0, vehicle.home.1);
            } else if vehicle.stuck_ticks >= UNFLIP_TICKS {
                // A walker or a hen is put back on its feet: its hull is as high as its legs, and its leg
                // boxes would be buried if it were set down a car's height above where it lies.
                let rise = if matches!(vehicle.kind, KIND_WALKER | KIND_HEN) {
                    world.map.ground.height_at(x, z) + drop_height(vehicle.kind) - y
                } else {
                    1.2
                };
                world.physics.reset_car(vid, [x, y + rise, z], pose.yaw());
                vehicle.stuck_ticks = 0;
            }
            for rider in vehicle.seats.iter().flatten() {
                // A drone's pilot stays where they stood.
                if is_drone(vehicle.kind) {
                    break;
                }
                if let Some(player) = world.players.get_mut(rider) {
                    player.character.position = [x, y, z];
                    player.character.yaw = pose.yaw();
                    player.character.grounded = true;
                }
            }
            if fresh && let Some(mut mounts) = world.entities.get_mut::<Mounts>(vid) {
                mounts.restock();
            }
        }
    }
}

#[cfg(test)]
mod tests;
