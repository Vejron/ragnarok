use super::Mechanic;
use super::combat::{Blast, Round, blast, first_player_hit, hitscan, look, rewind_ticks, scatter};
use crate::game::{Gun, PlaneArms, Rocket, World};
use crate::physics::CarPose;
use shared::consts::*;
use shared::protocol::{KIND_HELI, KIND_PLANE, PROJECTILE_BOMB, PROJECTILE_CANNON, PROJECTILE_SHELL, ShotEvent, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_MG, WEAPON_SHELL};
use shared::quant::{from_mrad, to_cm, wrap_angle};

/// Vehicle guns: the gunner's look direction aims the gun, and the fire button shoots it.
///
/// A helicopter's nose gun is an autocannon, worked from seat 1 as well (see `work_autocannon`): its
/// rounds are shells in miniature. A plane's pilot fires two forward machine guns and drops bombs,
/// and its gunner works a machine gun on a ring behind them (see `work_cockpit` and `work_ring`). A tank's seat 1 is its gunner. Their input drives the turret instead of anything on foot, while
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

/// A shell of `kind` going off at `point`: the burst everyone sees, and the blast. An autocannon round
/// is a third of a tank shell's reach, and hits for less.
fn burst(world: &mut World, kind: u8, owner: u32, point: [f32; 3], dir: [f32; 3]) {
    let (weapon, radius, damage, knock) = match kind {
        PROJECTILE_CANNON => (WEAPON_AUTOCANNON, CANNON_RADIUS, CANNON_DAMAGE, CANNON_KNOCK),
        PROJECTILE_BOMB => (WEAPON_BOMB, BOMB_RADIUS, BOMB_DAMAGE, BOMB_KNOCK),
        _ => (WEAPON_SHELL, EXPLOSION_RADIUS, EXPLOSION_DAMAGE, EXPLOSION_KNOCK),
    };
    let [x, y, z] = point.map(to_cm);
    world.shots.push(ShotEvent(owner, weapon, x, y, z));
    // A gunner who has left or died still gets the blast, just not the credit.
    let attacker = world.players.contains_key(&owner).then_some(owner);
    blast(world, Blast { point, radius, damage, knock, dir, attacker, weapon });
}

/// Fires a shell of `kind` from vehicle `vid` for `gunner`, from `origin` along `dir` (the muzzle's
/// root and the barrel's heading, `length` of barrel ahead): it leaves the muzzle and flies on its own
/// from there, so it takes time to arrive and falls on the way.
fn fire(world: &mut World, kind: u8, vid: u32, origin: [f32; 3], dir: [f32; 3], length: f32, gunner: u32) {
    // A barrel poking into a wall bursts the shell there, instead of starting it on the far side.
    if let Some(t) = world.physics.cast_ray(origin, dir, length, vid) {
        burst(world, kind, gunner, std::array::from_fn(|i| origin[i] + dir[i] * t), dir);
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
            velocity: dir.map(|v| v * SHELL_SPEED),
            ttl: SHELL_TTL_TICKS,
        },
    );
}

