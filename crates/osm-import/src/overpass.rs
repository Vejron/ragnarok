//! OpenStreetMap from the public Overpass servers: the fallback for a box no local extract (`pbf`)
//! covers. One query per piece asks for everything any pass wants (`osm::wants_*`).

use crate::osm::{Data, Member, Node, Relation, Seen};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

pub use crate::osm::{Area, LatLon, Osm, Way};

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
    lat: Option<f64>,
    lon: Option<f64>,
    #[serde(default)]
    geometry: Vec<LatLon>,
    #[serde(default)]
    members: Vec<ElementMember>,
}

#[derive(Debug, Deserialize)]
struct ElementMember {
    #[serde(rename = "type")]
    kind: String,
    #[serde(rename = "ref", default)]
    id: u64,
    #[serde(default)]
    role: String,
    #[serde(default)]
    geometry: Vec<LatLon>,
}

/// The public Overpass servers, tried in order.
pub const DEFAULT_ENDPOINTS: [&str; 6] = [
    "https://overpass-api.de/api/interpreter",
    "https://lz4.overpass-api.de/api/interpreter",
    "https://overpass.openstreetmap.fr/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
    "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
];

/// The tags that make a way or relation ground cover, as an Overpass filter list (`osm::is_cover`).
const COVER_FILTERS: &str = r#"[landuse];[natural~"^(wood|scrub|grassland|heath|wetland|sand|beach)$"];[leisure~"^(park|garden|pitch|playground|golf_course|recreation_ground)$"]"#;

