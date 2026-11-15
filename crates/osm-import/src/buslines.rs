//! Public transport: the bus lines of a world that asks for them (`"buses": true` in its
//! `area.json`). The major roads of the finished world are joined into one network, and a few long
//! lines are found on it, each on road no other line uses. Stops are the bus stops OpenStreetMap has
//! beside a line (`highway=bus_stop`), with more put in wherever those leave a long gap. BUSES.md.

use crate::assemble::Built;
use crate::osm::{Data, LatLon};
use shared::world::{BusLine, ROAD_MAJOR};
use std::collections::{BinaryHeap, HashMap};

/// The most lines a world gets.
pub const BUS_LINES: usize = 3;
/// A line shorter than this is not worth a bus, metres.
const LINE_MIN: f64 = 1000.0;
/// A line keeps this far inside the world's edge, metres: a bus is turned round at each end of it.
const EDGE_CLEAR: f64 = 30.0;
/// A mapped stop this near a line is a stop on it, metres.
const STOP_REACH: f64 = 25.0;
/// Stops nearer each other than this along a line are one stop, metres.
const STOP_MERGE: f64 = 150.0;
/// A stretch longer than `STOP_GAP` with no stop gets stops about `STOP_SPACING` apart, metres.
const STOP_GAP: f64 = 600.0;
const STOP_SPACING: f64 = 400.0;
/// Road this near a line is that line's road too, metres: the other carriageway of a divided road,
/// which a second line would only run beside the first.
const TWIN_REACH: f64 = 40.0;
/// A turn sharper than this, radians, within `HOOK_REACH` metres of a line's end is a hook, which is
/// cut off.
const HOOK_TURN: f64 = 1.0;
const HOOK_REACH: f64 = 60.0;
/// A line that comes within `TWIN_REACH` of where it was more than this far back along itself has
/// turned back along its own road, metres.
const TWIN_BACK: f64 = 300.0;

/// The bus stops of `data`.
pub fn stops(data: &Data) -> Vec<LatLon> {
    data.nodes.iter().filter(|node| crate::osm::is_bus_stop(&node.tags)).map(|node| node.at).collect()
}

/// A stop keeps this far from a bridge, metres, and so does the end of a line.
const BRIDGE_CLEAR: f64 = 10.0;
/// A stop's sign stands this far outside the edge of its road, metres.
const SIGN_VERGE: f64 = 0.6;
/// A stop that would be on a bridge is moved along the line in steps of this, metres.
const STOP_SHIFT: f64 = 10.0;

/// Puts the bus lines of the finished world in its manifest, with `stops` (metres) as the stops
/// OpenStreetMap maps. No stop is on or under a bridge (a road a deck carries), nor where its sign
/// would have nowhere to stand (`sign`), and each has the one sign.
pub fn lay(built: &mut Built, stops: &[[f64; 2]]) {
    let Built { manifest, tiles } = built;
    let roads: Vec<Vec<[i32; 2]>> = tiles.values().flat_map(|tile| &tile.roads).filter(|road| road.kind == ROAD_MAJOR).map(|road| road.points.clone()).collect();
    let half = manifest.half_extent;
    let clear = (EDGE_CLEAR * 100.0) as i32;
    let on_tile = |[x, z]: [i32; 2]| x.abs() < half && z.abs() < half && tiles.contains_key(&manifest.tile_at(x, z));
    let inside = |[x, z]: [i32; 2]| [[0, 0], [clear, 0], [-clear, 0], [0, clear], [0, -clear]].iter().all(|d| on_tile([x + d[0], z + d[1]]));
    let all: Vec<Way> = tiles.values().flat_map(|tile| &tile.roads).map(|road| Way { line: road.points.iter().map(|p| [f64::from(p[0]) / 100.0, f64::from(p[1]) / 100.0]).collect(), half: f64::from(road.width) / 200.0, major: road.kind == ROAD_MAJOR, carried: road.carried }).collect();
    let no_stop = |at: [f64; 2], way: [f64; 2]| off(&all, at, |way| way.carried) < BRIDGE_CLEAR || sign(&all, at, way).is_none();
    let mut lines = find(&roads, stops, inside, no_stop);
    for line in &mut lines {
        // A stop and its sign go together: one that rounding to the centimetre left with no sign is dropped.
        let placed = line.stops.iter().filter_map(|&stop| {
            let (at, way) = line.at(stop as f32 / 100.0);
            sign(&all, at.map(f64::from), way.map(f64::from)).map(|sign| (stop, sign.map(|v| (v * 100.0).round() as i32)))
        });
        (line.stops, line.signs) = placed.unzip();
    }
    manifest.bus_lines = lines;
}

