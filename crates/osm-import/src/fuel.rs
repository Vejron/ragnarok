//! The fuel stations of a world (FUEL.md): where OpenStreetMap maps an `amenity=fuel`, as a point
//! or as the outline of the station or of its building. A vehicle near one has its tank filled.

use crate::assemble::Built;
use crate::osm::{Data, LatLon, Lookup};
use shared::consts::FUEL_STATION_APART;

/// A node or a way that is a fuel station.
pub fn is_fuel<T: Lookup + ?Sized>(tags: &T) -> bool {
    tags.tag("amenity") == Some("fuel")
}

/// The fuel stations of `data`: each node that is one, and the middle of each way that is one.
pub fn stations(data: &Data) -> Vec<LatLon> {
    let nodes = data.nodes.iter().filter(|node| is_fuel(&node.tags)).map(|node| node.at);
    let ways = data.ways.iter().filter(|way| is_fuel(&way.tags) && !way.geometry.is_empty()).map(|way| {
        // A closed way repeats its first corner, which would count twice.
        let closed = way.geometry.len() > 1 && way.geometry.first() == way.geometry.last();
        let corners = &way.geometry[..way.geometry.len() - usize::from(closed)];
        let count = corners.len() as f64;
        LatLon { lat: corners.iter().map(|at| at.lat).sum::<f64>() / count, lon: corners.iter().map(|at| at.lon).sum::<f64>() / count }
    });
    nodes.chain(ways).collect()
}

/// `points` (metres) with those nearer than `FUEL_STATION_APART` to one already kept left out: the
/// pumps, the canopy and the shop of one station are mapped each on its own.
pub fn merged(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut kept: Vec<[f64; 2]> = Vec::new();
    for &[x, z] in points {
        if kept.iter().all(|&[kx, kz]| (kx - x).hypot(kz - z) >= f64::from(FUEL_STATION_APART)) {
            kept.push([x, z]);
        }
    }
    kept
}

/// Puts the fuel stations of the finished world in its manifest: `points` (metres) as OpenStreetMap
/// maps them, one for each station (`merged`), and only those on a tile of the world.
pub fn lay(built: &mut Built, points: &[[f64; 2]]) {
    let (half, tile) = (f64::from(built.manifest.half_extent) / 100.0, f64::from(built.manifest.tile_size) / 100.0);
    let inside = |[x, z]: [f64; 2]| {
        let (col, row) = (((x + half) / tile).floor(), ((z + half) / tile).floor());
        col >= 0.0 && row >= 0.0 && built.tiles.contains_key(&(col as u16, row as u16))
    };
    let within: Vec<[f64; 2]> = points.iter().copied().filter(|&point| inside(point)).collect();
    built.manifest.fuel_stations = merged(&within).into_iter().map(|[x, z]| [(x * 100.0).round() as i32, (z * 100.0).round() as i32]).collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osm::{Node, Tags, Way};

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn a_fuel_node_and_the_middle_of_a_fuel_way_are_stations_and_nothing_else_is() {
        let at = |lat, lon| LatLon { lat, lon };
        let data = Data {
            nodes: vec![Node { id: 1, tags: tags(&[("amenity", "fuel")]), at: at(63.0, 20.0) }, Node { id: 2, tags: tags(&[("amenity", "cafe")]), at: at(63.1, 20.1) }],
            ways: vec![
                Way { id: 3, tags: tags(&[("amenity", "fuel"), ("building", "roof")]), geometry: vec![at(64.0, 21.0), at(64.0, 21.2), at(64.2, 21.2), at(64.2, 21.0), at(64.0, 21.0)] },
                Way { id: 4, tags: tags(&[("building", "yes")]), geometry: vec![at(65.0, 22.0), at(65.0, 22.1)] },
            ],
            ..Default::default()
        };
        let found = stations(&data);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].lat, found[0].lon), (63.0, 20.0));
        assert!((found[1].lat - 64.1).abs() < 1e-9 && (found[1].lon - 21.1).abs() < 1e-9, "the middle of the outline: {:?}", found[1]);
    }

    #[test]
    fn the_pumps_the_canopy_and_the_shop_of_one_station_are_one_station() {
        let kept = merged(&[[0.0, 0.0], [20.0, 10.0], [-15.0, 30.0], [300.0, 0.0], [310.0, 5.0]]);
        assert_eq!(kept, [[0.0, 0.0], [300.0, 0.0]]);
    }

    #[test]
    fn the_synthetic_city_has_one_station_and_a_station_off_the_world_is_left_out() {
        let mut built = crate::assemble::synthetic_city(150.0, 100.0, Default::default(), Default::default());
        assert_eq!(built.manifest.fuel_stations.len(), 1, "the offline world has one to fill up at");
        lay(&mut built, &[[40.0, -60.0], [900.0, 0.0], [-151.0, 0.0]]);
        assert_eq!(built.manifest.fuel_stations, [[4_000, -6_000]]);
    }
}
