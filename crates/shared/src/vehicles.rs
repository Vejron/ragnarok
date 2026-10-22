//! What each vehicle kind is: one definition per kind, in `vehicles/<kind>.rs`, holding everything
//! the game knows about it. That is its facts (health, seats), its crew, its weapons as `mounts` (see
//! `crate::mounts`), how it stands and meets the world (`Body`), what the physics makes of it (`Spec`,
//! which `crates/sim` runs on the server and in the client's prediction) and its armour.
//!
//! Adding a kind: its `KIND_*` id in `consts.rs` (which the client's copy is generated from), its
//! file here and its place in `VEHICLES`, and its presentation on the client, `client/src/kinds/`,
//! with its model. Nothing in the server, the sim or the client branches on a kind: what a kind does
//! is its fields. A new way of moving is a `Lift` and its system in `crates/sim`. The client's table is built from `client/src/sim/tables.generated.json`, which
//! `UPDATE_PARITY=1 cargo test -p shared parity` regenerates from this one.

use crate::consts::*;
use crate::mounts::{Aim, Ammo, Fire, MountDef, Muzzle, Need, Refill, Show};
use crate::items::*;
use crate::protocol::{
    KIND_DRONE, KIND_HELI, KIND_HEN, KIND_JEEP, KIND_LOITER, KIND_MORTAR, KIND_PLANE, KIND_ROADSTER, KIND_SPORTS, KIND_STRV, KIND_TANK, KIND_WALKER, PROJECTILE_BOMB, PROJECTILE_CANNON,
    PROJECTILE_EGG, PROJECTILE_MORTAR, PROJECTILE_SHELL, WEAPON_AUTOCANNON, WEAPON_DRONE, WEAPON_LOITER, WEAPON_MG, WEAPON_SHELL,
};

mod drone;
mod heli;
mod hen;
mod jeep;
mod loiter;
mod mortar;
mod plane;
mod roadster;
mod spec;
mod sports;
mod strv;
mod tank;
mod walker;

pub use spec::{Buttons, Lift, Role, Shape, Solid, Spec};

#[derive(Clone, Copy, Debug)]
pub struct VehicleDef {
    pub kind: u8,
    pub name: &'static str,
    /// Health of a fresh one. Zero on the wire means wrecked.
    pub hp: u8,
    /// Seat 0 is the driver (a helicopter's or a plane's pilot, and a walker's, who also aims its guns);
    /// a tank's, helicopter's and plane's seat 1 is the gunner.
    pub seats: usize,
    /// What it fires, worked in this order every tick (`crate::mounts`).
    pub mounts: &'static [MountDef],
    /// A car's four wheels, which can be bent and shot off. Then `track_half` and `axle_z` place them.
    pub car_wheels: bool,
    /// Whether its crew can be shot, and through what.
    pub crew: Exposure,
    /// Each seat's head point in the hull frame, for kinds whose crew show (`crew` is not `Sealed`).
    /// The mortar's one seat is on its turning mount instead: `MORTAR_SEAT_HEAD`.
    pub seat_heads: &'static [[f32; 3]],
    /// Its passengers (every seat but the driver's) can fire the guns in their own hotbar.
    pub crew_guns: bool,
    /// How it stands and meets the world.
    pub body: Body,
    /// What the physics makes of it.
    pub spec: Spec,
    /// What a shot does to it, by where it lands and at what angle.
    pub armor: Armor,
    /// For a drone, flown from afar by whoever launched it: how it flies, strikes and is launched.
    pub drone: Option<DroneDef>,
    /// Its one seat is on its turning mount rather than in the hull: the mortar's.
    pub mount_seat: Option<MountSeat>,
}

/// A seat on a mount that turns with the gun.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountSeat {
    /// Where the mount turns, in the hull frame.
    pub pivot: [f32; 3],
    /// The occupant's head, from the pivot, with the gun pointing ahead.
    pub head: [f32; 3],
}

/// What a drone kind does in the `Drones` mechanic: how long and how far it flies, and its charge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DroneDef {
    pub energy_ticks: u32,
    pub range: f32,
    pub blast_radius: f32,
    pub damage: f32,
    pub knock: f32,
    pub pop_radius: f32,
    pub pop_damage: f32,
    pub impact_dv: f32,
    pub proximity: f32,
    pub fall_ticks: u32,
    /// What its blast is credited as.
    pub weapon: u8,
    /// How its pop is drawn when it is shot down: as the burst of a round about its size.
    pub pop_shown_as: u8,
    pub launch: Launch,
}

