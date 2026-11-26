//! The fuel stations of a world (FUEL.md): where OpenStreetMap maps an `amenity=fuel`, as a point
//! or as the outline of the station or of its building. Each becomes a whole station of the game's
//! own (`shared::station`): a lot turned to run along the nearest road and kept back from it
//! (`lots`), cleared of OpenStreetMap's own buildings (`clear`), levelled as a building's pad is
//! (`ring`), then stood on the finished ground with its trees taken out (`lay`).

use crate::assemble::Built;
use crate::osm::{Data, LatLon, Lookup, Osm};
use shared::consts::{FUEL_STATION_APART, STATION_LOT, STATION_ROAD_CLEAR, STATION_ROAD_REACH};
use shared::world::{COVER_ASPHALT, Cover, Frame, FuelStation};

type P = [f64; 2];

/// How far round the lot an OpenStreetMap building is cleared for the station, metres: none stands
/// right against its paving.
const CLEAR_MARGIN: f64 = 2.0;
/// How much of the lot (with its margin) a building may cover and still stand, square metres: a corner
/// mapped a hair over its edge.
const CLEAR_AREA: f64 = 1.0;
/// The spacing of the ground samples the lot's level is the middle of, metres.
const LEVEL_STEP: f64 = 2.0;
/// How far past the lot the ground is levelled with it, metres: the paving's edge is not left on a lip.
const LEVEL_MARGIN: f64 = 1.0;
/// How far round the levelled ground it eases back into the ground as it was, metres.
const APRON: f64 = 8.0;
/// How far under the top of the paving the levelled ground lies, metres: hidden, but not by much.
const UNDER_PAVING: f64 = 0.02;

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
pub fn merged(points: &[P]) -> Vec<P> {
    let mut kept: Vec<P> = Vec::new();
    for &[x, z] in points {
        if kept.iter().all(|&[kx, kz]| (kx - x).hypot(kz - z) >= f64::from(FUEL_STATION_APART)) {
            kept.push([x, z]);
        }
    }
    kept
}

/// Where a station's lot lies, in local metres: its middle, and how far its frame is turned
/// (`FuelStation::yaw`, radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lot {
    pub middle: P,
    pub yaw: f64,
}

impl Lot {
    /// The station's `(x, z)` in the world, metres.
    fn world(&self, [x, z]: P) -> P {
        let (sin, cos) = self.yaw.sin_cos();
        [self.middle[0] + x * cos + z * sin, self.middle[1] - x * sin + z * cos]
    }

    /// The world's `(x, z)` in the station's frame, metres.
    fn local(&self, [x, z]: P) -> P {
        let (sin, cos) = self.yaw.sin_cos();
        let (dx, dz) = (x - self.middle[0], z - self.middle[1]);
        [dx * cos - dz * sin, dx * sin + dz * cos]
    }

    /// Whether the footprint `ring` (metres, closed or open) covers any of the lot, or of `CLEAR_MARGIN`
    /// round it, by more than `CLEAR_AREA`: a building that would stand in the station.
    pub fn overlaps(&self, ring: &[P]) -> bool {
        let (hx, hz) = (f64::from(STATION_LOT[0]) / 2.0 + CLEAR_MARGIN, f64::from(STATION_LOT[1]) / 2.0 + CLEAR_MARGIN);
        let local: Vec<P> = ring.iter().map(|&at| self.local(at)).collect();
        let inside = crate::assemble::clip_ring(&local, [-hx, -hz, hx, hz]);
        inside.len() >= 3 && crate::assemble::signed_area(&inside).abs() > CLEAR_AREA
    }

    /// Whether `at` (metres) is on the lot, or within `margin` of it.
    pub fn covers(&self, at: P, margin: f64) -> bool {
        let [x, z] = self.local(at);
        x.abs() <= f64::from(STATION_LOT[0]) / 2.0 + margin && z.abs() <= f64::from(STATION_LOT[1]) / 2.0 + margin
    }
}

/// The lot's outline, an open ring of local metres: levelled as a building's pad (`condition::Features::pads`).
pub fn ring(lot: &Lot) -> Vec<P> {
    let (hx, hz) = (f64::from(STATION_LOT[0]) / 2.0, f64::from(STATION_LOT[1]) / 2.0);
    [[-hx, -hz], [hx, -hz], [hx, hz], [-hx, hz]].into_iter().map(|corner| lot.world(corner)).collect()
}

