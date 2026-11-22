//! Railways where they leave the ground: the decks that carry them over roads and passages, the
//! ramps that lead on from those, and the track laid on top (`Rail::deck`).
//!
//! A railway's bridges are built as a road's are (`bridge`), from a list of their own, so a track
//! never shares a deck with a road, follows one or hides one. What differs is here: tracks side by
//! side share one deck however many they are, as a station's yard over a footway does, and the
//! track itself is laid on the deck's top, piece by piece with the sections it lies on.

use crate::assemble::Deck;
use crate::bridge::{self, Pylons};
use std::collections::HashSet;

type P = [f64; 2];
type V3 = [f64; 3];

/// A railway bridge: its centre line, width, `bridge::TRACK`, pylons and deck returns.
type DeckWay = (Vec<P>, f64, u8, Pylons, Deck);

/// What `assemble` builds for the railways beside the tracks on the ground.
#[derive(Default)]
pub struct Railway {
    /// The decks, as bridges to build (`decks`).
    pub decks: Vec<DeckWay>,
    /// The tracks of the ways mapped as bridges: `(centre line, gauge, RAIL_*)`.
    pub spans: Vec<(Vec<P>, f64, u8)>,
    /// The roads on the ground with a signed height limit: `(centre line, metres)`.
    pub limits: Vec<(Vec<P>, f64)>,
}

/// Tracks whose beds are no further apart than this share a deck, metres: a double track's are a
/// metre apart, and a yard's little more.
const SHARED_GAP: f64 = 2.0;
/// How far apart two decks' lines are compared, metres.
const COMPARE_STEP: f64 = 2.0;
/// How far a deck's ramp is led along the track that goes on from it, metres: as long as a ramp is.
const LEAD: f64 = 150.0;
/// How far apart a track is tested for lying on a deck, metres.
const LAY_STEP: f64 = 1.0;
/// A track lies on a deck where it runs along it, within this of the deck's line in direction (the
/// cosine of the angle between them): one that crosses under it is on the ground.
const LAY_ALONG: f64 = 0.8;
/// This far past the end of a way is looked at for whether the track comes off its deck there, metres:
/// a bridge's own way begins where the track on the ground ends.
const PAST_END: f64 = 0.1;
/// How many times the step to a deck's edge is halved to find the edge: to a millimetre.
const EDGE_HALVINGS: usize = 10;
/// Where a track leaves the ground for a deck it eases onto the deck's height over this, metres.
const EASE: f64 = 6.0;
/// As far as it eases down, metres: the sleepers and rails still show over the deck's ballast.
const EASE_SINK: f64 = 0.15;
/// As far as it eases up, metres: where the ground at a deck's edge is a little over the deck's
/// middle, as at the side of a wide one. Any further and it is no edge the track runs over.
const EASE_RISE: f64 = 0.5;
/// A piece of track shorter than this between two others is left out, metres.
const SHORTEST: f64 = 0.02;

fn unit(a: P, b: P) -> P {
    let length = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
    [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
}

fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn bounds(points: impl Iterator<Item = P>) -> [f64; 4] {
    points.fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, [x, z]| [b[0].min(x), b[1].min(z), b[2].max(x), b[3].max(z)])
}

fn near(a: &[f64; 4], b: &[f64; 4], within: f64) -> bool {
    a[0] <= b[2] + within && b[0] <= a[2] + within && a[1] <= b[3] + within && b[1] <= a[3] + within
}

/// The segment of `line` nearest `p`: its ends, the point on it and how far that is.
fn nearest(line: &[P], p: P) -> Option<(P, P, P, f64)> {
    line.windows(2)
        .map(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let t = (dot([p[0] - a[0], p[1] - a[1]], d) / dot(d, d).max(1e-9)).clamp(0.0, 1.0);
            let on = [a[0] + d[0] * t, a[1] + d[1] * t];
            (a, b, on, (p[0] - on[0]).hypot(p[1] - on[1]))
        })
        .min_by(|a, b| a.3.total_cmp(&b.3))
}

