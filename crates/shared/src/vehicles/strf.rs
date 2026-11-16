//! The Strf 90: everything the game knows about it.

use super::*;

/// The Strf 90 (`KIND_STRF`): a tracked infantry fighting vehicle. The driver (seat 0) sits front left
/// beside the engine; the gunner (seat 1) works a 40 mm autocannon in a turret set forward of the
/// middle; two troops (seats 2 and 3) ride sealed in the back and get out by the rear ramp. Lighter
/// and quicker than the tank, and thinner-skinned. Same body axes as the tank.
pub const STRF_SEATS: usize = 4;
/// Half the hull's bounding box, metres: 6.55 m long, 3.2 m wide over the tracks and 1.3 m deep from the
/// belly to the hull roof, which is 1.75 m off the ground. The size of a CV90.
pub const STRF_HALF_EXTENTS: [f32; 3] = [1.6, 0.65, 3.275];
/// Road wheel radius, metres.
pub const STRF_WHEEL_RADIUS: f32 = 0.42;
/// How far each track runs from the middle, metres.
pub const STRF_TRACK_HALF: f32 = 1.33;
/// Z of the three axles, front first, metres.
pub const STRF_AXLES_Z: [f32; 3] = [-2.3, 0.0, 2.3];
/// Where the hull's centre settles over flat ground, metres: measured by `a_strf_settles_at_its_ride_height`.
pub const STRF_RIDE_HEIGHT: f32 = 1.1;
pub const STRF_HARD_POINT_Y: f32 = -0.3;
pub const STRF_SUSPENSION_REST: f32 = 0.44;
/// Top speed on the flat, m/s (about 70 km/h).
pub const STRF_MAX_SPEED: f32 = 19.0;
/// Yaw rate at full stick, rad/s.
pub const STRF_TURN_RATE: f32 = 1.4;
/// Where the turret ring sits in the hull frame (the middle of the turret's solid), forward of the
/// middle with the troop bay behind it.
pub const STRF_TURRET_PIVOT: [f32; 3] = [0.0, 1.025, -0.4];
/// The turret's solid: a cylinder on the ring, its base on the hull's roof and its top 2.5 m off the
/// ground, metres.
pub const STRF_TURRET_RADIUS: f32 = 1.0;
pub const STRF_TURRET_HALF_HEIGHT: f32 = 0.375;
/// How far the muzzle is from the ring, metres: it reaches a little past the nose.
pub const STRF_BARREL_LENGTH: f32 = 3.1;
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
pub const STRF_RAMP_EXIT: [f32; 3] = [0.0, -0.1, 4.2];

/// The hull's side outline, `[z, y]`: a high nose, from which the engine's cover climbs at 14 degrees
/// above level to the roof and the lower front plate drops steeply to the belly; a flat roof; and the
/// upright rear with the ramp in it.
const PROFILE: &[[f32; 2]] = &[[-3.275, 0.2], [-1.475, 0.65], [3.275, 0.65], [3.275, -0.3], [3.0, -0.65], [-2.6, -0.65]];

/// Light armour all round, thickest on the lower front plate and the turret's face. The plate over the
/// engine is thin, but lies so flat that a level shot has four times its thickness to get through. The
/// driver looks out through a periscope at the front left and the gunner through a sight beside the
/// gun; the rear is the troop ramp, the thinnest upright plate on the hull.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's periscope", Zone::Top, Box3::new([-0.8, 0.52, -2.0], [0.16, 0.06, 0.15])).looks(Zone::Front),
        Plate::slit("gunner's sight", Zone::Front, Box3::new([0.45, 1.15, -1.29], [0.07, 0.04, 0.1])).on(0),
        Plate::hull("turret front", Zone::Front, 30.0).on(0).sloped(15.0).glances(68.0),
        Plate::hull("turret roof", Zone::Top, 10.0).on(0).glances(80.0),
        Plate::hull("turret side", Zone::Side, 20.0).on(0).all_round().glances(70.0),
        Plate::hull("engine cover", Zone::Top, 8.0).within(Box3::new([0.0, 0.42, -2.375], [1.6, 0.3, 0.92])).glances(80.0),
        Plate::hull("lower front", Zone::Front, 30.0).glances(64.0),
        Plate::hull("hull side", Zone::Side, 18.0).glances(74.0),
        Plate::hull("rear ramp", Zone::Back, 10.0).glances(78.0),
        Plate::hull("hull roof", Zone::Top, 10.0).glances(80.0),
        Plate::hull("belly", Zone::Belly, 8.0).glances(80.0),
    ],
    // The engine front right with the gearbox ahead of it, the driver front left, fuel under the
    // turret basket, the rounds in the basket, and the troops on benches in the back.
    modules: &[
        Module::new(Part::TrackLeft, [-1.33, -0.35, 0.0], [0.27, 0.3, 3.275], 50),
        Module::new(Part::TrackRight, [1.33, -0.35, 0.0], [0.27, 0.3, 3.275], 50),
        Module::new(Part::Engine, [0.7, 0.0, -2.0], [0.7, 0.4, 0.75], 80),
        Module::new(Part::Transmission, [0.3, -0.1, -2.7], [0.9, 0.25, 0.2], 80),
        Module::new(Part::Fuel, [0.0, -0.4, -0.4], [1.1, 0.2, 0.5], 50),
        Module::new(Part::Ammo, [0.0, 0.1, -0.1], [0.5, 0.3, 0.3], 70),
        Module::crew(0, [-0.8, 0.1, -2.0], [0.35, 0.45, 0.45]),
        Module::new(Part::Breech, [0.0, 1.025, -0.85], [0.15, 0.12, 0.3], 60).in_turret(),
        Module::crew(1, [0.0, 0.95, 0.0], [0.5, 0.4, 0.45]).in_turret(),
        Module::crew(2, [-0.75, 0.05, 1.9], [0.38, 0.45, 0.85]),
        Module::crew(3, [0.75, 0.05, 1.9], [0.38, 0.45, 0.85]),
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
        // The hull's box at 507 kg/m³ is about 13.8 tonnes, a touch more than the tank. More engine than
        // the tank's for the same weight, and a higher top speed.
        density: 507.0,
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
        swim: None,
        bow: 0.0,
        profile: PROFILE,
        springs: Springs::FIRM,
        airframe: None,
        gait: None,
        arms: None,
        runners: None,
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
