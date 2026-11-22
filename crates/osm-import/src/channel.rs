//! Rivers that OpenStreetMap draws as a line, laid out as water like any river (RIVER-CHANNELS.md).
//!
//! A big river is a polygon from bank to bank. A smaller one is a centre line, here and there with a
//! wider pool painted over it. Such a line is cut into channels, each the stretch of it outside the
//! wide water (`TRUNK_WIDTH`) it comes from and runs out into. A channel has:
//!
//! - a width along it, `MEAN_WIDTH` on average and down to `NARROWEST` where its valley pinches,
//!   where it falls steeply, and where it must get by a house or a road;
//! - an outline from that width, which becomes a polygon of water like a mapped one;
//! - a long profile that only falls downstream and keeps under its banks.
//!
//! The pools painted over a line are widenings of its channel: one water, with one surface and one
//! current. `water::Surface` levels, cuts and solves all of it as it does a big river.

use crate::water::{Body, Grid, Kind, Map, interpolate, noise};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

type P = [f64; 2];

/// How wide a river drawn as a line is on average, metres.
const MEAN_WIDTH: f64 = 10.0;
/// Its narrowest, metres: in a pinch, a rapid, or past whatever stands by it.
const NARROWEST: f64 = 4.0;
/// Its widest, metres: in a slack reach of an open valley.
const WIDEST: f64 = 16.0;
/// A river polygon this wide on average (twice its area over its outline's length) is a river in its
/// own right, metres, as are the sea and a lake this wide. A narrower one that a river's line runs
/// through is a widening of that river.
const TRUNK_WIDTH: f64 = 30.0;
/// The centre line is sampled this often, metres.
const STEP: f64 = 2.0;
/// OpenStreetMap's corners are rounded over this many samples either way.
const ROUNDED: usize = 3;
/// How many samples a channel runs on into the wide water at its ends, so no dry sliver is left.
const INTO_TRUNK: usize = 6;
/// A line that crosses no more of a wide water's island than this, metres, is that water's own
/// centre line drawn over the island, and not a channel through it.
const ISLAND_CROSSING: f64 = 100.0;
/// The valley's floor is the ground no higher than this over its lowest, metres...
const VALLEY_RISE: f64 = 1.0;
/// ...looked for this far to each side of the line, metres.
const VALLEY_REACH: i32 = 10;
/// How much of what the valley is wider or narrower than its river's average the river takes.
const VALLEY_SHARE: f64 = 0.45;
/// How much the width wanders by itself, metres either way, and how long its two waves are.
const WIDTH_WANDER: [(f64, f64); 2] = [(60.0, 3.0), (19.0, 1.2)];
/// A river falling more steeply than the first of these, metres a metre, narrows, by all of
/// `PINCH` (metres) from the second.
const PINCH_SLOPES: [f64; 2] = [0.004, 0.03];
const PINCH: f64 = 3.5;
/// The width changes no faster than this, metres a metre along the river.
const WIDTH_EASE: f64 = 0.3;
/// Each bank wanders this much by itself, metres, where there is room for it.
const BANK_WANDER: f64 = 0.8;
/// ...over bays and points about this long, metres.
const BANK_WANDERS: f64 = 11.0;
/// The bank keeps this far from a wall or a road's edge, metres.
const ELBOW_ROOM: f64 = 1.0;
/// Nothing further than this from a river's line narrows it, metres.
const CROWDING: f64 = 24.0;
/// The profile's median is over this many samples either way: an embankment or a pit is no step.
const PROFILE_MEDIAN: usize = 6;
/// ...and it is then evened out over this many.
const PROFILE_MEAN: usize = 10;
/// The surface stays this far under the lower bank, metres.
const UNDER_BANK: f64 = 0.15;
/// How far out of the water the bank's crest is looked for, metres.
const CREST_REACH: i32 = 4;
/// A channel comes to the level of the water at its ends over this far, metres.
pub(crate) const MOUTH: f64 = 30.0;

/// One channel: a river's line from the wide water (or the edge of the world) it starts at to what
/// it runs out into, in the direction the water runs.
#[derive(Clone, Debug)]
pub struct Channel {
    /// The centre line, a point every `STEP` or so, local metres.
    pub points: Vec<P>,
    /// How far along the line each point is, metres.
    pub arc: Vec<f64>,
    /// Half the water's width at each point, metres.
    pub half: Vec<f64>,
    /// The surface at each point as the valley has it, metres above the height model's datum, before
    /// it is held to the water at its ends (`held`).
    pub profile: Vec<f64>,
}

