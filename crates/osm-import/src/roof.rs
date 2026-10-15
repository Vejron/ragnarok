//! Roofs. The laser returns over a footprint are fitted with a few parametric roof shapes (flat,
//! shed, gable, hip) and the best one becomes the building's walls and roof mesh. The mesh goes
//! into the tile's collision mesh, and the client draws buildings from the same triangles.

use crate::dem::sweref99tm;
use crate::lidar::Point;
use shared::world::{CollisionMesh, ROOF_FLAT, ROOF_GABLE, ROOF_HIP, ROOF_SHED};
use std::collections::HashMap;

type P = [f64; 2];

/// Smallest wall height under a roof, metres: a fit that dips below the floor still gets walls.
const MIN_WALL: f64 = 2.5;
/// Fewest laser returns that make a fit.
const MIN_POINTS: usize = 10;
/// Roof pitch (metres of rise per metre of run) a gable or hip may have: 9° to 50°.
const PITCH: (f64, f64) = (0.16, 1.2);
/// Below this slope (rise per run, about 3°) a single plane is called flat.
const FLAT_SLOPE: f64 = 0.05;
/// Steepest a shed roof may be (about 35°) and how far outside the measured heights a shape may
/// reach anywhere over its rectangle, metres.
const MAX_SHED: f64 = 0.7;
const REACH: f64 = 1.5;

/// A roof as a height field over the footprint. Heights are local y, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Flat { y: f64 },
    /// One sloping plane: `y` at `at`, rising by `slope` per metre along `[x, z]`.
    Shed { y: f64, at: P, slope: P },
    /// Two planes meeting at the ridge through `at` along the unit vector `ridge`, `pitch` apart.
    Gable { y: f64, at: P, ridge: P, pitch: f64 },
    /// Four planes: a gable whose ends are cut back; the ridge runs `half` metres either way of `at`.
    Hip { y: f64, at: P, ridge: P, pitch: f64, half: f64 },
}

impl Shape {
    pub fn kind(&self) -> u8 {
        match self {
            Shape::Flat { .. } => ROOF_FLAT,
            Shape::Shed { .. } => ROOF_SHED,
            Shape::Gable { .. } => ROOF_GABLE,
            Shape::Hip { .. } => ROOF_HIP,
        }
    }

    /// Roof height over `(x, z)`.
    pub fn height(&self, [x, z]: P) -> f64 {
        match *self {
            Shape::Flat { y } => y,
            Shape::Shed { y, at, slope } => y + slope[0] * (x - at[0]) + slope[1] * (z - at[1]),
            Shape::Gable { y, at, ridge, pitch } => {
                let across = -ridge[1] * (x - at[0]) + ridge[0] * (z - at[1]);
                y - pitch * across.abs()
            }
            Shape::Hip { y, at, ridge, pitch, half } => {
                let across = -ridge[1] * (x - at[0]) + ridge[0] * (z - at[1]);
                let along = ridge[0] * (x - at[0]) + ridge[1] * (z - at[1]);
                y - pitch * across.abs().max(along.abs() - half)
            }
        }
    }

    /// Lines `(point, direction)` where the roof bends. Between them it is a plane.
    fn creases(&self) -> Vec<(P, P)> {
        match *self {
            Shape::Flat { .. } | Shape::Shed { .. } => Vec::new(),
            Shape::Gable { at, ridge, .. } => vec![(at, ridge)],
            Shape::Hip { at, ridge, half, .. } => {
                let across = [-ridge[1], ridge[0]];
                let mut lines = vec![(at, ridge)];
                for end in [-half, half] {
                    let tip = [at[0] + ridge[0] * end, at[1] + ridge[1] * end];
                    for sign in [-1.0, 1.0] {
                        lines.push((tip, [ridge[0] + across[0] * sign, ridge[1] + across[1] * sign]));
                    }
                }
                lines
            }
        }
    }
}

/// A roof: one shape, or two roofs either side of a straight cut. `n` is the cut's unit normal;
/// what lies on the negative side of the plane through `o` is `low`.
#[derive(Clone, Debug, PartialEq)]
pub enum Roof {
    Leaf(Shape),
    Cut { o: P, n: P, low: Box<Roof>, high: Box<Roof> },
}

impl Roof {
    /// The most complex shape in the roof, as `ROOF_*`.
    pub fn kind(&self) -> u8 {
        match self {
            Roof::Leaf(shape) => shape.kind(),
            Roof::Cut { low, high, .. } => low.kind().max(high.kind()),
        }
    }

    #[cfg(test)]
    pub fn height(&self, p: P) -> f64 {
        match self {
            Roof::Leaf(shape) => shape.height(p),
            Roof::Cut { o, n, low, high } => {
                if n[0] * (p[0] - o[0]) + n[1] * (p[1] - o[1]) < 0.0 { low.height(p) } else { high.height(p) }
            }
        }
    }
}

/// One laser return of a building, local metres.
#[derive(Clone, Copy)]
struct Obs {
    u: f64,
    v: f64,
    y: f64,
}

