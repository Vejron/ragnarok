//! Turns OSM ways (or a procedural grid) into a tiled world: a manifest and one `Tile` per
//! square of the grid that has anything in it.

use crate::bridge::{self, Pylons};
use crate::overpass::{Area, LatLon, Osm, Way};
use crate::roof::{self, Cloud, Roof, Shape};
use crate::trees::Planted;
use crate::water::{Placed, STREAM_WIDTH};
use shared::terrain::Ground;
use shared::world::*;
use std::collections::BTreeMap;

type P = [f64; 2];

use shared::world::METRES_PER_DEG_LAT;
/// Buildings and spawns keep this far inside the boundary walls, metres.
const EDGE_MARGIN: f64 = 2.0;
const MAX_SPAWNS: usize = 64;
const SPAWN_SPACING: f64 = 15.0;
/// Spawns keep this far from any wall, metres.
const SPAWN_CLEARANCE: f64 = 1.5;
/// Trees this near a road or building have trunk colliders; those deeper in the woods do not.
const SOLID_REACH: f64 = 20.0;
/// No tree stands this close to the edge of a bridge, metres.
const BRIDGE_CLEAR: f64 = 4.0;

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
}

/// Equirectangular projection around `origin`: +X east, +Z south.
pub fn project(origin: (f64, f64), lat: f64, lon: f64) -> P {
    shared::world::project(origin, lat, lon)
}

fn signed_area(points: &[P]) -> f64 {
    let n = points.len();
    (0..n).map(|i| {
        let [x0, z0] = points[i];
        let [x1, z1] = points[(i + 1) % n];
        x0 * z1 - x1 * z0
    }).sum::<f64>() / 2.0
}