/// The points the `tracks` share with `roads`: their level crossings.
pub fn level_crossings<'a>(tracks: impl Iterator<Item = &'a [P]>, roads: impl Iterator<Item = &'a [P]>) -> Vec<P> {
    // Two ways that share a node of OpenStreetMap have it projected to the last bit.
    let key = |p: &P| p.map(f64::to_bits);
    let on_roads: HashSet<[u64; 2]> = roads.flatten().map(key).collect();
    tracks.flatten().filter(|p| on_roads.contains(&key(p))).copied().collect()
}

/// Whether two decks are one: somewhere they run alongside, their beds `SHARED_GAP` apart or less.
fn side_by_side(a: &DeckWay, b: &DeckWay) -> bool {
    let reach = (a.1 + b.1) / 2.0 + SHARED_GAP;
    if a.0.len() < 2 || b.0.len() < 2 || !near(&bounds(a.0.iter().copied()), &bounds(b.0.iter().copied()), reach) {
        return false;
    }
    bridge::resample(&a.0, COMPARE_STEP).windows(2).any(|pair| {
        let heading = unit(pair[0], pair[1]);
        nearest(&b.0, pair[0]).is_some_and(|(c, d, _, off)| off < reach && dot(heading, unit(c, d)).abs() > bridge::PARALLEL)
    })
}

/// One deck under both: along the longer, carried on straight as far as the other reaches past its
/// ends, and as wide as from the outer edge of one to the outer edge of the other.
fn union(a: &DeckWay, b: &DeckWay) -> DeckWay {
    let length = |way: &DeckWay| way.0.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum::<f64>();
    let (long, short) = if length(a) >= length(b) { (a, b) } else { (b, a) };
    let mut line = long.0.clone();
    let others = bridge::resample(&short.0, COMPARE_STEP);
    for _ in 0..2 {
        // The end at the line's start, then, turned round, the other.
        let (end, out) = (line[0], unit(line[1], line[0]));
        let past = others.iter().map(|q| dot([q[0] - end[0], q[1] - end[1]], out)).fold(0.0, f64::max);
        if past > 0.1 {
            line.insert(0, [end[0] + out[0] * past, end[1] + out[1] * past]);
        }
        line.reverse();
    }
    // How far each edge lies to the left of the line.
    let (mut low, mut high) = (-long.1 / 2.0, long.1 / 2.0);
    for &q in &others {
        let Some((c, d, ..)) = nearest(&line, q) else { continue };
        let along = unit(c, d);
        let left = dot([q[0] - c[0], q[1] - c[1]], [-along[1], along[0]]);
        (low, high) = (low.min(left - short.1 / 2.0), high.max(left + short.1 / 2.0));
    }
    let shift = (low + high) / 2.0;
    let last = line.len() - 1;
    let middle = (0..=last)
        .map(|i| {
            let along = unit(line[i.saturating_sub(1)], line[(i + 1).min(last)]);
            [line[i][0] - along[1] * shift, line[i][1] + along[0] * shift]
        })
        .collect();
    // A deck over a cutting that meets a bridge is part of the bridge.
    let pylons = if a.3 == Pylons::Flush { b.3 } else { a.3 };
    (middle, high - low, bridge::TRACK, pylons, long.4.iter().chain(&short.4).copied().collect())
}

/// The decks of the railways' bridges and cuttings, every lot of them that run side by side made
/// one: five tracks over a passage are one deck, and not five that lie over each other's edges.
pub fn decks(mut ways: Vec<DeckWay>) -> Vec<DeckWay> {
    ways.retain(|way| way.0.len() >= 2);
    while let Some((i, j)) = (0..ways.len()).flat_map(|i| (i + 1..ways.len()).map(move |j| (i, j))).find(|&(i, j)| side_by_side(&ways[i], &ways[j]) || side_by_side(&ways[j], &ways[i])) {
        let other = ways.remove(j);
        ways[i] = union(&ways[i], &other);
    }
    ways
}

