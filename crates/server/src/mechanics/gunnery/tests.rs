use super::*;
use crate::game::{Command, Game, Seat};
use sim::STANDING_Y;
use shared::items::Falloff;
use shared::mounts::Show;
use shared::protocol::{KIND_HELI, KIND_HEN, KIND_MORTAR, KIND_PLANE, KIND_STRV, KIND_TANK, KIND_WALKER, PROJECTILE_BOMB, PROJECTILE_CANNON, PROJECTILE_EGG, PROJECTILE_SHELL, WEAPON_BOMB, WEAPON_MG};
use shared::quant::to_mrad;
use shared::world::{Road, WORLD_VERSION, WorldFile};
use tokio::sync::{mpsc, oneshot};

fn game() -> Game {
    let map = WorldFile {
        version: WORLD_VERSION,
        half_extent: 30_000,
        // Eleven spots in the middle for the cars, tanks, walker and helicopter, and one far out at each end
        // for the planes. Players start on the first six.
        spawns: [[-25_000, 0], [25_000, 0]].into_iter().chain((0..11).map(|i| [i * 3000 - 12_000, 0])).collect(),
        roads: vec![Road { width: 800, kind: 0, points: vec![[-30_000, 0], [30_000, 0]] }],
        ..Default::default()
    };
    // Loose items would take a test's use button.
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<crate::game::Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Default::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// Puts `id` straight into a seat of vehicle `vid`.
fn seat(game: &mut Game, id: u32, vid: u32, index: usize) {
    game.world.entities.get_mut::<Vehicle>(vid).unwrap().seats[index] = Some(id);
    game.world.players.get_mut(&id).unwrap().seat = Some(Seat { vehicle: vid, index, remote: false });
}

/// What vehicle `vid`'s mount shown as `show` is doing.
fn shown(game: &Game, vid: u32, show: Show) -> Mount {
    *game.world.entities.of::<Vehicle>(vid).shown(show).expect("it has that mount")
}

fn turret(game: &Game, vid: u32) -> Mount {
    shown(game, vid, Show::Turret)
}

fn tank(game: &Game) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_TANK).expect("tanks are placed").0
}

fn heli(game: &Game) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_HELI).expect("a helicopter is placed").0
}

/// Feeds the gunner's look and trigger for `ticks` ticks.
fn aim_and_hold(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, pitch: f32, buttons: u16, ticks: u32) {
    for _ in 0..ticks {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: to_mrad(yaw), buttons, pitch: to_mrad(pitch), view: 0, slot: 0 }]));
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
    let gun = turret(&game, vid);
    assert!((gun.yaw - 1.0).abs() < 0.02 && (gun.pitch - 0.3).abs() < 0.02, "{gun:?}");
    // Looking far up asks for more than the barrel can do.
    aim_and_hold(&mut game, gunner, &mut seq, hull + 1.0, 1.2, 0, 180);
    assert!((turret(&game, vid).pitch - TANK_BARREL_MAX).abs() < 0.01);
    aim_and_hold(&mut game, gunner, &mut seq, hull - 2.5, 0.0, 0, 120);
    let turned = turret(&game, vid).yaw;
    assert!(turned < 1.0, "the turret slews at a limited rate, the short way round: {turned}");
}

#[test]
fn turret_slew_is_rate_limited() {
    let (mut game, gunner, vid, mut seq) = crewed();
    let hull = game.world.physics.car_pose(vid).unwrap().yaw();
    aim_and_hold(&mut game, gunner, &mut seq, hull + 2.0, 0.0, 0, 30);
    let yaw = turret(&game, vid).yaw;
    // 30 ticks is half a second: at most 0.7 rad, plus the jitter buffer's lag.
    assert!(yaw > 0.3 && yaw <= TANK_TURRET_SLEW * 0.5 + 0.01, "{yaw}");
}

