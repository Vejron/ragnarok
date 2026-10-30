//! The FPV drone: everything the game knows about it.

use super::*;

/// The FPV drone: a quadcopter the size of a dinner plate. It is flown by velocity: forward stick asks
/// for `DRONE_SPEED` along where its pilot looks (diving or climbing with the look's pitch), strafe for
/// `DRONE_STRAFE_SPEED` sideways, jump and run for `DRONE_CLIMB_SPEED` / `DRONE_SINK_SPEED` straight up
/// or down, and nothing held for a hover. The rotors close the gap at `DRONE_ACCEL_GAIN` per second, never
/// pushing harder than `DRONE_MAX_ACCEL` m/s² beyond holding it up, and the hull leans the way they push.
/// The look turns it: `DRONE_YAW_GAIN` rad/s per radian off, at most `DRONE_YAW_RATE`.
pub const DRONE_SEATS: usize = 1;
pub const DRONE_HALF_EXTENTS: [f32; 3] = [0.22, 0.06, 0.22];
pub const DRONE_SPEED: f32 = 30.0;
pub const DRONE_STRAFE_SPEED: f32 = 12.0;
pub const DRONE_CLIMB_SPEED: f32 = 10.0;
pub const DRONE_SINK_SPEED: f32 = 10.0;
pub const DRONE_ACCEL_GAIN: f32 = 2.0;
pub const DRONE_MAX_ACCEL: f32 = 18.0;
pub const DRONE_YAW_GAIN: f32 = 6.0;
pub const DRONE_YAW_RATE: f32 = 4.0;
/// Climbing fades out over the last 20 m below this height, and above it the rotors push down.
pub const DRONE_CEILING: f32 = 150.0;
/// How firmly the hull follows its lean (rad/s per unit of error), and how fast the angular velocity
/// follows that, per second.
pub const DRONE_LEVEL_GAIN: f32 = 8.0;
pub const DRONE_RESPONSE: f32 = 14.0;
/// Air drag on the body, which is all that slows a drone whose motors are off.
pub const DRONE_DAMPING: f32 = 0.1;

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