/// Where a ramp runs past the end of a deck (`width` wide; its last point with `from_end`, else its
/// first): the line of the track on the ground that goes on from there, on through the switches by
/// the straightest, moved over to start at the deck's middle. The deck may end part of the way along
/// a track's way, as one lengthened onto its bank does. Empty where no track goes on.
pub fn lead(deck: &[P], from_end: bool, width: f64, tracks: &[&[P]]) -> Vec<P> {
    let n = deck.len();
    if n < 2 {
        return Vec::new();
    }
    let (end, before) = if from_end { (deck[n - 1], deck[n - 2]) } else { (deck[0], deck[1]) };
    let heading = unit(before, end);
    let turn = std::f64::consts::FRAC_1_SQRT_2;
    let reach = width / 2.0 + 1.0;
    let mut best: Option<(f64, usize, Vec<P>)> = None;
    for (k, track) in tracks.iter().enumerate() {
        for i in 0..track.len().saturating_sub(1) {
            let Some((a, b, on, off)) = nearest(&track[i..=i + 1], end) else { continue };
            let straight = dot(unit(a, b), heading);
            if off > reach || straight.abs() < turn {
                continue;
            }
            let rest: Vec<P> = if straight > 0.0 { track[i + 1..].to_vec() } else { track[..=i].iter().rev().copied().collect() };
            let line: Vec<P> = std::iter::once(on).chain(rest).collect();
            // A track that only comes up to the deck's end does not go on from it.
            if dot([line[line.len() - 1][0] - end[0], line[line.len() - 1][1] - end[1]], heading) < 1.0 {
                continue;
            }
            let score = off + (1.0 - straight.abs());
            if best.as_ref().is_none_or(|(known, ..)| score < *known) {
                best = Some((score, k, line));
            }
        }
    }
    let Some((_, first, mut line)) = best else { return Vec::new() };
    let mut taken = vec![first];
    while line.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum::<f64>() < LEAD {
        let (at, going) = (line[line.len() - 1], unit(line[line.len() - 2], line[line.len() - 1]));
        let next = tracks
            .iter()
            .enumerate()
            .filter(|(k, track)| track.len() >= 2 && !taken.contains(k))
            .flat_map(|(k, track)| [(k, false, track[0], track[1]), (k, true, track[track.len() - 1], track[track.len() - 2])])
            .filter(|(.., start, _)| (start[0] - at[0]).hypot(start[1] - at[1]) < 0.5)
            .map(|(k, reversed, start, then)| (k, reversed, dot(going, unit(start, then))))
            .filter(|&(.., straight)| straight > turn)
            .max_by(|a, b| a.2.total_cmp(&b.2));
        let Some((k, reversed, _)) = next else { break };
        taken.push(k);
        let mut points = tracks[k].to_vec();
        if reversed {
            points.reverse();
        }
        line.extend(points.into_iter().skip(1));
    }
    let over = [end[0] - line[0][0], end[1] - line[0][1]];
    line.into_iter().map(|p| [p[0] + over[0], p[1] + over[1]]).collect()
}

/// The top of one section of a deck, for track to be laid on.
pub struct Bed {
    /// The tile that holds the section, and its place in that tile's buildings.
    pub tile: (u16, u16),
    pub building: u32,
    /// Whether a railway's deck: a tram's track may lie on a road's, a train's on no other.
    track: bool,
    roof: Vec<[V3; 3]>,
    /// The middle line of the whole deck.
    line: Vec<P>,
    bounds: [f64; 4],
}

impl Bed {
    pub fn new(tile: (u16, u16), building: u32, track: bool, roof: Vec<[V3; 3]>, line: Vec<P>) -> Self {
        let bounds = bounds(roof.iter().flatten().map(|v| [v[0], v[2]]));
        Bed { tile, building, track, roof, line, bounds }
    }
}

/// A stretch of a track: on the ground, or on a bed with the deck's height at each point, metres.
#[derive(Debug, PartialEq)]
pub struct Run {
    pub points: Vec<P>,
    pub on: Option<(usize, Vec<f64>)>,
}

