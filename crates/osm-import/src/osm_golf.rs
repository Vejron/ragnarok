//! Golf courses from OpenStreetMap: the mown surfaces (`golf=fairway|green|tee|bunker`), the holes
//! (`golf=hole` lines from tee to green) and the pins. A query of its own, as `osm_ski`'s is, because
//! the main one is its own cache key, and because a green or a tee mapped with no landuse is not cover
//! to it.

use crate::overpass::LatLon;
use anyhow::{Context, Result};
use serde_json::Value;
use shared::golf::{Course, Hole, Surface};
use shared::world::{COVER_GOLF_BUNKER, COVER_GOLF_FAIRWAY, COVER_GOLF_GREEN, COVER_GOLF_TEE};
use std::collections::HashSet;

type P = [f64; 2];

/// The Overpass QL for a box: also the cache key.
pub fn query([south, west, north, east]: [f64; 4]) -> String {
    let bbox = format!("{south},{west},{north},{east}");
    format!(r#"[out:json][timeout:120];(way["golf"]({bbox});node["golf"="pin"]({bbox}););out geom;"#)
}

/// A hole as mapped: its line from the tee to the green, and its number and par if they are tagged.
#[derive(Clone, Debug, Default)]
pub struct HoleLine {
    pub number: Option<u8>,
    pub par: Option<u8>,
    pub line: Vec<LatLon>,
}

/// What OSM has of the courses in a box.
#[derive(Clone, Debug, Default)]
pub struct Mapped {
    /// Each mown surface: its way's id, its kind (`COVER_GOLF_*`) and its outline, closed.
    pub surfaces: Vec<(u64, u8, Vec<LatLon>)>,
    pub holes: Vec<HoleLine>,
    pub pins: Vec<LatLon>,
}

/// The cover a `golf=*` value is drawn as, if it is a surface.
pub fn surface_kind(value: &str) -> Option<u8> {
    match value {
        "fairway" | "driving_range" => Some(COVER_GOLF_FAIRWAY),
        "green" => Some(COVER_GOLF_GREEN),
        "tee" => Some(COVER_GOLF_TEE),
        "bunker" => Some(COVER_GOLF_BUNKER),
        _ => None,
    }
}

fn line(value: &Value) -> Vec<LatLon> {
    value.as_array().map(|a| a.iter().filter_map(|p| Some(LatLon { lat: p["lat"].as_f64()?, lon: p["lon"].as_f64()? })).collect()).unwrap_or_default()
}

/// A tag's leading number, as in `ref=7` or `name=Hål 1`.
fn number(text: Option<&str>) -> Option<u8> {
    let digits: String = text?.chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|&n| n > 0)
}

/// Adds one answer's elements to `mapped`, each way once.
fn read(response: &Value, mapped: &mut Mapped, seen: &mut HashSet<(bool, u64)>) {
    for element in response["elements"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let id = element["id"].as_u64().unwrap_or(0);
        let tags = &element["tags"];
        let tag = |key: &str| tags[key].as_str();
        match element["type"].as_str() {
            Some("node") if tag("golf") == Some("pin") && seen.insert((false, id)) => {
                if let (Some(lat), Some(lon)) = (element["lat"].as_f64(), element["lon"].as_f64()) {
                    mapped.pins.push(LatLon { lat, lon });
                }
            }
            Some("way") if seen.insert((true, id)) => {
                let points = line(&element["geometry"]);
                match tag("golf") {
                    Some("hole") if points.len() >= 2 => mapped.holes.push(HoleLine { number: number(tag("ref")), par: number(tag("par")), line: points }),
                    Some(value) => {
                        if let Some(kind) = surface_kind(value)
                            && points.len() >= 4
                            && points.first().map(|p| (p.lat, p.lon)) == points.last().map(|p| (p.lat, p.lon))
                        {
                            mapped.surfaces.push((id, kind, points));
                        }
                    }
                    None => {}
                }
            }
            _ => {}
        }
    }
}

/// Fetches the golf of each of `pieces` (`[south, west, north, east]`) and merges the answers.
pub fn fetch_pieces(endpoints: &[String], pieces: &[[f64; 4]], retries: usize, pause: u64) -> Result<Mapped> {
    let mut seen = HashSet::new();
    let mut mapped = Mapped::default();
    for (index, piece) in pieces.iter().enumerate() {
        let text = crate::overpass::run(endpoints, &query(*piece), retries, pause)?;
        let response: Value = serde_json::from_str(&text).context("Overpass returned invalid JSON")?;
        read(&response, &mut mapped, &mut seen);
        crate::progress::steps(index + 1, pieces.len());
    }
    Ok(mapped)
}

fn centroid(ring: &[P]) -> P {
    let (mut area, mut cx, mut cz) = (0.0, 0.0, 0.0);
    for i in 0..ring.len() {
        let ([x0, z0], [x1, z1]) = (ring[i], ring[(i + 1) % ring.len()]);
        let cross = x0 * z1 - x1 * z0;
        area += cross;
        cx += (x0 + x1) * cross;
        cz += (z0 + z1) * cross;
    }
    if area.abs() < 1e-9 {
        let n = ring.len().max(1) as f64;
        return ring.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
    }
    [cx / (3.0 * area), cz / (3.0 * area)]
}

fn distance([ax, az]: P, [bx, bz]: P) -> f64 {
    (ax - bx).hypot(az - bz)
}

/// A green this far (metres) from the end of a hole's line is still its green.
const GREEN_REACH: f64 = 40.0;

