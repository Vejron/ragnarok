//! Windows and a door on a building's walls, worked out from its look (`Tile::look`). Mirrors
//! `client/src/render/windows.ts`, step for step: the client cuts these windows out of the walls it
//! draws, and here the same holes are cut out of the walls that are collided with (`facade`), so a
//! round goes through a window and nowhere else, and the pane it went through (`crossed`) is told to
//! every client to break.

use crate::world::{BUILDING_BARN, BUILDING_CHURCH, BUILDING_FARMHOUSE, BUILDING_FIRE_STATION, BUILDING_HALL, BUILDING_OUTBUILDING, BUILDING_PUBLIC, BUILDING_SHOP, BUILDING_VILLA, Look};
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
/// A wall whose foot is this far over the building's lowest is raised (a dormer's front, the step
/// up to a higher part of the roof): its windows are small, set low in it, and it has no door, metres.
const RAISED: f64 = 2.0;
const SMALL_SILL: f64 = 0.2;
const SMALL_HEIGHT: f64 = 0.7;
const SMALL_WIDTH: f64 = 0.8;
const SMALL_MARGIN: f64 = 0.15;
const SMALL_PITCH: f64 = 2.0;
/// How far outside a pane's rectangle a round still counts as through it, metres.
const SLACK: f32 = 0.03;
/// Two pieces of wall are one wall when their normals are this much alike (their dot product, about
/// a degree) and the corners of the one lie within this of the other's plane, metres.
const SAME_WAY: f64 = 0.9998;
const SAME_WALL: f64 = 0.06;
/// A church's long walls, at least this long, metres, carry one row of tall round-headed windows:
/// this wide, about this far apart, their sills this far up (or three tenths of a lower wall), and
/// reaching to this far under the wall's top (or fifteen hundredths of it) but no taller than
/// `CHURCH_HEIGHT`, metres.
const CHURCH_WALL: f64 = 8.0;
const CHURCH_WIDTH: f64 = 1.6;
const CHURCH_PITCH: f64 = 6.0;
const CHURCH_SILL: f64 = 2.2;
const CHURCH_TOP: f64 = 1.2;
const CHURCH_HEIGHT: f64 = 6.0;
/// A church's door, in the middle of its front end, metres.
const CHURCH_DOOR_WIDTH: f64 = 2.0;
const CHURCH_DOOR_HEIGHT: f64 = 3.2;
/// The round window in each end of a church: how far across, metres; its middle this share of the
/// way up the end, and its foot at least this far over the door's top, metres.
const ROSE: f64 = 2.2;
const ROSE_UP: f64 = 0.35;
const ROSE_CLEAR: f64 = 1.0;

/// How the walls of a barn, an outbuilding, a hall, a shop or a public building are opened: doors in
/// the middle of its longest walls, and windows in rows.
struct Style {
    /// The doors to try, widest first, `[width, height]`, metres: the first that fits its wall is hung.
    doors: &'static [[f64; 2]],
    /// How many of the walls, the front first, have a door.
    doored: usize,
    /// The most doors side by side in such a wall, `BAY_GAP` of wall between them: a row of garage doors.
    bays: f64,
    /// The row of windows: its sill over the wall's foot, a window's height and width, and the gap
    /// between window middles, about, metres.
    window: [f64; 4],
    /// Which walls carry the row, counted from the longest: from this one up to, not including, that one.
    walls: [usize; 2],
    /// The most windows in one wall.
    most: f64,
    /// Whether the row is repeated a storey up for as long as the wall reaches, or is the only one.
    storeys: bool,
}

/// The door of a house, `[width, height]`, metres: what any kind falls back on where its own does not fit.
const DOOR: [f64; 2] = [DOOR_WIDTH, DOOR_HEIGHT];
/// A barn: a gate a tractor drives in by, and small square windows far apart along its two long walls.
const BARN: Style = Style { doors: &[[3.2, 3.0], [2.4, 2.4], DOOR], doored: 1, bays: 1.0, window: [1.5, 0.7, 0.7, 7.0], walls: [0, 2], most: f64::INFINITY, storeys: false };
/// An outbuilding: a garage door, and one small window in the wall after the door's.
const OUTBUILDING: Style = Style { doors: &[[2.4, 2.1], DOOR], doored: 1, bays: 1.0, window: [1.2, 0.7, 0.7, 2.8], walls: [1, 2], most: 1.0, storeys: false };
/// A hall: a gate for lorries in each long wall, and a ribbon of wide low windows high up all round.
const HALL: Style = Style { doors: &[[4.0, 4.0], [3.2, 3.0], DOOR], doored: 2, bays: 1.0, window: [3.0, 1.0, 2.0, 3.2], walls: [0, usize::MAX], most: f64::INFINITY, storeys: false };
/// A shop: tall panes from near the ground along its front, with a double door in the middle of them.
const SHOP: Style = Style { doors: &[[1.6, 2.3], DOOR], doored: 1, bays: 1.0, window: [0.5, 2.0, 2.2, 3.0], walls: [0, 1], most: f64::INFINITY, storeys: false };
/// A public building: a double door, and wide tall windows round every wall on every storey.
const PUBLIC: Style = Style { doors: &[[1.6, 2.3], DOOR], doored: 1, bays: 1.0, window: [0.9, 1.6, 1.8, 3.0], walls: [0, usize::MAX], most: f64::INFINITY, storeys: true };

