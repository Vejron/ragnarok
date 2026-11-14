//! The Titan: everything the game knows about it.

use super::*;

/// The Titan (`KIND_TITAN`): a giant robot about fifty metres high, one pilot sealed in its head. Its
/// body is a single box, its torso, held up by two spring legs that are ray casts to the ground and
/// kept upright by a torque, exactly as a walker's is (its `Gait`). It has no guns. Its pilot moves its
/// two fists (`TITAN_ARMS`): they hit what they meet and hold what they close on. Its feet are solid
/// and crush what they come down on.
pub const TITAN_SEATS: usize = 1;
/// Half the torso's box, metres. The hull frame has its origin at the box's centre.
pub const TITAN_HALF_EXTENTS: [f32; 3] = [6.0, 8.0, 4.0];
/// Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
/// bottom of the torso to the ground when it stands.
pub const TITAN_FOOT_HALF: f32 = 5.0;
pub const TITAN_LEG_LENGTH: f32 = 26.0;
/// How much shorter its legs are when it crouches, metres, and how far it leans forward then, as a tilt.
pub const TITAN_CROUCH: f32 = 16.0;
pub const TITAN_CROUCH_LEAN: f32 = 0.3;
/// How fast it stands up from a crouch, m/s.
pub const TITAN_RISE: f32 = 7.0;
/// Top speed, m/s, and its yaw rate at full stick, rad/s.
pub const TITAN_MAX_SPEED: f32 = 11.0;
pub const TITAN_TURN_RATE: f32 = 0.6;
/// The leg spring, per kilogram of the body: soft, so it settles `CAR_GRAVITY / TITAN_LEG_SPRING` into
/// its legs and takes a second to crouch or to stand.
pub const TITAN_LEG_SPRING: f32 = 12.0;
/// Where the hull's centre settles over flat ground when it stands, metres. Measured by
/// `a_titan_stands_fifty_metres_high_on_its_legs`.
pub const TITAN_RIDE_HEIGHT: f32 = 33.2;
/// Where the right shoulder is in the hull frame; the left is its mirror in X.
pub const TITAN_SHOULDER: [f32; 3] = [8.5, 6.0, 0.0];
/// The furthest and the nearest a fist's middle can be from its shoulder, metres, and a fist's radius.
pub const TITAN_REACH: f32 = 24.0;
pub const TITAN_MIN_REACH: f32 = 6.0;
pub const TITAN_FIST_RADIUS: f32 = 2.5;
/// A fist's top speed against the hull, m/s, and how hard it speeds up and slows, m/s².
pub const TITAN_FIST_SPEED: f32 = 45.0;
pub const TITAN_FIST_ACCEL: f32 = 140.0;
/// Where the right fist hangs when nobody asks anything of it, in the hull frame.
pub const TITAN_FIST_REST: [f32; 3] = [10.0, -15.0, -3.0];
/// The heaviest vehicle a closed fist picks up, kilograms: a tank, but not a ship.
pub const TITAN_LIFT_MASS: f32 = 60_000.0;
/// Half extents of a shin's column, metres, which stands where the foot is planted.
pub const TITAN_SHIN_HALF: [f32; 3] = [2.2, 6.0, 3.2];
/// The fuel its thrusters burn, as `Spec::tank`: 1 is full.
pub const TITAN_FUEL: f32 = 1.0;
/// The upward speed its leap gives, the climb its thrusters hold and how fast they carry it along, m/s,
/// and how high over the ground they take it, metres.
pub const TITAN_LEAP_SPEED: f32 = 14.0;
pub const TITAN_JET_CLIMB: f32 = 8.0;
pub const TITAN_JET_SPEED: f32 = 20.0;
pub const TITAN_CEILING: f32 = 120.0;
/// Coming down at this speed or faster, m/s, its landing is a slam, which is at its worst at
/// `TITAN_SLAM_FULL`.
pub const TITAN_SLAM_SPEED: f32 = 8.0;
pub const TITAN_SLAM_FULL: f32 = 35.0;
/// How far out a slam reaches, metres, at `TITAN_SLAM_SPEED` and at `TITAN_SLAM_FULL`.
pub const TITAN_SLAM_RADIUS: f32 = 20.0;
pub const TITAN_SLAM_RADIUS_FULL: f32 = 50.0;
/// The speed a soldier or a vehicle at the middle of the worst slam is thrown outward with, m/s, and
/// the health each loses there. Both fall away to nothing at its edge.
pub const TITAN_SLAM_KNOCK: f32 = 22.0;
pub const TITAN_SLAM_DAMAGE: f32 = 160.0;
pub const TITAN_SLAM_VEHICLE_DAMAGE: f32 = 220.0;

