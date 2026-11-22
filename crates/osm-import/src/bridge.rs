//! Bridges: a deck, piers and, over wide water, a pylon with stay cables, built as triangles for
//! the tile's collision mesh (the client draws the same triangles, `Tile::bridges`).
//!
//! OSM only says that a way is a bridge and on which layer; there are no heights. The deck's
//! height is inferred: it follows the ground between its two ends, is lifted to clear the water
//! surface where it crosses water (the river bed is carved deep, so a road draped on the ground
//! would lie under the water), and where it ends above ground it carries on as a ramp down to the
//! ground. A small river's channel (`water::Placed::channel`) is no water to sail under: a deck over
//! one runs from bank to bank, just clear of it.

use shared::terrain::Ground;
use shared::world::{BRIDGE_CONCRETE, DECK_ASPHALT, DECK_BALLAST, DECK_PLANKS, NO_WATER, ROAD_FAINT, ROAD_MAJOR, ROAD_MINOR, ROAD_PATH, ROAD_PAVED_PATH, ROAD_TRACK};

type P = [f64; 2];
type V3 = [f64; 3];

/// Distance between deck cross sections, metres.
const STEP: f64 = 3.0;
/// Shorter spans (culverts tagged as bridges) are left to the terrain, unless the ground under them
/// dips more than `SHORT_DIP` metres below the line between their ends.
const MIN_LENGTH: f64 = 12.0;
const SHORT_DIP: f64 = 1.0;
/// Steepest slope of a deck, rise over run, and of the ramp that brings a deck still above ground at
/// its end down to the ground.
pub(crate) const MAX_GRADE: f64 = 0.06;
/// The same for a railway's deck, which a train has to climb.
pub(crate) const MAX_GRADE_RAIL: f64 = 0.025;
/// Distance between the cross sections of a railway's deck over a cutting, metres. Closer than
/// `STEP`: the passage under it is a few metres wide, and its underside must not slope down into it
/// from the solid ends.
const STEP_CUTTING_RAIL: f64 = 1.0;
/// The longest such ramp, metres.
const MAX_RAMP: f64 = 150.0;
/// How deep a deck's slab is, from its roadway to its underside, metres.
pub(crate) const THICKNESS: f64 = 0.7;
/// The deck's top is this far above the ground where it meets it.
const LIFT: f64 = 0.12;
/// A roadway less than this over the ground follows the ground's slope across it, metres.
const FOLLOWS: f64 = 0.5;
/// How far over the ground the last edge of a deck or a ramp that ends on it lies, metres.
const LANDING: f64 = 0.02;
/// Deck sections this close to the ground are solid down to it; higher ones stand on piers.
const SOLID_BELOW: f64 = 2.0;
const PIER_SPACING: f64 = 24.0;
/// A deck wider than this stands on two columns, one under each half, instead of one pier wall
/// across it, metres.
const WIDE_DECK: f64 = 14.0;
/// The concrete beam along each edge of a deck with a drop beside it, which the railing stands on:
/// how wide it is and how far it rises above the roadway, metres. The deck is this much wider each
/// side than its road, so the roadway between the beams keeps the road's width.
pub const EDGE_BEAM_WIDTH: f64 = 0.5;
pub const EDGE_BEAM_HEIGHT: f64 = 0.25;
/// The railing on the edge beam: steel posts at every cross section and two rails, the top one at
/// `RAILING_HEIGHT` and the middle one at `MID_RAIL_HEIGHT` above the roadway, metres.
pub const RAILING_HEIGHT: f64 = 1.1;
pub const MID_RAIL_HEIGHT: f64 = 0.6;
/// Half the side of a railing post's square section, and half a rail's, metres.
const POST_HALF: f64 = 0.05;
const RAIL_HALF: f64 = 0.04;
/// How far in from the deck's outer edge the posts and rails stand, metres: on the outer half of
/// the edge beam.
const RAILING_INSET: f64 = 0.15;
/// Clear height over a road that passes under a deck, from the road to the deck's underside, metres:
/// a bus's roof is 3.36 m up, and this clears it with room to spare even after the underside is
/// rounded down to the 0.72 m cells it is built of (`carve::CELL`). An underpass is dug to the same
/// (`condition`).
pub(crate) const HEADROOM_ROAD: f64 = 4.7;
/// Clear height over a track that passes under a deck, from the ground to the deck's underside,
/// metres: a train's, with room for the contact wire over it.
pub(crate) const HEADROOM_RAIL: f64 = 6.0;
/// How high a deck stands over a small river's channel, its roadway over the surface, metres: its
/// slab and a hand's breadth or two of air. A big river's is by the road's class (`clearance`).
const CHANNEL_CLEARANCE: f64 = THICKNESS + 0.6;
/// A deck's line is looked along this often for a channel under it, metres: one may be no wider
/// than 4 m.
const CHANNEL_LOOK: f64 = 1.0;
/// The least a track leaves over a road under it however low the road's `maxheight` is, metres:
/// a bus still gets under.
pub(crate) const HEADROOM_LOWEST: f64 = 3.6;
/// How far from a crossing of two lines a point they share may be for it to be that crossing, metres.
const SAME_CROSSING: f64 = 1.0;
/// How far past the edge of a road under a deck the headroom is kept, metres.
const UNDER_MARGIN: f64 = 1.0;
/// A ramp that has come down to within this of a road it crosses meets that road at grade, metres:
/// it is a crossroads on the ground, which OpenStreetMap has past the bridge's end. Any higher and the
/// ramp would stand in the road's way as a wall, so it rises to let a bus under instead.
pub(crate) const AT_GRADE: f64 = 0.3;
/// A way whose line runs within this cosine of a bridge's own direction runs alongside it, not
/// under it.
pub(crate) const PARALLEL: f64 = 0.9;
/// Two carriageways' ramps from a shared deck have no railing between them while their edges are
/// closer than this, metres: they come down side by side at one height, and a railing there would
/// stand in a slot too narrow to fall into.
const STRAND_GAP: f64 = 2.0;
/// A pylon is only worth building over a crossing at least this long, metres.
const PYLON_MIN_SPAN: f64 = 60.0;
const CABLE_SPACING: f64 = 12.0;

/// Where a bridge's pylons stand. OpenStreetMap does not say which bridges are cable-stayed (there
/// is no structure tag here, and the lidar canopy is no better), so the few that are in this map
/// are listed in `LANDMARKS`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pylons {
    None,
    /// No pylons, and no railings either: a deck over a cutting, flush with the road either side of
    /// it and with whatever deck lies beside it.
    Flush,
    /// No deck of its own at all: one carriageway of a divided road, carried on the deck it shares
    /// with the other (`assemble::twin_decks`).
    Carried,
    /// One tower at each end of the crossing, cables running in over the water.
    Ends,
    /// One tower in the middle of the crossing.
    Middle,
}

/// Cable-stayed bridges as (latitude, longitude, radius in metres, pylons): a bridge way whose
/// midpoint lies within the radius is built with these pylons. Lundabron has its towers at the two
/// ends; Kolbäcksbron has one in the middle, over each of its two carriageways.
pub const LANDMARKS: [(f64, f64, f64, Pylons); 2] = [(63.8263, 20.2204, 150.0, Pylons::Ends), (63.8013, 20.2994, 150.0, Pylons::Middle)];

/// The kind a railway's deck is built with, in place of a `ROAD_*`: a minor road's clearance over
/// water, a train's grade (`MAX_GRADE_RAIL`), and ballast on top (`deck_paint`) with the track laid
/// on it (`railway::lay`).
pub const TRACK: u8 = u8::MAX;

/// The steepest a deck of `kind` (`ROAD_*`, or `TRACK`) and its ramps run.
fn max_grade(kind: u8) -> f64 {
    if kind == TRACK { MAX_GRADE_RAIL } else { MAX_GRADE }
}

/// The clear height a track leaves over a road with the signed height limit `limit`, metres: a
/// bridge's over a road, or less where the road is signed lower, down to `HEADROOM_LOWEST`.
pub(crate) fn headroom_under(limit: Option<f64>) -> f64 {
    limit.map_or(HEADROOM_ROAD, |limit| limit.clamp(HEADROOM_LOWEST, HEADROOM_ROAD))
}

/// A road or a track on the ground that may pass under a deck, which rises to clear it.
#[derive(Clone, Copy)]
pub struct Crossed<'a> {
    pub line: &'a [P],
    /// Metres: a road's, or a track's ballast.
    pub width: f64,
    /// Clear height wanted over it, metres.
    pub headroom: f64,
    pub track: bool,
    /// Where it is crossed on the level, at points it shares with the way over it: no deck or ramp
    /// rises over it there.
    pub level: &'a [P],
}

impl<'a> Crossed<'a> {
    /// A road, with a bridge's headroom over it.
    pub fn road(line: &'a [P], width: f64) -> Self {
        Crossed { line, width, headroom: HEADROOM_ROAD, track: false, level: &[] }
    }
}

/// The colour of the top of a deck that carries a road of `kind` (`ROAD_*`, or `TRACK`): asphalt
/// for a paved road or path, timber planks for a footpath or a track, ballast for a railway, else
/// bare concrete. The client picks the surface's pattern from it (`buildingSurfaces`).
pub fn deck_paint(kind: u8) -> u32 {
    match kind {
        TRACK => DECK_BALLAST,
        ROAD_MAJOR | ROAD_MINOR | ROAD_PAVED_PATH => DECK_ASPHALT,
        ROAD_PATH | ROAD_TRACK | ROAD_FAINT => DECK_PLANKS,
        _ => BRIDGE_CONCRETE,
    }
}

