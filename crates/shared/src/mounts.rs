//! What a vehicle fires, as data: each kind's `VehicleDef::mounts` lists its weapon mounts, and the
//! server's `Gunnery` mechanic works every one of them with the same code. A mount says who works it
//! (`seat`), how their look turns it (`Aim`), where its rounds leave (`Muzzle`), what leaves (`Fire`),
//! how often, what it runs out of and what stops it. Arming a new vehicle is a row here; a new way of
//! aiming or firing is a variant and its arm in `Gunnery`.

use crate::consts::*;
use crate::protocol::{PROJECTILE_BOMB, PROJECTILE_CANNON, PROJECTILE_EGG, PROJECTILE_MORTAR, PROJECTILE_SHELL, WEAPON_MG};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MountDef {
    /// The seat whose occupant aims and fires it.
    pub seat: usize,
    pub aim: Aim,
    pub muzzle: Muzzle,
    pub fire: Fire,
    /// The `BUTTON_*` that fires it.
    pub trigger: u16,
    /// Ticks between rounds.
    pub cooldown: u32,
    /// How far a round strays from the barrel, radians (0 for none).
    pub spread: f32,
    /// How hard the hull is shoved back along the barrel when it fires (0 for not at all).
    pub recoil: f32,
    /// Heat a round puts in the barrels, of 255 at which they jam; 0 for a gun that never heats.
    pub heat: f32,
    /// What it carries, for a mount that can run out.
    pub ammo: Option<Ammo>,
    /// What has to hold for it to work.
    pub needs: &'static [Need],
    /// Where the snapshot shows it.
    pub show: Show,
}

/// How the occupant's look turns a mount. Yaw is relative to the hull, + turns left; pitch + is up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// It points along the hull: rocket pods, fixed guns, a bomb rack.
    Fixed,
    /// A stabilised turret: it follows the occupant's heading in the world at `slew`, whatever the
    /// hull does, and the barrel their look pitch within `pitch` at `pitch_slew`.
    Turret { slew: f32, pitch: (f32, f32), pitch_slew: f32 },
    /// A gun fixed in the hull that only elevates: yaw stays 0 and the occupant aims sideways by
    /// turning the vehicle.
    Bore { pitch: (f32, f32), slew: f32 },
    /// A gun mounted on the hull that follows the occupant's look as seen from the hull, so it holds
    /// its aim while the hull leans and turns, within `yaw_limit` either side (all the way round for
    /// none) and `pitch`, at `slew`.
    Mount { yaw_limit: Option<f32>, pitch: (f32, f32), slew: f32 },
    /// A mortar: the mount turns to the occupant's heading at `slew`, the range dial (`BUTTON_RANGE_UP`
    /// and `BUTTON_RANGE_DOWN`) sets how far the bomb flies, and the tube takes the high lob that lands
    /// it there.
    Lob { slew: f32 },
}

/// Where a mount's rounds leave it, in the hull frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Muzzle {
    /// One barrel, or one rack, on a mount at this point.
    One([f32; 3]),
    /// Several, which take turns: round n leaves the n-th, around.
    Turns(&'static [[f32; 3]]),
    /// Two barrels hung `drop` under a mount at `pivot`, `x` either side, which take turns and turn
    /// with it: the walker's chin guns.
    Twin { pivot: [f32; 3], x: f32, drop: f32 },
}

/// What leaves a mount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fire {
    /// A shell (`PROJECTILE_*`) out of a barrel `length` long, at `speed`, that bursts on its own after
    /// `ttl` ticks. A barrel poking into a wall bursts it there.
    Shell { kind: u8, speed: f32, length: f32, ttl: u32 },
    /// A rocket that starts at `speed` plus the vehicle's own speed along the barrel.
    Rocket { speed: f32, ttl: u32 },
    /// A machine gun round, a hitscan, from a barrel `length` long.
    Hitscan { length: f32, damage: u8, vehicle: u8, plane: u8, weapon: u8, range: f32 },
    /// Something let go (`PROJECTILE_*`): it leaves with the vehicle's velocity plus `toss` (hull frame).
    Drop { kind: u8, toss: [f32; 3], ttl: u32 },
}

/// What a mount carries and how it gets more.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ammo {
    pub full: u8,
    pub refill: Refill,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refill {
    /// Full again once the vehicle has stood still on the ground for this many ticks.
    Parked(u32),
    /// One more every this many ticks while not full.
    Regrow(u32),
}

/// Something that has to hold for a mount to work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Need {
    /// The vehicle is on its feet (`WALKER_FALLEN_UP`): fallen, it can neither aim nor fire.
    Upright,
    /// A tank's turret is still on: without it, nothing to aim or fire.
    Turret,
    /// It is not being towed: it aims, but cannot fire.
    Untowed,
    /// At least this high over the ground to fire.
    Height(f32),
    /// Pointing at least this far, radians, either side of the nose to fire: clear of the pilot.
    Clear(f32),
}