/// Water bodies, as ways and relations (`osm::is_water_body`).
const WATER_FILTERS: [&str; 3] = [r#"["natural"="water"]"#, r#"["waterway"="riverbank"]"#, r#"["landuse"="reservoir"]"#];

/// The Overpass QL for a box, asking for everything any pass wants (`osm::wants_way`,
/// `wants_relation`, `wants_node`): also the cache key, so it is the one place that spells it.
pub fn query([south, west, north, east]: [f64; 4]) -> String {
    let bbox = format!("{south},{west},{north},{east}");
    let mut query = format!("[out:json][timeout:180];(way[\"building\"]({bbox});way[\"highway\"]({bbox});way[\"railway\"]({bbox});");
    for filter in COVER_FILTERS.split(';') {
        query.push_str(&format!("way{filter}({bbox});relation[type=multipolygon]{filter}({bbox});"));
    }
    for filter in WATER_FILTERS {
        query.push_str(&format!("way{filter}({bbox});relation{filter}({bbox});"));
    }
    let lifts = crate::osm::LIFTS.join("|");
    query.push_str(&format!(
        r#"way["waterway"~"^(river|stream|canal)$"]({bbox});way["natural"="coastline"]({bbox});way["golf"]({bbox});node["golf"="pin"]({bbox});way["aerialway"~"^({lifts})$"]({bbox});way["piste:type"~"nordic"]({bbox});relation["piste:type"~"nordic"]({bbox});relation["route"="ski"]({bbox});"#
    ));
    query.push_str(");out geom;");
    query
}

/// The five queries a box was once asked as (main, water, golf, lifts, ski), each its own cache key,
/// spelled exactly as they were so the answers already cached are found again.
fn legacy([south, west, north, east]: [f64; 4]) -> [String; 5] {
    let bbox = format!("{south},{west},{north},{east}");
    let mut main = format!("[out:json][timeout:120];(way[\"building\"]({bbox});way[\"highway\"]({bbox});way[\"railway\"]({bbox});");
    for filter in COVER_FILTERS.split(';') {
        main.push_str(&format!("way{filter}({bbox});relation[type=multipolygon]{filter}({bbox});"));
    }
    main.push_str(");out geom;");
    let mut water = String::from("[out:json][timeout:120];(");
    for filter in WATER_FILTERS {
        water.push_str(&format!("way{filter}({bbox});relation{filter}({bbox});"));
    }
    water.push_str(&format!(r#"way["waterway"~"^(river|stream|canal)$"]({bbox});way["natural"="coastline"]({bbox});"#));
    water.push_str(");out geom;");
    let golf = format!(r#"[out:json][timeout:120];(way["golf"]({bbox});node["golf"="pin"]({bbox}););out geom;"#);
    let lifts = format!(r#"[out:json][timeout:120];way["aerialway"~"^({})$"]({bbox});out geom;"#, crate::osm::LIFTS.join("|"));
    let ski = format!(r#"[out:json][timeout:120];(way["piste:type"~"nordic"]({bbox});relation["piste:type"~"nordic"]({bbox});relation["route"="ski"]({bbox}););out geom;"#);
    [main, water, golf, lifts, ski]
}

/// Whether the box's answer is in the cache, as one query or as all of the old five.
pub fn cached(bbox: [f64; 4]) -> bool {
    crate::cache::has("overpass", &query(bbox)) || legacy(bbox).iter().all(|query| crate::cache::has("overpass", query))
}

/// Fetches each of `pieces` (`[south, west, north, east]`) and merges the answers. The pieces are
/// the cache's unit: pass the same boxes and the same entries are reused. `retries` is how many extra
/// rounds over `endpoints` each piece gets, `pause` seconds apart.
pub fn fetch_pieces(endpoints: &[String], pieces: &[[f64; 4]], retries: usize, pause: u64) -> Result<Data> {
    let mut merged = Data::default();
    let mut seen = Seen::default();
    for (index, piece) in pieces.iter().enumerate() {
        eprintln!("overpass: part {} of {}, {piece:?}", index + 1, pieces.len());
        let data = fetch_piece(endpoints, *piece, retries, pause).with_context(|| format!("part {} of {} (finished parts are cached)", index + 1, pieces.len()))?;
        merged.merge(data, &mut seen);
        crate::progress::steps(index + 1, pieces.len());
    }
    Ok(merged)
}

/// One piece: the one query's answer, or, for a piece cached whole before the queries were one, the
/// old five. A piece with only some of the five cached is asked for again as one query: one request,
/// where the missing ones would be up to four.
fn fetch_piece(endpoints: &[String], piece: [f64; 4], retries: usize, pause: u64) -> Result<Data> {
    let combined = query(piece);
    let legacy = legacy(piece);
    if !crate::cache::has("overpass", &combined) && legacy.iter().all(|query| crate::cache::has("overpass", query)) {
        let (mut data, mut seen) = (Data::default(), Seen::default());
        for query in &legacy {
            data.merge(parse(&run(endpoints, query, retries, pause)?)?, &mut seen);
        }
        // Each answer is sorted by id, as the one query's is: so is the merge, and every pass sees its
        // elements in the order its own query gave them.
        data.sort();
        return Ok(data);
    }
    parse(&run(endpoints, &combined, retries, pause)?)
}

/// An Overpass `out geom` answer as elements. Only way members of relations are kept.
pub fn parse(text: &str) -> Result<Data> {
    let response: Response = serde_json::from_str(text).context("Overpass returned invalid JSON")?;
    let mut data = Data::default();
    for element in response.elements {
        match element.kind.as_str() {
            "way" if element.geometry.len() >= 2 => data.ways.push(Way { id: element.id, tags: element.tags, geometry: element.geometry }),
            "relation" => {
                let members = element.members.into_iter().filter(|m| m.kind == "way").map(|m| Member { way: m.id, role: m.role, geometry: m.geometry }).collect();
                data.relations.push(Relation { id: element.id, tags: element.tags, members });
            }
            "node" => {
                if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
                    data.nodes.push(Node { id: element.id, tags: element.tags, at: LatLon { lat, lon } });
                }
            }
            _ => {}
        }
    }
    Ok(data)
}

/// Runs `query` on the first of `endpoints` that answers, trying them all `retries` more times,
/// `pause` seconds apart. The answer is cached by the query, not the endpoint: any server's answer
/// is as good as another's.
pub(crate) fn run(endpoints: &[String], query: &str, retries: usize, pause: u64) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(200))
        .build()?;
    for round in 0..=retries {
        if round > 0 {
            eprintln!("  retrying in {pause} s");
            crate::progress::retry(pause);
        }
        for endpoint in endpoints {
            let text = crate::cache::text("overpass", query, || {
                Ok(client
                    .post(endpoint)
                    .form(&[("data", query)])
                    .send()
                    .context("Overpass request failed")?
                    .error_for_status()
                    .context("Overpass returned an error (rate limited? try again in a minute)")?
                    .text()?)
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_old_queries_are_spelled_as_they_were() {
        // The cache keys of every area fetched before the queries were one.
        let [main, water, golf, lifts, ski] = legacy([1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            main,
            r#"[out:json][timeout:120];(way["building"](1,2,3,4);way["highway"](1,2,3,4);way["railway"](1,2,3,4);way[landuse](1,2,3,4);relation[type=multipolygon][landuse](1,2,3,4);way[natural~"^(wood|scrub|grassland|heath|wetland|sand|beach)$"](1,2,3,4);relation[type=multipolygon][natural~"^(wood|scrub|grassland|heath|wetland|sand|beach)$"](1,2,3,4);way[leisure~"^(park|garden|pitch|playground|golf_course|recreation_ground)$"](1,2,3,4);relation[type=multipolygon][leisure~"^(park|garden|pitch|playground|golf_course|recreation_ground)$"](1,2,3,4););out geom;"#
        );
        assert_eq!(
            water,
            r#"[out:json][timeout:120];(way["natural"="water"](1,2,3,4);relation["natural"="water"](1,2,3,4);way["waterway"="riverbank"](1,2,3,4);relation["waterway"="riverbank"](1,2,3,4);way["landuse"="reservoir"](1,2,3,4);relation["landuse"="reservoir"](1,2,3,4);way["waterway"~"^(river|stream|canal)$"](1,2,3,4);way["natural"="coastline"](1,2,3,4););out geom;"#
        );
        assert_eq!(golf, r#"[out:json][timeout:120];(way["golf"](1,2,3,4);node["golf"="pin"](1,2,3,4););out geom;"#);
        assert_eq!(lifts, r#"[out:json][timeout:120];way["aerialway"~"^(chair_lift|mixed_lift|drag_lift|t-bar|j-bar|platter|rope_tow|gondola|cable_car|magic_carpet)$"](1,2,3,4);out geom;"#);
        assert_eq!(ski, r#"[out:json][timeout:120];(way["piste:type"~"nordic"](1,2,3,4);relation["piste:type"~"nordic"](1,2,3,4);relation["route"="ski"](1,2,3,4););out geom;"#);
    }
}
