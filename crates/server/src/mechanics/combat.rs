use super::Mechanic;
use super::spawn;
use crate::game::{Rocket, World};
use shared::consts::*;
use shared::items::*;
use shared::protocol::{ItemSlot, Kill, PROJECTILE_BULLET, PROJECTILE_GRENADE, PROJECTILE_CANNON, PROJECTILE_ROCKET, PROJECTILE_SHELL, S_KILL, ShotEvent, WEAPON_GRENADE, WEAPON_RIFLE, WEAPON_ROCKET, WEAPON_SNIPER, WEAPON_WRECK};
use shared::quant::{from_mrad, to_cm};

/// Health, death, the items in the hotbar and what blasts do to people and vehicles.
///
/// Every player has `hp`. `damage` is the one place health goes down: at zero the player dies,
/// the attacker is credited, and a `Kill` is queued for the reliable stream. A dead player lies
/// where they fell, can be neither hit nor hurt, and respawns after `RESPAWN_TICKS` with full health.
///
/// What `BUTTON_FIRE` does depends on the item in the selected hotbar slot: a gun is a hitscan
/// from the eye along the look direction, a grenade is thrown, a bandage is applied. Seated
/// players never use items: a gunner's trigger belongs to the gun.
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
    // A bandage needs a steady hand.
    player.channel_left = 0;
    if player.hp > 0 {
        return;
    }
    player.respawn_in = RESPAWN_TICKS;
    player.deaths = player.deaths.saturating_add(1);
    player.fire_cooldown = 0;
    player.interrupt();
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
    world.physics.kick_car(vid, [0.0, 9.0, 0.0]);
    // ...and tumbling: a roll or a somersault, either way round, picked by the vehicle's id and
    // where it stood so that two wrecks do not turn alike.
    let seed = (vid as f32 * 12.9898 + pose.position[0] * 78.233 + pose.position[2] * 37.719).sin() * 43758.547;
    let pick = seed - seed.floor();
    let rate = WRECK_SPIN * (0.7 + 0.6 * ((pick * 7.0) - (pick * 7.0).floor()));
    let sign = if pick < 0.5 { -1.0 } else { 1.0 };
    let spin = if (pick * 4.0) as u32 % 2 == 0 { [0.0, 0.0, sign * rate] } else { [sign * rate, 0.0, 0.0] };
    world.physics.spin_car(vid, spin);
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

/// Launches a rocket from player `id`'s eye. No item does this yet; it is kept for a launcher pickup.
#[allow(dead_code)]
fn fire_rocket(world: &mut World, id: u32) {
    let Some(player) = world.players.get(&id) else { return };
    let input = player.input;
    let c = player.character;
    let pitch = from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH);
    let dir = look(from_mrad(input.3), pitch);
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons()), c.position[2]];
    // Up against a wall it bursts right there instead of starting on its far side.
    if let Some(t) = world.physics.cast_ray(eye, dir, ROCKET_MUZZLE, 0) {
        burst_rocket(world, id, std::array::from_fn(|i| eye[i] + dir[i] * t), dir);
        return;
    }
    let rocket_id = world.alloc_id();
    world.rockets.insert(
        rocket_id,
        Rocket {
            kind: PROJECTILE_ROCKET,
            owner: id,
            source: 0,
            position: std::array::from_fn(|i| eye[i] + dir[i] * ROCKET_MUZZLE),
            velocity: dir.map(|v| v * ROCKET_SPEED),
            ttl: ROCKET_TTL_TICKS,
        },
    );
}

/// Flies one sniper round a tick: a little gravity, then along its velocity until it meets a wall, a
/// vehicle or a stickman. A stickman takes the gun's damage; a vehicle a quarter of it.
fn step_bullet(world: &mut World, id: u32, dt: f32) {
    let Some(mut bullet) = world.rockets.get(&id).copied() else { return };
    bullet.velocity[1] -= GRAVITY * BULLET_GRAVITY_SCALE * dt;
    let speed = bullet.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
    let dir = bullet.velocity.map(|v| v / speed);
    let step = speed * dt;
    let wall = world.physics.cast_ray_hit(bullet.position, dir, step, 0);
    let reach = wall.map_or(step, |(t, _)| t);
    let hit = first_player_hit(world, bullet.position, dir, reach, Some(bullet.owner), 0);
    bullet.ttl = bullet.ttl.saturating_sub(1);
    if hit.is_some() || wall.is_some() {
        world.rockets.remove(&id);
        let end = hit.map_or(reach, |(_, t)| t);
        let point: [f32; 3] = std::array::from_fn(|i| bullet.position[i] + dir[i] * end);
        let damage_dealt = def(ITEM_SNIPER).map_or(0, |d| d.damage);
        let attacker = world.players.contains_key(&bullet.owner).then_some(bullet.owner);
        if let Some((victim, _)) = hit {
            damage(world, victim, damage_dealt, attacker, WEAPON_SNIPER);
        } else if let Some((_, Some(car))) = wall {
            damage_vehicle(world, car, (damage_dealt / 4).max(1), attacker);
        }
        let [x, y, z] = point.map(to_cm);
        world.shots.push(ShotEvent(bullet.owner, WEAPON_SNIPER, x, y, z));
        return;
    }
    bullet.position = std::array::from_fn(|i| bullet.position[i] + dir[i] * step);
    if bullet.ttl == 0 || bullet.position[1] < KILL_Y {
        world.rockets.remove(&id);
    } else {
        world.rockets.insert(id, bullet);
    }
}

