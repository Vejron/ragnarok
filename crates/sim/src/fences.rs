//! Fences and power poles (`Tile::lines`) as something to walk into.
//!
//! A pole is a collider like a rock's: it stops soldiers, vehicles and bullets alike. A fence stops a
//! soldier only. Vehicles go through it, and so do bullets, through wire, and rays. So it is not a
//! collider, which every query would meet, but lines on the ground that a soldier's step is cut short
//! of (`Fences::stop`) after the character controller has moved them. Server and client do the same
//! arithmetic over the same lines in the same order, so a step against a fence is predicted.

use rapier3d::prelude::*;
use shared::quant::from_cm;
use shared::world::{LINE_FENCE, LINE_FENCE_HIGH, LINE_POWER, LINE_POWER_MAJOR, Line};
use std::collections::BTreeMap;

/// Height of a wire fence and of a game fence, metres, as the client draws them (`render/lines.ts`).
pub const FENCE_HEIGHT: f32 = 1.25;
pub const GAME_FENCE_HEIGHT: f32 = 2.05;
/// How far from a fence's line its posts and wire reach, metres: a soldier keeps their radius past it.
const FENCE_HALF: f32 = 0.06;
/// Feet this little under a fence's top still clear it, metres.
const CLEAR: f32 = 0.02;

/// One straight piece of a fence: its ends `[x, z]` and the height of its top over each, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fence {
    a: [f32; 2],
    b: [f32; 2],
    top: [f32; 2],
}

/// The fences of `lines`, piece by piece.
pub fn fences_of(lines: &[Line]) -> Vec<Fence> {
    let mut out = Vec::new();
    for line in lines {
        let height = match line.kind {
            LINE_FENCE => FENCE_HEIGHT,
            LINE_FENCE_HIGH => GAME_FENCE_HEIGHT,
            _ => continue,
        };
        let at = |i: usize| [from_cm(line.points[i][0]), from_cm(line.points[i][1])];
        let top = |i: usize| line.base.get(i).map_or(0.0, |&base| from_cm(base)) + height;
        for i in 1..line.points.len() {
            out.push(Fence { a: at(i - 1), b: at(i), top: [top(i - 1), top(i)] });
        }
    }
    out
}

/// Every fence in the world, by the tile it came in, so a streamed world holds them in the order the
/// server's whole world does.
#[derive(Clone, Debug, Default)]
pub struct Fences {
    by_tile: BTreeMap<(u16, u16), Vec<Fence>>,
}

impl Fences {
    pub fn set(&mut self, tile: (u16, u16), fences: Vec<Fence>) {
        if fences.is_empty() {
            self.by_tile.remove(&tile);
        } else {
            self.by_tile.insert(tile, fences);
        }
    }

    pub fn remove(&mut self, tile: (u16, u16)) {
        self.by_tile.remove(&tile);
    }

    fn all(&self) -> impl Iterator<Item = &Fence> {
        self.by_tile.values().flatten()
    }

    /// Where a body of `radius` stepping from `from` to `to` (`[x, z]`), its feet at `feet`, ends up:
    /// on the side of every fence it started on, no nearer to it than its radius, sliding along it.
    /// A fence whose top is under its feet is no obstacle.
    pub fn stop(&self, from: [f32; 2], mut to: [f32; 2], feet: f32, radius: f32) -> [f32; 2] {
        let reach = radius + FENCE_HALF;
        // Twice round, for a corner, where pushing off one piece can push into the next.
        for _ in 0..2 {
            for fence in self.all() {
                let (a, b) = (fence.a, fence.b);
                let lo = [a[0].min(b[0]) - reach, a[1].min(b[1]) - reach];
                let hi = [a[0].max(b[0]) + reach, a[1].max(b[1]) + reach];
                if from[0].max(to[0]) < lo[0] || from[1].max(to[1]) < lo[1] || from[0].min(to[0]) > hi[0] || from[1].min(to[1]) > hi[1] {
                    continue;
                }
                let d = [b[0] - a[0], b[1] - a[1]];
                let length2 = d[0] * d[0] + d[1] * d[1];
                if length2 < 1e-6 {
                    continue;
                }
                let along = |p: [f32; 2]| (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length2).clamp(0.0, 1.0);
                let u = along(to);
                if feet >= fence.top[0] + (fence.top[1] - fence.top[0]) * u - CLEAR {
                    continue;
                }
                let length = length2.sqrt();
                let n = [-d[1] / length, d[0] / length];
                let side = |p: [f32; 2]| (p[0] - a[0]) * n[0] + (p[1] - a[1]) * n[1];
                let (s0, s1) = (side(from), side(to));
                let back = if s0 < 0.0 { -1.0 } else { 1.0 };
                // Through the line within the piece: back to the side it came from.
                if s0 * s1 < 0.0 {
                    let k = s0 / (s0 - s1);
                    let hit = [from[0] + (to[0] - from[0]) * k, from[1] + (to[1] - from[1]) * k];
                    let w = ((hit[0] - a[0]) * d[0] + (hit[1] - a[1]) * d[1]) / length2;
                    if (0.0..=1.0).contains(&w) {
                        to = [to[0] + n[0] * (back * reach - s1), to[1] + n[1] * (back * reach - s1)];
                    }
                }
                // Too near: out from the nearest point of the piece.
                let u = along(to);
                let near = [a[0] + d[0] * u, a[1] + d[1] * u];
                let e = [to[0] - near[0], to[1] - near[1]];
                let distance = e[0].hypot(e[1]);
                if distance < reach {
                    let away = if distance > 1e-4 { [e[0] / distance, e[1] / distance] } else { [n[0] * back, n[1] * back] };
                    to = [near[0] + away[0] * reach, near[1] + away[1] * reach];
                }
            }
        }
        to
    }

