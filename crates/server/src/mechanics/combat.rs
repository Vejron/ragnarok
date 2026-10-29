use super::Mechanic;
use super::armor::Outcome;
use super::bots::{HEAR_BLAST, HEAR_SHOT, make_noise};
use super::life::{Harm, blasted, body, damage_entity, first_hurtbox_hit};
use super::spawn;
use super::projectiles::{self, Flying, Stage};
use crate::entities::{Health, HurtBy, Respawn, With};
use crate::events::Hooks;
use crate::game::{Bot, Damaged, Died, Drone, Player, Seats, TEAM_NONE, Vehicle, World, Wreck, starting_hotbar};
use sim::Obj;
use shared::consts::*;
use shared::armor::Pen;
use shared::items::*;
use shared::vehicles::{Exposure, crew_gun, vehicle_def};
use shared::protocol::{BulletSpawn, ItemSlot, LIMB_ARMS, PROJECTILE_BULLET, PROJECTILE_FLARE, PROJECTILE_GRENADE, PROJECTILE_ROCKET, ShotEvent, WEAPON_FLARE, WEAPON_PELLET, WEAPON_RIFLE, WEAPON_WRECK};
use shared::quant::{from_mrad, to_cm, to_cm_i16, to_mrad};

/// Health, death, the items in the hotbar and what blasts do to people and vehicles.
///
/// Players live by `Health`, hurt through `life::damage_entity` as everything is. A dead player lies
/// where they fell, can be neither hit nor hurt, and comes back at a spawn point when their `Respawn`
/// has counted down, with full health and a fresh loadout.
///
/// What `BUTTON_FIRE` does depends on the item in the selected hotbar slot: a gun is a hitscan
/// from the eye along the look direction, a grenade is thrown, a bandage is applied. Seated
/// players use nothing (a gunner's trigger belongs to the gun) except a car's passengers, who fire
/// their own guns from their seat.
pub struct Combat;

/// Hop given to a soldier at the centre of a blast; it fades with distance.
const BLAST_HOP: f32 = 7.0;
const BLAST_HOP_MIN: f32 = 3.0;
/// A blast throws soldiers out to this many times the radius it hurts them in, and the push fades
/// gently, as `1 - (distance / reach)²`: well outside where it hurts it still knocks them down.
const BLAST_PUSH_REACH: f32 = 2.0;

/// How hard (m/s) a blast with `knock` at its centre and pushing out to `reach` throws a soldier
/// `distance` from it, at most `KNOCK_MAX_SPEED`; and the share of its full force that is.
pub fn blast_push(knock: f32, reach: f32, distance: f32) -> (f32, f32) {
    let falloff = (1.0 - (distance / reach).powi(2)).max(0.0);
    ((knock * falloff).min(KNOCK_MAX_SPEED), falloff)
}

/// Pitch beyond this is not a thing a client can send; clamp so a forged frame can't aim at the sky's far side.
pub(super) const MAX_PITCH: f32 = 1.5;

/// Unit look direction for a soldier facing `yaw` and looking `pitch` (+ is up). Yaw 0 faces -Z.
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

/// Where player `id` is: on foot (or flying a drone) where they stand, seated where their vehicle is.
pub fn position_of(world: &World, id: u32) -> Option<[f32; 3]> {
    let player = world.entities.get::<Player>(id)?;
    match player.seat {
        Some(seat) if !seat.remote => world.physics.car_pose(seat.vehicle).map(|pose| pose.position),
        _ => Some(player.character.position),
    }
}

/// Whether players `a` and `b` are on the same team, which spares each other. `TEAM_NONE` is no team.
pub fn same_team(world: &World, a: u32, b: u32) -> bool {
    let team = |id| world.entities.get::<Player>(id).map_or(TEAM_NONE, |p: &Player| p.team);
    team(a) != TEAM_NONE && team(a) == team(b)
}

/// Yaw (0 faces -Z, as for a look) from `at` towards `from`, in milliradians.
pub fn bearing(at: [f32; 3], from: [f32; 3]) -> i16 {
    to_mrad((-(from[0] - at[0])).atan2(-(from[2] - at[2])))
}

/// Whether a round that reaches `t` along `dir` from `origin` hits `victim` (where it stood `back`
/// ticks ago) in the head.
pub(super) fn is_headshot(world: &World, victim: u32, origin: [f32; 3], dir: [f32; 3], t: f32, back: u32) -> bool {
    body(world, victim, back).and_then(|body| body.head_y).is_some_and(|head_y| origin[1] + dir[1] * t > head_y)
}

/// What a blast does where it lands.
pub struct Blast {
    pub point: [f32; 3],
    pub radius: f32,
    /// Damage at the centre, falling off linearly to zero at `radius`.
    pub damage: f32,
    /// Knockback speed at the centre, m/s.
    pub knock: f32,
    /// Where it came from. A soldier right on the point is thrown along this.
    pub dir: [f32; 3],
    pub attacker: Option<u32>,
    pub weapon: u8,
}

