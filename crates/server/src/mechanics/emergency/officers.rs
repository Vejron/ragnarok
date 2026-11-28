//! The officers of a police car or a police van, out of it (EMERGENCY.md). Each is a player with
//! no connection, armed, with a mind that knows one person: the suspect their car was sent for. A
//! car's two carry a revolver each; a van's four, who come from four stars, a rifle too, and two of
//! them a Carl Gustaf. While the suspect has fewer than `LETHAL_STARS` stars they run them down
//! and arrest them, which takes standing by them a moment while they keep still; from then on they
//! stop where they have a shot and fire. A suspect in a tank is not to be arrested at any number of
//! stars: they keep away from it and fire on it, for all the good a revolver does.
//! When the suspect is wanted no more, or has got too far from the car to follow on foot, they
//! walk back to it and get in.

use super::wanted::{LETHAL_STARS, SIGHT, Wanted, in_sight, stars};
use super::{Emergency, EmergencyCall, UNSEEN, distance_2d, door, ticks};
use crate::entities::{Component, With};
use crate::game::{Player, Walk, World};
use crate::mechanics::bots::{bend, follow, fresh_goal, heading, push, steer};
use crate::mechanics::npcs::turn_towards;
use crate::mechanics::position_of;
use crate::mechanics::traffic::{Duty, crew_size};
use shared::consts::*;
use shared::items::{HOTBAR_SLOTS, ITEM_AK5, ITEM_CARL_GUSTAF, ITEM_NONE, ITEM_REVOLVER};
use shared::protocol::InputFrame;
use shared::quant::to_mrad;
use std::f32::consts::PI;

/// What the officer who gets out by seat `seat` carries: a revolver and nothing else, out of a
/// police car; and `heavy`, out of a police van, a rifle too, and every second one a Carl Gustaf
/// for a suspect in a tank.
pub fn officer_loadout(heavy: bool, seat: usize) -> [u8; HOTBAR_SLOTS] {
    let mut items = [ITEM_NONE; HOTBAR_SLOTS];
    items[0] = ITEM_REVOLVER;
    if heavy {
        items[1] = ITEM_AK5;
        if seat % 2 == ANTI_TANK_SEATS {
            items[2] = ITEM_CARL_GUSTAF;
        }
    }
    items
}
/// The officers of a van who carry a Carl Gustaf get out by the seats on this side: 1 for the
/// right, which is seats 1 and 3.
const ANTI_TANK_SEATS: usize = 1;
/// How fast an officer turns, radians a second, and how nearly they must face their way to walk on, radians.
const TURN_RATE: f32 = 7.0;
const WALK_WITHIN: f32 = 1.2;
/// An officer arrests a suspect within this of them, metres, who goes no faster than `ARREST_SPEED`
/// m/s on foot or `ARREST_DRIVING` in a vehicle, once they have stood by them for `ARREST_FOR` seconds.
const ARREST_WITHIN: f32 = 3.0;
const ARREST_SPEED: f32 = 1.5;
const ARREST_DRIVING: f32 = 2.0;
const ARREST_FOR: f32 = 2.0;
/// The share of their cash an arrested player is fined.
const FINE: f32 = 0.2;
/// An officer goes no further than this from the car, metres, and gives the search up after this
/// long without a sight of the suspect, seconds.
const LEASH: f32 = 120.0;
const SEARCH: f32 = 20.0;
/// An officer this far from where the police last knew their suspect to be, metres, where nobody
/// sees them, is gone at once.
const LEFT_BEHIND: f32 = 150.0;
/// An officer is back at the car within this of its door, metres.
const BY_DOOR: f32 = 1.8;
/// An officer who has been out this long is taken to be aboard wherever they have got to, seconds.
const OUT_MAX: f32 = 240.0;
/// An officer whose car is gone is gone too after this long, seconds, or once nobody sees them.
const STRANDED: f32 = 45.0;
/// A dead officer lies this long, seconds.
const LIES: f32 = 10.0;
/// The most grid cells looked at in one tick to find ways, among all the officers.
const SEARCH_PER_TICK: usize = 2_000;

/// The mind of an officer. Their body is the `Player` with the same id.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Officer {
    /// The unit of the fleet their car is, the player it was sent for, and the seat they got out by.
    pub unit: usize,
    pub suspect: u32,
    pub seat: usize,
    /// Out of a police van, armed for a fight (`officer_loadout`).
    pub heavy: bool,
    /// Facing and looking up or down, radians.
    pub yaw: f32,
    pub pitch: f32,
    pub walk: Walk,
    /// Going back to the car for it to take them nearer: too far from it, or done searching where
    /// the suspect was. Once set it stays. One with nobody to go after walks back without it.
    pub returning: bool,
    /// Out to kill their suspect: the soldier's mind it also has (`Bot`) does the fighting.
    pub armed: bool,
    /// Ticks they have had the suspect in sight, and ticks since they last had.
    pub tracked: u32,
    pub lost: u32,
    /// Ticks they have stood by the suspect to arrest them.
    pub arrest: u32,
    /// Where the suspect was last tick, to tell how fast they go.
    was_at: [f32; 3],
    /// Ticks out of the car, ticks since it was lost, and ticks lain dead.
    pub out: u32,
    pub stranded: u32,
    pub lain: u32,
}

