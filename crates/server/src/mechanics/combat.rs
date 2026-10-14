use super::Mechanic;
use super::armor::Outcome;
use super::npcs::{damage_rooster, first_rooster_hit};
use super::spawn;
use crate::game::{Rocket, World, starting_hotbar};
use crate::physics::Obj;
use shared::consts::*;
use shared::items::*;
use shared::protocol::{BEARING_NONE, HIT_BODY, HIT_DEALT, HIT_KILL, HIT_ROOSTER, HIT_TAKEN, HIT_VEHICLE, Hit, ItemSlot, KIND_PLANE, Kill, S_HIT, PROJECTILE_BOMB, PROJECTILE_BULLET, PROJECTILE_GRENADE, PROJECTILE_CANNON, PROJECTILE_ROCKET, PROJECTILE_SHELL, S_KILL, ShotEvent, BulletSpawn, WEAPON_BULLET, WEAPON_GRENADE, WEAPON_RIFLE, WEAPON_ROCKET, WEAPON_PELLET, WEAPON_ROADKILL, WEAPON_WRECK,
    DEATH_BLAST, DEATH_BODY, DEATH_HEAD, DEATH_OVERKILL, DEATH_ROADKILL, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_SHELL};
use shared::quant::{from_mrad, to_cm, to_cm_i16, to_mrad};

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
    Hit { tag: S_HIT, role, kind, amount, weapon, bearing, x, y, z, target }
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
    world.kills.push(Kill { tag: S_KILL, killer: killer_id, victim, weapon, killer_kills, victim_deaths: deaths, style, yaw, power });
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

/// A vehicle the blast's shell struck directly: it takes `DIRECT_SHARE` of the damage times `scale`
/// (what its armor made of the angle) instead of the damage its distance from the centre would give, and
/// the turret takes it too if that was what was hit.
#[derive(Clone, Copy)]
pub struct Direct {
    pub vehicle: u32,
    pub scale: f32,
    pub turret: bool,
}

/// Throws and hurts the standing players within `blast.radius`, and shoves and hurts the vehicles.
pub fn blast(world: &mut World, blast: Blast) {
    blast_at(world, blast, None);
}

/// [`blast`], for a shell that struck vehicle `direct.vehicle` itself.
pub fn blast_at(world: &mut World, blast: Blast, direct: Option<Direct>) {
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
    let mut reached = world.physics.blast_cars(blast.point, blast.radius);
    // A shell that struck a vehicle's edge may burst further from its centre than the blast reaches.
    if let Some(direct) = direct.filter(|direct| !reached.iter().any(|&(vid, _)| vid == direct.vehicle)) {
        reached.push((direct.vehicle, 0.0));
    }
    for (vid, falloff) in reached {
        let direct = direct.filter(|direct| direct.vehicle == vid);
        let dealt = match direct {
            Some(direct) => blast.damage * super::armor::DIRECT_SHARE * direct.scale,
            None => blast.damage * falloff,
        };
        let amount = dealt.round().min(255.0) as u8;
        damage_vehicle(world, vid, amount, blast.attacker);
        if direct.is_some_and(|direct| direct.turret) {
            super::armor::damage_turret(world, vid, amount, blast.attacker);
        }
        super::vehicles::shoot_wheels(world, vid, blast.point, dealt, super::vehicles::WHEEL_REACH);
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
    world.shots.push(ShotEvent { shooter: attacker.unwrap_or(0), weapon: WEAPON_WRECK, x, y, z });
    // Thrown up a little, so it is plainly not a parked car any more.
    world.physics.kick_car(vid, [0.0, 9.0, 0.0]);
    // ...and tumbling: a roll or a somersault, either way round, picked by the vehicle's id and
    // where it stood so that two wrecks do not turn alike.
    let seed = (vid as f32 * 12.9898 + pose.position[0] * 78.233 + pose.position[2] * 37.719).sin() * 43758.547;
    let pick = seed - seed.floor();
    let rate = WRECK_SPIN * (0.7 + 0.6 * ((pick * 7.0) - (pick * 7.0).floor()));
    let sign = if pick < 0.5 { -1.0 } else { 1.0 };
    let spin = if ((pick * 4.0) as u32).is_multiple_of(2) { [0.0, 0.0, sign * rate] } else { [sign * rate, 0.0, 0.0] };
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
    world.shots.push(ShotEvent { shooter: owner, weapon: WEAPON_ROCKET, x, y, z });
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
    let dir = look(from_mrad(input.yaw), pitch);
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
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
            item: 0,
            back: 0,
            flown: 0.0,
        },
    );
}

