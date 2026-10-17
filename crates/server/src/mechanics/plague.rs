//! The Orange Plague: a 40 m tycoon who comes to town to buy it up. He walks from one tall building to
//! the next and stomps each flat ("rezoning"), and fights whoever gets in his way with two attacks: a
//! stomp at his feet and an airstrike far off. His brain (`brain.rs`) picks which, and where.
//!
//! He is not a physics body. Rounds, shells and rockets test his hit capsules (`first_plague_hit`),
//! blasts measure their distance to them (`blast_plague`), and he walks through whatever is in the
//! way, his feet coming down on it every stride.

mod barks;
mod brain;
#[cfg(test)]
mod tests;

use super::Mechanic;
use super::combat::{Blast, blast, damage_object, hit_at};
use crate::game::{Plague, PlagueMode, PlaguePart, PlagueWound, Rocket, World};
use crate::physics::Obj;
use barks::Barker;
use brain::{Brain, Category, Contact, Decision, THINK_EVERY, fall_seconds};
use shared::consts::*;
use shared::plague::{self as body, Capsule};
use shared::protocol::{
    BARK_ADAPT_AIR, BARK_ADAPT_ARMOR, BARK_ADAPT_INFANTRY, BARK_ARRIVE, BARK_BLIND, BARK_DOWN, BARK_EAR, BARK_EYE, BARK_FIRED, BARK_GAGGED, BARK_HURT, BARK_LEAVE, BARK_NEMESIS, BARK_REZONE, BARK_STOMP,
    BARK_STRIKE, BEARING_NONE, FACE_EAR_LEFT, FACE_EAR_RIGHT, FACE_EYE_LEFT, FACE_EYE_RIGHT, FACE_MOUTH, HIT_DEALT, HIT_PLAGUE, KIND_DRONE, KIND_HELI, KIND_PLANE, PLAGUE_RECOVER, PLAGUE_STOMP, PLAGUE_STRIKE,
    PLAGUE_WALK, PROJECTILE_STRIKE, ShotEvent, WEAPON_AIRSTRIKE, WEAPON_STOMP,
};
use shared::quant::{from_cm, to_cm};
use shared::world::ObjId;
use std::f32::consts::{PI, TAU};

/// When he first comes to town, by default: five minutes of someone being online.
pub const FIRST_ARRIVAL_TICKS: u32 = 5 * 60 * TICK_RATE;
/// After he is shot down, or leaves with what he came for, he is back this much later.
const RETURN_TICKS: u32 = 15 * 60 * TICK_RATE;
/// He leaves town once he has bought this many buildings.
const ACQUISITIONS: u32 = 5;
/// With nobody alive to come to, he arrives this far from the centre, as a share of the world's
/// half-extent.
const ARRIVAL_RING: f32 = 0.7;
/// He comes to town this far from a random live player, metres, so someone sees him arrive.
const ARRIVAL_DISTANCE: f32 = 150.0;
/// He starts rezoning a building from this close to its centre, metres.
const REZONE_REACH: f32 = 16.0;
/// It takes this many stomps to flatten a building.
const REZONE_STOMPS: u32 = 3;
/// A building is worth buying for its height, less for every this many metres he has to walk to it.
const AGENDA_DISTANCE: f32 = 400.0;
/// He walks only when he faces within this of where he is going, radians: a giant turns, then steps.
const WALK_ALIGNED: f32 = 0.6;
/// Getting his balance back after an attack, ticks.
const RECOVER_TICKS: u32 = 40;
/// Falling over, ticks, and the blast where he lands, his length behind him.
const TOPPLE_TICKS: u32 = 4 * TICK_RATE;
const TOPPLE_RADIUS: f32 = 18.0;
const TOPPLE_DAMAGE: f32 = 150.0;
const TOPPLE_KNOCK: f32 = 25.0;
/// A headshot counts double.
const HEAD_SCALE: u32 = 2;
/// What each part of his face takes before it is shot out, by `FACE_*` bit number (each eye, each
/// ear, his mouth), and how many times over a hit there counts against his health while it is not.
const FACE_HP: [u32; 5] = [300, 300, 400, 400, 500];
const FACE_SCALE: [u32; 5] = [4, 4, 2, 2, 3];
/// Damage from one player in one think that is worth complaining about, and how often he does.
const HURT_COMPLAINT: u32 = 50;
const HURT_ONE_IN: u64 = 3;
/// His eyes, above his feet, metres: where he sees from.
const EYE_HEIGHT: f32 = 37.0;
/// With an eye gone he cannot see what is more than this far over on its side, metres.
const BLIND_SIDE: f32 = 4.0;
/// He hears where a shot came from to within this many metres for each ear he has lost.
const HEARING_SLOP: f32 = 15.0;
/// An aircraft this far above the ground counts as in the air.
const AIRBORNE: f32 = 4.0;
/// A stickman he walks into is put back outside him and sent off at this speed, m/s.
const SHOVE_SPEED: f32 = 6.0;

