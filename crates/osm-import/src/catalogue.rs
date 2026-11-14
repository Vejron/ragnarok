//! Roofs for houses from a catalogue of common shapes. A house's laser returns are not fitted
//! freely (`roof::fit`): its footprint (squared by `square`) is cut into at most four rectangular
//! wings, and each shape of the catalogue is fitted over them with one height at the eaves and one
//! slope for the whole house, the ridge down the middle of each wing:
//!
//! - flat, and shed (one slope along either side of the house);
//! - gable and hip (a square wing's hip is a pyramid), and gambrel, the barn roof;
//! - on an L, T or U plan the same over each wing, the side wings run in under the main roof so
//!   the two meet in valleys.
//!
//! The simplest shape that fits well wins, and the way's `roof:shape` tag tips the scales. On a
//! steep slope a run of returns standing well over it, as long as a window or a few, is a dormer.

use crate::roof::{self, Dormer, House, Pitch, Roof, Shape, Wing, eval, local_plane, solve_rows};
use crate::square;

type P = [f64; 2];

/// The shapes of the catalogue, as the importer counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Flat,
    Shed,
    Gable,
    Hip,
    Gambrel,
}

impl Category {
    /// What OpenStreetMap's `roof:shape` says the roof is, where it is one of these.
    pub fn of_tag(tag: &str) -> Option<Self> {
        Some(match tag {
            "flat" => Category::Flat,
            "skillion" | "lean_to" => Category::Shed,
            "gabled" | "saltbox" => Category::Gable,
            "hipped" | "half-hipped" | "side_hipped" | "pyramidal" => Category::Hip,
            "gambrel" | "mansard" => Category::Gambrel,
            _ => return None,
        })
    }
}

/// A roof from the catalogue and how well it fits.
#[derive(Clone, Debug)]
pub struct Fitted {
    pub roof: Roof,
    pub category: Category,
    /// Wings it was fitted over, and dormers found.
    pub wings: usize,
    pub dormers: usize,
    /// Root of the mean squared miss of the returns, each miss counted up to a metre, metres.
    pub miss: f64,
    /// Whether the returns showed no roof the catalogue has (a house under trees) and it was given the usual one (`MURKY`).
    pub guessed: bool,
}

/// A house whose best fit is flat or shed and still misses its returns by this much (the
/// `cost_of` measure, about 0.3 m) was not seen by the laser, only the trees over it: it gets
/// the usual roof instead, a gable (or the shape OSM names) at `USUAL_PITCH` from the eaves it
/// would have (`fit`'s `eaves`).
const MURKY: f64 = 0.1;
const USUAL_PITCH: f64 = 0.5;

/// A wing narrower than this, metres, is no wing but a porch or a bay: it is roofed flat at the eaves.
const MIN_WING: f64 = 1.5;
/// Most wings a plan may have; a plan with more is roofed over its rectangle.
const MOST_WINGS: usize = 4;
/// Most of a plan, as a share of its area, that may be porches.
const MOST_PORCH: f64 = 0.25;
/// A side wing at least this deep, for its width along the main wing, has its ridge run out from
/// the main one (a cross gable); a shallower one runs its ridge alongside.
const CROSS_DEPTH: f64 = 0.6;
/// The slope of a gable or hip, rise per run: 9° to 50°. Low roofs are common on Swedish houses.
const PITCH: (f64, f64) = (0.16, 1.192);
/// A gambrel's lower and upper slopes, and how much steeper the lower must be.
const GAMBREL_LOW: (f64, f64) = (0.7, 3.5);
const GAMBREL_HIGH: (f64, f64) = (0.15, 0.9);
const GAMBREL_BREAK: f64 = 1.4;
/// A shed roof slopes at least this (3°) and at most this (35°), rise per run.
const SHED: (f64, f64) = (0.05, 0.7);
/// How much each parameter past the first adds to a shape's cost, and the extra a hip pays over a gable, so the simpler shape wins a near tie.
const PER_PARAMETER: f64 = 0.15;
const HIP_OVER_GABLE: f64 = 1.1;
/// The cost of the shape OpenStreetMap names (`roof:shape`) is multiplied by this.
const TAGGED: f64 = 0.7;

/// Dormers: the least slope (25°) and depth from eave to ridge (metres) a slope must have to carry one.
const DORMER_SLOPE: f64 = 0.466;
const DORMER_DEPTH: f64 = 2.5;
/// Returns are laid along the eave in bins this long, metres; a bin is raised when it has
/// `DORMER_RETURNS` returns or more whose middle one stands `DORMER_RISE` metres or more over the slope.
const DORMER_BIN: f64 = 1.0;
const DORMER_RETURNS: usize = 3;
const DORMER_RISE: f64 = 0.6;
/// A dormer is two to six bins long, its front at least this tall over the slope, metres.
const DORMER_BINS: (usize, usize) = (2, 6);
const DORMER_FRONT: f64 = 1.2;
/// Most dormers on one slope, and the share of the slope's cost a dormer must take off.
const DORMERS_A_SLOPE: usize = 3;
const DORMER_GAIN: f64 = 0.7;

