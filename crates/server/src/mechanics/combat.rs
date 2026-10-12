use super::Mechanic;
use super::npcs::{damage_rooster, first_rooster_hit};
use super::spawn;
use crate::game::{Rocket, World, starting_hotbar};
use crate::physics::Obj;
use shared::consts::*;
use shared::items::*;
use shared::protocol::{BEARING_NONE, HIT_BODY, HIT_DEALT, HIT_KILL, HIT_ROOSTER, HIT_TAKEN, HIT_VEHICLE, Hit, ItemSlot, KIND_PLANE, Kill, S_HIT, PROJECTILE_BOMB, PROJECTILE_BULLET, PROJECTILE_GRENADE, PROJECTILE_CANNON, PROJECTILE_ROCKET, PROJECTILE_SHELL, S_KILL, ShotEvent, WEAPON_GRENADE, WEAPON_RIFLE, WEAPON_ROCKET, WEAPON_ROADKILL, WEAPON_SNIPER, WEAPON_WRECK,
    DEATH_BLAST, DEATH_BODY, DEATH_HEAD, DEATH_OVERKILL, DEATH_ROADKILL, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_SHELL};
use shared::quant::{from_mrad, to_cm, to_mrad};

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

/// How hard (m/s) a bullet's killing blow throws the body, when nothing else was pushing it.
const BULLET_DEATH_PUSH: f32 = 4.0;
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

/// Where player `id` is: on foot where they stand, seated where their vehicle is.
fn position_of(world: &World, id: u32) -> Option<[f32; 3]> {
    let player = world.players.get(&id)?;
    match player.seat {
        Some(seat) => world.physics.car_pose(seat.vehicle).map(|pose| pose.position),
        None => Some(player.character.position),
    }
}

/// A `Hit` for `target`, which is about `at`.
fn hit_at(role: u8, kind: u8, amount: u8, weapon: u8, bearing: i16, at: [f32; 3], target: u32) -> Hit {
    let [x, y, z] = at.map(to_cm);
    Hit(S_HIT, role, kind, amount, weapon, bearing, x, y, z, target)
}

/// Yaw (0 faces -Z, as for a look) from `at` towards `from`, in milliradians.
fn bearing(at: [f32; 3], from: [f32; 3]) -> i16 {
    to_mrad((-(from[0] - at[0])).atan2(-(from[2] - at[2])))
}

/// Takes `amount` health off `victim`. At zero they die: the attacker (a player id, if any other
/// than the victim) is credited and a kill is queued. Does nothing to the dead. The victim and the
/// attacker are each sent a `Hit`.
pub fn damage(world: &mut World, victim: u32, amount: u8, attacker: Option<u32>, weapon: u8) {
    damage_from(world, victim, amount, attacker, weapon, None);
}

/// `damage`, for harm that came from `source` (a blast's centre) rather than from the attacker's
/// own position, which is what the victim's hit direction points at when `source` is `None`.
pub fn damage_from(world: &mut World, victim: u32, amount: u8, attacker: Option<u32>, weapon: u8, source: Option<[f32; 3]>) {
    damage_hit(world, victim, amount, attacker, weapon, source, false);
}

/// Whether a round that reaches `t` along `dir` from `origin` hits `victim` (where they stood `back`
/// ticks ago) in the head.
fn is_headshot(world: &World, victim: u32, origin: [f32; 3], dir: [f32; 3], t: f32, back: u32) -> bool {
    let Some(player) = world.players.get(&victim) else { return false };
    let at = if back == 0 { player.character.position } else { world.position_at(victim, back).unwrap_or(player.character.position) };
    origin[1] + dir[1] * t - at[1] > HEAD_ZONE_Y
}

