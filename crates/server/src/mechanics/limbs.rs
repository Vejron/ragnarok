//! A soldier's body parts, each with health of its own: the torso's is its `Health`, and the head's and
//! each limb's are kept here. A round that strikes a soldier on foot low down hits a leg, and one out to
//! the side of the chest an arm (`limb_struck`); it hurts only that limb, which comes off when its health
//! runs out (`LIMB_HP`). A round to the head hurts only the head, and kills when its health runs out
//! (`HEAD_HP`). A blast hurts the torso and the limb nearest it. Only the torso or the head kills.
//!
//! What a lost limb means is everyone's to work out from `Wounds::lost` (replicated as `limbs`): with one
//! leg the soldier hops and with none it crawls (`sim::Physics::step_character`), with one arm its aim
//! wanders and its reloads are slow, and with none it can use nothing but a bandage (`combat::use_item`).
//! A stump bleeds (for show: it costs no health) until a bandage is finished, which heals every part and
//! puts one limb back, legs first.

use super::life::body;
use crate::entities::Component;
use crate::game::{Player, World};
use shared::consts::*;
use shared::protocol::*;

/// What a soldier's head and limbs have taken, and the limbs it has lost: absent while it is unhurt.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wounds {
    /// `LIMB_*`.
    pub lost: u8,
    /// The stumps that still bleed, `LIMB_*`.
    pub bleeding: u8,
    /// Damage done to each limb, by `slot`, and to the head (`HEAD`).
    pub taken: [u8; 5],
}

/// Where in `Wounds::taken` the head's damage is kept.
const HEAD: usize = 4;

impl Wounds {
    /// As the snapshot carries it: the lost limbs, and the bleeding ones above them.
    pub fn wire(&self) -> u8 {
        self.lost | (self.bleeding << LIMB_BLEED_SHIFT)
    }

    /// The health each part has left, the head first and then the limbs in `LIMB_*` order: 0 for one lost.
    pub fn left(&self) -> [u8; 5] {
        std::array::from_fn(|i| match i {
            0 => HEAD_HP.saturating_sub(self.taken[HEAD]),
            _ if self.lost & (1 << (i - 1)) != 0 => 0,
            _ => LIMB_HP.saturating_sub(self.taken[i - 1]),
        })
    }

    /// Whether any part is hurt or gone.
    fn hurt(&self) -> bool {
        self.lost != 0 || self.taken.iter().any(|&taken| taken > 0)
    }
}

/// Where in `Wounds::taken` a `LIMB_*` bit's damage is kept.
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

/// The limb at `point` on a soldier's body centred at `centre` facing `yaw`, if it is one: standing, a
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
    (lost(world, victim) & limb == 0).then_some(limb)
}

/// The limb of player `victim` nearest a blast at `point`: a leg when it goes off below the hips (a
/// grenade at their feet), an arm otherwise; on the side it is on. None for anyone seated.
fn limb_near(world: &World, victim: u32, point: [f32; 3]) -> Option<u8> {
    let body = body(world, victim, 0).filter(|body| body.vehicle.is_none())?;
    let player = world.entities.get::<Player>(victim)?;
    let [x, y, _] = local(body.centre, player.character.yaw, point);
    let arm = !player.character.prone && y >= HIP_ZONE_Y;
    let lost = lost(world, victim);
    // The one on the blast's side, or the other if that one is gone.
    [side(x < 0.0, arm), side(x >= 0.0, arm)].into_iter().find(|&limb| lost & limb == 0)
}

/// Hurts `limb` of player `id` by `amount` from `attacker`, and takes it off once its health has run out.
/// Losing an arm drops a reload or a bandage half done. Returns how much health the limb lost.
pub fn wound(world: &mut World, id: u32, limb: u8, amount: u8, attacker: Option<u32>) -> u8 {
    if !world.entities.has::<Player>(id) {
        return 0;
    }
    let mut wounds = world.entities.get::<Wounds>(id).copied().unwrap_or_default();
    if wounds.lost & limb != 0 {
        return 0;
    }
    let taken = &mut wounds.taken[slot(limb)];
    let lost = amount.min(LIMB_HP.saturating_sub(*taken));
    *taken += lost;
    let off = *taken >= LIMB_HP;
    if off {
        wounds.lost |= limb;
        wounds.bleeding |= limb;
    }
    world.entities.insert(id, wounds);
    if off {
        tracing::debug!(id, limb, attacker, "limb off");
        if limb & LIMB_ARMS != 0 {
            world.entities.of_mut::<Player>(id).interrupt();
        }
    }
    lost
}

