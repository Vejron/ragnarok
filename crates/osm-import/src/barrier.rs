//! The guard rail down the middle of a divided road (mitträcke), inferred from OpenStreetMap.
//!
//! OSM draws a road divided by a barrier as two one-way ways, one per carriageway, and does not tag
//! the barrier: a Swedish 2+1 road is a `lanes=1` way and a `lanes=2` way 6 to 12 metres apart, a
//! motorway's carriageways 10 to 24 metres. Driving on the right, the other carriageway is on the left.
//! So a rail is stood where a fast one-way carriageway has another heading the other way close on its
//! left, between their inner edges, and left open where a road joins either of them, as the real
//! barrier is at a junction. Where the two lie close, the strip between is paved, as it is on a 2+1
//! road; a slow divided road (a town's) is paved between too, with a solid line instead of a rail.

use std::collections::HashMap;

type P = [f64; 2];

/// How far apart samples along a carriageway are, metres.
const STEP: f64 = 4.0;
/// The farthest the other carriageway's centre line may be, metres: farther, they are two roads.
const PAIR_REACH: f64 = 30.0;
/// How nearly opposite the two carriageways must run: the cosine of the angle between them, negated.
const OPPOSITE: f64 = 0.9;
/// How far the rail stops short of a junction on either carriageway, metres.
const JUNCTION_CLEAR: f64 = 20.0;
/// The shortest length of rail kept, metres.
const SHORTEST: f64 = 20.0;
/// The farthest the rail's line may jump between samples before it is cut, metres: the pairing changed.
const JUMP: f64 = 8.0;
/// The widest gap between the two carriageways' edges that is paved over, metres; wider is a verge.
const PAVED_GAP: f64 = 4.0;
/// How far the paved strip reaches under each carriageway's edge, metres: far enough that their edges
/// lie inside the paved surface and do not crumble into the verge (`Reach` in the client's roads.ts),
/// short of where a carriageway's lanes start; the rail's side of it has no edge line.
const STRIP_OVERLAP: f64 = 1.0;

/// A driven road, as the barrier is worked out from: its centre line in the way it is driven (a
/// one-way road's), half its width, whether it is a carriageway of a divided road, and whether it is
/// fast enough for a rail.
pub struct Way {
    pub points: Vec<P>,
    pub half: f64,
    /// A one-way through road: a `motorway`, `trunk`, `primary` or `secondary` carriageway.
    pub carriageway: bool,
    /// Driven fast enough for a rail down the middle (a motorway, a motorroad, 70 km/h and over).
    pub fast: bool,
}

/// What runs along a carriageway's left side, most of its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beside {
    /// The verge, or nothing near.
    Open,
    /// The other carriageway close by, paved up to it, with a rail between.
    Rail,
    /// The other carriageway close by, paved up to it, with no rail: a slow divided road.
    Paved,
}

/// The rails, the paved strips between close carriageways, and what runs beside each way.
#[derive(Debug, Default)]
pub struct Barrier {
    /// Each rail's line.
    pub rails: Vec<Vec<P>>,
    /// Each paved strip's centre line and width, metres.
    pub strips: Vec<(Vec<P>, f64)>,
    /// For each way, what runs along its left side.
    pub beside: Vec<Beside>,
}

/// Where the other carriageway is beside a sample of one: the point between the two (where a rail
/// would stand), the gap between their edges, metres, which way it is, and whether a rail may stand
/// there (both fast, and clear of junctions).
struct Pair {
    at: P,
    gap: f64,
    other: usize,
    railed: bool,
}

