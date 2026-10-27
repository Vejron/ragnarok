//! Armor: what a shot does to a vehicle depends on where it lands, at what angle, and what it can go
//! through.
//!
//! A hit is judged in the vehicle's own frame (+X right, +Y up, -Z forward) from the solid it struck,
//! the surface normal and the point (see `Physics::armor_strike`); a turret's plates in the turret's
//! frame, turned as the gun is. The first of the kind's plates (`shared::armor`) that covers that spot is
//! the one met. A sloped plate leans its normal back, and the angle between the shot and that normal is
//! the incidence:
//!
//! - past the plate's `ricochet` angle the shot glances off and does nothing, unless it is a round so
//!   much bigger than the plate that it bites in anyway (`OVERMATCH`),
//! - otherwise it has to get through `mm / cos(incidence)` of steel. A round that does not is stopped
//!   (an HE shell's blast still does a little, `HE_BLOCKED_SHARE`); one that does lands for between
//!   half and all of its damage, the more the more it had to spare, and goes on inside with what is
//!   left over.
//!
//! No dice: the same shot at the same place always does the same thing, so the client and the tests
//! can tell what will happen.

use crate::game::{Burning, Mounts, Player, Seats, Vehicle, World};
use sim::Strike;
use shared::armor::{Box3, Module, PARTS, Part, PenKind, State, Zone, pack};
use shared::consts::*;
use shared::protocol::{ARMOR_BLOCKED, ARMOR_PIERCED, ARMOR_RICOCHET, ARMOR_SLIT, BURNING, DAMAGE_FIRE, DAMAGE_PARTS, DAMAGE_TURRET, HullMark, MARK_GLANCE, MARK_HOLE, MARK_PIERCED, MARK_SCUFF, MARK_VITAL, ShotEvent, TURRET_GONE, WEAPON_TURRET, WEAPON_WRECK};
use shared::quant::{to_cm, to_cm_i16};
use shared::vehicles::{Role, vehicle_def};

/// A direct hit lands this much of the shell's blast damage, before armor: a blast scaled by its distance
/// from the vehicle's centre would punish a hit on the hull's near face.
pub const DIRECT_SHARE: f32 = 0.8;
/// A glancing shot keeps this much of its speed.
pub const RICOCHET_KEEP: f32 = 0.6;
/// A round that goes through this many times the plate's thickness does not glance off it, however
/// shallow the angle: it bites in.
const OVERMATCH: f32 = 3.0;
/// An HE shell stopped by the plate still does this much of what it would have through it.
pub const HE_BLOCKED_SHARE: f32 = 0.15;
/// An armoured vehicle (`Armor::armoured`) takes this much of a blast's splash: the rest is turned by
/// its plates.
pub const SPLASH_ARMOURED: f32 = 0.2;
/// How shallow an angle the thickness is reckoned at, at most: past it the plate counts as this thick.
const MIN_COS: f32 = 0.1;