/// A road a station can face: its centre line (local metres) and half its width, metres.
#[derive(Clone)]
struct Road {
    line: Vec<P>,
    half: f64,
    /// A service road: the station's own way in, which it faces only when nothing else is near.
    service: bool,
}

/// The roads of `osm` a car drives on, on the ground.
fn roads(osm: &Osm, frame: Frame) -> Vec<Road> {
    osm.ways
        .iter()
        .filter(|way| !crate::assemble::is_bridge(way) && way.tags.get("tunnel").is_none_or(|value| value == "no"))
        .filter_map(|way| {
            let (width, kind, _) = crate::lanes::style(&way.tags)?;
            matches!(kind, shared::world::ROAD_MAJOR | shared::world::ROAD_MINOR).then(|| Road {
                line: way.geometry.iter().map(|at| frame.project(at.lat, at.lon)).collect(),
                half: width / 2.0,
                service: way.tags.get("highway").is_some_and(|value| value == "service"),
            })
        })
        .collect()
}

/// The nearest point of segment `a..b` to `p`, and the distance to it.
fn nearest(p: P, a: P, b: P) -> (P, f64) {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dz * dz;
    let t = if length > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length).clamp(0.0, 1.0) } else { 0.0 };
    let at = [a[0] + dx * t, a[1] + dz * t];
    (at, (p[0] - at[0]).hypot(p[1] - at[1]))
}

/// The lot of a station mapped at `point`, by `roads`: turned to run along the nearest road within
/// `STATION_ROAD_REACH` (one that is not a service road if there is one), on the side of it the
/// station is mapped on, and pushed back from it until its front edge is `STATION_ROAD_CLEAR` off the
/// road's edge. With no road near it stands where it is mapped, facing north.
fn lot(point: P, roads: &[Road]) -> Lot {
    let reach = f64::from(STATION_ROAD_REACH);
    let best = |service: bool| {
        roads
            .iter()
            .filter(|road| road.service == service)
            .flat_map(|road| road.line.windows(2).map(move |pair| (pair[0], pair[1], road.half)))
            .map(|(a, b, half)| (a, b, half, nearest(point, a, b)))
            .filter(|(.., (_, distance))| *distance <= reach)
            .min_by(|x, y| x.3.1.total_cmp(&y.3.1))
    };
    let Some((a, b, half, (foot, distance))) = best(false).or_else(|| best(true)) else {
        return Lot { middle: point, yaw: 0.0 };
    };
    // The station's +X along the road and its +Z away from it, towards where it is mapped.
    let length = (b[0] - a[0]).hypot(b[1] - a[1]).max(1e-9);
    let along = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
    let mut back = [-along[1], along[0]];
    if (point[0] - foot[0]) * back[0] + (point[1] - foot[1]) * back[1] < 0.0 {
        back = [-back[0], -back[1]];
    }
    let needed = half + f64::from(STATION_ROAD_CLEAR) + f64::from(STATION_LOT[1]) / 2.0;
    let out = distance.max(needed);
    // +Z of the station's frame goes to (sin, cos) of its yaw.
    Lot { middle: [foot[0] + back[0] * out, foot[1] + back[1] * out], yaw: back[0].atan2(back[1]) }
}

/// The stations' lots: one for each station (`merged`) of `points` (metres), laid along the roads of
/// `osm`.
pub fn lots(points: &[P], osm: &Osm, frame: Frame) -> Vec<Lot> {
    let roads = roads(osm, frame);
    merged(points).into_iter().map(|point| lot(point, &roads)).collect()
}

/// `osm` without the buildings that stand on any of a station's lot (`Lot::overlaps`): the station's own
/// shop, canopy and wash as OpenStreetMap maps them, and whatever else stood where the station is laid,
/// wherever their middle is.
pub fn clear(mut osm: Osm, lots: &[Lot], frame: Frame) -> (Osm, usize) {
    if lots.is_empty() {
        return (osm, 0);
    }
    let before = osm.ways.len();
    osm.ways.retain(|way| {
        if way.tags.get("building").is_none_or(|value| value == "no") || way.geometry.is_empty() {
            return true;
        }
        let corners: Vec<P> = way.geometry.iter().map(|at| frame.project(at.lat, at.lon)).collect();
        !lots.iter().any(|lot| lot.overlaps(&corners))
    });
    let cleared = before - osm.ways.len();
    (osm, cleared)
}

