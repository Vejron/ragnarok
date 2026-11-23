//! The road network traffic drives on (TRAFFIC.md), made once from the world's roads.
//!
//! The world has its roads as centre lines cut at the tiles' edges, with nothing to say which meet.
//! Here they are joined into a graph: a `Node` wherever a road ends or two roads share a point
//! (within a metre, so the halves of a road cut at a tile's edge meet), and an `Edge` for each
//! stretch of road between two nodes, with the height of the road's surface along it. A `Way` is an
//! edge driven in one direction, and only the ways that lead somewhere are `live`: traffic never
//! turns into a road it could not get out of again.

use shared::world::{MARK_CENTRE_SOLID, ROAD_MAJOR, ROAD_MINOR, Road};
use std::collections::HashMap;

/// An edge's centre line has a point at least this often, metres, so its heights follow the ground.
const SPACING: f32 = 6.0;
/// A road narrower than this is not driven, metres.
const NARROWEST: f32 = 3.5;
/// How fast traffic drives a major road, one with two lanes or more its own way, a minor road and a
/// road of a single lane, m/s. The world's roads carry no speed limits.
const SPEED_MAJOR: f32 = 16.0;
const SPEED_FAST: f32 = 22.0;
const SPEED_MINOR: f32 = 10.0;
const SPEED_SINGLE: f32 = 7.0;
/// The sideways pull a bend is taken at, and the slowing traffic plans on, m/s².
pub const CORNERING: f32 = 2.5;
pub const BRAKING: f32 = 2.0;
/// The slowest any bend or turn is taken, m/s.
pub const BEND_SPEED: f32 = 3.0;
/// A road that ends within this of the world's edge leads out of the world, metres.
const EDGE_REACH: f32 = 60.0;
/// A turn from one road into another is rounded off over this much of each, metres, or half the
/// road where it is shorter.
pub const TURN_REACH: f32 = 7.0;
/// Two roads that end at the same point of the map meet only if their surfaces are within this of
/// each other there, metres: a road through an underpass does not meet the road on the bridge over it.
const LEVEL: f32 = 1.5;
/// A car keeps at least this far inside the edge of its road, metres: half its width and a little.
const KERB: f32 = 1.3;

/// An edge driven one way: along its points, or `back` against them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Way {
    pub edge: u32,
    pub back: bool,
}

impl Way {
    /// Its place in a list with one entry a way.
    pub fn index(self) -> usize {
        self.edge as usize * 2 + usize::from(self.back)
    }

    /// The same edge the other way.
    pub fn reversed(self) -> Way {
        Way { edge: self.edge, back: !self.back }
    }
}

/// Where roads meet, or one ends.
#[derive(Clone, Debug, Default)]
pub struct Node {
    /// `[x, z]`, metres.
    pub at: [f32; 2],
    /// The live ways that leave it.
    pub out: Vec<Way>,
    /// The live ways that arrive at it.
    pub into: Vec<Way>,
    /// How many edges touch it, live or not.
    pub edges: u32,
    /// A road leaves the world here: traffic that arrives is gone, and fresh traffic starts from it.
    pub portal: bool,
}

/// A stretch of road between two nodes.
#[derive(Clone, Debug)]
pub struct Edge {
    /// The node at its first point and at its last.
    pub ends: [u32; 2],
    /// Its centre line with the height of the road's surface, metres.
    pub points: Vec<[f32; 3]>,
    /// How far along the line each point is, metres.
    pub gone: Vec<f32>,
    /// `ROAD_MAJOR` or `ROAD_MINOR`.
    pub kind: u8,
    /// Metres.
    pub width: f32,
    /// How many lanes run along its points, and how many against them.
    pub lanes: [u8; 2],
    /// How fast it is driven, m/s.
    pub speed: f32,
    /// A bridge's deck carries it.
    pub carried: bool,
    /// Its deck is broken: nothing drives it (`Graph::shut`).
    pub shut: bool,
    /// A solid line down its middle: no car crosses to the other side of it to pass another.
    pub no_passing: bool,
    /// The fastest a car may be at each point to make the bends ahead of it, driven along the points
    /// and against them, m/s.
    caps: [Vec<f32>; 2],
}

impl Edge {
    /// Metres.
    pub fn length(&self) -> f32 {
        self.gone.last().copied().unwrap_or(0.0)
    }

