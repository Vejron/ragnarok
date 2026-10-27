//! The triplane: everything the game knows about it.

use super::*;

const PLANE_GUN: Fire = Fire::Hitscan { length: 0.0, damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: PLANE_GUN_RANGE };

pub(super) const PLANE: VehicleDef = VehicleDef {
    kind: KIND_PLANE,
    name: "Triplane",
    hp: 70,
    seats: PLANE_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &PLANE_SEAT_HEADS,
    crew_guns: false,
    body: WINGS,
    spec: Spec {
        half_extents: PLANE_HALF_EXTENTS,
        // 6.7 m³ of fuselage is about 600 kg, a Dr.I fully loaded. The wings are a weightless second box.
        density: 90.0,
        wheel_radius: PLANE_WHEEL_RADIUS,
        track_half: PLANE_TRACK_HALF,
        axles: &PLANE_AXLES_Z,
        hard_point_y: PLANE_HARD_POINT_Y,
        suspension_rest: PLANE_SUSPENSION_REST,
        max_suspension_force: 30_000.0,
        // The wheels only roll and steer; the propeller is a force on the body.
        engine_force: 0.0,
        max_speed: 0.0,
        steer_lock: PLANE_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wing,
        // Drag is `PLANE_DRAG`, by the square of the speed.
        linear_damping: 0.0,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[Solid { shape: Shape::Cuboid(PLANE_WING_HALF_EXTENTS), at: PLANE_WING_CENTRE, role: Role::Wings }],
        tongue: 0.0,
    },
    armor: SOFT,
    drone: None,
    mount_seat: None,
};

/// The gunner's ring mount behind the cockpit, the pilot's two forward guns, which take turns, and
/// the bomb rack, which drops on crouch.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: None, pitch: (PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX), slew: PLANE_REAR_SLEW },
        muzzle: Muzzle::One(PLANE_REAR_PIVOT),
        fire: PLANE_GUN.with_length(PLANE_REAR_LENGTH),
        trigger: BUTTON_FIRE,
        cooldown: PLANE_REAR_COOLDOWN_TICKS,
        spread: PLANE_GUN_SPREAD,
        recoil: 0.0,
        heat: PLANE_REAR_HEAT,
        ammo: None,
        needs: &[Need::Clear(PLANE_REAR_DEAD_ZONE)],
        show: Show::Turret,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Turns(&PLANE_GUN_PIVOTS),
        fire: PLANE_GUN,
        trigger: BUTTON_FIRE,
        cooldown: PLANE_GUN_COOLDOWN_TICKS,
        spread: PLANE_GUN_SPREAD,
        recoil: 0.0,
        heat: PLANE_GUN_HEAT,
        ammo: None,
        needs: &[],
        show: Show::Guns,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::One(PLANE_BOMB_PIVOT),
        fire: Fire::Drop { kind: PROJECTILE_BOMB, toss: [0.0; 3], ttl: SHELL_TTL_TICKS * 2 },
        trigger: BUTTON_CROUCH,
        cooldown: PLANE_BOMB_COOLDOWN_TICKS,
        spread: 0.0,
        recoil: 0.0,
        heat: 0.0,
        ammo: Some(Ammo { full: PLANE_BOMBS, refill: Refill::Parked(PLANE_REARM_TICKS) }),
        // The rack only lets go in the air, where the bomb has room to fall clear.
        needs: &[Need::Height(PLANE_BOMB_MIN_HEIGHT)],
        show: Show::Rack,
    },
];