pub(crate) fn point_in_polygon([x, z]: P, polygon: &[P]) -> bool {
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
///
/// `ground` is the terrain over the whole grid of tiles (see `crate::ground`), or flat.
/// `trees` are `(position, TREE_*, height, crown radius, planted)`; those that fall on a building or
/// a road are dropped. A tree is solid (has a trunk collider) when it is planted, in the register,
/// or stands within `SOLID_REACH` of a road or a building.
#[allow(clippy::too_many_arguments)]
fn assemble(origin: (f64, f64), half: f64, tile_size: f64, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, buildings: Vec<(Vec<P>, f64, Option<Roof>)>, roads: Vec<(Vec<P>, f64, u8)>, bridges: Vec<(Vec<P>, f64, u8, Pylons)>, covers: Vec<(u8, Vec<Vec<P>>)>, trees: Vec<(P, u8, f64, f64, bool)>, ground: Ground, water: Placed) -> Built {
    let inner = half - EDGE_MARGIN;
    let mut manifest = Manifest {
        version: WORLD_VERSION,
        origin,
        half_extent: cm(half),
        tile_size: cm(tile_size),
        ground_cell: ground.cell.max(0),
        ..Default::default()
    };
    let mut tiles: BTreeMap<(u16, u16), Tile> = BTreeMap::new();
    let mut footprints = Vec::new();
    for (mut footprint, height, fitted) in buildings {
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
        // The floor is the lowest ground under the building, so no wall hangs in the air.
        let floor = footprint.iter().chain([&centre]).map(|p| ground.height_at(p[0] as f32, p[1] as f32) as f64).fold(f64::MAX, f64::min);
        let tile = tiles.entry(manifest.tile_at(cm(centre[0]), cm(centre[1]))).or_default();
        // Without laser data a building is a box of its tagged (or guessed) height.
        let shape = fitted.unwrap_or(Roof::Leaf(Shape::Flat { y: floor + height }));
        let first = tile.collision.triangles.len() as u32;
        let parts = roof::build(&mut tile.collision, &footprint, floor, &shape);
        tile.buildings.push(Building {
            height: cm(parts.top - floor),
            footprint: footprint.iter().map(|&[x, z]| [cm(x), cm(z)]).collect(),
            base: cm(floor),
            first,
            walls: parts.walls,
            roof: parts.roof,
            roof_kind: shape.kind(),
        });
        footprints.push(footprint);
    }
    let mut candidates = Vec::new();
    let side = tile_size;
    // Bridge roads are drawn like any road (and on the minimap) but never offer a spawn: the
    // ground under them is a river bed.
    for ((points, width, kind), spawnable) in roads.iter().map(|(points, width, kind)| ((points, width, kind), true)).chain(bridges.iter().map(|(points, width, kind, _)| ((points, width, kind), false))) {
        let (width, kind) = (*width, *kind);
        for run in clip_polyline(points, half) {
            // Sample along each run, not just at vertices, so long straight roads still offer spawns.
            for pair in run.windows(2).filter(|_| spawnable) {
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
    // Bridges are solid geometry in the collision mesh, each piece in the tile it lies in.
    for (points, width, kind, pylons) in &bridges {
        let pieces = bridge::build(points, *width, *kind, *pylons, &ground, &water.levels, inner);
        // Per tile: the deck pieces and the steel pieces that fall in it.
        type TilePieces<'a> = (Vec<&'a bridge::Piece>, Vec<&'a bridge::Piece>);
        let mut by_tile: BTreeMap<(u16, u16), TilePieces> = BTreeMap::new();
        for piece in &pieces {
            let entry = by_tile.entry(manifest.tile_at(cm(piece.at[0]), cm(piece.at[1]))).or_default();
            if piece.steel { entry.1.push(piece) } else { entry.0.push(piece) }
        }
        for (key, (deck, steel)) in by_tile {
            let tile = tiles.entry(key).or_default();
            let first = tile.collision.triangles.len() as u32;
            let mut shared_vertices: std::collections::HashMap<[i32; 3], u32> = std::collections::HashMap::new();
            let mut count = [0u32; 2];
            for (group, list) in [&deck, &steel].into_iter().enumerate() {
                for piece in list {
                    for tri in &piece.tris {
                        let mut index = [0u32; 3];
                        for (slot, v) in tri.iter().enumerate() {
                            let at = [cm(v[0]), cm(v[1]), cm(v[2])];
                            index[slot] = *shared_vertices.entry(at).or_insert_with(|| {
                                tile.collision.vertices.push(at);
                                tile.collision.vertices.len() as u32 - 1
                            });
                        }
                        if index[0] != index[1] && index[1] != index[2] && index[0] != index[2] {
                            tile.collision.triangles.push(index);
                            count[group] += 1;
                        }
                    }
                }
            }
            tile.bridges.push(Bridge { first, deck: count[0], steel: count[1] });
        }
    }
    // Cover is cut at tile borders like roads, so each tile's texture needs only its own file.
    // Rocks and shrubs the map does not record go through the same rejection as every tree.
    let props = crate::props::scatter(&covers, inner);
    for (kind, rings) in covers {
        let (min, max) = rings.iter().flatten().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        if min[0] > max[0] {
            continue;
        }
        let (first, last) = (manifest.tile_at(cm(min[0]), cm(min[1])), manifest.tile_at(cm(max[0]), cm(max[1])));
        for col in first.0..=last.0 {
            for row in first.1..=last.1 {
                let rect = [-half + col as f64 * side, -half + row as f64 * side, -half + (col + 1) as f64 * side, -half + (row + 1) as f64 * side];
                let clipped: Vec<Vec<[i32; 2]>> = rings
                    .iter()
                    .map(|ring| clip_ring(ring, rect))
                    .filter(|ring| ring.len() >= 3)
                    .map(|ring| ring.iter().map(|&[x, z]| [cm(x), cm(z)]).collect())
                    .collect();
                if !clipped.is_empty() {
                    tiles.entry((col, row)).or_default().cover.push(Cover { kind, rings: clipped });
                }
            }
        }
    }
    // Streams are cut at tile borders like roads, keeping the direction of the flow.
    for line in &water.streams {
        for run in clip_polyline(line, half) {
            let (min, max) = run.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
            let (first, last) = (manifest.tile_at(cm(min[0]), cm(min[1])), manifest.tile_at(cm(max[0]), cm(max[1])));
            for col in first.0..=last.0 {
                for row in first.1..=last.1 {
                    let rect = [-half + col as f64 * side, -half + row as f64 * side, -half + (col + 1) as f64 * side, -half + (row + 1) as f64 * side];
                    for piece in clip_polyline_rect(&run, rect) {
                        tiles.entry((col, row)).or_default().streams.push(Stream {
                            width: cm(STREAM_WIDTH),
                            points: piece.iter().map(|&[x, z]| [cm(x), cm(z)]).collect(),
                        });
                    }
                }
            }
        }
    }
    for tile in tiles.values_mut() {
        tile.cover.sort_by_key(|cover| cover_rank(cover.kind));
    }
    // Roads and footprints with their bounding boxes grown by the reach, so a tree is only measured
    // against the few that are near it.
    let reach_box = |points: &[P], grow: f64| {
        points.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - grow), b[1].min(p[1] - grow), b[2].max(p[0] + grow), b[3].max(p[1] + grow)])
    };
    let in_box = |b: &[f64; 4], p: P| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
    // Trees near a bridge are dropped: over water they are the laser's reading of pylons, cables
    // and railings, and elsewhere they would grow through the deck.
    let bridge_boxes: Vec<[f64; 4]> = bridges.iter().map(|(points, width, _, _)| reach_box(points, width / 2.0 + BRIDGE_CLEAR)).collect();
    let road_boxes: Vec<[f64; 4]> = roads.iter().map(|(points, width, _)| reach_box(points, width / 2.0 + SOLID_REACH)).collect();
    let footprint_boxes: Vec<[f64; 4]> = footprints.iter().map(|footprint| reach_box(footprint, SOLID_REACH)).collect();
    for (at, kind, height, crown, planted) in trees.into_iter().chain(props) {
        if at[0].abs() > inner || at[1].abs() > inner {
            continue;
        }
        let on_building = footprints.iter().zip(&footprint_boxes).any(|(footprint, b)| in_box(b, at) && point_in_polygon(at, footprint));
        let mut on_road = false;
        let mut near = planted;
        for ((points, width, _), b) in roads.iter().zip(&road_boxes) {
            if !in_box(b, at) {
                continue;
            }
            let d = points.windows(2).map(|pair| distance_to_segment(at, pair[0], pair[1])).fold(f64::MAX, f64::min);
            on_road |= d < width / 2.0;
            near |= d < width / 2.0 + SOLID_REACH;
        }
        let on_bridge = bridges.iter().zip(&bridge_boxes).any(|((points, width, _, _), b)| {
            in_box(b, at) && points.windows(2).any(|pair| distance_to_segment(at, pair[0], pair[1]) < width / 2.0 + BRIDGE_CLEAR)
        });
        if on_building || on_road || on_bridge {
            continue;
        }
        if !near {
            near = footprints.iter().zip(&footprint_boxes).any(|(footprint, b)| {
                in_box(b, at) && (0..footprint.len()).any(|i| distance_to_segment(at, footprint[i], footprint[(i + 1) % footprint.len()]) < SOLID_REACH)
            });
        }
        let tile = tiles.entry(manifest.tile_at(cm(at[0]), cm(at[1]))).or_default();
        tile.trees.push(Tree {
            x: cm(at[0]),
            z: cm(at[1]),
            base: cm(ground.height_at(at[0] as f32, at[1] as f32) as f64),
            height: cm(height),
            crown: cm(crown),
            kind,
            // A shrub is something to walk through, wherever it stands.
            solid: near && kind != TREE_BUSH,
        });
    }
    // The API gives no stable order, and tile files are hashed.
    for tile in tiles.values_mut() {
        tile.trees.sort_by_key(|tree| (tree.x, tree.z, tree.kind));
    }
    // Only the world's own tiles are kept, and only they offer spawns.
    let inside = |col: u16, row: u16| mask.is_none_or(|mask| mask.contains(&(col, row)));
    if mask.is_some() {
        candidates.retain(|p| {
            let (col, row) = manifest.tile_at(cm(p[0]), cm(p[1]));
            inside(col, row)
        });
    }
    manifest.spawns = pick_spawns(&candidates, &footprints, half);
    if manifest.spawns.is_empty()
        && let Some(mask) = mask
        && let Some(&(col, row)) = mask.iter().next()
    {
        // No road to spawn on: the middle of the first tile.
        let at = |i: u16| -half + (f64::from(i) + 0.5) * tile_size;
        manifest.spawns.push([cm(at(col)), cm(at(row))]);
    }
    // Every tile carries its piece of the ground, so every tile of the grid exists.
    let samples = manifest.tile_samples();
    if samples > 0 {
        let per_tile = samples - 1;
        for col in 0..manifest.grid() {
            for row in (0..manifest.grid()).filter(|&row| inside(col, row)) {
                tiles.entry((col, row)).or_default().ground = ground.block(col as u32 * per_tile, row as u32 * per_tile, samples);
            }
        }
        // A tile with no water at all carries no water samples.
        if water.levels.len() == ground.heights.len() {
            let all = Ground { origin: ground.origin, cell: ground.cell, side: ground.side, heights: water.levels };
            for col in 0..manifest.grid() {
                for row in (0..manifest.grid()).filter(|&row| inside(col, row)) {
                    let block = all.block(col as u32 * per_tile, row as u32 * per_tile, samples);
                    if block.iter().any(|&level| level != NO_WATER) {
                        tiles.entry((col, row)).or_default().water = block;
                    }
                }
            }
        }
    }
    if let Some(mask) = mask {
        tiles.retain(|key, _| mask.contains(key));
        // A painted tile with nothing in it, on flat ground, still exists.
        for &(col, row) in mask {
            tiles.entry((col, row)).or_default();
        }
    }
    Built { manifest, tiles }
}

/// The bounding box of the `half`-metre square around `(lat, lon)`, as `[south, west, north, east]`.
pub fn bbox_around(lat: f64, lon: f64, half: f64) -> [f64; 4] {
    let dlat = half / METRES_PER_DEG_LAT;
    let _ = lat;
    let dlon = half / shared::world::metres_per_deg_lon();
    [lat - dlat, lon - dlon, lat + dlat, lon + dlon]
}

/// The world's origin (lat, lon) and half-extent in metres for `bbox`: the largest square around
/// its centre.
pub fn frame([south, west, north, east]: [f64; 4]) -> ((f64, f64), f64) {
    let origin = ((south + north) / 2.0, (west + east) / 2.0);
    let [x0, z0] = project(origin, south, west);
    let [x1, z1] = project(origin, north, east);
    (origin, ((x1 - x0).abs().min((z1 - z0).abs()) / 2.0).round())
}

/// Local metres `(x, z)` to degrees `(lat, lon)`, the inverse of the projection used for OSM.
pub fn unproject(origin: (f64, f64), x: f64, z: f64) -> (f64, f64) {
    (origin.0 - z / METRES_PER_DEG_LAT, origin.1 + x / shared::world::metres_per_deg_lon())
}

#[allow(clippy::too_many_arguments)]
pub fn from_osm(bbox: [f64; 4], tile_size: f64, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, osm: &Osm, planted: &[Planted], ground: Ground, water: Placed, lidar: Option<&Cloud>) -> Built {
    let (origin, half) = frame(bbox);
    let mut buildings = Vec::new();
    let mut roads = Vec::new();
    let mut bridges = Vec::new();
    let mut covers = Vec::new();
    let line = |geometry: &[LatLon]| -> Vec<P> { geometry.iter().map(|g| project(origin, g.lat, g.lon)).collect() };
    for Area { tags, outer, inner, .. } in &osm.areas {
        let Some(kind) = cover_kind(tags) else { continue };
        let rings: Vec<Vec<P>> = join_rings(outer.iter().map(|w| line(w)).collect()).into_iter().chain(join_rings(inner.iter().map(|w| line(w)).collect())).collect();
        if !rings.is_empty() {
            covers.push((kind, rings));
        }
    }
    let is_building = |way: &crate::overpass::Way| way.tags.get("building").is_some_and(|value| value != "no" && value != "roof");
    let mut shapes = lidar.map(|cloud| fit_roofs(cloud, origin, half, tile_size, mask, osm, &line, is_building)).unwrap_or_default().into_iter();
    crate::progress::stage("buildings and roads");
    for (index, way) in osm.ways.iter().enumerate() {
        crate::progress::steps(index, osm.ways.len());
        let points = line(&way.geometry);
        if is_building(way) {
            // Roofs come from the laser returns over the footprint, where there are any.
            let shape = shapes.next().flatten();
            buildings.push((points, building_height(way), shape));
        } else if let Some((width, kind)) = way.tags.get("highway").and_then(|value| road_style(value)) {
            if is_bridge(way) {
                let pylons = pylons_for(origin, &points);
                bridges.push((points, width, kind, pylons));
            } else {
                roads.push((points, width, kind));
            }
        } else if let Some(kind) = cover_kind(&way.tags) {
            // Only closed ways are areas; an open one is a bank or a fence.
            if points.len() >= 4 && points.first() == points.last() {
                covers.push((kind, vec![points[..points.len() - 1].to_vec()]));
            }
        }
    }
    if lidar.is_some() {
        let split = buildings.iter().filter(|(.., shape)| matches!(shape, Some(Roof::Cut { .. }))).count();
        eprintln!("roofs: {split} of {} buildings have more than one part", buildings.len());
    }
    let trees = planted.iter().map(|tree| (project(origin, tree.lat, tree.lon), tree.kind, tree.height, tree.crown, tree.registered)).collect();
    crate::progress::stage("tiles");
    assemble(origin, half, tile_size, mask, buildings, roads, bridges, covers, trees, ground, water)
}

/// The fitted roof of every building way, in the order of the ways. Buildings are independent, so
/// they are fitted on all cores; one that lies wholly outside the painted tiles is not fitted, as
/// the tiles drop it anyway.
#[allow(clippy::too_many_arguments)]
fn fit_roofs(
    cloud: &Cloud,
    origin: (f64, f64),
    half: f64,
    tile_size: f64,
    mask: Option<&std::collections::BTreeSet<(u16, u16)>>,
    osm: &Osm,
    line: &(impl Fn(&[LatLon]) -> Vec<P> + Sync),
    is_building: impl Fn(&crate::overpass::Way) -> bool,
) -> Vec<Option<Roof>> {
    crate::progress::stage("roofs");
    let ways: Vec<&crate::overpass::Way> = osm.ways.iter().filter(|way| is_building(way)).collect();
    let painted = |ring: &[P]| {
        let Some(mask) = mask else { return true };
        let (lo, hi) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let span = |a: f64, b: f64| ((a + half) / tile_size).floor() as i64..=((b + half) / tile_size).floor() as i64;
        span(lo[0], hi[0]).any(|c| span(lo[1], hi[1]).any(|r| c >= 0 && r >= 0 && mask.contains(&(c as u16, r as u16))))
    };
    let fit = |way: &&crate::overpass::Way| {
        let points = line(&way.geometry);
        let ring = if points.len() > 1 && points.first() == points.last() { &points[..points.len() - 1] } else { &points[..] };
        if ring.len() < 3 || !painted(ring) {
            return None;
        }
        let returns = cloud.over(ring, |x, z| unproject(origin, x, z));
        roof::fit(ring, &returns)
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Option<Roof>>>> = ways.iter().map(|_| Default::default()).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(way) = ways.get(at) else { break };
                    *slots[at].lock().unwrap() = Some(fit(way));
                    crate::progress::steps(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed), ways.len());
                }
            });
        }
    });
    slots.into_iter().map(|slot| slot.into_inner().unwrap().expect("every building was taken")).collect()
}