/// A vehicle the blast's shell struck directly: it takes `DIRECT_SHARE` of the damage times `scale`
/// (what its armor made of the shot: nothing when the plate stopped it) instead of the damage its distance from the centre would give, and
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
    make_noise(world, blast.point, blast.attacker, HEAR_BLAST);
    // Soldiers are thrown; everything in the open is hurt, below.
    for id in world.entities.ids::<With<Player>>() {
        if !world.entities.alive(id) {
            continue;
        }
        let mut player = world.entities.of_mut::<Player>(id);
        if !player.embodied() {
            continue;
        }
        let c = &mut player.character;
        let away: [f32; 3] = std::array::from_fn(|i| c.position[i] - blast.point[i]);
        let distance = away.iter().map(|v| v * v).sum::<f32>().sqrt();
        let reach = blast.radius * BLAST_PUSH_REACH;
        if distance >= reach || blast.knock <= 0.0 {
            continue;
        }
        let (power, falloff) = blast_push(blast.knock, reach, distance);
        let flat = away[0].hypot(away[2]);
        // Right on the blast there is no "away": follow where it came from instead.
        let (dx, dz) = if flat > 0.1 { (away[0] / flat, away[2] / flat) } else { (blast.dir[0], blast.dir[2]) };
        c.knock = [dx * power, dz * power];
        c.vy = BLAST_HOP_MIN + (BLAST_HOP - BLAST_HOP_MIN) * falloff;
        c.grounded = false;
        c.ride = 0;
        // Hard enough, it knocks them down: thrown, then lying there a moment (KNOCKDOWN.md).
        let down = shared::movement::knockdown_ticks(power);
        if down > 0 {
            world.physics.knock_down(c, down);
            player.interrupt();
        }
        tracing::debug!(distance, power, down, "player blasted");
    }
    for (id, amount) in blasted(world, blast.point, blast.radius, blast.damage) {
        damage_entity(world, id, Harm::new(amount, blast.attacker, blast.weapon).from(blast.point));
        super::limbs::blasted(world, id, blast.point, amount, blast.attacker);
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
            // Splash is mostly turned by armour plate.
            None if super::armor::armoured(world, vid) => blast.damage * falloff * super::armor::SPLASH_ARMOURED,
            None => blast.damage * falloff,
        };
        let amount = dealt.round().min(255.0) as u8;
        // A shell that struck the vehicle has had its say about the tracks already.
        if direct.is_none() {
            super::armor::blast_parts(world, vid, blast.point, blast.damage * falloff.max(0.0), blast.attacker);
        }
        damage_vehicle(world, vid, amount, blast.attacker);
        if amount > 0 && direct.is_some_and(|direct| direct.turret) {
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
        if let Obj::Building(id) = object {
            bite_building(world, id, &blast, falloff);
        }
    }
    // Placed cubes are bitten as far as a wall would be.
    world.blast_cubes(blast.point, blast.radius * CARVE_REACH);
}

/// How much of a blast's radius is a bite out of a wall: the rest is heat and noise.
const CARVE_REACH: f32 = 0.6;
/// A bite smaller than this, metres, is not worth a collider: splash from far away.
const CARVE_MIN_RADIUS: f32 = 0.8;

/// Takes a bite out of standing building `id` where `blast` landed. A blast that was weak by the time it reached the wall bites less.
fn bite_building(world: &mut World, id: shared::world::ObjId, blast: &Blast, falloff: f32) {
    if world.destroyed.iter().any(|&(done, _)| done == Obj::Building(id)) {
        return;
    }
    let radius = blast.radius * CARVE_REACH * (0.5 + 0.5 * falloff);
    if radius < CARVE_MIN_RADIUS {
        return;
    }
    take_bite(world, id, blast.point, radius);
}

/// A round of `damage` lands on standing building `id` at `point`.
fn chip_building(world: &mut World, id: shared::world::ObjId, point: [f32; 3], damage: u8) {
    if world.destroyed.iter().any(|&(done, _)| done == Obj::Building(id)) {
        return;
    }
    world.chip(id, point, damage);
}

/// Cuts `radius` metres of standing building `id` away around `point`.
fn take_bite(world: &mut World, id: shared::world::ObjId, point: [f32; 3], radius: f32) {
    if world.destroyed.iter().any(|&(done, _)| done == Obj::Building(id)) {
        return;
    }
    world.carve(id, point, radius);
}

/// A vehicle drives into standing building `id` at `point`, `dv` m/s of speed taken out of it in
/// one step: the piece of wall it hit breaks out, and a hard enough hit takes a bite as well.
pub fn ram_building(world: &mut World, id: shared::world::ObjId, point: [f32; 3], dv: f32, weight: f32) {
    let dv = dv * weight;
    chip_building(world, id, point, (dv * RAM_CHIP_PER_DV).min(255.0) as u8);
    let radius = ((dv - RAM_BITE_MIN_DV) * RAM_BITE_PER_DV).min(RAM_BITE_MAX);
    if radius >= CARVE_MIN_RADIUS {
        take_bite(world, id, point, radius);
    }
    damage_object(world, Obj::Building(id), (dv * RAM_HP_PER_DV) as u16, point);
}