/// What armor makes of a shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// It glances off along `normal` (the effective surface normal, in the hull frame).
    Ricochet { normal: [f32; 3] },
    /// It is stopped by `plate`, which was `mm` thick along the way it came, at `at` (the hull frame).
    Blocked { turret: bool, plate: &'static str, mm: f32, at: [f32; 3] },
    /// It goes through at `at` (the hull frame), for `scale` of its damage, with `residual` mm of
    /// penetration left over. `slit`: through a gap rather than a plate.
    Pierced { turret: bool, plate: &'static str, scale: f32, residual: f32, slit: bool, at: [f32; 3] },
}

impl Outcome {
    /// How much of its damage a round of `kind` does to the vehicle.
    pub fn share(&self, kind: PenKind) -> f32 {
        match *self {
            Outcome::Ricochet { .. } => 0.0,
            Outcome::Blocked { .. } if kind == PenKind::He => HE_BLOCKED_SHARE,
            Outcome::Blocked { .. } => 0.0,
            // High explosive that gets through bursts in full; a solid shot does the more, the more it had to spare.
            Outcome::Pierced { .. } if kind == PenKind::He => 1.0,
            Outcome::Pierced { scale, .. } => scale,
        }
    }

    /// Whether the turret took it.
    pub fn turret(&self) -> bool {
        matches!(*self, Outcome::Blocked { turret: true, .. } | Outcome::Pierced { turret: true, .. })
    }

    /// Where it met the vehicle, in the hull frame; `None` for a glance.
    pub fn at(&self) -> Option<[f32; 3]> {
        match *self {
            Outcome::Ricochet { .. } => None,
            Outcome::Blocked { at, .. } | Outcome::Pierced { at, .. } => Some(at),
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(1e-6);
    v.map(|c| c / len)
}

/// `v` turned by `yaw` about the vertical, the way a turret turns (yaw 0 faces -Z, + turns left).
fn yawed(v: [f32; 3], yaw: f32) -> [f32; 3] {
    let (sin, cos) = yaw.sin_cos();
    [v[0] * cos + v[2] * sin, v[1], -v[0] * sin + v[2] * cos]
}

/// Judges a shot travelling along `dir` (a unit vector, in the hull frame) that met a `kind` at `strike`,
/// able to go through `pen` mm of steel, with the kind's turret (if it has one) turned to `turret_yaw`.
pub fn judge(kind: u8, strike: &Strike, dir: [f32; 3], pen: f32, turret_yaw: f32) -> Outcome {
    let def = vehicle_def(kind);
    let solid = strike.solid.and_then(|index| def.spec.solids.get(index as usize));
    let turret = solid.is_some_and(|solid| solid.role == Role::Turret);
    let len = dot(strike.normal, strike.normal).sqrt();
    // A shot that began inside a solid has no surface to glance off, and nothing to get through.
    if len < 0.5 {
        return Outcome::Pierced { turret, plate: "inside", scale: 1.0, residual: pen, slit: false, at: strike.point };
    }
    let at = strike.point;
    let (mut normal, mut point, mut dir) = (strike.normal.map(|v| v / len), strike.point, dir);
    // A turret's plates are where the turret has turned them: judge in its frame.
    if let (true, Some(solid)) = (turret, solid) {
        let from = [point[0] - solid.at[0], point[1] - solid.at[1], point[2] - solid.at[2]];
        let local = yawed(from, -turret_yaw);
        point = [local[0] + solid.at[0], local[1] + solid.at[1], local[2] + solid.at[2]];
        normal = yawed(normal, -turret_yaw);
        dir = yawed(dir, -turret_yaw);
    }
    let Some(plate) = def.armor.plate(strike.solid, Zone::of(normal), point) else {
        return Outcome::Pierced { turret, plate: "bare", scale: 1.0, residual: pen, slit: false, at };
    };
    if plate.slit {
        return Outcome::Pierced { turret, plate: plate.name, scale: 1.0, residual: pen, slit: true, at };
    }
    // A sloped plate leans back from its face: its normal is turned up by the slope.
    let mut effective = normal;
    if plate.slope != 0.0 && normal[1].abs() < 0.9 {
        let (sin, cos) = plate.slope.to_radians().sin_cos();
        effective = unit([normal[0] * cos, normal[1] * cos + sin, normal[2] * cos]);
    }
    let cos_incidence = (-dot(dir, effective)).clamp(-1.0, 1.0);
    let incidence = cos_incidence.acos().to_degrees();
    let overmatched = pen >= plate.mm * OVERMATCH;
    if incidence > plate.ricochet && !overmatched {
        // Back into the hull's frame, for the reflection.
        let normal = if turret { yawed(effective, turret_yaw) } else { effective };
        return Outcome::Ricochet { normal };
    }
    let mm = plate.mm / cos_incidence.max(MIN_COS);
    if pen < mm {
        return Outcome::Blocked { turret, plate: plate.name, mm, at };
    }
    let residual = pen - mm;
    Outcome::Pierced { turret, plate: plate.name, scale: 0.5 + 0.5 * residual / pen.max(1e-3), residual, slit: false, at }
}

/// What vehicle `vid`'s armor makes of a shot from `origin` along the unit `dir` that reaches it within
/// `max` metres and can go through `pen` mm. `None` if the shot does not touch it.
pub fn armor_of(world: &World, vid: u32, origin: [f32; 3], dir: [f32; 3], max: f32, pen: f32) -> Option<Outcome> {
    let kind = world.entities.get::<Vehicle>(vid)?.kind;
    let pose = world.physics.car_pose(vid)?;
    let strike = world.physics.armor_strike(origin, dir, max + 0.05, vid)?;
    let turret_yaw = world.entities.get::<Mounts>(vid).and_then(|mounts| mounts.turret().map(|mount| mount.yaw)).unwrap_or(0.0);
    Some(judge(kind, &strike, pose.local_dir(dir), pen, turret_yaw))
}

/// Whether vehicle `vid` is armoured against small arms and splash.
pub fn armoured(world: &World, vid: u32) -> bool {
    world.entities.get::<Vehicle>(vid).is_some_and(|vehicle| vehicle_def(vehicle.kind).armor.armoured())
}

/// The direction after glancing off a surface with unit `normal`: mirrored about it.
pub fn reflect(dir: [f32; 3], normal: [f32; 3]) -> [f32; 3] {
    let along = dot(dir, normal);
    std::array::from_fn(|i| dir[i] - 2.0 * along * normal[i])
}

/// What vehicle `vehicle`'s armour made of a round from `attacker` that struck it at `point`, and what
/// the round then did inside, for the shooter's and the crew's callouts (`S_ARMOR`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmorHit {
    pub vehicle: u32,
    pub attacker: Option<u32>,
    /// `ARMOR_*`.
    pub result: u8,
    pub inside: Inside,
    pub point: [f32; 3],
}

/// Tells everyone near what vehicle `vid`'s armour made of a round of `calibre` mm going along `dir`
/// that met it at world `point` (a spark for a glance, a scuff for a round it stopped, a hole for one it
/// did not, as a `HullMark` in the vehicle's frame), and the shooter and crew what it did.
///
/// A squad's rifles on a tank would bury it in scuffs: a small round that does not get in leaves a mark
/// only if the vehicle has no glance or scuff waiting for the snapshot yet. Holes and shells always do.
/// The shooter and the crew are told of every one.
#[allow(clippy::too_many_arguments)]
pub fn report(world: &mut World, vid: u32, outcome: &Outcome, point: [f32; 3], dir: [f32; 3], calibre: f32, inside: Inside, attacker: Option<u32>) {
    let hole = if calibre >= SPALL_CALIBRE { MARK_PIERCED } else { MARK_HOLE };
    let (result, style) = match *outcome {
        Outcome::Ricochet { .. } => (ARMOR_RICOCHET, MARK_GLANCE),
        Outcome::Blocked { .. } => (ARMOR_BLOCKED, MARK_SCUFF),
        Outcome::Pierced { slit: true, .. } => (ARMOR_SLIT, hole),
        Outcome::Pierced { .. } => (ARMOR_PIERCED, hole),
    };
    // A bullet through a car's skin that hit nothing inside is not worth a callout: a burst into a jeep
    // would bury the ones that are.
    let told = result != ARMOR_PIERCED || armoured(world, vid) || inside != Inside::default();
    if told {
        world.armor_hits.push(ArmorHit { vehicle: vid, attacker, result, inside, point });
    }
    let style = if inside.hurt != 0 && matches!(style, MARK_HOLE | MARK_PIERCED) { style | MARK_VITAL } else { style };
    let glance = matches!(style, MARK_GLANCE | MARK_SCUFF);
    if glance && calibre < SPALL_CALIBRE && world.marks.iter().any(|(mark, _)| mark.vehicle == vid && matches!(mark.style, MARK_GLANCE | MARK_SCUFF)) {
        return;
    }
    let Some(pose) = world.physics.car_pose(vid) else { return };
    let [x, y, z] = pose.local_dir(std::array::from_fn(|i| point[i] - pose.position[i])).map(to_cm_i16);
    let [dx, dy, dz] = pose.local_dir(dir).map(|v| (v * 127.0).round().clamp(-127.0, 127.0) as i8);
    world.marks.push((HullMark { vehicle: vid, style, x, y, z, dx, dy, dz }, pose.position));
}

/// Hits of `damage` that land on the turret of tank `vid`. At zero the turret comes off: it is gone
/// until the tank is put back, its solid goes, the gunner dies with it and `DAMAGE_TURRET` tells clients.
pub fn damage_turret(world: &mut World, vid: u32, damage: u8, attacker: Option<u32>) {
    if world.entities.dead(vid) {
        return;
    }
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return };
    if !vehicle_def(vehicle.kind).sheds_turret() || vehicle.damage[DAMAGE_TURRET] == TURRET_GONE {
        return;
    }
    vehicle.turret_hp = vehicle.turret_hp.saturating_sub(damage);
    if vehicle.turret_hp > 0 {
        return;
    }
    vehicle.damage[DAMAGE_TURRET] = TURRET_GONE;
    let gunner = world.entities.get_mut::<Seats>(vid).and_then(|mut seats| seats.take(1));
    world.physics.set_turret(vid, false);
    if let Some(pose) = world.physics.car_pose(vid) {
        let at = pose.world_point(TANK_TURRET_PIVOT);
        let [x, y, z] = at.map(to_cm);
        world.shots.push(ShotEvent { shooter: vid, weapon: WEAPON_TURRET, x, y, z });
        if let Some(id) = gunner {
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.seat = None;
                player.character.position = at;
            }
            super::life::damage_entity(world, id, super::life::Harm::new(PLAYER_MAX_HP, attacker, WEAPON_WRECK));
        }
    }
}

