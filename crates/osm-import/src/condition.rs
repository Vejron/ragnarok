//! The height model shaped to what OpenStreetMap says stands on it, so the ground looks built rather
//! than scanned. Lantmäteriet's model is the laser's ground, smoothed: a road comes out with a
//! lumpy crown and soft cuts, a stream is a smear the mesh barely shows, and a house stands on
//! whatever pits and humps the filter left where its walls were taken out. Four shapers fix that,
//! in this order, each on a 1 m raster of the model:
//!
//! - **Pads.** The ground inside a building's outline is pulled level to its low quartile, and the
//!   pull eases off over an apron round it.
//! - **Streams.** A stream's or ditch's centre line has a bed that only ever falls downstream, and a
//!   channel is cut down to it with steep banks. Cutting never raises the ground.
//! - **Roads and rails.** The model along a way's centre line is smoothed into a long profile. The
//!   carriageway is held level across, with a slight camber, and the ground beside it may differ from
//!   it only as much as a bank of `BANK` allows, so cuts and embankments come out as clean banks.
//!   A road over a stream fills the stream's channel back in, as over a culvert.
//! - **Underpasses.** A path or road in a short tunnel under a road or a railway is dug down to
//!   leave headroom under the deck over it, and the ways leading in ramp down to it.
//!
//! Every shaper moves the ground towards a level, less the further it is from the feature, and beyond
//! the feature's reach the model is left as it is. The result
//! is kept as an offset from the model (`Shaping::mean`), which the ground samples add to their own
//! mean of the model, so a 4 m ground and the 1 m detail slopes are shaped alike.

use crate::bridge::HEADROOM_ROAD;
use crate::overpass::Osm;
use shared::world::Frame;
use std::collections::HashMap;

type P = [f64; 2];

/// Side of a raster cell, metres. The model is 1 m, so finer gains nothing.
const CELL: f64 = 1.0;
/// Cells a side of a block of the raster, which is only kept where a feature reaches.
const BLOCK: i32 = 32;

/// How far a building's ground is moved towards its level, metres. A house on a steeper slope keeps
/// the rest of the slope, so a hillside is not dug out for a shed.
const PAD_MAX: f64 = 1.0;
/// The share of the ground under a building that lies below its level: the low quartile, so the
/// pad sits in the ground on a slope rather than on a plinth.
const PAD_QUANTILE: f64 = 0.25;
/// How fast that pull eases off outside the walls, metres per metre out: the apron is 2 m wide.
const PAD_SLOPE: f64 = 0.5;

/// Depth of a stream's bed below the lowest ground along it, metres.
const STREAM_DEPTH: f64 = 0.5;
/// Half the width of a stream's bed, metres: the water drawn in it (`water::STREAM_WIDTH`), so the
/// water lies flat in the channel and grass banks rise either side of it.
const STREAM_HALF_BED: f64 = crate::water::STREAM_WIDTH / 2.0;
/// Steepness of a stream's banks, metres of height per metre out from the bed.
const STREAM_BANK: f64 = 1.0;
/// How far out of the bed a stream may cut, metres: a bank this wide rises 2 m.
const STREAM_REACH: f64 = 2.0;
/// The bed is the lowest ground within this distance of the centre line, metres, so a line drawn a
/// little off the ditch still finds it.
const STREAM_SNAP: f64 = 1.5;
/// A stream's long profile is the lowest ground in each stretch this long, metres.
const STREAM_BIN: f64 = 10.0;
/// The most a stream cuts into the ground, metres, in its bed; the most it cuts falls to nothing at
/// the edge of its reach. Where the profile wants more (a line mapped over a rise the water does not
/// cross, a culvert under a deep embankment) the ground is left nearer what was scanned.
const STREAM_MAX_CUT: f64 = 1.0;

/// Steepness the ground beside a road or a track may rise or fall from it, metres per metre: a
/// bank of 1 in 2, as a road cut or an embankment is built.
const BANK: f64 = 0.5;
/// How much a carriageway is crowned, metres of fall per metre from the centre line.
const CAMBER: f64 = 0.02;
/// Distance between the samples of a way's long profile, metres.
const PROFILE_STEP: f64 = 2.0;

/// What a way is graded as: how far its long profile is smoothed and how far it may leave the model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grade {
    /// The long profile is smoothed over this length either side of each point, metres.
    pub smooth: f64,
    /// The most the profile may stand above or below the model at the centre line, metres.
    pub shift: f64,
}

/// A road: a smooth profile over a few car lengths, never more than a metre and a half from the model.
pub const GRADE_ROAD: Grade = Grade { smooth: 8.0, shift: 1.5 };
/// A footpath, cycleway or forest track: walked, not driven, and left closer to the ground.
pub const GRADE_PATH: Grade = Grade { smooth: 5.0, shift: 0.5 };
/// A railway: long and gentle.
pub const GRADE_RAIL: Grade = Grade { smooth: 25.0, shift: 2.0 };

/// A way the ground is graded to: its centre line in local metres, its width, metres, and its grade.
#[derive(Clone, Debug)]
pub struct Way {
    pub points: Vec<P>,
    pub width: f64,
    pub grade: Grade,
}

/// The longest stretch of tunnel taken as an underpass and dug out, metres: enough to pass under a
/// motorway's two carriageways. A longer tunnel goes through a hill and is left alone.
const PASSAGE_LONGEST: f64 = 60.0;
/// The same for a tunnel under railway tracks, metres: one under a station's yard runs on under its
/// platforms and sidings.
const PASSAGE_LONGEST_RAIL: f64 = 100.0;
/// Clear height over an underpass's floor, under the deck of the road over it, metres: a footway's or
/// cycleway's. A road's is a bridge's over a road (`bridge::HEADROOM_ROAD`).
const HEADROOM_PATH: f64 = 3.0;
/// How far either side of where a road crosses an underpass its level is read, metres: past the dip
/// the height model may have under it, as `assemble::cuttings` finds the deck.
const PASSAGE_ROAD_REACH: f64 = 12.0;
/// How steeply a way leading into an underpass comes down to its floor, rise over run.
const PASSAGE_RAMP: f64 = 0.06;
/// How far along each way leading in its ramp may reach, metres.
const PASSAGE_APPROACH: f64 = 60.0;
/// Steepness of the sides of an underpass's cutting, metres of height per metre out: the walls the
/// deck's ends stand on, and the cut a ramp comes down in.
const PASSAGE_BANK: f64 = 1.0;
/// A little floor either side of the way, metres, so the walls stand clear of its edges.
const PASSAGE_MARGIN: f64 = 0.5;
/// The most an underpass is dug below the model, metres.
const PASSAGE_MAX_CUT: f64 = 5.0;

