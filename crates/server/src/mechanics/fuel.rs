//! Fuel (FUEL.md): a vehicle with an engine burns its tank down while a player drives it, leaks it
//! through a holed fuel tank, fills it at a fuel station, and stops when it is dry.

use crate::game::Vehicle;
use shared::armor::{Part, State};
use shared::consts::{FUEL_BOOST_BURN, FUEL_FILL_SECONDS, FUEL_IDLE_BURN, FUEL_LEAK_GONE_SECONDS, FUEL_LEAK_HURT_SECONDS, FUEL_STATION_REACH};
use shared::vehicles::vehicle_def;
use sim::CarControls;

/// What a player at the wheel asks of the engine this tick.
#[derive(Clone, Copy, Debug)]
pub struct Burn {
    pub controls: CarControls,
    /// Its boost is open (`THRUST_BOOST`).
    pub boosting: bool,
}

/// Whether `position` (metres) is within `FUEL_STATION_REACH` of one of `stations` (`[x, z]`,
/// centimetres) on the ground plane.
pub fn at_station(stations: &[[i32; 2]], position: [f32; 3]) -> bool {
    stations.iter().any(|&[x, z]| (x as f32 / 100.0 - position[0]).hypot(z as f32 / 100.0 - position[2]) <= FUEL_STATION_REACH)
}

/// The share of a full tank a holed fuel tank of `vehicle` loses a second: nothing while it is sound.
fn leak(vehicle: &Vehicle) -> f32 {
    let armor = &vehicle_def(vehicle.kind).armor;
    let Some((slot, module)) = Part::Fuel.slot().and_then(|slot| Some((slot, armor.module(slot)?))) else { return 0.0 };
    match State::of(vehicle.parts[slot], module.hp) {
        State::Sound => 0.0,
        State::Hurt => 1.0 / FUEL_LEAK_HURT_SECONDS,
        State::Gone => 1.0 / FUEL_LEAK_GONE_SECONDS,
    }
}

/// One tick of `dt` seconds of `vehicle`'s tank: what a player `burning` it uses, what leaks out of a
/// holed fuel tank whoever drives or nobody does, and what a station it stands `at_station` puts
/// back. Counts the ticks it has stood dry with no one aboard (`crewed`). Whether it is dry; never,
/// for a kind that burns no fuel.
pub fn step(vehicle: &mut Vehicle, burning: Option<Burn>, at_station: bool, crewed: bool, dt: f32) -> bool {
    let Some(lasts) = vehicle_def(vehicle.kind).spec.fuel else { return false };
    let burn = burning.map_or(0.0, |Burn { controls, boosting }| {
        let effort = controls.throttle.abs().max(controls.lift.abs()).clamp(FUEL_IDLE_BURN, 1.0);
        effort * if boosting { FUEL_BOOST_BURN } else { 1.0 } / lasts
    });
    let fill = if at_station { 1.0 / FUEL_FILL_SECONDS } else { 0.0 };
    vehicle.fuel = (vehicle.fuel + (fill - burn - leak(vehicle)) * dt).clamp(0.0, 1.0);
    let dry = vehicle.fuel == 0.0;
    vehicle.dry_ticks = if dry && !crewed { vehicle.dry_ticks + 1 } else { 0 };
    dry
}

/// A vehicle's fuel as its `fuel` component carries it, 0..=255: rounded up, so that only an empty
/// tank reads 0 and the client stops the engine exactly when the server does.
pub fn byte(fuel: f32) -> u8 {
    (fuel * 255.0).ceil().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests;
