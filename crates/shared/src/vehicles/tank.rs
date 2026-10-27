//! The tank: everything the game knows about it.

use super::*;

/// A medium tank's armour: thickest in front, thin over the engine and underneath. The driver looks out
/// through a visor slit in the glacis, the gunner through a sight beside the mantlet and the commander
/// through the cupola's slits, and a well-aimed round can go in through any of them.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("driver's visor", Zone::Front, Box3::new([-0.55, 0.35, -2.6], [0.14, 0.04, 0.1])),
        Plate::slit("gunner's sight", Zone::Front, Box3::new([0.4, 0.95, -0.62], [0.06, 0.035, 0.1])).on(0),
        Plate { face: None, ..Plate::slit("cupola slit", Zone::Side, Box3::new([-0.6, 1.08, 1.1], [0.15, 0.04, 0.15])).on(0) },
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
    mount_seat: None,
};

const MOUNTS: &[MountDef] = &[MountDef {
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
}];
