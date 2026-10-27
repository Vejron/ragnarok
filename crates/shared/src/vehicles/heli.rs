//! The helicopter: everything the game knows about it.

use super::*;

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
        // 3.8 m³ of hull is about 1.1 tonnes. Nothing below uses the wheel fields.
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
        turn_rate: 0.0,
        lift: Lift::Rotor,
        // Air drag is what limits the speed: the rotor's lean gives about 4.9 m/s², so about 25 m/s.
        linear_damping: 0.2,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: 0.0,
    },
    armor: LIGHT,
    drone: None,
    mount_seat: None,
};

/// The gunner's nose autocannon, whose rounds are shells in miniature, and the pilot's rocket pods.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: Some(HELI_GUN_YAW_LIMIT), pitch: (HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX), slew: HELI_GUN_SLEW },
        muzzle: Muzzle::One(HELI_GUN_PIVOT),
        fire: Fire::Shell { kind: PROJECTILE_CANNON, speed: SHELL_SPEED, length: HELI_GUN_LENGTH, ttl: SHELL_TTL_TICKS },
        trigger: BUTTON_FIRE,
        cooldown: HELI_GUN_COOLDOWN_TICKS,
        spread: HELI_GUN_SPREAD,
        recoil: 0.0,
        heat: 0.0,
        ammo: None,
        needs: &[],
        show: Show::Turret,
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
    },
];