/// Fits a roof to the laser returns `[x, z, y, code]` over `footprint` (`code` is return number
/// in the low nibble and number of returns in the high one, 0 if unknown), or `None` when there
/// are too few. Canopy is dropped first, then the roof is split into parts while that pays.
pub fn fit(footprint: &[P], points: &[[f64; 4]]) -> Option<Roof> {
    let area = polygon_area(footprint).abs();
    if points.len() < MIN_POINTS || (points.len() as f64) < 0.3 * area {
        return None;
    }
    let frame = Frame::around(footprint)?;
    let to_obs = |p: &[f64; 4]| {
        let (u, v) = frame.to_uv([p[0], p[1]]);
        Obs { u, v, y: p[2] }
    };
    // Foliage returns more than once; a roof is one return. Keep the single returns when enough are left.
    let singles: Vec<Obs> = points.iter().filter(|p| (p[3] as u8) >> 4 <= 1).map(to_obs).collect();
    let mut obs: Vec<Obs> = if singles.len() >= MIN_POINTS && singles.len() as f64 >= 0.3 * area { singles } else { points.iter().map(to_obs).collect() };
    // Whatever still stands well above the first fit is a tree, an antenna or a chimney stack.
    let (first, _) = model(&obs, &frame);
    let keep: Vec<Obs> = obs
        .iter()
        .copied()
        .filter(|o| {
            let at = [frame.centre[0] + frame.axes[0][0] * o.u + frame.axes[1][0] * o.v, frame.centre[1] + frame.axes[0][1] * o.u + frame.axes[1][1] * o.v];
            o.y - first.height(at) <= CANOPY
        })
        .collect();
    // A few stray returns are a tree, an antenna or a chimney. A big share standing above the fit is
    // a taller part of the building (a tower over its podium): keep it, and `tree` splits it off.
    if keep.len() >= MIN_POINTS && (obs.len() - keep.len()) as f64 <= MAX_STRAY * obs.len() as f64 {
        obs = keep;
    }
    Some(tree(&obs, &frame, MAX_DEPTH).0)
}

/// Fewest returns in a part of a roof.
const PART_POINTS: usize = 24;
/// Narrowest a part may be, metres.
const PART_WIDTH: f64 = 4.0;
/// A split has to cost at most this share of the unsplit fit, and save this much, to be taken.
const SPLIT_SHARE: f64 = 0.5;
const SPLIT_SAVING: f64 = 0.04;
/// Where the loss says there are levels metres apart (a tower on a podium), one cut cannot isolate
/// a part, so a cut that merely helps is taken; the next level cuts again.
const STEP_LOSS: f64 = 4.0;
const STEP_SHARE: f64 = 0.85;
/// Smallest rectangle, square metres, worth splitting: a part is at least a room.
const SPLIT_AREA: f64 = 70.0;
/// Parts of parts: a roof is at most `2^MAX_DEPTH` shapes.
const MAX_DEPTH: u32 = 3;
/// Largest miss, metres squared, that `loss` counts.
const BIG_MISS: f64 = 100.0;
/// Returns this far over the fitted roof, metres, are not roof.
const CANOPY: f64 = 1.0;
/// Most of the returns that may stand over the first fit and still be dropped as strays.
const MAX_STRAY: f64 = 0.15;

/// Mean squared miss of `shape` over `obs`, each clipped at `BIG_MISS` metres squared. Wide enough
/// that a step of ten metres between two levels is a large cost and a cut across it pays, where
/// `model`'s own cost (clipped at 1 m², so a stray return costs no more than a miss by a metre) cannot see it.
fn loss(obs: &[Obs], frame: &Frame, shape: &Shape) -> f64 {
    let at = |o: &Obs| [frame.centre[0] + frame.axes[0][0] * o.u + frame.axes[1][0] * o.v, frame.centre[1] + frame.axes[0][1] * o.u + frame.axes[1][1] * o.v];
    obs.iter().map(|o| (shape.height(at(o)) - o.y).powi(2).min(BIG_MISS)).sum::<f64>() / obs.len().max(1) as f64
}

/// The best roof for `obs` in `frame`: one shape, or a straight cut across the frame's rectangle
/// with a roof on each side, when that fits clearly better. Also returns the mean loss.
fn tree(obs: &[Obs], frame: &Frame, depth: u32) -> (Roof, f64) {
    let (shape, cost) = model(obs, frame);
    let wide = loss(obs, frame, &shape);
    let leaf = (Roof::Leaf(shape), wide);
    if depth == 0 || cost < 0.04 || 4.0 * frame.half[0] * frame.half[1] < SPLIT_AREA {
        return leaf;
    }
    // The cut that leaves the lowest `score` behind (mean over the returns, one shape per side).
    let cut_by = |score: &dyn Fn(&Part) -> f64, reach: i32, spacing: f64| -> Option<(f64, usize, f64)> {
        let mut best: Option<(f64, usize, f64)> = None;
        for axis in 0..2 {
            for step in -reach..=reach {
                let cut = f64::from(step) * spacing * frame.half[axis];
                if frame.half[axis] + cut < PART_WIDTH || frame.half[axis] - cut < PART_WIDTH {
                    continue;
                }
                let (low, high) = part_of(obs, frame, axis, cut);
                if low.0.len() < PART_POINTS || high.0.len() < PART_POINTS {
                    continue;
                }
                let total = (score(&low) * low.0.len() as f64 + score(&high) * high.0.len() as f64) / obs.len() as f64;
                if best.is_none_or(|b| total < b.0) {
                    best = Some((total, axis, cut));
                }
            }
        }
        best
    };
    // First by the robust cost, which sees small steps and shrugs off strays. Where that finds
    // nothing and the loss says there are levels metres apart (a tower on a podium), one cut cannot
    // isolate a part, so by the plain loss a cut that merely helps is taken; the next level cuts again.
    let mut chosen = cut_by(&|part| model(&part.0, &part.1).1, 4, 0.15).filter(|&(total, ..)| total <= cost * SPLIT_SHARE && cost - total >= SPLIT_SAVING);
    if chosen.is_none() && wide > STEP_LOSS {
        chosen = cut_by(&|part| loss(&part.0, &part.1, &model(&part.0, &part.1).0), 6, 0.12).filter(|&(total, ..)| total <= wide * STEP_SHARE);
    }
    let Some((_, axis, cut)) = chosen else { return leaf };
    let (low, high) = part_of(obs, frame, axis, cut);
    let (low_roof, low_cost) = tree(&low.0, &low.1, depth - 1);
    let (high_roof, high_cost) = tree(&high.0, &high.1, depth - 1);
    let total = (low_cost * low.0.len() as f64 + high_cost * high.0.len() as f64) / obs.len() as f64;
    let n = frame.axes[axis];
    let o = [frame.centre[0] + n[0] * cut, frame.centre[1] + n[1] * cut];
    (Roof::Cut { o, n, low: Box::new(low_roof), high: Box::new(high_roof) }, total)
}

