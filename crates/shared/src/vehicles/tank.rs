//! The tank: everything the game knows about it.

use super::*;

/// The tank: a heavy six-wheel hull that turns on the spot, with a driver (seat 0), a gunner (seat 1)
/// who aims the turret and fires, and a machine gunner (seat 2) out of the roof hatch. Same body axes
/// as the car.
pub const TANK_SEATS: usize = 3;
pub const TANK_HALF_EXTENTS: [f32; 3] = [1.3, 0.5, 2.6];
pub const TANK_WHEEL_RADIUS: f32 = 0.45;
pub const TANK_TRACK_HALF: f32 = 1.2;
/// Z of the three axles, front first.
pub const TANK_AXLES_Z: [f32; 3] = [-1.9, 0.0, 1.9];
pub const TANK_HARD_POINT_Y: f32 = -0.2;
pub const TANK_SUSPENSION_REST: f32 = 0.35;
pub const TANK_MAX_SPEED: f32 = 12.0;
/// Yaw rate at full stick, rad/s.
pub const TANK_TURN_RATE: f32 = 1.1;
/// Where the turret ring sits in the hull frame, and how far the muzzle is from the ring.
pub const TANK_TURRET_PIVOT: [f32; 3] = [0.0, 0.85, 0.3];
pub const TANK_BARREL_LENGTH: f32 = 3.2;
/// The turret is solid: a cylinder on the ring, so it is the same shape whichever way it points.
/// Its base is the hull's top.
pub const TANK_TURRET_RADIUS: f32 = 1.0;
pub const TANK_TURRET_HALF_HEIGHT: f32 = 0.35;
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
pub const TANK_MG_AT: [f32; 3] = [0.45, 0.6, 0.3];
/// The machine gunner's head from the ring, with the turret ahead: out of the hatch, behind the gun, and
/// clear over the turret's solid, so they can be shot from all round.
pub const TANK_MG_HEAD: [f32; 3] = [0.45, 1.0, 0.75];
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

/// A medium tank's armour: thickest in front, thin over the engine and underneath. The driver looks out
/// through a visor slit in the glacis and the gunner through a sight beside the mantlet, and a well-aimed
/// round can go in through either.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's visor", Zone::Front, Box3::new([-0.55, 0.35, -2.6], [0.14, 0.04, 0.1])),
        Plate::slit("gunner's sight", Zone::Front, Box3::new([0.4, 0.95, -0.62], [0.06, 0.035, 0.1])).on(0),
        Plate::hull("mantlet", Zone::Front, 60.0).on(0).within(Box3::new([0.0, 0.85, -0.62], [0.32, 0.22, 0.15])).glances(72.0),
        Plate::hull("turret front", Zone::Front, 50.0).on(0).glances(68.0),
        Plate::hull("turret roof", Zone::Top, 12.0).on(0).glances(80.0),
        Plate::hull("turret side", Zone::Side, 30.0).on(0).all_round().glances(68.0),
        Plate::hull("engine grille", Zone::Top, 10.0).within(Box3::new([0.0, 0.5, 2.0], [1.3, 0.1, 0.6])).glances(80.0),
        Plate::hull("glacis", Zone::Front, 50.0).sloped(20.0).glances(66.0),
        Plate::hull("hull side", Zone::Side, 30.0).glances(74.0),
        Plate::hull("hull rear", Zone::Back, 20.0).glances(78.0),
        Plate::hull("hull roof", Zone::Top, 12.0).glances(80.0),
        Plate::hull("belly", Zone::Belly, 10.0).glances(80.0),
    ],
    // The engine at the back with the fuel ahead of it, the gearbox in the nose, the rounds racked
    // along the right of the fighting compartment, the driver front left and the gunner in the turret.
    modules: &[
        Module::new(Part::TrackLeft, [-1.15, -0.3, 0.0], [0.17, 0.3, 2.6], 60),
        Module::new(Part::TrackRight, [1.15, -0.3, 0.0], [0.17, 0.3, 2.6], 60),
        Module::new(Part::Engine, [0.0, 0.0, 1.9], [0.8, 0.4, 0.6], 90),
        Module::new(Part::Fuel, [0.0, -0.25, 1.15], [0.9, 0.2, 0.25], 60),
        Module::new(Part::Transmission, [0.0, -0.15, -2.2], [0.7, 0.3, 0.3], 90),
        Module::new(Part::Ammo, [0.75, -0.1, 0.2], [0.35, 0.3, 0.6], 80),
        Module::crew(0, [-0.55, 0.0, -1.6], [0.35, 0.45, 0.45]),
        Module::new(Part::Breech, [0.0, 0.85, -0.1], [0.2, 0.15, 0.35], 70).in_turret(),
        Module::crew(1, [0.0, 0.85, 0.6], [0.55, 0.35, 0.55]).in_turret(),
    ],
    canvas: false,
};

/// A 12.7 mm heavy machine gun.
const ROOF_GUN: Fire = Fire::Hitscan { length: TANK_MG_LENGTH, damage: TANK_MG_DAMAGE, vehicle: TANK_MG_VEHICLE_DAMAGE, plane: TANK_MG_PLANE_DAMAGE, weapon: WEAPON_MG, range: TANK_MG_RANGE, pen: Pen::ball(20.0, 12.0, TANK_MG_RANGE, 12.7) };

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
        // 13.5 m³ of hull is about 13.5 tonnes: ten times the car, so ramming one throws it aside. The
        // engine and suspension cap scale with it, which keeps the acceleration and the ride as they were.
        density: 1000.0,
        wheel_radius: TANK_WHEEL_RADIUS,
        track_half: TANK_TRACK_HALF,
        axles: &TANK_AXLES_Z,
        hard_point_y: TANK_HARD_POINT_Y,
        suspension_rest: TANK_SUSPENSION_REST,
        max_suspension_force: 800_000.0,
        engine_force: 88_000.0,
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
    },
    armor: ARMOR,
    drone: None,
    mount_seat: Some(MountSeat { seat: TANK_MG_SEAT, pivot: TANK_TURRET_PIVOT, head: TANK_MG_HEAD }),
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
    },
];
