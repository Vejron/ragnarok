//! Water from Lantmäteriet's *Hydrografi Direkt*: an OGC API Features service
//! (`api.lantmateriet.se/ogc-features/v1/hydrografi`) with lakes, river polygons and stream
//! centre lines for all of Sweden. It needs the same Basic credentials as the height model
//! (`LM_USER` / `LM_PASS`) and a Hydrografi Direkt subscription on Geotorget.
//!
//! Used collections: `StandingWater` (lakes), `WatercoursePolygon` (wide rivers) and
//! `WatercourseLine` (stream centre lines, in the direction the water flows). The data has no
//! width, depth or surface height: those come from the height model (`water.rs`).

use crate::overpass::{LatLon, split};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::HashSet;

const ITEMS: &str = "https://api.lantmateriet.se/ogc-features/v1/hydrografi/collections";
/// Features asked for per page.
const PAGE: usize = 1000;
/// Squares of about this many metres are fetched one by one, so every answer is small and cached.
const CHUNK: f64 = 2000.0;

/// A polygon: the outer ring first, then holes.
pub type Polygon = Vec<Vec<LatLon>>;

#[derive(Debug, Default)]
pub struct Hydro {
    pub lakes: Vec<Polygon>,
    pub rivers: Vec<Polygon>,
    pub streams: Vec<Vec<LatLon>>,
}

/// The first page's URL for a collection and box: the cache key of that request.
pub fn first_url(collection: &str, [south, west, north, east]: [f64; 4]) -> String {
    format!("{ITEMS}/{collection}/items?f=json&limit={PAGE}&bbox={west},{south},{east},{north}")
}

/// Fetches the water inside `bbox` (`[south, west, north, east]`).
pub fn fetch(bbox: [f64; 4], user: &str, password: &str) -> Result<Hydro> {
    fetch_pieces(&split(bbox, CHUNK), user, password)
}

/// Fetches the water in each of `pieces`; the pieces are the cache's unit.
pub fn fetch_pieces(pieces: &[[f64; 4]], user: &str, password: &str) -> Result<Hydro> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;
    let mut hydro = Hydro::default();
    for collection in ["StandingWater", "WatercoursePolygon", "WatercourseLine"] {
        crate::progress::stage(&format!("water, {collection}"));
        let mut seen = HashSet::new();
        for (index, &[south, west, north, east]) in pieces.iter().enumerate() {
            let mut url = Some(first_url(collection, [south, west, north, east]));
            while let Some(current) = url.take() {
                let text = get(&client, &current, user, password).with_context(|| format!("Hydrografi {collection}"))?;
                let page: Value = serde_json::from_str(&text).context("Hydrografi returned invalid JSON")?;
                for feature in page["features"].as_array().map(Vec::as_slice).unwrap_or_default() {
                    // A feature crossing a chunk border comes back from both sides.
                    if let Some(id) = feature["id"].as_u64()
                        && !seen.insert(id)
                    {
                        continue;
                    }
                    match collection {
                        "StandingWater" => polygons_of(&feature["geometry"], &mut hydro.lakes),
                        "WatercoursePolygon" => polygons_of(&feature["geometry"], &mut hydro.rivers),
                        _ => lines_of(&feature["geometry"], &mut hydro.streams),
                    }
                }
                url = page["links"]
                    .as_array()
                    .and_then(|links| links.iter().find(|link| link["rel"] == "next"))
                    .and_then(|link| link["href"].as_str())
                    .map(String::from);
            }
            crate::progress::steps(index + 1, pieces.len());
        }
    }
    Ok(hydro)
}

fn get(client: &reqwest::blocking::Client, url: &str, user: &str, password: &str) -> Result<String> {
    // The cache key is the URL alone, so the credentials are never stored.
    crate::cache::text("hydro", url, || {
        let mut last = None;
        for attempt in 0..4 {
            if attempt > 0 {
                crate::progress::retry(2 * attempt);
            }
            match client.get(url).basic_auth(user, Some(password)).send() {
                Ok(response) if response.status() == 401 || response.status() == 403 => {
                    bail!("Lantmäteriet refused the Hydrografi request ({}): LM_USER / LM_PASS need a Hydrografi Direkt subscription on Geotorget", response.status())
                }
                Ok(response) => match response.error_for_status() {
                    Ok(ok) => return Ok(ok.text()?),
                    Err(error) => last = Some(anyhow::Error::from(error)),
                },
                Err(error) => last = Some(anyhow::Error::from(error)),
            }
        }
        Err(last.expect("at least one attempt")).context("Hydrografi request failed")
    })
}

fn position(value: &Value) -> Option<LatLon> {
    // GeoJSON is [lon, lat].
    Some(LatLon { lat: value.get(1)?.as_f64()?, lon: value.get(0)?.as_f64()? })
}

fn line(value: &Value) -> Vec<LatLon> {
    value.as_array().map(|points| points.iter().filter_map(position).collect()).unwrap_or_default()
}

fn polygon(value: &Value) -> Polygon {
    value.as_array().map(|rings| rings.iter().map(line).filter(|r| r.len() >= 4).collect()).unwrap_or_default()
}

/// Appends the polygons of a `Polygon` or `MultiPolygon` geometry.
fn polygons_of(geometry: &Value, out: &mut Vec<Polygon>) {
    match geometry["type"].as_str() {
        Some("Polygon") => out.push(polygon(&geometry["coordinates"])),
        Some("MultiPolygon") => out.extend(geometry["coordinates"].as_array().into_iter().flatten().map(polygon)),
        _ => {}
    }
    out.retain(|p| !p.is_empty());
}

/// Appends the lines of a `LineString` or `MultiLineString` geometry.
fn lines_of(geometry: &Value, out: &mut Vec<Vec<LatLon>>) {
    match geometry["type"].as_str() {
        Some("LineString") => out.push(line(&geometry["coordinates"])),
        Some("MultiLineString") => out.extend(geometry["coordinates"].as_array().into_iter().flatten().map(line)),
        _ => {}
    }
    out.retain(|l| l.len() >= 2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_polygons_and_lines_as_lat_lon() {
        let square = serde_json::json!({"type": "Polygon", "coordinates": [[[20.0, 63.0], [20.1, 63.0], [20.1, 63.1], [20.0, 63.1], [20.0, 63.0]]]});
        let mut lakes = Vec::new();
        polygons_of(&square, &mut lakes);
        assert_eq!(lakes.len(), 1);
        assert_eq!((lakes[0][0][0].lat, lakes[0][0][0].lon), (63.0, 20.0));
        let multi = serde_json::json!({"type": "MultiLineString", "coordinates": [[[20.0, 63.0], [20.1, 63.0]], [[1.0, 2.0]]]});
        let mut lines = Vec::new();
        lines_of(&multi, &mut lines);
        assert_eq!(lines.len(), 1, "a one-point line is dropped");
    }
}