/// A vehicle drives into placed cubes at `point`, `dv` m/s of speed taken out of it in one step, as
/// into a wall (`ram_building`): the cube it hit takes the knock, and a hard enough hit takes a bite.
pub fn ram_cubes(world: &mut World, point: [f32; 3], dv: f32, weight: f32) {
    let dv = dv * weight;
    // The contact is on the cube's face: its middle is half a cube in.
    if let Some(pos) = world.construct.nearest(point.map(to_cm), shared::carve::CELL) {
        world.chip_cube(pos, (dv * RAM_CHIP_PER_DV).min(255.0) as u8);
    }
    let radius = ((dv - RAM_BITE_MIN_DV) * RAM_BITE_PER_DV).min(RAM_BITE_MAX);
    if radius >= CARVE_MIN_RADIUS {
        world.blast_cubes(point, radius);
    }
}

/// Cell damage per m/s a vehicle loses to a wall: a cell has 40, so a hit of 5 m/s breaks one out.
const RAM_CHIP_PER_DV: f32 = 8.0;
/// A hit has to take this much speed (m/s) out of the vehicle before it bites, and bites this many
/// metres of radius per m/s beyond, up to the largest.
const RAM_BITE_MIN_DV: f32 = 6.0;
const RAM_BITE_PER_DV: f32 = 0.3;
const RAM_BITE_MAX: f32 = 3.0;
/// Building health taken per m/s.
const RAM_HP_PER_DV: f32 = 12.0;

/// What a blast's centre damage becomes against a tree: they are tougher than people to
/// a rifle but not to a rocket.
const BLAST_TREE_SCALE: f32 = 4.0;

/// A bullet does this much less to a building than its damage: rifles chip walls, they don't fell them.
const BULLET_BUILDING_DIVISOR: u16 = 3;

/// `Destroyed` force of a killing blow that is exactly the object's health.
const FORCE_PER_HEALTH: f32 = 64.0;

/// Takes `amount` health off a building or tree. A tree at zero is destroyed: its collider goes, and
/// everyone is told, with the way it falls (away from `from`, where the damage came from). A
/// building never comes down: its health stops at 1, the worst damage stage, which it shows by colour.
/// Returns whether this destroyed it.
pub fn damage_object(world: &mut World, object: Obj, amount: u16, from: [f32; 3]) -> bool {
    if amount == 0 || world.destroyed.iter().any(|&(done, _)| done == object) {
        return false;
    }
    // Full health and where it stands, in metres.
    let (full, at) = match object {
        Obj::Tree(id) => match world.tree(id) {
            Some(tree) => (tree.hp(), [tree.x as f32 / 100.0, tree.z as f32 / 100.0]),
            None => return false,
        },
        Obj::Building(id) => match world.building(id) {
            Some(building) => {
                let [x, z] = building.centre();
                (building.hp(), [x as f32 / 100.0, z as f32 / 100.0])
            }
            None => return false,
        },
    };
    let before = world.damage_stage(object);
    let left = world.object_hp.get(&object).copied().unwrap_or(full).saturating_sub(amount);
    let left = if matches!(object, Obj::Building(_)) { left.max(1) } else { left };
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
        return false;
    }
    world.object_hp.remove(&object);
    if !world.physics.destroy(object) {
        return false;
    }
    // `look`'s convention: yaw 0 faces -Z.
    let yaw = to_mrad((-(at[0] - from[0])).atan2(-(at[1] - from[2])));
    world.destroyed.push((object, yaw));
    // How hard it was hit, for the way it falls: a killing blow of its own health is 64, and
    // four times that or more is the most violent a client shows.
    let force = (amount as f32 / full.max(1) as f32 * FORCE_PER_HEALTH).min(255.0) as u8;
    world.fresh_destroyed.push((object, yaw, force));
    true
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

/// Takes `amount` health off vehicle `vid`, and says so (`Damaged`, with who was aboard). Wrecks it at zero.
pub fn damage_vehicle(world: &mut World, vid: u32, amount: u8, attacker: Option<u32>) {
    let Some(seated) = world.entities.get::<Seats>(vid).map(|seats| seats.riders().collect::<Vec<u32>>()) else { return };
    let Some(mut health) = world.entities.get_mut::<Health>(vid) else { return };
    if !health.alive() || amount == 0 {
        return;
    }
    let before = health.hp;
    health.hp = health.hp.saturating_sub(amount);
    let lost = before - health.hp;
    let wrecked = !health.alive();
    if let Some(id) = attacker {
        world.entities.insert(vid, HurtBy(id));
    }
    let at = world.physics.car_pose(vid).map(|pose| pose.position);
    let from = attacker.and_then(|id| position_of(world, id));
    let centre = at.map_or([0.0; 3], |p| [p[0], p[1] + 0.8, p[2]]);
    crate::events::damaged(world, Damaged { victim: vid, lost, attacker, weapon: 0, from, at: centre, head: false, killed: wrecked, aboard: seated });
    if wrecked {
        wreck(world, vid);
    }
}