/// How much wider an underpass under tracks is dug, each side, metres: the ground is sampled every
/// 2 m and averaged round each sample, which narrows so short a trench to less than the way in it.
const PASSAGE_WIDER_RAIL: f64 = 1.25;
/// How much deeper, metres: the track may slope a little between where its level is read and the
/// middle of the deck, and the ways leading in start to climb under the deck's edges.
const PASSAGE_SPARE_RAIL: f64 = 0.35;
/// A footway, cycleway or road taken under a road or a railway in a short tunnel (an underpass,
/// `tunnel=yes` or a negative `layer`). The height model usually has it far too shallow, or not at
/// all: the embankment over it was the laser's ground. So its floor is dug down to leave headroom
/// under the deck (`assemble::cuttings` builds it over the cutting), and the ways leading in come
/// down to it.
#[derive(Clone, Debug)]
pub struct Passage {
    /// The way through the tunnel, from one portal to the other.
    pub under: Vec<P>,
    /// The ways leading in, each from the portal outwards, and whether it starts at the last point of
    /// `under` rather than the first.
    pub approaches: Vec<(bool, Vec<P>)>,
    /// Its width, metres.
    pub width: f64,
    /// Clear height wanted over its floor, metres.
    pub headroom: f64,
    /// The centre lines of the roads and tracks over it.
    pub over: Vec<Vec<P>>,
    /// Whether tracks are among them. Its levels are then read before any underpass is dug: a yard's
    /// tracks cross two side by side, and the second would be read off the first's trench.
    pub railway: bool,
}

/// What the ground is shaped to, in local metres.
#[derive(Clone, Debug, Default)]
pub struct Features {
    /// Building outlines, each an open ring.
    pub pads: Vec<Vec<P>>,
    /// Streams' and ditches' centre lines, drawn downstream as OpenStreetMap draws them.
    pub streams: Vec<Vec<P>>,
    /// Roads and rails, narrowest first, so the wider way wins where two meet.
    pub ways: Vec<Way>,
    /// Underpasses, dug after everything else.
    pub passages: Vec<Passage>,
}

impl Features {
    /// The buildings, roads and rails of `osm` (projected by `frame`) and `streams`, already local.
    /// Bridges and tunnels are not on the ground and are left out, except short tunnels under a road
    /// or a railway, which are dug out as underpasses.
    pub fn from_osm(osm: &Osm, frame: Frame, streams: &[Vec<P>]) -> Self {
        let line = |way: &crate::overpass::Way| -> Vec<P> { way.geometry.iter().map(|g| frame.project(g.lat, g.lon)).collect() };
        let mut features = Features { streams: streams.to_vec(), ..Default::default() };
        // Ways in tunnels, ways on the ground (which may lead into one), and the roads that may pass over one.
        let mut tunnels: Vec<(Vec<P>, f64, u8)> = Vec::new();
        let mut open: Vec<Vec<P>> = Vec::new();
        let mut roads: Vec<Vec<P>> = Vec::new();
        let mut tracks: Vec<Vec<P>> = Vec::new();
        for way in &osm.ways {
            let tag = |key: &str| way.tags.get(key).map(String::as_str);
            if tag("building").is_some_and(|value| value != "no" && value != "roof") {
                let mut ring = line(way);
                if ring.len() >= 4 && ring.first() == ring.last() {
                    ring.pop();
                    features.pads.push(ring);
                }
                continue;
            }
            let underground = tag("tunnel").is_some_and(|value| value != "no") || tag("layer").and_then(|layer| layer.parse::<i32>().ok()).is_some_and(|layer| layer < 0);
            let road = tag("highway").filter(|&value| value != "steps").and_then(|_| crate::lanes::style(&way.tags));
            if let Some((width, kind, _)) = road {
                if underground {
                    tunnels.push((line(way), width, kind));
                } else {
                    if matches!(kind, shared::world::ROAD_MAJOR | shared::world::ROAD_MINOR) {
                        roads.push(line(way));
                    }
                    if !crate::assemble::is_bridge(way) {
                        open.push(line(way));
                    }
                }
            }
            if underground || crate::assemble::is_bridge(way) {
                continue;
            }
            if let Some((width, kind, _)) = road {
                let grade = if matches!(kind, shared::world::ROAD_PATH | shared::world::ROAD_PAVED_PATH) { GRADE_PATH } else { GRADE_ROAD };
                features.ways.push(Way { points: line(way), width, grade });
            }
            if let Some((gauge, kind)) = crate::assemble::rail_style(way) {
                features.ways.push(Way { points: line(way), width: crate::assemble::ballast(gauge), grade: GRADE_RAIL });
                // A tram's track is in its street, which is over the passage itself.
                if kind == shared::world::RAIL_TRAIN {
                    tracks.push(line(way));
                }
            }
        }
        features.ways.sort_by(|a, b| a.width.total_cmp(&b.width));
        features.passages = passages(tunnels, &open, &roads, &tracks);
        features
    }
}

/// Whether two points are one node of OpenStreetMap, metres apart.
fn same(a: P, b: P) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 0.5
}

/// Whether segments `a → b` and `c → d` cross or touch.
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    d1 * d2 <= 0.0 && d3 * d4 <= 0.0 && (d1, d2, d3, d4) != (0.0, 0.0, 0.0, 0.0)
}