#[test]
fn firing_hits_and_throws_a_stickman_in_the_line_of_fire() {
    let (mut game, gunner, vid, mut seq) = crewed();
    let (victim, _inbox) = join(&mut game);
    let pose = game.world.physics.car_pose(vid).unwrap();
    let gun = turret(&game, vid);
    let (origin, dir) = aim(&pose, gun.yaw, gun.pitch);
    game.world.players.get_mut(&victim).unwrap().character.position = std::array::from_fn(|i| origin[i] + dir[i] * 25.0);

    aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, BUTTON_FIRE, 3);
    assert_eq!(turret(&game, vid).shots, 1);
    // 25 m at 140 m/s is about ten ticks: the shell is on its way, and nobody is hurt yet.
    assert_eq!(game.world.in_flight().len(), 1, "the shell is in flight");
    assert_eq!(game.world.players[&victim].character.knock, [0.0, 0.0]);
    aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, 0, 30);
    assert!(game.world.in_flight().is_empty(), "and then it has burst");
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
    let gun = turret(&game, vid);
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
    assert_eq!(turret(&game, vid).shots, 1, "trigger held for a second: one shell");
    aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
    assert_eq!(turret(&game, vid).shots, 2, "reloaded after a second and a half");
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
    assert_eq!(turret(&game, vid).shots, 0);
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
    assert!(game.world.in_flight().is_empty());
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
        match game.world.in_flight().values().next() {
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
    assert_eq!(listed[0].kind, PROJECTILE_SHELL);
    let before = game.world.in_flight().values().next().unwrap().velocity;
    aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, 0, 5);
    let after = game.world.in_flight().values().next().unwrap().velocity;
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
    let gun = turret(&game, vid);
    assert!((gun.yaw - 0.5).abs() < 0.03 && (gun.pitch + 0.3).abs() < 0.03, "{gun:?}");
    // Looking behind or high above asks for more than the mount can do.
    aim_and_hold(&mut game, gunner, &mut seq, hull + 2.5, 1.0, 0, 120);
    let gun = turret(&game, vid);
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
    assert_eq!(turret(&game, vid).shots, 1);
    let listed = game.projectile_states();
    assert_eq!(listed.len(), 1, "the round is in flight");
    assert_eq!(listed[0].kind, PROJECTILE_CANNON);
    assert_eq!(game.world.players[&victim].hp, before, "and has not arrived");

    aim_and_hold(&mut game, gunner, &mut seq, pose.yaw(), 0.0, 0, 30);
    assert!(game.world.in_flight().is_empty(), "it has burst");
    let hp = game.world.players[&victim].hp;
    assert!(hp < before && before - hp < 60, "hurt, but less than a tank shell: {before} to {hp}");
    assert_eq!(game.world.players[&bystander].hp, PLAYER_MAX_HP, "outside the small blast");
}