/// A vehicle's last moment: everyone inside dies, and it goes up in a blast that can take the
/// next one with it.
fn wreck(world: &mut World, vid: u32) {
    // A drone shot down only pops, and its pilot is back in their own body.
    if world.entities.has::<Drone>(vid) {
        super::drones::pop(world, vid);
        return;
    }
    let Some(pose) = world.physics.car_pose(vid) else { return };
    let attacker = world.entities.get::<HurtBy>(vid).map(|by| by.0);
    world.entities.insert(vid, Wreck { left: WRECK_TICKS });
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return };
    vehicle.controls = Default::default();
    let crew = world.entities.get_mut::<Seats>(vid).map(|mut seats| seats.take_all()).unwrap_or_default();
    for id in crew {
        if let Some(mut player) = world.entities.get_mut::<Player>(id) {
            player.seat = None;
            player.character.position = pose.position;
        }
        damage_entity(world, id, Harm::new(PLAYER_MAX_HP, attacker, WEAPON_WRECK));
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


/// Fires a rocket from player `id`'s launcher `launcher` (an `ITEM_*`), from the eye towards what the
/// crosshair is on (along the look down its sight). The launch is a shot event of the launcher's weapon
/// where the rocket leaves, and the blast is as hard as the launcher's damage.
fn fire_rocket(world: &mut World, id: u32, launcher: u8) {
    let Some(item) = def(launcher) else { return };
    let Some(Aim { origin: eye, dir, .. }) = aim(world, id, item) else { return };
    // Up against a wall it bursts right there instead of starting on its far side.
    let reach = world.physics.cast_ray(eye, dir, ROCKET_MUZZLE, 0);
    let start: [f32; 3] = std::array::from_fn(|i| eye[i] + dir[i] * reach.unwrap_or(ROCKET_MUZZLE));
    let [x, y, z] = start.map(to_cm);
    world.shots.push(ShotEvent { shooter: id, weapon: item.weapon, x, y, z });
    make_noise(world, eye, Some(id), HEAR_BLAST);
    if reach.is_some() {
        projectiles::burst_at(world, PROJECTILE_ROCKET, launcher, id, start, dir);
        return;
    }
    world.launch(Flying {
        kind: PROJECTILE_ROCKET,
        owner: id,
        source: 0,
        position: start,
        velocity: dir.map(|v| v * ROCKET_SPEED),
        ttl: ROCKET_TTL_TICKS,
        item: launcher,
        back: 0,
        flown: 0.0,
    });
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

/// The direction from `eye` to what the over-the-shoulder camera's crosshair is on: a wall, a creature
/// or a player (as `shooter` saw them `back` ticks ago) within `range`, else the far end of its line, as
/// seen from the camera's `pivot` (vehicle `ignore`, the shooter's own, is not in the way). The camera
/// is a function of the pivot, yaw and pitch alone, so the server finds the same ray the player sees.
/// Falls back to `look_dir` when the camera ray starts inside geometry or the point would send the
/// shot sideways.
#[allow(clippy::too_many_arguments)]
fn crosshair_dir(world: &World, shooter: u32, back: u32, pivot: [f32; 3], eye: [f32; 3], look_dir: [f32; 3], range: f32, ignore: u32) -> [f32; 3] {
    let wall = world.physics.cast_ray(pivot, look_dir, range, ignore).unwrap_or(range);
    let t = first_hurtbox_hit(world, pivot, look_dir, wall, Some(shooter), back, None).map_or(wall, |(_, t)| t);
    let to: [f32; 3] = std::array::from_fn(|i| pivot[i] + look_dir[i] * t - eye[i]);
    let len = to.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len < 1e-3 {
        return look_dir;
    }
    let dir = to.map(|v| v / len);
    let along: f32 = (0..3).map(|i| dir[i] * look_dir[i]).sum();
    if along < 0.5 { look_dir } else { dir }
}

/// The cone a shot of `item` is scattered over: aim held (`BUTTON_ALT`) tightens it, unless the gun is
/// held in `one_arm`, which widens it by `ONE_ARM_SPREAD` and can steady it no more.
fn effective_spread(item: &ItemDef, buttons: u16, one_arm: bool) -> f32 {
    if one_arm {
        item.spread + ONE_ARM_SPREAD
    } else if buttons & BUTTON_ALT != 0 {
        item.spread * AIM_SPREAD_SCALE
    } else {
        item.spread
    }
}

/// Whether player `id` has an arm left and no more.
fn one_armed(world: &World, id: u32) -> bool {
    super::limbs::lost(world, id) & LIMB_ARMS != 0
}

/// A round that ended in the world: damages the building, tree or placed cube it struck, if it was one.
pub(super) fn hit_object(world: &mut World, origin: [f32; 3], dir: [f32; 3], reach: f32, damage: u8) {
    // The ray stops at the surface; look a hair past it to be inside the collider.
    if let Some((_, Some(object))) = world.physics.cast_ray_object(origin, dir, reach + 0.05) {
        // Bullets only chip a wall: a third of what they do to a tree, and never nothing.
        let amount = match object {
            Obj::Tree(_) => damage as u16,
            Obj::Building(_) => (damage as u16 / BULLET_BUILDING_DIVISOR).max(1),
        };
        damage_object(world, object, amount, origin);
        if let Obj::Building(id) = object {
            // The piece of wall it struck takes the round's full damage and breaks out at zero.
            let point = [0, 1, 2].map(|i| origin[i] + dir[i] * (reach + 0.1));
            chip_building(world, id, point, damage);
        }
    } else if world.physics.ray_meets_construct(origin, dir, reach + 0.05) {
        // A placed cube is no building or tree, but takes a round as a piece of wall does.
        let point = [0, 1, 2].map(|i| origin[i] + dir[i] * (reach + 0.1));
        world.chip_cube(shared::construct::cell_at(point), damage);
    }
}

/// Whether player `id`'s reload goes to the gun in their hands: they sit in a car's passenger seat
/// holding one that is not full. Otherwise reload in a seat moves them to the next one.
pub fn reloads_in_seat(world: &World, id: u32) -> bool {
    let Some(player) = world.entities.get::<Player>(id) else { return false };
    let Some(seat) = player.seat.filter(|seat| !seat.remote) else { return false };
    let held = player.held();
    world.entities.get::<Vehicle>(seat.vehicle).is_some_and(|vehicle| crew_gun(vehicle.kind, seat.index))
        && def(held.item).is_some_and(|item| item.kind == ItemKind::Gun && held.ammo < item.mag)
}

/// Where a shot from a player's hands leaves, which way it goes, the vehicle it is fired from (or 0),
/// and how many ticks back the shooter saw the world.
struct Aim {
    origin: [f32; 3],
    dir: [f32; 3],
    vehicle: u32,
    back: u32,
}

/// How player `id` aims `item`: from their eye (their head, in a seat) towards what the crosshair is
/// on, scattered over the item's cone; or, looking down its scope, straight along the look.
fn aim(world: &mut World, id: u32, item: &ItemDef) -> Option<Aim> {
    let player = world.entities.get::<Player>(id)?;
    let input = player.input;
    let c = player.character;
    let pitch = from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH);
    let yaw = from_mrad(input.yaw);
    let look_dir = look(yaw, pitch);
    let back = rewind_ticks(world.tick, input.view());
    // The camera's pivot is to the right of the body, which is (cos yaw, 0, -sin yaw): of the capsule
    // on foot, of the head in a seat. A seated round leaves from the head and through the vehicle.
    let right = [yaw.cos(), 0.0, -yaw.sin()];
    let (origin, pivot, vehicle) = match world.seated_head(id) {
        Some((vehicle, head)) => (head, std::array::from_fn(|i| head[i] + right[i] * PASSENGER_SHOULDER + if i == 1 { PASSENGER_HEIGHT } else { 0.0 }), vehicle),
        None => (
            [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]],
            std::array::from_fn(|i| c.position[i] + right[i] * ON_FOOT_SHOULDER + if i == 1 { ON_FOOT_HEIGHT } else { 0.0 }),
            0,
        ),
    };
    // Looking down a scope the camera is the eye, so the shot goes straight along the look
    // direction, true; held in one hand, not even then.
    let scoped = item.scope && input.buttons() & BUTTON_ALT != 0;
    let one_arm = one_armed(world, id);
    let dir = if scoped && !one_arm {
        look_dir
    } else {
        // A bot has no camera: it aims its look from the eye itself.
        let aim = if scoped || world.entities.has::<Bot>(id) { look_dir } else { crosshair_dir(world, id, back, pivot, origin, look_dir, item.range, vehicle) };
        scatter(world, aim, effective_spread(item, input.buttons(), one_arm))
    };
    Some(Aim { origin, dir, vehicle, back })
}