/// What a channel narrows to get by, in local metres.
#[derive(Clone, Debug, Default)]
pub struct Beside {
    /// Buildings' outlines.
    pub buildings: Vec<Vec<P>>,
    /// Roads, paths and tracks on the ground, with their widths. A bridge's own way is not among
    /// them, but the road that leads up to its end is, so a short bridge narrows the river under it.
    pub ways: Vec<(Vec<P>, f64)>,
}

impl Beside {
    /// The buildings, roads and tracks of `osm`, projected by `frame`.
    pub fn from_osm(osm: &crate::overpass::Osm, frame: shared::world::Frame) -> Self {
        let mut beside = Self::default();
        for way in &osm.ways {
            let tag = |key: &str| way.tags.get(key).map_or("", String::as_str);
            let line = || -> Vec<P> { way.geometry.iter().map(|g| frame.project(g.lat, g.lon)).collect() };
            if !matches!(tag("building"), "" | "no" | "roof") {
                beside.buildings.push(line());
                continue;
            }
            if crate::assemble::is_bridge(way) || !matches!(tag("tunnel"), "" | "no") {
                continue;
            }
            if let Some((width, ..)) = crate::lanes::style(&way.tags) {
                beside.ways.push((line(), width));
            } else if let Some((gauge, _)) = crate::assemble::rail_style(way) {
                beside.ways.push((line(), crate::assemble::ballast(gauge)));
            }
        }
        beside
    }
}

/// Turns `map.river_lines` into channels: adds the bodies of their water under the mapped ones, so
/// those win where they overlap, and marks the pools along them. `inside` is the world, with room
/// to spare, and `model(x, z)` the height model, metres.
pub fn lay(map: &mut Map, beside: &Beside, inside: impl Fn(P) -> bool, mut model: impl FnMut(f64, f64) -> Option<f64>) {
    if map.river_lines.is_empty() {
        return;
    }
    let shapes: Vec<Shape> = map.bodies.iter().map(|body| Shape::new(&body.rings)).collect();
    let trunk: Vec<bool> = map.bodies.iter().zip(&shapes).map(|(body, shape)| body.kind == Kind::Sea || shape.width >= TRUNK_WIDTH).collect();
    let courses: Vec<Vec<P>> = courses(&map.river_lines).iter().map(|line| rounded(&crate::bridge::resample(line, STEP))).filter(|line| line.len() >= 2).collect();
    let pools = pools(&map.bodies, &shapes, &trunk, &courses, &inside);

    let mut runs: Vec<Vec<P>> = Vec::new();
    for course in &courses {
        let wide: Vec<Option<usize>> = course.iter().map(|&p| (0..shapes.len()).find(|&k| trunk[k] && shapes[k].contains(p))).collect();
        let open: Vec<bool> = course.iter().zip(&wide).map(|(&p, wide)| wide.is_none() && inside(p)).collect();
        let mut first = 0;
        while first < course.len() {
            if !open[first] {
                first += 1;
                continue;
            }
            let end = (first..course.len()).find(|&k| !open[k]).unwrap_or(course.len());
            let before = first.checked_sub(1).and_then(|k| wide[k]);
            let after = wide.get(end).copied().flatten();
            let over_island = before.is_some_and(|k| {
                after == Some(k) && (end - first) as f64 * STEP <= ISLAND_CROSSING && map.bodies[k].rings[1..].iter().any(|hole| crate::assemble::point_in_polygon(course[(first + end) / 2], hole))
            });
            if !over_island {
                let from = before.map_or(first, |_| (first.saturating_sub(INTO_TRUNK)..first).find(|&k| wide[k].is_some()).unwrap_or(first));
                let to = after.map_or(end, |_| (end..(end + INTO_TRUNK).min(course.len())).rev().find(|&k| wide[k].is_some()).map_or(end, |k| k + 1));
                if to - from >= 2 {
                    runs.push(course[from..to].to_vec());
                }
            }
            first = end;
        }
    }
    // The longest first: what joins it along the way is then levelled to it.
    runs.sort_by_key(|run| std::cmp::Reverse(run.len()));
    let crowd = Crowd::new(beside);
    let in_pool = |p: P| pools.iter().any(|&k| shapes[k].contains(p));
    let mut channels = Vec::new();
    let mut waters = Vec::new();
    for points in runs {
        let Some((channel, room)) = measure(points, &crowd, &in_pool, &mut model) else { continue };
        waters.push(Body { kind: Kind::River, rings: outline(&channel, &room), level: None });
        channels.push(channel);
    }
    if !channels.is_empty() {
        let width = channels.iter().flat_map(|channel| &channel.half).sum::<f64>() * 2.0 / channels.iter().map(|channel| channel.half.len()).sum::<usize>() as f64;
        let length = channels.iter().filter_map(|channel| channel.arc.last()).sum::<f64>();
        eprintln!("water: {} channels along river lines, {length:.0} m of them {width:.1} m wide on average, {} pools along them", channels.len(), pools.len());
    }
    for &pool in &pools {
        map.bodies[pool].kind = Kind::River;
    }
    let count = waters.len();
    // Filled in this order, so the longest channel is over the others, and the mapped water over all.
    waters.reverse();
    map.bodies.splice(0..0, waters);
    map.narrow = (0..count).chain(pools.iter().map(|pool| pool + count)).collect();
    map.channels = channels;
}

