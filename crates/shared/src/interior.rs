//! The inside of a house: its floors and the stairs between them, worked out from its footprint, its look
//! (`Tile::look`) and the openings on its walls. Nothing of it is in the world file or on the wire:
//! the server works it out as it reads the world and the client through the WebAssembly build
//! (`interior_of` in `crates/sim-wasm`), with this same code, so both have the same floors to walk
//! on. Its surfaces are marked into the building's cells (`Shell::furnish`), so a blast takes a bite
//! out of a floor as out of a wall and what hung from it falls.
//!
//! Only `+ - * /` and `sqrt` are used on the way, and every corner is rounded to a centimetre, so
//! a native build and a WebAssembly one get the same numbers.

use crate::carve::{CELL, Tri};
use crate::facade::{P, Poly, centre_of, split_plane};
use crate::windows::Window;

/// The top of a slab: a storey's floor.
pub const PART_FLOOR: u8 = 0;
/// The underside of a slab, the ceiling of the storey under it, and its edge round the stairwell.
pub const PART_CEILING: u8 = 1;
/// The slope a flight of stairs is climbed as, drawn as its underside.
pub const PART_RAMP: u8 = 2;
/// A tread or a riser of a flight: drawn, not collided with.
pub const PART_STEP: u8 = 3;

/// One triangle of a house's inside (centimetres) and what it is (`PART_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub tri: Tri,
    pub kind: u8,
}

impl Part {
    /// Whether the part is collided with and cut into cells: everything but the steps drawn over a ramp.
    pub fn collided(&self) -> bool {
        self.kind != PART_STEP
    }
}

/// The smallest house that gets floors: the long and the short side of the box round its footprint,
/// metres, and its floor area, square metres. Sheds and garages stay empty and shut.
const MIN_LONG: f64 = 6.0;
const MIN_SHORT: f64 = 5.0;
const MIN_AREA: f64 = 40.0;
/// How far above the foot of the walls the ground floor lies, centimetres: a plinth a soldier steps
/// up at the door (under `AUTOSTEP_HEIGHT`), so uneven ground inside the footprint stays under it.
pub const PLINTH: i32 = 30;
/// From one upper floor to the next, centimetres, as the rows of windows are (`windows::STOREY`).
const STOREY: i32 = 280;
/// An upper floor is laid only this far below the eaves or more, and with this much room under the ridge, centimetres.
const EAVE_CLEAR: i32 = 20;
const HEADROOM: i32 = 200;
/// Most upper floors in one house.
const MAX_STOREYS: usize = 8;
/// How thick an upper floor is, centimetres: its underside is this far under its top.
pub const SLAB: i32 = 20;
/// A flight of stairs: how wide, how far it runs, the landing at each end, metres, and the tallest riser.
const LANE: f64 = 1.0;
const RUN: f64 = 2.9;
const LANDING: f64 = 0.9;
const RISER: f64 = 0.19;
/// How far the stairwell keeps from the outer walls, and how far from the door, metres.
const WELL_INSET: f64 = 0.1;
const WELL_CLEAR: f64 = 1.0;
/// A slab keeps this far from a boundary between cells, centimetres, so the cell it is cut into
/// (`Shell::furnish`) is the cell it is judged by when that cell goes (`facade::standing`).
const OFF_LATTICE: i32 = 5;

/// `y` moved off the boundaries between cells (`OFF_LATTICE`).
fn off_lattice(y: i32) -> i32 {
    let m = y.rem_euclid(CELL);
    if m < OFF_LATTICE {
        y - m + OFF_LATTICE
    } else if m > CELL - OFF_LATTICE {
        y - m + CELL - OFF_LATTICE
    } else {
        y
    }
}

/// Numbers that look random and are the same for the same house everywhere (xorshift).
struct Dice(u64);

