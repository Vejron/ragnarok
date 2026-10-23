//! The loitering drone: everything the game knows about it.

use super::*;

/// It cannot hover, so it starts out far up and back, already cruising; shot down, it pops about as
/// big as a tank shell's burst.
const LOITERING: DroneDef = DroneDef {
    energy_ticks: LOITER_ENERGY_TICKS,
    range: LOITER_RANGE,
    blast_radius: LOITER_BLAST_RADIUS,
    damage: LOITER_DAMAGE,
    knock: LOITER_KNOCK,
    pop_radius: LOITER_POP_RADIUS,
    pop_damage: LOITER_POP_DAMAGE,
    impact_dv: LOITER_IMPACT_DV,
    proximity: LOITER_PROXIMITY,
    fall_ticks: LOITER_FALL_TICKS,
    weapon: WEAPON_LOITER,
    pop_shown_as: WEAPON_SHELL,
    launch: Launch::Behind { back: LOITER_LAUNCH_BACK, up: LOITER_LAUNCH_UP, speed: LOITER_SPEED },
};

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
    drone: Some(LOITERING),
    mount_seat: None,
};