/// A point `right` metres to the right of and `ahead` metres in front of feet at `position`, facing `yaw`.
fn local_point(position: [f32; 3], yaw: f32, right: f32, ahead: f32) -> [f32; 3] {
    shared::plague::to_world(position, yaw, [right, 0.0, -ahead])
}

/// His mode as the snapshot carries it (`PLAGUE_*`), and whether he is toppling.
pub fn wire_mode(plague: &Plague) -> (u8, bool) {
    match plague.mode {
        PlagueMode::Walk => (PLAGUE_WALK, false),
        PlagueMode::Toppling => (PLAGUE_WALK, true),
        PlagueMode::Stomp => (PLAGUE_STOMP, false),
        PlagueMode::Strike => (PLAGUE_STRIKE, false),
        PlagueMode::Recover => (PLAGUE_RECOVER, false),
    }
}

/// How far through his mode he is, as the snapshot carries it: the clients pose him from it.
pub fn progress_byte(plague: &Plague) -> u8 {
    let (mode, toppling) = wire_mode(plague);
    let done = |length: u32| 1.0 - plague.mode_left as f32 / length as f32;
    let progress = match plague.mode {
        // His place in the two-stride cycle: the left foot comes down at 0, the right at one half.
        PlagueMode::Walk => (plague.stride / PLAGUE_STRIDE + if plague.right_foot { 1.0 } else { 0.0 }) / 2.0,
        PlagueMode::Stomp => done(PLAGUE_STOMP_WINDUP_TICKS),
        PlagueMode::Strike => done(PLAGUE_STRIKE_WINDUP_TICKS),
        PlagueMode::Recover => done(RECOVER_TICKS),
        PlagueMode::Toppling => done(TOPPLE_TICKS),
    };
    body::progress_byte(mode, toppling, progress)
}

/// His body as everyone sees it this tick: posed from what the snapshot says, and in the world.
pub fn body_of(plague: &Plague) -> Vec<Capsule> {
    let (mode, toppling) = wire_mode(plague);
    let pose = body::pose(mode, body::progress_of(mode, toppling, progress_byte(plague)), toppling);
    body::world_capsules(&pose, plague.position, plague.yaw)
}

/// Whether he is there to be hit: in town and not yet fallen.
fn standing(world: &World) -> Option<&Plague> {
    world.plague.as_ref().filter(|p| p.alive())
}

/// Where the ray first meets him within `max`, and which part, unless he is down or it is his own.
pub fn first_plague_hit(world: &World, origin: [f32; 3], dir: [f32; 3], max: f32, owner: u32) -> Option<(f32, PlaguePart)> {
    let plague = standing(world).filter(|p| p.id != owner)?;
    let mut best: Option<(f32, PlaguePart)> = None;
    for capsule in body_of(plague) {
        let limit = best.map_or(max, |(t, _)| t);
        if let Some(t) = body::ray_capsule(origin, dir, &capsule, limit)
            && best.is_none_or(|(b, _)| t < b)
        {
            best = Some((t, capsule.part));
        }
    }
    best
}

/// What `blast` does to him: its damage, falling off with the distance to his nearest part, which
/// takes it. His own stomps and rockets leave him be.
pub fn blast_plague(world: &mut World, blast: &Blast) {
    if blast.weapon == WEAPON_STOMP || blast.weapon == WEAPON_AIRSTRIKE {
        return;
    }
    let Some(plague) = standing(world) else { return };
    let Some((distance, part)) = body_of(plague).iter().map(|c| (body::distance_to(c, blast.point), c.part)).min_by(|a, b| a.0.total_cmp(&b.0)) else { return };
    if distance >= blast.radius {
        return;
    }
    let amount = (blast.damage * (1.0 - distance / blast.radius)).round() as u32;
    damage_plague(world, amount, blast.attacker, part, blast.weapon, blast.point);
}