impl Officer {
    pub fn new(unit: usize, suspect: u32, seat: usize, yaw: f32, heavy: bool) -> Self {
        Officer { unit, suspect, seat, heavy, yaw, pitch: 0.0, walk: Walk::new(), returning: false, armed: false, tracked: 0, lost: 0, arrest: 0, was_at: [0.0; 3], out: 0, stranded: 0, lain: 0 }
    }
}

/// Player `id` is arrested: out of whatever they sit in, with what a fresh life carries and less
/// cash, back where a life starts, and wanted no more.
pub fn bust(world: &mut World, id: u32) {
    if world.entities.get::<Player>(id).is_some_and(|player| player.seat.is_some()) {
        crate::mechanics::vehicles::alight(world, id);
    }
    if let Some(mut player) = world.entities.get_mut::<Player>(id) {
        player.interrupt();
        player.hands.hotbar = player.hands.loadout;
        player.score.cash -= (player.score.cash as f32 * FINE) as u32;
    }
    crate::mechanics::spawn::place(world, id);
    super::wanted::clear(world, id);
    world.fresh.told.push((id, "Busted".into()));
    tracing::info!(id, "a player is arrested");
}

impl Emergency {
    /// One tick of every officer who is out: what they do now, as the input a player would send.
    pub(super) fn police(&mut self, world: &mut World, dt: f32, viewers: &[[f32; 3]]) {
        let ids = world.entities.ids::<With<Officer>>();
        let crowd: Vec<(u32, [f32; 2])> = ids.iter().filter_map(|&id| world.entities.get::<Player>(id).filter(|_| world.entities.alive(id)).map(|p| (id, [p.character.position[0], p.character.position[2]]))).collect();
        let mut budget = SEARCH_PER_TICK;
        for &id in &ids {
            let Some(mut mind) = world.entities.get_mut::<Officer>(id) else { continue };
            let mut officer = std::mem::replace(&mut *mind, Officer::new(0, 0, 0, 0.0, false));
            let (input, gone) = think(world, id, &mut officer, dt, viewers, &crowd, &mut budget);
            let armed = officer.armed;
            *world.entities.of_mut::<Officer>(id) = officer;
            // Armed, the soldier's mind in it already wrote what it does this tick.
            if !armed && let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.input = input;
            }
            if gone {
                // At their own door, they get back in by it; anyone else is simply gone.
                let unit = world.entities.get::<Officer>(id).map_or(usize::MAX, |officer| officer.unit);
                match super::boarding(world, id, unit) {
                    Some((vehicle, seat)) => world.emergency_calls.push(EmergencyCall::Aboard { id, vehicle, seat, kind: super::OFFICER_KIND }),
                    None => world.emergency_calls.push(EmergencyCall::Gone(id)),
                }
            }
        }
    }
}