/// A piece of the bridge, assigned to the tile of `at` so a long bridge is cut at tile borders.
pub struct Piece {
    pub at: P,
    /// Steel (pylon, cables) rather than concrete (deck, piers, railings).
    pub steel: bool,
    /// A stretch of the deck itself: its top is the roadway, drawn in the road's surface
    /// (`deck_paint`); everything else is concrete.
    pub deck: bool,
    /// A stretch of edge beam, or a railing's post or rail, along the deck's edge, which is left out
    /// where another bridge's deck lies beside this one (`assemble`): the carriageways of a divided
    /// road are one roadway.
    pub railing: bool,
    /// A pier under the deck, which is left out where it would stand on a road or a track that goes
    /// under the bridge (`assemble`): OSM does not say where a bridge's piers are.
    pub pier: bool,
    /// How far along the bridge it stands, metres from the start of its ramp (`sections` cuts there).
    pub arc: f64,
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
    /// Whether it is over a small river's channel, or a section's length from it: the deck is an open
    /// span there, so its solid ends do not close the river.
    over_channel: bool,
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
pub(crate) fn water_at(ground: &Ground, levels: &[i32], p: P) -> Option<f64> {
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
pub(crate) fn resample(points: &[P], step: f64) -> Vec<P> {
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
    /// How far along the bridge the pieces being made stand, metres (`Piece::arc`).
    arc: f64,
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
        self.pieces.push(Piece { at, steel, deck: false, railing: false, pier: false, arc: self.arc, tris });
    }

    /// A box from `from` to `to` with the cross section spanned by unit axes `u` and `v`.
    #[allow(clippy::too_many_arguments)]
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

    /// Marks the piece just made as part of the railing (`Piece::railing`).
    fn mark_railing(&mut self) {
        if let Some(piece) = self.pieces.last_mut() {
            piece.railing = true;
        }
    }
}

/// The point `arc` metres along `line`, clamped to its ends.
fn point_along(line: &[P], arc: f64) -> P {
    let mut run = 0.0;
    for pair in line.windows(2) {
        let length = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
        if run + length >= arc && length > 0.0 {
            let t = ((arc - run) / length).clamp(0.0, 1.0);
            return [pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t];
        }
        run += length;
    }
    line[line.len() - 1]
}

/// Where segments `a → b` and `c → d` cross, as how far along each (0..1), or `None`.
fn crossing(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let denominator = r[0] * s[1] - r[1] * s[0];
    if denominator.abs() < 1e-12 {
        return None;
    }
    let (qx, qz) = (c[0] - a[0], c[1] - a[1]);
    let t = (qx * s[1] - qz * s[0]) / denominator;
    let u = (qx * r[1] - qz * r[0]) / denominator;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// A road or a track passing under a deck: where it crosses, metres along the deck's nodes, how far
/// either side of that the deck stands over it, metres, how high its surface is, and the lowest the
/// deck's top may be there to keep its headroom under the slab.
struct Under {
    middle: f64,
    reach: f64,
    surface: f64,
    top: f64,
}

/// Where the roads and tracks `under` pass under the line of `nodes`, a deck of `kind`. A road that runs alongside the deck (the bridge's own road going on along the ground) does
/// not pass under, nor does one that ends at an end of the deck (a road joining the bridge there).
/// One that ends anywhere else under the deck does: OpenStreetMap often cuts a road in two where a
/// bridge crosses it. Under a ramp (`nodes` past `deck`, the arcs of the deck's own ends) a road
/// that ends there joins the road the ramp lies over. A track never joins a road's bridge, nor a road a
/// railway's, so one that ends under the other's deck always passes under, and neither is in the
/// way where they cross on the level (`Crossed::level`). `hw` is half the deck's width, edge beams included.
fn roads_under(nodes: &[Node], kind: u8, hw: f64, deck: (f64, f64), under: &[Crossed], ground: &Ground) -> Vec<Under> {
    let mut found = Vec::new();
    let height = |p: P| f64::from(ground.height_at(p[0] as f32, p[1] as f32));
    // Only the roads that come near the nodes at all.
    let grow = hw + UNDER_MARGIN;
    let (lo, hi) = nodes.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), n| ([lo[0].min(n.p[0] - grow), lo[1].min(n.p[1] - grow)], [hi[0].max(n.p[0] + grow), hi[1].max(n.p[1] + grow)]));
    for &Crossed { line, width: across, headroom, track, level } in under {
        let total: f64 = line.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum();
        let mut run = 0.0;
        for pair in line.windows(2) {
            let (c, d) = (pair[0], pair[1]);
            let length = (d[0] - c[0]).hypot(d[1] - c[1]);
            let apart = c[0].max(d[0]) < lo[0] || c[0].min(d[0]) > hi[0] || c[1].max(d[1]) < lo[1] || c[1].min(d[1]) > hi[1];
            if length <= 0.0 || apart {
                run += length;
                continue;
            }
            let along_road = [(d[0] - c[0]) / length, (d[1] - c[1]) / length];
            for i in 0..nodes.len().saturating_sub(1) {
                let (a, b) = (nodes[i].p, nodes[i + 1].p);
                let Some((t, u)) = crossing(a, b, c, d) else { continue };
                let span = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
                let cos = ((b[0] - a[0]) * along_road[0] + (b[1] - a[1]) * along_road[1]).abs() / span;
                let at = run + u * length;
                let middle = nodes[i].arc + t * (nodes[i + 1].arc - nodes[i].arc);
                let near = hw + across / 2.0 + UNDER_MARGIN;
                let joins = track == (kind == TRACK) && (at < near || total - at < near) && (middle < deck.0 + near || middle > deck.1 - near);
                let crossing = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                let on_the_level = level.iter().any(|&p| (p[0] - crossing[0]).hypot(p[1] - crossing[1]) < SAME_CROSSING);
                if cos > PARALLEL || joins || on_the_level {
                    continue;
                }
                let sin = (1.0 - cos * cos).sqrt();
                // The road's surface: its highest ground across the deck's strip, every metre.
                let strip = hw / sin + UNDER_MARGIN;
                let steps = (2.0 * strip).ceil() as usize;
                let surface = (0..=steps).map(|k| height(point_along(line, at - strip + 2.0 * strip * k as f64 / steps as f64))).fold(f64::MIN, f64::max);
                let top = surface + headroom + THICKNESS;
                // The stretch of deck over the road's width, and a cross section past it either side,
                // so the deck between two sections is not sloping down over the road.
                let reach = (across / 2.0 + UNDER_MARGIN) / sin + hw * cos / sin + STEP;
                found.push(Under { middle, reach, surface, top });
            }
            run += length;
        }
    }
    found
}

/// The lowest the deck's top may be at each node (its distance along, `arcs`) for the roads `under`
/// it, or minus infinity where none passes under.
fn need_over(arcs: &[f64], under: &[&Under]) -> Vec<f64> {
    arcs.iter().map(|&arc| under.iter().filter(|u| (arc - u.middle).abs() <= u.reach).map(|u| u.top).fold(f64::NEG_INFINITY, f64::max)).collect()
}

/// Raises the deck's heights `y` where it would sag between two crests, to the lower of the two: a
/// bridge over two roads rises over the first and runs level to the second, rather than dipping
/// between them.
fn fill_sags(y: &mut [f64]) {
    let mut before = vec![f64::MIN; y.len()];
    let mut after = vec![f64::MIN; y.len()];
    for i in 0..y.len() {
        before[i] = if i > 0 { before[i - 1].max(y[i]) } else { y[i] };
    }
    for i in (0..y.len()).rev() {
        after[i] = if i + 1 < y.len() { after[i + 1].max(y[i]) } else { y[i] };
    }
    for i in 0..y.len() {
        y[i] = y[i].max(before[i].min(after[i]));
    }
}

/// Raises `y` so that no node falls more steeply than `grade` from a neighbour, in both directions.
fn limit_grade(y: &mut [f64], arcs: &[f64], grade: f64) {
    for i in 1..y.len() {
        y[i] = y[i].max(y[i - 1] - grade * (arcs[i] - arcs[i - 1]));
    }
    for i in (0..y.len().saturating_sub(1)).rev() {
        y[i] = y[i].max(y[i + 1] - grade * (arcs[i + 1] - arcs[i]));
    }
}

/// Deck returns within this distance of a cross section are its height, metres.
const MEASURE_REACH: f64 = 2.0;
/// Fewest returns that measure a cross section, and the share of sections that must be measured
/// for the measurement to replace the inferred deck.
const MEASURE_POINTS: usize = 3;
const MEASURE_SHARE: f64 = 0.5;

/// The deck's height at each node as measured: the median of the returns near it, and gaps between
/// measured nodes filled linearly. Before the first and after the last measured node (the way
/// carries on over an embankment the laser calls ground) the deck descends at `grade`, never
/// below the ground. `None` when too few nodes are measured.
fn measured_profile(nodes: &[Node], returns: &[[f64; 3]], grade: f64) -> Option<Vec<f64>> {
    if returns.is_empty() {
        return None;
    }
    let mut found: Vec<Option<f64>> = nodes
        .iter()
        .map(|n| {
            let mut ys: Vec<f64> = returns.iter().filter(|r| (r[0] - n.p[0]).hypot(r[1] - n.p[1]) <= MEASURE_REACH).map(|r| r[2]).collect();
            if ys.len() < MEASURE_POINTS {
                return None;
            }
            ys.sort_by(f64::total_cmp);
            Some(ys[ys.len() / 2])
        })
        .collect();
    let known: Vec<usize> = (0..found.len()).filter(|&i| found[i].is_some()).collect();
    if (known.len() as f64) < MEASURE_SHARE * nodes.len() as f64 {
        return None;
    }
    for pair in known.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (ya, yb) = (found[a]?, found[b]?);
        for i in a + 1..b {
            let t = (nodes[i].arc - nodes[a].arc) / (nodes[b].arc - nodes[a].arc).max(1e-9);
            found[i] = Some(ya + (yb - ya) * t);
        }
    }
    let (first, last) = (*known.first()?, *known.last()?);
    let towards = |edge: usize, i: usize| (found[edge].unwrap_or(0.0) - grade * (nodes[i].arc - nodes[edge].arc).abs()).max(nodes[i].g + LIFT);
    Some((0..nodes.len()).map(|i| if i < first { towards(first, i) } else if i > last { towards(last, i) } else { found[i].unwrap_or(nodes[i].g + LIFT) }).collect())
}

/// Whether `p` is in a small river's channel or on the bank cut down to it (`channel`,
/// `water::Placed::channel`: one entry per sample of `ground`, empty without channels).
pub(crate) fn in_channel(ground: &Ground, channel: &[bool], p: P) -> bool {
    if channel.len() != ground.heights.len() || ground.is_flat() {
        return false;
    }
    let at = |v: f64| ((v * 100.0 - f64::from(ground.origin)) / f64::from(ground.cell)).round();
    let (i, j) = (at(p[0]), at(p[1]));
    let last = f64::from(ground.side) - 1.0;
    i >= 0.0 && j >= 0.0 && i <= last && j <= last && channel[j as usize * ground.side as usize + i as usize]
}

/// Where the line of `points` is over the water of a small river's channel.
fn channel_crossed(points: &[P], ground: &Ground, levels: &[i32], channel: &[bool]) -> Vec<P> {
    if channel.is_empty() {
        return Vec::new();
    }
    resample(points, CHANNEL_LOOK).into_iter().filter(|&p| in_channel(ground, channel, p) && water_at(ground, levels, p).is_some()).collect()
}