/// Takes `amount` health off him, struck on `part` at `at`, and tells `attacker` what they did. A
/// headshot counts double; an eye, an ear or his mouth more, until it is shot out. A hit on his
/// phone hand drops a call for an airstrike, and so does losing his mouth. At zero he falls.
pub fn damage_plague(world: &mut World, amount: u32, attacker: Option<u32>, part: PlaguePart, weapon: u8, at: [f32; 3]) {
    let Some(plague) = world.plague.as_mut().filter(|p| p.alive()) else { return };
    let scaled = match part {
        PlaguePart::Face(bit) if plague.face_lost & bit == 0 => {
            let index = bit.trailing_zeros() as usize;
            plague.face_hp[index] = plague.face_hp[index].saturating_sub(amount);
            if plague.face_hp[index] == 0 {
                plague.face_lost |= bit;
                plague.losses.push((bit, attacker));
                if bit == FACE_MOUTH && plague.mode == PlagueMode::Strike {
                    plague.call_dropped = true;
                }
                tracing::info!(part = bit, by = ?attacker, "the Orange Plague lost part of his face");
            }
            amount * FACE_SCALE[index]
        }
        PlaguePart::Head | PlaguePart::Face(_) => amount * HEAD_SCALE,
        _ => amount,
    };
    if scaled == 0 {
        return;
    }
    plague.hp = plague.hp.saturating_sub(scaled);
    plague.wounds.push(PlagueWound { attacker, amount: scaled });
    if part == PlaguePart::Hand && plague.mode == PlagueMode::Strike {
        plague.call_dropped = true;
    }
    if plague.hp == 0 {
        plague.mode = PlagueMode::Toppling;
        plague.mode_left = TOPPLE_TICKS;
        tracing::info!(killer = ?attacker, "the Orange Plague is down");
    }
    let id = plague.id;
    if let Some(attacker) = attacker {
        world.hits.push((attacker, hit_at(HIT_DEALT, HIT_PLAGUE, scaled.min(255) as u8, weapon, BEARING_NONE, at, id)));
    }
}

/// What `id` is fighting from now, for the damage mix: on foot, or in a vehicle, on the ground or in the air.
fn category_of(world: &World, id: u32) -> Option<Category> {
    let player = world.players.get(&id)?;
    let Some(seat) = player.seat else { return Some(Category::Infantry) };
    Some(vehicle_category(world, seat.vehicle))
}

fn vehicle_category(world: &World, vid: u32) -> Category {
    let flies = world.vehicles.get(&vid).is_some_and(|v| matches!(v.kind, KIND_HELI | KIND_PLANE | KIND_DRONE));
    let airborne = world.physics.car_pose(vid).is_some_and(|pose| {
        let [x, y, z] = pose.position;
        y - world.map.ground.height_at(x, z) > AIRBORNE
    });
    if flies && airborne { Category::Air } else { Category::Armor }
}

/// Everything out there worth an attack, and whether he can see it from where he stands: every
/// living stickman in the world, and every vehicle with someone in it. An eye shot out blinds him
/// on its side; with both gone he sees nothing.
fn contacts(world: &World, plague: &Plague) -> Vec<Contact> {
    let eye = [plague.position[0], plague.position[1] + EYE_HEIGHT, plague.position[2]];
    let right = local_point([0.0; 3], plague.yaw, 1.0, 0.0);
    let lost = plague.face_lost;
    let sees = |target: [f32; 3], ignore: u32| {
        let side = (target[0] - eye[0]) * right[0] + (target[2] - eye[2]) * right[2];
        if (lost & FACE_EYE_LEFT != 0 && side < -BLIND_SIDE) || (lost & FACE_EYE_RIGHT != 0 && side > BLIND_SIDE) || lost & (FACE_EYE_LEFT | FACE_EYE_RIGHT) == FACE_EYE_LEFT | FACE_EYE_RIGHT {
            return false;
        }
        let to: [f32; 3] = std::array::from_fn(|i| target[i] - eye[i]);
        let length = to.iter().map(|v| v * v).sum::<f32>().sqrt();
        length < 1e-3 || world.physics.cast_ray(eye, to.map(|v| v / length), length, ignore).is_none()
    };
    let mut out = Vec::new();
    for (&id, player) in world.players.iter().filter(|(_, p)| p.alive() && p.embodied()) {
        let at = player.character.position;
        out.push(Contact { id, position: at, category: Category::Infantry, crew: vec![id], visible: sees([at[0], at[1] + 1.0, at[2]], 0) });
    }
    for (&vid, vehicle) in &world.vehicles {
        let crew: Vec<u32> = vehicle.seats.iter().flatten().copied().collect();
        if crew.is_empty() || vehicle.hp == 0 {
            continue;
        }
        let Some(pose) = world.physics.car_pose(vid) else { continue };
        out.push(Contact { id: vid, position: pose.position, category: vehicle_category(world, vid), crew, visible: sees(pose.position, vid) });
    }
    out
}