#[test]
fn the_nose_gun_fires_a_burst_at_its_rate_and_only_for_the_gunner() {
    let (mut game, gunner, vid, mut seq) = heli_crewed();
    let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
    aim_and_hold(&mut game, gunner, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
    let fired = turret(&game, vid).shots;
    let expected = (60 / HELI_GUN_COOLDOWN_TICKS) as u8;
    assert!(fired.abs_diff(expected) <= 2, "{fired} rounds in a second, expected about {expected}");

    // The pilot holding fire works nothing: only seat 1 has the gun.
    let mut alone = self::game();
    let (pilot, _inbox) = join(&mut alone);
    let vid = heli(&alone);
    seat(&mut alone, pilot, vid, 0);
    let mut seq = 0;
    aim_and_hold(&mut alone, pilot, &mut seq, 0.0, 0.0, BUTTON_FIRE, 30);
    assert_eq!(turret(&alone, vid).shots, 0);
}

#[test]
fn the_pilot_fires_rockets_that_accelerate_straight_ahead() {
    let mut game = game();
    let (pilot, _inbox) = join(&mut game);
    let vid = heli(&game);
    seat(&mut game, pilot, vid, 0);
    let mut seq = 0;
    aim_and_hold(&mut game, pilot, &mut seq, 0.0, 0.0, BUTTON_FIRE, 6);
    let fired = shown(&game, vid, Show::Hidden).shots;
    assert!(fired >= 1, "the pilot's trigger fires the pods");
    assert_eq!(turret(&game, vid).shots, 0, "the nose gun is the gunner's");
    let speed = |game: &Game| {
        game.world.in_flight().values().filter(|r| r.kind == PROJECTILE_ROCKET).map(|r| r.velocity.iter().map(|v| v * v).sum::<f32>().sqrt()).fold(0.0, f32::max)
    };
    let early = speed(&game);
    aim_and_hold(&mut game, pilot, &mut seq, 0.0, 0.0, 0, 10);
    let later = speed(&game);
    assert!(early > 0.0 && later > early, "{early} then {later}");
    for rocket in game.world.in_flight().values().filter(|r| r.kind == PROJECTILE_ROCKET) {
        assert!(rocket.velocity[1].abs() < rocket.velocity[0].abs().max(rocket.velocity[2].abs()), "no arc: {rocket:?}");
    }
}

#[test]
fn the_walkers_driver_aims_with_the_upper_body_within_thirty_degrees_and_fires() {
    let mut game = game();
    let (driver, _inbox) = join(&mut game);
    let vid = game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_WALKER).expect("a walker is placed").0;
    seat(&mut game, driver, vid, 0);
    let mut seq = 0;
    let hull = game.world.physics.car_pose(vid).unwrap().yaw();
    aim_and_hold(&mut game, driver, &mut seq, hull, 0.0, 0, 90);
    // A look inside its travel is followed, whichever way the legs face.
    aim_and_hold(&mut game, driver, &mut seq, hull + 0.3, 0.1, 0, 90);
    let gun = turret(&game, vid);
    assert!((gun.yaw - 0.3).abs() < 0.03 && (gun.pitch - 0.1).abs() < 0.03, "{gun:?}");
    // And one beyond it stops at the limit.
    aim_and_hold(&mut game, driver, &mut seq, hull + 1.5, 0.0, 0, 120);
    let gun = turret(&game, vid);
    assert!((gun.yaw - WALKER_TORSO_LIMIT).abs() < 0.01, "{gun:?}");
    // The driver's trigger fires the guns, at their rate, and a round is in the air.
    aim_and_hold(&mut game, driver, &mut seq, hull + 0.3, 0.0, BUTTON_FIRE, 60);
    let fired = turret(&game, vid).shots;
    let expected = (60 / WALKER_GUN_COOLDOWN_TICKS) as u8;
    assert!(fired.abs_diff(expected) <= 2, "{fired} rounds in a second, expected about {expected}");
}

fn strv(game: &Game) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_STRV).expect("a Strv S is placed").0
}

#[test]
fn a_strv_driver_fires_the_hull_gun_and_only_elevation_follows_the_look() {
    let mut game = game();
    let (driver, _inbox) = join(&mut game);
    let vid = strv(&game);
    assert_eq!(game.world.entities.of::<Vehicle>(vid).seats.iter().flatten().count(), 0);
    seat(&mut game, driver, vid, 0);
    let mut seq = 0;
    let hull = game.world.physics.car_pose(vid).unwrap().yaw();
    aim_and_hold(&mut game, driver, &mut seq, hull, 0.0, 0, 90);
    // Looking sideways does nothing to the gun, and looking up lifts the barrel to its limit.
    aim_and_hold(&mut game, driver, &mut seq, hull + 1.0, 1.2, 0, 120);
    let gun = turret(&game, vid);
    assert!(gun.yaw.abs() < 1e-6, "{gun:?}");
    assert!((gun.pitch - STRV_BARREL_MAX).abs() < 0.01, "{gun:?}");
    aim_and_hold(&mut game, driver, &mut seq, hull, -1.0, 0, 120);
    let gun = turret(&game, vid);
    assert!((gun.pitch - STRV_BARREL_MIN).abs() < 0.01, "{gun:?}");
    // The driver's trigger fires a shell, at the shell's cooldown, and kicks the hull back.
    aim_and_hold(&mut game, driver, &mut seq, hull, 0.0, BUTTON_FIRE, 3);
    assert_eq!(turret(&game, vid).shots, 1);
    assert!(game.world.physics.car_pose(vid).unwrap().forward_speed < -0.5);
    aim_and_hold(&mut game, driver, &mut seq, hull, 0.0, BUTTON_FIRE, SHELL_COOLDOWN_TICKS);
    assert_eq!(turret(&game, vid).shots, 2);
}

