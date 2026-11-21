//! What the modes that have soldiers carry something about share (the backpack, BACKPACK.md, and a
//! delivery's parcel, DELIVERY.md): where it falls when its carrier dies or leaves, and who walks onto
//! it where it lies.

use super::combat::position_of;
use super::life::open_ground;
use crate::game::{Player, World};

/// The floor under a soldier's body centred at `at`, which they could stand on: none off the world.
pub fn floor_under(world: &World, at: [f32; 3]) -> Option<f32> {
    if !world.map.is_inside((at[0] * 100.0) as i32, (at[2] * 100.0) as i32) {
        return None;
    }
    let ground = world.map.ground.height_at(at[0], at[2]);
    let floor = world.physics.cast_ray(at, [0.0, -1.0, 0.0], (at[1] - ground).max(0.0) + 1.0, 0).map_or(ground, |t| at[1] - t);
    Some(floor.max(ground))
}

/// Where what player `id` carries falls when they die or leave: under their body on foot (on the floor
/// of the room they died in, or the ground they fell to), on open ground beside a vehicle they sit in,
/// and nowhere when there is nowhere to put it (off the world).
pub fn drop_spot(world: &World, id: u32) -> Option<[f32; 3]> {
    let player = world.entities.get::<Player>(id);
    let on_foot = player.filter(|player| player.seat.is_none()).map(|player| player.character.position);
    match (on_foot, position_of(world, id)) {
        (Some(at), _) => floor_under(world, at).map(|floor| [at[0], floor, at[2]]),
        (None, Some(at)) => open_ground(world, at[0], at[2]).map(|floor| [at[0], floor, at[2]]),
        (None, None) => None,
    }
}

/// The living soldiers on foot within `reach` metres of `at` across the ground and `reach_y` up or
/// down, nearest first (by id among the equally near).
pub fn within(world: &World, at: [f32; 3], reach: f32, reach_y: f32) -> Vec<u32> {
    let mut near: Vec<(u32, f32)> = world
        .entities
        .all::<Player>()
        .filter(|&(id, player)| player.seat.is_none() && world.entities.alive(id))
        .map(|(id, player)| (id, player.character.position))
        .filter(|(_, p)| (p[1] - at[1]).abs() <= reach_y)
        .map(|(id, p)| (id, (p[0] - at[0]).hypot(p[2] - at[2])))
        .filter(|&(_, d)| d <= reach)
        .collect();
    near.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    near.into_iter().map(|(id, _)| id).collect()
}
