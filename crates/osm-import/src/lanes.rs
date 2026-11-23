//! How wide a road is, how many lanes it has and what is painted on it, from its OpenStreetMap tags.
//!
//! Swedish roads carry `lanes` and often `width` (from the national road database), and
//! `lane_markings` says whether a smaller road is painted at all. Where nothing is tagged, the
//! class's own width stands (`assemble::road_style`). The paint follows Swedish practice: white
//! dashed edge lines on country roads (kantlinje), solid ones on motorways and motorroads, a dashed
//! centre line, and a solid one on the single side of a road with two lanes one way and one the other.

use crate::osm::Tags;
use shared::world::{MARK_CENTRE, MARK_CENTRE_SOLID, MARK_EDGE, MARK_EDGE_SOLID, MARK_LANES, ROAD_MAJOR, ROAD_MINOR};

/// How wide a lane of a major road is, metres.
const LANE_MAJOR: f64 = 3.25;
/// How wide a lane of a minor road is, metres.
const LANE_MINOR: f64 = 3.0;
/// The shoulders either side of the lanes together, metres: the edge lines are painted on them.
const SHOULDERS: f64 = 1.0;
/// How wide a road of one lane is, metres, whatever its class: a slip road, a carriageway, a service
/// road or a one-way street. A lane and its shoulders alone (4 m) left a car half a metre either
/// side once the asphalt's edge had crumbled, and looked too slim to drive.
const ONE_LANE: f64 = 5.5;
/// The narrowest a roundabout's ring is, metres: a bus sweeps wide going round it.
const ROUNDABOUT: f64 = 6.0;
/// How far under and over what its lanes make it a `width` tag may put a road, metres. A tag far off
/// is a kerb-to-kerb measure of a street with parking, or of one lane of several.
const TAGGED_WIDTH: (f64, f64) = (1.0, 2.0);

/// What a road's tags say of its lanes and paint (`Road::lanes`, `forward`, `marks`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lanes {
    pub lanes: u8,
    pub forward: u8,
    pub marks: u8,
    /// Whether it is driven one way only (`Road::one_way`).
    pub one_way: bool,
}

/// Which way a road is driven, from its `oneway` tag (or a motorway's or roundabout's own rule).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Oneway {
    Both,
    /// One way, along its points.
    Along,
    /// One way, against its points (`oneway=-1`).
    Against,
}

/// The `oneway` of a highway way.
pub fn oneway(tags: &Tags) -> Oneway {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    match tag("oneway") {
        Some("yes" | "true" | "1") => Oneway::Along,
        Some("-1" | "reverse") => Oneway::Against,
        Some(_) => Oneway::Both,
        None if matches!(tag("highway"), Some("motorway" | "motorway_link")) || matches!(tag("junction"), Some("roundabout" | "circular")) => Oneway::Along,
        None => Oneway::Both,
    }
}

/// A highway way's width in metres, kind, lanes and paint, or `None` for a way that is not a road
/// drawn (as `assemble::road_style`).
pub fn style(tags: &Tags) -> Option<(f64, u8, Lanes)> {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    let highway = tag("highway")?;
    let (class_width, kind) = crate::assemble::road_style(highway, tag("surface"))?;
    if !matches!(kind, ROAD_MAJOR | ROAD_MINOR) {
        return Some((class_width, kind, Lanes::default()));
    }
    let way = oneway(tags);
    let count = |key: &str| tag(key).and_then(|value| value.trim().parse::<u8>().ok()).filter(|&n| (1..=8).contains(&n));
    let tagged = count("lanes");
    let single = matches!(highway, "service" | "living_street");
    let lanes = tagged.unwrap_or(match way {
        Oneway::Both if single => 1,
        Oneway::Both => 2,
        _ if highway == "motorway" => 2,
        _ => 1,
    });
    let lane = if kind == ROAD_MAJOR { LANE_MAJOR } else { LANE_MINOR };
    // As wide as its lanes, whatever its class: a carriageway of a divided road is one lane wide, and
    // an untagged main road two. A paved track keeps its own width.
    let ring = matches!(tag("junction"), Some("roundabout" | "circular"));
    let natural = match lanes {
        _ if highway == "track" => class_width,
        1 => ONE_LANE,
        _ => f64::from(lanes) * lane + SHOULDERS,
    }
    .max(if ring { ROUNDABOUT } else { 0.0 });
    let width = tag("width").and_then(|value| value.trim().trim_end_matches('m').trim().parse::<f64>().ok()).map_or(natural, |width| width.clamp(natural - TAGGED_WIDTH.0, natural + TAGGED_WIDTH.1));
    let forward = match way {
        Oneway::Along => lanes,
        Oneway::Against => 0,
        Oneway::Both => count("lanes:forward").filter(|&n| n <= lanes).unwrap_or(lanes.div_ceil(2)),
    };
    Some((width, kind, Lanes { lanes, forward, marks: marks(tags, kind, way, lanes, forward), one_way: way != Oneway::Both }))
}

