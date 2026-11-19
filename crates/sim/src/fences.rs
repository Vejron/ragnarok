//! Fences, walls and power poles (`Tile::lines`) as something to run into.
//!
//! A pole is a collider like a rock's: it stops soldiers, vehicles and bullets alike, and nothing
//! fells it. A fence or a wall is a row of pieces, one between each two points of its line
//! (`shared::world::line_pieces`), each a thin box that can be broken on its own ([`Obj::Line`]).
//!
//! A wall is solid to everything, in the trees' group: a light vehicle stops on it, a heavy one goes
//! through and the server breaks what it ploughs. A fence is very weak: it is in a group no body
//! touches (`fence_groups`), so only what asks without naming groups meets it. That is a soldier's
//! step and the bots' grid. Vehicles, rounds and sight go through, and the server breaks the pieces a
//! vehicle drives over. Server and client build the same pieces under the same ids, so a soldier's
//! step against one is predicted.

use crate::{Obj, TREE_GROUP, fence_groups};
use rapier3d::prelude::*;
use shared::quant::from_cm;
use shared::world::{LINE_GUARDRAIL, LINE_POWER, LINE_POWER_MAJOR, Line, LinePiece, line_pieces};

/// How far a wall's piece reaches past each of its ends, metres, so that a corner is closed.
const WALL_LAP: f32 = 0.1;
/// The friction of a guard rail's smooth steel, so a car scraping along it slides on.
const GUARDRAIL_FRICTION: f32 = 0.05;

/// The collider of one piece of a fence or wall: a box from under the ground (`SUNK`) to its top,
/// turned to the piece's line and tilted to the slope it stands on.
fn piece_collider(piece: &LinePiece) -> Collider {
    let def = piece.def;
    let across = [piece.b[0] - piece.a[0], piece.base[1] - piece.base[0], piece.b[1] - piece.a[1]];
    let flat = across[0].hypot(across[2]);
    let length = flat.hypot(across[1]);
    // The box's X runs along the piece: turned about Y to its heading, then about its own Z to the slope.
    let turn = Rotation::from_rotation_y((-across[2]).atan2(across[0])) * Rotation::from_rotation_z(across[1].atan2(flat));
    let half_height = (def.height + SUNK) / 2.0;
    let centre = Vector::new((piece.a[0] + piece.b[0]) / 2.0, (piece.base[0] + piece.base[1]) / 2.0 - SUNK + half_height, (piece.a[1] + piece.b[1]) / 2.0);
    let lap = if def.wall { WALL_LAP } else { 0.0 };
    let groups = if def.wall { InteractionGroups::new(TREE_GROUP, Group::ALL, InteractionTestMode::And) } else { fence_groups() };
    let builder = ColliderBuilder::cuboid(length / 2.0 + lap, half_height, def.half);
    // A guard rail is slippery steel, and pushes only across its line (`RailHooks`).
    let builder = if def.kind == LINE_GUARDRAIL { builder.friction(GUARDRAIL_FRICTION).active_hooks(ActiveHooks::MODIFY_SOLVER_CONTACTS) } else { builder };
    builder.position(Pose::from_parts(centre, turn)).collision_groups(groups).build()
}

/// What makes a guard rail forgiving: each of its pieces pushes only straight across its own line,
/// level, towards the side the other body's middle is on.
/// - **Never along it.** The pieces are separate boxes with square ends. A car scraping along the
///   rail would catch its corner on the end of the next piece where two meet and stop dead at speed,
///   where a real rail is one long beam that turns the car aside. A car meeting the bare end of a
///   rail is turned aside the same way.
/// - **Never up.** The rail is thinner than a fast car moves in a tick, so a car hitting it square
///   would be pushed out over its top and thrown into the air.
pub struct RailHooks;

impl PhysicsHooks for RailHooks {
    fn modify_solver_contacts(&self, context: &mut ContactModificationContext) {
        let (Some(one), Some(two)) = (context.colliders.get(context.collider1), context.colliders.get(context.collider2)) else { return };
        let rail = if one.active_hooks().contains(ActiveHooks::MODIFY_SOLVER_CONTACTS) { one } else { two };
        let along = rail.rotation() * Vector::X;
        let across = Vector::new(-along.z, 0.0, along.x).normalize_or_zero();
        // A manifold's normal points from its first collider to its second.
        let side = (two.translation() - one.translation()).dot(across);
        let Some(manifold) = context.rigid_mut() else { return };
        *manifold.normal = if side < 0.0 { -across } else { across };
    }
}

/// The colliders of the fences and walls of `lines` (each tile's, with its `(col, row)`), piece by
/// piece, each with the object it is.
pub fn piece_colliders(lines: &[(u16, u16, Line)]) -> Vec<(Collider, Obj)> {
    line_pieces(lines).iter().map(|piece| (piece_collider(piece), Obj::Line(piece.id))).collect()
}

/// Wooden power poles: radius and height, metres (`render/lines.ts`).
const POLE_RADIUS: f32 = 0.14;
const POLE_HEIGHT: f32 = 9.5;
/// A steel pylon's legs: radius, height, and how far each stands from the line, metres.
const LEG_RADIUS: f32 = 0.5;
const LEG_HEIGHT: f32 = 23.0;
const LEG_OUT: f32 = 5.8;
/// How deep a pole is sunk into the ground, metres, so it stands on a slope.
const SUNK: f32 = 0.4;