/// Fires gun `item` from player `id`'s eye (their head, in a seat), towards what the crosshair is on.
fn fire_gun(world: &mut World, id: u32, item: &ItemDef) {
    let Some(Aim { origin, dir, vehicle, back }) = aim(world, id, item) else { return };
    let buttons = world.entities.get::<Player>(id).map_or(0, |player| player.input.buttons());
    make_noise(world, origin, Some(id), HEAR_SHOT);

    if item.projectile {
        // The round leaves the eye and flies on its own, so it takes time to arrive and drops a little.
        let velocity = dir.map(|v| v * BULLET_SPEED);
        let gun = gun_for_weapon(item.weapon);
        let bullet_id = world.launch(Flying {
            kind: PROJECTILE_BULLET,
            owner: id,
            source: vehicle,
            position: origin,
            velocity,
            ttl: (item.range / BULLET_SPEED * TICK_RATE as f32).ceil() as u32,
            item: gun,
            back,
            flown: 0.0,
        });
        // Clients fly it themselves from this: where, which way, how fast, and when.
        let [x, y, z] = origin.map(to_cm);
        let [vx, vy, vz] = velocity.map(to_cm_i16);
        world.bullets.push(BulletSpawn { id: bullet_id, shooter: id, item: gun, x, y, z, vx, vy, vz, tick: world.tick });
        return;
    }
    // A nuisance to a vehicle, not a way to kill one.
    let chip = (item.damage / 4).max(1);
    let round = Round { damage: item.damage, vehicle: chip, plane: chip, weapon: item.weapon, range: item.range, falloff: item.falloff, pen: item.pen };
    // Every pellet is its own ray through the same rewound world. The first goes along the aim as it
    // is (scattered once, above), the rest are scattered around it. Each is announced where it ended,
    // so a client can show where a spread landed; only the first carries the gun's own weapon id.
    let spread = effective_spread(item, buttons, one_armed(world, id));
    for pellet in 0..item.pellets {
        let ray = if pellet == 0 { dir } else { scatter(world, dir, spread) };
        let (end, _) = hitscan(world, id, origin, ray, &round, vehicle, back);
        let point: [i32; 3] = std::array::from_fn(|i| to_cm(origin[i] + ray[i] * end));
        let weapon = if pellet == 0 { item.weapon } else { WEAPON_PELLET };
        world.shots.push(ShotEvent { shooter: id, weapon, x: point[0], y: point[1], z: point[2] });
    }
}

