//! The tank: everything the game knows about it.

use super::*;

/// The tank: a heavy six-wheel hull that turns on the spot, with a driver (seat 0), a gunner (seat 1)
/// who aims the turret and fires, and a machine gunner (seat 2) out of the roof hatch. It is the size
/// and shape of a Strv 104 (a Centurion). Same body axes as the car.
pub const TANK_SEATS: usize = 3;
/// Half the hull's bounding box, metres: 7.6 m long, 3.4 m wide over the tracks and 1.25 m deep from
/// the belly to the hull roof, which is 1.75 m off the ground.
pub const TANK_HALF_EXTENTS: [f32; 3] = [1.7, 0.625, 3.8];
pub const TANK_WHEEL_RADIUS: f32 = 0.45;
pub const TANK_TRACK_HALF: f32 = 1.4;
/// Z of the three axles, front first.
pub const TANK_AXLES_Z: [f32; 3] = [-2.6, 0.0, 2.6];
/// Where the hull's centre settles over flat ground, metres: measured by `scripted_drive_matches_fixture`.
/// A whole number of centimetres, as a snapshot carries it: a settled tank's prediction then starts
/// from where the server has it.
pub const TANK_RIDE_HEIGHT: f32 = 1.13;
pub const TANK_HARD_POINT_Y: f32 = -0.3;
pub const TANK_SUSPENSION_REST: f32 = 0.44;
pub const TANK_MAX_SPEED: f32 = 12.0;
/// Yaw rate at full stick, rad/s.
pub const TANK_TURN_RATE: f32 = 1.1;
/// Where the turret ring sits in the hull frame (the middle of the turret's solid), a little ahead of
/// the hull's middle, and how far the muzzle is from the ring: the long gun reaches 2.3 m past the nose.
pub const TANK_TURRET_PIVOT: [f32; 3] = [0.0, 1.125, -0.4];
pub const TANK_BARREL_LENGTH: f32 = 5.7;
/// The turret is solid: a cylinder on the ring, so it is the same shape whichever way it points.
/// Its base is the hull's top, and its roof is 2.75 m off the ground.
pub const TANK_TURRET_RADIUS: f32 = 1.15;
pub const TANK_TURRET_HALF_HEIGHT: f32 = 0.5;
/// How fast the turret and barrel follow the gunner's aim, rad/s.
pub const TANK_TURRET_SLEW: f32 = 1.4;
pub const TANK_BARREL_SLEW: f32 = 0.9;
/// Barrel elevation limits, radians (+ is up).
pub const TANK_BARREL_MIN: f32 = -0.12;
pub const TANK_BARREL_MAX: f32 = 0.5;
/// What it takes to knock the turret off: damage from hits on the turret and the hull's roof, which a
/// hull hit never adds to. Past it `DAMAGE_TURRET` is set, the gunner dies and the gun is gone until the
/// tank respawns. Mirrors `TANK_TURRET_HP` in `sim/consts.ts`.
pub const TANK_TURRET_HP: u8 = 90;

/// The seat that works the roof machine gun.
pub const TANK_MG_SEAT: usize = 2;
/// Where the roof gun turns, from the turret ring in the turret's frame, over the hatch on its right.
pub const TANK_MG_AT: [f32; 3] = [0.5, 0.75, 0.35];
/// The machine gunner's head from the ring, with the turret ahead: out of the hatch, behind the gun, and
/// clear over the turret's solid, so they can be shot from all round.
pub const TANK_MG_HEAD: [f32; 3] = [0.5, 1.15, 0.85];
pub const TANK_MG_LENGTH: f32 = 1.1;
/// How far the roof gun turns either side of where the cannon points, and its elevation, radians (+ is
/// up): steep enough for aircraft.
pub const TANK_MG_YAW_LIMIT: f32 = 1.57;
pub const TANK_MG_PITCH_MIN: f32 = -0.25;
pub const TANK_MG_PITCH_MAX: f32 = 0.9;
pub const TANK_MG_SLEW: f32 = 2.5;
pub const TANK_MG_COOLDOWN_TICKS: u32 = 6;
pub const TANK_MG_SPREAD: f32 = 0.012;
pub const TANK_MG_RANGE: f32 = 300.0;
pub const TANK_MG_DAMAGE: u8 = 16;
pub const TANK_MG_PLANE_DAMAGE: u8 = 8;
pub const TANK_MG_VEHICLE_DAMAGE: u8 = 4;
/// Heat a round puts in the barrel: about six seconds of holding the trigger jams it.
pub const TANK_MG_HEAT: f32 = 10.5;