type Part = (Vec<Obs>, Frame);

/// The returns on either side of `cut` along `axis` of `frame`, each laid out in the frame of its
/// own half of the rectangle (long side first, so `model` can assume it).
fn part_of(obs: &[Obs], frame: &Frame, axis: usize, cut: f64) -> (Part, Part) {
    let make = |lo: f64, hi: f64, take: &dyn Fn(f64) -> bool| -> Part {
        let mid = (lo + hi) / 2.0;
        let mut half = frame.half;
        half[axis] = (hi - lo) / 2.0;
        let shifted = obs.iter().filter(|o| take(if axis == 0 { o.u } else { o.v })).map(|o| if axis == 0 { Obs { u: o.u - mid, ..*o } } else { Obs { v: o.v - mid, ..*o } });
        let centre = [frame.centre[0] + frame.axes[axis][0] * mid, frame.centre[1] + frame.axes[axis][1] * mid];
        if half[0] >= half[1] {
            (shifted.collect(), Frame { centre, axes: frame.axes, half })
        } else {
            let axes = [frame.axes[1], [-frame.axes[0][1], frame.axes[0][0]]];
            (shifted.map(|o| Obs { u: o.v, v: o.u, y: o.y }).collect(), Frame { centre, axes, half: [half[1], half[0]] })
        }
    };
    let h = frame.half[axis];
    (make(-h, cut, &|c| c < cut), make(cut, h, &|c| c >= cut))
}

/// The best roof shape for returns `obs` laid out in `frame` (u along its long side), and its cost.
fn model(obs: &[Obs], frame: &Frame) -> (Shape, f64) {
    let swapped: Vec<Obs> = obs.iter().map(|o| Obs { u: o.v, v: o.u, y: o.y }).collect();
    let (hu, hv) = (frame.half[0], frame.half[1]);
    let (axis_u, axis_v) = (frame.axes[0], frame.axes[1]);
    let at = |across_axis: P, v0: f64| -> P { [frame.centre[0] + across_axis[0] * v0, frame.centre[1] + across_axis[1] * v0] };
    // A shape has to stay near what was measured over the whole rectangle, not just where there are
    // returns: a plane fitted to a small part would otherwise climb to the sky at its far edge.
    let mut ys: Vec<f64> = obs.iter().map(|o| o.y).collect();
    ys.sort_by(f64::total_cmp);
    let (lo, hi) = (ys[ys.len() / 50] - REACH, ys[ys.len() - 1 - ys.len() / 50] + REACH);
    let plausible = |shape: &Shape| {
        [(0.0, 0.0), (-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)].iter().all(|&(a, b)| {
            let p = [
                frame.centre[0] + axis_u[0] * hu * a + axis_v[0] * hv * b,
                frame.centre[1] + axis_u[1] * hu * a + axis_v[1] * hv * b,
            ];
            (lo..=hi).contains(&shape.height(p))
        })
    };

    // Flat: one level.
    let (flat, flat_cost) = solve(obs, |_| [1.0, 0.0, 0.0]);
    let mut best = (Shape::Flat { y: flat[0] }, flat_cost);

    // Shed: a plane.
    let (plane, plane_cost) = solve(obs, |o| [1.0, o.u, o.v]);
    let slope = [plane[1] * axis_u[0] + plane[2] * axis_v[0], plane[1] * axis_u[1] + plane[2] * axis_v[1]];
    let shed = Shape::Shed { y: plane[0], at: frame.centre, slope };
    if (FLAT_SLOPE..=MAX_SHED).contains(&slope[0].hypot(slope[1])) && plausible(&shed) && improves(plane_cost, best.1) {
        best = (shed, plane_cost);
    }

    // Gable, with the ridge along either axis of the footprint's rectangle.
    let mut gable: Option<(Shape, f64)> = None;
    for (set, extent, ridge_axis, across_axis) in [(obs, hv, axis_u, axis_v), (&swapped, hu, axis_v, axis_u)] {
        for step in -4..=4 {
            let v0 = f64::from(step) * extent * 0.12;
            let (c, cost) = solve(set, |o| [1.0, (o.v - v0).abs(), 0.0]);
            let pitch = -c[1];
            if !(PITCH.0..=PITCH.1).contains(&pitch) {
                continue;
            }
            if gable.as_ref().is_none_or(|g| cost < g.1) {
                let shape = Shape::Gable { y: c[0], at: at(across_axis, v0), ridge: ridge_axis, pitch };
                if plausible(&shape) {
                    gable = Some((shape, cost));
                }
            }
        }
    }
    if let Some((shape, cost)) = gable.filter(|&(_, cost)| improves(cost, best.1)) {
        best = (shape, cost);
    }

    // Hip: only along the long axis, where the ridge is what is left of it.
    let half = (hu - hv).max(0.0);
    let mut hip: Option<(Shape, f64)> = None;
    for step in -4..=4 {
        let v0 = f64::from(step) * hv * 0.12;
        let (c, cost) = solve(obs, |o| [1.0, (o.v - v0).abs().max(o.u.abs() - half), 0.0]);
        let pitch = -c[1];
        if !(PITCH.0..=PITCH.1).contains(&pitch) {
            continue;
        }
        if hip.as_ref().is_none_or(|h| cost < h.1) {
            let shape = Shape::Hip { y: c[0], at: at(axis_v, v0), ridge: axis_u, pitch, half };
            if plausible(&shape) {
                hip = Some((shape, cost));
            }
        }
    }
    if let Some((shape, cost)) = hip {
        // Against a gable only the ends differ, a small share of the returns, so the margin is small.
        let better = if matches!(best.0, Shape::Gable { .. }) { cost < best.1 * 0.9 - 0.002 } else { improves(cost, best.1) };
        if better {
            best = (shape, cost);
        }
    }
    best
}