/// What one hitscan round does: to a soldier or a rooster, to a vehicle, and to a plane, which is all
/// wood and canvas.
pub struct Round {
    pub damage: u8,
    pub vehicle: u8,
    pub plane: u8,
    pub weapon: u8,
    pub range: f32,
    /// How the damage of a hit on a soldier or a rooster shrinks with how far the round flew.
    pub falloff: Falloff,
    /// How much armour it goes through, and how that shrinks with how far it flew.
    pub pen: Pen,
}

/// Fires one round for `shooter` from `origin` along the unit `dir`, through vehicle `ignore` (the one
/// it is fired from, or 0), at soldiers where they stood `back` ticks ago. It hurts the first thing it
/// meets (see [`land`]). Returns how far it went, and whether it met anything at all before the end of
/// its range.
pub fn hitscan(world: &mut World, shooter: u32, origin: [f32; 3], dir: [f32; 3], round: &Round, ignore: u32, back: u32) -> (f32, bool) {
    let wall = world.physics.cast_ray_hit(origin, dir, round.range, ignore);
    let flight = Flight { origin, dir, shooter: Some(shooter), attacker: Some(shooter), back, flown: 0.0 };
    let met = strike(world, &flight, wall, round.range);
    land(world, &flight, &met, round);
    (met.end, met.any())
}

/// A round on its way, for [`strike`] and [`land`]: where it is and which way it goes, whose it is,
/// and how far it has come.
pub struct Flight {
    pub origin: [f32; 3],
    pub dir: [f32; 3],
    /// Never hit by their own round.
    pub shooter: Option<u32>,
    /// Credited with what it does; `None` once they have left.
    pub attacker: Option<u32>,
    /// Soldiers are tested where they stood this many ticks ago.
    pub back: u32,
    /// Metres flown before `origin`, for the damage falloff.
    pub flown: f32,
}

/// What a round's last stretch meets, found by [`strike`] before [`land`] does anything about it.
pub struct Strike {
    /// The world the ray met first: how far, and the vehicle if it was one.
    wall: Option<(f32, Option<u32>)>,
    /// How far the stretch goes: to the wall, or the whole range.
    reach: f32,
    /// The body met (a soldier, a rooster), and how far along.
    hit: Option<(u32, f32)>,
    /// Where it stops.
    pub end: f32,
}

impl Strike {
    /// Whether it met anything at all.
    pub fn any(&self) -> bool {
        self.hit.is_some() || self.wall.is_some()
    }
}

/// What a round flying `range` metres along `flight` meets, `wall` being what the world's ray met: the
/// first of a body (a soldier, a rooster), a vehicle, a building or a tree. A round that strikes a
/// glazed vehicle (`Exposure::Glazed`) goes on through the glass to its crew, as far as whatever is
/// behind them.
pub fn strike(world: &World, flight: &Flight, wall: Option<(f32, Option<u32>)>, range: f32) -> Strike {
    let Flight { origin, dir, shooter, back, .. } = *flight;
    let reach = wall.map_or(range, |(t, _)| t);
    let glazed = wall.and_then(|(_, car)| car).filter(|car| world.entities.get::<Vehicle>(*car).is_some_and(|vehicle| vehicle_def(vehicle.kind).crew == Exposure::Glazed));
    let through = glazed.map(|car| (car, world.physics.cast_ray(origin, dir, range, car).unwrap_or(range)));
    let hit = first_hurtbox_hit(world, origin, dir, reach, shooter, back, through);
    Strike { wall, reach, hit, end: hit.map_or(reach, |(_, t)| t) }
}

