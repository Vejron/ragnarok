use super::Mechanic;
use super::armor::{self, Outcome};
use super::bots::{HEAR_BLAST, HEAR_SHOT, make_noise};
use super::combat::{Blast, Direct, Round, blast_at, first_player_hit, hitscan, look, rewind_ticks, scatter};
use crate::game::{Gun, PlaneArms, Rocket, World};
use crate::physics::CarPose;
use shared::consts::*;
use shared::items::Falloff;
use shared::protocol::{DAMAGE_TURRET, TURRET_GONE, KIND_HELI, KIND_HEN, KIND_MORTAR, KIND_PLANE, KIND_STRV, KIND_WALKER, PROJECTILE_BOMB, PROJECTILE_CANNON, PROJECTILE_EGG, PROJECTILE_MORTAR, PROJECTILE_ROCKET, PROJECTILE_SHELL, ShotEvent, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_MG, WEAPON_MORTAR, WEAPON_SHELL};
use shared::quant::{from_mrad, to_cm, wrap_angle};

/// Vehicle guns: the gunner's look direction aims the gun, and the fire button shoots it.
///
/// A helicopter's nose gun is an autocannon, worked from seat 1 as well (see `work_autocannon`): its
/// rounds are shells in miniature. A plane's pilot fires two forward machine guns and drops bombs,
/// and its gunner works a machine gun on a ring behind them (see `work_cockpit` and `work_ring`). A walker's
/// driver aims and fires its chin guns (see `work_walker_guns`), and a Strv S's driver its fixed main gun
/// (see `work_driver_cannon`). A hen's driver lays egg bombs (see `work_hen`). A tank's seat 1 is its gunner. Their input drives the turret instead of anything on foot, while
/// the driver keeps the hull. The turret is stabilised: it follows the gunner's *world* heading
/// at a limited slew rate, so turning the hull does not drag the aim around. A shell is a
/// projectile: it leaves the muzzle at `SHELL_SPEED`, falls under gravity, and explodes at the first
/// wall, vehicle or stickman it meets, throwing everyone nearby and shoving vehicles.
pub struct Gunnery;

/// The seat that works the gun.
const GUNNER_SEAT: usize = 1;

fn step_toward(current: f32, target: f32, step: f32) -> f32 {
    current + (target - current).clamp(-step, step)
}

/// Where the shell leaves the turret ring, and its unit direction, for a gun at `yaw` / `pitch`
/// (hull frame) on a hull at `pose`. The client draws the barrel and the tracer from the same maths.
pub fn aim(pose: &CarPose, yaw: f32, pitch: f32) -> ([f32; 3], [f32; 3]) {
    aim_from(pose, TANK_TURRET_PIVOT, yaw, pitch)
}

/// The same for a gun whose mount is at `pivot` in the hull frame.
fn aim_from(pose: &CarPose, pivot: [f32; 3], yaw: f32, pitch: f32) -> ([f32; 3], [f32; 3]) {
    let local = [-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()];
    (pose.world_point(pivot), pose.world_dir(local))
}

/// How fast a shell of `kind` leaves its muzzle, m/s. A mortar bomb is slower and lobs high.
fn muzzle_speed(kind: u8) -> f32 {
    if kind == PROJECTILE_MORTAR { MORTAR_SHELL_SPEED } else { SHELL_SPEED }
}

/// How long a shell of `kind` flies before it bursts on its own.
fn shell_ttl(kind: u8) -> u32 {
    if kind == PROJECTILE_MORTAR { MORTAR_SHELL_TTL_TICKS } else { SHELL_TTL_TICKS }
}

