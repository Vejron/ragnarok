//! Bridges: a deck, piers and, over wide water, a pylon with stay cables, built as triangles for
//! the tile's collision mesh (the client draws the same triangles, `Tile::bridges`).
//!
//! OSM only says that a way is a bridge and on which layer; there are no heights. The deck's
//! height is inferred: it follows the ground between its two ends, is lifted to clear the water
//! surface where it crosses water (the river bed is carved deep, so a road draped on the ground
//! would lie under the water), and where it ends above ground it carries on as a ramp down to the
//! ground.

use shared::terrain::Ground;
use shared::world::{NO_WATER, ROAD_MAJOR, ROAD_PATH};

type P = [f64; 2];
type V3 = [f64; 3];

/// Distance between deck cross sections, metres.
const STEP: f64 = 3.0;
/// Shorter spans (culverts tagged as bridges) are left to the terrain.
const MIN_LENGTH: f64 = 12.0;
/// Steepest slope of a deck, rise over run.
const MAX_GRADE: f64 = 0.06;
/// Slope of the ramp that brings a deck still above ground at its end down to the ground.
const RAMP_GRADE: f64 = 0.06;
const MAX_RAMP: f64 = 150.0;
const THICKNESS: f64 = 0.7;
/// The deck's top is this far above the ground where it meets it.
const LIFT: f64 = 0.12;
/// Deck sections this close to the ground are solid down to it; higher ones stand on piers.
const SOLID_BELOW: f64 = 2.0;
const PIER_SPACING: f64 = 24.0;
const PARAPET: f64 = 1.0;
/// A pylon is only worth building over a crossing at least this long, metres.
const PYLON_MIN_SPAN: f64 = 60.0;
const CABLE_SPACING: f64 = 12.0;

/// Where a bridge's pylons stand. OpenStreetMap does not say which bridges are cable-stayed (there
/// is no structure tag here, and the lidar canopy is no better), so the few that are in this map
/// are listed in `LANDMARKS`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pylons {
    None,
    /// One tower at each end of the crossing, cables running in over the water.
    Ends,
    /// One tower in the middle of the crossing.
    Middle,
}

/// Cable-stayed bridges as (latitude, longitude, radius in metres, pylons): a bridge way whose
/// midpoint lies within the radius is built with these pylons. Lundabron has its towers at the two
/// ends; Kolbäcksbron has one in the middle, over each of its two carriageways.
pub const LANDMARKS: [(f64, f64, f64, Pylons); 2] = [(63.8263, 20.2204, 150.0, Pylons::Ends), (63.8013, 20.2994, 150.0, Pylons::Middle)];

/// A piece of the bridge, assigned to the tile of `at` so a long bridge is cut at tile borders.
pub struct Piece {
    pub at: P,
    /// Steel (pylon, cables) rather than concrete (deck, piers, parapets).
    pub steel: bool,
    pub tris: Vec<[V3; 3]>,
}

struct Node {
    p: P,
    /// Deck top.
    y: f64,
    /// Ground (the bed under water).
    g: f64,
    /// Water surface, where the sample is under water.
    level: Option<f64>,
    /// Distance along the deck from its first node.
    arc: f64,
}

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: V3) -> V3 {
    let length = dot(a, a).sqrt();
    if length > 0.0 { scale(a, 1.0 / length) } else { a }
}

/// The surface normal the client computes for a triangle (`flatGeometry` after it reverses the
/// winding): it must point out of the solid.
pub fn client_normal([a, b, c]: [V3; 3]) -> V3 {
    cross(sub(b, c), sub(a, c))
}

/// Ground heights are looked up on the grid, so is the water: `levels` has one entry per sample.
fn water_at(ground: &Ground, levels: &[i16], p: P) -> Option<f64> {
    if levels.len() != ground.heights.len() || ground.is_flat() {
        return None;
    }
    let at = |v: f64| ((v * 100.0 - ground.origin as f64) / ground.cell as f64).round();
    let (i, j) = (at(p[0]), at(p[1]));
    let last = ground.side as f64 - 1.0;
    if i < 0.0 || j < 0.0 || i > last || j > last {
        return None;
    }
    let index = j as usize * ground.side as usize + i as usize;
    // Samples next to water carry a surface just under the bank: dry. Wet means the bed is below it.
    (levels[index] != NO_WATER && levels[index] > ground.heights[index]).then(|| levels[index] as f64 / 100.0)
}