impl Dice {
    fn of(footprint: &[[i32; 2]]) -> Self {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for p in footprint {
            for c in p {
                h = (h ^ *c as u32 as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
        Dice(h | 1)
    }

    /// The next number, from 0 up to but not 1.
    fn roll(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// The house's own axes: `a` along its longest side and `b` across it, metres.
#[derive(Clone, Copy)]
struct Frame {
    u: [f64; 2],
    v: [f64; 2],
}

impl Frame {
    /// `[x, z]` (metres) as `[a, b]`.
    fn of(&self, [x, z]: [f64; 2]) -> [f64; 2] {
        [x * self.u[0] + z * self.u[1], x * self.v[0] + z * self.v[1]]
    }

    /// The point at `[a, b]`, `y` metres up.
    fn at(&self, [a, b]: [f64; 2], y: f64) -> P {
        [a * self.u[0] + b * self.v[0], y, a * self.u[1] + b * self.v[1]]
    }

    /// The axis (`0` for `a`, `1` for `b`) as a direction in the world.
    fn axis(&self, axis: usize) -> P {
        let d = if axis == 0 { self.u } else { self.v };
        [d[0], 0.0, d[1]]
    }
}

/// A rectangle in the house's axes: `[a0, a1, b0, b1]`.
type Rect = [f64; 4];

fn grown(r: Rect, by: f64) -> Rect {
    [r[0] - by, r[1] + by, r[2] - by, r[3] + by]
}

/// The footprint in the house's axes, to ask what lies inside it.
struct Plan {
    ring: Vec<[f64; 2]>,
}

impl Plan {
    /// Whether `[a, b]` is inside the footprint.
    fn holds(&self, [a, b]: [f64; 2]) -> bool {
        let mut inside = false;
        for i in 0..self.ring.len() {
            let (p, q) = (self.ring[i], self.ring[(i + 1) % self.ring.len()]);
            if (p[1] > b) != (q[1] > b) && a < p[0] + (b - p[1]) / (q[1] - p[1]) * (q[0] - p[0]) {
                inside = !inside;
            }
        }
        inside
    }

    /// Whether the whole of `rect` is inside the footprint: its edge is, at points half a metre apart or less.
    fn holds_rect(&self, [a0, a1, b0, b1]: Rect) -> bool {
        let steps = |len: f64| (len / 0.5) as usize + 1;
        let (na, nb) = (steps(a1 - a0), steps(b1 - b0));
        (0..=na).all(|i| {
            let a = a0 + (a1 - a0) * i as f64 / na as f64;
            self.holds([a, b0]) && self.holds([a, b1])
        }) && (0..=nb).all(|i| {
            let b = b0 + (b1 - b0) * i as f64 / nb as f64;
            self.holds([a0, b]) && self.holds([a1, b])
        })
    }
}

/// The stairwell: its rectangle, and the end its first flight starts from (`near`, in the corner)
/// and the end it comes up at (`far`), along `a`; `wall` is the side along `b` an outer wall is on.
struct Well {
    rect: Rect,
    near: f64,
    far: f64,
    wall: f64,
}

impl Well {
    /// `lane` 0 is the flight by the outer wall, 1 the one beside it: its stretch along `b`.
    fn lane(&self, lane: usize) -> [f64; 2] {
        let inward = if self.wall == self.rect[2] { 1.0 } else { -1.0 };
        let (x, y) = (self.wall + inward * LANE * lane as f64, self.wall + inward * LANE * (lane + 1) as f64);
        [x.min(y), x.max(y)]
    }

    /// Where flight `flight` (from storey `flight` to the one above) starts and ends along `a`:
    /// flights by the wall go from the near end to the far one, the others come back.
    fn run(&self, flight: usize) -> [f64; 2] {
        let dir = if self.far > self.near { 1.0 } else { -1.0 };
        let (from, to) = (self.near + dir * LANDING, self.far - dir * LANDING);
        if flight.is_multiple_of(2) { [from, to] } else { [to, from] }
    }
}

/// The triangles of the ring `ring` (centimetres), three corner numbers each, by clipping ears.
fn triangulate(ring: &[[i64; 2]]) -> Vec<[usize; 3]> {
    let cross = |o: [i64; 2], p: [i64; 2], q: [i64; 2]| (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
    let area: i64 = (0..ring.len()).map(|i| ring[i][0] * ring[(i + 1) % ring.len()][1] - ring[(i + 1) % ring.len()][0] * ring[i][1]).sum();
    let mut left: Vec<usize> = if area >= 0 { (0..ring.len()).collect() } else { (0..ring.len()).rev().collect() };
    let mut out = Vec::new();
    while left.len() > 3 {
        let n = left.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (ring[left[(i + n - 1) % n]], ring[left[i]], ring[left[(i + 1) % n]]);
            if cross(a, b, c) <= 0 {
                return false;
            }
            // No other corner inside the ear.
            !left.iter().any(|&k| {
                let p = ring[k];
                p != a && p != b && p != c && cross(a, b, p) >= 0 && cross(b, c, p) >= 0 && cross(c, a, p) >= 0
            })
        });
        // A ring that folds over itself has no ear left: what is cut so far is the floor.
        let Some(i) = ear else { return out };
        out.push([left[(i + n - 1) % n], left[i], left[(i + 1) % n]]);
        left.remove(i);
    }
    if left.len() == 3 && cross(ring[left[0]], ring[left[1]], ring[left[2]]) != 0 {
        out.push([left[0], left[1], left[2]]);
    }
    out
}

/// What is being built.
struct Build {
    frame: Frame,
    parts: Vec<Part>,
}

impl Build {
    fn tri(&mut self, kind: u8, a: P, b: P, c: P) {
        let cm = |p: P| p.map(|v| (v * 100.0).round() as i32);
        let (a, b, c) = (cm(a), cm(b), cm(c));
        if a != b && b != c && a != c {
            self.parts.push(Part { tri: [a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]], kind });
        }
    }

    fn poly(&mut self, kind: u8, poly: &[P]) {
        for i in 1..poly.len().saturating_sub(1) {
            self.tri(kind, poly[0], poly[i], poly[i + 1]);
        }
    }

    /// The upright rectangle on the line `axis = pos` from `lo` to `hi` along the other axis, from `y0` up to `y1`.
    fn upright(&mut self, kind: u8, axis: usize, pos: f64, [lo, hi]: [f64; 2], [y0, y1]: [f64; 2]) {
        if hi - lo < 0.01 || y1 - y0 < 0.01 {
            return;
        }
        let at = |along: f64, y: f64| self.frame.at(if axis == 0 { [pos, along] } else { [along, pos] }, y);
        let corners = [at(lo, y0), at(hi, y0), at(hi, y1), at(lo, y1)];
        self.poly(kind, &corners);
    }
}

/// `polys` with what lies inside `rect` (in the house's axes) cut out of them.
fn without(polys: Vec<Poly>, frame: &Frame, [a0, a1, b0, b1]: Rect) -> Vec<Poly> {
    let mut out = Vec::new();
    for poly in polys {
        let mut pieces = vec![poly];
        for (axis, d) in [(0, a0), (0, a1), (1, b0), (1, b1)] {
            let mut next = Vec::new();
            for piece in &pieces {
                let (behind, front) = split_plane(piece, frame.axis(axis), d);
                next.extend([behind, front].into_iter().filter(|part| part.len() >= 3));
            }
            pieces = next;
        }
        for piece in pieces {
            let c = centre_of(&piece);
            let [a, b] = frame.of([c[0], c[2]]);
            if !(a > a0 && a < a1 && b > b0 && b < b1) {
                out.push(piece);
            }
        }
    }
    out
}

/// The inside of the house on `footprint` (`[x, z]`, centimetres) whose look is `walls` and `roof`
/// and whose openings are `windows` (`windows::openings_of`): a ground floor on a plinth, and a
/// floor `SLAB` thick for every storey that fits under the eaves, with stairs up to it. Empty for a
/// house too small, or with no door to come in by. The outer door of a house with an inside is an
/// open doorway (`facade`).
pub fn rooms(footprint: &[[i32; 2]], walls: &[Tri], roof: &[Tri], windows: &[Window]) -> Vec<Part> {
    let mut ring: Vec<[i32; 2]> = Vec::with_capacity(footprint.len());
    for &p in footprint {
        if ring.last() != Some(&p) {
            ring.push(p);
        }
    }
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    let Some(door) = windows.iter().find(|w| w.door) else { return Vec::new() };
    if ring.len() < 3 || walls.is_empty() || roof.is_empty() {
        return Vec::new();
    }
    let heights = |tris: &[Tri]| tris.iter().flat_map(|t| [t[1], t[4], t[7]]).fold((i32::MAX, i32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
    let (base, _) = heights(walls);
    let (eave, ridge) = heights(roof);

    // The house's axes: along its longest side, turned so the box round it is longer along `a`.
    let metres = |p: [i32; 2]| [p[0] as f64 / 100.0, p[1] as f64 / 100.0];
    let mut longest = (0.0f64, [1.0f64, 0.0f64]);
    for i in 0..ring.len() {
        let (p, q) = (metres(ring[i]), metres(ring[(i + 1) % ring.len()]));
        let (dx, dz) = (q[0] - p[0], q[1] - p[1]);
        let length = (dx * dx + dz * dz).sqrt();
        if length > longest.0 {
            longest = (length, [dx / length, dz / length]);
        }
    }
    let u = longest.1;
    let mut frame = Frame { u, v: [-u[1], u[0]] };
    let boxed = |frame: &Frame| {
        ring.iter().map(|&p| frame.of(metres(p))).fold([f64::MAX, f64::MIN, f64::MAX, f64::MIN], |r, [a, b]| [r[0].min(a), r[1].max(a), r[2].min(b), r[3].max(b)])
    };
    let mut bounds = boxed(&frame);
    if bounds[3] - bounds[2] > bounds[1] - bounds[0] {
        frame = Frame { u: frame.v, v: [-frame.u[0], -frame.u[1]] };
        bounds = boxed(&frame);
    }
    let plan = Plan { ring: ring.iter().map(|&p| frame.of(metres(p))).collect() };
    let area = (0..plan.ring.len()).map(|i| { let (p, q) = (plan.ring[i], plan.ring[(i + 1) % plan.ring.len()]); p[0] * q[1] - q[0] * p[1] }).sum::<f64>().abs() / 2.0;
    let [a0, a1, b0, b1] = bounds;
    if a1 - a0 < MIN_LONG || b1 - b0 < MIN_SHORT || area < MIN_AREA {
        return Vec::new();
    }
    let mut dice = Dice::of(&ring);

    // Where the door is, for the stairwell to keep off.
    let door_at = frame.of([0, 2].map(|k| (door.corners[0][k] as f64 + door.corners[1][k] as f64) / 2.0));

    // The floors: the ground floor on its plinth, then one a storey while there is room under the roof.
    let mut levels = vec![off_lattice(base + PLINTH)];
    for k in 1..=MAX_STOREYS as i32 {
        let y = off_lattice(base + k * STOREY);
        if y > eave - EAVE_CLEAR || ridge - y < HEADROOM {
            break;
        }
        levels.push(y);
    }

    // The stairwell, in a corner of the box, the first that lies inside the footprint clear of the door.
    let mut well: Option<Well> = None;
    if levels.len() > 1 {
        let (length, width) = (RUN + 2.0 * LANDING, 2.0 * LANE);
        let first = (dice.roll() * 4.0) as usize;
        for corner in 0..4 {
            let (at_a1, at_b1) = ((corner + first) % 2 == 1, ((corner + first) / 2) % 2 == 1);
            let wa = if at_a1 { [a1 - WELL_INSET - length, a1 - WELL_INSET] } else { [a0 + WELL_INSET, a0 + WELL_INSET + length] };
            let wb = if at_b1 { [b1 - WELL_INSET - width, b1 - WELL_INSET] } else { [b0 + WELL_INSET, b0 + WELL_INSET + width] };
            let rect = [wa[0], wa[1], wb[0], wb[1]];
            let by_door = grown(rect, WELL_CLEAR);
            let blocks_door = door_at[0] > by_door[0] && door_at[0] < by_door[1] && door_at[1] > by_door[2] && door_at[1] < by_door[3];
            if !blocks_door && plan.holds_rect(grown(rect, WELL_INSET / 2.0)) {
                well = Some(Well { rect, near: if at_a1 { wa[1] } else { wa[0] }, far: if at_a1 { wa[0] } else { wa[1] }, wall: if at_b1 { wb[1] } else { wb[0] } });
                break;
            }
        }
        // No room for stairs: the house is one storey, open to its roof.
        if well.is_none() {
            levels.truncate(1);
        }
    }

    let mut build = Build { frame, parts: Vec::new() };

    // Slabs: the footprint at each floor's height, with the hole the flight from below comes up
    // through. An upper floor has an underside too, and an edge round the hole.
    let wide: Vec<[i64; 2]> = ring.iter().map(|p| [p[0] as i64, p[1] as i64]).collect();
    let slab = triangulate(&wide);
    for (k, &top) in levels.iter().enumerate() {
        let at = |y: i32| -> Vec<Poly> { slab.iter().map(|t| t.map(|i| { let p = metres(ring[i]); [p[0], y as f64 / 100.0, p[1]] }).to_vec()).collect() };
        let hole = well.as_ref().filter(|_| k > 0).map(|well| {
            let [from, to] = well.run(k - 1);
            let [lo, hi] = well.lane((k - 1) % 2);
            [from.min(to), from.max(to), lo, hi]
        });
        let cut = |polys: Vec<Poly>| match hole { Some(hole) => without(polys, &frame, hole), None => polys };
        for poly in &cut(at(top)) {
            build.poly(PART_FLOOR, poly);
        }
        if k == 0 {
            continue;
        }
        let under = off_lattice(top - SLAB);
        for poly in &cut(at(under)) {
            build.poly(PART_CEILING, poly);
        }
        if let Some([a0, a1, b0, b1]) = hole {
            let heights = [under as f64 / 100.0, top as f64 / 100.0];
            build.upright(PART_CEILING, 0, a0, [b0, b1], heights);
            build.upright(PART_CEILING, 0, a1, [b0, b1], heights);
            build.upright(PART_CEILING, 1, b0, [a0, a1], heights);
            build.upright(PART_CEILING, 1, b1, [a0, a1], heights);
        }
    }

    // Stairs: a slope to climb, and the steps drawn over it.
    if let Some(well) = &well {
        for flight in 0..levels.len() - 1 {
            let [from, to] = well.run(flight);
            let [lo, hi] = well.lane(flight % 2);
            let (y0, y1) = (levels[flight] as f64 / 100.0, levels[flight + 1] as f64 / 100.0);
            build.poly(PART_RAMP, &[frame.at([from, lo], y0), frame.at([from, hi], y0), frame.at([to, hi], y1), frame.at([to, lo], y1)]);
            let steps = ((y1 - y0) / RISER).ceil().max(1.0);
            let (tread, rise) = ((to - from) / steps, (y1 - y0) / steps);
            for i in 0..steps as usize {
                let (a, top) = (from + tread * i as f64, y0 + rise * (i + 1) as f64);
                build.upright(PART_STEP, 0, a, [lo, hi], [top - rise, top]);
                build.poly(PART_STEP, &[frame.at([a, lo], top), frame.at([a, hi], top), frame.at([a + tread, hi], top), frame.at([a + tread, lo], top)]);
            }
        }
    }

    build.parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::openings_of;
    use crate::world::Look;

    /// A house on `footprint` (centimetres, counter-clockwise from above) with walls `eave` high and
    /// a flat roof at `ridge`, as a look wound inwards, and its openings.
    fn house(footprint: &[[i32; 2]], eave: i32, ridge: i32) -> (Vec<Tri>, Vec<Tri>, Vec<Window>) {
        let mut walls: Vec<Tri> = Vec::new();
        for i in 0..footprint.len() {
            let (p, q) = (footprint[i], footprint[(i + 1) % footprint.len()]);
            walls.push([p[0], 0, p[1], q[0], 0, q[1], q[0], eave, q[1]]);
            walls.push([p[0], 0, p[1], q[0], eave, q[1], p[0], eave, p[1]]);
        }
        let wide: Vec<[i64; 2]> = footprint.iter().map(|p| [p[0] as i64, p[1] as i64]).collect();
        // A roof whose corners are at the eaves and whose middle is at the ridge would need a real
        // fit; for these tests a lid at the eaves and one triangle up at the ridge give the same heights.
        let mut roof: Vec<Tri> = triangulate(&wide).iter().map(|t| { let [a, b, c] = t.map(|i| footprint[i]); [a[0], eave, a[1], b[0], eave, b[1], c[0], eave, c[1]] }).collect();
        let [p, q] = [footprint[0], footprint[1]];
        roof.push([p[0], eave, p[1], q[0], eave, q[1], (p[0] + q[0]) / 2, ridge, (p[1] + q[1]) / 2 + 100]);
        let mut look = Look::default();
        for t in walls.iter().chain(&roof) {
            let at = look.vertices.len() as u32;
            look.vertices.extend([[t[0], t[1], t[2]], [t[3], t[4], t[5]], [t[6], t[7], t[8]]]);
            look.triangles.push([at, at + 1, at + 2]);
        }
        let windows = openings_of(&look, [0, walls.len() as u32, roof.len() as u32]);
        (walls, roof, windows)
    }

    fn rect(w: i32, d: i32) -> Vec<[i32; 2]> {
        vec![[0, 0], [w, 0], [w, d], [0, d]]
    }

    /// Whether the footprint holds `[x, z]` (centimetres).
    fn within(footprint: &[[i32; 2]], [x, z]: [f64; 2]) -> bool {
        Plan { ring: footprint.iter().map(|p| [p[0] as f64, p[1] as f64]).collect() }.holds([x, z])
    }

    /// The heights (centimetres) the house's floors are at, lowest first.
    fn floors(parts: &[Part]) -> Vec<i32> {
        let mut levels: Vec<i32> = parts.iter().filter(|p| p.kind == PART_FLOOR).map(|p| p.tri[1]).collect();
        levels.sort_unstable();
        levels.dedup();
        levels
    }

    #[test]
    fn a_shed_gets_no_rooms() {
        let footprint = rect(500, 400);
        let (walls, roof, windows) = house(&footprint, 300, 300);
        assert!(rooms(&footprint, &walls, &roof, &windows).is_empty());
    }

    #[test]
    fn a_house_without_a_door_stays_shut() {
        let footprint = rect(1200, 800);
        let (walls, roof, windows) = house(&footprint, 300, 300);
        let panes: Vec<Window> = windows.into_iter().filter(|w| !w.door).collect();
        assert!(rooms(&footprint, &walls, &roof, &panes).is_empty());
    }

    #[test]
    fn a_low_house_gets_a_ground_floor_and_nothing_more() {
        for (w, d) in [(1200, 800), (900, 700), (2000, 1100)] {
            let footprint = rect(w, d);
            let (walls, roof, windows) = house(&footprint, 300, 300);
            let parts = rooms(&footprint, &walls, &roof, &windows);
            assert_eq!(floors(&parts).len(), 1, "one storey under a 3 m roof");
            assert!(parts.iter().all(|p| p.kind == PART_FLOOR), "{w} × {d}: a floor on the plinth, with no underside");
        }
    }

    #[test]
    fn floors_stay_inside_an_l_shaped_footprint() {
        let footprint = vec![[0, 0], [1600, 0], [1600, 700], [800, 700], [800, 1400], [0, 1400]];
        let (walls, roof, windows) = house(&footprint, 600, 600);
        let parts = rooms(&footprint, &walls, &roof, &windows);
        assert_eq!(floors(&parts).len(), 2, "two storeys under a 6 m roof");
        for part in &parts {
            for k in 0..3 {
                let (x, z) = (part.tri[k * 3] as f64, part.tri[k * 3 + 2] as f64);
                // In the notch of the L there is nothing: a corner is in the footprint or on its edge.
                assert!(!(x > 801.0 && z > 701.0), "{:?} is in the notch", part.tri);
                assert!((-1.0..=1601.0).contains(&x) && (-1.0..=1401.0).contains(&z), "{:?} is outside", part.tri);
            }
        }
    }

    #[test]
    fn a_tall_house_gets_a_floor_a_storey_and_stairs_between_them() {
        let footprint = rect(1200, 800);
        let (walls, roof, windows) = house(&footprint, 900, 900);
        let parts = rooms(&footprint, &walls, &roof, &windows);
        // 9 m of wall: floors at the plinth, 2.8 and 5.6 m; one at 8.4 m would have no headroom.
        assert_eq!(floors(&parts).len(), 3, "{:?}", floors(&parts));
        let levels = floors(&parts);
        assert!((levels[0] - 30).abs() <= OFF_LATTICE && (levels[1] - 280).abs() <= OFF_LATTICE && (levels[2] - 560).abs() <= OFF_LATTICE, "{levels:?}");
        // Two flights, each a slope no steeper than a soldier climbs, from one floor to the next.
        let ramps: Vec<&Part> = parts.iter().filter(|p| p.kind == PART_RAMP).collect();
        assert_eq!(ramps.len(), 4, "two triangles a flight");
        for ramp in &ramps {
            let ys = [ramp.tri[1], ramp.tri[4], ramp.tri[7]];
            let (lo, hi) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
            assert!(levels.contains(&lo) && levels.contains(&hi) && hi > lo, "from a floor to the next, {ys:?}");
            let slope = ((hi - lo) as f64 / 100.0 / RUN).atan().to_degrees();
            assert!(slope < crate::consts::MAX_SLOPE_CLIMB_DEG as f64 - 4.0, "{slope}°");
        }
        // The upper floors have a hole over the flight that comes up through them, and the ground floor none.
        let area = |level: i32| parts.iter().filter(|p| p.kind == PART_FLOOR && p.tri[1] == level).map(|p| { let t = p.tri; (((t[3] - t[0]) as f64 * (t[8] - t[2]) as f64 - (t[5] - t[2]) as f64 * (t[6] - t[0]) as f64) / 2.0).abs() / 1e4 }).sum::<f64>();
        assert!((area(levels[0]) - 96.0).abs() < 0.01, "{}", area(levels[0]));
        for &level in &levels[1..] {
            assert!((area(level) - (96.0 - RUN * LANE)).abs() < 0.05, "{}", area(level));
        }
        assert!(parts.iter().any(|p| p.kind == PART_STEP));
        // An upper floor is a slab: its underside `SLAB` under its top, give or take the lattice, with the same hole.
        let unders: Vec<i32> = { let mut ys: Vec<i32> = parts.iter().filter(|p| p.kind == PART_CEILING && p.tri[1] == p.tri[4] && p.tri[4] == p.tri[7]).map(|p| p.tri[1]).collect(); ys.sort_unstable(); ys.dedup(); ys };
        assert_eq!(unders.len(), 2, "{unders:?}");
        for (under, top) in unders.iter().zip(&levels[1..]) {
            assert!((top - under - SLAB).abs() <= OFF_LATTICE, "{under} under {top}");
        }
        assert!(parts.iter().any(|p| p.kind == PART_CEILING && (p.tri[1] != p.tri[4] || p.tri[4] != p.tri[7])), "the hole has an edge");
    }

    #[test]
    fn a_house_too_short_for_stairs_keeps_one_storey() {
        // 6 × 5 m is rooms enough, but its footprint is cut at every corner, so no stairwell fits.
        let footprint = vec![[150, 0], [450, 0], [600, 150], [600, 350], [450, 500], [150, 500], [0, 350], [0, 150]];
        let (walls, roof, windows) = house(&footprint, 600, 600);
        if windows.iter().any(|w| w.door) {
            let parts = rooms(&footprint, &walls, &roof, &windows);
            assert!(floors(&parts).len() <= 1 && !parts.iter().any(|p| p.kind == PART_RAMP));
        }
    }

    #[test]
    fn a_house_turned_on_the_map_has_the_floors_it_has_square_on() {
        // The same 12 × 8 m house turned 30°: its corners rounded to the centimetre.
        let turned: Vec<[i32; 2]> = rect(1200, 800).iter().map(|p| { let (c, s) = (0.866_025_403_784f64, 0.5f64); [((p[0] as f64) * c - (p[1] as f64) * s).round() as i32 + 5000, ((p[0] as f64) * s + (p[1] as f64) * c).round() as i32 + 5000] }).collect();
        let (walls, roof, windows) = house(&turned, 600, 600);
        let parts = rooms(&turned, &walls, &roof, &windows);
        assert_eq!(floors(&parts).len(), 2);
        assert!(parts.iter().any(|p| p.kind == PART_RAMP) && parts.iter().any(|p| p.kind == PART_CEILING));
        for part in parts.iter().filter(|p| p.kind != PART_STEP) {
            for k in 0..3 {
                assert!(within(&turned, [part.tri[k * 3] as f64, part.tri[k * 3 + 2] as f64]) || {
                    // On the edge, to the centimetre: a step in towards the middle is inside.
                    let mid = [5000.0 + 600.0 * 0.866 - 400.0 * 0.5, 5000.0 + 600.0 * 0.5 + 400.0 * 0.866];
                    let p = [part.tri[k * 3] as f64, part.tri[k * 3 + 2] as f64];
                    within(&turned, [p[0] + (mid[0] - p[0]) * 0.01, p[1] + (mid[1] - p[1]) * 0.01])
                });
            }
        }
    }

    #[test]
    fn the_same_house_gets_the_same_floors() {
        let footprint = rect(1200, 800);
        let (walls, roof, windows) = house(&footprint, 600, 600);
        let parts = rooms(&footprint, &walls, &roof, &windows);
        assert_eq!(parts, rooms(&footprint, &walls, &roof, &windows));
        // Pinned, for `interior.spec.ts` to find the same through the WebAssembly build.
        let sum = parts.iter().fold(0u64, |h, p| p.tri.iter().fold(h.wrapping_mul(31).wrapping_add(p.kind as u64), |h, &v| h.wrapping_mul(31).wrapping_add(v as u32 as u64)));
        assert_eq!((parts.len(), sum % 1_000_000_007), (112, 490_359_646));
    }
}