/// The bodies that are widenings of the rivers: the narrow water a line runs through, and the
/// narrow river polygons that lie up against those.
fn pools(bodies: &[Body], shapes: &[Shape], trunk: &[bool], courses: &[Vec<P>], inside: &impl Fn(P) -> bool) -> BTreeSet<usize> {
    let narrow: Vec<usize> = (0..bodies.len()).filter(|&k| !trunk[k]).collect();
    let mut found: BTreeSet<usize> = narrow.iter().copied().filter(|&k| courses.iter().flatten().filter(|&&p| shapes[k].contains(p) && inside(p)).nth(1).is_some()).collect();
    loop {
        let touching: Vec<usize> = narrow
            .iter()
            .copied()
            .filter(|&k| bodies[k].kind == Kind::River && !found.contains(&k))
            .filter(|&k| {
                found.iter().any(|&pool| {
                    shapes[k].near(&shapes[pool])
                        && bodies[k].rings.iter().flatten().any(|&[x, z]| [[x + 1.0, z], [x - 1.0, z], [x, z + 1.0], [x, z - 1.0]].into_iter().any(|p| shapes[pool].contains(p)))
                })
            })
            .collect();
        if touching.is_empty() {
            return found;
        }
        found.extend(touching);
    }
}

/// The ways joined end to start into courses. Where rivers meet or part, the two that make the
/// longest course run on as one, and the others end or start on it.
fn courses(lines: &[Vec<P>]) -> Vec<Vec<P>> {
    let same = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) < 0.01;
    let mut left: Vec<(Vec<P>, f64)> = lines.iter().filter(|line| line.len() >= 2).map(|line| (line.clone(), arcs(line).last().copied().unwrap_or(0.0))).collect();
    loop {
        let mut best: Option<(f64, usize, usize)> = None;
        for (i, (line, length)) in left.iter().enumerate() {
            let end = line[line.len() - 1];
            for (j, (next, onward)) in left.iter().enumerate() {
                if i != j && same(end, next[0]) && !same(line[0], next[next.len() - 1]) && best.is_none_or(|(longest, ..)| length + onward > longest) {
                    best = Some((length + onward, i, j));
                }
            }
        }
        let Some((length, i, j)) = best else { return left.into_iter().map(|(line, _)| line).collect() };
        let next = std::mem::take(&mut left[j].0);
        left[i].0.extend(next.into_iter().skip(1));
        left[i].1 = length;
        left.remove(j);
    }
}

/// The line with its corners rounded over `ROUNDED` samples either way, its ends where they were.
fn rounded(line: &[P]) -> Vec<P> {
    let last = line.len().saturating_sub(1);
    (0..line.len())
        .map(|k| {
            let reach = ROUNDED.min(k).min(last - k);
            let near = &line[k - reach..=k + reach];
            [0, 1].map(|axis| near.iter().map(|p| p[axis]).sum::<f64>() / near.len() as f64)
        })
        .collect()
}