/// Its longest step and the shortest it walks with, metres; how long a foot is in the air and how long
/// the weight takes to go from one foot to the other, seconds; and how high a foot lifts in the middle
/// of a step, metres. A leg 26 m long swings no faster than this: the slowness is what makes it big.
pub const TITAN_STRIDE: f32 = 18.0;
pub const TITAN_STRIDE_MIN: f32 = 10.0;
pub const TITAN_STEP_TIME: f32 = 1.3;
pub const TITAN_SHIFT_TIME: f32 = 0.35;
pub const TITAN_STEP_LIFT: f32 = 4.5;
/// Half extents of each thigh, a solid on the hull that hangs from the hip, metres.
pub const TITAN_THIGH_HALF: [f32; 3] = [2.0, 3.5, 2.4];
/// Where it is boarded from and where its pilot is set down, in the hull frame: on the ground between
/// its feet, and ahead of them.
pub const TITAN_BOARD: [f32; 3] = [0.0, -32.2, 0.0];
pub const TITAN_EXIT: [f32; 3] = [0.0, -32.2, -10.0];
/// The pilot's eye, in the hull frame: at the visor, just clear of the head's own plates.
pub const TITAN_EYE: [f32; 3] = [0.0, 12.0, -3.2];
/// A fist moving at least this fast, m/s, hits what it meets; slower, it only shoves.
pub const TITAN_PUNCH_SPEED: f32 = 8.0;
/// What a punch takes off a soldier, per m/s of the fist's speed, and off a vehicle.
pub const TITAN_PUNCH_DAMAGE: f32 = 6.0;
pub const TITAN_PUNCH_VEHICLE_DAMAGE: f32 = 4.0;
/// How far around a foot coming down soldiers are knocked off their feet, metres, and how hard they
/// are thrown at its edge, m/s. Under the foot itself they are crushed.
pub const TITAN_STOMP_RADIUS: f32 = 14.0;
pub const TITAN_STOMP_KNOCK: f32 = 9.0;
/// What a foot coming down takes off a vehicle under it.
pub const TITAN_STOMP_VEHICLE_DAMAGE: f32 = 120.0;
/// A soldier thrown from a fist keeps this share of the fist's speed.
pub const TITAN_THROW_SHARE: f32 = 1.0;

/// The thighs: solids on the hull, from the hips down. They stop short of the knee, so that crouched
/// they clear the ground; below them the shins are the feet's own solids (`TITAN_FEET`).
const THIGHS: [Solid; 2] = [
    Solid { shape: Shape::Cuboid(TITAN_THIGH_HALF), at: [-TITAN_FOOT_HALF, -TITAN_HALF_EXTENTS[1] - TITAN_THIGH_HALF[1], 0.0], role: Role::Part },
    Solid { shape: Shape::Cuboid(TITAN_THIGH_HALF), at: [TITAN_FOOT_HALF, -TITAN_HALF_EXTENTS[1] - TITAN_THIGH_HALF[1], 0.0], role: Role::Part },
];

/// Armour a tank would envy, all round: a rifle or a machine gun does nothing to it. The reactor is in
/// its chest behind the glowing core, and the pilot in the head is above the box, out of any round's way.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::hull("chest", Zone::Front, 90.0).glances(70.0),
        Plate::hull("flank", Zone::Side, 70.0).glances(72.0),
        Plate::hull("back", Zone::Back, 50.0).glances(74.0),
        Plate::hull("shoulders", Zone::Top, 60.0).glances(76.0),
        Plate::hull("hips", Zone::Belly, 60.0).glances(76.0),
    ],
    modules: &[Module::new(Part::Engine, [0.0, 1.0, -1.5], [2.0, 2.0, 1.5], 200)],
    canvas: false,
};