/// The rails down the middle of the divided roads among `ways`.
pub fn lay(ways: &[Way]) -> Barrier {
    let junctions = junctions(ways);
    let junction_grid = grid(junctions.iter().map(|&p| (p, p)), JUNCTION_CLEAR);
    let segments: Vec<(usize, P, P)> = ways.iter().enumerate().filter(|(_, way)| way.carriageway).flat_map(|(i, way)| way.points.windows(2).map(move |pair| (i, pair[0], pair[1]))).collect();
    let segment_grid = grid(segments.iter().map(|&(_, a, b)| (a, b)), PAIR_REACH);
    let mut barrier = Barrier { beside: vec![Beside::Open; ways.len()], ..Default::default() };
    for (i, way) in ways.iter().enumerate().filter(|(_, way)| way.carriageway) {
        let clear = |p: P| !near(&junction_grid, &junctions, p, JUNCTION_CLEAR);
        let pairs: Vec<Option<Pair>> = along(&way.points)
            .map(|(q, d)| {
                nearest_opposite(i, q, d, ways, &segments, &segment_grid).map(|(j, q2, distance)| {
                    let u = [(q2[0] - q[0]) / distance, (q2[1] - q[1]) / distance];
                    let (ha, hb) = (way.half, ways[j].half);
                    let gap = distance - ha - hb;
                    let reach = if gap >= 0.0 { ha + gap / 2.0 } else { distance * ha / (ha + hb) };
                    let at = [q[0] + u[0] * reach, q[1] + u[1] * reach];
                    Pair { at, gap, other: j, railed: way.fast && ways[j].fast && clear(at) && clear(q) }
                })
            })
            .collect();
        let close: Vec<&Pair> = pairs.iter().flatten().filter(|pair| pair.gap < PAVED_GAP).collect();
        if close.len() * 2 >= pairs.len().max(1) {
            let railed = close.iter().filter(|pair| pair.railed).count();
            barrier.beside[i] = if railed * 2 >= close.len() { Beside::Rail } else { Beside::Paved };
        }
        // Only the first of a pair lays the rail and the paving, so neither is laid twice.
        let mut run: Vec<P> = Vec::new();
        let mut strip: Vec<(P, f64)> = Vec::new();
        let rail = |run: &mut Vec<P>, barrier: &mut Barrier| {
            if length(run) >= SHORTEST {
                barrier.rails.push(std::mem::take(run));
            }
            run.clear();
        };
        let pave = |strip: &mut Vec<(P, f64)>, barrier: &mut Barrier| {
            if strip.len() > 1 {
                // Carriageways that touch or overlap are paved over their meeting too, so their edges do not crumble there.
                let width = (strip.iter().map(|(_, gap)| gap).sum::<f64>() / strip.len() as f64).max(0.0) + 2.0 * STRIP_OVERLAP;
                barrier.strips.push((strip.iter().map(|(p, _)| *p).collect(), width));
            }
            strip.clear();
        };
        for pair in &pairs {
            let Some(pair) = pair.as_ref().filter(|pair| pair.other > i) else {
                rail(&mut run, &mut barrier);
                pave(&mut strip, &mut barrier);
                continue;
            };
            let jumped = |line: Option<P>| line.is_some_and(|last| (pair.at[0] - last[0]).hypot(pair.at[1] - last[1]) > JUMP);
            if pair.railed && !jumped(run.last().copied()) {
                run.push(pair.at);
            } else {
                rail(&mut run, &mut barrier);
                if pair.railed {
                    run.push(pair.at);
                }
            }
            if pair.gap < PAVED_GAP && !jumped(strip.last().map(|(p, _)| *p)) {
                strip.push((pair.at, pair.gap));
            } else {
                pave(&mut strip, &mut barrier);
                if pair.gap < PAVED_GAP {
                    strip.push((pair.at, pair.gap));
                }
            }
        }
        rail(&mut run, &mut barrier);
        pave(&mut strip, &mut barrier);
        // Where a wider two-way road forks into this carriageway and the other, both start on its
        // centre line, and until they have drawn apart the two together are narrower than the road
        // they came from: a waist in the asphalt. The fork is paved as wide as that road as far as
        // the carriageways' outer edges are closer than it.
        for from_start in [true, false] {
            let fork = if from_start { way.points[0] } else { way.points[way.points.len() - 1] };
            let ends_here = |other: &Way| [other.points[0], other.points[other.points.len() - 1]].iter().any(|end| (end[0] - fork[0]).hypot(end[1] - fork[1]) < 0.1);
            let wide = ways.iter().filter(|other| !other.carriageway && other.points.len() >= 2 && ends_here(other)).map(|other| 2.0 * other.half).fold(0.0, f64::max);
            let order: Vec<&Option<Pair>> = if from_start { pairs.iter().collect() } else { pairs.iter().rev().collect() };
            let mut apron = vec![fork];
            for (k, pair) in order.into_iter().enumerate() {
                match pair {
                    Some(pair) if pair.other > i && pair.gap + 2.0 * (way.half + ways[pair.other].half) < wide => apron.push(pair.at),
                    // The two lie on each other at the fork itself: no pair there yet, or one with a
                    // carriageway further off.
                    _ if k < 2 => {}
                    _ => break,
                }
            }
            if apron.len() > 1 {
                barrier.strips.push((apron, wide));
            }
        }
    }
    barrier
}

