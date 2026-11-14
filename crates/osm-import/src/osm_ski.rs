//! Cross-country ski tracks from OpenStreetMap, for winter worlds: ways tagged as nordic pistes, and
//! the member ways of nordic pistes and ski routes mapped as relations.

use crate::osm::{Data, LatLon};
use std::collections::HashSet;

/// The ski tracks of `data`. A way that is both tagged itself and a member of a route is taken once.
pub fn from(data: &Data) -> Vec<Vec<LatLon>> {
    let mut seen = HashSet::new();
    let ways = data.ways.iter().filter(|way| crate::osm::is_nordic(&way.tags)).map(|way| (way.id, &way.geometry));
    let members = data.relations.iter().filter(|relation| crate::osm::is_ski_relation(&relation.tags)).flat_map(|relation| relation.members.iter().map(|member| (member.way, &member.geometry)));
    ways.chain(members).filter(|(id, points)| points.len() >= 2 && seen.insert(*id)).map(|(_, points)| points.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_way_is_a_track_and_a_routes_members_are_too() {
        let response = serde_json::json!({ "elements": [
            { "type": "way", "id": 1, "tags": { "piste:type": "nordic" }, "geometry": [{ "lat": 1.0, "lon": 2.0 }, { "lat": 1.5, "lon": 2.5 }] },
            { "type": "relation", "id": 9, "tags": { "route": "ski" }, "members": [
                { "type": "way", "ref": 1, "geometry": [{ "lat": 1.0, "lon": 2.0 }, { "lat": 1.5, "lon": 2.5 }] },
                { "type": "way", "ref": 2, "geometry": [{ "lat": 3.0, "lon": 4.0 }, { "lat": 3.5, "lon": 4.5 }] },
                { "type": "node", "ref": 3 },
            ] },
        ] });
        let found = from(&crate::overpass::parse(&response.to_string()).unwrap());
        assert_eq!(found.iter().map(|line| line[0].lat).collect::<Vec<_>>(), [1.0, 3.0], "the tagged way once, then the route's other member");
    }
}
