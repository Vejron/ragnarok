//! The air ambulance: everything the game knows about it.

use super::*;

/// The air ambulance (`KIND_AIR_AMBULANCE`): a light twin-engined helicopter after the EC135, with
/// skids, a shrouded tail fan and no weapons. The pilot sits front right, as helicopter pilots do, with
/// one passenger beside them and two behind. It flies as the gunship does (`Lift::Rotor`), faster and
/// climbing slower. Same body axes as the car.
pub const AIR_AMBULANCE_SEATS: usize = 4;
/// Half the box round the cabin, from the skids' soles to the top of the windows, metres.
pub const AIR_AMBULANCE_HALF_EXTENTS: [f32; 3] = [0.85, 1.1, 2.55];
/// Half the engines' housing on the cabin's roof, and its middle in the hull frame, metres.
pub const AIR_AMBULANCE_HOUSING_HALF: [f32; 3] = [0.55, 0.3, 1.3];
pub const AIR_AMBULANCE_HOUSING_CENTRE: [f32; 3] = [0.0, 1.4, 1.1];
/// Half the tail boom and its middle, then half the fin round the fan and its middle, metres: the tail
/// reaches five metres past the cabin's box.
pub const AIR_AMBULANCE_BOOM_HALF: [f32; 3] = [0.18, 0.25, 1.7];
pub const AIR_AMBULANCE_BOOM_CENTRE: [f32; 3] = [0.0, 0.68, 4.2];
pub const AIR_AMBULANCE_FIN_HALF: [f32; 3] = [0.16, 1.2, 0.8];
pub const AIR_AMBULANCE_FIN_CENTRE: [f32; 3] = [0.0, 1.05, 6.7];
/// The main rotor's hub in the hull frame and its radius, then the tail fan's hub, metres.
pub const AIR_AMBULANCE_ROTOR_HUB: [f32; 3] = [0.0, 2.1, 0.7];
pub const AIR_AMBULANCE_ROTOR_RADIUS: f32 = 5.1;
pub const AIR_AMBULANCE_FAN_HUB: [f32; 3] = [0.0, 0.65, 6.55];
/// The lamp under the nose that is its searchlight, in the hull frame, metres.
pub const AIR_AMBULANCE_LAMP: [f32; 3] = [0.0, -0.5, -2.1];
/// The lever that hovers it out of ground effect, measured by
/// `the_air_ambulances_hover_lever_holds_it_out_of_ground_effect`.
pub const AIR_AMBULANCE_HOVER_LEVER: f32 = 0.56;
/// The pilot front right and a passenger front left, then two passengers behind them.
pub const AIR_AMBULANCE_SEAT_HEADS: [[f32; 3]; 4] = [[0.31, 0.72, -0.85], [-0.31, 0.72, -0.85], [-0.31, 0.74, 0.35], [0.31, 0.74, 0.35]];

/// How it flies: about 62 m/s flat out and a climb of about 8 m/s. Its blades' pitch gives way sooner to
/// a climb than the gunship's (`pitch_flow`), which is what limits it.
const FLIGHT: Rotorcraft = Rotorcraft {
    thrust: 1.8 * CAR_GRAVITY,
    hover_lever: AIR_AMBULANCE_HOVER_LEVER,
    pitch_flow: 20.0,
    tilt_flow: 0.25,
    hover_inflow: 14.0,
    radius: AIR_AMBULANCE_ROTOR_RADIUS,
    hub: 2.1,
    ground_effect: 0.15,
    etl_speed: HELI_ETL_SPEED,
    etl_gain: 0.1,
    vortex: 0.4,
    power: 2.6,
    profile: 0.15,
    spool: 0.3,
    inertia: 2.0,
    brake: 0.1,
    low_rotor: HELI_LOW_ROTOR,
    drag: [0.006, 0.003, 0.001],
    pitch_rate: 0.6,
    roll_rate: 0.9,
    rate_gain: 8.0,
    hold_gain: 3.0,
    cyclic_accel: 6.0,
    yaw_gain: 2.5,
    yaw_rate: 1.6,
    yaw_accel: 4.0,
    torque_yaw: 0.8,
    coordinate_speed: 10.0,
    bank_max: 0.7,
    bank_gain: 3.0,
    weathercock: 2.0,
    tail_arm: 6.55,
    flapback: 0.006,
    transverse: 0.25,
    stall_speed: 72.0,
    blade_stall: 0.3,
    lever_rate: HELI_LEVER_RATE,
    ceiling: HELI_CEILING,
};

/// The engines in the housing on the roof and the tanks under the floor; those aboard are behind glass,
/// hit as anyone is.
const ARMOR: Armor = Armor {
    plates: ALLOY.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 1.3, 1.1], [0.45, 0.3, 0.7], 110),
        Module::new(Part::Fuel, [0.0, -0.5, 0.3], [0.5, 0.15, 0.6], 90),
    ],
    canvas: false,
};

pub(super) const AIR_AMBULANCE: VehicleDef = VehicleDef {
    kind: KIND_AIR_AMBULANCE,
    name: "Air ambulance",
    hp: 100,
    seats: AIR_AMBULANCE_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Glazed,
    seat_heads: &AIR_AMBULANCE_SEAT_HEADS,
    // Nobody shoots from it: it has no guns, and those it carries keep theirs put away.
    crew_guns: false,
    // Its box is taller than the gunship's, so it starts higher to be clear of the ground.
    body: Body { drop_y: 1.3, ..ROTOR },
    spec: Spec {
        half_extents: AIR_AMBULANCE_HALF_EXTENTS,
        // 19.1 m³ of box is about 2.6 tonnes; the solids add no mass. Nothing below uses the wheel fields.
        density: 136.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: 0.0,
        steer_lock: 0.0,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: 0.0,
        lift: Lift::Rotor,
        // The rotor's own drag model (`FLIGHT.drag`) holds it back, not a damping.
        linear_damping: 0.0,
        com_drop: 0.0,
        // The rotor's speed: it starts stopped.
        tank: Some(0.0),
        buttons: Buttons::Collective,
        boost: Spec::NO_BOOST,
        // The pedals turn it towards where its pilot looks.
        aims: true,
        solids: &[
            Solid { shape: Shape::Cuboid(AIR_AMBULANCE_HOUSING_HALF), at: AIR_AMBULANCE_HOUSING_CENTRE, role: Role::Part },
            Solid { shape: Shape::Cuboid(AIR_AMBULANCE_BOOM_HALF), at: AIR_AMBULANCE_BOOM_CENTRE, role: Role::Part },
            // Kept clear of the world as it is placed, as wings are: it stands far behind the box.
            Solid { shape: Shape::Cuboid(AIR_AMBULANCE_FIN_HALF), at: AIR_AMBULANCE_FIN_CENTRE, role: Role::Wings },
        ],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        sweep: Spec::FLAT,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        swim: None,
        bow: 0.0,
        profile: Spec::BOX,
        springs: Springs::FIRM,
        airframe: None,
        rotor: Some(FLIGHT),
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
    doors: NO_DOORS,
};
