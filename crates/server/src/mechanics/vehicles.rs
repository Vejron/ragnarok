use super::Mechanic;
use crate::game::{Gun, Seat, Vehicle, World};
use crate::physics::{CarControls, CarPose, Character, half_extents};
use shared::consts::*;
use shared::protocol::{KIND_HELI, KIND_JEEP, KIND_SPORTS, KIND_TANK, seat_count, vehicle_hp};
use shared::quant::from_axis;
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
/// Spawn points `Spawn` hands to players first; cars prefer the others.
const PLAYER_SPAWNS: usize = 6;
/// Spawn height of the chassis centre, a little above its resting height.
const CAR_DROP_Y: f32 = 1.0;
/// Cars at least this fast (or any overlap at all) hurl a standing player away.
/// Knockback speed is the car's speed times this, within the limits below.
const KNOCK_SPEED_FACTOR: f32 = 1.1;
const KNOCK_MIN_PUSH: f32 = 2.5;
/// Hop given to a player who is hit: `KNOCK_HOP + KNOCK_HOP_PER_SPEED * speed`, at most `KNOCK_HOP_MAX`.
const KNOCK_HOP: f32 = 2.0;
const KNOCK_HOP_PER_SPEED: f32 = 0.25;
const KNOCK_HOP_MAX: f32 = 8.0;
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
            gun: (kind == KIND_TANK || kind == KIND_HELI).then(Gun::default),
            hp: vehicle_hp(kind),
            wreck_in: 0,
            last_attacker: None,
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

/// Throws every standing player a car overlaps. The direction leans on the car's heading with a
/// little outward push, so a car drives a player ahead of it and off to the side.
fn knock_players(world: &mut World) {
    for player in world.players.values_mut() {
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
    }
}

impl Mechanic for Vehicles {
    fn on_start(&mut self, world: &mut World) {
        let spawns = world.map.spawns.clone();
        // Prefer spawn points players do not start on, then fall back to those.
        let split = PLAYER_SPAWNS.min(spawns.len());
        let candidates = spawns[split..].iter().chain(spawns[..split].iter().rev());
        let mut placed = 0;
        for &[cx, cz] in candidates {
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
                placed += 1;
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
                    }
                }
                None => CarControls { handbrake: true, ..Default::default() },
            };
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
                    vehicle.last_attacker = None;
                    world.physics.reset_car(vid, vehicle.home.0, vehicle.home.1);
                }
            } else if y < KILL_Y {
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
            half_extent: 20_000,
            spawns: (0..9).map(|i| [i * 3000 - 12_000, 0]).collect(),
            roads: vec![Road { width: 800, kind: 0, points: vec![[-20_000, 0], [20_000, 0]] }],
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
        assert_eq!(game.world.vehicles.len(), CAR_COUNT + TANK_COUNT + HELI_COUNT);
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
        for _ in 0..240 {
            seq_a += 1;
            seq_b += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq_a, 0, 127, 0, 0, 0, 0, 0)]));
            game.handle(Command::Input(victim, 0, vec![InputFrame(seq_b, 0, 0, 0, 0, 0, 0, 0)]));
            game.step();
            let character = game.world.players[&victim].character;
            peak_knock = peak_knock.max(character.knock[0].hypot(character.knock[1]));
            hopped |= character.position[1] > STANDING_Y + 0.3;
        }
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
            assert!(after.up_y > 0.99 && after.position[1] < ground + 1.0 && moved < 0.5, "car {id}: {before:?} -> {after:?}");
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
}