/// A road of the finished world, metres.
struct Way {
    /// Its centre line.
    line: Vec<[f64; 2]>,
    /// Half its width.
    half: f64,
    /// Whether it is a major road, which a line can follow.
    major: bool,
    /// Whether a bridge's deck carries it.
    carried: bool,
}

/// How far `p` is from the edge of the nearest of the `ways` that are `wanted`, metres: negative on one.
fn off(ways: &[Way], p: [f64; 2], wanted: impl Fn(&Way) -> bool) -> f64 {
    ways.iter().filter(|way| wanted(way)).flat_map(|way| way.line.windows(2).map(move |pair| crate::assemble::distance_to_segment(p, pair[0], pair[1]) - way.half)).fold(f64::MAX, f64::min)
}

/// A sign stands at least this far from the edge of every road and path, metres.
const SIGN_CLEAR: f64 = 0.5;
/// A road at least this wide is one cars drive on, metres: a sign does not stand with one of those
/// just beyond it, as the strip between two carriageways has.
const CARRIAGEWAY: f64 = 4.0;

/// Where the one sign of a stop stands, or `None` where there is nowhere for it: `SIGN_VERGE` or a
/// few metres more outside the edge of the line's road at `at` (where the line runs along the unit
/// direction `way`), clear of every road and path, on a side with no carriageway within 8 m beyond
/// the edge. That is the outside of a divided road, and past a cycle path that runs along the road.
/// It is the right of the way out when both sides will do.
fn sign(ways: &[Way], at: [f64; 2], way: [f64; 2]) -> Option<[f64; 2]> {
    // Half the width of the road the line is on here: the nearest major road's.
    let own = ways.iter().filter(|road| road.major).flat_map(|road| road.line.windows(2).map(move |pair| (crate::assemble::distance_to_segment(at, pair[0], pair[1]), road.half))).min_by(|a, b| a.0.total_cmp(&b.0)).map_or(5.0, |(_, half)| half);
    // With the way ahead along -Z the right is +X: a quarter turn clockwise from the way.
    let beside = |side: f64, out: f64| [at[0] - way[1] * side * (own + out), at[1] + way[0] * side * (own + out)];
    let open = |side: f64| [2.0, 4.0, 6.0, 8.0].iter().all(|&out| off(ways, beside(side, out), |road| 2.0 * road.half >= CARRIAGEWAY) > 0.0);
    [1.0, -1.0].into_iter().filter(|&side| open(side)).find_map(|side| [0.0, 1.0, 2.0, 3.0].into_iter().map(|further| beside(side, SIGN_VERGE + further)).find(|&p| off(ways, p, |_| true) >= SIGN_CLEAR))
}

/// The network: a node wherever road points lie within a metre of each other, and a link between
/// nodes that follow each other along a road.
struct Network {
    /// Where each node is, centimetres.
    points: Vec<[i32; 2]>,
    /// From each node, the nodes it is linked to and how far off they are, metres.
    links: Vec<Vec<(usize, f64)>>,
}

impl Network {
    /// The network of `roads` (centimetres), leaving out every point that is not `inside`.
    fn of(roads: &[Vec<[i32; 2]>], inside: impl Fn([i32; 2]) -> bool) -> Self {
        let mut network = Network { points: Vec::new(), links: Vec::new() };
        let mut index: HashMap<[i32; 2], usize> = HashMap::new();
        for road in roads {
            let mut last: Option<usize> = None;
            for p in road.iter().map(|&p| inside(p).then_some(p)) {
                let Some(p) = p else {
                    last = None;
                    continue;
                };
                // Rounded to the metre, so the two halves of a road cut at a tile's edge meet.
                let key = [(p[0] + 50).div_euclid(100), (p[1] + 50).div_euclid(100)];
                let node = *index.entry(key).or_insert_with(|| {
                    network.points.push(p);
                    network.links.push(Vec::new());
                    network.points.len() - 1
                });
                if let Some(from) = last.filter(|&from| from != node)
                    && !network.links[from].iter().any(|&(to, _)| to == node)
                {
                    let far = network.far(from, node);
                    network.links[from].push((node, far));
                    network.links[node].push((from, far));
                }
                last = Some(node);
            }
        }
        network
    }

