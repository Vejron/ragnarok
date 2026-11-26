//! What a vehicle fires, as data: each kind's `VehicleDef::mounts` lists its weapon mounts, and the
//! server's `Gunnery` mechanic works every one of them with the same code. A mount says who works it
//! (`seat`), how their look turns it (`Aim`), where its rounds leave (`Muzzle`), what leaves (`Fire`),
//! how often, what it runs out of and what stops it. Arming a vehicle is a row in its definition
//! (`crate::vehicles`); a new way of aiming or firing is a variant here and its arm in `Gunnery`.

use crate::armor::Pen;
use crate::consts::GRAVITY;

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
    /// Where the camera behind it sits, for a mount the occupant aims with the reticle in the middle
    /// of the screen: the gun is pointed at what that camera's ray meets, not along the look, so the
    /// round goes where the reticle is however far above the barrel the camera is. `None` for a mount
    /// aimed some other way (along the hull, by the range dial, by a plane's nose).
    pub eye: Option<Eye>,
    /// Its seat has a gun sight (aim held, `BUTTON_ALT`): the reticle's camera is then the sight's eye,
    /// `SIGHT_EYE_UP` over the pivot, rather than `eye`.
    pub sight: bool,
    /// The seat whose occupant works it while its own seat is empty: a turret's driver, alone in the
    /// hull, aims and fires it. `None` for nobody.
    pub stand_in: Option<usize>,
}

/// The reticle camera behind a mount: `back` metres behind the gun's pivot along the look and `up`
/// above that, looking along the look. The client draws it from the same numbers (`Scene.aimFrom`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Eye {
    pub back: f32,
    pub up: f32,
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
    /// and `BUTTON_RANGE_DOWN`) sets how far the bomb flies, within `dial`, and the tube rises or falls at
    /// `pitch_slew` to the high lob that lands it there.
    Lob { slew: f32, pitch_slew: f32, dial: Dial },
    /// A gun on a turret's roof that follows the occupant's look in the world, whatever the hull and the
    /// turret do, but only within `yaw_limit` either side of the turret, which carries it round. Its yaw
    /// is from the turret, so a vehicle with one has a `Show::Turret` mount, listed before it. `pitch` at
    /// `slew`, as is its turning.
    Pintle { yaw_limit: f32, pitch: (f32, f32), slew: f32 },
}

/// A mortar's range dial: how far its bombs can be sent and how fast the dial walks. Its lobs are the
/// high ones, the steeper of the two elevations that reach a range, so the shorter the range the closer
/// to vertical the tube stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dial {
    /// The bomb's muzzle speed, m/s: the mount's `Fire::Shell::speed`.
    pub speed: f32,
    /// The shortest and the longest range the dial reaches, metres, and where it starts.
    pub min: f32,
    pub max: f32,
    pub start: f32,
    /// How fast holding a dial button walks it, metres a second.
    pub rate: f32,
}

impl Dial {
    /// The elevation of the high lob that lands a bomb `range` metres out on level ground, radians, kept
    /// within what the dial reaches.
    pub fn pitch(&self, range: f32) -> f32 {
        self.lob(range.clamp(self.min, self.max))
    }

    /// How far a bomb lobbed at elevation `pitch` (radians) lands on level ground, metres: the inverse
    /// of [`Dial::pitch`], so a tube laid by its elevation shows the range that would have laid it.
    pub fn range(&self, pitch: f32) -> f32 {
        self.speed * self.speed * (2.0 * pitch).sin() / GRAVITY
    }

    /// The flattest and the steepest elevation the tube is laid to: the ends of the dial.
    pub fn pitches(&self) -> (f32, f32) {
        (self.lob(self.max), self.lob(self.min))
    }

