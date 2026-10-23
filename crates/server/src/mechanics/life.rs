//! Life and death for ECS entities, whatever they are: the one hit test, the one damage path and the
//! one blast gather for everything with a `Hurtbox` and `Health`, and coming back at `Home` for
//! everything with a `Respawn`. Players keep their own `combat::damage` for now.

use super::Mechanic;
use super::combat::{position_of, ray_capsule};
use crate::entities::{DropOnDeath, Health, Home, Hurtbox, Respawn, Respawned, Transform, With};
use crate::game::{Damaged, World};

pub struct Life;

/// How high above where it died a dropped item floats, metres, as pickups do.
const DROP_FLOAT: f32 = 0.5;

/// Ground height at `x`, `z` if it is open ground (nothing, such as a roof, above it).
pub(super) fn open_ground(world: &World, x: f32, z: f32) -> Option<f32> {
    if !world.map.is_inside((x * 100.0) as i32, (z * 100.0) as i32) {
        return None;
    }
    let ground = world.map.ground.height_at(x, z);
    let top = ground + 40.0;
    let floor = world.physics.cast_ray([x, top, z], [0.0, -1.0, 0.0], 80.0, 0).map(|t| top - t)?;
    ((floor - ground).abs() < 0.5).then_some(floor)
}

/// The nearest living entity with a hurtbox that the ray meets within `max`, and how far along it is.
/// Of two at the same distance the lower id is hit, so the outcome does not hang on storage order.
pub fn first_hurtbox_hit(world: &World, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(u32, f32)> {
    world.entities.scan::<(&crate::entities::NetId, &Transform, &Hurtbox, &Health), (), _>(|living| {
        let mut best: Option<(u32, f32)> = None;
        for (id, transform, hurtbox, health) in living {
            if !health.alive() {
                continue;
            }
            let limit = best.map_or(max, |(_, t)| t);
            if let Some(t) = ray_capsule(origin, dir, hurtbox.centre(transform.position), hurtbox.half_height, hurtbox.radius, limit)
                && best.is_none_or(|(other, at)| t < at || (t == at && id.0 < other))
            {
                best = Some((id.0, t));
            }
        }
        best
    })?
}

/// Whether a living entity with a hurtbox has the axis of its capsule within `proximity` of `point`:
/// what sets off a proximity fuse, as a stickman does.
pub fn hurtbox_near(world: &World, point: [f32; 3], proximity: f32) -> bool {
    world.entities.scan::<(&Transform, &Hurtbox, &Health), (), _>(|living| {
        living.filter(|(_, _, health)| health.alive()).any(|(transform, hurtbox, _)| {
            let c = hurtbox.centre(transform.position);
            let dy = (point[1] - c[1]).clamp(-hurtbox.half_height, hurtbox.half_height);
            let d = [point[0] - c[0], point[1] - c[1] - dy, point[2] - c[2]];
            d.iter().map(|v| v * v).sum::<f32>() < proximity * proximity
        })
    })
    .unwrap_or(false)
}

/// Where a hit on entity `id` is shown: the top of its capsule.
fn hit_mark(world: &World, id: u32) -> Option<[f32; 3]> {
    let transform = world.entities.get::<Transform>(id)?;
    let hurtbox = world.entities.get::<Hurtbox>(id)?;
    let [x, y, z] = transform.position;
    Some([x, y + hurtbox.centre_y + hurtbox.half_height, z])
}

/// The living entities with a hurtbox within `radius` of `point` (measured to the capsule's centre),
/// in id order, each with the damage that reaches it.
pub fn blasted(world: &World, point: [f32; 3], radius: f32, damage: f32) -> Vec<(u32, u8)> {
    let mut reached = Vec::new();
    for id in world.entities.ids::<(With<Transform>, With<Hurtbox>, With<Health>)>() {
        let (Some(transform), Some(hurtbox), Some(health)) = (world.entities.get::<Transform>(id), world.entities.get::<Hurtbox>(id), world.entities.get::<Health>(id)) else { continue };
        if !health.alive() {
            continue;
        }
        let centre = hurtbox.centre(transform.position);
        let distance = (0..3).map(|i| (centre[i] - point[i]).powi(2)).sum::<f32>().sqrt();
        if distance < radius {
            reached.push((id, (damage * (1.0 - distance / radius)).round().min(255.0) as u8));
        }
    }
    reached
}

/// Takes `amount` health off entity `id`, by `attacker` with `weapon`, and says so (`Damaged`); the
/// harm came `from` there, or from where the attacker is. At zero it dies: its `Respawn` starts
/// counting and its `DropOnDeath` is left where it fell. Returns whether this killed it; does nothing
/// to the dead.
pub fn damage_entity(world: &mut World, id: u32, amount: u8, attacker: Option<u32>, weapon: u8, from: Option<[f32; 3]>) -> bool {
    let Some(mut health) = world.entities.get_mut::<Health>(id) else { return false };
    if !health.alive() || amount == 0 {
        return false;
    }
    let before = health.hp;
    health.hp = health.hp.saturating_sub(amount);
    let (lost, killed) = (before - health.hp, !health.alive());
    let from = from.or_else(|| attacker.and_then(|by| position_of(world, by)));
    let at = hit_mark(world, id).unwrap_or_default();
    world.send(Damaged { victim: id, lost, attacker, weapon, from, at, head: false, killed, aboard: Vec::new() });
    if !killed {
        return false;
    }
    if let Some(mut respawn) = world.entities.get_mut::<Respawn>(id) {
        respawn.left = respawn.delay;
    }
    let drop = world.entities.get::<DropOnDeath>(id).copied();
    let at = world.entities.get::<Transform>(id).map(|transform| transform.position);
    if let (Some(drop), Some([x, y, z])) = (drop, at) {
        if drop.unique {
            world.remove_pickups(|p| !(p.dropped && p.item == drop.item));
        }
        let item = world.add_pickup([x, y + DROP_FLOAT, z], drop.item, drop.count, true);
        tracing::debug!(id, item, "dropped on death");
    }
    true
}

/// Puts entity `id` at its home on the ground, whole, and marks it `Respawned`.
pub fn place_at_home(world: &mut World, id: u32) {
    let Some(Home(home)) = world.entities.get::<Home>(id).copied() else { return };
    let y = open_ground(world, home[0], home[1]).unwrap_or_else(|| world.map.ground.height_at(home[0], home[1]));
    if let Some(mut transform) = world.entities.get_mut::<Transform>(id) {
        transform.position = [home[0], y, home[1]];
    }
    if let Some(mut health) = world.entities.get_mut::<Health>(id) {
        health.hp = health.max;
    }
    if let Some(mut respawn) = world.entities.get_mut::<Respawn>(id) {
        respawn.left = 0;
    }
    world.entities.insert(id, Respawned);
}

impl Mechanic for Life {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        for id in world.entities.ids::<(With<Health>, With<Respawn>)>() {
            if world.entities.get::<Health>(id).is_none_or(Health::alive) {
                continue;
            }
            let Some(mut respawn) = world.entities.get_mut::<Respawn>(id) else { continue };
            respawn.left = respawn.left.saturating_sub(1);
            if respawn.left == 0 {
                place_at_home(world, id);
            }
        }
    }
}
