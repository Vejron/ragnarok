use super::Mechanic;
use super::combat::{Blast, blast, first_player_hit};
use crate::game::World;
use crate::physics::CarPose;
use shared::consts::*;
use shared::protocol::WEAPON_SHELL;
use shared::quant::{from_mrad, wrap_angle};

/// Turret guns: the gunner's look direction aims the turret, and the fire button shoots shells.
///
/// A tank's seat 1 is its gunner. Their input drives the turret instead of anything on foot, while
/// the driver keeps the hull. The turret is stabilised: it follows the gunner's *world* heading
/// at a limited slew rate, so turning the hull does not drag the aim around. A shell is a hitscan:
/// it flies along the barrel, stops at the first wall, vehicle or stickman, and explodes there,
/// throwing everyone nearby and shoving vehicles.
pub struct Gunnery;

/// The seat that works the gun.
const GUNNER_SEAT: usize = 1;

fn step_toward(current: f32, target: f32, step: f32) -> f32 {
    current + (target - current).clamp(-step, step)
}

/// Where the shell leaves the turret ring, and its unit direction, for a gun at `yaw` / `pitch`
/// (hull frame) on a hull at `pose`. The client draws the barrel and the tracer from the same maths.
pub fn aim(pose: &CarPose, yaw: f32, pitch: f32) -> ([f32; 3], [f32; 3]) {
    let local = [-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()];
    (pose.world_point(TANK_TURRET_PIVOT), pose.world_dir(local))
}

/// Fires from vehicle `vid` for `gunner`: finds what the shell hits and blows it up. Returns how far it flew.
fn shoot(world: &mut World, vid: u32, pose: &CarPose, yaw: f32, pitch: f32, gunner: Option<u32>) -> f32 {
    let (origin, dir) = aim(pose, yaw, pitch);
    let wall = world.physics.cast_ray(origin, dir, SHELL_RANGE, vid);
    let mut reach = wall.unwrap_or(SHELL_RANGE);
    let mut hit = wall.is_some();
    // Stickmen are not physics bodies, so test the shell's path against each one.
    if let Some((_, along)) = first_player_hit(world, origin, dir, reach, None, 0) {
        reach = along;
        hit = true;
    }
    if hit {
        let point = std::array::from_fn(|i| origin[i] + dir[i] * reach);
        blast(
            world,
            Blast { point, radius: EXPLOSION_RADIUS, damage: EXPLOSION_DAMAGE, knock: EXPLOSION_KNOCK, dir, attacker: gunner, weapon: WEAPON_SHELL },
        );
    }
    reach
}

impl Mechanic for Gunnery {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let ids: Vec<u32> = world.vehicles.iter().filter(|(_, vehicle)| vehicle.gun.is_some()).map(|(&id, _)| id).collect();
        for vid in ids {
            let vehicle = &world.vehicles[&vid];
            let Some(mut gun) = vehicle.gun else { continue };
            let gunner = vehicle.seats[GUNNER_SEAT].and_then(|id| world.players.get(&id)).map(|player| player.input);
            let Some(pose) = world.physics.car_pose(vid) else { continue };

            gun.cooldown = gun.cooldown.saturating_sub(1);
            let mut fire = false;
            if let Some(input) = gunner {
                let wanted = wrap_angle(from_mrad(input.3) - pose.yaw());
                gun.yaw = wrap_angle(step_toward(gun.yaw, gun.yaw + wrap_angle(wanted - gun.yaw), TANK_TURRET_SLEW * dt));
                gun.pitch = step_toward(gun.pitch, from_mrad(input.5).clamp(TANK_BARREL_MIN, TANK_BARREL_MAX), TANK_BARREL_SLEW * dt);
                fire = input.buttons() & BUTTON_FIRE != 0 && gun.cooldown == 0;
            }
            if fire {
                let gunner_id = world.vehicles[&vid].seats[GUNNER_SEAT];
                gun.range = shoot(world, vid, &pose, gun.yaw, gun.pitch, gunner_id);
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
    use shared::protocol::{InputFrame, KIND_TANK};
    use shared::quant::to_mrad;
    use shared::world::{Road, WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

    fn game() -> Game {
        let map = WorldFile {
            version: WORLD_VERSION,
            half_extent: 20_000,
            spawns: (0..8).map(|i| [i * 3000 - 10_000, 0]).collect(),
            roads: vec![Road { width: 800, kind: 0, points: vec![[-20_000, 0], [20_000, 0]] }],
            ..Default::default()
        };
        Game::new(map, 0).unwrap()
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

    /// Feeds the gunner's look and trigger for `ticks` ticks.
    fn aim_and_hold(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, pitch: f32, buttons: u8, ticks: u32) {
        for _ in 0..ticks {
            *seq += 1;
            game.handle(Command::Input(id, 0, vec![InputFrame(*seq, 0, 0, to_mrad(yaw), buttons, to_mrad(pitch), 0)]));
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
        let gun = game.world.vehicles[&vid].gun.unwrap();
        assert_eq!(gun.shots, 1);
        assert!((gun.range - 25.0).abs() < 1.0, "the shell flew to the victim: {}", gun.range);
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

        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 3);
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
        aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 3);
        assert!((game.world.vehicles[&vid].gun.unwrap().range - 15.0).abs() < 1.0);
        assert_eq!(game.world.players[&behind].character.knock, [0.0, 0.0], "outside the blast radius");
    }
}
