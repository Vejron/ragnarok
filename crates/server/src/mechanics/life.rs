//! Life and death for ECS entities, whatever they are: the one hit test, the one damage path and the
//! one blast gather for everything with a `Hurtbox` and `Health`, and coming back at `Home` for
//! everything with a `Respawn`. Players keep their own `combat::damage` for now.

use super::Mechanic;
use super::combat::{position_of, ray_capsule};
use crate::entities::{DropOnDeath, Health, Home, Hurtbox, NetId, Respawn, Respawned, Trail, Transform, With};
use crate::game::{Damaged, Player, World};
use shared::consts::{MAX_REWIND_TICKS, SEATED_CENTRE_DROP, SEATED_HALF_HEIGHT, SEATED_HEAD_ZONE, SEATED_RADIUS};

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

/// What a round or a blast can meet of a living entity, where it is (or was): an upright capsule
/// (`half_height` above and below `centre`, plus `radius`), the head above `head_y` if it has one, the
/// point a hit on it is shown at, and the vehicle it sits in, if it sits in one.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub centre: [f32; 3],
    pub half_height: f32,
    pub radius: f32,
    pub head_y: Option<f32>,
    pub mark: [f32; 3],
    pub vehicle: Option<u32>,
}

/// The point entity `id`'s body hangs from now, and the vehicle it sits in, if the body is out in the
/// world: a creature's feet (its `Transform`), a stickman's centre on foot (or standing over a drone's
/// controls), or their head in a seat whose crew show. A stickman inside armour has none.
fn anchor(world: &World, id: u32, transform: Option<&Transform>, player: Option<&Player>) -> Option<([f32; 3], Option<u32>)> {
    match player {
        Some(player) if player.embodied() => Some((player.character.position, None)),
        Some(_) => world.seated_head(id).map(|(vehicle, head)| (head, Some(vehicle))),
        None => transform.map(|transform| (transform.position, None)),
    }
}

/// Entity `id`'s body as it was `back` ticks ago, if a round can hit it now: alive, with a `Hurtbox`,
/// and out in the world (see [`anchor`]). Where it was comes from its `Trail`, and is where it is when
/// the trail does not go back that far, or had it seated then and standing now or the other way round.
/// Seated, it is the short capsule from the head down to the waist.
pub fn body(world: &World, id: u32, back: u32) -> Option<Body> {
    let hurtbox = world.entities.get::<Hurtbox>(id).filter(|_| world.entities.alive(id))?;
    body_of(world, id, hurtbox, world.entities.get::<Transform>(id), world.entities.get::<Player>(id), world.entities.get::<Trail>(id), back)
}

/// [`body`], with the living entity's components in hand.
fn body_of(world: &World, id: u32, hurtbox: &Hurtbox, transform: Option<&Transform>, player: Option<&Player>, trail: Option<&Trail>, back: u32) -> Option<Body> {
    let (now, vehicle) = anchor(world, id, transform, player)?;
    let at = trail.filter(|_| back > 0).and_then(|trail| trail.at(world.tick.wrapping_sub(back), vehicle.is_some())).unwrap_or(now);
    Some(match vehicle {
        None => {
            let centre = hurtbox.centre(at);
            let (half_height, radius) = (hurtbox.half_height, hurtbox.radius);
            Body { centre, half_height, radius, head_y: hurtbox.head.map(|head| centre[1] + head), mark: [centre[0], centre[1] + hurtbox.mark_y, centre[2]], vehicle: None }
        }
        Some(vehicle) => {
            let centre = [at[0], at[1] - SEATED_CENTRE_DROP, at[2]];
            Body { centre, half_height: SEATED_HALF_HEIGHT, radius: SEATED_RADIUS, head_y: hurtbox.head.map(|_| at[1] - SEATED_HEAD_ZONE), mark: centre, vehicle: Some(vehicle) }
        }
    })
}

/// The nearest living body the ray meets within `max`, and how far along it is: a stickman or a
/// creature, as it was `back` ticks ago, which is where a shooter with that much lag saw it. `ignore`
/// (the shooter) is passed through. A round that struck a glazed vehicle (`through`: the vehicle, and
/// how far the ray goes on behind its glass) also meets the crew sitting in it, out to there. Of two at
/// the same distance the lower id is hit, so the outcome does not hang on storage order.
pub fn first_hurtbox_hit(world: &World, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: Option<u32>, back: u32, through: Option<(u32, f32)>) -> Option<(u32, f32)> {
    world.entities.scan::<(&NetId, &Hurtbox, &Health, Option<&Transform>, Option<&Player>, Option<&Trail>), (), _>(|living| {
        let mut best: Option<(u32, f32)> = None;
        for (&NetId(id), hurtbox, health, transform, player, trail) in living {
            if !health.alive() || Some(id) == ignore {
                continue;
            }
            let Some(body) = body_of(world, id, hurtbox, transform, player, trail, back) else { continue };
            let reach = match through {
                Some((car, beyond)) if body.vehicle == Some(car) => beyond,
                _ => max,
            };
            let limit = best.map_or(reach, |(_, t)| t.min(reach));
            if let Some(t) = ray_capsule(origin, dir, body.centre, body.half_height, body.radius, limit)
                && best.is_none_or(|(other, at)| t < at || (t == at && id < other))
            {
                best = Some((id, t));
            }
        }
        best
    })?
}

/// Notes where every living body is, once a tick has finished, in its `Trail`.
pub fn record_trails(world: &mut World) {
    let frames: Vec<(u32, [f32; 3], bool)> = world
        .entities
        .scan::<(&NetId, &Health, Option<&Transform>, Option<&Player>), With<Trail>, _>(|living| {
            living.filter(|(_, health, ..)| health.alive()).filter_map(|(&NetId(id), _, transform, player)| anchor(world, id, transform, player).map(|(at, vehicle)| (id, at, vehicle.is_some()))).collect()
        })
        .unwrap_or_default();
    let tick = world.tick;
    for (id, at, seated) in frames {
        if let Some(mut trail) = world.entities.get_mut::<Trail>(id) {
            trail.record(tick, at, seated, MAX_REWIND_TICKS as usize);
        }
    }
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

/// Where a hit on living entity `id` is shown (its body's `mark`), or, with no body out in the world,
/// where it is.
pub fn mark(world: &World, id: u32) -> [f32; 3] {
    body(world, id, 0).map(|body| body.mark).or_else(|| position_of(world, id)).unwrap_or_default()
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
    let at = mark(world, id);
    let Some(mut health) = world.entities.get_mut::<Health>(id) else { return false };
    if !health.alive() || amount == 0 {
        return false;
    }
    let before = health.hp;
    health.hp = health.hp.saturating_sub(amount);
    let (lost, killed) = (before - health.hp, !health.alive());
    let from = from.or_else(|| attacker.and_then(|by| position_of(world, by)));
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