/// `points` at about `step` metres, ends included.
fn resample(points: &[P], step: f64) -> Vec<P> {
    let lengths: Vec<f64> = points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).collect();
    let total: f64 = lengths.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let count = (total / step).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(count + 1);
    let (mut segment, mut before) = (0usize, 0.0);
    for k in 0..=count {
        let s = total * k as f64 / count as f64;
        while segment + 1 < lengths.len() && before + lengths[segment] < s {
            before += lengths[segment];
            segment += 1;
        }
        let t = if lengths[segment] > 0.0 { ((s - before) / lengths[segment]).clamp(0.0, 1.0) } else { 0.0 };
        let (a, b) = (points[segment], points[segment + 1]);
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
    out
}

/// Collects triangles.
struct Mesh {
    pieces: Vec<Piece>,
}

impl Mesh {
    /// A hexahedron between ring `a` and ring `b` (four corners each, in order around), facing out.
    fn hexa(&mut self, at: P, steel: bool, a: [V3; 4], b: [V3; 4], cap_a: bool, cap_b: bool) {
        let corners: Vec<V3> = a.iter().chain(&b).copied().collect();
        let centre = scale(corners.iter().fold([0.0; 3], |sum, &c| add(sum, c)), 1.0 / 8.0);
        let mut tris = Vec::new();
        let mut quad = |q: [V3; 4]| {
            for tri in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                let middle = scale(add(add(tri[0], tri[1]), tri[2]), 1.0 / 3.0);
                let outwards = dot(client_normal(tri), sub(middle, centre)) >= 0.0;
                tris.push(if outwards { tri } else { [tri[0], tri[2], tri[1]] });
            }
        };
        if cap_a {
            quad(a);
        }
        if cap_b {
            quad(b);
        }
        for k in 0..4 {
            let l = (k + 1) % 4;
            quad([a[k], a[l], b[l], b[k]]);
        }
        self.pieces.push(Piece { at, steel, tris });
    }

    /// A box from `from` to `to` with the cross section spanned by unit axes `u` and `v`.
    fn prism(&mut self, at: P, steel: bool, from: V3, to: V3, u: V3, v: V3, hu: f64, hv: f64) {
        let ring = |c: V3| {
            [
                add(c, add(scale(u, hu), scale(v, hv))),
                add(c, add(scale(u, -hu), scale(v, hv))),
                add(c, add(scale(u, -hu), scale(v, -hv))),
                add(c, add(scale(u, hu), scale(v, -hv))),
            ]
        };
        self.hexa(at, steel, ring(from), ring(to), true, true);
    }

    /// A square beam of half-width `half` from `from` to `to`.
    fn beam(&mut self, steel: bool, from: V3, to: V3, half: f64) {
        let d = unit(sub(to, from));
        let reference = if d[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let u = unit(cross(d, reference));
        let v = unit(cross(d, u));
        let at = [(from[0] + to[0]) / 2.0, (from[2] + to[2]) / 2.0];
        self.prism(at, steel, from, to, u, v, half, half);
    }
}

/// Raises `y` so that no node is more than `MAX_GRADE` below a neighbour, in both directions.
fn limit_grade(y: &mut [f64], arcs: &[f64]) {
    for i in 1..y.len() {
        y[i] = y[i].max(y[i - 1] - MAX_GRADE * (arcs[i] - arcs[i - 1]));
    }
    for i in (0..y.len().saturating_sub(1)).rev() {
        y[i] = y[i].max(y[i + 1] - MAX_GRADE * (arcs[i + 1] - arcs[i]));
    }
}

/// How high the deck stands over water: the surface plus this, by road class.
fn clearance(kind: u8) -> f64 {
    match kind {
        ROAD_MAJOR => 7.0,
        ROAD_PATH => 3.5,
        _ => 5.5,
    }
}