/// The building he goes for next: the standing one with the best height for the walk, if any is left.
fn next_property(world: &World, at: [f32; 3]) -> Option<(ObjId, [f32; 2])> {
    let standing = |id: ObjId| !world.destroyed.iter().any(|&(done, _)| done == Obj::Building(id));
    world
        .map
        .building_ids
        .iter()
        .zip(&world.map.buildings)
        .filter(|&(&id, _)| standing(id))
        .map(|(&id, building)| {
            let [x, z] = building.centre().map(from_cm);
            let distance = (x - at[0]).hypot(z - at[2]);
            (id, [x, z], from_cm(building.height) / (1.0 + distance / AGENDA_DISTANCE))
        })
        .max_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(id, at, _)| (id, at))
}

/// Turns `yaw` towards `want` by at most `step`, the short way round.
fn turn_towards(yaw: f32, want: f32, step: f32) -> f32 {
    let delta = (want - yaw + PI).rem_euclid(TAU) - PI;
    yaw + delta.clamp(-step, step)
}

/// The yaw that faces from `from` to `to`.
fn facing(from: [f32; 3], to: [f32; 3]) -> f32 {
    (-(to[0] - from[0])).atan2(-(to[2] - from[2]))
}

/// The ground under `[x, z]`.
fn ground(world: &World, x: f32, z: f32) -> [f32; 3] {
    [x, world.map.ground.height_at(x, z), z]
}

/// The Orange Plague's mechanic: his arrival and departure, his agenda, and his brain.
pub struct OrangePlague {
    /// Ticks (with someone online) until he comes to town; counts only while he is away.
    arrive_in: u32,
    brain: Brain,
    /// The building he is buying, and how many stomps he has given it.
    property: Option<(ObjId, [f32; 2])>,
    rezone_stomps: u32,
    acquired: u32,
    /// Airstrike rockets still to drop: the tick each is due, and where it lands.
    barrage: Vec<(u32, [f32; 3])>,
    barker: Barker,
    /// The Enemy of the Week and the kind of attacker he last said was hurting him, so he says so
    /// when either changes.
    nemesis: Option<u32>,
    worst: Option<Category>,
    /// Whether he has said he is going down.
    fell: bool,
}

impl OrangePlague {
    /// He first comes to town after `arrive_in` ticks with someone online.
    pub fn new(arrive_in: u32) -> Self {
        Self { arrive_in, brain: Brain::default(), property: None, rezone_stomps: 0, acquired: 0, barrage: Vec::new(), barker: Barker::default(), nemesis: None, worst: None, fell: false }
    }