/// Points `STEP` apart along `points`, the last point too, each with the unit direction there.
fn along(points: &[P]) -> impl Iterator<Item = (P, P)> + '_ {
    let mut carried = 0.0;
    points.windows(2).enumerate().flat_map(move |(index, pair)| {
        let (a, b) = (pair[0], pair[1]);
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let mut out = Vec::new();
        if length > 1e-6 {
            let d = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
            let mut s = carried;
            while s < length {
                out.push(([a[0] + d[0] * s, a[1] + d[1] * s], d));
                s += STEP;
            }
            carried = s - length;
            if index + 2 == points.len() {
                out.push((b, d));
            }
        }
        out
    })
}

/// The nearest point on another carriageway running the other way on the left of `q` (heading `d`):
/// which way, the point, and how far it is.
fn nearest_opposite(own: usize, q: P, d: P, ways: &[Way], segments: &[(usize, P, P)], grid: &Grid) -> Option<(usize, P, f64)> {
    let mut best: Option<(usize, P, f64)> = None;
    for &k in grid.near(q) {
        let (j, a, b) = segments[k];
        if j == own || !ways[j].carriageway {
            continue;
        }
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        if length < 1e-6 || (d[0] * (b[0] - a[0]) + d[1] * (b[1] - a[1])) / length > -OPPOSITE {
            continue;
        }
        let t = (((q[0] - a[0]) * (b[0] - a[0]) + (q[1] - a[1]) * (b[1] - a[1])) / (length * length)).clamp(0.0, 1.0);
        let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let distance = (p[0] - q[0]).hypot(p[1] - q[1]);
        // Right of the way it runs is (-dz, dx), x east and z south; the other carriageway is on the left.
        let right = (p[0] - q[0]) * -d[1] + (p[1] - q[1]) * d[0];
        if distance > 1e-3 && distance <= PAIR_REACH && right < 0.0 && best.is_none_or(|(.., nearest)| distance < nearest) {
            best = Some((j, p, distance));
        }
    }
    best
}

/// Where another road joins a carriageway: a point of both that is not just the next stretch of the
/// same carriageway carrying on.
fn junctions(ways: &[Way]) -> Vec<P> {
    let key = |p: P| ((p[0] * 10.0).round() as i64, (p[1] * 10.0).round() as i64);
    let mut at: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
    for (i, way) in ways.iter().enumerate() {
        for (k, &p) in way.points.iter().enumerate() {
            at.entry(key(p)).or_default().push((i, k));
        }
    }
    let heading = |way: &Way, k: usize| -> P {
        let n = way.points.len();
        let (a, b) = if k + 1 < n { (way.points[k], way.points[k + 1]) } else { (way.points[k - 1], way.points[k]) };
        let length = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
        [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
    };
    let mut out = Vec::new();
    for meeting in at.values().filter(|meeting| meeting.len() > 1) {
        let on_carriageway = meeting.iter().any(|&(i, _)| ways[i].carriageway);
        if !on_carriageway {
            continue;
        }
        // Two ends of carriageways running on in the same direction are one road cut in two.
        let continuing = meeting.len() == 2 && {
            let [(i, k), (j, l)] = [meeting[0], meeting[1]];
            let end = |i: usize, k: usize| k == 0 || k + 1 == ways[i].points.len();
            let (a, b) = (heading(&ways[i], k), heading(&ways[j], l));
            ways[i].carriageway && ways[j].carriageway && end(i, k) && end(j, l) && a[0] * b[0] + a[1] * b[1] > 0.7
        };
        if !continuing {
            out.push(ways[meeting[0].0].points[meeting[0].1]);
        }
    }
    out
}

/// Segments (or points, as segments of no length) sorted into square cells, for finding those near a point.
struct Grid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
    empty: Vec<usize>,
}

