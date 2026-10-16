use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
struct Response {
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    geometry: Vec<LatLon>,
    #[serde(default)]
    members: Vec<Member>,
}

#[derive(Debug, Deserialize)]
struct Member {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    geometry: Vec<LatLon>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

/// An OSM way with its inline geometry.
#[derive(Debug)]
pub struct Way {
    pub id: u64,
    pub tags: HashMap<String, String>,
    pub geometry: Vec<LatLon>,
}

/// A multipolygon relation: its tags and its member ways, outer and inner, not yet joined into rings.
#[derive(Debug)]
pub struct Area {
    pub id: u64,
    pub tags: HashMap<String, String>,
    pub outer: Vec<Vec<LatLon>>,
    pub inner: Vec<Vec<LatLon>>,
}

/// The public Overpass servers, tried in order.
pub const DEFAULT_ENDPOINTS: [&str; 4] = [
    "https://overpass-api.de/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
    "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
];

/// What Overpass returned for the box.
#[derive(Debug, Default)]
pub struct Osm {
    pub ways: Vec<Way>,
    pub areas: Vec<Area>,
}

/// The tags that make a way or relation ground cover, as an Overpass filter list.
const COVER_FILTERS: &str = r#"[landuse];[natural~"^(wood|scrub|grassland|heath|wetland|sand|beach)$"];[leisure~"^(park|garden|pitch|playground|golf_course|recreation_ground)$"]"#;

/// Fetches the box in squares of about `chunk` metres and merges them, so a city-sized box is a
/// handful of modest queries (each cached on its own, so an interrupted run resumes) and not one
/// that the public servers time out on. A box no bigger than a chunk is one query, as before.
/// `retries` is how many extra rounds over `endpoints` each chunk gets, `pause` seconds apart.
pub fn fetch_chunked(endpoints: &[String], bbox: [f64; 4], chunk: f64, retries: usize, pause: u64) -> Result<Osm> {
    fetch_pieces(endpoints, &split(bbox, chunk), retries, pause)
}

/// Fetches each of `pieces` (`[south, west, north, east]`) and merges the answers. The pieces are
/// the cache's unit: pass the same boxes and the same entries are reused.
pub fn fetch_pieces(endpoints: &[String], pieces: &[[f64; 4]], retries: usize, pause: u64) -> Result<Osm> {
    let mut merged = Osm::default();
    let (mut ways, mut areas) = (HashSet::new(), HashSet::new());
    for (index, piece) in pieces.iter().enumerate() {
        let mut found = None;
        'rounds: for round in 0..=retries {
            if round > 0 {
                eprintln!("  retrying in {pause} s");
                crate::progress::retry(pause);
            }
            for endpoint in endpoints {
                eprintln!("querying {endpoint} for {piece:?} (part {} of {})", index + 1, pieces.len());
                match fetch(endpoint, *piece) {
                    Ok(result) => {
                        found = Some(result);
                        break 'rounds;
                    }
                    Err(error) => eprintln!("  failed: {error:#}"),
                }
            }
        }
        let Some(osm) = found else { anyhow::bail!("every Overpass endpoint failed for part {} of {}; retry later (finished parts are cached)", index + 1, pieces.len()) };
        // A way or relation that crosses a border comes back from both sides; keep the first.
        merged.ways.extend(osm.ways.into_iter().filter(|way| ways.insert(way.id)));
        merged.areas.extend(osm.areas.into_iter().filter(|area| areas.insert(area.id)));
        crate::progress::steps(index + 1, pieces.len());
    }
    Ok(merged)
}

/// Runs `query` on the first of `endpoints` that answers, trying them all `retries` more times,
/// `pause` seconds apart. The answer is cached by the query.
pub(crate) fn run(endpoints: &[String], query: &str, retries: usize, pause: u64) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    for round in 0..=retries {
        if round > 0 {
            eprintln!("  retrying in {pause} s");
            crate::progress::retry(pause);
        }
        for endpoint in endpoints {
            let text = crate::cache::text("overpass", query, || {
                Ok(client.post(endpoint).form(&[("data", query)]).send().context("Overpass request failed")?.error_for_status().context("Overpass returned an error")?.text()?)
            });
            match text {
                Ok(text) => return Ok(text),
                Err(error) => eprintln!("  {endpoint} failed: {error:#}"),
            }
        }
    }
    anyhow::bail!("every Overpass endpoint failed; retry later (finished parts are cached)")
}

