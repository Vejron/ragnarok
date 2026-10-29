//! Bricks (`ITEM_BRICKS`): a player on foot puts a cube where their crosshair is, or with aim held digs
//! one out. The cube goes on the lattice of the buildings' cells, on the near side of the face the
//! crosshair ray meets, so it sits against a wall, on a roof, on the ground or on another cube. What
//! the world does with it (colliders, what falls, telling everyone) is `game/construct.rs`.

use super::combat::{MAX_PITCH, look};
use super::life::body;
use crate::entities::{Health, Hurtbox, With};
use crate::game::{HALF_CUBE, Player, World, cube_centre};
use shared::consts::*;
use shared::construct::{Cube, MATERIAL_BRICK, NEIGHBOURS, Pos, cell_at, step};
use shared::quant::from_mrad;

/// What a player's crosshair is on, for bricks: the cube a new one would go in, and the cube the
/// crosshair ray meets (which may be a building's or nothing placed). `None` when it meets nothing in reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aim {
    pub place: Pos,
    pub dig: Pos,
}

/// Where player `id`'s crosshair puts a brick: the ray from the on-foot camera pivot (`ON_FOOT_SHOULDER`
/// right of the capsule, `ON_FOOT_HEIGHT` up) along the look, as the client's camera and ghost cube
/// build it. Only a target whose middle is within `BUILD_REACH` of the eye counts.
pub fn aim(world: &World, id: u32) -> Option<Aim> {
    let player = world.entities.get::<Player>(id)?;
    if player.seat.is_some() {
        return None;
    }
    let input = player.input;
    let c = player.character;
    let yaw = from_mrad(input.yaw);
    let dir = look(yaw, from_mrad(input.pitch()).clamp(-MAX_PITCH, MAX_PITCH));
    let right = [yaw.cos(), 0.0, -yaw.sin()];
    let pivot: [f32; 3] = std::array::from_fn(|i| c.position[i] + right[i] * ON_FOOT_SHOULDER + if i == 1 { ON_FOOT_HEIGHT } else { 0.0 });
    let eye = [c.position[0], c.position[1] + eye_height(input.buttons(), c.prone), c.position[2]];
    let (t, normal) = world.physics.cast_ray_normal(pivot, dir, BUILD_REACH + ON_FOOT_SHOULDER + ON_FOOT_HEIGHT)?;
    let hit: [f32; 3] = std::array::from_fn(|i| pivot[i] + dir[i] * t);
    let place = cell_at(std::array::from_fn(|i| hit[i] + normal[i] * HALF_CUBE));
    let dig = cell_at(std::array::from_fn(|i| hit[i] - normal[i] * HALF_CUBE));
    let middle = cube_centre(place);
    let reach = (0..3).map(|i| (middle[i] - eye[i]).powi(2)).sum::<f32>().sqrt();
    (reach <= BUILD_REACH).then_some(Aim { place, dig })
}

/// Whether a soldier, a creature or anything else with a body stands in cube `pos`.
fn body_in(world: &World, pos: Pos) -> bool {
    let middle = cube_centre(pos);
    world.entities.ids::<(With<Hurtbox>, With<Health>)>().into_iter().filter_map(|id| body(world, id, 0)).any(|b| {
        (b.centre[0] - middle[0]).abs() < HALF_CUBE + b.radius
            && (b.centre[2] - middle[2]).abs() < HALF_CUBE + b.radius
            && (b.centre[1] - middle[1]).abs() < HALF_CUBE + b.half_height + b.radius
    })
}

/// Whether a new cube may go at `pos`: the world is not full, nothing is there (no cube, wall, tree,
/// vehicle or body), it is not buried, and it rests on a cube, a building or the ground.
pub fn may_place(world: &mut World, pos: Pos) -> bool {
    if world.construct.len() >= MAX_BUILT || world.construct.has(pos) {
        return false;
    }
    let middle = cube_centre(pos);
    let top = middle[1] + HALF_CUBE;
    if top <= world.map.ground.height_at(middle[0], middle[2]) {
        return false;
    }
    // A hair smaller than the cube, so the faces it shares with a wall or a cube do not count.
    if world.physics.cube_blocked(middle, HALF_CUBE - 0.02) || body_in(world, pos) {
        return false;
    }
    NEIGHBOURS.iter().any(|&s| world.construct.has(step(pos, s))) || world.held_from_outside(pos)
}

/// Player `id` uses bricks once: digs out the placed cube their crosshair is on when `dig`, else puts
/// a cube of their colour where it points. Returns whether anything changed.
pub fn use_bricks(world: &mut World, id: u32, dig: bool) -> bool {
    let Some(aim) = aim(world, id) else { return false };
    if dig {
        return world.dig_cube(aim.dig);
    }
    if !may_place(world, aim.place) {
        return false;
    }
    let colour = world.entities.get::<Player>(id).map_or(0, |player| player.color);
    world.place_cube(aim.place, Cube { colour, material: MATERIAL_BRICK })
}

#[cfg(test)]
mod tests;