/// Whether a span through `samples` is short and has no drop under it. Over level ground it is a
/// culvert; one over a cutting (a path under a road, which the terrain has as a trench across it)
/// needs its deck, or the road has a ditch in it.
fn short_and_even(samples: &[P], ground: &Ground) -> bool {
    let mut arc = 0.0;
    let mut along: Vec<(f64, f64)> = Vec::with_capacity(samples.len());
    for (k, &p) in samples.iter().enumerate() {
        if k > 0 {
            arc += (p[0] - samples[k - 1][0]).hypot(p[1] - samples[k - 1][1]);
        }
        along.push((arc, f64::from(ground.height_at(p[0] as f32, p[1] as f32))));
    }
    let (Some(&(_, from)), Some(&(_, to))) = (along.first(), along.last()) else { return true };
    let dip = along.iter().map(|&(at, g)| from + (to - from) * at / arc.max(1e-9) - g).fold(0.0, f64::max);
    arc < MIN_LENGTH && dip < SHORT_DIP
}

/// Whether a bridge along `points` gets no deck, the terrain carrying it: it is short, the ground
/// under it is even and it crosses no channel.
pub(crate) fn left_to_terrain(points: &[P], ground: &Ground, levels: &[i32], channel: &[bool]) -> bool {
    short_and_even(&resample(points, STEP), ground) && channel_crossed(points, ground, levels, channel).is_empty()
}

/// How high the deck stands over water: the surface plus this, by road class.
fn clearance(kind: u8) -> f64 {
    match kind {
        ROAD_MAJOR => 7.0,
        ROAD_PATH | ROAD_PAVED_PATH => 3.5,
        _ => 5.5,
    }
}

/// The height of the roadway of a deck over a cutting (`Pylons::Flush`, along `points`, `width` wide)
/// at `p`, metres, or `None` where `p` is not over the deck. Such a deck runs straight from the ground
/// at one end to the ground at the other (`build`), so what stands on its road, a guard rail down the
/// middle, stands on it rather than down in the cutting.
pub(crate) fn flush_top(points: &[P], width: f64, ground: &Ground, p: P) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let height = |q: P| f64::from(ground.height_at(q[0] as f32, q[1] as f32));
    let (h0, h1) = (height(points[0]), height(points[points.len() - 1]));
    let total: f64 = points.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum();
    let mut run = 0.0;
    // The nearest point of the deck's line: how far, how far along, and whether `p` is past an end.
    let mut best: Option<(f64, f64, bool)> = None;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz);
        if length > 0.0 {
            let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / (length * length);
            let d = (p[0] - a[0] - t.clamp(0.0, 1.0) * dx).hypot(p[1] - a[1] - t.clamp(0.0, 1.0) * dz);
            let beyond = (t < 0.0 && run == 0.0) || (t > 1.0 && run + length >= total - 1e-9);
            if best.is_none_or(|(known, ..)| d < known) {
                best = Some((d, run + t.clamp(0.0, 1.0) * length, beyond));
            }
        }
        run += length;
    }
    // Past either end of the deck is the road on the ground again.
    let (d, along, beyond) = best?;
    if beyond || d > width / 2.0 + EDGE_BEAM_WIDTH {
        return None;
    }
    Some((h0 + (h1 - h0) * along / total.max(1e-9) + LIFT).max(height(p) + LIFT))
}

/// Lays the deck along `nodes`, `hw` wide each side of them: a slab, solid down to the ground where
/// it is close to it, and with `railings` an edge beam and a railing where there is a drop beside it.
/// Low down a road's deck (`leans`) follows the ground across; a railway's is level, as its bed is.
fn lay_deck(mesh: &mut Mesh, nodes: &[Node], hw: f64, railings: bool, leans: bool, ground: &Ground) {
    let tangent = |i: usize| {
        let (a, b) = (nodes[i.saturating_sub(1)].p, nodes[(i + 1).min(nodes.len() - 1)].p);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz).max(1e-9);
        [dx / length, 0.0, dz / length]
    };
    let side = |t: V3| [-t[2], 0.0, t[0]];
    // Underside: solid down to the ground where the deck is close to it, else a slab. Always a slab
    // over a river's channel, or it would dam it.
    let bottom = |n: &Node| if !n.over_channel && n.y - n.g <= SOLID_BELOW + THICKNESS { n.g - 0.3 } else { n.y - THICKNESS };
    let last = nodes.len() - 1;
    let section = |i: usize| {
        let n = &nodes[i];
        let t = tangent(i);
        let s = scale(side(t), hw);
        let at = |sign: f64, y: f64| [n.p[0] + sign * s[0], y, n.p[1] + sign * s[2]];
        let b = bottom(n);
        // Close to the ground the roadway follows it across, as the road it meets does, each edge as
        // far over the ground under it as the middle is. Where the deck ends on the ground, its last
        // edge lies on it: no step up onto the bridge, on the crown of the road or at its edges.
        let landed = (i == 0 || i == last) && n.y - n.g <= LIFT + 0.06;
        let top = |sign: f64| {
            if n.y - n.g >= FOLLOWS {
                return n.y;
            }
            if !leans {
                return if landed { n.g + LANDING } else { n.y };
            }
            let lean = (f64::from(ground.height_at((n.p[0] + sign * s[0]) as f32, (n.p[1] + sign * s[2]) as f32)) - n.g).clamp(-FOLLOWS, FOLLOWS);
            if landed { n.g + lean + LANDING } else { n.y + lean }
        };
        [at(1.0, top(1.0)), at(-1.0, top(-1.0)), at(-1.0, b), at(1.0, b)]
    };
    let rings: Vec<[V3; 4]> = (0..nodes.len()).map(section).collect();
    for i in 0..last {
        let mid = [(nodes[i].p[0] + nodes[i + 1].p[0]) / 2.0, (nodes[i].p[1] + nodes[i + 1].p[1]) / 2.0];
        mesh.arc = (nodes[i].arc + nodes[i + 1].arc) / 2.0;
        // Capped at both ends, so a section cut from the bridge (`sections`) is a closed solid: an open
        // end would be the edge of a hole to the client, drawn as a broken rim in the air once all
        // round it is gone. The caps between two stretches are inside the concrete.
        mesh.hexa(mid, false, rings[i], rings[i + 1], true, true);
        if let Some(piece) = mesh.pieces.last_mut() {
            piece.deck = true;
        }
        // A railing on an edge beam where there is a drop beside the deck.
        if railings && nodes[i].y - nodes[i].g > 1.0 && nodes[i + 1].y - nodes[i + 1].g > 1.0 {
            for sign in [1.0, -1.0] {
                // A point `inward` metres in from the deck's edge on this side, `up` above node `k`'s roadway.
                let edge = |k: usize, inward: f64, up: f64| {
                    let s = scale(side(tangent(k)), sign * (hw - inward));
                    [nodes[k].p[0] + s[0], nodes[k].y + up, nodes[k].p[1] + s[2]]
                };
                let flat = |a: V3, b: V3| unit(side(unit([b[0] - a[0], 0.0, b[2] - a[2]])));
                // Where a stretch of railing stands: its own middle, not the deck's.
                let middle = |a: V3, b: V3| [(a[0] + b[0]) / 2.0, (a[2] + b[2]) / 2.0];
                let (a, b) = (edge(i, EDGE_BEAM_WIDTH / 2.0, EDGE_BEAM_HEIGHT / 2.0), edge(i + 1, EDGE_BEAM_WIDTH / 2.0, EDGE_BEAM_HEIGHT / 2.0));
                mesh.prism(middle(a, b), false, a, b, [0.0, 1.0, 0.0], flat(a, b), EDGE_BEAM_HEIGHT / 2.0, EDGE_BEAM_WIDTH / 2.0);
                mesh.mark_railing();
                for height in [MID_RAIL_HEIGHT, RAILING_HEIGHT - RAIL_HALF] {
                    let (a, b) = (edge(i, RAILING_INSET, height), edge(i + 1, RAILING_INSET, height));
                    mesh.prism(middle(a, b), false, a, b, [0.0, 1.0, 0.0], flat(a, b), RAIL_HALF, RAIL_HALF);
                    mesh.mark_railing();
                }
                // A post at this cross section, and at the last one where the railing ends.
                let ends = if i + 1 == last || nodes[i + 2].y - nodes[i + 2].g <= 1.0 { vec![i, i + 1] } else { vec![i] };
                for k in ends {
                    let foot = edge(k, RAILING_INSET, EDGE_BEAM_HEIGHT);
                    let t = tangent(k);
                    mesh.prism([foot[0], foot[2]], false, foot, edge(k, RAILING_INSET, RAILING_HEIGHT), t, side(t), POST_HALF, POST_HALF);
                    mesh.mark_railing();
                }
            }
        }
    }
}

/// The height of the roadway of the deck or ramp among `pieces` (of a bridge whose deck is `hw` to
/// each side of its line) at `p`, metres, or `None` where `p` is not on it.
pub fn top_at(pieces: &[Piece], hw: f64, p: P) -> Option<f64> {
    let piece = pieces.iter().filter(|piece| piece.deck).min_by(|a, b| (a.at[0] - p[0]).hypot(a.at[1] - p[1]).total_cmp(&(b.at[0] - p[0]).hypot(b.at[1] - p[1])))?;
    if (piece.at[0] - p[0]).hypot(piece.at[1] - p[1]) > hw + STEP {
        return None;
    }
    let (low, high) = piece.tris.iter().flatten().fold((f64::MAX, f64::MIN), |(low, high), v| (low.min(v[1]), high.max(v[1])));
    let top = piece.tris.iter().flatten().map(|v| v[1]).filter(|&y| y > (low + high) / 2.0).fold(f64::MAX, f64::min);
    Some((top + high) / 2.0)
}

/// The steepest a path's ramp runs down from a deck it leaves, metres per metre: steps or a steep
/// walk, which is what leads off a bridge to the bank under it, where a road's grade would carry the
/// path far out over the ground.
pub(crate) const PATH_GRADE: f64 = 0.25;