/// The colliders of the poles and pylons of `lines`: upright cylinders, one a pole, two a pylon. A
/// power line's last point has a pole only where the line ends there (`Line::end`).
pub fn pole_colliders(lines: &[Line]) -> Vec<Collider> {
    let mut out = Vec::new();
    for line in lines.iter().filter(|line| matches!(line.kind, LINE_POWER | LINE_POWER_MAJOR)) {
        let points: Vec<[f32; 2]> = line.points.iter().map(|&[x, z]| [from_cm(x), from_cm(z)]).collect();
        let poles = if line.end { points.len() } else { points.len().saturating_sub(1) };
        for i in 0..poles {
            let base = line.base.get(i).map_or(0.0, |&base| from_cm(base));
            let cylinder = |x: f32, z: f32, radius: f32, height: f32| {
                let half = (height + SUNK) / 2.0;
                ColliderBuilder::cylinder(half, radius).translation(Vector::new(x, base - SUNK + half, z)).build()
            };
            let [x, z] = points[i];
            if line.kind == LINE_POWER {
                out.push(cylinder(x, z, POLE_RADIUS, POLE_HEIGHT));
                continue;
            }
            // Square to the line, halfway between its spans at a bend, as the client turns the beam.
            let heading = |p: [f32; 2], q: [f32; 2]| (q[1] - p[1]).atan2(q[0] - p[0]);
            let back = (i > 0).then(|| heading(points[i - 1], points[i]));
            let ahead = (i + 1 < points.len()).then(|| heading(points[i], points[i + 1]));
            let mean = match (back, ahead) {
                (Some(back), Some(ahead)) => back + (ahead - back).sin().atan2((ahead - back).cos()) / 2.0,
                (Some(heading), None) | (None, Some(heading)) => heading,
                (None, None) => 0.0,
            };
            let across = [-mean.sin(), mean.cos()];
            for out_by in [-LEG_OUT, LEG_OUT] {
                out.push(cylinder(x + across[0] * out_by, z + across[1] * out_by, LEG_RADIUS, LEG_HEIGHT));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{LINE_FENCE, LINE_WALL, ObjId};

    #[test]
    fn a_piece_is_a_box_along_its_line_from_under_the_ground_to_its_top() {
        // A wall up a slope: 3 m along +Z, rising 1 m.
        let line = Line { kind: LINE_WALL, points: vec![[0, 0], [0, 300]], base: vec![0, 100], end: true };
        let pieces = piece_colliders(&[(2, 3, line)]);
        assert_eq!(pieces.len(), 1);
        let (collider, object) = &pieces[0];
        assert_eq!(*object, Obj::Line(ObjId { col: 2, row: 3, index: 0 }));
        let aabb = collider.compute_aabb();
        // Over both ends' tops (1 m wall), under both ends' ground, and thin across.
        assert!(aabb.maxs.y > 1.9 && aabb.mins.y < -0.2, "{aabb:?}");
        assert!(aabb.mins.z < 0.0 && aabb.maxs.z > 3.0 && aabb.maxs.x < 0.6 && aabb.mins.x > -0.6, "{aabb:?}");
        // Halfway along, its middle is halfway up the slope.
        assert!((collider.translation().y - (0.5 - SUNK + (1.0 + SUNK) / 2.0)).abs() < 1e-4);
    }

    #[test]
    fn a_wall_is_in_the_trees_group_and_a_fence_in_one_no_body_touches() {
        let line = |kind| Line { kind, points: vec![[0, 0], [300, 0]], base: vec![0, 0], end: true };
        let pieces = piece_colliders(&[(0, 0, line(LINE_FENCE)), (0, 0, line(LINE_WALL))]);
        let car = InteractionGroups::all();
        assert!(!pieces[0].0.collision_groups().test(car), "a vehicle goes through a fence");
        assert!(pieces[1].0.collision_groups().test(car), "and stops on a wall");
        assert!(!pieces[1].0.collision_groups().test(crate::plows_trees()), "which a tank goes through");
        assert_eq!(pieces[1].1, Obj::Line(ObjId { col: 0, row: 0, index: 1 << 16 }));
    }

    #[test]
    fn a_pole_stands_at_each_point_but_where_the_line_goes_on() {
        let line = |kind, end| Line { kind, points: vec![[0, 0], [5000, 0], [10000, 0]], base: vec![0, 100, 200], end };
        assert_eq!(pole_colliders(&[line(LINE_POWER, true)]).len(), 3);
        assert_eq!(pole_colliders(&[line(LINE_POWER, false)]).len(), 2);
        assert_eq!(pole_colliders(&[line(LINE_POWER_MAJOR, false)]).len(), 4);
        assert!(pole_colliders(&[line(LINE_FENCE, true)]).is_empty());
        // The legs of a pylon on a line along x stand either side of it, along z.
        let legs = pole_colliders(&[line(LINE_POWER_MAJOR, true)]);
        assert!((legs[0].translation().z.abs() - LEG_OUT).abs() < 1e-4 && legs[0].translation().x.abs() < 1e-4);
    }
}