/// The pylons a bridge is built with: those of the landmark it lies at, if any.
fn pylons_for(origin: (f64, f64), points: &[P]) -> Pylons {
    let n = points.len().max(1) as f64;
    let mid = [points.iter().map(|p| p[0]).sum::<f64>() / n, points.iter().map(|p| p[1]).sum::<f64>() / n];
    bridge::LANDMARKS
        .iter()
        .find(|(lat, lon, radius, _)| {
            let at = project(origin, *lat, *lon);
            (at[0] - mid[0]).hypot(at[1] - mid[1]) <= *radius
        })
        .map_or(Pylons::None, |landmark| landmark.3)
}

/// Whether a road way is carried on a bridge (and not, say, a tunnel with a stray bridge tag).
fn is_bridge(way: &Way) -> bool {
    let tag = |key: &str| way.tags.get(key).map(String::as_str);
    tag("bridge").is_some_and(|value| value != "no") && tag("tunnel").is_none_or(|value| value == "no")
}

/// Ground cover for a way's or relation's tags, or `None` if it is not cover we draw. Water is not
/// cover: it comes from Hydrografi (`water.rs`).
fn cover_kind(tags: &std::collections::HashMap<String, String>) -> Option<u8> {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    if let Some(value) = tag("natural") {
        return match value {
            "wood" => Some(COVER_WOOD),
            "scrub" | "heath" | "grassland" => Some(COVER_PARK),
            "wetland" => Some(COVER_FARM),
            "sand" | "beach" => Some(COVER_SAND),
            _ => None,
        };
    }
    if let Some(value) = tag("leisure") {
        return matches!(value, "park" | "garden" | "pitch" | "playground" | "golf_course" | "recreation_ground" | "nature_reserve").then_some(COVER_PARK);
    }
    match tag("landuse")? {
        "forest" => Some(COVER_WOOD),
        "meadow" | "grass" | "village_green" | "recreation_ground" | "cemetery" | "allotments" | "greenfield" | "flowerbed" => Some(COVER_PARK),
        "farmland" | "farmyard" | "orchard" | "vineyard" | "plant_nursery" => Some(COVER_FARM),
        "residential" | "commercial" | "retail" | "garages" | "education" | "institutional" => Some(COVER_URBAN),
        "industrial" | "railway" | "quarry" | "construction" | "brownfield" | "landfill" | "depot" => Some(COVER_INDUSTRIAL),
        _ => None,
    }
}