/// Cuts a track into the runs on the ground and the runs on each of `beds`. A run on the ground
/// keeps the way's own points; one on a deck has one every `LAY_STEP`, each at the deck's height
/// there. Runs meet at the beds' edges, and where one leaves the ground it starts at the ground's
/// height (`ground`, metres at a point). A `tram` may lie on any deck, else only a railway's.
pub fn lay(points: &[P], tram: bool, beds: &[Bed], ground: &impl Fn(P) -> f64) -> Vec<Run> {
    let whole = bounds(points.iter().copied());
    let candidates: Vec<usize> = (0..beds.len()).filter(|&k| (tram || beds[k].track) && near(&whole, &beds[k].bounds, 0.0)).collect();
    if candidates.is_empty() || points.len() < 2 {
        return vec![Run { points: points.to_vec(), on: None }];
    }
    // The bed under `p`, the highest, and the deck's height there; the one it is on already wins a tie.
    let under = |p: P, heading: P, local: &[usize], current: Option<usize>| -> Option<(usize, f64)> {
        let mut found: Option<(usize, f64)> = None;
        for &k in local {
            let bed = &beds[k];
            if !near(&[p[0], p[1], p[0], p[1]], &bed.bounds, 0.01) {
                continue;
            }
            let Some(top) = bridge::roadway_at(&bed.roof, p) else { continue };
            let higher = found.is_none_or(|(_, known)| top > known + 1e-6 || (Some(k) == current && top > known - 1e-6));
            if higher && nearest(&bed.line, p).is_some_and(|(a, b, ..)| dot(heading, unit(a, b)).abs() > LAY_ALONG) {
                found = Some((k, top));
            }
        }
        found
    };
    let mut runs: Vec<Run> = Vec::new();
    let mut run: Option<Run> = None;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        if length <= 0.0 {
            continue;
        }
        let heading = unit(a, b);
        let box_of = bounds(pair.iter().copied());
        let local: Vec<usize> = candidates.iter().copied().filter(|&k| near(&box_of, &beds[k].bounds, 0.01)).collect();
        let steps = if local.is_empty() { 1 } else { (length / LAY_STEP).ceil() as usize };
        let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let state = |t: f64, current: Option<usize>| under(at(t), heading, &local, current);
        let starting = |t: f64, on: Option<(usize, f64)>| Run { points: vec![at(t)], on: on.map(|(bed, top)| (bed, vec![top])) };
        let current = run.as_ref().and_then(|run| run.on.as_ref().map(|on| on.0));
        let here = state(0.0, current);
        // The run changes at the point itself where the track turns onto or off a deck's line.
        match &mut run {
            Some(known) if current != here.map(|found| found.0) => runs.push(std::mem::replace(known, starting(0.0, here))),
            Some(_) => {}
            None => run = Some(starting(0.0, here)),
        }
        let run = run.as_mut().expect("just started");
        for step in 1..=steps {
            let (mut from, to) = ((step - 1) as f64 / steps as f64, step as f64 / steps as f64);
            loop {
                let current = run.on.as_ref().map(|on| on.0);
                let there = state(to, current);
                if there.map(|found| found.0) == current {
                    if let (Some((_, tops)), Some((_, top))) = (run.on.as_mut(), there) {
                        tops.push(top);
                        run.points.push(at(to));
                    } else if step == steps {
                        run.points.push(b);
                    }
                    break;
                }
                // Halve the step to the edge: `low` is still where the run is, `high` past it.
                let (mut low, mut high) = (from, to);
                for _ in 0..EDGE_HALVINGS {
                    let middle = (low + high) / 2.0;
                    if state(middle, current).map(|found| found.0) == current { low = middle } else { high = middle }
                }
                let last = state(low, current);
                run.points.push(at(if current.is_some() { low } else { high }));
                if let Some((_, tops)) = run.on.as_mut() {
                    let height = last.map(|found| found.1).unwrap_or(tops[tops.len() - 1]);
                    tops.push(height);
                }
                runs.push(std::mem::replace(run, starting(high, state(high, current))));
                from = high;
            }
        }
    }
    runs.extend(run);
    runs.retain(|run| run.points.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum::<f64>() >= SHORTEST);
    // Whether the way comes straight from the ground at its first point, and goes on to it at its last.
    let last = points.len() - 1;
    let off_deck = [(points[0], points[1]), (points[last], points[last - 1])].map(|(end, inside)| {
        let out = unit(inside, end);
        under([end[0] + out[0] * PAST_END, end[1] + out[1] * PAST_END], out, &candidates, None).is_none()
    });
    ease(&mut runs, off_deck, ground);
    runs
}