/// The roof from the catalogue over `ring` (local metres, counter-clockwise, not closed) for the
/// laser returns `points` (`[x, z, y, code]`, as `roof::fit` takes them), `tagged` the shape
/// OpenStreetMap names. `None` when there are too few returns.
/// `eaves` is the height (local y, metres) a house this size has its eaves at when the laser shows
/// nothing better; without it a house the laser did not see keeps its best fit.
pub fn fit(ring: &[P], points: &[[f64; 4]], tagged: Option<Category>, eaves: Option<f64>) -> Option<Fitted> {
    let (returns, unstripped) = roof::roof_returns(ring, points)?;
    let (wings, porch) = plan(ring)?;
    // A footprint drawn a little off where the house stands would read a gable as a low or tilted
    // roof: the returns are moved back under it first.
    let shift = misplaced(&returns, &wings);
    let moved = |list: &[[f64; 3]]| list.iter().map(|r| [r[0] - shift[0], r[1] - shift[1], r[2]]).collect::<Vec<_>>();
    let (returns, unstripped) = (moved(&returns), moved(&unstripped));
    let ys: Vec<f64> = returns.iter().map(|r| r[2]).collect();
    let at = |r: &[f64; 3]| [r[0], r[1]];
    let weigh = |category: Category, cost: f64, parameters: usize| {
        let mut cost = cost * (1.0 + PER_PARAMETER * (parameters - 1) as f64);
        if category == Category::Hip {
            cost *= HIP_OVER_GABLE;
        }
        if tagged == Some(category) {
            cost *= TAGGED;
        }
        cost
    };

    // Flat: the one height.
    let (flat, _) = solve_rows(&vec![[1.0, 0.0, 0.0]; returns.len()], &ys);
    let flat_cost = cost_of(&returns, &|_| flat[0]);
    let mut best = (weigh(Category::Flat, flat_cost, 1), flat_cost, Category::Flat, Roof::Leaf(Shape::Flat { y: flat[0] }));
    let consider = |category: Category, cost: f64, parameters: usize, roof: Roof, best: &mut (f64, f64, Category, Roof)| {
        let weighed = weigh(category, cost, parameters);
        // Noise alone must not turn a flat roof into anything else: it has to miss by clearly less.
        if weighed < best.0 && weigh(category, cost, 1) < weigh(Category::Flat, flat_cost, 1) * 0.7 && flat_cost - cost > 0.002 {
            *best = (weighed, cost, category, roof);
        }
    };

    // Shed: one slope along either side of the house's rectangle.
    let main = wings[0];
    for axis in [main.u, [-main.u[1], main.u[0]]] {
        let rows: Vec<[f64; 3]> = returns.iter().map(|r| [1.0, (r[0] - main.centre[0]) * axis[0] + (r[1] - main.centre[1]) * axis[1], 0.0]).collect();
        let (c, _) = solve_rows(&rows, &ys);
        let cost = cost_of(&returns, &|p| c[0] + c[1] * ((p[0] - main.centre[0]) * axis[0] + (p[1] - main.centre[1]) * axis[1]));
        if (SHED.0..=SHED.1).contains(&c[1].abs()) {
            consider(Category::Shed, cost, 2, Roof::Leaf(Shape::Shed { y: c[0], at: main.centre, slope: [axis[0] * c[1], axis[1] * c[1]] }), &mut best);
        }
    }

    // Gable, hip and gambrel over the wings; a lone wing may have its ridge either way.
    let mut layouts = vec![wings.clone()];
    if wings.len() == 1 {
        layouts.push(vec![Wing { centre: main.centre, u: [-main.u[1], main.u[0]], half: [main.half[1], main.half[0]] }]);
    }
    for layout in &layouts {
        for (pitch, category, parameters) in [(Pitch::Gable, Category::Gable, 2), (Pitch::Hip, Category::Hip, 2), (Pitch::Gambrel, Category::Gambrel, 3)] {
            if let Some(house) = fit_wings(&returns, layout, pitch) {
                let cost = cost_of(&returns, &|p| house.height(p));
                consider(category, cost, parameters, Roof::House(Box::new(house)), &mut best);
            }
        }
    }
    let (_, cost, mut category, mut roof) = best;
    let mut guessed = false;
    if let (true, Some(eave)) = (matches!(category, Category::Flat | Category::Shed) && cost > MURKY, eaves) {
        let pitch = match tagged {
            Some(Category::Hip) => Pitch::Hip,
            Some(Category::Gambrel) => Pitch::Gambrel,
            _ => Pitch::Gable,
        };
        category = match pitch { Pitch::Hip => Category::Hip, Pitch::Gambrel => Category::Gambrel, Pitch::Gable => Category::Gable };
        let k = if pitch == Pitch::Gambrel { [3.0 * USUAL_PITCH, USUAL_PITCH / 2.0] } else { [USUAL_PITCH, 0.0] };
        roof = Roof::House(Box::new(House { eave, pitch, k, wings: wings.clone(), dormers: Vec::new() }));
        guessed = true;
    }
    let mut dormers = 0;
    if let (Roof::House(house), false) = (&mut roof, guessed) {
        // A dormer stands well over the slope, where a first fit took it for a tree.
        house.dormers = find_dormers(house, &unstripped);
        dormers = house.dormers.len();
    }
    let miss = cost_of(if dormers > 0 { &unstripped } else { &returns }, &|p| roof_height(&roof, p)).sqrt();
    let _ = (porch, at);
    Some(Fitted { roof, category, wings: wings.len(), dormers, miss, guessed })
}

