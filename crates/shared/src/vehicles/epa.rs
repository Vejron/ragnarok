//! The EPA traktor: everything the game knows about it.

use super::*;

/// The EPA traktor (`KIND_EPA`): an old boxy saloon cut down into a two-seat pickup, as Swedish teenagers
/// drive them. The back doors are welded shut and the roof ends behind the front seats, where a wall with
/// a window and a slow-vehicle triangle closes the cab off from a short bed. Its gearbox is locked so that
/// it can go no faster than 30 km/h, but it keeps the saloon's big engine, so it gets there briskly. A
/// driver and a passenger sit behind glass.
pub const EPA_SEATS: usize = 2;
pub const EPA_HALF_EXTENTS: [f32; 3] = [0.87, 0.3, 2.38];
/// Low-profile tyres on big aftermarket rims, metres.
pub const EPA_WHEEL_RADIUS: f32 = 0.32;
pub const EPA_TRACK_HALF: f32 = 0.74;
pub const EPA_AXLE_Z: f32 = 1.385;
pub const EPA_HARD_POINT_Y: f32 = 0.0;
pub const EPA_SUSPENSION_REST: f32 = 0.22;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const EPA_RIDE_HEIGHT: f32 = 0.447;
pub const EPA_MAX_STEER: f32 = 0.55;
/// 30 km/h, the most the law lets an EPA traktor do, m/s.
pub const EPA_MAX_SPEED: f32 = 8.33;
/// The driver on the left and the passenger beside them, under the front of the roof.
pub const EPA_SEAT_HEADS: [[f32; 3]; 2] = [[-0.38, 0.75, 0.1], [0.38, 0.75, 0.1]];

/// The engine under the bonnet and the fuel tank under the bed; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.05, -1.6], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.15, 1.6], [0.45, 0.12, 0.3], 90),
    ],
    canvas: false,
};

pub(super) const EPA: VehicleDef = VehicleDef {
    kind: KIND_EPA,
    name: "EPA traktor",
    hp: 100,
    seats: EPA_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &EPA_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: EPA_HALF_EXTENTS,
        // 5.0 m³ of box is about 1.3 tonnes, an old saloon's weight.
        density: 260.0,
        wheel_radius: EPA_WHEEL_RADIUS,
        track_half: EPA_TRACK_HALF,
        axles: &[-EPA_AXLE_Z, EPA_AXLE_Z],
        hard_point_y: EPA_HARD_POINT_Y,
        suspension_rest: EPA_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 6 m/s² away from a standstill: a saloon's engine in a low gear, so it is at its top speed
        // in under two seconds.
        engine_force: 8_000.0,
        max_speed: EPA_MAX_SPEED,
        steer_lock: EPA_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.15,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        // The underside rises from each axle out to the bumper.
        sweep: 0.9,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Far too slow to skim: it sinks and wades along the bed.
        skim: None,
        swim: None,
        bow: 0.0,
        profile: Spec::BOX,
        springs: Springs::FIRM,
        airframe: None,
        rotor: None,
        gait: None,
        arms: None,
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};