    /// The distance between two nodes, metres.
    fn far(&self, a: usize, b: usize) -> f64 {
        let (a, b) = (self.points[a], self.points[b]);
        f64::from(b[0] - a[0]).hypot(f64::from(b[1] - a[1])) / 100.0
    }

    /// The shortest way from `from` to every node it can reach over links that are not `used`: how
    /// far each is (`f64::INFINITY` when out of reach), metres, and the node before it on the way.
    fn reach(&self, from: usize, used: &[(usize, usize)]) -> (Vec<f64>, Vec<usize>) {
        let mut far = vec![f64::INFINITY; self.points.len()];
        let mut before = vec![usize::MAX; self.points.len()];
        let mut open = BinaryHeap::new();
        far[from] = 0.0;
        // Ordered by centimetres gone, nearest first.
        let cm = |metres: f64| (metres * 100.0) as u64;
        open.push((std::cmp::Reverse(0u64), from));
        while let Some((std::cmp::Reverse(gone), node)) = open.pop() {
            if gone > cm(far[node]) {
                continue;
            }
            for &(next, length) in &self.links[node] {
                if used.contains(&(node.min(next), node.max(next))) {
                    continue;
                }
                let total = far[node] + length;
                if total < far[next] {
                    far[next] = total;
                    before[next] = node;
                    open.push((std::cmp::Reverse(cm(total)), next));
                }
            }
        }
        (far, before)
    }

    /// The longest line there is over links that are not `used`: the nodes of the shortest way
    /// between the two nodes furthest apart along the roads, and its length, metres. Found from each
    /// separate piece of the network in turn: the node furthest from anywhere in a piece is one end,
    /// and the node furthest from that is the other.
    fn longest(&self, used: &[(usize, usize)]) -> Option<(Vec<usize>, f64)> {
        let mut seen = vec![false; self.points.len()];
        let mut best: Option<(Vec<usize>, f64)> = None;
        for start in 0..self.points.len() {
            if seen[start] {
                continue;
            }
            let (far, _) = self.reach(start, used);
            let furthest = |far: &[f64]| (0..far.len()).filter(|&n| far[n].is_finite()).max_by(|&a, &b| far[a].total_cmp(&far[b]));
            for node in (0..far.len()).filter(|&n| far[n].is_finite()) {
                seen[node] = true;
            }
            let Some(end) = furthest(&far) else { continue };
            let (far, before) = self.reach(end, used);
            let Some(other) = furthest(&far) else { continue };
            if best.as_ref().is_some_and(|(_, length)| far[other] <= *length) {
                continue;
            }
            let mut nodes = vec![other];
            while let Some(&node) = nodes.last().filter(|&&node| node != end) {
                nodes.push(before[node]);
            }
            best = Some((nodes, far[other]));
        }
        best
    }
}

/// The bus lines over `roads` (the major roads, centimetres): the longest line there is, then the
/// longest over road no line has yet, up to `BUS_LINES` of them and none shorter than `LINE_MIN`.
/// Each has the `stops` (metres) that lie beside it, and its own where those are far apart. A line
/// neither ends nor stops where `no_stop` says a stop cannot be: a point in metres and the unit
/// direction the line runs there. The signs are left for `lay`, which knows the roads round each stop.
pub fn find(roads: &[Vec<[i32; 2]>], stops: &[[f64; 2]], inside: impl Fn([i32; 2]) -> bool, no_stop: impl Fn([f64; 2], [f64; 2]) -> bool) -> Vec<BusLine> {
    let network = Network::of(roads, inside);
    let mut used: Vec<(usize, usize)> = Vec::new();
    let mut lines = Vec::new();
    while lines.len() < BUS_LINES {
        let Some((nodes, length)) = network.longest(&used) else { break };
        if length < LINE_MIN {
            break;
        }
        let whole: Vec<[i32; 2]> = nodes.iter().map(|&node| network.points[node]).collect();
        // Up one carriageway of a divided road and back down the other is the same road twice: the
        // line is the longest stretch of it that does not come back beside itself.
        let mut points = unhooked(straightened(&whole));
        // Each end is a stop, where the bus is turned round: the line ends where a stop can be.
        let metres = |p: [i32; 2]| [f64::from(p[0]) / 100.0, f64::from(p[1]) / 100.0];
        let unit = |a: [i32; 2], b: [i32; 2]| {
            let (a, b) = (metres(a), metres(b));
            let far = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
            [(b[0] - a[0]) / far, (b[1] - a[1]) / far]
        };
        for _ in 0..2 {
            let from = (0..points.len().saturating_sub(1)).find(|&i| !no_stop(metres(points[i]), unit(points[i], points[i + 1]))).unwrap_or(points.len());
            points.drain(..from);
            points.reverse();
        }
        // The line's own road, and any that runs beside it, the rest of a doubled one too.
        let beside: Vec<bool> = network.points.iter().map(|&p| off_line(&whole, p) <= TWIN_REACH).collect();
        for (from, links) in network.links.iter().enumerate() {
            used.extend(links.iter().filter(|&&(to, _)| from < to && beside[from] && beside[to]).map(|&(to, _)| (from, to)));
        }
        let length: f64 = points.windows(2).map(|pair| f64::from(pair[1][0] - pair[0][0]).hypot(f64::from(pair[1][1] - pair[0][1])) / 100.0).sum();
        if length < LINE_MIN {
            continue;
        }
        let stops = stops_along(&points, stops, &no_stop);
        lines.push(BusLine { points, stops, signs: Vec::new() });
    }
    lines
}

