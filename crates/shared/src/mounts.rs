//! What a vehicle fires, as data: each kind's `VehicleDef::mounts` lists its weapon mounts, and the
//! server's `Gunnery` mechanic works every one of them with the same code. A mount says who works it
//! (`seat`), how their look turns it (`Aim`), where its rounds leave (`Muzzle`), what leaves (`Fire`),
//! how often, what it runs out of and what stops it. Arming a vehicle is a row in its definition
//! (`crate::vehicles`); a new way of aiming or firing is a variant here and its arm in `Gunnery`.

use crate::armor::Pen;

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
    /// A gun on a turret's roof that follows the occupant's look in the world, whatever the hull and the
    /// turret do, but only within `yaw_limit` either side of the turret, which carries it round. Its yaw
    /// is from the turret, so a vehicle with one has a `Show::Turret` mount, listed before it. `pitch` at
    /// `slew`, as is its turning.
    Pintle { yaw_limit: f32, pitch: (f32, f32), slew: f32 },
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
    /// One barrel at `at` in the turret's frame, on a turret ring at `ring`: it goes round with the turret.
    Pintle { ring: [f32; 3], at: [f32; 3] },
}

/// What leaves a mount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fire {
    /// A shell (`PROJECTILE_*`) out of a barrel `length` long, at `speed`, that bursts on its own after
    /// `ttl` ticks. A barrel poking into a wall bursts it there. `alt` is the other kind of round it can
    /// be loaded with instead (`BUTTON_AMMO`), the same as `kind` for a gun with only the one.
    Shell { kind: u8, alt: u8, speed: f32, length: f32, ttl: u32 },
    /// A rocket that starts at `speed` plus the vehicle's own speed along the barrel.
    Rocket { speed: f32, ttl: u32 },
    /// A machine gun round, a hitscan, from a barrel `length` long, going through `pen` of armour.
    Hitscan { length: f32, damage: u8, vehicle: u8, plane: u8, weapon: u8, range: f32, pen: Pen },
    /// A beam: the trigger held for `charge` ticks starts it, and letting go before then wastes the
    /// charge. It then burns for `burn` ticks whatever the trigger does, one hitscan every `cooldown`, and
    /// needs `recover` ticks before it can charge again. Only the vehicle's driver being gone or fallen
    /// cuts it short.
    Beam { charge: u32, burn: u32, recover: u32, length: f32, radius: f32, damage: u8, vehicle: u8, plane: u8, weapon: u8, range: f32, pen: Pen },
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
    /// The gun's breech (`armor::Part::Breech`) has not been shot to pieces: it aims, but cannot fire.
    Breech,
    /// At least this high over the ground to fire.
    Height(f32),
    /// Pointing at least this far, radians, either side of the nose to fire: clear of the pilot.
    Clear(f32),
}

/// Where the snapshot shows a mount. Until components replicate on their own (protocol v2) a vehicle
/// shows at most one turret (the `turret` component), one roof gun (`pintle`) and one set of pilot arms
/// (`arms`).
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
    /// As the roof gun: aim from the turret, rounds, heat and jam.
    Pintle,
}

impl Show {
    /// Whether it is part of the pilot's arms, which a fresh vehicle has as new.
    pub fn arms(self) -> bool {
        matches!(self, Show::Guns | Show::Rack)
    }
}

impl Fire {
    /// The same machine gun round from a barrel `length` long.
    pub(crate) const fn with_length(self, length: f32) -> Fire {
        match self {
            Fire::Hitscan { damage, vehicle, plane, weapon, range, pen, .. } => Fire::Hitscan { length, damage, vehicle, plane, weapon, range, pen },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicles::VEHICLES;

    #[test]
    fn every_mount_is_worked_from_a_seat_its_vehicle_has() {
        for def in VEHICLES {
            for mount in def.mounts {
                assert!(mount.seat < def.seats, "{}: a mount for seat {}", def.name, mount.seat);
            }
        }
    }

    #[test]
    fn a_vehicle_shows_at_most_one_turret_and_one_of_each_part_of_its_arms() {
        for def in VEHICLES {
            for show in [Show::Turret, Show::Guns, Show::Rack, Show::Pintle] {
                assert!(def.mounts.iter().filter(|m| m.show == show).count() <= 1, "{}: two {show:?} mounts", def.name);
            }
        }
    }

    #[test]
    fn a_pintle_gun_rides_a_turret_listed_before_it() {
        for def in VEHICLES {
            for (index, mount) in def.mounts.iter().enumerate() {
                if matches!(mount.aim, Aim::Pintle { .. }) || matches!(mount.muzzle, Muzzle::Pintle { .. }) {
                    assert!(def.mounts[..index].iter().any(|m| m.show == Show::Turret), "{}", def.name);
                    assert_eq!(mount.show, Show::Pintle, "{}", def.name);
                }
            }
        }
    }

    #[test]
    fn a_mount_that_must_point_clear_can_turn_and_one_that_runs_out_carries_some() {
        for def in VEHICLES {
            for mount in def.mounts {
                if mount.needs.iter().any(|need| matches!(need, Need::Clear(_))) {
                    assert!(matches!(mount.aim, Aim::Mount { .. }), "{}", def.name);
                }
                assert!(mount.ammo.is_none_or(|ammo| ammo.full > 0), "{}", def.name);
            }
        }
    }
}
