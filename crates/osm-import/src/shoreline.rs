//! Natural shorelines from OpenStreetMap's outlines. A lake or a riverbank in OSM is a polygon of
//! long straight sides meeting at sharp corners, as it was traced, where a real shore curves and
//! wanders. Before the water is laid out (`water.rs`) every outline is given a point every
//! `SPACING` metres, its corners are rounded (`ROUND`) and it is bent by a smooth noise a few
//! metres across (`WANDER`). The edges of natural ground cover are bent by the same noise
//! (`roughen`, from `edges.rs`), so a wood that runs down to a lake meets its shore.
//!
//! Outlines share edges: a lake and the river that drains it, the pieces of one riverbank, an
//! island and the lake round it, a wood and the heath beside it. Each shared edge must come out the
//! same from both polygons, or a sliver or a seam opens between them. So the noise moves a point by
//! where it is, never by which way its outline runs, and a corner is rounded only where every
//! outline through it runs on to the same neighbours; where outlines part, it is kept.

use std::collections::HashMap;

type P = [f64; 2];

/// The longest a side of a shore is left, metres: shorter than the noise's finest wave.
const SPACING: f64 = 1.0;

/// The most a corner is cut back along each of its sides, metres...
const ROUND: f64 = 4.0;
/// ...and no more than this share of the side.
const ROUND_SHARE: f64 = 0.25;
/// How many times the corners are cut: each rounds what the last left.
const ROUND_PASSES: usize = 3;

/// The noise the outline is bent by: for each octave, its wavelength and the most it moves a
/// point, metres. A shore wanders by a few metres over a hundred, with smaller bays and points on
/// that; at worst a point moves about 5 m. Each octave's steepest stretch is well under one, so the noise never folds an
/// outline over itself, and the two banks of a narrow river move together.
const WANDER: [(f64, f64); 4] = [(64.0, 3.0), (24.0, 1.4), (9.0, 0.55), (3.5, 0.18)];

/// Smooths the outlines of polygons (`rings`, each closed implicitly, its first point not repeated)
/// and of open lines (`lines`, such as coastlines: a line whose ends meet is a ring, and the ends
/// of one that does not stay put, since the sea is closed along the world's edge from them).
pub fn smooth(rings: &mut [&mut Vec<P>], lines: &mut [&mut Vec<P>]) {
    roughen(rings, lines, SPACING, &|_| 1.0, false);
}

/// Smooths outlines as `smooth` does, a point every `spacing` metres, with the rounding and the
/// noise scaled at each place by `strength` (0 to 1, a function of position only, so outlines that
/// share an edge still agree on it). Where it is 0 a corner stays sharp and a side straight, and
/// with `sparse` the points added along such a side are left out again, since they lie on it.
pub fn roughen(rings: &mut [&mut Vec<P>], lines: &mut [&mut Vec<P>], spacing: f64, strength: &impl Fn(P) -> f64, sparse: bool) {
    // Each outline as its points, each with whether it is kept where it is, and whether it is closed.
    let mut outlines: Vec<(Vec<(P, bool)>, bool)> = Vec::with_capacity(rings.len() + lines.len());
    for ring in rings.iter() {
        outlines.push((ring.iter().map(|&p| (p, false)).collect(), true));
    }
    for line in lines.iter() {
        let closed = line.len() > 3 && line.first() == line.last();
        let points = if closed { &line[..line.len() - 1] } else { &line[..] };
        let last = points.len().saturating_sub(1);
        outlines.push((points.iter().enumerate().map(|(k, &p)| (p, !closed && (k == 0 || k == last))).collect(), closed));
    }
    round(&mut outlines, strength);
    let smoothed: Vec<Vec<P>> = outlines
        .iter()
        .map(|(points, closed)| {
            let points: Vec<P> = points.iter().map(|&(p, _)| p).collect();
            if points.len() < if *closed { 3 } else { 2 } {
                return points;
            }
            wander(&densify(&points, *closed, spacing), *closed, strength, sparse)
        })
        .collect();
    let mut smoothed = smoothed.into_iter();
    for ring in rings.iter_mut() {
        let next = smoothed.next().unwrap_or_default();
        if ring.len() >= 3 {
            **ring = next;
        }
    }
    for line in lines.iter_mut() {
        let next = smoothed.next().unwrap_or_default();
        let closed = line.len() > 3 && line.first() == line.last();
        if !closed && line.len() < 2 {
            continue;
        }
        **line = next;
        if closed {
            line.push(line[0]);
        }
    }
}

/// The key a point is known by among all the outlines: its coordinates' bits.
type Key = (u64, u64);