/// Draw order: ground uses first, then green, water on top. The client paints in list order.
fn cover_rank(kind: u8) -> u8 {
    match kind {
        COVER_URBAN | COVER_INDUSTRIAL | COVER_FARM => 0,
        COVER_WOOD | COVER_PARK | COVER_SAND => 1,
        _ => 2,
    }
}

/// Joins open polylines that share end points into closed rings; pieces that never close are dropped.
fn join_rings(mut pieces: Vec<Vec<P>>) -> Vec<Vec<P>> {
    let mut rings = Vec::new();
    while let Some(mut ring) = pieces.pop() {
        loop {
            if ring.len() > 2 && ring.first() == ring.last() {
                ring.pop();
                rings.push(ring);
                break;
            }
            let end = *ring.last().unwrap();
            let Some(at) = pieces.iter().position(|p| p.first() == Some(&end) || p.last() == Some(&end)) else { break };
            let mut next = pieces.swap_remove(at);
            if next.first() != Some(&end) {
                next.reverse();
            }
            ring.extend(next.into_iter().skip(1));
        }
    }
    rings
}

/// Sutherland–Hodgman: the part of the polygon `ring` inside `rect`. May run along the rect's
/// edges where the polygon leaves it, which is invisible under a fill.
fn clip_ring(ring: &[P], [x0, z0, x1, z1]: Rect) -> Vec<P> {
    // Each edge: is the point inside, and where does a segment cross it.
    type Edge = (fn(P, Rect) -> bool, fn(P, P, Rect) -> P);
    fn at_x(a: P, b: P, x: f64) -> P {
        [x, a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0])]
    }
    fn at_z(a: P, b: P, z: f64) -> P {
        [a[0] + (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]), z]
    }
    let edges: [Edge; 4] = [
        (|p, r| p[0] >= r[0], |a, b, r| at_x(a, b, r[0])),
        (|p, r| p[0] <= r[2], |a, b, r| at_x(a, b, r[2])),
        (|p, r| p[1] >= r[1], |a, b, r| at_z(a, b, r[1])),
        (|p, r| p[1] <= r[3], |a, b, r| at_z(a, b, r[3])),
    ];
    let rect = [x0, z0, x1, z1];
    let mut points = ring.to_vec();
    for (inside, cross) in edges {
        if points.is_empty() {
            break;
        }
        let input = std::mem::take(&mut points);
        let mut previous = *input.last().unwrap();
        for &current in &input {
            match (inside(current, rect), inside(previous, rect)) {
                (true, true) => points.push(current),
                (true, false) => points.extend([cross(previous, current, rect), current]),
                (false, true) => points.push(cross(previous, current, rect)),
                (false, false) => {}
            }
            previous = current;
        }
    }
    points
}

