//! The submarine: everything the game knows about it.

use super::*;

/// The submarine (`KIND_SUBMARINE`): a small diesel-electric boat of about 22 m, with a sail (its conning
/// tower) forward of the middle, a periscope that rises out of it, and four vertical launch tubes in
/// the deck behind it. Its captain (seat 0) drives it and takes it up and down: jump rises at
/// `SUB_DIVE_RATE`, run dives, and with neither it holds its depth. It floats on the surface at
/// `SUB_SURFACE_DEPTH` and goes down as far as the bed lets it. Its weapons officer (seat 1) looks
/// through the periscope while its head is above the water, and from no deeper than
/// `SUB_LAUNCH_DEPTH` fires a guided missile (`KIND_MISSILE`) out of a loaded tube, which they then fly
/// from its nose. Both are sealed in its hull. Its bow tapers over `SUB_BOW`.
pub const SUB_SEATS: usize = 2;
pub const SUB_HALF_EXTENTS: [f32; 3] = [1.4, 1.3, 11.0];
/// How far back from the stem the hull tapers out to its full beam, metres.
pub const SUB_BOW: f32 = 3.0;
/// The sail, from the deck: half its width, height and length, metres, and how far forward of the
/// middle it stands (its centre's Z in the hull frame).
pub const SUB_SAIL_HALF: [f32; 3] = [0.6, 1.0, 2.0];
pub const SUB_SAIL_Z: f32 = -3.0;
/// The top of the sail, in the hull frame: where its hatch is, which it is boarded from.
pub const SUB_HATCH: [f32; 3] = [0.0, 3.3, -3.0];
/// Where whoever leaves it stands, on the deck behind the sail (the middle of a standing soldier).
pub const SUB_EXIT: [f32; 3] = [0.0, 1.35 + STAND_CENTRE, 4.0];
/// How far below the surface its middle floats when it is up, metres: the deck stands
/// `SUB_HALF_EXTENTS[1] - SUB_SURFACE_DEPTH` out of the water.
pub const SUB_SURFACE_DEPTH: f32 = 0.6;
/// How high above its middle the periscope's head stands when it is raised, metres. It sees out while
/// that is above the water, so from any depth of its middle less than this.
pub const SUB_MAST_HEIGHT: f32 = 5.5;
/// The deepest its middle may be for a tube to fire, metres: periscope depth.
pub const SUB_LAUNCH_DEPTH: f32 = 4.5;
/// How fast it rises or dives with jump or run held, m/s.
pub const SUB_DIVE_RATE: f32 = 2.0;
/// Top speed ahead, m/s (about 16 knots), and how hard the motor pushes it along, m/s².
pub const SUB_MAX_SPEED: f32 = 8.0;
pub const SUB_ACCEL: f32 = 1.2;
/// How quickly it turns under way at full rudder, rad/s.
pub const SUB_TURN_RATE: f32 = 0.35;
/// Its launch tubes: how many, and where each one's mouth is, on the deck behind the sail, in the hull
/// frame.
pub const SUB_TUBES: usize = 4;
pub const SUB_TUBE_AT: [[f32; 3]; 4] = [[-0.55, 1.3, 0.6], [0.55, 1.3, 0.6], [-0.55, 1.3, 1.9], [0.55, 1.3, 1.9]];
/// How long a fired tube takes to load again, ticks.
pub const SUB_TUBE_RELOAD: u32 = 20 * TICK_RATE;
/// How fast a missile leaves its tube, straight up, m/s.
pub const SUB_LAUNCH_SPEED: f32 = 12.0;

/// A pressure hull of thick steel: rifles and machine guns ring off it, a cannon's shell goes in. Its
/// battery room is in the bottom of the hull, and the missiles' fuel in the tubes.
const ARMOR: Armor = Armor {
    plates: &[Plate::hull("pressure hull", Zone::Front, 25.0).all_round().glances(80.0)],
    modules: &[
        Module::new(Part::Engine, [0.0, -0.5, 6.0], [0.9, 0.6, 2.0], 60),
        Module::new(Part::Fuel, [0.0, 0.6, 1.2], [0.9, 0.5, 1.2], 40),
    ],
    canvas: false,
};

/// Its middle starts as deep under the water as it floats, and it shoulders its way through a tree on
/// the shore.
const SUB_BODY: Body = Body { drop_y: -SUB_SURFACE_DEPTH, heavy: true, ram: 3.0, crash: 0.3, ..CAR };

pub(super) const SUBMARINE: VehicleDef = VehicleDef {
    kind: KIND_SUBMARINE,
    name: "Submarine",
    hp: 250,
    seats: SUB_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: SUB_BODY,
    spec: Spec {
        half_extents: SUB_HALF_EXTENTS,
        // About 160 m³ of box, some 6.4 t.
        density: 40.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: SUB_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: SUB_TURN_RATE,
        lift: Lift::Dive,
        linear_damping: 0.0,
        // Ballast low in the hull: it rolls back upright.
        com_drop: 0.5,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[Solid {
            shape: Shape::Cuboid(SUB_SAIL_HALF),
            at: [0.0, SUB_HALF_EXTENTS[1] + SUB_SAIL_HALF[1], SUB_SAIL_Z],
            role: Role::Part,
        }],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        bow: SUB_BOW,
        springs: Springs::FIRM,
        airframe: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: Some(SUB_HATCH),
    exit: Some(SUB_EXIT),
    fittings: &[],
};