/// Furthest a footprint is taken to be off the house the laser saw, metres, and the steps it is searched in.
const MOST_OFF: f64 = 1.0;
const OFF_STEP: f64 = 0.25;

/// How far the house the laser saw stands off `wings` (`[x, z]`, metres): the shift of the returns,
/// within `MOST_OFF` along each side of the main wing, under which a gable fits them best. Nothing
/// unless it fits clearly better than where it was drawn.
fn misplaced(returns: &[[f64; 3]], wings: &[Wing]) -> P {
    let main = wings[0];
    let v = [-main.u[1], main.u[0]];
    let cost = |shift: P| {
        let moved: Vec<[f64; 3]> = returns.iter().map(|r| [r[0] - shift[0], r[1] - shift[1], r[2]]).collect();
        fit_wings_free(&moved, wings, Pitch::Gable).map_or(f64::MAX, |house| if house.k[0] > 0.0 { cost_of(&moved, &|p| house.height(p)) } else { f64::MAX })
    };
    let steps = (MOST_OFF / OFF_STEP).round() as i32;
    let mut best = ([0.0, 0.0], cost([0.0, 0.0]));
    let here = best.1;
    for i in -steps..=steps {
        for j in -steps..=steps {
            let (a, b) = (f64::from(i) * OFF_STEP, f64::from(j) * OFF_STEP);
            let shift = [a * main.u[0] + b * v[0], a * main.u[1] + b * v[1]];
            let found = cost(shift);
            if found < best.1 {
                best = (shift, found);
            }
        }
    }
    if best.1 < 0.8 * here { best.0 } else { [0.0, 0.0] }
}

/// Height of `roof` over `p`.
fn roof_height(roof: &Roof, p: P) -> f64 {
    eval(&roof.plane_at(p), p)
}

/// Mean squared miss of `height` over the `TRIMMED` share of `returns` it misses least, each counted
/// up to a metre: what is left over is mostly the crowns of trees leaning over the roof, and would
/// otherwise hide which shape the roof is.
fn cost_of(returns: &[[f64; 3]], height: &dyn Fn(P) -> f64) -> f64 {
    let mut misses: Vec<f64> = returns.iter().map(|r| (r[2] - height([r[0], r[1]])).powi(2).min(1.0)).collect();
    misses.sort_by(f64::total_cmp);
    let kept = ((misses.len() as f64 * TRIMMED).ceil() as usize).clamp(1, misses.len().max(1));
    misses.iter().take(kept).sum::<f64>() / kept as f64
}

/// The share of the returns a shape is judged by (`cost_of`).
const TRIMMED: f64 = 0.75;

/// The plan of `ring` as wings, the biggest (the main wing) first, and whether any of it is porch.
/// A footprint that is not square, or cut into more than `MOST_WINGS` wings, is one wing: the
/// smallest rectangle around it.
pub fn plan(ring: &[P]) -> Option<(Vec<Wing>, bool)> {
    let u = square::direction(ring);
    if square::is_square(ring, u) && let Some(found) = wings(ring, u) {
        return Some(found);
    }
    roof::rectangle(ring).map(|wing| (vec![wing], false))
}