/// Flies every rocket one tick, and bursts the ones that meet something.
fn step_rockets(world: &mut World, dt: f32) {
    let ids: Vec<u32> = world.rockets.keys().copied().collect();
    for id in ids {
        let Some(rocket) = world.rockets.get(&id).copied() else { continue };
        if rocket.kind == PROJECTILE_GRENADE {
            step_grenade(world, id, dt);
            continue;
        }
        if rocket.kind == PROJECTILE_BULLET {
            step_bullet(world, id, dt);
            continue;
        }
        // Shells are the `Gunnery` mechanic's.
        if rocket.kind == PROJECTILE_SHELL || rocket.kind == PROJECTILE_CANNON {
            continue;
        }
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

/// A unit vector `spread` radians at most from `dir`, uniformly in a cone.
pub fn scatter(world: &mut World, dir: [f32; 3], spread: f32) -> [f32; 3] {
    if spread <= 0.0 {
        return dir;
    }
    // Two unit vectors at right angles to `dir`.
    let up = if dir[1].abs() > 0.99 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |v: [f32; 3]| {
        let n = v.iter().map(|c| c * c).sum::<f32>().sqrt();
        v.map(|c| c / n)
    };
    let right = norm(cross(dir, up));
    let above = cross(right, dir);
    let angle = world.rng.next_f64() as f32 * std::f32::consts::TAU;
    let radius = (world.rng.next_f64() as f32).sqrt() * spread;
    let (a, b) = (radius * angle.cos(), radius * angle.sin());
    norm(std::array::from_fn(|i| dir[i] + right[i] * a + above[i] * b))
}

/// The direction from `eye` to what the over-the-shoulder camera's crosshair is on: a wall or a
/// player (as `shooter` saw them `back` ticks ago) within `range`, else the far end of its line. The camera is a function of yaw and pitch alone, so the
/// server finds the same ray the player sees. Falls back to `look_dir` when the camera ray starts
/// inside geometry or the point would send the shot sideways.
fn crosshair_dir(world: &World, shooter: u32, back: u32, position: [f32; 3], eye: [f32; 3], yaw: f32, look_dir: [f32; 3], range: f32) -> [f32; 3] {
    // The camera's pivot is to the right of the capsule, which is (cos yaw, 0, -sin yaw).
    let pivot = [position[0] + yaw.cos() * ON_FOOT_SHOULDER, position[1] + ON_FOOT_HEIGHT, position[2] - yaw.sin() * ON_FOOT_SHOULDER];
    let wall = world.physics.cast_ray(pivot, look_dir, range, 0).unwrap_or(range);
    let t = first_player_hit(world, pivot, look_dir, wall, Some(shooter), back).map_or(wall, |(_, t)| t);
    let to: [f32; 3] = std::array::from_fn(|i| pivot[i] + look_dir[i] * t - eye[i]);
    let len = to.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len < 1e-3 {
        return look_dir;
    }
    let dir = to.map(|v| v / len);
    let along: f32 = (0..3).map(|i| dir[i] * look_dir[i]).sum();
    if along < 0.5 { look_dir } else { dir }
}

/// The cone a shot of `item` is scattered over: aim held (`BUTTON_ALT`) tightens it.
fn effective_spread(item: &ItemDef, buttons: u8) -> f32 {
    if buttons & BUTTON_ALT != 0 { item.spread * AIM_SPREAD_SCALE } else { item.spread }
}

/// Fires gun `item` from player `id`'s eye, towards what the crosshair is on.
fn fire_gun(world: &mut World, id: u32, item: &ItemDef) {
    let Some(player) = world.players.get(&id) else { return };
    let input = player.input;
    let c = player.character;
    let pitch = from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH);
    let yaw = from_mrad(input.3);
    let look_dir = look(yaw, pitch);
    let origin = [c.position[0], c.position[1] + eye_height(input.buttons()), c.position[2]];
    let back = rewind_ticks(world.tick, input.view());
    // Looking down a scope the camera is the eye, so the shot goes straight along the look
    // direction, true.
    let scoped = item.scope && input.buttons() & BUTTON_ALT != 0;
    let dir = if scoped {
        look_dir
    } else {
        let aim = crosshair_dir(world, id, back, c.position, origin, yaw, look_dir, item.range);
        scatter(world, aim, effective_spread(item, input.buttons()))
    };

    if item.projectile {
        // The round leaves the eye and flies on its own, so it takes time to arrive and drops a little.
        let bullet_id = world.alloc_id();
        world.rockets.insert(
            bullet_id,
            Rocket {
                kind: PROJECTILE_BULLET,
                owner: id,
                source: 0,
                position: origin,
                velocity: dir.map(|v| v * BULLET_SPEED),
                ttl: (item.range / BULLET_SPEED * TICK_RATE as f32).ceil() as u32,
            },
        );
        return;
    }
    let wall = world.physics.cast_ray_hit(origin, dir, item.range, 0);
    let reach = wall.map_or(item.range, |(t, _)| t);
    let hit = first_player_hit(world, origin, dir, reach, Some(id), back);
    let end = hit.map_or(reach, |(_, t)| t);
    if let Some((victim, _)) = hit {
        damage(world, victim, item.damage, Some(id), item.weapon);
    } else if let Some((_, Some(car))) = wall {
        // A nuisance, not a way to kill a vehicle.
        damage_vehicle(world, car, (item.damage / 4).max(1), Some(id));
    }
    let point: [i32; 3] = std::array::from_fn(|i| to_cm(origin[i] + dir[i] * end));
    world.shots.push(ShotEvent(id, item.weapon, point[0], point[1], point[2]));
}

/// Throws a grenade from player `id`'s eye, a little above where they look so it arcs.
fn throw_grenade(world: &mut World, id: u32) {
    let Some(player) = world.players.get(&id) else { return };
    let input = player.input;
    let c = player.character;
    let pitch = (from_mrad(input.pitch()) + GRENADE_LOFT).clamp(-MAX_PITCH, MAX_PITCH);
    let dir = look(from_mrad(input.3), pitch);
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons()), c.position[2]];
    // Up against a wall it starts at the wall instead of on its far side.
    let reach = world.physics.cast_ray(eye, dir, GRENADE_MUZZLE + GRENADE_RADIUS, 0).map_or(GRENADE_MUZZLE, |t| (t - GRENADE_RADIUS).max(0.0));
    let grenade_id = world.alloc_id();
    world.rockets.insert(
        grenade_id,
        Rocket {
            kind: PROJECTILE_GRENADE,
            owner: id,
            source: 0,
            position: std::array::from_fn(|i| eye[i] + dir[i] * reach),
            velocity: dir.map(|v| v * GRENADE_SPEED),
            ttl: GRENADE_FUSE_TICKS,
        },
    );
}