/// A shell of `kind` going off at `point`: the burst everyone sees, and the blast. An autocannon round
/// is a third of a tank shell's reach, and hits for less.
fn burst(world: &mut World, kind: u8, owner: u32, point: [f32; 3], dir: [f32; 3], direct: Option<Direct>) {
    let (weapon, radius, damage, knock) = match kind {
        PROJECTILE_CANNON => (WEAPON_AUTOCANNON, CANNON_RADIUS, CANNON_DAMAGE, CANNON_KNOCK),
        PROJECTILE_BOMB => (WEAPON_BOMB, BOMB_RADIUS, BOMB_DAMAGE, BOMB_KNOCK),
        PROJECTILE_MORTAR => (WEAPON_MORTAR, MORTAR_RADIUS, MORTAR_DAMAGE, MORTAR_KNOCK),
        _ => (WEAPON_SHELL, EXPLOSION_RADIUS, EXPLOSION_DAMAGE, EXPLOSION_KNOCK),
    };
    let [x, y, z] = point.map(to_cm);
    world.shots.push(ShotEvent { shooter: owner, weapon, x, y, z });
    // A gunner who has left or died still gets the blast, just not the credit.
    let attacker = world.players.contains_key(&owner).then_some(owner);
    blast_at(world, Blast { point, radius, damage, knock, dir, attacker, weapon }, direct);
}

/// Fires a shell of `kind` from vehicle `vid` for `gunner`, from `origin` along `dir` (the muzzle's
/// root and the barrel's heading, `length` of barrel ahead): it leaves the muzzle and flies on its own
/// from there, so it takes time to arrive and falls on the way.
fn fire(world: &mut World, kind: u8, vid: u32, origin: [f32; 3], dir: [f32; 3], length: f32, gunner: u32) {
    make_noise(world, origin, Some(gunner), HEAR_BLAST);
    // A barrel poking into a wall bursts the shell there, instead of starting it on the far side.
    if let Some(t) = world.physics.cast_ray(origin, dir, length, vid) {
        burst(world, kind, gunner, std::array::from_fn(|i| origin[i] + dir[i] * t), dir, None);
        return;
    }
    let id = world.alloc_id();
    world.rockets.insert(
        id,
        Rocket {
            kind,
            owner: gunner,
            source: vid,
            position: std::array::from_fn(|i| origin[i] + dir[i] * length),
            velocity: dir.map(|v| v * muzzle_speed(kind)),
            ttl: shell_ttl(kind),
            item: 0,
            back: 0,
            flown: 0.0,
        },
    );
}

/// Flies every shell one tick: gravity, then along its velocity until it meets a wall, a vehicle
/// or a stickman, or its time runs out. It bursts where it stopped.
fn step_shells(world: &mut World, dt: f32) {
    let falls = |kind| kind == PROJECTILE_SHELL || kind == PROJECTILE_CANNON || kind == PROJECTILE_BOMB || kind == PROJECTILE_MORTAR;
    let ids: Vec<u32> = world.rockets.iter().filter(|(_, rocket)| falls(rocket.kind)).map(|(&id, _)| id).collect();
    for id in ids {
        let Some(mut shell) = world.rockets.get(&id).copied() else { continue };
        shell.velocity[1] -= GRAVITY * dt;
        let speed = shell.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        let dir = shell.velocity.map(|v| v / speed);
        let step = speed * dt;
        let hit = world.physics.cast_ray_hit(shell.position, dir, step, shell.source);
        let wall = hit.map(|(t, _)| t);
        // Stickmen are not physics bodies, so test the shell's path against each one.
        let player = first_player_hit(world, shell.position, dir, wall.unwrap_or(step), Some(shell.owner), 0);
        shell.ttl = shell.ttl.saturating_sub(1);
        if let Some(t) = player.map(|(_, t)| t).or(wall) {
            let point: [f32; 3] = std::array::from_fn(|i| shell.position[i] + dir[i] * t);
            // A vehicle was hit first: its armor decides whether the shell bursts on it or glances off.
            let mut direct = None;
            if let (None, Some((_, Some(vid)))) = (player, hit) {
                match armor::armor_of(world, vid, shell.position, dir, step) {
                    Some(Outcome::Ricochet { normal }) => {
                        let normal = world.physics.car_pose(vid).map_or(normal, |pose| pose.world_dir(normal));
                        armor::announce_ricochet(world, vid, point);
                        let out = armor::reflect(dir, normal);
                        shell.velocity = out.map(|v| v * speed * armor::RICOCHET_KEEP);
                        // Just clear of the plate, so the next step does not hit it again.
                        shell.position = std::array::from_fn(|i| point[i] + normal[i] * 0.1);
                        world.rockets.insert(id, shell);
                        continue;
                    }
                    Some(Outcome::Hit { scale, turret }) => direct = Some(Direct { vehicle: vid, scale, turret }),
                    None => {}
                }
            }
            world.rockets.remove(&id);
            burst(world, shell.kind, shell.owner, point, dir, direct);
            continue;
        }
        shell.position = std::array::from_fn(|i| shell.position[i] + dir[i] * step);
        if shell.position[1] < KILL_Y {
            world.rockets.remove(&id);
        } else if shell.ttl == 0 {
            world.rockets.remove(&id);
            burst(world, shell.kind, shell.owner, shell.position, dir, None);
        } else {
            world.rockets.insert(id, shell);
        }
    }
}

