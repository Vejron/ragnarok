//! OpenStreetMap as the importer reads it: one read of a box that serves every pass (buildings and
//! roads, water, golf, lifts, ski tracks), from a local Geofabrik extract (`pbf`) when one covers the
//! box and from Overpass (`overpass`) when none does.
//!
//! What is read is the union of what the passes want (`wants_way`, `wants_relation`, `wants_node`).
//! Each pass then takes its own view of it, with the same tag filter its own Overpass query once had,
//! so a world comes out the same whichever source served it.

use anyhow::Result;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

pub type Tags = HashMap<String, String>;

/// An OSM way with its inline geometry.
#[derive(Clone, Debug)]
pub struct Way {
    pub id: u64,
    pub tags: Tags,
    pub geometry: Vec<LatLon>,
}

/// A multipolygon relation: its tags and its member ways, outer and inner, not yet joined into rings.
#[derive(Clone, Debug)]
pub struct Area {
    pub id: u64,
    pub tags: Tags,
    pub outer: Vec<Vec<LatLon>>,
    pub inner: Vec<Vec<LatLon>>,
}

/// The buildings, highways, railways and ground cover of a box: what `assemble` builds the world from.
#[derive(Debug, Default)]
pub struct Osm {
    pub ways: Vec<Way>,
    pub areas: Vec<Area>,
}

/// A way that is a member of a relation, with its role and geometry.
#[derive(Clone, Debug)]
pub struct Member {
    pub way: u64,
    pub role: String,
    pub geometry: Vec<LatLon>,
}

/// A relation and its member ways. Node and relation members are not kept: no pass reads them.
#[derive(Clone, Debug)]
pub struct Relation {
    pub id: u64,
    pub tags: Tags,
    pub members: Vec<Member>,
}

/// A tagged node.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: u64,
    pub tags: Tags,
    pub at: LatLon,
}

/// Everything any pass wants in a box, each element once.
#[derive(Clone, Debug, Default)]
pub struct Data {
    pub ways: Vec<Way>,
    pub relations: Vec<Relation>,
    pub nodes: Vec<Node>,
}

impl Data {
    /// Adds `other`'s elements that are not here yet. A way or relation that crosses the border of
    /// two pieces comes back from both.
    pub fn merge(&mut self, other: Data, seen: &mut Seen) {
        self.ways.extend(other.ways.into_iter().filter(|way| seen.ways.insert(way.id)));
        self.relations.extend(other.relations.into_iter().filter(|relation| seen.relations.insert(relation.id)));
        self.nodes.extend(other.nodes.into_iter().filter(|node| seen.nodes.insert(node.id)));
    }

    /// Sorts each type by id, the order Overpass answers in.
    pub fn sort(&mut self) {
        self.ways.sort_unstable_by_key(|way| way.id);
        self.relations.sort_unstable_by_key(|relation| relation.id);
        self.nodes.sort_unstable_by_key(|node| node.id);
    }

    /// The buildings, highways, railways and ground cover, as `assemble` takes them: the ways the main
    /// query asked for, and multipolygons of cover.
    pub fn main(&self) -> Osm {
        let ways = self.ways.iter().filter(|way| is_main_way(&way.tags)).cloned().collect();
        let areas = self
            .relations
            .iter()
            .filter(|relation| is_cover_area(&relation.tags))
            .map(|relation| {
                let (mut outer, mut inner) = (Vec::new(), Vec::new());
                for member in relation.members.iter().filter(|member| member.geometry.len() >= 2) {
                    if member.role == "inner" { inner.push(member.geometry.clone()) } else { outer.push(member.geometry.clone()) }
                }
                Area { id: relation.id, tags: relation.tags.clone(), outer, inner }
            })
            .collect();
        Osm { ways, areas }
    }
}

/// The ids already taken, by type, for `Data::merge`.
#[derive(Debug, Default)]
pub struct Seen {
    ways: HashSet<u64>,
    relations: HashSet<u64>,
    nodes: HashSet<u64>,
}

/// Tags to test: an element's own map, or the borrowed pairs the extract's reader holds.
pub trait Lookup {
    fn tag(&self, key: &str) -> Option<&str>;
}

impl Lookup for Tags {
    fn tag(&self, key: &str) -> Option<&str> {
        self.get(key).map(String::as_str)
    }
}

impl Lookup for [(&str, &str)] {
    fn tag(&self, key: &str) -> Option<&str> {
        self.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
}

fn tag<'a, T: Lookup + ?Sized>(tags: &'a T, key: &str) -> Option<&'a str> {
    tags.tag(key)
}