#[test]
fn an_empty_strv_does_not_fire() {
    let mut game = game();
    let vid = strv(&game);
    for _ in 0..60 {
        game.step();
    }
    assert_eq!(turret(&game, vid).shots, 0);
}

fn plane(game: &Game) -> u32 {
    game.world.entities.all::<Vehicle>().find(|(_, vehicle)| vehicle.kind == KIND_PLANE).expect("a plane is placed").0
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
fn hold_watching(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, buttons: u16, ticks: u32, weapon: u8) -> Vec<ShotEvent> {
    let mut seen = vec![];
    for _ in 0..ticks {
        aim_and_hold(game, id, seq, yaw, 0.0, buttons, 1);
        seen.extend(game.world.shots.drain(..).filter(|shot| shot.weapon == weapon));
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
    let hp = game.world.entities.of::<Vehicle>(vid).hp;

    let hits = hold_watching(&mut game, pilot, &mut seq, pose.yaw(), BUTTON_FIRE, 30, WEAPON_MG);
    let arms = shown(&game, vid, Show::Guns);
    let expected = (30 / PLANE_GUN_COOLDOWN_TICKS) as u8;
    assert!(arms.shots.abs_diff(expected) <= 1, "{} rounds in half a second, expected about {expected}", arms.shots);
    assert!(game.world.players[&victim].hp < PLAYER_MAX_HP, "the stickman ahead is hit");
    assert!(!hits.is_empty() && hits.iter().all(|shot| shot.shooter == pilot), "{hits:?}");
    assert_eq!(game.world.entities.of::<Vehicle>(vid).hp, hp, "its own guns never hurt the plane");
    assert!(game.world.in_flight().is_empty(), "rounds are hitscans, not projectiles");
}

#[test]
fn the_forward_guns_jam_when_hot_and_clear_when_cool() {
    let (mut game, pilot, vid, mut seq) = plane_crewed(0);
    let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
    let mut jammed_at = None;
    for tick in 0..60 * 8 {
        aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, BUTTON_FIRE, 1);
        if shown(&game, vid, Show::Guns).jammed {
            jammed_at = Some(tick);
            break;
        }
    }
    let jammed_at = jammed_at.expect("the guns jam");
    assert!((60 * 3..60 * 6).contains(&jammed_at), "after about four seconds: {jammed_at} ticks");
    let shots = shown(&game, vid, Show::Guns).shots;
    aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, BUTTON_FIRE, 60);
    assert_eq!(shown(&game, vid, Show::Guns).shots, shots, "a jammed gun does not fire");
    assert!(game.vehicle_states().iter().find(|v| v.id == vid).unwrap().arms.unwrap().flags & shared::protocol::ARMS_JAMMED != 0);
    aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, 0, 60 * 3);
    assert!(!shown(&game, vid, Show::Guns).jammed, "cool again");
    aim_and_hold(&mut game, pilot, &mut seq, yaw, 0.0, BUTTON_FIRE, 20);
    assert!(shown(&game, vid, Show::Guns).shots != shots, "and firing");
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
    let bombs: Vec<_> = game.world.in_flight().values().filter(|rocket| rocket.kind == PROJECTILE_BOMB).copied().collect();
    assert_eq!(bombs.len(), 1, "{:?}", game.world.in_flight());
    assert_eq!(shown(&game, vid, Show::Rack).ammo, PLANE_BOMBS - 1);
    let along = -sin * bombs[0].velocity[0] - cos * bombs[0].velocity[2];
    assert!(along > 25.0, "it carries the plane's speed: {:?}", bombs[0].velocity);
    let dropped_at = bombs[0].position;

    let bursts = hold_watching(&mut game, pilot, &mut seq, pose.yaw(), 0, 120, WEAPON_BOMB);
    assert_eq!(bursts.len(), 1, "it lands and bursts");
    let at = [bursts[0].x, bursts[0].y, bursts[0].z].map(|cm| cm as f32 / 100.0);
    let ahead = -sin * (at[0] - dropped_at[0]) - cos * (at[2] - dropped_at[2]);
    assert!(ahead > 20.0 && at[1] < 2.0, "well ahead of where it was let go, on the ground: {at:?}");
}

