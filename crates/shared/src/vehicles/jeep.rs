//! The jeep: everything the game knows about it.

use super::*;

pub(super) const JEEP: VehicleDef = VehicleDef {
    kind: KIND_JEEP,
    name: "Jeep",
    hp: 100,
    seats: CAR_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Open,
    seat_heads: &JEEP_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: JEEP_HALF_EXTENTS,
        // The box is 6.6 m³, so a jeep weighs about 1.3 tonnes. Big wheels and a soft top speed: it
        // gets there slowly but clears kerbs the sports car would ground on.
        density: 200.0,
        wheel_radius: JEEP_WHEEL_RADIUS,
        track_half: JEEP_TRACK_HALF,
        axles: &[-JEEP_AXLE_Z, JEEP_AXLE_Z],
        hard_point_y: JEEP_HARD_POINT_Y,
        suspension_rest: JEEP_SUSPENSION_REST,
        max_suspension_force: 60_000.0,
        engine_force: 10_000.0,
        max_speed: JEEP_MAX_SPEED,
        steer_lock: JEEP_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.15,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
    },
    armor: LIGHT,
};
