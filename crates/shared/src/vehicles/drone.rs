//! The FPV drone: everything the game knows about it.

use super::*;

/// One shot down pops about as big as an autocannon round's burst.
pub(super) const FPV: &DroneDef = &DroneDef {
    energy_ticks: DRONE_ENERGY_TICKS,
    range: DRONE_RANGE,
    blast_radius: DRONE_BLAST_RADIUS,
    damage: DRONE_DAMAGE,
    knock: DRONE_KNOCK,
    pop_radius: DRONE_POP_RADIUS,
    pop_damage: DRONE_POP_DAMAGE,
    impact_dv: DRONE_IMPACT_DV,
    proximity: DRONE_PROXIMITY,
    fall_ticks: DRONE_FALL_TICKS,
    weapon: WEAPON_DRONE,
    pop_shown_as: WEAPON_AUTOCANNON,
    launch: Launch::Ahead { ahead: DRONE_LAUNCH_AHEAD, up: DRONE_LAUNCH_UP },
};

/// One hit brings it down.
pub(super) const DRONE: VehicleDef = VehicleDef {
    kind: KIND_DRONE,
    name: "FPV drone",
    hp: 1,
    seats: DRONE_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: ROTOR,
    spec: Spec {
        half_extents: DRONE_HALF_EXTENTS,
        // 0.025 m³ of airframe and battery, about 1.5 kg. Nothing below uses the wheel fields.
        density: 60.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: 0.0,
        steer_lock: 0.0,
        turn_rate: 0.0,
        lift: Lift::Drone,
        linear_damping: DRONE_DAMPING,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[],
        tongue: 0.0,
    },
    armor: SHEET,
    drone: Some(*FPV),
    mount_seat: None,
};
