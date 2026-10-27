//! The field mortar: everything the game knows about it.

use super::*;

/// The field mortar: a towed gun on two wheels. Nothing drives it, so it has no engine; its wheels only
/// roll it, and the brake holds it once it is unhitched.
pub(super) const MORTAR: VehicleDef = VehicleDef {
    kind: KIND_MORTAR,
    name: "Field mortar",
    hp: 150,
    seats: MORTAR_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &[],
    crew_guns: false,
    body: TOWED,
    spec: Spec {
        half_extents: MORTAR_HALF_EXTENTS,
        // 6 m³ of steel, about 900 kg: a jeep can pull it, a sports car struggles.
        density: 150.0,
        wheel_radius: MORTAR_WHEEL_RADIUS,
        track_half: MORTAR_TRACK_HALF,
        // Two axles, one fore and one aft, so it stands on four wheels and does not teeter on a pair at its middle.
        axles: &[-0.9, 0.9],
        hard_point_y: -0.25,
        suspension_rest: 0.3,
        max_suspension_force: 30_000.0,
        engine_force: 0.0,
        max_speed: 1.0,
        steer_lock: 0.0,
        turn_rate: 0.0,
        lift: Lift::Wheels,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: false,
        solids: &[],
        tongue: -(MORTAR_HALF_EXTENTS[2] + TOW_BACK),
    },
    armor: ALLOY,
    drone: None,
    mount_seat: Some(MountSeat { pivot: MORTAR_GUN_PIVOT, head: MORTAR_SEAT_HEAD }),
};

/// The tube, which a towed mortar cannot fire: it is being pulled.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Lob { slew: MORTAR_TRAVERSE_SLEW },
    muzzle: Muzzle::One(MORTAR_GUN_PIVOT),
    fire: Fire::Shell { kind: PROJECTILE_MORTAR, alt: PROJECTILE_MORTAR, speed: MORTAR_SHELL_SPEED, length: MORTAR_BARREL_LENGTH, ttl: MORTAR_SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: MORTAR_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: MORTAR_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Untowed],
    show: Show::Turret,
}];
