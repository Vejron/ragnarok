//! The helicopter: everything the game knows about it.

use super::*;

/// The helicopter: a hull flown by a pilot (seat 0) with a gunner (seat 1) on the nose gun. There are no
/// wheels; the rotor is a thrust along the hull's up axis, the cyclic leans the hull to go anywhere,
/// the pedals turn it towards where the pilot looks and the collective lever sets the lift. Same body
/// axes as the car.
pub const HELI_SEATS: usize = 2;
pub const HELI_HALF_EXTENTS: [f32; 3] = [1.0, 0.8, 2.4];
/// How fast jump and run move the pilot's collective lever, share of its travel per second.
pub const HELI_LEVER_RATE: f32 = 0.6;
/// The lever that hovers a fully crewed helicopter out of ground effect, measured by
/// `the_hover_lever_holds_it_steady_out_of_ground_effect`. A pilot who takes the seat in the air starts on it.
pub const HELI_HOVER_LEVER: f32 = 0.53;
/// The thinning air takes the engine's power away over the last 40 m below this height above the
/// terrain, until it only just hovers there. Every flier's ceiling is measured from the terrain under it,
/// not from y = 0.
pub const HELI_CEILING: f32 = 120.0;
/// Under this share of its governed speed the rotor is low, and the pilot is warned.
pub const HELI_LOW_ROTOR: f32 = 0.9;
/// Through the air at about this speed, m/s, the rotor flies into clean air (translational lift), and
/// the airframe shudders on the way.
pub const HELI_ETL_SPEED: f32 = 10.0;

/// How the helicopter flies: a light attack helicopter with power to spare, about 50 m/s flat out,
/// that climbs at over 10 m/s and autorotates down from an engine failure. Its stability augmentation
/// only damps its rates: the pilot holds its attitude.
const FLIGHT: Rotorcraft = Rotorcraft {
    thrust: 1.9 * CAR_GRAVITY,
    hover_lever: HELI_HOVER_LEVER,
    pitch_flow: 30.0,
    tilt_flow: 0.35,
    hover_inflow: 14.0,
    radius: 6.0,
    hub: 1.0,
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
    drag: [0.006, 0.003, 0.0019],
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
    tail_arm: 7.0,
    flapback: 0.008,
    transverse: 0.25,
    stall_speed: 58.0,
    blade_stall: 0.3,
    lever_rate: HELI_LEVER_RATE,
    ceiling: HELI_CEILING,
};
/// The nose gun: a four-barrel autocannon on a chin mount. `HELI_GUN_PIVOT` is the mount in the hull
/// frame and `HELI_GUN_LENGTH` how far the muzzle is from it. The gunner's look direction aims it, within
/// `HELI_GUN_YAW_LIMIT` either side of the nose and `HELI_GUN_PITCH_MIN..=HELI_GUN_PITCH_MAX` (radians, the
/// hull's frame), at `HELI_GUN_SLEW` rad/s. It fires a shell (`SHELL_SPEED`, same fall and lifetime) every
/// `HELI_GUN_COOLDOWN_TICKS`, three a second, scattered by up to `HELI_GUN_SPREAD` radians. It bursts
/// like a tank shell on a smaller scale: a third of the radius and a third of the damage.
pub const HELI_GUN_PIVOT: [f32; 3] = [0.0, -0.6, -2.0];
pub const HELI_GUN_LENGTH: f32 = 1.3;
pub const HELI_GUN_YAW_LIMIT: f32 = 1.0;
pub const HELI_GUN_PITCH_MIN: f32 = -0.75;
pub const HELI_GUN_PITCH_MAX: f32 = 0.2;
pub const HELI_GUN_SLEW: f32 = 3.0;
pub const HELI_GUN_COOLDOWN_TICKS: u32 = TICK_RATE / 3;
pub const HELI_GUN_SPREAD: f32 = 0.01;
/// The rocket pods under the stub wings, fired by the pilot (seat 0) on the fire button. `HELI_POD_PIVOTS`
/// are the four tube mouths in the hull frame, used in turn, one rocket every `HELI_ROCKET_COOLDOWN_TICKS`.
/// A rocket leaves at `HELI_ROCKET_LAUNCH_SPEED` over the helicopter's own velocity, along the nose, and
/// is driven at `ROCKET_ACCEL` m/s² up to `ROCKET_MAX_SPEED`: it flies straight, no gravity.
pub const HELI_POD_PIVOTS: [[f32; 3]; 4] = [[-1.0, -0.39, -0.3], [1.0, -0.39, -0.3], [-1.75, -0.525, -0.3], [1.75, -0.525, -0.3]];
pub const HELI_ROCKET_COOLDOWN_TICKS: u32 = TICK_RATE / 5;
pub const HELI_ROCKET_LAUNCH_SPEED: f32 = 20.0;
/// The pilot raised behind and above the gunner in the nose.
pub const HELI_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.45, -0.45], [0.0, 0.1, -1.55]];

/// The turbine over the cabin, which turns the rotor, and the fuel under its floor; the crew behind the glass are hit as anyone is.
const ARMOR: Armor = Armor {
    plates: ALLOY.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.55, 0.5], [0.4, 0.22, 0.6], 110),
        Module::new(Part::Fuel, [0.0, -0.45, 0.5], [0.5, 0.2, 0.5], 90),
    ],
    canvas: false,
};

pub(super) const HELI: VehicleDef = VehicleDef {
    kind: KIND_HELI,
    name: "Helicopter",
    hp: 100,
    seats: HELI_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Glazed,
    seat_heads: &HELI_SEAT_HEADS,
    crew_guns: false,
    body: ROTOR,
    spec: Spec {
        half_extents: HELI_HALF_EXTENTS,
        // 15.4 m³ of hull is about 4.6 tonnes. Nothing below uses the wheel fields.
        density: 300.0,
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
        solids: &[],
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

/// The gunner's nose autocannon, whose rounds are shells in miniature, and the pilot's rocket pods.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: Some(HELI_GUN_YAW_LIMIT), pitch: (HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX), slew: HELI_GUN_SLEW },
        muzzle: Muzzle::One(HELI_GUN_PIVOT),
        fire: Fire::Shell { kind: PROJECTILE_CANNON, alt: PROJECTILE_CANNON, speed: SHELL_SPEED, length: HELI_GUN_LENGTH, ttl: SHELL_TTL_TICKS },
        trigger: BUTTON_FIRE,
        cooldown: HELI_GUN_COOLDOWN_TICKS,
        spread: HELI_GUN_SPREAD,
        recoil: 0.0,
        heat: 0.0,
        ammo: None,
        needs: &[],
        show: Show::Turret,
        eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
        sight: false,
        stand_in: None,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Turns(&HELI_POD_PIVOTS),
        fire: Fire::Rocket { speed: HELI_ROCKET_LAUNCH_SPEED, ttl: ROCKET_TTL_TICKS },
        trigger: BUTTON_FIRE,
        cooldown: HELI_ROCKET_COOLDOWN_TICKS,
        spread: 0.0,
        recoil: 0.0,
        heat: 0.0,
        ammo: None,
        needs: &[],
        show: Show::Hidden,
        eye: None,
        sight: false,
        stand_in: None,
    },
];
