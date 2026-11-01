//! Life and death for everything that can be hurt, whatever it is: where its body is, the one hit test,
//! the one damage path and the one blast gather for everything with a `Hurtbox` and `Health`, players
//! and creatures alike, and the count back to life for everything with a `Respawn` (at its `Home`, or
//! a player where `Combat` puts them).

use shared::vec3;
use super::Mechanic;
use super::combat::{position_of, ray_capsule, same_team};
use crate::entities::{DropOnDeath, Health, Home, Hurtbox, NetId, Posture, Respawn, Respawned, Trail, Transform, With};
use crate::events;
use crate::game::{Damaged, Died, Player, World};
use shared::consts::{CROUCH_DROP, MAX_REWIND_TICKS, PRONE_HURT_HALF_LENGTH, PRONE_HURT_RADIUS, SEATED_CENTRE_DROP, SEATED_HALF_HEIGHT, SEATED_HEAD_ZONE, SEATED_RADIUS};
use shared::protocol::{
    PART_HEAD, PART_TORSO, DEATH_BLAST, DEATH_BODY, DEATH_HEAD, DEATH_OVERKILL, DEATH_ROADKILL, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_DRONE, WEAPON_EGG, WEAPON_GRENADE, WEAPON_LOITER, WEAPON_ROADKILL, WEAPON_ROCKET,
    WEAPON_AP, WEAPON_SHELL, WEAPON_WRECK,
};

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

/// What a round or a blast can meet of a living entity, where it is (or was): a capsule (`half_height`
/// either way along `axis` from `centre`, plus `radius`), upright unless it is lying down, the head
/// past `head_y` along that axis if it has one, the point a hit on it is shown at, and the vehicle it
/// sits in, if it sits in one.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    /// The point it hangs from (see [`anchor`]): where a soldier is drawn from.
    pub anchor: [f32; 3],
    pub centre: [f32; 3],
    pub half_height: f32,
    pub radius: f32,
    pub axis: [f32; 3],
    /// Which way its head lies, when it lies down.
    pub lying: Option<f32>,
    pub head_y: Option<f32>,
    pub mark: [f32; 3],
    /// Where the limb zones are measured from: `centre`, or for a crouched soldier as low as the
    /// crouch took their hips and shoulders.
    pub zones: [f32; 3],
    pub vehicle: Option<u32>,
}

