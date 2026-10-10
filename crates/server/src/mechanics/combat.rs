use super::Mechanic;
use super::spawn;
use crate::game::{Rocket, World};
use shared::consts::*;
use shared::protocol::{Kill, S_KILL, ShotEvent, WEAPON_RIFLE, WEAPON_ROCKET, WEAPON_WRECK};
use shared::quant::{from_mrad, to_cm};

/// Health, death, the rifle, the rocket launcher and what blasts do to people and vehicles.
///
/// Every player has `hp`. `damage` is the one place health goes down: at zero the player dies,
/// the attacker is credited, and a `Kill` is queued for the reliable stream. A dead player lies
/// where they fell, can be neither hit nor hurt, and respawns after `RESPAWN_TICKS` with full health.
///
/// The rifle is a hitscan from the eye along the look direction, fired by `BUTTON_FIRE` while
/// standing. Seated players never fire it: a gunner's trigger belongs to the gun.
pub struct Combat;

/// Hop given to a stickman at the centre of a blast; it fades with distance.
const BLAST_HOP: f32 = 7.0;
const BLAST_HOP_MIN: f32 = 3.0;

/// Pitch beyond this is not a thing a client can send; clamp so a forged frame can't aim at the sky's far side.
const MAX_PITCH: f32 = 1.5;

/// Unit look direction for a stickman facing `yaw` and looking `pitch` (+ is up). Yaw 0 faces -Z.
pub fn look(yaw: f32, pitch: f32) -> [f32; 3] {
    [-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos()]
}

/// Distance along a unit-direction ray at which it first enters a vertical capsule, if within
/// `max`. The capsule's centre is `centre`; it spans `half_height` above and below it, plus `radius`.
pub fn ray_capsule(origin: [f32; 3], dir: [f32; 3], centre: [f32; 3], half_height: f32, radius: f32, max: f32) -> Option<f32> {
    let rel = [origin[0] - centre[0], origin[1] - centre[1], origin[2] - centre[2]];
    let mut best: Option<f32> = None;
    let mut consider = |t: f32| {
        if t >= 0.0 && t <= max && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    };
    // Already inside: the shot starts in the body.
    let (inside_y, flat) = (rel[1].abs() <= half_height, rel[0].hypot(rel[2]));
    if inside_y && flat <= radius {
        return Some(0.0);
    }
    // The cylinder wall: a circle in the XZ plane, limited to the straight section.
    let a = dir[0] * dir[0] + dir[2] * dir[2];
    if a > 1e-9 {
        let b = rel[0] * dir[0] + rel[2] * dir[2];
        let c = rel[0] * rel[0] + rel[2] * rel[2] - radius * radius;
        let disc = b * b - a * c;
        if disc >= 0.0 {
            let t = (-b - disc.sqrt()) / a;
            if (rel[1] + dir[1] * t).abs() <= half_height {
                consider(t);
            }
        }
    }
    // The two end caps.
    for cap in [half_height, -half_height] {
        let to = [rel[0], rel[1] - cap, rel[2]];
        let b = to[0] * dir[0] + to[1] * dir[1] + to[2] * dir[2];
        let c = to[0] * to[0] + to[1] * to[1] + to[2] * to[2] - radius * radius;
        let disc = b * b - c;
        if disc >= 0.0 {
            let t = -b - disc.sqrt();
            // Only the outer half of the sphere is a cap; the rest is inside the cylinder.
            if (rel[1] + dir[1] * t - cap) * cap.signum() >= 0.0 {
                consider(t);
            }
        }
    }
    best
}

/// How many ticks back a shooter whose frame showed server tick `view` was looking, as the server
/// sees it at tick `now`: never forwards, and never more than `MAX_REWIND_TICKS`.
pub fn rewind_ticks(now: u32, view: u16) -> u32 {
    let back = (now as u16).wrapping_sub(view) as i16;
    (back.max(0) as u32).min(MAX_REWIND_TICKS)
}

