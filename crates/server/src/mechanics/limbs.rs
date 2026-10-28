//! Arms and legs, which rounds and blasts can take off. A round that strikes a stickman on foot low
//! down hits a leg, and one out to the side of the chest an arm (`limb_struck`); it does less to their
//! health than a round in the body, and wears the limb, which comes off at one heavy hit or enough
//! light ones (`SEVER_HIT`, `SEVER_WEAR`). A blast that does not kill can tear off the limb nearest it.
//!
//! What a lost limb means is everyone's to work out from `Limbs::lost` (replicated as `limbs`): with one
//! leg the stickman hops and with none it crawls (`sim::Physics::step_character`), with one arm its aim
//! wanders and its reloads are slow, and with none it can use nothing but a bandage (`combat::use_item`).
//! A stump bleeds (for show: it costs no health) until a bandage is finished, which also puts one limb
//! back, legs first.

use super::life::body;
use crate::entities::Component;
use crate::game::{Player, World};
use shared::consts::*;
use shared::protocol::*;

/// The limbs a stickman has lost and what the rest have taken: absent while it is whole and unhurt.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limbs {
    /// `LIMB_*`.
    pub lost: u8,
    /// The stumps that still bleed, `LIMB_*`.
    pub bleeding: u8,
    /// Damage done to each limb, by `slot`.
    pub wear: [u8; 4],
}

impl Limbs {
    /// As the snapshot carries it: the lost limbs, and the bleeding ones above them.
    pub fn wire(&self) -> u8 {
        self.lost | (self.bleeding << LIMB_BLEED_SHIFT)
    }
}

/// Where in `wear` a `LIMB_*` bit's damage is kept.
fn slot(limb: u8) -> usize {
    limb.trailing_zeros() as usize
}

/// The `LIMB_*` on one side: the left (`-x` of its facing) or the right.
fn side(left: bool, arm: bool) -> u8 {
    match (arm, left) {
        (true, true) => LIMB_ARM_L,
        (true, false) => LIMB_ARM_R,
        (false, true) => LIMB_LEG_L,
        (false, false) => LIMB_LEG_R,
    }
}

/// `point` in the frame of a body centred at `centre` facing `yaw` (0 faces -Z): to its right, up, and
/// ahead of it.
fn local(centre: [f32; 3], yaw: f32, point: [f32; 3]) -> [f32; 3] {
    let (sin, cos) = yaw.sin_cos();
    let [dx, dy, dz] = [point[0] - centre[0], point[1] - centre[1], point[2] - centre[2]];
    [dx * cos - dz * sin, dy, -dx * sin - dz * cos]
}

/// The limb at `point` on a stickman's body centred at `centre` facing `yaw`, if it is one: standing, a
/// leg below the hips and an arm out to the side of the chest; lying down, a leg in the back half of the
/// body and an arm out to the side of the front.
pub fn limb_at(centre: [f32; 3], yaw: f32, prone: bool, point: [f32; 3]) -> Option<u8> {
    let [x, y, ahead] = local(centre, yaw, point);
    let left = x < 0.0;
    if prone {
        return if ahead < -0.1 {
            Some(side(left, false))
        } else if x.abs() > ARM_ZONE_X {
            Some(side(left, true))
        } else {
            None
        };
    }
    if y < HIP_ZONE_Y {
        Some(side(left, false))
    } else if y < HEAD_ZONE_Y && x.abs() > ARM_ZONE_X {
        Some(side(left, true))
    } else {
        None
    }
}

/// The limb of player `victim`, as their body was `back` ticks ago, that a round striking it at `point`
/// hit: none for a head or the body, anyone seated (their legs are inside), or a creature.
pub fn limb_struck(world: &World, victim: u32, point: [f32; 3], back: u32) -> Option<u8> {
    let body = body(world, victim, back).filter(|body| body.vehicle.is_none())?;
    if body.head_y.is_some_and(|head| point[1] > head) {
        return None;
    }
    let player = world.entities.get::<Player>(victim)?;
    let limb = limb_at(body.centre, player.character.yaw, player.character.prone, point)?;
    // A round through where a limb was strikes the body.
    (world.entities.get::<Limbs>(victim).is_none_or(|limbs| limbs.lost & limb == 0)).then_some(limb)
}