    /// He comes to town: on a ring around the middle, facing the first building he wants.
    fn arrive(&mut self, world: &mut World) {
        let half = from_cm(world.map.half_extent);
        let angle = world.rng.next_f64() as f32 * TAU;
        let alive: Vec<[f32; 3]> = world.players.values().filter(|p| p.alive()).map(|p| p.character.position).collect();
        let [x, z] = if alive.is_empty() {
            [angle.cos() * half * ARRIVAL_RING, angle.sin() * half * ARRIVAL_RING]
        } else {
            let [px, _, pz] = alive[(world.rng.next_u64() % alive.len() as u64) as usize];
            let inside = half - PLAGUE_HEIGHT;
            [(px + angle.cos() * ARRIVAL_DISTANCE).clamp(-inside, inside), (pz + angle.sin() * ARRIVAL_DISTANCE).clamp(-inside, inside)]
        };
        let position = ground(world, x, z);
        self.brain = Brain::default();
        self.property = next_property(world, position);
        self.rezone_stomps = 0;
        self.acquired = 0;
        self.barker = Barker::default();
        self.nemesis = None;
        self.worst = None;
        self.fell = false;
        let yaw = self.property.map_or(0.0, |(_, [x, z])| facing(position, [x, 0.0, z]));
        let id = world.alloc_id();
        world.plague = Some(Plague {
            id,
            position,
            yaw,
            hp: PLAGUE_HP,
            mode: PlagueMode::Walk,
            mode_left: 0,
            target: position,
            stride: 0.0,
            right_foot: false,
            wounds: Vec::new(),
            call_dropped: false,
            face_hp: FACE_HP,
            face_lost: 0,
            losses: Vec::new(),
        });
        tracing::info!(id, x = position[0], z = position[2], "the Orange Plague is in town");
        self.barker.announce(world, BARK_ARRIVE, 0);
    }

    /// He is gone, shot down or done: back in `RETURN_TICKS`.
    fn leave(&mut self, world: &mut World) {
        world.plague = None;
        world.physics.clear_giant();
        self.arrive_in = RETURN_TICKS;
        self.property = None;
        self.barrage.clear();
    }

    /// Thinks: takes in what hurt him, what he can see and what he hears, and maybe starts an attack.
    fn think(&mut self, world: &mut World, plague: &mut Plague) {
        let wounds = std::mem::take(&mut plague.wounds);
        self.brain.feel(&wounds, |id| category_of(world, id));
        self.brain.forget(THINK_EVERY);
        if !self.mourn(world, plague) {
            self.react(world, &wounds);
        }
        let seen = contacts(world, plague);
        self.brain.perceive(world.tick, &seen);
        self.hear(world, plague, &wounds, &seen);
        if plague.mode != PlagueMode::Walk {
            return;
        }
        let can_call = plague.face_lost & FACE_MOUTH == 0;
        let rng = &mut world.rng;
        match self.brain.decide(world.tick, plague.position, &seen, can_call, || rng.next_f64() as f32) {
            Decision::Stomp(point) => {
                self.wind_up(world, plague, PlagueMode::Stomp, point);
                self.barker.say(world, BARK_STOMP, 0);
            }
            Decision::Strike(point) => {
                self.wind_up(world, plague, PlagueMode::Strike, point);
                // Named, if the one he hates most is in the circle.
                let meant = self.nemesis.filter(|id| seen.iter().any(|c| c.crew.contains(id) && brain::flat_distance(c.position, point) < PLAGUE_STRIKE_RADIUS * 1.5));
                self.barker.say(world, BARK_STRIKE, meant.unwrap_or(0));
            }
            Decision::Walk => self.barker.idle(world),
        }
    }

    /// Says what he makes of losing part of his face, if he has since he last thought. Returns
    /// whether he said anything.
    fn mourn(&mut self, world: &mut World, plague: &mut Plague) -> bool {
        let losses = std::mem::take(&mut plague.losses);
        let Some(&(bit, by)) = losses.last() else { return false };
        let eyes = FACE_EYE_LEFT | FACE_EYE_RIGHT;
        let trigger = match bit {
            FACE_MOUTH => BARK_GAGGED,
            _ if bit & eyes != 0 && plague.face_lost & eyes == eyes => BARK_BLIND,
            _ if bit & eyes != 0 => BARK_EYE,
            _ => BARK_EAR,
        };
        self.barker.cut_in(world, trigger, by.unwrap_or(0))
    }

    /// Hears where whoever hurt him from out of sight is: as well as he can with the ears he has
    /// left, and not at all with neither.
    fn hear(&mut self, world: &mut World, plague: &Plague, wounds: &[PlagueWound], seen: &[Contact]) {
        let ears = [FACE_EAR_LEFT, FACE_EAR_RIGHT].iter().filter(|&&bit| plague.face_lost & bit == 0).count();
        if ears == 0 {
            return;
        }
        let slop = HEARING_SLOP * (2 - ears) as f32;
        let mut heard: Vec<u32> = wounds.iter().filter_map(|w| w.attacker).collect();
        heard.sort_unstable();
        heard.dedup();
        for attacker in heard {
            let Some(contact) = seen.iter().find(|c| !c.visible && c.crew.contains(&attacker)) else { continue };
            let angle = world.rng.next_f64() as f32 * TAU;
            let off = world.rng.next_f64() as f32 * slop;
            let [x, y, z] = contact.position;
            self.brain.hear(world.tick, contact, [x + angle.cos() * off, y, z + angle.sin() * off]);
        }
    }

