use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

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
    pub tags: HashMap<String, String>,
    pub outer: Vec<Vec<LatLon>>,
    pub inner: Vec<Vec<LatLon>>,
}

/// What Overpass returned for the box.
#[derive(Debug, Default)]
pub struct Osm {
    pub ways: Vec<Way>,
    pub areas: Vec<Area>,
}

/// The tags that make a way or relation ground cover, as an Overpass filter list.
const COVER_FILTERS: &str = r#"[landuse];[natural~"^(water|wood|scrub|grassland|heath|wetland|sand|beach)$"];[leisure~"^(park|garden|pitch|playground|golf_course|recreation_ground)$"];[waterway=riverbank]"#;

/// Fetches every building, highway and piece of ground cover in `[south, west, north, east]`.
/// Multipolygon relations are returned for cover only; courtyard buildings are not handled yet.
pub fn fetch(endpoint: &str, [south, west, north, east]: [f64; 4]) -> Result<Osm> {
    let bbox = format!("{south},{west},{north},{east}");
    let mut query = format!("[out:json][timeout:120];(way[\"building\"]({bbox});way[\"highway\"]({bbox});");
    for filter in COVER_FILTERS.split(';') {
        query.push_str(&format!("way{filter}({bbox});relation[type=multipolygon]{filter}({bbox});"));
    }
    query.push_str(");out geom;");
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
                osm.areas.push(Area { tags: element.tags, outer, inner });
            }
            _ => {}
        }
    }
    Ok(osm)
}
