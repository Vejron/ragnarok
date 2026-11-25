//! The medics of an ambulance, out of it (EMERGENCY.md). Each is a player with no connection, as a
//! civilian is, with empty hands and a mind that walks to a casualty, kneels by them a moment,
//! carries them back to the ambulance and goes for the next, until there are none left or the
//! ambulance is full. Then they get back in.

use super::{CARRIES, Emergency, EmergencyCall, UNSEEN, distance_2d, door, ticks};
use crate::entities::{Component, With};
use crate::game::{Player, Walk, World};
use crate::mechanics::bots::{bend, follow, fresh_goal, heading, push, steer};
use crate::mechanics::npcs::turn_towards;
use crate::mechanics::traffic::Duty;
use shared::consts::*;
use shared::protocol::InputFrame;
use shared::quant::to_mrad;
use std::f32::consts::PI;

/// How fast a medic turns, radians a second, and how nearly they must face their way to walk on, radians.
const TURN_RATE: f32 = 7.0;
const WALK_WITHIN: f32 = 1.2;
/// A medic is by a casualty within this of them, metres, and by the ambulance's door within this of it.
const BY_CASUALTY: f32 = 1.6;
const BY_DOOR: f32 = 1.8;
/// How long a medic kneels by a casualty before they have them up, seconds.
const LIFT: f32 = 2.0;
/// A medic who has been out this long goes back to the ambulance with what they have, seconds, and
/// one out this much longer is taken to be aboard wherever they have got to.
const OUT_MAX: f32 = 120.0;
const OUT_OVER: f32 = 30.0;
/// A medic whose ambulance is gone is gone too after this long, seconds, or once nobody sees them.
const STRANDED: f32 = 45.0;
/// A dead medic lies this long, seconds.
const LIES: f32 = 10.0;
/// The most grid cells looked at in one tick to find ways, among all the medics.
const SEARCH_PER_TICK: usize = 1_000;

/// What a medic is about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Task {
    /// Looking for the next casualty to go to.
    Seeking,
    /// Walking to casualty `0`.
    Going(u32),
    /// Kneeling by casualty `0` for `1` more ticks.
    Lifting(u32, u32),
    /// Walking back to the ambulance, with a casualty or without.
    Returning { carrying: bool },
}

/// The mind of a medic. Their body is the `Player` with the same id.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Medic {
    /// The unit of the fleet their ambulance is, what it was sent for, and the seat they got out by.
    pub unit: usize,
    pub tag: u32,
    pub seat: usize,
    /// Facing, radians.
    pub yaw: f32,
    pub task: Task,
    pub walk: Walk,
    /// Ticks out of the ambulance, ticks since it was lost, and ticks lain dead.
    pub out: u32,
    pub stranded: u32,
    pub lain: u32,
}

impl Medic {
    pub fn new(unit: usize, tag: u32, seat: usize, yaw: f32) -> Self {
        Medic { unit, tag, seat, yaw, task: Task::Seeking, walk: Walk::new(), out: 0, stranded: 0, lain: 0 }
    }
}

impl Emergency {
    /// One tick of every medic who is out: what they do now, as the input a player would send.
    pub(super) fn tend(&mut self, world: &mut World, dt: f32, viewers: &[[f32; 3]]) {
        let ids = world.entities.ids::<With<Medic>>();
        let crowd: Vec<(u32, [f32; 2])> = ids.iter().filter_map(|&id| world.entities.get::<Player>(id).filter(|_| world.entities.alive(id)).map(|p| (id, [p.character.position[0], p.character.position[2]]))).collect();
        let mut budget = SEARCH_PER_TICK;
        for &id in &ids {
            let Some(mut mind) = world.entities.get_mut::<Medic>(id) else { continue };
            let mut medic = std::mem::replace(&mut *mind, Medic::new(0, 0, 0, 0.0));
            // The casualties the other medics are seeing to.
            let taken: Vec<u32> = ids
                .iter()
                .filter(|&&other| other != id)
                .filter_map(|&other| world.entities.get::<Medic>(other))
                .filter_map(|other| match other.task {
                    Task::Going(casualty) | Task::Lifting(casualty, _) => Some(casualty),
                    _ => None,
                })
                .collect();
            let (input, gone) = self.think(world, id, &mut medic, dt, viewers, &crowd, &taken, &mut budget);
            *world.entities.of_mut::<Medic>(id) = medic;
            if let Some(mut player) = world.entities.get_mut::<Player>(id) {
                player.input = input;
            }
            if gone {
                // At their own door, they get back in by it; anyone else is simply gone.
                let unit = world.entities.get::<Medic>(id).map_or(usize::MAX, |medic| medic.unit);
                match super::boarding(world, id, unit) {
                    Some((vehicle, seat)) => world.emergency_calls.push(EmergencyCall::Aboard { id, vehicle, seat, kind: super::MEDIC_KIND }),
                    None => world.emergency_calls.push(EmergencyCall::Gone(id)),
                }
            }
        }
    }