/// The `MARK_*` lines painted on a road of `kind` with `lanes`, `forward` of them along its points.
fn marks(tags: &Tags, kind: u8, way: Oneway, lanes: u8, forward: u8) -> u8 {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    let painted = match tag("lane_markings") {
        Some("no") => false,
        Some("yes") => true,
        _ => kind == ROAD_MAJOR,
    };
    if !painted {
        return 0;
    }
    let fast = matches!(tag("highway"), Some("motorway" | "motorway_link")) || tag("motorroad") == Some("yes");
    let mut marks = if fast { MARK_EDGE_SOLID } else { MARK_EDGE };
    let backward = lanes - forward;
    if forward > 1 || backward > 1 {
        marks |= MARK_LANES;
    }
    if way == Oneway::Both && lanes >= 2 {
        // Two lanes one way and one the other is a 2+1 stretch: the single lane may not overtake.
        let barred = tag("overtaking") == Some("no") || (tag("lanes:forward").is_some() && forward != backward);
        marks |= if barred { MARK_CENTRE_SOLID } else { MARK_CENTRE };
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn a_tagged_width_sets_the_road_width() {
        let (width, kind, _) = style(&tags(&[("highway", "secondary"), ("width", "8.5")])).unwrap();
        assert_eq!((width, kind), (8.5, ROAD_MAJOR));
        // A tag far from what the lanes make it is brought to within a metre under and two over.
        assert_eq!(style(&tags(&[("highway", "residential"), ("width", "2")])).unwrap().0, 6.0);
        assert_eq!(style(&tags(&[("highway", "residential"), ("width", "18")])).unwrap().0, 9.0);
    }

    #[test]
    fn lanes_without_width_give_three_and_a_quarter_metres_each() {
        let (width, _, lanes) = style(&tags(&[("highway", "trunk"), ("lanes", "2")])).unwrap();
        assert_eq!(width, 2.0 * LANE_MAJOR + SHOULDERS);
        assert_eq!((lanes.lanes, lanes.forward), (2, 1));
        assert_eq!(lanes.marks, MARK_EDGE | MARK_CENTRE);
    }

    #[test]
    fn an_untagged_road_is_as_wide_as_its_two_lanes_and_a_service_road_as_one() {
        assert_eq!(style(&tags(&[("highway", "primary")])).unwrap().0, 2.0 * LANE_MAJOR + SHOULDERS);
        assert_eq!(style(&tags(&[("highway", "residential")])).unwrap().0, 7.0);
        assert_eq!(style(&tags(&[("highway", "service")])).unwrap().0, ONE_LANE);
        assert_eq!(style(&tags(&[("highway", "track"), ("surface", "asphalt")])).unwrap().0, 4.5);
    }

    #[test]
    fn a_roundabout_is_wide_enough_to_sweep_round() {
        assert_eq!(style(&tags(&[("highway", "tertiary"), ("junction", "roundabout")])).unwrap().0, ROUNDABOUT);
        assert_eq!(style(&tags(&[("highway", "primary"), ("junction", "roundabout"), ("lanes", "2")])).unwrap().0, 2.0 * LANE_MAJOR + SHOULDERS);
    }

    #[test]
    fn a_one_lane_carriageway_of_a_divided_road_is_one_lane_wide() {
        let (width, _, lanes) = style(&tags(&[("highway", "trunk"), ("oneway", "yes")])).unwrap();
        assert_eq!(width, ONE_LANE);
        assert_eq!((lanes.lanes, lanes.forward), (1, 1));
        assert_eq!(lanes.marks, MARK_EDGE);
    }

    #[test]
    fn a_tertiary_without_lane_markings_is_not_painted() {
        assert_eq!(style(&tags(&[("highway", "tertiary"), ("lanes", "2")])).unwrap().2.marks, 0);
        assert_eq!(style(&tags(&[("highway", "tertiary"), ("lane_markings", "yes")])).unwrap().2.marks, MARK_EDGE | MARK_CENTRE);
        assert_eq!(style(&tags(&[("highway", "primary"), ("lane_markings", "no")])).unwrap().2.marks, 0);
    }

    #[test]
    fn two_lanes_one_way_and_one_the_other_bar_the_single_lane_from_overtaking() {
        let (_, _, lanes) = style(&tags(&[("highway", "primary"), ("lanes", "3"), ("lanes:forward", "1"), ("lanes:backward", "2")])).unwrap();
        assert_eq!((lanes.lanes, lanes.forward), (3, 1));
        assert_eq!(lanes.marks, MARK_EDGE | MARK_LANES | MARK_CENTRE_SOLID);
    }

    #[test]
    fn a_motorway_has_solid_edges_and_lane_lines() {
        let (_, _, lanes) = style(&tags(&[("highway", "motorway"), ("lanes", "2")])).unwrap();
        assert_eq!((lanes.lanes, lanes.forward), (2, 2));
        assert_eq!(lanes.marks, MARK_EDGE_SOLID | MARK_LANES);
    }

    #[test]
    fn paths_have_no_lanes() {
        assert_eq!(style(&tags(&[("highway", "footway")])).unwrap().2, Lanes::default());
    }
}
