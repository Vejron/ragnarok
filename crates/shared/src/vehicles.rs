//! What each vehicle kind is, in one table: the gameplay facts both the server and the client need
//! (health, seats, size, what it carries). The physics tuning stays in the server's `Spec`s and the
//! client's `sim/vehicle.ts`; a kind's wheel layout and extents are the constants in `consts.rs`.
//!
//! Adding a kind: a `KIND_*` id in `protocol.rs`, its constants in `consts.rs`, a `VehicleDef` here
//! and in `client/src/sim/vehicles.ts`, then its physics `Spec` and model. `fixtures/parity/
//! gameplay.json` fails until the two tables agree.

use crate::consts::*;
use crate::protocol::{KIND_HELI, KIND_JEEP, KIND_MORTAR, KIND_PLANE, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER};

#[derive(Clone, Copy, Debug)]
pub struct VehicleDef {
    pub kind: u8,
    pub name: &'static str,
    /// Health of a fresh one. Zero on the wire means wrecked.
    pub hp: u8,
    /// Seat 0 is the driver (a helicopter's or a plane's pilot, and a walker's, who also aims its guns);
    /// a tank's, helicopter's and plane's seat 1 is the gunner.
    pub seats: usize,
    /// Half extents of the chassis box, metres.
    pub half_extents: [f32; 3],
    /// Has a turret or a mounted gun the gunner aims (`Vehicle::gun`).
    pub gun: bool,
    /// Has a plane's pilot weapons: forward guns that heat and jam, and bombs (`Vehicle::arms`).
    pub arms: bool,
    /// A car's four wheels, which can be bent and shot off. Then `track_half` and `axle_z` place them.
    pub car_wheels: bool,
    pub track_half: f32,
    pub axle_z: f32,
}

const JEEP: VehicleDef = VehicleDef {
    kind: KIND_JEEP,
    name: "Jeep",
    hp: 100,
    seats: CAR_SEATS,
    half_extents: JEEP_HALF_EXTENTS,
    gun: false,
    arms: false,
    car_wheels: true,
    track_half: JEEP_TRACK_HALF,
    axle_z: JEEP_AXLE_Z,
};

const TANK: VehicleDef = VehicleDef {
    kind: KIND_TANK,
    name: "Tank",
    hp: 250,
    seats: TANK_SEATS,
    half_extents: TANK_HALF_EXTENTS,
    gun: true,
    arms: false,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
};

const HELI: VehicleDef = VehicleDef {
    kind: KIND_HELI,
    name: "Helicopter",
    hp: 100,
    seats: HELI_SEATS,
    half_extents: HELI_HALF_EXTENTS,
    gun: true,
    arms: false,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
};

const SPORTS: VehicleDef = VehicleDef {
    kind: KIND_SPORTS,
    name: "Sports car",
    hp: 100,
    seats: SPORTS_SEATS,
    half_extents: SPORTS_HALF_EXTENTS,
    gun: false,
    arms: false,
    car_wheels: true,
    track_half: SPORTS_TRACK_HALF,
    axle_z: SPORTS_AXLE_Z,
};

const PLANE: VehicleDef = VehicleDef {
    kind: KIND_PLANE,
    name: "Triplane",
    hp: 70,
    seats: PLANE_SEATS,
    half_extents: PLANE_HALF_EXTENTS,
    gun: true,
    arms: true,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
};

const WALKER: VehicleDef = VehicleDef {
    kind: KIND_WALKER,
    name: "Walker",
    hp: 200,
    seats: WALKER_SEATS,
    half_extents: WALKER_HALF_EXTENTS,
    gun: true,
    arms: false,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
};

const STRV: VehicleDef = VehicleDef {
    kind: KIND_STRV,
    name: "Strv S",
    hp: 220,
    seats: STRV_SEATS,
    half_extents: STRV_HALF_EXTENTS,
    gun: true,
    arms: false,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
};

const MORTAR: VehicleDef = VehicleDef {
    kind: KIND_MORTAR,
    name: "Field mortar",
    hp: 150,
    seats: MORTAR_SEATS,
    half_extents: MORTAR_HALF_EXTENTS,
    gun: true,
    arms: false,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
};

/// Every vehicle kind, in id order.
pub const VEHICLES: [VehicleDef; 8] = [JEEP, TANK, HELI, SPORTS, PLANE, WALKER, STRV, MORTAR];

/// The seat whose occupant aims and fires `kind`'s gun: a tank's, helicopter's and plane's seat 1, but
/// the driver's seat 0 where the vehicle has only one (a walker, a Strv S).
pub fn gun_seat(kind: u8) -> usize {
    if vehicle_def(kind).seats == 1 { 0 } else { 1 }
}

/// The definition of a vehicle `kind`. Unknown kinds are jeeps.
pub fn vehicle_def(kind: u8) -> &'static VehicleDef {
    VEHICLES.iter().find(|def| def.kind == kind).unwrap_or(&VEHICLES[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_distinct_definition() {
        for (index, def) in VEHICLES.iter().enumerate() {
            assert!(def.seats >= 1 && def.seats <= CAR_SEATS, "{} fits the seat array", def.name);
            assert!(def.hp > 0, "{} starts alive", def.name);
            assert_eq!(vehicle_def(def.kind).kind, def.kind);
            assert!(VEHICLES[index + 1..].iter().all(|other| other.kind != def.kind), "{} id is unique", def.name);
        }
    }

    #[test]
    fn only_armed_kinds_have_a_gun_and_only_the_plane_has_pilot_arms() {
        assert!(vehicle_def(KIND_PLANE).arms && !vehicle_def(KIND_TANK).arms);
        assert!(vehicle_def(KIND_TANK).gun && vehicle_def(KIND_HELI).gun && vehicle_def(KIND_PLANE).gun && vehicle_def(KIND_WALKER).gun && vehicle_def(KIND_STRV).gun);
        assert!(!vehicle_def(KIND_JEEP).gun && !vehicle_def(KIND_SPORTS).gun);
    }
}