/// The line through `points` without a hook at either end: where it turns by more than `HOOK_TURN`
/// within `HOOK_REACH` metres of an end (the way out of a roundabout or a junction the line happens
/// to start in), it starts from that turn instead. A bus could not get round it.
fn unhooked(mut points: Vec<[i32; 2]>) -> Vec<[i32; 2]> {
    for _ in 0..2 {
        // The last sharp turn near the start; the second time round, near the other end.
        let mut gone = 0.0;
        let mut from = 0;
        for i in 1..points.len().saturating_sub(1) {
            gone += f64::from(points[i][0] - points[i - 1][0]).hypot(f64::from(points[i][1] - points[i - 1][1])) / 100.0;
            if gone > HOOK_REACH {
                break;
            }
            let (a, b) = ([points[i][0] - points[i - 1][0], points[i][1] - points[i - 1][1]].map(f64::from), [points[i + 1][0] - points[i][0], points[i + 1][1] - points[i][1]].map(f64::from));
            if (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).abs() > HOOK_TURN {
                from = i;
            }
        }
        points.drain(..from);
        points.reverse();
    }
    points
}

/// The longest stretch of the line through `points` that does not double back on itself: the line
/// cut wherever it turns round (`doubling_back`), and the longest piece kept.
fn straightened(points: &[[i32; 2]]) -> Vec<[i32; 2]> {
    let length = |piece: &[[i32; 2]]| piece.windows(2).map(|pair| f64::from(pair[1][0] - pair[0][0]).hypot(f64::from(pair[1][1] - pair[0][1]))).sum::<f64>();
    let (mut rest, mut best) = (points, &points[..0]);
    while rest.len() >= 2 {
        let cut = doubling_back(rest);
        if length(&rest[..cut]) > length(best) {
            best = &rest[..cut];
        }
        rest = &rest[cut..];
    }
    best.to_vec()
}

/// How many of `points` (centimetres) a line keeps: all of them, or, when one of them lies within
/// `TWIN_REACH` of the line more than `TWIN_BACK` metres before it, as far as where the line turned
/// round to get back there.
fn doubling_back(points: &[[i32; 2]]) -> usize {
    let far = |a: [i32; 2], b: [i32; 2]| f64::from(b[0] - a[0]).hypot(f64::from(b[1] - a[1])) / 100.0;
    let mut gone = vec![0.0; points.len()];
    for i in 1..points.len() {
        gone[i] = gone[i - 1] + far(points[i - 1], points[i]);
    }
    for i in 1..points.len() {
        let earlier = gone.partition_point(|&at| at < gone[i] - TWIN_BACK);
        if earlier >= 2 && off_line(&points[..earlier], points[i]) <= TWIN_REACH {
            // It turned round half way between here and where it was beside here before.
            let beside = (0..earlier).min_by(|&a, &b| far(points[a], points[i]).total_cmp(&far(points[b], points[i]))).unwrap_or(0);
            return gone.partition_point(|&at| at <= (gone[beside] + gone[i]) / 2.0).max(2);
        }
    }
    points.len()
}