/// Puts the fuel stations on `lots` in the finished world's manifest: only those on a tile of the
/// world, each standing at the middle height of its lot's ground (levelled, so nearly all one height),
/// and takes the trees, rocks and bushes off each lot.
pub fn lay(built: &mut Built, lots: &[Lot]) {
    let (half, tile) = (f64::from(built.manifest.half_extent) / 100.0, f64::from(built.manifest.tile_size) / 100.0);
    let inside = |[x, z]: P| {
        let (col, row) = (((x + half) / tile).floor(), ((z + half) / tile).floor());
        col >= 0.0 && row >= 0.0 && built.tiles.contains_key(&(col as u16, row as u16))
    };
    let within: Vec<Lot> = lots.iter().copied().filter(|lot| inside(lot.middle)).collect();
    let mut stations = Vec::new();
    for lot in &within {
        let (hx, hz) = (f64::from(STATION_LOT[0]) / 2.0, f64::from(STATION_LOT[1]) / 2.0);
        let mut heights = Vec::new();
        let (nx, nz) = ((hx * 2.0 / LEVEL_STEP) as i32, (hz * 2.0 / LEVEL_STEP) as i32);
        for i in 0..=nx {
            for j in 0..=nz {
                heights.extend(ground(built, lot.world([-hx + f64::from(i) * LEVEL_STEP, -hz + f64::from(j) * LEVEL_STEP])));
            }
        }
        heights.sort_by(f64::total_cmp);
        let base = heights.get(heights.len() / 2).copied().unwrap_or(0.0);
        level(built, lot, base);
        pave(built, lot);
        stations.push(FuelStation { at: [cm(lot.middle[0]), cm(lot.middle[1])], yaw: (lot.yaw * 1000.0).round() as i16, base: cm(base) });
    }
    for tile in built.tiles.values_mut() {
        tile.trees.retain(|tree| {
            let at = [f64::from(tree.x) / 100.0, f64::from(tree.z) / 100.0];
            !within.iter().any(|lot| lot.covers(at, f64::from(tree.crown) / 100.0))
        });
    }
    built.manifest.fuel_stations = stations;
}

/// Levels the ground of `built` under `lot` to just below its paving at `base` (metres), so the paving
/// lies flush on it, and eases it back to the ground as it was over `APRON` round the lot: a station on
/// a slope is cut into it on one side and built up on the other, as one is.
fn level(built: &mut Built, lot: &Lot, base: f64) {
    let manifest = &built.manifest;
    if manifest.ground_cell <= 0 {
        return;
    }
    let (half, tile, cell) = (f64::from(manifest.half_extent) / 100.0, f64::from(manifest.tile_size) / 100.0, f64::from(manifest.ground_cell) / 100.0);
    let samples = (manifest.tile_size / manifest.ground_cell) as usize + 1;
    let (hx, hz) = (f64::from(STATION_LOT[0]) / 2.0 + LEVEL_MARGIN, f64::from(STATION_LOT[1]) / 2.0 + LEVEL_MARGIN);
    let reach = hx.hypot(hz) + APRON;
    let flat = base - UNDER_PAVING;
    for (&(col, row), tile_data) in built.tiles.iter_mut() {
        if tile_data.ground.len() != samples * samples {
            continue;
        }
        let (x0, z0) = (-half + f64::from(col) * tile, -half + f64::from(row) * tile);
        if lot.middle[0] < x0 - reach || lot.middle[0] > x0 + tile + reach || lot.middle[1] < z0 - reach || lot.middle[1] > z0 + tile + reach {
            continue;
        }
        for j in 0..samples {
            for i in 0..samples {
                let [x, z] = lot.local([x0 + i as f64 * cell, z0 + j as f64 * cell]);
                // How far outside the levelled rectangle the sample is, metres.
                let out = (x.abs() - hx).max(0.0).hypot((z.abs() - hz).max(0.0));
                if out >= APRON {
                    continue;
                }
                let t = out / APRON;
                let keep = t * t * (3.0 - 2.0 * t);
                let h = &mut tile_data.ground[j * samples + i];
                *h = cm(flat + (f64::from(*h) / 100.0 - flat) * keep);
            }
        }
    }
}