/// How far along the line each of its points is, metres.
fn arcs(line: &[P]) -> Vec<f64> {
    let mut along = 0.0;
    std::iter::once(0.0)
        .chain(line.windows(2).map(|pair| {
            along += (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
            along
        }))
        .collect()
}

/// The median of what is known within `reach` samples of each, infinite where nothing is.
fn median(values: &[f64], reach: usize) -> Vec<f64> {
    (0..values.len())
        .map(|k| {
            let mut near: Vec<f64> = values[k.saturating_sub(reach)..(k + reach + 1).min(values.len())].iter().copied().filter(|v| v.is_finite()).collect();
            near.sort_by(f64::total_cmp);
            near.get(near.len() / 2).copied().unwrap_or(f64::INFINITY)
        })
        .collect()
}

/// The mean within `reach` samples of each, fewer towards the ends so those keep their values and a
/// sequence that only falls still does.
fn mean(values: &[f64], reach: usize) -> Vec<f64> {
    let last = values.len().saturating_sub(1);
    (0..values.len())
        .map(|k| {
            let reach = reach.min(k).min(last - k);
            values[k - reach..=k + reach].iter().sum::<f64>() / (2 * reach + 1) as f64
        })
        .collect()
}

/// The highest sequence under `values` that never rises.
fn falling(values: &[f64]) -> Vec<f64> {
    let mut lowest = f64::INFINITY;
    values
        .iter()
        .map(|&value| {
            lowest = lowest.min(value);
            lowest
        })
        .collect()
}

/// The unit direction downstream at point `k`, over the rounding's reach either way.
fn heading(points: &[P], k: usize) -> P {
    let (a, b) = (points[k.saturating_sub(ROUNDED)], points[(k + ROUNDED).min(points.len() - 1)]);
    let length = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
    [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
}

/// The channel along `points`, and how much each bank may wander by itself at each of them. `None`
/// where the height model knows nothing of it.
fn measure(points: Vec<P>, crowd: &Crowd, in_pool: &impl Fn(P) -> bool, model: &mut impl FnMut(f64, f64) -> Option<f64>) -> Option<(Channel, Vec<f64>)> {
    let arc = arcs(&points);
    let count = points.len();
    let reach = (WIDEST / 2.0).ceil() as i32 + CREST_REACH;
    let reach = reach.max(VALLEY_REACH);
    // The ground across the river at each point, a metre apart, the right bank to the left.
    let across: Vec<Vec<f64>> = (0..count)
        .map(|k| {
            let ([x, z], [dx, dz]) = (points[k], heading(&points, k));
            (-reach..=reach).map(|o| model(x - dz * f64::from(o), z + dx * f64::from(o)).unwrap_or(f64::INFINITY)).collect()
        })
        .collect();
    let at = |k: usize, offset: i32| across[k][(offset + reach) as usize];
    let lowest = |k: usize, within: i32| (-within..=within).map(|o| at(k, o)).fold(f64::INFINITY, f64::min);

    // The valley's fall, before the width is known, for the pinch.
    let floor: Vec<f64> = (0..count).map(|k| lowest(k, (NARROWEST / 2.0) as i32)).collect();
    let floor = mean(&falling(&median(&floor, PROFILE_MEDIAN)), PROFILE_MEAN);
    if floor.iter().any(|height| !height.is_finite()) {
        return None;
    }
    let valley: Vec<f64> = (0..count)
        .map(|k| {
            let top = at(k, 0).min(floor[k]) + VALLEY_RISE;
            [-1, 1].into_iter().map(|side| f64::from((1..=VALLEY_REACH).find(|&o| at(k, side * o) > top).unwrap_or(VALLEY_REACH + 1) - 1)).sum()
        })
        .collect();
    let valley = mean(&valley, PROFILE_MEAN);
    // Seeded by where the river starts, so two rivers are not alike and every import is.
    let seed = (points[0][0] * 0.37 + points[0][1] * 0.61).floor();
    let wanted: Vec<f64> = (0..count)
        .map(|k| {
            let (back, on) = (k.saturating_sub(PROFILE_MEAN), (k + PROFILE_MEAN).min(count - 1));
            let slope = (floor[back] - floor[on]) / (arc[on] - arc[back]).max(STEP);
            let t = ((slope - PINCH_SLOPES[0]) / (PINCH_SLOPES[1] - PINCH_SLOPES[0])).clamp(0.0, 1.0);
            let wander: f64 = WIDTH_WANDER.iter().enumerate().map(|(wave, &(long, by))| by * noise(arc[k] / long, seed, 41 + wave as u64)).sum();
            MEAN_WIDTH + VALLEY_SHARE * (valley[k] - MEAN_WIDTH) + wander - PINCH * t
        })
        .collect();
    let most: Vec<f64> = points.iter().map(|&p| (2.0 * crowd.room(p)).clamp(NARROWEST, WIDEST)).collect();
    // A pool has its own outline: only what is cut counts towards the average.
    let counted: Vec<bool> = points.iter().map(|&p| !in_pool(p)).collect();
    let counts = counted.iter().filter(|&&counted| counted).count();
    let widths_with = |more: f64| -> Vec<f64> {
        let mut width: Vec<f64> = wanted.iter().zip(&most).map(|(wanted, &most)| (wanted + more).clamp(NARROWEST, most)).collect();
        for k in 1..count {
            width[k] = width[k].min(width[k - 1] + WIDTH_EASE * (arc[k] - arc[k - 1]));
        }
        for k in (0..count - 1).rev() {
            width[k] = width[k].min(width[k + 1] + WIDTH_EASE * (arc[k + 1] - arc[k]));
        }
        width
    };
    let (mut less, mut more) = (-2.0 * WIDEST, 2.0 * WIDEST);
    for _ in 0..24 {
        let middle = (less + more) / 2.0;
        let carved: f64 = widths_with(middle).iter().zip(&counted).filter(|(_, counted)| **counted).map(|(width, _)| width).sum();
        if carved < MEAN_WIDTH * counts as f64 { less = middle } else { more = middle }
    }
    let width = widths_with(if counts > 0 { (less + more) / 2.0 } else { 0.0 });
    let room: Vec<f64> = width.iter().zip(&most).map(|(width, most)| BANK_WANDER.min((width - NARROWEST) / 2.0).min((most - width) / 2.0).max(0.0)).collect();

    // The surface: the lowest across the water, under the lower of the two banks' crests.
    let mut low = Vec::with_capacity(count);
    let mut crest = Vec::with_capacity(count);
    for k in 0..count {
        let edge = (width[k] / 2.0).ceil() as i32;
        low.push(lowest(k, edge));
        let highest = |side: i32| (edge..=edge + CREST_REACH).map(|o| at(k, side * o)).filter(|h| h.is_finite()).fold(f64::NEG_INFINITY, f64::max);
        let lower = highest(-1).min(highest(1));
        crest.push(if in_pool(points[k]) || !lower.is_finite() { f64::INFINITY } else { lower - UNDER_BANK });
    }
    let low = median(&low, PROFILE_MEDIAN);
    let crest = median(&crest, PROFILE_MEDIAN);
    let under = |values: &[f64]| falling(&values.iter().zip(&crest).map(|(value, crest)| value.min(*crest)).collect::<Vec<_>>());
    let profile = under(&mean(&under(&low), PROFILE_MEAN));
    if profile.iter().any(|height| !height.is_finite()) {
        return None;
    }
    Some((Channel { half: width.iter().map(|width| width / 2.0).collect(), points, arc, profile }, room))
}

impl Channel {
    /// How long it is, metres.
    pub(crate) fn length(&self) -> f64 {
        self.arc.last().copied().unwrap_or(0.0)
    }

    /// The surface along it held to the water at its ends: it starts at the level of what it comes
    /// from and ends at that of what it runs out into, which backs up into it as far as it stands
    /// higher. It still only falls.
    pub(crate) fn held(&self, from: Option<f64>, to: Option<f64>) -> Vec<f64> {
        let to = match (from, to) {
            (Some(from), Some(to)) => Some(to.min(from)),
            _ => to,
        };
        let length = self.length();
        let over = MOUTH.min(length / 2.0).max(1e-6);
        let out: Vec<f64> = self
            .profile
            .iter()
            .zip(&self.arc)
            .map(|(&own, &arc)| {
                let mut level = own;
                if let Some(from) = from {
                    level = from + (level.min(from) - from) * (arc / over).min(1.0);
                }
                if let Some(to) = to {
                    level = to + (level.max(to) - to) * ((length - arc) / over).min(1.0);
                }
                level
            })
            .collect();
        falling(&out)
    }

    /// The unit direction downstream `along` metres along it.
    pub(crate) fn course(&self, along: f64) -> P {
        heading(&self.points, self.arc.partition_point(|&arc| arc < along).min(self.points.len() - 1))
    }

    /// Calls `visit(i, j, beyond, along)` for each sample of `grid` within `margin` of the water's edge
    /// or inside it, once for each segment of the line near it: how far beyond the edge it is, metres
    /// (negative in the water), and how far along the line.
    fn around(&self, grid: Grid, margin: f64, mut visit: impl FnMut(i64, i64, f64, f64)) {
        let (origin, cell) = (f64::from(grid.origin) / 100.0, f64::from(grid.cell) / 100.0);
        for k in 0..self.points.len() - 1 {
            let (a, b) = (self.points[k], self.points[k + 1]);
            let (ha, hb) = (self.half[k], self.half[k + 1]);
            let reach = ha.max(hb) + margin;
            let span = |axis: usize| ((a[axis].min(b[axis]) - reach - origin) / cell).floor() as i64..=((a[axis].max(b[axis]) + reach - origin) / cell).ceil() as i64;
            for j in span(1) {
                for i in span(0) {
                    let p = [origin + i as f64 * cell, origin + j as f64 * cell];
                    let (d, t) = nearest(p, a, b);
                    if d <= reach {
                        visit(i, j, d - (ha + (hb - ha) * t), self.arc[k] + (self.arc[k + 1] - self.arc[k]) * t);
                    }
                }
            }
        }
    }
}

/// Where a sample of narrow water is among the channels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct At {
    /// Which channel's water it is.
    pub channel: usize,
    /// How far along the channel, metres, or where the pool it is in leaves it.
    pub along: f64,
    /// How far out in a pool it is from the channel itself, metres through the water; 0 in it.
    pub out: f64,
}

/// Where each sample of narrow water is among the channels: `narrow(sample)` says whether a sample
/// is a channel's or a pool's. One in a channel takes the nearest of the line, and one further out
/// in a pool takes the channel it is reached from, the shortest way through the water.
pub(crate) fn courses_at(channels: &[Channel], grid: Grid, narrow: impl Fn(usize) -> bool) -> BTreeMap<usize, At> {
    let side = i64::from(grid.side);
    let mut nearest_to: BTreeMap<usize, (f64, usize, f64)> = BTreeMap::new();
    for (index, channel) in channels.iter().enumerate() {
        // A sample over the banks' wander is still the channel's.
        channel.around(grid, 2.0 * BANK_WANDER, |i, j, beyond, along| {
            if i < 0 || j < 0 || i >= side || j >= side {
                return;
            }
            let at = (j * side + i) as usize;
            if narrow(at) && nearest_to.get(&at).is_none_or(|&(known, ..)| beyond < known) {
                nearest_to.insert(at, (beyond, index, along));
            }
        });
    }
    let mut found: BTreeMap<usize, At> = nearest_to.into_iter().map(|(at, (_, channel, along))| (at, At { channel, along, out: 0.0 })).collect();
    let cell = f64::from(grid.cell) / 100.0;
    let mut queue: VecDeque<usize> = found.keys().copied().collect();
    while let Some(at) = queue.pop_front() {
        let (i, j) = (at as i64 % side, at as i64 / side);
        let from = found[&at];
        for (di, dj) in [(0, -1), (-1, 0), (1, 0), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
            let (ni, nj) = (i + di, j + dj);
            if ni < 0 || nj < 0 || ni >= side || nj >= side {
                continue;
            }
            let next = (nj * side + ni) as usize;
            if !found.contains_key(&next) && narrow(next) {
                found.insert(next, At { out: from.out + cell * ((di * di + dj * dj) as f64).sqrt(), ..from });
                queue.push_back(next);
            }
        }
    }
    found
}

/// The distance from `p` to the segment `a → b`, and how far along the segment the nearest point is (0..1).
fn nearest(p: P, a: P, b: P) -> (f64, f64) {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let squared = dx * dx + dz * dz;
    let t = if squared > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / squared).clamp(0.0, 1.0) } else { 0.0 };
    ((p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dz), t)
}