/// The underpasses among the ways in tunnels (`(line, width, ROAD_*)`): the ones that join end to end
/// into a stretch no longer than `PASSAGE_LONGEST` that a road on the ground or a bridge crosses
/// over, or no longer than `PASSAGE_LONGEST_RAIL` that one of the `tracks` on the ground does. `open`
/// are the ways on the ground, from which the ways leading in are taken.
fn passages(mut tunnels: Vec<(Vec<P>, f64, u8)>, open: &[Vec<P>], roads: &[Vec<P>], tracks: &[Vec<P>]) -> Vec<Passage> {
    // OpenStreetMap often cuts a tunnel where its layer or surface changes: join the pieces.
    let mut joined: Vec<(Vec<P>, f64, u8)> = Vec::new();
    while let Some((mut points, width, kind)) = tunnels.pop() {
        if points.len() < 2 {
            continue;
        }
        while let Some(k) = tunnels.iter().position(|(other, ..)| other.len() >= 2 && [points[0], points[points.len() - 1]].iter().any(|&end| same(end, other[0]) || same(end, other[other.len() - 1]))) {
            let (mut other, ..) = tunnels.swap_remove(k);
            if same(points[points.len() - 1], other[other.len() - 1]) || same(points[0], other[0]) {
                other.reverse();
            }
            if same(points[points.len() - 1], other[0]) {
                points.extend(other.into_iter().skip(1));
            } else {
                other.extend(points.into_iter().skip(1));
                points = other;
            }
        }
        joined.push((points, width, kind));
    }
    let mut found = Vec::new();
    for (under, width, kind) in joined {
        let length = arcs(&under).last().copied().unwrap_or(0.0);
        if length > PASSAGE_LONGEST_RAIL {
            continue;
        }
        let crossing = |lines: &[Vec<P>]| -> Vec<Vec<P>> { lines.iter().filter(|line| line.windows(2).any(|r| under.windows(2).any(|u| crosses(u[0], u[1], r[0], r[1])))).cloned().collect() };
        let (mut over, railway) = (crossing(roads), crossing(tracks));
        let railway_over = !railway.is_empty();
        let longest = if railway_over { PASSAGE_LONGEST_RAIL } else { PASSAGE_LONGEST };
        over.extend(railway);
        if over.is_empty() || length > longest {
            continue;
        }
        let mut approaches = Vec::new();
        for (at_end, portal) in [(false, under[0]), (true, under[under.len() - 1])] {
            for way in open.iter().filter(|way| way.len() >= 2) {
                let outward: Vec<P> = if same(way[0], portal) {
                    way.clone()
                } else if same(way[way.len() - 1], portal) {
                    way.iter().rev().copied().collect()
                } else {
                    continue;
                };
                approaches.push((at_end, clip(&outward, PASSAGE_APPROACH)));
            }
        }
        let (wider, spare) = if railway_over { (PASSAGE_WIDER_RAIL, PASSAGE_SPARE_RAIL) } else { (0.0, 0.0) };
        let headroom = if matches!(kind, shared::world::ROAD_MAJOR | shared::world::ROAD_MINOR) { HEADROOM_ROAD } else { HEADROOM_PATH };
        found.push(Passage { under, approaches, width: width + 2.0 * wider, headroom: headroom + spare, over, railway: railway_over });
    }
    found
}

/// The first `length` metres of a polyline.
fn clip(points: &[P], length: f64) -> Vec<P> {
    let mut out = vec![points[0]];
    let mut run = 0.0;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let step = (b[0] - a[0]).hypot(b[1] - a[1]);
        if run + step >= length {
            let t = (length - run) / step.max(1e-9);
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            break;
        }
        run += step;
        out.push(b);
    }
    out
}

/// What an underpass is dug to: the highest its floor may be for its headroom under what is over it,
/// and the ground at its two portals, metres.
struct Levels {
    ceiling: f64,
    portals: [f64; 2],
}

/// One block of the raster: the model and the shaped ground, NaN where the model has no data.
struct Block {
    model: Vec<f32>,
    shaped: Vec<f32>,
}

/// The ground shaped to the features, as an offset from the model on a 1 m raster.
#[derive(Default)]
pub struct Shaping {
    blocks: HashMap<(i32, i32), Block>,
}

/// A cell of the raster: its centre is at `(i, j)` metres.
type Cell = (i32, i32);

fn cell_at([x, z]: P) -> Cell {
    ((x / CELL).round() as i32, (z / CELL).round() as i32)
}

fn centre((i, j): Cell) -> P {
    [f64::from(i) * CELL, f64::from(j) * CELL]
}

/// The point of segment `a → b` nearest `p`: the distance to it and how far along the segment it is (0..1).
fn nearest(p: P, a: P, b: P) -> (f64, f64) {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dz * dz;
    let t = if length > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length).clamp(0.0, 1.0) } else { 0.0 };
    ((p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dz), t)
}

/// Every cell within `reach` of the polyline, with its distance from the line and the arc length,
/// metres along the line, of the nearest point of it.
fn along(points: &[P], reach: f64) -> HashMap<Cell, (f64, f64)> {
    let mut out: HashMap<Cell, (f64, f64)> = HashMap::new();
    let mut start = 0.0;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let (lo, hi) = (cell_at([a[0].min(b[0]) - reach, a[1].min(b[1]) - reach]), cell_at([a[0].max(b[0]) + reach, a[1].max(b[1]) + reach]));
        for i in lo.0..=hi.0 {
            for j in lo.1..=hi.1 {
                let (d, t) = nearest(centre((i, j)), a, b);
                if d <= reach && out.get(&(i, j)).is_none_or(|&(known, _)| d < known) {
                    out.insert((i, j), (d, start + t * length));
                }
            }
        }
        start += length;
    }
    out
}

/// 1 out to `full` metres from a feature, falling linearly to 0 at `reach`.
fn taper(d: f64, full: f64, reach: f64) -> f64 {
    if d <= full { 1.0 } else { ((reach - d) / (reach - full)).clamp(0.0, 1.0) }
}

/// Arc length at each point of a polyline, metres.
fn arcs(points: &[P]) -> Vec<f64> {
    let mut out = Vec::with_capacity(points.len());
    let mut total = 0.0;
    for (k, p) in points.iter().enumerate() {
        if k > 0 {
            total += (p[0] - points[k - 1][0]).hypot(p[1] - points[k - 1][1]);
        }
        out.push(total);
    }
    out
}

