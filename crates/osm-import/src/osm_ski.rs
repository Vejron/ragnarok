//! Cross-country ski tracks from OpenStreetMap, for winter worlds. A query of its own, because the
//! main one (`overpass::query`) is its own cache key: changing it would fetch every world again.

use crate::overpass::LatLon;
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashSet;

/// The Overpass QL for a box: also the cache key.
pub fn query([south, west, north, east]: [f64; 4]) -> String {
    let bbox = format!("{south},{west},{north},{east}");
    format!(r#"[out:json][timeout:120];(way["piste:type"~"nordic"]({bbox});relation["piste:type"~"nordic"]({bbox});relation["route"="ski"]({bbox}););out geom;"#)
}

fn line(value: &Value) -> Vec<LatLon> {
    value.as_array().map(|a| a.iter().filter_map(|p| Some(LatLon { lat: p["lat"].as_f64()?, lon: p["lon"].as_f64()? })).collect()).unwrap_or_default()
}

/// The ski tracks of one answer, each with the way it is, so a way that is both tagged itself and a
/// member of a route can be taken once.
fn tracks(response: &Value) -> Vec<(u64, Vec<LatLon>)> {
    let mut found = Vec::new();
    for element in response["elements"].as_array().map(Vec::as_slice).unwrap_or_default() {
        match element["type"].as_str() {
            Some("way") => found.push((element["id"].as_u64().unwrap_or(0), line(&element["geometry"]))),
            Some("relation") => {
                for member in element["members"].as_array().map(Vec::as_slice).unwrap_or_default() {
                    if member["type"] == "way" {
                        found.push((member["ref"].as_u64().unwrap_or(0), line(&member["geometry"])));
                    }
                }
            }
            _ => {}
        }
    }
    found.retain(|(_, points)| points.len() >= 2);
    found
}

/// Fetches the ski tracks of each of `pieces` (`[south, west, north, east]`) and merges the answers.
pub fn fetch_pieces(endpoints: &[String], pieces: &[[f64; 4]], retries: usize, pause: u64) -> Result<Vec<Vec<LatLon>>> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let text = crate::overpass::run(endpoints, &query(*piece), retries, pause)?;
        let response: Value = serde_json::from_str(&text).context("Overpass returned invalid JSON")?;
        found.extend(tracks(&response).into_iter().filter(|(id, _)| seen.insert(*id)).map(|(_, points)| points));
        crate::progress::steps(index + 1, pieces.len());
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_way_is_a_track_and_a_routes_members_are_too() {
        let response = serde_json::json!({ "elements": [
            { "type": "way", "id": 1, "geometry": [{ "lat": 1.0, "lon": 2.0 }, { "lat": 1.5, "lon": 2.5 }] },
            { "type": "relation", "id": 9, "members": [
                { "type": "way", "ref": 2, "geometry": [{ "lat": 3.0, "lon": 4.0 }, { "lat": 3.5, "lon": 4.5 }] },
                { "type": "node", "ref": 3 },
            ] },
        ] });
        let ids: Vec<u64> = tracks(&response).into_iter().map(|(id, _)| id).collect();
        assert_eq!(ids, [1, 2]);
    }
}