    fn lob(&self, range: f32) -> f32 {
        let sine = (GRAVITY * range / (self.speed * self.speed)).min(1.0);
        0.5 * (std::f32::consts::PI - sine.asin())
    }
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
    /// A gun held in fist `hand` of a vehicle with arms (`Spec::arms`, 0 the left and 1 the right): its
    /// rounds leave the fist's middle along the forearm, from the elbow to the fist, as the arm is bent
    /// at that moment. It is aimed by moving the arm, so its mount is `Aim::Fixed`; and a hand that
    /// holds a gun never closes on anything.
    Fist { hand: usize },
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
    /// A machine gun round, a bullet out of a barrel `length` long that flies at `BULLET_SPEED` and falls
    /// under `BULLET_GRAVITY_SCALE` of gravity, as a hand gun's does, out to `range`; it goes through `pen`
    /// of armour.
    Bullet { length: f32, damage: u8, vehicle: u8, plane: u8, weapon: u8, range: f32, pen: Pen },
    /// A beam: the trigger held for `charge` ticks starts it, and letting go before then wastes the
    /// charge. It then burns for `burn` ticks whatever the trigger does, one hitscan every `cooldown`, and
    /// needs `recover` ticks before it can charge again. Only the vehicle's driver being gone or fallen
    /// cuts it short.
    Beam { charge: u32, burn: u32, recover: u32, length: f32, radius: f32, damage: u8, vehicle: u8, plane: u8, weapon: u8, range: f32, pen: Pen },
    /// Something let go (`PROJECTILE_*`): it leaves with the vehicle's velocity plus `toss` (hull frame).
    Drop { kind: u8, toss: [f32; 3], ttl: u32 },
    /// A guided missile (a drone `kind`, `KIND_MISSILE`) sent up out of a launch tube, its nose `pitch`
    /// radians up from level towards where the occupant looks, at `speed` m/s. The occupant then flies it
    /// from its nose as a submarine's officer does, their body staying in their seat, and is back in it
    /// once it is gone.
    Guided { kind: u8, speed: f32, pitch: f32 },
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
    /// Full again this many ticks after the last round fired: a crew loading every tube at once, which
    /// starts over with each round that goes.
    Reload(u32),
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
    /// This button (`BUTTON_*`) is not held: a Titan's gun fist thrown as a punch (Shift held) does not
    /// fire.
    Released(u16),
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
    /// As the roof gun: aim, rounds, heat and jam. Its aim is from the turret for an `Aim::Pintle` gun,
    /// and from the hull for any other (a Stridsbåt 90's machine guns, beside its cannon).
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
            Fire::Bullet { damage, vehicle, plane, weapon, range, pen, .. } => Fire::Bullet { length, damage, vehicle, plane, weapon, range, pen },
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
                if let Some(stand_in) = mount.stand_in {
                    assert!(stand_in < def.seats && stand_in != mount.seat, "{}: seat {stand_in} stands in for seat {}", def.name, mount.seat);
                }
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
    fn a_mount_that_must_point_clear_can_turn_one_that_runs_out_carries_some_and_a_mortars_dial_fits_its_bomb() {
        for def in VEHICLES {
            for mount in def.mounts {
                if mount.needs.iter().any(|need| matches!(need, Need::Clear(_))) {
                    assert!(matches!(mount.aim, Aim::Mount { .. }), "{}", def.name);
                }
                assert!(mount.ammo.is_none_or(|ammo| ammo.full > 0), "{}", def.name);
                // A mortar's dial lobs the bomb the mount fires, and reaches no further than it can.
                if let Aim::Lob { dial, .. } = mount.aim {
                    let Fire::Shell { speed, .. } = mount.fire else { panic!("{}: a mortar fires shells", def.name) };
                    assert_eq!(dial.speed, speed, "{}", def.name);
                    assert!(dial.min < dial.start && dial.start < dial.max && dial.max <= speed * speed / GRAVITY + 0.5, "{}", def.name);
                    assert!(dial.pitches().1 < 1.5, "{}: the shortest lob is short of vertical", def.name);
                }
            }
        }
    }

    #[test]
    fn a_gun_in_a_fist_is_held_by_a_vehicle_with_arms_and_a_guided_missile_runs_out() {
        for def in VEHICLES {
            for mount in def.mounts {
                if let Muzzle::Fist { hand } = mount.muzzle {
                    assert!(def.spec.arms.is_some() && hand < 2, "{}", def.name);
                    assert_eq!(mount.aim, Aim::Fixed, "{}", def.name);
                }
                if matches!(mount.fire, Fire::Guided { .. }) {
                    assert!(mount.ammo.is_some(), "{}", def.name);
                }
            }
        }
    }
}
