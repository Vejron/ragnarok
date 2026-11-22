//! Footways and cycleways that run along a bigger road are left out. OpenStreetMap maps a street's
//! pavements and its cycle track as ways of their own beside it, with a short way across the road at
//! every crossing and another wherever a pavement meets a side street. In the game they add nothing
//! to the street they belong to, and what the importer makes of them is a ribbon of asphalt a few
//! metres from the road, and a litter of short stumps.
//!
//! A path goes when it is a pavement or a crossing by its tags, or when most of its length runs
//! beside a street and along it (`BESIDE`, `ALONG`, `SHARE`). Then the short paths that led to
//! what went, and now end in the air, go too (`STUMP`), and so does a short path from one road to
//! another (`LINK`). A path through a park, or one that follows a
//! road at a distance, stays: the rule is rough, and errs towards dropping a path too many.

use crate::osm::{Osm, Way};
use shared::world::Frame;
use std::collections::{HashMap, HashSet};

type P = [f64; 2];

/// A path this near a street's edge, metres, is beside it: across a verge or a line of trees, not
/// across a ditch and a field.
const BESIDE: f64 = 7.0;
/// A path within this cosine of a street's direction runs along it (about 30 degrees).
const ALONG: f64 = 0.85;
/// A path goes when this share of its length is beside a street and along it.
const SHARE: f64 = 0.7;
/// How far apart a path is measured against the streets, metres.
const STEP: f64 = 2.0;
/// A path no longer than this, metres, that led to a path left out and now ends at nothing, is a
/// stump and goes too.
const STUMP: f64 = 40.0;
/// A path no longer than this, metres, with a road at both its ends is a link across a corner or a
/// traffic island, and goes: the road is there to walk on.
const LINK: f64 = 25.0;
/// Side of a cell of the grid the streets' pieces are found in, metres.
const CELL: f64 = 32.0;

/// Whether `way` is a footway, cycleway or path on the ground: what may be left out. Steps, and a
/// path in a tunnel (an underpass, which crosses its road), are kept.
fn is_path(way: &Way) -> bool {
    let tag = |key: &str| way.tags.get(key).map(String::as_str);
    matches!(tag("highway"), Some("footway" | "cycleway" | "path")) && tag("tunnel").is_none_or(|value| value == "no")
}

/// Half the width of the street `way` is, metres, or `None` when it is not one: a road with houses
/// or traffic along it, not a service road or a track.
fn street_half(way: &Way) -> Option<f64> {
    let highway = way.tags.get("highway")?.as_str();
    matches!(highway, "motorway" | "trunk" | "primary" | "secondary" | "tertiary" | "residential" | "unclassified" | "living_street" | "motorway_link" | "trunk_link" | "primary_link" | "secondary_link" | "tertiary_link")
        .then(|| crate::lanes::style(&way.tags).map(|(width, ..)| width / 2.0))
        .flatten()
}

/// A pavement or a crossing by its tags: part of its street whatever its line.
fn of_the_street(way: &Way) -> bool {
    ["footway", "cycleway", "path"].iter().any(|key| matches!(way.tags.get(*key).map(String::as_str), Some("sidewalk" | "crossing")))
}

fn length(line: &[P]) -> f64 {
    line.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum()
}

