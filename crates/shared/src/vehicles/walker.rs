//! The walker: everything the game knows about it.

use super::*;

/// A walker's two legs under a hull `hy` high. They hang from the bottom of the hull and stop short of
/// the ground, so a car or a shell hits them but the springs, not the boxes, carry the walker.
pub(super) const fn legs(hy: f32) -> [Solid; 2] {
    let half = (WALKER_LEG_LENGTH - WALKER_LEG_CLEARANCE) / 2.0;
    let leg = Shape::Cuboid([WALKER_LEG_HALF_WIDTH, half, WALKER_LEG_HALF_DEPTH]);
    [
        Solid { shape: leg, at: [-WALKER_FOOT_HALF, -hy - half, 0.0], role: Role::Part },
        Solid { shape: leg, at: [WALKER_FOOT_HALF, -hy - half, 0.0], role: Role::Part },
    ]
}

const LEG_SOLIDS: [Solid; 2] = legs(WALKER_HALF_EXTENTS[1]);

pub(super) const WALKER: VehicleDef = VehicleDef {
    kind: KIND_WALKER,
    name: "Walker",
    hp: 200,
    seats: WALKER_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: LEGS,
    spec: Spec {
        half_extents: WALKER_HALF_EXTENTS,
        // 22.5 m³ of hull at this density is about 5.6 tonnes. Nothing below uses the wheel fields; the legs
        // are the `WALKER_*` constants.
        density: 250.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: WALKER_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: WALKER_TURN_RATE,
        lift: Lift::Walker,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &LEG_SOLIDS,
    },
    armor: LIGHT,
    drone: None,
    mount_seat: None,
};

/// The driver's chin guns: the upper body turns within `WALKER_TORSO_LIMIT` of the legs.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Mount { yaw_limit: Some(WALKER_TORSO_LIMIT), pitch: (WALKER_GUN_PITCH_MIN, WALKER_GUN_PITCH_MAX), slew: WALKER_GUN_SLEW },
    muzzle: Muzzle::Twin { pivot: WALKER_GUN_PIVOT, x: WALKER_GUN_BARREL_X, drop: WALKER_GUN_BARREL_DROP },
    fire: Fire::Shell { kind: PROJECTILE_CANNON, speed: SHELL_SPEED, length: WALKER_GUN_LENGTH, ttl: SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: WALKER_GUN_COOLDOWN_TICKS,
    spread: WALKER_GUN_SPREAD,
    recoil: 0.0,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Upright],
    show: Show::Turret,
}];