/// `damage_from`, for a round that hit the head when `head`: it does `HEADSHOT_MULT` times the
/// damage, and the kill is staged as a headshot.
pub fn damage_hit(world: &mut World, victim: u32, amount: u8, attacker: Option<u32>, weapon: u8, source: Option<[f32; 3]>, head: bool) {
    let amount = if head { (f32::from(amount) * HEADSHOT_MULT).min(255.0) as u8 } else { amount };
    let other = attacker.filter(|&id| id != victim);
    let from = source.or_else(|| other.and_then(|id| position_of(world, id)));
    let Some(player) = world.players.get_mut(&victim) else { return };
    if !player.alive() || amount == 0 {
        return;
    }
    let before = player.hp;
    player.hp = player.hp.saturating_sub(amount);
    // A bandage needs a steady hand.
    player.channel_left = 0;
    let kind = if player.hp == 0 { HIT_KILL } else { HIT_BODY };
    let direction = from.map_or(BEARING_NONE, |from| bearing(player.character.position, from));
    let lost = before - player.hp;
    // Chest height, where a number or a flash belongs.
    let chest = [player.character.position[0], player.character.position[1] + 0.9, player.character.position[2]];
    world.hits.push((victim, hit_at(HIT_TAKEN, kind, lost, weapon, direction, chest, victim)));
    if let Some(id) = other {
        world.hits.push((id, hit_at(HIT_DEALT, kind, lost, weapon, BEARING_NONE, chest, victim)));
    }
    if player.hp > 0 {
        return;
    }
    player.respawn_in = RESPAWN_TICKS;
    player.deaths = player.deaths.saturating_add(1);
    player.fire_cooldown = 0;
    player.interrupt();
    let deaths = player.deaths;
    let (knock, died_at) = (player.character.knock, player.character.position);
    let mut killer_kills = 0;
    let mut killer_id = 0;
    if let Some(killer) = attacker.filter(|&id| id != victim).and_then(|id| world.players.get_mut(&id).map(|p| (id, p))) {
        killer.1.kills = killer.1.kills.saturating_add(1);
        killer_kills = killer.1.kills;
        killer_id = killer.0;
    }
    let style = match weapon {
        WEAPON_ROADKILL => DEATH_ROADKILL,
        WEAPON_ROCKET | WEAPON_SHELL | WEAPON_WRECK | WEAPON_GRENADE | WEAPON_AUTOCANNON | WEAPON_BOMB => DEATH_BLAST,
        _ if head => DEATH_HEAD,
        _ if lost > 0 && amount > before.saturating_mul(2) => DEATH_OVERKILL,
        _ => DEATH_BODY,
    };
    // A shove already on the body (a blast, a car) is the way it is thrown; a bullet pushes it away
    // from where it came from.
    let [kx, kz] = knock;
    let pushed = kx.hypot(kz);
    let (yaw, power) = if pushed > 0.5 {
        (to_mrad((-kx).atan2(-kz)), pushed)
    } else {
        let toward = from.map_or(0, |from| bearing(died_at, from));
        (toward.wrapping_add(3142), BULLET_DEATH_PUSH)
    };
    let power = (power * 5.0).round().min(255.0) as u8;
    world.kills.push(Kill(S_KILL, killer_id, victim, weapon, killer_kills, deaths, style, yaw, power));
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
        damage_from(world, id, amount, blast.attacker, blast.weapon, Some(blast.point));
    }
    let roosters: Vec<(u32, u8)> = world
        .roosters
        .iter()
        .filter(|(_, r)| r.alive())
        .filter_map(|(&id, r)| {
            let centre = [r.position[0], r.position[1] + ROOSTER_CAPSULE_CENTRE, r.position[2]];
            let distance = (0..3).map(|i| (centre[i] - blast.point[i]).powi(2)).sum::<f32>().sqrt();
            (distance < blast.radius).then(|| (id, (blast.damage * (1.0 - distance / blast.radius)).round().min(255.0) as u8))
        })
        .collect();
    for (id, amount) in roosters {
        damage_rooster(world, id, amount);
    }
    for (vid, falloff) in world.physics.blast_cars(blast.point, blast.radius) {
        damage_vehicle(world, vid, (blast.damage * falloff).round().min(255.0) as u8, blast.attacker);
        super::vehicles::shoot_wheels(world, vid, blast.point, blast.damage * falloff, super::vehicles::WHEEL_REACH);
    }
    for object in world.physics.objects_near(blast.point, blast.radius) {
        let (distance, scale) = match object {
            Obj::Tree(id) => {
                let Some(tree) = world.tree(id) else { continue };
                let at = [tree.x as f32 / 100.0, tree.base as f32 / 100.0 + tree.trunk_height() as f32 / 200.0, tree.z as f32 / 100.0];
                ((0..3).map(|i| (at[i] - blast.point[i]).powi(2)).sum::<f32>().sqrt(), BLAST_TREE_SCALE)
            }
            // A wall takes the blast at its nearest face, not from the middle of the house.
            Obj::Building(id) => match distance_to_building(world, id, blast.point) {
                Some(distance) => (distance, 1.0),
                None => continue,
            },
        };
        let falloff = (1.0 - distance / blast.radius).max(0.0);
        damage_object(world, object, (blast.damage * falloff * scale).round().min(u16::MAX as f32) as u16, blast.point);
    }
}