/// The limb of player `victim` nearest a blast at `point`: a leg when it goes off below the hips (a
/// grenade at their feet), an arm otherwise; on the side it is on. None for anyone seated.
fn limb_near(world: &World, victim: u32, point: [f32; 3]) -> Option<u8> {
    let body = body(world, victim, 0).filter(|body| body.vehicle.is_none())?;
    let player = world.entities.get::<Player>(victim)?;
    let [x, y, _] = local(body.centre, player.character.yaw, point);
    let arm = !player.character.prone && y >= HIP_ZONE_Y;
    let lost = world.entities.get::<Limbs>(victim).map_or(0, |limbs| limbs.lost);
    // The one on the blast's side, or the other if that one is gone.
    [side(x < 0.0, arm), side(x >= 0.0, arm)].into_iter().find(|&limb| lost & limb == 0)
}

/// Wears `limb` of player `id` by `amount` from `attacker`, and takes it off at a hit of `SEVER_HIT` or
/// once it has taken `SEVER_WEAR` in all. Losing an arm drops a reload or a bandage half done.
pub fn wound(world: &mut World, id: u32, limb: u8, amount: u8, attacker: Option<u32>) {
    if !world.entities.has::<Player>(id) {
        return;
    }
    let mut limbs = world.entities.get::<Limbs>(id).copied().unwrap_or_default();
    if limbs.lost & limb != 0 {
        return;
    }
    let wear = &mut limbs.wear[slot(limb)];
    *wear = wear.saturating_add(amount);
    let off = amount >= SEVER_HIT || *wear >= SEVER_WEAR;
    if off {
        limbs.lost |= limb;
        limbs.bleeding |= limb;
    }
    world.entities.insert(id, limbs);
    if !off {
        return;
    }
    tracing::debug!(id, limb, attacker, "limb off");
    if limb & LIMB_ARMS != 0 {
        world.entities.of_mut::<Player>(id).interrupt();
    }
}

/// A blast that did `amount` to player `id` from `point` and left them alive: one big enough tears off
/// the limb nearest it.
pub fn blasted(world: &mut World, id: u32, point: [f32; 3], amount: u8, attacker: Option<u32>) {
    if amount < SEVER_HIT || !world.entities.alive(id) {
        return;
    }
    if let Some(limb) = limb_near(world, id, point) {
        wound(world, id, limb, amount, attacker);
    }
}

/// The limbs player `id` has lost (`LIMB_*`).
pub fn lost(world: &World, id: u32) -> u8 {
    world.entities.get::<Limbs>(id).map_or(0, |limbs| limbs.lost)
}

/// A bandage finished on player `id`: the bleeding stops and one limb comes back, a leg before an arm.
pub fn bandaged(world: &mut World, id: u32) {
    let Some(mut limbs) = world.entities.get::<Limbs>(id).copied() else { return };
    limbs.bleeding = 0;
    if let Some(limb) = [LIMB_LEG_L, LIMB_LEG_R, LIMB_ARM_L, LIMB_ARM_R].into_iter().find(|&limb| limbs.lost & limb != 0) {
        limbs.lost &= !limb;
        limbs.wear[slot(limb)] = 0;
    }
    if limbs.lost == 0 {
        world.entities.remove::<Limbs>(id);
    } else {
        world.entities.insert(id, limbs);
    }
}

/// Puts every limb back, for a fresh life.
pub fn restore(world: &mut World, id: u32) {
    world.entities.remove::<Limbs>(id);
}

#[cfg(test)]
mod tests;
