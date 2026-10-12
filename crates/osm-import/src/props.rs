//! Rocks and bushes the map does not record, scattered over woods and parks.
//!
//! OpenStreetMap and the laser data have no boulders or shrubs worth drawing, so they are placed
//! deterministically from the cover polygons: the same input always gives the same world, and tile
//! files (which are hashed) do not churn. They are returned as trees of the rock and bush
//! archetypes, so they go through the same rejection (buildings, roads, bridges) and tiling as
//! every tree. Rocks are solid; a bush never is (`assemble` keeps it passable).

use crate::assemble::point_in_polygon;
use shared::world::{COVER_PARK, COVER_SAND, COVER_WATER, COVER_WOOD, TREE_BUSH, TREE_ROCK_BOULDER, TREE_ROCK_SLAB};

type P = [f64; 2];
/// `(position, TREE_*, height, crown radius, solid)`, as `assemble` takes its trees.
pub type Placed = (P, u8, f64, f64, bool);

/// One attempt per this many square metres of a cover's bounding box, per kind of prop.
const ROCK_AREA: f64 = 9_000.0;
const BUSH_AREA: f64 = 2_500.0;
/// Most attempts of one kind in one cover, so a huge wood is not a quarry.
const MAX_PER_COVER: usize = 400;

/// SplitMix64: a well-mixed 64-bit value from a counter.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A value in `[0, 1)` from a hash.
fn unit(hash: u64) -> f64 {
    (hash >> 11) as f64 / (1u64 << 53) as f64
}

/// Inside the rings by the even-odd rule, so an inner ring is a hole.
fn inside(rings: &[Vec<P>], at: P) -> bool {
    rings.iter().filter(|ring| ring.len() > 2 && point_in_polygon(at, ring)).count() % 2 == 1
}

fn bounds(rings: &[Vec<P>]) -> Option<[f64; 4]> {
    let mut found = false;
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in rings.iter().flatten() {
        found = true;
        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
    }
    found.then_some(b)
}

/// Rocks and bushes inside the wood, park and sand covers, never in water, within `±inner` metres.
pub fn scatter(covers: &[(u8, Vec<Vec<P>>)], inner: f64) -> Vec<Placed> {
    let water: Vec<([f64; 4], &Vec<Vec<P>>)> =
        covers.iter().filter(|(kind, _)| *kind == COVER_WATER).filter_map(|(_, rings)| Some((bounds(rings)?, rings))).collect();
    let in_water = |at: P| water.iter().any(|(b, rings)| at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3] && inside(rings, at));
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
                if at[0].abs() > inner || at[1].abs() > inner || !inside(rings, at) || in_water(at) {
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
    fn nothing_lands_beyond_the_inner_edge() {
        let props = scatter(&[(COVER_WOOD, vec![square(-300.0, 300.0)])], 150.0);
        assert!(props.iter().all(|(at, ..)| at[0].abs() <= 150.0 && at[1].abs() <= 150.0));
    }
}
