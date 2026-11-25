//! Finding the way by road (EMERGENCY.md): the road nearest a place, and the quickest way from one
//! place on the network to another.
//!
//! Traffic wanders, and needs neither. A police car and an ambulance are sent somewhere, so they
//! look the place up on the network (`Graph::locate`) and plan how to get there (`Graph::route`).

use super::graph::{Graph, Way};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// The side of a square of the grid the roads are looked up in, metres.
const CELL: f32 = 64.0;
/// A road whose surface is further than this above or below a place is on another level, metres:
/// the deck of a bridge over someone who stands under it.
const LEVEL_APART: f32 = 3.0;

/// A place on the network: a live way, how far along it, and how far from its centre line the
/// place that was looked up is, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub way: Way,
    pub along: f32,
    pub off: f32,
}

/// A way to drive from one place on the network to another.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// The ways to take after the one it starts on, in order. Empty when it ends on that one.
    pub ways: Vec<Way>,
    /// How far along the last of them it ends, metres.
    pub stop: f32,
    /// How long it takes at the roads' speeds, seconds.
    pub cost: f32,
}

/// The square of the grid `(x, z)` is in.
fn cell(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

impl Graph {
    /// Notes which edges pass through each square of the grid, for `locate`. Done once, when the
    /// network is made: an edge that is shut later is still found, and passed over.
    pub(super) fn index(&mut self) {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (index, edge) in self.edges.iter().enumerate() {
            // The points are no more than a few metres apart, so no square is stepped over.
            for point in &edge.points {
                let edges = cells.entry(cell(point[0], point[2])).or_default();
                if edges.last() != Some(&(index as u32)) {
                    edges.push(index as u32);
                }
            }
        }
        self.cells = cells;
    }

    /// The place on the network nearest `p`, no further than `within` metres from it: on a road at
    /// the level of `p` rather than one over or under it, where there is one.
    pub fn locate(&self, p: [f32; 3], within: f32) -> Option<Spot> {
        let (cx, cz) = cell(p[0], p[2]);
        let rings = (within / CELL).ceil() as i32;
        let mut best: Option<(bool, Spot)> = None;
        let mut seen: Vec<u32> = Vec::new();
        for (dx, dz) in (-rings..=rings).flat_map(|dx| (-rings..=rings).map(move |dz| (dx, dz))) {
            for &edge in self.cells.get(&(cx + dx, cz + dz)).into_iter().flatten() {
                if seen.contains(&edge) {
                    continue;
                }
                seen.push(edge);
                let Some(way) = [false, true].into_iter().map(|back| Way { edge, back }).find(|way| self.live[way.index()]) else { continue };
                let (along, off) = self.nearest(way, [p[0], p[2]], 0.0, self.length(way));
                if off > within {
                    continue;
                }
                let (at, _) = self.edge(way).at(if way.back { self.length(way) - along } else { along });
                let apart = (at[1] - p[1]).abs() > LEVEL_APART;
                if best.is_none_or(|(level, spot)| (apart, off) < (level, spot.off)) {
                    best = Some((apart, Spot { way, along, off }));
                }
            }
        }
        best.map(|(_, spot)| spot)
    }

    /// The quickest way by road from `along` metres down `from` to `to`, arriving on either side of
    /// its road, if there is one that takes no longer than `longest` seconds. It never turns straight
    /// back (`exits`), so a place just behind is reached round the block.
    pub fn route(&self, from: Way, along: f32, to: Spot, longest: f32) -> Option<Route> {
        let time = |way: Way, metres: f32| metres.max(0.0) / self.edge(way).speed.max(1.0);
        // Where on `way` the place is, if it is on it: on its own way, or the same road the other way.
        let stop_on = |way: Way| {
            if way == to.way {
                Some(to.along)
            } else if way == to.way.reversed() && self.live[way.index()] {
                Some(self.length(way) - to.along)
            } else {
                None
            }
        };
        if let Some(stop) = stop_on(from).filter(|&stop| stop >= along) {
            return Some(Route { ways: Vec::new(), stop, cost: time(from, stop - along) });
        }
        // The least time to the end of each way, and the way each was come to by.
        let mut best = vec![f32::MAX; self.live.len()];
        let mut came: Vec<Option<Way>> = vec![None; self.live.len()];
        let mut open: BinaryHeap<(Reverse<u32>, u32, bool)> = BinaryHeap::new();
        let key = |cost: f32| Reverse((cost * 1000.0) as u32);
        best[from.index()] = time(from, self.length(from) - along);
        open.push((key(best[from.index()]), from.edge, from.back));
        // The quickest arrival found so far: its time, the way it ends on and the one before that.
        let mut found: Option<(f32, Way, f32, Way)> = None;
        while let Some((_, edge, back)) = open.pop() {
            let way = Way { edge, back };
            let cost = best[way.index()];
            if cost > longest || found.is_some_and(|(total, ..)| cost >= total) {
                break;
            }
            for out in self.exits(way) {
                if let Some(stop) = stop_on(out) {
                    let total = cost + time(out, stop);
                    if found.is_none_or(|(known, ..)| total < known) {
                        found = Some((total, out, stop, way));
                    }
                }
                let through = cost + time(out, self.length(out));
                if through < best[out.index()] {
                    best[out.index()] = through;
                    came[out.index()] = Some(way);
                    open.push((key(through), out.edge, out.back));
                }
            }
        }
        let (cost, last, stop, mut way) = found.filter(|&(total, ..)| total <= longest)?;
        let mut ways = vec![last];
        while way != from {
            ways.push(way);
            way = came[way.index()]?;
        }
        ways.reverse();
        Some(Route { ways, stop, cost })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{ROAD_MAJOR, ROAD_MINOR, Road};

    /// A two-lane minor road along `points` (metres).
    fn road(points: &[[f32; 2]]) -> Road {
        Road { width: 700, kind: ROAD_MINOR, points: points.iter().map(|p| [(p[0] * 100.0) as i32, (p[1] * 100.0) as i32]).collect(), lanes: 2, forward: 1, ..Default::default() }
    }

    fn flat(roads: &[Road]) -> Graph {
        // A bridge's deck rises from the ground at x = -50 and 50 to 8 m over it at 0.
        Graph::build(roads, |p| p[0].abs() < 500.0 && p[1].abs() < 500.0, |x, _, carried| if carried { (8.0 * (1.0 - x.abs() / 50.0)).max(0.0) } else { 0.0 })
    }

    /// A block of streets 200 m square with a road in from the world's edge at two of its corners:
    /// the south side of the block is a major road, and the rest are minor.
    fn block() -> Graph {
        let major = |points: &[[f32; 2]]| Road { kind: ROAD_MAJOR, ..road(points) };
        flat(&[
            road(&[[-500.0, -100.0], [-100.0, -100.0]]),
            road(&[[-100.0, -100.0], [100.0, -100.0]]),
            road(&[[100.0, -100.0], [100.0, 100.0]]),
            major(&[[100.0, 100.0], [-100.0, 100.0]]),
            road(&[[-100.0, 100.0], [-100.0, -100.0]]),
            road(&[[100.0, 100.0], [500.0, 100.0]]),
        ])
    }

    /// Where a route ends, driven from `from`: the point of the lane it stops at.
    fn end(graph: &Graph, from: Way, route: &Route) -> [f32; 3] {
        graph.pose(None, route.ways.last().copied().unwrap_or(from), None, route.stop).0
    }

    #[test]
    fn a_route_is_the_quickest_way_by_road_and_never_turns_straight_back() {
        let graph = block();
        // In from the west, to a place half way down the east side of the block.
        let from = Way { edge: 0, back: false };
        let to = graph.locate([100.0, 0.0, 20.0], 30.0).expect("a road by it");
        assert!(to.off < 0.1 && graph.edge(to.way).points[0][0] == 100.0, "{to:?}");
        let route = graph.route(from, 100.0, to, 300.0).expect("a way there");
        // Along the north side and down the east: 300 m of the road in, 200 m and 120 m, at 10 m/s.
        assert_eq!(route.ways.len(), 2, "{route:?}");
        assert!((route.cost - 62.0).abs() < 0.5, "{} s", route.cost);
        let at = end(&graph, from, &route);
        assert!((at[0] - 100.0).abs() < 2.0 && (at[2] - 20.0).abs() < 0.5, "{at:?}");
        // Each way of it leaves where the last arrived, and none is the last one driven back.
        let mut last = from;
        for &way in &route.ways {
            assert_eq!(graph.start(way), graph.end(last));
            assert_ne!(way.edge, last.edge);
            last = way;
        }
        // A place just behind is reached round the block, not by turning in the road.
        let behind = Spot { way: Way { edge: 1, back: false }, along: 20.0, off: 0.0 };
        let round = graph.route(Way { edge: 1, back: false }, 60.0, behind, 300.0).expect("a way round");
        assert_eq!(round.ways.len(), 4, "{round:?}");
        assert_eq!(round.ways.last(), Some(&Way { edge: 1, back: false }));
        // And one a little ahead on the same road is driven straight to.
        let ahead = graph.route(Way { edge: 1, back: false }, 10.0, behind, 300.0).expect("it is ahead");
        assert!(ahead.ways.is_empty() && ahead.stop == 20.0 && (ahead.cost - 1.0).abs() < 0.01, "{ahead:?}");
    }

    #[test]
    fn a_route_takes_the_faster_road_where_it_is_the_longer_way_round() {
        let graph = block();
        // From the north-west corner to just short of the south-east one, on the south side: the
        // north and east sides are 400 m at 10 m/s, the west and south 400 m with 200 m at 16 m/s.
        let from = Way { edge: 0, back: false };
        let to = graph.locate([80.0, 0.0, 100.0], 30.0).expect("the south side");
        let route = graph.route(from, 399.0, to, 300.0).expect("a way there");
        assert_eq!(route.ways.first().map(|way| way.edge), Some(4), "down the west side: {route:?}");
        assert!(graph.route(from, 399.0, to, 20.0).is_none(), "and no way at all in 20 s");
    }

    #[test]
    fn the_nearest_way_to_a_point_under_a_bridge_is_the_road_below() {
        let carried = Road { carried: true, lanes: 0, forward: 0, ..road(&[[-50.0, 0.0], [0.0, 0.0], [50.0, 0.0]]) };
        let graph = flat(&[road(&[[-500.0, 0.0], [-50.0, 0.0]]), carried, road(&[[50.0, 0.0], [500.0, 0.0]]), road(&[[0.0, -500.0], [0.0, 0.0], [0.0, 500.0]])]);
        let under = graph.locate([1.0, 0.9, 0.5], 40.0).expect("a road");
        assert!(!graph.edge(under.way).carried, "someone on the ground is on the road under the bridge");
        let over = graph.locate([1.0, 8.9, 0.5], 40.0).expect("a road");
        assert!(graph.edge(over.way).carried, "and someone on the deck is on the bridge");
        assert!(graph.locate([300.0, 0.0, 300.0], 40.0).is_none(), "a field far from any road is on none");
    }

    #[test]
    fn a_road_that_is_shut_is_neither_found_nor_driven() {
        let carried = Road { carried: true, lanes: 0, forward: 0, ..road(&[[-50.0, 0.0], [50.0, 0.0]]) };
        // A through road with a bridge in its middle, and a long way round it to the north.
        let roads = [road(&[[-500.0, 0.0], [-200.0, 0.0], [-50.0, 0.0]]), carried, road(&[[50.0, 0.0], [200.0, 0.0], [500.0, 0.0]]), road(&[[-200.0, 0.0], [-200.0, -300.0], [200.0, -300.0], [200.0, 0.0]])];
        let mut graph = flat(&roads);
        let from = graph.locate([-400.0, 0.0, 0.0], 10.0).expect("the road in").way;
        let from = if graph.end(from) == graph.edges[from.edge as usize].ends[1] { from } else { from.reversed() };
        let to = graph.locate([400.0, 0.0, 0.0], 10.0).expect("the road out");
        let over = graph.route(from, 0.0, to, 600.0).expect("a way over the bridge");
        assert!(over.ways.iter().any(|&way| graph.edge(way).carried));
        let deck = graph.edges.iter().position(|edge| edge.carried).unwrap() as u32;
        graph.shut(deck);
        let round = graph.route(from, 0.0, to, 600.0).expect("a way round");
        assert!(round.ways.iter().all(|&way| !graph.edge(way).carried) && round.cost > over.cost, "{round:?}");
        assert!(graph.locate([0.0, 8.0, 0.0], 5.0).is_none(), "nothing is sent onto the broken bridge");
    }
}
