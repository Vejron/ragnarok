use super::Mechanic;
use super::combat::{Blast, blast, damage, damage_vehicle, first_player_hit, look, rewind_ticks, scatter};
use crate::game::{Gun, Rocket, World};
use crate::physics::CarPose;
use shared::consts::*;
use shared::protocol::{KIND_HELI, PROJECTILE_SHELL, ShotEvent, WEAPON_CHAINGUN, WEAPON_SHELL};
use shared::quant::{from_mrad, to_cm, wrap_angle};

/// Vehicle guns: the gunner's look direction aims the gun, and the fire button shoots it.
///
/// A helicopter's nose gun is a machine gun, worked from seat 1 as well (see `work_chaingun`). A
/// tank's seat 1 is its gunner. Their input drives the turret instead of anything on foot, while
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

/// A shell going off at `point`: the burst everyone sees, and the blast.
fn burst(world: &mut World, owner: u32, point: [f32; 3], dir: [f32; 3]) {
    let [x, y, z] = point.map(to_cm);
    world.shots.push(ShotEvent(owner, WEAPON_SHELL, x, y, z));
    // A gunner who has left or died still gets the blast, just not the credit.
    let attacker = world.players.contains_key(&owner).then_some(owner);
    blast(world, Blast { point, radius: EXPLOSION_RADIUS, damage: EXPLOSION_DAMAGE, knock: EXPLOSION_KNOCK, dir, attacker, weapon: WEAPON_SHELL });
}

/// Fires from vehicle `vid` for `gunner`: a shell leaves the muzzle along the barrel and flies on
/// its own from there, so it takes time to arrive and falls on the way.
fn fire(world: &mut World, vid: u32, pose: &CarPose, yaw: f32, pitch: f32, gunner: u32) {
    let (origin, dir) = aim(pose, yaw, pitch);
    // A barrel poking into a wall bursts the shell there, instead of starting it on the far side.
    if let Some(t) = world.physics.cast_ray(origin, dir, TANK_BARREL_LENGTH, vid) {
        burst(world, gunner, std::array::from_fn(|i| origin[i] + dir[i] * t), dir);
        return;
    }
    let id = world.alloc_id();
    world.rockets.insert(
        id,
        Rocket {
            kind: PROJECTILE_SHELL,
            owner: gunner,
            source: vid,
            position: std::array::from_fn(|i| origin[i] + dir[i] * TANK_BARREL_LENGTH),
            velocity: dir.map(|v| v * SHELL_SPEED),
            ttl: SHELL_TTL_TICKS,
        },
    );
}

/// Flies every shell one tick: gravity, then along its velocity until it meets a wall, a vehicle
/// or a stickman, or its time runs out. It bursts where it stopped.
fn step_shells(world: &mut World, dt: f32) {
    let ids: Vec<u32> = world.rockets.iter().filter(|(_, rocket)| rocket.kind == PROJECTILE_SHELL).map(|(&id, _)| id).collect();
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
            burst(world, shell.owner, std::array::from_fn(|i| shell.position[i] + dir[i] * t), dir);
            continue;
        }
        shell.position = std::array::from_fn(|i| shell.position[i] + dir[i] * step);
        if shell.position[1] < KILL_Y {
            world.rockets.remove(&id);
        } else if shell.ttl == 0 {
            world.rockets.remove(&id);
            burst(world, shell.owner, shell.position, dir);
        } else {
            world.rockets.insert(id, shell);
        }
    }
}

/// One tick of a helicopter's nose gun, for the gunner in seat 1 if there is one, and the gun as it is
/// after it. The gunner's look direction is taken in the world and expressed in the hull's frame, so the
/// gun holds its aim while the helicopter leans and turns, until the target leaves its travel. A shot
/// is a ray from the mount, which stickmen and vehicles in its way take damage from.
fn work_chaingun(world: &mut World, vid: u32, mut gun: Gun, pose: &CarPose, dt: f32) -> Gun {
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
    let back = rewind_ticks(world.tick, input.view());
    let wall = world.physics.cast_ray_hit(origin, dir, HELI_GUN_RANGE, vid);
    let reach = wall.map_or(HELI_GUN_RANGE, |(t, _)| t);
    let hit = first_player_hit(world, origin, dir, reach, Some(gunner), back);
    if let Some((victim, _)) = hit {
        damage(world, victim, HELI_GUN_DAMAGE, Some(gunner), WEAPON_CHAINGUN);
    } else if let Some((_, Some(car))) = wall {
        damage_vehicle(world, car, (HELI_GUN_DAMAGE / 2).max(1), Some(gunner));
    }
    let end = hit.map_or(reach, |(_, t)| t);
    let [x, y, z] = std::array::from_fn(|i| to_cm(origin[i] + dir[i] * end));
    world.shots.push(ShotEvent(gunner, WEAPON_CHAINGUN, x, y, z));
    gun.shots = gun.shots.wrapping_add(1);
    gun.cooldown = HELI_GUN_COOLDOWN_TICKS;
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
            if vehicle.kind == KIND_HELI {
                let gun = work_chaingun(world, vid, gun, &pose, dt);
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
                fire(world, vid, &pose, gun.yaw, gun.pitch, gunner_id);
                // Recoil: the hull is shoved back along the barrel (level, so it doesn't dig in).
                let (_, dir) = aim(&pose, gun.yaw, gun.pitch);
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
    use shared::protocol::{InputFrame, KIND_HELI, KIND_TANK};
    use shared::quant::to_mrad;
    use shared::world::{Road, WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

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
    fn the_nose_gun_hurts_what_it_points_at() {
        let (mut game, gunner, vid, mut seq) = heli_crewed();
        let (victim, _inbox) = join(&mut game);
        let pose = game.world.physics.car_pose(vid).unwrap();
        let (origin, dir) = aim_from(&pose, HELI_GUN_PIVOT, 0.0, 0.0);
        game.world.players.get_mut(&victim).unwrap().character.position = std::array::from_fn(|i| origin[i] + dir[i] * 15.0);
        let before = game.world.players[&victim].hp;
        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 6);
        assert!(game.world.vehicles[&vid].gun.unwrap().shots >= 1);
        assert!(game.world.players[&victim].hp < before, "hit");
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
}
