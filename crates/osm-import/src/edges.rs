//! Natural edges for natural ground cover. OpenStreetMap traces a wood, a heath or a stretch of bare
//! rock as it does a car park: long straight sides meeting at sharp corners. Where the ground is
//! left to itself its edges curve and wander, so those outlines are rounded and bent by the noise
//! the shores are (`shoreline::roughen`), and bare rock spreads onto the rough, steep ground beside
//! it that the laser shows.
//!
//! Man-made edges stay straight: the strength of the bending (`Hard::strength`) falls to nothing
//! near a road, a track, a building or the outline of a garden, a field or other ground in use, so
//! a wood runs straight along the road through it and the field it borders. It depends on position
//! only, so a wood and the heath beside it, which share their edge, bend it alike.

use crate::assemble::{distance_to_segment, point_in_polygon};
use shared::terrain::Ground;
use shared::world::COVER_ROCK;
use std::collections::HashMap;

type P = [f64; 2];

/// Nearer than this to a man-made edge, metres (past a road's or track's own edge), a natural
/// outline is not moved at all...
const CLEAR: f64 = 1.0;
/// ...and it is bent fully this much further out, metres.
const FADE: f64 = 8.0;
/// How far a man-made edge reaches, metres.
const REACH: f64 = CLEAR + FADE;
/// Side of the squares the man-made edges are sorted into, metres.
const BUCKET: f64 = 16.0;

/// The longest a side of a natural outline is left, metres: the noise's finest wave is 3.5 m.
const SPACING: f64 = 1.5;

/// The furthest bare rock spreads past its outline onto rough or steep ground, metres.
const ROCK_REACH: f64 = 4.0;
/// The step rock spreads by, metres.
const ROCK_STEP: f64 = 0.5;
/// Ground at least this steep is bare rock beside a rock outline, as rise over run (25°).
const ROCK_SLOPE: f64 = 0.466;
/// Ground standing at least this far above the ground round it, metres, is a knoll of bare rock...
const ROCK_KNOLL: f64 = 0.2;
/// ...measured on a circle of this radius, metres.
const KNOLL_RADIUS: f64 = 2.5;
/// How many points either side a point's spread is averaged over, so the rock's new edge is smooth.
const ROCK_SMOOTH: usize = 3;

/// Whether an area's tags make it ground left to itself, whose edges are bent: woods, scrub, heath,
/// grassland, sand, bare rock, wetland, meadows and nature reserves. Parks, mown grass, gardens,
/// fields and every other ground in use keep their straight sides, as do golf courses.
pub fn is_wild(tags: &HashMap<String, String>) -> bool {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    if tags.contains_key("golf") {
        return false;
    }
    matches!(tag("natural"), Some("wood" | "scrub" | "heath" | "grassland" | "sand" | "beach" | "bare_rock" | "wetland"))
        || matches!(tag("landuse"), Some("forest" | "meadow"))
        || tag("leisure") == Some("nature_reserve")
}

/// The man-made edges natural outlines run straight along: road and track surfaces, building
/// walls, and the outlines of ground in use, sorted into squares of `BUCKET` metres.
#[derive(Default)]
pub struct Hard {
    /// Each edge's ends and how far it reaches either side of its line, metres.
    segments: Vec<(P, P, f64)>,
    /// The edges within `REACH` of each square.
    buckets: HashMap<(i32, i32), Vec<u32>>,
}

impl Hard {
    pub fn new() -> Self {
        Self::default()
    }

    /// A road or track down `points`, `width` metres wide.
    pub fn line(&mut self, points: &[P], width: f64) {
        for pair in points.windows(2) {
            self.add(pair[0], pair[1], width / 2.0);
        }
    }

    /// The outline of a building or of ground in use (closed implicitly).
    pub fn ring(&mut self, ring: &[P]) {
        for k in 0..ring.len() {
            self.add(ring[k], ring[(k + 1) % ring.len()], 0.0);
        }
    }