/// A fire station: a row of garage doors for the engines in its front, and a public building's windows.
const FIRE_STATION: Style = Style { doors: &[[3.6, 3.6], [3.0, 3.0], DOOR], doored: 1, bays: 3.0, window: [0.9, 1.6, 1.8, 3.0], walls: [0, usize::MAX], most: f64::INFINITY, storeys: true };
/// How much wall stands between two doors of a row, metres.
const BAY_GAP: f64 = 0.8;
/// The front of a building that stands by a road (`Building::front`) is the wall that faces the road
/// most squarely, of those at least this share of its longest wall long and turned no further from
/// the road than about 60° (this, the cosine of it, times the 127 a unit of `front` is).
const FRONT_SHARE: f64 = 0.6;
const FRONT_FACING: f64 = 63.5;

/// An old wooden house: tall narrow windows close together, on every wall and storey.
const FARMHOUSE: Style = Style { doors: &[DOOR], doored: 1, bays: 1.0, window: [0.8, 1.5, 0.9, 2.3], walls: [0, usize::MAX], most: f64::INFINITY, storeys: true };
/// A post-war villa: wide low windows far apart.
const VILLA: Style = Style { doors: &[DOOR], doored: 1, bays: 1.0, window: [1.0, 1.2, 1.7, 3.6], walls: [0, usize::MAX], most: f64::INFINITY, storeys: true };

/// The style of a `kind` (`BUILDING_*`) that has one; a house and a church are opened by their own rules.
fn style_of(kind: u8) -> Option<&'static Style> {
    match kind {
        BUILDING_BARN => Some(&BARN),
        BUILDING_OUTBUILDING => Some(&OUTBUILDING),
        BUILDING_HALL => Some(&HALL),
        BUILDING_SHOP => Some(&SHOP),
        BUILDING_PUBLIC => Some(&PUBLIC),
        BUILDING_FARMHOUSE => Some(&FARMHOUSE),
        BUILDING_VILLA => Some(&VILLA),
        BUILDING_FIRE_STATION => Some(&FIRE_STATION),
        _ => None,
    }
}

/// A door at least this wide, metres, is a gate (a barn's, a hall's, a garage's): it is driven
/// through, and it is opened and shut (`gates`).
pub const GATE_WIDTH: f32 = 2.2;

impl Window {
    /// Whether the opening is a gate.
    pub fn is_gate(&self) -> bool {
        let [a, b, ..] = self.corners;
        self.door && (b[0] - a[0]).hypot(b[2] - a[2]) >= GATE_WIDTH
    }
}

/// Which of `windows` are gates, by their number among them.
pub fn gates(windows: &[Window]) -> Vec<u32> {
    (0..windows.len() as u32).filter(|&at| windows[at as usize].is_gate()).collect()
}

/// How an opening is drawn at its top: square; with a round arch over its width; or as a circle
/// (its hole is a square). The hole cut in the wall is the rectangle of its corners whatever its head.
pub const HEAD_SQUARE: u8 = 0;
pub const HEAD_ARCH: u8 = 1;
pub const HEAD_ROUND: u8 = 2;

/// One window or door: its four corners on the wall (counter-clockwise seen from outside, the first
/// two along the bottom), metres, and the wall's outward normal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub corners: [[f32; 3]; 4],
    pub normal: [f32; 3],
    pub door: bool,
    /// `HEAD_*`.
    pub head: u8,
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

