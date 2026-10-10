//! Turns OSM ways (or a procedural grid) into a tiled world: a manifest and one `Tile` per
//! square of the grid that has anything in it.

use crate::overpass::Way;
use shared::world::*;
use std::collections::BTreeMap;

type P = [f64; 2];

/// Metres per degree of latitude; longitude is scaled by cos(latitude).
const METRES_PER_DEG_LAT: f64 = 110_540.0;
const METRES_PER_DEG_LON: f64 = 111_320.0;
/// Buildings and spawns keep this far inside the boundary walls, metres.
const EDGE_MARGIN: f64 = 2.0;
const MAX_SPAWNS: usize = 64;
const SPAWN_SPACING: f64 = 15.0;
/// Spawns keep this far from any wall, metres.
const SPAWN_CLEARANCE: f64 = 1.5;

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
}

/// Equirectangular projection around `origin`: +X east, +Z south.
fn project(origin: (f64, f64), lat: f64, lon: f64) -> P {
    let x = (lon - origin.1) * METRES_PER_DEG_LON * origin.0.to_radians().cos();
    let z = -(lat - origin.0) * METRES_PER_DEG_LAT;
    [x, z]
}

fn signed_area(points: &[P]) -> f64 {
    let n = points.len();
    (0..n).map(|i| {
        let [x0, z0] = points[i];
        let [x1, z1] = points[(i + 1) % n];
        x0 * z1 - x1 * z0
    }).sum::<f64>() / 2.0
}