#[test]
fn the_rack_holds_four_bombs_and_fills_again_on_the_ground() {
    let (mut game, pilot, vid, mut seq) = plane_crewed(0);
    let pose = game.world.physics.car_pose(vid).unwrap();
    // On the ground the rack holds on to them.
    aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, BUTTON_CROUCH, 60);
    assert_eq!(shown(&game, vid, Show::Rack).ammo, PLANE_BOMBS);
    // High up, held crouch drops one every half second, and the fifth is not there.
    game.world.physics.reset_car(vid, [pose.position[0], 120.0, 60.0], pose.yaw());
    let mut dropped = 0;
    for _ in 0..PLANE_BOMB_COOLDOWN_TICKS * 5 + 5 {
        let before: Vec<u32> = game.world.in_flight().keys().copied().collect();
        aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, BUTTON_CROUCH, 1);
        dropped += game.world.in_flight().iter().filter(|(id, r)| r.kind == PROJECTILE_BOMB && !before.contains(id)).count();
    }
    assert_eq!((dropped, shown(&game, vid, Show::Rack).ammo), (PLANE_BOMBS as usize, 0));
    // Back home and standing still, the ground crew loads it again.
    game.world.physics.reset_car(vid, pose.position, pose.yaw());
    aim_and_hold(&mut game, pilot, &mut seq, pose.yaw(), 0.0, 0, PLANE_REARM_TICKS + 90);
    assert_eq!(shown(&game, vid, Show::Rack).ammo, PLANE_BOMBS);
}

#[test]
fn the_rear_gun_turns_all_the_way_round_but_never_fires_at_the_pilot() {
    let (mut game, gunner, vid, mut seq) = plane_crewed(1);
    let hull = game.world.physics.car_pose(vid).unwrap().yaw();
    // Looking ahead over the pilot's head: the ring follows, the trigger does nothing.
    aim_and_hold(&mut game, gunner, &mut seq, hull, 0.2, BUTTON_FIRE, 60);
    let gun = turret(&game, vid);
    assert!(gun.yaw.abs() < 0.05 && (gun.pitch - 0.2).abs() < 0.05, "{gun:?}");
    assert_eq!(gun.shots, 0);
    // Straight behind, and up high.
    aim_and_hold(&mut game, gunner, &mut seq, hull + std::f32::consts::PI, 1.4, BUTTON_FIRE, 90);
    let gun = turret(&game, vid);
    assert!(wrap_angle(gun.yaw - std::f32::consts::PI).abs() < 0.05, "{gun:?}");
    assert!((gun.pitch - PLANE_REAR_PITCH_MAX).abs() < 0.01, "{gun:?}");
    assert!(gun.shots > 3, "{gun:?}");
    assert_eq!(shown(&game, vid, Show::Guns).shots, 0, "the gunner's trigger is not the pilot's");
}