    /// The point `along` metres from its first (held to its ends) and the unit direction the line
    /// runs there.
    pub fn at(&self, along: f32) -> ([f32; 3], [f32; 3]) {
        let along = along.clamp(0.0, self.length());
        let i = self.gone.partition_point(|&gone| gone <= along).clamp(1, self.points.len() - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let span = (self.gone[i] - self.gone[i - 1]).max(1e-3);
        let t = (along - self.gone[i - 1]) / span;
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let size = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
        ([a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t], d.map(|v| v / size))
    }
}

/// The road network.
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Whether each way (`Way::index`) is driven: it has lanes its way, and leads somewhere.
    pub live: Vec<bool>,
    /// The live ways that start at a portal: where fresh traffic comes into the world.
    pub entries: Vec<Way>,
    /// The live ways, each with the length of all those up to and including it, metres: for picking
    /// a place on the network with every metre as likely as the next.
    pub lengths: Vec<(Way, f32)>,
    /// At how many points of the map roads end on more than one level, and so do not meet.
    pub split: usize,
}

/// The point of a quadratic curve from `a` by `c` to `b` at `t`, and the unit direction it runs there.
fn curve(a: [f32; 3], c: [f32; 3], b: [f32; 3], t: f32) -> ([f32; 3], [f32; 3]) {
    let u = 1.0 - t;
    let p = std::array::from_fn(|k| u * u * a[k] + 2.0 * u * t * c[k] + t * t * b[k]);
    let d: [f32; 3] = std::array::from_fn(|k| 2.0 * u * (c[k] - a[k]) + 2.0 * t * (b[k] - c[k]));
    let size = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
    (p, d.map(|v| v / size))
}

impl Graph {
    pub fn edge(&self, way: Way) -> &Edge {
        &self.edges[way.edge as usize]
    }

    /// Metres.
    pub fn length(&self, way: Way) -> f32 {
        self.edge(way).length()
    }

    /// The node `way` starts at.
    pub fn start(&self, way: Way) -> u32 {
        self.edge(way).ends[usize::from(way.back)]
    }

    /// The node `way` arrives at.
    pub fn end(&self, way: Way) -> u32 {
        self.edge(way).ends[usize::from(!way.back)]
    }

    /// How far right of the road's centre line a car on `way` drives, metres: the middle of the
    /// outermost lane its way, or of the road where it is all one way and a single lane.
    pub fn offset(&self, way: Way) -> f32 {
        let edge = self.edge(way);
        let (ours, theirs) = (edge.lanes[usize::from(way.back)], edge.lanes[usize::from(!way.back)]);
        if theirs == 0 && ours <= 1 {
            return 0.0;
        }
        let lane = edge.width / f32::from((ours + theirs).max(1));
        (edge.width / 2.0 - lane / 2.0).min(edge.width / 2.0 - KERB).max(0.0)
    }

    /// Where a car `along` metres down `way` is on its lane, with no turn rounded off, and the unit
    /// direction it drives there.
    fn lane(&self, way: Way, along: f32) -> ([f32; 3], [f32; 3]) {
        let edge = self.edge(way);
        let (p, d) = if way.back { edge.at(edge.length() - along) } else { edge.at(along) };
        let d = if way.back { d.map(|v| -v) } else { d };
        // With the nose along -Z the right is +X: a quarter turn clockwise from the way it goes.
        let flat = d[0].hypot(d[2]).max(1e-6);
        let (right, off) = ([-d[2] / flat, d[0] / flat], self.offset(way));
        ([p[0] + right[0] * off, p[1], p[2] + right[1] * off], d)
    }

    /// How much of each of two ways a turn from one into the other is rounded off over, metres.
    fn reach(&self, from: Way, to: Way) -> f32 {
        TURN_REACH.min(self.length(from) / 2.0).min(self.length(to) / 2.0)
    }