fn burst_grenade(world: &mut World, grenade: &Rocket) {
    let [x, y, z] = grenade.position.map(to_cm);
    world.shots.push(ShotEvent(grenade.owner, WEAPON_GRENADE, x, y, z));
    let attacker = world.players.contains_key(&grenade.owner).then_some(grenade.owner);
    blast(
        world,
        Blast {
            point: grenade.position,
            radius: GRENADE_BLAST_RADIUS,
            damage: GRENADE_DAMAGE,
            knock: GRENADE_KNOCK,
            dir: [0.0, 1.0, 0.0],
            attacker,
            weapon: WEAPON_GRENADE,
        },
    );
}

/// Flies one grenade a tick: gravity, a bounce off whatever it meets, and a burst when the fuse is out.
fn step_grenade(world: &mut World, id: u32, dt: f32) {
    let Some(mut grenade) = world.rockets.get(&id).copied() else { return };
    grenade.ttl = grenade.ttl.saturating_sub(1);
    grenade.velocity[1] -= GRAVITY * dt;
    let mut remaining = dt;
    // A grenade can meet a surface more than once in a tick in a corner; three is plenty.
    for _ in 0..3 {
        let speed = grenade.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        if speed < 1e-3 {
            break;
        }
        let dir = grenade.velocity.map(|v| v / speed);
        let step = speed * remaining;
        match world.physics.cast_ray_normal(grenade.position, dir, step + GRENADE_RADIUS) {
            Some((t, normal)) => {
                let travel = (t - GRENADE_RADIUS).clamp(0.0, step);
                grenade.position = std::array::from_fn(|i| grenade.position[i] + dir[i] * travel);
                // Reflect, keeping some of the speed, and rolling to rest on a floor.
                let along: f32 = (0..3).map(|i| grenade.velocity[i] * normal[i]).sum();
                grenade.velocity = std::array::from_fn(|i| (grenade.velocity[i] - 2.0 * along * normal[i]) * GRENADE_BOUNCE);
                if grenade.velocity.iter().map(|v| v * v).sum::<f32>() < 0.5 {
                    grenade.velocity = [0.0; 3];
                }
                remaining *= 1.0 - travel / step.max(1e-6);
            }
            None => {
                grenade.position = std::array::from_fn(|i| grenade.position[i] + dir[i] * step);
                break;
            }
        }
    }
    if grenade.ttl == 0 || grenade.position[1] < KILL_Y {
        world.rockets.remove(&id);
        burst_grenade(world, &grenade);
    } else {
        world.rockets.insert(id, grenade);
    }
}