/// The polyline resampled every `step` metres or less, with the arc length of each sample.
fn resample(points: &[P], step: f64) -> (Vec<P>, Vec<f64>) {
    let at = arcs(points);
    let total = *at.last().unwrap_or(&0.0);
    let n = (total / step).ceil().max(1.0) as usize;
    let mut out = Vec::with_capacity(n + 1);
    let mut s_out = Vec::with_capacity(n + 1);
    let mut k = 0;
    for m in 0..=n {
        let s = total * m as f64 / n as f64;
        while k + 2 < at.len() && at[k + 1] < s {
            k += 1;
        }
        let span = (at[k + 1] - at[k]).max(1e-9);
        let t = ((s - at[k]) / span).clamp(0.0, 1.0);
        let (a, b) = (points[k], points[k + 1]);
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        s_out.push(s);
    }
    (out, s_out)
}

/// The median of each value's neighbours within `reach` samples either side, NaNs left out.
fn median_filter(values: &[f64], reach: usize) -> Vec<f64> {
    (0..values.len())
        .map(|k| {
            let mut near: Vec<f64> = values[k.saturating_sub(reach)..(k + reach + 1).min(values.len())].iter().copied().filter(|v| v.is_finite()).collect();
            if near.is_empty() {
                return f64::NAN;
            }
            near.sort_by(f64::total_cmp);
            near[near.len() / 2]
        })
        .collect()
}

/// The mean of each value's neighbours within `reach` samples either side, NaNs left out.
fn mean_filter(values: &[f64], reach: usize) -> Vec<f64> {
    (0..values.len())
        .map(|k| {
            let near = values[k.saturating_sub(reach)..(k + reach + 1).min(values.len())].iter().filter(|v| v.is_finite());
            let (sum, count) = near.fold((0.0, 0), |(sum, count), v| (sum + v, count + 1));
            if count == 0 { f64::NAN } else { sum / f64::from(count) }
        })
        .collect()
}

/// Whether `p` is inside the ring (even-odd).
fn inside(ring: &[P], p: P) -> bool {
    let mut odd = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            odd = !odd;
        }
        j = i;
    }
    odd
}

impl Shaping {
    /// Shapes the model (`model(x, z)`, the elevation at a local point averaged over a cell, `None`
    /// where there is no data) to `features`.
    pub fn new(features: &Features, model: impl Fn(f64, f64) -> Option<f64>) -> Self {
        let mut shaping = Shaping::default();
        for ring in &features.pads {
            shaping.pad(ring, &model);
        }
        for line in &features.streams {
            shaping.stream(line, &model);
        }
        for way in &features.ways {
            shaping.grade(way, &model);
        }
        let ahead: Vec<Option<Levels>> = features.passages.iter().map(|passage| if passage.railway { shaping.levels(passage, &model) } else { None }).collect();
        for (passage, ahead) in features.passages.iter().zip(ahead) {
            let levels = if passage.railway { ahead } else { shaping.levels(passage, &model) };
            if let Some(levels) = levels {
                shaping.passage(passage, levels, &model);
            }
        }
        shaping.blocks.retain(|_, block| block.model.iter().zip(&block.shaped).any(|(m, s)| m != s && m.is_finite()));
        shaping
    }

    /// How many cells of the ground the features moved, and by how much in all, metres.
    pub fn moved(&self) -> (usize, f64) {
        self.blocks.values().flat_map(|block| block.model.iter().zip(&block.shaped)).filter(|(m, s)| m.is_finite() && m != s).fold((0, 0.0), |(n, sum), (m, s)| (n + 1, sum + f64::from((s - m).abs())))
    }

    fn slot((i, j): Cell) -> ((i32, i32), usize) {
        let key = (i.div_euclid(BLOCK), j.div_euclid(BLOCK));
        (key, (j.rem_euclid(BLOCK) * BLOCK + i.rem_euclid(BLOCK)) as usize)
    }

    /// The model and the shaped ground at a cell, filling its block from the model the first time
    /// it is touched.
    fn cell(&mut self, at: Cell, model: &impl Fn(f64, f64) -> Option<f64>) -> (f64, &mut f32) {
        let (key, index) = Self::slot(at);
        let block = self.blocks.entry(key).or_insert_with(|| {
            let mut heights = vec![f32::NAN; (BLOCK * BLOCK) as usize];
            for j in 0..BLOCK {
                for i in 0..BLOCK {
                    let p = centre((key.0 * BLOCK + i, key.1 * BLOCK + j));
                    heights[(j * BLOCK + i) as usize] = model(p[0], p[1]).map_or(f32::NAN, |h| h as f32);
                }
            }
            Block { model: heights.clone(), shaped: heights }
        });
        (f64::from(block.model[index]), &mut block.shaped[index])
    }

    /// The ground as shaped so far at a cell, `None` where the model has no data.
    fn height(&mut self, at: Cell, model: &impl Fn(f64, f64) -> Option<f64>) -> Option<f64> {
        let h = *self.cell(at, model).1;
        h.is_finite().then_some(f64::from(h))
    }

    fn pad(&mut self, ring: &[P], model: &impl Fn(f64, f64) -> Option<f64>) {
        if ring.len() < 3 {
            return;
        }
        let reach = PAD_MAX / PAD_SLOPE;
        let (min, max) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let (lo, hi) = (cell_at([min[0] - reach, min[1] - reach]), cell_at([max[0] + reach, max[1] + reach]));
        let mut cells = Vec::new();
        let mut under = Vec::new();
        for i in lo.0..=hi.0 {
            for j in lo.1..=hi.1 {
                let p = centre((i, j));
                let out = if inside(ring, p) { 0.0 } else { (0..ring.len()).map(|k| nearest(p, ring[k], ring[(k + 1) % ring.len()]).0).fold(f64::MAX, f64::min) };
                if out == 0.0
                    && let Some(h) = self.height((i, j), model)
                {
                    under.push(h);
                }
                if out <= reach {
                    cells.push(((i, j), out));
                }
            }
        }
        // A footprint smaller than a cell still has the ground at its middle.
        if under.is_empty() {
            let n = ring.len() as f64;
            let middle = ring.iter().fold([0.0, 0.0], |sum, p| [sum[0] + p[0] / n, sum[1] + p[1] / n]);
            under.extend(self.height(cell_at(middle), model));
        }
        if under.is_empty() {
            return;
        }
        under.sort_by(f64::total_cmp);
        let level = under[((under.len() - 1) as f64 * PAD_QUANTILE).round() as usize];
        for (at, out) in cells {
            let pull = PAD_MAX - out * PAD_SLOPE;
            let (_, h) = self.cell(at, model);
            if h.is_finite() {
                let off = f64::from(*h) - level;
                *h = (level + off - off.clamp(-pull, pull)) as f32;
            }
        }
    }