/// Lays out the course in world metres, `project` taking latitude and longitude there. Each hole is
/// played from where its line starts to a cup on the green it ends on: on the pin mapped on that
/// green, or in the green's middle. Holes without a number take the lowest free ones.
pub fn lay_out(mapped: &Mapped, project: impl Fn(f64, f64) -> P) -> Course {
    let at = |p: &LatLon| project(p.lat, p.lon);
    let surfaces: Vec<(u8, Vec<P>)> = mapped.surfaces.iter().map(|(_, kind, ring)| (*kind, ring[..ring.len() - 1].iter().map(at).collect())).collect();
    let greens: Vec<&Vec<P>> = surfaces.iter().filter(|(kind, _)| *kind == COVER_GOLF_GREEN).map(|(_, ring)| ring).collect();
    let pins: Vec<P> = mapped.pins.iter().map(at).collect();
    let mut taken: HashSet<u8> = HashSet::new();
    let mut holes = Vec::new();
    let mut lines: Vec<&HoleLine> = mapped.holes.iter().collect();
    // Numbered holes first, so an unnumbered one never takes a number that is mapped.
    lines.sort_by_key(|hole| hole.number.is_none());
    let mut unnumbered = Vec::new();
    for hole in lines {
        let points: Vec<P> = hole.line.iter().map(at).collect();
        let (tee, end) = (points[0], points[points.len() - 1]);
        let green = greens
            .iter()
            .find(|ring| crate::assemble::point_in_polygon(end, ring))
            .or_else(|| greens.iter().map(|ring| (ring, distance(centroid(ring), end))).filter(|(_, d)| *d < GREEN_REACH).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(ring, _)| ring));
        let cup = match green {
            Some(ring) => pins.iter().copied().find(|pin| crate::assemble::point_in_polygon(*pin, ring)).unwrap_or_else(|| centroid(ring)),
            None => end,
        };
        let number = hole.number.filter(|n| taken.insert(*n));
        let hole = Hole { number: number.unwrap_or(0), par: hole.par, tee: [tee[0] as f32, tee[1] as f32], cup: [cup[0] as f32, cup[1] as f32] };
        if number.is_some() { holes.push(hole) } else { unnumbered.push(hole) }
    }
    let mut free = (1..=u8::MAX).filter(|n| !taken.contains(n));
    for mut hole in unnumbered {
        hole.number = free.next().unwrap_or(u8::MAX);
        holes.push(hole);
    }
    holes.sort_by_key(|hole| hole.number);
    let surfaces = surfaces.into_iter().map(|(kind, ring)| Surface { kind, ring: ring.iter().map(|&[x, z]| [x as f32, z as f32]).collect() }).collect();
    Course { holes, surfaces }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed(points: &[[f64; 2]]) -> Vec<serde_json::Value> {
        points.iter().chain(points.first()).map(|[lat, lon]| serde_json::json!({ "lat": lat, "lon": lon })).collect()
    }

    #[test]
    fn holes_run_from_their_tee_to_a_cup_on_their_green() {
        let response = serde_json::json!({ "elements": [
            { "type": "way", "id": 1, "tags": { "golf": "green" }, "geometry": closed(&[[10.0, 10.0], [10.0, 20.0], [20.0, 20.0], [20.0, 10.0]]) },
            { "type": "way", "id": 2, "tags": { "golf": "green", "landuse": "grass" }, "geometry": closed(&[[110.0, 10.0], [110.0, 20.0], [120.0, 20.0], [120.0, 10.0]]) },
            { "type": "way", "id": 3, "tags": { "golf": "tee", "name": "Hål 1" }, "geometry": closed(&[[0.0, 0.0], [0.0, 2.0], [2.0, 2.0], [2.0, 0.0]]) },
            { "type": "way", "id": 4, "tags": { "golf": "hole", "ref": "2", "par": "4" }, "geometry": [{ "lat": 1.0, "lon": 1.0 }, { "lat": 15.0, "lon": 15.0 }] },
            // Unnumbered, and ending short of its green: it is still that green's.
            { "type": "way", "id": 5, "tags": { "golf": "hole" }, "geometry": [{ "lat": 60.0, "lon": 15.0 }, { "lat": 100.0, "lon": 15.0 }] },
            { "type": "node", "id": 6, "lat": 12.0, "lon": 18.0, "tags": { "golf": "pin" } },
            { "type": "way", "id": 7, "tags": { "golf": "cartpath" }, "geometry": [{ "lat": 0.0, "lon": 0.0 }, { "lat": 5.0, "lon": 5.0 }] },
        ] });
        let mut mapped = Mapped::default();
        let mut seen = HashSet::new();
        read(&response, &mut mapped, &mut seen);
        read(&response, &mut mapped, &mut seen);
        assert_eq!(mapped.surfaces.len(), 3, "each way once, and the cart path is no surface");
        assert_eq!(mapped.holes.len(), 2);
        // Latitude and longitude taken as metres, as z and x.
        let course = lay_out(&mapped, |lat, lon| [lon, lat]);
        assert_eq!(course.holes.len(), 2);
        let (first, second) = (&course.holes[0], &course.holes[1]);
        assert_eq!((first.number, first.par), (1, None), "the unnumbered hole takes the free 1");
        assert_eq!(first.tee, [15.0, 60.0]);
        assert_eq!(first.cup, [15.0, 115.0], "in the middle of a green with no pin");
        assert_eq!((second.number, second.par), (2, Some(4)));
        assert_eq!(second.tee, [1.0, 1.0]);
        assert_eq!(second.cup, [18.0, 12.0], "on its green's pin");
        assert_eq!(number(Some("Hål 12")), Some(12));
    }
}