    fn add(&mut self, a: P, b: P, half: f64) {
        // In pieces no longer than a square, so a long side is not filed under squares it passes far from.
        let pieces = ((b[0] - a[0]).hypot(b[1] - a[1]) / BUCKET).ceil().max(1.0) as usize;
        for piece in 0..pieces {
            let (t0, t1) = (piece as f64 / pieces as f64, (piece + 1) as f64 / pieces as f64);
            let from = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0];
            let to = [a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
            let index = self.segments.len() as u32;
            self.segments.push((from, to, half));
            let grow = half + REACH;
            let square = |v: f64| (v / BUCKET).floor() as i32;
            for i in square(from[0].min(to[0]) - grow)..=square(from[0].max(to[0]) + grow) {
                for j in square(from[1].min(to[1]) - grow)..=square(from[1].max(to[1]) + grow) {
                    self.buckets.entry((i, j)).or_default().push(index);
                }
            }
        }
    }

    /// How fully a natural outline is bent at `p`, 0 to 1: nothing within `CLEAR` of a man-made
    /// edge, fully from `REACH` away.
    pub fn strength(&self, p: P) -> f64 {
        let square = ((p[0] / BUCKET).floor() as i32, (p[1] / BUCKET).floor() as i32);
        let Some(near) = self.buckets.get(&square) else { return 1.0 };
        let distance = near.iter().map(|&k| {
            let (a, b, half) = self.segments[k as usize];
            (distance_to_segment(p, a, b) - half).max(0.0)
        });
        let distance = distance.fold(f64::INFINITY, f64::min);
        let t = ((distance - CLEAR) / (REACH - CLEAR)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// Bends the outlines of the `wild` covers (`is_wild`, one flag per cover) as shores are, as far
/// as `hard` lets them, then spreads bare rock over the rough ground beside it.
pub fn naturalise(covers: &mut [(u8, Vec<Vec<P>>)], wild: &[bool], hard: &Hard, ground: &Ground) {
    let strength = |p: P| hard.strength(p);
    let count = |covers: &[(u8, Vec<Vec<P>>)]| covers.iter().zip(wild).filter(|(_, wild)| **wild).flat_map(|((_, rings), _)| rings).map(Vec::len).sum::<usize>();
    let before = count(covers);
    {
        let mut rings: Vec<&mut Vec<P>> = covers.iter_mut().zip(wild).filter(|(_, wild)| **wild).flat_map(|((_, rings), _)| rings.iter_mut()).collect();
        crate::shoreline::roughen(&mut rings, &mut [], SPACING, &strength, true);
    }
    let mut spread = 0.0;
    for ((_, rings), _) in covers.iter_mut().zip(wild).filter(|((kind, _), wild)| **wild && *kind == COVER_ROCK) {
        spread += grow_rock(rings, ground, &strength);
    }
    let wilds = wild.iter().filter(|wild| **wild).count();
    eprintln!("cover: {wilds} natural outlines bent ({before} points to {}), bare rock spread over {:.0} m² of rough ground", count(covers), spread);
}

/// Moves each point of a rock cover's `rings` out from the rock, in steps of `ROCK_STEP` up to
/// `ROCK_REACH`, for as long as the ground there is steep or a knoll (`is_rocky`), smoothed along
/// the ring and scaled by `strength`. Rock only spreads: it is painted over the cover beside it, so
/// no gap opens. Returns roughly how much ground it spread over, m².
fn grow_rock(rings: &mut [Vec<P>], ground: &Ground, strength: &impl Fn(P) -> f64) -> f64 {
    if ground.is_flat() {
        return 0.0;
    }
    let original: Vec<Vec<P>> = rings.to_vec();
    let inside = |p: P| original.iter().filter(|ring| ring.len() > 2 && point_in_polygon(p, ring)).count() % 2 == 1;
    let mut area = 0.0;
    for ring in rings.iter_mut() {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        // Each point's normal to the left of the way the ring runs.
        let left: Vec<P> = (0..n)
            .map(|k| {
                let (a, b) = (ring[(k + n - 1) % n], ring[(k + 1) % n]);
                let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
                let length = dx.hypot(dz).max(1e-9);
                [-dz / length, dx / length]
            })
            .collect();
        // Which side is out of the rock, by a vote of points round the ring.
        let votes: i32 = (0..n).step_by((n / 16).max(1)).map(|k| if inside([ring[k][0] + left[k][0] * 0.3, ring[k][1] + left[k][1] * 0.3]) { -1 } else { 1 }).sum();
        let out = if votes >= 0 { 1.0 } else { -1.0 };
        let reach: Vec<f64> = (0..n)
            .map(|k| {
                let normal = [left[k][0] * out, left[k][1] * out];
                let mut reach = 0.0;
                while reach + ROCK_STEP <= ROCK_REACH + 1e-9 {
                    let s = reach + ROCK_STEP;
                    if !is_rocky(ground, [ring[k][0] + normal[0] * s, ring[k][1] + normal[1] * s]) {
                        break;
                    }
                    reach = s;
                }
                reach
            })
            .collect();
        let width = 2 * ROCK_SMOOTH + 1;
        for k in 0..n {
            let mean = (0..width).map(|d| reach[(k + n + d - ROCK_SMOOTH) % n]).sum::<f64>() / width as f64;
            let push = mean * strength(ring[k]).clamp(0.0, 1.0);
            let side = ring[(k + 1) % n];
            area += push * (side[0] - ring[k][0]).hypot(side[1] - ring[k][1]);
            ring[k] = [ring[k][0] + left[k][0] * out * push, ring[k][1] + left[k][1] * out * push];
        }
    }
    area
}

/// Whether the ground at `p` is bare rock beside a rock outline: steeper than `ROCK_SLOPE`, or a
/// knoll standing `ROCK_KNOLL` above the ground `KNOLL_RADIUS` round it.
fn is_rocky(ground: &Ground, p: P) -> bool {
    let h = |x: f64, z: f64| f64::from(ground.height_at(x as f32, z as f32));
    let (x, z) = (p[0], p[1]);
    let slope = ((h(x + 1.0, z) - h(x - 1.0, z)) / 2.0).hypot((h(x, z + 1.0) - h(x, z - 1.0)) / 2.0);
    if slope >= ROCK_SLOPE {
        return true;
    }
    let around = (0..8).map(|k| {
        let angle = k as f64 * std::f64::consts::FRAC_PI_4;
        h(x + KNOLL_RADIUS * angle.cos(), z + KNOLL_RADIUS * angle.sin())
    });
    h(x, z) - around.sum::<f64>() / 8.0 >= ROCK_KNOLL
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{COVER_FARM, COVER_PARK, COVER_WOOD};

    fn rect(x0: f64, z0: f64, x1: f64, z1: f64) -> Vec<P> {
        vec![[x0, z0], [x1, z0], [x1, z1], [x0, z1]]
    }

    /// The distance from `p` to the nearest point of `ring`'s outline, metres.
    fn off(ring: &[P], p: P) -> f64 {
        (0..ring.len()).map(|k| distance_to_segment(p, ring[k], ring[(k + 1) % ring.len()])).fold(f64::INFINITY, f64::min)
    }

    fn spread(values: impl Iterator<Item = f64>) -> f64 {
        let (lo, hi) = values.fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        hi - lo
    }

    #[test]
    fn a_forests_edge_wanders_but_stays_straight_along_a_road() {
        let mut hard = Hard::new();
        // A road 6 m wide along the wood's south side.
        hard.line(&[[-50.0, 0.0], [250.0, 0.0]], 6.0);
        let mut covers = vec![(COVER_WOOD, vec![rect(0.0, 0.0, 200.0, 200.0)])];
        naturalise(&mut covers, &[true], &hard, &Ground::default());
        let ring = &covers[0].1[0];
        let south: Vec<&P> = ring.iter().filter(|p| p[1] < 2.0).collect();
        assert!(south.iter().all(|p| p[1] == 0.0), "the side along the road stays on it");
        assert!(south.len() <= 4, "and carries no points it does not need: {}", south.len());
        let north = ring.iter().filter(|p| p[1] > 150.0 && p[0] > 30.0 && p[0] < 170.0).map(|p| p[1]);
        assert!(spread(north) > 0.8, "the far side bends");
    }

    #[test]
    fn a_wood_and_the_heath_beside_it_bend_together_without_a_gap() {
        // They share the chain x = 100 with a kink at its middle, walked opposite ways.
        let wood = vec![[0.0, 0.0], [100.0, 0.0], [110.0, 50.0], [100.0, 100.0], [0.0, 100.0]];
        let heath = vec![[100.0, 0.0], [160.0, 0.0], [160.0, 100.0], [100.0, 100.0], [110.0, 50.0]];
        let mut covers = vec![(COVER_WOOD, vec![wood]), (COVER_PARK, vec![heath])];
        naturalise(&mut covers, &[true, true], &Hard::new(), &Ground::default());
        let (wood, heath) = (&covers[0].1[0], &covers[1].1[0]);
        let near = |ring: &[P], p: P| ring.iter().map(|q| (q[0] - p[0]).hypot(q[1] - p[1])).fold(f64::INFINITY, f64::min);
        let shared: Vec<&P> = wood.iter().filter(|p| p[0] > 90.0 && p[1] > 15.0 && p[1] < 85.0).collect();
        assert!(shared.len() > 30, "{}", shared.len());
        for &&p in &shared {
            assert!(near(heath, p) < 1e-6, "the wood's {p:?} is {:.3} m off the heath", near(heath, p));
        }
        assert!(spread(shared.iter().map(|p| p[0] - (110.0 - (p[1] - 50.0).abs() / 5.0))) > 0.5, "the shared edge bends");
        // The kink they share is rounded in both, not kept sharp.
        let n = wood.len();
        for k in 0..n {
            let (a, b, c) = (wood[(k + n - 1) % n], wood[k], wood[(k + 1) % n]);
            if (b[1] - 50.0).abs() > 10.0 || b[0] < 90.0 {
                continue;
            }
            let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            let cos = (u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]));
            assert!(cos > 0.8, "a turn of {:.0}° at {b:?}", cos.acos().to_degrees());
        }
    }

    #[test]
    fn a_fields_sides_stay_straight() {
        let field = rect(100.0, 0.0, 200.0, 100.0);
        let mut hard = Hard::new();
        hard.ring(&field);
        let mut covers = vec![(COVER_WOOD, vec![rect(0.0, 0.0, 100.0, 100.0)]), (COVER_FARM, vec![field.clone()])];
        naturalise(&mut covers, &[true, false], &hard, &Ground::default());
        assert_eq!(covers[1].1[0], field, "the field is left as it was mapped");
        let wood = &covers[0].1[0];
        assert!(wood.contains(&[100.0, 0.0]) && wood.contains(&[100.0, 100.0]), "the corners it shares with the field stay");
        assert!(wood.iter().all(|p| p[0] <= 100.0), "and nothing of the wood spills into the field");
        assert!(wood.iter().filter(|p| p[0] > 99.0).count() == 2, "the side along the field is straight, with no points on it");
        assert!(wood.iter().any(|p| off(&rect(0.0, 0.0, 100.0, 100.0), *p) > 0.5), "and bends away from it");
    }

    #[test]
    fn rock_spreads_up_a_steep_slope_but_not_over_flat_ground() {
        // Flat to x = 20 m, then rising 1 m a metre; samples a metre apart from -50 m.
        let side = 101u32;
        let heights = (0..side * side).map(|k| ((f64::from(k % side) - 50.0 - 20.0).max(0.0) * 100.0) as i32).collect();
        let ground = Ground { origin: -5000, cell: 100, side, heights };
        // A 10 m square with a point every metre, its east side at the foot of the slope.
        let mut square = Vec::new();
        for k in 0..10 {
            square.push([10.0 + f64::from(k), 0.0]);
        }
        for k in 0..10 {
            square.push([20.0, f64::from(k)]);
        }
        for k in 0..10 {
            square.push([20.0 - f64::from(k), 10.0]);
        }
        for k in 0..10 {
            square.push([10.0, 10.0 - f64::from(k)]);
        }
        let mut rings = vec![square];
        let spread = grow_rock(&mut rings, &ground, &|_| 1.0);
        assert!(spread > 20.0, "{spread:.0} m²");
        let east: Vec<f64> = rings[0][13..17].iter().map(|p| p[0]).collect();
        assert!(east.iter().all(|&x| x > 23.0 && x <= 24.0 + 1e-9), "the east side climbs the slope: {east:?}");
        let west: Vec<f64> = rings[0][33..37].iter().map(|p| p[0]).collect();
        assert!(west.iter().all(|&x| x == 10.0), "the west side stays on the flat: {west:?}");
    }
}