/// The hull's side outline, `[z, y]`: a sharp nose a little below mid height, the glacis leaning 57 degrees back from
/// upright to the roof, a short bevel down to the tail, and the plates under the nose and the tail
/// leaning in to the belly, steep enough that a shot into one meets front or rear armour (`Zone::of`).
const PROFILE: &[[f32; 2]] = &[[-3.8, -0.1], [-2.68, 0.625], [3.5, 0.625], [3.8, 0.2], [3.8, -0.2], [3.45, -0.625], [-3.35, -0.625]];

/// A medium tank's armour: thickest in front, thin over the engine and underneath. The glacis is not
/// thick, but it leans so far back that a level shot has nearly twice its thickness to get through. The
/// driver looks out through a visor slit in the glacis and the gunner through a sight beside the mantlet,
/// and a well-aimed round can go in through either.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's visor", Zone::Top, Box3::new([-0.7, 0.33, -3.15], [0.16, 0.06, 0.1])).looks(Zone::Front),
        Plate::slit("gunner's sight", Zone::Front, Box3::new([0.45, 1.25, -1.46], [0.07, 0.04, 0.1])).on(0),
        Plate::hull("mantlet", Zone::Front, 60.0).on(0).within(Box3::new([0.0, 1.125, -1.55], [0.4, 0.3, 0.15])).glances(72.0),
        Plate::hull("turret front", Zone::Front, 50.0).on(0).glances(68.0),
        Plate::hull("turret roof", Zone::Top, 12.0).on(0).glances(80.0),
        Plate::hull("turret side", Zone::Side, 30.0).on(0).all_round().glances(68.0),
        Plate::hull("engine grille", Zone::Top, 10.0).within(Box3::new([0.0, 0.625, 2.4], [1.7, 0.1, 1.1])).glances(80.0),
        Plate::hull("glacis", Zone::Top, 32.0).within(Box3::new([0.0, 0.26, -3.24], [1.7, 0.4, 0.58])).glances(70.0),
        Plate::hull("lower front", Zone::Front, 45.0).glances(70.0),
        Plate::hull("hull side", Zone::Side, 30.0).glances(74.0),
        Plate::hull("hull rear", Zone::Back, 20.0).glances(78.0),
        Plate::hull("hull roof", Zone::Top, 12.0).glances(80.0),
        Plate::hull("belly", Zone::Belly, 10.0).glances(80.0),
    ],
    // The engine at the back with the fuel ahead of it, the gearbox in the nose, the rounds racked
    // along the right of the fighting compartment, the driver front left and the gunner in the turret.
    modules: &[
        Module::new(Part::TrackLeft, [-1.4, -0.3, 0.0], [0.3, 0.33, 3.8], 60),
        Module::new(Part::TrackRight, [1.4, -0.3, 0.0], [0.3, 0.33, 3.8], 60),
        Module::new(Part::Engine, [0.0, 0.0, 2.6], [1.0, 0.45, 0.9], 90),
        Module::new(Part::Fuel, [0.0, -0.3, 1.5], [1.1, 0.25, 0.3], 60),
        Module::new(Part::Transmission, [0.0, -0.2, -3.0], [0.9, 0.3, 0.35], 90),
        Module::new(Part::Ammo, [1.0, -0.1, -0.2], [0.45, 0.35, 0.8], 80),
        Module::crew(0, [-0.7, 0.0, -2.4], [0.4, 0.5, 0.5]),
        Module::new(Part::Breech, [0.0, 1.125, -0.85], [0.16, 0.15, 0.3], 70).in_turret(),
        Module::crew(1, [0.0, 1.1, 0.1], [0.6, 0.45, 0.6]).in_turret(),
    ],
    canvas: false,
};