/// `ring`, whose walls all run along `u` or across it, cut into rectangles: the biggest first,
/// then the biggest of what is left, and so on. Side wings deep enough are turned to run out from
/// the main one and carried in under its roof to its ridge.
fn wings(ring: &[P], u: P) -> Option<(Vec<Wing>, bool)> {
    let v = [-u[1], u[0]];
    let local: Vec<P> = ring.iter().map(|p| [p[0] * u[0] + p[1] * u[1], p[0] * v[0] + p[1] * v[1]]).collect();
    let lines = |axis: usize| {
        let mut values: Vec<f64> = local.iter().map(|p| p[axis]).collect();
        values.sort_by(f64::total_cmp);
        values.dedup_by(|a, b| (*a - *b).abs() < 0.02);
        values
    };
    let (xs, zs) = (lines(0), lines(1));
    let (nx, nz) = (xs.len() - 1, zs.len() - 1);
    if nx == 0 || nz == 0 || nx * nz > 400 {
        return None;
    }
    let inside: Vec<bool> = (0..nz).flat_map(|j| (0..nx).map(move |i| (i, j))).map(|(i, j)| contains(&local, [(xs[i] + xs[i + 1]) / 2.0, (zs[j] + zs[j + 1]) / 2.0])).collect();
    let mut covered = vec![false; nx * nz];
    let mut rects: Vec<[f64; 4]> = Vec::new();
    loop {
        // The biggest rectangle of cells inside the plan that no rectangle has yet.
        let mut best: Option<(f64, [usize; 4])> = None;
        for i0 in 0..nx {
            for j0 in 0..nz {
                for i1 in i0..nx {
                    for j1 in j0..nz {
                        let free = (j0..=j1).all(|j| (i0..=i1).all(|i| inside[j * nx + i] && !covered[j * nx + i]));
                        if !free {
                            break;
                        }
                        let area = (xs[i1 + 1] - xs[i0]) * (zs[j1 + 1] - zs[j0]);
                        if best.is_none_or(|(known, _)| area > known + 1e-9) {
                            best = Some((area, [i0, i1, j0, j1]));
                        }
                    }
                }
            }
        }
        let Some((_, [i0, i1, j0, j1])) = best else { break };
        for j in j0..=j1 {
            for i in i0..=i1 {
                covered[j * nx + i] = true;
            }
        }
        rects.push([xs[i0], xs[i1 + 1], zs[j0], zs[j1 + 1]]);
        if rects.len() > 3 * MOST_WINGS {
            return None;
        }
    }
    let total: f64 = rects.iter().map(|r| (r[1] - r[0]) * (r[3] - r[2])).sum();
    let (big, small): (Vec<[f64; 4]>, Vec<[f64; 4]>) = rects.into_iter().partition(|r| (r[1] - r[0]).min(r[3] - r[2]) >= MIN_WING);
    let porch: f64 = small.iter().map(|r| (r[1] - r[0]) * (r[3] - r[2])).sum();
    if big.is_empty() || big.len() > MOST_WINGS || porch > MOST_PORCH * total {
        return None;
    }
    let world = |[a, b]: P| [a * u[0] + b * v[0], a * u[1] + b * v[1]];
    // A wing over `[a0, a1] × [b0, b1]` with its ridge along `along` (0 for `u`, 1 for `v`).
    let wing = |[a0, a1, b0, b1]: [f64; 4], along: usize| {
        let centre = world([(a0 + a1) / 2.0, (b0 + b1) / 2.0]);
        let (sa, sb) = ((a1 - a0) / 2.0, (b1 - b0) / 2.0);
        if along == 0 { Wing { centre, u, half: [sa, sb] } } else { Wing { centre, u: v, half: [sb, sa] } }
    };
    let long = |r: &[f64; 4]| usize::from(r[3] - r[2] > r[1] - r[0]);
    let main = big[0];
    let main_long = long(&main);
    let across = 1 - main_long;
    let mut out = vec![wing(main, main_long)];
    for &side in &big[1..] {
        let mut side = side;
        let mut along = long(&side);
        // Its extent along and across the main wing's ridge, as `[lo, hi]`.
        let span = |r: &[f64; 4], axis: usize| if axis == 0 { [r[0], r[1]] } else { [r[2], r[3]] };
        let (m_across, s_across) = (span(&main, across), span(&side, across));
        let (m_along, s_along) = (span(&main, main_long), span(&side, main_long));
        let overlap = s_along[1].min(m_along[1]) - s_along[0].max(m_along[0]);
        let touches_high = (s_across[0] - m_across[1]).abs() < 0.02;
        let touches_low = (s_across[1] - m_across[0]).abs() < 0.02;
        if overlap > 0.5 && (touches_high || touches_low) && s_across[1] - s_across[0] >= CROSS_DEPTH * (s_along[1] - s_along[0]) {
            // A cross wing: its ridge runs out from the main one, and it is carried in to the main ridge.
            along = across;
            let middle = (m_across[0] + m_across[1]) / 2.0;
            let reach = if touches_high { [middle, s_across[1]] } else { [s_across[0], middle] };
            if across == 0 {
                side[0] = reach[0];
                side[1] = reach[1];
            } else {
                side[2] = reach[0];
                side[3] = reach[1];
            }
        }
        out.push(wing(side, along));
    }
    Some((out, porch > 0.0))
}

