//! Ski lifts from OpenStreetMap (`aerialway=*` lines, `osm::LIFTS`), for any world. Every lift is
//! built as a chair lift; what OSM calls it is kept (`LIFT_*`). LIFTS.md.

use crate::osm::{Data, LatLon};
use shared::consts::{LIFT_MAX_SEATS, LIFT_SEATS};
use shared::world::{LIFT_CABIN, LIFT_CHAIR, LIFT_DRAG, LIFT_OTHER};

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

/// The lifts of `data`.
pub fn from(data: &Data) -> Vec<Mapped> {
    let mut found = Vec::new();
    for way in data.ways.iter().filter(|way| crate::osm::is_lift(&way.tags) && way.geometry.len() >= 2) {
        let tag = |key: &str| way.tags.get(key).map(String::as_str);
        // A chair for one is still a chair; a cabin for eight is drawn as a chair for four.
        let seats = tag("aerialway:occupancy").and_then(|text| text.trim().parse::<u8>().ok()).unwrap_or(LIFT_SEATS).clamp(1, LIFT_MAX_SEATS);
        found.push(Mapped { kind: kind(tag("aerialway").unwrap_or("")), seats, line: way.geometry.clone() });
    }
    found
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
        let found: Vec<(u8, u8)> = from(&crate::overpass::parse(&response.to_string()).unwrap()).into_iter().map(|lift| (lift.kind, lift.seats)).collect();
        assert_eq!(found, [(LIFT_CHAIR, 4), (LIFT_DRAG, LIFT_SEATS), (LIFT_CABIN, LIFT_MAX_SEATS)]);
    }
}
