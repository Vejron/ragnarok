//! Ski lifts from OpenStreetMap (`aerialway=*` lines), for any world. A query of its own, because the
//! main one (`overpass::query`) is its own cache key: changing it would fetch every world again. Every
//! lift is built as a chair lift; what OSM calls it is kept (`LIFT_*`). LIFTS.md.

use crate::overpass::LatLon;
use anyhow::{Context, Result};
use serde_json::Value;
use shared::consts::{LIFT_MAX_SEATS, LIFT_SEATS};
use shared::world::{LIFT_CABIN, LIFT_CHAIR, LIFT_DRAG, LIFT_OTHER};
use std::collections::HashSet;

/// The `aerialway` values that are lifts a skier rides: not stations, pylons, goods lines or zip lines.
const LIFTS: &str = "chair_lift|mixed_lift|drag_lift|t-bar|j-bar|platter|rope_tow|gondola|cable_car|magic_carpet";

/// The Overpass QL for a box: also the cache key.
pub fn query([south, west, north, east]: [f64; 4]) -> String {
    format!(r#"[out:json][timeout:120];way["aerialway"~"^({LIFTS})$"]({south},{west},{north},{east});out geom;"#)
}

/// One lift as OSM maps it: its `LIFT_*`, riders a chair carries, and its line from the bottom
/// station to the top.
#[derive(Clone, Debug)]
pub struct Mapped {
    pub kind: u8,
    pub seats: u8,
    pub line: Vec<LatLon>,
}

/// `LIFT_*` of an `aerialway` value.
fn kind(value: &str) -> u8 {
    match value {
        "chair_lift" | "mixed_lift" => LIFT_CHAIR,
        "drag_lift" | "t-bar" | "j-bar" | "platter" | "rope_tow" => LIFT_DRAG,
        "gondola" | "cable_car" => LIFT_CABIN,
        _ => LIFT_OTHER,
    }
}

/// The lifts of one answer, each with the way it is.
fn lifts(response: &Value) -> Vec<(u64, Mapped)> {
    let mut found = Vec::new();
    for element in response["elements"].as_array().map(Vec::as_slice).unwrap_or_default() {
        if element["type"] != "way" {
            continue;
        }
        let tags = &element["tags"];
        let line: Vec<LatLon> = element["geometry"].as_array().map(|a| a.iter().filter_map(|p| Some(LatLon { lat: p["lat"].as_f64()?, lon: p["lon"].as_f64()? })).collect()).unwrap_or_default();
        if line.len() < 2 {
            continue;
        }
        // A chair for one is still a chair; a cabin for eight is drawn as a chair for four.
        let seats = tags["aerialway:occupancy"].as_str().and_then(|text| text.trim().parse::<u8>().ok()).unwrap_or(LIFT_SEATS).clamp(1, LIFT_MAX_SEATS);
        let kind = kind(tags["aerialway"].as_str().unwrap_or(""));
        found.push((element["id"].as_u64().unwrap_or(0), Mapped { kind, seats, line }));
    }
    found
}

/// Fetches the lifts of each of `pieces` (`[south, west, north, east]`) and merges the answers.
pub fn fetch_pieces(endpoints: &[String], pieces: &[[f64; 4]], retries: usize, pause: u64) -> Result<Vec<Mapped>> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let text = crate::overpass::run(endpoints, &query(*piece), retries, pause)?;
        let response: Value = serde_json::from_str(&text).context("Overpass returned invalid JSON")?;
        found.extend(lifts(&response).into_iter().filter(|(id, _)| seen.insert(*id)).map(|(_, lift)| lift));
        crate::progress::steps(index + 1, pieces.len());
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lift_keeps_its_kind_and_how_many_a_chair_carries() {
        let response = serde_json::json!({ "elements": [
            { "type": "way", "id": 1, "tags": { "aerialway": "chair_lift", "aerialway:occupancy": "4" }, "geometry": [{ "lat": 1.0, "lon": 2.0 }, { "lat": 1.5, "lon": 2.5 }] },
            { "type": "way", "id": 2, "tags": { "aerialway": "t-bar" }, "geometry": [{ "lat": 3.0, "lon": 4.0 }, { "lat": 3.5, "lon": 4.5 }] },
            { "type": "way", "id": 3, "tags": { "aerialway": "gondola", "aerialway:occupancy": "8" }, "geometry": [{ "lat": 5.0, "lon": 6.0 }, { "lat": 5.5, "lon": 6.5 }] },
            { "type": "way", "id": 4, "tags": { "aerialway": "chair_lift" }, "geometry": [{ "lat": 5.0, "lon": 6.0 }] },
        ] });
        let found: Vec<(u64, u8, u8)> = lifts(&response).into_iter().map(|(id, lift)| (id, lift.kind, lift.seats)).collect();
        assert_eq!(found, [(1, LIFT_CHAIR, 4), (2, LIFT_DRAG, LIFT_SEATS), (3, LIFT_CABIN, LIFT_MAX_SEATS)]);
    }
}
