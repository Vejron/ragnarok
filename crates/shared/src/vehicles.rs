//! What each vehicle kind is, in one table: the gameplay facts both the server and the client need
//! (health, seats, size, what it carries). The physics tuning stays in the server's `Spec`s and the
//! client's `sim/vehicle.ts`; a kind's wheel layout and extents are the constants in `consts.rs`.
//!
//! Adding a kind: a `KIND_*` id in `protocol.rs`, its constants in `consts.rs`, a `VehicleDef` here
//! and in `client/src/sim/vehicles.ts`, then its physics `Spec` and model. `fixtures/parity/
//! gameplay.json` fails until the two tables agree.

use crate::consts::*;
use crate::protocol::{KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER};

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
    /// Has a plane's pilot weapons: forward guns that heat and jam, and bombs (`Vehicle::arms`). A hen's
    /// eggs are its bombs.
    pub arms: bool,
    /// A car's four wheels, which can be bent and shot off. Then `track_half` and `axle_z` place them.
    pub car_wheels: bool,
    pub track_half: f32,
    pub axle_z: f32,
    /// Whether its crew can be shot, and through what.
    pub crew: Exposure,
    /// Each seat's head point in the hull frame, for kinds whose crew show (`crew` is not `Sealed`).
    /// The mortar's one seat is on its turning mount instead: `MORTAR_SEAT_HEAD`.
    pub seat_heads: &'static [[f32; 3]],
    /// Its passengers (every seat but the driver's) can fire the guns in their own hotbar.
    pub crew_guns: bool,
}

/// How exposed a vehicle's crew are to gunfire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exposure {
    /// Inside armour: only the vehicle can be hit.
    Sealed,
    /// Out in the open above the hull: the hull box still stops the rounds that meet it first.
    Open,
    /// Behind glass: rounds that meet the hull box can go on to hit the crew inside it.
    Glazed,
}

impl Exposure {
    /// Its name in `fixtures/parity/gameplay.json`.
    pub fn name(self) -> &'static str {
        match self {
            Exposure::Sealed => "sealed",
            Exposure::Open => "open",
            Exposure::Glazed => "glazed",
        }
    }
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
    crew: Exposure::Open,
    seat_heads: &JEEP_SEAT_HEADS,
    crew_guns: true,
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
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
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
    crew: Exposure::Glazed,
    seat_heads: &HELI_SEAT_HEADS,
    crew_guns: false,
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
    crew: Exposure::Open,
    seat_heads: &SPORTS_SEAT_HEADS,
    crew_guns: true,
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
    crew: Exposure::Open,
    seat_heads: &PLANE_SEAT_HEADS,
    crew_guns: false,
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
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
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
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
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
    crew: Exposure::Open,
    seat_heads: &[],
    crew_guns: false,
};

const ROADSTER: VehicleDef = VehicleDef {
    kind: KIND_ROADSTER,
    name: "Roadster",
    hp: 100,
    seats: ROADSTER_SEATS,
    half_extents: ROADSTER_HALF_EXTENTS,
    gun: false,
    arms: false,
    car_wheels: true,
    track_half: ROADSTER_TRACK_HALF,
    axle_z: ROADSTER_AXLE_Z,
    crew: Exposure::Open,
    seat_heads: &ROADSTER_SEAT_HEADS,
    crew_guns: true,
};

/// One hit brings it down.
const DRONE: VehicleDef = VehicleDef {
    kind: KIND_DRONE,
    name: "FPV drone",
    hp: 1,
    seats: DRONE_SEATS,
    half_extents: DRONE_HALF_EXTENTS,
    gun: false,
    arms: false,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
};

/// The walker's legs and hull, with wings and eggs in place of guns.
const HEN: VehicleDef = VehicleDef {
    kind: KIND_HEN,
    name: "Hen",
    hp: 180,
    seats: HEN_SEATS,
    half_extents: HEN_HALF_EXTENTS,
    gun: false,
    arms: true,
    car_wheels: false,
    track_half: 0.0,
    axle_z: 0.0,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
};

/// Every vehicle kind, in id order.
pub const VEHICLES: [VehicleDef; 11] = [JEEP, TANK, HELI, SPORTS, PLANE, WALKER, STRV, MORTAR, ROADSTER, DRONE, HEN];

/// The seat whose occupant aims and fires `kind`'s gun: a tank's, helicopter's and plane's seat 1, but
/// the driver's seat 0 where the vehicle has only one (a walker, a Strv S).
pub fn gun_seat(kind: u8) -> usize {
    if vehicle_def(kind).seats == 1 { 0 } else { 1 }
}

/// Where seat `index` of a `kind` puts its occupant's head, in the hull frame, with the mortar's mount
/// turned to `gun_yaw`. `None` for a crew that does not show.
pub fn seat_head(kind: u8, index: usize, gun_yaw: f32) -> Option<[f32; 3]> {
    let def = vehicle_def(kind);
    if def.crew == Exposure::Sealed {
        return None;
    }
    if kind == KIND_MORTAR {
        let [x, y, z] = MORTAR_SEAT_HEAD;
        let (sin, cos) = gun_yaw.sin_cos();
        let [px, py, pz] = MORTAR_GUN_PIVOT;
        return (index == 0).then_some([px + x * cos + z * sin, py + y, pz - x * sin + z * cos]);
    }
    def.seat_heads.get(index).copied()
}

/// Whether seat `index` of a `kind` fires its occupant's own guns: a car's passengers.
pub fn crew_gun(kind: u8, index: usize) -> bool {
    vehicle_def(kind).crew_guns && index != 0
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
            if def.crew != Exposure::Sealed && def.kind != KIND_MORTAR {
                assert_eq!(def.seat_heads.len(), def.seats, "{} places every seat's head", def.name);
            }
            assert!(!def.crew_guns || def.crew != Exposure::Sealed, "{} passengers who shoot can be shot", def.name);
            assert!(VEHICLES[index + 1..].iter().all(|other| other.kind != def.kind), "{} id is unique", def.name);
        }
    }

    #[test]
    fn only_armed_kinds_have_a_gun_and_only_the_plane_and_the_hen_have_pilot_arms() {
        assert!(vehicle_def(KIND_PLANE).arms && vehicle_def(KIND_HEN).arms && !vehicle_def(KIND_TANK).arms);
        assert!(vehicle_def(KIND_TANK).gun && vehicle_def(KIND_HELI).gun && vehicle_def(KIND_PLANE).gun && vehicle_def(KIND_WALKER).gun && vehicle_def(KIND_STRV).gun);
        assert!(!vehicle_def(KIND_JEEP).gun && !vehicle_def(KIND_SPORTS).gun && !vehicle_def(KIND_ROADSTER).gun && !vehicle_def(KIND_HEN).gun);
    }
}
