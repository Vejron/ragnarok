//! The Strf 90: everything the game knows about it.

use super::*;

/// The Strf 90 (`KIND_STRF`): a tracked infantry fighting vehicle. The driver (seat 0) sits front left
/// beside the engine; the gunner (seat 1) works a 40 mm autocannon in a turret set forward of the
/// middle; two troops (seats 2 and 3) ride sealed in the back and get out by the rear ramp. Lighter
/// and quicker than the tank, and thinner-skinned. Same body axes as the tank.
pub const STRF_SEATS: usize = 4;
/// Half the hull box, metres: about 6.4 m long, 3 m wide and 1.2 m deep without the turret.
pub const STRF_HALF_EXTENTS: [f32; 3] = [1.5, 0.6, 3.2];
/// Road wheel radius, metres.
pub const STRF_WHEEL_RADIUS: f32 = 0.42;
/// How far each track runs from the middle, metres.
pub const STRF_TRACK_HALF: f32 = 1.3;
/// Z of the three axles, front first, metres.
pub const STRF_AXLES_Z: [f32; 3] = [-2.3, 0.0, 2.3];
/// Where the hull's centre settles over flat ground, metres: measured by `a_strf_settles_at_its_ride_height`.
pub const STRF_RIDE_HEIGHT: f32 = 1.01;
pub const STRF_HARD_POINT_Y: f32 = -0.25;
pub const STRF_SUSPENSION_REST: f32 = 0.4;
/// Top speed on the flat, m/s (about 70 km/h).
pub const STRF_MAX_SPEED: f32 = 19.0;
/// Yaw rate at full stick, rad/s.
pub const STRF_TURN_RATE: f32 = 1.4;
/// Where the turret ring sits in the hull frame, forward of the middle with the troop bay behind it.
pub const STRF_TURRET_PIVOT: [f32; 3] = [0.0, 0.9, -0.5];
/// The turret's solid: a cylinder on the ring, its base on the hull's roof, metres.
pub const STRF_TURRET_RADIUS: f32 = 0.95;
pub const STRF_TURRET_HALF_HEIGHT: f32 = 0.3;
/// How far the muzzle is from the ring, metres.
pub const STRF_BARREL_LENGTH: f32 = 3.0;
/// How fast the turret and barrel follow the gunner's aim, rad/s: quicker than the tank's.
pub const STRF_TURRET_SLEW: f32 = 2.0;
pub const STRF_BARREL_SLEW: f32 = 1.4;
/// Barrel elevation limits, radians (+ is up): steep enough to fire at aircraft.
pub const STRF_BARREL_MIN: f32 = -0.14;
pub const STRF_BARREL_MAX: f32 = 0.85;
/// Ticks between rounds: five a second.
pub const STRF_CANNON_COOLDOWN_TICKS: u32 = 12;
/// How far a round strays from the bore, radians.
pub const STRF_CANNON_SPREAD: f32 = 0.004;
/// Heat a round puts in the barrel, of 255: about five seconds of holding the trigger jams it.
pub const STRF_CANNON_HEAT: f32 = 22.0;
/// Where a rider who gets out stands, in the hull frame (the middle of a standing soldier): behind the
/// rear ramp.
pub const STRF_RAMP_EXIT: [f32; 3] = [0.0, -0.1, 4.1];