/// One tick of a helicopter's nose gun, for the gunner in seat 1 if there is one, and the gun as it is
/// after it. The gunner's look direction is taken in the world and expressed in the hull's frame, so the
/// gun holds its aim while the helicopter leans and turns, until the target leaves its travel. A round
/// is a small shell, scattered a little: it flies, falls and bursts like a tank's.
fn work_autocannon(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose, dt: f32) -> Gun {
    gun.cooldown = gun.cooldown.saturating_sub(1);
    let Some(gunner) = world.vehicles[&vid].seats[GUNNER_SEAT] else { return gun };
    let Some(input) = world.players.get(&gunner).map(|player| player.input) else { return gun };

    let local = pose.local_dir(look(from_mrad(input.yaw), from_mrad(input.pitch())));
    let yaw = (-local[0]).atan2(-local[2]).clamp(-HELI_GUN_YAW_LIMIT, HELI_GUN_YAW_LIMIT);
    let pitch = local[1].clamp(-1.0, 1.0).asin().clamp(HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX);
    gun.yaw = step_toward(gun.yaw, yaw, HELI_GUN_SLEW * dt);
    gun.pitch = step_toward(gun.pitch, pitch, HELI_GUN_SLEW * dt);
    if input.buttons() & BUTTON_FIRE == 0 || gun.cooldown > 0 {
        return gun;
    }

    let (origin, dir) = aim_from(pose, HELI_GUN_PIVOT, gun.yaw, gun.pitch);
    let dir = scatter(world, dir, HELI_GUN_SPREAD);
    fire(world, PROJECTILE_CANNON, vid, origin, dir, HELI_GUN_LENGTH, gunner);
    gun.shots = gun.shots.wrapping_add(1);
    gun.cooldown = HELI_GUN_COOLDOWN_TICKS;
    gun
}

/// One tick of a helicopter's rocket pods, for the pilot in seat 0 if there is one. Holding fire sends a
/// rocket from the next tube along the nose every `HELI_ROCKET_COOLDOWN_TICKS`. The rocket starts at the
/// helicopter's speed along the nose plus a small push and accelerates from there (see `step_rockets`).
fn work_pods(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose) -> Gun {
    gun.pod_cooldown = gun.pod_cooldown.saturating_sub(1);
    let Some(pilot) = world.vehicles[&vid].seats[0] else { return gun };
    let Some(buttons) = world.players.get(&pilot).map(|player| player.input.buttons()) else { return gun };
    if buttons & BUTTON_FIRE == 0 || gun.pod_cooldown > 0 {
        return gun;
    }
    let pivot = HELI_POD_PIVOTS[usize::from(gun.pod_shots) % HELI_POD_PIVOTS.len()];
    let (origin, dir) = aim_from(pose, pivot, 0.0, 0.0);
    // It flies where the nose points, not where the helicopter is drifting: only the helicopter's speed
    // along the nose is carried over, so a sideways slide does not bend the rocket's heading.
    let along: f32 = (0..3).map(|i| pose.velocity[i] * dir[i]).sum::<f32>().max(0.0);
    let id = world.alloc_id();
    world.rockets.insert(
        id,
        Rocket {
            kind: PROJECTILE_ROCKET,
            owner: pilot,
            source: vid,
            position: origin,
            velocity: dir.map(|d| d * (HELI_ROCKET_LAUNCH_SPEED + along)),
            ttl: ROCKET_TTL_TICKS,
            item: 0,
            back: 0,
            flown: 0.0,
        },
    );
    gun.pod_shots = gun.pod_shots.wrapping_add(1);
    gun.pod_cooldown = HELI_ROCKET_COOLDOWN_TICKS;
    gun
}