    /// One tick of medic `id`: their input, and whether they are to go.
    #[allow(clippy::too_many_arguments)]
    fn think(&mut self, world: &mut World, id: u32, medic: &mut Medic, dt: f32, viewers: &[[f32; 3]], crowd: &[(u32, [f32; 2])], taken: &[u32], budget: &mut usize) -> (InputFrame, bool) {
        let idle = InputFrame { yaw: to_mrad(medic.yaw), view: world.tick as u16, ..Default::default() };
        let Some(player) = world.entities.get::<Player>(id) else { return (idle, true) };
        let (at, down) = (player.character.position, player.character.down);
        if !world.entities.alive(id) {
            medic.lain += 1;
            return (idle, medic.lain > ticks(LIES));
        }
        // Their ambulance, if it still stands where it parked for them.
        let unit = world.fleet.units.get(medic.unit).filter(|unit| unit.tag == medic.tag && unit.duty == Duty::OnScene).and_then(|unit| unit.vid);
        // The door of it nearer them: they need not walk round it to the one they got out of.
        let doors = unit.map(|vid| [0, 1].map(|seat| door(world, vid, seat))).into_iter().flatten().flatten();
        let Some((home, _)) = doors.min_by(|a, b| distance_2d(at, a.0).total_cmp(&distance_2d(at, b.0))) else {
            // It is gone without them: they stand there until nobody is looking.
            medic.stranded += 1;
            let unseen = viewers.iter().all(|v| distance_2d(*v, at) > UNSEEN);
            return (idle, unseen || medic.stranded > ticks(STRANDED));
        };
        let home = [home[0], home[1] + sim::STANDING_Y, home[2]];
        medic.out += 1;
        if medic.out > ticks(OUT_MAX + OUT_OVER) {
            return (idle, true);
        }
        // Knocked down, or with shots going off near: they keep still and low.
        if down > 0 {
            return (idle, false);
        }
        if self.danger(at) {
            return (InputFrame { buttons: BUTTON_CROUCH, ..idle }, false);
        }
        let Some(incident) = self.incidents.iter_mut().find(|incident| incident.tag == medic.tag) else {
            medic.task = Task::Returning { carrying: false };
            return self.walk(world, id, medic, at, home, dt, crowd, budget);
        };
        if medic.out > ticks(OUT_MAX) && !matches!(medic.task, Task::Returning { .. }) {
            medic.task = Task::Returning { carrying: false };
        }
        let place = |world: &World, casualty: u32| world.entities.get::<Player>(casualty).filter(|_| world.entities.has::<crate::mechanics::Civilian>(casualty)).map(|player| player.character.position);
        loop {
            match medic.task {
                Task::Seeking => {
                    let room = incident.loaded + taken.len() < CARRIES;
                    let next = incident
                        .casualties
                        .iter()
                        .copied()
                        .filter(|casualty| room && !taken.contains(casualty))
                        .filter_map(|casualty| place(world, casualty).map(|p| (casualty, distance_2d(at, p))))
                        .min_by(|a, b| a.1.total_cmp(&b.1));
                    medic.task = next.map_or(Task::Returning { carrying: false }, |(casualty, _)| Task::Going(casualty));
                    medic.walk.path.clear();
                    fresh_goal(&mut medic.walk);
                    if matches!(medic.task, Task::Returning { .. }) {
                        break;
                    }
                }
                Task::Going(casualty) => {
                    let Some(p) = place(world, casualty) else {
                        medic.task = Task::Seeking;
                        continue;
                    };
                    if distance_2d(at, p) < BY_CASUALTY {
                        medic.task = Task::Lifting(casualty, ticks(LIFT));
                        continue;
                    }
                    return self.walk(world, id, medic, at, p, dt, crowd, budget);
                }
                Task::Lifting(casualty, left) => {
                    if place(world, casualty).is_none() {
                        medic.task = Task::Seeking;
                        continue;
                    }
                    if left > 1 {
                        medic.task = Task::Lifting(casualty, left - 1);
                        return (InputFrame { buttons: BUTTON_CROUCH, ..idle }, false);
                    }
                    // Up they come: the casualty is the medic's to carry from here.
                    world.traffic_calls.push(crate::mechanics::TrafficCall::Gone(casualty));
                    incident.casualties.retain(|&other| other != casualty);
                    incident.loaded += 1;
                    medic.task = Task::Returning { carrying: true };
                    medic.walk.path.clear();
                    fresh_goal(&mut medic.walk);
                    break;
                }
                Task::Returning { .. } => break,
            }
        }
        let Task::Returning { carrying } = medic.task else { return (idle, false) };
        if distance_2d(at, home) < BY_DOOR {
            // Set down inside, and off for the next; or with nobody left to fetch, back in.
            if carrying {
                medic.task = Task::Seeking;
                return (idle, false);
            }
            return (idle, true);
        }
        self.walk(world, id, medic, at, home, dt, crowd, budget)
    }

    /// The input that walks medic `id`, at `at`, on towards `goal`.
    #[allow(clippy::too_many_arguments)]
    fn walk(&mut self, world: &mut World, id: u32, medic: &mut Medic, at: [f32; 3], goal: [f32; 3], dt: f32, crowd: &[(u32, [f32; 2])], budget: &mut usize) -> (InputFrame, bool) {
        let toward = follow(world, &mut medic.walk, at, goal, budget);
        let want = steer(world, at, bend(heading(at, toward), push(id, at, crowd)));
        medic.yaw = turn_towards(medic.yaw, want, TURN_RATE * dt);
        let facing = (turn_towards(medic.yaw, want, PI) - medic.yaw).abs() < WALK_WITHIN;
        (InputFrame { yaw: to_mrad(medic.yaw), view: world.tick as u16, forward: if facing { 127 } else { 0 }, buttons: BUTTON_RUN, ..Default::default() }, false)
    }
}
