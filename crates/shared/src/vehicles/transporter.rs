//! The VW Transporter: everything the game knows about it.

use super::*;

/// The Volkswagen Transporter T3 (`KIND_TRANSPORTER`): a flat-fronted van with its engine under the
/// floor at the back. Its driver sits over the front wheels with one passenger beside them, then a
/// bench of two by the sliding door and a bench of three at the back. Tall and slow, the slowest
/// car on the road but the EPA. Seven seats behind glass.
pub const TRANSPORTER_SEATS: usize = 7;
pub const TRANSPORTER_HALF_EXTENTS: [f32; 3] = [0.92, 0.5, 2.25];
pub const TRANSPORTER_WHEEL_RADIUS: f32 = 0.33;
pub const TRANSPORTER_TRACK_HALF: f32 = 0.785;
pub const TRANSPORTER_AXLE_Z: f32 = 1.23;
pub const TRANSPORTER_HARD_POINT_Y: f32 = 0.0;
pub const TRANSPORTER_SUSPENSION_REST: f32 = 0.486;
/// Measured by `ride_heights_differ_between_cars` in vehicle.rs.
pub const TRANSPORTER_RIDE_HEIGHT: f32 = 0.685;
pub const TRANSPORTER_MAX_STEER: f32 = 0.6;
/// Flat out, m/s.
pub const TRANSPORTER_MAX_SPEED: f32 = 30.0;
/// The driver on the left, then the passengers, front to back.
pub const TRANSPORTER_SEAT_HEADS: [[f32; 3]; 7] = [[-0.45, 0.87, -1.25], [0.45, 0.87, -1.25], [-0.4, 0.87, 0.05], [0.4, 0.87, 0.05], [-0.5, 0.87, 1.15], [0.0, 0.87, 1.15], [0.5, 0.87, 1.15]];

/// The engine under the floor at the back and the fuel tank under the middle of the floor; the crew sit behind glass.
const ARMOR: Armor = Armor {
    plates: SHEET.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, -0.1, 1.7], [0.4, 0.25, 0.4], 110),
        Module::new(Part::Fuel, [0.0, -0.3, 0.2], [0.4, 0.12, 0.3], 90),
    ],
    canvas: false,
};

pub(super) const TRANSPORTER: VehicleDef = VehicleDef {
    kind: KIND_TRANSPORTER,
    name: "VW Transporter",
    hp: 100,
    seats: TRANSPORTER_SEATS,
    mounts: &[],
    car_wheels: true,
    crew: Exposure::Glazed,
    seat_heads: &TRANSPORTER_SEAT_HEADS,
    crew_guns: true,
    body: CAR,
    spec: Spec {
        half_extents: TRANSPORTER_HALF_EXTENTS,
        // 8.3 m³ of box, mostly air, is about 1.6 tonnes.
        density: 193.0,
        wheel_radius: TRANSPORTER_WHEEL_RADIUS,
        track_half: TRANSPORTER_TRACK_HALF,
        axles: &[-TRANSPORTER_AXLE_Z, TRANSPORTER_AXLE_Z],
        hard_point_y: TRANSPORTER_HARD_POINT_Y,
        suspension_rest: TRANSPORTER_SUSPENSION_REST,
        max_suspension_force: 50_000.0,
        // About 4 m/s² away from a standstill: loaded, it crawls.
        engine_force: 6_400.0,
        max_speed: TRANSPORTER_MAX_SPEED,
        steer_lock: TRANSPORTER_MAX_STEER,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        // Low enough that it stays on its wheels at full lock flat out (measured in vehicle/tests.rs).
        com_drop: 0.4,
        tank: Some(BOOST_FULL),
        buttons: Buttons::Boost,
        boost: Some(Boost::CAR),
        aims: false,
        solids: &[],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        // The underside rises from each axle out to the bumper.
        sweep: 0.6,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        // Too heavy and too slow to skim: it sinks.
        skim: None,
        swim: None,
        bow: 0.0,
        profile: Spec::BOX,
        // Soft, with a long travel: it pitches and rolls.
        springs: Springs { stiffness: 18.0, compression: 1.6, damping: 2.4, travel: 0.4 },
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