const UP: [f32; 3] = [0.0, 1.0, 0.0];

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Body {
    /// How far along the ray from `origin` it is first met, within `max`.
    pub fn ray(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<f32> {
        if self.lying.is_none() {
            return ray_capsule(origin, dir, self.centre, self.half_height, self.radius, max);
        }
        // Seen in a frame where it stands up along y, it is the upright capsule at the origin.
        let side = [self.axis[2], 0.0, -self.axis[0]];
        let frame = |v: [f32; 3]| [dot(side, v), dot(self.axis, v), v[1]];
        let from: [f32; 3] = std::array::from_fn(|i| origin[i] - self.centre[i]);
        ray_capsule(frame(from), frame(dir), [0.0; 3], self.half_height, self.radius, max)
    }

    /// From the nearest point of the capsule's axis to `point`.
    pub fn off_axis(&self, point: [f32; 3]) -> [f32; 3] {
        let to: [f32; 3] = std::array::from_fn(|i| point[i] - self.centre[i]);
        let along = dot(to, self.axis).clamp(-self.half_height, self.half_height);
        std::array::from_fn(|i| to[i] - self.axis[i] * along)
    }

    /// Whether `point` is on its head, when it has one.
    pub fn is_head(&self, point: [f32; 3]) -> bool {
        self.head_y.is_some_and(|head| dot(point, self.axis) > head)
    }

    /// The limb at `point`, for a soldier now facing `yaw`, however it lay when it was struck.
    pub fn limb_at(&self, yaw: f32, point: [f32; 3]) -> Option<u8> {
        super::limbs::limb_at(self.zones, self.lying.unwrap_or(yaw), self.lying.is_some(), point)
    }
}

/// The point entity `id`'s body hangs from now, and the vehicle it sits in, if the body is out in the
/// world: a creature's feet (its `Transform`), a soldier's centre on foot (or standing over a drone's
/// controls), or their head in a seat whose crew show. A soldier inside armour has none.
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
/// Seated, it is the short capsule from the head down to the waist. Crouched, it is as low at the top
/// by `CROUCH_DROP`, with the feet where they were and the head and the hit mark lower with it. Lying
/// down, it is a capsule on its side, along the way the soldier faces, with the head at the front.
pub fn body(world: &World, id: u32, back: u32) -> Option<Body> {
    let hurtbox = world.entities.get::<Hurtbox>(id).filter(|_| world.entities.alive(id))?;
    body_of(world, id, hurtbox, world.entities.get::<Transform>(id), world.entities.get::<Player>(id), world.entities.get::<Trail>(id), back)
}

/// [`body`], with the living entity's components in hand.
fn body_of(world: &World, id: u32, hurtbox: &Hurtbox, transform: Option<&Transform>, player: Option<&Player>, trail: Option<&Trail>, back: u32) -> Option<Body> {
    let (now, vehicle) = anchor(world, id, transform, player)?;
    let then = trail.filter(|_| back > 0).and_then(|trail| trail.then(world.tick.wrapping_sub(back), vehicle.is_some()));
    let (at, posture) = then.unwrap_or((now, player.map_or(Posture::Upright, Player::posture)));
    Some(match vehicle {
        None => {
            let upright = hurtbox.centre(at);
            if let Posture::Lying { yaw } = posture {
                let axis = [-yaw.sin(), 0.0, -yaw.cos()];
                let (half_height, radius) = (PRONE_HURT_HALF_LENGTH, PRONE_HURT_RADIUS);
                let head_y = hurtbox.head.map(|head| dot(upright, axis) + head);
                return Some(Body { anchor: at, centre: upright, half_height, radius, axis, lying: Some(yaw), head_y, mark: [upright[0], upright[1] + radius, upright[2]], zones: upright, vehicle: None });
            }
            let drop = if posture == Posture::Crouched { CROUCH_DROP } else { 0.0 };
            let lowered = |by: f32| [upright[0], upright[1] - by, upright[2]];
            let (half_height, radius) = (hurtbox.half_height - drop / 2.0, hurtbox.radius);
            let zones = lowered(drop);
            Body { anchor: at, centre: lowered(drop / 2.0), half_height, radius, axis: UP, lying: None, head_y: hurtbox.head.map(|head| zones[1] + head), mark: [zones[0], zones[1] + hurtbox.mark_y, zones[2]], zones, vehicle: None }
        }
        Some(vehicle) => {
            let centre = [at[0], at[1] - SEATED_CENTRE_DROP, at[2]];
            Body { anchor: at, centre, half_height: SEATED_HALF_HEIGHT, radius: SEATED_RADIUS, axis: UP, lying: None, head_y: hurtbox.head.map(|_| at[1] - SEATED_HEAD_ZONE), mark: centre, zones: centre, vehicle: Some(vehicle) }
        }
    })
}

/// The nearest living body the ray meets within `max`, and how far along it is: a soldier or a
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
            if let Some(t) = body.ray(origin, dir, limit).and_then(|t| past_lost_limbs(&body, player, super::limbs::lost(world, id), origin, dir, t, limit))
                && best.is_none_or(|(other, at)| t < at || (t == at && id < other))
            {
                best = Some((id, t));
            }
        }
        best
    })?
}

/// How far along the ray a round entering `body` at `entry` first meets something of it: a soldier's
/// lost limbs (`lost`) are not there, so a round goes on through where one was, into the body behind it if the
/// way lies through that, and out the other side if not (none: it missed). Within `limit`.
fn past_lost_limbs(body: &Body, player: Option<&Player>, lost: u8, origin: [f32; 3], dir: [f32; 3], entry: f32, limit: f32) -> Option<f32> {
    let Some(character) = player.map(|player| &player.character).filter(|_| lost != 0 && body.vehicle.is_none()) else { return Some(entry) };
    // Steps of 2 cm through the capsule, as far as it is long.
    const STEP: f32 = 0.02;
    let length = 2.0 * (body.half_height + body.radius);
    let mut t = entry;
    while t <= (entry + length).min(limit) {
        let p: [f32; 3] = vec3::along(origin, dir, t);
        let inside = body.off_axis(p).iter().map(|v| v * v).sum::<f32>() <= (body.radius + 1e-3).powi(2);
        if !inside && t > entry + STEP {
            return None;
        }
        let gone = !body.is_head(p) && body.limb_at(character.yaw, p).is_some_and(|limb| lost & limb != 0);
        if inside && !gone {
            return Some(t);
        }
        t += STEP;
    }
    None
}