/// `osm` without the paths that belong to a street (see the module's notes), and how many went.
pub fn prune(mut osm: Osm, frame: Frame) -> (Osm, usize) {
    let line = |way: &Way| -> Vec<P> { way.geometry.iter().map(|g| frame.project(g.lat, g.lon)).collect() };
    // The streets' pieces, each `(a, b, half width)`, by the grid cells its box touches.
    let cell = |v: f64| (v / CELL).floor() as i64;
    let mut pieces: Vec<(P, P, f64)> = Vec::new();
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for way in &osm.ways {
        let Some(half) = street_half(way) else { continue };
        for pair in line(way).windows(2) {
            let reach = half + BESIDE;
            for i in cell(pair[0][0].min(pair[1][0]) - reach)..=cell(pair[0][0].max(pair[1][0]) + reach) {
                for j in cell(pair[0][1].min(pair[1][1]) - reach)..=cell(pair[0][1].max(pair[1][1]) + reach) {
                    grid.entry((i, j)).or_default().push(pieces.len());
                }
            }
            pieces.push((pair[0], pair[1], half));
        }
    }
    // Whether a path at `p`, heading `d` (a unit vector), runs beside a street and along it there.
    let beside = |p: P, d: P| {
        grid.get(&(cell(p[0]), cell(p[1]))).is_some_and(|near| {
            near.iter().any(|&k| {
                let (a, b, half) = pieces[k];
                let (ex, ez) = (b[0] - a[0], b[1] - a[1]);
                let span = ex.hypot(ez);
                span > 0.0 && crate::assemble::distance_to_segment(p, a, b) < half + BESIDE && ((d[0] * ex + d[1] * ez) / span).abs() > ALONG
            })
        })
    };
    let lines: Vec<Option<Vec<P>>> = osm.ways.iter().map(|way| is_path(way).then(|| line(way)).filter(|line| line.len() >= 2)).collect();
    let mut gone: Vec<bool> = osm
        .ways
        .iter()
        .zip(&lines)
        .map(|(way, line)| {
            let Some(line) = line else { return false };
            if of_the_street(way) {
                return true;
            }
            let (mut along, mut total) = (0.0, 0.0);
            for pair in line.windows(2) {
                let span = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
                if span <= 0.0 {
                    continue;
                }
                let d = [(pair[1][0] - pair[0][0]) / span, (pair[1][1] - pair[0][1]) / span];
                let steps = (span / STEP).ceil().max(1.0) as usize;
                let hits = (0..steps).filter(|&k| {
                    let t = (k as f64 + 0.5) / steps as f64;
                    beside([pair[0][0] + (pair[1][0] - pair[0][0]) * t, pair[0][1] + (pair[1][1] - pair[0][1]) * t], d)
                });
                along += span * hits.count() as f64 / steps as f64;
                total += span;
            }
            total > 0.0 && along >= SHARE * total
        })
        .collect();
    // Links: a short path from one driven road to another, or back to the same.
    let key = |g: &crate::osm::LatLon| (g.lat.to_bits(), g.lon.to_bits());
    let driven: HashSet<(u64, u64)> = osm.ways.iter().filter(|way| crate::lanes::style(&way.tags).is_some_and(|(_, kind, _)| matches!(kind, shared::world::ROAD_MAJOR | shared::world::ROAD_MINOR))).flat_map(|way| way.geometry.iter().map(key)).collect();
    for (k, line) in lines.iter().enumerate() {
        let geometry = &osm.ways[k].geometry;
        if line.as_ref().is_some_and(|line| length(line) <= LINK) && driven.contains(&key(&geometry[0])) && driven.contains(&key(&geometry[geometry.len() - 1])) {
            gone[k] = true;
        }
    }
    // Stumps: a short path with an end at a path that went, where nothing that stays meets it. The
    // points of OpenStreetMap that two ways share have the same coordinates to the last digit.
    let roads_or_paths: Vec<usize> = (0..osm.ways.len()).filter(|&k| crate::lanes::style(&osm.ways[k].tags).is_some()).collect();
    loop {
        // How many ways that stay have each point, and the points of the ways that went.
        let mut kept: HashMap<(u64, u64), u32> = HashMap::new();
        let mut left: HashSet<(u64, u64)> = HashSet::new();
        for &k in &roads_or_paths {
            let points: HashSet<(u64, u64)> = osm.ways[k].geometry.iter().map(key).collect();
            if gone[k] {
                left.extend(points);
            } else {
                for point in points {
                    *kept.entry(point).or_default() += 1;
                }
            }
        }
        let mut more = false;
        for k in 0..osm.ways.len() {
            if gone[k] || !lines[k].as_ref().is_some_and(|line| length(line) <= STUMP) {
                continue;
            }
            let geometry = &osm.ways[k].geometry;
            // An end meets something that stays where another way has that point too.
            let dangles = |end: &crate::osm::LatLon| left.contains(&key(end)) && kept.get(&key(end)).is_none_or(|&ways| ways < 2);
            if dangles(&geometry[0]) || dangles(&geometry[geometry.len() - 1]) {
                gone[k] = true;
                more = true;
            }
        }
        if !more {
            break;
        }
    }
    let count = gone.iter().filter(|&&gone| gone).count();
    let mut index = 0;
    osm.ways.retain(|_| {
        index += 1;
        !gone[index - 1]
    });
    (osm, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osm::LatLon;

    /// A way along `points` (metres east and south of the frame's origin) with `tags`.
    fn way(id: u64, tags: &[(&str, &str)], points: &[P]) -> Way {
        let geometry = points.iter().map(|p| LatLon { lat: -p[1] / shared::world::METRES_PER_DEG_LAT, lon: p[0] / shared::world::metres_per_deg_lon() }).collect();
        Way { id, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), geometry }
    }

    fn pruned(ways: Vec<Way>) -> Vec<u64> {
        let frame = Frame::Equirect { origin: (0.0, 0.0) };
        prune(Osm { ways, areas: Vec::new(), pois: Vec::new() }, frame).0.ways.iter().map(|way| way.id).collect()
    }

    #[test]
    fn a_cycleway_along_a_street_goes_and_one_through_the_park_stays() {
        let kept = pruned(vec![
            way(1, &[("highway", "secondary")], &[[0.0, 0.0], [300.0, 0.0]]),
            // Six metres from the street's edge all the way, with a kink.
            way(2, &[("highway", "cycleway")], &[[0.0, 11.0], [150.0, 9.0], [300.0, 11.0]]),
            // Along the street, but across a field from it.
            way(3, &[("highway", "cycleway")], &[[0.0, 40.0], [300.0, 40.0]]),
            // Near the street at one end only, then off through the park.
            way(4, &[("highway", "footway")], &[[100.0, 8.0], [130.0, 9.0], [200.0, 120.0], [260.0, 200.0]]),
            // Straight across the street.
            way(5, &[("highway", "path")], &[[50.0, -60.0], [50.0, 60.0]]),
        ]);
        assert_eq!(kept, [1, 3, 4, 5]);
    }

    #[test]
    fn a_pavement_and_a_crossing_go_by_their_tags_and_the_stumps_that_led_to_them_too() {
        let kept = pruned(vec![
            way(1, &[("highway", "residential")], &[[0.0, 0.0], [15.0, 0.0], [200.0, 0.0]]),
            // A pavement drawn well off the street's line, and the crossing over the street to it.
            way(2, &[("highway", "footway"), ("footway", "sidewalk")], &[[0.0, 30.0], [100.0, 30.0], [200.0, 30.0]]),
            way(3, &[("highway", "footway"), ("footway", "crossing")], &[[100.0, 30.0], [100.0, -30.0]]),
            // A short link from a house's door to the pavement: a stump once the pavement is gone.
            way(4, &[("highway", "footway")], &[[100.0, 30.0], [100.0, 45.0]]),
            // A long path from the pavement off into the woods stays, though its end is in the air.
            way(5, &[("highway", "path")], &[[200.0, 30.0], [200.0, 300.0]]),
            // A short path between the street and a path that stays is no stump.
            way(6, &[("highway", "footway")], &[[200.0, 0.0], [230.0, 20.0], [200.0, 300.0]]),
            // A short path across the corner between two streets is a link, and goes.
            way(9, &[("highway", "residential")], &[[0.0, 0.0], [0.0, -15.0], [0.0, -200.0]]),
            way(10, &[("highway", "footway")], &[[0.0, -15.0], [8.0, -8.0], [15.0, 0.0]]),
            // Steps and a path in a tunnel beside the street are kept.
            way(7, &[("highway", "steps")], &[[0.0, 6.0], [20.0, 6.0]]),
            way(8, &[("highway", "footway"), ("tunnel", "yes")], &[[30.0, 6.0], [60.0, 6.0]]),
        ]);
        assert_eq!(kept, [1, 5, 6, 9, 7, 8]);
    }
}
