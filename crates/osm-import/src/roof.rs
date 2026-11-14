//! Roofs. The laser returns over a footprint are fitted with a few parametric roof shapes (flat,
//! shed, gable, hip) and the best one becomes the building's walls and roof mesh. The mesh goes
//! into the tile's collision mesh, and the client draws buildings from the same triangles.

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
    /// A house from the catalogue: wings, valleys and dormers (`House`).
    House(Box<House>),
}

impl Roof {
    /// The most complex shape in the roof, as `ROOF_*`.
    pub fn kind(&self) -> u8 {
        match self {
            Roof::Leaf(shape) => shape.kind(),
            Roof::Cut { low, high, .. } => low.kind().max(high.kind()),
            Roof::House(house) => if house.pitch == Pitch::Hip { ROOF_HIP } else { ROOF_GABLE },
        }
    }

    #[cfg(test)]
    pub fn height(&self, p: P) -> f64 {
        match self {
            Roof::Leaf(shape) => shape.height(p),
            Roof::Cut { o, n, low, high } => {
                if n[0] * (p[0] - o[0]) + n[1] * (p[1] - o[1]) < 0.0 { low.height(p) } else { high.height(p) }
            }
            Roof::House(house) => house.height(p),
        }
    }
}

/// What a building's roof is made of: a shape fitted to the returns, or the returns themselves.
#[derive(Clone, Debug, PartialEq)]
pub enum Top {
    Fitted(Roof),
    Measured(crate::field::Field),
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
    let frame = Frame::around(footprint)?;
    let obs: Vec<Obs> = roof_returns(footprint, points)?
        .0
        .iter()
        .map(|r| {
            let (u, v) = frame.to_uv([r[0], r[1]]);
            Obs { u, v, y: r[2] }
        })
        .collect();
    // A house is one roof; only a big building is fitted in parts.
    Some(tree(&obs, &frame, if area >= SPLIT_FROM { MAX_DEPTH } else { 0 }).0)
}

/// Laser returns `[x, z, y]`, local metres.
pub(crate) type Returns = Vec<[f64; 3]>;

/// The laser returns `[x, z, y, code]` over `footprint` that are its roof, as `[x, z, y]`: foliage
/// and what stands well over a first fit (a tree, an antenna, a chimney) left out. Also the returns
/// before what stood over the fit was left out, in which a dormer still stands. `None` when there
/// are too few to fit a roof to.
pub(crate) fn roof_returns(footprint: &[P], points: &[[f64; 4]]) -> Option<(Returns, Returns)> {
    let area = polygon_area(footprint).abs();
    if points.len() < MIN_POINTS || (points.len() as f64) < 0.3 * area {
        return None;
    }
    let frame = Frame::around(footprint)?;
    let to_obs = |p: &[f64; 4]| {
        let (u, v) = frame.to_uv([p[0], p[1]]);
        Obs { u, v, y: p[2] }
    };
    // Foliage returns more than once; a roof is one return. Keep the single returns when enough are
    // left, else the last ones: under a crown, what a pulse hits last is the roof.
    let enough = |obs: &Vec<Obs>| obs.len() >= MIN_POINTS && obs.len() as f64 >= 0.3 * area;
    let singles: Vec<Obs> = points.iter().filter(|p| (p[3] as u8) >> 4 <= 1).map(to_obs).collect();
    let lasts = || -> Vec<Obs> { points.iter().filter(|p| (p[3] as u8) & 0x0f == (p[3] as u8) >> 4).map(to_obs).collect() };
    let mut obs: Vec<Obs> = if enough(&singles) {
        singles
    } else {
        Some(lasts()).filter(enough).unwrap_or_else(|| points.iter().map(to_obs).collect())
    };
    let world = |o: &Obs| [frame.centre[0] + frame.axes[0][0] * o.u + frame.axes[1][0] * o.v, frame.centre[1] + frame.axes[0][1] * o.u + frame.axes[1][1] * o.v];
    // Whatever still stands well above the first fit is a tree, an antenna or a chimney stack.
    let (first, _) = model(&obs, &frame);
    let keep: Vec<Obs> = obs.iter().copied().filter(|o| o.y - first.height(world(o)) <= CANOPY).collect();
    let all: Vec<[f64; 3]> = obs.iter().map(|o| { let p = world(o); [p[0], p[1], o.y] }).collect();
    // A few stray returns are a tree, an antenna or a chimney. A big share standing above the fit is
    // a taller part of the building (a tower over its podium): keep it, and `tree` splits it off.
    if keep.len() >= MIN_POINTS && (obs.len() - keep.len()) as f64 <= MAX_STRAY * obs.len() as f64 {
        obs = keep;
    }
    Some((obs.iter().map(|o| { let p = world(o); [p[0], p[1], o.y] }).collect(), all))
}