/// One tick of a walker's chin guns, for its driver in seat 0. The driver's look direction, taken into
/// the hull's frame, turns the upper body within `WALKER_TORSO_LIMIT` of the way the legs face and lifts
/// the guns within their travel, at `WALKER_GUN_SLEW`. A walker that has fallen cannot aim or fire.
fn work_walker_guns(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose, dt: f32) -> Gun {
    gun.cooldown = gun.cooldown.saturating_sub(1);
    let Some(driver) = world.vehicles[&vid].seats[0] else { return gun };
    let Some(input) = world.players.get(&driver).map(|player| player.input) else { return gun };
    if pose.up_y < WALKER_FALLEN_UP {
        return gun;
    }

    let local = pose.local_dir(look(from_mrad(input.yaw), from_mrad(input.pitch())));
    let yaw = (-local[0]).atan2(-local[2]).clamp(-WALKER_TORSO_LIMIT, WALKER_TORSO_LIMIT);
    let pitch = local[1].clamp(-1.0, 1.0).asin().clamp(WALKER_GUN_PITCH_MIN, WALKER_GUN_PITCH_MAX);
    gun.yaw = step_toward(gun.yaw, yaw, WALKER_GUN_SLEW * dt);
    gun.pitch = step_toward(gun.pitch, pitch, WALKER_GUN_SLEW * dt);
    if input.buttons() & BUTTON_FIRE == 0 || gun.cooldown > 0 {
        return gun;
    }

    let (_, dir) = aim_from(pose, WALKER_GUN_PIVOT, gun.yaw, gun.pitch);
    // The barrels hang under the chin, one either side, and take turns: the shell starts at the one
    // whose turn it is, the offset turned with the upper body (yaw) and the guns (pitch).
    let side = if gun.shots % 2 == 0 { -1.0 } else { 1.0 };
    let (sp, cp) = gun.pitch.sin_cos();
    let (sy, cy) = gun.yaw.sin_cos();
    let (bx, by) = (side * WALKER_GUN_BARREL_X, -WALKER_GUN_BARREL_DROP);
    let (ly, lz) = (by * cp, by * sp);
    let offset = [bx * cy + lz * sy, ly, -bx * sy + lz * cy];
    let pivot = std::array::from_fn(|i| WALKER_GUN_PIVOT[i] + offset[i]);
    let origin = pose.world_point(pivot);
    let dir = scatter(world, dir, WALKER_GUN_SPREAD);
    fire(world, PROJECTILE_CANNON, vid, origin, dir, WALKER_GUN_LENGTH, driver);
    gun.shots = gun.shots.wrapping_add(1);
    gun.cooldown = WALKER_GUN_COOLDOWN_TICKS;
    gun
}