/// Hurts what `met` says the round met, by `round`'s damage less what the distance flown takes off. A
/// hitscan round and a bullet in flight both end here.
pub fn land(world: &mut World, flight: &Flight, met: &Strike, round: &Round) {
    let Flight { origin, dir, attacker, back, flown, .. } = *flight;
    let &Strike { wall, reach, hit, end } = met;
    let dealt = round.falloff.apply(round.damage, flown + end);
    if let Some((victim, t)) = hit {
        let head = is_headshot(world, victim, origin, dir, t, back);
        let point = std::array::from_fn(|i| origin[i] + dir[i] * t);
        let limb = if head { None } else { super::limbs::limb_struck(world, victim, point, back) };
        // Where on the body, for the wound to be drawn there.
        let offset = body(world, victim, back).filter(|body| body.vehicle.is_none()).map(|body| std::array::from_fn(|i| point[i] - body.anchor[i]));
        damage_entity(world, victim, Harm::new(dealt, attacker, round.weapon).head(head).limb(limb).at(point, offset));
    } else if let Some((_, Some(car))) = wall {
        let canvas = world.entities.get::<Vehicle>(car).is_some_and(|vehicle| vehicle_def(vehicle.kind).armor.canvas);
        let point = std::array::from_fn(|i| origin[i] + dir[i] * reach);
        // A round that does not get through the plate only strikes sparks off it.
        match super::armor::armor_of(world, car, origin, dir, reach, round.pen.at(flown + reach)) {
            Some(outcome @ (Outcome::Ricochet { .. } | Outcome::Blocked { .. })) => super::armor::report(world, car, &outcome, point, dir, round.pen.calibre, Default::default(), attacker),
            struck => {
                let scale = if let Some(Outcome::Pierced { scale, .. }) = struck { scale } else { 1.0 };
                let base = if canvas { round.plane } else { round.vehicle };
                // Through a slit or a thin plate, it goes on to whoever and whatever is behind.
                if let (Some(outcome @ Outcome::Pierced { residual, slit, at, .. }), Some(pose)) = (struck, world.physics.car_pose(car)) {
                    let entry = super::armor::Entry { at, dir: pose.local_dir(dir), residual, calibre: round.pen.calibre, kind: round.pen.kind, slit, crew: dealt, weapon: round.weapon, attacker };
                    let done = super::armor::inside(world, car, entry);
                    super::armor::report(world, car, &outcome, point, dir, round.pen.calibre, done, attacker);
                    // Through a gap, it never touched the vehicle itself.
                    if slit {
                        return;
                    }
                }
                damage_vehicle(world, car, (f32::from(base) * scale).round().max(1.0) as u8, attacker);
                super::vehicles::shoot_wheels(world, car, point, f32::from(round.damage), super::vehicles::BULLET_WHEEL_REACH);
            }
        }
    } else if wall.is_some() {
        hit_object(world, origin, dir, reach, round.damage);
    }
}

/// Throws a grenade from player `id`'s eye, a little above where they look so it arcs.
fn throw_grenade(world: &mut World, id: u32) {
    let Some(player) = world.entities.get::<Player>(id) else { return };
    let input = player.input;
    let c = player.character;
    let pitch = (from_mrad(input.pitch()) + GRENADE_LOFT).clamp(-MAX_PITCH, MAX_PITCH);
    let dir = look(from_mrad(input.yaw), pitch);
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
    // Up against a wall it starts at the wall instead of on its far side.
    let reach = world.physics.cast_ray(eye, dir, GRENADE_MUZZLE + GRENADE_RADIUS, 0).map_or(GRENADE_MUZZLE, |t| (t - GRENADE_RADIUS).max(0.0));
    world.launch(Flying {
        kind: PROJECTILE_GRENADE,
        owner: id,
        source: 0,
        position: std::array::from_fn(|i| eye[i] + dir[i] * reach),
        velocity: dir.map(|v| v * GRENADE_SPEED),
        ttl: GRENADE_FUSE_TICKS,
        item: 0,
        back: 0,
        flown: 0.0,
    });
}



/// Fires a flare from player `id`'s eye, well above where they look so it goes up.
fn fire_flare(world: &mut World, id: u32) {
    let Some(player) = world.entities.get::<Player>(id) else { return };
    let input = player.input;
    let c = player.character;
    let dir = look(from_mrad(input.yaw), (from_mrad(input.pitch()) + FLARE_LOFT).clamp(-MAX_PITCH, MAX_PITCH));
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
    // Up against a wall it starts at the wall instead of on its far side.
    let reach = world.physics.cast_ray(eye, dir, FLARE_MUZZLE + FLARE_RADIUS, 0).map_or(FLARE_MUZZLE, |t| (t - FLARE_RADIUS).max(0.0));
    let position: [f32; 3] = std::array::from_fn(|i| eye[i] + dir[i] * reach);
    let [x, y, z] = position.map(to_cm);
    world.shots.push(ShotEvent { shooter: id, weapon: WEAPON_FLARE, x, y, z });
    world.launch(Flying { kind: PROJECTILE_FLARE, owner: id, source: 0, position, velocity: dir.map(|v| v * FLARE_SPEED), ttl: FLARE_BURN_TICKS, item: 0, back: 0, flown: 0.0 });
}


/// The slot in hand after it has been used once: one fewer, and empty when none are left.
fn spend(slot: &mut ItemSlot) {
    slot.count = slot.count.saturating_sub(1);
    if slot.count == 0 {
        *slot = ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 };
    }
}