/// Smallest footprint, square metres, whose roof may be fitted in parts (`tree`): a house is one
/// roof, from the catalogue (`catalogue`), however its laser returns scatter.
pub const SPLIT_FROM: f64 = 250.0;

/// Half the short side of the smallest rectangle around `footprint`, metres.
pub fn half_width(footprint: &[P]) -> Option<f64> {
    Frame::around(footprint).map(|frame| frame.half[1])
}

/// The smallest rectangle around `footprint` as a wing: its middle, its long side's direction and
/// its half sides, long first.
pub fn rectangle(footprint: &[P]) -> Option<Wing> {
    Frame::around(footprint).map(|frame| Wing { centre: frame.centre, u: frame.axes[0], half: frame.half })
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

    // Gable, with the ridge along either axis of the footprint's rectangle, down its middle.
    let mut gable: Option<(Shape, f64)> = None;
    for (set, ridge_axis, across_axis) in [(obs, axis_u, axis_v), (&swapped, axis_v, axis_u)] {
        let (c, cost) = solve(set, |o| [1.0, o.v.abs(), 0.0]);
        let pitch = -c[1];
        if !(PITCH.0..=PITCH.1).contains(&pitch) {
            continue;
        }
        if gable.as_ref().is_none_or(|g| cost < g.1) {
            let shape = Shape::Gable { y: c[0], at: at(across_axis, 0.0), ridge: ridge_axis, pitch };
            if plausible(&shape) {
                gable = Some((shape, cost));
            }
        }
    }
    if let Some((shape, cost)) = gable.filter(|&(_, cost)| improves(cost, best.1)) {
        best = (shape, cost);
    }

    // Hip: only along the long axis, where the ridge is what is left of it, down the middle.
    let half = (hu - hv).max(0.0);
    let mut hip: Option<(Shape, f64)> = None;
    let (c, cost) = solve(obs, |o| [1.0, o.v.abs().max(o.u.abs() - half), 0.0]);
    let pitch = -c[1];
    if (PITCH.0..=PITCH.1).contains(&pitch) {
        let shape = Shape::Hip { y: c[0], at: at(axis_v, 0.0), ridge: axis_u, pitch, half };
        if plausible(&shape) {
            hip = Some((shape, cost));
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
    let rows: Vec<[f64; 3]> = obs.iter().map(&features).collect();
    let ys: Vec<f64> = obs.iter().map(|o| o.y).collect();
    solve_rows(&rows, &ys)
}

/// `solve` over rows of features already worked out, one for each height in `ys`.
pub(crate) fn solve_rows(rows: &[[f64; 3]], ys: &[f64]) -> ([f64; 3], f64) {
    let obs: Vec<Obs> = ys.iter().enumerate().map(|(i, &y)| Obs { u: i as f64, v: 0.0, y }).collect();
    let features = |o: &Obs| rows[o.u as usize];
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

/// A plane over the ground: height `c[0] + c[1] x + c[2] z`, metres.
pub type Plane = [f64; 3];

/// Height of `plane` over `p`.
pub fn eval(plane: &Plane, p: P) -> f64 {
    plane[0] + plane[1] * p[0] + plane[2] * p[1]
}

/// The plane `A + B a + C b` laid out in the frame at `at` with `a` along the unit `u` and `b` along
/// `[-u[1], u[0]]`, as a plane over the ground.
pub fn local_plane(at: P, u: P, [a, b, c]: [f64; 3]) -> Plane {
    let v = [-u[1], u[0]];
    let (cx, cz) = (b * u[0] + c * v[0], b * u[1] + c * v[1]);
    [a - cx * at[0] - cz * at[1], cx, cz]
}

impl Shape {
    /// The plane the roof lies in over `p`: on a crease, either of the two that meet there.
    pub fn plane_at(&self, p: P) -> Plane {
        match *self {
            Shape::Flat { y } => [y, 0.0, 0.0],
            Shape::Shed { y, at, slope } => [y - slope[0] * at[0] - slope[1] * at[1], slope[0], slope[1]],
            Shape::Gable { y, at, ridge, pitch } => {
                let across = -ridge[1] * (p[0] - at[0]) + ridge[0] * (p[1] - at[1]);
                local_plane(at, ridge, [y, 0.0, -pitch * sign(across)])
            }
            Shape::Hip { y, at, ridge, pitch, half } => {
                let across = -ridge[1] * (p[0] - at[0]) + ridge[0] * (p[1] - at[1]);
                let along = ridge[0] * (p[0] - at[0]) + ridge[1] * (p[1] - at[1]);
                if across.abs() >= along.abs() - half {
                    local_plane(at, ridge, [y, 0.0, -pitch * sign(across)])
                } else {
                    local_plane(at, ridge, [y + pitch * half, -pitch * sign(along), 0.0])
                }
            }
        }
    }
}

/// `1` for zero and up, else `-1`.
fn sign(v: f64) -> f64 {
    if v >= 0.0 { 1.0 } else { -1.0 }
}

/// How a house's wings are roofed (`House`): every wing the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pitch {
    /// Two slopes meeting at the ridge, a gable wall at each end.
    Gable,
    /// Four slopes: a gable whose ends slope too. A square wing is a pyramid.
    Hip,
    /// A barn roof: each side steep up to halfway in, then shallower up to the ridge.
    Gambrel,
}

/// One rectangular wing of a house: its middle, the unit direction its ridge runs along (`u`),
/// and its half length along the ridge and half width across it, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wing {
    pub centre: P,
    pub u: P,
    pub half: [f64; 2],
}

/// How far outside a wing a point still counts as on it, metres.
const ON: f64 = 0.01;

impl Wing {
    /// `p` in the wing's own axes: along the ridge, and across it.
    pub fn local(&self, p: P) -> (f64, f64) {
        let (dx, dz) = (p[0] - self.centre[0], p[1] - self.centre[1]);
        (dx * self.u[0] + dz * self.u[1], -dx * self.u[1] + dz * self.u[0])
    }

    /// The point at `(a, b)` in the wing's axes.
    pub fn world(&self, a: f64, b: f64) -> P {
        [self.centre[0] + a * self.u[0] - b * self.u[1], self.centre[1] + a * self.u[1] + b * self.u[0]]
    }

    pub fn contains(&self, p: P) -> bool {
        let (a, b) = self.local(p);
        a.abs() <= self.half[0] + ON && b.abs() <= self.half[1] + ON
    }

    /// The plane the wing's roof lies in over `p`, eaves at `eave` and slopes `k` (the second only for a gambrel's upper slope).
    pub fn plane_at(&self, p: P, pitch: Pitch, eave: f64, k: [f64; 2]) -> Plane {
        let (a, b) = self.local(p);
        let (l, w) = (self.half[0], self.half[1]);
        let (s, t) = (sign(b), sign(a));
        let side = local_plane(self.centre, self.u, [eave + k[0] * w, 0.0, -k[0] * s]);
        match pitch {
            Pitch::Gable => side,
            Pitch::Hip => {
                if w - b.abs() <= l - a.abs() { side } else { local_plane(self.centre, self.u, [eave + k[0] * l, -k[0] * t, 0.0]) }
            }
            Pitch::Gambrel => {
                if w - b.abs() <= w / 2.0 { side } else { local_plane(self.centre, self.u, [eave + (k[0] + k[1]) * w / 2.0, 0.0, -k[1] * s]) }
            }
        }
    }

    /// Every plane the wing's roof has.
    fn planes(&self, pitch: Pitch, eave: f64, k: [f64; 2]) -> Vec<Plane> {
        let (l, w) = (self.half[0], self.half[1]);
        let probe = |a: f64, b: f64| self.plane_at(self.world(a, b), pitch, eave, k);
        let mut out = vec![probe(0.0, w * 0.9), probe(0.0, -w * 0.9)];
        match pitch {
            Pitch::Gable => {}
            Pitch::Hip => out.extend([probe(l * 0.999, 0.0), probe(-l * 0.999, 0.0)]),
            Pitch::Gambrel => out.extend([probe(0.0, w * 0.1), probe(0.0, -w * 0.1)]),
        }
        out
    }

    /// Lines `(point, direction)` where the wing's roof bends, and its four sides.
    fn lines(&self, pitch: Pitch, out: &mut Vec<(P, P)>) {
        let (l, w) = (self.half[0], self.half[1]);
        let v = [-self.u[1], self.u[0]];
        out.push((self.centre, self.u));
        for sign in [-1.0, 1.0] {
            out.push((self.world(sign * l, 0.0), v));
            out.push((self.world(0.0, sign * w), self.u));
        }
        match pitch {
            Pitch::Gable => {}
            Pitch::Hip => {
                for t in [-1.0, 1.0] {
                    for s in [-1.0, 1.0] {
                        let d = [s * self.u[0] + t * v[0], s * self.u[1] + t * v[1]];
                        out.push((self.world(t * (l - w), 0.0), d));
                    }
                }
            }
            Pitch::Gambrel => {
                for sign in [-1.0, 1.0] {
                    out.push((self.world(0.0, sign * w / 2.0), self.u));
                }
            }
        }
    }
}

/// A dormer: a box standing out of a slope with a roof of its own, a window in its front. Its
/// rectangle on the ground is laid out as a wing's is; `wing` is the one it stands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dormer {
    pub area: Wing,
    pub plane: Plane,
    pub wing: usize,
}