/// Where a drone starts, from its pilot's eye and look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Launch {
    /// `ahead` metres along the look, or short of a wall there, and `up` above the eye.
    Ahead { ahead: f32, up: f32 },
    /// `back` metres behind the eye and `up` above it, already flying along the look at `speed`: it
    /// comes in over its pilot's head.
    Behind { back: f32, up: f32, speed: f32 },
}

impl VehicleDef {
    /// Half extents of the chassis box, metres.
    pub fn half_extents(&self) -> [f32; 3] {
        self.spec.half_extents
    }

    /// How far either side of the centre line its wheels are, metres.
    pub fn track_half(&self) -> f32 {
        self.spec.track_half
    }

    /// Whether its turret can be shot off: it has a `Role::Turret` solid.
    pub fn sheds_turret(&self) -> bool {
        self.spec.solids.iter().any(|solid| solid.role == Role::Turret)
    }

    /// How far fore and aft of the centre a car's axles are, metres.
    pub fn axle_z(&self) -> f32 {
        self.spec.axles.last().copied().unwrap_or(0.0)
    }

    /// Whether the snapshot carries a turret for it (the `turret` component).
    pub fn gun(&self) -> bool {
        self.mounts.iter().any(|mount| mount.show == Show::Turret)
    }

    /// Whether the snapshot carries pilot arms for it (the `arms` component): guns that heat and jam, and
    /// bombs. A hen's eggs are its bombs.
    pub fn arms(&self) -> bool {
        self.mounts.iter().any(|mount| mount.show.arms())
    }
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
    /// Its name in `client/src/sim/tables.generated.json`.
    pub fn name(self) -> &'static str {
        match self {
            Exposure::Sealed => "sealed",
            Exposure::Open => "open",
            Exposure::Glazed => "glazed",
        }
    }
}

/// How a kind stands and meets the world, as the game sees it (its physics are `crates/sim`'s `Spec`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// How high over the ground its chassis centre starts, a little above where it comes to rest.
    pub drop_y: f32,
    /// How far its legs reach below the hull, 0 for none: it is boarded from between its feet, and
    /// stood back up on them.
    pub legs: f32,
    /// Too heavy for a tree to stop: it goes through trunks, felling them (and the physics leaves them
    /// out of what it touches).
    pub heavy: bool,
    /// How hard it rams a building, against a car's 1.
    pub ram: f32,
    /// Crash damage it takes, against a car's 1.
    pub crash: f32,
    /// Flies: a tree it fells does not slow it.
    pub flies: bool,
    /// Nobody drives it: it rolls when towed, holds its brake when not, and can be hitched.
    pub towed: bool,
}

const CAR: Body = Body { drop_y: 1.0, legs: 0.0, heavy: false, ram: 1.0, crash: 1.0, flies: false, towed: false };
/// A tank's armour takes a fifth of what a car would, and it rams with three times a car's weight.
const TRACKS: Body = Body { heavy: true, ram: 3.0, crash: 0.2, ..CAR };
const ROTOR: Body = Body { flies: true, ..CAR };
/// A plane is wood and canvas: it takes twice the crash damage, so flying it into anything at speed
/// is the end of it.
const WINGS: Body = Body { drop_y: 1.5, crash: 2.0, flies: true, ..CAR };
/// A walker's or a hen's hull starts as high as its legs hold it up.
const LEGS: Body = Body { drop_y: WALKER_LEG_LENGTH + WALKER_HALF_EXTENTS[1], legs: WALKER_LEG_LENGTH, ..CAR };
const TOWED: Body = Body { towed: true, ..CAR };

/// A vehicle's armour. A hit is judged in the hull's frame: the side it lands on is the zone, in the
/// order front, side, back and top (the server's `armor.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Armor {
    /// How far the front plate leans back from upright, radians.
    pub front_slope: f32,
    /// Incidence beyond which a shot glances off, by zone (degrees).
    pub ricochet: [f32; 4],
    /// Damage multiplier by zone.
    pub mult: [f32; 4],
    pub weak: &'static [WeakSpot],
    /// Canvas and wood, which a machine gun's rounds tear through: they deal their `plane` damage.
    pub canvas: bool,
}

/// A box on the hull where a hit does extra: an engine deck, a roof hatch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeakSpot {
    pub centre: [f32; 3],
    pub half: [f32; 3],
    pub mult: f32,
}