/// A better model by a margin: noise alone must not turn a flat roof into a gable.
fn improves(cost: f64, over: f64) -> bool {
    cost < over * 0.7 - 0.005
}

/// Weighted least squares `y ≈ c · features`, re-weighted a few times so chimneys, antennae and
/// overhanging trees do not drag the fit. Returns the coefficients and the mean clipped squared
/// residual (clipped at 1 m², so one outlier costs no more than a miss by a metre).
fn solve(obs: &[Obs], features: impl Fn(&Obs) -> [f64; 3]) -> ([f64; 3], f64) {
    let mut weights = vec![1.0; obs.len()];
    let mut coefficients = [0.0; 3];
    for _ in 0..5 {
        let mut a = [[0.0; 3]; 3];
        let mut b = [0.0; 3];
        for (o, &w) in obs.iter().zip(&weights) {
            let f = features(o);
            for i in 0..3 {
                b[i] += w * f[i] * o.y;
                for j in 0..3 {
                    a[i][j] += w * f[i] * f[j];
                }
            }
        }
        for (i, row) in a.iter_mut().enumerate() {
            row[i] += 1e-9;
        }
        coefficients = solve3(a, b);
        for (o, w) in obs.iter().zip(weights.iter_mut()) {
            let f = features(o);
            let residual = o.y - (coefficients[0] * f[0] + coefficients[1] * f[1] + coefficients[2] * f[2]);
            *w = 1.0 / (1.0 + (residual / 0.4).powi(2));
        }
    }
    let cost = obs
        .iter()
        .map(|o| {
            let f = features(o);
            let residual = o.y - (coefficients[0] * f[0] + coefficients[1] * f[1] + coefficients[2] * f[2]);
            (residual * residual).min(1.0)
        })
        .sum::<f64>()
        / obs.len() as f64;
    (coefficients, cost)
}

/// Gauss–Jordan on a 3×3 system.
fn solve3(mut a: [[f64; 3]; 3], mut b: [f64; 3]) -> [f64; 3] {
    for col in 0..3 {
        let pivot = (col..3).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs())).unwrap();
        a.swap(col, pivot);
        b.swap(col, pivot);
        let d = a[col][col];
        if d.abs() < 1e-12 {
            continue;
        }
        for row in 0..3 {
            if row == col {
                continue;
            }
            let k = a[row][col] / d;
            let pivot_row = a[col];
            for (value, pivot) in a[row].iter_mut().zip(pivot_row).skip(col) {
                *value -= k * pivot;
            }
            b[row] -= k * b[col];
        }
    }
    [0, 1, 2].map(|i| if a[i][i].abs() < 1e-12 { 0.0 } else { b[i] / a[i][i] })
}

/// The footprint's smallest enclosing rectangle: centre, unit axes (long first) and half extents.
struct Frame {
    centre: P,
    axes: [P; 2],
    half: [f64; 2],
}

impl Frame {
    fn around(polygon: &[P]) -> Option<Self> {
        let mut best: Option<(f64, Frame)> = None;
        for i in 0..polygon.len() {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            let length = (b[0] - a[0]).hypot(b[1] - a[1]);
            if length < 0.5 {
                continue;
            }
            let u = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
            let v = [-u[1], u[0]];
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for p in polygon {
                for (k, axis) in [u, v].iter().enumerate() {
                    let s = p[0] * axis[0] + p[1] * axis[1];
                    lo[k] = lo[k].min(s);
                    hi[k] = hi[k].max(s);
                }
            }
            let area = (hi[0] - lo[0]) * (hi[1] - lo[1]);
            if best.as_ref().is_none_or(|(known, _)| area < *known) {
                let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
                let half = [(hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0];
                let centre = [u[0] * mid[0] + v[0] * mid[1], u[1] * mid[0] + v[1] * mid[1]];
                let frame = if half[0] >= half[1] { Frame { centre, axes: [u, v], half } } else { Frame { centre, axes: [v, [-v[1], v[0]]], half: [half[1], half[0]] } };
                best = Some((area, frame));
            }
        }
        best.map(|(_, frame)| frame)
    }

    fn to_uv(&self, [x, z]: P) -> (f64, f64) {
        let (dx, dz) = (x - self.centre[0], z - self.centre[1]);
        (dx * self.axes[0][0] + dz * self.axes[0][1], dx * self.axes[1][0] + dz * self.axes[1][1])
    }
}

fn polygon_area(polygon: &[P]) -> f64 {
    (0..polygon.len()).map(|i| {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0
}

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
}

/// How many triangles of a building's mesh are wall and roof; its floor follows them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Parts {
    pub walls: u32,
    pub roof: u32,
    /// Highest point of the roof, metres.
    pub top: f64,
}

/// Splits a convex polygon by the line through `o` along `d`, into the pieces on each side.
fn split(polygon: &[P], o: P, d: P) -> (Vec<P>, Vec<P>) {
    let side = |p: P| d[0] * (p[1] - o[1]) - d[1] * (p[0] - o[0]);
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            left.push(a);
        }
        if sa <= 0.0 {
            right.push(a);
        }
        if (sa > 0.0 && sb < 0.0) || (sa < 0.0 && sb > 0.0) {
            let t = sa / (sa - sb);
            let cut = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            left.push(cut);
            right.push(cut);
        }
    }
    (left, right)
}

/// A piece of the footprint under one shape of the roof.
struct Piece<'a> {
    polygon: Vec<P>,
    shape: &'a Shape,
}