const SYNTHETIC_PITCH: f64 = 58.0;
const SYNTHETIC_STREET: f64 = 10.0;

/// The water of `synthetic_city`: the last column of blocks is left to a pond and the stream
/// that runs out of it, so the offline world has water to draw.
pub fn synthetic_water(half: f64) -> crate::water::Map {
    use crate::water::{Body, Kind, Map};
    let count = (half * 2.0 / SYNTHETIC_PITCH).floor() as i32;
    let start = -(count as f64) * SYNTHETIC_PITCH / 2.0;
    let size = SYNTHETIC_PITCH - SYNTHETIC_STREET - 4.0;
    let x = start + (count - 1) as f64 * SYNTHETIC_PITCH + SYNTHETIC_STREET / 2.0 + 2.0 + size / 2.0;
    let top = -half + 70.0;
    let pond: Vec<P> = (0..16).map(|k| {
        let a = k as f64 / 16.0 * std::f64::consts::TAU;
        [x + 20.0 * a.cos(), top + 16.0 * a.sin()]
    }).collect();
    let stream: Vec<P> = (0..40).map(|k| [x + 10.0 * (k as f64 * 0.5).sin(), top + 14.0 + k as f64 * 12.0]).collect();
    Map { bodies: vec![Body { kind: Kind::Lake, rings: vec![pond], level: None }], streams: vec![stream] }
}