/// Notes where every living body is, once a tick has finished, in its `Trail`.
pub fn record_trails(world: &mut World) {
    let frames: Vec<(u32, [f32; 3], bool, Posture)> = world
        .entities
        .scan::<(&NetId, &Health, Option<&Transform>, Option<&Player>), With<Trail>, _>(|living| {
            living.filter(|(_, health, ..)| health.alive()).filter_map(|(&NetId(id), _, transform, player)| anchor(world, id, transform, player).map(|(at, vehicle)| (id, at, vehicle.is_some(), player.map_or(Posture::Upright, Player::posture)))).collect()
        })
        .unwrap_or_default();
    let tick = world.tick;
    for (id, at, seated, posture) in frames {
        if let Some(mut trail) = world.entities.get_mut::<Trail>(id) {
            trail.record(tick, at, seated, posture, MAX_REWIND_TICKS as usize);
        }
    }
}

/// Whether a living body out in the open (not in a seat) other than `ignore` has the axis of its
/// capsule within `proximity` of `point`: what sets off a proximity fuse.
pub fn hurtbox_near(world: &World, point: [f32; 3], proximity: f32, ignore: Option<u32>) -> bool {
    world
        .entities
        .scan::<(&NetId, &Hurtbox, &Health, Option<&Transform>, Option<&Player>), (), _>(|living| {
            living.filter(|&(&NetId(id), _, health, ..)| health.alive() && Some(id) != ignore).any(|(&NetId(id), hurtbox, _, transform, player)| {
                let Some(body) = body_of(world, id, hurtbox, transform, player, None, 0).filter(|body| body.vehicle.is_none()) else { return false };
                body.off_axis(point).iter().map(|v| v * v).sum::<f32>() < proximity * proximity
            })
        })
        .unwrap_or(false)
}

/// Where a hit on living entity `id` is shown (its body's `mark`), or, with no body out in the world,
/// where it is.
pub fn mark(world: &World, id: u32) -> [f32; 3] {
    body(world, id, 0).map(|body| body.mark).or_else(|| position_of(world, id)).unwrap_or_default()
}

/// The living bodies out in the open (not in a seat: a vehicle takes the blast for its crew) within
/// `radius` of `point`, measured to the capsule's centre, in id order, each with the damage that
/// reaches it.
pub fn blasted(world: &World, point: [f32; 3], radius: f32, damage: f32) -> Vec<(u32, u8)> {
    let mut reached = Vec::new();
    for id in world.entities.ids::<(With<Hurtbox>, With<Health>)>() {
        let Some(body) = body(world, id, 0).filter(|body| body.vehicle.is_none()) else { continue };
        let distance = vec3::distance(body.centre, point);
        if distance < radius {
            reached.push((id, (damage * (1.0 - distance / radius)).round().min(255.0) as u8));
        }
    }
    reached
}

/// Harm done to something: how much, by whom (if anyone) with what (`WEAPON_*`), where it came from if
/// not from where the attacker is (a blast's centre), and whether a round struck the head or a limb
/// (`LIMB_*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Harm {
    pub amount: u8,
    pub attacker: Option<u32>,
    pub weapon: u8,
    pub from: Option<[f32; 3]>,
    pub head: bool,
    pub limb: Option<u8>,
    /// Where it struck, when a round did; the body's mark otherwise.
    pub at: Option<[f32; 3]>,
    /// That point less the point the body hangs from (`Body::anchor`) as it stood when struck, for
    /// the wound to be drawn on the body; none for a body in a seat.
    pub offset: Option<[f32; 3]>,
    /// Hurts the attacker's own side too: nothing spares the crew of a vehicle going up.
    pub unsparing: bool,
}

impl Harm {
    pub fn new(amount: u8, attacker: Option<u32>, weapon: u8) -> Self {
        Harm { amount, attacker, weapon, from: None, head: false, limb: None, at: None, offset: None, unsparing: false }
    }

    /// The same, sparing no one.
    pub fn unsparing(self) -> Self {
        Harm { unsparing: true, ..self }
    }