    /// Where a car `along` metres down `way` is, and the unit direction it drives: on its lane, and
    /// near either end on the curve that takes it from the way it came by (`before`) or into the way
    /// it goes on by (`after`).
    pub fn pose(&self, before: Option<Way>, way: Way, after: Option<Way>, along: f32) -> ([f32; 3], [f32; 3]) {
        let length = self.length(way);
        let turn = |from: Way, to: Way, t: f32| {
            let reach = self.reach(from, to);
            let (a, _) = self.lane(from, self.length(from) - reach);
            let (b, _) = self.lane(to, reach);
            let (end, start) = (self.lane(from, self.length(from)).0, self.lane(to, 0.0).0);
            let corner = std::array::from_fn(|k| (end[k] + start[k]) / 2.0);
            curve(a, corner, b, t)
        };
        if let Some(after) = after {
            let reach = self.reach(way, after);
            if reach > 0.1 && along > length - reach {
                return turn(way, after, (along - (length - reach)) / (2.0 * reach));
            }
        }
        if let Some(before) = before {
            let reach = self.reach(before, way);
            if reach > 0.1 && along < reach {
                return turn(before, way, 0.5 + along / (2.0 * reach));
            }
        }
        self.lane(way, along)
    }

    /// The fastest a car `along` metres down `way` may be to make the bends ahead of it on that way, m/s.
    pub fn cap(&self, way: Way, along: f32) -> f32 {
        let edge = self.edge(way);
        let at = if way.back { edge.length() - along } else { along }.clamp(0.0, edge.length());
        let i = edge.gone.partition_point(|&gone| gone <= at).clamp(1, edge.points.len() - 1);
        let caps = &edge.caps[usize::from(way.back)];
        let t = (at - edge.gone[i - 1]) / (edge.gone[i] - edge.gone[i - 1]).max(1e-3);
        caps[i - 1] + (caps[i] - caps[i - 1]) * t
    }

    /// The fastest the turn from the end of `from` into `to` is taken, m/s.
    pub fn turn_speed(&self, from: Way, to: Way) -> f32 {
        let (_, a) = self.lane(from, self.length(from));
        let (_, b) = self.lane(to, 0.0);
        let turn = (a[0] * b[2] - a[2] * b[0]).atan2(a[0] * b[0] + a[2] * b[2]).abs();
        if turn < 0.05 {
            return f32::MAX;
        }
        // The curve it drives is about as long as what it is rounded off over, both sides.
        (CORNERING * 2.0 * self.reach(from, to).max(1.0) / turn).sqrt().max(BEND_SPEED)
    }