/// The windows and door of the building whose look is `[first, walls, roof]` in `look`, which is a
/// `kind` (`BUILDING_*`) and stands by the road that lies `front` of it (`Building::front`). A house
/// has rows of windows a storey apart and a door in its front, the wall facing the road or else its
/// longest; a church a row of tall windows along its long walls, and a door and a round window in an end;
/// a barn, an outbuilding, a hall and a shop the doors and the one row of windows of their `Style`.
pub fn openings_of(look: &Look, [first, walls, _]: [u32; 3], kind: u8, front: [i8; 2]) -> Vec<Window> {
    // A bridge has no windows, no doors and so no inside; a shelter is open along one side instead.
    if kind == crate::world::BUILDING_BRIDGE || kind == crate::world::BUILDING_SHELTER {
        return Vec::new();
    }
    // As the client reads them: centimetres to metres, held in 32-bit floats.
    let corner = |v: u32| -> [f64; 3] { look.vertices.get(v as usize).map_or([0.0; 3], |p| p.map(|c| (c as f64 / 100.0) as f32 as f64)) };
    let mut planes: Vec<Plane> = Vec::new();
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
        // The wall it is a piece of: one facing the same way that all its corners lie in. A wall is
        // often in pieces (a gable end is cut at the ridge) whose corners were rounded apart by a centimetre.
        let found = planes.iter().position(|plane| {
            plane.normal[0] * n[0] + plane.normal[2] * n[2] > SAME_WAY && [a, b, c].iter().all(|p| (plane.normal[0] * p[0] + plane.normal[2] * p[2] - plane.d).abs() < SAME_WALL)
        });
        let at = found.unwrap_or_else(|| {
            let d = n[0] * a[0] + n[2] * a[2];
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
    let fits = |plane: &Plane, u0: f64, u1: f64, v0: f64, v1: f64, margin: f64| {
        [[u0 - margin, v0 - margin], [u1 + margin, v0 - margin], [u1 + margin, v1 + margin], [u0 - margin, v1 + margin]].iter().all(|&[u, v]| inside(plane, u, v))
    };
    let lowest = planes.iter().fold(f64::INFINITY, |lowest, plane| lowest.min(plane.v0));
    // The longest wall first, and of two as long the one with the smaller offset.
    let mut order: Vec<usize> = (0..planes.len()).collect();
    order.sort_by(|&i, &j| {
        let (a, b) = (&planes[i], &planes[j]);
        let (la, lb) = (a.u1 - a.u0, b.u1 - b.u0);
        if la == lb { a.d.partial_cmp(&b.d).unwrap_or(std::cmp::Ordering::Equal) } else { lb.partial_cmp(&la).unwrap_or(std::cmp::Ordering::Equal) }
    });
    let is_raised = |plane: &Plane| plane.v0 - lowest >= RAISED;
    // The front goes first: the wall that faces the road, where one of the longer walls does. A
    // church's front is the end away from its annexe, whatever road passes it.
    if kind != BUILDING_CHURCH && front != [0, 0] {
        let longest = order.iter().map(|&at| &planes[at]).filter(|plane| !is_raised(plane)).fold(0.0f64, |longest, plane| longest.max(plane.u1 - plane.u0));
        let mut best: Option<(usize, f64)> = None;
        for (rank, &at) in order.iter().enumerate() {
            let plane = &planes[at];
            let facing = plane.normal[0] * f64::from(front[0]) + plane.normal[2] * f64::from(front[1]);
            if !is_raised(plane) && plane.u1 - plane.u0 >= FRONT_SHARE * longest && facing > best.map_or(FRONT_FACING, |(_, most)| most) {
                best = Some((rank, facing));
            }
        }
        if let Some((rank, _)) = best {
            let at = order.remove(rank);
            order.insert(0, at);
        }
    }
    if let Some(style) = style_of(kind) {
        let [sill, height, width, pitch] = style.window;
        let mut out = Vec::new();
        // The walls from the ground, longest first: a raised one is left blank.
        for (rank, &at) in order.iter().filter(|&&at| !is_raised(&planes[at])).enumerate() {
            let plane = &planes[at];
            let length = plane.u1 - plane.u0;
            let to_world = |u: f64, v: f64| -> [f32; 3] { [(plane.u[0] * u + plane.normal[0] * plane.d) as f32, v as f32, (plane.u[2] * u + plane.normal[2] * plane.d) as f32] };
            let normal = plane.normal.map(|v| v as f32);
            let mut push = |u0: f64, u1: f64, v0: f64, v1: f64, door: bool| out.push(Window { corners: [to_world(u0, v0), to_world(u1, v0), to_world(u1, v1), to_world(u0, v1)], normal, door, head: HEAD_SQUARE });
            // The doors, a row about the middle of the wall, as many as it has room for: the row
            // is `[from, to, top]` once it is hung.
            let mid = (plane.u0 + plane.u1) / 2.0;
            let mut door: Option<[f64; 3]> = None;
            if rank < style.doored {
                for &[w, h] in style.doors {
                    let bays = ((length - 2.0 * MARGIN + BAY_GAP) / (w + BAY_GAP)).floor().min(style.bays).max(1.0);
                    let mut i = 0.0f64;
                    while i < bays {
                        let at = mid + (i - (bays - 1.0) / 2.0) * (w + BAY_GAP);
                        if fits(plane, at - w / 2.0, at + w / 2.0, plane.v0 + MARGIN, plane.v0 + h, MARGIN) {
                            push(at - w / 2.0, at + w / 2.0, plane.v0 + 0.02, plane.v0 + h, true);
                            door = Some([door.map_or(at - w / 2.0, |[from, ..]| from), at + w / 2.0, plane.v0 + h]);
                        }
                        i += 1.0;
                    }
                    if door.is_some() {
                        break;
                    }
                }
            }
            if rank < style.walls[0] || rank >= style.walls[1] || length < width + 2.0 * MARGIN {
                continue;
            }
            let count = js_round(length / pitch).max(1.0).min(style.most);
            let step = length / count;
            let mut storey = 0.0f64;
            while plane.v0 + sill + storey * STOREY + height + MARGIN <= plane.v1 && (style.storeys || storey < 1.0) {
                let v0 = plane.v0 + sill + storey * STOREY;
                let mut i = 0.0f64;
                while i < count {
                    let at = plane.u0 + (i + 0.5) * step;
                    let (u0, u1) = (at - width / 2.0, at + width / 2.0);
                    // A window keeps clear of the door in its wall.
                    let by_door = door.is_some_and(|[from, to, top]| u1 + MARGIN > from && u0 - MARGIN < to && v0 - MARGIN < top);
                    if !by_door && fits(plane, u0, u1, v0, v0 + height, MARGIN) {
                        push(u0, u1, v0, v0 + height, false);
                    }
                    i += 1.0;
                }
                storey += 1.0;
            }
        }
        return out;
    }
    // A church's long walls run the way its longest does; the two widest walls across that are its ends.
    let long = order.first().map_or([1.0, 0.0, 0.0], |&at| planes[at].u);
    let along = |plane: &Plane| (plane.u[0] * long[0] + plane.u[2] * long[2]).abs() > 0.7;
    let is_long = |plane: &Plane| along(plane) && plane.u1 - plane.u0 >= CHURCH_WALL;
    let mut ends: Vec<usize> = Vec::new();
    let mut door_end: Option<usize> = None;
    if kind == BUILDING_CHURCH {
        ends = order.iter().copied().filter(|&at| !is_raised(&planes[at]) && !along(&planes[at])).take(2).collect();
        // How far an end is from the lesser walls (an annexe's), summed: the front is the end away from them.
        let far = |end: usize| {
            let e = &planes[end];
            let mut sum = 0.0f64;
            for &at in &order {
                let p = &planes[at];
                if ends.contains(&at) || is_raised(p) || is_long(p) {
                    continue;
                }
                let mid = (p.u0 + p.u1) / 2.0;
                let (x, z) = (p.u[0] * mid + p.normal[0] * p.d, p.u[2] * mid + p.normal[2] * p.d);
                sum += (e.normal[0] * x + e.normal[2] * z - e.d).abs();
            }
            sum
        };
        let mut fronts = ends.clone();
        if fronts.len() == 2 && far(fronts[1]) > far(fronts[0]) + 0.01 {
            fronts.swap(0, 1);
        }
        door_end = fronts.into_iter().find(|&at| {
            let p = &planes[at];
            let mid = (p.u0 + p.u1) / 2.0;
            fits(p, mid - CHURCH_DOOR_WIDTH / 2.0, mid + CHURCH_DOOR_WIDTH / 2.0, p.v0 + MARGIN, p.v0 + CHURCH_DOOR_HEIGHT, MARGIN)
        });
    }
    // A church with no end its door fits in (a hut of a chapel) is given windows as a house is.
    let church = door_end.is_some();
    let mut out = Vec::new();
    for (rank, &at) in order.iter().enumerate() {
        let plane = &planes[at];
        let raised = is_raised(plane);
        let length = plane.u1 - plane.u0;
        let to_world = |u: f64, v: f64| -> [f32; 3] { [(plane.u[0] * u + plane.normal[0] * plane.d) as f32, v as f32, (plane.u[2] * u + plane.normal[2] * plane.d) as f32] };
        let normal = plane.normal.map(|v| v as f32);
        let mut push = |u0: f64, u1: f64, v0: f64, v1: f64, door: bool, head: u8| out.push(Window { corners: [to_world(u0, v0), to_world(u1, v0), to_world(u1, v1), to_world(u0, v1)], normal, door, head });
        // The wall of a church's nave standing over its annexe's roof is left blank.
        if church && raised {
            continue;
        }
        if church && !raised && ends.contains(&at) {
            // An end of a church: the door in the front one, and a round window over the middle of each.
            let mid = (plane.u0 + plane.u1) / 2.0;
            if door_end == Some(at) {
                push(mid - CHURCH_DOOR_WIDTH / 2.0, mid + CHURCH_DOOR_WIDTH / 2.0, plane.v0 + 0.02, plane.v0 + CHURCH_DOOR_HEIGHT, true, HEAD_SQUARE);
            }
            let centre = plane.v0 + (ROSE_UP * (plane.v1 - plane.v0)).max(CHURCH_DOOR_HEIGHT + ROSE_CLEAR + ROSE / 2.0);
            if fits(plane, mid - ROSE / 2.0, mid + ROSE / 2.0, centre - ROSE / 2.0, centre + ROSE / 2.0, MARGIN) {
                push(mid - ROSE / 2.0, mid + ROSE / 2.0, centre - ROSE / 2.0, centre + ROSE / 2.0, false, HEAD_ROUND);
            }
            continue;
        }
        if church && !raised && is_long(plane) {
            // A long wall of a church: one row of tall round-headed windows.
            let tall = plane.v1 - plane.v0;
            let sill = CHURCH_SILL.min(tall * 0.3);
            let height = CHURCH_HEIGHT.min(tall - sill - CHURCH_TOP.min(tall * 0.15));
            let count = js_round(length / CHURCH_PITCH).max(1.0);
            let step = length / count;
            let mut i = 0.0f64;
            while i < count {
                let mid = plane.u0 + (i + 0.5) * step;
                let (u0, u1, v0) = (mid - CHURCH_WIDTH / 2.0, mid + CHURCH_WIDTH / 2.0, plane.v0 + sill);
                if fits(plane, u0, u1, v0, v0 + height, MARGIN) {
                    push(u0, u1, v0, v0 + height, false, HEAD_ARCH);
                }
                i += 1.0;
            }
            continue;
        }
        let (sill, height, width, margin, pitch) = if raised { (SMALL_SILL, SMALL_HEIGHT, SMALL_WIDTH, SMALL_MARGIN, SMALL_PITCH) } else { (SILL, WINDOW_HEIGHT, WINDOW_WIDTH, MARGIN, PITCH) };
        if length < width + 2.0 * margin {
            continue;
        }
        let count = js_round(length / pitch).max(1.0);
        let step = length / count;
        // Rows a storey apart, as high as the wall reaches (`fits` keeps them off a gable's slopes);
        // the lesser walls of a church (its annexe's) have the one row.
        let mut storey = 0.0f64;
        while plane.v0 + sill + storey * STOREY + height + margin <= plane.v1 && !(church && storey >= 1.0) {
            let v0 = plane.v0 + sill + storey * STOREY;
            let mut i = 0.0f64;
            while i < count {
                let mid = plane.u0 + (i + 0.5) * step;
                // The longest wall's door: the middle of its ground row.
                if !church && !raised && rank == 0 && storey == 0.0 && i == (count / 2.0).floor() && fits(plane, mid - DOOR_WIDTH / 2.0, mid + DOOR_WIDTH / 2.0, plane.v0 + MARGIN, plane.v0 + DOOR_HEIGHT, MARGIN) {
                    push(mid - DOOR_WIDTH / 2.0, mid + DOOR_WIDTH / 2.0, plane.v0 + 0.02, plane.v0 + DOOR_HEIGHT, true, HEAD_SQUARE);
                    i += 1.0;
                    continue;
                }
                let (u0, u1) = (mid - width / 2.0, mid + width / 2.0);
                if fits(plane, u0, u1, v0, v0 + height, margin) {
                    push(u0, u1, v0, v0 + height, false, HEAD_SQUARE);
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
    use crate::world::BUILDING_PLAIN;

    /// A box house 5 × 4 × 3 m as a look: walls then a flat roof, wound inwards.
    fn house() -> Look {
        block(500, 400, 300)
    }

    /// A box `w` × `d` × `h` centimetres as a look: walls then a flat roof, wound inwards.
    fn block(w: i32, d: i32, h: i32) -> Look {
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
        let openings = openings_of(&house(), [0, 8, 2], BUILDING_PLAIN, [0, 0]);
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

    /// The width and height of an opening, metres.
    fn size(o: &Window) -> (f32, f32) {
        let [a, b, _, d] = o.corners;
        ((b[0] - a[0]).hypot(b[2] - a[2]), d[1] - a[1])
    }

    #[test]
    fn a_barn_has_one_gate_in_a_long_wall_and_a_few_small_windows_along_its_long_walls_only() {
        // 22 × 11 m, its walls 4 m high.
        let openings = openings_of(&block(2200, 1100, 400), [0, 8, 2], BUILDING_BARN, [0, 0]);
        let doors: Vec<&Window> = openings.iter().filter(|o| o.door).collect();
        assert_eq!(doors.len(), 1);
        assert!(doors[0].normal[2].abs() > 0.9, "in a 22 m wall");
        let (w, h) = size(doors[0]);
        assert!((w - 3.2).abs() < 1e-4 && (h - 2.98).abs() < 1e-4, "{w} × {h}: wide enough to drive in by");
        let panes: Vec<&Window> = openings.iter().filter(|o| !o.door).collect();
        assert!(!panes.is_empty() && panes.len() <= 8, "{} windows", panes.len());
        for pane in &panes {
            assert!(pane.normal[2].abs() > 0.9, "none in an end");
            let (w, h) = size(pane);
            assert!(w < 0.75 && h < 0.75, "{w} × {h}: small");
            // Clear of the gate.
            let [a, b, ..] = pane.corners;
            let [from, to, ..] = doors[0].corners;
            assert!(pane.normal[2] != doors[0].normal[2] || a[0].max(b[0]) < from[0].min(to[0]) || a[0].min(b[0]) > from[0].max(to[0]), "{pane:?}");
        }
        // A house that long has several times the glass.
        assert!(openings_of(&block(2200, 1100, 400), [0, 8, 2], BUILDING_PLAIN, [0, 0]).len() > 3 * panes.len());
    }

    #[test]
    fn a_low_barn_gets_the_gate_that_fits_under_its_eaves() {
        let openings = openings_of(&block(1200, 600, 300), [0, 8, 2], BUILDING_BARN, [0, 0]);
        let (w, h) = size(openings.iter().find(|o| o.door).unwrap());
        assert!((w - 2.4).abs() < 1e-4 && (h - 2.38).abs() < 1e-4, "{w} × {h}");
    }

    #[test]
    fn an_outbuilding_has_a_garage_door_and_one_small_window() {
        let openings = openings_of(&block(600, 400, 280), [0, 8, 2], BUILDING_OUTBUILDING, [0, 0]);
        let (w, _) = size(openings.iter().find(|o| o.door).unwrap());
        assert!((w - 2.4).abs() < 1e-4, "{w}");
        assert_eq!(openings.iter().filter(|o| !o.door).count(), 1);
    }

    #[test]
    fn a_hall_has_a_lorry_gate_in_each_long_wall_and_a_ribbon_of_windows_high_up() {
        // 40 × 20 m, its walls 7 m high.
        let openings = openings_of(&block(4000, 2000, 700), [0, 8, 2], BUILDING_HALL, [0, 0]);
        let doors: Vec<&Window> = openings.iter().filter(|o| o.door).collect();
        assert_eq!(doors.len(), 2);
        assert!(doors[0].normal[2] * doors[1].normal[2] < -0.9, "one in each long wall");
        assert!(doors.iter().all(|d| (size(d).0 - 4.0).abs() < 1e-4));
        let panes: Vec<&Window> = openings.iter().filter(|o| !o.door).collect();
        assert!(panes.len() > 20, "{} windows", panes.len());
        assert!(panes.iter().all(|p| p.corners[0][1] >= 3.0 - 1e-4 && size(p).0 > size(p).1), "wide and low, over a soldier's head");
        for axis in [0, 2] {
            assert!(panes.iter().any(|p| p.normal[axis].abs() > 0.9), "round every wall");
        }
    }

    #[test]
    fn a_shop_has_a_glass_front_and_blank_walls_behind_it() {
        let openings = openings_of(&block(3000, 2000, 450), [0, 8, 2], BUILDING_SHOP, [0, 0]);
        let door = openings.iter().find(|o| o.door).unwrap();
        let panes: Vec<&Window> = openings.iter().filter(|o| !o.door).collect();
        assert!(panes.len() >= 6, "{} panes", panes.len());
        assert!(panes.iter().all(|p| p.normal == door.normal), "all in the front");
        assert!(panes.iter().all(|p| size(p).1 > 1.9 && p.corners[0][1] < 0.6), "from near the ground");
    }

    #[test]
    fn an_old_house_has_tall_windows_close_together_and_a_villa_wide_ones_far_apart() {
        let of = |kind: u8| openings_of(&block(1200, 900, 300), [0, 8, 2], kind, [0, 0]);
        let (old, villa) = (of(BUILDING_FARMHOUSE), of(BUILDING_VILLA));
        for openings in [&old, &villa] {
            assert_eq!(openings.iter().filter(|o| o.door).count(), 1, "one front door");
            for axis in [0, 2] {
                assert!(openings.iter().any(|o| !o.door && o.normal[axis].abs() > 0.9), "windows round every wall");
            }
        }
        assert!(old.iter().filter(|o| !o.door).all(|o| size(o).1 > size(o).0), "taller than wide");
        assert!(villa.iter().filter(|o| !o.door).all(|o| size(o).0 > size(o).1), "wider than tall");
        assert!(old.len() > villa.len(), "{} and {}", old.len(), villa.len());
    }

    #[test]
    fn a_fire_station_has_a_row_of_garage_doors_in_its_front_and_as_many_as_its_wall_has_room_for() {
        let doors = |w: i32| openings_of(&block(w, 1200, 620), [0, 8, 2], crate::world::BUILDING_FIRE_STATION, [0, 0]).into_iter().filter(|o| o.door).collect::<Vec<_>>();
        let row = doors(3000);
        assert_eq!(row.len(), 3, "three bays");
        assert!(row.iter().all(|door| door.is_gate() && door.normal == row[0].normal && (size(door).0 - 3.6).abs() < 0.01 && (size(door).1 - 3.58).abs() < 0.01), "{row:?}");
        // Side by side, with wall between them, about the middle of the 30 m front.
        let spans: Vec<[f32; 2]> = row.iter().map(|door| { let xs = door.corners.map(|c| c[0]); [xs.iter().copied().fold(f32::MAX, f32::min), xs.iter().copied().fold(f32::MIN, f32::max)] }).collect();
        let (from, to) = (spans.iter().map(|s| s[0]).fold(f32::MAX, f32::min), spans.iter().map(|s| s[1]).fold(f32::MIN, f32::max));
        assert!((to - from - (3.0 * 3.6 + 2.0 * 0.8)).abs() < 0.01 && ((from + to) / 2.0 - 15.0).abs() < 0.01, "{spans:?}");
        // No window of the front reaches into the row.
        let all = openings_of(&block(3000, 1200, 620), [0, 8, 2], crate::world::BUILDING_FIRE_STATION, [0, 0]);
        assert!(all.iter().filter(|o| !o.door && o.normal == row[0].normal && o.corners[0][1] < 3.6).all(|o| { let x = o.corners.map(|c| c[0]); x.iter().all(|&x| x < from - 0.3 || x > to + 0.3) }));
        assert_eq!(doors(1000).len(), 2, "a 10 m front has room for two");
    }

    #[test]
    fn the_front_of_a_building_by_a_road_is_the_wall_facing_the_road() {
        // 12 × 9 m: the walls at z = 0 and z = 9 are the longest, the ones at x = 0 and x = 12 three quarters of them.
        let door = |kind: u8, front: [i8; 2]| openings_of(&block(1200, 900, 300), [0, 8, 2], kind, front).into_iter().find(|o| o.door).unwrap().normal;
        for kind in [BUILDING_PLAIN, BUILDING_FARMHOUSE, BUILDING_SHOP, BUILDING_BARN] {
            let unset = door(kind, [0, 0]);
            assert_eq!(unset[0], 0.0, "with no road, a long wall");
            assert_eq!(door(kind, [0, 127]), [0.0, 0.0, 1.0], "the long wall towards the road");
            assert_eq!(door(kind, [0, -127]), [0.0, 0.0, -1.0]);
            assert_eq!(door(kind, [127, 0]), [1.0, 0.0, 0.0], "an end nearly as long, when the road passes it");
            assert_eq!(door(kind, [-90, 90])[1], 0.0);
        }
        // An end much shorter than the long walls is never the front: the long wall nearest the road is.
        let long = |front: [i8; 2]| openings_of(&block(2000, 600, 300), [0, 8, 2], BUILDING_PLAIN, front).into_iter().find(|o| o.door).unwrap().normal;
        assert_eq!(long([127, 0]), long([0, 0]));
        assert_eq!(long([110, 64]), [0.0, 0.0, 1.0]);
        // A church keeps its door in its end.
        let look = church();
        assert_eq!(openings_of(&look, look.buildings[0], BUILDING_CHURCH, [0, 0]), openings_of(&look, look.buildings[0], BUILDING_CHURCH, [127, 0]));
    }

    #[test]
    fn a_public_building_has_a_double_door_and_big_windows_on_every_storey() {
        // 30 × 12 m, two storeys.
        let openings = openings_of(&block(3000, 1200, 620), [0, 8, 2], crate::world::BUILDING_PUBLIC, [0, 0]);
        let (w, _) = size(openings.iter().find(|o| o.door).unwrap());
        assert!((w - 1.6).abs() < 1e-4, "{w}");
        let panes: Vec<&Window> = openings.iter().filter(|o| !o.door).collect();
        assert!(panes.iter().all(|p| size(p).0 > 1.7 && size(p).1 > 1.5), "big");
        let mut sills: Vec<i32> = panes.iter().map(|p| (p.corners[0][1] * 100.0).round() as i32).collect();
        sills.sort_unstable();
        sills.dedup();
        assert_eq!(sills, [90, 370], "a row a storey");
        for axis in [0, 2] {
            assert!(panes.iter().any(|p| p.normal[axis].abs() > 0.9), "round every wall");
        }
    }

    #[test]
    fn a_round_through_a_pane_is_seen_and_one_past_it_is_not() {
        let openings = openings_of(&house(), [0, 8, 2], BUILDING_PLAIN, [0, 0]);
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

    /// A church as a look: a nave 30 × 12 m along x with walls 9 m high and its ridge 17 m up, and a
    /// sacristy 6 × 5 m with a flat roof 4 m up against the far (north) wall near the east end.
    fn church() -> Look {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut face = |corners: &[[i32; 3]]| {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(corners);
            for i in 1..corners.len() as u32 - 1 {
                triangles.push([base, base + i, base + i + 1]);
            }
        };
        let (w, d, eave, ridge) = (3000, 1200, 900, 1700);
        // The south wall, the two gable ends, and the north wall either side of and over the sacristy.
        face(&[[0, 0, 0], [w, 0, 0], [w, eave, 0], [0, eave, 0]]);
        face(&[[w, 0, 0], [w, 0, d], [w, eave, d], [w, ridge, d / 2], [w, eave, 0]]);
        // The west end in two pieces, cut at the ridge and rounded a centimetre apart, as the importer leaves it.
        face(&[[0, 0, d], [0, 0, d / 2], [0, ridge, d / 2], [0, eave, d]]);
        face(&[[1, 0, d / 2], [1, 0, 0], [1, eave, 0], [1, ridge, d / 2]]);
        face(&[[2000, 0, d], [0, 0, d], [0, eave, d], [2000, eave, d]]);
        face(&[[w, 0, d], [2600, 0, d], [2600, eave, d], [w, eave, d]]);
        face(&[[2600, 400, d], [2000, 400, d], [2000, eave, d], [2600, eave, d]]);
        // The sacristy's three walls.
        face(&[[2600, 0, d], [2600, 0, d + 500], [2600, 400, d + 500], [2600, 400, d]]);
        face(&[[2600, 0, d + 500], [2000, 0, d + 500], [2000, 400, d + 500], [2600, 400, d + 500]]);
        face(&[[2000, 0, d + 500], [2000, 0, d], [2000, 400, d], [2000, 400, d + 500]]);
        let walls = triangles.len() as u32;
        Look { vertices, triangles, buildings: vec![[0, walls, 0]] }
    }

    #[test]
    fn a_church_has_tall_arched_windows_along_its_nave_and_a_door_and_a_round_window_in_its_end() {
        let look = church();
        let openings = openings_of(&look, look.buildings[0], BUILDING_CHURCH, [0, 0]);
        // One row of five along the 30 m south wall, 6 m tall under round heads.
        let south: Vec<&Window> = openings.iter().filter(|o| o.normal[2] < -0.9).collect();
        assert_eq!(south.len(), 5);
        for o in &south {
            assert_eq!((o.head, o.door), (HEAD_ARCH, false));
            assert!((o.corners[0][1] - 2.2).abs() < 1e-4 && (o.corners[3][1] - 7.8).abs() < 1e-4, "{:?}", o.corners);
        }
        // The north wall has them too, but none where the sacristy stands against it.
        let north: Vec<&Window> = openings.iter().filter(|o| o.normal[2] > 0.9 && o.head == HEAD_ARCH).collect();
        assert_eq!(north.len(), 3);
        assert!(north.iter().all(|o| o.corners[0][0] < 20.0 && o.corners[1][0] < 20.0));
        // The door is in the west end, away from the sacristy, and each end has its round window.
        let doors: Vec<&Window> = openings.iter().filter(|o| o.door).collect();
        assert_eq!(doors.len(), 1);
        assert!(doors[0].normal[0] < -0.9, "the door faces {:?}", doors[0].normal);
        assert!((doors[0].corners[1][2] - doors[0].corners[0][2]).abs() > 1.99 && (doors[0].corners[3][1] - 3.2).abs() < 1e-4);
        assert!(((doors[0].corners[0][2] + doors[0].corners[1][2]) / 2.0 - 6.0).abs() < 1e-4, "in the middle of the end, under the ridge");
        for sign in [-1.0, 1.0] {
            let round: Vec<&Window> = openings.iter().filter(|o| o.head == HEAD_ROUND && o.normal[0] * sign > 0.9).collect();
            assert_eq!(round.len(), 1);
            assert!((round[0].corners[3][1] - round[0].corners[0][1] - 2.2).abs() < 1e-4);
            assert!(round[0].corners[0][1] >= 3.2 + 1.0 - 1e-4, "over the door");
        }
        // The sacristy keeps a house's windows, one row of them, and no door.
        let low: Vec<&Window> = openings.iter().filter(|o| o.head == HEAD_SQUARE && !o.door).collect();
        assert!(!low.is_empty() && low.iter().all(|o| o.corners[3][1] <= 4.0));
    }

    #[test]
    fn the_same_walls_as_a_house_get_rows_of_small_windows() {
        let look = church();
        let openings = openings_of(&look, look.buildings[0], BUILDING_PLAIN, [0, 0]);
        assert!(openings.iter().all(|o| o.head == HEAD_SQUARE));
        assert!(openings.iter().filter(|o| o.normal[2] < -0.9).count() > 20);
    }
}