#[test]
fn the_rear_gun_hits_a_stickman_behind_and_only_its_gunner_works_it() {
    let (mut game, gunner, vid, mut seq) = plane_crewed(1);
    let (victim, _inbox) = join(&mut game);
    let hull = game.world.physics.car_pose(vid).unwrap().yaw();
    aim_and_hold(&mut game, gunner, &mut seq, hull + std::f32::consts::PI, 0.0, 0, 60);
    let pose = game.world.physics.car_pose(vid).unwrap();
    let gun = turret(&game, vid);
    let (ring, dir) = aim_from(&pose, PLANE_REAR_PIVOT, gun.yaw, gun.pitch);
    game.world.players.get_mut(&victim).unwrap().character.position = std::array::from_fn(|i| ring[i] + dir[i] * 20.0 + if i == 1 { -0.3 } else { 0.0 });
    aim_and_hold(&mut game, gunner, &mut seq, hull + std::f32::consts::PI, 0.0, BUTTON_FIRE, 30);
    assert!(game.world.players[&victim].hp < PLAYER_MAX_HP);

    // A pilot alone works only the forward guns.
    let (mut game, pilot, vid, mut seq) = plane_crewed(0);
    let yaw = game.world.physics.car_pose(vid).unwrap().yaw();
    aim_and_hold(&mut game, pilot, &mut seq, yaw + std::f32::consts::PI, 0.0, BUTTON_FIRE, 30);
    assert_eq!(turret(&game, vid).shots, 0);
}

#[test]
fn machine_gun_rounds_hurt_a_plane_more_than_a_car() {
    let mut game = game();
    let (shooter, _inbox) = join(&mut game);
    let target = plane(&game);
    let car = game.world.entities.all::<Vehicle>().find(|(_, v)| v.kind == shared::protocol::KIND_JEEP).unwrap().0;
    // The broad-phase learns of the vehicles in the first step.
    game.step();
    let round = Round { damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: 50.0, falloff: Falloff::NONE };
    for vid in [target, car] {
        let at = game.world.physics.car_pose(vid).unwrap().position;
        let origin = [at[0], at[1] + 20.0, at[2]];
        let (_, struck) = hitscan(&mut game.world, shooter, origin, [0.0, -1.0, 0.0], &round, 0, 0);
        assert!(struck);
    }
    assert_eq!(game.world.entities.of::<Vehicle>(target).hp, shared::protocol::vehicle_hp(KIND_PLANE) - PLANE_GUN_PLANE_DAMAGE);
    assert_eq!(game.world.entities.of::<Vehicle>(car).hp, 100 - PLANE_GUN_VEHICLE_DAMAGE);
}

#[test]
fn a_mortar_bomb_arcs_up_and_lands_on_the_range_it_was_dialled_to() {
    let mut game = game();
    let (gunner, _inbox) = join(&mut game);
    let vid = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_MORTAR, [0.0, 1.0, 20.0], 0.0);
    seat(&mut game, gunner, vid, 0);
    let mut seq = 0;
    // Straight ahead, and the dial left where it starts.
    aim_and_hold(&mut game, gunner, &mut seq, 0.0, 0.0, BUTTON_FIRE, 2);
    assert_eq!(turret(&game, vid).shots, 1, "one bomb away");
    // It bursts where it meets the ground, and is gone the tick after. Track it until then.
    let (mut apex, mut last) = (0.0_f32, None);
    for _ in 0..900 {
        aim_and_hold(&mut game, gunner, &mut seq, 0.0, 0.0, 0, 1);
        match game.world.in_flight().values().next() {
            Some(rocket) => {
                apex = apex.max(rocket.position[1]);
                last = Some(rocket.position);
            }
            None => break,
        }
    }
    let [x, y, z] = last.expect("the bomb was in flight");
    assert!(game.world.in_flight().is_empty(), "and it has burst");
    assert!(apex > 10.0 && apex < 120.0, "it lobs high, to {apex} m");
    assert!(y < 3.0, "and bursts on the ground, not in the sky: {y} m up");
    let range = x.hypot(z - 20.0);
    assert!((range - MORTAR_RANGE_START).abs() < 10.0, "it was dialled to {MORTAR_RANGE_START} m and lands {range} m out");
}