/// Flies one bullet a tick: a little gravity, then along its velocity until it meets a wall, a vehicle
/// or a stickman. A stickman takes the gun's damage, less the longer it flew; a vehicle a quarter of it.
/// Stickmen are met where they were when the shooter fired (`back` ticks ago), so a shot that was on
/// target on the shooter's screen is on target here. Clients fly it themselves (`BulletSpawn`); the
/// shot event at its end says where it stopped.
fn step_bullet(world: &mut World, id: u32, dt: f32) {
    let Some(mut bullet) = world.rockets.get(&id).copied() else { return };
    let item = def(bullet.item).copied();
    bullet.velocity[1] -= GRAVITY * BULLET_GRAVITY_SCALE * dt;
    let speed = bullet.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
    let dir = bullet.velocity.map(|v| v / speed);
    let step = speed * dt;
    let wall = world.physics.cast_ray_hit(bullet.position, dir, step, 0);
    let reach = wall.map_or(step, |(t, _)| t);
    let hit = first_player_hit(world, bullet.position, dir, reach, Some(bullet.owner), bullet.back);
    let rooster = first_rooster_hit(world, bullet.position, dir, hit.map_or(reach, |(_, t)| t));
    bullet.ttl = bullet.ttl.saturating_sub(1);
    if hit.is_some() || rooster.is_some() || wall.is_some() {
        world.rockets.remove(&id);
        let end = rooster.map(|(_, t)| t).or(hit.map(|(_, t)| t)).unwrap_or(reach);
        let point: [f32; 3] = std::array::from_fn(|i| bullet.position[i] + dir[i] * end);
        let (base, falloff, weapon) = item.map_or((0, Falloff::NONE, WEAPON_RIFLE), |d| (d.damage, d.falloff, d.weapon));
        let damage_dealt = falloff.apply(base, bullet.flown + end);
        let attacker = world.players.contains_key(&bullet.owner).then_some(bullet.owner);
        if let Some((target, _)) = rooster {
            damage_rooster(world, target, damage_dealt);
            if let Some(owner) = attacker {
                let at = world.roosters.get(&target).map_or([0.0; 3], |r| [r.position[0], r.position[1] + 0.5, r.position[2]]);
                world.hits.push((owner, hit_at(HIT_DEALT, HIT_ROOSTER, damage_dealt, weapon, BEARING_NONE, at, target)));
            }
        } else if let Some((victim, _)) = hit {
            let head = hit.is_some_and(|(_, t)| is_headshot(world, victim, bullet.position, dir, t, bullet.back));
            damage_hit(world, victim, damage_dealt, attacker, weapon, None, head);
        } else if let Some((_, Some(car))) = wall {
            match super::armor::armor_of(world, car, bullet.position, dir, reach) {
                Some(Outcome::Ricochet { .. }) => super::armor::announce_ricochet(world, car, point),
                struck => {
                    let scale = if let Some(Outcome::Hit { scale, .. }) = struck { scale } else { 1.0 };
                    let chip = (f32::from((base / 4).max(1)) * scale).round().max(1.0) as u8;
                    damage_vehicle(world, car, chip, attacker);
                    super::vehicles::shoot_wheels(world, car, point, f32::from(base), super::vehicles::BULLET_WHEEL_REACH);
                }
            }
        } else if wall.is_some() {
            hit_object(world, bullet.position, dir, reach, base);
        }
        let [x, y, z] = point.map(to_cm);
        world.shots.push(ShotEvent { shooter: id, weapon: WEAPON_BULLET, x, y, z });
        return;
    }
    bullet.flown += step;
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
        // A rocket's motor keeps pushing it along its heading, up to its top speed.
        let speed = rocket.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        let dir = rocket.velocity.map(|v| v / speed);
        let speed = (speed + ROCKET_ACCEL * dt).min(ROCKET_MAX_SPEED.max(speed));
        if let Some(r) = world.rockets.get_mut(&id) {
            r.velocity = dir.map(|v| v * speed);
        }
        let step = speed * dt;
        let wall = world.physics.cast_ray(rocket.position, dir, step, rocket.source);
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
#[allow(clippy::too_many_arguments)]
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
fn effective_spread(item: &ItemDef, buttons: u16) -> f32 {
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
    let yaw = from_mrad(input.yaw);
    let look_dir = look(yaw, pitch);
    let origin = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
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
        let velocity = dir.map(|v| v * BULLET_SPEED);
        let gun = gun_for_weapon(item.weapon);
        world.rockets.insert(
            bullet_id,
            Rocket {
                kind: PROJECTILE_BULLET,
                owner: id,
                source: 0,
                position: origin,
                velocity,
                ttl: (item.range / BULLET_SPEED * TICK_RATE as f32).ceil() as u32,
                item: gun,
                back,
                flown: 0.0,
            },
        );
        // Clients fly it themselves from this: where, which way, how fast, and when.
        let [x, y, z] = origin.map(to_cm);
        let [vx, vy, vz] = velocity.map(to_cm_i16);
        world.bullets.push(BulletSpawn { id: bullet_id, shooter: id, item: gun, x, y, z, vx, vy, vz, tick: world.tick });
        return;
    }
    // A nuisance to a vehicle, not a way to kill one.
    let chip = (item.damage / 4).max(1);
    let round = Round { damage: item.damage, vehicle: chip, plane: chip, weapon: item.weapon, range: item.range, falloff: item.falloff };
    // Every pellet is its own ray through the same rewound world. The first goes along the aim as it
    // is (scattered once, above), the rest are scattered around it. Each is announced where it ended,
    // so a client can show where a spread landed; only the first carries the gun's own weapon id.
    let spread = effective_spread(item, input.buttons());
    for pellet in 0..item.pellets {
        let ray = if pellet == 0 { dir } else { scatter(world, dir, spread) };
        let (end, _) = hitscan(world, id, origin, ray, &round, 0, back);
        let point: [i32; 3] = std::array::from_fn(|i| to_cm(origin[i] + ray[i] * end));
        let weapon = if pellet == 0 { item.weapon } else { WEAPON_PELLET };
        world.shots.push(ShotEvent { shooter: id, weapon, x: point[0], y: point[1], z: point[2] });
    }
}