/// A house roofed from the catalogue (`catalogue`): rectangular wings with the same eaves and
/// slope, each roofed the same way, meeting in valleys; and its dormers.
#[derive(Clone, Debug, PartialEq)]
pub struct House {
    pub eave: f64,
    pub pitch: Pitch,
    pub k: [f64; 2],
    pub wings: Vec<Wing>,
    pub dormers: Vec<Dormer>,
}

impl House {
    /// The plane the roof lies in over `p`: the highest of the wings it is on, or of a dormer
    /// there, or the eaves for a bit of the house under no wing (a porch).
    pub fn plane_at(&self, p: P) -> Plane {
        let mut best: Option<(f64, Plane)> = None;
        for wing in self.wings.iter().filter(|wing| wing.contains(p)) {
            let plane = wing.plane_at(p, self.pitch, self.eave, self.k);
            let y = eval(&plane, p);
            if best.is_none_or(|(top, _)| y > top + 1e-9) {
                best = Some((y, plane));
            }
        }
        for dormer in self.dormers.iter().filter(|dormer| dormer.area.contains(p)) {
            let y = eval(&dormer.plane, p);
            if best.is_none_or(|(top, _)| y > top + 1e-9) {
                best = Some((y, dormer.plane));
            }
        }
        best.map_or([self.eave, 0.0, 0.0], |(_, plane)| plane)
    }

