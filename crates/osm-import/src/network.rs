//! Ways that are one road are joined into one. OpenStreetMap cuts a road into a new way wherever a
//! tag changes (a name, a speed limit, a bus route), so one street is a dozen ways end to end. Each
//! was graded, drawn and painted on its own: the ground kinked where two met, the asphalt showed a
//! joint, and the dashes started again. Joined, a road is one line from junction to junction.
//!
//! Two ways are joined where they meet end to end at a point no other road touches, and are the same
//! road there: the same kind, lanes and paint, both on the ground or both a bridge or a tunnel, and
//! driven the same way. Their widths may differ a little (`EVEN`), and the longer one's is kept, so a
//! road does not step in and out by half a metre where one mapper measured it and the next did not.

use crate::lanes::{Lanes, Oneway};
use crate::osm::{LatLon, Osm, Way};
use shared::world::Frame;
use std::collections::HashMap;

/// Two ways of one road whose widths differ by no more than this, metres, are one width: the longer one's.
const EVEN: f64 = 1.5;

/// What must be the same of two ways for them to be one road.
#[derive(PartialEq)]
struct Alike {
    kind: u8,
    lanes: Lanes,
    oneway: Oneway,
    bridge: bool,
    underground: bool,
    layer: Option<String>,
    junction: Option<String>,
    carriageway: bool,
    fast: bool,
    /// A pavement or a crossing by its tags (`sidepaths`).
    part: Option<String>,
}

/// A road way's width, metres, and what it must share with another to be joined to it, or `None` for
/// a way that is not a road drawn as a line: no highway, or an area.
fn alike(way: &Way) -> Option<(f64, Alike)> {
    let tag = |key: &str| way.tags.get(key).cloned();
    let (width, kind, lanes) = crate::lanes::style(&way.tags)?;
    if tag("area").is_some_and(|value| value != "no") || way.geometry.len() < 2 {
        return None;
    }
    Some((
        width,
        Alike {
            kind,
            lanes,
            oneway: crate::lanes::oneway(&way.tags),
            bridge: crate::assemble::is_bridge(way),
            underground: tag("tunnel").is_some_and(|value| value != "no"),
            layer: tag("layer"),
            junction: tag("junction"),
            carriageway: crate::assemble::is_carriageway(&way.tags),
            fast: crate::assemble::is_fast(&way.tags),
            part: ["footway", "cycleway", "path"].iter().find_map(|key| tag(key)),
        },
    ))
}

/// Whether a way may be turned round: driven both ways, with nothing tagged by direction.
fn turns(way: &Way, like: &Alike) -> bool {
    like.oneway == Oneway::Both && !way.tags.keys().any(|key| key.contains(":forward") || key.contains(":backward"))
}

/// A point of OpenStreetMap: two ways that share one have its coordinates to the last digit.
fn key(g: &LatLon) -> (u64, u64) {
    (g.lat.to_bits(), g.lon.to_bits())
}

fn length(way: &Way, frame: Frame) -> f64 {
    let line: Vec<[f64; 2]> = way.geometry.iter().map(|g| frame.project(g.lat, g.lon)).collect();
    line.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum()
}