    fn stream(&mut self, line: &[P], model: &impl Fn(f64, f64) -> Option<f64>) {
        if line.len() < 2 {
            return;
        }
        let (samples, s) = resample(line, CELL);
        // The lowest ground across the line at each sample, then the lowest in each bin.
        let snap = (STREAM_SNAP / CELL).round() as i32;
        let lowest: Vec<f64> = samples
            .iter()
            .map(|&p| {
                let c = cell_at(p);
                (-snap..=snap).flat_map(|di| (-snap..=snap).map(move |dj| (c.0 + di, c.1 + dj))).filter_map(|at| self.height(at, model)).fold(f64::NAN, f64::min)
            })
            .collect();
        let total = *s.last().unwrap();
        let bins = (total / STREAM_BIN).ceil().max(1.0) as usize;
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for bin in 0..bins {
            let (from, to) = (bin as f64 * STREAM_BIN, (bin + 1) as f64 * STREAM_BIN);
            // The lowest sample of the bin, where it is, so a steady slope is not cut deeper.
            let low = s.iter().zip(&lowest).filter(|(at, h)| **at >= from && (**at < to || bin + 1 == bins) && h.is_finite()).min_by(|a, b| a.1.total_cmp(b.1));
            if let Some((&at, &h)) = low {
                xs.push(at);
                ys.push(h);
            }
        }
        if ys.is_empty() {
            return;
        }
        // Never rising along its course: the closest monotonic profile, in whichever direction fits
        // the ground better, since a stream is now and then drawn the wrong way.
        let ones = vec![1.0; ys.len()];
        let rising = crate::water::isotonic(&ys, &ones);
        let reversed: Vec<f64> = ys.iter().rev().copied().collect();
        let mut falling = crate::water::isotonic(&reversed, &ones);
        falling.reverse();
        let misfit = |fit: &[f64]| fit.iter().zip(&ys).map(|(f, y)| (f - y).powi(2)).sum::<f64>();
        let profile = if misfit(&rising) < misfit(&falling) { rising } else { falling };
        let reach = STREAM_HALF_BED + STREAM_REACH;
        for (at, (d, along)) in along(line, reach) {
            let bed = crate::water::interpolate(&xs, &profile, along) - STREAM_DEPTH;
            let allowed = bed + (d - STREAM_HALF_BED).max(0.0) * STREAM_BANK;
            let deepest = STREAM_MAX_CUT * taper(d, STREAM_HALF_BED, reach);
            // Never deeper than `deepest` below the model, whatever cut here before.
            let (scanned, h) = self.cell(at, model);
            if h.is_finite() && f64::from(*h) > allowed {
                *h = allowed.max(scanned - deepest).min(f64::from(*h)) as f32;
            }
        }
    }

    fn grade(&mut self, way: &Way, model: &impl Fn(f64, f64) -> Option<f64>) {
        if way.points.len() < 2 {
            return;
        }
        let (samples, s) = resample(&way.points, PROFILE_STEP);
        let natural: Vec<f64> = samples.iter().map(|&p| self.height(cell_at(p), model).unwrap_or(f64::NAN)).collect();
        if natural.iter().all(|h| !h.is_finite()) {
            return;
        }
        let reach = (way.grade.smooth / PROFILE_STEP).round() as usize;
        let smooth = mean_filter(&median_filter(&natural, reach), reach);
        // Where the smoothing would take the way far from the model (a cut the model already has
        // deeper, a mapped line off the real road), it stays within `shift` of it.
        let profile: Vec<f64> = smooth.iter().zip(&natural).map(|(&p, &n)| if n.is_finite() { p.clamp(n - way.grade.shift, n + way.grade.shift) } else { p }).collect();
        let (xs, ys): (Vec<f64>, Vec<f64>) = s.iter().zip(&profile).filter(|(_, p)| p.is_finite()).map(|(&x, &y)| (x, y)).unzip();
        if ys.is_empty() {
            return;
        }
        let half = way.width / 2.0;
        let reach = half + 2.0 * way.grade.shift / BANK;
        for (at, (d, along)) in along(&way.points, reach) {
            let level = crate::water::interpolate(&xs, &ys, along);
            let (scanned, h) = self.cell(at, model);
            if !h.is_finite() {
                continue;
            }
            let natural = f64::from(*h);
            let wanted = if d <= half { level - CAMBER * d } else { natural.clamp(level - CAMBER * half - (d - half) * BANK, level - CAMBER * half + (d - half) * BANK) };
            // No cell moves more than the grade's shift, and less towards the edge of its reach, so a
            // way's end or a steep hillside beside it is eased into rather than stepped.
            // Ways that overlap (a road and the footway beside it) do not add up: none takes the
            // ground further than its shift from the model.
            let most = way.grade.shift * taper(d, half, reach);
            let moved = natural + (wanted - natural).clamp(-most, most);
            *h = moved.clamp(natural.min(scanned - way.grade.shift), natural.max(scanned + way.grade.shift)) as f32;
        }
    }