/// Cuts a convex `polygon` along the roof's cuts and then its shapes' creases, into planar pieces,
/// and collects where two parts of the roof meet inside it as `(a, b, low, high)`.
fn pieces<'a>(roof: &'a Roof, polygon: Vec<P>, out: &mut Vec<Piece<'a>>, seams: &mut Vec<(P, P, &'a Roof, &'a Roof)>) {
    match roof {
        Roof::Leaf(shape) => {
            let mut parts = vec![polygon];
            for (o, d) in shape.creases() {
                parts = parts
                    .into_iter()
                    .flat_map(|piece| {
                        let (left, right) = split(&piece, o, d);
                        [left, right]
                    })
                    .filter(|piece| piece.len() >= 3)
                    .collect();
            }
            out.extend(parts.into_iter().map(|polygon| Piece { polygon, shape }));
        }
        Roof::Cut { o, n, low, high } => {
            // `split` takes a line by a direction; its "left" is where `n · (p - o)` is not positive.
            let d = [-n[1], n[0]];
            let side = |p: P| n[0] * (p[0] - o[0]) + n[1] * (p[1] - o[1]);
            let (mut s0, mut s1): (Option<P>, Option<P>) = (None, None);
            let along = |p: P| d[0] * p[0] + d[1] * p[1];
            let mut note = |p: P| {
                if s0.is_none_or(|q| along(p) < along(q)) {
                    s0 = Some(p);
                }
                if s1.is_none_or(|q| along(p) > along(q)) {
                    s1 = Some(p);
                }
            };
            for i in 0..polygon.len() {
                let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
                let (sa, sb) = (side(a), side(b));
                if sa == 0.0 {
                    note(a);
                }
                if (sa < 0.0 && sb > 0.0) || (sa > 0.0 && sb < 0.0) {
                    let t = sa / (sa - sb);
                    note([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                }
            }
            let (left, right) = split(&polygon, *o, d);
            if let (Some(a), Some(b), true) = (s0, s1, left.len() >= 3 && right.len() >= 3 && s0 != s1) {
                seams.push((a, b, low, high));
            }
            if left.len() >= 3 {
                pieces(low, left, out, seams);
            }
            if right.len() >= 3 {
                pieces(high, right, out, seams);
            }
        }
    }
}

/// The shape of the roof over `p`.
fn leaf_at(roof: &Roof, p: P) -> &Shape {
    match roof {
        Roof::Leaf(shape) => shape,
        Roof::Cut { o, n, low, high } => {
            if n[0] * (p[0] - o[0]) + n[1] * (p[1] - o[1]) < 0.0 { leaf_at(low, p) } else { leaf_at(high, p) }
        }
    }
}

/// Every line where the roof bends or changes shape: creases and cuts, as `(point, direction)`.
fn bends(roof: &Roof, out: &mut Vec<(P, P)>) {
    match roof {
        Roof::Leaf(shape) => out.extend(shape.creases()),
        Roof::Cut { o, n, low, high } => {
            out.push((*o, [-n[1], n[0]]));
            bends(low, out);
            bends(high, out);
        }
    }
}

/// Splits the edge `a → b` where the roof changes shape or bends, as `(from, to, shape)` runs.
fn edge_runs<'a>(roof: &'a Roof, a: P, b: P, out: &mut Vec<(P, P, &'a Shape)>) {
    match roof {
        Roof::Leaf(shape) => {
            let mut ts = vec![0.0, 1.0];
            for (o, d) in shape.creases() {
                let (sa, sb) = (d[0] * (a[1] - o[1]) - d[1] * (a[0] - o[0]), d[0] * (b[1] - o[1]) - d[1] * (b[0] - o[0]));
                if sa * sb < 0.0 {
                    ts.push(sa / (sa - sb));
                }
            }
            ts.sort_by(f64::total_cmp);
            let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            for pair in ts.windows(2) {
                if pair[1] > pair[0] {
                    out.push((at(pair[0]), at(pair[1]), shape));
                }
            }
        }
        Roof::Cut { o, n, low, high } => {
            let (sa, sb) = (n[0] * (a[0] - o[0]) + n[1] * (a[1] - o[1]), n[0] * (b[0] - o[0]) + n[1] * (b[1] - o[1]));
            if sa < 0.0 && sb <= 0.0 {
                edge_runs(low, a, b, out);
            } else if sa >= 0.0 && sb >= 0.0 {
                edge_runs(high, a, b, out);
            } else {
                let t = sa / (sa - sb);
                let m = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                let (first, second) = if sa < 0.0 { (low, high) } else { (high, low) };
                edge_runs(first, a, m, out);
                edge_runs(second, m, b, out);
            }
        }
    }
}

/// Appends a building to `mesh`: walls up to the roof (and steps where parts of the roof differ in
/// height), the roof, then the floor, in that order. `footprint` is counter-clockwise from above.
/// The floor is at `floor` metres.
pub fn build(mesh: &mut CollisionMesh, footprint: &[P], floor: f64, roof: &Roof) -> Parts {
    let mut ids: HashMap<[i32; 3], u32> = HashMap::new();
    let mut vertex = |mesh: &mut CollisionMesh, x: f64, y: f64, z: f64| -> u32 {
        let key = [cm(x), cm(y), cm(z)];
        *ids.entry(key).or_insert_with(|| {
            mesh.vertices.push(key);
            mesh.vertices.len() as u32 - 1
        })
    };
    let clamp = |y: f64| y.max(floor + MIN_WALL);
    let mut highest = floor;
    let push = |mesh: &mut CollisionMesh, tri: [u32; 3]| {
        if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
            mesh.triangles.push(tri);
        }
    };

    let flat: Vec<f64> = footprint.iter().flat_map(|p| [p[0], p[1]]).collect();
    let triangles: Vec<[usize; 3]> = earcutr::earcut(&flat, &[], 2).map(|i| i.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect()).unwrap_or_default();
    let mut roof_pieces = Vec::new();
    let mut seams = Vec::new();
    for [a, b, c] in &triangles {
        pieces(roof, vec![footprint[*a], footprint[*b], footprint[*c]], &mut roof_pieces, &mut seams);
    }

    // Walls: each edge is cut where the roof changes, so the top edge follows it.
    let first_wall = mesh.triangles.len();
    for i in 0..footprint.len() {
        let (a, b) = (footprint[i], footprint[(i + 1) % footprint.len()]);
        let mut runs = Vec::new();
        edge_runs(roof, a, b, &mut runs);
        for (p, q, shape) in runs {
            let (tp, tq) = (clamp(shape.height(p)), clamp(shape.height(q)));
            highest = highest.max(tp).max(tq);
            let (bp, bq) = (vertex(mesh, p[0], floor, p[1]), vertex(mesh, q[0], floor, q[1]));
            let (up, uq) = (vertex(mesh, p[0], tp, p[1]), vertex(mesh, q[0], tq, q[1]));
            push(mesh, [bp, bq, uq]);
            push(mesh, [bp, uq, up]);
        }
    }
    // Where two parts of the roof meet at different heights there is a step, a wall seen from
    // either side.
    for (a, b, low, high) in seams {
        // Either side may bend along the seam, so the step is cut wherever a crease or a cut crosses it.
        let mut lines = Vec::new();
        bends(low, &mut lines);
        bends(high, &mut lines);
        let mut ts = vec![0.0, 1.0];
        for (o, d) in lines {
            let (sa, sb) = (d[0] * (a[1] - o[1]) - d[1] * (a[0] - o[0]), d[0] * (b[1] - o[1]) - d[1] * (b[0] - o[0]));
            if sa * sb < 0.0 {
                ts.push(sa / (sa - sb));
            }
        }
        ts.sort_by(f64::total_cmp);
        let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        for pair in ts.windows(2) {
            if pair[1] - pair[0] < 1e-6 {
                continue;
            }
            let (p, q, mid) = (at(pair[0]), at(pair[1]), at((pair[0] + pair[1]) / 2.0));
            let (lo, hi) = (leaf_at(low, mid), leaf_at(high, mid));
            let (l0, l1, h0, h1) = (clamp(lo.height(p)), clamp(lo.height(q)), clamp(hi.height(p)), clamp(hi.height(q)));
            if (l0 - h0).abs() < 0.02 && (l1 - h1).abs() < 0.02 {
                continue;
            }
            let v = [vertex(mesh, p[0], l0, p[1]), vertex(mesh, q[0], l1, q[1]), vertex(mesh, q[0], h1, q[1]), vertex(mesh, p[0], h0, p[1])];
            for tri in [[v[0], v[1], v[2]], [v[0], v[2], v[3]], [v[2], v[1], v[0]], [v[3], v[2], v[0]]] {
                push(mesh, tri);
            }
        }
    }
    let walls = (mesh.triangles.len() - first_wall) as u32;

    // Roof: each piece is a plane, fan-triangulated.
    let first_roof = mesh.triangles.len();
    for Piece { polygon, shape } in &roof_pieces {
        let ids: Vec<u32> = polygon
            .iter()
            .map(|&[x, z]| {
                let y = clamp(shape.height([x, z]));
                highest = highest.max(y);
                vertex(mesh, x, y, z)
            })
            .collect();
        for k in 1..ids.len() - 1 {
            push(mesh, [ids[0], ids[k], ids[k + 1]]);
        }
    }
    let roof_count = (mesh.triangles.len() - first_roof) as u32;

    // Floor, facing down.
    for [a, b, c] in &triangles {
        let ids = [*c, *b, *a].map(|k| vertex(mesh, footprint[k][0], floor, footprint[k][1]));
        push(mesh, ids);
    }
    Parts { walls, roof: roof_count, top: highest }
}