/// `osm` with the ways of one road joined (see the module's notes), and how many joints went.
pub fn join(mut osm: Osm, frame: Frame) -> (Osm, usize) {
    // How many times a road touches each point, at an end or along the way: two ways meet alone
    // where it is twice. Joining leaves the count as it was.
    let mut touched: HashMap<(u64, u64), u32> = HashMap::new();
    for way in osm.ways.iter().filter(|way| way.tags.contains_key("highway")) {
        for g in &way.geometry {
            *touched.entry(key(g)).or_default() += 1;
        }
    }
    let mut ways: Vec<Option<Way>> = osm.ways.drain(..).map(Some).collect();
    let mut joined = 0;
    loop {
        // The ends of the roads that are left, by point: `(way, whether its last point)`.
        let mut ends: HashMap<(u64, u64), Vec<(usize, bool)>> = HashMap::new();
        for (index, way) in ways.iter().enumerate() {
            let Some(way) = way.as_ref().filter(|way| alike(way).is_some()) else { continue };
            let (first, last) = (key(&way.geometry[0]), key(&way.geometry[way.geometry.len() - 1]));
            // A way that ends where it starts is a ring: nothing joins it there.
            if first != last {
                ends.entry(first).or_default().push((index, false));
                ends.entry(last).or_default().push((index, true));
            }
        }
        let mut busy = vec![false; ways.len()];
        let mut more = false;
        // In a fixed order, so the same world comes out every time.
        let mut meetings: Vec<_> = ends.iter().collect();
        meetings.sort();
        for (point, meeting) in meetings {
            let &[(a, a_last), (b, b_last)] = meeting.as_slice() else { continue };
            if touched.get(point) != Some(&2) || a == b || busy[a] || busy[b] {
                continue;
            }
            let (Some(first), Some(second)) = (ways[a].as_ref(), ways[b].as_ref()) else { continue };
            let (Some((wide_a, like_a)), Some((wide_b, like_b))) = (alike(first), alike(second)) else { continue };
            if like_a != like_b || (wide_a - wide_b).abs() > EVEN {
                continue;
            }
            // Head to tail as they are, or with one turned round where it may be.
            let (turn_a, turn_b) = match (a_last, b_last) {
                (true, false) | (false, true) => (false, false),
                (true, true) if turns(second, &like_b) => (false, true),
                (false, false) if turns(first, &like_a) => (true, false),
                (true, true) if turns(first, &like_a) => (true, false),
                (false, false) if turns(second, &like_b) => (false, true),
                _ => continue,
            };
            let longer_is_a = length(first, frame) >= length(second, frame);
            let (mut first, mut second) = (ways[a].take().expect("checked"), ways[b].take().expect("checked"));
            if turn_a {
                first.geometry.reverse();
            }
            if turn_b {
                second.geometry.reverse();
            }
            // The one whose last point is the meeting point leads.
            let a_leads = key(&first.geometry[first.geometry.len() - 1]) == *point;
            let (mut lead, follow, lead_is_a) = if a_leads { (first, second, true) } else { (second, first, false) };
            lead.geometry.extend(follow.geometry.into_iter().skip(1));
            if longer_is_a != lead_is_a {
                lead.tags = follow.tags;
            }
            ways[if lead_is_a { a } else { b }] = Some(lead);
            (busy[a], busy[b]) = (true, true);
            joined += 1;
            more = true;
        }
        if !more {
            break;
        }
    }
    osm.ways = ways.into_iter().flatten().collect();
    (osm, joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A way along `points` (metres east and south of the frame's origin) with `tags`.
    fn way(id: u64, tags: &[(&str, &str)], points: &[[f64; 2]]) -> Way {
        let geometry = points.iter().map(|p| LatLon { lat: -p[1] / shared::world::METRES_PER_DEG_LAT, lon: p[0] / shared::world::metres_per_deg_lon() }).collect();
        Way { id, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), geometry }
    }

    /// The ways left of `ways`, each as its id and how many points it has.
    fn joined(ways: Vec<Way>) -> Vec<(u64, usize)> {
        let frame = Frame::Equirect { origin: (0.0, 0.0) };
        let mut left: Vec<(u64, usize)> = join(Osm { ways, areas: Vec::new(), pois: Vec::new() }, frame).0.ways.iter().map(|way| (way.id, way.geometry.len())).collect();
        left.sort();
        left
    }

    #[test]
    fn a_street_cut_in_three_is_one_way_whichever_way_its_pieces_are_drawn() {
        let street = [("highway", "residential")];
        let ways = vec![way(1, &street, &[[0.0, 0.0], [100.0, 0.0]]), way(2, &street, &[[200.0, 0.0], [150.0, 5.0], [100.0, 0.0]]), way(3, &street, &[[200.0, 0.0], [300.0, 0.0]])];
        let left = joined(ways);
        assert_eq!(left.len(), 1, "{left:?}");
        assert_eq!(left[0].1, 5);
    }

    #[test]
    fn a_junction_a_bridge_and_a_change_of_road_are_left_as_they_are() {
        let street = [("highway", "residential")];
        let ways = vec![
            // Three streets at one point: a junction.
            way(1, &street, &[[0.0, 0.0], [100.0, 0.0]]),
            way(2, &street, &[[100.0, 0.0], [200.0, 0.0]]),
            way(3, &street, &[[100.0, 0.0], [100.0, 100.0]]),
            // A street onto its bridge.
            way(4, &[("highway", "residential"), ("bridge", "yes")], &[[200.0, 0.0], [260.0, 0.0]]),
            // A street that becomes a main road, and a main road of two lanes that becomes one of four.
            way(5, &[("highway", "primary"), ("lanes", "2")], &[[260.0, 0.0], [400.0, 0.0]]),
            way(6, &[("highway", "primary"), ("lanes", "4")], &[[400.0, 0.0], [500.0, 0.0]]),
            // A street crossed in the middle of a way by a path: no end of the street's is there.
            way(7, &[("highway", "footway")], &[[450.0, -20.0], [450.0, 0.0]]),
            way(8, &[("highway", "footway")], &[[450.0, 0.0], [450.0, 20.0]]),
            way(9, &[("highway", "primary"), ("lanes", "4")], &[[500.0, 0.0], [450.0, 0.0], [430.0, 30.0]]),
        ];
        // Only the two four-lane pieces are one road; the paths meet on it, where three ways touch.
        assert_eq!(joined(ways).iter().map(|&(id, _)| id).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn one_way_streets_join_head_to_tail_only() {
        let one = [("highway", "residential"), ("oneway", "yes")];
        assert_eq!(joined(vec![way(1, &one, &[[0.0, 0.0], [100.0, 0.0]]), way(2, &one, &[[100.0, 0.0], [200.0, 0.0]])]).len(), 1);
        assert_eq!(joined(vec![way(1, &one, &[[0.0, 0.0], [100.0, 0.0]]), way(2, &one, &[[200.0, 0.0], [100.0, 0.0]])]).len(), 2);
    }

    #[test]
    fn a_small_change_of_width_is_evened_out_to_the_longer_piece() {
        let frame = Frame::Equirect { origin: (0.0, 0.0) };
        let ways = vec![way(1, &[("highway", "residential"), ("width", "6")], &[[0.0, 0.0], [40.0, 0.0]]), way(2, &[("highway", "residential")], &[[40.0, 0.0], [300.0, 0.0]])];
        let (osm, count) = join(Osm { ways, areas: Vec::new(), pois: Vec::new() }, frame);
        assert_eq!((osm.ways.len(), count), (1, 1));
        assert!(!osm.ways[0].tags.contains_key("width"), "the longer piece's tags are kept");
        assert_eq!(osm.ways[0].geometry.len(), 3);
        // Three metres' difference is two roads.
        let ways = vec![way(1, &[("highway", "residential"), ("width", "10")], &[[0.0, 0.0], [40.0, 0.0]]), way(2, &[("highway", "residential")], &[[40.0, 0.0], [300.0, 0.0]])];
        assert_eq!(join(Osm { ways, areas: Vec::new(), pois: Vec::new() }, frame).1, 0);
    }
}