/// The nearest living standing player the ray meets within `max`, and how far along it is.
/// `ignore` is the shooter. Stickmen are not physics bodies, so this is the only way to hit one.
/// Players are tested where they stood `back` ticks ago, which is where a shooter with that much
/// lag saw them.
pub fn first_player_hit(world: &World, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: Option<u32>, back: u32) -> Option<(u32, f32)> {
    let mut best: Option<(u32, f32)> = None;
    for (&id, player) in &world.players {
        if Some(id) == ignore || player.seat.is_some() || !player.alive() {
            continue;
        }
        let limit = best.map_or(max, |(_, t)| t);
        let at = if back == 0 { player.character.position } else { world.position_at(id, back).unwrap_or(player.character.position) };
        if let Some(t) = ray_capsule(origin, dir, at, CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS, limit) {
            best = Some((id, t));
        }
    }
    best
}

/// Takes `amount` health off `victim`. At zero they die: the attacker (a player id, if any other
/// than the victim) is credited and a kill is queued. Does nothing to the dead.
pub fn damage(world: &mut World, victim: u32, amount: u8, attacker: Option<u32>, weapon: u8) {
    let Some(player) = world.players.get_mut(&victim) else { return };
    if !player.alive() || amount == 0 {
        return;
    }
    player.hp = player.hp.saturating_sub(amount);
    if player.hp > 0 {
        return;
    }
    player.respawn_in = RESPAWN_TICKS;
    player.deaths = player.deaths.saturating_add(1);
    player.fire_cooldown = 0;
    let deaths = player.deaths;
    let mut killer_kills = 0;
    let mut killer_id = 0;
    if let Some(killer) = attacker.filter(|&id| id != victim).and_then(|id| world.players.get_mut(&id).map(|p| (id, p))) {
        killer.1.kills = killer.1.kills.saturating_add(1);
        killer_kills = killer.1.kills;
        killer_id = killer.0;
    }
    world.kills.push(Kill(S_KILL, killer_id, victim, weapon, killer_kills, deaths));
    tracing::debug!(victim, killer = killer_id, weapon, "player killed");
}

/// What a blast does where it lands.
pub struct Blast {
    pub point: [f32; 3],
    pub radius: f32,
    /// Damage at the centre, falling off linearly to zero at `radius`.
    pub damage: f32,
    /// Knockback speed at the centre, m/s.
    pub knock: f32,
    /// Where it came from. A stickman right on the point is thrown along this.
    pub dir: [f32; 3],
    pub attacker: Option<u32>,
    pub weapon: u8,
}

/// Throws and hurts the standing players within `blast.radius`, and shoves and hurts the vehicles.
pub fn blast(world: &mut World, blast: Blast) {
    let mut hurt = Vec::new();
    for (&id, player) in world.players.iter_mut().filter(|(_, player)| player.seat.is_none() && player.alive()) {
        let c = &mut player.character;
        let away: [f32; 3] = std::array::from_fn(|i| c.position[i] - blast.point[i]);
        let distance = away.iter().map(|v| v * v).sum::<f32>().sqrt();
        if distance >= blast.radius {
            continue;
        }
        let falloff = 1.0 - distance / blast.radius;
        let flat = away[0].hypot(away[2]);
        // Right on the blast there is no "away": follow where it came from instead.
        let (dx, dz) = if flat > 0.1 { (away[0] / flat, away[2] / flat) } else { (blast.dir[0], blast.dir[2]) };
        let power = (blast.knock * falloff).min(KNOCK_MAX_SPEED);
        c.knock = [dx * power, dz * power];
        c.vy = BLAST_HOP_MIN + (BLAST_HOP - BLAST_HOP_MIN) * falloff;
        c.grounded = false;
        c.ride = 0;
        hurt.push((id, (blast.damage * falloff).round().min(255.0) as u8));
        tracing::debug!(distance, power, "player blasted");
    }
    for (id, amount) in hurt {
        damage(world, id, amount, blast.attacker, blast.weapon);
    }
    for (vid, falloff) in world.physics.blast_cars(blast.point, blast.radius) {
        damage_vehicle(world, vid, (blast.damage * falloff).round().min(255.0) as u8, blast.attacker);
    }
}

/// Takes `amount` health off vehicle `vid`, and wrecks it at zero.
pub fn damage_vehicle(world: &mut World, vid: u32, amount: u8, attacker: Option<u32>) {
    let Some(vehicle) = world.vehicles.get_mut(&vid) else { return };
    if vehicle.wrecked() || amount == 0 {
        return;
    }
    if attacker.is_some() {
        vehicle.last_attacker = attacker;
    }
    vehicle.hp = vehicle.hp.saturating_sub(amount);
    if vehicle.hp == 0 {
        wreck(world, vid);
    }
}

