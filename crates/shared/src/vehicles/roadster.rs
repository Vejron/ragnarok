//! The Roadster: everything the game knows about it.

use super::*;

pub(super) const ROADSTER: VehicleDef = VehicleDef {
    kind: KIND_ROADSTER,
    name: "Roadster",
    hp: 100,
    seats: ROADSTER_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &ROADSTER_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: ROADSTER_HALF_EXTENTS,
        // The sports car's body with a bigger motor; its thrusters are forces of their own (see `thrust`).
        density: 200.0,
        wheel_radius: ROADSTER_WHEEL_RADIUS,
        track_half: ROADSTER_TRACK_HALF,
        axles: &[-ROADSTER_AXLE_Z, ROADSTER_AXLE_Z],
        hard_point_y: ROADSTER_HARD_POINT_Y,
        suspension_rest: ROADSTER_SUSPENSION_REST,
        max_suspension_force: 60_000.0,
        engine_force: 18_000.0,
        max_speed: ROADSTER_MAX_SPEED,
        steer_lock: ROADSTER_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.02,
        tank: Some(ROADSTER_AIR),
        buttons: Buttons::Stunt,
        aims: false,
        solids: &[],
        tongue: 0.0,
    },
    armor: LIGHT,
    drone: None,
    mount_seat: None,
};