/// Whether `p` is inside `ring`.
fn contains(ring: &[P], [x, z]: P) -> bool {
    let mut hit = false;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            hit = !hit;
        }
    }
    hit
}

/// The house roofed `pitch` over `wings` that fits `returns` best, or `None` when its slopes come
/// out unlike a real roof's. Each return is put under the wing whose roof is highest over it, and
/// the house fitted again, a few times over.
fn fit_wings(returns: &[[f64; 3]], wings: &[Wing], pitch: Pitch) -> Option<House> {
    let house = fit_wings_free(returns, wings, pitch)?;
    let fits = match pitch {
        Pitch::Gambrel => (GAMBREL_LOW.0..=GAMBREL_LOW.1).contains(&house.k[0]) && (GAMBREL_HIGH.0..=GAMBREL_HIGH.1).contains(&house.k[1]) && house.k[0] > GAMBREL_BREAK * house.k[1],
        _ => (PITCH.0..=PITCH.1).contains(&house.k[0]),
    };
    fits.then_some(house)
}

/// `fit_wings` before its slopes are checked against a real roof's.
fn fit_wings_free(returns: &[[f64; 3]], wings: &[Wing], pitch: Pitch) -> Option<House> {
    let ys: Vec<f64> = returns.iter().map(|r| r[2]).collect();
    let mut sorted = ys.clone();
    sorted.sort_by(f64::total_cmp);
    let k = if pitch == Pitch::Gambrel { [1.2, 0.4] } else { [0.6, 0.0] };
    let mut house = House { eave: sorted[sorted.len() / 10], pitch, k, wings: wings.to_vec(), dormers: Vec::new() };
    for _ in 0..4 {
        let rows: Vec<[f64; 3]> = returns
            .iter()
            .map(|r| {
                let p = [r[0], r[1]];
                let under = wings.iter().filter(|w| w.contains(p)).max_by(|a, b| {
                    let (ya, yb) = (eval(&a.plane_at(p, pitch, house.eave, house.k), p), eval(&b.plane_at(p, pitch, house.eave, house.k), p));
                    ya.total_cmp(&yb)
                });
                let Some(wing) = under else { return [1.0, 0.0, 0.0] };
                let (a, b) = wing.local(p);
                let (l, w) = (wing.half[0], wing.half[1]);
                let d = (w - b.abs()).max(0.0);
                match pitch {
                    Pitch::Gable => [1.0, d, 0.0],
                    Pitch::Hip => [1.0, d.min((l - a.abs()).max(0.0)), 0.0],
                    Pitch::Gambrel => [1.0, d.min(w / 2.0), (d - w / 2.0).max(0.0)],
                }
            })
            .collect();
        let (c, _) = solve_rows(&rows, &ys);
        house.eave = c[0];
        house.k = [c[1], if pitch == Pitch::Gambrel { c[2] } else { 0.0 }];
    }
    Some(house)
}

