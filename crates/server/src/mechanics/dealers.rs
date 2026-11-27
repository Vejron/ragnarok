//! Vehicle dealers (DEALERS.md): a soldier on foot by a dealer's carport (`shared::dealer`) taps a
//! vehicle on their phone's Dealer app, and pays for it from their delivery cash. It stands ready under
//! the carport's roof facing out onto the lot, or, a helicopter, out on the lot in front; and everyone
//! is told (`S_ORDERED`), so the buyer's cash shows and the carport's door rolls up.
//!
//! An order is refused, and nothing is paid, when the buyer is not on foot by a dealer, cannot afford
//! it, the world has `MAX_VEHICLES` already, or the carport (or the helicopter's spot) is taken.
//!
//! A bought vehicle is `Bought`: once it has burnt out, fallen out of the world or stood stranded, it
//! is gone for good (`vehicles`), not put back in the carport as new as a placed one is at its home.
//!
//! A dealer also buys (`Dealers`): a vehicle of a kind it sells, driven forwards in through its door
//! (`shared::dealer::driven_in`) by a player the police are not after, fetches its price over
//! `DEALER_RESALE_DIVISOR`, less as it is hurt (`resale`), paid to the driver. Everyone aboard is set
//! down, and the vehicle goes: one placed in the world back to its home as new, so the world keeps
//! its vehicles; one bought, or taken from the traffic or a bus line, for good. A wanted driver is told
//! the dealer will not deal with them, once each time they drive in.

use super::Mechanic;
use super::vehicles::{add_vehicle, alight, drop_height, remove_vehicle, restore};
use crate::entities::{Health, With};
use crate::game::{Bot, Player, Seats, Vehicle, World};
use shared::consts::MAX_VEHICLES;
use shared::dealer::{at_dealer, driven_in, floor, heli_spot, port, price, resale};
use shared::protocol::{Ordered, S_ORDERED, S_SOLD, Sold};
use shared::vehicles::vehicle_def;
use std::collections::HashSet;

/// The dealers buying back what is driven in through their doors.
#[derive(Default)]
pub struct Dealers {
    /// The vehicles at a door whose wanted driver has been told no, so they are told once.
    refused: HashSet<u32>,
}

