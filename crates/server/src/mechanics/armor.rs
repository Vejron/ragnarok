//! Armor: what a shot does to a vehicle depends on where it lands and at what angle.
//!
//! A hit is judged in the vehicle's own frame (+X right, +Y up, -Z forward) from the surface normal and
//! the point the shot struck (see `Physics::armor_strike`). The side it hit is the zone (front, side,
//! back, top); the front plate of a kind that has one is tilted back by its slope, so the *effective*
//! normal leans up. The angle between the shot and that normal is the incidence angle:
//!
//! - past the zone's `ricochet` angle the shot glances off and does nothing,
//! - otherwise it does `cos(incidence)` of its damage, times the zone's multiplier and a weak spot's.
//!
//! No dice: the same shot at the same place always does the same thing, so the client and the tests
//! can tell what will happen. Every kind has its armour in its definition (`shared::vehicles`); the Strv S's steep glacis turns shots away from
//! well off the nose, a jeep only deflects the most grazing.

use crate::game::{Player, Seats, Vehicle, World};
use sim::Strike;
use shared::consts::*;
use shared::protocol::{DAMAGE_TURRET, KIND_TANK, ShotEvent, TURRET_GONE, WEAPON_RICOCHET, WEAPON_TURRET, WEAPON_WRECK};
use shared::quant::to_cm;
use shared::vehicles::vehicle_def;

/// A direct hit lands this much of the shell's blast damage, before armor: a blast scaled by its distance
/// from the vehicle's centre would punish a hit on the hull's near face.
pub const DIRECT_SHARE: f32 = 0.8;
/// Armor never takes more than this off a hit that did not glance off.
const MIN_SCALE: f32 = 0.25;
/// A glancing shot keeps this much of its speed.
pub const RICOCHET_KEEP: f32 = 0.6;

/// The sides of a hull, in the order of a profile's arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Front = 0,
    Side = 1,
    Back = 2,
    Top = 3,
}

/// A tank's turret is a drum with a rounded face: it turns shots a little more readily than the roof.
const TURRET_RICOCHET: f32 = 68.0;

/// What armor makes of a shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// It glances off along `normal` (the effective surface normal, in the hull frame).
    Ricochet { normal: [f32; 3] },
    /// It lands, for `scale` of its damage; `turret` if the turret took it.
    Hit { scale: f32, turret: bool },
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The zone a hull-frame outward `normal` faces: the axis it is most along.
fn zone_of(normal: [f32; 3]) -> Zone {
    let [x, y, z] = normal;
    if y.abs() >= x.abs() && y.abs() >= z.abs() {
        Zone::Top
    } else if z.abs() >= x.abs() {
        if z < 0.0 { Zone::Front } else { Zone::Back }
    } else {
        Zone::Side
    }
}

/// Judges a shot travelling along `dir` (a unit vector, in the hull frame) that met a `kind` at `strike`.
pub fn judge(kind: u8, strike: &Strike, dir: [f32; 3]) -> Outcome {
    let profile = &vehicle_def(kind).armor;
    let len = dot(strike.normal, strike.normal).sqrt();
    // A shot that began inside a solid has no surface to glance off.
    if len < 0.5 {
        return Outcome::Hit { scale: 1.0, turret: strike.turret };
    }
    let mut normal = strike.normal.map(|v| v / len);
    let zone = zone_of(normal);
    // The front plate leans back: turn its normal about the X axis, so it points up as well as forward.
    if zone == Zone::Front && !strike.turret {
        let (sin, cos) = profile.front_slope.sin_cos();
        normal = [normal[0], normal[1] * cos - normal[2] * sin, normal[1] * sin + normal[2] * cos];
        let len = dot(normal, normal).sqrt();
        normal = normal.map(|v| v / len);
    }
    let cos_incidence = (-dot(dir, normal)).clamp(-1.0, 1.0);
    let incidence = cos_incidence.acos().to_degrees();
    let limit = if strike.turret { TURRET_RICOCHET } else { profile.ricochet[zone as usize] };
    if incidence > limit {
        return Outcome::Ricochet { normal };
    }
    let mut scale = cos_incidence.max(MIN_SCALE) * profile.mult[zone as usize];
    for spot in profile.weak {
        let inside = (0..3).all(|i| (strike.point[i] - spot.centre[i]).abs() <= spot.half[i]);
        if inside {
            scale *= spot.mult;
        }
    }
    Outcome::Hit { scale, turret: strike.turret }
}

/// What vehicle `vid`'s armor makes of a shot from `origin` along the unit `dir` that reaches it within
/// `max` metres. `None` if the shot does not touch it.
pub fn armor_of(world: &World, vid: u32, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<Outcome> {
    let kind = world.entities.get::<Vehicle>(vid)?.kind;
    let pose = world.physics.car_pose(vid)?;
    let strike = world.physics.armor_strike(origin, dir, max + 0.05, vid)?;
    Some(judge(kind, &strike, pose.local_dir(dir)))
}

/// The direction after glancing off a surface with unit `normal`: mirrored about it.
pub fn reflect(dir: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let along = dot(dir, normal);
    std::array::from_fn(|i| dir[i] - 2.0 * along * normal[i])
}

/// Announces a shot that glanced off vehicle `vid` at `point`: a spark and a ping for everyone near.
pub fn announce_ricochet(world: &mut World, vid: u32, point: [f32; 3]) {
    let [x, y, z] = point.map(to_cm);
    world.shots.push(ShotEvent { shooter: vid, weapon: WEAPON_RICOCHET, x, y, z });
}

/// Hits of `damage` that land on the turret of tank `vid`. At zero the turret comes off: it is gone
/// until the tank is put back, its solid goes, the gunner dies with it and `DAMAGE_TURRET` tells clients.
pub fn damage_turret(world: &mut World, vid: u32, damage: u8, attacker: Option<u32>) {
    if world.entities.dead(vid) {
        return;
    }
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return };
    if vehicle.kind != KIND_TANK || vehicle.damage[DAMAGE_TURRET] == TURRET_GONE {
        return;
    }
    vehicle.turret_hp = vehicle.turret_hp.saturating_sub(damage);
    if vehicle.turret_hp > 0 {
        return;
    }
    vehicle.damage[DAMAGE_TURRET] = TURRET_GONE;
    let gunner = world.entities.get_mut::<Seats>(vid).and_then(|mut seats| seats.take(1));
    world.physics.set_turret(vid, false);
    if let Some(pose) = world.physics.car_pose(vid) {
        let at = pose.world_point(TANK_TURRET_PIVOT);
        let [x, y, z] = at.map(to_cm);
        world.shots.push(ShotEvent { shooter: vid, weapon: WEAPON_TURRET, x, y, z });
        if let Some(id) = gunner {
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.seat = None;
                player.character.position = at;
            }
            super::combat::damage(world, id, PLAYER_MAX_HP, attacker, WEAPON_WRECK);
        }
    }
}

#[cfg(test)]
mod tests;