/// The dormers `returns` show on the slopes of `house`: on each side of each wing, a run of
/// returns along the eave standing well over the slope, roofed with a gentle slope of its own.
fn find_dormers(house: &House, returns: &[[f64; 3]]) -> Vec<Dormer> {
    let mut found = Vec::new();
    let steep = match house.pitch {
        Pitch::Gambrel => true,
        _ => house.k[0] >= DORMER_SLOPE,
    };
    if !steep {
        return found;
    }
    let k = house.k[0];
    for (index, wing) in house.wings.iter().enumerate() {
        let (l, w) = (wing.half[0], wing.half[1]);
        if w < DORMER_DEPTH {
            continue;
        }
        let slope_at = |p: P| eval(&wing.plane_at(p, house.pitch, house.eave, house.k), p);
        for s in [-1.0, 1.0] {
            // The returns on this slope: under this wing's roof, on its side (not a hip's end), off the ridge.
            let slope: Vec<(f64, f64, f64, f64)> = returns
                .iter()
                .filter_map(|r| {
                    let p = [r[0], r[1]];
                    if !wing.contains(p) {
                        return None;
                    }
                    let (a, b) = wing.local(p);
                    let d = w - b.abs();
                    let on_side = b * s > 0.0 && d >= 0.0 && d <= 0.9 * w && (house.pitch != Pitch::Hip || d <= l - a.abs());
                    // Under another wing's higher roof it is that wing's.
                    let own = (house.height(p) - slope_at(p)).abs() < 0.01;
                    (on_side && own).then(|| (a, d, r[2], r[2] - slope_at(p)))
                })
                .collect();
            let bins = ((2.0 * l) / DORMER_BIN).floor().max(1.0) as usize;
            let raised: Vec<bool> = (0..bins)
                .map(|i| {
                    let mut rises: Vec<f64> = slope.iter().filter(|o| ((o.0 + l) / DORMER_BIN).floor() as usize == i).map(|o| o.3).collect();
                    rises.sort_by(f64::total_cmp);
                    rises.len() >= DORMER_RETURNS && rises[rises.len() / 2] >= DORMER_RISE
                })
                .collect();
            let mut runs = Vec::new();
            let mut i = 0;
            while i < bins {
                if !raised[i] {
                    i += 1;
                    continue;
                }
                let start = i;
                while i < bins && raised[i] {
                    i += 1;
                }
                if (DORMER_BINS.0..=DORMER_BINS.1).contains(&(i - start)) {
                    runs.push((start, i));
                }
            }
            let mut side: Vec<(f64, Dormer)> = Vec::new();
            for (start, end) in runs {
                let (a0, a1) = ((-l + start as f64 * DORMER_BIN).max(-l + 0.3), (-l + end as f64 * DORMER_BIN).min(l - 0.3));
                let on: Vec<&(f64, f64, f64, f64)> = slope.iter().filter(|o| o.0 >= a0 && o.0 <= a1 && o.3 > 0.3).collect();
                if on.len() < 6 {
                    continue;
                }
                // Its roof: a plane over the distance in from the eave, rising towards the ridge less steeply than the slope.
                let rows: Vec<[f64; 3]> = on.iter().map(|o| [1.0, o.1, 0.0]).collect();
                let ys: Vec<f64> = on.iter().map(|o| o.2).collect();
                let (mut c, _) = solve_rows(&rows, &ys);
                let rise = c[1].clamp(0.0, 0.5 * k);
                if rise != c[1] {
                    c[1] = rise;
                    c[0] = on.iter().map(|o| o.2 - rise * o.1).sum::<f64>() / on.len() as f64;
                }
                let front = (on.iter().map(|o| o.1).fold(f64::MAX, f64::min) - 0.1).max(0.0);
                if c[0] + c[1] * front - (house.eave + k * front) < DORMER_FRONT {
                    continue;
                }
                // Where its roof meets the slope behind it.
                let back = (c[0] - house.eave) / (k - c[1]);
                let lower = if house.pitch == Pitch::Gambrel { w / 2.0 } else { w - 0.3 };
                if back > lower || back < front + 0.5 || (house.pitch == Pitch::Hip && l - a0.abs().max(a1.abs()) < back) {
                    continue;
                }
                let area = Wing { centre: wing.world((a0 + a1) / 2.0, s * (w - (front + back) / 2.0)), u: wing.u, half: [(a1 - a0) / 2.0, (back - front) / 2.0] };
                let plane = local_plane(wing.centre, wing.u, [c[0] + c[1] * w, 0.0, -c[1] * s]);
                let dormer = Dormer { area, plane, wing: index };
                // It has to take a good share off what the slope round it misses by.
                let near: Vec<&(f64, f64, f64, f64)> = slope.iter().filter(|o| o.0 >= a0 - 1.0 && o.0 <= a1 + 1.0).collect();
                let without = near.iter().map(|o| o.3.powi(2).min(1.0)).sum::<f64>();
                let with = near
                    .iter()
                    .map(|o| {
                        let p = wing.world(o.0, s * (w - o.1));
                        let top = if area.contains(p) { slope_at(p).max(eval(&plane, p)) } else { slope_at(p) };
                        (o.2 - top).powi(2).min(1.0)
                    })
                    .sum::<f64>();
                if with <= DORMER_GAIN * without {
                    side.push((without - with, dormer));
                }
            }
            side.sort_by(|a, b| b.0.total_cmp(&a.0));
            found.extend(side.into_iter().take(DORMERS_A_SLOPE).map(|(_, dormer)| dormer));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::CollisionMesh;

    /// Laser returns every 0.5 m over `ring`, heights from `height`, with ±5 cm of noise and a chimney.
    fn sample(ring: &[P], height: impl Fn(P) -> f64) -> Vec<[f64; 4]> {
        let (lo, hi) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let mut out = Vec::new();
        let mut k = 0u64;
        let mut x = lo[0] + 0.25;
        while x < hi[0] {
            let mut z = lo[1] + 0.25;
            while z < hi[1] {
                if contains(ring, [x, z]) {
                    k += 1;
                    let noise = ((k * 2654435761) % 1000) as f64 / 1000.0 * 0.1 - 0.05;
                    // A chimney: a few returns a metre up.
                    let chimney = if k.is_multiple_of(97) { 1.2 } else { 0.0 };
                    out.push([x, z, height([x, z]) + noise + chimney, 0.0]);
                }
                z += 0.5;
            }
            x += 0.5;
        }
        out
    }

    fn rect(w: f64, d: f64) -> Vec<P> {
        vec![[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]]
    }

    fn house_of(fitted: &Fitted) -> &House {
        match &fitted.roof {
            Roof::House(house) => house,
            other => panic!("not a house: {other:?}"),
        }
    }

    #[test]
    fn each_shape_of_the_catalogue_is_told_apart() {
        let ring = rect(12.0, 8.0);
        let ridge = |p: P| 5.0 + 0.7 * (4.0 - (p[1] - 4.0).abs());
        let hip = |p: P| 5.0 + 0.7 * (4.0 - (p[1] - 4.0).abs()).min(6.0 - (p[0] - 6.0).abs());
        let gambrel = |p: P| { let d = 4.0 - (p[1] - 4.0).abs(); 5.0 + 1.6 * d.min(2.0) + 0.4 * (d - 2.0).max(0.0) };
        let cases: [(Category, &dyn Fn(P) -> f64); 5] = [
            (Category::Flat, &|_| 6.0),
            (Category::Shed, &|p: P| 5.0 + 0.2 * p[1]),
            (Category::Gable, &ridge),
            (Category::Hip, &hip),
            (Category::Gambrel, &gambrel),
        ];
        for (category, height) in cases {
            let fitted = fit(&ring, &sample(&ring, height), None, None).unwrap();
            assert_eq!(fitted.category, category, "{:?}", fitted.roof);
            assert!(fitted.miss < 0.2, "{category:?} misses by {}", fitted.miss);
            assert_eq!(fitted.dormers, 0);
        }
        let fitted = fit(&ring, &sample(&ring, ridge), None, None).unwrap();
        let house = house_of(&fitted);
        assert!((house.eave - 5.0).abs() < 0.1 && (house.k[0] - 0.7).abs() < 0.035, "{house:?}");
        assert!((house.wings[0].centre[1] - 4.0).abs() < 1e-9, "the ridge is down the middle");
    }

    #[test]
    fn an_l_shaped_house_is_two_gabled_wings_meeting_in_a_valley() {
        // A 14 × 7 main wing and a 6 × 6 wing out of its north side.
        let ring = vec![[0.0, 0.0], [14.0, 0.0], [14.0, 7.0], [10.0, 7.0], [10.0, 13.0], [4.0, 13.0], [4.0, 7.0], [0.0, 7.0]];
        let main = |p: P| 4.0 + 0.8 * (3.5 - (p[1] - 3.5).abs()).max(0.0);
        let side = |p: P| if (4.0..=10.0).contains(&p[0]) && p[1] >= 3.5 { 4.0 + 0.8 * (3.0 - (p[0] - 7.0).abs()) } else { f64::MIN };
        let fitted = fit(&ring, &sample(&ring, |p| main(p).max(side(p))), None, None).unwrap();
        assert_eq!((fitted.category, fitted.wings), (Category::Gable, 2), "{:?}", fitted.roof);
        assert!(fitted.miss < 0.2, "{}", fitted.miss);
        let house = house_of(&fitted);
        assert!(house.wings[1].u[1].abs() > 0.99, "the side wing's ridge runs out from the main one: {:?}", house.wings[1]);
        // The roof over the valley, and a mesh without cracks.
        assert!((house.height([5.0, 5.0]) - main([5.0, 5.0]).max(side([5.0, 5.0]))).abs() < 0.15);
        let mut mesh = CollisionMesh::default();
        let parts = roof::build(&mut mesh, &ring, 0.0, &fitted.roof);
        assert!((roof::tests::roof_area(&mesh, parts) - (14.0 * 7.0 + 36.0)).abs() < 0.05, "covers {}", roof::tests::roof_area(&mesh, parts));
        roof::tests::no_cracks(&mesh, parts);
    }

    #[test]
    fn a_dormer_on_a_steep_slope_is_found() {
        let ring = rect(12.0, 9.0);
        let k = 0.9;
        let main = |p: P| 4.0 + k * (4.5 - (p[1] - 4.5).abs());
        // A dormer 3 m wide on the south slope (z < 4.5), its front 1 m in from the wall, roof 7.5 m up.
        let dormer = |p: P| if (4.5..=7.5).contains(&p[0]) && (1.0..=3.5).contains(&p[1]) { 7.5f64.max(main(p)) } else { main(p) };
        let fitted = fit(&ring, &sample(&ring, dormer), None, None).unwrap();
        assert_eq!(fitted.category, Category::Gable);
        assert_eq!(fitted.dormers, 1, "{:?}", fitted.roof);
        let house = house_of(&fitted);
        let found = house.dormers[0];
        assert!((found.area.centre[0] - 6.0).abs() < 0.6, "{found:?}");
        assert!((house.height([6.0, 1.6]) - 7.5).abs() < 0.25, "the dormer's roof is {}", house.height([6.0, 1.6]));
        let mut mesh = CollisionMesh::default();
        let parts = roof::build(&mut mesh, &ring, 0.0, &fitted.roof);
        let mut plain = CollisionMesh::default();
        let without = roof::build(&mut plain, &ring, 0.0, &Roof::House(Box::new(House { dormers: Vec::new(), ..house.clone() })));
        assert!(parts.walls > without.walls, "a front and cheeks");
        assert!((roof::tests::roof_area(&mesh, parts) - 108.0).abs() < 0.05);
        roof::tests::no_cracks(&mesh, parts);
    }

    #[test]
    fn a_shallow_roof_or_bumpy_returns_get_no_dormer() {
        let ring = rect(12.0, 9.0);
        let main = |p: P| 4.0 + 0.3 * (4.5 - (p[1] - 4.5).abs());
        let bump = |p: P| if (4.5..=7.5).contains(&p[0]) && (1.0..=3.5).contains(&p[1]) { main(p) + 1.5 } else { main(p) };
        assert_eq!(fit(&ring, &sample(&ring, bump), None, None).unwrap().dormers, 0, "under 25°");
    }

    #[test]
    fn a_noisy_flat_roof_stays_flat_and_a_tag_breaks_a_near_tie() {
        let ring = rect(12.0, 8.0);
        let fitted = fit(&ring, &sample(&ring, |p| 6.0 + 0.05 * (p[0] * 1.7).sin()), None, None).unwrap();
        assert_eq!(fitted.category, Category::Flat);
        // Halfway between a gable and a hip: either fits about as well, and what OSM says decides.
        let gable = |p: P| 5.0 + 0.7 * (4.0 - (p[1] - 4.0).abs());
        let hip = |p: P| 5.0 + 0.7 * (4.0 - (p[1] - 4.0).abs()).min(6.0 - (p[0] - 6.0).abs());
        let near = sample(&ring, |p| (gable(p) + hip(p)) / 2.0);
        assert_eq!(fit(&ring, &near, Some(Category::Hip), None).unwrap().category, Category::Hip);
        assert_eq!(fit(&ring, &near, Some(Category::Gable), None).unwrap().category, Category::Gable);
    }

    #[test]
    fn a_footprint_drawn_off_the_house_still_reads_its_gable() {
        // The house stands 0.7 m east and south of where its footprint is drawn.
        let ring = rect(12.0, 8.0);
        let gable = |p: P| 5.0 + 0.6 * (4.0 - (p[1] - 0.7 - 4.0).abs());
        let mut returns = sample(&rect(14.0, 10.0), gable);
        returns.retain(|r| (0.7..12.7).contains(&r[0]) && (0.7..8.7).contains(&r[1]));
        let fitted = fit(&ring, &returns, None, None).unwrap();
        assert_eq!(fitted.category, Category::Gable);
        let house = house_of(&fitted);
        assert!((house.k[0] - 0.6).abs() < 0.05 && (house.eave - 5.0).abs() < 0.15, "{house:?}");
        assert!((house.wings[0].centre[1] - 4.0).abs() < 1e-9, "the roof is built on the footprint as drawn");
    }

    #[test]
    fn a_house_the_laser_only_saw_trees_over_gets_the_usual_gable() {
        let ring = rect(10.0, 9.0);
        // Crowns from 6 to 12 m all over it.
        let crowns = sample(&ring, |p| 9.0 + 3.0 * (p[0] * 1.3).sin() * (p[1] * 0.9).cos());
        let fitted = fit(&ring, &crowns, None, Some(6.5)).unwrap();
        assert!(fitted.guessed);
        assert_eq!(fitted.category, Category::Gable);
        let house = house_of(&fitted);
        assert!((house.eave - 6.5).abs() < 1e-9 && (house.k[0] - USUAL_PITCH).abs() < 1e-9);
        // Without an eaves height to go by it keeps what fitted best.
        assert!(!fit(&ring, &crowns, None, None).unwrap().guessed);
        // A real flat roof is no guess.
        assert!(!fit(&ring, &sample(&ring, |_| 6.0), None, Some(6.5)).unwrap().guessed);
    }

    #[test]
    fn a_footprint_that_is_not_square_is_roofed_over_its_rectangle() {
        let ring = vec![[0.0, 0.0], [12.0, 0.0], [12.0, 8.0], [3.0, 9.5]];
        let fitted = fit(&ring, &sample(&ring, |_| 6.0), None, None).unwrap();
        assert_eq!((fitted.category, fitted.wings), (Category::Flat, 1));
    }
}