/// The height of the top of the deck among the triangles `roof` at `p` exactly, metres, the highest
/// where they overlap, or `None` where `p` is on none of them. `top_at` is a piece's middle height,
/// which is near enough to tell decks apart and too coarse to lay a track on.
pub fn roadway_at(roof: &[[V3; 3]], p: P) -> Option<f64> {
    // A hair of slack, so a point on the edge between two triangles is on one of them.
    const EDGE: f64 = -1e-7;
    let mut top: Option<f64> = None;
    for &[a, b, c] in roof {
        let denominator = (b[2] - c[2]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[2] - c[2]);
        if denominator.abs() < 1e-12 {
            continue;
        }
        let u = ((b[2] - c[2]) * (p[0] - c[0]) + (c[0] - b[0]) * (p[1] - c[2])) / denominator;
        let v = ((c[2] - a[2]) * (p[0] - c[0]) + (a[0] - c[0]) * (p[1] - c[2])) / denominator;
        let w = 1.0 - u - v;
        if u >= EDGE && v >= EDGE && w >= EDGE {
            let y = u * a[1] + v * b[1] + w * c[1];
            top = Some(top.map_or(y, |known| known.max(y)));
        }
    }
    top
}

/// The middle line of the deck and ramps among `pieces`, in order along the bridge.
pub fn line_of(pieces: &[Piece]) -> Vec<P> {
    pieces.iter().filter(|piece| piece.deck).map(|piece| piece.at).collect()
}

/// A ramp along `line` (a road that joins a deck or a ramp standing above the ground, from where it
/// joins) of road `width`, from `from` metres up at its start down to the ground at `grade` (metres
/// down per metre along): without it the road would end at the side of the deck, below it. It stops
/// at the end of `line`, at water, or `limit` from the origin.
pub fn slip(line: &[P], width: f64, from: f64, grade: f64, ground: &Ground, levels: &[i32], limit: f64) -> Vec<Piece> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut arc = 0.0;
    for p in resample(line, STEP) {
        if let Some(before) = nodes.last() {
            arc += (p[0] - before.p[0]).hypot(p[1] - before.p[1]);
        }
        let g = f64::from(ground.height_at(p[0] as f32, p[1] as f32));
        if p[0].abs() > limit || p[1].abs() > limit || water_at(ground, levels, p).is_some() {
            break;
        }
        let y = (from - grade * arc).max(g + LIFT);
        nodes.push(Node { p, y, g, level: None, over_channel: false, arc });
        if y - g <= LIFT + 0.05 {
            break;
        }
    }
    let mut mesh = Mesh { pieces: Vec::new(), arc: 0.0 };
    if nodes.len() >= 2 {
        lay_deck(&mut mesh, &nodes, width / 2.0 + EDGE_BEAM_WIDTH, true, true, ground);
    }
    mesh.pieces
}

/// Builds the bridge along `points` (metres, centre line) of road `width`. `measured` holds deck
/// returns `[x, z, y]` from a classified cloud, empty without one; `under` are the drivable roads on
/// the ground (centre lines and widths) that may pass under it, which it rises to clear
/// (`HEADROOM_ROAD`); `levels` is the water surface at every sample of `ground`, empty without water;
/// `limit` is how far from the origin the deck may reach.
#[allow(clippy::too_many_arguments)]
pub fn build(points: &[P], width: f64, kind: u8, pylons: Pylons, measured: &[[f64; 3]], under: &[(&[P], f64)], ground: &Ground, levels: &[i32], limit: f64) -> Vec<Piece> {
    let under = &under.iter().map(|&(line, width)| Crossed::road(line, width)).collect::<Vec<_>>();
    build_along(points, [&[], &[]], [None, None], &|_| None, width, kind, pylons, measured, under, ground, levels, &[], limit).0
}

/// The line midway between two lines that set out side by side: each walked from its start in
/// equal steps, as far as the shorter goes.
fn between(a: &[P], b: &[P]) -> Vec<P> {
    resample_by(a, STEP).iter().zip(&resample_by(b, STEP)).map(|(p, q)| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]).collect()
}

/// `points` every `step` metres from its start, exactly, and no point for what is left at its end.
fn resample_by(points: &[P], step: f64) -> Vec<P> {
    let total: f64 = points.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
    (0..=(total / step).floor() as usize).map(|k| point_along(points, k as f64 * step)).collect()
}

/// A bridge as `build_along` makes it: its pieces, the line down the middle of what was built, and
/// each carriageway's own ramp as its line and its road's width.
pub type Built = (Vec<Piece>, Vec<P>, Vec<(Vec<P>, f64)>);