    /// Whether a fence higher than `floor` passes within `reach` of `at` (`[x, z]`): for the bots' grid.
    pub fn near(&self, at: [f32; 2], reach: f32, floor: f32) -> bool {
        self.all().any(|fence| {
            let (a, b) = (fence.a, fence.b);
            let d = [b[0] - a[0], b[1] - a[1]];
            let length2 = (d[0] * d[0] + d[1] * d[1]).max(1e-6);
            let u = (((at[0] - a[0]) * d[0] + (at[1] - a[1]) * d[1]) / length2).clamp(0.0, 1.0);
            let near = [a[0] + d[0] * u, a[1] + d[1] * u];
            (at[0] - near[0]).hypot(at[1] - near[1]) < reach && fence.top[0] + (fence.top[1] - fence.top[0]) * u > floor
        })
    }
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

    fn fence(points: &[[i32; 2]], kind: u8) -> Fences {
        let line = Line { kind, points: points.to_vec(), base: vec![0; points.len()], end: true };
        let mut fences = Fences::default();
        fences.set((0, 0), fences_of(&[line]));
        fences
    }

    #[test]
    fn a_step_through_a_fence_stops_short_of_it_and_slides_along() {
        // Along z = 0 from x = -10 m to 10 m.
        let fences = fence(&[[-1000, 0], [1000, 0]], LINE_FENCE);
        let [x, z] = fences.stop([0.0, -1.0], [0.3, 1.0], 0.0, 0.35);
        assert!((z + 0.41).abs() < 1e-4, "kept its radius and the posts' on its own side: {z}");
        assert!((x - 0.3).abs() < 1e-4, "slid along: {x}");
        // Walking along it is no trouble, nor is walking away.
        assert_eq!(fences.stop([0.0, -1.0], [2.0, -1.0], 0.0, 0.35), [2.0, -1.0]);
        // Round its end is open.
        assert_eq!(fences.stop([11.0, -1.0], [11.0, 1.0], 0.0, 0.35), [11.0, 1.0]);
        // From the other side, back the other way.
        assert!(fences.stop([0.0, 1.0], [0.0, -1.0], 0.0, 0.35)[1] > 0.4);
    }

    #[test]
    fn feet_over_the_top_clear_it_and_a_game_fence_is_higher() {
        let wire = fence(&[[-1000, 0], [1000, 0]], LINE_FENCE);
        assert_eq!(wire.stop([0.0, -1.0], [0.0, 1.0], 1.3, 0.35), [0.0, 1.0]);
        let game = fence(&[[-1000, 0], [1000, 0]], LINE_FENCE_HIGH);
        assert!(game.stop([0.0, -1.0], [0.0, 1.0], 1.3, 0.35)[1] < 0.0);
    }

    #[test]
    fn a_corner_holds_from_inside() {
        // An L: along z = 0, then up x = 0.
        let fences = fence(&[[-1000, 0], [0, 0], [0, 1000]], LINE_FENCE);
        let [x, z] = fences.stop([-1.0, 1.0], [0.5, -0.5], 0.0, 0.35);
        assert!(x <= -0.4 && z >= 0.4, "{x} {z}");
    }

    #[test]
    fn the_grid_sees_a_fence_where_it_stands() {
        let fences = fence(&[[-1000, 0], [1000, 0]], LINE_FENCE);
        assert!(fences.near([0.0, 0.5], 0.75, 0.0));
        assert!(!fences.near([0.0, 1.0], 0.75, 0.0));
        assert!(!fences.near([0.0, 0.5], 0.75, 2.0), "a floor over its top");
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