/// Brings each stretch of deck track onto the ground's height where the track comes off it, over
/// `EASE`, and nowhere lets it under ground that is a little over the deck.
fn ease(runs: &mut [Run], off_deck: [bool; 2], ground: &impl Fn(P) -> f64) {
    for run in runs.iter_mut() {
        if let Some((_, tops)) = run.on.as_mut() {
            for (&p, top) in run.points.iter().zip(tops.iter_mut()) {
                let level = ground(p);
                if level > *top && level - *top <= EASE_RISE {
                    *top = level;
                }
            }
        }
    }
    for backwards in [false, true] {
        // The points of each stretch within `EASE` of where it comes off the ground: run, point and how far on.
        let mut stretches: Vec<Vec<(usize, usize, f64)>> = Vec::new();
        let mut open = off_deck[usize::from(backwards)];
        let mut onto: Option<(f64, P)> = None;
        for k in 0..runs.len() {
            let k = if backwards { runs.len() - 1 - k } else { k };
            if runs[k].on.is_none() {
                (open, onto) = (true, None);
                continue;
            }
            let n = runs[k].points.len();
            for i in 0..n {
                let i = if backwards { n - 1 - i } else { i };
                let p = runs[k].points[i];
                if open {
                    (open, onto) = (false, Some((0.0, p)));
                    stretches.push(Vec::new());
                }
                let Some((far, last)) = onto else { continue };
                let far = far + (p[0] - last[0]).hypot(p[1] - last[1]);
                if far > EASE {
                    onto = None;
                    continue;
                }
                onto = Some((far, p));
                stretches.last_mut().expect("just opened").push((k, i, far));
            }
        }
        // A straight ramp from the ground's height to the deck's, not the deck's own lip.
        let top_of = |runs: &[Run], k: usize, i: usize| runs[k].on.as_ref().map_or(0.0, |on| on.1[i]);
        for stretch in stretches {
            let ((k, i, _), (end_k, end_i, length)) = (stretch[0], stretch[stretch.len() - 1]);
            let (from, to) = (ground(runs[k].points[i]), top_of(runs, end_k, end_i));
            if length <= 0.0 || (from - top_of(runs, k, i)).abs() > EASE_RISE {
                continue;
            }
            for (k, i, far) in stretch {
                let ramp = (from + (to - from) * far / length).max(top_of(runs, k, i) - EASE_SINK);
                if let Some((_, tops)) = runs[k].on.as_mut() {
                    tops[i] = ramp;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(z: f64, from: f64, to: f64) -> DeckWay {
        (vec![[from, z], [to, z]], 3.6, bridge::TRACK, Pylons::Flush, Deck::new())
    }

    #[test]
    fn five_tracks_over_a_skewed_passage_share_one_deck_and_one_far_off_keeps_its_own() {
        let mut ways: Vec<DeckWay> = (0..5).map(|k| track(f64::from(k) * 4.5, -10.0 + f64::from(k) * 2.0, 10.0 + f64::from(k) * 2.0)).collect();
        ways.push(track(40.0, -10.0, 10.0));
        // A track that crosses them is no part of their deck.
        ways.push((vec![[0.0, -8.0], [0.0, 24.0]], 3.6, bridge::TRACK, Pylons::Flush, Deck::new()));
        let mut found = decks(ways);
        assert_eq!(found.len(), 3, "{found:?}");
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        let (line, width, _, pylons, _) = &found[0];
        assert!((width - (4.0 * 4.5 + 3.6)).abs() < 1e-6, "the deck is {width} m wide");
        assert!(line.iter().all(|p| (p[1] - 9.0).abs() < 1e-6), "down the middle of the five: {line:?}");
        let [west, _, east, _] = bounds(line.iter().copied());
        assert!(west < -9.9 && east > 17.9, "as long as all of them: {west} to {east}");
        assert_eq!(*pylons, Pylons::Flush);
    }

    #[test]
    fn a_cutting_that_meets_a_railway_bridge_is_part_of_the_bridge() {
        let mut spanning = track(0.0, 0.0, 30.0);
        spanning.3 = Pylons::None;
        let found = decks(vec![track(0.0, -20.0, 2.0), spanning]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].3, Pylons::None);
        assert_eq!(bounds(found[0].0.iter().copied()), [-20.0, 0.0, 30.0, 0.0]);
    }

    #[test]
    fn a_railway_ramp_follows_its_track_from_the_middle_of_a_deck_that_ends_along_the_way() {
        let first: &[P] = &[[5.0, 2.0], [40.0, 2.0], [80.0, 6.0]];
        let next: &[P] = &[[140.0, 16.0], [80.0, 6.0]];
        let crossing: &[P] = &[[20.0, -30.0], [20.0, 30.0]];
        let line = lead(&[[-20.0, 0.0], [20.0, 0.0]], true, 8.0, &[crossing, first, next]);
        assert_eq!(line, [[20.0, 0.0], [40.0, 0.0], [80.0, 4.0], [140.0, 14.0]]);
        assert!(lead(&[[-20.0, 0.0], [20.0, 0.0]], false, 8.0, &[first]).is_empty(), "nothing goes on from the west end");
    }

    #[test]
    fn track_on_a_deck_has_the_decks_height_and_is_cut_at_each_section() {
        // Two sections along x, rising a metre over 20 m.
        let bed = |building: u32, x0: f64, x1: f64| {
            let top = |x: f64| 5.0 + x / 20.0;
            let (a, b, c, d) = ([x0, top(x0), -2.0], [x1, top(x1), -2.0], [x1, top(x1), 2.0], [x0, top(x0), 2.0]);
            Bed::new((3, 4), building, true, vec![[a, b, c], [a, c, d]], vec![[0.0, 0.0], [20.0, 0.0]])
        };
        let beds = [bed(7, 0.0, 10.0), bed(8, 10.0, 20.0)];
        // Far below it but at the west end, where the deck is a little over the ground it starts from.
        let ground = |p: P| if p[0] < 0.5 { 4.9 } else { 0.0 };
        let runs = lay(&[[-30.0, 0.5], [50.0, 0.5]], false, &beds, &ground);
        let on: Vec<Option<usize>> = runs.iter().map(|run| run.on.as_ref().map(|on| on.0)).collect();
        assert_eq!(on, [None, Some(0), Some(1), None]);
        for pair in runs.windows(2) {
            let (end, start) = (pair[0].points[pair[0].points.len() - 1], pair[1].points[0]);
            assert!((end[0] - start[0]).abs() < 0.01, "a gap at {end:?}");
        }
        for run in &runs {
            let Some((_, tops)) = &run.on else {
                assert_eq!(run.points.len(), 2, "the track on the ground keeps its own points");
                continue;
            };
            for (p, top) in run.points.iter().zip(tops) {
                let eased = -0.1 * (1.0 - p[0] / EASE).max(0.0);
                assert!((top - (5.0 + p[0] / 20.0 + eased)).abs() < 0.01, "{top} m at {p:?}");
            }
        }
        // A bridge's own way, which begins at the deck's edge, eases on as well.
        let span = lay(&[[0.0, 0.5], [8.0, 0.5]], false, &beds, &ground);
        let (_, tops) = span[0].on.as_ref().expect("on the deck");
        assert!((tops[0] - 4.9).abs() < 0.01, "the span starts at {} m", tops[0]);
        // A track under it, and a train's on a road's deck, are on the ground.
        assert_eq!(lay(&[[5.0, -20.0], [5.0, 20.0]], false, &beds, &ground).iter().filter(|run| run.on.is_some()).count(), 0);
        let road = [Bed::new((0, 0), 0, false, beds[0].roof.clone(), beds[0].line.clone())];
        assert_eq!(lay(&[[-30.0, 0.5], [50.0, 0.5]], false, &road, &ground).len(), 1);
        assert_eq!(lay(&[[-30.0, 0.5], [50.0, 0.5]], true, &road, &ground).len(), 3, "a tram's may lie on one");
    }
}