    /// The levels an underpass is dug to (`Levels`), from the ground as shaped so far, or `None`
    /// where the model has no ground to read one from.
    fn levels(&mut self, passage: &Passage, model: &impl Fn(f64, f64) -> Option<f64>) -> Option<Levels> {
        // The roadway over it: where each road crosses, the lower of the highest ground along the road
        // either side, so a dip the model has under the road is bridged.
        let mut ceiling = f64::INFINITY;
        // A crossover that ends over a railway's passage has only the trench to read at that end: the
        // level is the tracks' that run on past it both ways, if any does.
        let mut short = f64::INFINITY;
        for road in &passage.over {
            let (samples, s) = resample(road, CELL);
            let heights: Vec<f64> = samples.iter().map(|&p| self.height(cell_at(p), model).unwrap_or(f64::NAN)).collect();
            let crossing = samples
                .iter()
                .map(|&p| passage.under.windows(2).map(|pair| nearest(p, pair[0], pair[1]).0).fold(f64::MAX, f64::min))
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(k, _)| s[k]);
            let Some(crossing) = crossing else { continue };
            let highest = |from: f64, to: f64| s.iter().zip(&heights).filter(|(at, h)| (from..=to).contains(*at) && h.is_finite()).map(|(_, &h)| h).fold(f64::NAN, f64::max);
            let level = highest(crossing - PASSAGE_ROAD_REACH, crossing).min(highest(crossing, crossing + PASSAGE_ROAD_REACH));
            if level.is_finite() {
                let ends_over_it = crossing < PASSAGE_ROAD_REACH || s[s.len() - 1] - crossing < PASSAGE_ROAD_REACH;
                let lowest = if passage.railway && ends_over_it { &mut short } else { &mut ceiling };
                *lowest = lowest.min(level - crate::bridge::THICKNESS - passage.headroom);
            }
        }
        if !ceiling.is_finite() {
            ceiling = short;
        }
        if !ceiling.is_finite() {
            return None;
        }
        let (first, last) = (passage.under[0], passage.under[passage.under.len() - 1]);
        Some(Levels { ceiling, portals: [self.height(cell_at(first), model)?, self.height(cell_at(last), model)?] })
    }

    /// Digs out an underpass: its floor runs straight from portal to portal, but never higher than its
    /// headroom and the deck's thickness under the lowest road or track over it, and the ways leading in come
    /// down to it at `PASSAGE_RAMP`. Banks of `PASSAGE_BANK` rise either side. It only ever lowers the
    /// ground, so where the model has the underpass deep enough already, nothing changes.
    fn passage(&mut self, passage: &Passage, Levels { ceiling, portals: [first, last] }: Levels, model: &impl Fn(f64, f64) -> Option<f64>) {
        let length = *arcs(&passage.under).last().unwrap();
        let floor = |along: f64| (first + (last - first) * along / length.max(1e-9)).min(ceiling);
        let half = passage.width / 2.0 + PASSAGE_MARGIN;
        let reach = half + PASSAGE_MAX_CUT / PASSAGE_BANK;
        let mut cut = |line: &[P], wanted: &dyn Fn(f64) -> f64| {
            for (at, (d, along)) in along(line, reach) {
                let allowed = wanted(along) + (d - half).max(0.0) * PASSAGE_BANK;
                let (scanned, h) = self.cell(at, model);
                if h.is_finite() && f64::from(*h) > allowed {
                    *h = allowed.max(scanned - PASSAGE_MAX_CUT).min(f64::from(*h)) as f32;
                }
            }
        };
        cut(&passage.under, &floor);
        for (at_end, approach) in &passage.approaches {
            let portal = floor(if *at_end { length } else { 0.0 });
            cut(approach, &|along| portal + PASSAGE_RAMP * along);
        }
    }

    /// The mean offset of the shaped ground from the model over the cells within `half` metres of
    /// `(x, z)`, metres; 0 where nothing was shaped.
    pub fn mean(&self, x: f64, z: f64, half: f64) -> f64 {
        if self.blocks.is_empty() {
            return 0.0;
        }
        let eps = 1e-6;
        let (lo, hi) = (cell_at([x - half + eps, z - half + eps]), cell_at([x + half - eps, z + half - eps]));
        let (mut sum, mut count) = (0.0, 0u32);
        for j in lo.1..=hi.1 {
            for i in lo.0..=hi.0 {
                count += 1;
                let (key, index) = Self::slot((i, j));
                if let Some(block) = self.blocks.get(&key) {
                    let (m, s) = (block.model[index], block.shaped[index]);
                    if m.is_finite() {
                        sum += f64::from(s - m);
                    }
                }
            }
        }
        if count == 0 { 0.0 } else { sum / f64::from(count) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shaped ground at a point: the model plus the offset at its cell.
    fn at(shaping: &Shaping, model: impl Fn(f64, f64) -> Option<f64>, x: f64, z: f64) -> f64 {
        model(x, z).unwrap() + shaping.mean(x, z, 0.5)
    }

    #[test]
    fn a_house_on_a_bumpy_yard_stands_on_a_flat_pad_that_eases_into_the_yard() {
        // Half-metre bumps every few metres over flat ground at 10 m.
        let model = |x: f64, z: f64| Some(10.0 + 0.5 * (x * 1.3).sin() * (z * 1.7).cos());
        let ring = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]];
        let shaping = Shaping::new(&Features { pads: vec![ring], ..Default::default() }, model);
        let inside: Vec<f64> = (1..10).flat_map(|x| (1..8).map(move |z| (x, z))).map(|(x, z)| at(&shaping, model, f64::from(x), f64::from(z))).collect();
        let (low, high) = inside.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        assert!(high - low < 1e-4, "the pad is flat: {low}..{high}");
        // A metre out, the ground may differ from the pad by half a metre more than the pad's own allowance.
        for x in 0..=10 {
            let out = at(&shaping, model, f64::from(x), -1.0);
            assert!((out - low).abs() <= PAD_MAX + PAD_SLOPE + 1e-4, "{x}: {out} against {low}");
        }
        // Beyond the apron the yard is as scanned.
        assert_eq!(at(&shaping, model, 20.0, 20.0), model(20.0, 20.0).unwrap());
    }

    #[test]
    fn a_road_is_level_across_and_smooth_along_with_banks_beside_it() {
        // A gentle slope along x with 30 cm lumps.
        let model = |x: f64, z: f64| Some(20.0 + 0.03 * x + 0.3 * (x * 0.9).sin() + 0.2 * (z * 2.1).cos());
        let way = Way { points: vec![[-50.0, 0.0], [50.0, 0.0]], width: 7.0, grade: GRADE_ROAD };
        let shaping = Shaping::new(&Features { ways: vec![way], ..Default::default() }, model);
        for x in -30..=30 {
            let x = f64::from(x);
            let centre = at(&shaping, model, x, 0.0);
            // Across the carriageway, only the camber.
            for z in [-3.0, -2.0, -1.0, 1.0, 2.0, 3.0] {
                let h = at(&shaping, model, x, z);
                assert!((centre - h - CAMBER * f64::abs(z)).abs() < 1e-4, "x {x} z {z}: {h} against {centre}");
            }
            // Along it, no step bigger than the slope plus a little of what is left of the lumps.
            let next = at(&shaping, model, x + 1.0, 0.0);
            assert!((next - centre).abs() < 0.08, "x {x}: {centre} then {next}");
        }
        // Two metres off the edge the ground is within a metre of the edge (a 1:2 bank).
        let edge = at(&shaping, model, 0.0, 3.0);
        assert!((at(&shaping, model, 0.0, 6.0) - edge).abs() <= 2.5 * BANK + 0.1);
    }

    #[test]
    fn a_stream_bed_never_rises_downstream_and_sits_below_its_banks() {
        // Ground falling along x with a bump in the middle that would dam a naive channel.
        let model = |x: f64, z: f64| Some(30.0 - 0.02 * x + if x.abs() < 10.0 { 0.4 } else { 0.0 } + 0.05 * z.abs());
        let line = vec![[-80.0, 0.0], [80.0, 0.0]];
        let shaping = Shaping::new(&Features { streams: vec![line], ..Default::default() }, model);
        let bed: Vec<f64> = (-70..=70).map(|x| at(&shaping, model, f64::from(x), 0.0)).collect();
        assert!(bed.windows(2).all(|w| w[1] <= w[0] + 1e-4), "falls downstream: {bed:?}");
        for x in [-60.0, 0.0, 60.0] {
            let (middle, bank) = (at(&shaping, model, x, 0.0), at(&shaping, model, x, 3.5));
            assert!(bank - middle >= STREAM_DEPTH - 1e-3, "x {x}: bed {middle}, bank {bank}");
        }
        // Cutting never raises the ground, nor digs deeper than it may.
        for x in -70..=70 {
            for z in -4..=4 {
                let (x, z) = (f64::from(x), f64::from(z));
                let cut = model(x, z).unwrap() - at(&shaping, model, x, z);
                assert!((-1e-4..=STREAM_MAX_CUT + 1e-4).contains(&cut), "{x}, {z}: cut {cut}");
            }
        }
    }

    #[test]
    fn a_stream_drawn_against_the_slope_is_not_dug_into_a_trench() {
        // Ground rising 10 m along the line, which OpenStreetMap draws uphill.
        let model = |x: f64, _: f64| Some(10.0 + 0.05 * x);
        let shaping = Shaping::new(&Features { streams: vec![vec![[-100.0, 0.0], [100.0, 0.0]]], ..Default::default() }, model);
        for x in -90..=90 {
            let x = f64::from(x);
            let cut = model(x, 0.0).unwrap() - at(&shaping, model, x, 0.0);
            assert!((STREAM_DEPTH - 0.3..=STREAM_DEPTH + 0.3).contains(&cut), "{x}: cut {cut}");
        }
    }

    #[test]
    fn a_road_at_the_end_of_a_bridge_moves_no_ground_more_than_its_grade_allows() {
        // A road that ends where the model drops 8 m into a valley, as at a bridge abutment.
        let model = |x: f64, _: f64| Some(if x < 40.0 { 20.0 } else { 12.0 });
        let way = Way { points: vec![[-40.0, 0.0], [40.0, 0.0]], width: 10.0, grade: GRADE_ROAD };
        let shaping = Shaping::new(&Features { ways: vec![way], ..Default::default() }, model);
        for x in -50..=60 {
            for z in -15..=15 {
                let (x, z) = (f64::from(x), f64::from(z));
                let moved = (at(&shaping, model, x, z) - model(x, z).unwrap()).abs();
                assert!(moved <= GRADE_ROAD.shift + 1e-4, "{x}, {z}: moved {moved}");
            }
        }
    }

    #[test]
    fn a_road_over_a_stream_fills_its_channel_as_over_a_culvert() {
        let model = |_: f64, _: f64| Some(5.0);
        let stream = vec![[0.0, -30.0], [0.0, 30.0]];
        let road = Way { points: vec![[-30.0, 0.0], [30.0, 0.0]], width: 7.0, grade: GRADE_ROAD };
        let shaping = Shaping::new(&Features { streams: vec![stream], ways: vec![road], ..Default::default() }, model);
        assert!((at(&shaping, model, 0.0, 0.0) - 5.0).abs() < 1e-4, "the road runs level over the stream");
        assert!(at(&shaping, model, 0.0, 20.0) < 5.0 - STREAM_DEPTH + 1e-4, "the stream's channel away from the road");
    }

    #[test]
    fn an_underpass_is_dug_to_headroom_under_the_road_and_its_paths_ramp_down_to_it() {
        // Flat ground, as a height model has it where the road's embankment hides the underpass.
        let model = |_: f64, _: f64| Some(10.0);
        let road = Way { points: vec![[-60.0, 0.0], [60.0, 0.0]], width: 10.0, grade: GRADE_ROAD };
        let tunnel = (vec![[0.0, -6.0], [0.0, 0.0], [0.0, 6.0]], 2.5, shared::world::ROAD_PAVED_PATH);
        let open = vec![vec![[0.0, -6.0], [0.0, -80.0]], vec![[0.0, 80.0], [0.0, 6.0]], road.points.clone()];
        let found = passages(vec![tunnel], &open, std::slice::from_ref(&road.points), &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].approaches.len(), 2, "a way leads in at each portal");
        let shaping = Shaping::new(&Features { ways: vec![road], passages: found, ..Default::default() }, model);
        let floor = 10.0 - crate::bridge::THICKNESS - HEADROOM_PATH;
        for z in [-6.0, -3.0, 0.0, 3.0, 6.0] {
            let h = at(&shaping, model, 0.0, z);
            assert!((h - floor).abs() < 1e-3, "the floor under the road, z {z}: {h}");
        }
        // The way's edges are on the floor too, and the walls rise from it.
        assert!((at(&shaping, model, 1.0, 0.0) - floor).abs() < 1e-3);
        let half = 2.5 / 2.0 + PASSAGE_MARGIN;
        assert!((at(&shaping, model, 4.0, 0.0) - (floor + (4.0 - half) * PASSAGE_BANK)).abs() < 1e-3, "the wall beside it");
        // The paths come down to it at the ramp's slope, and are left alone past where they meet the ground.
        for z in [16.0, 26.0, 36.0] {
            let h = at(&shaping, model, 0.0, z);
            assert!((h - (floor + PASSAGE_RAMP * (z - 6.0))).abs() < 1e-3, "the ramp at z {z}: {h}");
        }
        assert_eq!(at(&shaping, model, 0.0, 75.0), 10.0);
        assert_eq!(at(&shaping, model, 30.0, 0.0), 10.0, "the road away from it");
    }

    #[test]
    fn a_tunnel_cut_in_pieces_is_one_underpass_and_a_long_one_or_one_under_no_road_is_none() {
        let road = vec![[-30.0, 0.0], [30.0, 0.0]];
        // In two pieces, one drawn against the other.
        let pieces = vec![(vec![[0.0, -6.0], [0.0, 1.0]], 2.5, shared::world::ROAD_PATH), (vec![[0.0, 8.0], [0.0, 1.0]], 2.5, shared::world::ROAD_PATH)];
        let found = passages(pieces, &[], std::slice::from_ref(&road), &[]);
        assert_eq!(found.len(), 1);
        assert!((arcs(&found[0].under).last().unwrap() - 14.0).abs() < 1e-9, "{:?}", found[0].under);
        // Through a hill, under the road and on.
        assert!(passages(vec![(vec![[0.0, -40.0], [0.0, 40.0]], 7.0, shared::world::ROAD_MINOR)], &[], std::slice::from_ref(&road), &[]).is_empty());
        // A short tunnel beside the road, under nothing.
        assert!(passages(vec![(vec![[0.0, 5.0], [0.0, 15.0]], 2.5, shared::world::ROAD_PATH)], &[], std::slice::from_ref(&road), &[]).is_empty());
    }

    #[test]
    fn a_path_under_a_railway_is_dug_out_and_one_76_m_long_under_a_yard_too_but_not_under_a_road() {
        let model = |_: f64, _: f64| Some(10.0);
        let yard: Vec<Vec<P>> = [-9.0, -4.5, 0.0, 4.5, 9.0].iter().map(|&z| vec![[-80.0, z], [80.0, z]]).collect();
        for half in [6.0, 38.0] {
            let tunnel = vec![(vec![[0.0, -half], [0.0, half]], 3.0, shared::world::ROAD_PAVED_PATH)];
            let found = passages(tunnel, &[], &[], &yard);
            assert_eq!(found.len(), 1, "a tunnel {} m long under the tracks", half * 2.0);
            let ways = yard.iter().map(|track| Way { points: track.clone(), width: 3.6, grade: GRADE_RAIL }).collect();
            let shaping = Shaping::new(&Features { ways, passages: found, ..Default::default() }, model);
            let dug = 10.0 - at(&shaping, model, 0.0, 0.0);
            assert!(dug >= crate::bridge::THICKNESS + HEADROOM_PATH, "the floor is {dug} m under the tracks");
        }
        // A crossover that ends in the trench the model already has does not set the level.
        let mut switched = yard.clone();
        switched.push(vec![[-2.0, 2.0], [30.0, 4.0]]);
        let trench = |x: f64, _: f64| Some(if x.abs() < 3.0 { 7.0 } else { 10.0 });
        let found = passages(vec![(vec![[0.0, -38.0], [0.0, 38.0]], 3.0, shared::world::ROAD_PAVED_PATH)], &[], &[], &switched);
        let shaping = Shaping::new(&Features { passages: found, ..Default::default() }, trench);
        let floor = at(&shaping, trench, 0.0, 0.0);
        assert!((floor - (10.0 - crate::bridge::THICKNESS - HEADROOM_PATH - PASSAGE_SPARE_RAIL)).abs() < 1e-3, "the crossover set the floor at {floor} m");
        // Two side by side are dug as deep as one, the second not read off the first's trench.
        let pair = [0.0, 8.0].map(|x| (vec![[x, -38.0], [x, 38.0]], 3.0, shared::world::ROAD_PAVED_PATH)).to_vec();
        let shaping = Shaping::new(&Features { passages: passages(pair, &[], &[], &yard), ..Default::default() }, model);
        for x in [0.0, 8.0] {
            let dug = 10.0 - at(&shaping, model, x, 0.0);
            assert!((dug - crate::bridge::THICKNESS - HEADROOM_PATH - PASSAGE_SPARE_RAIL).abs() < 1e-3, "one of the pair is dug {dug} m");
        }
        let tunnel = vec![(vec![[0.0, -38.0], [0.0, 38.0]], 3.0, shared::world::ROAD_PAVED_PATH)];
        assert!(passages(tunnel, &[], &yard, &[]).is_empty(), "as long a tunnel under roads alone");
    }

    #[test]
    fn the_offset_is_a_mean_over_the_box() {
        let model = |_: f64, _: f64| Some(0.0);
        let ring = vec![[-0.4, -0.4], [0.4, -0.4], [0.4, 0.4], [-0.4, 0.4]];
        let mut shaping = Shaping::new(&Features { pads: vec![ring], ..Default::default() }, model);
        // Flat ground needs no shaping at all.
        assert!(shaping.blocks.is_empty());
        shaping.blocks.insert((0, 0), Block { model: vec![0.0; (BLOCK * BLOCK) as usize], shaped: vec![0.0; (BLOCK * BLOCK) as usize] });
        shaping.blocks.get_mut(&(0, 0)).unwrap().shaped[0] = -4.0;
        assert_eq!(shaping.mean(0.0, 0.0, 0.5), -4.0);
        // A 3 m box over the cell at the block's corner holds 9 cells, one moved.
        assert!((shaping.mean(1.0, 1.0, 1.5) + 4.0 / 9.0).abs() < 1e-6);
    }
}