/// The returns of a building's footprint, found in a grid of every laser return of the world
/// that is not ground.
pub struct Cloud {
    e0: f64,
    n0: f64,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<[f32; 4]>>,
}

const CELL: f64 = 25.0;

impl Cloud {
    /// An empty grid over `[e0, n0, e1, n1]`.
    pub fn new([e0, n0, e1, n1]: [f64; 4]) -> Self {
        let (cols, rows) = (((e1 - e0) / CELL).ceil() as usize + 1, ((n1 - n0) / CELL).ceil() as usize + 1);
        Self { e0, n0, cols, rows, cells: vec![Vec::new(); cols * rows] }
    }

    /// Keeps a return if it can be part of a roof; `datum` is subtracted from heights, so they
    /// match the world's local y.
    pub fn add(&mut self, p: &Point, datum: f64) {
        // Ground, low and high noise are never roof.
        if matches!(p.class, 2 | 7 | 18) {
            return;
        }
        let (c, r) = (((p.e - self.e0) / CELL).floor(), ((p.n - self.n0) / CELL).floor());
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return;
        }
        self.cells[r as usize * self.cols + c as usize].push([(p.e - self.e0) as f32, (p.n - self.n0) as f32, (p.z - datum) as f32, f32::from(p.ret | p.returns << 4)]);
    }

    pub fn len(&self) -> usize {
        self.cells.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(Vec::is_empty)
    }

    /// The returns inside `footprint`, a local-metre polygon, as `[x, z, y]`. `origin` is the world
    /// frame's origin in degrees (lat, lon) and `unproject` maps local metres to degrees.
    pub fn over(&self, footprint: &[P], unproject: impl Fn(f64, f64) -> (f64, f64)) -> Vec<[f64; 4]> {
        let n = footprint.len() as f64;
        let centre = footprint.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
        let grid = |x: f64, z: f64| {
            let (lat, lon) = unproject(x, z);
            sweref99tm(lat, lon)
        };
        // The world's local frame is turned a few degrees from the grid; a linear map is exact enough
        // over one building.
        let o = grid(centre[0], centre[1]);
        let ex = grid(centre[0] + 10.0, centre[1]);
        let ez = grid(centre[0], centre[1] + 10.0);
        let m = [[(ex.0 - o.0) / 10.0, (ez.0 - o.0) / 10.0], [(ex.1 - o.1) / 10.0, (ez.1 - o.1) / 10.0]];
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if det.abs() < 1e-9 {
            return Vec::new();
        }
        let forward = |p: P| (o.0 + m[0][0] * (p[0] - centre[0]) + m[0][1] * (p[1] - centre[1]), o.1 + m[1][0] * (p[0] - centre[0]) + m[1][1] * (p[1] - centre[1]));
        let ring: Vec<P> = footprint.iter().map(|&p| { let (e, n) = forward(p); [e, n] }).collect();
        let (lo, hi) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let (c0, c1) = (((lo[0] - self.e0) / CELL).floor().max(0.0) as usize, (((hi[0] - self.e0) / CELL).floor().max(0.0) as usize).min(self.cols - 1));
        let (r0, r1) = (((lo[1] - self.n0) / CELL).floor().max(0.0) as usize, (((hi[1] - self.n0) / CELL).floor().max(0.0) as usize).min(self.rows - 1));
        let mut out = Vec::new();
        for r in r0..=r1 {
            for c in c0..=c1 {
                for &[e, n, y, code] in &self.cells[r * self.cols + c] {
                    let (e, n) = (f64::from(e) + self.e0, f64::from(n) + self.n0);
                    if !inside([e, n], &ring) {
                        continue;
                    }
                    let (de, dn) = (e - o.0, n - o.1);
                    let x = centre[0] + (m[1][1] * de - m[0][1] * dn) / det;
                    let z = centre[1] + (-m[1][0] * de + m[0][0] * dn) / det;
                    out.push([x, z, f64::from(y), f64::from(code)]);
                }
            }
        }
        out
    }
}

