//! The tank: everything the game knows about it.

use super::*;

/// Its weak spot is the engine deck, over the back of the hull.
const ARMOR: Armor = Armor { front_slope: 0.35, ricochet: [66.0, 74.0, 78.0, 80.0], mult: [1.0, 1.0, 1.35, 1.3], weak: &[WeakSpot { centre: [0.0, 0.3, 2.0], half: [1.1, 0.4, 0.6], mult: 1.5 }], canvas: false };

const SHELL: Fire = Fire::Shell { kind: PROJECTILE_SHELL, speed: SHELL_SPEED, length: TANK_BARREL_LENGTH, ttl: SHELL_TTL_TICKS };

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
    needs: &[Need::Turret],
    show: Show::Turret,
}];