/// How far `p` is from the line through `points`, both centimetres, in metres.
fn off_line(points: &[[i32; 2]], p: [i32; 2]) -> f64 {
    let metres = |q: [i32; 2]| [f64::from(q[0]) / 100.0, f64::from(q[1]) / 100.0];
    points.windows(2).map(|pair| crate::assemble::distance_to_segment(metres(p), metres(pair[0]), metres(pair[1]))).fold(f64::MAX, f64::min)
}

/// Where the stops of a line through `points` (centimetres) are, as centimetres along it: both its
/// ends, the mapped `stops` (metres) within `STOP_REACH` of it, and stops put in along any stretch
/// longer than `STOP_GAP` that has none. A mapped stop where `no_stop` says none can be is left out, and
/// one put in there is moved along the line to the nearest place that will do, or left out if its
/// stretch has none.
fn stops_along(points: &[[i32; 2]], stops: &[[f64; 2]], no_stop: &impl Fn([f64; 2], [f64; 2]) -> bool) -> Vec<u32> {
    let line = BusLine { points: points.to_vec(), stops: Vec::new(), signs: Vec::new() };
    let barred = |at: f64| {
        let (p, way) = line.at(at as f32);
        no_stop(p.map(f64::from), way.map(f64::from))
    };
    let metres = |p: [i32; 2]| [f64::from(p[0]) / 100.0, f64::from(p[1]) / 100.0];
    let length: f64 = points.windows(2).map(|pair| (metres(pair[1])[0] - metres(pair[0])[0]).hypot(metres(pair[1])[1] - metres(pair[0])[1])).sum();
    // How far along the line the point of it nearest a stop is, when the stop is beside it.
    let along = |stop: &[f64; 2]| {
        let (mut best, mut gone) = ((0.0, f64::MAX), 0.0);
        for pair in points.windows(2) {
            let (a, b) = (metres(pair[0]), metres(pair[1]));
            let d = [b[0] - a[0], b[1] - a[1]];
            let span = d[0].hypot(d[1]);
            let t = if span < 1e-6 { 0.0 } else { (((stop[0] - a[0]) * d[0] + (stop[1] - a[1]) * d[1]) / (span * span)).clamp(0.0, 1.0) };
            let off = (stop[0] - a[0] - d[0] * t).hypot(stop[1] - a[1] - d[1] * t);
            if off < best.1 {
                best = (gone + span * t, off);
            }
            gone += span;
        }
        (best.1 <= STOP_REACH).then_some(best.0)
    };
    let mut mapped: Vec<f64> = stops.iter().filter_map(along).filter(|&at| !barred(at)).collect();
    mapped.sort_by(f64::total_cmp);
    // The ends first, so a mapped stop near one gives way to it.
    let mut kept = vec![0.0, length];
    for at in mapped {
        if kept.iter().all(|other| (other - at).abs() >= STOP_MERGE) {
            kept.push(at);
        }
    }
    kept.sort_by(f64::total_cmp);
    let mut all = Vec::new();
    for pair in kept.windows(2) {
        all.push(pair[0]);
        let gap = pair[1] - pair[0];
        if gap > STOP_GAP {
            let pieces = (gap / STOP_SPACING).round().max(2.0);
            for n in 1..pieces as usize {
                let wanted = pair[0] + gap * n as f64 / pieces;
                // The nearest place to it where a stop can be, no nearer the stops either side than stops are merged.
                let last = all.last().copied().unwrap_or(pair[0]);
                let free = |at: &f64| *at >= last + STOP_MERGE && *at <= pair[1] - STOP_MERGE && !barred(*at);
                let shifts = (0..=(gap / STOP_SHIFT) as usize).flat_map(|k| [wanted + k as f64 * STOP_SHIFT, wanted - k as f64 * STOP_SHIFT]);
                all.extend(shifts.into_iter().find(free));
            }
        }
    }
    all.push(length);
    all.into_iter().map(|at| (at * 100.0).round() as u32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight road from `a` to `b` (metres) with a point every 50 m, in centimetres.
    fn road(a: [f64; 2], b: [f64; 2]) -> Vec<[i32; 2]> {
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / 50.0).ceil().max(1.0) as usize;
        (0..=steps).map(|n| [0, 1].map(|i| ((a[i] + (b[i] - a[i]) * n as f64 / steps as f64) * 100.0).round() as i32)).collect()
    }

    fn anywhere(_: [i32; 2]) -> bool {
        true
    }

    /// A stop can be anywhere.
    fn dry(_: [f64; 2], _: [f64; 2]) -> bool {
        false
    }

    #[test]
    fn a_stop_has_one_sign_on_the_verge_away_from_the_other_carriageway() {
        let way = |z: f64, half: f64| Way { line: vec![[0.0, z], [1000.0, z]], half, major: half > 2.0, carried: false };
        let east = |ways: &[Way]| sign(ways, [500.0, 0.0], [1.0, 0.0]);
        // A road ten metres wide running east: the sign is on its right, 0.6 m outside its edge.
        assert_eq!(east(&[way(0.0, 5.0)]), Some([500.0, 5.6]));
        // With the other carriageway 12 m to the right, it is on the left, not between the two.
        assert_eq!(east(&[way(0.0, 5.0), way(12.0, 5.0)]), Some([500.0, -5.6]));
        // A cycle path along the right of the road: the sign stands past it.
        assert_eq!(east(&[way(0.0, 5.0), way(6.1, 1.2)]), Some([500.0, 8.6]));
        // Roads close on both sides leave nowhere for a sign, so no stop is put there.
        assert_eq!(east(&[way(0.0, 5.0), way(12.0, 5.0), way(-12.0, 5.0)]), None);
        // A narrower road has it nearer its middle.
        assert_eq!(sign(&[way(0.0, 3.5)], [500.0, 0.0], [-1.0, 0.0]), Some([500.0, -4.1]));
    }

    #[test]
    fn a_line_neither_stops_nor_ends_on_a_bridge() {
        // A road 4 km long, on a bridge from 1700 m to 2100 m and again over its last 150 m.
        let roads = [road([0.0, 0.0], [4000.0, 0.0])];
        let bridged = |p: [f64; 2], _: [f64; 2]| (1700.0..=2100.0).contains(&p[0]) || p[0] >= 3850.0;
        let lines = find(&roads, &[[1900.0, 5.0], [1000.0, 5.0]], anywhere, bridged);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        let ends = [line.points[0][0], line.points.last().unwrap()[0]];
        assert!(ends.contains(&0) && ends.contains(&380_000), "it ends before the bridge: {ends:?}");
        let at: Vec<f64> = line.stops.iter().map(|&stop| f64::from(line.at(stop as f32 / 100.0).0[0])).collect();
        assert!(at.iter().all(|&x| !bridged([x, 0.0], [1.0, 0.0])), "no stop on a bridge: {at:?}");
        assert!(at.iter().any(|&x| (x - 1000.0).abs() < 1.0), "the mapped stop off the bridge is kept: {at:?}");
        // The long stretch across the bridge still gets its stops, either side of it.
        let mut sorted = at.clone();
        sorted.sort_by(f64::total_cmp);
        assert!(sorted.windows(2).all(|pair| pair[1] - pair[0] < 1200.0), "{sorted:?}");
    }

    #[test]
    fn the_first_line_is_the_longest_way_along_the_roads() {
        // A main road east to west with a shorter road north from its middle.
        let roads = [road([-1500.0, 0.0], [0.0, 0.0]), road([0.0, 0.0], [1500.0, 0.0]), road([0.0, 0.0], [0.0, -1200.0])];
        let lines = find(&roads, &[], anywhere, dry);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].length(), 3000.0);
        let ends = [lines[0].points[0], *lines[0].points.last().unwrap()];
        assert!(ends.contains(&[-150_000, 0]) && ends.contains(&[150_000, 0]), "{ends:?}");
        // The second runs on the road the first left: the branch, and none of the main road.
        assert_eq!(lines[1].length(), 1200.0);
        assert!(lines[1].points.iter().all(|p| p[0] == 0), "{:?}", lines[1].points);
    }

    #[test]
    fn roads_cut_at_a_tile_edge_are_one_road_and_short_ones_get_no_line() {
        // Two halves that meet a few centimetres apart, and a stub far from both.
        let roads = [road([-800.0, 0.0], [0.0, 0.0]), vec![[3, -2], [80_000, 0]], road([5000.0, 5000.0], [5400.0, 5000.0])];
        let lines = find(&roads, &[], anywhere, dry);
        assert_eq!(lines.len(), 1, "the stub is under a kilometre");
        assert!((lines[0].length() - 1600.0).abs() < 0.1);
        assert!(find(&[road([0.0, 0.0], [900.0, 0.0])], &[], anywhere, dry).is_empty());
        assert!(find(&[], &[], anywhere, dry).is_empty());
    }

    #[test]
    fn there_are_never_more_than_three_lines() {
        let roads: Vec<_> = (0..5).map(|n| road([0.0, n as f64 * 300.0], [2000.0 + n as f64 * 100.0, n as f64 * 300.0])).collect();
        let lines = find(&roads, &[], anywhere, dry);
        assert_eq!(lines.len(), BUS_LINES);
        assert_eq!(lines.iter().map(BusLine::length).collect::<Vec<_>>(), [2400.0, 2300.0, 2200.0], "the longest first");
    }

    #[test]
    fn a_line_stops_where_the_map_has_stops_and_fills_the_long_gaps() {
        let roads = [road([0.0, 0.0], [2000.0, 0.0])];
        // One beside the line, one too far from it, one too near the first to be its own stop, and
        // one almost at the end of the line.
        let lines = find(&roads, &[[500.0, 12.0], [900.0, 60.0], [560.0, -8.0], [1950.0, 5.0]], anywhere, dry);
        assert_eq!(lines.len(), 1);
        // From whichever end the line starts, measured from the west end.
        let west = lines[0].points[0] == [0, 0];
        let mut stops: Vec<u32> = lines[0].stops.iter().map(|&at| if west { at } else { 200_000 - at }).collect();
        stops.sort_unstable();
        assert_eq!(stops, [0, 50_000, 87_500, 125_000, 162_500, 200_000], "the ends, the mapped stop, and three in the 1500 m after it");
        assert_eq!(lines[0].stops.first(), Some(&0));
        assert_eq!(lines[0].stops.last(), Some(&200_000));
    }

    #[test]
    fn the_other_carriageway_of_a_divided_road_gets_no_line_of_its_own() {
        // Two carriageways 20 m apart, joined at both ends, and a separate road well away.
        let roads = [road([0.0, 0.0], [2000.0, 0.0]), road([0.0, 20.0], [2000.0, 20.0]), road([0.0, 0.0], [0.0, 20.0]), road([2000.0, 0.0], [2000.0, 20.0]), road([0.0, 500.0], [1500.0, 500.0])];
        let lines = find(&roads, &[], anywhere, dry);
        assert_eq!(lines.len(), 2, "{:?}", lines.iter().map(BusLine::length).collect::<Vec<_>>());
        assert!(lines[1].points.iter().all(|p| p[1] == 50_000), "the second is the road away from the first");
    }

    #[test]
    fn a_line_does_not_come_back_down_the_other_carriageway() {
        // Two carriageways 20 m apart joined only at the east end: the longest way along the roads is
        // out along one and back along the other.
        let roads = [road([0.0, 0.0], [2000.0, 0.0]), road([2000.0, 0.0], [2000.0, 20.0]), road([2000.0, 20.0], [0.0, 20.0])];
        let lines = find(&roads, &[], anywhere, dry);
        assert_eq!(lines.len(), 1, "and the other carriageway gets no line of its own");
        assert!(lines[0].length() >= 1700.0 && lines[0].length() <= 2020.0, "one carriageway's length: {}", lines[0].length());
        let side = lines[0].points[0][1];
        assert!(lines[0].points.iter().filter(|p| p[1] != side).count() <= 1, "all on one carriageway: {:?}", lines[0].points);
    }

    #[test]
    fn a_hook_at_the_end_of_a_line_is_cut_off() {
        // The road starts with ten metres the wrong way, as out of a roundabout.
        let roads = [vec![[-500, 800], [0, 0]], road([0.0, 0.0], [1500.0, 0.0])];
        let lines = find(&roads, &[], anywhere, dry);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].length(), 1500.0);
        assert!(lines[0].points.iter().all(|p| p[1] == 0), "{:?}", &lines[0].points[..3]);
    }

    #[test]
    fn a_line_keeps_inside_the_world() {
        // The road runs on out of the world, which ends at x = 1200 m.
        let lines = find(&[road([0.0, 0.0], [3000.0, 0.0])], &[], |p| p[0] <= 120_000, dry);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].length(), 1200.0);
    }
}