    pub fn height(&self, p: P) -> f64 {
        eval(&self.plane_at(p), p)
    }
}

impl Roof {
    /// The plane the roof lies in over `p`.
    pub fn plane_at(&self, p: P) -> Plane {
        match self {
            Roof::Leaf(shape) => shape.plane_at(p),
            Roof::Cut { .. } => leaf_at(self, p).plane_at(p),
            Roof::House(house) => house.plane_at(p),
        }
    }

    /// Lines the whole footprint is cut along: where the roof bends or changes part, and every wing's sides.
    fn lines(&self, out: &mut Vec<(P, P)>) {
        match self {
            Roof::House(house) => house.wings.iter().for_each(|wing| wing.lines(house.pitch, out)),
            roof => bends(roof, out),
        }
    }

    /// Lines only the pieces on given wings are cut along (by the wing numbers): the valleys
    /// where two wings meet, and the sides of the dormers on a wing.
    fn local_lines(&self) -> Vec<(Vec<usize>, (P, P))> {
        let Roof::House(house) = self else { return Vec::new() };
        let mut out = Vec::new();
        for i in 0..house.wings.len() {
            for j in i + 1..house.wings.len() {
                for a in house.wings[i].planes(house.pitch, house.eave, house.k) {
                    for b in house.wings[j].planes(house.pitch, house.eave, house.k) {
                        // Where the two planes are at one height.
                        let n = [a[1] - b[1], a[2] - b[2]];
                        let length = n[0].hypot(n[1]);
                        if length < 1e-6 {
                            continue;
                        }
                        let o = [-(a[0] - b[0]) * n[0] / (length * length), -(a[0] - b[0]) * n[1] / (length * length)];
                        out.push((vec![i, j], (o, [-n[1] / length, n[0] / length])));
                    }
                }
            }
        }
        for dormer in &house.dormers {
            let mut sides = Vec::new();
            let area = dormer.area;
            let v = [-area.u[1], area.u[0]];
            for sign in [-1.0, 1.0] {
                sides.push((area.world(sign * area.half[0], 0.0), v));
                sides.push((area.world(0.0, sign * area.half[1]), area.u));
            }
            out.extend(sides.into_iter().map(|line| (vec![dormer.wing], line)));
        }
        out
    }
}