/// Lays `lot` as asphalt in the cover of every tile it reaches, over whatever cover was there: the
/// ground under the paving is painted as a car park's, and no grass or forest floor grows through it.
fn pave(built: &mut Built, lot: &Lot) {
    let (half, tile) = (f64::from(built.manifest.half_extent) / 100.0, f64::from(built.manifest.tile_size) / 100.0);
    let outline = ring(lot);
    for (&(col, row), tile_data) in built.tiles.iter_mut() {
        let (x0, z0) = (-half + f64::from(col) * tile, -half + f64::from(row) * tile);
        let part = crate::assemble::clip_ring(&outline, [x0, z0, x0 + tile, z0 + tile]);
        if part.len() >= 3 {
            tile_data.cover.push(Cover { kind: COVER_ASPHALT, rings: vec![part.iter().map(|&[x, z]| [cm(x), cm(z)]).collect()] });
        }
    }
}

/// The ground's height at `at` (metres) in `built`, from the nearest sample of its tile; `None` off
/// the tiles or where there is no terrain.
fn ground(built: &Built, at: P) -> Option<f64> {
    let manifest = &built.manifest;
    if manifest.ground_cell <= 0 {
        return None;
    }
    let (col, row) = manifest.tile_at(cm(at[0]), cm(at[1]));
    let tile = built.tiles.get(&(col, row))?;
    let samples = (manifest.tile_size / manifest.ground_cell) as usize + 1;
    if tile.ground.len() != samples * samples {
        return None;
    }
    let local = |v: f64, index: u16| ((v * 100.0 + f64::from(manifest.half_extent) - f64::from(index) * f64::from(manifest.tile_size)) / f64::from(manifest.ground_cell)).round().clamp(0.0, samples as f64 - 1.0) as usize;
    Some(f64::from(tile.ground[local(at[1], row) * samples + local(at[0], col)]) / 100.0)
}

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
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
    fn a_station_runs_along_the_nearest_road_on_its_own_side_kept_back_from_it() {
        // A road along X at z = 0, 7 m wide, and a station mapped 5 m north of it (-Z), too near.
        let roads = [Road { line: vec![[-200.0, 0.0], [200.0, 0.0]], half: 3.5, service: false }];
        let laid = lot([10.0, -5.0], &roads);
        let needed = 3.5 + f64::from(STATION_ROAD_CLEAR) + f64::from(STATION_LOT[1]) / 2.0;
        assert!((laid.middle[0] - 10.0).abs() < 1e-9 && (laid.middle[1] + needed).abs() < 1e-9, "pushed back off the road: {laid:?}");
        // Its +Z, back from the road, is -Z of the world: a yaw of π.
        assert!((laid.yaw.abs() - std::f64::consts::PI).abs() < 1e-9, "{laid:?}");
        let [_, z] = laid.world([0.0, -f64::from(STATION_LOT[1]) / 2.0]);
        assert!((z + 3.5 + f64::from(STATION_ROAD_CLEAR)).abs() < 1e-9, "the front edge faces the road: {z}");
        // One mapped far enough back stays where it is, and one with no road near faces north.
        assert_eq!(lot([10.0, 40.0], &roads).middle, [10.0, 40.0]);
        assert_eq!(lot([10.0, 500.0], &roads), Lot { middle: [10.0, 500.0], yaw: 0.0 });
        // A service road counts only when nothing else is near.
        let both = [Road { line: vec![[30.0, -100.0], [30.0, 100.0]], half: 2.75, service: true }, roads[0].clone()];
        assert!(lot([25.0, 40.0], &both).yaw.abs() < 1e-9, "faces the street, not the service road");
    }

    #[test]
    fn the_station_clears_the_buildings_on_its_lot_and_keeps_the_rest() {
        let frame = Frame::default();
        let way = |id, lat: f64, lon: f64, building: &str| {
            let d = 0.00005;
            let at = |lat, lon| LatLon { lat, lon };
            Way { id, tags: tags(&[("building", building)]), geometry: vec![at(lat, lon), at(lat + d, lon), at(lat + d, lon + d), at(lat, lon + d), at(lat, lon)] }
        };
        let osm = Osm { ways: vec![way(1, 0.0, 0.0, "roof"), way(2, 0.0, 0.01, "yes"), Way { id: 3, tags: tags(&[("highway", "residential")]), geometry: vec![LatLon { lat: 0.0, lon: 0.0 }, LatLon { lat: 0.0, lon: 0.001 }] }], ..Default::default() };
        let (osm, cleared) = clear(osm, &[Lot { middle: [3.0, -3.0], yaw: 0.3 }], frame);
        assert_eq!(cleared, 1);
        assert_eq!(osm.ways.iter().map(|way| way.id).collect::<Vec<_>>(), [2, 3]);
        // A hall whose middle is well off the lot but whose end reaches over the wash bay goes too; one
        // touching the margin by a corner stays.
        let lot = Lot { middle: [0.0, 0.0], yaw: 0.0 };
        let hall = [[-10.0, 8.0], [10.0, 8.0], [10.0, 40.0], [-10.0, 40.0], [-10.0, 8.0]];
        assert!(lot.overlaps(&hall), "its middle is 24 m off, but it stands on the lot");
        let corner = [[19.5, 13.5], [30.0, 13.5], [30.0, 30.0], [19.5, 30.0]];
        assert!(!lot.overlaps(&corner), "a corner over the margin by a quarter of a square metre");
    }

    #[test]
    fn a_station_on_a_slope_levels_its_lot_to_its_paving_and_eases_back_into_the_slope() {
        // A slope rising 1 in 10 towards +X, 5 m samples over the city's 300 m.
        let side = 61u32;
        let heights = (0..side).flat_map(|_| (0..side).map(|i| i as i32 * 50)).collect();
        let slope = shared::terrain::Ground { origin: -15_000, cell: 500, side, heights };
        let built = crate::assemble::synthetic_city(150.0, 100.0, slope, Default::default());
        let station = built.manifest.fuel_stations[0];
        let lot = Lot { middle: [f64::from(station.at[0]) / 100.0, f64::from(station.at[1]) / 100.0], yaw: f64::from(station.yaw) / 1000.0 };
        let base = f64::from(station.base) / 100.0;
        let height = |x: f64, z: f64| ground(&built, lot.world([x, z])).unwrap();
        let (col, row) = built.manifest.tile_at(station.at[0], station.at[1]);
        assert!(built.tiles[&(col, row)].cover.iter().any(|cover| cover.kind == COVER_ASPHALT), "the lot is paved, so nothing grows through it");
        for [x, z] in [[-17.0, -11.0], [17.0, 11.0], [0.0, 0.0], [17.0, -11.0]] {
            assert!((height(x, z) - (base - UNDER_PAVING)).abs() < 0.06, "level under the paving at {x}, {z}: {} against {base}", height(x, z));
        }
        // Well past the apron the slope is as it was: 1 in 10.
        let far = f64::from(STATION_LOT[0]) / 2.0 + LEVEL_MARGIN + APRON + 6.0;
        let (left, right) = (height(-far, 0.0), height(far, 0.0));
        assert!(((right - left) / (2.0 * far) - 0.1).abs() < 0.01, "{left} .. {right}");
    }

    #[test]
    fn the_synthetic_city_has_one_station_standing_on_its_ground_and_a_station_off_the_world_is_left_out() {
        let mut built = crate::assemble::synthetic_city(150.0, 100.0, Default::default(), Default::default());
        assert_eq!(built.manifest.fuel_stations.len(), 1, "the offline world has one to fill up at");
        lay(&mut built, &[Lot { middle: [40.0, -60.0], yaw: 0.5 }, Lot { middle: [900.0, 0.0], yaw: 0.0 }, Lot { middle: [-151.0, 0.0], yaw: 0.0 }]);
        assert_eq!(built.manifest.fuel_stations, [FuelStation { at: [4_000, -6_000], yaw: 500, base: 0 }]);
    }
}