/// A vehicle's last moment: everyone inside dies, and it goes up in a blast that can take the
/// next one with it.
fn wreck(world: &mut World, vid: u32) {
    let Some(pose) = world.physics.car_pose(vid) else { return };
    let Some(vehicle) = world.vehicles.get_mut(&vid) else { return };
    vehicle.wreck_in = WRECK_TICKS;
    vehicle.controls = Default::default();
    let attacker = vehicle.last_attacker;
    let crew: Vec<u32> = vehicle.seats.iter_mut().filter_map(Option::take).collect();
    for id in crew {
        if let Some(player) = world.players.get_mut(&id) {
            player.seat = None;
            player.character.position = pose.position;
        }
        damage(world, id, PLAYER_MAX_HP, attacker, WEAPON_WRECK);
    }
    let [x, y, z] = pose.position.map(to_cm);
    world.shots.push(ShotEvent(attacker.unwrap_or(0), WEAPON_WRECK, x, y, z));
    // Thrown up a little, so it is plainly not a parked car any more.
    world.physics.kick_car(vid, [0.0, 6.0, 0.0]);
    blast(
        world,
        Blast {
            point: pose.position,
            radius: WRECK_BLAST_RADIUS,
            damage: WRECK_BLAST_DAMAGE,
            knock: WRECK_BLAST_KNOCK,
            dir: [0.0, 1.0, 0.0],
            attacker,
            weapon: WEAPON_WRECK,
        },
    );
    tracing::debug!(vid, "vehicle wrecked");
}

fn burst_rocket(world: &mut World, owner: u32, point: [f32; 3], dir: [f32; 3]) {
    let [x, y, z] = point.map(to_cm);
    world.shots.push(ShotEvent(owner, WEAPON_ROCKET, x, y, z));
    // A dead or departed owner still gets the blast, just not the credit.
    let attacker = world.players.contains_key(&owner).then_some(owner);
    blast(world, Blast { point, radius: ROCKET_BLAST_RADIUS, damage: ROCKET_DAMAGE, knock: ROCKET_KNOCK, dir, attacker, weapon: WEAPON_ROCKET });
}

fn fire_rocket(world: &mut World, id: u32) {
    let Some(player) = world.players.get(&id) else { return };
    let input = player.input;
    let c = player.character;
    let pitch = from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH);
    let dir = look(from_mrad(input.3), pitch);
    let eye = [c.position[0], c.position[1] + EYE_HEIGHT, c.position[2]];
    // Up against a wall it bursts right there instead of starting on its far side.
    if let Some(t) = world.physics.cast_ray(eye, dir, ROCKET_MUZZLE, 0) {
        burst_rocket(world, id, std::array::from_fn(|i| eye[i] + dir[i] * t), dir);
        return;
    }
    let rocket_id = world.alloc_id();
    world.rockets.insert(
        rocket_id,
        Rocket {
            owner: id,
            position: std::array::from_fn(|i| eye[i] + dir[i] * ROCKET_MUZZLE),
            velocity: dir.map(|v| v * ROCKET_SPEED),
            ttl: ROCKET_TTL_TICKS,
        },
    );
}

/// Flies every rocket one tick, and bursts the ones that meet something.
fn step_rockets(world: &mut World, dt: f32) {
    let ids: Vec<u32> = world.rockets.keys().copied().collect();
    for id in ids {
        let Some(rocket) = world.rockets.get(&id).copied() else { continue };
        let speed = rocket.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        let dir = rocket.velocity.map(|v| v / speed);
        let step = speed * dt;
        let wall = world.physics.cast_ray(rocket.position, dir, step, 0);
        let player = first_player_hit(world, rocket.position, dir, wall.unwrap_or(step), Some(rocket.owner), 0);
        let end = player.map(|(_, t)| t).or(wall);
        let ttl = rocket.ttl.saturating_sub(1);
        if let Some(t) = end {
            world.rockets.remove(&id);
            burst_rocket(world, rocket.owner, std::array::from_fn(|i| rocket.position[i] + dir[i] * t), dir);
        } else if ttl == 0 {
            world.rockets.remove(&id);
            burst_rocket(world, rocket.owner, std::array::from_fn(|i| rocket.position[i] + dir[i] * step), dir);
        } else if let Some(rocket) = world.rockets.get_mut(&id) {
            rocket.position = std::array::from_fn(|i| rocket.position[i] + dir[i] * step);
            rocket.ttl = ttl;
        }
    }
}