/// The slot in hand after it has been used once: one fewer, and empty when none are left.
fn spend(slot: &mut ItemSlot) {
    slot.1 = slot.1.saturating_sub(1);
    if slot.1 == 0 {
        *slot = ItemSlot(ITEM_NONE, 0, 0);
    }
}

/// One tick of item use for a living, standing player: switching, reloading, and what fire does.
fn use_item(world: &mut World, id: u32) {
    let Some(player) = world.players.get_mut(&id) else { return };
    let buttons = player.input.buttons();
    let slot = (player.input.slot() as usize).min(HOTBAR_SLOTS - 1);
    if slot != player.selected {
        player.selected = slot;
        player.interrupt();
    }
    let pressed = buttons & BUTTON_FIRE != 0;
    let fresh = pressed && !player.trigger_held;
    player.trigger_held = pressed;
    if player.seat.is_some() {
        player.interrupt();
        return;
    }
    let held = player.held();
    let Some(item) = def(held.0).copied() else {
        player.interrupt();
        return;
    };
    match item.kind {
        ItemKind::Gun => {
            if player.reload_left > 0 {
                player.reload_left -= 1;
                if player.reload_left == 0 {
                    player.hotbar[slot].2 = item.mag;
                }
                return;
            }
            let empty = held.2 == 0;
            let wants_reload = buttons & BUTTON_RELOAD != 0 || (empty && (pressed));
            if wants_reload && held.2 < item.mag {
                player.reload_left = item.reload_ticks;
                return;
            }
            let trigger = if item.auto { pressed } else { fresh };
            if trigger && player.fire_cooldown == 0 && !empty {
                player.fire_cooldown = item.cooldown_ticks;
                player.hotbar[slot].2 -= 1;
                fire_gun(world, id, &item);
            }
        }
        ItemKind::Grenade => {
            if fresh && player.fire_cooldown == 0 && world.rockets.len() < MAX_ROCKETS + MAX_GRENADES {
                player.fire_cooldown = item.cooldown_ticks;
                spend(&mut player.hotbar[slot]);
                throw_grenade(world, id);
            }
        }
        ItemKind::Heal => {
            if player.channel_left > 0 {
                player.channel_left -= 1;
                if player.channel_left == 0 {
                    player.hp = player.hp.saturating_add(item.damage).min(PLAYER_MAX_HP);
                    spend(&mut player.hotbar[slot]);
                }
            } else if fresh && player.hp < PLAYER_MAX_HP {
                player.channel_left = BANDAGE_TICKS;
            }
        }
    }
}