fn inside([x, z]: P, ring: &[P]) -> bool {
    let mut hit = false;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            hit = !hit;
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `w × d` rectangle turned by `angle`, counter-clockwise from above.
    fn rectangle(w: f64, d: f64, angle: f64, at: P) -> Vec<P> {
        let (s, c) = angle.sin_cos();
        [[-w / 2.0, -d / 2.0], [w / 2.0, -d / 2.0], [w / 2.0, d / 2.0], [-w / 2.0, d / 2.0]].iter().map(|p| [at[0] + p[0] * c - p[1] * s, at[1] + p[0] * s + p[1] * c]).collect()
    }

    /// Laser returns every 0.7 m over the footprint's box, heights from `roof`, with a little noise.
    fn sample(footprint: &[P], roof: &Shape) -> Vec<[f64; 4]> {
        let (lo, hi) = footprint.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let mut out = Vec::new();
        let mut k = 0u32;
        let mut x = lo[0];
        while x < hi[0] {
            let mut z = lo[1];
            while z < hi[1] {
                if inside([x, z], footprint) {
                    k += 1;
                    let noise = ((u64::from(k) * 2654435761) % 1000) as f64 / 1000.0 * 0.2 - 0.1;
                    out.push([x, z, roof.height([x, z]) + noise, 0.0]);
                }
                z += 0.7;
            }
            x += 0.7;
        }
        out
    }

    fn leaf(roof: Option<Roof>) -> Shape {
        match roof {
            Some(Roof::Leaf(shape)) => shape,
            other => panic!("expected one shape, got {other:?}"),
        }
    }

    #[test]
    fn a_gable_roof_is_found_along_the_long_side() {
        let footprint = rectangle(14.0, 8.0, 0.5, [30.0, -12.0]);
        let ridge = [0.5f64.cos(), 0.5f64.sin()];
        let truth = Shape::Gable { y: 10.0, at: [30.0, -12.0], ridge, pitch: 0.7 };
        let Shape::Gable { y, ridge: found, pitch, .. } = leaf(fit(&footprint, &sample(&footprint, &truth))) else { panic!("not a gable") };
        assert!((y - 10.0).abs() < 0.2, "ridge at {y}");
        assert!((pitch - 0.7).abs() < 0.08, "pitch {pitch}");
        assert!((found[0] * ridge[0] + found[1] * ridge[1]).abs() > 0.99, "ridge direction {found:?}");
    }

    #[test]
    fn hip_shed_and_flat_roofs_are_told_apart() {
        let footprint = rectangle(16.0, 9.0, -0.3, [0.0, 0.0]);
        let ridge = [(-0.3f64).cos(), (-0.3f64).sin()];
        let hip = Shape::Hip { y: 9.0, at: [0.0, 0.0], ridge, pitch: 0.6, half: 3.5 };
        assert!(matches!(leaf(fit(&footprint, &sample(&footprint, &hip))), Shape::Hip { .. }));
        let shed = Shape::Shed { y: 7.0, at: [0.0, 0.0], slope: [0.0, 0.25] };
        assert!(matches!(leaf(fit(&footprint, &sample(&footprint, &shed))), Shape::Shed { .. }));
        let flat = Shape::Flat { y: 6.0 };
        assert!(matches!(leaf(fit(&footprint, &sample(&footprint, &flat))), Shape::Flat { .. }));
    }

    #[test]
    fn too_few_returns_make_no_fit() {
        let footprint = rectangle(10.0, 10.0, 0.0, [0.0, 0.0]);
        assert_eq!(fit(&footprint, &[[0.0, 0.0, 5.0, 0.0]; 3]), None);
    }

    #[test]
    fn canopy_over_a_roof_is_not_the_roof() {
        let footprint = rectangle(14.0, 8.0, 0.0, [0.0, 0.0]);
        let truth = Shape::Gable { y: 9.0, at: [0.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6 };
        let mut points = sample(&footprint, &truth);
        // A tree over one end: many returns, 7 m up, some of them with a single return.
        for (i, p) in points.iter_mut().enumerate() {
            if p[0] > 2.0 {
                p[2] += 7.0;
                p[3] = if i % 5 == 0 { 0.0 } else { f64::from(1 | 3 << 4) };
            }
        }
        let Shape::Gable { y, .. } = leaf(fit(&footprint, &points)) else { panic!("not a gable") };
        assert!(y < 10.0, "the tree pulled the ridge to {y}");
    }

    #[test]
    fn a_tower_over_a_podium_is_not_thrown_away_as_canopy() {
        // 40 × 20: a flat podium 9 m up, a tower 36 m up over a third of it, all single returns.
        let footprint = rectangle(40.0, 20.0, 0.0, [0.0, 0.0]);
        let mut points = sample(&footprint, &Shape::Flat { y: 9.0 });
        for p in &mut points {
            if p[0] > 6.0 {
                p[2] = 36.0;
            }
        }
        let roof = fit(&footprint, &points).unwrap();
        assert!((roof.height([12.0, 0.0]) - 36.0).abs() < 1.0, "the tower is {} m", roof.height([12.0, 0.0]));
        assert!((roof.height([-12.0, 0.0]) - 9.0).abs() < 1.0);
    }

    #[test]
    fn a_house_with_a_lower_flat_wing_is_two_roofs() {
        // 20 × 8: a gable over the west half, a flat roof 3 m lower over the east half.
        let footprint = rectangle(20.0, 8.0, 0.0, [0.0, 0.0]);
        let gable = Shape::Gable { y: 10.0, at: [-5.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6 };
        let flat = Shape::Flat { y: 4.0 };
        let mut points = sample(&footprint, &gable);
        for p in &mut points {
            if p[0] >= 0.0 {
                p[2] = flat.height([p[0], p[1]]) + (p[2] % 0.1);
            }
        }
        let Some(Roof::Cut { o, n, low, high }) = fit(&footprint, &points) else { panic!("one shape") };
        assert!(n[0].abs() > 0.99 && o[0].abs() < 2.0, "cut at {o:?} normal {n:?}");
        let (west, east) = if n[0] > 0.0 { (*low, *high) } else { (*high, *low) };
        assert!((west.height([-5.0, 0.0]) - 10.0).abs() < 0.4, "{west:?}");
        assert!((east.height([5.0, 0.0]) - 4.0).abs() < 0.4, "{east:?}");
    }

    #[test]
    fn the_roof_mesh_covers_the_footprint_and_rises_to_the_ridge() {
        for shape in [
            Shape::Flat { y: 6.0 },
            Shape::Gable { y: 9.0, at: [0.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6 },
            Shape::Hip { y: 9.0, at: [0.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6, half: 3.0 },
        ] {
            let footprint = rectangle(12.0, 6.0, 0.0, [0.0, 0.0]);
            let mut mesh = CollisionMesh::default();
            let parts = build(&mut mesh, &footprint, 1.0, &Roof::Leaf(shape));
            assert!((roof_area(&mesh, parts) - 72.0).abs() < 0.05, "{shape:?}: roof covers {} m²", roof_area(&mesh, parts));
            let expect = if matches!(shape, Shape::Flat { .. }) { 6.0 } else { 9.0 };
            assert!((parts.top - expect).abs() < 0.01, "{shape:?}: top {}", parts.top);
        }
    }

    /// The footprint area the roof triangles cover seen from above, signed so a hole shows.
    fn roof_area(mesh: &CollisionMesh, parts: Parts) -> f64 {
        let mut area = 0.0;
        for tri in &mesh.triangles[parts.walls as usize..(parts.walls + parts.roof) as usize] {
            let p = tri.map(|i| mesh.vertices[i as usize]);
            let cross = f64::from(p[1][0] - p[0][0]) * f64::from(p[2][2] - p[0][2]) - f64::from(p[2][0] - p[0][0]) * f64::from(p[1][2] - p[0][2]);
            area += cross / 2.0 / 1e4;
        }
        area
    }

    #[test]
    fn two_roofs_meet_in_a_step_wall_and_still_cover_the_footprint() {
        let footprint = rectangle(12.0, 6.0, 0.0, [0.0, 0.0]);
        let west = Roof::Leaf(Shape::Gable { y: 9.0, at: [-3.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6 });
        let east = Roof::Leaf(Shape::Flat { y: 5.0 });
        let cut = Roof::Cut { o: [0.0, 0.0], n: [1.0, 0.0], low: Box::new(west), high: Box::new(east) };
        let mut stepped = CollisionMesh::default();
        let parts = build(&mut stepped, &footprint, 0.0, &cut);
        assert!((roof_area(&stepped, parts) - 72.0).abs() < 0.05, "covers {}", roof_area(&stepped, parts));
        let mut plain = CollisionMesh::default();
        let single = build(&mut plain, &footprint, 0.0, &Roof::Leaf(Shape::Flat { y: 5.0 }));
        assert!(parts.walls > single.walls, "no step wall: {} vs {}", parts.walls, single.walls);
        assert!((parts.top - 9.0).abs() < 0.01);
    }

    #[test]
    fn a_concave_footprint_is_triangulated_too() {
        let l = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 4.0], [4.0, 4.0], [4.0, 10.0], [0.0, 10.0]];
        let mut mesh = CollisionMesh::default();
        let shape = Shape::Gable { y: 8.0, at: [2.0, 2.0], ridge: [1.0, 0.0], pitch: 0.5 };
        let parts = build(&mut mesh, &l, 0.0, &Roof::Leaf(shape));
        assert!(parts.roof >= 4 && parts.walls >= 12);
    }
}