/// Flies every shell one tick: gravity, then along its velocity until it meets a wall, a vehicle
/// or a stickman, or its time runs out. It bursts where it stopped.
fn step_shells(world: &mut World, dt: f32) {
    let falls = |kind| kind == PROJECTILE_SHELL || kind == PROJECTILE_CANNON || kind == PROJECTILE_BOMB;
    let ids: Vec<u32> = world.rockets.iter().filter(|(_, rocket)| falls(rocket.kind)).map(|(&id, _)| id).collect();
    for id in ids {
        let Some(mut shell) = world.rockets.get(&id).copied() else { continue };
        shell.velocity[1] -= GRAVITY * dt;
        let speed = shell.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        let dir = shell.velocity.map(|v| v / speed);
        let step = speed * dt;
        let wall = world.physics.cast_ray(shell.position, dir, step, shell.source);
        // Stickmen are not physics bodies, so test the shell's path against each one.
        let player = first_player_hit(world, shell.position, dir, wall.unwrap_or(step), Some(shell.owner), 0);
        shell.ttl = shell.ttl.saturating_sub(1);
        if let Some(t) = player.map(|(_, t)| t).or(wall) {
            world.rockets.remove(&id);
            burst(world, shell.kind, shell.owner, std::array::from_fn(|i| shell.position[i] + dir[i] * t), dir);
            continue;
        }
        shell.position = std::array::from_fn(|i| shell.position[i] + dir[i] * step);
        if shell.position[1] < KILL_Y {
            world.rockets.remove(&id);
        } else if shell.ttl == 0 {
            world.rockets.remove(&id);
            burst(world, shell.kind, shell.owner, shell.position, dir);
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

    let local = pose.local_dir(look(from_mrad(input.3), from_mrad(input.pitch())));
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
    let round = Round { damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: PLANE_GUN_RANGE };
    let (end, struck) = hitscan(world, shooter, muzzle, dir, &round, vid, back);
    if struck {
        let [x, y, z] = std::array::from_fn(|i| to_cm(muzzle[i] + dir[i] * end));
        world.shots.push(ShotEvent(shooter, WEAPON_MG, x, y, z));
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
        world.rockets.insert(id, Rocket { kind: PROJECTILE_BOMB, owner: pilot, source: vid, position: pose.world_point(PLANE_BOMB_PIVOT), velocity: pose.velocity, ttl });
        arms.bombs -= 1;
        arms.bomb_cooldown = PLANE_BOMB_COOLDOWN_TICKS;
    }
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

    let local = pose.local_dir(look(from_mrad(input.3), from_mrad(input.pitch())));
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
            if vehicle.kind == KIND_HELI {
                let gun = work_autocannon(world, vid, gun, &pose, dt);
                if let Some(vehicle) = world.vehicles.get_mut(&vid) {
                    vehicle.gun = Some(gun);
                }
                continue;
            }

            gun.cooldown = gun.cooldown.saturating_sub(1);
            let mut trigger = false;
            if let Some(input) = gunner {
                let wanted = wrap_angle(from_mrad(input.3) - pose.yaw());
                gun.yaw = wrap_angle(step_toward(gun.yaw, gun.yaw + wrap_angle(wanted - gun.yaw), TANK_TURRET_SLEW * dt));
                gun.pitch = step_toward(gun.pitch, from_mrad(input.5).clamp(TANK_BARREL_MIN, TANK_BARREL_MAX), TANK_BARREL_SLEW * dt);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Command, Game, Seat};
    use crate::physics::STANDING_Y;
    use shared::protocol::{InputFrame, KIND_HELI, KIND_TANK, PROJECTILE_BOMB, WEAPON_BOMB, WEAPON_MG};
    use shared::quant::to_mrad;
    use shared::world::{Road, WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

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

    fn join(game: &mut Game) -> (u32, mpsc::Receiver<crate::game::Outgoing>) {
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply });
        (response.try_recv().unwrap().unwrap(), inbox)
    }

    /// Puts `id` straight into a seat of vehicle `vid`.
    fn seat(game: &mut Game, id: u32, vid: u32, index: usize) {
        game.world.vehicles.get_mut(&vid).unwrap().seats[index] = Some(id);
        game.world.players.get_mut(&id).unwrap().seat = Some(Seat { vehicle: vid, index });
    }

    fn tank(game: &Game) -> u32 {
        *game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_TANK).expect("tanks are placed").0
    }

    fn heli(game: &Game) -> u32 {
        *game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_HELI).expect("a helicopter is placed").0
    }

    /// Feeds the gunner's look and trigger for `ticks` ticks.
    fn aim_and_hold(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, pitch: f32, buttons: u8, ticks: u32) {
        for _ in 0..ticks {
            *seq += 1;
            game.handle(Command::Input(id, 0, vec![InputFrame(*seq, 0, 0, to_mrad(yaw), buttons, to_mrad(pitch), 0, 0)]));
            game.step();
        }
    }

    /// A tank with a gunner in seat 1, settled, with the gunner's first frames queued.
    fn crewed() -> (Game, u32, u32, u32) {
        let mut game = game();
        let (gunner, _inbox) = join(&mut game);
        let vid = tank(&game);
        seat(&mut game, gunner, vid, 1);
        let mut seq = 0;
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, 0, 90);
        (game, gunner, vid, seq)
    }

    #[test]
    fn the_turret_follows_the_gunner_and_the_barrel_is_limited() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let hull = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, hull + 1.0, 0.3, 0, 180);
        let gun = game.world.vehicles[&vid].gun.unwrap();
        assert!((gun.yaw - 1.0).abs() < 0.02 && (gun.pitch - 0.3).abs() < 0.02, "{gun:?}");
        // Looking far up asks for more than the barrel can do.
        aim_and_hold(&mut game, gunner, &mut seq, hull + 1.0, 1.2, 0, 180);
        assert!((game.world.vehicles[&vid].gun.unwrap().pitch - TANK_BARREL_MAX).abs() < 0.01);
        aim_and_hold(&mut game, gunner, &mut seq, hull - 2.5, 0.0, 0, 120);
        let turned = game.world.vehicles[&vid].gun.unwrap().yaw;
        assert!(turned < 1.0, "the turret slews at a limited rate, the short way round: {turned}");
    }

    #[test]
    fn turret_slew_is_rate_limited() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let hull = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, hull + 2.0, 0.0, 0, 30);
        let yaw = game.world.vehicles[&vid].gun.unwrap().yaw;
        // 30 ticks is half a second: at most 0.7 rad, plus the jitter buffer's lag.
        assert!(yaw > 0.3 && yaw <= TANK_TURRET_SLEW * 0.5 + 0.01, "{yaw}");
    }

    #[test]
    fn firing_hits_and_throws_a_stickman_in_the_line_of_fire() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let (victim, _inbox) = join(&mut game);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let gun = game.world.vehicles[&vid].gun.unwrap();
        let (origin, dir) = aim(&pose, gun.yaw, gun.pitch);
        game.world.players.get_mut(&victim).unwrap().character.position = std::array::from_fn(|i| origin[i] + dir[i] * 25.0);

        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 3);
        assert_eq!(game.world.vehicles[&vid].gun.unwrap().shots, 1);
        // 25 m at 140 m/s is about ten ticks: the shell is on its way, and nobody is hurt yet.
        assert_eq!(game.world.rockets.len(), 1, "the shell is in flight");
        assert_eq!(game.world.players[&victim].character.knock, [0.0, 0.0]);
        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, 0, 30);
        assert!(game.world.rockets.is_empty(), "and then it has burst");
        let character = &game.world.players[&victim].character;
        let speed = character.knock[0].hypot(character.knock[1]);
        assert!(speed > 10.0, "blast knockback {speed}");
        // Thrown away from the tank, along the shell's way.
        assert!(character.knock[0] * dir[0] + character.knock[1] * dir[2] > 0.0);
        assert!(character.vy > 3.0);
    }

    #[test]
    fn a_direct_hit_kills_and_the_gunner_gets_the_credit() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let (victim, _inbox) = join(&mut game);
        let (near, _inbox2) = join(&mut game);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let gun = game.world.vehicles[&vid].gun.unwrap();
        let (origin, dir) = aim(&pose, gun.yaw, gun.pitch);
        let at = |metres: f32, side: f32| std::array::from_fn::<f32, 3, _>(|i| origin[i] + dir[i] * metres + if i == 0 { side } else { 0.0 });
        game.world.players.get_mut(&victim).unwrap().character.position = at(25.0, 0.0);
        // Caught at the edge of the blast: hurt, not dead.
        game.world.players.get_mut(&near).unwrap().character.position = at(25.0, 5.0);

        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 40);
        assert!(!game.world.players[&victim].alive());
        assert_eq!(game.world.players[&gunner].kills, 1);
        let bystander = &game.world.players[&near];
        assert!(bystander.alive() && bystander.hp < PLAYER_MAX_HP, "{}", bystander.hp);
    }

    #[test]
    fn the_gun_reloads_between_shots_and_a_stickman_far_away_is_unharmed() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let (bystander, _inbox) = join(&mut game);
        game.world.players.get_mut(&bystander).unwrap().character.position = [0.0, STANDING_Y, 150.0];
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
        assert_eq!(game.world.vehicles[&vid].gun.unwrap().shots, 1, "trigger held for a second: one shell");
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
        assert_eq!(game.world.vehicles[&vid].gun.unwrap().shots, 2, "reloaded after a second and a half");
        assert_eq!(game.world.players[&bystander].character.knock, [0.0, 0.0]);
    }

    #[test]
    fn firing_kicks_the_tank_backwards() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        let before = game.world.physics.car_pose(vid).unwrap();
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 3);
        let after = game.world.physics.car_pose(vid).unwrap();
        // The barrel points along the hull's nose, so the recoil is along its tail.
        assert!(after.forward_speed < -0.5, "{}", after.forward_speed);
        assert!(before.forward_speed.abs() < 0.1);
    }

    #[test]
    fn nobody_in_the_gunner_seat_means_no_shooting() {
        let mut game = game();
        let (driver, _inbox) = join(&mut game);
        let vid = tank(&game);
        seat(&mut game, driver, vid, 0);
        let mut seq = 0;
        // The driver holds fire, but only the gunner's seat works the gun.
        aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, BUTTON_FIRE, 120);
        assert_eq!(game.world.vehicles[&vid].gun.unwrap().shots, 0);
    }

    #[test]
    fn the_shell_stops_at_the_first_thing_it_meets() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let (blocker, _a) = join(&mut game);
        let (behind, _b) = join(&mut game);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (origin, dir) = aim(&pose, 0.0, 0.0);
        game.world.players.get_mut(&blocker).unwrap().character.position = std::array::from_fn(|i| origin[i] + dir[i] * 15.0);
        game.world.players.get_mut(&behind).unwrap().character.position = std::array::from_fn(|i| origin[i] + dir[i] * 40.0);
        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 40);
        let blocker_hp = game.world.players[&blocker].hp;
        assert!(blocker_hp < PLAYER_MAX_HP, "{blocker_hp}");
        assert!(game.world.rockets.is_empty());
        assert_eq!(game.world.players[&behind].character.knock, [0.0, 0.0], "outside the blast radius");
    }

    /// Fires level, or at `pitch`, from a fresh tank, and flies the shell until it is gone. Returns
    /// the ticks it took and how far from the tank it was last seen, level, in metres.
    fn fly(pitch: f32) -> (u32, f32) {
        let (mut game, gunner, vid, mut seq) = crewed();
        let pose = game.world.physics.car_pose(vid).unwrap();
        // Let the barrel come up to `pitch` before the trigger.
        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), pitch, 0, 60);
        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), pitch, BUTTON_FIRE, 1);
        let mut last = [0.0; 3];
        let mut ticks = 0;
        while ticks < 600 {
            match game.world.rockets.values().next() {
                Some(shell) => last = shell.position,
                None if ticks > 0 => break,
                None => {}
            }
            aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), pitch, 0, 1);
            ticks += 1;
        }
        (ticks, (last[0] - pose.position[0]).hypot(last[2] - pose.position[2]))
    }

    #[test]
    fn a_shell_takes_time_and_falls_so_a_raised_barrel_reaches_further() {
        let (level_ticks, level) = fly(0.0);
        let (raised_ticks, raised) = fly(0.12);
        assert!(level_ticks > 10, "it is not instant: {level_ticks} ticks");
        assert!(level > 20.0 && level < 100.0, "a level shell drops into the ground well short of its range: {level}");
        assert!(raised > level * 1.8, "{raised} against {level}");
        assert!(raised_ticks > level_ticks, "the longer arc takes longer: {raised_ticks} against {level_ticks}");
    }

    #[test]
    fn a_shell_in_flight_is_in_the_snapshot_and_falls() {
        let (mut game, gunner, vid, mut seq) = crewed();
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 3);
        let listed = game.projectile_states();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].1, PROJECTILE_SHELL);
        let before = game.world.rockets.values().next().unwrap().velocity;
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, 0, 5);
        let after = game.world.rockets.values().next().unwrap().velocity;
        assert!(after[1] < before[1] - 1.0, "gravity pulls it down: {before:?} to {after:?}");
    }

    /// A helicopter with a gunner in seat 1, settled on the ground, with the gunner's first frames queued.
    fn heli_crewed() -> (Game, u32, u32, u32) {
        let mut game = game();
        let (gunner, _inbox) = join(&mut game);
        let vid = heli(&game);
        seat(&mut game, gunner, vid, 1);
        let mut seq = 0;
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, 0, 90);
        (game, gunner, vid, seq)
    }

    #[test]
    fn the_nose_gun_follows_the_gunner_within_its_travel() {
        let (mut game, gunner, vid, mut seq) = heli_crewed();
        let hull = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, hull + 0.5, -0.3, 0, 60);
        let gun = game.world.vehicles[&vid].gun.unwrap();
        assert!((gun.yaw - 0.5).abs() < 0.03 && (gun.pitch + 0.3).abs() < 0.03, "{gun:?}");
        // Looking behind or high above asks for more than the mount can do.
        aim_and_hold(&mut game, gunner, &mut seq, hull + 2.5, 1.0, 0, 120);
        let gun = game.world.vehicles[&vid].gun.unwrap();
        assert!((gun.yaw - HELI_GUN_YAW_LIMIT).abs() < 0.01, "{gun:?}");
        assert!((gun.pitch - HELI_GUN_PITCH_MAX).abs() < 0.01, "{gun:?}");
    }

    #[test]
    fn the_nose_gun_throws_small_shells_that_burst_a_third_as_wide() {
        let (mut game, gunner, vid, mut seq) = heli_crewed();
        let (victim, _inbox) = join(&mut game);
        let (bystander, _inbox2) = join(&mut game);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (origin, dir) = aim_from(&pose, HELI_GUN_PIVOT, 0.0, 0.0);
        let at: [f32; 3] = std::array::from_fn(|i| origin[i] + dir[i] * 15.0);
        game.world.players.get_mut(&victim).unwrap().character.position = at;
        // Three metres to the side: inside a tank shell's blast, outside this one's.
        let mut beside = at;
        beside[0] += 4.0;
        game.world.players.get_mut(&bystander).unwrap().character.position = beside;
        let before = game.world.players[&victim].hp;

        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 2);
        assert_eq!(game.world.vehicles[&vid].gun.unwrap().shots, 1);
        let listed = game.projectile_states();
        assert_eq!(listed.len(), 1, "the round is in flight");
        assert_eq!(listed[0].1, PROJECTILE_CANNON);
        assert_eq!(game.world.players[&victim].hp, before, "and has not arrived");

        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, 0, 30);
        assert!(game.world.rockets.is_empty(), "it has burst");
        let hp = game.world.players[&victim].hp;
        assert!(hp < before && before - hp < 60, "hurt, but less than a tank shell: {before} to {hp}");
        assert_eq!(game.world.players[&bystander].hp, PLAYER_MAX_HP, "outside the small blast");
    }

    #[test]
    fn the_nose_gun_fires_a_burst_at_its_rate_and_only_for_the_gunner() {
        let (mut game, gunner, vid, mut seq) = heli_crewed();
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
        let fired = game.world.vehicles[&vid].gun.unwrap().shots;
        let expected = (60 / HELI_GUN_COOLDOWN_TICKS) as u8;
        assert!(fired.abs_diff(expected) <= 2, "{fired} rounds in a second, expected about {expected}");

        // The pilot holding fire works nothing: only seat 1 has the gun.
        let mut alone = self::game();
        let (pilot, _inbox) = join(&mut alone);
        let vid = heli(&alone);
        seat(&mut alone, pilot, vid, 0);
        let mut seq = 0;
        aim_and_hold(&mut alone, pilot, &mut seq, 0.0, 0.0, BUTTON_FIRE, 30);
        assert_eq!(alone.world.vehicles[&vid].gun.unwrap().shots, 0);
    }

    fn plane(game: &Game) -> u32 {
        *game.world.vehicles.iter().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).expect("a plane is placed").0
    }

    /// A parked plane with a crewman in `seat`, settled, with their first frames queued looking along the nose.
    fn plane_crewed(index: usize) -> (Game, u32, u32, u32) {
        let mut game = game();
        let (crewman, _inbox) = join(&mut game);
        let vid = plane(&game);
        seat(&mut game, crewman, vid, index);
        let mut seq = 0;
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, crewman, &mut seq, yaw, 0.0, 0, 90);
        (game, crewman, vid, seq)
    }

    /// The shot events of `weapon` seen while looking level along `yaw` and holding `buttons` for `ticks`.
    fn hold_watching(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, buttons: u8, ticks: u32, weapon: u8) -> Vec<ShotEvent> {
        let mut seen = vec![];
        for _ in 0..ticks {
            aim_and_hold(game, id, seq, yaw, 0.0, buttons, 1);
            seen.extend(game.world.shots.drain(..).filter(|shot| shot.1 == weapon));
        }
        seen
    }

    #[test]
    fn the_forward_guns_take_turns_and_hit_what_is_ahead_of_the_nose() {
        let (mut game, pilot, vid, mut seq) = plane_crewed(0);
        let (victim, _inbox) = join(&mut game);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (muzzle, dir) = aim_from(&pose, PLANE_GUN_PIVOTS[0], 0.0, 0.0);
        game.world.players.get_mut(&victim).unwrap().character.position = std::array::from_fn(|i| muzzle[i] + dir[i] * 25.0 + if i == 1 { -0.3 } else { 0.0 });
        let hp = game.world.vehicles[&vid].hp;

        let hits = hold_watching(&mut game, pilot, &mut seq, pose.yaw(), BUTTON_FIRE, 30, WEAPON_MG);
        let arms = game.world.vehicles[&vid].arms.unwrap();
        let expected = (30 / PLANE_GUN_COOLDOWN_TICKS) as u8;
        assert!(arms.shots.abs_diff(expected) <= 1, "{} rounds in half a second, expected about {expected}", arms.shots);
        assert!(game.world.players[&victim].hp < PLAYER_MAX_HP, "the stickman ahead is hit");
        assert!(!hits.is_empty() && hits.iter().all(|shot| shot.0 == pilot), "{hits:?}");
        assert_eq!(game.world.vehicles[&vid].hp, hp, "its own guns never hurt the plane");
        assert!(game.world.rockets.is_empty(), "rounds are hitscans, not projectiles");
    }

    #[test]
    fn the_forward_guns_jam_when_hot_and_clear_when_cool() {
        let (mut game, pilot, vid, mut seq) = plane_crewed(0);
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        let mut jammed_at = None;
        for tick in 0..60 * 8 {
            aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, BUTTON_FIRE, 1);
            if game.world.vehicles[&vid].arms.unwrap().jammed {
                jammed_at = Some(tick);
                break;
            }
        }
        let jammed_at = jammed_at.expect("the guns jam");
        assert!((60 * 3..60 * 6).contains(&jammed_at), "after about four seconds: {jammed_at} ticks");
        let shots = game.world.vehicles[&vid].arms.unwrap().shots;
        aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
        assert_eq!(game.world.vehicles[&vid].arms.unwrap().shots, shots, "a jammed gun does not fire");
        assert!(game.vehicle_states().iter().find(|v| v.0 == vid).unwrap().13.unwrap().4 & shared::protocol::ARMS_JAMMED != 0);
        aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, 0, 60 * 3);
        assert!(!game.world.vehicles[&vid].arms.unwrap().jammed, "cool again");
        aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, BUTTON_FIRE, 20);
        assert!(game.world.vehicles[&vid].arms.unwrap().shots != shots, "and firing");
    }

    #[test]
    fn a_bomb_leaves_with_the_planes_speed_and_bursts_where_it_lands() {
        let (mut game, pilot, vid, mut seq) = plane_crewed(0);
        let pose = game.world.physics.car_pose(vid).unwrap();
        // Lift it up off the road and send it along at speed, then let one go.
        game.world.physics.reset_car(vid, [pose.position[0], 40.0, 60.0], pose.yaw());
        let (sin, cos) = pose.yaw().sin_cos();
        game.world.physics.kick_car(vid, [-sin * 30.0, 0.0, -cos * 30.0]);
        aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, BUTTON_CROUCH, 1);
        aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, 0, 1);
        let bombs: Vec<_> = game.world.rockets.values().filter(|rocket| rocket.kind == PROJECTILE_BOMB).copied().collect();
        assert_eq!(bombs.len(), 1, "{:?}", game.world.rockets);
        assert_eq!(game.world.vehicles[&vid].arms.unwrap().bombs, PLANE_BOMBS - 1);
        let along = -sin * bombs[0].velocity[0] - cos * bombs[0].velocity[2];
        assert!(along > 25.0, "it carries the plane's speed: {:?}", bombs[0].velocity);
        let dropped_at = bombs[0].position;

        let bursts = hold_watching(&mut game, pilot, &mut seq, pose.yaw(), 0, 120, WEAPON_BOMB);
        assert_eq!(bursts.len(), 1, "it lands and bursts");
        let at = [bursts[0].2, bursts[0].3, bursts[0].4].map(|cm| cm as f32 / 100.0);
        let ahead = -sin * (at[0] - dropped_at[0]) - cos * (at[2] - dropped_at[2]);
        assert!(ahead > 20.0 && at[1] < 2.0, "well ahead of where it was let go, on the ground: {at:?}");
    }

    #[test]
    fn the_rack_holds_four_bombs_and_fills_again_on_the_ground() {
        let (mut game, pilot, vid, mut seq) = plane_crewed(0);
        let pose = game.world.physics.car_pose(vid).unwrap();
        // On the ground the rack holds on to them.
        aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, BUTTON_CROUCH, 60);
        assert_eq!(game.world.vehicles[&vid].arms.unwrap().bombs, PLANE_BOMBS);
        // High up, held crouch drops one every half second, and the fifth is not there.
        game.world.physics.reset_car(vid, [pose.position[0], 120.0, 60.0], pose.yaw());
        let mut dropped = 0;
        for _ in 0..PLANE_BOMB_COOLDOWN_TICKS * 5 + 5 {
            let before: Vec<u32> = game.world.rockets.keys().copied().collect();
            aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, BUTTON_CROUCH, 1);
            dropped += game.world.rockets.iter().filter(|(id, r)| r.kind == PROJECTILE_BOMB && !before.contains(id)).count();
        }
        assert_eq!((dropped, game.world.vehicles[&vid].arms.unwrap().bombs), (PLANE_BOMBS as usize, 0));
        // Back home and standing still, the ground crew loads it again.
        game.world.physics.reset_car(vid, pose.position, pose.yaw());
        aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, 0, PLANE_REARM_TICKS + 90);
        assert_eq!(game.world.vehicles[&vid].arms.unwrap().bombs, PLANE_BOMBS);
    }

    #[test]
    fn the_rear_gun_turns_all_the_way_round_but_never_fires_at_the_pilot() {
        let (mut game, gunner, vid, mut seq) = plane_crewed(1);
        let hull = game.world.physics.car_pose(vid).unwrap().yaw();
        // Looking ahead over the pilot's head: the ring follows, the trigger does nothing.
        aim_and_hold(&mut game, gunner, &mut seq, hull, 0.2, BUTTON_FIRE, 60);
        let gun = game.world.vehicles[&vid].gun.unwrap();
        assert!(gun.yaw.abs() < 0.05 && (gun.pitch - 0.2).abs() < 0.05, "{gun:?}");
        assert_eq!(gun.shots, 0);
        // Straight behind, and up high.
        aim_and_hold(&mut game, gunner, &mut seq, hull + std::f32::consts::PI, 1.4, BUTTON_FIRE, 90);
        let gun = game.world.vehicles[&vid].gun.unwrap();
        assert!(wrap_angle(gun.yaw - std::f32::consts::PI).abs() < 0.05, "{gun:?}");
        assert!((gun.pitch - PLANE_REAR_PITCH_MAX).abs() < 0.01, "{gun:?}");
        assert!(gun.shots > 3, "{gun:?}");
        assert_eq!(game.world.vehicles[&vid].arms.unwrap().shots, 0, "the gunner's trigger is not the pilot's");
    }

    #[test]
    fn the_rear_gun_hits_a_stickman_behind_and_only_its_gunner_works_it() {
        let (mut game, gunner, vid, mut seq) = plane_crewed(1);
        let (victim, _inbox) = join(&mut game);
        let hull = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, gunner, &mut seq, hull + std::f32::consts::PI, 0.0, 0, 60);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let gun = game.world.vehicles[&vid].gun.unwrap();
        let (ring, dir) = aim_from(&pose, PLANE_REAR_PIVOT, gun.yaw, gun.pitch);
        game.world.players.get_mut(&victim).unwrap().character.position = std::array::from_fn(|i| ring[i] + dir[i] * 20.0 + if i == 1 { -0.3 } else { 0.0 });
        aim_and_hold(&mut game, gunner, &mut seq, hull + std::f32::consts::PI, 0.0, BUTTON_FIRE, 30);
        assert!(game.world.players[&victim].hp < PLAYER_MAX_HP);

        // A pilot alone works only the forward guns.
        let (mut game, pilot, vid, mut seq) = plane_crewed(0);
        let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
        aim_and_hold(&mut game, pilot, &mut seq, yaw + std::f32::consts::PI, 0.0, BUTTON_FIRE, 30);
        assert_eq!(game.world.vehicles[&vid].gun.unwrap().shots, 0);
    }

    #[test]
    fn machine_gun_rounds_hurt_a_plane_more_than_a_car() {
        let mut game = game();
        let (shooter, _inbox) = join(&mut game);
        let target = plane(&game);
        let car = *game.world.vehicles.iter().find(|(_, v)| v.kind == shared::protocol::KIND_JEEP).unwrap().0;
        // The broad-phase learns of the vehicles in the first step.
        game.step();
        let round = Round { damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: 50.0 };
        for vid in [target, car] {
            let at = game.world.physics.car_pose(vid).unwrap().position;
            let origin = [at[0], at[1] + 20.0, at[2]];
            let (_, struck) = hitscan(&mut game.world, shooter, origin, [0.0, -1.0, 0.0], &round, 0, 0);
            assert!(struck);
        }
        assert_eq!(game.world.vehicles[&target].hp, shared::protocol::vehicle_hp(KIND_PLANE) - PLANE_GUN_PLANE_DAMAGE);
        assert_eq!(game.world.vehicles[&car].hp, 100 - PLANE_GUN_VEHICLE_DAMAGE);
    }
}