impl Mechanic for Dealers {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        if world.map.dealers.is_empty() {
            return;
        }
        let mut refused = HashSet::new();
        for vid in world.entities.ids::<With<Vehicle>>() {
            let Some((dealer, driver)) = at_door(world, vid) else { continue };
            if super::emergency::wanted_stars(world, driver) > 0 {
                if !self.refused.contains(&vid) {
                    world.fresh.told.push((driver, "The dealer won't buy from someone the police are after".into()));
                }
                refused.insert(vid);
                continue;
            }
            sell(world, vid, driver, dealer);
        }
        self.refused = refused;
    }

    #[cfg(test)]
    fn inside(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

/// The dealer whose door vehicle `vid` has been driven in through, and the player at its wheel: a
/// living vehicle of a kind dealers sell, with a person (not a bot) driving.
fn at_door(world: &World, vid: u32) -> Option<(usize, u32)> {
    if !world.entities.alive(vid) {
        return None;
    }
    let kind = world.entities.get::<Vehicle>(vid)?.kind;
    price(kind)?;
    let driver = world.entities.get::<Seats>(vid)?[0]?;
    if world.entities.has::<Bot>(driver) || !world.entities.has::<Player>(driver) {
        return None;
    }
    let pose = world.physics.car_pose(vid)?;
    let [x, y, z] = pose.position;
    let half = vehicle_def(kind).spec.half_extents[2];
    world.map.dealers.iter().position(|dealer| (y - floor(dealer)).abs() < 3.0 && driven_in(dealer, [x, z], pose.yaw(), half)).map(|dealer| (dealer, driver))
}

/// Vehicle `vid`, driven in through dealer `dealer`'s door by `driver`, sold: the driver paid, everyone
/// aboard set down, and the vehicle gone (home as new, or for good), and everyone told.
fn sell(world: &mut World, vid: u32, driver: u32, dealer: usize) {
    let kind = world.entities.of::<Vehicle>(vid).kind;
    let health = world.entities.get::<Health>(vid).map_or(1.0, |health| f32::from(health.hp) / f32::from(health.max.max(1)));
    let Some(pay) = resale(kind, health) else { return };
    let riders: Vec<u32> = world.entities.get::<Seats>(vid).map(|seats| seats.riders().collect()).unwrap_or_default();
    for rider in riders {
        alight(world, rider);
    }
    let cash = {
        let mut player = world.entities.of_mut::<Player>(driver);
        player.score.cash = player.score.cash.saturating_add(pay);
        player.score.cash
    };
    // One the world placed goes home as new; any other is gone.
    let placed = !(world.entities.has::<Bought>(vid) || world.entities.has::<super::traffic::Abandoned>(vid) || world.entities.has::<super::buses::Strayed>(vid));
    if placed {
        world.physics.detach(vid);
        let home = {
            let mut vehicle = world.entities.of_mut::<Vehicle>(vid);
            vehicle.renew();
            vehicle.home
        };
        world.physics.reset_car(vid, home.0, home.1);
        restore(world, vid, false);
    } else {
        remove_vehicle(world, vid);
    }
    tracing::debug!(driver, kind, vid, pay, placed, "sold to a dealer");
    world.fresh.sold.push(Sold { tag: S_SOLD, by: driver, kind, vehicle: vid, pay, cash, dealer: dealer as u16 });
}

/// A vehicle bought at a dealer: gone once wrecked, not put back as new.
#[derive(crate::entities::Component, Clone, Copy, Debug, Default)]
pub struct Bought;

/// How near the spot an ordered vehicle would stand another vehicle may be and still leave it free,
/// metres across the ground: a carport holds one at a time.
const TAKEN: f32 = 7.0;

/// Player `id` asks for a vehicle of `kind` at the dealer they stand by. Returns the new vehicle's id,
/// or `None` where the order is refused.
pub fn order(world: &mut World, id: u32, kind: u8) -> Option<u32> {
    let player = world.entities.get::<Player>(id)?;
    if !world.entities.alive(id) || player.seat.is_some() || player.lift.is_some() {
        return None;
    }
    let [x, _, z] = player.character.position;
    let cash = player.score.cash;
    let dealer_index = at_dealer(&world.map.dealers, [x, z])?;
    let dealer = world.map.dealers[dealer_index];
    let cost = price(kind)?;
    if cash < cost || world.entities.count::<Vehicle>() >= MAX_VEHICLES {
        return None;
    }
    let rotorcraft = vehicle_def(kind).spec.rotor.is_some();
    let ([sx, sz], yaw) = if rotorcraft { heli_spot(&dealer) } else { port(&dealer) };
    // On the carport's floor, or the lot's ground out in front where that stands higher.
    let ground = world.map.ground.height_at(sx, sz);
    let position = [sx, if rotorcraft { ground } else { ground.max(floor(&dealer)) } + drop_height(kind), sz];
    if !world.physics.car_fits(kind, position, yaw) || taken(world, [sx, sz]) {
        return None;
    }
    let vehicle = add_vehicle(world, kind, position, yaw);
    world.entities.insert(vehicle, Bought);
    let cash = {
        let mut player = world.entities.of_mut::<Player>(id);
        player.score.cash -= cost;
        player.score.cash
    };
    tracing::debug!(id, kind, vehicle, cost, "bought at a dealer");
    world.fresh.ordered.push(Ordered { tag: S_ORDERED, by: id, kind, vehicle, cash, dealer: dealer_index as u16 });
    Some(vehicle)
}

/// Whether a vehicle stands within `TAKEN` of `(x, z)`.
fn taken(world: &World, [x, z]: [f32; 2]) -> bool {
    world.entities.ids::<With<Vehicle>>().into_iter().any(|vid| world.physics.car_pose(vid).is_some_and(|pose| (pose.position[0] - x).hypot(pose.position[2] - z) < TAKEN))
}

#[cfg(test)]
mod tests;