    /// The same, come from `point`.
    pub fn from(self, point: [f32; 3]) -> Self {
        Harm { from: Some(point), ..self }
    }

    /// The same, to the head when `head`.
    pub fn head(self, head: bool) -> Self {
        Harm { head, ..self }
    }

    /// The same, to `limb` if it struck one.
    pub fn limb(self, limb: Option<u8>) -> Self {
        Harm { limb, ..self }
    }

    /// The same, struck at `point`, `offset` from where the body hangs.
    pub fn at(self, point: [f32; 3], offset: Option<[f32; 3]>) -> Self {
        Harm { at: Some(point), offset, ..self }
    }
}

/// The weapons whose kill throws the body down as a blast does.
const BLASTS: [u8; 10] = [WEAPON_ROCKET, WEAPON_SHELL, WEAPON_AP, WEAPON_WRECK, WEAPON_GRENADE, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_DRONE, WEAPON_LOITER, WEAPON_EGG];

/// The one place health goes down, for anything with `Health` and a body: takes `harm` off entity
/// `id`, and says so (`Damaged`). A soldier's head and limbs have health of their own (`limbs`): a blow
/// to the head hurts only the head, and kills once it has none left; one to a limb hurts only that limb,
/// which comes off once it has none left, and never kills. Anything else hurts the torso (`Health`).
/// The attacker's own team is spared. At zero it dies (`Died`, crediting the attacker if it was anyone
/// other than itself): its `Respawn` starts counting and its `DropOnDeath` is left where it fell.
/// Returns whether this killed it; does nothing to the dead.
pub fn damage_entity(world: &mut World, id: u32, harm: Harm) -> bool {
    let Harm { amount, attacker, weapon, from, head, limb, at, offset, unsparing } = harm;
    let other = attacker.filter(|&by| by != id);
    if (!unsparing && other.is_some_and(|by| same_team(world, by, id))) || amount == 0 || !world.entities.alive(id) {
        return false;
    }
    let from = from.or_else(|| other.and_then(|by| position_of(world, by)));
    let at = at.unwrap_or_else(|| mark(world, id));
    let soldier = world.entities.has::<Player>(id);
    // What a part of a soldier other than the torso lost, and whether that killed them.
    let (part, spent) = match (soldier, head, limb) {
        (true, true, _) => (PART_HEAD, Some(super::limbs::head(world, id, amount))),
        (true, false, Some(limb)) => (limb, Some((super::limbs::wound(world, id, limb, amount, attacker), false))),
        _ => (PART_TORSO, None),
    };
    let Some(mut health) = world.entities.get_mut::<Health>(id) else { return false };
    let before = health.hp;
    let lost = match spent {
        Some((lost, fatal)) => {
            if fatal {
                health.hp = 0;
            }
            lost
        }
        None => {
            health.hp = health.hp.saturating_sub(amount);
            before - health.hp
        }
    };
    let killed = !health.alive();
    if lost == 0 && !killed {
        return false;
    }
    super::limbs::struck(world, id, part, lost, from, offset);
    events::damaged(world, Damaged { victim: id, lost, attacker, weapon, from, at, head: part == PART_HEAD, killed, aboard: Vec::new() });
    if !killed {
        return false;
    }
    if let Some(mut respawn) = world.entities.get_mut::<Respawn>(id) {
        respawn.left = respawn.delay;
    }
    let drop = world.entities.get::<DropOnDeath>(id).copied();
    let feet = world.entities.get::<Transform>(id).map(|transform| transform.position);
    if let (Some(drop), Some([x, y, z])) = (drop, feet) {
        if drop.unique {
            world.remove_pickups(|p| !(p.dropped && p.item == drop.item));
        }
        let item = world.add_pickup([x, y + DROP_FLOAT, z], drop.item, drop.count, true);
        tracing::debug!(id, item, "dropped on death");
    }
    let style = match weapon {
        WEAPON_ROADKILL => DEATH_ROADKILL,
        _ if BLASTS.contains(&weapon) => DEATH_BLAST,
        _ if head => DEATH_HEAD,
        _ if amount > before.saturating_mul(2) => DEATH_OVERKILL,
        _ => DEATH_BODY,
    };
    events::died(world, Died { victim: id, killer: other, weapon, style, from });
    tracing::debug!(id, killer = other, weapon, "killed");
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
