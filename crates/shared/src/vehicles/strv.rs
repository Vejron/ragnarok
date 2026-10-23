//! The Strv S: everything the game knows about it.

use super::*;

/// The steep glacis turns shots away from well off the nose. The engine is at the back too, under a thin deck.
const ARMOR: Armor = Armor {
    front_slope: 0.96,
    ricochet: [58.0, 72.0, 75.0, 78.0],
    mult: [1.0, 1.0, 1.4, 1.3],
    weak: &[WeakSpot { centre: [0.0, 0.2, 2.0], half: [1.1, 0.4, 0.7], mult: 1.6 }, WeakSpot { centre: [0.0, 0.6, -0.3], half: [0.7, 0.2, 0.9], mult: 1.3 }],
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
    fire: Fire::Shell { kind: PROJECTILE_SHELL, speed: SHELL_SPEED, length: STRV_BARREL_LENGTH, ttl: SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: SHELL_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: SHELL_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[],
    show: Show::Turret,
}];