/// The channel's outline: where the distance from the line less the half width crosses zero, traced
/// over a 1 m raster (marching squares). Offsetting the line to each side instead would fold over
/// itself in a bend tighter than the river is wide, and leave a hole there. An outer ring and, round
/// any island a loop of the river makes, holes.
fn outline(channel: &Channel, room: &[f64]) -> Vec<Vec<P>> {
    let metre = Grid { origin: 0, cell: 100, side: 0 };
    let mut field: HashMap<(i64, i64), f64> = HashMap::new();
    channel.around(metre, 2.0, |i, j, beyond, along| {
        let wander = interpolate(&channel.arc, room, along) * noise(i as f64 / BANK_WANDERS, j as f64 / BANK_WANDERS, 43);
        let value = beyond - wander;
        field.entry((i, j)).and_modify(|known| *known = known.min(value)).or_insert(value);
    });
    let value = |at: (i64, i64)| field.get(&at).copied().unwrap_or(1.0);
    // The raster's cells with any corner in the water, in order, so the rings are the same every run.
    let cells: BTreeSet<(i64, i64)> = field.iter().filter(|(_, value)| **value < 0.0).flat_map(|(&(i, j), _)| [(i, j), (i - 1, j), (i, j - 1), (i - 1, j - 1)]).collect();
    // A crossing is named by the raster's edge it is on: its first end, and whether it runs along z.
    type Crossing = (i64, i64, bool);
    let mut next: BTreeMap<Crossing, Crossing> = BTreeMap::new();
    for (i, j) in cells {
        let corners = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
        let edges: [Crossing; 4] = [(i, j, false), (i + 1, j, true), (i, j + 1, false), (i, j, true)];
        let values = corners.map(value);
        let wet = values.map(|value| value < 0.0);
        let middle_wet = values.iter().sum::<f64>() < 0.0;
        // Round the cell, the water is left by one edge and entered again by another: that piece of
        // the bank has the water on its left. At a saddle the middle says which banks join.
        for leave in (0..4).filter(|&k| wet[k] && !wet[(k + 1) % 4]) {
            let turn = |step: usize| if middle_wet { (leave + step) % 4 } else { (leave + 4 - step) % 4 };
            if let Some(enter) = (1..4).map(turn).find(|&k| !wet[k] && wet[(k + 1) % 4]) {
                next.insert(edges[leave], edges[enter]);
            }
        }
    }
    let place = |(i, j, along_z): Crossing| -> P {
        let (a, b) = (value((i, j)), value(if along_z { (i, j + 1) } else { (i + 1, j) }));
        let t = (a / (a - b)).clamp(0.0, 1.0);
        if along_z { [i as f64, j as f64 + t] } else { [i as f64 + t, j as f64] }
    };
    let mut rings = Vec::new();
    while let Some((&start, _)) = next.first_key_value() {
        let mut ring = Vec::new();
        let mut at = start;
        while let Some(following) = next.remove(&at) {
            ring.push(place(at));
            at = following;
        }
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }
    // The outer ring is the one that reaches furthest west.
    let west = |ring: &Vec<P>| ring.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    rings.sort_by(|a, b| west(a).total_cmp(&west(b)));
    rings
}