#[test]
fn the_range_dial_walks_the_lob_and_the_elevation_follows_it() {
    let mut game = game();
    let (gunner, _inbox) = join(&mut game);
    let vid = crate::mechanics::vehicles::add_vehicle(&mut game.world, KIND_MORTAR, [0.0, 1.0, 20.0], 0.0);
    seat(&mut game, gunner, vid, 0);
    let mut seq = 0;
    // Dialled all the way out, the longest lob is the lowest elevation.
    aim_and_hold(&mut game, gunner, &mut seq, 0.0, 0.0, BUTTON_RANGE_UP, 600);
    let gun = turret(&game, vid);
    assert_eq!(gun.range, MORTAR_RANGE_MAX);
    assert!((gun.pitch - MORTAR_PITCH_MIN).abs() < 0.01, "45 degrees at the longest range: {}", gun.pitch);
    // And all the way in, it is nearly vertical, but never past the top of the travel.
    aim_and_hold(&mut game, gunner, &mut seq, 0.0, 0.0, BUTTON_RANGE_DOWN, 600);
    let gun = turret(&game, vid);
    assert_eq!(gun.range, MORTAR_RANGE_MIN);
    assert!(gun.pitch > 1.4 && gun.pitch <= MORTAR_PITCH_MAX, "nearly vertical at the shortest: {}", gun.pitch);
}

/// A hen standing off the road, `height` metres up, with a driver aboard and their first frames queued.
fn hen_crewed(height: f32) -> (Game, u32, u32, u32) {
    let mut game = game();
    let (driver, _inbox) = join(&mut game);
    let ground = game.world.map.ground.height_at(0.0, 60.0);
    let vid = super::super::vehicles::add_vehicle(&mut game.world, KIND_HEN, [0.0, ground + WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1] + 0.2, 60.0], 0.0);
    seat(&mut game, driver, vid, 0);
    let mut seq = 0;
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, 60);
    if height > 0.0 {
        game.world.physics.reset_car(vid, [0.0, ground + height, 60.0], 0.0);
    }
    (game, driver, vid, seq)
}

fn eggs(game: &Game) -> Vec<Flying> {
    game.world.in_flight().values().filter(|rocket| rocket.kind == PROJECTILE_EGG).copied().collect()
}

#[test]
fn a_hen_lays_an_egg_behind_it_that_bursts_when_its_fuse_runs_out() {
    let (mut game, driver, vid, mut seq) = hen_crewed(0.0);
    let pose = game.world.physics.car_pose(vid).unwrap();
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, BUTTON_FIRE, 1);
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, 1);
    let laid = eggs(&game);
    assert_eq!(laid.len(), 1, "{:?}", game.world.in_flight());
    let clutch = shown(&game, vid, Show::Rack);
    assert_eq!((clutch.ammo, clutch.shots), (HEN_EGGS - 1, 1));
    // Under the tail, which is behind the hull (+Z when it faces -Z).
    assert!(laid[0].position[2] > pose.position[2] + 1.5 && laid[0].position[1] < pose.position[1], "{:?} vs {pose:?}", laid[0].position);

    // Laid from a standing hen it lands too softly to smash, and goes off on its fuse.
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, HEN_EGG_FUSE_TICKS - 10);
    let resting = eggs(&game);
    assert_eq!(resting.len(), 1, "not before its fuse");
    assert!(resting[0].position[1] < 1.0, "it lies on the ground: {:?}", resting[0].position);
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, 20);
    assert!(eggs(&game).is_empty(), "it has burst");
}

#[test]
fn an_egg_dropped_from_high_up_smashes_where_it_lands() {
    let (mut game, driver, _, mut seq) = hen_crewed(25.0);
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, BUTTON_FIRE, 1);
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, 1);
    assert_eq!(eggs(&game).len(), 1);
    // Watched until it is gone: it smashed on the ground with time left on its fuse.
    let mut last = eggs(&game)[0];
    for _ in 0..HEN_EGG_FUSE_TICKS {
        aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, 1);
        match eggs(&game).first() {
            Some(&egg) => last = egg,
            None => break,
        }
    }
    assert!(eggs(&game).is_empty(), "it burst");
    assert!(last.ttl > 20 && last.position[1] < 5.0, "on landing, well before its fuse: {last:?}");
}