fn fire_rifle(world: &mut World, id: u32) {
    let Some(player) = world.players.get(&id) else { return };
    let input = player.input;
    let c = player.character;
    let pitch = from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH);
    let dir = look(from_mrad(input.3), pitch);
    let origin = [c.position[0], c.position[1] + EYE_HEIGHT, c.position[2]];

    let wall = world.physics.cast_ray_hit(origin, dir, RIFLE_RANGE, 0);
    let reach = wall.map_or(RIFLE_RANGE, |(t, _)| t);
    let back = rewind_ticks(world.tick, input.view());
    let hit = first_player_hit(world, origin, dir, reach, Some(id), back);
    let end = hit.map_or(reach, |(_, t)| t);
    if let Some((victim, _)) = hit {
        damage(world, victim, RIFLE_DAMAGE, Some(id), WEAPON_RIFLE);
    } else if let Some((_, Some(car))) = wall {
        damage_vehicle(world, car, RIFLE_VEHICLE_DAMAGE, Some(id));
    }
    let point: [i32; 3] = std::array::from_fn(|i| to_cm(origin[i] + dir[i] * end));
    world.shots.push(ShotEvent(id, WEAPON_RIFLE, point[0], point[1], point[2]));
}

impl Mechanic for Combat {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let ids: Vec<u32> = world.players.keys().copied().collect();
        for id in ids {
            let Some(player) = world.players.get_mut(&id) else { continue };
            player.fire_cooldown = player.fire_cooldown.saturating_sub(1);
            player.rocket_cooldown = player.rocket_cooldown.saturating_sub(1);
            if !player.alive() {
                player.respawn_in = player.respawn_in.saturating_sub(1);
                if player.respawn_in == 0 {
                    player.hp = PLAYER_MAX_HP;
                    spawn::place(world, id);
                }
                continue;
            }
            if player.character.position[1] < KILL_Y {
                damage(world, id, PLAYER_MAX_HP, None, WEAPON_RIFLE);
                continue;
            }
            let firing = player.seat.is_none() && player.input.buttons() & BUTTON_FIRE != 0 && player.fire_cooldown == 0;
            if firing {
                player.fire_cooldown = RIFLE_COOLDOWN_TICKS;
                fire_rifle(world, id);
            }
            let player = &mut *world.players.get_mut(&id).expect("still here");
            let launching = player.alive()
                && player.seat.is_none()
                && player.input.buttons() & BUTTON_ALT != 0
                && player.rocket_cooldown == 0
                && world.rockets.len() < MAX_ROCKETS;
            if launching {
                player.rocket_cooldown = ROCKET_COOLDOWN_TICKS;
                fire_rocket(world, id);
            }
        }
        step_rockets(world, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Command, Game, Outgoing};
    use crate::physics::STANDING_Y;
    use shared::protocol::{InputFrame, Snapshot, decode};
    use shared::quant::to_mrad;
    use shared::world::{WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

    fn game() -> Game {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
        Game::new(map, 0).unwrap()
    }

    fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply });
        (response.try_recv().unwrap().unwrap(), inbox)
    }

    /// Feeds `ticks` frames of `buttons`, facing `yaw`.
    fn hold(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, buttons: u8, ticks: u32) {
        for _ in 0..ticks {
            *seq += 1;
            game.handle(Command::Input(id, 0, vec![InputFrame(*seq, 0, 0, to_mrad(yaw), buttons, 0, 0)]));
            game.step();
        }
    }

    /// Two players 20 m apart on flat ground, clear of the car that parks on the spawn point, the
    /// first facing the second (-Z), both settled.
    fn duel() -> (Game, u32, u32, u32, u32) {
        let mut game = game();
        let (shooter, _a) = join(&mut game);
        let (target, _b) = join(&mut game);
        std::mem::forget((_a, _b));
        game.world.players.get_mut(&shooter).unwrap().character.position = [100.0, STANDING_Y, 0.0];
        game.world.players.get_mut(&target).unwrap().character.position = [100.0, STANDING_Y, -20.0];
        let (mut sa, mut sb) = (0, 0);
        hold(&mut game, shooter, &mut sa, 0.0, 0, 3);
        hold(&mut game, target, &mut sb, 0.0, 0, 1);
        (game, shooter, target, sa, sb)
    }

    #[test]
    fn a_ray_meets_a_capsule_on_its_side_and_misses_wide() {
        let centre = [0.0, 1.0, -10.0];
        let t = ray_capsule([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0).unwrap();
        assert!((t - 9.65).abs() < 1e-4, "{t}");
        assert_eq!(ray_capsule([0.5, 1.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0), None, "0.5 m to the side");
        assert_eq!(ray_capsule([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 5.0), None, "out of range");
        assert_eq!(ray_capsule([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], centre, 0.5, 0.35, 100.0), None, "behind");
        // Over the head is over the head; just inside the rounded cap is a hit.
        assert_eq!(ray_capsule([0.0, 2.0, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0), None);
        assert!(ray_capsule([0.0, 1.8, 0.0], [0.0, 0.0, -1.0], centre, 0.5, 0.35, 100.0).is_some());
    }

    #[test]
    fn the_rifle_hurts_what_it_points_at_and_credits_the_shooter() {
        let (mut game, shooter, target, mut seq, _) = duel();
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 3);
        let hp = game.world.players[&target].hp;
        assert_eq!(hp, PLAYER_MAX_HP - RIFLE_DAMAGE, "one shot while held for three ticks");
        assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP);

        let shots_needed = PLAYER_MAX_HP.div_ceil(RIFLE_DAMAGE) as u32;
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, shots_needed * RIFLE_COOLDOWN_TICKS);
        assert!(!game.world.players[&target].alive());
        assert_eq!(game.world.players[&shooter].kills, 1);
        assert_eq!(game.world.players[&target].deaths, 1);
    }

    #[test]
    fn a_shot_at_the_ground_hits_nobody_and_ends_on_it() {
        let (mut game, shooter, target, _, _) = duel();
        game.world.players.get_mut(&shooter).unwrap().input = InputFrame(1, 0, 0, 0, BUTTON_FIRE, to_mrad(-0.8), 0);
        game.world.shots.clear();
        fire_rifle(&mut game.world, shooter);
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
        let end = game.world.shots.last().expect("a shot was recorded");
        assert!(end.3.abs() < 20, "ends on the ground, y = {} cm", end.3);
        assert!(end.4 < 0 && end.4 > -2000, "a little ahead of the shooter: z = {} cm", end.4);
    }

    #[test]
    fn dead_players_do_not_fire_and_cannot_be_hit_until_they_respawn() {
        let (mut game, shooter, target, mut seq, mut seq_t) = duel();
        damage(&mut game.world, target, PLAYER_MAX_HP, Some(shooter), WEAPON_RIFLE);
        assert_eq!(game.world.kills.len(), 1);
        assert_eq!(game.world.kills[0].1, shooter);
        // The dead hold fire at the shooter; nothing happens.
        hold(&mut game, target, &mut seq_t, std::f32::consts::PI, BUTTON_FIRE, 30);
        assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP);
        // And are not hit again while down.
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 30);
        assert_eq!(game.world.players[&target].deaths, 1);
        assert_eq!(game.world.players[&shooter].kills, 1);

        hold(&mut game, target, &mut seq_t, 0.0, 0, RESPAWN_TICKS);
        let target = &game.world.players[&target];
        assert!(target.alive());
        assert_eq!(target.hp, PLAYER_MAX_HP);
    }

    #[test]
    fn falling_out_of_the_world_is_a_death_with_no_killer() {
        let (mut game, shooter, target, _, mut seq_t) = duel();
        game.world.players.get_mut(&target).unwrap().character.position[1] = KILL_Y - 1.0;
        hold(&mut game, target, &mut seq_t, 0.0, 0, 1);
        assert!(!game.world.players[&target].alive());
        assert_eq!(game.world.players[&target].deaths, 1);
        assert_eq!(game.world.players[&shooter].kills, 0);
    }

    #[test]
    fn snapshots_carry_own_health_and_shot_events() {
        let mut game = game();
        let (shooter, mut inbox) = join(&mut game);
        game.world.players.get_mut(&shooter).unwrap().character.position = [100.0, STANDING_Y, 0.0];
        let mut seq = 0;
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 6);
        let snapshots: Vec<Snapshot> = std::iter::from_fn(|| inbox.try_recv().ok())
            .filter_map(|out| match out {
                Outgoing::Datagram(bytes) => decode(&bytes),
                Outgoing::Reliable(_) => None,
            })
            .collect();
        assert!(!snapshots.is_empty());
        assert!(snapshots.iter().all(|s| s.11 == PLAYER_MAX_HP));
        let shots: Vec<_> = snapshots.iter().flat_map(|s| s.12.iter()).collect();
        assert_eq!(shots.len(), 1, "one shot in six ticks at this cooldown");
        assert_eq!(shots[0].0, shooter);
    }

    /// The one car in the test world, and a player sitting in its driver seat.
    fn driver_in_the_car(game: &mut Game) -> (u32, u32) {
        let (&vid, _) = game.world.vehicles.iter().next().expect("a car is placed at the spawn point");
        let (id, inbox) = join(game);
        std::mem::forget(inbox);
        game.world.vehicles.get_mut(&vid).unwrap().seats[0] = Some(id);
        game.world.players.get_mut(&id).unwrap().seat = Some(crate::game::Seat { vehicle: vid, index: 0 });
        (vid, id)
    }

    #[test]
    fn a_rocket_flies_and_bursts_on_the_target_and_hurts_it() {
        let (mut game, shooter, target, mut seq, _) = duel();
        // 20 m at 45 m/s is under half a second.
        for _ in 0..40 {
            seq += 1;
            game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, BUTTON_ALT, 0, 0)]));
            game.step();
            if game.world.players[&target].hp < PLAYER_MAX_HP {
                break;
            }
        }
        let hp = game.world.players[&target].hp;
        assert!(hp < PLAYER_MAX_HP, "the rocket landed");
        assert!(hp <= PLAYER_MAX_HP - 90, "a direct hit is nearly lethal: {hp}");
        assert!(game.world.rockets.is_empty(), "a burst rocket is gone");
        assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP, "20 m away is outside the blast");
    }

    #[test]
    fn rockets_are_paced_and_capped() {
        let (mut game, shooter, _, mut seq, _) = duel();
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_ALT, 3);
        assert_eq!(game.world.rockets.len(), 1);
        // The first has burst on the target by now; the trigger is still held but the launcher is reloading.
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_ALT, 33);
        assert!(game.world.rockets.is_empty(), "no second launch within the cooldown");
        assert!(game.world.players[&shooter].rocket_cooldown > 0);
    }

    #[test]
    fn a_rocket_into_a_wall_next_to_you_hurts_you_and_credits_nobody() {
        let (mut game, shooter, _, mut seq, _) = duel();
        // Straight down at the feet: bursts a metre or so away.
        for _ in 0..4 {
            seq += 1;
            game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, BUTTON_ALT, to_mrad(-1.4), 0)]));
            game.step();
        }
        assert!(game.world.players[&shooter].hp < PLAYER_MAX_HP, "hit by your own blast");
        assert_eq!(game.world.players[&shooter].kills, 0);
    }

    #[test]
    fn the_rifle_chips_a_vehicle_and_a_rocket_wrecks_it() {
        let mut game = game();
        let (vid, driver) = driver_in_the_car(&mut game);
        let (shooter, _inbox) = join(&mut game);
        std::mem::forget(_inbox);
        let pose = game.world.physics.car_pose(vid).unwrap();
        game.world.players.get_mut(&shooter).unwrap().character.position = [pose.position[0], STANDING_Y, pose.position[2] + 15.0];
        let mut seq = 0;
        hold(&mut game, shooter, &mut seq, 0.0, 0, 3);
        // The eye is above the roof; look a little down at the car.
        for _ in 0..3 {
            seq += 1;
            game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, BUTTON_FIRE, to_mrad(-0.05), 0)]));
            game.step();
        }
        let max = shared::protocol::vehicle_hp(shared::protocol::KIND_CAR);
        assert_eq!(game.world.vehicles[&vid].hp, max - RIFLE_VEHICLE_DAMAGE);

        damage_vehicle(&mut game.world, vid, max, Some(shooter));
        let vehicle = &game.world.vehicles[&vid];
        assert!(vehicle.wrecked());
        assert_eq!(vehicle.seats, [None; CAR_SEATS], "the crew is out of their seats");
        assert!(!game.world.players[&driver].alive(), "and dead");
        assert_eq!(game.world.players[&shooter].kills, 1, "the credit goes to whoever wrecked it");
        assert!(game.world.shots.iter().any(|shot| shot.1 == WEAPON_WRECK));
    }

    #[test]
    fn a_wreck_cannot_be_boarded_and_comes_back_whole() {
        let mut game = game();
        let (vid, _driver) = driver_in_the_car(&mut game);
        damage_vehicle(&mut game.world, vid, 255, None);
        assert!(game.world.vehicles[&vid].wrecked());
        let (id, inbox) = join(&mut game);
        std::mem::forget(inbox);
        let pose = game.world.physics.car_pose(vid).unwrap();
        game.world.players.get_mut(&id).unwrap().character.position = [pose.position[0] + 2.0, STANDING_Y, pose.position[2]];
        let mut seq = 0;
        hold(&mut game, id, &mut seq, 0.0, 0, 3);
        hold(&mut game, id, &mut seq, 0.0, BUTTON_USE, 1);
        hold(&mut game, id, &mut seq, 0.0, 0, 3);
        assert!(game.world.players[&id].seat.is_none(), "nobody boards a wreck");

        for _ in 0..WRECK_TICKS {
            game.step();
        }
        let vehicle = &game.world.vehicles[&vid];
        assert!(!vehicle.wrecked());
        assert_eq!(vehicle.hp, shared::protocol::vehicle_hp(shared::protocol::KIND_CAR));
    }

    #[test]
    fn a_rocket_in_flight_is_in_the_snapshot() {
        let (mut game, shooter, _, mut seq, _) = duel();
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_ALT, 1);
        hold(&mut game, shooter, &mut seq, 0.0, 0, 1);
        let rockets = game.projectile_states();
        assert_eq!(rockets.len(), 1);
        let shared::protocol::ProjectileState(_, _, _, z, _, _, vz) = rockets[0];
        assert!(z < 0, "flying towards -Z from the shooter at z = 0: {z}");
        assert!((vz as f32 / 100.0 + ROCKET_SPEED).abs() < 0.1, "{vz}");
    }

    /// The target steps 5 m aside, then the shooter fires a frame whose picture was `lag` ticks old.
    /// Returns the target's health afterwards.
    fn shoot_at_a_target_that_has_moved(lag: u32) -> u8 {
        let (mut game, shooter, target, mut seq, _) = duel();
        for _ in 0..3 {
            game.step();
        }
        let tick_before = game.world.tick;
        game.world.players.get_mut(&target).unwrap().character.position = [105.0, STANDING_Y, -20.0];
        hold(&mut game, shooter, &mut seq, 0.0, 0, 4);
        // The picture on the shooter's screen was `lag` ticks behind the server.
        let view = game.world.tick.wrapping_sub(lag);
        assert!(view <= tick_before + 8, "the test means to look at a moment around the move");
        seq += 1;
        game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, BUTTON_FIRE, 0, view as u16)]));
        hold(&mut game, shooter, &mut seq, 0.0, 0, 3);
        game.world.players[&target].hp
    }

    #[test]
    fn a_shot_hits_where_the_shooter_saw_the_target() {
        // Seen 6 ticks ago, when the target still stood in line.
        assert_eq!(shoot_at_a_target_that_has_moved(6), PLAYER_MAX_HP - RIFLE_DAMAGE);
        // Seen right now, the target is 5 m to the side.
        assert_eq!(shoot_at_a_target_that_has_moved(0), PLAYER_MAX_HP);
    }

    #[test]
    fn rewinding_is_capped_and_never_forwards() {
        assert_eq!(rewind_ticks(1000, 1000), 0);
        assert_eq!(rewind_ticks(1000, 994), 6);
        assert_eq!(rewind_ticks(1000, 900), MAX_REWIND_TICKS, "a very old picture is capped");
        assert_eq!(rewind_ticks(1000, 1010), 0, "a picture from the future is no picture");
        // The wire carries the tick modulo 65536.
        assert_eq!(rewind_ticks(65_540, 65_534), 6);
        assert_eq!(rewind_ticks(70_000, (69_994 % 65_536) as u16), 6);
    }
}