/// A body's rings, indexed by rows for asking whether a point is in it.
struct Shape {
    /// `[x0, z0, x1, z1]`.
    bounds: [f64; 4],
    /// The edges that reach into each row of `ROW` metres from the top of the bounds.
    rows: Vec<Vec<(P, P)>>,
    /// Twice the area over the length of the outline, metres: a long water's mean width.
    width: f64,
}

/// How deep a row of a `Shape`'s index is, metres.
const ROW: f64 = 16.0;

impl Shape {
    fn new(rings: &[Vec<P>]) -> Self {
        let bounds = rings.iter().flatten().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]);
        if bounds[0] > bounds[2] {
            return Self { bounds, rows: Vec::new(), width: 0.0 };
        }
        let mut rows = vec![Vec::new(); ((bounds[3] - bounds[1]) / ROW) as usize + 1];
        let (mut area, mut round) = (0.0, 0.0);
        for (index, ring) in rings.iter().enumerate() {
            let mut twice = 0.0;
            for (k, &b) in ring.iter().enumerate() {
                let a = ring[(k + ring.len() - 1) % ring.len()];
                twice += a[0] * b[1] - b[0] * a[1];
                round += (b[0] - a[0]).hypot(b[1] - a[1]);
                let row = |z: f64| ((z - bounds[1]) / ROW) as usize;
                for row in &mut rows[row(a[1].min(b[1]))..=row(a[1].max(b[1]))] {
                    row.push((a, b));
                }
            }
            area += if index == 0 { twice.abs() } else { -twice.abs() };
        }
        Self { bounds, rows, width: if round > 0.0 { area.abs() / round } else { 0.0 } }
    }

    fn contains(&self, [x, z]: P) -> bool {
        let [x0, z0, x1, z1] = self.bounds;
        if x < x0 || x > x1 || z < z0 || z > z1 {
            return false;
        }
        self.rows[((z - z0) / ROW) as usize].iter().filter(|(a, b)| (a[1] > z) != (b[1] > z) && x < a[0] + (z - a[1]) * (b[0] - a[0]) / (b[1] - a[1])).count() % 2 == 1
    }

    /// Whether the two bodies' bounds come within a metre of each other.
    fn near(&self, other: &Shape) -> bool {
        (0..2).all(|axis| self.bounds[axis] <= other.bounds[axis + 2] + 1.0 && other.bounds[axis] <= self.bounds[axis + 2] + 1.0)
    }
}