/// An offline grid city: 48 m blocks separated by 10 m streets, each block holding a few
/// buildings of varied height around a courtyard.
pub fn synthetic_city(half: f64, tile_size: f64, ground: Ground, water: Placed) -> Built {
    const PITCH: f64 = SYNTHETIC_PITCH;
    const STREET: f64 = SYNTHETIC_STREET;
    let mut buildings = Vec::new();
    let mut roads = Vec::new();
    let mut covers = Vec::new();
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
    // The last column of blocks is the pond and its stream (`synthetic_water`).
    for i in 0..count - 1 {
        for j in 0..count {
            let (bx, bz) = (start + i as f64 * PITCH + STREET / 2.0 + 2.0, start + j as f64 * PITCH + STREET / 2.0 + 2.0);
            let size = PITCH - STREET - 4.0;
            if next() < 0.15 {
                // A little park.
                covers.push((COVER_PARK, vec![vec![[bx, bz], [bx + size, bz], [bx + size, bz + size], [bx, bz + size]]]));
                continue;
            }
            let half_size = size / 2.0 - 1.0;
            for (ox, oz) in [(0.0, 0.0), (size / 2.0 + 1.0, 0.0), (0.0, size / 2.0 + 1.0), (size / 2.0 + 1.0, size / 2.0 + 1.0)] {
                if next() < 0.2 {
                    continue;
                }
                let (x, z) = (bx + ox, bz + oz);
                let height = 5.0 + (next() * 5.0).floor() * 3.0;
                // Every other house gets a pitched roof, so the offline world has roofs to draw.
                let roof = (next() < 0.5).then(|| {
                    let at = [x + half_size / 2.0, z + half_size / 2.0];
                    let eaves = ground.height_at(at[0] as f32, at[1] as f32) as f64 + height;
                    let ridge = if next() < 0.5 { [1.0, 0.0] } else { [0.0, 1.0] };
                    let top = eaves + half_size / 2.0 * 0.6;
                    Roof::Leaf(if next() < 0.5 {
                        Shape::Gable { y: top, at, ridge, pitch: 0.6 }
                    } else {
                        Shape::Hip { y: top, at, ridge, pitch: 0.6, half: 0.0 }
                    })
                });
                buildings.push((vec![[x, z], [x + half_size, z], [x + half_size, z + half_size], [x, z + half_size]], height, roof));
            }
        }
    }
    // A few trees in the parks and along the streets, so the offline world has some to draw.
    let mut trees = Vec::new();
    for i in 0..=count {
        for j in 0..count {
            let line = start + i as f64 * PITCH;
            let along = start + j as f64 * PITCH + PITCH / 2.0;
            let kind = (i + j) as u8 % 7;
            let (height, crown) = crate::trees::size(kind, (i * 100 + j) as u64, None);
            trees.push(([line + STREET / 2.0 + 1.5, along], kind, height, crown, true));
        }
    }
    // A bridge over the pond (`synthetic_water`), so the offline world has one to draw and cross.
    let pond_x = start + (count - 1) as f64 * PITCH + STREET / 2.0 + 2.0 + (PITCH - STREET - 4.0) / 2.0;
    let pond_z = -half + 70.0;
    let bridges = vec![(vec![[pond_x, pond_z - 50.0], [pond_x, pond_z + 50.0]], 7.0, ROAD_MINOR, bridge::Pylons::Middle)];
    assemble((0.0, 0.0), half, tile_size, None, buildings, roads, bridges, covers, trees, ground, water)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_is_metric() {
        // 0.002 degrees of longitude at the fixed scale latitude (63.82), wherever the origin is.
        let expected = 0.002 * shared::world::metres_per_deg_lon();
        let [x, z] = project((60.0, 18.0), 60.001, 18.002);
        assert!((x - expected).abs() < 0.01, "x = {x}");
        assert!((expected - 0.002 * 111_320.0 * 63.8225f64.to_radians().cos()).abs() < 1e-9);
        assert!((z + 110.54).abs() < 0.5, "z = {z}");
    }

    #[test]
    fn joins_pieces_into_a_ring_whatever_their_direction() {
        let rings = join_rings(vec![
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]],
            vec![[0.0, 4.0], [4.0, 4.0]], // runs against the others
            vec![[0.0, 4.0], [0.0, 0.0]],
            vec![[9.0, 9.0], [10.0, 9.0]], // never closes
        ]);
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0].len(), 4);
        assert!((signed_area(&rings[0]).abs() - 16.0).abs() < 1e-9);
    }

    #[test]
    fn clipping_a_ring_keeps_its_part_inside() {
        let square = [[-5.0, -5.0], [5.0, -5.0], [5.0, 5.0], [-5.0, 5.0]];
        let part = clip_ring(&square, [0.0, 0.0, 10.0, 10.0]);
        assert!((signed_area(&part).abs() - 25.0).abs() < 1e-9);
        assert!(clip_ring(&square, [20.0, 20.0, 30.0, 30.0]).is_empty());
        // A ring that swallows the rect clips to the rect.
        let big = [[-50.0, -50.0], [50.0, -50.0], [50.0, 50.0], [-50.0, 50.0]];
        assert!((signed_area(&clip_ring(&big, [0.0, 0.0, 10.0, 10.0])).abs() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn cover_comes_from_tags() {
        let tags = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<std::collections::HashMap<_, _>>();
        assert_eq!(cover_kind(&tags(&[("natural", "water")])), None, "water comes from Hydrografi");
        assert_eq!(cover_kind(&tags(&[("landuse", "forest")])), Some(COVER_WOOD));
        assert_eq!(cover_kind(&tags(&[("leisure", "park")])), Some(COVER_PARK));
        assert_eq!(cover_kind(&tags(&[("landuse", "military")])), None);
    }

    #[test]
    fn synthetic_parks_land_in_their_tiles_water_last() {
        let built = synthetic_city(150.0, 100.0, Ground::default(), Placed::default());
        assert!(built.tiles.values().any(|tile| !tile.cover.is_empty()));
    }

    #[test]
    fn the_synthetic_pond_and_stream_land_in_tiles() {
        use crate::water::{Grid, Surface};
        let map = synthetic_water(300.0);
        let grid = Grid { origin: -30_000, cell: 500, side: 121 };
        let surface = Surface::new(&map, grid, |_, _| Some(3.0));
        let heights = vec![300i16; 121 * 121];
        let ground = Ground { origin: -30_000, cell: 500, side: 121, heights };
        let placed = Placed::new(&map, &surface, 0.0, &ground);
        assert!(!placed.levels.is_empty() && !placed.streams.is_empty());
        let built = synthetic_city(300.0, 100.0, ground, placed);
        let with_water = built.tiles.values().filter(|tile| !tile.water.is_empty()).count();
        assert!(with_water >= 1 && with_water < built.tiles.len());
        let samples = built.manifest.tile_samples() as usize;
        assert!(built.tiles.values().filter(|t| !t.water.is_empty()).all(|t| t.water.len() == samples * samples));
        assert!(built.tiles.values().any(|tile| !tile.streams.is_empty()));
        assert!(built.tiles.values().flat_map(|t| &t.buildings).all(|b| b.footprint.iter().all(|p| p[0] < 25_000)), "the last column of blocks is left to the water");
    }

    #[test]
    fn trees_stay_off_buildings_and_roads_and_sit_in_their_tile() {
        let built = synthetic_city(300.0, 100.0, Ground::default(), Placed::default());
        let trees: usize = built.tiles.values().map(|tile| tile.trees.len()).sum();
        assert!(trees > 0);
        for (&(col, row), tile) in &built.tiles {
            for tree in &tile.trees {
                assert_eq!(built.manifest.tile_at(tree.x, tree.z), (col, row));
                let p = [tree.x as f64 / 100.0, tree.z as f64 / 100.0];
                for building in &tile.buildings {
                    let footprint: Vec<P> = building.footprint.iter().map(|&[x, z]| [x as f64 / 100.0, z as f64 / 100.0]).collect();
                    assert!(!point_in_polygon(p, &footprint));
                }
            }
        }
    }

    #[test]
    fn clips_polylines_to_the_square() {
        let runs = clip_polyline(&[[-20.0, 0.0], [20.0, 0.0], [20.0, 30.0], [-20.0, 30.0]], 10.0);
        assert_eq!(runs, vec![vec![[-10.0, 0.0], [10.0, 0.0]]]);
    }

    #[test]
    fn a_flat_box_is_a_closed_prism() {
        let mut mesh = CollisionMesh::default();
        let parts = roof::build(&mut mesh, &[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]], 0.0, &Roof::Leaf(Shape::Flat { y: 3.0 }));
        assert_eq!(mesh.vertices.len(), 8);
        // 4 walls × 2 + roof 2 + floor 2.
        assert_eq!((parts.walls, parts.roof, mesh.triangles.len()), (8, 2, 12));
    }

    #[test]
    fn spawns_avoid_buildings() {
        let built = synthetic_city(150.0, 100.0, Ground::default(), Placed::default());
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
        let built = synthetic_city(300.0, 100.0, Ground::default(), Placed::default());
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
        let whole = synthetic_city(300.0, 1000.0, Ground::default(), Placed::default());
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