/// Light armour all round, thickest on the sloped nose and the turret's face. The driver looks out
/// through a periscope at the front left and the gunner through a sight beside the gun; the rear is
/// the troop ramp, the thinnest plate on the hull.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's periscope", Zone::Front, Box3::new([-0.75, 0.5, -2.75], [0.15, 0.05, 0.1])),
        Plate::slit("gunner's sight", Zone::Front, Box3::new([0.45, 1.0, -1.35], [0.07, 0.04, 0.1])).on(0),
        Plate::hull("turret front", Zone::Front, 30.0).on(0).sloped(15.0).glances(68.0),
        Plate::hull("turret roof", Zone::Top, 10.0).on(0).glances(80.0),
        Plate::hull("turret side", Zone::Side, 20.0).on(0).all_round().glances(70.0),
        Plate::hull("engine grille", Zone::Top, 8.0).within(Box3::new([0.75, 0.6, -2.2], [0.7, 0.1, 0.7])).glances(80.0),
        Plate::hull("glacis", Zone::Front, 30.0).sloped(30.0).glances(64.0),
        Plate::hull("hull side", Zone::Side, 18.0).glances(74.0),
        Plate::hull("rear ramp", Zone::Back, 10.0).glances(78.0),
        Plate::hull("hull roof", Zone::Top, 10.0).glances(80.0),
        Plate::hull("belly", Zone::Belly, 8.0).glances(80.0),
    ],
    // The engine front right with the gearbox ahead of it, the driver front left, fuel under the
    // turret basket, the rounds in the basket, and the troops on benches in the back.
    modules: &[
        Module::new(Part::TrackLeft, [-1.3, -0.35, 0.0], [0.18, 0.3, 3.2], 50),
        Module::new(Part::TrackRight, [1.3, -0.35, 0.0], [0.18, 0.3, 3.2], 50),
        Module::new(Part::Engine, [0.65, 0.0, -2.1], [0.65, 0.4, 0.75], 80),
        Module::new(Part::Transmission, [0.3, -0.2, -2.95], [0.9, 0.25, 0.2], 80),
        Module::new(Part::Fuel, [0.0, -0.35, -0.5], [1.0, 0.18, 0.5], 50),
        Module::new(Part::Ammo, [0.0, 0.1, -0.15], [0.5, 0.3, 0.3], 70),
        Module::crew(0, [-0.75, 0.1, -2.3], [0.35, 0.45, 0.45]),
        Module::new(Part::Breech, [0.0, 0.9, -0.9], [0.15, 0.12, 0.3], 60).in_turret(),
        Module::crew(1, [0.0, 0.8, -0.1], [0.5, 0.4, 0.45]).in_turret(),
        Module::crew(2, [-0.7, 0.05, 1.8], [0.35, 0.45, 0.8]),
        Module::crew(3, [0.7, 0.05, 1.8], [0.35, 0.45, 0.8]),
    ],
    canvas: false,
};

/// A 40 mm armour-piercing round, with high explosive as its other load.
const CANNON: Fire = Fire::Shell { kind: PROJECTILE_CANNON_AP, alt: PROJECTILE_CANNON, speed: SHELL_SPEED, length: STRF_BARREL_LENGTH, ttl: SHELL_TTL_TICKS };

pub(super) const STRF: VehicleDef = VehicleDef {
    kind: KIND_STRF,
    name: "Strf 90",
    hp: 180,
    seats: STRF_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: TRACKS,
    spec: Spec {
        half_extents: STRF_HALF_EXTENTS,
        // 23 m³ of hull at 600 kg/m³ is about 13.8 tonnes, a touch more than the tank: it is a big box,
        // but mostly air. More engine than the tank's for the same weight, and a higher top speed.
        density: 600.0,
        wheel_radius: STRF_WHEEL_RADIUS,
        track_half: STRF_TRACK_HALF,
        axles: &STRF_AXLES_Z,
        hard_point_y: STRF_HARD_POINT_Y,
        suspension_rest: STRF_SUSPENSION_REST,
        max_suspension_force: 820_000.0,
        engine_force: 115_000.0,
        max_speed: STRF_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: STRF_TURN_RATE,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[Solid { shape: Shape::Cylinder { half_height: STRF_TURRET_HALF_HEIGHT, radius: STRF_TURRET_RADIUS }, at: STRF_TURRET_PIVOT, role: Role::Turret }],
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
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: Some(STRF_RAMP_EXIT),
    fittings: &[],
};

/// The gunner's autocannon in the turret.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 1,
    aim: Aim::Turret { slew: STRF_TURRET_SLEW, pitch: (STRF_BARREL_MIN, STRF_BARREL_MAX), pitch_slew: STRF_BARREL_SLEW },
    muzzle: Muzzle::One(STRF_TURRET_PIVOT),
    fire: CANNON,
    trigger: BUTTON_FIRE,
    cooldown: STRF_CANNON_COOLDOWN_TICKS,
    spread: STRF_CANNON_SPREAD,
    recoil: 0.0,
    heat: STRF_CANNON_HEAT,
    ammo: None,
    needs: &[Need::Turret, Need::Breech],
    show: Show::Turret,
    eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
    sight: true,
}];