impl Grid {
    /// The segments in the cell of `p` and its eight neighbours: every one within `cell` of it.
    fn near(&self, p: P) -> impl Iterator<Item = &usize> {
        let (cx, cz) = ((p[0] / self.cell).floor() as i64, (p[1] / self.cell).floor() as i64);
        (-1..=1).flat_map(move |dx| (-1..=1).map(move |dz| (cx + dx, cz + dz))).flat_map(|key| self.cells.get(&key).unwrap_or(&self.empty).iter())
    }
}

/// A grid of `cell` metres over the segments `a -> b`.
fn grid(segments: impl Iterator<Item = (P, P)>, cell: f64) -> Grid {
    let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (index, (a, b)) in segments.enumerate() {
        let lo = |i: usize| (a[i].min(b[i]) / cell).floor() as i64;
        let hi = |i: usize| (a[i].max(b[i]) / cell).floor() as i64;
        for cx in lo(0)..=hi(0) {
            for cz in lo(1)..=hi(1) {
                cells.entry((cx, cz)).or_default().push(index);
            }
        }
    }
    Grid { cell, cells, empty: Vec::new() }
}

/// Whether any of `points` (sorted into `grid`) is within `reach` of `p`.
fn near(grid: &Grid, points: &[P], p: P, reach: f64) -> bool {
    grid.near(p).any(|&k| (points[k][0] - p[0]).hypot(points[k][1] - p[1]) <= reach)
}