    /// How far along `way` the point of its centre line nearest `p` (`[x, z]`) is, looking only
    /// between `from` and `to` metres along it, and how far from the line `p` is there, metres.
    pub fn nearest(&self, way: Way, p: [f32; 2], from: f32, to: f32) -> (f32, f32) {
        let edge = self.edge(way);
        let length = edge.length();
        let (low, high) = if way.back { (length - to, length - from) } else { (from, to) };
        let mut best = (low.clamp(0.0, length), f32::MAX);
        for i in 1..edge.points.len() {
            if edge.gone[i] < low || edge.gone[i - 1] > high {
                continue;
            }
            let (a, b) = (edge.points[i - 1], edge.points[i]);
            let d = [b[0] - a[0], b[2] - a[2]];
            let flat = d[0] * d[0] + d[1] * d[1];
            let t = if flat < 1e-6 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[2]) * d[1]) / flat).clamp(0.0, 1.0) };
            let off = (p[0] - a[0] - d[0] * t).hypot(p[1] - a[2] - d[1] * t);
            if off < best.1 {
                best = (edge.gone[i - 1] + (edge.gone[i] - edge.gone[i - 1]) * t, off);
            }
        }
        (if way.back { length - best.0 } else { best.0 }, best.1)
    }

    /// The network of `roads`. `inside` says whether a point (`[x, z]`, metres) is in the world, and
    /// `surface` gives the height of the road at one: of the deck where a bridge carries it (its last
    /// argument), else of the ground.
    pub fn build(roads: &[Road], inside: impl Fn([f32; 2]) -> bool, surface: impl Fn(f32, f32, bool) -> f32) -> Graph {
        let driven: Vec<&Road> = roads.iter().filter(|road| matches!(road.kind, ROAD_MAJOR | ROAD_MINOR) && road.points.len() >= 2 && road.width as f32 / 100.0 >= NARROWEST).collect();
        // Rounded to the metre, so the two halves of a road cut at a tile's edge meet. A point in the
        // middle of a stretch a bridge carries is on a level of its own: the road under the bridge
        // does not meet it.
        let key = |road: &Road, i: usize| {
            let p = road.points[i];
            let inner = road.carried && i > 0 && i + 1 < road.points.len();
            ((p[0] + 50).div_euclid(100), (p[1] + 50).div_euclid(100), inner)
        };
        // A node wherever a road ends or two roads share a point.
        let mut uses: HashMap<(i32, i32, bool), (u32, bool)> = HashMap::new();
        for road in &driven {
            let mut last = None;
            for i in 0..road.points.len() {
                let k = key(road, i);
                if last == Some(k) {
                    continue;
                }
                last = Some(k);
                let entry = uses.entry(k).or_default();
                entry.0 += 1;
                entry.1 |= i == 0 || i + 1 == road.points.len();
            }
        }
        let mut graph = Graph::default();
        // The nodes at each point of the map, one a level.
        let mut nodes_at: HashMap<(i32, i32, bool), Vec<(u32, f32)>> = HashMap::new();
        for road in &driven {
            let metres = |i: usize| [road.points[i][0] as f32 / 100.0, road.points[i][1] as f32 / 100.0];
            let mut from = 0;
            for i in 1..road.points.len() {
                let (count, ended) = uses[&key(road, i)];
                if !(ended || count >= 2) {
                    continue;
                }
                let line: Vec<[f32; 2]> = (from..=i).map(metres).collect();
                let (a, b) = (key(road, from), key(road, i));
                from = i;
                let length: f32 = line.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum();
                // A ring with the one node on it is an edge from that node to itself.
                if length < 0.5 {
                    continue;
                }
                // A bridge's own road has no lanes written: it is taken as one each way, and the
                // ways that lead nowhere are found below. A single lane that is not one-way is
                // driven both ways, each car keeping to its side of it.
                let lanes = match (road.lanes, road.one_way) {
                    (0, _) | (1, false) => [1, 1],
                    _ => [road.forward, road.lanes - road.forward.min(road.lanes)],
                };
                let width = road.width as f32 / 100.0;
                let speed = match (road.kind, lanes[0].max(lanes[1])) {
                    (ROAD_MAJOR, 2..) => SPEED_FAST,
                    (ROAD_MAJOR, _) => SPEED_MAJOR,
                    _ if road.lanes == 1 => SPEED_SINGLE,
                    _ => SPEED_MINOR,
                };
                let mut points = Vec::new();
                for pair in line.windows(2) {
                    let steps = ((pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]) / SPACING).ceil().max(1.0) as usize;
                    for step in 0..steps {
                        let t = step as f32 / steps as f32;
                        points.push([pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t]);
                    }
                }
                points.push(line[line.len() - 1]);
                let points: Vec<[f32; 3]> = points.into_iter().map(|[x, z]| [x, surface(x, z, road.carried), z]).collect();
                let mut node = |k: (i32, i32, bool), p: [f32; 3]| {
                    let known = nodes_at.entry(k).or_default();
                    if let Some(&(node, _)) = known.iter().find(|&&(_, y)| (y - p[1]).abs() < LEVEL) {
                        return node;
                    }
                    graph.nodes.push(Node { at: [p[0], p[2]], ..Default::default() });
                    known.push((graph.nodes.len() as u32 - 1, p[1]));
                    graph.nodes.len() as u32 - 1
                };
                let ends = [node(a, points[0]), node(b, points[points.len() - 1])];
                let mut gone = vec![0.0; points.len()];
                for i in 1..points.len() {
                    gone[i] = gone[i - 1] + (points[i][0] - points[i - 1][0]).hypot(points[i][2] - points[i - 1][2]);
                }
                let caps = caps(&points, &gone, speed);
                graph.edges.push(Edge { ends, points, gone, kind: road.kind, width, lanes, speed, carried: road.carried, shut: false, no_passing: road.marks & MARK_CENTRE_SOLID != 0, caps });
            }
        }
        for edge in &graph.edges {
            for end in edge.ends {
                graph.nodes[end as usize].edges += 1;
            }
        }
        // A road that ends by the world's edge leads out of it.
        for node in &mut graph.nodes {
            let [x, z] = node.at;
            node.portal = node.edges == 1 && [[EDGE_REACH, 0.0], [-EDGE_REACH, 0.0], [0.0, EDGE_REACH], [0.0, -EDGE_REACH]].iter().any(|d| !inside([x + d[0], z + d[1]]));
        }
        graph.split = nodes_at.values().filter(|levels| levels.len() > 1).count();
        graph.settle();
        graph
    }

    /// Edge `edge` can no longer be driven: its bridge is down. The ways that only led to it, or
    /// only away from it, go with it.
    pub fn shut(&mut self, edge: u32) {
        self.edges[edge as usize].shut = true;
        self.settle();
    }

    /// Finds the live ways: those with a lane their way, on an edge that is not shut, that traffic
    /// can both come into and get out of again, by another road or a portal. A cul-de-sac, a one-way
    /// street nothing feeds and the wrong way over a bridge of a divided road all go, and whatever
    /// only led to them after them.
    fn settle(&mut self) {
        let ways: Vec<Way> = (0..self.edges.len() as u32).flat_map(|edge| [Way { edge, back: false }, Way { edge, back: true }]).collect();
        let mut live: Vec<bool> = ways.iter().map(|&way| !self.edge(way).shut && self.edge(way).lanes[usize::from(way.back)] > 0).collect();
        for node in &mut self.nodes {
            node.out.clear();
            node.into.clear();
        }
        self.entries.clear();
        self.lengths.clear();
        let mut touching: Vec<Vec<u32>> = vec![Vec::new(); self.nodes.len()];
        for (index, edge) in self.edges.iter().enumerate() {
            touching[edge.ends[0] as usize].push(index as u32);
            touching[edge.ends[1] as usize].push(index as u32);
        }
        loop {
            let mut changed = false;
            for &way in &ways {
                if !live[way.index()] {
                    continue;
                }
                let (start, end) = (self.start(way), self.end(way));
                // Another edge's way out of where it arrives, and another's into where it starts.
                let onward = touching[end as usize].iter().filter(|&&edge| edge != way.edge).any(|&edge| [false, true].iter().any(|&back| live[Way { edge, back }.index()] && self.start(Way { edge, back }) == end));
                let fed = touching[start as usize].iter().filter(|&&edge| edge != way.edge).any(|&edge| [false, true].iter().any(|&back| live[Way { edge, back }.index()] && self.end(Way { edge, back }) == start));
                if !(onward || self.nodes[end as usize].portal) || !(fed || self.nodes[start as usize].portal) {
                    live[way.index()] = false;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut total = 0.0;
        for &way in ways.iter().filter(|way| live[way.index()]) {
            let (start, end) = (self.start(way), self.end(way));
            self.nodes[start as usize].out.push(way);
            self.nodes[end as usize].into.push(way);
            if self.nodes[start as usize].portal {
                self.entries.push(way);
            }
            total += self.length(way);
            self.lengths.push((way, total));
        }
        self.live = live;
    }

    /// The ways a car that came down `way` may go on by: every live way out of where it arrives but
    /// those that lead straight back to where it came from, by its own road or one beside it.
    pub fn exits(&self, way: Way) -> impl Iterator<Item = Way> + '_ {
        let from = self.start(way);
        self.nodes[self.end(way) as usize].out.iter().copied().filter(move |&out| out.edge != way.edge && self.end(out) != from)
    }

    /// The live way that has the metre `at` of all their lengths together, and how far along it that is.
    pub fn place(&self, at: f32) -> Option<(Way, f32)> {
        let i = self.lengths.partition_point(|&(_, total)| total <= at).min(self.lengths.len().checked_sub(1)?);
        let (way, total) = self.lengths[i];
        Some((way, (at - (total - self.length(way))).clamp(0.0, self.length(way))))
    }

    /// The length of all the live ways together, metres.
    pub fn driven(&self) -> f32 {
        self.lengths.last().map_or(0.0, |&(_, total)| total)
    }
}

/// The fastest a car may be at each of `points` to make the bends ahead of it, driven along them and
/// against them, m/s: `speed` on the straight, less through a bend, and less before one by what it
/// takes to slow down for it.
fn caps(points: &[[f32; 3]], gone: &[f32], speed: f32) -> [Vec<f32>; 2] {
    let n = points.len();
    let mut bend = vec![speed; n];
    for i in 1..n.saturating_sub(1) {
        let (a, b) = ([points[i][0] - points[i - 1][0], points[i][2] - points[i - 1][2]], [points[i + 1][0] - points[i][0], points[i + 1][2] - points[i][2]]);
        let turn = (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).abs();
        if turn > 0.01 {
            let span = (gone[i + 1] - gone[i - 1]) / 2.0;
            bend[i] = (CORNERING * span / turn).sqrt().clamp(BEND_SPEED, speed);
        }
    }
    let slowed = |from: f32, over: f32| (from * from + 2.0 * BRAKING * over).sqrt();
    let mut along = bend.clone();
    for i in (0..n.saturating_sub(1)).rev() {
        along[i] = along[i].min(slowed(along[i + 1], gone[i + 1] - gone[i]));
    }
    let mut against = bend;
    for i in 1..n {
        against[i] = against[i].min(slowed(against[i - 1], gone[i] - gone[i - 1]));
    }
    [along, against]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-lane minor road along `points` (metres).
    fn road(points: &[[f32; 2]]) -> Road {
        Road { width: 700, kind: ROAD_MINOR, points: points.iter().map(|p| [(p[0] * 100.0) as i32, (p[1] * 100.0) as i32]).collect(), lanes: 2, forward: 1, ..Default::default() }
    }

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 0.01)
    }

    fn flat(roads: &[Road]) -> Graph {
        // A bridge's deck rises from the ground at x = -50 and 50 to 8 m over it at 0.
        Graph::build(roads, |p| p[0].abs() < 500.0 && p[1].abs() < 500.0, |x, _, carried| if carried { (8.0 * (1.0 - x.abs() / 50.0)).max(0.0) } else { 0.0 })
    }

    #[test]
    fn a_side_road_meets_a_through_road_at_a_node_and_a_cul_de_sac_is_not_driven() {
        // A road right across the world, cut in two as at a tile's edge, with a side road off its
        // middle that ends in a field.
        let graph = flat(&[road(&[[-500.0, 0.0], [0.0, 0.0]]), road(&[[0.4, 0.3], [200.0, 0.0], [500.0, 0.0]]), road(&[[200.0, 0.0], [200.0, 150.0]])]);
        assert_eq!(graph.edges.len(), 4, "the second piece is cut where the side road leaves it");
        let junction = graph.nodes.iter().find(|node| node.edges == 3).expect("a junction");
        assert_eq!(junction.at, [200.0, 0.0]);
        assert_eq!(graph.nodes.iter().filter(|node| node.portal).count(), 2, "both ends of the through road leave the world");
        // The through road is driven both ways, and the side road neither.
        assert_eq!(graph.live.iter().filter(|&&live| live).count(), 6);
        assert_eq!(junction.out.len(), 2);
        assert_eq!(graph.entries.len(), 2);
        assert!((graph.driven() - 2000.0).abs() < 2.0, "{}", graph.driven());
    }

    #[test]
    fn a_one_way_ring_is_driven_one_way_round_and_slowly_at_its_corners() {
        let one_way = |points: &[[f32; 2]]| Road { lanes: 1, forward: 1, width: 550, one_way: true, ..road(points) };
        // A square ring, one way, with a two-way road in from the world's edge.
        let graph = flat(&[one_way(&[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0], [0.0, 0.0]]), road(&[[-500.0, 0.0], [0.0, 0.0]])]);
        let ring = Way { edge: 0, back: false };
        assert!(graph.live[ring.index()] && !graph.live[ring.reversed().index()]);
        assert_eq!(graph.offset(ring), 0.0, "a single lane is driven down its middle");
        assert!(graph.cap(ring, 50.0) > graph.cap(ring, 98.0), "it slows for the corner");
        assert!(graph.cap(ring, 100.0) <= BEND_SPEED + 0.5);
        // The road in keeps right of its middle, either way.
        let lane = Way { edge: 1, back: false };
        assert!(near(graph.pose(None, lane, None, 100.0).0, [-400.0, 0.0, 1.75]));
        assert!(near(graph.pose(None, lane.reversed(), None, 100.0).0, [-100.0, 0.0, -1.75]));
    }

    #[test]
    fn a_bridge_is_on_its_deck_and_does_not_meet_the_road_under_it() {
        let carried = Road { carried: true, lanes: 0, forward: 0, ..road(&[[-50.0, 0.0], [0.0, 0.0], [50.0, 0.0]]) };
        let graph = flat(&[road(&[[-500.0, 0.0], [-50.0, 0.0]]), carried, road(&[[50.0, 0.0], [500.0, 0.0]]), road(&[[0.0, -500.0], [0.0, 0.0], [0.0, 500.0]])]);
        assert_eq!(graph.edges.len(), 4, "the road under the bridge is not cut at it");
        let deck = graph.edges.iter().position(|edge| edge.points.iter().any(|p| p[1] > 0.0)).expect("the bridge") as u32;
        let (p, _) = graph.pose(None, Way { edge: deck, back: false }, None, 50.0);
        assert_eq!(p[1], 8.0);
        assert!(graph.live.iter().all(|&live| live), "every road runs from one edge of the world to another");
    }

    #[test]
    fn a_bridge_that_is_shut_takes_the_roads_that_only_led_to_it() {
        // A through road with a bridge in its middle, and a road off each half of it to the world's edge.
        let carried = Road { carried: true, lanes: 0, forward: 0, ..road(&[[-50.0, 0.0], [50.0, 0.0]]) };
        let roads = [road(&[[-500.0, 0.0], [-200.0, 0.0], [-50.0, 0.0]]), carried, road(&[[50.0, 0.0], [200.0, 0.0], [500.0, 0.0]]), road(&[[-200.0, 0.0], [-200.0, 500.0]]), road(&[[200.0, 0.0], [200.0, 500.0]])];
        let mut graph = flat(&roads);
        assert!(graph.live.iter().all(|&live| live));
        let deck = graph.edges.iter().position(|edge| edge.carried).expect("the bridge") as u32;
        let before = graph.driven();
        graph.shut(deck);
        // The bridge and the 150 m either side of it, both ways.
        assert!((before - graph.driven() - 800.0).abs() < 2.0, "{} m fewer are driven", before - graph.driven());
        assert_eq!(graph.live.iter().filter(|&&live| !live).count(), 6);
        assert_eq!(graph.entries.len(), 4, "the world's edges are still ways in");
    }

    #[test]
    fn a_road_that_ends_under_a_bridge_does_not_meet_the_road_on_it() {
        // A bridge in two pieces that meet over the end of a road through the underpass below.
        let piece = |points: &[[f32; 2]]| Road { carried: true, lanes: 0, forward: 0, ..road(points) };
        let roads = [road(&[[-500.0, 0.0], [-50.0, 0.0]]), piece(&[[-50.0, 0.0], [0.0, 0.0]]), piece(&[[0.0, 0.0], [50.0, 0.0]]), road(&[[50.0, 0.0], [500.0, 0.0]]), road(&[[0.0, -500.0], [0.0, 0.0]])];
        let graph = flat(&roads);
        let under = graph.edges.iter().position(|edge| edge.points[0][2] < -100.0).expect("the road below") as u32;
        let end = graph.edges[under as usize].ends[1];
        assert_eq!(graph.nodes[end as usize].edges, 1, "nothing meets it under the bridge");
        assert!(!graph.live[Way { edge: under, back: false }.index()], "and so it leads nowhere");
        assert_eq!(graph.live.iter().filter(|&&live| live).count(), 8, "the road over the bridge is driven both ways");
    }

    #[test]
    fn a_turn_is_one_smooth_curve_from_lane_to_lane() {
        // East, then south: a right turn at (0, 0).
        let graph = flat(&[road(&[[-500.0, 0.0], [0.0, 0.0]]), road(&[[0.0, 0.0], [0.0, 500.0]])]);
        let (first, second) = (Way { edge: 0, back: false }, Way { edge: 1, back: false });
        let before = graph.pose(None, first, Some(second), 500.0 - 0.01);
        let after = graph.pose(Some(first), second, None, 0.01);
        for k in 0..3 {
            assert!((before.0[k] - after.0[k]).abs() < 0.05 && (before.1[k] - after.1[k]).abs() < 0.05, "{before:?} {after:?}");
        }
        // Half way round it heads south-east, inside the corner of the two lanes.
        assert!(before.1[0] > 0.5 && before.1[2] > 0.5, "{before:?}");
        assert!(graph.turn_speed(first, second) < 6.0);
        let (along, off) = graph.nearest(second, [3.0, 40.0], 0.0, 100.0);
        assert!((along - 40.0).abs() < 0.01 && (off - 3.0).abs() < 0.01);
    }
}