    /// Says what he makes of the latest wounds: a new Enemy of the Week, a new kind of attacker to
    /// aim at, or just that it stung.
    fn react(&mut self, world: &mut World, wounds: &[PlagueWound]) {
        let nemesis = self.brain.nemesis();
        if nemesis.is_some() && nemesis != self.nemesis {
            self.nemesis = nemesis;
            self.barker.cut_in(world, BARK_NEMESIS, nemesis.unwrap_or(0));
            return;
        }
        let worst = self.brain.worst();
        if worst.is_some() && worst != self.worst {
            self.worst = worst;
            let trigger = match worst {
                Some(Category::Infantry) => BARK_ADAPT_INFANTRY,
                Some(Category::Armor) => BARK_ADAPT_ARMOR,
                _ => BARK_ADAPT_AIR,
            };
            self.barker.say(world, trigger, 0);
            return;
        }
        let mut by: std::collections::BTreeMap<u32, u32> = Default::default();
        for wound in wounds {
            if let Some(attacker) = wound.attacker {
                *by.entry(attacker).or_default() += wound.amount;
            }
        }
        let stung = by.into_iter().max_by_key(|&(_, amount)| amount).filter(|&(_, amount)| amount >= HURT_COMPLAINT);
        if let Some((attacker, _)) = stung
            && world.rng.next_u64().is_multiple_of(HURT_ONE_IN)
        {
            self.barker.say(world, BARK_HURT, attacker);
        }
    }

    fn wind_up(&mut self, world: &World, plague: &mut Plague, mode: PlagueMode, point: [f32; 3]) {
        plague.mode = mode;
        plague.mode_left = if mode == PlagueMode::Stomp { PLAGUE_STOMP_WINDUP_TICKS } else { PLAGUE_STRIKE_WINDUP_TICKS };
        plague.target = ground(world, point[0], point[2]);
        plague.call_dropped = false;
    }

    /// Walks towards the building he is buying, a foot coming down every stride, and starts
    /// rezoning it once he is there.
    fn walk(&mut self, world: &mut World, plague: &mut Plague, dt: f32) {
        if self.property.is_none_or(|(id, _)| world.destroyed.iter().any(|&(done, _)| done == Obj::Building(id))) {
            self.property = next_property(world, plague.position);
            self.rezone_stomps = 0;
        }
        let Some((_, [x, z])) = self.property else { return };
        let goal = ground(world, x, z);
        let [px, _, pz] = plague.position;
        if (x - px).hypot(z - pz) < REZONE_REACH {
            self.wind_up(world, plague, PlagueMode::Stomp, goal);
            return;
        }
        let want = facing(plague.position, goal);
        plague.yaw = turn_towards(plague.yaw, want, PLAGUE_TURN_RATE * dt);
        if (turn_towards(plague.yaw, want, PI) - plague.yaw).abs() > WALK_ALIGNED {
            return;
        }
        let step = PLAGUE_SPEED * dt;
        let ahead = local_point(plague.position, plague.yaw, 0.0, step);
        plague.position = ground(world, ahead[0], ahead[2]);
        plague.stride += step;
        if plague.stride >= PLAGUE_STRIDE {
            plague.stride -= PLAGUE_STRIDE;
            plague.right_foot = !plague.right_foot;
            let side = if plague.right_foot { PLAGUE_FOOT_SPREAD } else { -PLAGUE_FOOT_SPREAD };
            let foot = local_point(plague.position, plague.yaw, side, 1.5);
            let foot = ground(world, foot[0], foot[2]);
            let forward = local_point([0.0; 3], plague.yaw, 0.0, 1.0);
            // Quiet on the wire: clients bring the foot down themselves, from how far he has walked.
            blast(world, Blast { point: foot, radius: PLAGUE_FOOTFALL_RADIUS, damage: PLAGUE_FOOTFALL_DAMAGE, knock: PLAGUE_FOOTFALL_KNOCK, dir: forward, attacker: None, weapon: WEAPON_STOMP });
        }
    }

