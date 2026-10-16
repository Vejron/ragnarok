//! Water from OpenStreetMap, the alternative to Lantmäteriet's Hydrografi (`hydro.rs`), which stops
//! at the coast: lakes and river polygons, stream centre lines and the sea.
//!
//! OSM has no sea polygon. The sea is the side of every `natural=coastline` way that is on its
//! right (land is on the left), so it is built here: the coastline chains are cut at the edge of
//! the world and closed along that edge, clockwise, into polygons (`sea`).

use crate::overpass::LatLon;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashSet;

type P = [f64; 2];

/// A polygon: the outer ring first, then holes.
pub type Polygon = Vec<Vec<LatLon>>;

#[derive(Debug, Default)]
pub struct Found {
    pub lakes: Vec<Polygon>,
    pub rivers: Vec<Polygon>,
    pub streams: Vec<Vec<LatLon>>,
    /// Coastline ways, joined end to end, land on the left.
    pub coast: Vec<Vec<LatLon>>,
}

/// The Overpass QL for a box: also the cache key.
pub fn query([south, west, north, east]: [f64; 4]) -> String {
    let bbox = format!("{south},{west},{north},{east}");
    let mut query = String::from("[out:json][timeout:120];(");
    for filter in [r#"["natural"="water"]"#, r#"["waterway"="riverbank"]"#, r#"["landuse"="reservoir"]"#] {
        query.push_str(&format!("way{filter}({bbox});relation{filter}({bbox});"));
    }
    query.push_str(&format!(r#"way["waterway"~"^(river|stream|canal)$"]({bbox});way["natural"="coastline"]({bbox});"#));
    query.push_str(");out geom;");
    query
}

/// Fetches the water of each of `pieces` (`[south, west, north, east]`) and merges the answers.
pub fn fetch_pieces(endpoints: &[String], pieces: &[[f64; 4]], retries: usize, pause: u64) -> Result<Found> {
    let mut seen = HashSet::new();
    let mut found = Found::default();
    let mut coast = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        eprintln!("water: OSM part {} of {}", index + 1, pieces.len());
        let text = crate::overpass::run(endpoints, &query(*piece), retries, pause)?;
        let response: Value = serde_json::from_str(&text).context("Overpass returned invalid JSON")?;
        for element in response["elements"].as_array().map(Vec::as_slice).unwrap_or_default() {
            // A way or relation that crosses a border comes back from both sides.
            let key = (element["type"].as_str().unwrap_or("").to_string(), element["id"].as_u64().unwrap_or(0));
            if !seen.insert(key) {
                continue;
            }
            read(element, &mut found, &mut coast);
        }
        crate::progress::steps(index + 1, pieces.len());
    }
    let (closed, open) = join(coast, true);
    found.coast = open.into_iter().chain(closed).collect();
    Ok(found)
}

fn points(value: &Value) -> Vec<LatLon> {
    value.as_array().map(|a| a.iter().filter_map(|p| Some(LatLon { lat: p["lat"].as_f64()?, lon: p["lon"].as_f64()? })).collect()).unwrap_or_default()
}

fn read(element: &Value, found: &mut Found, coast: &mut Vec<Vec<LatLon>>) {
    let tag = |key: &str| element["tags"][key].as_str().unwrap_or("");
    // Rivers are drawn from their banks; a wide `water=river` polygon is one too.
    let river = tag("waterway") == "riverbank" || matches!(tag("water"), "river" | "canal" | "stream");
    match element["type"].as_str() {
        Some("way") => {
            let geometry = points(&element["geometry"]);
            if geometry.len() < 2 {
                return;
            }
            if tag("natural") == "coastline" {
                coast.push(geometry);
            } else if matches!(tag("waterway"), "river" | "stream" | "canal") {
                found.streams.push(geometry);
            } else if is_closed(&geometry) {
                (if river { &mut found.rivers } else { &mut found.lakes }).push(vec![geometry]);
            }
        }
        Some("relation") => {
            let members = |role: &str| -> Vec<Vec<LatLon>> {
                element["members"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|m| m["type"] == "way" && (m["role"] == role || (role == "outer" && m["role"] == "")))
                    .map(|m| points(&m["geometry"]))
                    .filter(|g| g.len() >= 2)
                    .collect()
            };
            let (outer, inner) = (join(members("outer"), false).0, join(members("inner"), false).0);
            for ring in outer {
                let holes = inner.iter().filter(|hole| contains(&ring, hole[0])).cloned();
                let polygon: Polygon = std::iter::once(ring.clone()).chain(holes).collect();
                (if river { &mut found.rivers } else { &mut found.lakes }).push(polygon);
            }
        }
        _ => {}
    }
}

fn same(a: LatLon, b: LatLon) -> bool {
    a.lat == b.lat && a.lon == b.lon
}

fn is_closed(line: &[LatLon]) -> bool {
    line.len() >= 4 && same(line[0], line[line.len() - 1])
}

fn contains(ring: &[LatLon], p: LatLon) -> bool {
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        if (current.lat > p.lat) != (previous.lat > p.lat)
            && p.lon < previous.lon + (p.lat - previous.lat) * (current.lon - previous.lon) / (current.lat - previous.lat)
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// Joins ways that share an end node into longer ones. Returns the closed rings and the chains
/// that stayed open. `directed` joins only start to end (coastline ways all run the same way);
/// otherwise a way may be reversed to fit (multipolygon members have no direction).
pub fn join(ways: Vec<Vec<LatLon>>, directed: bool) -> (Vec<Vec<LatLon>>, Vec<Vec<LatLon>>) {
    let mut left = ways;
    let (mut closed, mut open) = (Vec::new(), Vec::new());
    while let Some(mut current) = left.pop() {
        loop {
            if is_closed(&current) {
                closed.push(current);
                break;
            }
            let (start, end) = (current[0], current[current.len() - 1]);
            if let Some(i) = left.iter().position(|c| same(c[0], end)) {
                let next = left.swap_remove(i);
                current.extend(next.into_iter().skip(1));
            } else if let Some(i) = left.iter().position(|c| same(c[c.len() - 1], start)) {
                let mut next = left.swap_remove(i);
                next.extend(current.into_iter().skip(1));
                current = next;
            } else if !directed && let Some(i) = left.iter().position(|c| same(c[c.len() - 1], end)) {
                let mut next = left.swap_remove(i);
                next.reverse();
                current.extend(next.into_iter().skip(1));
            } else if !directed && let Some(i) = left.iter().position(|c| same(c[0], start)) {
                let mut next = left.swap_remove(i);
                next.reverse();
                next.extend(current.into_iter().skip(1));
                current = next;
            } else {
                open.push(current);
                break;
            }
        }
    }
    (closed, open)
}

/// The sea inside the square `[-reach, reach]²`, as polygons (outer ring first, then islands).
/// `coast` is the coastline as `[east, north]` metres, each chain with the land on its left.
/// Closed chains that are islands (counter-clockwise) become holes; clockwise ones (water
/// inside, a lake mapped as coastline) are returned as polygons of their own.
pub fn sea(coast: &[Vec<P>], reach: f64) -> Vec<Vec<Vec<P>>> {
    let r = reach;
    let perimeter = 8.0 * r;
    // Position along the edge, clockwise from the top-left corner.
    let at = |p: P| -> f64 {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        if near(p[1], r) {
            p[0] + r
        } else if near(p[0], r) {
            2.0 * r + (r - p[1])
        } else if near(p[1], -r) {
            4.0 * r + (r - p[0])
        } else {
            6.0 * r + (p[1] + r)
        }
    };
    let on_edge = |p: P| (p[0].abs() - r).abs() < 1e-6 || (p[1].abs() - r).abs() < 1e-6;
    let inside = |p: P| p[0].abs() <= r && p[1].abs() <= r;

    let mut runs: Vec<Vec<P>> = Vec::new();
    let mut islands: Vec<Vec<P>> = Vec::new();
    let mut lakes: Vec<Vec<P>> = Vec::new();
    for chain in coast {
        let mut chain = chain.clone();
        let closed = chain.len() >= 4 && chain[0] == chain[chain.len() - 1];
        if closed {
            if chain.iter().all(|&p| inside(p)) {
                let area: f64 = (0..chain.len() - 1).map(|i| chain[i][0] * chain[i + 1][1] - chain[i + 1][0] * chain[i][1]).sum::<f64>() / 2.0;
                chain.pop();
                if area > 0.0 { islands.push(chain) } else { lakes.push(chain) }
                continue;
            }
            // Start the ring outside the square so that no run wraps around its start.
            if let Some(k) = chain.iter().position(|&p| !inside(p)) {
                chain.pop();
                chain.rotate_left(k);
                chain.push(chain[0]);
            }
        }
        let mut run: Vec<P> = Vec::new();
        for pair in chain.windows(2) {
            let Some((t0, t1)) = clip(pair[0], pair[1], r) else {
                continue;
            };
            let at_t = |t: f64| -> P {
                let p = [pair[0][0] + t * (pair[1][0] - pair[0][0]), pair[0][1] + t * (pair[1][1] - pair[0][1])];
                // Exactly on the edge, so that `at` can tell which one.
                [if (p[0].abs() - r).abs() < 1e-6 { r.copysign(p[0]) } else { p[0] }, if (p[1].abs() - r).abs() < 1e-6 { r.copysign(p[1]) } else { p[1] }]
            };
            if run.is_empty() {
                run.push(at_t(t0));
            }
            run.push(at_t(t1));
            if t1 < 1.0 {
                if run.len() >= 2 && on_edge(run[0]) {
                    runs.push(std::mem::take(&mut run));
                }
                run.clear();
            }
        }
        // A chain that ends inside the square (cut data) is not a coast to close: dropped.
    }

    let mut polygons: Vec<Vec<P>> = Vec::new();
    let mut used = vec![false; runs.len()];
    let corners = [0.0, 2.0 * r, 4.0 * r, 6.0 * r];
    let corner = |t: f64| -> P {
        match (t / (2.0 * r)).round() as i32 {
            0 => [-r, r],
            1 => [r, r],
            2 => [r, -r],
            _ => [-r, -r],
        }
    };
    for first in 0..runs.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut poly = runs[first].clone();
        let mut from = at(*poly.last().unwrap());
        let mut guard = 0;
        loop {
            guard += 1;
            // The next run to start clockwise from where this one left the square.
            let gap = |t: f64| (t - from).rem_euclid(perimeter);
            let next = (0..runs.len()).filter(|&i| i == first || !used[i]).min_by(|&a, &b| gap(at(runs[a][0])).total_cmp(&gap(at(runs[b][0]))));
            let Some(next) = next else { break };
            let to = at(runs[next][0]);
            let mut between: Vec<f64> = corners.iter().copied().filter(|&c| gap(c) > 1e-6 && gap(c) < gap(to) - 1e-6).collect();
            between.sort_by(|a, b| gap(*a).total_cmp(&gap(*b)));
            poly.extend(between.into_iter().map(corner));
            if next == first || guard > runs.len() + 1 {
                break;
            }
            used[next] = true;
            poly.extend(runs[next].iter().copied());
            from = at(*runs[next].last().unwrap());
        }
        if poly.len() >= 3 {
            polygons.push(poly);
        }
    }

    let mut out: Vec<Vec<Vec<P>>> = polygons.into_iter().map(|p| vec![p]).collect();
    for island in islands {
        if let Some(sea) = out.iter_mut().find(|rings| point_in(&rings[0], island[0])) {
            sea.push(island);
        }
    }
    out.extend(lakes.into_iter().map(|l| vec![l]));
    out
}

fn point_in(ring: &[P], p: P) -> bool {
    let mut inside = false;
    let mut previous = ring[ring.len() - 1];
    for &current in ring {
        if (current[1] > p[1]) != (previous[1] > p[1]) && p[0] < previous[0] + (p[1] - previous[1]) * (current[0] - previous[0]) / (current[1] - previous[1]) {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// The part of the segment `a`→`b` inside `[-r, r]²` as `(t0, t1)` along it (Liang–Barsky).
fn clip(a: P, b: P, r: f64) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for axis in 0..2 {
        let d = b[axis] - a[axis];
        for (edge, sign) in [(-r, -1.0), (r, 1.0)] {
            // sign * (a + t d) <= r
            let (p, q) = (sign * d, r - sign * a[axis]);
            let _ = edge;
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
    }
    (t0 < t1).then_some((t0, t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(ring: &[P]) -> f64 {
        let n = ring.len();
        (0..n).map(|i| ring[i][0] * ring[(i + 1) % n][1] - ring[(i + 1) % n][0] * ring[i][1]).sum::<f64>().abs() / 2.0
    }

    #[test]
    fn the_sea_is_on_the_right_of_the_coast() {
        // A coast running north along x = 0 with land to the west (left): the sea is the east half.
        let polygons = sea(&[vec![[0.0, -500.0], [0.0, 500.0]]], 100.0);
        assert_eq!(polygons.len(), 1);
        assert!((area(&polygons[0][0]) - 100.0 * 200.0).abs() < 1.0, "{:?}", polygons[0][0]);
        assert!(point_in(&polygons[0][0], [50.0, 0.0]));
        assert!(!point_in(&polygons[0][0], [-50.0, 0.0]));
    }

    #[test]
    fn a_coast_that_turns_a_corner_closes_the_right_quarter() {
        // Heading east along y = 0 then south along x = 0, land is on the left: the sea is the
        // south-west quarter of the square.
        let polygons = sea(&[vec![[-500.0, 0.0], [0.0, 0.0], [0.0, -500.0]]], 100.0);
        assert_eq!(polygons.len(), 1);
        assert!((area(&polygons[0][0]) - 100.0 * 100.0).abs() < 1.0, "{}", area(&polygons[0][0]));
    }

    #[test]
    fn an_island_in_the_sea_is_a_hole() {
        // CCW (land inside); the coast east with land to the north makes the sea the south half.
        let island = vec![[-10.0, -60.0], [10.0, -60.0], [10.0, -40.0], [-10.0, -40.0], [-10.0, -60.0]];
        let polygons = sea(&[vec![[-500.0, 0.0], [500.0, 0.0]], island], 100.0);
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].len(), 2);
    }

    #[test]
    fn ways_join_end_to_end() {
        let p = |lat: f64| LatLon { lat, lon: 0.0 };
        let (closed, open) = join(vec![vec![p(0.0), p(1.0)], vec![p(1.0), p(2.0)]], true);
        assert!(closed.is_empty());
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].len(), 3);
    }
}
