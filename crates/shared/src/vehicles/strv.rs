//! The Strv S: everything the game knows about it.

use super::*;

/// The Strv S (`KIND_STRV`): a turretless tank with the gun fixed in the hull, the size and shape of a
/// Stridsvagn 103. One seat: the driver steers by pivoting the hull and fires the main gun, which only
/// the barrel's elevation can aim. Quicker and lower than the tank, and a wedge: its glacis is half its
/// length. Same body axes as the tank; mirrors `STRV_*` in `sim/consts.ts`.
pub const STRV_SEATS: usize = 1;
/// Half the hull's bounding box, metres: 7.04 m long, 3.4 m wide over the tracks and 1.4 m deep from the
/// belly to the roof, which is 1.9 m off the ground.
pub const STRV_HALF_EXTENTS: [f32; 3] = [1.7, 0.7, 3.52];
/// Bigger wheels and a longer suspension than the tank's, with the hull higher on them: it steps over
/// obstacles the tank has to go around.
pub const STRV_WHEEL_RADIUS: f32 = 0.5;
pub const STRV_TRACK_HALF: f32 = 1.36;
/// Z of the three axles, front first.
pub const STRV_AXLES_Z: [f32; 3] = [-2.4, 0.0, 2.4];
/// Where the hull's centre settles over flat ground, metres: measured by `scripted_drive_matches_fixture`.
pub const STRV_RIDE_HEIGHT: f32 = 1.2;
pub const STRV_HARD_POINT_Y: f32 = -0.3;
pub const STRV_SUSPENSION_REST: f32 = 0.46;
pub const STRV_MAX_SPEED: f32 = 16.0;
/// Yaw rate at full stick, rad/s.
pub const STRV_TURN_RATE: f32 = 1.5;
/// Where the gun's trunnion is in the hull frame (where the barrel leaves the glacis), how far the muzzle
/// is from it (2 m past the nose), how fast the barrel elevates (rad/s) and its limits, radians (+ is
/// up). The gun points where the hull does.
pub const STRV_GUN_PIVOT: [f32; 3] = [0.0, 0.5, -1.0];
pub const STRV_BARREL_LENGTH: f32 = 4.5;
pub const STRV_BARREL_SLEW: f32 = 1.2;
pub const STRV_BARREL_MIN: f32 = -0.15;
pub const STRV_BARREL_MAX: f32 = 0.3;

/// The hull's side outline, `[z, y]`: a wedge. From a sharp nose the glacis climbs at 13 degrees above
/// level for half the hull's length to a flat roof, the rear deck falls a little to the tail, and the
/// plate under the nose leans far back to the belly.
const PROFILE: &[[f32; 2]] = &[[-3.52, -0.1], [0.0, 0.7], [1.6, 0.7], [3.52, 0.5], [3.52, -0.2], [3.1, -0.7], [-2.6, -0.7]];

/// The long glacis lies so nearly flat that a level shot meets it at 77 degrees and skids off, as does
/// one into the plate under the nose: from ahead only a shot plunging from well above, or one through
/// the gunner's sight, gets in. The sides, the tail and the decks are thin. The engine is at the back,
/// under a thin deck, and the driver (who is the gunner too) looks out through a sight in the glacis.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::slit("periscope", Zone::Top, Box3::new([-0.6, 0.6, -0.45], [0.15, 0.06, 0.2])).looks(Zone::Front),
        Plate::hull("glacis", Zone::Top, 40.0).within(Box3::new([0.0, 0.3, -1.76], [1.7, 0.45, 1.77])).glances(68.0),
        Plate::hull("engine deck", Zone::Top, 8.0).within(Box3::new([0.0, 0.6, 2.56], [1.7, 0.15, 0.97])).glances(78.0),
        Plate::hull("lower front", Zone::Belly, 40.0).within(Box3::new([0.0, -0.4, -3.06], [1.7, 0.35, 0.47])).glances(54.0),
        Plate::hull("hull side", Zone::Side, 25.0).glances(72.0),
        Plate::hull("hull rear", Zone::Back, 20.0).glances(75.0),
        Plate::hull("hull roof", Zone::Top, 12.0).glances(78.0),
        Plate::hull("belly", Zone::Belly, 10.0).glances(78.0),
    ],
    // The engine and gearbox at the back, fuel under the middle, the gun's magazine behind its one crew.
    modules: &[
        Module::new(Part::TrackLeft, [-1.36, -0.35, 0.0], [0.34, 0.35, 3.52], 60),
        Module::new(Part::TrackRight, [1.36, -0.35, 0.0], [0.34, 0.35, 3.52], 60),
        Module::new(Part::Engine, [0.0, 0.0, 2.6], [1.1, 0.45, 0.8], 90),
        Module::new(Part::Transmission, [0.0, -0.3, 3.25], [1.0, 0.3, 0.2], 90),
        Module::new(Part::Fuel, [0.0, -0.35, 1.5], [1.2, 0.3, 0.3], 60),
        Module::new(Part::Ammo, [0.0, 0.1, 0.9], [0.7, 0.4, 0.5], 80),
        Module::new(Part::Breech, [0.0, 0.45, -0.3], [0.22, 0.2, 0.5], 70),
        Module::crew(0, [-0.65, 0.2, -0.3], [0.4, 0.45, 0.5]),
    ],
    canvas: false,
};

/// The Strv S: a tank with a fixed gun in a wedge of a hull, and quicker: more engine for a little less
/// weight, and a faster pivot.
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
        // The hull's box at 376 kg/m³ is about 12.6 tonnes.
        density: 376.0,
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
        solids: &[],
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
    exit: None,
    fittings: &[],
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
    eye: Some(Eye { back: STRV_AIM_BACK, up: STRV_AIM_UP }),
    sight: false,
}];