/// What a blast's centre damage becomes against a tree: they are tougher than people to
/// a rifle but not to a rocket.
const BLAST_TREE_SCALE: f32 = 4.0;

/// A bullet does this much less to a building than its damage: rifles chip walls, they don't fell them.
const BULLET_BUILDING_DIVISOR: u16 = 3;

/// Takes `amount` health off a building or tree. At zero it is destroyed: its collider goes, and
/// everyone is told, with the way it falls (a tree: away from `from`, where the damage came from;
/// a building: the same, for the way its rubble is thrown).
pub fn damage_object(world: &mut World, object: Obj, amount: u16, from: [f32; 3]) {
    if amount == 0 || world.destroyed.iter().any(|&(done, _)| done == object) {
        return;
    }
    // Full health and where it stands, in metres.
    let (full, at) = match object {
        Obj::Tree(id) => match world.tree(id) {
            Some(tree) => (tree.hp(), [tree.x as f32 / 100.0, tree.z as f32 / 100.0]),
            None => return,
        },
        Obj::Building(id) => match world.building(id) {
            Some(building) => {
                let [x, z] = building.centre();
                (building.hp(), [x as f32 / 100.0, z as f32 / 100.0])
            }
            None => return,
        },
    };
    let before = world.damage_stage(object);
    let left = world.object_hp.get(&object).copied().unwrap_or(full).saturating_sub(amount);
    if left > 0 {
        world.object_hp.insert(object, left);
        // Buildings show it: a worse stage, or a blast big enough to leave a mark.
        if let Obj::Building(id) = object {
            let stage = world.damage_stage(object).unwrap_or(0);
            let worse = before.is_some_and(|before| stage > before);
            let marked = amount >= DAMAGE_MARK_MIN;
            if worse || marked {
                world.fresh_damaged.push((id, stage, marked.then_some(from)));
            }
        }
        return;
    }
    world.object_hp.remove(&object);
    if !world.physics.destroy(object) {
        return;
    }
    // `look`'s convention: yaw 0 faces -Z.
    let yaw = to_mrad((-(at[0] - from[0])).atan2(-(at[1] - from[2])));
    world.destroyed.push((object, yaw));
    world.fresh_destroyed.push((object, yaw));
}

