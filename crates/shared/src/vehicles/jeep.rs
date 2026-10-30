//! The jeep: everything the game knows about it.

use super::*;

/// Cars come in two kinds, which share the ray-cast wheel physics and differ in these numbers.
/// The jeep (`KIND_JEEP`) is tall on big wheels with plenty of ground clearance: slower, but it
/// rides over kerbs and rough ground. Half extents of the chassis box, metres.
pub const JEEP_HALF_EXTENTS: [f32; 3] = [1.0, 0.42, 1.95];
pub const JEEP_WHEEL_RADIUS: f32 = 0.55;
/// Wheel hard points relative to the chassis centre: half the track, and the distance of the
/// axles from the centre along Z. Front axle is at -Z.
pub const JEEP_TRACK_HALF: f32 = 0.9;
pub const JEEP_AXLE_Z: f32 = 1.25;
pub const JEEP_HARD_POINT_Y: f32 = 0.0;
pub const JEEP_SUSPENSION_REST: f32 = 0.4;
/// Height of the chassis centre above flat ground once the suspension has settled (measured by
/// `ride_heights_differ_between_cars` in vehicle.rs). The clearance under the box is this minus the half height.
pub const JEEP_RIDE_HEIGHT: f32 = 0.857;
/// Largest steering angle of the front wheels, radians. The wire carries it as -127..=127.
pub const JEEP_MAX_STEER: f32 = 0.5;
/// Forward speed above which engine force fades to zero, m/s.
pub const JEEP_MAX_SPEED: f32 = 32.0;
/// Seated players. Each seat of a vehicle with open seats has a head point in the hull frame, where
/// the client draws the sitting body's head (a car's `seat_N` markers, built by `build_car.py`). The
/// body that can be shot is a short upright capsule hanging below it: head and shoulders down to the
/// waist, so the hull box in front of the lower body takes the rounds aimed there.
pub const JEEP_SEAT_HEADS: [[f32; 3]; 4] = [[-0.4, 0.62, -0.3], [0.4, 0.62, -0.3], [-0.4, 0.62, 0.75], [0.4, 0.62, 0.75]];

/// A jeep's engine under the bonnet and its fuel tank at the back; its crew sit in the open, hit as anyone is.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.4], [0.45, 0.3, 0.4], 120),
        Module::new(Part::Fuel, [0.0, -0.15, 1.6], [0.4, 0.15, 0.25], 100),
    ],
    canvas: false,
};

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
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};