/// One tick of a field mortar, for its gunner in seat 0. The gunner's look turns the mount to face where they
/// look, and the range dial (W and S) sets how far the bomb flies. The barrel's elevation is the lob that
/// lands it there, so the tube always points where the bomb will go. The bomb leaves at `MORTAR_SHELL_SPEED`,
/// and the mortar is shoved back on its wheels. A towed mortar cannot fire: it is being pulled.
fn work_mortar(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose, dt: f32) -> Gun {
    gun.cooldown = gun.cooldown.saturating_sub(1);
    let Some(gunner) = world.vehicles[&vid].seats[0] else { return gun };
    let Some(input) = world.players.get(&gunner).map(|player| player.input) else { return gun };

    let wanted = wrap_angle(from_mrad(input.yaw) - pose.yaw());
    gun.yaw = wrap_angle(step_toward(gun.yaw, gun.yaw + wrap_angle(wanted - gun.yaw), MORTAR_TRAVERSE_SLEW * dt));
    // A gun that has never been dialled starts at the middle of the range, not at zero.
    let range = if gun.range == 0.0 { MORTAR_RANGE_START } else { gun.range };
    let buttons = input.buttons();
    let dial = f32::from(buttons & BUTTON_RANGE_UP != 0) - f32::from(buttons & BUTTON_RANGE_DOWN != 0);
    gun.range = (range + dial * MORTAR_RANGE_RATE * dt).clamp(MORTAR_RANGE_MIN, MORTAR_RANGE_MAX);
    gun.pitch = lob_pitch(gun.range);
    if buttons & BUTTON_FIRE == 0 || gun.cooldown > 0 || world.physics.towed_by(vid).is_some() {
        return gun;
    }

    let (origin, dir) = aim_from(pose, MORTAR_GUN_PIVOT, gun.yaw, gun.pitch);
    fire(world, PROJECTILE_MORTAR, vid, origin, dir, MORTAR_BARREL_LENGTH, gunner);
    world.physics.kick_car(vid, [-dir[0] * MORTAR_RECOIL, 0.0, -dir[2] * MORTAR_RECOIL]);
    gun.shots = gun.shots.wrapping_add(1);
    gun.cooldown = MORTAR_COOLDOWN_TICKS;
    gun
}

/// The elevation of the high lob that lands a bomb `range` metres out on level ground, from `MORTAR_SHELL_SPEED`
/// and `GRAVITY`: the steeper of the two angles that reach it, so the shorter the range the closer to vertical.
/// Clamped to the mortar's elevation travel.
pub fn lob_pitch(range: f32) -> f32 {
    let sine = (GRAVITY * range / (MORTAR_SHELL_SPEED * MORTAR_SHELL_SPEED)).min(1.0);
    (0.5 * (std::f32::consts::PI - sine.asin())).clamp(MORTAR_PITCH_MIN, MORTAR_PITCH_MAX)
}

/// One tick of a Strv S's main gun, for its driver in seat 0. The gun is fixed in the hull, so it points
/// where the hull does (the yaw stays 0: the driver aims sideways by turning the tank) and the driver's
/// look pitch only elevates the barrel, within its travel, at `STRV_BARREL_SLEW`. It kicks the hull back
/// like a tank's.
fn work_driver_cannon(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose, dt: f32) -> Gun {
    gun.cooldown = gun.cooldown.saturating_sub(1);
    gun.yaw = 0.0;
    let Some(driver) = world.vehicles[&vid].seats[0] else { return gun };
    let Some(input) = world.players.get(&driver).map(|player| player.input) else { return gun };

    gun.pitch = step_toward(gun.pitch, from_mrad(input.pitch).clamp(STRV_BARREL_MIN, STRV_BARREL_MAX), STRV_BARREL_SLEW * dt);
    if input.buttons() & BUTTON_FIRE == 0 || gun.cooldown > 0 {
        return gun;
    }
    let (origin, dir) = aim_from(pose, STRV_GUN_PIVOT, 0.0, gun.pitch);
    fire(world, PROJECTILE_SHELL, vid, origin, dir, STRV_BARREL_LENGTH, driver);
    world.physics.kick_car(vid, [-dir[0] * SHELL_RECOIL, 0.0, -dir[2] * SHELL_RECOIL]);
    gun.shots = gun.shots.wrapping_add(1);
    gun.cooldown = SHELL_COOLDOWN_TICKS;
    gun
}

/// A machine gun's barrels cooling for a tick; a jammed gun clears once they are cool enough.
fn cool(heat: &mut f32, jammed: &mut bool) {
    *heat = (*heat - GUN_COOL).max(0.0);
    if *jammed && *heat <= GUN_UNJAM {
        *jammed = false;
    }
}