/// One tick of officer `id`: their input, and whether they are to go.
fn think(world: &mut World, id: u32, officer: &mut Officer, dt: f32, viewers: &[[f32; 3]], crowd: &[(u32, [f32; 2])], budget: &mut usize) -> (InputFrame, bool) {
    let idle = InputFrame { yaw: to_mrad(officer.yaw), view: world.tick as u16, ..Default::default() };
    officer.armed = false;
    let Some(player) = world.entities.get::<Player>(id) else { return (idle, true) };
    let (at, eye, down) = (player.character.position, player.eye(), player.character.down);
    if !world.entities.alive(id) {
        officer.lain += 1;
        return (idle, officer.lain > ticks(LIES));
    }
    // Their car, if it still stands where it parked for them.
    let unit = world.fleet.units.get(officer.unit).filter(|unit| unit.tag == officer.suspect && unit.duty == Duty::OnScene).and_then(|unit| unit.vid.map(|vid| (vid, unit.at, unit.kind)));
    let seen: &World = world;
    let doors = unit.into_iter().flat_map(|(vid, _, kind)| (0..crew_size(kind)).filter_map(move |seat| door(seen, vid, seat)));
    let home = doors.min_by(|a, b| distance_2d(at, a.0).total_cmp(&distance_2d(at, b.0))).map(|(feet, _)| [feet[0], feet[1] + sim::STANDING_Y, feet[2]]);
    let (Some(home), Some((_, car, _))) = (home, unit) else {
        // It is gone without them: they stand there until nobody is looking.
        officer.stranded += 1;
        let unseen = viewers.iter().all(|v| distance_2d(*v, at) > UNSEEN);
        return (idle, unseen || officer.stranded > ticks(STRANDED));
    };
    officer.out += 1;
    if officer.out > ticks(OUT_MAX) {
        return (idle, true);
    }
    if down > 0 {
        return (idle, false);
    }
    let star = stars(world, officer.suspect);
    let target = position_of(world, officer.suspect).filter(|_| star > 0 && world.entities.alive(officer.suspect));
    let sees = target.is_some_and(|target| distance_2d(at, target) < SIGHT && in_sight(world, eye, officer.suspect, target));
    (officer.tracked, officer.lost) = if sees { (officer.tracked + 1, 0) } else { (0, officer.lost + 1) };
    // How fast the suspect goes, from where they were a tick ago.
    let speed = target.map_or(0.0, |target| distance_2d(officer.was_at, target) * TICK_RATE as f32);
    officer.was_at = target.unwrap_or(at);
    // Where the police last knew the suspect to be: there, if they are not in sight.
    let known = world.entities.get::<Wanted>(officer.suspect).map(|wanted| wanted.seen_at);
    let goal = if sees { target } else { known };
    // With nobody to go after, or left far behind where the police last knew them to be, where
    // nobody sees: gone.
    if (target.is_none() || known.is_none_or(|known| distance_2d(at, known) > LEFT_BEHIND)) && crate::mechanics::traffic::hidden(world, at) {
        return (idle, true);
    }
    let searched = officer.lost > ticks(SEARCH) && goal.is_none_or(|goal| distance_2d(at, goal) < 6.0);
    if target.is_none() {
        // Nobody to go after: back to the car, but not for good. A suspect who is wanted again
        // before they are in it is theirs to go after again, and not one they have searched for.
        (officer.lost, officer.arrest) = (0, 0);
    } else if distance_2d(at, car) > LEASH || goal.is_none_or(|goal| distance_2d(goal, car) > LEASH) || searched {
        officer.returning = true;
    }
    if target.is_none() || officer.returning {
        if distance_2d(at, home) < BY_DOOR {
            return (idle, true);
        }
        return walk(world, id, officer, at, home, dt, crowd, budget, 0);
    }
    let (Some(target), Some(goal)) = (target, goal) else { return (idle, false) };
    let distance = distance_2d(at, target);
    // One in a tank is not to be arrested, however few their stars: that takes shooting too.
    if star < LETHAL_STARS && !crate::mechanics::armoured(world, officer.suspect) {
        // To arrest them: up to them, and standing by them while they keep still.
        let seated = world.entities.get::<Player>(officer.suspect).is_some_and(|suspect| suspect.seat.is_some());
        let still = world.entities.get::<Player>(officer.suspect).is_some_and(|suspect| suspect.character.down > 0) || speed < if seated { ARREST_DRIVING } else { ARREST_SPEED };
        if sees && distance < ARREST_WITHIN {
            officer.arrest = if still { officer.arrest + 1 } else { 0 };
            if officer.arrest > ticks(ARREST_FOR) {
                bust(world, officer.suspect);
            }
            officer.yaw = turn_towards(officer.yaw, heading(at, target), TURN_RATE * dt);
            return (InputFrame { yaw: to_mrad(officer.yaw), ..idle }, false);
        }
        officer.arrest = 0;
        return walk(world, id, officer, at, goal, dt, crowd, budget, 0);
    }
    // To shoot them: a soldier's mind takes over (`bots`), after this one person only, and this one
    // takes it back when the chase is over.
    officer.armed = true;
    (idle, false)
}

/// The input that runs officer `id`, at `at`, on towards `goal`, with the thing in `slot` in hand.
#[allow(clippy::too_many_arguments)]
fn walk(world: &mut World, id: u32, officer: &mut Officer, at: [f32; 3], goal: [f32; 3], dt: f32, crowd: &[(u32, [f32; 2])], budget: &mut usize, slot: u8) -> (InputFrame, bool) {
    if (officer.walk.path_to[0] - goal[0]).hypot(officer.walk.path_to[1] - goal[2]) > 30.0 {
        officer.walk.path.clear();
        fresh_goal(&mut officer.walk);
    }
    let toward = follow(world, &mut officer.walk, at, goal, budget);
    let want = steer(world, at, bend(heading(at, toward), push(id, at, crowd)));
    officer.yaw = turn_towards(officer.yaw, want, TURN_RATE * dt);
    officer.pitch = 0.0;
    let facing = (turn_towards(officer.yaw, want, PI) - officer.yaw).abs() < WALK_WITHIN;
    (InputFrame { yaw: to_mrad(officer.yaw), view: world.tick as u16, forward: if facing { 127 } else { 0 }, buttons: BUTTON_RUN, slot, ..Default::default() }, false)
}