/// A piece of what stands beside a river: a segment, and how far from its middle line it reaches, metres.
type Stands = (P, P, f64);

/// What stands beside the rivers, in squares `CROWDING` across.
struct Crowd {
    squares: HashMap<(i64, i64), Vec<Stands>>,
}

impl Crowd {
    fn new(beside: &Beside) -> Self {
        let mut squares: HashMap<(i64, i64), Vec<Stands>> = HashMap::new();
        let square = |v: f64| (v / CROWDING).floor() as i64;
        let lines = beside.ways.iter().map(|(line, width)| (line, width / 2.0)).chain(beside.buildings.iter().map(|ring| (ring, 0.0)));
        for (line, half) in lines {
            for pair in line.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                for i in square(a[0].min(b[0]))..=square(a[0].max(b[0])) {
                    for j in square(a[1].min(b[1]))..=square(a[1].max(b[1])) {
                        squares.entry((i, j)).or_default().push((a, b, half));
                    }
                }
            }
        }
        Self { squares }
    }

    /// How far out from `p` the water may reach, metres.
    fn room(&self, p: P) -> f64 {
        let (i, j) = ((p[0] / CROWDING).floor() as i64, (p[1] / CROWDING).floor() as i64);
        (-1..=1)
            .flat_map(|di| (-1..=1).map(move |dj| (i + di, j + dj)))
            .filter_map(|square| self.squares.get(&square))
            .flatten()
            .map(|&(a, b, half)| nearest(p, a, b).0 - half - ELBOW_ROOM)
            .fold(CROWDING - ELBOW_ROOM, f64::min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A river drawn as `line` in a valley whose floor falls 2 m a km to the east, laid out.
    fn laid(line: Vec<P>, beside: &Beside) -> Map {
        let mut map = Map { river_lines: vec![line], ..Map::default() };
        lay(&mut map, beside, |_| true, |x, z| Some(10.0 - x * 0.002 + 0.2 * z.abs().min(12.0)));
        map
    }

    /// How much of the line across at `x` is in the water, metres.
    fn width(map: &Map, x: f64) -> f64 {
        let shape = Shape::new(&map.bodies[0].rings);
        (-300..=300).filter(|&k| shape.contains([x, f64::from(k) * 0.05])).count() as f64 * 0.05
    }

    #[test]
    fn a_river_drawn_as_a_line_is_ten_metres_across_on_average_and_never_under_four() {
        let map = laid(vec![[-600.0, 0.0], [600.0, 0.0]], &Beside::default());
        let widths: Vec<f64> = (-590..=590).step_by(5).map(|x| width(&map, f64::from(x) + 0.31)).collect();
        let average = widths.iter().sum::<f64>() / widths.len() as f64;
        assert!((average - MEAN_WIDTH).abs() < 0.6, "{average}");
        let (least, most) = widths.iter().fold((f64::MAX, f64::MIN), |(least, most), &w| (least.min(w), most.max(w)));
        assert!(least >= NARROWEST - 0.2, "{least}");
        assert!(most - least > 2.0, "not a ditch of one width: {least} to {most}");
    }

    #[test]
    fn a_river_narrows_past_a_house_and_under_a_short_bridge() {
        let beside = Beside {
            buildings: vec![vec![[-110.0, 3.5], [-90.0, 3.5], [-90.0, 12.0], [-110.0, 12.0], [-110.0, 3.5]]],
            // The road up to each end of a bridge 10 m long at x = 100.
            ways: vec![(vec![[100.0, -40.0], [100.0, -5.0]], 4.0), (vec![[100.0, 5.0], [100.0, 40.0]], 4.0)],
        };
        let map = laid(vec![[-300.0, 0.0], [300.0, 0.0]], &beside);
        for x in [-99.7, 100.3] {
            assert!((3.7..5.6).contains(&width(&map, x)), "{} at {x}", width(&map, x));
        }
        let open: f64 = (-40..40).step_by(4).map(|x| width(&map, f64::from(x) + 0.3)).sum::<f64>() / 20.0;
        assert!(open > 6.0, "and is wider where nothing stands by it: {open}");
    }

    #[test]
    fn a_sharp_bend_leaves_no_hole_in_the_river() {
        // A hairpin whose sides are closer than the river is wide.
        let map = laid(vec![[-80.0, -3.0], [40.0, -3.0], [42.0, 0.0], [40.0, 3.0], [-80.0, 3.0]], &Beside::default());
        let shape = Shape::new(&map.bodies[0].rings);
        for &p in &map.channels[0].points {
            assert!(shape.contains([p[0] + 0.01, p[1] + 0.013]), "dry on the line at {p:?}");
        }
    }

    #[test]
    fn where_rivers_meet_the_longest_course_runs_on() {
        let lines = vec![vec![[-30.0, 40.0], [0.0, 0.0]], vec![[-300.0, 0.0], [0.0, 0.0]], vec![[0.0, 0.0], [200.0, 0.0]]];
        let found = courses(&lines);
        assert_eq!(found.len(), 2);
        assert!(found.contains(&vec![[-300.0, 0.0], [0.0, 0.0], [200.0, 0.0]]), "{found:?}");
    }

    #[test]
    fn a_river_is_cut_through_a_rise_and_never_stands_over_its_banks() {
        let mut map = Map { river_lines: vec![vec![[-300.0, 0.0], [300.0, 0.0]]], ..Map::default() };
        // A dam 60 m long across the valley, and a reach where the ground beside the line is lower.
        let ground = |x: f64, z: f64| {
            if x.abs() < 30.0 { 14.0 } else if (150.0..220.0).contains(&x) && z > 3.0 { 7.0 } else { 10.0 - x * 0.002 + 0.1 * z.abs() }
        };
        lay(&mut map, &Beside::default(), |_| true, |x, z| Some(ground(x, z)));
        let channel = &map.channels[0];
        assert!(channel.profile.windows(2).all(|pair| pair[1] <= pair[0]), "it only falls");
        let level = |x: f64| interpolate(&channel.arc, &channel.profile, x + 300.0);
        assert!(level(0.0) < 10.8, "cut through the rise: {}", level(0.0));
        assert!(level(185.0) < 7.0, "under the low bank: {}", level(185.0));
    }
}