/// How close `point` is to building `id`'s bounding box, metres: 0 inside it.
fn distance_to_building(world: &World, id: shared::world::ObjId, point: [f32; 3]) -> Option<f32> {
    let building = world.building(id)?;
    let [min_x, min_z, max_x, max_z] = building.bounds();
    let (bottom, top) = (building.base as f32 / 100.0, (building.base + building.height) as f32 / 100.0);
    let outside = |lo: f32, hi: f32, v: f32| (lo - v).max(v - hi).max(0.0);
    let (dx, dy, dz) = (outside(min_x as f32 / 100.0, max_x as f32 / 100.0, point[0]), outside(bottom, top, point[1]), outside(min_z as f32 / 100.0, max_z as f32 / 100.0, point[2]));
    Some((dx * dx + dy * dy + dz * dz).sqrt())
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
    let before = vehicle.hp;
    vehicle.hp = vehicle.hp.saturating_sub(amount);
    let lost = before - vehicle.hp;
    let wrecked = vehicle.hp == 0;
    let seated: Vec<u32> = vehicle.seats.iter().flatten().copied().collect();
    let at = world.physics.car_pose(vid).map(|pose| pose.position);
    let from = attacker.and_then(|id| position_of(world, id));
    let direction = at.zip(from).map_or(BEARING_NONE, |(at, from)| bearing(at, from));
    let centre = at.map_or([0.0; 3], |p| [p[0], p[1] + 0.8, p[2]]);
    for &id in &seated {
        world.hits.push((id, hit_at(HIT_TAKEN, HIT_VEHICLE, lost, 0, direction, centre, vid)));
    }
    if let Some(id) = attacker.filter(|id| !seated.contains(id) && world.players.contains_key(id)) {
        world.hits.push((id, hit_at(HIT_DEALT, HIT_VEHICLE, lost, 0, BEARING_NONE, centre, vid)));
    }
    if wrecked {
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
    let rooster = first_rooster_hit(world, bullet.position, dir, hit.map_or(reach, |(_, t)| t));
    bullet.ttl = bullet.ttl.saturating_sub(1);
    if hit.is_some() || rooster.is_some() || wall.is_some() {
        world.rockets.remove(&id);
        let end = rooster.map(|(_, t)| t).or(hit.map(|(_, t)| t)).unwrap_or(reach);
        let point: [f32; 3] = std::array::from_fn(|i| bullet.position[i] + dir[i] * end);
        let damage_dealt = def(ITEM_SNIPER).map_or(0, |d| d.damage);
        let attacker = world.players.contains_key(&bullet.owner).then_some(bullet.owner);
        if let Some((target, _)) = rooster {
            damage_rooster(world, target, damage_dealt);
            if let Some(owner) = attacker {
                let at = world.roosters.get(&target).map_or([0.0; 3], |r| [r.position[0], r.position[1] + 0.5, r.position[2]]);
                world.hits.push((owner, hit_at(HIT_DEALT, HIT_ROOSTER, damage_dealt, WEAPON_SNIPER, BEARING_NONE, at, target)));
            }
        } else if let Some((victim, _)) = hit {
            let head = hit.is_some_and(|(_, t)| is_headshot(world, victim, bullet.position, dir, t, 0));
            damage_hit(world, victim, damage_dealt, attacker, WEAPON_SNIPER, None, head);
        } else if let Some((_, Some(car))) = wall {
            damage_vehicle(world, car, (damage_dealt / 4).max(1), attacker);
            super::vehicles::shoot_wheels(world, car, point, f32::from(damage_dealt), super::vehicles::BULLET_WHEEL_REACH);
        } else if wall.is_some() {
            hit_object(world, bullet.position, dir, reach, damage_dealt);
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
        // Shells and bombs are the `Gunnery` mechanic's.
        if rocket.kind == PROJECTILE_SHELL || rocket.kind == PROJECTILE_CANNON || rocket.kind == PROJECTILE_BOMB {
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

/// A round that ended in the world: damages the building or tree it struck, if it was one.
fn hit_object(world: &mut World, origin: [f32; 3], dir: [f32; 3], reach: f32, damage: u8) {
    // The ray stops at the surface; look a hair past it to be inside the collider.
    if let Some((_, Some(object))) = world.physics.cast_ray_object(origin, dir, reach + 0.05) {
        // Bullets only chip a wall: a third of what they do to a tree, and never nothing.
        let amount = match object {
            Obj::Tree(_) => damage as u16,
            Obj::Building(_) => (damage as u16 / BULLET_BUILDING_DIVISOR).max(1),
        };
        damage_object(world, object, amount, origin);
    }
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
    // A nuisance to a vehicle, not a way to kill one.
    let chip = (item.damage / 4).max(1);
    let round = Round { damage: item.damage, vehicle: chip, plane: chip, weapon: item.weapon, range: item.range };
    let (end, _) = hitscan(world, id, origin, dir, &round, 0, back);
    let point: [i32; 3] = std::array::from_fn(|i| to_cm(origin[i] + dir[i] * end));
    world.shots.push(ShotEvent(id, item.weapon, point[0], point[1], point[2]));
}

/// What one hitscan round does: to a stickman or a rooster, to a vehicle, and to a plane, which is all
/// wood and canvas.
pub struct Round {
    pub damage: u8,
    pub vehicle: u8,
    pub plane: u8,
    pub weapon: u8,
    pub range: f32,
}

/// Fires one round for `shooter` from `origin` along the unit `dir`, through vehicle `ignore` (the one
/// it is fired from, or 0), at stickmen where they stood `back` ticks ago. It hurts the first thing it
/// meets: a stickman, a rooster, a vehicle, or a building or a tree. Returns how far it went, and
/// whether it met anything at all before the end of its range.
pub fn hitscan(world: &mut World, shooter: u32, origin: [f32; 3], dir: [f32; 3], round: &Round, ignore: u32, back: u32) -> (f32, bool) {
    let wall = world.physics.cast_ray_hit(origin, dir, round.range, ignore);
    let reach = wall.map_or(round.range, |(t, _)| t);
    let hit = first_player_hit(world, origin, dir, reach, Some(shooter), back);
    // A rooster nearer than the player (or with no player behind it) takes the shot.
    let rooster = first_rooster_hit(world, origin, dir, hit.map_or(reach, |(_, t)| t));
    let end = rooster.map(|(_, t)| t).or(hit.map(|(_, t)| t)).unwrap_or(reach);
    if let Some((target, _)) = rooster {
        damage_rooster(world, target, round.damage);
        let at = world.roosters.get(&target).map_or([0.0; 3], |r| [r.position[0], r.position[1] + 0.5, r.position[2]]);
        world.hits.push((shooter, hit_at(HIT_DEALT, HIT_ROOSTER, round.damage, round.weapon, BEARING_NONE, at, target)));
    } else if let Some((victim, _)) = hit {
        let head = hit.is_some_and(|(_, t)| is_headshot(world, victim, origin, dir, t, back));
        damage_hit(world, victim, round.damage, Some(shooter), round.weapon, None, head);
    } else if let Some((_, Some(car))) = wall {
        let plane = world.vehicles.get(&car).is_some_and(|vehicle| vehicle.kind == KIND_PLANE);
        damage_vehicle(world, car, if plane { round.plane } else { round.vehicle }, Some(shooter));
        let point = std::array::from_fn(|i| origin[i] + dir[i] * reach);
        super::vehicles::shoot_wheels(world, car, point, f32::from(round.damage), super::vehicles::BULLET_WHEEL_REACH);
    } else if wall.is_some() {
        hit_object(world, origin, dir, reach, round.damage);
    }
    (end, rooster.is_some() || hit.is_some() || wall.is_some())
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
                    // A fresh life starts with the standard loadout.
                    player.hotbar = starting_hotbar();
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
    fn a_hit_is_reported_to_the_shooter_and_the_victim_with_a_bearing() {
        let (mut game, shooter, target, _, _) = duel();
        game.world.players.get_mut(&shooter).unwrap().character.position = [0.0, STANDING_Y, 0.0];
        game.world.players.get_mut(&target).unwrap().character.position = [0.0, STANDING_Y, -20.0];
        game.world.hits.clear();
        damage(&mut game.world, target, 30, Some(shooter), WEAPON_RIFLE);
        let hits = &game.world.hits;
        assert_eq!(hits.len(), 2);
        // The shooter is straight ahead of nobody: from the victim at -20 the shooter is at +z.
        assert_eq!((hits[0].0, hits[0].1.1, hits[0].1.2, hits[0].1.3, hits[0].1.5, hits[0].1.9), (target, HIT_TAKEN, HIT_BODY, 30, to_mrad(std::f32::consts::PI), target));
        assert_eq!((hits[1].0, hits[1].1.1, hits[1].1.3, hits[1].1.5, hits[1].1.9), (shooter, HIT_DEALT, 30, BEARING_NONE, target));
        // The number belongs at the target's chest.
        assert_eq!(hits[1].1.8, to_cm(-20.0));
        // The finishing blow is a kill, and amounts are what was actually lost.
        game.world.hits.clear();
        damage(&mut game.world, target, 255, Some(shooter), WEAPON_RIFLE);
        let last = &game.world.hits[1].1;
        assert_eq!((last.1, last.2, last.3), (HIT_DEALT, HIT_KILL, PLAYER_MAX_HP - 30));
    }

    #[test]
    fn hurting_yourself_reports_no_dealt_hit() {
        let (mut game, shooter, _, _, _) = duel();
        game.world.hits.clear();
        damage(&mut game.world, shooter, 5, Some(shooter), WEAPON_RIFLE);
        assert_eq!(game.world.hits.len(), 1);
        assert_eq!(game.world.hits[0].1.1, HIT_TAKEN);
        assert_eq!(game.world.hits[0].1.5, BEARING_NONE);
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
        // Straight ahead of the eye is the head: double damage, and a headshot kill.
        let expected = PLAYER_MAX_HP.saturating_sub(sniper.damage * 2);
        assert_eq!(game.world.players[&target].hp, expected);
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
    fn a_new_life_starts_with_the_standard_loadout() {
        let (mut game, shooter, target, mut seq, mut seq_t) = duel();
        give(&mut game, target, 4, ITEM_BANDAGE, 2);
        damage(&mut game.world, target, PLAYER_MAX_HP, Some(shooter), WEAPON_RIFLE);
        hold(&mut game, target, &mut seq_t, 0.0, 0, RESPAWN_TICKS);
        assert!(game.world.players[&target].alive());
        let hotbar = game.world.players[&target].hotbar;
        assert_eq!(hotbar, starting_hotbar());
        let items: Vec<u8> = hotbar.iter().map(|slot| slot.0).collect();
        assert_eq!(items, [ITEM_REVOLVER, ITEM_AK5, ITEM_SNIPER, ITEM_GRENADE, ITEM_NONE]);
        assert_eq!(hotbar[1].2, def(ITEM_AK5).unwrap().mag, "magazines are full");
        let _ = &mut seq;
    }
    /// A game on a world with one solid pine at (10, 0), identified as tile (2, 3) index 0.
    fn game_with_tree() -> (Game, Obj) {
        use shared::world::{ObjId, TREE_PINE, Tree};
        let tree = Tree { x: 1000, z: 0, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true };
        let id = ObjId { col: 2, row: 3, index: 0 };
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], trees: vec![tree], tree_ids: vec![id], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        game.world.pickups.clear();
        (game, Obj::Tree(id))
    }

    #[test]
    fn rifle_rounds_wear_a_tree_down_until_it_falls_away_from_the_shooter() {
        let (mut game, tree) = game_with_tree();
        let hp = game.world.tree(match tree { Obj::Tree(id) => id, _ => unreachable!() }).unwrap().hp();
        let rounds = hp.div_ceil(12);
        for _ in 0..rounds - 1 {
            damage_object(&mut game.world, tree, 12, [0.0, 1.5, 0.0]);
        }
        assert!(game.world.destroyed.is_empty(), "fell after {} rounds, wanted {rounds}", rounds - 1);
        damage_object(&mut game.world, tree, 12, [0.0, 1.5, 0.0]);
        assert_eq!(game.world.destroyed.len(), 1);
        // The shooter is at x = 0 and the tree at x = +10, so it falls towards +X: yaw -90°.
        let yaw = from_mrad(game.world.destroyed[0].1);
        assert!((yaw + std::f32::consts::FRAC_PI_2).abs() < 0.01, "{yaw}");
        // Nothing happens twice.
        damage_object(&mut game.world, tree, 255, [0.0, 1.5, 0.0]);
        assert_eq!(game.world.destroyed.len(), 1);
    }

    #[test]
    fn a_car_at_speed_fells_the_tree_it_drives_into() {
        use shared::world::{ObjId, TREE_PINE, Tree};
        // The car's heading is not worth working out: trees stand 30 m off on every side.
        let trees: Vec<Tree> = [[3000, 0], [-3000, 0], [0, 3000], [0, -3000]]
            .into_iter()
            .map(|[x, z]| Tree { x, z, base: 0, height: 1600, crown: 300, kind: TREE_PINE, solid: true })
            .collect();
        let tree_ids = (0..4).map(|index| ObjId { col: 2, row: 3, index }).collect();
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], trees, tree_ids, ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        game.world.pickups.clear();
        let (_, driver) = driver_in_the_car(&mut game);
        let mut seq = 0;
        for _ in 0..TICK_RATE * 6 {
            seq += 1;
            game.handle(Command::Input(driver, 0, vec![InputFrame(seq, 0, 127, 0, 0, 0, 0, 0)]));
            game.step();
        }
        assert!(!game.world.destroyed.is_empty(), "the car drove 6 s and felled nothing");
    }

    #[test]
    fn a_blast_fells_a_tree_in_reach_and_everyone_hears_of_it() {
        let (mut game, _) = game_with_tree();
        let (id, mut inbox) = join(&mut game);
        while inbox.try_recv().is_ok() {}
        let point = [10.0, 1.0, 1.5];
        assert_eq!(game.world.physics.cast_ray_object([10.0, 1.0, 5.0], [0.0, 0.0, -1.0], 10.0).map(|(_, o)| o.is_some()), Some(true));
        blast(&mut game.world, Blast { point, radius: 5.0, damage: 100.0, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: Some(id), weapon: WEAPON_ROCKET });
        game.step();
        assert_eq!(game.world.destroyed.len(), 1);
        let mut told = false;
        while let Ok(Outgoing::Reliable(bytes)) = inbox.try_recv() {
            if shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DESTROYED) {
                let message: shared::protocol::Destroyed = decode(&bytes).unwrap();
                assert_eq!(message.1.len(), 1);
                assert_eq!((message.1[0].0, message.1[0].1, message.1[0].2, message.1[0].3), (shared::protocol::OBJ_TREE, 2, 3, 0));
                told = true;
            }
        }
        assert!(told, "no Destroyed message");
        // Its trunk no longer stops a ray, and a player who joins now is told it is gone.
        assert!(game.world.physics.cast_ray_object([10.0, 1.0, 5.0], [0.0, 0.0, -1.0], 10.0).is_none());
        let (_, mut late) = join(&mut game);
        let mut late_told = false;
        while let Ok(Outgoing::Reliable(bytes)) = late.try_recv() {
            late_told |= shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DESTROYED);
        }
        assert!(late_told, "a late joiner was not told");
    }

    #[test]
    fn a_blast_out_of_reach_leaves_the_tree_standing() {
        let (mut game, _) = game_with_tree();
        blast(&mut game.world, Blast { point: [10.0, 1.0, 30.0], radius: 5.0, damage: 100.0, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_ROCKET });
        game.step();
        assert!(game.world.destroyed.is_empty());
    }
    /// A 10 x 10 m, 6 m tall house with its middle at (20, 0), as a closed box mesh, in tile (1, 1).
    fn game_with_house() -> (Game, Obj) {
        use shared::world::{Building, CollisionMesh, ObjId};
        let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
        let vertices = vec![
            v(15.0, 0.0, -5.0), v(25.0, 0.0, -5.0), v(25.0, 0.0, 5.0), v(15.0, 0.0, 5.0),
            v(15.0, 6.0, -5.0), v(25.0, 6.0, -5.0), v(25.0, 6.0, 5.0), v(15.0, 6.0, 5.0),
        ];
        let triangles = vec![
            [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
            [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
        ];
        let building = Building {
            height: 600,
            footprint: vec![[1500, -500], [2500, -500], [2500, 500], [1500, 500]],
            base: 0,
            first: 0,
            walls: 8,
            roof: 2,
            ..Default::default()
        };
        let id = ObjId { col: 1, row: 1, index: 0 };
        let map = WorldFile {
            version: WORLD_VERSION,
            half_extent: 20_000,
            spawns: vec![[0, 0]],
            buildings: vec![building],
            building_ids: vec![id],
            building_triangles: vec![[0, 12]],
            collision: CollisionMesh { vertices, triangles },
            ..Default::default()
        };
        let mut game = Game::new(map, 0).unwrap();
        game.world.pickups.clear();
        (game, Obj::Building(id))
    }

    fn rocket_at(game: &mut Game, point: [f32; 3]) {
        blast(&mut game.world, Blast { point, radius: ROCKET_BLAST_RADIUS, damage: ROCKET_DAMAGE, knock: 0.0, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_ROCKET });
    }

    #[test]
    fn a_house_survives_a_rifle_magazine_but_not_two_rockets() {
        let (mut game, house) = game_with_house();
        let Obj::Building(id) = house else { unreachable!() };
        let hp = game.world.building(id).unwrap().hp();
        assert_eq!(hp, 150, "a 10 x 10 x 6 m house");
        // A magazine of 10 AK5 rounds, at a third of 12 each: 40 of 150.
        for _ in 0..10 {
            damage_object(&mut game.world, house, (12u16 / BULLET_BUILDING_DIVISOR).max(1), [0.0, 1.5, 0.0]);
        }
        assert!(game.world.destroyed.is_empty(), "a magazine brought it down");
        // A rocket 1 m from the wall does 88 of the 110 left: one is not enough, two are.
        rocket_at(&mut game, [14.0, 1.0, 0.0]);
        assert!(game.world.destroyed.is_empty(), "one rocket after a magazine was enough");
        rocket_at(&mut game, [14.0, 1.0, 0.0]);
        assert_eq!(game.world.destroyed.len(), 1);
        // The rubble flies away from the blast, which came from -X: towards +X, yaw -90 degrees.
        let yaw = from_mrad(game.world.destroyed[0].1);
        assert!((yaw + std::f32::consts::FRAC_PI_2).abs() < 0.2, "{yaw}");
    }

    #[test]
    fn a_rocket_on_the_wall_hurts_but_one_across_the_street_does_not() {
        let (mut game, house) = game_with_house();
        rocket_at(&mut game, [30.0, 1.0, 0.0]);
        game.step();
        assert!(game.world.object_hp.is_empty() && game.world.destroyed.is_empty(), "5 m from the wall is out of reach: {:?}", game.world.object_hp);
        rocket_at(&mut game, [27.0, 1.0, 0.0]);
        assert!(game.world.object_hp.get(&house).is_some_and(|&hp| hp < 150));
    }

    #[test]
    fn a_collapsed_house_is_open_air_and_a_joiner_is_told() {
        let (mut game, house) = game_with_house();
        let (_, mut told) = join(&mut game);
        while told.try_recv().is_ok() {}
        assert!(game.world.physics.cast_ray_object([12.0, 1.0, 0.0], [1.0, 0.0, 0.0], 40.0).is_some(), "the house is there to begin with");
        damage_object(&mut game.world, house, 150, [0.0, 1.5, 0.0]);
        game.step();
        assert!(game.world.physics.cast_ray_object([12.0, 1.0, 0.0], [1.0, 0.0, 0.0], 40.0).is_none(), "the walls still stop a ray");
        let mut kinds = Vec::new();
        while let Ok(Outgoing::Reliable(bytes)) = told.try_recv() {
            if shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DESTROYED) {
                kinds.extend(decode::<shared::protocol::Destroyed>(&bytes).unwrap().1.into_iter().map(|e| (e.0, e.1, e.2, e.3)));
            }
        }
        assert_eq!(kinds, vec![(shared::protocol::OBJ_BUILDING, 1, 1, 0)]);
        let (_, mut late) = join(&mut game);
        let mut seen = false;
        while let Ok(Outgoing::Reliable(bytes)) = late.try_recv() {
            seen |= shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DESTROYED);
        }
        assert!(seen, "a late joiner never heard the house fell");
    }

    /// What `Damaged` entries (stage, marked, x, y, z in cm) `inbox` has been sent.
    fn damaged_told(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<(u8, u8, i32, i32, i32)> {
        let mut out = Vec::new();
        while let Ok(Outgoing::Reliable(bytes)) = inbox.try_recv() {
            if shared::protocol::peek_tag(&bytes) == Some(shared::protocol::S_DAMAGED) {
                out.extend(decode::<shared::protocol::Damaged>(&bytes).unwrap().1.into_iter().map(|e| (e.3, e.4, e.5, e.6, e.7)));
            }
        }
        out
    }

    #[test]
    fn a_shell_leaves_a_mark_and_each_stage_is_announced_once() {
        let (mut game, house) = game_with_house();
        let (_, mut inbox) = join(&mut game);
        while inbox.try_recv().is_ok() {}
        // 150 HP: stages begin at 99, 49.5 and 22.5 left. A 30-damage blast leaves a mark but no stage.
        damage_object(&mut game.world, house, 30, [14.0, 1.0, 0.0]);
        game.step();
        assert_eq!(damaged_told(&mut inbox), vec![(0, 1, 1400, 100, 0)]);
        // 30 more: 90 left, stage 1; a mark too.
        damage_object(&mut game.world, house, 30, [14.0, 1.0, 0.0]);
        game.step();
        assert_eq!(damaged_told(&mut inbox), vec![(1, 1, 1400, 100, 0)]);
        // Rifle chips of 4 leave no mark, and say nothing until they cross into stage 2 (46 left).
        for _ in 0..11 {
            damage_object(&mut game.world, house, 4, [0.0, 1.5, 0.0]);
        }
        game.step();
        assert_eq!(damaged_told(&mut inbox), vec![(2, 0, 0, 0, 0)]);
    }

    #[test]
    fn a_joiner_is_told_how_hurt_a_standing_house_is() {
        let (mut game, house) = game_with_house();
        damage_object(&mut game.world, house, 140, [14.0, 1.0, 0.0]);
        let (_, mut late) = join(&mut game);
        assert_eq!(damaged_told(&mut late), vec![(3, 0, 0, 0, 0)]);
    }

    #[test]
    fn a_collapse_announces_no_damage() {
        let (mut game, house) = game_with_house();
        game.world.fresh_damaged.clear();
        damage_object(&mut game.world, house, 150, [14.0, 1.0, 0.0]);
        assert!(game.world.fresh_damaged.is_empty());
    }
}