/// A machine gun's barrels taking a round's worth of `heat`; at the top they jam.
fn heat_up(heat: &mut f32, jammed: &mut bool, by: f32) {
    *heat = (*heat + by).min(255.0);
    if *heat >= 255.0 {
        *jammed = true;
    }
}

/// One machine gun round for `shooter` from vehicle `vid`, out of `muzzle` along `dir`. Every round is
/// drawn by the client from the gun's counter; only one that hit something is announced, for the spark.
fn machine_gun(world: &mut World, shooter: u32, vid: u32, muzzle: [f32; 3], dir: [f32; 3]) {
    let back = world.players.get(&shooter).map_or(0, |player| rewind_ticks(world.tick, player.input.view()));
    let round = Round { damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: PLANE_GUN_RANGE, falloff: Falloff::NONE };
    make_noise(world, muzzle, Some(shooter), HEAR_SHOT);
    let (end, struck) = hitscan(world, shooter, muzzle, dir, &round, vid, back);
    if struck {
        let [x, y, z] = std::array::from_fn(|i| to_cm(muzzle[i] + dir[i] * end));
        world.shots.push(ShotEvent { shooter, weapon: WEAPON_MG, x, y, z });
    }
}

/// One tick of a plane's cockpit for its pilot in seat 0: the forward guns, which fire in turn along the
/// nose while the trigger is held and they are not jammed, and the bomb rack, which drops one on crouch.
/// A bomb leaves with the plane's own velocity. The rack fills again once the plane has stood still on
/// the ground for a while. No bomb drops on the ground.
fn work_cockpit(world: &mut World, vid: u32, mut arms: PlaneArms, pose: &CarPose) -> PlaneArms {
    arms.cooldown = arms.cooldown.saturating_sub(1);
    arms.bomb_cooldown = arms.bomb_cooldown.saturating_sub(1);
    cool(&mut arms.heat, &mut arms.jammed);
    let speed = pose.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
    let [x, y, z] = pose.position;
    let height = y - world.map.ground.height_at(x, z);
    let parked = speed < 0.5 && height < 2.0;
    arms.rearm = if parked { arms.rearm + 1 } else { 0 };
    if arms.rearm >= PLANE_REARM_TICKS {
        arms.bombs = PLANE_BOMBS;
    }

    let Some(pilot) = world.vehicles[&vid].seats[0] else { return arms };
    let Some(buttons) = world.players.get(&pilot).map(|player| player.input.buttons()) else { return arms };
    if buttons & BUTTON_FIRE != 0 && arms.cooldown == 0 && !arms.jammed {
        // The left gun fires the even rounds.
        let pivot = PLANE_GUN_PIVOTS[usize::from(arms.shots % 2)];
        let (muzzle, dir) = aim_from(pose, pivot, 0.0, 0.0);
        let dir = scatter(world, dir, PLANE_GUN_SPREAD);
        machine_gun(world, pilot, vid, muzzle, dir);
        arms.shots = arms.shots.wrapping_add(1);
        arms.cooldown = PLANE_GUN_COOLDOWN_TICKS;
        heat_up(&mut arms.heat, &mut arms.jammed, PLANE_GUN_HEAT);
    }
    // The rack only lets go in the air, where the bomb has room to fall clear.
    if buttons & BUTTON_CROUCH != 0 && arms.bomb_cooldown == 0 && arms.bombs > 0 && height > PLANE_BOMB_MIN_HEIGHT {
        let id = world.alloc_id();
        let ttl = SHELL_TTL_TICKS * 2;
        world.rockets.insert(id, Rocket { kind: PROJECTILE_BOMB, owner: pilot, source: vid, position: pose.world_point(PLANE_BOMB_PIVOT), velocity: pose.velocity, ttl, item: 0, back: 0, flown: 0.0 });
        arms.bombs -= 1;
        arms.bomb_cooldown = PLANE_BOMB_COOLDOWN_TICKS;
    }
    arms
}