/// The length of a line, metres.
fn length(points: &[P]) -> f64 {
    points.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A carriageway along x at `z`, eastward or westward, `half` wide each side.
    fn carriageway(z: f64, east: bool, half: f64, from: f64, to: f64) -> Way {
        let points = if east { vec![[from, z], [to, z]] } else { vec![[to, z], [from, z]] };
        Way { points, half, carriageway: true, fast: true }
    }

    #[test]
    fn two_opposite_carriageways_eight_metres_apart_get_a_barrier_between_them() {
        // Eastward on the south side (+z), so westward is on its left, as driving on the right.
        let ways = [carriageway(4.0, true, 2.25, 0.0, 200.0), carriageway(-4.0, false, 4.0, 0.0, 200.0)];
        let barrier = lay(&ways);
        assert_eq!(barrier.rails.len(), 1, "{barrier:?}");
        let rail = &barrier.rails[0];
        assert!(length(rail) > 180.0);
        // Between the inner edges: the eastward one's at 1.75, the westward one's at 0.
        for p in rail {
            assert!((p[1] - 0.875).abs() < 0.01, "{p:?}");
        }
        assert_eq!(barrier.strips.len(), 1);
        assert!((barrier.strips[0].1 - (1.75 + 2.0 * STRIP_OVERLAP)).abs() < 0.01);
        assert_eq!(barrier.beside, vec![Beside::Rail, Beside::Rail]);
    }

    #[test]
    fn carriageways_driven_on_the_left_get_no_barrier() {
        let ways = [carriageway(-4.0, true, 2.25, 0.0, 200.0), carriageway(4.0, false, 4.0, 0.0, 200.0)];
        assert!(lay(&ways).rails.is_empty());
    }

    #[test]
    fn the_barrier_has_a_gap_at_a_junction() {
        let mut ways = vec![carriageway(4.0, true, 2.25, 0.0, 100.0), carriageway(4.0, true, 2.25, 100.0, 200.0), carriageway(-4.0, false, 4.0, 0.0, 200.0)];
        // A side road joins the eastward carriageway at x = 100, where it is cut in two.
        ways.push(Way { points: vec![[100.0, 4.0], [100.0, 60.0]], half: 3.0, carriageway: false, fast: false });
        let barrier = lay(&ways);
        assert_eq!(barrier.rails.len(), 2, "{barrier:?}");
        for rail in &barrier.rails {
            assert!(rail.iter().all(|p| (p[0] - 100.0).abs() > JUNCTION_CLEAR - STEP), "{rail:?}");
        }
    }

    #[test]
    fn a_carriageway_cut_in_two_keeps_its_rail_whole() {
        let ways = [carriageway(4.0, true, 2.25, 0.0, 100.0), carriageway(4.0, true, 2.25, 100.0, 200.0), carriageway(-4.0, false, 4.0, 0.0, 200.0)];
        let barrier = lay(&ways);
        let total: f64 = barrier.rails.iter().map(|rail| length(rail)).sum();
        assert!(total > 180.0, "{barrier:?}");
    }

    #[test]
    fn a_slow_divided_road_is_paved_between_but_gets_no_barrier() {
        let mut ways = [carriageway(4.0, true, 2.25, 0.0, 200.0), carriageway(-4.0, false, 4.0, 0.0, 200.0)];
        ways[1].fast = false;
        let barrier = lay(&ways);
        assert!(barrier.rails.is_empty());
        assert_eq!(barrier.strips.len(), 1);
        assert_eq!(barrier.beside, vec![Beside::Paved, Beside::Paved]);
    }

    #[test]
    fn carriageways_that_touch_are_paved_over_where_they_meet() {
        let ways = [carriageway(3.9, true, 4.0, 0.0, 200.0), carriageway(-3.9, false, 4.0, 0.0, 200.0)];
        let barrier = lay(&ways);
        assert_eq!(barrier.strips.len(), 1);
        assert!((barrier.strips[0].1 - 2.0 * STRIP_OVERLAP).abs() < 0.01);
    }

    #[test]
    fn a_fork_into_two_carriageways_is_paved_as_wide_as_the_road_it_leaves() {
        // A 10 m road from the west ends at the origin, where two 4.5 m carriageways start on its
        // centre line and draw apart to 4 m either side of it over 80 m.
        let ways = [
            Way { points: vec![[-100.0, 0.0], [0.0, 0.0]], half: 5.0, carriageway: false, fast: false },
            Way { points: vec![[0.0, 0.0], [80.0, 4.0], [200.0, 4.0]], half: 2.25, carriageway: true, fast: false },
            Way { points: vec![[200.0, -4.0], [80.0, -4.0], [0.0, 0.0]], half: 2.25, carriageway: true, fast: false },
        ];
        let barrier = lay(&ways);
        let apron = barrier.strips.iter().find(|(_, width)| *width == 10.0).expect("the fork is paved 10 m wide");
        assert_eq!(apron.0[0], [0.0, 0.0]);
        // It ends where the carriageways' outer edges are 10 m apart: their lines 5.5 m, at x = 55.
        let reach = apron.0.iter().map(|p| p[0]).fold(0.0, f64::max);
        assert!((40.0..=56.0).contains(&reach), "the apron reaches {reach} m");
        assert!(apron.0.iter().all(|p| p[1].abs() < 0.5), "it lies between the two: {:?}", apron.0);
    }

    #[test]
    fn a_wide_median_is_not_paved() {
        let ways = [carriageway(10.0, true, 4.0, 0.0, 200.0), carriageway(-10.0, false, 4.0, 0.0, 200.0)];
        let barrier = lay(&ways);
        assert_eq!(barrier.rails.len(), 1);
        assert!(barrier.strips.is_empty());
        assert_eq!(barrier.beside, vec![Beside::Open, Beside::Open]);
    }
}