/// The key a point is known by among all the outlines.
fn key(p: P) -> Key {
    (p[0].to_bits(), p[1].to_bits())
}

/// Cuts the outlines' corners (as Chaikin's), `ROUND_PASSES` times. A corner is kept where it is
/// when it is pinned (the ends of an open line), where `strength` is nothing, or where the outlines
/// through it do not all run on to the same two neighbours: where a lake meets a river, or a wood
/// both a heath and a field. Along an edge two outlines share, both cut each corner alike.
fn round(outlines: &mut [(Vec<(P, bool)>, bool)], strength: &impl Fn(P) -> f64) {
    for _ in 0..ROUND_PASSES {
        // Each point's neighbours, as every outline through it has them; `None` once two disagree.
        let mut neighbours: HashMap<Key, Option<[Key; 2]>> = HashMap::new();
        for (points, closed) in outlines.iter() {
            let n = points.len();
            for k in 0..n {
                let (before, after) = match (*closed, k) {
                    (true, _) => (points[(k + n - 1) % n].0, points[(k + 1) % n].0),
                    (false, 0) => (points[0].0, points[(1).min(n - 1)].0),
                    (false, _) if k + 1 == n => (points[k - 1].0, points[k].0),
                    (false, _) => (points[k - 1].0, points[k + 1].0),
                };
                let mut pair = [key(before), key(after)];
                pair.sort_unstable();
                neighbours.entry(key(points[k].0)).and_modify(|seen| if *seen != Some(pair) { *seen = None }).or_insert(Some(pair));
            }
        }
        for (points, closed) in outlines.iter_mut() {
            let n = points.len();
            if n < 3 {
                continue;
            }
            let mut next = Vec::with_capacity(2 * n);
            for k in 0..n {
                let (p, pinned) = points[k];
                let reach = strength(p).clamp(0.0, 1.0);
                if pinned || reach <= 0.0 || neighbours.get(&key(p)).is_none_or(Option::is_none) {
                    next.push((p, pinned));
                    continue;
                }
                let (before, after) = match (*closed, k) {
                    (true, _) => (points[(k + n - 1) % n].0, points[(k + 1) % n].0),
                    (false, _) => (points[k - 1].0, points[k + 1].0),
                };
                // A point a side's share of the way back towards each neighbour, the corner between left out.
                let towards = |q: P| {
                    let length = (q[0] - p[0]).hypot(q[1] - p[1]);
                    let t = if length > 0.0 { (ROUND * reach / length).min(ROUND_SHARE) } else { 0.0 };
                    [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
                };
                next.push((towards(before), false));
                next.push((towards(after), false));
            }
            *points = next;
        }
    }
}

/// `outline` with points added so that no side is longer than `spacing`, evenly along each side
/// (so a side two outlines share is split at the same places from either end), each point with
/// whether it was added.
fn densify(outline: &[P], closed: bool, spacing: f64) -> Vec<(P, bool)> {
    let n = outline.len();
    let sides = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(n);
    for k in 0..sides {
        let (a, b) = (outline[k], outline[(k + 1) % n]);
        let pieces = ((b[0] - a[0]).hypot(b[1] - a[1]) / spacing).ceil().max(1.0) as usize;
        for i in 0..pieces {
            let t = i as f64 / pieces as f64;
            out.push(([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], i > 0));
        }
    }
    if !closed {
        out.push((outline[n - 1], false));
    }
    out
}

/// `outline` with every point moved by the noise at its place (`offset`), scaled by `strength`;
/// the ends of an open line stay put. With `sparse`, an added point the noise does not move is
/// left out.
fn wander(outline: &[(P, bool)], closed: bool, strength: &impl Fn(P) -> f64, sparse: bool) -> Vec<P> {
    let last = outline.len() - 1;
    outline
        .iter()
        .enumerate()
        .filter_map(|(k, &(p, added))| {
            if !closed && (k == 0 || k == last) {
                return Some(p);
            }
            let reach = strength(p).clamp(0.0, 1.0);
            if reach <= 0.0 {
                return (!sparse || !added).then_some(p);
            }
            let d = offset(p);
            Some([p[0] + d[0] * reach, p[1] + d[1] * reach])
        })
        .collect()
}

/// How far the noise moves a point at `p`, metres: the same for every outline through it.
pub fn offset(p: P) -> P {
    let mut d = [0.0; 2];
    for (octave, &(wavelength, reach)) in WANDER.iter().enumerate() {
        let q = [p[0] / wavelength, p[1] / wavelength];
        let seed = 2 * octave as u32;
        d[0] += reach * value_noise(q, seed);
        d[1] += reach * value_noise(q, seed + 1);
    }
    d
}

/// Smooth value noise over a unit lattice, -1 to 1, a different field for each `seed`.
fn value_noise(p: P, seed: u32) -> f64 {
    let (i, j) = (p[0].floor(), p[1].floor());
    let (fx, fy) = (p[0] - i, p[1] - j);
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let corner = |di: f64, dj: f64| lattice((i + di) as i64, (j + dj) as i64, seed);
    let near = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * u;
    let far = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * u;
    near + (far - near) * v
}

/// A lattice point's value, -1 to 1: a hash of where it is, the same every import.
fn lattice(i: i64, j: i64, seed: u32) -> f64 {
    let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ u64::from(seed).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<P> {
        vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]
    }

    /// The distance from `p` to the nearest side of `ring`, metres.
    fn distance(ring: &[P], p: P) -> f64 {
        (0..ring.len())
            .map(|k| {
                let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                let along = [b[0] - a[0], b[1] - a[1]];
                let t = (((p[0] - a[0]) * along[0] + (p[1] - a[1]) * along[1]) / (along[0] * along[0] + along[1] * along[1])).clamp(0.0, 1.0);
                (p[0] - a[0] - along[0] * t).hypot(p[1] - a[1] - along[1] * t)
            })
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn a_square_ponds_sides_wander_and_its_corners_round() {
        let mut ring = square();
        smooth(&mut [&mut ring], &mut []);
        assert!(ring.len() > 350, "a point every metre or so: {}", ring.len());
        let most: f64 = WANDER.iter().map(|&(_, reach)| reach).sum();
        // Every point stays near the outline it came from, but the sides no longer run straight.
        let offs: Vec<f64> = ring.iter().map(|&p| distance(&square(), p)).collect();
        assert!(offs.iter().all(|&d| d < ROUND + most), "{:?}", offs.iter().cloned().fold(0.0, f64::max));
        let side: Vec<f64> = ring.iter().filter(|p| p[0] > 10.0 && p[0] < 90.0 && p[1] < 10.0).map(|p| p[1]).collect();
        let spread = side.iter().cloned().fold(f64::MIN, f64::max) - side.iter().cloned().fold(f64::MAX, f64::min);
        assert!(spread > 0.8, "the south side bends by {spread:.2} m");
        // The corner is cut off.
        assert!(ring.iter().map(|&p| p[0].hypot(p[1])).fold(f64::INFINITY, f64::min) > 1.0);
        // And no turn is sharp any more.
        let n = ring.len();
        for k in 0..n {
            let (a, b, c) = (ring[(k + n - 1) % n], ring[k], ring[(k + 1) % n]);
            let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            let cos = (u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]));
            assert!(cos > 0.5, "a turn of {:.0}° at {b:?}", cos.acos().to_degrees());
        }
    }

    #[test]
    fn two_bodies_sharing_a_side_still_meet_along_it() {
        // A lake and the river beside it, the side x = 100 between them, walked opposite ways.
        let mut lake = square();
        let mut river = vec![[100.0, 0.0], [160.0, 0.0], [160.0, 100.0], [100.0, 100.0]];
        smooth(&mut [&mut lake, &mut river], &mut []);
        let near = |ring: &[P], p: P| ring.iter().map(|q| (q[0] - p[0]).hypot(q[1] - p[1])).fold(f64::INFINITY, f64::min);
        // The middle of the shared side, clear of the other sides however the noise moves them.
        let most: f64 = WANDER.iter().map(|&(_, reach)| reach).sum();
        let joined = lake.iter().filter(|p| (p[0] - 100.0).abs() < most && p[1] > ROUND + 2.0 * most && p[1] < 100.0 - ROUND - 2.0 * most).collect::<Vec<_>>();
        assert!(joined.len() > 50, "{}", joined.len());
        for &&p in &joined {
            assert!(near(&river, p) < 1e-6, "the lake's {p:?} is {:.3} m off the river", near(&river, p));
        }
    }

    #[test]
    fn a_coastlines_ends_stay_put() {
        let mut coast = vec![[-500.0, 3.0], [0.0, -2.0], [500.0, 4.0]];
        smooth(&mut [], &mut [&mut coast]);
        assert_eq!(coast.first(), Some(&[-500.0, 3.0]));
        assert_eq!(coast.last(), Some(&[500.0, 4.0]));
        assert!(coast.len() > 900);
    }

    #[test]
    fn the_same_outline_comes_out_the_same_every_import() {
        let (mut a, mut b) = (square(), square());
        smooth(&mut [&mut a], &mut []);
        smooth(&mut [&mut b], &mut []);
        assert_eq!(a, b);
    }
}
