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

/// Fetches every building and highway way in `[south, west, north, east]`. Multipolygon
/// relations (courtyard buildings) are not handled yet.
pub fn fetch(endpoint: &str, [south, west, north, east]: [f64; 4]) -> Result<Vec<Way>> {
    let bbox = format!("{south},{west},{north},{east}");
    let query = format!(
        "[out:json][timeout:90];(way[\"building\"]({bbox});way[\"highway\"]({bbox}););out geom;"
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let response: Response = client
        .post(endpoint)
        .form(&[("data", query)])
        .send()
        .context("Overpass request failed")?
        .error_for_status()
        .context("Overpass returned an error (rate limited? try again in a minute)")?
        .json()
        .context("Overpass returned invalid JSON")?;
    Ok(response
        .elements
        .into_iter()
        .filter(|element| element.kind == "way" && element.geometry.len() >= 2)
        .map(|element| Way { id: element.id, tags: element.tags, geometry: element.geometry })
        .collect())
}