/// Cars, the helicopter and the walker: only the most grazing hit glances off.
const LIGHT: Armor = Armor { front_slope: 0.0, ricochet: [82.0, 82.0, 82.0, 82.0], mult: [1.0; 4], weak: &[], canvas: false };
/// Canvas and wood: nothing glances off.
const SOFT: Armor = Armor { front_slope: 0.0, ricochet: [91.0; 4], mult: [1.0; 4], weak: &[], canvas: true };

/// Every vehicle kind, in id order.
pub static VEHICLES: &[VehicleDef] = &[
    jeep::JEEP,
    tank::TANK,
    heli::HELI,
    sports::SPORTS,
    plane::PLANE,
    walker::WALKER,
    strv::STRV,
    mortar::MORTAR,
    roadster::ROADSTER,
    drone::DRONE,
    hen::HEN,
    loiter::LOITER,
];

/// Where each kind id's definition is in `VEHICLES`: 0, the jeep's, for ids that are not a vehicle.
static BY_KIND: [u8; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < VEHICLES.len() {
        table[VEHICLES[index].kind as usize] = index as u8;
        index += 1;
    }
    table
};

/// The seat whose occupant aims and fires `kind`'s gun: a tank's, helicopter's and plane's seat 1, but
/// the driver's seat 0 where the vehicle has only one (a walker, a Strv S).
pub fn gun_seat(kind: u8) -> usize {
    if vehicle_def(kind).seats == 1 { 0 } else { 1 }
}

/// Where seat `index` of a `kind` puts its occupant's head, in the hull frame, with a seat on a
/// turning mount turned to `gun_yaw`. `None` for a crew that does not show.
pub fn seat_head(kind: u8, index: usize, gun_yaw: f32) -> Option<[f32; 3]> {
    let def = vehicle_def(kind);
    if def.crew == Exposure::Sealed {
        return None;
    }
    if let Some(MountSeat { pivot: [px, py, pz], head: [x, y, z] }) = def.mount_seat {
        let (sin, cos) = gun_yaw.sin_cos();
        return (index == 0).then_some([px + x * cos + z * sin, py + y, pz - x * sin + z * cos]);
    }
    def.seat_heads.get(index).copied()
}

/// Whether seat `index` of a `kind` fires its occupant's own guns: a car's passengers.
pub fn crew_gun(kind: u8, index: usize) -> bool {
    vehicle_def(kind).crew_guns && index != 0
}

/// What drone `kind` does: an FPV drone's for a kind that is not a drone.
pub fn drone_def(kind: u8) -> &'static DroneDef {
    vehicle_def(kind).drone.as_ref().unwrap_or(drone::FPV)
}

/// The definition of a vehicle `kind`. Unknown kinds are jeeps.
pub fn vehicle_def(kind: u8) -> &'static VehicleDef {
    &VEHICLES[BY_KIND[kind as usize] as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_distinct_definition() {
        for (index, def) in VEHICLES.iter().enumerate() {
            assert!(index == 0 || def.kind > VEHICLES[index - 1].kind, "{} is in id order", def.name);
            assert!(def.seats >= 1 && def.seats <= CAR_SEATS, "{} fits the seat array", def.name);
            assert!(def.hp > 0, "{} starts alive", def.name);
            assert_eq!(vehicle_def(def.kind).kind, def.kind);
            if def.crew != Exposure::Sealed && def.mount_seat.is_none() {
                assert_eq!(def.seat_heads.len(), def.seats, "{} places every seat's head", def.name);
            }
            assert!(!def.crew_guns || def.crew != Exposure::Sealed, "{} passengers who shoot can be shot", def.name);
            assert!(VEHICLES[index + 1..].iter().all(|other| other.kind != def.kind), "{} id is unique", def.name);
        }
    }

    #[test]
    fn only_armed_kinds_have_a_gun_and_only_the_plane_and_the_hen_have_pilot_arms() {
        assert!(vehicle_def(KIND_PLANE).arms() && vehicle_def(KIND_HEN).arms() && !vehicle_def(KIND_TANK).arms());
        assert!(vehicle_def(KIND_TANK).gun() && vehicle_def(KIND_HELI).gun() && vehicle_def(KIND_PLANE).gun() && vehicle_def(KIND_WALKER).gun() && vehicle_def(KIND_STRV).gun());
        assert!(!vehicle_def(KIND_JEEP).gun() && !vehicle_def(KIND_SPORTS).gun() && !vehicle_def(KIND_ROADSTER).gun() && !vehicle_def(KIND_HEN).gun());
    }
}