/// Builds the bridge along `points` (metres, centre line) of road `width`. `levels` is the water
/// surface at every sample of `ground`, empty without water; `limit` is how far from the origin
/// the deck may reach.
pub fn build(points: &[P], width: f64, kind: u8, pylons: Pylons, ground: &Ground, levels: &[i16], limit: f64) -> Vec<Piece> {
    let samples = resample(points, STEP);
    if samples.len() < 2 {
        return Vec::new();
    }
    let node = |p: P, arc: f64| Node {
        p,
        y: 0.0,
        g: ground.height_at(p[0] as f32, p[1] as f32) as f64,
        level: water_at(ground, levels, p),
        arc,
    };
    let mut arc = 0.0;
    let mut nodes: Vec<Node> = Vec::with_capacity(samples.len());
    for (k, &p) in samples.iter().enumerate() {
        if k > 0 {
            arc += (p[0] - samples[k - 1][0]).hypot(p[1] - samples[k - 1][1]);
        }
        nodes.push(node(p, arc));
    }
    if arc < MIN_LENGTH {
        return Vec::new();
    }
    let lift = clearance(kind);
    let required: Vec<f64> = nodes.iter().map(|n| n.level.map_or(n.g + LIFT, |level| level + lift)).collect();
    let end_height = |n: &Node| n.level.unwrap_or(n.g);
    let (h0, h1) = (end_height(&nodes[0]), end_height(&nodes[nodes.len() - 1]));
    let arcs: Vec<f64> = nodes.iter().map(|n| n.arc).collect();
    let mut y: Vec<f64> = (0..nodes.len()).map(|i| (h0 + (h1 - h0) * arcs[i] / arc + LIFT).max(required[i])).collect();
    limit_grade(&mut y, &arcs);
    // Soften the corners the envelope leaves, never dipping under what is needed.
    for _ in 0..3 {
        let before = y.clone();
        for i in 0..y.len() {
            let (lo, hi) = (i.saturating_sub(2), (i + 2).min(y.len() - 1));
            y[i] = (before[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64).max(required[i]);
        }
    }
    for (n, y) in nodes.iter_mut().zip(&y) {
        n.y = *y;
    }

    // Ramps: an end still above the ground carries on in its direction, descending to the ground.
    let reach = |p: P| p[0].abs() <= limit && p[1].abs() <= limit;
    let ramp = |nodes: &[Node], from_end: bool| -> Vec<Node> {
        let (last, before) = if from_end { (&nodes[nodes.len() - 1], &nodes[nodes.len() - 2]) } else { (&nodes[0], &nodes[1]) };
        let (dx, dz) = (last.p[0] - before.p[0], last.p[1] - before.p[1]);
        let length = dx.hypot(dz).max(1e-9);
        let (dx, dz) = (dx / length, dz / length);
        let mut out = Vec::new();
        let (mut y, mut g) = (last.y, last.g);
        let mut distance = 0.0;
        while y - g > LIFT + 0.3 && distance < MAX_RAMP {
            distance += STEP;
            let p = [last.p[0] + dx * distance, last.p[1] + dz * distance];
            if !reach(p) {
                break;
            }
            let mut next = node(p, 0.0);
            if next.level.is_some() {
                break;
            }
            g = next.g;
            y = (y - RAMP_GRADE * STEP).max(g + LIFT);
            next.y = y;
            out.push(next);
        }
        out
    };
    let tail = ramp(&nodes, true);
    let head = ramp(&nodes, false);
    if !head.is_empty() {
        let shift = head.len() as f64 * STEP;
        for n in &mut nodes {
            n.arc += shift;
        }
        let mut joined: Vec<Node> = head.into_iter().rev().collect();
        for (k, n) in joined.iter_mut().enumerate() {
            n.arc = k as f64 * STEP;
        }
        joined.append(&mut nodes);
        nodes = joined;
    }
    let mut end = nodes.last().map_or(0.0, |n| n.arc);
    for mut n in tail {
        end += STEP;
        n.arc = end;
        nodes.push(n);
    }

    let hw = width / 2.0;
    let tangent = |i: usize| {
        let (a, b) = (nodes[i.saturating_sub(1)].p, nodes[(i + 1).min(nodes.len() - 1)].p);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz).max(1e-9);
        [dx / length, 0.0, dz / length]
    };
    let side = |t: V3| [-t[2], 0.0, t[0]];
    // Underside: solid down to the ground where the deck is close to it, else a slab.
    let bottom = |n: &Node| if n.y - n.g <= SOLID_BELOW + THICKNESS { n.g - 0.3 } else { n.y - THICKNESS };
    let section = |i: usize| {
        let n = &nodes[i];
        let t = tangent(i);
        let s = scale(side(t), hw);
        let at = |sign: f64, y: f64| [n.p[0] + sign * s[0], y, n.p[1] + sign * s[2]];
        let b = bottom(n);
        [at(1.0, n.y), at(-1.0, n.y), at(-1.0, b), at(1.0, b)]
    };
    let mut mesh = Mesh { pieces: Vec::new() };
    let last = nodes.len() - 1;
    let rings: Vec<[V3; 4]> = (0..nodes.len()).map(section).collect();
    for i in 0..last {
        let mid = [(nodes[i].p[0] + nodes[i + 1].p[0]) / 2.0, (nodes[i].p[1] + nodes[i + 1].p[1]) / 2.0];
        mesh.hexa(mid, false, rings[i], rings[i + 1], i == 0, i + 1 == last);
        // Parapets where there is a drop beside the deck.
        if nodes[i].y - nodes[i].g > 1.0 && nodes[i + 1].y - nodes[i + 1].g > 1.0 {
            for sign in [1.0, -1.0] {
                let edge = |k: usize| {
                    let t = tangent(k);
                    let s = scale(side(t), sign * (hw - 0.12));
                    [nodes[k].p[0] + s[0], nodes[k].y + PARAPET / 2.0, nodes[k].p[1] + s[2]]
                };
                let (a, b) = (edge(i), edge(i + 1));
                let across = unit(side(unit([b[0] - a[0], 0.0, b[2] - a[2]])));
                mesh.prism(mid, false, a, b, [0.0, 1.0, 0.0], across, PARAPET / 2.0, 0.12);
            }
        }
    }
    // Piers under the high stretches.
    let mut since = PIER_SPACING;
    for i in 1..last {
        since += nodes[i].arc - nodes[i - 1].arc;
        let n = &nodes[i];
        if since >= PIER_SPACING && n.y - n.g > SOLID_BELOW + THICKNESS {
            since = 0.0;
            let t = tangent(i);
            let top = [n.p[0], n.y - THICKNESS, n.p[1]];
            let foot = [n.p[0], n.g - 0.5, n.p[1]];
            mesh.prism(n.p, false, top, foot, t, side(t), 0.9, hw * 0.55);
        }
    }

    // Pylons with stay cables over the longest crossing of open water.
    if pylons != Pylons::None {
        let (mut best, mut run_start) = ((0usize, 0usize), None::<usize>);
        for i in 0..=nodes.len() {
            let wet = i < nodes.len() && nodes[i].level.is_some();
            match (wet, run_start) {
                (true, None) => run_start = Some(i),
                (false, Some(s)) => {
                    if i - s > best.1 - best.0 {
                        best = (s, i);
                    }
                    run_start = None;
                }
                _ => {}
            }
        }
        let (first, after) = best;
        let span = if after > first { nodes[after - 1].arc - nodes[first].arc } else { 0.0 };
        if span >= PYLON_MIN_SPAN {
            let towers: Vec<f64> = match pylons {
                Pylons::Ends => vec![nodes[first].arc + 4.0, nodes[after - 1].arc - 4.0],
                _ => vec![nodes[first].arc + span / 2.0],
            };
            let height = (span / 6.0).clamp(15.0, 40.0);
            let reach = (span / 4.0).min(90.0);
            let y_at = |s: f64| {
                let k = nodes.partition_point(|n| n.arc < s).clamp(1, nodes.len() - 1);
                let (a, b) = (&nodes[k - 1], &nodes[k]);
                let t = ((s - a.arc) / (b.arc - a.arc).max(1e-9)).clamp(0.0, 1.0);
                (a.y + (b.y - a.y) * t, [a.p[0] + (b.p[0] - a.p[0]) * t, a.p[1] + (b.p[1] - a.p[1]) * t])
            };
            for s in towers {
                let k = nodes.partition_point(|n| n.arc < s).clamp(1, nodes.len() - 1);
                let t = tangent(k);
                let n = side(t);
                let (deck_y, centre) = y_at(s);
                let mut tops = [[0.0; 3]; 2];
                for (leg, sign) in [1.0, -1.0].into_iter().enumerate() {
                    let foot_xz = [centre[0] + n[0] * sign * (hw + 1.2), centre[1] + n[2] * sign * (hw + 1.2)];
                    let foot_y = ground.height_at(foot_xz[0] as f32, foot_xz[1] as f32) as f64 - 0.5;
                    let top = [centre[0] + n[0] * sign * hw * 0.6, deck_y + height, centre[1] + n[2] * sign * hw * 0.6];
                    mesh.beam(true, [foot_xz[0], foot_y, foot_xz[1]], top, 0.9);
                    tops[leg] = top;
                }
                mesh.beam(true, tops[0], tops[1], 0.4);
                for (leg, sign) in [1.0, -1.0].into_iter().enumerate() {
                    let mut offset = CABLE_SPACING;
                    let mut index = 0.0;
                    while offset <= reach {
                        for direction in [1.0, -1.0] {
                            let sa = s + direction * offset;
                            if sa < nodes[0].arc || sa > nodes[nodes.len() - 1].arc {
                                continue;
                            }
                            let (y, p) = y_at(sa);
                            let anchor = [p[0] + n[0] * sign * hw, y, p[1] + n[2] * sign * hw];
                            let from = [tops[leg][0], tops[leg][1] - 1.5 * (index + 1.0), tops[leg][2]];
                            mesh.beam(true, from, anchor, 0.1);
                        }
                        offset += CABLE_SPACING;
                        index += 1.0;
                    }
                }
            }
        }
    }
    mesh.pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 600 m square of 10 m cells: banks at +3 m, a river 160 m wide along Z with its bed at -4 m
    /// and its surface at 0.
    fn river() -> (Ground, Vec<i16>) {
        let side = 61u32;
        let mut heights = Vec::new();
        let mut levels = Vec::new();
        for _j in 0..side {
            for i in 0..side {
                let x = -300.0 + i as f64 * 10.0;
                if x.abs() <= 80.0 {
                    heights.push(-400);
                    levels.push(0);
                } else {
                    heights.push(300);
                    levels.push(NO_WATER);
                }
            }
        }
        (Ground { origin: -30_000, cell: 1_000, side, heights }, levels)
    }

    fn vertices(pieces: &[Piece]) -> impl Iterator<Item = V3> + '_ {
        pieces.iter().flat_map(|p| p.tris.iter().flatten().copied())
    }

    #[test]
    fn a_deck_clears_the_water_and_meets_the_banks() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::Middle, &ground, &levels, 295.0);
        let top = vertices(&pieces).filter(|v| v[0].abs() < 40.0).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top >= 7.0, "mid-span deck top is {top}");
        let near_end = vertices(&pieces).filter(|v| v[0] > 140.0 && v[1] > 1.0).map(|v| v[1]).fold(f64::MAX, f64::min);
        assert!((near_end - 3.12).abs() < 0.5, "the deck meets the bank, got {near_end}");
        assert!(pieces.iter().any(|p| p.steel), "a 160 m crossing gets a pylon");
    }

    #[test]
    fn a_short_footbridge_gets_no_pylon_and_stays_near_the_ground() {
        let (ground, levels) = river();
        let pieces = build(&[[100.0, 0.0], [160.0, 0.0]], 2.5, ROAD_PATH, Pylons::None, &ground, &levels, 295.0);
        assert!(!pieces.is_empty());
        assert!(pieces.iter().all(|p| !p.steel));
        assert!(vertices(&pieces).all(|v| v[1] < 4.0));
    }

    #[test]
    fn faces_point_out_of_the_solid() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::Middle, &ground, &levels, 295.0);
        // The deck's flat top: every triangle at the highest level of a section faces up.
        let flat_tops = pieces.iter().filter(|p| !p.steel).flat_map(|p| &p.tris).filter(|t| t.iter().all(|v| (v[1] - t[0][1]).abs() < 1e-9) && t[0][1] > 6.0);
        let mut count = 0;
        for tri in flat_tops {
            let n = client_normal(*tri);
            // Tops face up; bottoms (also flat) face down: both exist, so only check consistency
            // with the winding being non-degenerate.
            assert!(n[1].abs() > 0.0);
            count += 1;
        }
        assert!(count > 0);
        let highest = pieces.iter().filter(|p| !p.steel).flat_map(|p| &p.tris).filter(|t| t.iter().all(|v| (v[1] - t[0][1]).abs() < 1e-9)).map(|t| t[0][1]).fold(f64::MIN, f64::max);
        let top = pieces.iter().flat_map(|p| &p.tris).find(|t| t.iter().all(|v| (v[1] - highest).abs() < 1e-9) && t[0][0].abs() < 40.0).expect("a top triangle");
        assert!(client_normal(*top)[1] > 0.0, "the top of the deck faces up");
    }
}