/// Hurts the head of player `id` by `amount`. Returns how much health it lost, and whether that was the
/// last of it.
pub fn head(world: &mut World, id: u32, amount: u8) -> (u8, bool) {
    if !world.entities.has::<Player>(id) {
        return (0, false);
    }
    let mut wounds = world.entities.get::<Wounds>(id).copied().unwrap_or_default();
    let taken = &mut wounds.taken[HEAD];
    let lost = amount.min(HEAD_HP.saturating_sub(*taken));
    *taken += lost;
    let gone = *taken >= HEAD_HP;
    world.entities.insert(id, wounds);
    (lost, gone)
}

/// A blast that did `amount` to player `id` from `point`: it hurts the limb nearest it as much.
pub fn blasted(world: &mut World, id: u32, point: [f32; 3], amount: u8, attacker: Option<u32>) {
    if amount == 0 || !world.entities.alive(id) {
        return;
    }
    if let Some(limb) = limb_near(world, id, point) {
        wound(world, id, limb, amount, attacker);
    }
}

/// The limbs player `id` has lost (`LIMB_*`).
pub fn lost(world: &World, id: u32) -> u8 {
    world.entities.get::<Wounds>(id).map_or(0, |wounds| wounds.lost)
}

/// Whether any of player `id`'s head and limbs is hurt or gone.
pub fn hurt(world: &World, id: u32) -> bool {
    world.entities.get::<Wounds>(id).is_some_and(Wounds::hurt)
}

/// Heals player `id`'s head and remaining limbs by `amount`.
pub fn heal(world: &mut World, id: u32, amount: u8) {
    let Some(mut wounds) = world.entities.get::<Wounds>(id).copied() else { return };
    for taken in &mut wounds.taken {
        *taken = taken.saturating_sub(amount);
    }
    store(world, id, wounds);
}

/// A bandage finished on player `id`: the bleeding stops and one limb comes back whole, a leg before an
/// arm. (Healing is `heal`'s.)
pub fn bandaged(world: &mut World, id: u32) {
    let Some(mut wounds) = world.entities.get::<Wounds>(id).copied() else { return };
    wounds.bleeding = 0;
    if let Some(limb) = [LIMB_LEG_L, LIMB_LEG_R, LIMB_ARM_L, LIMB_ARM_R].into_iter().find(|&limb| wounds.lost & limb != 0) {
        wounds.lost &= !limb;
        wounds.taken[slot(limb)] = 0;
    }
    store(world, id, wounds);
}

/// Keeps `wounds` on player `id`, or drops them once nothing is hurt.
fn store(world: &mut World, id: u32, wounds: Wounds) {
    if wounds.hurt() {
        world.entities.insert(id, wounds);
    } else {
        world.entities.remove::<Wounds>(id);
    }
}

/// Puts every part back whole, for a fresh life, and forgets the last hit.
pub fn restore(world: &mut World, id: u32) {
    world.entities.remove::<Wounds>(id);
    world.entities.remove::<Struck>(id);
}

/// The last hit a soldier took this life (replicated as `struck`), so others see it flinch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Struck {
    /// Hits so far this life, modulo 256.
    pub count: u8,
    /// `PART_*`.
    pub part: u8,
    pub amount: u8,
    /// Where it came from, the bearing in the world (`combat::bearing`'s).
    pub yaw: i16,
    /// Where on the body it struck, cm from the point the soldier is drawn from; all zero when not
    /// known (a blast, or a hit in a seat).
    pub at: [i16; 3],
}

/// Notes that player `id` was struck in `part` (`PART_*`) for `amount`, from `from` if known, at
/// `offset` metres from the point they hang from if a round found the spot.
pub fn struck(world: &mut World, id: u32, part: u8, amount: u8, from: Option<[f32; 3]>, offset: Option<[f32; 3]>) {
    if !world.entities.has::<Player>(id) {
        return;
    }
    let count = world.entities.get::<Struck>(id).map_or(0, |struck| struck.count).wrapping_add(1);
    let at = super::life::mark(world, id);
    let yaw = from.map_or(0, |from| super::combat::bearing(at, from));
    let at = offset.map_or([0; 3], |offset| offset.map(|metres| (metres * 100.0).round().clamp(-32767.0, 32767.0) as i16));
    world.entities.insert(id, Struck { count, part, amount, yaw, at });
}

#[cfg(test)]
mod tests;