/// Ground cover: `landuse=*`, some `natural` and some `leisure`. The main query's `COVER_FILTERS`.
fn is_cover<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "landuse").is_some()
        || matches!(tag(tags, "natural"), Some("wood" | "scrub" | "grassland" | "heath" | "wetland" | "sand" | "beach"))
        || matches!(tag(tags, "leisure"), Some("park" | "garden" | "pitch" | "playground" | "golf_course" | "recreation_ground"))
}

/// A way of the main query: a building, a highway, a railway or cover.
pub fn is_main_way<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "building").is_some() || tag(tags, "highway").is_some() || tag(tags, "railway").is_some() || is_cover(tags)
}

/// A relation of the main query: a multipolygon of cover.
pub fn is_cover_area<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "type") == Some("multipolygon") && is_cover(tags)
}

/// A lake, a river's polygon or a reservoir, as a way or a relation.
pub fn is_water_body<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "natural") == Some("water") || tag(tags, "waterway") == Some("riverbank") || tag(tags, "landuse") == Some("reservoir")
}

/// A way of the water query: a body, a stream's centre line or the coast.
pub fn is_water_way<T: Lookup + ?Sized>(tags: &T) -> bool {
    is_water_body(tags) || matches!(tag(tags, "waterway"), Some("river" | "stream" | "canal")) || tag(tags, "natural") == Some("coastline")
}

/// A way of the golf query: anything tagged `golf`.
pub fn is_golf_way<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "golf").is_some()
}

/// A node of the golf query: a pin.
pub fn is_golf_node<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "golf") == Some("pin")
}

/// The `aerialway` values that are lifts a skier rides: not stations, pylons, goods lines or zip lines.
pub const LIFTS: [&str; 10] = ["chair_lift", "mixed_lift", "drag_lift", "t-bar", "j-bar", "platter", "rope_tow", "gondola", "cable_car", "magic_carpet"];

/// A way of the lifts query.
pub fn is_lift<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "aerialway").is_some_and(|value| LIFTS.contains(&value))
}

/// A cross-country ski track, as a way or a relation (`piste:type` containing "nordic").
pub fn is_nordic<T: Lookup + ?Sized>(tags: &T) -> bool {
    tag(tags, "piste:type").is_some_and(|value| value.contains("nordic"))
}

/// A relation of the ski query: a nordic piste or a ski route.
pub fn is_ski_relation<T: Lookup + ?Sized>(tags: &T) -> bool {
    is_nordic(tags) || tag(tags, "route") == Some("ski")
}

/// Whether any pass wants this way.
pub fn wants_way<T: Lookup + ?Sized>(tags: &T) -> bool {
    is_main_way(tags) || is_water_way(tags) || is_golf_way(tags) || is_lift(tags) || is_nordic(tags)
}

/// Whether any pass wants this relation.
pub fn wants_relation<T: Lookup + ?Sized>(tags: &T) -> bool {
    is_cover_area(tags) || is_water_body(tags) || is_ski_relation(tags)
}

/// Whether any pass wants this node.
pub fn wants_node<T: Lookup + ?Sized>(tags: &T) -> bool {
    is_golf_node(tags)
}

/// Reads everything any pass wants in each of `pieces` (`[south, west, north, east]`): from a local
/// extract when one covers them all, else from Overpass, piece by piece (each cached on its own).
pub fn load(pieces: &[[f64; 4]], endpoints: &[String], retries: usize, pause: u64) -> Result<Data> {
    if let Some(index) = crate::pbf::Index::covering(pieces) {
        eprintln!("osm: local extract {} of {}", index.name(), index.date());
        return index.read(pieces);
    }
    eprintln!("osm: Overpass (no local extract covers the world; `--fetch-osm sweden` makes one)");
    crate::overpass::fetch_pieces(endpoints, pieces, retries, pause)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn the_main_view_keeps_what_the_main_query_asked_for() {
        let way = |id, pairs: &[(&str, &str)]| Way { id, tags: tags(pairs), geometry: vec![LatLon { lat: 0.0, lon: 0.0 }; 2] };
        let member = |role: &str| Member { way: 9, role: role.into(), geometry: vec![LatLon { lat: 0.0, lon: 0.0 }; 2] };
        let data = Data {
            ways: vec![way(1, &[("building", "yes")]), way(2, &[("golf", "green")]), way(3, &[("natural", "wood")]), way(4, &[("natural", "water")])],
            relations: vec![
                Relation { id: 5, tags: tags(&[("type", "multipolygon"), ("landuse", "forest")]), members: vec![member("outer"), member("inner")] },
                Relation { id: 6, tags: tags(&[("type", "multipolygon"), ("natural", "water")]), members: vec![member("outer")] },
            ],
            nodes: Vec::new(),
        };
        let osm = data.main();
        assert_eq!(osm.ways.iter().map(|w| w.id).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(osm.areas.len(), 1);
        assert_eq!((osm.areas[0].outer.len(), osm.areas[0].inner.len()), (1, 1));
    }
}