/// One tick of item use for a living player, standing or in a car's passenger seat: switching,
/// reloading, and what fire does.
fn use_item(world: &mut World, id: u32) {
    let Some(seat) = world.entities.get::<Player>(id).map(|player| player.seat) else { return };
    let hp = world.entities.get::<Health>(id).map_or(0, |health| health.hp);
    let lost = super::limbs::lost(world, id);
    let hurt = super::limbs::hurt(world, id);
    let passenger = seat.is_some_and(|seat| !seat.remote && world.entities.get::<Vehicle>(seat.vehicle).is_some_and(|vehicle| crew_gun(vehicle.kind, seat.index)));
    let mut player = world.entities.of_mut::<Player>(id);
    let buttons = player.input.buttons();
    let slot = (player.input.slot() as usize).min(HOTBAR_SLOTS - 1);
    if slot != player.selected {
        player.selected = slot;
        player.interrupt();
    }
    let pressed = buttons & BUTTON_FIRE != 0;
    let fresh = pressed && !player.trigger_held;
    player.trigger_held = pressed;
    let held = player.held();
    // In a seat only a car's passengers use anything, and only their guns.
    if seat.is_some() && (!passenger || def(held.item).is_none_or(|item| item.kind != ItemKind::Gun)) {
        player.interrupt();
        return;
    }
    let Some(item) = def(held.item).copied() else {
        player.interrupt();
        return;
    };
    // With no arms there is nothing to hold a gun with; a bandage can still be got on somehow. Knocked
    // down, nothing at all.
    if (lost & LIMB_ARMS == LIMB_ARMS && item.kind != ItemKind::Heal) || player.character.down > 0 {
        player.interrupt();
        return;
    }
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
                player.reload_left = item.reload_ticks * if lost & LIMB_ARMS != 0 { ONE_ARM_RELOAD } else { 1 };
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
            if fresh && player.fire_cooldown == 0 && projectiles::count(&world.entities, None) < MAX_ROCKETS + MAX_GRENADES {
                let mut player = world.entities.of_mut::<Player>(id);
                player.fire_cooldown = item.cooldown_ticks;
                spend(&mut player.hotbar[slot]);
                throw_grenade(world, id);
            }
        }
        ItemKind::Launcher => {
            if fresh && player.fire_cooldown == 0 && projectiles::count(&world.entities, Some(PROJECTILE_ROCKET)) < MAX_ROCKETS {
                let mut player = world.entities.of_mut::<Player>(id);
                player.fire_cooldown = item.cooldown_ticks;
                spend(&mut player.hotbar[slot]);
                fire_rocket(world, id, held.item);
            }
        }
        ItemKind::Flare => {
            if fresh && player.fire_cooldown == 0 && projectiles::count(&world.entities, Some(PROJECTILE_FLARE)) < MAX_FLARES {
                let mut player = world.entities.of_mut::<Player>(id);
                player.fire_cooldown = item.cooldown_ticks;
                spend(&mut player.hotbar[slot]);
                fire_flare(world, id);
            }
        }
        ItemKind::Drone(kind) => {
            if fresh && player.fire_cooldown == 0 && super::drones::launch(world, id, kind) {
                let mut player = world.entities.get_mut::<Player>(id).expect("checked above");
                player.fire_cooldown = item.cooldown_ticks;
                spend(&mut player.hotbar[slot]);
            }
        }
        ItemKind::Build => {
            if pressed && player.fire_cooldown == 0 {
                player.fire_cooldown = item.cooldown_ticks;
                super::building::use_bricks(world, id, buttons & BUTTON_ALT != 0);
            }
        }
        ItemKind::Heal => {
            if player.channel_left > 0 {
                player.channel_left -= 1;
                if player.channel_left == 0 {
                    spend(&mut player.hotbar[slot]);
                    {
                        let mut health = world.entities.of_mut::<Health>(id);
                        health.hp = health.hp.saturating_add(item.damage).min(PLAYER_MAX_HP);
                    }
                    super::limbs::heal(world, id, item.damage);
                    super::limbs::bandaged(world, id);
                }
            } else if fresh && (hp < PLAYER_MAX_HP || hurt) {
                player.channel_left = BANDAGE_TICKS;
            }
        }
    }
}

/// A bandage needs a steady hand: any hurt spoils one being put on.
fn spoil_bandage(world: &mut World, damaged: &Damaged) {
    if let Some(mut player) = world.entities.get_mut::<Player>(damaged.victim) {
        player.channel_left = 0;
    }
}

/// The dead let go of what they held: a reload or a bandage half done, and the wait to fire again.
fn let_go(world: &mut World, died: &Died) {
    if let Some(mut player) = world.entities.get_mut::<Player>(died.victim) {
        player.fire_cooldown = 0;
        player.interrupt();
    }
}

impl Mechanic for Combat {
    fn hooks(&self, hooks: &mut Hooks) {
        hooks.damaged.push(spoil_bandage);
        hooks.died.push(let_go);
    }

    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let ids = world.entities.ids::<With<Player>>();
        for id in ids {
            let loadout = world.entities.get::<Bot>(id).and_then(|bot| bot.loadout);
            let alive = world.entities.alive(id);
            // `Life` counts a dead player's `Respawn` down; at zero they come back here, at a spawn point.
            let due = !alive && world.entities.get::<Respawn>(id).is_some_and(|respawn| respawn.left == 0);
            let Some(mut player) = world.entities.get_mut::<Player>(id) else { continue };
            player.fire_cooldown = player.fire_cooldown.saturating_sub(1);
            if !alive {
                if due {
                    // A fresh life starts with the standard loadout, or a scenario bot's own.
                    player.hotbar = loadout.unwrap_or_else(starting_hotbar);
                    player.interrupt();
                    if let Some(mut health) = world.entities.get_mut::<Health>(id) {
                        health.hp = health.max;
                    }
                    spawn::place(world, id);
                }
                continue;
            }
            if player.character.position[1] < KILL_Y {
                damage_entity(world, id, Harm::new(PLAYER_MAX_HP, None, WEAPON_RIFLE));
                continue;
            }
            use_item(world, id);
        }
        projectiles::step(world, dt, Stage::Hands);
    }
}

#[cfg(test)]
mod tests;
