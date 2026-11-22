//! Rocks and bushes the map does not record, scattered over woods and parks.
//!
//! OpenStreetMap and the laser data have no boulders or shrubs worth drawing, so they are placed
//! deterministically from the cover polygons: the same input always gives the same world, and tile
//! files (which are hashed) do not churn. They are returned as trees of the rock and bush
//! archetypes, so they go through the same rejection (buildings, roads, bridges) and tiling as
//! every tree. Rocks are solid; a bush never is (`assemble` keeps it passable).

use crate::assemble::point_in_polygon;
use shared::world::{COVER_ASPHALT, COVER_PARK, COVER_ROCK, COVER_SAND, COVER_WATER, COVER_WOOD, TREE_BIRCH, TREE_BUSH, TREE_ROCK_BOULDER, TREE_ROCK_SLAB};
use std::collections::HashMap;

type P = [f64; 2];
/// `(position, TREE_*, height, crown radius, solid)`, as `assemble` takes its trees.
pub type Placed = (P, u8, f64, f64, bool);

/// One attempt per this many square metres of a cover's bounding box, per kind of prop.
const ROCK_AREA: f64 = 9_000.0;
const BUSH_AREA: f64 = 2_500.0;
/// Most attempts of one kind in one cover, so a huge wood is not a quarry.
const MAX_PER_COVER: usize = 400;

/// A wood's edge is tried for a shrub or a young tree every this many metres along it...
const EDGE_STEP: f64 = 3.0;
/// ...and this share of the tries is taken.
const EDGE_SHARE: f64 = 0.7;
/// How far into the wood and out of it the edge's growth stands, metres: mostly outside, where
/// the light is.
const EDGE_DEPTH: (f64, f64) = (-2.0, 4.0);
/// This share of the edge's growth is young birch, the rest shrubs.
const YOUNG_SHARE: f64 = 0.25;
/// A young tree stands no nearer than this to a tree of the wood, metres, and a shrub no nearer
/// than `BUSH_ROOM`: where the laser found trees at the edge, those are its growth.
const YOUNG_ROOM: f64 = 4.0;
const BUSH_ROOM: f64 = 1.5;
/// Where the edge bends less than this (`edges::Hard::strength`), it runs along a road, a garden or a
/// field, kept clear, and nothing grows there.
const EDGE_WILD: f64 = 0.5;

/// SplitMix64: a well-mixed 64-bit value from a counter.
pub(crate) fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A value in `[0, 1)` from a hash.
pub(crate) fn unit(hash: u64) -> f64 {
    (hash >> 11) as f64 / (1u64 << 53) as f64
}

/// Inside the rings by the even-odd rule, so an inner ring is a hole.
pub(crate) fn inside(rings: &[Vec<P>], at: P) -> bool {
    rings.iter().filter(|ring| ring.len() > 2 && point_in_polygon(at, ring)).count() % 2 == 1
}

pub(crate) fn bounds(rings: &[Vec<P>]) -> Option<[f64; 4]> {
    let mut found = false;
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in rings.iter().flatten() {
        found = true;
        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
    }
    found.then_some(b)
}

/// Rocks and bushes inside the wood, park and sand covers, never in water or on a car park's asphalt
/// and no bush on bare rock, within `±inner` metres.
pub fn scatter(covers: &[(u8, Vec<Vec<P>>)], inner: f64) -> Vec<Placed> {
    let water: Vec<([f64; 4], &Vec<Vec<P>>)> =
        covers.iter().filter(|(kind, _)| matches!(*kind, COVER_WATER | COVER_ASPHALT)).filter_map(|(_, rings)| Some((bounds(rings)?, rings))).collect();
    let in_water = |at: P| water.iter().any(|(b, rings)| at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3] && inside(rings, at));
    let rock: Vec<([f64; 4], &Vec<Vec<P>>)> =
        covers.iter().filter(|(kind, _)| *kind == COVER_ROCK).filter_map(|(_, rings)| Some((bounds(rings)?, rings))).collect();
    let on_rock = |at: P| rock.iter().any(|(b, rings)| at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3] && inside(rings, at));
    let mut placed = Vec::new();
    for (index, (kind, rings)) in covers.iter().enumerate() {
        // Sand is bare: rocks, no shrubs. Parks are tended: shrubs, few rocks.
        let (rock_area, bush_area) = match *kind {
            COVER_WOOD => (ROCK_AREA, BUSH_AREA),
            COVER_PARK => (ROCK_AREA * 2.0, BUSH_AREA * 1.5),
            COVER_SAND => (ROCK_AREA * 1.5, f64::INFINITY),
            _ => continue,
        };
        let Some(b) = bounds(rings) else { continue };
        let area = (b[2] - b[0]) * (b[3] - b[1]);
        for (salt, per, is_rock) in [(1u64, rock_area, true), (2, bush_area, false)] {
            let attempts = ((area / per) as usize).min(MAX_PER_COVER);
            for attempt in 0..attempts as u64 {
                let seed = mix((index as u64) << 24 ^ salt << 56 ^ attempt);
                let at = [b[0] + unit(seed) * (b[2] - b[0]), b[1] + unit(mix(seed)) * (b[3] - b[1])];
                if at[0].abs() > inner || at[1].abs() > inner || !inside(rings, at) || in_water(at) || (!is_rock && on_rock(at)) {
                    continue;
                }
                let (u, v) = (unit(mix(seed ^ 0xA5A5)), unit(mix(seed ^ 0x5A5A)));
                placed.push(if is_rock {
                    if u < 0.65 {
                        (at, TREE_ROCK_BOULDER, 0.6 + v * 0.9, 0.5 + unit(mix(seed ^ 3)) * 0.9, true)
                    } else {
                        (at, TREE_ROCK_SLAB, 0.4 + v * 0.5, 1.0 + unit(mix(seed ^ 3)) * 1.2, true)
                    }
                } else {
                    (at, TREE_BUSH, 0.8 + v * 0.6, 0.6 + u * 0.6, false)
                });
            }
        }
    }
    placed
}