/// A 12.7 mm heavy machine gun.
const ROOF_GUN: Fire = Fire::Bullet { length: TANK_MG_LENGTH, damage: TANK_MG_DAMAGE, vehicle: TANK_MG_VEHICLE_DAMAGE, plane: TANK_MG_PLANE_DAMAGE, weapon: WEAPON_MG, range: TANK_MG_RANGE, pen: Pen::ball(20.0, 12.0, TANK_MG_RANGE, 12.7) };

const SHELL: Fire = Fire::Shell { kind: PROJECTILE_SHELL, alt: PROJECTILE_HE, speed: SHELL_SPEED, length: TANK_BARREL_LENGTH, ttl: SHELL_TTL_TICKS };

pub(super) const TANK: VehicleDef = VehicleDef {
    kind: KIND_TANK,
    name: "Tank",
    hp: 250,
    seats: TANK_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: TRACKS,
    spec: Spec {
        half_extents: TANK_HALF_EXTENTS,
        // The hull's box at 418 kg/m³ is about 13.5 tonnes: ten times the car, so ramming one throws it
        // aside. The engine pulls about 7.4 m/s², enough to grind up a 35° slope from a standing start.
        density: 418.0,
        wheel_radius: TANK_WHEEL_RADIUS,
        track_half: TANK_TRACK_HALF,
        axles: &TANK_AXLES_Z,
        hard_point_y: TANK_HARD_POINT_Y,
        suspension_rest: TANK_SUSPENSION_REST,
        max_suspension_force: 800_000.0,
        engine_force: 100_000.0,
        max_speed: TANK_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: TANK_TURN_RATE,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[Solid { shape: Shape::Cylinder { half_height: TANK_TURRET_HALF_HEIGHT, radius: TANK_TURRET_RADIUS }, at: TANK_TURRET_PIVOT, role: Role::Turret }],
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
    mount_seat: Some(MountSeat { seat: TANK_MG_SEAT, pivot: TANK_TURRET_PIVOT, head: TANK_MG_HEAD }),
    board: None,
    exit: None,
    fittings: &[],
};

/// The cannon, then the roof gun, which reads the turret's yaw as of this tick.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Turret { slew: TANK_TURRET_SLEW, pitch: (TANK_BARREL_MIN, TANK_BARREL_MAX), pitch_slew: TANK_BARREL_SLEW },
        muzzle: Muzzle::One(TANK_TURRET_PIVOT),
        fire: SHELL,
        trigger: BUTTON_FIRE,
        cooldown: SHELL_COOLDOWN_TICKS,
        spread: 0.0,
        recoil: SHELL_RECOIL,
        heat: 0.0,
        ammo: None,
        needs: &[Need::Turret, Need::Breech],
        show: Show::Turret,
        eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
        sight: true,
    },
    MountDef {
        seat: TANK_MG_SEAT,
        aim: Aim::Pintle { yaw_limit: TANK_MG_YAW_LIMIT, pitch: (TANK_MG_PITCH_MIN, TANK_MG_PITCH_MAX), slew: TANK_MG_SLEW },
        muzzle: Muzzle::Pintle { ring: TANK_TURRET_PIVOT, at: TANK_MG_AT },
        fire: ROOF_GUN,
        trigger: BUTTON_FIRE,
        cooldown: TANK_MG_COOLDOWN_TICKS,
        spread: TANK_MG_SPREAD,
        recoil: 0.0,
        heat: TANK_MG_HEAT,
        ammo: None,
        needs: &[Need::Turret],
        show: Show::Pintle,
        eye: Some(Eye { back: ROOF_AIM_BACK, up: ROOF_AIM_UP }),
        sight: false,
    },
];