    /// A wind-up runs out: the foot comes down, or the airstrike is on its way.
    fn strike_home(&mut self, world: &mut World, plague: &mut Plague) {
        let point = plague.target;
        match plague.mode {
            PlagueMode::Stomp => {
                let forward = local_point([0.0; 3], plague.yaw, 0.0, 1.0);
                let [x, y, z] = point.map(to_cm);
                world.shots.push(ShotEvent { shooter: plague.id, weapon: WEAPON_STOMP, x, y, z });
                blast(world, Blast { point: [point[0], point[1] + 0.5, point[2]], radius: PLAGUE_STOMP_RADIUS, damage: PLAGUE_STOMP_DAMAGE, knock: PLAGUE_STOMP_KNOCK, dir: forward, attacker: None, weapon: WEAPON_STOMP });
                self.rezone(world, point);
            }
            PlagueMode::Strike if plague.call_dropped => {
                self.brain.delay_strike(world.tick);
            }
            PlagueMode::Strike => {
                for i in 0..PLAGUE_STRIKE_ROCKETS {
                    // The first dead on the mark, the rest spread evenly over the circle.
                    let (angle, reach) = if i == 0 { (0.0, 0.0) } else { (world.rng.next_f64() as f32 * TAU, (world.rng.next_f64() as f32).sqrt() * PLAGUE_STRIKE_RADIUS) };
                    let landing = ground(world, point[0] + angle.cos() * reach, point[2] + angle.sin() * reach);
                    self.barrage.push((world.tick + i * PLAGUE_STRIKE_INTERVAL_TICKS, landing));
                }
            }
            _ => {}
        }
        plague.mode = PlagueMode::Recover;
        plague.mode_left = RECOVER_TICKS;
    }

    /// A stomp on the building he is buying: the last of `REZONE_STOMPS` brings it down, and it is his.
    fn rezone(&mut self, world: &mut World, point: [f32; 3]) {
        let Some((id, [x, z])) = self.property else { return };
        if (x - point[0]).hypot(z - point[2]) > 1.0 {
            return;
        }
        self.rezone_stomps += 1;
        if self.rezone_stomps < REZONE_STOMPS {
            return;
        }
        damage_object(world, Obj::Building(id), u16::MAX, point);
        self.acquired += 1;
        self.property = None;
        self.rezone_stomps = 0;
        tracing::info!(acquired = self.acquired, "the Orange Plague bought a building");
        self.barker.announce(world, BARK_REZONE, 0);
    }

    /// Drops the airstrike rockets that are due: each starts high above and a little to the side of
    /// where it lands, and falls there.
    fn drop_rockets(&mut self, world: &mut World, owner: u32) {
        let now = world.tick;
        let due: Vec<[f32; 3]> = self.barrage.iter().filter(|(tick, _)| *tick <= now).map(|&(_, at)| at).collect();
        self.barrage.retain(|(tick, _)| *tick > now);
        let fall = fall_seconds();
        for landing in due {
            // A slight slant, from a random side, so the rockets are seen coming in.
            let angle = world.rng.next_f64() as f32 * TAU;
            let drift = [angle.cos() * 12.0, angle.sin() * 12.0];
            let id = world.alloc_id();
            world.rockets.insert(
                id,
                Rocket {
                    kind: PROJECTILE_STRIKE,
                    owner,
                    source: 0,
                    position: [landing[0] - drift[0], landing[1] + PLAGUE_STRIKE_DROP_HEIGHT, landing[2] - drift[1]],
                    velocity: [drift[0] / fall, -PLAGUE_STRIKE_DROP_SPEED, drift[1] / fall],
                    ttl: 10 * TICK_RATE,
                    item: 0,
                    back: 0,
                    flown: 0.0,
                },
            );
        }
    }
}