impl Mechanic for Combat {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let ids: Vec<u32> = world.players.keys().copied().collect();
        for id in ids {
            let Some(player) = world.players.get_mut(&id) else { continue };
            player.fire_cooldown = player.fire_cooldown.saturating_sub(1);
            if !player.alive() {
                player.respawn_in = player.respawn_in.saturating_sub(1);
                if player.respawn_in == 0 {
                    player.hp = PLAYER_MAX_HP;
                    // A fresh life starts empty-handed.
                    player.hotbar = Default::default();
                    player.interrupt();
                    spawn::place(world, id);
                }
                continue;
            }
            if player.character.position[1] < KILL_Y {
                damage(world, id, PLAYER_MAX_HP, None, WEAPON_RIFLE);
                continue;
            }
            use_item(world, id);
        }
        step_rockets(world, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Command, Game, Outgoing};
    use crate::physics::STANDING_Y;
    use shared::protocol::{InputFrame, Snapshot, WEAPON_REVOLVER, decode};
    use shared::quant::to_mrad;
    use shared::world::{WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

    fn game() -> Game {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
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

    /// The camera is above and to the right of the eye, so a shooter aims a little down to put the
    /// crosshair on a target 20 m away at the same height.
    const AIM_20M: f32 = -0.075;

    /// How a player aims with `slot` in hand: a gun's crosshair on a target 20 m away, anything else level.
    fn aim_for(game: &Game, id: u32, slot: usize) -> f32 {
        let item = game.world.players[&id].hotbar[slot].0;
        if def(item).is_some_and(|d| d.kind == ItemKind::Gun) { AIM_20M } else { 0.0 }
    }

    /// Feeds `ticks` frames of `buttons`, facing `yaw`.
    fn hold(game: &mut Game, id: u32, seq: &mut u32, yaw: f32, buttons: u8, ticks: u32) {
        let aim = aim_for(game, id, 0);
        for _ in 0..ticks {
            *seq += 1;
            game.handle(Command::Input(id, 0, vec![InputFrame(*seq, 0, 0, to_mrad(yaw), buttons, to_mrad(aim), 0, 0)]));
            game.step();
        }
    }

    /// Two players 20 m apart on flat ground, clear of the car that parks on the spawn point, the
    /// first facing the second (-Z), both settled. The target stands where the shooter's shoulder
    /// camera looks, as a player turns until the crosshair is on someone.
    fn duel() -> (Game, u32, u32, u32, u32) {
        let mut game = game();
        let (shooter, _a) = join(&mut game);
        let (target, _b) = join(&mut game);
        std::mem::forget((_a, _b));
        game.world.players.get_mut(&shooter).unwrap().character.position = [100.0, STANDING_Y, 0.0];
        game.world.players.get_mut(&target).unwrap().character.position = [100.0 + ON_FOOT_SHOULDER, STANDING_Y, -20.0];
        // The shooter carries an AK5 in slot 0, which is where `hold` keeps the selection.
        give(&mut game, shooter, 0, ITEM_AK5, 1);
        let (mut sa, mut sb) = (0, 0);
        hold(&mut game, shooter, &mut sa, 0.0, 0, 3);
        hold(&mut game, target, &mut sb, 0.0, 0, 1);
        (game, shooter, target, sa, sb)
    }

    fn give(game: &mut Game, id: u32, slot: usize, item: u8, count: u8) {
        let mag = def(item).map_or(0, |d| d.mag);
        game.world.players.get_mut(&id).unwrap().hotbar[slot] = ItemSlot(item, count, mag);
    }

    /// One tick of `buttons` with `slot` selected, looking `pitch` radians up.
    fn press(game: &mut Game, id: u32, seq: &mut u32, buttons: u8, slot: u8, pitch: f32) {
        *seq += 1;
        game.handle(Command::Input(id, 0, vec![InputFrame(*seq, 0, 0, 0, buttons, to_mrad(pitch), 0, slot)]));
        game.step();
    }

    fn ak5() -> &'static ItemDef {
        def(ITEM_AK5).unwrap()
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
    fn the_ak5_hurts_what_it_points_at_and_credits_the_shooter() {
        let (mut game, shooter, target, mut seq, _) = duel();
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 3);
        let hp = game.world.players[&target].hp;
        assert_eq!(hp, PLAYER_MAX_HP - ak5().damage, "one shot while held for three ticks");
        assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP);

        let shots_needed = PLAYER_MAX_HP.div_ceil(ak5().damage) as u32;
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, shots_needed * ak5().cooldown_ticks);
        assert!(!game.world.players[&target].alive());
        assert_eq!(game.world.players[&shooter].kills, 1);
        assert_eq!(game.world.players[&target].deaths, 1);
    }

    #[test]
    fn a_shot_at_the_ground_hits_nobody_and_ends_on_it() {
        let (mut game, shooter, target, _, _) = duel();
        game.world.players.get_mut(&shooter).unwrap().input = InputFrame(1, 0, 0, 0, BUTTON_FIRE, to_mrad(-0.8), 0, 0);
        game.world.shots.clear();
        fire_gun(&mut game.world, shooter, ak5());
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
        let end = game.world.shots.last().expect("a shot was recorded");
        assert!(end.3.abs() < 20, "ends on the ground, y = {} cm", end.3);
        assert!(end.4 < 0 && end.4 > -2000, "a little ahead of the shooter: z = {} cm", end.4);
    }

    #[test]
    fn a_shot_goes_where_the_shoulder_camera_crosshair_is() {
        let (mut game, shooter, _, _, _) = duel();
        // The pivot is 0.9 m to the right; a target whose centre is on that line is under the crosshair,
        // and one 5 m off to the side is not.
        game.world.players.get_mut(&shooter).unwrap().input = InputFrame(1, 0, 0, 0, BUTTON_FIRE, 0, 0, 0);
        let c = game.world.players[&shooter].character;
        let pivot_y = c.position[1] + ON_FOOT_HEIGHT;
        let eye = [c.position[0], c.position[1] + EYE_HEIGHT, c.position[2]];
        let dir = crosshair_dir(&game.world, shooter, 0, c.position, eye, 0.0, look(0.0, 0.0), 100.0);
        // Over 100 m the line from the eye crosses the camera's ray, which is 0.9 m to the right and 0.9 m above.
        let at = |t: f32| [eye[0] + dir[0] * t, eye[1] + dir[1] * t];
        assert!((at(100.0)[0] - (c.position[0] + ON_FOOT_SHOULDER)).abs() < 0.05, "x {}", at(100.0)[0]);
        assert!((at(100.0)[1] - pivot_y).abs() < 0.1, "y {}", at(100.0)[1]);
        assert!(dir[2] < -0.99, "still forwards: {dir:?}");
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
        give(&mut game, shooter, 0, ITEM_AK5, 1);
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 6);
        let snapshots: Vec<Snapshot> = std::iter::from_fn(|| inbox.try_recv().ok())
            .filter_map(|out| match out {
                Outgoing::Datagram(bytes) => decode(&bytes),
                Outgoing::Reliable(_) => None,
            })
            .collect();
        assert!(!snapshots.is_empty());
        assert!(snapshots.iter().all(|s| s.11.0 == PLAYER_MAX_HP));
        let last = snapshots.last().unwrap();
        assert_eq!(last.11.2[0], ItemSlot(ITEM_AK5, 1, ak5().mag - 1), "own hotbar, with a round fired");
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
        // A rocket flies along the look direction from the eye, so it is the target that stands in its line.
        game.world.players.get_mut(&target).unwrap().character.position[0] = 100.0;
        game.world.players.get_mut(&shooter).unwrap().input = InputFrame(0, 0, 0, 0, 0, 0, 0, 0);
        fire_rocket(&mut game.world, shooter);
        // 20 m at 45 m/s is under half a second.
        for _ in 0..40 {
            seq += 1;
            game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, 0, 0, 0, 0)]));
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
    fn a_rocket_into_a_wall_next_to_you_hurts_you_and_credits_nobody() {
        let (mut game, shooter, _, mut seq, _) = duel();
        // Straight down at the feet: bursts a metre or so away.
        game.world.players.get_mut(&shooter).unwrap().input = InputFrame(1, 0, 0, 0, 0, to_mrad(-1.4), 0, 0);
        fire_rocket(&mut game.world, shooter);
        for _ in 0..4 {
            seq += 1;
            game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, 0, to_mrad(-1.4), 0, 0)]));
            game.step();
        }
        assert!(game.world.players[&shooter].hp < PLAYER_MAX_HP, "hit by your own blast");
        assert_eq!(game.world.players[&shooter].kills, 0);
    }

    #[test]
    fn the_ak5_chips_a_vehicle_and_a_wreck_kills_the_crew() {
        let mut game = game();
        let (vid, driver) = driver_in_the_car(&mut game);
        let (shooter, _inbox) = join(&mut game);
        std::mem::forget(_inbox);
        give(&mut game, shooter, 0, ITEM_AK5, 1);
        let pose = game.world.physics.car_pose(vid).unwrap();
        game.world.players.get_mut(&shooter).unwrap().character.position = [pose.position[0], STANDING_Y, pose.position[2] + 15.0];
        let mut seq = 0;
        hold(&mut game, shooter, &mut seq, 0.0, 0, 3);
        // The eye is above the roof; look a little down at the car.
        for _ in 0..3 {
            seq += 1;
            game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, BUTTON_FIRE, to_mrad(-0.05), 0, 0)]));
            game.step();
        }
        let max = shared::protocol::vehicle_hp(shared::protocol::KIND_JEEP);
        assert_eq!(game.world.vehicles[&vid].hp, max - (ak5().damage / 4).max(1));

        damage_vehicle(&mut game.world, vid, max, Some(shooter));
        let vehicle = &game.world.vehicles[&vid];
        assert!(vehicle.wrecked());
        assert_eq!(vehicle.seats, [None; CAR_SEATS], "the crew is out of their seats");
        assert!(!game.world.players[&driver].alive(), "and dead");
        assert_eq!(game.world.players[&shooter].kills, 1, "the credit goes to whoever wrecked it");
        assert!(game.world.shots.iter().any(|shot| shot.1 == WEAPON_WRECK));
    }

    #[test]
    fn a_wreck_tumbles_as_it_is_thrown_up() {
        let mut game = game();
        let (vid, _driver) = driver_in_the_car(&mut game);
        damage_vehicle(&mut game.world, vid, 255, None);
        let motion = game.world.physics.car_motion(vid).unwrap();
        let spin = motion.angvel.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(spin > 3.0, "a wreck is sent turning over, not just straight up (spin {spin})");
        assert!(motion.linvel[1] > 5.0);
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
        assert_eq!(vehicle.hp, shared::protocol::vehicle_hp(shared::protocol::KIND_JEEP));
    }

    #[test]
    fn a_rocket_in_flight_is_in_the_snapshot() {
        let (mut game, shooter, _, mut seq, _) = duel();
        game.world.players.get_mut(&shooter).unwrap().input = InputFrame(0, 0, 0, 0, 0, 0, 0, 0);
        fire_rocket(&mut game.world, shooter);
        hold(&mut game, shooter, &mut seq, 0.0, 0, 1);
        let rockets = game.projectile_states();
        assert_eq!(rockets.len(), 1);
        let shared::protocol::ProjectileState(_, kind, _, _, z, _, _, vz) = rockets[0];
        assert_eq!(kind, PROJECTILE_ROCKET);
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
        game.handle(Command::Input(shooter, 0, vec![InputFrame(seq, 0, 0, 0, BUTTON_FIRE, to_mrad(aim_for(&game, shooter, 0)), view as u16, 0)]));
        hold(&mut game, shooter, &mut seq, 0.0, 0, 3);
        game.world.players[&target].hp
    }

    #[test]
    fn a_shot_hits_where_the_shooter_saw_the_target() {
        // Seen 6 ticks ago, when the target still stood in line.
        assert_eq!(shoot_at_a_target_that_has_moved(6), PLAYER_MAX_HP - ak5().damage);
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

    #[test]
    fn an_empty_hand_does_nothing() {
        let (mut game, shooter, target, mut seq, _) = duel();
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 4, 10);
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
        assert!(game.world.shots.is_empty());
    }

    fn hold_slot(game: &mut Game, id: u32, seq: &mut u32, buttons: u8, slot: u8, ticks: u32) {
        for _ in 0..ticks {
            press(game, id, seq, buttons, slot, aim_for(game, id, slot as usize));
        }
    }

    #[test]
    fn the_revolver_needs_a_press_per_shot_and_hits_hard() {
        let (mut game, shooter, target, mut seq, _) = duel();
        give(&mut game, shooter, 1, ITEM_REVOLVER, 1);
        let revolver = def(ITEM_REVOLVER).unwrap();
        // Held down for a second: still one shot.
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 1, TICK_RATE);
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP - revolver.damage);
        assert_eq!(game.world.players[&shooter].hotbar[1].2, revolver.mag - 1);
        // Let go and press again once the hammer is back.
        hold_slot(&mut game, shooter, &mut seq, 0, 1, revolver.cooldown_ticks);
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 1, 1);
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP - 2 * revolver.damage);
        assert!(game.world.shots.iter().all(|shot| shot.1 == WEAPON_REVOLVER) || game.world.shots.is_empty());
    }

    #[test]
    fn a_scoped_sniper_shot_goes_true_along_the_look_direction() {
        let (mut game, shooter, target, mut seq, _) = duel();
        let sniper = def(ITEM_SNIPER).unwrap();
        give(&mut game, shooter, 1, ITEM_SNIPER, 1);
        // Straight ahead of the eye, not off to the side where the over-the-shoulder crosshair
        // would find a target.
        game.world.players.get_mut(&target).unwrap().character.position = [100.0, STANDING_Y, -20.0];
        press(&mut game, shooter, &mut seq, BUTTON_ALT | BUTTON_FIRE, 1, 0.0);
        // A round in flight, not an instant hit.
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP);
        assert!(game.world.rockets.values().any(|r| r.kind == PROJECTILE_BULLET));
        // 20 m at twice a tank shell's speed is a handful of ticks.
        let mut landed = false;
        for _ in 0..TICK_RATE / 4 {
            press(&mut game, shooter, &mut seq, BUTTON_ALT, 1, 0.0);
            landed |= game.world.shots.iter().any(|shot| shot.1 == WEAPON_SNIPER);
        }
        assert!(landed);
        assert_eq!(game.world.players[&target].hp, PLAYER_MAX_HP - sniper.damage);
        assert_eq!(game.world.players[&shooter].hotbar[1].2, sniper.mag - 1);
    }

    #[test]
    fn aiming_tightens_any_guns_spread() {
        let ak = ak5();
        assert_eq!(effective_spread(&ak, 0), ak.spread);
        assert_eq!(effective_spread(&ak, BUTTON_ALT | BUTTON_FIRE), ak.spread * AIM_SPREAD_SCALE);
        assert!(effective_spread(&ak, BUTTON_ALT) < effective_spread(&ak, 0));
    }

    #[test]
    fn aim_without_a_scoped_gun_changes_nothing() {
        let (mut game, shooter, target, mut seq, _) = duel();
        // The AK5 has no scope: aim held leaves the over-the-shoulder crosshair, which finds the
        // target the duel sets up to the side.
        let pitch = aim_for(&game, shooter, 0);
        press(&mut game, shooter, &mut seq, BUTTON_ALT | BUTTON_FIRE, 0, pitch);
        assert!(game.world.players[&target].hp < PLAYER_MAX_HP);
    }

    #[test]
    fn a_gun_runs_dry_reloads_and_fires_again() {
        let (mut game, shooter, target, mut seq, _) = duel();
        game.world.players.get_mut(&shooter).unwrap().hotbar[0].2 = 1;
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 1);
        assert_eq!(game.world.players[&shooter].hotbar[0].2, 0);
        let hp = game.world.players[&target].hp;
        // Dry: pulling the trigger starts a reload instead of a shot.
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, ak5().cooldown_ticks + 1);
        assert_eq!(game.world.players[&target].hp, hp);
        assert!(game.world.players[&shooter].reload_left > 0);
        hold(&mut game, shooter, &mut seq, 0.0, 0, ak5().reload_ticks + 1);
        assert_eq!(game.world.players[&shooter].hotbar[0].2, ak5().mag);
        hold(&mut game, shooter, &mut seq, 0.0, BUTTON_FIRE, 1);
        assert_eq!(game.world.players[&target].hp, hp - ak5().damage);
    }

    #[test]
    fn switching_slots_drops_a_reload() {
        let (mut game, shooter, _, mut seq, _) = duel();
        game.world.players.get_mut(&shooter).unwrap().hotbar[0].2 = 3;
        hold_slot(&mut game, shooter, &mut seq, BUTTON_RELOAD, 0, 2);
        assert!(game.world.players[&shooter].reload_left > 0);
        hold_slot(&mut game, shooter, &mut seq, 0, 3, 1);
        assert_eq!(game.world.players[&shooter].reload_left, 0);
        assert_eq!(game.world.players[&shooter].hotbar[0].2, 3, "it never finished");
    }

    #[test]
    fn a_grenade_arcs_falls_and_bursts_when_the_fuse_is_out() {
        let (mut game, shooter, target, mut seq, _) = duel();
        give(&mut game, shooter, 2, ITEM_GRENADE, 2);
        // Thrown level at a target 20 m away: it lands short of them, then the fuse takes it.
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 2, 1);
        assert_eq!(game.world.players[&shooter].hotbar[2].1, 1, "one thrown");
        assert_eq!(game.world.rockets.len(), 1);
        let grenade = *game.world.rockets.values().next().unwrap();
        assert_eq!(grenade.kind, PROJECTILE_GRENADE);
        hold_slot(&mut game, shooter, &mut seq, 0, 2, 20);
        let grenade = *game.world.rockets.values().next().expect("still in flight");
        assert!(grenade.position[1] < 3.0, "gravity pulls it down: y = {}", grenade.position[1]);
        hold_slot(&mut game, shooter, &mut seq, 0, 2, GRENADE_FUSE_TICKS);
        assert!(game.world.rockets.is_empty(), "burst");
        assert!(game.world.shots.iter().any(|s| s.1 == WEAPON_GRENADE) || game.world.players[&target].hp <= PLAYER_MAX_HP);
    }

    #[test]
    fn a_grenade_at_your_feet_hurts_and_one_beside_a_target_kills_them() {
        let (mut game, shooter, target, _, _) = duel();
        let at = game.world.players[&target].character.position;
        let id = game.world.alloc_id();
        game.world.rockets.insert(id, Rocket { kind: PROJECTILE_GRENADE, owner: shooter, source: 0, position: [at[0] + 1.0, at[1], at[2]], velocity: [0.0; 3], ttl: 1 });
        game.step();
        assert!(game.world.rockets.is_empty());
        assert!(game.world.players[&target].hp < PLAYER_MAX_HP - 50, "{}", game.world.players[&target].hp);
        assert_eq!(game.world.players[&shooter].hp, PLAYER_MAX_HP, "20 m clear of the blast");
    }

    #[test]
    fn a_bandage_heals_after_a_pause_and_is_used_up() {
        let (mut game, shooter, _, mut seq, _) = duel();
        give(&mut game, shooter, 3, ITEM_BANDAGE, 1);
        game.world.players.get_mut(&shooter).unwrap().hp = 30;
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 3, 1);
        assert_eq!(game.world.players[&shooter].hp, 30, "not yet");
        assert!(game.world.players[&shooter].channel_left > 0);
        hold_slot(&mut game, shooter, &mut seq, 0, 3, BANDAGE_TICKS + 1);
        assert_eq!(game.world.players[&shooter].hp, 30 + def(ITEM_BANDAGE).unwrap().damage);
        assert_eq!(game.world.players[&shooter].hotbar[3], ItemSlot(ITEM_NONE, 0, 0));
    }

    #[test]
    fn being_hit_spoils_a_bandage_and_full_health_does_not_start_one() {
        let (mut game, shooter, target, mut seq, _) = duel();
        give(&mut game, shooter, 3, ITEM_BANDAGE, 2);
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 3, 1);
        assert_eq!(game.world.players[&shooter].channel_left, 0, "nothing to heal");

        game.world.players.get_mut(&shooter).unwrap().hp = 50;
        hold_slot(&mut game, shooter, &mut seq, 0, 3, 5);
        hold_slot(&mut game, shooter, &mut seq, BUTTON_FIRE, 3, 1);
        assert!(game.world.players[&shooter].channel_left > 0);
        damage(&mut game.world, shooter, 5, Some(target), WEAPON_RIFLE);
        assert_eq!(game.world.players[&shooter].channel_left, 0);
        hold_slot(&mut game, shooter, &mut seq, 0, 3, BANDAGE_TICKS + 1);
        assert_eq!(game.world.players[&shooter].hp, 45);
        assert_eq!(game.world.players[&shooter].hotbar[3].1, 2, "not spent");
    }

    #[test]
    fn a_new_life_starts_empty_handed() {
        let (mut game, shooter, target, mut seq, mut seq_t) = duel();
        give(&mut game, target, 0, ITEM_REVOLVER, 1);
        damage(&mut game.world, target, PLAYER_MAX_HP, Some(shooter), WEAPON_RIFLE);
        hold(&mut game, target, &mut seq_t, 0.0, 0, RESPAWN_TICKS);
        assert!(game.world.players[&target].alive());
        assert!(game.world.players[&target].hotbar.iter().all(|slot| slot.0 == ITEM_NONE));
        let _ = &mut seq;
    }
}