/// `build`, with the roads its ramps follow. With the pieces come the line down the middle of what
/// was built, ramps and all, and the line and road width of each carriageway's own ramp: `onward[0]` are the roads that lead on from the bridge's first point and `onward[1]` from
/// its last, each a line from where it leaves the bridge and its width, as the road the bridge
/// belongs to goes on along the ground. One road is followed by a ramp as wide as the deck. Two are
/// the carriageways that share the deck: each gets a ramp of its own, on its own line and as wide as
/// it, so that both are driven down wherever they go, and the ramps' heights are worked out along the
/// line midway between them. A ramp that outruns its road, or has none, carries straight on.
/// `joined` is the height of the roadway of another bridge that the first point, and the last, is a
/// point of, where there is one: a road that leaves a bridge out over the water starts on its deck,
/// at that height, and has no ramp there. From such an end it keeps to that bridge's roadway, whose
/// height at a point `rides` gives, for as long as it lies on it: the two leave each other slowly,
/// side by side, and must be one surface until they have parted.
#[allow(clippy::too_many_arguments)]
pub fn build_along(points: &[P], onward: [&[(Vec<P>, f64)]; 2], joined: [Option<f64>; 2], rides: &dyn Fn(P) -> Option<f64>, width: f64, kind: u8, pylons: Pylons, measured: &[[f64; 3]], under: &[Crossed], ground: &Ground, levels: &[i32], channel: &[bool], limit: f64) -> Built {
    let track_over_cutting = kind == TRACK && pylons == Pylons::Flush;
    let grade = max_grade(kind);
    let crossed = channel_crossed(points, ground, levels, channel);
    let crosses_channel = !crossed.is_empty();
    let samples = resample(points, if track_over_cutting { STEP_CUTTING_RAIL } else { STEP });
    if samples.len() < 2 || pylons == Pylons::Carried {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let node = |p: P, arc: f64| Node {
        p,
        y: 0.0,
        g: ground.height_at(p[0] as f32, p[1] as f32) as f64,
        level: water_at(ground, levels, p),
        over_channel: crossed.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) <= STEP),
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
    if !crosses_channel && short_and_even(&samples, ground) {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let lift = clearance(kind);
    let hw = width / 2.0 + EDGE_BEAM_WIDTH;
    // Where each ramp could run: on from each end in its direction, as far as `MAX_RAMP`, short of
    // water and of the edge of the world. How much of it is built follows from the deck's height.
    let reach = |p: P| p[0].abs() <= limit && p[1].abs() <= limit;
    let ramp_line = |from_end: bool| -> Vec<Node> {
        if joined[usize::from(from_end)].is_some() {
            return Vec::new();
        }
        let (last, before) = if from_end { (&nodes[nodes.len() - 1], &nodes[nodes.len() - 2]) } else { (&nodes[0], &nodes[1]) };
        // Along the road that leads on, then straight on in the direction it, or the deck, ends in.
        let along: Vec<P> = match onward[usize::from(from_end)] {
            [(path, _)] if path.len() >= 2 => resample_by(path, STEP).into_iter().skip(1).collect(),
            [(a, _), (b, _), ..] if a.len() >= 2 && b.len() >= 2 => between(a, b).into_iter().filter(|p| (p[0] - last.p[0]).hypot(p[1] - last.p[1]) > STEP / 2.0).collect(),
            _ => Vec::new(),
        };
        let (from, towards) = match along.len() {
            0 => (last.p, [last.p[0] - before.p[0], last.p[1] - before.p[1]]),
            1 => (along[0], [along[0][0] - last.p[0], along[0][1] - last.p[1]]),
            n => (along[n - 1], [along[n - 1][0] - along[n - 2][0], along[n - 1][1] - along[n - 2][1]]),
        };
        let length = towards[0].hypot(towards[1]).max(1e-9);
        let (dx, dz) = (towards[0] / length, towards[1] / length);
        let mut out = Vec::new();
        let mut distance = 0.0;
        let mut step = 0;
        while distance < MAX_RAMP {
            distance += STEP;
            step += 1;
            let beyond = (step - along.len().min(step)) as f64 * STEP;
            let p = if step <= along.len() { along[step - 1] } else { [from[0] + dx * beyond, from[1] + dz * beyond] };
            let next = node(p, 0.0);
            if !reach(p) || next.level.is_some() {
                break;
            }
            out.push(next);
        }
        out
    };
    let (head, tail) = (ramp_line(false), ramp_line(true));
    // Every node from the far end of the head's ramp to the far end of the tail's, the deck's among them.
    let deck = head.len()..head.len() + nodes.len();
    let shift = head.len() as f64 * STEP;
    let mut all: Vec<Node> = head.into_iter().rev().enumerate().map(|(k, n)| Node { arc: k as f64 * STEP, ..n }).collect();
    all.extend(nodes.into_iter().map(|n| Node { arc: n.arc + shift, ..n }));
    let mut end = all.last().map_or(0.0, |n| n.arc);
    for n in tail {
        end += STEP;
        all.push(Node { arc: end, ..n });
    }
    let arcs: Vec<f64> = all.iter().map(|n| n.arc).collect();
    let (a0, a1) = (arcs[deck.start], arcs[deck.end - 1]);
    // A deck over a cutting stays flush with its road: what goes under it is dug down instead.
    let crossings = if pylons == Pylons::Flush { Vec::new() } else { roads_under(&all, kind, hw, (a0, a1), under, ground) };
    let end_height = |n: &Node| n.level.unwrap_or(n.g);
    let (h0, h1) = (joined[0].map_or(end_height(&all[deck.start]), |top| top - LIFT), joined[1].map_or(end_height(&all[deck.end - 1]), |top| top - LIFT));
    // The heights the deck is held to where it rides on the bridge it leaves: from a joined end in, as
    // far as it lies on that bridge's roadway, and no farther than its own middle.
    let mut ridden: Vec<Option<f64>> = vec![None; all.len()];
    let middle = (deck.start + deck.end) / 2;
    if let Some(top) = joined[0] {
        ridden[deck.start] = Some(top);
        for i in deck.start + 1..middle {
            let Some(top) = rides(all[i].p) else { break };
            ridden[i] = Some(top);
        }
    }
    if let Some(top) = joined[1] {
        ridden[deck.end - 1] = Some(top);
        for i in (middle + 1..deck.end - 1).rev() {
            let Some(top) = rides(all[i].p) else { break };
            ridden[i] = Some(top);
        }
    }
    let measured = measured_profile(&all[deck.clone()], measured, grade);
    // The deck's profile, its ramps descending from its ends at its `grade`, held up over the roads
    // under it that `lifting` picks out.
    let profile = |lifting: &[bool]| -> Vec<f64> {
        let over = need_over(&arcs, &crossings.iter().zip(lifting).filter(|(_, lift)| **lift).map(|(u, _)| u).collect::<Vec<_>>());
        // The ground under a ramp's line holds up only the ramp itself, below: rising ground past a
        // bridge's end does not lift its deck. A road under the ramp does.
        let required: Vec<f64> = (0..all.len()).map(|i| if deck.contains(&i) { all[i].level.map_or(all[i].g + LIFT, |level| level + if all[i].over_channel { CHANNEL_CLEARANCE } else { lift }).max(over[i]).max(ridden[i].unwrap_or(f64::MIN)) } else { over[i] }).collect();
        let mut y: Vec<f64> = (0..all.len()).map(|i| if deck.contains(&i) { (h0 + (h1 - h0) * (arcs[i] - a0) / arc + LIFT).max(required[i]) } else { required[i] }).collect();
        limit_grade(&mut y, &arcs, grade);
        // The deck, and a ramp held up over a road, from the first thing that holds it up to the last.
        let held: Vec<usize> = (0..all.len()).filter(|&i| deck.contains(&i) || over[i].is_finite()).collect();
        let span = held.first().copied().unwrap_or(deck.start)..=held.last().copied().unwrap_or(deck.end - 1);
        fill_sags(&mut y[span.clone()]);
        // Soften the corners the envelope leaves on the deck, never dipping under what is needed.
        for _ in 0..3 {
            let before = y.clone();
            for i in deck.clone() {
                let (lo, hi) = (i.saturating_sub(2).max(deck.start), (i + 2).min(deck.end - 1));
                y[i] = (before[lo..=hi].iter().sum::<f64>() / (hi - lo + 1) as f64).max(required[i]);
            }
        }
        if let Some(found) = &measured {
            for (k, &m) in found.iter().enumerate() {
                let i = deck.start + k;
                y[i] = m.max(all[i].g + LIFT).max(over[i]);
            }
        } else {
            fill_sags(&mut y[span]);
        }
        limit_grade(&mut y, &arcs, grade);
        for (i, y) in y.iter_mut().enumerate().filter(|(i, _)| !deck.contains(i)) {
            *y = y.max(all[i].g + LIFT);
        }
        // An end on another bridge's deck is at its roadway, whatever the rest wants.
        for (y, ridden) in y.iter_mut().zip(&ridden) {
            if let Some(top) = ridden {
                *y = *top;
            }
        }
        y
    };
    // The ramps run out to the first node down at the lip every deck has over the ground: a step
    // any higher stops a wheel.
    let built = |y: &[f64]| -> std::ops::RangeInclusive<usize> {
        let above = |i: usize| y[i] - all[i].g > LIFT + 0.05;
        let (mut lo, mut hi) = (deck.start, deck.end - 1);
        while lo > 0 && above(lo) {
            lo -= 1;
        }
        while hi + 1 < all.len() && above(hi) {
            hi += 1;
        }
        lo..=hi
    };
    // Every road under the deck holds it up. One under a ramp does where the ramp reaches it standing
    // in its way, which may carry the ramp on over the next: until nothing more is reached.
    let mut lifting: Vec<bool> = crossings.iter().map(|u| (a0..=a1).contains(&u.middle)).collect();
    let mut y = profile(&lifting);
    for _ in 0..8 {
        let span = built(&y);
        let (from, to) = (arcs[*span.start()], arcs[*span.end()]);
        // The ramp's height where the road crosses, between the nodes either side of it.
        let at = |arc: f64| {
            let k = arcs.partition_point(|&a| a < arc).clamp(1, arcs.len() - 1);
            let t = ((arc - arcs[k - 1]) / (arcs[k] - arcs[k - 1]).max(1e-9)).clamp(0.0, 1.0);
            y[k - 1] + (y[k] - y[k - 1]) * t
        };
        let more: Vec<usize> = (0..crossings.len()).filter(|&k| !lifting[k] && (from..=to).contains(&crossings[k].middle) && at(crossings[k].middle) > crossings[k].surface + AT_GRADE).collect();
        if more.is_empty() {
            break;
        }
        for k in more {
            lifting[k] = true;
        }
        y = profile(&lifting);
    }
    let span = built(&y);
    let first = arcs[*span.start()];
    let whole: Vec<Node> = all.into_iter().zip(y).enumerate().filter(|(i, _)| span.contains(i)).map(|(_, (n, y))| Node { y, arc: n.arc - first, ..n }).collect();
    let line: Vec<P> = whole.iter().map(|n| n.p).collect();
    // Where two carriageways lead on from an end, the ramp there is theirs, each on its own line: the
    // deck is laid without it, and they take their heights from the profile, by how far out they are.
    let (from, to) = (deck.start - *span.start(), deck.end - 1 - *span.start());
    let twin = |end: usize| onward[end].len() >= 2 && pylons != Pylons::Flush;
    let height_at = |arc: f64| {
        let k = whole.partition_point(|n| n.arc < arc).clamp(1, whole.len() - 1);
        let (a, b) = (&whole[k - 1], &whole[k]);
        a.y + (b.y - a.y) * ((arc - a.arc) / (b.arc - a.arc).max(1e-9)).clamp(0.0, 1.0)
    };
    let mut strands: Vec<(Vec<Node>, f64)> = Vec::new();
    for end in [0, 1].into_iter().filter(|&end| twin(end)) {
        let (edge, out, most) = if end == 1 { (whole[to].arc, 1.0, whole[whole.len() - 1].arc - whole[to].arc) } else { (whole[from].arc, -1.0, whole[from].arc) };
        for (path, across) in onward[end].iter().take(2) {
            let strand: Vec<Node> = resample_by(path, STEP)
                .into_iter()
                .enumerate()
                .map(|(k, p)| (k as f64 * STEP, p))
                .take_while(|&(run, p)| run <= most + STEP && p[0].abs() <= limit && p[1].abs() <= limit)
                .map(|(run, p)| {
                    let g = f64::from(ground.height_at(p[0] as f32, p[1] as f32));
                    Node { p, y: height_at(edge + out * run.min(most)).max(g + LIFT), g, level: None, over_channel: false, arc: edge + out * run.min(most) }
                })
                .collect();
            if strand.len() >= 2 {
                strands.push((strand, across / 2.0 + EDGE_BEAM_WIDTH));
            }
        }
    }
    let nodes: Vec<Node> = whole.into_iter().enumerate().filter(|&(i, _)| (i >= from || !twin(0)) && (i <= to || !twin(1))).map(|(_, n)| n).collect();

    let tangent = |i: usize| {
        let (a, b) = (nodes[i.saturating_sub(1)].p, nodes[(i + 1).min(nodes.len() - 1)].p);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz).max(1e-9);
        [dx / length, 0.0, dz / length]
    };
    let side = |t: V3| [-t[2], 0.0, t[0]];
    let mut mesh = Mesh { pieces: Vec::new(), arc: 0.0 };
    let last = nodes.len() - 1;
    lay_deck(&mut mesh, &nodes, hw, pylons != Pylons::Flush, kind != TRACK, ground);
    // The carriageways' ramps, with no railing of one standing on the other or across a narrow slot
    // between the two.
    for (k, (strand, half)) in strands.iter().enumerate() {
        let before = mesh.pieces.len();
        lay_deck(&mut mesh, strand, *half, true, kind != TRACK, ground);
        let others: Vec<(&[Node], f64)> = strands.iter().enumerate().filter(|&(other, _)| other != k).map(|(_, (nodes, half))| (nodes.as_slice(), *half)).collect();
        let on_another = |p: P| others.iter().any(|(nodes, half)| nodes.windows(2).any(|pair| crate::assemble::distance_to_segment(p, pair[0].p, pair[1].p) < *half + STRAND_GAP));
        let laid = mesh.pieces.split_off(before);
        mesh.pieces.extend(laid.into_iter().filter(|piece| !piece.railing || !on_another(piece.at)));
    }
    // Piers under the high stretches.
    let mut since = PIER_SPACING;
    // A railway's deck over a cutting spans it: a pier would stand in the passage.
    let piered = if track_over_cutting { 1..1 } else { 1..last };
    for i in piered {
        since += nodes[i].arc - nodes[i - 1].arc;
        let n = &nodes[i];
        // A small river is spanned: no pier stands in its channel.
        if since >= PIER_SPACING && n.y - n.g > SOLID_BELOW + THICKNESS && !n.over_channel {
            since = 0.0;
            let t = tangent(i);
            mesh.arc = n.arc;
            // One wall of a pier under a deck, or a column under each half of a wide one: one piece.
            let columns: Vec<(f64, f64)> = if hw * 2.0 > WIDE_DECK { vec![(-hw * 0.5, hw * 0.25), (hw * 0.5, hw * 0.25)] } else { vec![(0.0, hw * 0.55)] };
            let mut tris = Vec::new();
            for (offset, half) in columns {
                let s = scale(side(t), offset);
                let at = [n.p[0] + s[0], n.p[1] + s[2]];
                let foot_y = ground.height_at(at[0] as f32, at[1] as f32) as f64 - 0.5;
                mesh.prism(n.p, false, [at[0], n.y - THICKNESS, at[1]], [at[0], foot_y.min(n.g - 0.5), at[1]], t, side(t), 0.9, half);
                if let Some(piece) = mesh.pieces.pop() {
                    tris.extend(piece.tris);
                }
            }
            mesh.pieces.push(Piece { at: n.p, steel: false, deck: false, railing: false, pier: true, arc: n.arc, tris });
        }
    }

    // Pylons with stay cables over the longest crossing of open water.
    if matches!(pylons, Pylons::Ends | Pylons::Middle) {
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
    // Each carriageway's ramp, as its line and the width of its road.
    let ramps = strands.iter().map(|(nodes, half)| (nodes.iter().map(|n| n.p).collect(), 2.0 * (half - EDGE_BEAM_WIDTH))).collect();
    (mesh.pieces, line, ramps)
}

/// Longest section a bridge is cut into, metres: a long stretch without piers (a ramp, a deck on the
/// ground) is cut more often, so no section's cells make too big a grid.
pub const MAX_SECTION: f64 = 48.0;

/// Groups a bridge's concrete `pieces` into the sections it is carved as (`BUILDING_BRIDGE`), as
/// numbers into `pieces`: cut halfway between each pier and the next, so each section stands on one
/// pier with half a span either side of it, and a stretch longer than `MAX_SECTION` cut evenly
/// again. Sections in order along the bridge; none empty.
pub fn sections(pieces: &[&Piece]) -> Vec<Vec<usize>> {
    if pieces.is_empty() {
        return Vec::new();
    }
    let (lo, hi) = pieces.iter().fold((f64::MAX, f64::MIN), |(lo, hi), piece| (lo.min(piece.arc), hi.max(piece.arc)));
    let mut piers: Vec<f64> = pieces.iter().filter(|piece| piece.pier).map(|piece| piece.arc).collect();
    piers.sort_by(f64::total_cmp);
    piers.dedup();
    let halfway: Vec<f64> = piers.windows(2).map(|pair| (pair[0] + pair[1]) / 2.0).collect();
    let mut cuts = Vec::new();
    let mut from = lo;
    for to in halfway.into_iter().chain([hi]) {
        let parts = ((to - from) / MAX_SECTION).ceil().max(1.0) as usize;
        cuts.extend((1..parts).map(|k| from + (to - from) * k as f64 / parts as f64));
        if to < hi {
            cuts.push(to);
        }
        from = to;
    }
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); cuts.len() + 1];
    for (k, piece) in pieces.iter().enumerate() {
        out[cuts.partition_point(|&cut| cut <= piece.arc)].push(k);
    }
    out.retain(|section| !section.is_empty());
    out
}