impl Mechanic for OrangePlague {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let Some(mut plague) = world.plague.take() else {
            if !world.players.is_empty() {
                self.arrive_in = self.arrive_in.saturating_sub(1);
                if self.arrive_in == 0 {
                    self.arrive(world);
                }
            }
            return;
        };
        self.drop_rockets(world, plague.id);
        if plague.mode == PlagueMode::Toppling {
            if !self.fell {
                self.fell = true;
                // Falling, he is no longer in anyone's way.
                world.physics.clear_giant();
                let blamed = self.brain.nemesis().unwrap_or(0);
                self.barker.announce(world, BARK_DOWN, blamed);
            }
            plague.mode_left = plague.mode_left.saturating_sub(1);
            if plague.mode_left == 0 {
                // He lands flat on his back.
                let back = local_point(plague.position, plague.yaw, 0.0, -PLAGUE_HEIGHT * 0.6);
                let landing = ground(world, back[0], back[2]);
                let [x, y, z] = landing.map(to_cm);
                world.shots.push(ShotEvent { shooter: plague.id, weapon: WEAPON_STOMP, x, y, z });
                blast(world, Blast { point: landing, radius: TOPPLE_RADIUS, damage: TOPPLE_DAMAGE, knock: TOPPLE_KNOCK, dir: [0.0, 1.0, 0.0], attacker: None, weapon: WEAPON_STOMP });
                self.leave(world);
                return;
            }
            world.plague = Some(plague);
            return;
        }
        match plague.mode {
            PlagueMode::Walk => self.walk(world, &mut plague, dt),
            PlagueMode::Stomp | PlagueMode::Strike => {
                let want = facing(plague.position, plague.target);
                plague.yaw = turn_towards(plague.yaw, want, PLAGUE_TURN_RATE * dt);
                plague.mode_left = plague.mode_left.saturating_sub(1);
                if plague.mode == PlagueMode::Strike && plague.call_dropped {
                    plague.mode_left = 0;
                }
                if plague.mode_left == 0 {
                    self.strike_home(world, &mut plague);
                }
            }
            PlagueMode::Recover => {
                plague.mode_left = plague.mode_left.saturating_sub(1);
                if plague.mode_left == 0 {
                    plague.mode = PlagueMode::Walk;
                }
            }
            PlagueMode::Toppling => {}
        }
        // Whoever his feet and rockets finished this tick (rockets burst in `Gunnery`, earlier on): the
        // first is fired.
        let fired = world.kills.iter().find(|k| k.killer == 0 && (k.weapon == WEAPON_STOMP || k.weapon == WEAPON_AIRSTRIKE)).map(|k| k.victim);
        if let Some(victim) = fired {
            self.barker.cut_in(world, BARK_FIRED, victim);
        }
        // Last, so an attack he decides on now is wound up for its full length from the next tick.
        if world.tick.is_multiple_of(THINK_EVERY) {
            self.think(world, &mut plague);
        }
        if self.acquired >= ACQUISITIONS && plague.mode == PlagueMode::Walk {
            tracing::info!("the Orange Plague has what he came for, and leaves");
            self.barker.announce(world, BARK_LEAVE, 0);
            self.leave(world);
            return;
        }
        embody(world, &plague);
        world.plague = Some(plague);
    }
}

/// Makes him solid where he stands this tick, and puts back outside him any stickman he walked into.
fn embody(world: &mut World, plague: &Plague) {
    let solid: Vec<Capsule> = body_of(plague).into_iter().filter(|c| c.solid).collect();
    world.physics.set_giant(&solid.iter().map(|c| (c.a, c.b, c.radius)).collect::<Vec<_>>());
    let centre = plague.position;
    for player in world.players.values_mut().filter(|p| p.alive() && p.embodied() && p.seat.is_none()) {
        let at = player.character.position;
        for capsule in &solid {
            let near = body::nearest_on_segment(at, capsule.a, capsule.b);
            let need = capsule.radius + CAPSULE_RADIUS;
            let [dx, dz] = [at[0] - near[0], at[2] - near[2]];
            let reach = (dx * dx + (at[1] - near[1]).powi(2) + dz * dz).sqrt();
            if reach >= need {
                continue;
            }
            let flat = dx.hypot(dz);
            let [ox, oz] = if flat > 1e-3 { [dx / flat, dz / flat] } else {
                let (x, z) = (at[0] - centre[0], at[2] - centre[2]);
                let length = x.hypot(z).max(1e-3);
                [x / length, z / length]
            };
            let out = need - reach;
            player.character.position[0] += ox * out;
            player.character.position[2] += oz * out;
            player.character.knock = [ox * SHOVE_SPEED, oz * SHOVE_SPEED];
            break;
        }
    }
}