/// One tick of a hen's clutch for its driver in seat 0: fire lays an egg from under the tail, with the
/// hull's velocity and a little backwards, one every `HEN_EGG_COOLDOWN_TICKS`. An egg is a projectile of
/// its own that bounces until its fuse runs out (see `combat::step_grenade`). A clutch that is not full
/// grows one back every `HEN_EGG_REGROW_TICKS`, laying or not. A fallen hen lays nothing.
fn work_hen(world: &mut World, vid: u32, mut arms: PlaneArms, pose: &CarPose) -> PlaneArms {
    arms.bomb_cooldown = arms.bomb_cooldown.saturating_sub(1);
    arms.rearm = if arms.bombs < HEN_EGGS { arms.rearm + 1 } else { 0 };
    if arms.rearm >= HEN_EGG_REGROW_TICKS {
        arms.bombs += 1;
        arms.rearm = 0;
    }
    let Some(driver) = world.vehicles[&vid].seats[0] else { return arms };
    let Some(buttons) = world.players.get(&driver).map(|player| player.input.buttons()) else { return arms };
    if buttons & BUTTON_FIRE == 0 || arms.bomb_cooldown > 0 || arms.bombs == 0 || pose.up_y < WALKER_FALLEN_UP {
        return arms;
    }
    let toss = pose.world_dir([0.0, 0.0, HEN_EGG_TOSS]);
    let id = world.alloc_id();
    world.rockets.insert(
        id,
        Rocket {
            kind: PROJECTILE_EGG,
            owner: driver,
            source: vid,
            position: pose.world_point(HEN_VENT),
            velocity: std::array::from_fn(|i| pose.velocity[i] + toss[i]),
            ttl: HEN_EGG_FUSE_TICKS,
            item: 0,
            back: 0,
            flown: 0.0,
        },
    );
    arms.bombs -= 1;
    arms.shots = arms.shots.wrapping_add(1);
    arms.bomb_cooldown = HEN_EGG_COOLDOWN_TICKS;
    arms
}

/// One tick of a plane's ring mount for its gunner in seat 1. The gunner's look direction, taken into
/// the hull's frame, turns the ring all the way round and lifts the barrel within its travel; it holds
/// fire while it points within `PLANE_REAR_DEAD_ZONE` of the nose, at the pilot.
fn work_ring(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose, dt: f32) -> Gun {
    gun.cooldown = gun.cooldown.saturating_sub(1);
    cool(&mut gun.heat, &mut gun.jammed);
    let Some(gunner) = world.vehicles[&vid].seats[GUNNER_SEAT] else { return gun };
    let Some(input) = world.players.get(&gunner).map(|player| player.input) else { return gun };

    let local = pose.local_dir(look(from_mrad(input.yaw), from_mrad(input.pitch())));
    let yaw = (-local[0]).atan2(-local[2]);
    let pitch = local[1].clamp(-1.0, 1.0).asin().clamp(PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX);
    gun.yaw = wrap_angle(step_toward(gun.yaw, gun.yaw + wrap_angle(yaw - gun.yaw), PLANE_REAR_SLEW * dt));
    gun.pitch = step_toward(gun.pitch, pitch, PLANE_REAR_SLEW * dt);
    if input.buttons() & BUTTON_FIRE == 0 || gun.cooldown > 0 || gun.jammed || gun.yaw.abs() < PLANE_REAR_DEAD_ZONE {
        return gun;
    }
    let (ring, dir) = aim_from(pose, PLANE_REAR_PIVOT, gun.yaw, gun.pitch);
    let muzzle = std::array::from_fn(|i| ring[i] + dir[i] * PLANE_REAR_LENGTH);
    let dir = scatter(world, dir, PLANE_GUN_SPREAD);
    machine_gun(world, gunner, vid, muzzle, dir);
    gun.shots = gun.shots.wrapping_add(1);
    gun.cooldown = PLANE_REAR_COOLDOWN_TICKS;
    heat_up(&mut gun.heat, &mut gun.jammed, PLANE_REAR_HEAT);
    gun
}