/// Whether a bridge triangle faces up: the roadway (the section's roof, where the piece is deck), or
/// the top of an edge beam or a rail.
pub fn faces_up(tri: [V3; 3]) -> bool {
    let n = client_normal(tri);
    n[1] > 0.7 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_measured_deck_replaces_the_inferred_one_and_ramps_down_beyond_it() {
        // Flat ground at 0, 120 m of way; the laser saw the deck at 9 m over its middle 60 m only.
        let ground = Ground::default();
        let points = [[0.0, 0.0], [120.0, 0.0]];
        let mut returns = Vec::new();
        for k in 0..240 {
            let x = 30.0 + f64::from(k) * 0.25;
            returns.extend([[x, -1.0, 9.0], [x, 0.0, 9.1], [x, 1.0, 8.9]]);
        }
        let nodes: Vec<Node> = resample(&points, STEP).iter().map(|&p| Node { p, y: 0.0, g: 0.0, level: None, over_channel: false, arc: p[0] }).collect();
        let y = measured_profile(&nodes, &returns, MAX_GRADE).unwrap();
        let at = |x: f64| y[(x / STEP).round() as usize];
        assert!((at(60.0) - 9.0).abs() < 0.11);
        // 6 % down from the edge of what was measured, then the ground.
        assert!((at(15.0) - (9.0 - MAX_GRADE * 15.0)).abs() < 0.2, "{}", at(15.0));
        assert!((at(0.0) - (9.0 - MAX_GRADE * 30.0)).abs() < 0.2);
        assert!(build(&points, 8.0, ROAD_MAJOR, Pylons::None, &returns, &[], &ground, &[], 1000.0).iter().any(|piece| piece.tris.iter().flatten().any(|v| (v[1] - 9.0).abs() < 0.2)));
    }

    use super::*;

    /// 600 m square of 10 m cells: banks at +3 m, a river 160 m wide along Z with its bed at -4 m
    /// and its surface at 0.
    fn river() -> (Ground, Vec<i32>) {
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
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::Middle, &[], &[], &ground, &levels, 295.0);
        let top = vertices(&pieces).filter(|v| v[0].abs() < 40.0).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top >= 7.0, "mid-span deck top is {top}");
        let near_end = vertices(&pieces).filter(|v| v[0] > 140.0 && v[1] > 1.0).map(|v| v[1]).fold(f64::MAX, f64::min);
        assert!((near_end - 3.12).abs() < 0.5, "the deck meets the bank, got {near_end}");
        assert!(pieces.iter().any(|p| p.steel), "a 160 m crossing gets a pylon");
    }

    /// Ground at 3 m with a small river down the middle: 8 m of water at 1.5 m, its bed at 0.5 m.
    fn small_river() -> (Ground, Vec<i32>, Vec<bool>) {
        let side = 201u32;
        let across = |k: u32| (f64::from(k % side) - 100.0).abs();
        let heights = (0..side * side).map(|k| if across(k) <= 4.0 { 50 } else if across(k) <= 6.0 { 200 } else { 300 }).collect();
        let levels = (0..side * side).map(|k| if across(k) <= 4.0 { 150 } else { NO_WATER }).collect();
        let channel = (0..side * side).map(|k| across(k) <= 6.0).collect();
        (Ground { origin: -10_000, cell: 100, side, heights }, levels, channel)
    }

    #[test]
    fn a_bridge_over_a_small_river_stays_level_with_its_banks() {
        let (ground, levels, channel) = small_river();
        let (pieces, ..) = build_along(&[[-15.0, 0.0], [15.0, 0.0]], [&[], &[]], [None, None], &|_| None, 8.0, ROAD_MAJOR, Pylons::None, &[], &[], &ground, &levels, &channel, 95.0);
        let top = top_at(&pieces, 4.5, [0.0, 0.0]).expect("a deck over the river");
        assert!((2.8..3.5).contains(&top), "not the 7 m a big river's is raised: {top}");
        assert!(!pieces.iter().any(|piece| piece.pier && piece.at[0].abs() <= 4.0), "and no pier in it");
    }

    #[test]
    fn a_short_bridge_over_a_carved_river_is_still_built() {
        let (ground, levels, channel) = small_river();
        // Both ends in the channel, so between them the ground does not dip at all.
        let (pieces, ..) = build_along(&[[-3.0, 0.0], [3.0, 0.0]], [&[], &[]], [None, None], &|_| None, 4.0, ROAD_PATH, Pylons::None, &[], &[], &ground, &levels, &channel, 95.0);
        let under = vertices(&pieces).filter(|v| v[0].abs() < 0.5).map(|v| v[1]).fold(f64::MAX, f64::min);
        assert!(!pieces.is_empty(), "a deck, not a culvert");
        assert!(under > 1.6, "its underside is clear of the water, no dam: {under}");
    }

    #[test]
    fn a_short_footbridge_gets_no_pylon_and_stays_near_the_ground() {
        let (ground, levels) = river();
        let pieces = build(&[[100.0, 0.0], [160.0, 0.0]], 2.5, ROAD_PATH, Pylons::None, &[], &[], &ground, &levels, 295.0);
        assert!(!pieces.is_empty());
        assert!(pieces.iter().all(|p| !p.steel));
        assert!(vertices(&pieces).all(|v| v[1] < 4.0));
    }

    #[test]
    fn a_bridge_is_cut_into_sections_at_mid_span_each_with_one_pier() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, &[], &[], &ground, &levels, 295.0);
        let concrete: Vec<&Piece> = pieces.iter().filter(|p| !p.steel).collect();
        let sections = sections(&concrete);
        assert!(sections.len() >= 4, "a 300 m bridge is cut in several sections, got {}", sections.len());
        assert_eq!(sections.iter().map(Vec::len).sum::<usize>(), concrete.len(), "every piece in one section");
        for section in &sections {
            let arcs: Vec<f64> = section.iter().map(|&k| concrete[k].arc).collect();
            let (lo, hi) = arcs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &a| (lo.min(a), hi.max(a)));
            assert!(hi - lo <= MAX_SECTION + STEP, "a section {} m long", hi - lo);
            assert!(section.iter().filter(|&&k| concrete[k].pier).count() <= 1, "at most one pier a section");
        }
        // Over the water every section stands on a pier, with deck on both sides of it.
        let piers: Vec<&Vec<usize>> = sections.iter().filter(|section| section.iter().any(|&k| concrete[k].pier)).collect();
        assert!(piers.len() >= 3);
        for section in piers {
            let pier = section.iter().map(|&k| concrete[k]).find(|p| p.pier).unwrap().arc;
            assert!(section.iter().any(|&k| concrete[k].arc < pier - 6.0) && section.iter().any(|&k| concrete[k].arc > pier + 6.0), "deck either side of the pier at {pier}");
        }
    }

    #[test]
    fn faces_point_out_of_the_solid() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::Middle, &[], &[], &ground, &levels, 295.0);
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
        let highest = pieces.iter().filter(|p| p.deck).flat_map(|p| &p.tris).filter(|t| t.iter().all(|v| (v[1] - t[0][1]).abs() < 1e-9)).map(|t| t[0][1]).fold(f64::MIN, f64::max);
        let top = pieces.iter().filter(|p| p.deck).flat_map(|p| &p.tris).find(|t| t.iter().all(|v| (v[1] - highest).abs() < 1e-9) && t[0][0].abs() < 40.0).expect("a top triangle");
        assert!(client_normal(*top)[1] > 0.0, "the top of the deck faces up");
    }

    #[test]
    fn a_high_deck_has_an_edge_beam_posts_and_two_rails_each_side() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, &[], &[], &ground, &levels, 295.0);
        // Mid-span, over the water: the railing pieces within 10 m of the middle, on the +z side.
        let near: Vec<&Piece> = pieces.iter().filter(|p| p.railing && p.at[0].abs() < 10.0 && p.at[1] > 0.0).collect();
        let deck = vertices(&pieces).filter(|v| v[0].abs() < 1.0).map(|v| v[1]).fold(f64::MIN, f64::max) - RAILING_HEIGHT;
        let height = |p: &Piece| p.tris.iter().flatten().map(|v| v[1]).fold(f64::MIN, f64::max) - deck;
        let tall = |p: &Piece| p.tris.iter().flatten().map(|v| v[1]).fold(f64::MIN, f64::max) - p.tris.iter().flatten().map(|v| v[1]).fold(f64::MAX, f64::min);
        let beams = near.iter().filter(|p| (height(p) - EDGE_BEAM_HEIGHT).abs() < 0.05).count();
        let posts = near.iter().filter(|p| (height(p) - RAILING_HEIGHT).abs() < 0.05 && tall(p) > RAILING_HEIGHT - EDGE_BEAM_HEIGHT - 0.05).count();
        let top_rails = near.iter().filter(|p| (height(p) - RAILING_HEIGHT).abs() < 0.05 && tall(p) < 0.2).count();
        let mid_rails = near.iter().filter(|p| (height(p) - MID_RAIL_HEIGHT - RAIL_HALF).abs() < 0.05).count();
        // 20 m at 3 m a cross section: six or seven stretches, a post at each.
        assert!(beams >= 6 && posts >= 6 && top_rails >= 6 && mid_rails >= 6, "beams {beams}, posts {posts}, top rails {top_rails}, middle rails {mid_rails}");
        assert_eq!(beams, top_rails);
        assert_eq!(beams, mid_rails);
    }

    #[test]
    fn the_deck_keeps_the_roads_width_between_its_edge_beams() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, &[], &[], &ground, &levels, 295.0);
        let deck = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten()).filter(|v| v[0].abs() < 10.0).map(|v| v[2].abs()).fold(0.0, f64::max);
        assert!((deck - (5.0 + EDGE_BEAM_WIDTH)).abs() < 1e-6, "deck half width {deck}");
        let inner = pieces.iter().filter(|p| p.railing && p.at[0].abs() < 10.0).flat_map(|p| p.tris.iter().flatten()).map(|v| v[2].abs()).fold(f64::MAX, f64::min);
        assert!((inner - 5.0).abs() < 1e-6, "the edge beams start {inner} m from the middle");
    }

    #[test]
    fn only_the_deck_is_drawn_as_road() {
        let (ground, levels) = river();
        let pieces = build(&[[-150.0, 0.0], [150.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::Middle, &[], &[], &ground, &levels, 295.0);
        assert!(pieces.iter().any(|p| p.deck));
        assert!(pieces.iter().filter(|p| p.deck).all(|p| !p.steel && !p.railing && !p.pier));
        // Every stretch of deck has its roadway on top, which `assemble` makes the section's roof.
        assert!(pieces.iter().filter(|p| p.deck).all(|p| p.tris.iter().any(|&t| faces_up(t))));
    }

    #[test]
    fn a_wide_deck_stands_on_two_columns() {
        let (ground, levels) = river();
        let pier = |width: f64| build(&[[-150.0, 0.0], [150.0, 0.0]], width, ROAD_MAJOR, Pylons::None, &[], &[], &ground, &levels, 295.0).into_iter().find(|p| p.pier).expect("a pier");
        // A closed box is 12 triangles.
        assert_eq!(pier(10.0).tris.len(), 12, "a 10 m road stands on one pier wall");
        assert_eq!(pier(20.0).tris.len(), 24, "a 20 m road on two columns");
        let wide = pier(20.0);
        let middle = wide.tris.iter().flatten().filter(|v| v[2].abs() < 2.0).count();
        assert_eq!(middle, 0, "a 20 m road stands on two columns with a gap between");
        assert!(wide.tris.iter().flatten().any(|v| v[2] > 3.0) && wide.tris.iter().flatten().any(|v| v[2] < -3.0));
    }

    #[test]
    fn a_deck_is_paved_planked_or_bare_as_what_it_carries() {
        assert_eq!(deck_paint(ROAD_MAJOR), DECK_ASPHALT);
        assert_eq!(deck_paint(ROAD_MINOR), DECK_ASPHALT);
        assert_eq!(deck_paint(ROAD_PAVED_PATH), DECK_ASPHALT);
        assert_eq!(deck_paint(ROAD_PATH), DECK_PLANKS);
        assert_eq!(deck_paint(ROAD_TRACK), DECK_PLANKS);
        assert_eq!(deck_paint(TRACK), DECK_BALLAST);
    }

    /// The lowest underside of the deck over the strip `inside` picks out, metres.
    fn underside_over(pieces: &[Piece], inside: impl Fn(V3) -> bool) -> f64 {
        pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten().copied()).filter(|&v| inside(v)).map(|v| v[1]).fold(f64::MAX, f64::min)
    }

    #[test]
    fn a_deck_over_a_road_rises_to_let_a_bus_under() {
        // Flat ground at 0: a 40 m bridge along x over a road 10 m wide along z.
        let ground = Ground::default();
        let road: &[P] = &[[0.0, -80.0], [0.0, 80.0]];
        let pieces = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(road, 10.0)], &ground, &[], 1000.0);
        let under = underside_over(&pieces, |v| v[0].abs() <= 5.0);
        assert!(under >= HEADROOM_ROAD - 1e-6, "the deck's underside over the road is {under} m up");
        // The same road cut in two under the deck, as OpenStreetMap often has it, still passes under.
        let (north, south): (&[P], &[P]) = (&[[0.0, -80.0], [0.0, 0.0]], &[[0.0, 0.0], [0.0, 80.0]]);
        let split = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(north, 10.0), (south, 10.0)], &ground, &[], 1000.0);
        let under = underside_over(&split, |v| v[0].abs() <= 5.0);
        assert!(under >= HEADROOM_ROAD - 1e-6, "over a road cut in two the underside is {under} m up");
        // It comes down again on ramps past its ends, to the lip it has over the ground.
        let far = pieces.iter().flat_map(|p| p.tris.iter().flatten()).map(|v| v[0].abs()).fold(0.0, f64::max);
        assert!(far > 60.0, "the ramps reach {far} m out");
        let end = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten()).filter(|v| v[0].abs() >= far - 0.5).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(end < LIFT + 0.3, "the ramp ends {end} m up");
        // The same bridge with no road under it lies on the ground.
        let without = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[], &ground, &[], 1000.0);
        let top = without.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten()).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top < LIFT + 0.01, "the deck is {top} m up");
    }

    #[test]
    fn a_ramp_over_a_road_past_the_bridges_end_lets_a_bus_under_but_one_where_it_has_come_down_is_met_at_grade() {
        // Flat ground: the road under the bridge lifts it, and its ramps run on some 90 m past each end.
        let ground = Ground::default();
        let under: &[P] = &[[0.0, -80.0], [0.0, 80.0]];
        let beyond: &[P] = &[[40.0, -80.0], [40.0, 80.0]];
        let pieces = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(under, 10.0), (beyond, 7.0)], &ground, &[], 1000.0);
        let over = underside_over(&pieces, |v| (v[0] - 40.0).abs() <= 3.5);
        assert!(over >= HEADROOM_ROAD - 1e-6, "the ramp's underside over the road past the end is {over} m up");
        // A road far out along the bridge's line, where the ramps have long come down, changes nothing.
        let far: &[P] = &[[200.0, -80.0], [200.0, 80.0]];
        let alone = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(under, 10.0)], &ground, &[], 1000.0);
        let with_far = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(under, 10.0), (far, 7.0)], &ground, &[], 1000.0);
        let reach = |pieces: &[Piece]| pieces.iter().flat_map(|p| p.tris.iter().flatten()).map(|v| v[0]).fold(f64::MIN, f64::max);
        assert!(reach(&alone) < 150.0, "the ramp runs out to {}", reach(&alone));
        assert_eq!(reach(&with_far), reach(&alone));
        // A crossroads where the ramp has all but come down is met at grade, not bridged.
        let x = reach(&alone) - 2.0;
        let landing: &[P] = &[[x, -80.0], [x, 80.0]];
        let with_landing = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(under, 10.0), (landing, 7.0)], &ground, &[], 1000.0);
        assert_eq!(reach(&with_landing), reach(&alone));
    }

    #[test]
    fn a_deck_over_two_roads_runs_level_between_them_and_does_not_sag() {
        // Flat ground: a 160 m bridge over two roads 60 m apart.
        let ground = Ground::default();
        let (west, east): (&[P], &[P]) = (&[[-30.0, -80.0], [-30.0, 80.0]], &[[30.0, -80.0], [30.0, 80.0]]);
        let pieces = build(&[[-80.0, 0.0], [80.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(west, 8.0), (east, 8.0)], &ground, &[], 1000.0);
        // The roadway's height along the bridge, every metre, from its highest vertex in each strip.
        let top = |x: f64| pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten()).filter(|v| (v[0] - x).abs() < 1.6).map(|v| v[1]).fold(f64::MIN, f64::max);
        let between: Vec<f64> = (-30..=30).map(|x| top(f64::from(x))).collect();
        let crest = top(-30.0).min(top(30.0));
        assert!(between.iter().all(|&y| y >= crest - 0.01), "the deck dips between the roads: {between:?}");
    }

    #[test]
    fn a_ramp_follows_the_road_that_leads_on_from_the_bridges_end() {
        // Flat ground: a bridge along x lifted over a road, and the road it belongs to turning off
        // 30 degrees to the south at its east end.
        let ground = Ground::default();
        let under: &[P] = &[[0.0, -80.0], [0.0, 80.0]];
        let (dx, dz) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let on = [(vec![[20.0, 0.0], [20.0 + 200.0 * dx, 200.0 * dz]], 8.0)];
        let (pieces, ..) = build_along(&[[-20.0, 0.0], [20.0, 0.0]], [&[], &on], [None, None], &|_| None, 8.0, ROAD_MAJOR, Pylons::None, &[], &[Crossed::road(under, 10.0)], &ground, &[], &[], 1000.0);
        // Every bit of ramp east of the bridge lies on the road, within its width of the road's line.
        let off = pieces.iter().filter(|p| p.deck && p.at[0] > 21.0).map(|p| ((p.at[0] - 20.0) * dz - p.at[1] * dx).abs()).fold(0.0, f64::max);
        assert!(off < 0.5, "the ramp strays {off} m from the road");
        let far = pieces.iter().filter(|p| p.deck).map(|p| p.at[0]).fold(f64::MIN, f64::max);
        assert!(far > 60.0, "the ramp runs on to x = {far}");
        // West of the bridge, with no road given, it carries straight on.
        assert!(pieces.iter().filter(|p| p.deck && p.at[0] < -21.0).all(|p| p.at[1].abs() < 1e-6));
    }

    #[test]
    fn two_carriageways_past_a_shared_deck_each_come_down_on_a_ramp_of_their_own() {
        // Flat ground: a deck 17 m wide lifted over a road, and past its east end two carriageways
        // 8 m wide that start 4.5 m either side of its line and draw apart, one bending off south.
        let ground = Ground::default();
        let under: &[P] = &[[0.0, -80.0], [0.0, 80.0]];
        let roads = [(vec![[20.0, -4.5], [60.0, -8.0], [200.0, -8.0]], 8.0), (vec![[20.0, 4.5], [60.0, 14.0], [160.0, 90.0]], 8.0)];
        let (pieces, line, _) = build_along(&[[-20.0, 0.0], [20.0, 0.0]], [&[], &roads], [None, None], &|_| None, 17.0, ROAD_MAJOR, Pylons::None, &[], &[Crossed::road(under, 10.0)], &ground, &[], &[], 1000.0);
        let off = |p: P, road: &[P]| road.windows(2).map(|w| crate::assemble::distance_to_segment(p, w[0], w[1])).fold(f64::MAX, f64::min);
        // Past the deck every piece of roadway is on one carriageway or the other, and both are covered
        // as far as the ramps go.
        let ramp: Vec<&Piece> = pieces.iter().filter(|p| p.deck && p.at[0] > 23.0).collect();
        assert!(ramp.iter().all(|p| off(p.at, &roads[0].0).min(off(p.at, &roads[1].0)) < 0.5), "a piece of ramp lies off both roads");
        for (road, _) in &roads {
            let far = ramp.iter().filter(|p| off(p.at, road) < 0.5).map(|p| p.at[0]).fold(f64::MIN, f64::max);
            assert!(far > 60.0, "a carriageway's ramp ends at x = {far}");
        }
        // They leave the deck at its height and come down together.
        let top = |x: f64, road: &[P]| ramp.iter().filter(|p| off(p.at, road) < 0.5 && (p.at[0] - x).abs() < 2.5).flat_map(|p| p.tris.iter().flatten()).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top(26.0, &roads[0].0) > 3.5 && (top(26.0, &roads[0].0) - top(26.0, &roads[1].0)).abs() < 0.3);
        // No railing between the two where they still lie against each other.
        let between: Vec<P> = pieces.iter().filter(|p| p.railing && p.at[0] > 21.0 && p.at[0] < 24.0 && p.at[1].abs() < 2.0).map(|p| p.at).collect();
        assert!(between.is_empty(), "a railing stands between the carriageways: {between:?}");
        assert!(line.len() > 20);
    }

    #[test]
    fn a_bridge_that_leaves_another_out_over_the_water_starts_at_its_deck() {
        // A slip road's bridge from the middle of the river, where it leaves a bridge 9 m up, to the bank.
        let (ground, levels) = river();
        let (pieces, ..) = build_along(&[[0.0, 0.0], [150.0, 0.0]], [&[], &[]], [Some(9.0), None], &|p| (p[0] < 30.0).then_some(9.0), 4.5, ROAD_MINOR, Pylons::None, &[], &[], &ground, &levels, &[], 295.0);
        let top = |x: f64| pieces.iter().filter(|p| p.deck && (p.at[0] - x).abs() < 2.0).flat_map(|p| p.tris.iter().flatten()).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!((top(1.5) - 9.0).abs() < 0.2, "it starts {} m up", top(1.5));
        // It keeps to the deck it leaves for the 30 m it lies on it, then comes down.
        assert!((top(25.0) - 9.0).abs() < 0.05, "25 m out, still on the other deck, it is {} m up", top(25.0));
        assert!(top(40.0) > 5.4 && top(40.0) < 9.0, "over the water it is {} m up", top(40.0));
        assert!(pieces.iter().all(|p| p.at[0] > -0.1), "it has a ramp back from the bridge it leaves");
    }

    #[test]
    fn a_ramp_ends_on_the_ground_and_follows_its_slope_across_where_it_is_low() {
        // Ground falling 5 % towards +z: a bridge along x lifted over a road, its ramps running out
        // along the slope.
        let side = 61u32;
        let heights = (0..side).flat_map(|j| (0..side).map(move |_| (30 - j as i32) * 25)).collect();
        let ground = Ground { origin: -15_000, cell: 500, side, heights };
        let height = |x: f64, z: f64| f64::from(ground.height_at(x as f32, z as f32));
        let under: &[P] = &[[0.0, -100.0], [0.0, 100.0]];
        let pieces = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(under, 8.0)], &ground, &[], 140.0);
        let tops: Vec<V3> = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten().copied()).filter(|v| v[1] > height(v[0], v[2]) - 0.01).collect();
        // The far edge of each ramp lies on the ground at both its corners.
        let far = tops.iter().map(|v| v[0]).fold(f64::MIN, f64::max);
        for v in tops.iter().filter(|v| v[0] > far - 0.01) {
            assert!((v[1] - height(v[0], v[2]) - LANDING).abs() < 0.03, "the ramp's end is {} m over the ground at z = {}", v[1] - height(v[0], v[2]), v[2]);
        }
        // Two cross sections back it is the deck's lip up at both edges, uphill and down.
        for v in tops.iter().filter(|v| (v[0] - (far - 2.0 * STEP)).abs() < 0.01) {
            let over = v[1] - height(v[0], v[2]);
            assert!(over > 0.05 && over < FOLLOWS, "a low section is {over} m over the ground at z = {}", v[2]);
        }
    }

    /// A bridge of `kind` along x over `under` on flat ground at 0.
    fn over(half: f64, kind: u8, under: &[Crossed]) -> Vec<Piece> {
        build_along(&[[-half, 0.0], [half, 0.0]], [&[], &[]], [None, None], &|_| None, 8.0, kind, Pylons::None, &[], under, &Ground::default(), &[], &[], 1000.0).0
    }

    #[test]
    fn a_road_deck_rises_to_let_a_train_under_but_not_at_a_level_crossing() {
        let line: &[P] = &[[0.0, -80.0], [0.0, 80.0]];
        let track = Crossed { line, width: 3.6, headroom: HEADROOM_RAIL, track: true, level: &[] };
        let under = underside_over(&over(20.0, ROAD_MINOR, &[track]), |v| v[0].abs() <= 1.8);
        assert!(under >= HEADROOM_RAIL - 1e-6, "the deck's underside over the track is {under} m up");
        let crossing: &[P] = &[[0.0, 0.0]];
        let pieces = over(20.0, ROAD_MINOR, &[Crossed { level: crossing, ..track }]);
        let top = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten()).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top < LIFT + 0.01, "a level crossing is lifted {top} m");
    }

    #[test]
    fn a_road_that_ends_under_a_short_railway_bridge_passes_under_it_at_its_signed_height() {
        // As Ostra Kyrkogatan's way ends under its bridge, 15 m long, whose ends are within the road's reach.
        let line: &[P] = &[[0.0, -60.0], [0.0, 0.0]];
        let road = Crossed { headroom: headroom_under(Some(3.6)), ..Crossed::road(line, 7.0) };
        let pieces = over(7.5, TRACK, &[road]);
        let under = underside_over(&pieces, |v| v[0].abs() <= 3.5);
        assert!(under >= 3.6 - 1e-6, "the underside over the road is {under} m up");
        assert!(under < HEADROOM_ROAD, "the signed height is all it needs, not {under} m");
        // It comes down no more steeply than a train climbs.
        let far = pieces.iter().flat_map(|p| p.tris.iter().flatten()).map(|v| v[0].abs()).fold(0.0, f64::max);
        assert!(far > (3.6 + THICKNESS - 0.5) / MAX_GRADE_RAIL, "its ramps reach only {far} m out");
        assert_eq!([headroom_under(None), headroom_under(Some(2.4)), headroom_under(Some(5.5))], [HEADROOM_ROAD, HEADROOM_LOWEST, HEADROOM_ROAD]);
    }

    #[test]
    fn a_railway_deck_over_a_narrow_passage_has_no_pier_in_it_and_keeps_its_headroom() {
        // A passage 3.5 m wide and 3.7 m deep, its walls 1:1, across the track.
        let side = 81u32;
        let heights = (0..side * side).map(|at| ((((f64::from(at % side) - 40.0).abs() - 1.75).max(0.0) - 3.7).min(0.0) * 100.0) as i32).collect();
        let ground = Ground { origin: -4_000, cell: 100, side, heights };
        let pieces = build(&[[-12.0, 0.0], [12.0, 0.0]], 4.0, TRACK, Pylons::Flush, &[], &[], &ground, &[], 1000.0);
        assert!(pieces.iter().all(|p| !p.pier), "a pier in the passage");
        let under = underside_over(&pieces, |v| v[0].abs() <= 1.0);
        assert!(under + 3.7 >= 3.0 - 0.02, "{} m clear in the passage", under + 3.7);
        let roof: Vec<[V3; 3]> = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().copied()).filter(|&t| faces_up(t)).collect();
        assert!(roadway_at(&roof, [0.3, 0.2]).is_some_and(|top| top.abs() < 0.2), "the track over it is level");
        assert!(roadway_at(&roof, [0.0, 9.0]).is_none(), "beside the deck");
    }

    #[test]
    fn a_road_alongside_or_joining_a_bridge_does_not_lift_it() {
        let ground = Ground::default();
        // The bridge's own road going on along the ground past both ends, and a road that joins it at its end.
        let along: &[P] = &[[-80.0, 0.0], [80.0, 0.0]];
        let joining: &[P] = &[[20.0, -60.0], [20.0, 0.0]];
        let pieces = build(&[[-20.0, 0.0], [20.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(along, 8.0), (joining, 6.0)], &ground, &[], 1000.0);
        let top = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten()).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top < LIFT + 0.01, "the deck is {top} m up");
    }

    #[test]
    fn a_road_crossing_at_a_slant_is_cleared_across_its_whole_width() {
        // A road 8 m wide crossing the bridge at 30 degrees to it, on ground rising 2 % along z.
        let side = 41u32;
        let heights = (0..side).flat_map(|j| (0..side).map(move |_| (j as i32 - 20) * 20)).collect();
        let ground = Ground { origin: -20_000, cell: 1_000, side, heights };
        let (dx, dz) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let road: &[P] = &[[-120.0 * dx, -120.0 * dz], [120.0 * dx, 120.0 * dz]];
        let pieces = build(&[[-40.0, 0.0], [40.0, 0.0]], 8.0, ROAD_MAJOR, Pylons::None, &[], &[(road, 8.0)], &ground, &[], 1000.0);
        // Every bit of deck over the road's width, measured square to the road, against the ground under it.
        let over = |v: V3| (v[0] * dz - v[2] * dx).abs() <= 4.0;
        let clear = pieces.iter().filter(|p| p.deck).flat_map(|p| p.tris.iter().flatten().copied()).filter(|&v| over(v)).map(|v| v[1] - f64::from(ground.height_at(v[0] as f32, v[2] as f32))).fold(f64::MAX, f64::min);
        assert!(clear >= HEADROOM_ROAD - 0.25, "the deck clears the road by {clear} m");
    }
}