/// Where the snapshot shows a mount. Until components replicate on their own (protocol v2) a vehicle
/// shows at most one turret (`VehicleState::turret`) and one set of pilot arms (`VehicleState::arms`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// Not at all.
    Hidden,
    /// As the turret: aim, rounds and range, and, when the vehicle has arms, heat and jam as the rear gun's.
    Turret,
    /// As the arms' guns: rounds, heat and jam.
    Guns,
    /// As the arms' bombs, and their rounds when there are no guns.
    Rack,
}

impl Show {
    /// Whether it is part of the pilot's arms, which a fresh vehicle has as new.
    pub fn arms(self) -> bool {
        matches!(self, Show::Guns | Show::Rack)
    }
}

const SHELL: Fire = Fire::Shell { kind: PROJECTILE_SHELL, speed: SHELL_SPEED, length: TANK_BARREL_LENGTH, ttl: SHELL_TTL_TICKS };

pub const TANK_MOUNTS: &[MountDef] = &[MountDef {
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

/// The gunner's nose autocannon, whose rounds are shells in miniature, and the pilot's rocket pods.
pub const HELI_MOUNTS: &[MountDef] = &[
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

/// The gunner's ring mount behind the cockpit, the pilot's two forward guns, which take turns, and
/// the bomb rack, which drops on crouch.
pub const PLANE_MOUNTS: &[MountDef] = &[
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

const PLANE_GUN: Fire = Fire::Hitscan { length: 0.0, damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: PLANE_GUN_RANGE };

impl Fire {
    const fn with_length(self, length: f32) -> Fire {
        match self {
            Fire::Hitscan { damage, vehicle, plane, weapon, range, .. } => Fire::Hitscan { length, damage, vehicle, plane, weapon, range },
            other => other,
        }
    }
}

/// The driver's chin guns: the upper body turns within `WALKER_TORSO_LIMIT` of the legs.
pub const WALKER_MOUNTS: &[MountDef] = &[MountDef {
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

/// The driver's main gun, fixed in the hull.
pub const STRV_MOUNTS: &[MountDef] = &[MountDef {
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

/// The tube, which a towed mortar cannot fire: it is being pulled.
pub const MORTAR_MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Lob { slew: MORTAR_TRAVERSE_SLEW },
    muzzle: Muzzle::One(MORTAR_GUN_PIVOT),
    fire: Fire::Shell { kind: PROJECTILE_MORTAR, speed: MORTAR_SHELL_SPEED, length: MORTAR_BARREL_LENGTH, ttl: MORTAR_SHELL_TTL_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: MORTAR_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: MORTAR_RECOIL,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Untowed],
    show: Show::Turret,
}];

/// The clutch: an egg from under the tail, with the hull's velocity and a little backwards.
pub const HEN_MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Fixed,
    muzzle: Muzzle::One(HEN_VENT),
    fire: Fire::Drop { kind: PROJECTILE_EGG, toss: [0.0, 0.0, HEN_EGG_TOSS], ttl: HEN_EGG_FUSE_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: HEN_EGG_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: 0.0,
    heat: 0.0,
    ammo: Some(Ammo { full: HEN_EGGS, refill: Refill::Regrow(HEN_EGG_REGROW_TICKS) }),
    needs: &[Need::Upright],
    show: Show::Rack,
}];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicles::VEHICLES;

    #[test]
    fn every_mount_is_worked_from_a_seat_its_vehicle_has() {
        for def in &VEHICLES {
            for mount in def.mounts {
                assert!(mount.seat < def.seats, "{}: a mount for seat {}", def.name, mount.seat);
            }
        }
    }

    #[test]
    fn a_vehicle_shows_at_most_one_turret_and_one_of_each_part_of_its_arms() {
        for def in &VEHICLES {
            for show in [Show::Turret, Show::Guns, Show::Rack] {
                assert!(def.mounts.iter().filter(|m| m.show == show).count() <= 1, "{}: two {show:?} mounts", def.name);
            }
        }
    }

    #[test]
    fn a_mount_that_must_point_clear_can_turn_and_one_that_runs_out_carries_some() {
        for def in &VEHICLES {
            for mount in def.mounts {
                if mount.needs.iter().any(|need| matches!(need, Need::Clear(_))) {
                    assert!(matches!(mount.aim, Aim::Mount { .. }), "{}", def.name);
                }
                assert!(mount.ammo.is_none_or(|ammo| ammo.full > 0), "{}", def.name);
            }
        }
    }
}