fn point_in_polygon([x, z]: P, polygon: &[P]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let [xi, zi] = polygon[i];
        let [xj, zj] = polygon[j];
        if (zi > z) != (zj > z) && x < (xj - xi) * (z - zi) / (zj - zi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn distance_to_segment([px, pz]: P, [ax, az]: P, [bx, bz]: P) -> f64 {
    let (dx, dz) = (bx - ax, bz - az);
    let length = dx * dx + dz * dz;
    let t = if length > 0.0 { (((px - ax) * dx + (pz - az) * dz) / length).clamp(0.0, 1.0) } else { 0.0 };
    ((px - ax - t * dx).powi(2) + (pz - az - t * dz).powi(2)).sqrt()
}

/// An axis-aligned rectangle `[x0, z0, x1, z1]`, metres.
type Rect = [f64; 4];

/// Liang–Barsky: the part of segment `a → b` inside `rect`.
fn clip_segment(a: P, b: P, [x0, z0, x1, z1]: Rect) -> Option<(P, P)> {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - x0), (dx, x1 - a[0]), (-dz, a[1] - z0), (dz, z1 - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 { t0 = t0.max(r) } else { t1 = t1.min(r) }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some(([a[0] + t0 * dx, a[1] + t0 * dz], [a[0] + t1 * dx, a[1] + t1 * dz]))
}

/// Splits a polyline into the runs that lie inside the square `[-half, half]²`.
fn clip_polyline(points: &[P], half: f64) -> Vec<Vec<P>> {
    clip_polyline_rect(points, [-half, -half, half, half])
}

/// Splits a polyline into the runs that lie inside `rect`.
fn clip_polyline_rect(points: &[P], rect: Rect) -> Vec<Vec<P>> {
    let mut runs: Vec<Vec<P>> = Vec::new();
    let mut current: Vec<P> = Vec::new();
    for pair in points.windows(2) {
        match clip_segment(pair[0], pair[1], rect) {
            Some((start, end)) => {
                if current.last() != Some(&start) {
                    if current.len() >= 2 {
                        runs.push(std::mem::take(&mut current));
                    }
                    current = vec![start];
                }
                current.push(end);
            }
            None => {
                if current.len() >= 2 {
                    runs.push(std::mem::take(&mut current));
                }
                current.clear();
            }
        }
    }
    if current.len() >= 2 {
        runs.push(current);
    }
    runs
}

fn parse_metres(text: &str) -> Option<f64> {
    let number: String = text.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    number.parse::<f64>().ok().filter(|value| *value > 0.0)
}

/// Height in metres: `height`, else `building:levels` × 3, else a varied 6–12 m so blocks of
/// untagged buildings do not look like one flat slab.
fn building_height(way: &Way) -> f64 {
    let tag = |key: &str| way.tags.get(key).map(String::as_str);
    if let Some(height) = tag("height").and_then(parse_metres) {
        return height.min(200.0);
    }
    if let Some(levels) = tag("building:levels").and_then(parse_metres) {
        return (levels * 3.0 + 1.0).min(200.0);
    }
    6.0 + (way.id % 4) as f64 * 2.0
}

/// Road width in metres and kind, or `None` for ways that are not walkable roads.
fn road_style(highway: &str) -> Option<(f64, u8)> {
    Some(match highway {
        "motorway" | "trunk" | "primary" | "secondary" | "motorway_link" | "trunk_link" | "primary_link" | "secondary_link" => (10.0, ROAD_MAJOR),
        "tertiary" | "tertiary_link" | "residential" | "unclassified" | "living_street" => (7.0, ROAD_MINOR),
        "service" => (4.5, ROAD_MINOR),
        "pedestrian" => (5.0, ROAD_PATH),
        "footway" | "path" | "cycleway" | "steps" | "track" => (2.5, ROAD_PATH),
        _ => return None,
    })
}

/// Appends a closed prism (walls + roof + floor) for one building footprint to `mesh`.
fn extrude(mesh: &mut CollisionMesh, footprint: &[P], height: f64) {
    let base = mesh.vertices.len() as u32;
    let n = footprint.len() as u32;
    for &[x, z] in footprint {
        mesh.vertices.push([cm(x), 0, cm(z)]);
    }
    for &[x, z] in footprint {
        mesh.vertices.push([cm(x), cm(height), cm(z)]);
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.triangles.push([base + i, base + j, base + n + j]);
        mesh.triangles.push([base + i, base + n + j, base + n + i]);
    }
    let flat: Vec<f64> = footprint.iter().flat_map(|p| [p[0], p[1]]).collect();
    if let Ok(indices) = earcutr::earcut(&flat, &[], 2) {
        for tri in indices.chunks_exact(3) {
            let [a, b, c] = [tri[0] as u32, tri[1] as u32, tri[2] as u32];
            mesh.triangles.push([base + n + a, base + n + b, base + n + c]);
            mesh.triangles.push([base + c, base + b, base + a]);
        }
    }
}

/// Picks well-spread outdoor spawn points from road vertices.
fn pick_spawns(candidates: &[P], footprints: &[Vec<P>], half: f64) -> Vec<[i32; 2]> {
    let limit = half - EDGE_MARGIN - SPAWN_CLEARANCE;
    let clear = |p: P| {
        p[0].abs() < limit
            && p[1].abs() < limit
            && footprints.iter().all(|footprint| {
                !point_in_polygon(p, footprint)
                    && (0..footprint.len()).all(|i| distance_to_segment(p, footprint[i], footprint[(i + 1) % footprint.len()]) >= SPAWN_CLEARANCE)
            })
    };
    // Prefer points near the centre so players start together.
    let mut sorted: Vec<P> = candidates.to_vec();
    sorted.sort_by(|a, b| (a[0].hypot(a[1])).total_cmp(&b[0].hypot(b[1])));
    let mut chosen: Vec<P> = Vec::new();
    for p in sorted {
        if chosen.len() >= MAX_SPAWNS {
            break;
        }
        if chosen.iter().all(|q| (p[0] - q[0]).hypot(p[1] - q[1]) >= SPAWN_SPACING) && clear(p) {
            chosen.push(p);
        }
    }
    if chosen.is_empty() && clear([0.0, 0.0]) {
        chosen.push([0.0, 0.0]);
    }
    chosen.into_iter().map(|[x, z]| [cm(x), cm(z)]).collect()
}

/// The result of an import: the manifest (without tile hashes yet) and the tiles by `(col, row)`.
pub struct Built {
    pub manifest: Manifest,
    pub tiles: BTreeMap<(u16, u16), Tile>,
}

impl Built {
    /// Encodes every tile, records its hash in the manifest and returns `(manifest bytes, [(path, bytes)])`.
    pub fn into_files(mut self) -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
        let mut files = Vec::new();
        self.manifest.tiles.clear();
        for (&(col, row), tile) in &self.tiles {
            let bytes = tile.encode();
            let entry = TileRef { col, row, hash: world_hash(&bytes) };
            files.push((entry.path(), bytes));
            self.manifest.tiles.push(entry);
        }
        (self.manifest.encode(), files)
    }
}

/// Assembles the world from building outlines (metres, any winding) and road polylines, and cuts
/// it into square tiles of `tile_size` metres.
fn assemble(origin: (f64, f64), half: f64, tile_size: f64, buildings: Vec<(Vec<P>, f64)>, roads: Vec<(Vec<P>, f64, u8)>) -> Built {
    let inner = half - EDGE_MARGIN;
    let mut manifest = Manifest { version: WORLD_VERSION, origin, half_extent: cm(half), tile_size: cm(tile_size), ..Default::default() };
    let mut tiles: BTreeMap<(u16, u16), Tile> = BTreeMap::new();
    let mut footprints = Vec::new();
    for (mut footprint, height) in buildings {
        if footprint.len() > 1 && footprint.first() == footprint.last() {
            footprint.pop();
        }
        footprint.dedup();
        if footprint.len() < 3 || footprint.iter().any(|p| p[0].abs() > inner || p[1].abs() > inner) {
            continue;
        }
        let area = signed_area(&footprint);
        if area.abs() < 4.0 {
            continue;
        }
        if area < 0.0 {
            footprint.reverse();
        }
        // A building belongs to the tile its centre is in, whole, so a tile never needs its neighbour.
        let n = footprint.len() as f64;
        let centre = footprint.iter().fold([0.0, 0.0], |sum, p| [sum[0] + p[0] / n, sum[1] + p[1] / n]);
        let tile = tiles.entry(manifest.tile_at(cm(centre[0]), cm(centre[1]))).or_default();
        extrude(&mut tile.collision, &footprint, height);
        tile.buildings.push(Building { height: cm(height), footprint: footprint.iter().map(|&[x, z]| [cm(x), cm(z)]).collect() });
        footprints.push(footprint);
    }
    let mut candidates = Vec::new();
    let side = tile_size;
    for (points, width, kind) in roads {
        for run in clip_polyline(&points, half) {
            // Sample along each run, not just at vertices, so long straight roads still offer spawns.
            for pair in run.windows(2) {
                let ([ax, az], [bx, bz]) = (pair[0], pair[1]);
                let steps = ((bx - ax).hypot(bz - az) / 8.0).ceil().max(1.0) as usize;
                candidates.extend((0..=steps).map(|s| {
                    let t = s as f64 / steps as f64;
                    [ax + (bx - ax) * t, az + (bz - az) * t]
                }));
            }
            // A road crossing tile borders is cut at them: each tile holds only its own pieces.
            let (min, max) = run.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
            let (first, last) = (manifest.tile_at(cm(min[0]), cm(min[1])), manifest.tile_at(cm(max[0]), cm(max[1])));
            for col in first.0..=last.0 {
                for row in first.1..=last.1 {
                    let rect = [-half + col as f64 * side, -half + row as f64 * side, -half + (col + 1) as f64 * side, -half + (row + 1) as f64 * side];
                    for piece in clip_polyline_rect(&run, rect) {
                        tiles.entry((col, row)).or_default().roads.push(Road {
                            width: cm(width),
                            kind,
                            points: piece.iter().map(|&[x, z]| [cm(x), cm(z)]).collect(),
                        });
                    }
                }
            }
        }
    }
    manifest.spawns = pick_spawns(&candidates, &footprints, half);
    Built { manifest, tiles }
}

/// The bounding box of the `half`-metre square around `(lat, lon)`, as `[south, west, north, east]`.
pub fn bbox_around(lat: f64, lon: f64, half: f64) -> [f64; 4] {
    let dlat = half / METRES_PER_DEG_LAT;
    let dlon = half / (METRES_PER_DEG_LON * lat.to_radians().cos());
    [lat - dlat, lon - dlon, lat + dlat, lon + dlon]
}

pub fn from_osm([south, west, north, east]: [f64; 4], tile_size: f64, ways: &[Way]) -> Built {
    let origin = ((south + north) / 2.0, (west + east) / 2.0);
    let [x0, z0] = project(origin, south, west);
    let [x1, z1] = project(origin, north, east);
    let half = ((x1 - x0).abs().min((z1 - z0).abs()) / 2.0).round();
    let mut buildings = Vec::new();
    let mut roads = Vec::new();
    for way in ways {
        let points: Vec<P> = way.geometry.iter().map(|g| project(origin, g.lat, g.lon)).collect();
        if way.tags.get("building").is_some_and(|value| value != "no" && value != "roof") {
            buildings.push((points, building_height(way)));
        } else if let Some((width, kind)) = way.tags.get("highway").and_then(|value| road_style(value)) {
            roads.push((points, width, kind));
        }
    }
    assemble(origin, half, tile_size, buildings, roads)
}

/// An offline grid city: 48 m blocks separated by 10 m streets, each block holding a few
/// buildings of varied height around a courtyard.
pub fn synthetic_city(half: f64, tile_size: f64) -> Built {
    const PITCH: f64 = 58.0;
    const STREET: f64 = 10.0;
    let mut buildings = Vec::new();
    let mut roads = Vec::new();
    let count = (half * 2.0 / PITCH).floor() as i32;
    let start = -(count as f64) * PITCH / 2.0;
    let mut seed: u64 = 0x2545_f491;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 1000) as f64 / 1000.0
    };
    for i in 0..=count {
        let line = start + i as f64 * PITCH;
        roads.push((vec![[line, -half], [line, half]], STREET, ROAD_MINOR));
        roads.push((vec![[-half, line], [half, line]], STREET, ROAD_MINOR));
    }
    for i in 0..count {
        for j in 0..count {
            let (bx, bz) = (start + i as f64 * PITCH + STREET / 2.0 + 2.0, start + j as f64 * PITCH + STREET / 2.0 + 2.0);
            let size = PITCH - STREET - 4.0;
            if next() < 0.15 {
                continue; // a little park
            }
            let half_size = size / 2.0 - 1.0;
            for (ox, oz) in [(0.0, 0.0), (size / 2.0 + 1.0, 0.0), (0.0, size / 2.0 + 1.0), (size / 2.0 + 1.0, size / 2.0 + 1.0)] {
                if next() < 0.2 {
                    continue;
                }
                let (x, z) = (bx + ox, bz + oz);
                let height = 5.0 + (next() * 5.0).floor() * 3.0;
                buildings.push((vec![[x, z], [x + half_size, z], [x + half_size, z + half_size], [x, z + half_size]], height));
            }
        }
    }
    assemble((0.0, 0.0), half, tile_size, buildings, roads)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_is_metric() {
        let [x, z] = project((60.0, 18.0), 60.001, 18.002);
        assert!((x - 111.32).abs() < 0.5, "x = {x}");
        assert!((z + 110.54).abs() < 0.5, "z = {z}");
    }

    #[test]
    fn clips_polylines_to_the_square() {
        let runs = clip_polyline(&[[-20.0, 0.0], [20.0, 0.0], [20.0, 30.0], [-20.0, 30.0]], 10.0);
        assert_eq!(runs, vec![vec![[-10.0, 0.0], [10.0, 0.0]]]);
    }

    #[test]
    fn extrudes_closed_prism() {
        let mut mesh = CollisionMesh::default();
        extrude(&mut mesh, &[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]], 3.0);
        assert_eq!(mesh.vertices.len(), 8);
        // 4 walls × 2 + roof 2 + floor 2.
        assert_eq!(mesh.triangles.len(), 12);
    }

    #[test]
    fn spawns_avoid_buildings() {
        let built = synthetic_city(150.0, 100.0);
        assert!(!built.manifest.spawns.is_empty());
        for &[x, z] in &built.manifest.spawns {
            let p = [x as f64 / 100.0, z as f64 / 100.0];
            for building in built.tiles.values().flat_map(|tile| &tile.buildings) {
                let footprint: Vec<P> = building.footprint.iter().map(|&[x, z]| [x as f64 / 100.0, z as f64 / 100.0]).collect();
                assert!(!point_in_polygon(p, &footprint));
            }
        }
    }

    #[test]
    fn every_building_and_road_piece_sits_in_its_own_tile() {
        let built = synthetic_city(300.0, 100.0);
        assert!(built.tiles.len() > 4, "a 600 m city in 100 m tiles has many tiles");
        for (&(col, row), tile) in &built.tiles {
            for building in &tile.buildings {
                let n = building.footprint.len() as i64;
                let cx = (building.footprint.iter().map(|p| p[0] as i64).sum::<i64>() / n) as i32;
                let cz = (building.footprint.iter().map(|p| p[1] as i64).sum::<i64>() / n) as i32;
                assert_eq!(built.manifest.tile_at(cx, cz), (col, row));
            }
            let (x0, z0) = (-built.manifest.half_extent + col as i32 * built.manifest.tile_size, -built.manifest.half_extent + row as i32 * built.manifest.tile_size);
            for road in &tile.roads {
                for &[x, z] in &road.points {
                    assert!((x0 - 1..=x0 + built.manifest.tile_size + 1).contains(&x), "x {x} outside tile {col}_{row}");
                    assert!((z0 - 1..=z0 + built.manifest.tile_size + 1).contains(&z), "z {z} outside tile {col}_{row}");
                }
            }
        }
        // Tiling loses nothing: merging them back gives every building the single-tile import had.
        let whole = synthetic_city(300.0, 1000.0);
        let count = |b: &Built| b.tiles.values().map(|t| t.buildings.len()).sum::<usize>();
        assert_eq!(count(&built), count(&whole));
    }

    #[test]
    fn bbox_is_a_square_of_the_asked_half_extent() {
        let [south, west, north, east] = bbox_around(63.832127, 20.168286, 1000.0);
        let origin = ((south + north) / 2.0, (west + east) / 2.0);
        let [x0, z0] = project(origin, south, west);
        let [x1, z1] = project(origin, north, east);
        assert!(((x1 - x0) / 2.0 - 1000.0).abs() < 0.01);
        assert!(((z0 - z1) / 2.0 - 1000.0).abs() < 0.01);
    }

    #[test]
    fn heights_prefer_tags() {
        let way = |tags: &[(&str, &str)]| Way { id: 3, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), geometry: vec![] };
        assert_eq!(building_height(&way(&[("height", "12.5 m")])), 12.5);
        assert_eq!(building_height(&way(&[("building:levels", "4")])), 13.0);
        assert_eq!(building_height(&way(&[])), 12.0);
    }
}
