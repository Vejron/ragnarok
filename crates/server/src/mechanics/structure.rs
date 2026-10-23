//! Damaged buildings that cannot hold themselves up come down: the carved ones are checked now and
//! then, a couple a tick (`World::settle`), after the blast that hurt them has had a moment. What
//! comes down lands on whoever is under it (`land`).

use super::Mechanic;
use super::combat::damage_vehicle;
use super::life::{Harm, body, damage_entity};
use crate::entities::{Health, Hurtbox, With};
use crate::game::{Rubble, Vehicle, World};
use shared::protocol::WEAPON_RUBBLE;

/// Damage a cell does for each metre it falls (a roof cell dropping 6 m: 24).
const DAMAGE_PER_CELL_METRE: f32 = 4.0;
/// Most metres of fall that count: past that it is as bad as it gets.
const FALL_CAP: f32 = 8.0;
/// Half the width of a cell, metres, which a body's radius is added to.
const HALF_CELL: f32 = 0.36;
/// How far round a vehicle's middle rubble reaches it, metres, and the share of the damage it takes.
const VEHICLE_REACH: f32 = 2.2;
const VEHICLE_SHARE: f32 = 0.4;

pub struct Structure;

impl Mechanic for Structure {
    fn on_tick(&mut self, world: &mut World, _dt: f32) {
        world.settle();
        land(world);
    }
}

/// What a column of rubble does to something `reach` metres across at `at`, `y` high (its middle): the
/// cells' count times the fall, if it is under the column.
fn crush(column: &Rubble, at: [f32; 3], reach: f32) -> f32 {
    let near = (at[0] - column.at[0]).abs() <= HALF_CELL + reach && (at[2] - column.at[1]).abs() <= HALF_CELL + reach;
    if !near || at[1] > column.top || at[1] < column.ground - 1.0 {
        return 0.0;
    }
    column.cells as f32 * (column.top - column.ground).clamp(0.5, FALL_CAP) * DAMAGE_PER_CELL_METRE
}

/// The rubble that lands this tick hurts the bodies and vehicles under it.
fn land(world: &mut World) {
    if world.rubble.is_empty() {
        return;
    }
    let tick = world.tick;
    let (landing, falling): (Vec<Rubble>, Vec<Rubble>) = std::mem::take(&mut world.rubble).into_iter().partition(|column| tick.wrapping_sub(column.due) < u32::MAX / 2);
    world.rubble = falling;
    if landing.is_empty() {
        return;
    }
    for id in world.entities.ids::<(With<Hurtbox>, With<Health>)>() {
        let Some(body) = body(world, id, 0).filter(|body| body.vehicle.is_none()) else { continue };
        let dealt: f32 = landing.iter().map(|column| crush(column, [body.centre[0], body.centre[1], body.centre[2]], body.radius)).sum();
        if dealt >= 1.0 {
            damage_entity(world, id, Harm::new(dealt.min(255.0) as u8, None, WEAPON_RUBBLE));
        }
    }
    for vid in world.entities.ids::<With<Vehicle>>() {
        let Some(at) = world.physics.car_pose(vid).map(|pose| pose.position) else { continue };
        let dealt: f32 = landing.iter().map(|column| crush(column, at, VEHICLE_REACH)).sum::<f32>() * VEHICLE_SHARE;
        if dealt >= 1.0 {
            damage_vehicle(world, vid, dealt.min(255.0) as u8, None);
        }
    }
}
