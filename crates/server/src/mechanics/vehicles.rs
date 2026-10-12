use super::Mechanic;
use crate::game::{Gun, PlaneArms, Seat, Vehicle, World};
use crate::physics::{CarCondition, CarControls, CarImpact, CarPose, Character, Obj, half_extents};
use shared::consts::*;
use shared::vehicles::{VehicleDef, vehicle_def};
use shared::protocol::{
    DAMAGE_FRONT, DAMAGE_LEFT, DAMAGE_REAR, DAMAGE_RIGHT, DAMAGE_WHEELS, KIND_HELI, KIND_JEEP, KIND_PLANE, KIND_SPORTS, KIND_TANK, ShotEvent, WEAPON_CRASH,
    WEAPON_ROADKILL, WEAPON_SCRAPE, seat_count, vehicle_hp,
};
use shared::quant::{from_axis, to_cm, wrap_angle};
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
    let parked_vehicles: Vec<[f32; 3]> = world.vehicles.keys().filter_map(|&vid| world.physics.car_pose(vid)).map(|pose| pose.position).collect();
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
        if parked.len() == PLANE_COUNT || world.vehicles.len() >= MAX_VEHICLES {
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

fn add_vehicle(world: &mut World, kind: u8, position: [f32; 3], yaw: f32) -> u32 {
    let id = world.alloc_id();
    world.physics.add_car(id, kind, position, yaw);
    world.vehicles.insert(
        id,
        Vehicle {
            kind,
            seats: [None; CAR_SEATS],
            controls: CarControls::default(),
            home: (position, yaw),
            stuck_ticks: 0,
            gun: vehicle_def(kind).gun.then(Gun::default),
            arms: vehicle_def(kind).arms.then(PlaneArms::default),
            hp: vehicle_hp(kind),
            wreck_in: 0,
            last_attacker: None,
            damage: [0; 8],
            scrape_in: 0,
        },
    );
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

fn board(world: &mut World, id: u32) {
    let Some(player) = world.players.get(&id) else { return };
    let here = player.character.position;
    let mut nearest: Option<(f32, u32, usize)> = None;
    for (&vid, vehicle) in &world.vehicles {
        if vehicle.wrecked() {
            continue;
        }
        let Some(free) = vehicle.seats[..seat_count(vehicle.kind)].iter().position(Option::is_none) else { continue };
        let Some(pose) = world.physics.car_pose(vid) else { continue };
        let distance = pose.position.iter().zip(here).map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
        if distance <= ENTER_RADIUS && nearest.is_none_or(|(best, ..)| distance < best) {
            nearest = Some((distance, vid, free));
        }
    }
    if let Some((_, vehicle, index)) = nearest {
        world.vehicles.get_mut(&vehicle).expect("found above").seats[index] = Some(id);
        let player = world.players.get_mut(&id).expect("checked above");
        player.seat = Some(Seat { vehicle, index });
        player.character.vy = 0.0;
        player.character.ride = 0;
        tracing::debug!(id, vehicle, index, "boarded");
    }
}

/// Empties the player's seat. Returns where they sat.
fn vacate(world: &mut World, id: u32) -> Option<Seat> {
    let seat = world.players.get_mut(&id)?.seat.take()?;
    if let Some(vehicle) = world.vehicles.get_mut(&seat.vehicle) {
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
    let Some(vehicle) = world.vehicles.get_mut(&seat.vehicle) else { return };
    let seats = seat_count(vehicle.kind);
    let Some(free) = (1..seats).map(|step| (seat.index + step) % seats).find(|&index| vehicle.seats[index].is_none()) else { return };
    vehicle.seats[seat.index] = None;
    vehicle.seats[free] = Some(id);
    if let Some(player) = world.players.get_mut(&id) {
        player.seat = Some(Seat { vehicle: seat.vehicle, index: free });
    }
    tracing::debug!(id, vehicle = seat.vehicle, from = seat.index, to = free, "switched seat");
}

fn alight(world: &mut World, id: u32) {
    let Some(seat) = vacate(world, id) else { return };
    let Some(pose) = world.physics.car_pose(seat.vehicle) else { return };
    let kind = world.vehicles.get(&seat.vehicle).map_or(KIND_JEEP, |vehicle| vehicle.kind);
    let position = exit_position(world, kind, &pose, seat.index);
    if let Some(player) = world.players.get_mut(&id) {
        player.character = Character { position, yaw: pose.yaw(), ..Default::default() };
    }
    tracing::debug!(id, vehicle = seat.vehicle, "alighted");
}

/// A car slower than this (m/s) only nudges a tree.
const TREE_RAM_MIN_SPEED: f32 = 4.0;
/// Tree damage per m/s of speed at the moment of impact: ten m/s fells a sapling, a broad trunk wants a run-up.
const TREE_RAM_DAMAGE_PER_SPEED: f32 = 6.0;
/// A tank's weight against a car's.
const TANK_RAM_FACTOR: f32 = 3.0;

/// Fells the trees a car is driving into. Speed is taken from before the step, as the impact
/// itself has already slowed the car by now.
fn run_down_trees(world: &mut World, before: &BTreeMap<u32, CarPose>) {
    for (vid, object) in world.physics.car_tree_contacts() {
        let (Some(pose), Some(vehicle)) = (before.get(&vid), world.vehicles.get(&vid)) else { continue };
        let speed = pose.velocity[0].hypot(pose.velocity[2]);
        if speed < TREE_RAM_MIN_SPEED || vehicle.wrecked() {
            continue;
        }
        let weight = if vehicle.kind == KIND_TANK { TANK_RAM_FACTOR } else { 1.0 };
        let amount = (speed * TREE_RAM_DAMAGE_PER_SPEED * weight) as u16;
        super::combat::damage_object(world, object, amount, pose.position);
    }
}

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
    let Some(vehicle) = world.vehicles.get_mut(&vid) else { return };
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
    for vehicle in world.vehicles.values_mut() {
        vehicle.scrape_in = vehicle.scrape_in.saturating_sub(1);
    }
    for (vid, impact) in hardest {
        let Some(vehicle) = world.vehicles.get_mut(&vid) else { continue };
        if vehicle.wrecked() {
            continue;
        }
        let [x, y, z] = impact.point.map(to_cm);
        let mut over = impact.dv - CRASH_MIN_DV;
        if over <= 0.0 {
            let speed = before.get(&vid).map_or(0.0, |pose| pose.velocity.iter().map(|v| v * v).sum::<f32>().sqrt());
            if impact.dv > SCRAPE_MIN_DV && speed > SCRAPE_SPEED && vehicle.scrape_in == 0 {
                vehicle.scrape_in = SCRAPE_EVERY;
                world.shots.push(ShotEvent(vid, WEAPON_SCRAPE, x, y, z));
            }
            continue;
        }
        if vehicle.kind == KIND_TANK {
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
        let attacker = impact.other_car.and_then(|other| world.vehicles.get(&other)?.seats[0]);
        world.shots.push(ShotEvent(vid, WEAPON_CRASH, x, y, z));
        super::combat::damage_vehicle(world, vid, (over * CRASH_HP_PER_DV).round().min(255.0) as u8, attacker);
    }
}

/// Throws every standing player a car overlaps. The direction leans on the car's heading with a
/// little outward push, so a car drives a player ahead of it and off to the side.
fn knock_players(world: &mut World) {
    let mut run_down = Vec::new();
    for (&id, player) in world.players.iter_mut() {
        player.hit_cooldown = player.hit_cooldown.saturating_sub(1);
        if player.seat.is_some() || !player.alive() || player.character.ride != 0 || player.hit_cooldown > 0 {
            continue;
        }
        let Some(hit) = world.physics.car_hit(player.character.position) else { continue };
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
        let driver = world.vehicles.get(&car).and_then(|vehicle| vehicle.seats[0]);
        super::combat::damage_from(world, id, amount, driver, WEAPON_ROADKILL, Some(at));
    }
}

impl Mechanic for Vehicles {
    fn on_start(&mut self, world: &mut World) {
        let spawns = world.map.spawns.clone();
        // Prefer spawn points players do not start on, then fall back to those.
        let split = PLAYER_SPAWNS.min(spawns.len());
        let candidates: Vec<[i32; 2]> = spawns[split..].iter().chain(spawns[..split].iter().rev()).copied().collect();
        let mut used = vec![false; candidates.len()];
        let mut placed = 0;
        for (index, &[cx, cz]) in candidates.iter().enumerate() {
            if placed == CAR_COUNT + TANK_COUNT + HELI_COUNT {
                break;
            }
            // Cars first (jeeps and sports cars take turns), then the tanks, then the helicopter.
            let kind = match placed {
                0..CAR_COUNT if placed % 2 == 0 => KIND_JEEP,
                0..CAR_COUNT => KIND_SPORTS,
                _ if placed < CAR_COUNT + TANK_COUNT => KIND_TANK,
                _ => KIND_HELI,
            };
            let (x, z) = (cx as f32 / 100.0, cz as f32 / 100.0);
            let yaw = road_heading(&world.map, x, z);
            let position = [x, world.map.ground.height_at(x, z) + CAR_DROP_Y, z];
            if world.physics.car_fits(kind, position, yaw) && world.vehicles.len() < MAX_VEHICLES {
                add_vehicle(world, kind, position, yaw);
                used[index] = true;
                placed += 1;
            }
        }
        placed += place_planes(world, &candidates, &used);
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
        // Reload has no use inside a vehicle, so it moves to the other seat.
        let switching: Vec<u32> = world
            .players
            .iter()
            .filter(|(_, player)| {
                player.alive() && player.seat.is_some() && player.input.buttons() & (BUTTON_RELOAD | BUTTON_USE) == BUTTON_RELOAD
            })
            .map(|(&id, _)| id)
            .collect();
        for id in switching {
            switch_seat(world, id);
        }

        // The driver's input becomes the car's controls; an empty car holds its handbrake.
        for (&vid, vehicle) in &mut world.vehicles {
            vehicle.controls = match vehicle.seats[0].and_then(|driver| world.players.get(&driver)) {
                Some(driver) => {
                    let input = &driver.input;
                    let buttons = input.buttons();
                    CarControls {
                        steer: from_axis(input.1),
                        throttle: from_axis(input.2),
                        handbrake: buttons & BUTTON_JUMP != 0,
                        // A helicopter climbs on jump and sinks on run.
                        lift: f32::from(buttons & BUTTON_JUMP != 0) - f32::from(buttons & BUTTON_RUN != 0),
                        powered: true,
                        // A plane flies towards where its pilot looks.
                        aim: (vehicle.kind == KIND_PLANE).then(|| [shared::quant::from_mrad(input.3), shared::quant::from_mrad(input.pitch())]),
                    }
                }
                None => CarControls { handbrake: true, ..Default::default() },
            };
            world.physics.set_car_condition(vid, CarCondition::of(vehicle.kind, vehicle.hp, vehicle.damage));
            world.physics.drive_car(vid, vehicle.controls, dt);
        }
        let before: BTreeMap<u32, CarPose> = world.vehicles.keys().filter_map(|&vid| Some((vid, world.physics.car_pose(vid)?))).collect();
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
        crash(world, &before);

        // Riders travel with their car; stuck or fallen cars are put right.
        for (&vid, vehicle) in &mut world.vehicles {
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            let [x, y, z] = pose.position;
            let slow = pose.forward_speed.abs() < 1.5;
            vehicle.stuck_ticks = if pose.up_y < 0.2 && slow { vehicle.stuck_ticks + 1 } else { 0 };
            if vehicle.wrecked() {
                // A wreck burns where it lies, then the room puts a fresh one back.
                vehicle.wreck_in = vehicle.wreck_in.saturating_sub(1);
                if vehicle.wreck_in == 0 {
                    vehicle.hp = vehicle_hp(vehicle.kind);
                    vehicle.damage = [0; 8];
                    vehicle.last_attacker = None;
                    vehicle.arms = vehicle.arms.map(|_| PlaneArms::default());
                    world.physics.reset_car(vid, vehicle.home.0, vehicle.home.1);
                }
            } else if y < KILL_Y {
                // Fallen out of the world: a fresh one at home.
                vehicle.hp = vehicle_hp(vehicle.kind);
                vehicle.damage = [0; 8];
                vehicle.arms = vehicle.arms.map(|_| PlaneArms::default());
                world.physics.reset_car(vid, vehicle.home.0, vehicle.home.1);
            } else if vehicle.stuck_ticks >= UNFLIP_TICKS {
                world.physics.reset_car(vid, [x, y + 1.2, z], pose.yaw());
                vehicle.stuck_ticks = 0;
            }
            for rider in vehicle.seats.iter().flatten() {
                if let Some(player) = world.players.get_mut(rider) {
                    player.character.position = [x, y, z];
                    player.character.yaw = pose.yaw();
                    player.character.grounded = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Command, Game, Outgoing};
    use crate::physics::STANDING_Y;
    use shared::protocol::InputFrame;
    use shared::world::{Road, WORLD_VERSION};
    use tokio::sync::{mpsc, oneshot};

    /// A flat world with one road along X, and spawn points on it.
    fn game() -> Game {
        let map = WorldFile {
            version: WORLD_VERSION,
            half_extent: 30_000,
            // Nine spots in the middle for the cars, tanks and helicopter, and one far out at each end
            // for the planes. Players start on the first six.
            spawns: [[-25_000, 0], [25_000, 0]].into_iter().chain((0..9).map(|i| [i * 3000 - 12_000, 0])).collect(),
            roads: vec![Road { width: 800, kind: 0, points: vec![[-30_000, 0], [30_000, 0]] }],
            ..Default::default()
        };
        // Loose items would take a test's use button.
        let mut game = Game::new(map, 0).unwrap();
        game.world.pickups.clear();
        game
    }

    fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply });
        (response.try_recv().unwrap().unwrap(), inbox)
    }

    /// Feeds one frame per tick, with the frame counter owned by the caller.
    fn press(game: &mut Game, id: u32, seq: &mut u32, axes: (i8, i8), buttons: u8, ticks: u32) {
        for _ in 0..ticks {
            *seq += 1;
            game.handle(Command::Input(id, 0, vec![InputFrame(*seq, axes.0, axes.1, 0, buttons, 0, 0, 0)]));
            game.step();
        }
    }

    /// Settles the jitter buffer: two frames must queue before any is consumed.
    fn idle(game: &mut Game, id: u32, seq: &mut u32, ticks: u32) {
        press(game, id, seq, (0, 0), 0, ticks);
    }

    fn move_player_next_to_a_car(game: &mut Game, id: u32) -> u32 {
        let (&vid, _) = game.world.vehicles.iter().next().unwrap();
        let pose = game.world.physics.car_pose(vid).unwrap();
        let player = game.world.players.get_mut(&id).unwrap();
        player.character.position = [pose.position[0] + 2.0, STANDING_Y, pose.position[2]];
        vid
    }

    #[test]
    fn cars_are_placed_along_the_road() {
        let game = game();
        assert_eq!(game.world.vehicles.len(), CAR_COUNT + TANK_COUNT + HELI_COUNT + PLANE_COUNT);
        let planes = game.world.vehicles.values().filter(|vehicle| vehicle.kind == KIND_PLANE).count();
        assert_eq!(planes, PLANE_COUNT);
        let tanks = game.world.vehicles.values().filter(|vehicle| vehicle.kind == KIND_TANK).count();
        assert_eq!(tanks, TANK_COUNT);
        let helis = game.world.vehicles.values().filter(|vehicle| vehicle.kind == KIND_HELI).count();
        assert_eq!(helis, HELI_COUNT);
        let count = |kind| game.world.vehicles.values().filter(|vehicle| vehicle.kind == kind).count();
        assert_eq!(count(KIND_JEEP), CAR_COUNT / 2, "half the cars are jeeps");
        assert_eq!(count(KIND_SPORTS), CAR_COUNT / 2, "and half are sports cars");
        for &vid in game.world.vehicles.keys() {
            let pose = game.world.physics.car_pose(vid).unwrap();
            // The road runs along X, so the car faces +X or -X: |yaw| is a quarter turn.
            assert!((pose.yaw().abs() - std::f32::consts::FRAC_PI_2).abs() < 0.05, "{}", pose.yaw());
        }
    }

    #[test]
    fn use_boards_the_nearest_car_and_use_again_leaves() {
        let mut game = game();
        let (id, _inbox) = join(&mut game);
        let mut seq = 0;
        idle(&mut game, id, &mut seq, 30);
        let vid = move_player_next_to_a_car(&mut game, id);

        press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, id, &mut seq, 3);
        assert_eq!(game.world.vehicles[&vid].seats[0], Some(id));
        assert!(game.entity_states().iter().all(|e| e.1 != shared::protocol::KIND_STICKMAN), "a seated player is not a stickman entity");
        assert_eq!(game.vehicle_states().iter().find(|v| v.0 == vid).unwrap().9[0], id);

        press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, id, &mut seq, 3);
        assert_eq!(game.world.vehicles[&vid].seats, [None; CAR_SEATS]);
        let states: Vec<_> = game.entity_states().into_iter().filter(|e| e.1 == shared::protocol::KIND_STICKMAN).collect();
        assert_eq!(states.len(), 1);
        // The stickman stands beside the car, not inside it.
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (dx, dz) = (states[0].2 as f32 / 100.0 - pose.position[0], states[0].4 as f32 / 100.0 - pose.position[2]);
        assert!(dx.hypot(dz) > 1.5, "{dx} {dz}");
    }

    #[test]
    fn a_player_out_of_reach_does_not_board() {
        let mut game = game();
        let (id, _inbox) = join(&mut game);
        let mut seq = 0;
        idle(&mut game, id, &mut seq, 30);
        game.world.players.get_mut(&id).unwrap().character.position = [0.0, STANDING_Y, 15.0];
        press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, id, &mut seq, 3);
        assert!(game.world.players[&id].seat.is_none());
    }

    #[test]
    fn the_driver_steers_and_passengers_ride_along() {
        let mut game = game();
        let (driver, _a) = join(&mut game);
        let (rider, _b) = join(&mut game);
        let (mut seq_a, mut seq_b) = (0, 0);
        idle(&mut game, driver, &mut seq_a, 3);
        idle(&mut game, rider, &mut seq_b, 3);
        let vid = move_player_next_to_a_car(&mut game, driver);
        move_player_next_to_a_car(&mut game, rider);

        // Drivers board first so they take seat 0.
        press(&mut game, driver, &mut seq_a, (0, 0), BUTTON_USE, 1);
        idle(&mut game, driver, &mut seq_a, 3);
        press(&mut game, rider, &mut seq_b, (0, 0), BUTTON_USE, 1);
        idle(&mut game, rider, &mut seq_b, 3);
        assert_eq!(game.world.vehicles[&vid].seats[..2], [Some(driver), Some(rider)]);

        let start = game.world.physics.car_pose(vid).unwrap().position;
        for _ in 0..180 {
            seq_a += 1;
            seq_b += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
            // A passenger's stick does nothing.
            game.handle(Command::Input(rider, 0, vec![InputFrame(seq_b, 127, -127, 0, 0, 0, 0, 0)]));
            game.step();
        }
        let end = game.world.physics.car_pose(vid).unwrap();
        let travelled = (end.position[0] - start[0]).hypot(end.position[2] - start[2]);
        assert!(travelled > 5.0, "the car drove {travelled} m");
        let rider_at = game.world.players[&rider].character.position;
        assert_eq!(rider_at, end.position, "passengers move with the car");

        // Only the driver is told the car's exact motion, so they can predict it.
        let drive = game.drive_state(driver).expect("the driver gets the car's motion");
        let speed_cm = (drive.0 as f32).hypot(drive.2 as f32);
        assert!(speed_cm > 500.0, "driving at over 5 m/s: {drive:?}");
        assert_eq!(game.drive_state(rider), None);
    }

    #[test]
    fn a_pilot_lifts_off_with_one_passenger() {
        let mut game = game();
        let (pilot, _a) = join(&mut game);
        let (rider, _b) = join(&mut game);
        let (third, _c) = join(&mut game);
        let (mut seq_a, mut seq_b, mut seq_c) = (0, 0, 0);
        for (id, seq) in [(pilot, &mut seq_a), (rider, &mut seq_b), (third, &mut seq_c)] {
            idle(&mut game, id, seq, 3);
        }
        let (&vid, _) = game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_HELI).expect("a helicopter is placed");
        let start = game.world.physics.car_pose(vid).unwrap();
        for id in [pilot, rider, third] {
            game.world.players.get_mut(&id).unwrap().character.position = [start.position[0] + 2.5, STANDING_Y, start.position[2]];
        }
        for (id, seq) in [(pilot, &mut seq_a), (rider, &mut seq_b), (third, &mut seq_c)] {
            press(&mut game, id, seq, (0, 0), BUTTON_USE, 1);
            idle(&mut game, id, seq, 3);
        }
        assert_eq!(game.world.vehicles[&vid].seats[..3], [Some(pilot), Some(rider), None], "two seats, and the pilot first");
        assert!(game.world.players[&third].seat.is_none());

        // Jump climbs: a second of rotor spin-up is not needed, the lift is immediate.
        press(&mut game, pilot, &mut seq_a, (0, 0), BUTTON_JUMP, 120);
        let end = game.world.physics.car_pose(vid).unwrap();
        assert!(end.position[1] > start.position[1] + 3.0, "climbed from {} to {}", start.position[1], end.position[1]);
        assert_eq!(game.world.players[&rider].character.position, end.position, "the passenger goes up with it");
        let drive = game.drive_state(pilot).expect("the pilot gets the motion");
        assert!(drive.1 > 100, "climbing at over 1 m/s: {drive:?}");

        // Let go of the stick: it hovers where it is.
        idle(&mut game, pilot, &mut seq_a, 240);
        let hover = game.world.physics.car_pose(vid).unwrap();
        idle(&mut game, pilot, &mut seq_a, 60);
        let later = game.world.physics.car_pose(vid).unwrap();
        assert!((later.position[1] - hover.position[1]).abs() < 0.5, "hovering: {} -> {}", hover.position[1], later.position[1]);
    }

    #[test]
    fn leaving_the_game_frees_the_seat() {
        let mut game = game();
        let (id, _inbox) = join(&mut game);
        let mut seq = 0;
        idle(&mut game, id, &mut seq, 30);
        let vid = move_player_next_to_a_car(&mut game, id);
        press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, id, &mut seq, 3);
        assert_eq!(game.world.vehicles[&vid].seats[0], Some(id));
        game.handle(Command::Leave(id));
        assert_eq!(game.world.vehicles[&vid].seats, [None; CAR_SEATS]);
    }

    #[test]
    fn a_car_on_its_roof_is_put_back_on_its_wheels() {
        let mut game = game();
        let (&vid, _) = game.world.vehicles.iter().next().unwrap();
        let pose = game.world.physics.car_pose(vid).unwrap();
        // Flip it by resetting the body upside down.
        game.world.physics.set_car_roll(vid, std::f32::consts::PI, [pose.position[0], 1.5, pose.position[2]]);
        for _ in 0..(UNFLIP_TICKS + 240) {
            game.step();
        }
        assert!(game.world.physics.car_pose(vid).unwrap().up_y > 0.9);
    }

    #[test]
    fn a_driven_car_knocks_back_a_player_in_its_way() {
        let mut game = game();
        let (driver, _a) = join(&mut game);
        let (victim, _b) = join(&mut game);
        let (mut seq_a, mut seq_b) = (0, 0);
        idle(&mut game, driver, &mut seq_a, 3);
        idle(&mut game, victim, &mut seq_b, 3);
        let vid = move_player_next_to_a_car(&mut game, driver);
        press(&mut game, driver, &mut seq_a, (0, 0), BUTTON_USE, 1);
        idle(&mut game, driver, &mut seq_a, 3);
        assert_eq!(game.world.vehicles[&vid].seats[0], Some(driver));

        // Stand the victim 12 m ahead of the nose.
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (sin, cos) = pose.yaw().sin_cos();
        let ahead = [-sin, -cos];
        let spot = [pose.position[0] + ahead[0] * 12.0, STANDING_Y, pose.position[2] + ahead[1] * 12.0];
        game.world.players.get_mut(&victim).unwrap().character.position = spot;

        let mut peak_knock: f32 = 0.0;
        let mut hopped = false;
        let mut lowest_hp = PLAYER_MAX_HP;
        for _ in 0..240 {
            seq_a += 1;
            seq_b += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
            game.handle(Command::Input(victim, 0, vec![InputFrame(seq_b, 0, 0, 0, 0, 0, 0, 0)]));
            game.step();
            let character = game.world.players[&victim].character;
            peak_knock = peak_knock.max(character.knock[0].hypot(character.knock[1]));
            hopped |= character.position[1] > STANDING_Y + 0.3;
            lowest_hp = lowest_hp.min(game.world.players[&victim].hp);
        }
        assert!(lowest_hp < PLAYER_MAX_HP, "a car at speed hurts: lowest hp {lowest_hp}");
        assert!(peak_knock > KNOCK_MIN_PUSH, "peak knockback {peak_knock}");
        assert!(hopped, "a hit sends the player up");
        let character = game.world.players[&victim].character;
        let moved = (character.position[0] - spot[0]) * ahead[0] + (character.position[2] - spot[2]) * ahead[1];
        assert!(moved > 2.0, "thrown {moved} m along the car's heading");
        assert!(game.world.players[&victim].seat.is_none());
        // And it shows on the wire.
        assert!(game.entity_states().iter().any(|e| e.0 == victim));
    }

    /// A driver in a car and a second player standing on its roof.
    struct RoofScene {
        vid: u32,
        driver: u32,
        rider: u32,
        /// Where the car started, `[x, z]`.
        start: [f32; 2],
        /// The next input sequence numbers of the driver and the rider.
        seqs: (u32, u32),
        _inboxes: (mpsc::Receiver<Outgoing>, mpsc::Receiver<Outgoing>),
    }

    fn driver_and_roof_rider(game: &mut Game) -> RoofScene {
        let (driver, a) = join(game);
        let (rider, b) = join(game);
        let (mut seq_a, mut seq_b) = (0, 0);
        idle(game, driver, &mut seq_a, 3);
        idle(game, rider, &mut seq_b, 3);
        let vid = move_player_next_to_a_car(game, driver);
        press(game, driver, &mut seq_a, (0, 0), BUTTON_USE, 1);
        idle(game, driver, &mut seq_a, 3);
        let pose = game.world.physics.car_pose(vid).unwrap();
        // Drop the rider on the roof; it settles a tick or two later.
        let top = pose.position[1] + JEEP_HALF_EXTENTS[1] + STANDING_Y + 0.05;
        game.world.players.get_mut(&rider).unwrap().character.position = [pose.position[0], top, pose.position[2]];
        for _ in 0..30 {
            seq_a += 1;
            seq_b += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 0, 0, 0, 0, 0, 0)]));
            game.handle(Command::Input(rider, 0, vec![InputFrame(seq_b, 0, 0, 0, 0, 0, 0, 0)]));
            game.step();
        }
        RoofScene { vid, driver, rider, start: [pose.position[0], pose.position[2]], seqs: (seq_a, seq_b), _inboxes: (a, b) }
    }

    #[test]
    fn a_player_on_the_roof_rides_along() {
        let mut game = game();
        let RoofScene { vid, driver, rider, start, seqs: (mut seq_a, mut seq_b), .. } = driver_and_roof_rider(&mut game);
        assert_eq!(game.world.players[&rider].character.ride, vid, "standing on the roof");
        for _ in 0..240 {
            seq_a += 1;
            seq_b += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
            game.handle(Command::Input(rider, 0, vec![InputFrame(seq_b, 0, 0, 0, 0, 0, 0, 0)]));
            game.step();
        }
        let car = game.world.physics.car_pose(vid).unwrap();
        let on = game.world.players[&rider].character;
        let travelled = (car.position[0] - start[0]).hypot(car.position[2] - start[1]);
        assert!(travelled > 8.0, "the car drove {travelled} m");
        assert_eq!(on.ride, vid);
        let apart = (on.position[0] - car.position[0]).hypot(on.position[2] - car.position[2]);
        assert!(apart < 1.5, "the rider is {apart} m from the car's centre");
        assert!(on.position[1] > car.position[1] + 0.8, "still on top: {:?} vs {:?}", on.position, car.position);
        assert_eq!(game.world.players[&rider].hit_cooldown, 0, "a rider is never run over by their own car");
    }

    #[test]
    fn jumping_off_a_moving_car_keeps_its_speed() {
        let mut game = game();
        let RoofScene { vid, driver, rider, seqs: (mut seq_a, mut seq_b), .. } = driver_and_roof_rider(&mut game);
        for _ in 0..180 {
            seq_a += 1;
            seq_b += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
            game.handle(Command::Input(rider, 0, vec![InputFrame(seq_b, 0, 0, 0, 0, 0, 0, 0)]));
            game.step();
        }
        let speed = game.world.physics.car_pose(vid).unwrap().velocity;
        let speed = speed[0].hypot(speed[2]);
        assert!(speed > 6.0, "the car is moving at {speed}");
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
        game.handle(Command::Input(rider, 0, vec![InputFrame(seq_b, 0, 0, 0, BUTTON_JUMP, 0, 0, 0)]));
        game.step();
        // The jitter buffer holds one frame back, so the jump is applied on the next tick.
        seq_a += 1;
        seq_b += 1;
        game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
        game.handle(Command::Input(rider, 0, vec![InputFrame(seq_b, 0, 0, 0, 0, 0, 0, 0)]));
        game.step();
        let left = game.world.players[&rider].character;
        assert_eq!(left.ride, 0);
        assert!(!left.grounded);
        let knock = left.knock[0].hypot(left.knock[1]);
        assert!((knock - speed).abs() < 2.0, "carries {knock} m/s of the car's {speed}");
    }

    #[test]
    fn parked_cars_in_the_fixture_world_settle_and_stay_put() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/world/world.msgpack");
        let (world, hash) = crate::read_world(&path).unwrap();
        let mut game = Game::new(world, hash).unwrap();
        assert!(game.world.vehicles.len() >= 3, "only {} cars fit", game.world.vehicles.len());
        let start: Vec<_> = game.world.vehicles.keys().map(|&id| (id, game.world.physics.car_pose(id).unwrap())).collect();
        for _ in 0..5 * TICK_RATE {
            game.step();
        }
        for (id, before) in start {
            let after = game.world.physics.car_pose(id).unwrap();
            let moved = (after.position[0] - before.position[0]).hypot(after.position[2] - before.position[2]);
            let ground = game.world.map.ground.height_at(after.position[0], after.position[2]);
            // A plane stands taller on its undercarriage than any car.
            let ride = if game.world.vehicles[&id].kind == KIND_PLANE { 1.3 } else { 1.0 };
            assert!(after.up_y > 0.99 && after.position[1] < ground + ride && moved < 0.5, "car {id}: {before:?} -> {after:?}");
        }
        let planes = game.world.vehicles.values().filter(|vehicle| vehicle.kind == KIND_PLANE).count();
        assert_eq!(planes, PLANE_COUNT, "both planes find somewhere to stand");
        for (&id, _) in game.world.vehicles.iter().filter(|(_, vehicle)| vehicle.kind == KIND_PLANE) {
            let pose = game.world.physics.car_pose(id).unwrap();
            let runway = game.world.physics.runway(&game.world.map.ground, pose.position, pose.yaw(), RUNWAY_SEARCH);
            assert!(runway >= PLANE_RUNWAY, "plane {id} has {runway} m to take off in");
        }
    }

    #[test]
    fn a_tank_has_a_driver_and_a_gunner_and_no_third_seat() {
        let mut game = game();
        let vid = *game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_TANK).unwrap().0;
        let pose = game.world.physics.car_pose(vid).unwrap();
        let mut crew = Vec::new();
        for _ in 0..3 {
            let (id, inbox) = join(&mut game);
            crew.push((id, inbox, 0));
        }
        for (id, _, seq) in &mut crew {
            idle(&mut game, *id, seq, 3);
        }
        for (id, _, seq) in &mut crew {
            game.world.players.get_mut(id).unwrap().character.position = [pose.position[0] + 2.5, STANDING_Y, pose.position[2]];
            press(&mut game, *id, seq, (0, 0), BUTTON_USE, 1);
            idle(&mut game, *id, seq, 3);
        }
        let seated: Vec<_> = crew.iter().map(|(id, ..)| game.world.players[id].seat.map(|seat| (seat.vehicle, seat.index))).collect();
        assert_eq!(seated, [Some((vid, 0)), Some((vid, 1)), None], "two seats, the third stays outside");
        let state = game.vehicle_states().into_iter().find(|v| v.0 == vid).unwrap();
        assert_eq!(state.9.len(), 2);
        assert!(state.10.is_some(), "a tank reports its turret");
    }

    #[test]
    fn a_lone_crewman_can_switch_between_driver_and_gunner() {
        let mut game = game();
        let vid = *game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_TANK).unwrap().0;
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (id, _inbox) = join(&mut game);
        let mut seq = 0;
        idle(&mut game, id, &mut seq, 3);
        game.world.players.get_mut(&id).unwrap().character.position = [pose.position[0] + 2.5, STANDING_Y, pose.position[2]];
        press(&mut game, id, &mut seq, (0, 0), BUTTON_USE, 1);
        idle(&mut game, id, &mut seq, 3);
        assert_eq!(game.world.vehicles[&vid].seats[..2], [Some(id), None]);

        press(&mut game, id, &mut seq, (0, 0), BUTTON_RELOAD, 1);
        idle(&mut game, id, &mut seq, 3);
        assert_eq!(game.world.vehicles[&vid].seats[..2], [None, Some(id)]);
        assert_eq!(game.world.players[&id].seat, Some(Seat { vehicle: vid, index: 1 }));

        press(&mut game, id, &mut seq, (0, 0), BUTTON_RELOAD, 1);
        idle(&mut game, id, &mut seq, 3);
        assert_eq!(game.world.vehicles[&vid].seats[..2], [Some(id), None]);
    }

    #[test]
    fn a_full_tank_has_nowhere_to_switch_to() {
        let mut game = game();
        let vid = *game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_TANK).unwrap().0;
        let pose = game.world.physics.car_pose(vid).unwrap();
        let mut crew = Vec::new();
        for _ in 0..2 {
            let (id, inbox) = join(&mut game);
            crew.push((id, inbox, 0));
        }
        for (id, _, seq) in &mut crew {
            idle(&mut game, *id, seq, 3);
            game.world.players.get_mut(id).unwrap().character.position = [pose.position[0] + 2.5, STANDING_Y, pose.position[2]];
            press(&mut game, *id, seq, (0, 0), BUTTON_USE, 1);
            idle(&mut game, *id, seq, 3);
        }
        let (first, _, seq) = &mut crew[0];
        press(&mut game, *first, seq, (0, 0), BUTTON_RELOAD, 1);
        idle(&mut game, *first, seq, 3);
        let seats = game.world.vehicles[&vid].seats;
        assert_eq!(seats[..2], [Some(crew[0].0), Some(crew[1].0)]);
    }

    /// Two cars off the road, `a` facing -Z towards the flank of `b` 12 m ahead, settled.
    fn two_cars(game: &mut Game) -> (u32, u32) {
        let mut cars = game.world.vehicles.iter().filter(|(_, v)| v.kind == KIND_JEEP).map(|(&id, _)| id);
        let (a, b) = (cars.next().unwrap(), cars.next().unwrap());
        game.world.physics.reset_car(a, [15.0, 1.0, 30.0], 0.0);
        game.world.physics.reset_car(b, [15.0, 1.0, 18.0], std::f32::consts::FRAC_PI_2);
        for _ in 0..60 {
            Vehicles.on_tick(&mut game.world, TICK_DT);
        }
        (a, b)
    }

    /// Steps the vehicles until `ticks` have passed, and returns the crash and scrape events seen.
    fn crash_events(game: &mut Game, ticks: u32) -> Vec<ShotEvent> {
        let mut seen = vec![];
        for _ in 0..ticks {
            Vehicles.on_tick(&mut game.world, TICK_DT);
            seen.extend(game.world.shots.drain(..).filter(|shot| shot.1 == WEAPON_CRASH || shot.1 == WEAPON_SCRAPE));
        }
        seen
    }

    #[test]
    fn ramming_a_car_hurts_both_and_blames_the_driver() {
        let mut game = game();
        let (id, _inbox) = join(&mut game);
        let (a, b) = two_cars(&mut game);
        game.world.vehicles.get_mut(&a).unwrap().seats[0] = Some(id);
        game.world.players.get_mut(&id).unwrap().seat = Some(Seat { vehicle: a, index: 0 });
        game.world.physics.kick_car(a, [0.0, 0.0, -22.0]);
        let events = crash_events(&mut game, 60);
        let (rammer, rammed) = (&game.world.vehicles[&a], &game.world.vehicles[&b]);
        assert!(rammer.hp < vehicle_hp(KIND_JEEP) && rammed.hp < vehicle_hp(KIND_JEEP), "{} {}", rammer.hp, rammed.hp);
        assert!(rammer.damage[DAMAGE_FRONT] > 50, "the rammer's nose: {:?}", rammer.damage);
        // `b` faces -X, so its left side faces +Z, where `a` came from.
        assert!(rammed.damage[DAMAGE_LEFT] > 50 && rammed.damage[DAMAGE_RIGHT] == 0, "the rammed car's flank: {:?}", rammed.damage);
        assert_eq!(rammed.last_attacker, Some(id));
        assert!(events.iter().any(|shot| shot.0 == a && shot.1 == WEAPON_CRASH), "{events:?}");
        let hit = events.iter().find(|shot| shot.1 == WEAPON_CRASH).unwrap();
        assert!((hit.4 as f32 / 100.0 - 19.0).abs() < 2.0, "where they met: {hit:?}");
    }

    #[test]
    fn a_nudge_does_no_harm() {
        let mut game = game();
        let (a, b) = two_cars(&mut game);
        game.world.physics.kick_car(a, [0.0, 0.0, -3.0]);
        let events = crash_events(&mut game, 300);
        assert_eq!((game.world.vehicles[&a].hp, game.world.vehicles[&b].hp), (vehicle_hp(KIND_JEEP), vehicle_hp(KIND_JEEP)));
        assert_eq!(game.world.vehicles[&b].damage, [0; 8]);
        assert!(events.iter().all(|shot| shot.1 != WEAPON_CRASH), "{events:?}");
    }

    #[test]
    fn a_wheel_takes_a_hit_beside_it_and_a_wreck_comes_back_whole() {
        let mut damage = [0; 8];
        // Just behind the front left wheel, on the left side.
        dent(KIND_JEEP, &mut damage, [-JEEP_HALF_EXTENTS[0], 0.0, -JEEP_AXLE_Z + 0.4], 15.0);
        assert!(damage[DAMAGE_LEFT] > 100 && damage[DAMAGE_RIGHT] == 0, "{damage:?}");
        assert!(damage[DAMAGE_WHEELS] > 100, "the front left wheel: {damage:?}");
        assert_eq!(damage[DAMAGE_WHEELS + 3], 0, "the far rear wheel is untouched: {damage:?}");
        dent(KIND_JEEP, &mut damage, [-JEEP_HALF_EXTENTS[0], 0.0, -JEEP_AXLE_Z], 30.0);
        assert_eq!(damage[DAMAGE_WHEELS], shared::protocol::WHEEL_GONE, "a second hit takes it off: {damage:?}");

        let mut game = game();
        let (a, _) = two_cars(&mut game);
        game.world.vehicles.get_mut(&a).unwrap().damage = damage;
        super::super::combat::damage_vehicle(&mut game.world, a, 255, None);
        for _ in 0..WRECK_TICKS + 2 {
            Vehicles.on_tick(&mut game.world, TICK_DT);
        }
        let vehicle = &game.world.vehicles[&a];
        assert_eq!((vehicle.hp, vehicle.damage), (vehicle_hp(KIND_JEEP), [0; 8]));
    }

    #[test]
    fn gunfire_at_a_wheel_bends_it_and_a_second_round_takes_it_off_but_the_middle_is_safe() {
        let mut game = game();
        let (a, b) = two_cars(&mut game);
        let pose = game.world.physics.car_pose(a).unwrap();
        let kind = game.world.vehicles[&a].kind;
        let front_left = wheel_spots(kind)[0];
        // A revolver round (35) on the front left wheel bends it; two sniper rounds (90) take it off.
        let at = pose.world_point([front_left[0], 0.0, front_left[1]]);
        shoot_wheels(&mut game.world, a, at, 35.0, BULLET_WHEEL_REACH);
        let first = game.world.vehicles[&a].damage[DAMAGE_WHEELS];
        assert!(first > 40, "one round bends it: {first}");
        assert_eq!(game.world.vehicles[&a].damage[DAMAGE_WHEELS + 3], 0);
        shoot_wheels(&mut game.world, a, at, 90.0, BULLET_WHEEL_REACH);
        shoot_wheels(&mut game.world, a, at, 90.0, BULLET_WHEEL_REACH);
        assert_eq!(game.world.vehicles[&a].damage[DAMAGE_WHEELS], shared::protocol::WHEEL_GONE);
        // Fire into the middle of the car: no wheel is within a bullet's reach.
        let middle = game.world.physics.car_pose(b).unwrap().position;
        shoot_wheels(&mut game.world, b, middle, 90.0, BULLET_WHEEL_REACH);
        assert_eq!(game.world.vehicles[&b].damage[DAMAGE_WHEELS..], [0; 4]);
    }

    #[test]
    fn a_plane_is_never_parked_on_another_ones_runway() {
        let game = game();
        let planes: Vec<CarPose> = game.world.vehicles.iter().filter(|(_, v)| v.kind == KIND_PLANE).map(|(&id, _)| game.world.physics.car_pose(id).unwrap()).collect();
        let [a, b] = &planes[..] else { panic!("two planes: {planes:?}") };
        assert!(!on_runway(a.position, a.yaw(), PLANE_RUNWAY, b.position) && !on_runway(b.position, b.yaw(), PLANE_RUNWAY, a.position), "{a:?} {b:?}");
        // Each faces in along the road, towards the middle, where it has room.
        assert!(a.position[0] * b.position[0] < 0.0, "one at each end: {a:?} {b:?}");
        for pose in [a, b] {
            let inward = -pose.position[0].signum();
            let (sin, _) = pose.yaw().sin_cos();
            assert!(-sin * inward > 0.99, "{pose:?}");
        }
        // Nothing parked in the way either.
        for (&vid, _) in game.world.vehicles.iter().filter(|(_, v)| v.kind != KIND_PLANE) {
            let other = game.world.physics.car_pose(vid).unwrap().position;
            assert!(!on_runway(a.position, a.yaw(), PLANE_RUNWAY, other) && !on_runway(b.position, b.yaw(), PLANE_RUNWAY, other), "{other:?}");
        }
    }

    #[test]
    fn a_pilot_and_a_gunner_take_a_plane_off_and_a_third_stays_behind() {
        let mut game = game();
        let (&vid, _) = game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).expect("a plane is placed");
        let start = game.world.physics.car_pose(vid).unwrap();
        let mut crew = Vec::new();
        for _ in 0..3 {
            let (id, inbox) = join(&mut game);
            crew.push((id, inbox, 0));
        }
        for (id, _, seq) in &mut crew {
            idle(&mut game, *id, seq, 3);
            game.world.players.get_mut(id).unwrap().character.position = [start.position[0], STANDING_Y, start.position[2] + 3.0];
            press(&mut game, *id, seq, (0, 0), BUTTON_USE, 1);
            idle(&mut game, *id, seq, 3);
        }
        let seated: Vec<_> = crew.iter().map(|(id, ..)| game.world.players[id].seat.map(|seat| seat.index)).collect();
        assert_eq!(seated, [Some(0), Some(1), None], "a pilot, a gunner, and nobody else");
        let state = game.vehicle_states().into_iter().find(|v| v.0 == vid).unwrap();
        assert_eq!(state.13, Some(shared::protocol::Arms(0, 0, 0, PLANE_BOMBS, 0)), "the plane reports its weapons");

        // Space for full throttle and a little back stick (S) to lift off, along the road.
        let (pilot, gunner, mut seq) = (crew[0].0, crew[1].0, crew[0].2);
        let yaw = shared::quant::to_mrad(start.yaw());
        for _ in 0..60 * 7 {
            seq += 1;
            game.handle(Command::Input(pilot, 0, vec![InputFrame(seq, 0, -60, yaw, BUTTON_JUMP, 0, 0, 0)]));
            game.step();
        }
        let flying = game.world.physics.car_pose(vid).unwrap();
        assert!(flying.position[1] > start.position[1] + 5.0 && flying.forward_speed > PLANE_STALL_SPEED, "{start:?} -> {flying:?}");
        assert_eq!(game.world.players[&gunner].character.position, flying.position, "the gunner goes up with it");
        assert!(game.drive_state(pilot).is_some(), "the pilot predicts the plane");
    }

    #[test]
    fn a_gentle_landing_does_no_harm() {
        let mut game = game();
        let (&vid, _) = game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).unwrap();
        let pose = game.world.physics.car_pose(vid).unwrap();
        // Coming in at 22 m/s and sinking at a metre and a half a second.
        game.world.physics.set_car_dive(vid, [pose.position[0], 3.0, pose.position[2]], pose.yaw(), -0.07, 22.0);
        let events = crash_events(&mut game, 180);
        let vehicle = &game.world.vehicles[&vid];
        assert_eq!(vehicle.hp, vehicle_hp(KIND_PLANE), "{events:?}");
        let landed = game.world.physics.car_pose(vid).unwrap();
        assert!(landed.up_y > 0.95 && landed.position[1] < 1.5, "{landed:?}");
    }

    #[test]
    fn flying_a_plane_into_the_ground_wrecks_it() {
        let mut game = game();
        let (&vid, _) = game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).unwrap();
        let pose = game.world.physics.car_pose(vid).unwrap();
        // Diving at 45° and 30 m/s, a few metres up.
        game.world.physics.set_car_dive(vid, [pose.position[0], 6.0, pose.position[2]], pose.yaw(), -0.8, 30.0);
        for _ in 0..120 {
            Vehicles.on_tick(&mut game.world, TICK_DT);
        }
        assert!(game.world.vehicles[&vid].wrecked(), "hp {}", game.world.vehicles[&vid].hp);
    }
}