/// Shrubs and young birch along the edges of the woods, mostly just outside them: where the light
/// reaches, the ground at a wood's edge grows thick, so the wood does not end in a line. `strength`
/// is how freely each place's edge bends (`edges::Hard::strength`); none grows where it runs along
/// something man-made. `trees` are the trees already standing, which the growth keeps its room from.
pub fn edge_scatter(covers: &[(u8, Vec<Vec<P>>)], strength: impl Fn(P) -> f64, trees: &[P], inner: f64) -> Vec<Placed> {
    let mut standing: HashMap<(i32, i32), Vec<P>> = HashMap::new();
    let square = |p: P| ((p[0] / YOUNG_ROOM).floor() as i32, (p[1] / YOUNG_ROOM).floor() as i32);
    for &tree in trees {
        standing.entry(square(tree)).or_default().push(tree);
    }
    let room = |at: P, need: f64| {
        let (i, j) = square(at);
        !(i - 1..=i + 1).flat_map(|i| (j - 1..=j + 1).map(move |j| (i, j))).filter_map(|key| standing.get(&key)).flatten().any(|t| (t[0] - at[0]).hypot(t[1] - at[1]) < need)
    };
    let bare: Vec<([f64; 4], &Vec<Vec<P>>)> =
        covers.iter().filter(|(kind, _)| matches!(*kind, COVER_ROCK | COVER_WATER)).filter_map(|(_, rings)| Some((bounds(rings)?, rings))).collect();
    let on_bare = |at: P| bare.iter().any(|(b, rings)| at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3] && inside(rings, at));
    let mut placed = Vec::new();
    for (_, rings) in covers.iter().filter(|(kind, _)| *kind == COVER_WOOD) {
        for ring in rings.iter().filter(|ring| ring.len() >= 3) {
            let n = ring.len();
            // Which side of the ring is out of the wood, by a vote of points round it.
            let left = |k: usize| {
                let (a, b) = (ring[k], ring[(k + 1) % n]);
                let length = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
                [-(b[1] - a[1]) / length, (b[0] - a[0]) / length]
            };
            let votes: i32 = (0..n).step_by((n / 16).max(1)).map(|k| {
                let (mid, normal) = ([(ring[k][0] + ring[(k + 1) % n][0]) / 2.0, (ring[k][1] + ring[(k + 1) % n][1]) / 2.0], left(k));
                if inside(rings, [mid[0] + normal[0] * 0.3, mid[1] + normal[1] * 0.3]) { -1 } else { 1 }
            }).sum();
            let out = if votes >= 0 { 1.0 } else { -1.0 };
            // Along each side in steps of `EDGE_STEP`, carrying what is left of a step over to the next side.
            let mut carry = 0.0;
            for k in 0..n {
                let (a, b) = (ring[k], ring[(k + 1) % n]);
                let length = (b[0] - a[0]).hypot(b[1] - a[1]);
                let normal = left(k);
                let mut along = carry;
                while along < length {
                    let t = along / length;
                    let edge = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                    along += EDGE_STEP;
                    // Hashed from the place, so the same edge grows the same however it is walked.
                    let seed = mix((edge[0] * 10.0).round() as i64 as u64 ^ ((edge[1] * 10.0).round() as i64 as u64).rotate_left(32));
                    if unit(seed) >= EDGE_SHARE {
                        continue;
                    }
                    let depth = EDGE_DEPTH.0 + unit(mix(seed ^ 1)) * (EDGE_DEPTH.1 - EDGE_DEPTH.0);
                    let at = [edge[0] + normal[0] * out * depth, edge[1] + normal[1] * out * depth];
                    if at[0].abs() > inner || at[1].abs() > inner || strength(edge) < EDGE_WILD || strength(at) < EDGE_WILD || on_bare(at) {
                        continue;
                    }
                    let (u, v) = (unit(mix(seed ^ 2)), unit(mix(seed ^ 3)));
                    if unit(mix(seed ^ 4)) < YOUNG_SHARE && room(at, YOUNG_ROOM) {
                        placed.push((at, TREE_BIRCH, 2.5 + u * 2.5, 0.8 + v * 0.8, false));
                    } else if room(at, BUSH_ROOM) {
                        placed.push((at, TREE_BUSH, 0.7 + u * 0.8, 0.6 + v * 0.7, false));
                    }
                }
                carry = along - length;
            }
        }
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(min: f64, max: f64) -> Vec<P> {
        vec![[min, min], [max, min], [max, max], [min, max]]
    }

    #[test]
    fn the_same_covers_give_the_same_props() {
        let covers = vec![(COVER_WOOD, vec![square(-300.0, 300.0)])];
        assert_eq!(scatter(&covers, 1000.0), scatter(&covers, 1000.0));
    }

    #[test]
    fn props_stay_in_their_cover_and_out_of_water_and_holes() {
        let covers = vec![
            (COVER_WOOD, vec![square(-300.0, 300.0), square(-100.0, 100.0)]),
            (COVER_WATER, vec![square(100.0, 250.0)]),
        ];
        let props = scatter(&covers, 1000.0);
        assert!(props.len() > 20, "{}", props.len());
        for (at, ..) in &props {
            assert!(at[0].abs() <= 300.0 && at[1].abs() <= 300.0);
            assert!(at[0].abs() > 100.0 || at[1].abs() > 100.0, "in the hole: {at:?}");
            assert!(!(at[0] >= 100.0 && at[0] <= 250.0 && at[1] >= 100.0 && at[1] <= 250.0), "in the water: {at:?}");
        }
    }

    #[test]
    fn rocks_are_solid_and_bushes_are_not_and_urban_ground_gets_neither() {
        let props = scatter(&[(COVER_WOOD, vec![square(-300.0, 300.0)])], 1000.0);
        assert!(props.iter().any(|p| p.1 == TREE_ROCK_BOULDER) && props.iter().any(|p| p.1 == TREE_BUSH));
        assert!(props.iter().all(|p| p.4 == (p.1 != TREE_BUSH)));
        assert!(scatter(&[(shared::world::COVER_URBAN, vec![square(-300.0, 300.0)])], 1000.0).is_empty());
    }

    #[test]
    fn a_car_park_in_a_park_is_kept_clear_of_rocks_and_bushes() {
        let props = scatter(&[(COVER_PARK, vec![square(-300.0, 300.0)]), (COVER_ASPHALT, vec![square(-200.0, 200.0)])], 1000.0);
        assert!(!props.is_empty());
        assert!(props.iter().all(|(at, ..)| at[0].abs() > 200.0 || at[1].abs() > 200.0), "on the asphalt");
    }

    #[test]
    fn nothing_lands_beyond_the_inner_edge() {
        let props = scatter(&[(COVER_WOOD, vec![square(-300.0, 300.0)])], 150.0);
        assert!(props.iter().all(|(at, ..)| at[0].abs() <= 150.0 && at[1].abs() <= 150.0));
    }

    #[test]
    fn bushes_line_a_woods_edge_and_not_a_roadside() {
        let wood = vec![(COVER_WOOD, vec![square(0.0, 200.0)])];
        // A road runs along the wood's south side: the edge bends nowhere near it.
        let strength = |p: P| if p[1] < 10.0 { 0.0 } else { 1.0 };
        let grown = edge_scatter(&wood, strength, &[], 1000.0);
        assert!(grown.len() > 100, "{}", grown.len());
        assert!(grown.iter().any(|g| g.1 == TREE_BUSH) && grown.iter().any(|g| g.1 == TREE_BIRCH));
        assert!(grown.iter().all(|g| !g.4), "none of it is solid");
        let edge = |p: P| p[0].min(200.0 - p[0]).abs().min(p[1].min(200.0 - p[1]).abs());
        let outside = |p: P| p[0] < 0.0 || p[0] > 200.0 || p[1] < 0.0 || p[1] > 200.0;
        for (at, ..) in &grown {
            assert!(at[1] >= 10.0, "nothing along the road: {at:?}");
            assert!(edge(*at) <= 4.0 + 1e-9 || outside(*at) && (at[0].clamp(0.0, 200.0) - at[0]).hypot(at[1].clamp(0.0, 200.0) - at[1]) <= 4.0 + 1e-9, "near the edge: {at:?}");
        }
        assert!(grown.iter().filter(|(at, ..)| outside(*at)).count() * 2 > grown.len(), "mostly outside the wood");
        // Where the laser found trees all along the edge, it grows no young trees of its own.
        let trees: Vec<P> = (0..=200).step_by(2).flat_map(|k| [[f64::from(k), 202.0], [f64::from(k), 198.0]]).collect();
        let near_north = edge_scatter(&wood, strength, &trees, 1000.0);
        assert!(near_north.iter().filter(|(at, ..)| at[1] > 195.0).all(|g| g.1 == TREE_BUSH));
    }
}