impl Mechanic for Gunnery {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        step_shells(world, dt);
        let ids: Vec<u32> = world.vehicles.iter().filter(|(_, vehicle)| vehicle.gun.is_some()).map(|(&id, _)| id).collect();
        for vid in ids {
            let vehicle = &world.vehicles[&vid];
            let Some(mut gun) = vehicle.gun else { continue };
            let gunner = vehicle.seats[GUNNER_SEAT].and_then(|id| world.players.get(&id)).map(|player| player.input);
            let Some(pose) = world.physics.car_pose(vid) else { continue };
            if vehicle.kind == KIND_PLANE {
                let arms = vehicle.arms.unwrap_or_default();
                let gun = work_ring(world, vid, gun, &pose, dt);
                let arms = work_cockpit(world, vid, arms, &pose);
                if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                    vehicle.gun = Some(gun);
                    vehicle.arms = Some(arms);
                }
                continue;
            }
            if vehicle.kind == KIND_WALKER {
                let gun = work_walker_guns(world, vid, gun, &pose, dt);
                if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                    vehicle.gun = Some(gun);
                }
                continue;
            }
            if vehicle.kind == KIND_MORTAR {
                let gun = work_mortar(world, vid, gun, &pose, dt);
                if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                    vehicle.gun = Some(gun);
                }
                continue;
            }
            if vehicle.kind == KIND_STRV {
                let gun = work_driver_cannon(world, vid, gun, &pose, dt);
                if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                    vehicle.gun = Some(gun);
                }
                continue;
            }
            if vehicle.kind == KIND_HELI {
                let gun = work_autocannon(world, vid, gun, &pose, dt);
                let gun = work_pods(world, vid, gun, &pose);
                if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                    vehicle.gun = Some(gun);
                }
                continue;
            }

            gun.cooldown = gun.cooldown.saturating_sub(1);
            let mut trigger = false;
            // A tank whose turret has been shot off has no gun to work.
            let turret_on = vehicle.damage[DAMAGE_TURRET] != TURRET_GONE;
            if let (true, Some(input)) = (turret_on, gunner) {
                let wanted = wrap_angle(from_mrad(input.yaw) - pose.yaw());
                gun.yaw = wrap_angle(step_toward(gun.yaw, gun.yaw + wrap_angle(wanted - gun.yaw), TANK_TURRET_SLEW * dt));
                gun.pitch = step_toward(gun.pitch, from_mrad(input.pitch).clamp(TANK_BARREL_MIN, TANK_BARREL_MAX), TANK_BARREL_SLEW * dt);
                trigger = input.buttons() & BUTTON_FIRE != 0 && gun.cooldown == 0;
            }
            if let (true, Some(gunner_id)) = (trigger, world.vehicles[&vid].seats[GUNNER_SEAT]) {
                let (origin, dir) = aim(&pose, gun.yaw, gun.pitch);
                fire(world, PROJECTILE_SHELL, vid, origin, dir, TANK_BARREL_LENGTH, gunner_id);
                // Recoil: the hull is shoved back along the barrel (level, so it doesn't dig in).
                world.physics.kick_car(vid, [-dir[0] * SHELL_RECOIL, 0.0, -dir[2] * SHELL_RECOIL]);
                gun.shots = gun.shots.wrapping_add(1);
                gun.cooldown = SHELL_COOLDOWN_TICKS;
            }
            if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                vehicle.gun = Some(gun);
            }
        }
        // A hen has no gun, only its eggs.
        let hens: Vec<u32> = world.vehicles.iter().filter(|(_, vehicle)| vehicle.kind == KIND_HEN).map(|(&id, _)| id).collect();
        for vid in hens {
            let (Some(arms), Some(pose)) = (world.vehicles[&vid].arms, world.physics.car_pose(vid)) else { continue };
            let arms = work_hen(world, vid, arms, &pose);
            if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                vehicle.arms = Some(arms);
            }
        }
    }
}

#[cfg(test)]
mod tests;