/// The shape of the roof over `p`.
fn leaf_at(roof: &Roof, p: P) -> &Shape {
    match roof {
        Roof::Leaf(shape) => shape,
        Roof::Cut { o, n, low, high } => {
            if n[0] * (p[0] - o[0]) + n[1] * (p[1] - o[1]) < 0.0 { leaf_at(low, p) } else { leaf_at(high, p) }
        }
        Roof::House(_) => unreachable!("a house is no part of a cut roof"),
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
        Roof::House(_) => roof.lines(out),
    }
}

/// Middle of a polygon's corners.
fn centroid(polygon: &[P]) -> P {
    let n = polygon.len() as f64;
    polygon.iter().fold([0.0, 0.0], |c, p| [c[0] + p[0] / n, c[1] + p[1] / n])
}

/// A side of a roof piece by the keys of its two ends, in the order its piece goes round.
type Side = ([i64; 2], [i64; 2]);

/// Key of a point to the tenth of a millimetre, so the same corner reached two ways is one.
fn key(p: P) -> [i64; 2] {
    [(p[0] * 1e4).round() as i64, (p[1] * 1e4).round() as i64]
}

/// The footprint triangles cut along every line of `roof` into pieces each under one plane,
/// counter-clockwise, with every corner of a neighbouring piece that lies on one of its sides
/// added to it, so two pieces always meet corner to corner.
fn roof_pieces(roof: &Roof, triangles: Vec<Vec<P>>) -> Vec<Vec<P>> {
    let mut lines = Vec::new();
    roof.lines(&mut lines);
    let keep = |piece: &Vec<P>| piece.len() >= 3 && polygon_area(piece).abs() > 1e-8;
    let mut pieces: Vec<Vec<P>> = Vec::new();
    for triangle in triangles {
        let mut parts = vec![triangle];
        for &(o, d) in &lines {
            parts = parts.into_iter().flat_map(|piece| { let (left, right) = split(&piece, o, d); [left, right] }).filter(keep).collect();
        }
        pieces.extend(parts);
    }
    if let Roof::House(house) = roof {
        for (wings, (o, d)) in roof.local_lines() {
            pieces = pieces
                .into_iter()
                .flat_map(|piece| {
                    let c = centroid(&piece);
                    if wings.iter().all(|&w| house.wings[w].contains(c)) {
                        let (left, right) = split(&piece, o, d);
                        vec![left, right]
                    } else {
                        vec![piece]
                    }
                })
                .filter(keep)
                .collect();
        }
    }
    for piece in &mut pieces {
        if polygon_area(piece) < 0.0 {
            piece.reverse();
        }
        piece.dedup_by(|a, b| key(*a) == key(*b));
        if piece.len() > 1 && key(piece[0]) == key(piece[piece.len() - 1]) {
            piece.pop();
        }
    }
    pieces.retain(keep);
    // Corners of other pieces lying on a side, put in along it.
    let mut corners: Vec<P> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in pieces.iter().flatten() {
        if seen.insert(key(*p)) {
            corners.push(*p);
        }
    }
    pieces
        .into_iter()
        .map(|piece| {
            let mut out = Vec::with_capacity(piece.len());
            for i in 0..piece.len() {
                let (a, b) = (piece[i], piece[(i + 1) % piece.len()]);
                out.push(a);
                let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
                let length = dx * dx + dz * dz;
                if length < 1e-12 {
                    continue;
                }
                let (lo, hi) = ([a[0].min(b[0]) - 1e-3, a[1].min(b[1]) - 1e-3], [a[0].max(b[0]) + 1e-3, a[1].max(b[1]) + 1e-3]);
                let mut on: Vec<(f64, P)> = corners
                    .iter()
                    .filter(|p| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1])
                    .filter(|p| key(**p) != key(a) && key(**p) != key(b))
                    .filter_map(|&p| {
                        let t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length;
                        let off = ((p[0] - a[0]) * dz - (p[1] - a[1]) * dx).abs() / length.sqrt();
                        (t > 1e-6 && t < 1.0 - 1e-6 && off < 5e-4).then_some((t, p))
                    })
                    .collect();
                on.sort_by(|x, y| x.0.total_cmp(&y.0));
                out.extend(on.into_iter().map(|(_, p)| p));
            }
            out
        })
        .collect()
}

