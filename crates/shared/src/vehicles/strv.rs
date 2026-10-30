//! The Strv S: everything the game knows about it.

use super::*;

/// The Strv S (`KIND_STRV`): a turretless tank with the gun fixed in the hull. One seat: the driver steers
/// by pivoting the hull and fires the main gun, which only the barrel's elevation can aim. Quicker and
/// lower than the tank, with a steeply sloped glacis. Same body axes as the tank; mirrors `STRV_*` in
/// `sim/consts.ts`.
pub const STRV_SEATS: usize = 1;
pub const STRV_HALF_EXTENTS: [f32; 3] = [1.3, 0.45, 2.7];
/// Bigger wheels and a longer suspension than the tank's, with the hull higher on them: it steps over
/// obstacles the tank has to go around.
pub const STRV_WHEEL_RADIUS: f32 = 0.52;
pub const STRV_TRACK_HALF: f32 = 1.2;
/// Z of the three axles, front first.
pub const STRV_AXLES_Z: [f32; 3] = [-1.9, 0.0, 1.9];
pub const STRV_HARD_POINT_Y: f32 = -0.2;
pub const STRV_SUSPENSION_REST: f32 = 0.42;
pub const STRV_MAX_SPEED: f32 = 16.0;
/// Yaw rate at full stick, rad/s.
pub const STRV_TURN_RATE: f32 = 1.5;
/// The casemate the gun sits in is a solid box on the hull's roof, weightless like the tank's turret:
/// its half extents and centre in the hull frame.
pub const STRV_CASEMATE_HALF: [f32; 3] = [0.9, 0.17, 1.25];
pub const STRV_CASEMATE_CENTRE: [f32; 3] = [0.0, 0.62, -0.25];
/// Where the gun's trunnion is in the hull frame, how far the muzzle is from it, how fast the barrel
/// elevates (rad/s) and its limits, radians (+ is up). The gun points where the hull does.
pub const STRV_GUN_PIVOT: [f32; 3] = [0.0, 0.72, -1.3];
pub const STRV_BARREL_LENGTH: f32 = 4.4;
pub const STRV_BARREL_SLEW: f32 = 1.2;
pub const STRV_BARREL_MIN: f32 = -0.15;
pub const STRV_BARREL_MAX: f32 = 0.3;

/// The steep glacis turns shots away from well off the nose, and goes a long way through for one that
/// does land. The engine is at the back, under a thin deck, and the driver (who is the gunner too) looks
/// out through a periscope in the casemate's face.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("periscope", Zone::Front, Box3::new([-0.5, 0.7, -1.5], [0.12, 0.05, 0.05])).on(0),
        Plate::hull("casemate front", Zone::Front, 40.0).on(0).sloped(30.0).glances(66.0),
        Plate::hull("casemate roof", Zone::Top, 10.0).on(0).glances(78.0),
        Plate::hull("casemate side", Zone::Side, 20.0).on(0).all_round().glances(72.0),
        Plate::hull("engine deck", Zone::Top, 8.0).within(Box3::new([0.0, 0.45, 2.0], [1.3, 0.1, 0.7])).glances(78.0),
        Plate::hull("glacis", Zone::Front, 40.0).sloped(55.0).glances(58.0),
        Plate::hull("hull side", Zone::Side, 25.0).glances(72.0),
        Plate::hull("hull rear", Zone::Back, 20.0).glances(75.0),
        Plate::hull("hull roof", Zone::Top, 12.0).glances(78.0),
        Plate::hull("belly", Zone::Belly, 10.0).glances(78.0),
    ],
    // The engine and gearbox at the back, fuel under the middle, the gun's magazine behind its one crew.
    modules: &[
        Module::new(Part::TrackLeft, [-1.15, -0.25, 0.0], [0.17, 0.25, 2.7], 60),
        Module::new(Part::TrackRight, [1.15, -0.25, 0.0], [0.17, 0.25, 2.7], 60),
        Module::new(Part::Engine, [0.0, 0.0, 1.9], [0.9, 0.35, 0.7], 90),
        Module::new(Part::Transmission, [0.0, -0.2, 2.45], [0.8, 0.2, 0.2], 90),
        Module::new(Part::Fuel, [0.0, -0.2, 1.0], [1.0, 0.2, 0.25], 60),
        Module::new(Part::Ammo, [0.0, 0.1, 0.6], [0.6, 0.3, 0.4], 80),
        Module::new(Part::Breech, [0.0, 0.55, -0.6], [0.2, 0.2, 0.4], 70),
        Module::crew(0, [-0.55, 0.35, -1.0], [0.35, 0.4, 0.45]),
    ],
    canvas: false,
};

/// The Strv S: a tank with a fixed gun in a casemate (a weightless solid, see `add_car`), and quicker: more engine for a little less hull,
/// and a faster pivot.
pub(super) const STRV: VehicleDef = VehicleDef {
    kind: KIND_STRV,
    name: "Strv S",
    hp: 220,
    seats: STRV_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: TRACKS,
    spec: Spec {
        half_extents: STRV_HALF_EXTENTS,
        // 12.6 m³ of hull, about 12.6 tonnes. The engine and suspension cap scale with it.
        density: 1000.0,
        wheel_radius: STRV_WHEEL_RADIUS,
        track_half: STRV_TRACK_HALF,
        axles: &STRV_AXLES_Z,
        hard_point_y: STRV_HARD_POINT_Y,
        suspension_rest: STRV_SUSPENSION_REST,
        max_suspension_force: 750_000.0,
        engine_force: 112_000.0,
        max_speed: STRV_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: STRV_TURN_RATE,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[Solid { shape: Shape::Cuboid(STRV_CASEMATE_HALF), at: STRV_CASEMATE_CENTRE, role: Role::Part }],
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};

/// The driver's main gun, fixed in the hull.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Bore { pitch: (STRV_BARREL_MIN, STRV_BARREL_MAX), slew: STRV_BARREL_SLEW },
    muzzle: Muzzle::One(STRV_GUN_PIVOT),
    fire: Fire::Shell { kind: PROJECTILE_SHELL, alt: PROJECTILE_HE, speed: SHELL_SPEED, length: STRV_BARREL_LENGTH, ttl: SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: SHELL_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: SHELL_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Breech],
    show: Show::Turret,
}];