/// `[south, west, north, east]` cut into a grid of equal pieces no wider than `chunk` metres.
pub fn split([south, west, north, east]: [f64; 4], chunk: f64) -> Vec<[f64; 4]> {
    const METRES_PER_DEGREE: f64 = 111_320.0;
    let height = (north - south) * METRES_PER_DEGREE;
    let width = (east - west) * METRES_PER_DEGREE * ((south + north) / 2.0).to_radians().cos();
    // A few percent over a chunk is not worth a second query: the default 2 km world stays one piece
    // (and keeps its cache entry) however the degrees round.
    let count = |span: f64| ((span / chunk / 1.05).ceil() as usize).max(1);
    let (rows, cols) = (count(height), count(width));
    let (dlat, dlon) = ((north - south) / rows as f64, (east - west) / cols as f64);
    let mut pieces = Vec::with_capacity(rows * cols);
    for row in 0..rows {
        for col in 0..cols {
            let (s, w) = (south + dlat * row as f64, west + dlon * col as f64);
            // The last piece ends exactly on the box edge, whatever the rounding.
            let n = if row + 1 == rows { north } else { s + dlat };
            let e = if col + 1 == cols { east } else { w + dlon };
            pieces.push([s, w, n, e]);
        }
    }
    pieces
}

/// The Overpass QL for a box: also the cache key, so it is the one place that spells it.
pub fn query([south, west, north, east]: [f64; 4]) -> String {
    let bbox = format!("{south},{west},{north},{east}");
    let mut query = format!("[out:json][timeout:120];(way[\"building\"]({bbox});way[\"highway\"]({bbox});");
    for filter in COVER_FILTERS.split(';') {
        query.push_str(&format!("way{filter}({bbox});relation[type=multipolygon]{filter}({bbox});"));
    }
    query.push_str(");out geom;");
    query
}

/// Fetches every building, highway and piece of ground cover in `[south, west, north, east]`.
/// Multipolygon relations are returned for cover only; courtyard buildings are not handled yet.
fn fetch(endpoint: &str, bbox: [f64; 4]) -> Result<Osm> {
    let query = query(bbox);
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    // Keyed by the query, not the endpoint: any server's answer is as good as another's.
    let text = crate::cache::text("overpass", &query, || {
        Ok(client
            .post(endpoint)
            .form(&[("data", &query)])
            .send()
            .context("Overpass request failed")?
            .error_for_status()
            .context("Overpass returned an error (rate limited? try again in a minute)")?
            .text()?)
    })?;
    let response: Response = serde_json::from_str(&text).context("Overpass returned invalid JSON")?;
    let mut osm = Osm::default();
    for element in response.elements {
        match element.kind.as_str() {
            "way" if element.geometry.len() >= 2 => osm.ways.push(Way { id: element.id, tags: element.tags, geometry: element.geometry }),
            "relation" => {
                let (mut outer, mut inner) = (Vec::new(), Vec::new());
                for member in element.members.into_iter().filter(|m| m.kind == "way" && m.geometry.len() >= 2) {
                    if member.role == "inner" { inner.push(member.geometry) } else { outer.push(member.geometry) }
                }
                osm.areas.push(Area { id: element.id, tags: element.tags, outer, inner });
            }
            _ => {}
        }
    }
    Ok(osm)
}

#[cfg(test)]
mod tests {
    use super::split;

    #[test]
    fn a_small_box_is_one_piece() {
        let bbox = [63.82, 20.16, 63.84, 20.18];
        assert_eq!(split(bbox, 5000.0), vec![bbox]);
    }

    #[test]
    fn a_city_is_cut_into_pieces_that_tile_it() {
        let bbox = [63.80, 20.14, 63.845, 20.33];
        let pieces = split(bbox, 2000.0);
        // About 5 km by 9.3 km: three rows, five columns.
        assert_eq!(pieces.len(), 15);
        let area: f64 = pieces.iter().map(|[s, w, n, e]| (n - s) * (e - w)).sum();
        assert!((area - (bbox[2] - bbox[0]) * (bbox[3] - bbox[1])).abs() < 1e-9);
        assert_eq!(pieces.iter().map(|p| p[0]).fold(f64::MAX, f64::min), bbox[0]);
        assert_eq!(pieces.iter().map(|p| p[3]).fold(f64::MIN, f64::max), bbox[3]);
    }
}