// --- inside: parts and crew ---

/// A round's main hit on a part does this, plus this much per mm of penetration it had to spare (a
/// round that only just got through has little left to do harm with)...
const PART_HIT: f32 = 30.0;
const PART_PER_MM: f32 = 1.5;
/// ...and keeps this much of it for the next part along its way, until it is spent.
const PART_KEEP: f32 = 0.7;
const PART_SPENT: f32 = 10.0;
/// How far, metres, a round goes on inside a vehicle, at most.
const INSIDE_REACH: f32 = 8.0;
/// Rounds of at least this calibre (mm) throw fragments off the inside of the plate they go through,
/// and break tracks.
const SPALL_CALIBRE: f32 = 20.0;
/// The fragments: how many at the least and most, the half-angle of their cone, how far they fly,
/// and what each does to a part and to a person.
const SPALL_MIN: usize = 2;
const SPALL_MAX: usize = 8;
const SPALL_CONE: f32 = 0.45;
const SPALL_REACH: f32 = 3.0;
const SPALL_PART: f32 = 15.0;
const SPALL_CREW: u8 = 45;
/// A track is broken by a shell that bursts within this many metres of it, for its blast damage times
/// this share, falling to nothing at the edge.
const TRACK_SPLASH_REACH: f32 = 1.5;
const TRACK_SPLASH_SHARE: f32 = 0.5;
/// A shell that strikes a track does it this, plus this much per mm of its calibre, beyond its burst.
const TRACK_HIT: f32 = 20.0;
const TRACK_PER_CALIBRE: f32 = 0.5;
/// Burning fuel takes one health every this many ticks.
const BURN_EVERY: u32 = 10;

