//! Windows and a door on a building's walls, worked out from its look (`Tile::look`). Mirrors
//! `client/src/render/windows.ts`, step for step: the client cuts these windows out of the walls it
//! draws, and here the same holes are cut out of the walls that are collided with (`facade`), so a
//! round goes through a window and nowhere else, and the pane it went through (`crossed`) is told to
//! every client to break.

use crate::world::Look;
use serde::{Deserialize, Serialize};

const STOREY: f64 = 2.8;
const SILL: f64 = 0.9;
const WINDOW_HEIGHT: f64 = 1.3;
const WINDOW_WIDTH: f64 = 1.0;
/// Gap between window middles along a wall, about, metres.
const PITCH: f64 = 2.8;
/// Space kept clear round a window inside its wall, metres.
const MARGIN: f64 = 0.35;
const DOOR_WIDTH: f64 = 1.0;
const DOOR_HEIGHT: f64 = 2.1;
/// How far outside a pane's rectangle a round still counts as through it, metres.
const SLACK: f32 = 0.03;

/// One window or door: its four corners on the wall (counter-clockwise seen from outside, the first
/// two along the bottom), metres, and the wall's outward normal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub corners: [[f32; 3]; 4],
    pub normal: [f32; 3],
    pub door: bool,
}

/// `Math.hypot` as V8 computes it: scaled by the largest and summed with compensation, so the client
/// and the server get the same last bit.
pub fn js_hypot(values: &[f64]) -> f64 {
    let max = values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if max == 0.0 {
        return 0.0;
    }
    let (mut sum, mut compensation) = (0.0f64, 0.0f64);
    for v in values {
        let n = v.abs() / max;
        let summand = n * n - compensation;
        let preliminary = sum + summand;
        compensation = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// `Math.round`: halves go up.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

struct Plane {
    normal: [f64; 3],
    /// Along the wall, horizontal.
    u: [f64; 3],
    tris: Vec<[[f64; 2]; 3]>,
    u0: f64,
    u1: f64,
    v0: f64,
    v1: f64,
    d: f64,
}

/// The windows and door of the building whose look is `[first, walls, roof]` in `look`.
pub fn openings_of(look: &Look, [first, walls, _]: [u32; 3]) -> Vec<Window> {
    // As the client reads them: centimetres to metres, held in 32-bit floats.
    let corner = |v: u32| -> [f64; 3] { look.vertices.get(v as usize).map_or([0.0; 3], |p| p.map(|c| (c as f64 / 100.0) as f32 as f64)) };
    let mut planes: Vec<Plane> = Vec::new();
    let mut keys: std::collections::HashMap<(i64, i64), usize> = std::collections::HashMap::new();
    for t in first..first + walls {
        let Some(tri) = look.triangles.get(t as usize) else { break };
        // Wound outwards: the look is wound inwards.
        let (a, b, c) = (corner(tri[0]), corner(tri[2]), corner(tri[1]));
        let e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let f = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let mut n = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
        let l = js_hypot(&n);
        if l < 1e-6 {
            continue;
        }
        n = [n[0] / l, n[1] / l, n[2] / l];
        // Upright walls only.
        if n[1].abs() > 0.1 {
            continue;
        }
        let h = js_hypot(&[n[0], n[2]]);
        n = [n[0] / h, 0.0, n[2] / h];
        let d = n[0] * a[0] + n[2] * a[2];
        let key = (js_round(n[2].atan2(n[0]) * 100.0) as i64, js_round(d * 10.0) as i64);
        let at = *keys.entry(key).or_insert_with(|| {
            planes.push(Plane { normal: n, u: [-n[2], 0.0, n[0]], tris: Vec::new(), u0: f64::INFINITY, u1: f64::NEG_INFINITY, v0: f64::INFINITY, v1: f64::NEG_INFINITY, d });
            planes.len() - 1
        });
        let plane = &mut planes[at];
        let flat = [a, b, c].map(|p| [p[0] * plane.u[0] + p[2] * plane.u[2], p[1]]);
        plane.tris.push(flat);
        for [u, v] in flat {
            plane.u0 = plane.u0.min(u);
            plane.u1 = plane.u1.max(u);
            plane.v0 = plane.v0.min(v);
            plane.v1 = plane.v1.max(v);
        }
    }
    let inside = |plane: &Plane, u: f64, v: f64| {
        plane.tris.iter().any(|[a, b, c]| {
            let s = |p: &[f64; 2], q: &[f64; 2]| (q[0] - p[0]) * (v - p[1]) - (q[1] - p[1]) * (u - p[0]);
            let (x, y, z) = (s(a, b), s(b, c), s(c, a));
            (x >= 0.0 && y >= 0.0 && z >= 0.0) || (x <= 0.0 && y <= 0.0 && z <= 0.0)
        })
    };
    let fits = |plane: &Plane, u0: f64, u1: f64, v0: f64, v1: f64| {
        [[u0 - MARGIN, v0 - MARGIN], [u1 + MARGIN, v0 - MARGIN], [u1 + MARGIN, v1 + MARGIN], [u0 - MARGIN, v1 + MARGIN]].iter().all(|&[u, v]| inside(plane, u, v))
    };
    // The longest wall first, and of two as long the one with the smaller offset.
    let mut order: Vec<usize> = (0..planes.len()).collect();
    order.sort_by(|&i, &j| {
        let (a, b) = (&planes[i], &planes[j]);
        let (la, lb) = (a.u1 - a.u0, b.u1 - b.u0);
        if la == lb { a.d.partial_cmp(&b.d).unwrap_or(std::cmp::Ordering::Equal) } else { lb.partial_cmp(&la).unwrap_or(std::cmp::Ordering::Equal) }
    });
    let mut out = Vec::new();
    for (rank, &at) in order.iter().enumerate() {
        let plane = &planes[at];
        let length = plane.u1 - plane.u0;
        if length < WINDOW_WIDTH + 2.0 * MARGIN {
            continue;
        }
        let count = js_round(length / PITCH).max(1.0);
        let step = length / count;
        let to_world = |u: f64, v: f64| -> [f32; 3] { [(plane.u[0] * u + plane.normal[0] * plane.d) as f32, v as f32, (plane.u[2] * u + plane.normal[2] * plane.d) as f32] };
        let normal = plane.normal.map(|v| v as f32);
        let mut push = |u0: f64, u1: f64, v0: f64, v1: f64, door: bool| out.push(Window { corners: [to_world(u0, v0), to_world(u1, v0), to_world(u1, v1), to_world(u0, v1)], normal, door });
        // Rows a storey apart, as high as the wall reaches (`fits` keeps them off a gable's slopes).
        let mut storey = 0.0f64;
        while plane.v0 + SILL + storey * STOREY + WINDOW_HEIGHT + MARGIN <= plane.v1 {
            let v0 = plane.v0 + SILL + storey * STOREY;
            let mut i = 0.0f64;
            while i < count {
                let mid = plane.u0 + (i + 0.5) * step;
                // The longest wall's door: the middle of its ground row.
                if rank == 0 && storey == 0.0 && i == (count / 2.0).floor() && fits(plane, mid - DOOR_WIDTH / 2.0, mid + DOOR_WIDTH / 2.0, plane.v0 + MARGIN, plane.v0 + DOOR_HEIGHT) {
                    push(mid - DOOR_WIDTH / 2.0, mid + DOOR_WIDTH / 2.0, plane.v0 + 0.02, plane.v0 + DOOR_HEIGHT, true);
                    i += 1.0;
                    continue;
                }
                let (u0, u1) = (mid - WINDOW_WIDTH / 2.0, mid + WINDOW_WIDTH / 2.0);
                if fits(plane, u0, u1, v0, v0 + WINDOW_HEIGHT) {
                    push(u0, u1, v0, v0 + WINDOW_HEIGHT, false);
                }
                i += 1.0;
            }
            storey += 1.0;
        }
    }
    out
}

/// Where a round from `origin` going `reach` metres along the unit `dir` passes through a window's
/// glass (a door is no glass): the point on each pane it crosses, nearest first.
pub fn crossed(windows: &[Window], origin: [f32; 3], dir: [f32; 3], reach: f32) -> Vec<[f32; 3]> {
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut hits: Vec<(f32, [f32; 3])> = Vec::new();
    for w in windows.iter().filter(|w| !w.door) {
        let [a, b, _, d] = w.corners;
        let n = w.normal;
        let denominator = dot(n, dir);
        if denominator.abs() < 1e-5 {
            continue;
        }
        let t = (dot(n, a) - dot(n, origin)) / denominator;
        if !(0.0..=reach).contains(&t) {
            continue;
        }
        let p = [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t];
        let width = (b[0] - a[0]).hypot(b[2] - a[2]);
        let along = [(b[0] - a[0]) / width, 0.0, (b[2] - a[2]) / width];
        let u = dot([p[0] - a[0], 0.0, p[2] - a[2]], along);
        let v = p[1] - a[1];
        if u >= -SLACK && u <= width + SLACK && v >= -SLACK && v <= d[1] - a[1] + SLACK {
            hits.push((t, p));
        }
    }
    hits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    hits.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box house 5 × 4 × 3 m as a look: walls then a flat roof, wound inwards.
    fn house() -> Look {
        let (w, d, h) = (500, 400, 300);
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut quad = |a: [i32; 3], b: [i32; 3], c: [i32; 3], e: [i32; 3]| {
            let base = vertices.len() as u32;
            vertices.extend([a, b, c, e]);
            triangles.push([base, base + 1, base + 2]);
            triangles.push([base, base + 2, base + 3]);
        };
        quad([0, 0, 0], [w, 0, 0], [w, h, 0], [0, h, 0]);
        quad([w, 0, 0], [w, 0, d], [w, h, d], [w, h, 0]);
        quad([w, 0, d], [0, 0, d], [0, h, d], [w, h, d]);
        quad([0, 0, d], [0, 0, 0], [0, h, 0], [0, h, d]);
        quad([0, h, 0], [w, h, 0], [w, h, d], [0, h, d]);
        Look { vertices, triangles, buildings: vec![[0, 8, 2]] }
    }

    #[test]
    fn windows_on_every_wall_and_a_door_on_the_longest() {
        let openings = openings_of(&house(), [0, 8, 2]);
        assert_eq!(openings.iter().filter(|o| o.door).count(), 1);
        let door = openings.iter().find(|o| o.door).unwrap();
        assert!(door.normal[2].abs() > 0.9, "on a 5 m wall, along z");
        for axis in [0, 2] {
            for sign in [-1.0, 1.0] {
                assert!(openings.iter().any(|o| o.normal[axis].signum() == sign && o.normal[axis].abs() > 0.9), "a window faces {axis} {sign}");
            }
        }
        for o in &openings {
            for p in o.corners {
                assert!(p[0] >= -1e-6 && p[0] <= 5.0 + 1e-6 && p[1] <= 3.0, "{p:?} within the walls");
            }
        }
    }

    #[test]
    fn a_round_through_a_pane_is_seen_and_one_past_it_is_not() {
        let openings = openings_of(&house(), [0, 8, 2]);
        let pane = openings.iter().find(|o| !o.door && o.normal[2] < -0.9).unwrap();
        let [a, b, _, d] = pane.corners;
        let centre = [(a[0] + b[0]) / 2.0, (a[1] + d[1]) / 2.0, a[2]];
        let hits = crossed(&openings, [centre[0], centre[1], -5.0], [0.0, 0.0, 1.0], 7.0);
        assert_eq!(hits.len(), 1, "the front pane, not yet the back wall's at 9 m");
        assert_eq!(crossed(&openings, [centre[0], centre[1], -5.0], [0.0, 0.0, 1.0], 10.0).len(), 2, "and out through the back");
        assert!((hits[0][0] - centre[0]).abs() < 1e-4 && (hits[0][2] - centre[2]).abs() < 1e-4);
        assert!(crossed(&openings, [centre[0] + 0.8, centre[1], -5.0], [0.0, 0.0, 1.0], 10.0).is_empty(), "beside it");
        assert!(crossed(&openings, [centre[0], centre[1], -5.0], [0.0, 0.0, 1.0], 4.0).is_empty(), "stopped short of it");
    }

    #[test]
    fn hypot_matches_the_browser_on_plain_values() {
        assert_eq!(js_hypot(&[3.0, 4.0]), 5.0);
        assert_eq!(js_hypot(&[0.0, 0.0, 0.0]), 0.0);
        assert!((js_hypot(&[1.0, 1.0, 1.0]) - 3f64.sqrt()).abs() < 1e-15);
    }
}