#[test]
fn a_clutch_is_six_eggs_laid_one_at_a_time_and_grows_back() {
    let (mut game, driver, vid, mut seq) = hen_crewed(0.0);
    // Held for a second, it lays at its rate; the clutch runs out after six.
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, BUTTON_FIRE, TICK_RATE);
    let expected = TICK_RATE.div_ceil(HEN_EGG_COOLDOWN_TICKS) as u8;
    assert_eq!(shown(&game, vid, Show::Rack).shots, expected);
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, BUTTON_FIRE, TICK_RATE * 3);
    let clutch = shown(&game, vid, Show::Rack);
    assert!(clutch.ammo <= 1 && clutch.shots >= HEN_EGGS, "{clutch:?}");
    // Let go, and they come back one at a time.
    let before = clutch.ammo;
    aim_and_hold(&mut game, driver, &mut seq, 0.0, 0.0, 0, HEN_EGG_REGROW_TICKS + 1);
    assert_eq!(shown(&game, vid, Show::Rack).ammo, before + 1);
}

#[test]
fn a_mount_made_of_parts_no_vehicle_has_works() {
    // A rocket launcher on a ring behind a jeep's passenger: two rockets, one more a second.
    const LAUNCHER: MountDef = MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: None, pitch: (-0.2, 0.6), slew: 2.0 },
        muzzle: Muzzle::One([0.0, 1.2, 0.5]),
        fire: Fire::Rocket { speed: 20.0, ttl: ROCKET_TTL_TICKS },
        trigger: BUTTON_FIRE,
        cooldown: 10,
        spread: 0.0,
        recoil: 0.0,
        heat: 0.0,
        ammo: Some(Ammo { full: 2, refill: Refill::Regrow(TICK_RATE) }),
        needs: &[],
        show: Show::Hidden,
    };
    let mut game = game();
    let (passenger, _inbox) = join(&mut game);
    let ground = game.world.map.ground.height_at(0.0, 60.0);
    let vid = super::super::vehicles::add_vehicle(&mut game.world, shared::protocol::KIND_JEEP, [0.0, ground + 1.0, 60.0], 0.0);
    seat(&mut game, passenger, vid, 1);
    let pose = game.world.physics.car_pose(vid).unwrap();
    // Looking straight back over the tailgate, trigger held.
    let behind = wrap_angle(pose.yaw() + std::f32::consts::PI);
    game.world.players.get_mut(&passenger).unwrap().input = InputFrame { seq: 1, strafe: 0, forward: 0, yaw: to_mrad(behind), buttons: BUTTON_FIRE, pitch: 0, view: 0, slot: 0 };
    let mut mount = Mount::fresh(&LAUNCHER);
    let rockets = |game: &Game| projectiles::count(&game.world.entities, Some(PROJECTILE_ROCKET));
    for _ in 0..TICK_RATE {
        mount = work(&mut game.world, vid, &LAUNCHER, mount, &pose, TICK_DT);
    }
    assert_eq!((rockets(&game), mount.ammo), (2, 0), "it runs out");
    // The second comes back a second after the first was fired, and goes at once.
    for _ in 0..TICK_RATE - 5 {
        mount = work(&mut game.world, vid, &LAUNCHER, mount, &pose, TICK_DT);
    }
    assert_eq!(rockets(&game), 3, "it gets one back a second after it began to run low");
    assert!((mount.yaw.abs() - std::f32::consts::PI).abs() < 0.05, "it turned to look behind: {}", mount.yaw);
    assert!(game.world.in_flight().values().all(|rocket| rocket.owner == passenger && rocket.source == vid));
}