/// What a round that got into a vehicle did there, for the shooter's hit marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inside {
    /// Parts it hurt, and parts it finished off, as bits by `Part::slot`.
    pub hurt: u8,
    pub gone: u8,
    /// Seats whose occupant it hit, and whose occupant it killed, as bits by seat.
    pub crew: u8,
    pub killed: u8,
}

impl Inside {
    pub fn add(&mut self, other: Inside) {
        self.hurt |= other.hurt;
        self.gone |= other.gone;
        self.crew |= other.crew;
        self.killed |= other.killed;
    }
}

/// A round that got into a vehicle: where it went in, which way it goes (both in the hull frame), the
/// penetration it had left, its calibre and kind, what it does to a person it meets (`crew`), and
/// what that is credited as.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub at: [f32; 3],
    pub dir: [f32; 3],
    pub residual: f32,
    pub calibre: f32,
    pub kind: PenKind,
    pub slit: bool,
    pub crew: u8,
    pub weapon: u8,
    pub attacker: Option<u32>,
}

/// Where a ray from `origin` along the unit `dir` first enters `b`, if within `max`: 0 when it starts inside.
fn ray_box(origin: [f32; 3], dir: [f32; 3], b: &Box3, max: f32) -> Option<f32> {
    let (mut near, mut far) = (0.0f32, max);
    for i in 0..3 {
        let (lo, hi) = (b.centre[i] - b.half[i], b.centre[i] + b.half[i]);
        if dir[i].abs() < 1e-6 {
            if origin[i] < lo || origin[i] > hi {
                return None;
            }
            continue;
        }
        let (a, c) = ((lo - origin[i]) / dir[i], (hi - origin[i]) / dir[i]);
        near = near.max(a.min(c));
        far = far.min(a.max(c));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// The modules of vehicle `vid` a ray from `origin` along `dir` (hull frame) passes through within
/// `max`, nearest first, with how far along. One in a turret is met where the turret has turned it,
/// and not at all once the turret is gone.
fn modules_along(world: &World, vid: u32, origin: [f32; 3], dir: [f32; 3], max: f32) -> Vec<(f32, &'static Module)> {
    let Some(vehicle) = world.entities.get::<Vehicle>(vid) else { return Vec::new() };
    let def = vehicle_def(vehicle.kind);
    let turret_on = vehicle.damage[DAMAGE_TURRET] != TURRET_GONE;
    let pivot = def.spec.solids.iter().find(|solid| solid.role == Role::Turret).map_or([0.0; 3], |solid| solid.at);
    let yaw = world.entities.get::<Mounts>(vid).and_then(|mounts| mounts.turret().map(|mount| mount.yaw)).unwrap_or(0.0);
    let from = [origin[0] - pivot[0], origin[1] - pivot[1], origin[2] - pivot[2]];
    let turned = yawed(from, -yaw);
    let (turret_origin, turret_dir) = ([turned[0] + pivot[0], turned[1] + pivot[1], turned[2] + pivot[2]], yawed(dir, -yaw));
    let mut met: Vec<(f32, &'static Module)> = def
        .armor
        .modules
        .iter()
        .filter(|module| !module.part.outside() && (!module.turret || turret_on))
        .filter_map(|module| {
            let (o, d) = if module.turret { (turret_origin, turret_dir) } else { (origin, dir) };
            ray_box(o, d, &module.at, max).map(|t| (t, module))
        })
        .collect();
    met.sort_by(|a, b| a.0.total_cmp(&b.0));
    met
}

/// Whether vehicle `vid`'s `part` has been shot to pieces.
pub fn part_gone(world: &World, vid: u32, part: Part) -> bool {
    let Some(vehicle) = world.entities.get::<Vehicle>(vid) else { return false };
    let armor = &vehicle_def(vehicle.kind).armor;
    part.slot().is_some_and(|slot| armor.module(slot).is_some() && vehicle.parts[slot] == 0)
}

/// Writes vehicle `vid`'s part states, and whether it burns, into its damage as the snapshot carries it.
fn sync_parts(world: &mut World, vid: u32) {
    let burning = world.entities.has::<Burning>(vid);
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return };
    let armor = &vehicle_def(vehicle.kind).armor;
    if armor.modules.is_empty() {
        return;
    }
    let states: [State; PARTS] = std::array::from_fn(|slot| armor.module(slot).map_or(State::Sound, |module| State::of(vehicle.parts[slot], module.hp)));
    let [low, high] = pack(states);
    vehicle.damage[DAMAGE_PARTS] = low;
    vehicle.damage[DAMAGE_PARTS + 1] = high;
    vehicle.damage[DAMAGE_FIRE] = if burning { BURNING } else { 0 };
}

/// Takes `amount` off part `slot` of vehicle `vid`. A part that is finished off does what it does:
/// ammunition cooks off and wrecks the vehicle, throwing its turret; fuel catches fire. A `fragment`
/// can hurt the ammunition but not set it off: only a round itself does that.
fn hit_part(world: &mut World, vid: u32, slot: usize, amount: f32, attacker: Option<u32>, fragment: bool) -> Inside {
    let Some(mut vehicle) = world.entities.get_mut::<Vehicle>(vid) else { return Inside::default() };
    let before = vehicle.parts[slot];
    if before == 0 || amount < 1.0 {
        return Inside::default();
    }
    let least = u8::from(fragment && Part::Ammo.slot() == Some(slot));
    vehicle.parts[slot] = before.saturating_sub(amount.min(255.0) as u8).max(least);
    let gone = vehicle.parts[slot] == 0;
    let bit = 1 << slot;
    if gone {
        if let Some(id) = attacker {
            world.entities.insert(vid, crate::entities::HurtBy(id));
        }
        if Part::Ammo.slot() == Some(slot) {
            damage_turret(world, vid, u8::MAX, attacker);
            super::combat::damage_vehicle(world, vid, u8::MAX, attacker);
        } else if Part::Fuel.slot() == Some(slot) && !world.entities.dead(vid) {
            world.entities.insert(vid, Burning { ticks: 0 });
        }
    }
    sync_parts(world, vid);
    Inside { hurt: bit, gone: if gone { bit } else { 0 }, ..Inside::default() }
}

/// `amount` to whoever sits in seat `seat` of vehicle `vid`.
fn hit_crew(world: &mut World, vid: u32, seat: u8, amount: u8, weapon: u8, attacker: Option<u32>) -> Inside {
    let Some(id) = world.entities.get::<Seats>(vid).and_then(|seats| seats.rider(usize::from(seat))) else { return Inside::default() };
    super::life::damage_entity(world, id, super::life::Harm::new(amount, attacker, weapon));
    let bit = 1 << seat;
    Inside { crew: bit, killed: if world.entities.alive(id) { 0 } else { bit }, ..Inside::default() }
}

/// What meeting `module` does, for `part_damage` to a part or `crew` to a person.
fn hit_module(world: &mut World, vid: u32, module: &Module, part_damage: f32, crew: u8, entry: &Entry, fragment: bool) -> Inside {
    match module.part {
        Part::Crew(seat) => hit_crew(world, vid, seat, crew, entry.weapon, entry.attacker),
        part => part.slot().map_or_else(Inside::default, |slot| hit_part(world, vid, slot, part_damage, entry.attacker, fragment)),
    }
}

/// A unit vector at `angle` off the unit `dir`, turned `around` about it.
fn off_axis(dir: [f32; 3], angle: f32, around: f32) -> [f32; 3] {
    let helper = if dir[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let u = unit(cross(dir, helper));
    let v = cross(dir, u);
    let (sa, ca) = angle.sin_cos();
    let (sr, cr) = around.sin_cos();
    unit(std::array::from_fn(|i| dir[i] * ca + (u[i] * cr + v[i] * sr) * sa))
}

/// A round that got into vehicle `vid` goes on inside: it hits the parts and the people in its way,
/// each for less than the last, until it is spent (a plain bullet stops in the first). A big enough one
/// that went through a plate, not a slit, throws a fan of fragments too, each of which hits the first
/// thing it meets. No dice: the fan is the same every time.
pub fn inside(world: &mut World, vid: u32, entry: Entry) -> Inside {
    let mut done = Inside::default();
    let mut damage = PART_HIT + entry.residual * PART_PER_MM;
    let mut crew = entry.crew;
    for (_, module) in modules_along(world, vid, entry.at, entry.dir, INSIDE_REACH) {
        if damage < PART_SPENT || world.entities.dead(vid) {
            break;
        }
        done.add(hit_module(world, vid, module, damage, crew, &entry, false));
        if entry.kind == PenKind::Ball {
            break;
        }
        damage *= PART_KEEP;
        crew = (f32::from(crew) * PART_KEEP) as u8;
    }
    if entry.slit || entry.calibre < SPALL_CALIBRE {
        return done;
    }
    let fragments = (SPALL_MIN + (entry.residual / 12.0) as usize + (entry.calibre / 30.0) as usize).min(SPALL_MAX);
    let part_damage = SPALL_PART + entry.residual / 5.0;
    for index in 0..fragments {
        if world.entities.dead(vid) {
            break;
        }
        let around = std::f32::consts::TAU * index as f32 / fragments as f32;
        let dir = off_axis(entry.dir, SPALL_CONE, around);
        if let Some(&(_, module)) = modules_along(world, vid, entry.at, dir, SPALL_REACH).first() {
            done.add(hit_module(world, vid, module, part_damage, SPALL_CREW, &entry, true));
        }
    }
    done
}

/// A shell of `calibre` that bursts for `damage` on vehicle `vid` at `at` (the hull frame), through its
/// armour or not: a track it lands on takes the shot itself as well as its burst; one beside it, the burst.
pub fn on_outside(world: &mut World, vid: u32, at: [f32; 3], calibre: f32, damage: f32, attacker: Option<u32>) -> Inside {
    if calibre < SPALL_CALIBRE {
        return Inside::default();
    }
    let Some(kind) = world.entities.get::<Vehicle>(vid).map(|vehicle| vehicle.kind) else { return Inside::default() };
    let mut done = Inside::default();
    for module in vehicle_def(kind).armor.modules.iter().filter(|module| module.part.outside() && module.at.holds(at, 0.15)) {
        if let Some(slot) = module.part.slot() {
            done.add(hit_part(world, vid, slot, TRACK_HIT + calibre * TRACK_PER_CALIBRE, attacker, false));
        }
    }
    done.add(track_splash(world, vid, at, damage, attacker));
    done
}

/// A blast of `damage` at `at` (the hull frame of vehicle `vid`) hurts the tracks within reach.
fn track_splash(world: &mut World, vid: u32, at: [f32; 3], damage: f32, attacker: Option<u32>) -> Inside {
    let Some(kind) = world.entities.get::<Vehicle>(vid).map(|vehicle| vehicle.kind) else { return Inside::default() };
    let mut done = Inside::default();
    for module in vehicle_def(kind).armor.modules.iter().filter(|module| module.part.outside()) {
        let gap = (0..3).map(|i| ((at[i] - module.at.centre[i]).abs() - module.at.half[i]).max(0.0).powi(2)).sum::<f32>().sqrt();
        if gap >= TRACK_SPLASH_REACH {
            continue;
        }
        let amount = damage * TRACK_SPLASH_SHARE * (1.0 - gap / TRACK_SPLASH_REACH);
        if let Some(slot) = module.part.slot() {
            done.add(hit_part(world, vid, slot, amount, attacker, false));
        }
    }
    done
}

/// A blast of `damage` at world point `point` near vehicle `vid`: its tracks take it, if they are close.
pub fn blast_parts(world: &mut World, vid: u32, point: [f32; 3], damage: f32, attacker: Option<u32>) {
    let Some(pose) = world.physics.car_pose(vid) else { return };
    let at = pose.local_dir([point[0] - pose.position[0], point[1] - pose.position[1], point[2] - pose.position[2]]);
    track_splash(world, vid, at, damage, attacker);
}

/// Burning vehicles lose health, a little at a time, until they are wrecks. Called every tick.
pub fn burn(world: &mut World) {
    for vid in world.entities.ids::<crate::entities::With<Burning>>() {
        if world.entities.dead(vid) {
            world.entities.remove::<Burning>(vid);
            sync_parts(world, vid);
            continue;
        }
        let Some(ticks) = world.entities.get_mut::<Burning>(vid).map(|mut burning| {
            burning.ticks += 1;
            burning.ticks
        }) else {
            continue;
        };
        if ticks == 1 {
            sync_parts(world, vid);
        }
        if ticks.is_multiple_of(BURN_EVERY) {
            let attacker = world.entities.get::<crate::entities::HurtBy>(vid).map(|by| by.0);
            super::combat::damage_vehicle(world, vid, 1, attacker);
        }
    }
}

#[cfg(test)]
mod tests;