/// Appends a building to `mesh`: walls up to the roof (and steps where parts of the roof differ in
/// height), the roof, then the floor, in that order. `footprint` is counter-clockwise from above.
/// The floor is at `floor` metres.
///
/// The footprint is cut into pieces each under one plane of the roof (`roof_pieces`). A side of a
/// piece no other piece shares is the top of an outer wall; a side two pieces share, where the
/// roof is higher on one than the other, is a step: a wall facing the lower side (a dormer's front
/// and cheeks, the end of a higher part).
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
    let pieces = roof_pieces(roof, triangles.iter().map(|t| t.iter().map(|&i| footprint[i]).collect()).collect());
    let planes: Vec<Plane> = pieces.iter().map(|piece| roof.plane_at(centroid(piece))).collect();
    let height = |piece: usize, p: P| clamp(eval(&planes[piece], p));

    // Every side of every piece, by its two ends: a side two pieces share is in both, the other way round.
    let mut sides: HashMap<Side, (usize, P, P)> = HashMap::new();
    for (i, piece) in pieces.iter().enumerate() {
        for k in 0..piece.len() {
            let (p, q) = (piece[k], piece[(k + 1) % piece.len()]);
            sides.insert((key(p), key(q)), (i, p, q));
        }
    }
    let mut order: Vec<(Side, (usize, P, P))> = sides.iter().map(|(k, v)| (*k, *v)).collect();
    order.sort_by_key(|(k, _)| *k);

    let first_wall = mesh.triangles.len();
    for &((kp, kq), (i, p, q)) in &order {
        match sides.get(&(kq, kp)) {
            // An outer wall, from the floor to the roof.
            None => {
                let (tp, tq) = (height(i, p), height(i, q));
                highest = highest.max(tp).max(tq);
                let (bp, bq) = (vertex(mesh, p[0], floor, p[1]), vertex(mesh, q[0], floor, q[1]));
                let (up, uq) = (vertex(mesh, p[0], tp, p[1]), vertex(mesh, q[0], tq, q[1]));
                push(mesh, [bp, bq, uq]);
                push(mesh, [bp, uq, up]);
            }
            // A side shared with the piece `j`: once, from the pair's lesser key.
            Some(&(j, ..)) if (kp, kq) < (kq, kp) => {
                let mut runs = vec![(p, q)];
                let (dp, dq) = (height(i, p) - height(j, p), height(i, q) - height(j, q));
                if dp * dq < 0.0 && dp.abs() > 0.02 && dq.abs() > 0.02 {
                    let t = dp / (dp - dq);
                    let m = [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
                    runs = vec![(p, m), (m, q)];
                }
                for (a, b) in runs {
                    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                    let up = height(i, mid) - height(j, mid);
                    if up.abs() < 0.02 && (height(i, a) - height(j, a)).abs() < 0.02 && (height(i, b) - height(j, b)).abs() < 0.02 {
                        continue;
                    }
                    // Walked with the higher piece on the left, as an outer wall is with the house.
                    let (high, low, a, b) = if up > 0.0 { (i, j, a, b) } else { (j, i, b, a) };
                    let (la, lb) = (vertex(mesh, a[0], height(low, a), a[1]), vertex(mesh, b[0], height(low, b), b[1]));
                    let (ha, hb) = (vertex(mesh, a[0], height(high, a), a[1]), vertex(mesh, b[0], height(high, b), b[1]));
                    push(mesh, [la, lb, hb]);
                    push(mesh, [la, hb, ha]);
                }
            }
            Some(_) => {}
        }
    }
    let walls = (mesh.triangles.len() - first_wall) as u32;

    // Roof: each piece is a plane. A piece with a corner in the middle of a side is fanned from its middle.
    let first_roof = mesh.triangles.len();
    for (i, piece) in pieces.iter().enumerate() {
        let mut corner = |mesh: &mut CollisionMesh, p: P| {
            let y = height(i, p);
            highest = highest.max(y);
            vertex(mesh, p[0], y, p[1])
        };
        let n = piece.len();
        let straight = (0..n).any(|k| {
            let (a, b, c) = (piece[(k + n - 1) % n], piece[k], piece[(k + 1) % n]);
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() < 1e-7
        });
        let ids: Vec<u32> = piece.iter().map(|&p| corner(mesh, p)).collect();
        if straight && n > 3 {
            let middle = corner(mesh, centroid(piece));
            for k in 0..n {
                push(mesh, [middle, ids[k], ids[(k + 1) % n]]);
            }
        } else {
            for k in 1..n - 1 {
                push(mesh, [ids[0], ids[k], ids[k + 1]]);
            }
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

    /// The returns inside `footprint`, a local-metre polygon, as `[x, z, y]`. `grid` maps local
    /// metres to the cloud's own `(easting, northing)` (`region::Place::source`).
    pub fn over(&self, footprint: &[P], grid: impl Fn(f64, f64) -> (f64, f64)) -> Vec<[f64; 4]> {
        let n = footprint.len() as f64;
        let centre = footprint.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
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
pub(crate) mod tests {
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
    fn a_crown_over_the_whole_roof_is_seen_through() {
        // A 6 × 8 cottage wholly under a crown 17 m up: every pulse hits the crown, then the roof.
        let footprint = rectangle(8.0, 6.0, 0.0, [0.0, 0.0]);
        let truth = Shape::Gable { y: 5.0, at: [0.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6 };
        let points: Vec<[f64; 4]> = sample(&footprint, &truth)
            .into_iter()
            .flat_map(|[x, z, y, _]| [[x, z, 17.0 + (x * z).sin(), f64::from(1 | 2 << 4)], [x, z, y, f64::from(2 | 2 << 4)]])
            .collect();
        let Shape::Gable { y, .. } = leaf(fit(&footprint, &points)) else { panic!("not a gable") };
        assert!((y - 5.0).abs() < 0.5, "the crown pulled the ridge to {y}");
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
    fn a_big_building_with_a_lower_flat_wing_is_two_roofs() {
        // 32 × 10, big enough to be fitted in parts: a gable over the west half, a flat roof lower over the east half.
        let footprint = rectangle(32.0, 10.0, 0.0, [0.0, 0.0]);
        let gable = Shape::Gable { y: 10.0, at: [-8.0, 0.0], ridge: [1.0, 0.0], pitch: 0.6 };
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
        assert!((west.height([-8.0, 0.0]) - 10.0).abs() < 0.4, "{west:?}");
        assert!((east.height([8.0, 0.0]) - 4.0).abs() < 0.4, "{east:?}");
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

    /// Fails when a corner of the roof lies in the middle of another roof triangle's edge: there
    /// the two pieces do not meet corner to corner, and a crack shows between them.
    pub(crate) fn no_cracks(mesh: &CollisionMesh, parts: Parts) {
        let roof = &mesh.triangles[parts.walls as usize..(parts.walls + parts.roof) as usize];
        let at = |i: u32| mesh.vertices[i as usize].map(f64::from);
        let mut used: Vec<u32> = roof.iter().flatten().copied().collect();
        used.sort_unstable();
        used.dedup();
        for tri in roof {
            for k in 0..3 {
                let (a, b) = (at(tri[k]), at(tri[(k + 1) % 3]));
                let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let length = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                for &r in &used {
                    if r == tri[k] || r == tri[(k + 1) % 3] {
                        continue;
                    }
                    let p = at(r);
                    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1] + (p[2] - a[2]) * d[2]) / length;
                    if t <= 0.01 || t >= 0.99 {
                        continue;
                    }
                    let off = (0..3).map(|i| (a[i] + d[i] * t - p[i]).powi(2)).sum::<f64>().sqrt();
                    assert!(off > 0.6, "a crack: {p:?} lies on the edge {a:?} to {b:?}");
                }
            }
        }
    }

    /// The footprint area the roof triangles cover seen from above, signed so a hole shows.
    pub(crate) fn roof_area(mesh: &CollisionMesh, parts: Parts) -> f64 {
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
        no_cracks(&stepped, parts);
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