/// How it stands and walks: the walker's way, at ten times the size and a third of the pace.
pub const TITAN_GAIT: Gait = Gait {
    foot_half: TITAN_FOOT_HALF,
    leg_length: TITAN_LEG_LENGTH,
    crouch: TITAN_CROUCH,
    crouch_lean: TITAN_CROUCH_LEAN,
    max_speed: TITAN_MAX_SPEED,
    reverse_share: 0.5,
    accel: 3.0,
    brake: 4.0,
    grip_gain: 4.0,
    grip: 6.0,
    turn_rate: TITAN_TURN_RATE,
    turn_accel: 1.5,
    spring: TITAN_LEG_SPRING,
    damping: 5.5,
    // Stiff enough that the weight on its hips, which are behind it when it leans, does not bow it over.
    balance_gain: 10.0,
    balance_damp: 5.0,
    balance_max: 1.2,
    balance_max_pitch: 2.5,
    catch_gain: 12.0,
    catch_damp: 3.0,
    catch_max: 5.0,
    catch_lean_margin: 0.15,
    catch_fall_margin: 0.4,
    cruise_lean: 0.05,
    accel_lean: 0.5,
    blast_lever: 6.0,
    fallen_up: 0.6,
    // Lying down, it takes three times the pull that keeps it standing to lift its weight off the ground.
    getup_share: 3.0,
    rise: TITAN_RISE,
    blast_share: 0.04,
    feet: Some(Feet {
        half: TITAN_SHIN_HALF,
        stride: TITAN_STRIDE,
        stride_min: TITAN_STRIDE_MIN,
        step_time: TITAN_STEP_TIME,
        lift: TITAN_STEP_LIFT,
        shift_time: TITAN_SHIFT_TIME,
        sway: 0.3,
        sway_gain: 4.0,
        tilt: 0.04,
        pulse: 0.3,
        crouch_pace: 0.4,
    }),
    jets: Some(TITAN_JETS),
};

/// The thrusters in its feet. A leap throws it about 10 m up; held, they climb at `TITAN_JET_CLIMB`
/// for about seven seconds, and held on the way down they slow a fall by 12 m/s every second.
pub const TITAN_JETS: Jets = Jets {
    leap: TITAN_LEAP_SPEED,
    leap_forward: 6.0,
    leap_cost: 0.1,
    climb: TITAN_JET_CLIMB,
    gain: 2.0,
    accel: 12.0,
    drain: 0.14,
    refill: 0.1,
    speed: TITAN_JET_SPEED,
    push: 5.0,
    tilt: 0.15,
    level: 3.0,
    response: 4.0,
    yaw: TITAN_TURN_RATE,
    ceiling: TITAN_CEILING,
};

/// Its arms.
pub const TITAN_ARMS: Arms = Arms {
    shoulder: TITAN_SHOULDER,
    reach: TITAN_REACH,
    min_reach: TITAN_MIN_REACH,
    fist: TITAN_FIST_RADIUS,
    speed: TITAN_FIST_SPEED,
    accel: TITAN_FIST_ACCEL,
    rest: TITAN_FIST_REST,
    lift: TITAN_LIFT_MASS,
};

pub(super) const TITAN: VehicleDef = VehicleDef {
    kind: KIND_TITAN,
    name: "Titan",
    hp: 255,
    seats: TITAN_SEATS,
    mounts: &[],
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: GIANT,
    spec: Spec {
        half_extents: TITAN_HALF_EXTENTS,
        // 1,536 m³ of torso at this density is about 2,300 tonnes. Nothing below uses the wheel fields:
        // the legs are `TITAN_GAIT`.
        density: 1500.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: TITAN_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: TITAN_TURN_RATE,
        lift: Lift::Walker,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: Some(TITAN_FUEL),
        buttons: Buttons::Stride,
        aims: false,
        solids: &THIGHS,
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        bow: 0.0,
        springs: Springs::FIRM,
        airframe: None,
        gait: Some(TITAN_GAIT),
        arms: Some(TITAN_ARMS),
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: Some(TITAN_BOARD),
    exit: Some(TITAN_EXIT),
    fittings: &[],
};
