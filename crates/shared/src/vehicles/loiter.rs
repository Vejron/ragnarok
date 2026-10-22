//! The loitering drone: everything the game knows about it.

use super::*;

/// Takes a few rifle rounds to bring down.
pub(super) const LOITER: VehicleDef = VehicleDef {
    kind: KIND_LOITER,
    name: "Loitering drone",
    hp: 40,
    seats: LOITER_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: ROTOR,
    spec: Spec {
        half_extents: LOITER_HALF_EXTENTS,
        // A box 2.5 x 0.5 x 3.5 m around the wing, about 200 kg. Nothing below uses the wheel fields.
        density: 45.0,
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
        lift: Lift::Loiter,
        linear_damping: LOITER_DAMPING,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[],
    },
    armor: LIGHT,
};