/// What one hitscan round does: to a stickman or a rooster, to a vehicle, and to a plane, which is all
/// wood and canvas.
pub struct Round {
    pub damage: u8,
    pub vehicle: u8,
    pub plane: u8,
    pub weapon: u8,
    pub range: f32,
    /// How the damage of a hit on a stickman or a rooster shrinks with how far the round flew.
    pub falloff: Falloff,
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
        let dealt = round.falloff.apply(round.damage, end);
        damage_rooster(world, target, dealt);
        let at = world.roosters.get(&target).map_or([0.0; 3], |r| [r.position[0], r.position[1] + 0.5, r.position[2]]);
        world.hits.push((shooter, hit_at(HIT_DEALT, HIT_ROOSTER, dealt, round.weapon, BEARING_NONE, at, target)));
    } else if let Some((victim, _)) = hit {
        let head = hit.is_some_and(|(_, t)| is_headshot(world, victim, origin, dir, t, back));
        damage_hit(world, victim, round.falloff.apply(round.damage, end), Some(shooter), round.weapon, None, head);
    } else if let Some((_, Some(car))) = wall {
        let plane = world.vehicles.get(&car).is_some_and(|vehicle| vehicle.kind == KIND_PLANE);
        let point = std::array::from_fn(|i| origin[i] + dir[i] * reach);
        match super::armor::armor_of(world, car, origin, dir, reach) {
            Some(Outcome::Ricochet { .. }) => super::armor::announce_ricochet(world, car, point),
            struck => {
                let scale = if let Some(Outcome::Hit { scale, .. }) = struck { scale } else { 1.0 };
                let base = if plane { round.plane } else { round.vehicle };
                damage_vehicle(world, car, (f32::from(base) * scale).round().max(1.0) as u8, Some(shooter));
                super::vehicles::shoot_wheels(world, car, point, f32::from(round.damage), super::vehicles::BULLET_WHEEL_REACH);
            }
        }
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
    let dir = look(from_mrad(input.yaw), pitch);
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
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
            item: 0,
            back: 0,
            flown: 0.0,
        },
    );
}

fn burst_grenade(world: &mut World, grenade: &Rocket) {
    let [x, y, z] = grenade.position.map(to_cm);
    world.shots.push(ShotEvent { shooter: grenade.owner, weapon: WEAPON_GRENADE, x, y, z });
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
    slot.count = slot.count.saturating_sub(1);
    if slot.count == 0 {
        *slot = ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 };
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
    let Some(item) = def(held.item).copied() else {
        player.interrupt();
        return;
    };
    match item.kind {
        ItemKind::Gun => {
            if player.reload_left > 0 {
                player.reload_left -= 1;
                if player.reload_left == 0 {
                    player.hotbar[slot].ammo = item.mag;
                }
                return;
            }
            let empty = held.ammo == 0;
            let wants_reload = buttons & BUTTON_RELOAD != 0 || (empty && (pressed));
            if wants_reload && held.ammo < item.mag {
                player.reload_left = item.reload_ticks;
                return;
            }
            let trigger = if item.auto { pressed } else { fresh };
            if trigger && player.fire_cooldown == 0 && !empty {
                player.fire_cooldown = item.cooldown_ticks;
                player.hotbar[slot].ammo -= 1;
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
mod tests;
