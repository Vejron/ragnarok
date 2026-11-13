//! Turns OSM ways (or a procedural grid) into a tiled world: a manifest and one `Tile` per
//! square of the grid that has anything in it.

use crate::bridge::{self, Pylons};
use crate::overpass::{Area, LatLon, Osm, Way};
use crate::region::Place;
use crate::roof::{self, Cloud, Roof, Shape, Top};
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
/// A golf flagstick's height, metres.
const FLAG_HEIGHT: f64 = 2.3;
/// How much wider a track's ballast bed is than its gauge, metres.
const BALLAST_EXTRA: f64 = 2.2;

fn cm(metres: f64) -> i32 {
    (metres * 100.0).round() as i32
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

/// From this footprint, square metres, a building is a barn or a block: the laser is trusted.
const BIG_HOUSE: f64 = 250.0;
/// Under this footprint, square metres, a house is one storey (a shed, a sauna); else two at most.
const SMALL_HOUSE: f64 = 40.0;
/// One storey, the plinth under them, and the steepest roof (rise per run), in metres.
const STOREY: f64 = 3.0;
const PLINTH: f64 = 0.5;
const STEEPEST: f64 = 0.8;
/// Highest a roof rises over its eaves, metres.
const MAX_RISE: f64 = 4.0;
/// Buildings that are taller than their footprint says.
const TALL: [&str; 9] = ["church", "chapel", "cathedral", "mosque", "temple", "tower", "water_tower", "silo", "industrial"];

/// `[eaves, top]` in metres over the floor that an untagged house may reach, or `None` when it may
/// be any height: it is tagged with a height, or as something tall, or big enough that its returns
/// are mostly roof. A small house in an unclassified cloud is otherwise built from the crowns over it.
fn cap(way: &Way, ring: &[P]) -> Option<[f64; 2]> {
    let area = signed_area(ring).abs();
    let tagged = ["height", "building:levels"].iter().any(|key| way.tags.contains_key(*key));
    let tall = way.tags.get("building").is_some_and(|value| TALL.contains(&value.as_str()));
    if tagged || tall || area >= BIG_HOUSE {
        return None;
    }
    let storeys = if area < SMALL_HOUSE { 1.0 } else { 2.0 };
    let eaves = storeys * STOREY + PLINTH;
    let rise = roof::half_width(ring).map_or(MAX_RISE, |half| (half * STEEPEST).min(MAX_RISE));
    Some([eaves, eaves + rise])
}

/// The floor of a building: the lowest ground under its corners and its centre, so no wall hangs in the air.
fn floor_of(ground: &Ground, footprint: &[P]) -> f64 {
    let n = footprint.len() as f64;
    let centre = footprint.iter().fold([0.0, 0.0], |sum, p| [sum[0] + p[0] / n, sum[1] + p[1] / n]);
    footprint.iter().chain([&centre]).map(|p| ground.height_at(p[0] as f32, p[1] as f32) as f64).fold(f64::MAX, f64::min)
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

/// The kind a road way of `kind` is drawn and driven as: a paved kind (`ROAD_MAJOR`, `ROAD_MINOR`)
/// whose `surface` tag says it is not paved is a gravel road; anything else keeps its kind.
fn surfaced(way: &Way, kind: u8) -> u8 {
    let unpaved = matches!(
        way.tags.get("surface").map(String::as_str),
        Some("unpaved" | "gravel" | "fine_gravel" | "compacted" | "dirt" | "earth" | "ground" | "grass" | "sand" | "mud" | "pebblestone")
    );
    if unpaved && (kind == ROAD_MAJOR || kind == ROAD_MINOR) { ROAD_GRAVEL } else { kind }
}

/// Gauge in metres and `RAIL_*` kind of a railway way, or `None` for lines that are not drawn:
/// underground, disused, planned, or not a track at all.
fn rail_style(way: &Way) -> Option<(f64, u8)> {
    let tag = |key: &str| way.tags.get(key).map(String::as_str);
    if tag("tunnel").is_some_and(|value| value != "no") || tag("location") == Some("underground") {
        return None;
    }
    let (fallback, kind) = match tag("railway")? {
        "rail" | "light_rail" | "preserved" => (1.435, RAIL_TRAIN),
        "narrow_gauge" => (1.0, RAIL_TRAIN),
        "tram" => (1.435, RAIL_TRAM),
        _ => return None,
    };
    // `gauge` is in millimetres, and lists every gauge on a dual-gauge track.
    let tagged = tag("gauge").and_then(|value| value.split(';').next()?.trim().parse::<f64>().ok()).filter(|mm| (500.0..=2000.0).contains(mm));
    Some((tagged.map_or(fallback, |mm| mm / 1000.0), kind))
}

/// Width of the ballast bed under a track of `gauge` metres.
pub(crate) fn ballast(gauge: f64) -> f64 {
    gauge + BALLAST_EXTRA
}

/// Groomed width of a ski track, metres: room for two classic tracks and a skating lane.
const SKI_TRAIL_WIDTH: f64 = 4.0;

/// Cuts `run` (already inside the world) at tile borders and hands each piece, in centimetres, to
/// `f` with the tile it lies in, so each tile holds only its own pieces.
fn per_tile(manifest: &Manifest, half: f64, side: f64, run: &[P], mut f: impl FnMut((u16, u16), Vec<[i32; 2]>)) {
    let (min, max) = run.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let (first, last) = (manifest.tile_at(cm(min[0]), cm(min[1])), manifest.tile_at(cm(max[0]), cm(max[1])));
    for col in first.0..=last.0 {
        for row in first.1..=last.1 {
            let rect = [-half + col as f64 * side, -half + row as f64 * side, -half + (col + 1) as f64 * side, -half + (row + 1) as f64 * side];
            for piece in clip_polyline_rect(run, rect) {
                f((col, row), piece.iter().map(|&[x, z]| [cm(x), cm(z)]).collect());
            }
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
    /// Bakes every building's cubes (`voxels::bake`, then `voxels::share`), encodes every tile, records its hash in the
    /// manifest and returns `(manifest bytes, [(path, bytes)])`.
    pub fn into_files(mut self) -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
        let mut files = Vec::new();
        self.manifest.tiles.clear();
        let mut tiles: Vec<&mut Tile> = self.tiles.values_mut().collect();
        // Cutting the buildings is most of the time spent here: spread over the cores.
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let chunk = tiles.len().div_ceil(threads).max(1);
        std::thread::scope(|scope| {
            for part in tiles.chunks_mut(chunk) {
                scope.spawn(move || part.iter_mut().for_each(|tile| crate::voxels::bake(tile)));
            }
        });
        crate::voxels::share(&mut tiles);
        for (&(col, row), tile) in &self.tiles {
            let bytes = tile.encode();
            let entry = TileRef { col, row, hash: world_hash(&bytes) };
            files.push((entry.path(), bytes));
            self.manifest.tiles.push(entry);
        }
        (self.manifest.encode(), files)
    }
}

/// A building to assemble: its footprint, its tagged height, its roof (if the laser gave one), its
/// `[wall, roof]` colours (if the photo gave them) and the most it may rise over its floor (`cap`).
type Footprint = (Vec<P>, f64, Option<Top>, Option<[u32; 2]>, Option<f64>);

/// A building outline before its roof is fitted: a `Footprint` without the `Top`.
type Outline = (Vec<P>, f64, Option<[u32; 2]>, Option<f64>);

/// Assembles the world from building outlines (metres, any winding) and road polylines, and cuts
/// it into square tiles of `tile_size` metres.
///
/// `ground` is the terrain over the whole grid of tiles (see `crate::ground`), or flat.
/// `trees` are `(position, TREE_*, height, crown radius, planted)`; those that fall on a building or
/// a road or a track are dropped. A tree is solid (has a trunk collider) when it is planted, in the
/// register, or stands within `SOLID_REACH` of a road, a track or a building.
/// `rails` are `(centre line, gauge, RAIL_*)`; `rail_bridges` carry tracks and are built like road
/// bridges, but draw no road. No rocks or shrubs are made up inside `mapped`, an orienteering map's outline.
#[allow(clippy::too_many_arguments)]
fn assemble(frame: Frame, half: f64, tile_size: f64, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, buildings: Vec<Footprint>, roads: Vec<(Vec<P>, f64, u8)>, bridges: Vec<(Vec<P>, f64, u8, Pylons, Deck)>, rails: Vec<(Vec<P>, f64, u8)>, rail_bridges: Vec<(Vec<P>, f64, u8, Pylons, Deck)>, ski: Vec<Vec<P>>, covers: Vec<(u8, Vec<Vec<P>>)>, trees: Vec<(P, u8, f64, f64, bool)>, mapped: &[P], ground: Ground, water: Placed) -> Built {
    let inner = half - EDGE_MARGIN;
    let mut manifest = Manifest {
        version: WORLD_VERSION,
        frame,
        half_extent: cm(half),
        tile_size: cm(tile_size),
        ground_cell: ground.cell.max(0),
        ..Default::default()
    };
    let mut tiles: BTreeMap<(u16, u16), Tile> = BTreeMap::new();
    let mut footprints = Vec::new();
    for (mut footprint, height, fitted, paint, ceiling) in buildings {
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
        let floor = floor_of(&ground, &footprint);
        let tile = tiles.entry(manifest.tile_at(cm(centre[0]), cm(centre[1]))).or_default();
        let first = tile.collision.triangles.len() as u32;
        let vertices = tile.collision.vertices.len();
        let boxed = Roof::Leaf(Shape::Flat { y: floor + height });
        let (mut parts, mut roof_kind) = match fitted {
            Some(Top::Measured(field)) => (crate::field::build(&mut tile.collision, &footprint, floor, &field), field.kind()),
            fitted => {
                // Without laser data a building is a box of its tagged (or guessed) height.
                let shape = match fitted {
                    Some(Top::Fitted(roof)) => roof,
                    _ => boxed.clone(),
                };
                (roof::build(&mut tile.collision, &footprint, floor, &shape), shape.kind())
            }
        };
        // A fit may reach a little past the returns it was fitted to: past its cap, a house is a box.
        if ceiling.is_some_and(|ceiling| parts.top - floor > ceiling + 0.01) {
            tile.collision.triangles.truncate(first as usize);
            tile.collision.vertices.truncate(vertices);
            (parts, roof_kind) = (roof::build(&mut tile.collision, &footprint, floor, &boxed), boxed.kind());
        }
        tile.buildings.push(Building {
            height: cm(parts.top - floor),
            footprint: footprint.iter().map(|&[x, z]| [cm(x), cm(z)]).collect(),
            base: cm(floor),
            first,
            walls: parts.walls,
            roof: parts.roof,
            roof_kind,
            cells: Default::default(),
            paint,
        });
        footprints.push(footprint);
    }
    let mut candidates = Vec::new();
    let side = tile_size;
    // Bridge roads are drawn like any road (and on the minimap) but never offer a spawn: the
    // ground under them is a river bed.
    for ((points, width, kind), spawnable) in roads.iter().map(|(points, width, kind)| ((points, width, kind), true)).chain(bridges.iter().map(|(points, width, kind, ..)| ((points, width, kind), false))) {
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
            per_tile(&manifest, half, side, &run, |key, points| tiles.entry(key).or_default().roads.push(Road { width: cm(width), kind, points }));
        }
    }
    // Tracks are cut at tile borders like roads, but offer no spawns: nobody starts on a railway.
    for (points, gauge, kind) in &rails {
        for run in clip_polyline(points, half) {
            per_tile(&manifest, half, side, &run, |key, points| tiles.entry(key).or_default().rails.push(Rail { gauge: cm(*gauge), kind: *kind, points }));
        }
    }
    for points in &ski {
        for run in clip_polyline(points, half) {
            per_tile(&manifest, half, side, &run, |key, points| tiles.entry(key).or_default().ski.push(SkiTrail { width: cm(SKI_TRAIL_WIDTH), points }));
        }
    }
    // Bridges are solid geometry in the collision mesh, each piece in the tile it lies in.
    for (points, width, kind, pylons, deck) in bridges.iter().chain(&rail_bridges) {
        let pieces = bridge::build(points, *width, *kind, *pylons, deck, &ground, &water.levels, inner);
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
    // Rocks and shrubs the map does not record go through the same rejection as every tree. Not in
    // France: its laser classes every bush and wall it saw, so made-up ones would only be clutter.
    let mut props = if matches!(frame, Frame::Lambert93 { .. }) { Vec::new() } else { crate::props::scatter(&covers, inner) };
    if mapped.len() >= 3 {
        props.retain(|(at, ..)| !point_in_polygon(*at, mapped));
    }
    // A golf course's mown surfaces are kept clear of trees, rocks and shrubs.
    let golf: Vec<([f64; 4], Vec<P>)> = covers
        .iter()
        .filter(|(kind, _)| is_golf_cover(*kind))
        .flat_map(|(_, rings)| rings.iter())
        .filter(|ring| ring.len() >= 3)
        .map(|ring| (ring.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]), ring.clone()))
        .collect();
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
            per_tile(&manifest, half, side, &run, |key, points| tiles.entry(key).or_default().streams.push(Stream { width: cm(STREAM_WIDTH), points }));
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
    let bridges: Vec<_> = bridges.iter().chain(&rail_bridges).collect();
    let bridge_boxes: Vec<[f64; 4]> = bridges.iter().map(|(points, width, ..)| reach_box(points, width / 2.0 + BRIDGE_CLEAR)).collect();
    // A track keeps trees off its ballast bed, as a road does off its surface.
    let roads: Vec<(&[P], f64)> = roads.iter().map(|(points, width, _)| (&points[..], *width)).chain(rails.iter().map(|(points, gauge, _)| (&points[..], ballast(*gauge)))).chain(ski.iter().map(|points| (&points[..], SKI_TRAIL_WIDTH))).collect();
    let road_boxes: Vec<[f64; 4]> = roads.iter().map(|(points, width)| reach_box(points, width / 2.0 + SOLID_REACH)).collect();
    let footprint_boxes: Vec<[f64; 4]> = footprints.iter().map(|footprint| reach_box(footprint, SOLID_REACH)).collect();
    for (at, kind, height, crown, planted) in trees.into_iter().chain(props) {
        if at[0].abs() > inner || at[1].abs() > inner {
            continue;
        }
        let on_building = footprints.iter().zip(&footprint_boxes).any(|(footprint, b)| in_box(b, at) && point_in_polygon(at, footprint));
        let mut on_road = false;
        let mut near = planted;
        for ((points, width), b) in roads.iter().zip(&road_boxes) {
            if !in_box(b, at) {
                continue;
            }
            let d = points.windows(2).map(|pair| distance_to_segment(at, pair[0], pair[1])).fold(f64::MAX, f64::min);
            on_road |= d < width / 2.0;
            near |= d < width / 2.0 + SOLID_REACH;
        }
        let on_bridge = bridges.iter().zip(&bridge_boxes).any(|((points, width, ..), b)| {
            in_box(b, at) && points.windows(2).any(|pair| distance_to_segment(at, pair[0], pair[1]) < width / 2.0 + BRIDGE_CLEAR)
        });
        let on_course = kind != TREE_FLAG && golf.iter().any(|(b, ring)| in_box(b, at) && point_in_polygon(at, ring));
        if on_building || on_road || on_bridge || on_course {
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
            solid: near && kind != TREE_BUSH && kind != TREE_FLAG,
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

/// A Swedish world's origin (lat, lon) and half-extent in metres for `bbox`: the largest square
/// around its centre, in the equirectangular frame (`region::Place::sweden`).
pub fn frame([south, west, north, east]: [f64; 4]) -> ((f64, f64), f64) {
    let origin = ((south + north) / 2.0, (west + east) / 2.0);
    let [x0, z0] = project(origin, south, west);
    let [x1, z1] = project(origin, north, east);
    (origin, ((x1 - x0).abs().min((z1 - z0).abs()) / 2.0).round())
}

#[allow(clippy::too_many_arguments)]
/// `lidar` holds the returns that can be roof, `decks` the bridge deck returns of a classified cloud,
/// and `photo` the aerial photo the buildings' colours come from.
pub fn from_osm(place: &Place, tile_size: f64, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, osm: &Osm, planted: &[Planted], ground: Ground, water: Placed, lidar: Option<&Cloud>, decks: Option<&Cloud>, photo: Option<&crate::paint::Photo>, ski: &[Vec<crate::overpass::LatLon>], ocad: Option<&crate::ocad::Source>, golf: Option<&shared::golf::Course>, lifts: &[crate::osm_lifts::Mapped]) -> Built {
    let (frame, half) = (place.frame, place.half);
    let source = |x: f64, z: f64| place.source(x, z);
    let mut buildings = Vec::new();
    let mut roads = Vec::new();
    let mut bridges = Vec::new();
    let mut rails = Vec::new();
    let mut rail_bridges = Vec::new();
    let mut covers = Vec::new();
    let line = |geometry: &[LatLon]| -> Vec<P> { geometry.iter().map(|g| frame.project(g.lat, g.lon)).collect() };
    for Area { tags, outer, inner, .. } in &osm.areas {
        let Some(kind) = cover_kind(tags) else { continue };
        let rings: Vec<Vec<P>> = join_rings(outer.iter().map(|w| line(w)).collect()).into_iter().chain(join_rings(inner.iter().map(|w| line(w)).collect())).collect();
        if !rings.is_empty() {
            covers.push((kind, rings));
        }
    }
    let is_building = |way: &crate::overpass::Way| way.tags.get("building").is_some_and(|value| value != "no" && value != "roof");
    // A classified cloud (France) has nothing but roof in its building returns: measure the roof.
    let measured = place.region == crate::region::Region::France;
    // The outlines, in the order of the ways. Where the roof is measured a big one is cut into pieces
    // on a lattice, so no one building is a huge structure to check when it is hit. The pieces share
    // the colours of the whole.
    let paint = |rings: &[&[P]]| photo.map(|photo| crate::paint::of(photo, rings, source));
    let outlines: Vec<Outline> = osm
        .ways
        .iter()
        .filter(|way| is_building(way))
        .flat_map(|way| {
            let points = line(&way.geometry);
            let ring = if points.len() > 1 && points.first() == points.last() { &points[..points.len() - 1] } else { &points[..] };
            // A classified cloud's building returns are roof, whatever stands round it.
            let cap = if measured { None } else { cap(way, ring) };
            let height = cap.map_or(building_height(way), |[eaves, _]| building_height(way).min(eaves));
            let ceiling = cap.map(|[_, top]| top);
            let paint = paint(&[ring]);
            if measured && ring.len() >= 3 && signed_area(ring).abs() > crate::footprints::BIG {
                crate::footprints::pieces(ring).into_iter().map(|piece| (piece, height, paint, ceiling)).collect()
            } else {
                vec![(points, height, paint, ceiling)]
            }
        })
        .collect();
    let rings: Vec<(&[P], Option<f64>)> = outlines.iter().map(|(points, .., ceiling)| (points.as_slice(), *ceiling)).collect();
    let shapes = lidar.map(|cloud| fit_roofs(cloud, &source, measured, half, tile_size, mask, &ground, &rings)).unwrap_or_default();
    let mut shapes = shapes.into_iter();
    for (points, height, paint, ceiling) in outlines {
        // Roofs come from the laser returns over the footprint, where there are any.
        buildings.push((points, height, shapes.next().flatten(), paint, ceiling));
    }
    crate::progress::stage("buildings and roads");
    for (index, way) in osm.ways.iter().enumerate() {
        crate::progress::steps(index, osm.ways.len());
        let points = line(&way.geometry);
        if is_building(way) {
            continue;
        } else if let Some((width, kind)) = way.tags.get("highway").and_then(|value| road_style(value)) {
            if is_bridge(way) {
                let pylons = pylons_for(frame, &points);
                let deck = decks.map(|cloud| deck_returns(cloud, &source, &points, width)).unwrap_or_default();
                bridges.push((points, width, kind, pylons, deck));
            } else {
                // A bridge keeps its class, which sets how high its deck stands (`bridge::clearance`).
                roads.push((points, width, surfaced(way, kind)));
            }
        } else if golf.is_some() && way.tags.get("golf").and_then(|value| crate::osm_golf::surface_kind(value)).is_some() {
            // The course has its surfaces, every one of them, not only those with a landuse too.
            continue;
        } else if let Some(kind) = cover_kind(&way.tags) {
            // Only closed ways are areas; an open one is a bank or a fence.
            if points.len() >= 4 && points.first() == points.last() {
                covers.push((kind, vec![points[..points.len() - 1].to_vec()]));
            }
        }
        // Not part of the chain above: a tram line is often the same way as the street it runs in.
        if let Some((gauge, kind)) = rail_style(way) {
            let points = line(&way.geometry);
            if is_bridge(way) {
                let pylons = pylons_for(frame, &points);
                let deck = decks.map(|cloud| deck_returns(cloud, &source, &points, ballast(gauge))).unwrap_or_default();
                rail_bridges.push((points, ballast(gauge), ROAD_MINOR, pylons, deck));
            } else {
                rails.push((points, gauge, kind));
            }
        }
    }
    // What the laser maps as building and no OSM outline covers is a building OSM is missing.
    if let (true, Some(cloud)) = (measured, lidar) {
        crate::progress::stage("buildings from the laser");
        let tiles = (2.0 * half / tile_size).ceil() as u16;
        let rects: Vec<[f64; 4]> = (0..tiles)
            .flat_map(|row| (0..tiles).map(move |col| (col, row)))
            .filter(|key| mask.is_none_or(|mask| mask.contains(key)))
            .map(|(col, row)| {
                let (x0, z0) = (-half + f64::from(col) * tile_size, -half + f64::from(row) * tile_size);
                [x0, z0, x0 + tile_size, z0 + tile_size]
            })
            .collect();
        let known: Vec<Vec<P>> = buildings.iter().map(|(points, ..)| points.clone()).collect();
        let height = |x: f64, z: f64| f64::from(ground.height_at(x as f32, z as f32));
        let found = crate::footprints::missing(cloud, &source, &rects, &known, &height);
        let before = buildings.len();
        for pieces in found {
            // The pieces of one building share its colours.
            let paint = paint(&pieces.iter().map(Vec::as_slice).collect::<Vec<_>>());
            for ring in pieces {
                // Moored barges and houseboats are building returns too; a building stands on land.
                let n = ring.len() as f64;
                let mid = ring.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
                if bridge::water_at(&ground, &water.levels, mid).is_some() {
                    continue;
                }
                if let Some(field) = crate::field::Field::measure(&ring, &cloud.over(&ring, source)) {
                    buildings.push((ring, 0.0, Some(Top::Measured(field)), paint, None));
                }
            }
        }
        eprintln!("buildings: {} from the laser that OpenStreetMap does not have", buildings.len() - before);
    }
    if lidar.is_some() {
        let split = buildings.iter().filter(|(_, _, shape, ..)| matches!(shape, Some(Top::Fitted(Roof::Cut { .. })))).count();
        let measured = buildings.iter().filter(|(_, _, shape, ..)| matches!(shape, Some(Top::Measured(_)))).count();
        eprintln!("roofs: {measured} of {} buildings measured, {split} fitted in more than one part", buildings.len());
    }
    if photo.is_some() {
        let mut roofs = std::collections::BTreeMap::new();
        for (.., paint, _) in &buildings {
            *roofs.entry(paint.map(|[_, roof]| format!("{roof:06x}"))).or_insert(0usize) += 1;
        }
        eprintln!("roof colours: {roofs:?}");
    }
    let lifts: Vec<(u8, u8, Vec<P>)> = lifts.iter().map(|lift| (lift.kind, lift.seats, line(&lift.line))).collect();
    // A lift runs up a cleared lane through the forest.
    let in_lane = |p: P| lifts.iter().any(|(.., points)| points.windows(2).any(|pair| distance_to_segment(p, pair[0], pair[1]) < LIFT_LANE));
    let mut trees: Vec<_> = planted.iter().map(|tree| (frame.project(tree.lat, tree.lon), tree.kind, tree.height, tree.crown, tree.registered)).filter(|(p, ..)| !in_lane(*p)).collect();
    let mut course = Vec::new();
    if let Some(golf) = golf {
        course.extend(golf.surfaces.iter().map(|surface| (surface.kind, vec![surface.ring.iter().map(|&[x, z]| [f64::from(x), f64::from(z)]).collect::<Vec<P>>()])));
        // A flag in every cup.
        trees.extend(golf.holes.iter().map(|hole| ([f64::from(hole.cup[0]), f64::from(hole.cup[1])], TREE_FLAG, FLAG_HEIGHT, 0.3, false)));
    }
    crate::progress::stage("tiles");
    let ski = ski.iter().map(|way| way.iter().map(|p| frame.project(p.lat, p.lon)).collect()).collect();
    let mut mapped = Vec::new();
    if let Some(ocad) = ocad {
        roads.extend(ocad.roads.iter().map(|(geometry, width, kind)| (line(geometry), *width, *kind)));
        covers = covers.into_iter().flat_map(|(kind, rings)| if is_golf_cover(kind) { vec![rings] } else { ocad.outside(&rings) }.into_iter().map(move |rings| (kind, rings))).collect();
        covers.extend(ocad.covers.iter().cloned());
        mapped.clone_from(&ocad.outline);
    }
    // After the map's own, which would cut them away where it covers the course.
    covers.extend(course);
    // The lines are laid on the ground once the tiles are built, so it is kept for them.
    let kept = (ocad.is_some_and(|ocad| !ocad.lines.is_empty()) || !lifts.is_empty()).then(|| ground.clone());
    // A lift's own station stands at each of its ends: the station building OSM or the laser has there goes.
    let ends: Vec<P> = lifts.iter().flat_map(|(.., points)| points.first().into_iter().chain(points.last())).copied().collect();
    let over_station = |ring: &[P]| ends.iter().any(|&end| point_in_polygon(end, ring) || (0..ring.len()).any(|i| distance_to_segment(end, ring[i], ring[(i + 1) % ring.len()]) < LIFT_STATION_CLEAR));
    buildings.retain(|(ring, ..)| !over_station(ring));
    let mut built = assemble(frame, half, tile_size, mask, buildings, roads, bridges, rails, rail_bridges, ski, covers, trees, &mapped, ground, water);
    if let Some(ground) = &kept {
        lay_lifts(&mut built, &lifts, half, ground);
    }
    if let (Some(ocad), Some(ground)) = (ocad, kept) {
        lay_lines(&mut built, &ocad.lines, half, tile_size, &ground);
    }
    if let Some(ocad) = ocad {
        lay_stones(&mut built, &ocad.stony_dots, &ocad.stony_areas, half, tile_size);
    }
    built
}

/// Side of a cell of a tile's `stones`, metres, about: the tile is cut into a whole number of them.
const STONES_CELL: f64 = 2.0;
/// How far a dot of stony ground reaches, metres: the spread of the patch it stands for.
const STONY_SPREAD: f64 = 5.0;

/// Fills each tile's `stones` from stony ground drawn as dots (each a soft patch about its point,
/// filling in where patches overlap) and as areas (stony to the edge). A cell is as stony as the
/// symbols over it say, never more: many dots of slow running are still slow running. Cells take the
/// angular blocks of a boulder field where those are the larger share.
fn lay_stones(built: &mut Built, dots: &[(P, crate::ocad::isom::Stony)], areas: &[(Vec<Vec<P>>, crate::ocad::isom::Stony)], half: f64, tile_size: f64) {
    if dots.is_empty() && areas.is_empty() {
        return;
    }
    let n = (tile_size / STONES_CELL).round().max(1.0) as usize;
    let cell = tile_size / n as f64;
    let reach = STONY_SPREAD * 3.0;
    let bounds: Vec<Option<[f64; 4]>> = areas.iter().map(|(rings, _)| crate::props::bounds(rings)).collect();
    // The cells, clamped to the tile, that `lo..hi` metres along an axis from `origin` covers.
    let span = |lo: f64, hi: f64, origin: f64| (((lo - origin) / cell).floor().max(0.0) as usize, (((hi - origin) / cell).ceil().max(0.0) as usize).min(n));
    for (&(col, row), tile) in &mut built.tiles {
        let (x0, z0) = (-half + f64::from(col) * tile_size, -half + f64::from(row) * tile_size);
        // [cobbles, blocks]: first the dots' reach and their levels by it, then how stony.
        let mut reach_of = [vec![0.0f64; n * n], vec![0.0f64; n * n]];
        let mut share = [vec![0.0f64; n * n], vec![0.0f64; n * n]];
        for &(p, stony) in dots {
            let (i0, i1) = span(p[0] - reach, p[0] + reach, x0);
            let (j0, j1) = span(p[1] - reach, p[1] + reach, z0);
            for j in j0..j1 {
                for i in i0..i1 {
                    let (dx, dz) = (x0 + (i as f64 + 0.5) * cell - p[0], z0 + (j as f64 + 0.5) * cell - p[1]);
                    let weight = (-(dx * dx + dz * dz) / (2.0 * STONY_SPREAD * STONY_SPREAD)).exp();
                    reach_of[usize::from(stony.angular)][j * n + i] += weight;
                    share[usize::from(stony.angular)][j * n + i] += stony.level * weight;
                }
            }
        }
        // The dots' level, as much of it as they reach: most of it at a lone dot's middle, all of it where they crowd.
        for (share, reach) in share.iter_mut().zip(&reach_of) {
            for (value, &reach) in share.iter_mut().zip(reach) {
                *value = if reach > 0.0 { *value / reach * (1.0 - (-2.0 * reach).exp()) } else { 0.0 };
            }
        }
        for ((rings, stony), b) in areas.iter().zip(&bounds) {
            let Some(b) = b else { continue };
            let (i0, i1) = span(b[0], b[2], x0);
            let (j0, j1) = span(b[1], b[3], z0);
            for j in j0..j1 {
                for i in i0..i1 {
                    let at = [x0 + (i as f64 + 0.5) * cell, z0 + (j as f64 + 0.5) * cell];
                    let value = &mut share[usize::from(stony.angular)][j * n + i];
                    if *value < stony.level && crate::props::inside(rings, at) {
                        *value = stony.level;
                    }
                }
            }
        }
        let stones: Vec<u8> = (0..n * n)
            .map(|k| {
                let (cobbles, blocks) = (share[0][k], share[1][k]);
                let level = (cobbles.max(blocks).min(1.0) * f64::from(STONES_DENSITY)).round() as u8;
                if level == 0 { 0 } else if blocks > cobbles { level | STONES_ANGULAR } else { level }
            })
            .collect();
        if stones.iter().any(|&value| value != 0) {
            tile.stones = stones;
        }
    }
}

/// Longest span between two poles of a power line, metres: wooden poles, then steel pylons.
const POLE_SPAN: f64 = 55.0;
const PYLON_SPAN: f64 = 260.0;
/// A power line turns at a pole: a bend sharper than this, radians, gets one of its own.
const POLE_BEND: f64 = 0.14;
/// Closest a pole at a bend stands to the one before, metres.
const POLE_GAP: f64 = 12.0;

/// A power line's poles along `points`: at its ends and sharper bends, and evenly between them, no
/// span longer than `span`. A line drawn with many small turns is straightened, as a power line is.
fn poles(points: &[P], span: f64) -> Vec<P> {
    let Some(&first) = points.first() else { return Vec::new() };
    let heading = |a: P, b: P| (b[1] - a[1]).atan2(b[0] - a[0]);
    let mut corners = vec![first];
    for i in 1..points.len() {
        let last = *corners.last().unwrap();
        let p = points[i];
        let far = (p[0] - last[0]).hypot(p[1] - last[1]);
        let end = i + 1 == points.len();
        let bend = !end && {
            let turn = (heading(p, points[i + 1]) - heading(last, p) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            turn.abs() > POLE_BEND && far >= POLE_GAP
        };
        if end && far < POLE_GAP && corners.len() > 1 {
            // Too near the pole before: the line ends there instead.
            corners.pop();
        }
        if end || bend {
            corners.push(p);
        }
    }
    let mut poles = vec![first];
    for pair in corners.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let spans = ((b[0] - a[0]).hypot(b[1] - a[1]) / span).ceil().max(1.0) as usize;
        poles.extend((1..=spans).map(|i| {
            let t = i as f64 / spans as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        }));
    }
    poles
}

/// Half the width of the lane cleared through the forest under a ski lift, metres.
const LIFT_LANE: f64 = 7.0;
/// A building this near a lift's end (metres) is its station building, taken away for the lift's own.
const LIFT_STATION_CLEAR: f64 = 8.0;

/// Puts the ski lifts (`(LIFT_*, seats, line)`) in the manifest: the ones wholly in the world, each
/// from its lower end, with pylons put in where a span is longer than `LIFT_SPAN` and every pylon tall
/// enough that a chair clears the ground (LIFTS.md).
fn lay_lifts(built: &mut Built, lifts: &[(u8, u8, Vec<P>)], half: f64, ground: &Ground) {
    use shared::consts::{LIFT_CHAIR_DROP, LIFT_CLEARANCE, LIFT_GAUGE, LIFT_PYLON_CABLE, LIFT_PYLON_MAX, LIFT_SPAN, LIFT_STATION_CABLE};
    let inner = half - EDGE_MARGIN;
    let Built { manifest, tiles } = built;
    for (kind, seats, line) in lifts {
        let mut points: Vec<P> = Vec::new();
        for &p in line {
            if points.last().is_none_or(|q: &P| (p[0] - q[0]).hypot(p[1] - q[1]) > 1.0) {
                points.push(p);
            }
        }
        let inside = |p: &P| p[0].abs() < inner && p[1].abs() < inner && tiles.contains_key(&manifest.tile_at(cm(p[0]), cm(p[1])));
        if points.len() < 2 || !points.iter().all(inside) {
            continue;
        }
        // The ground under the line and under both cables.
        let height = |p: P, across: [f64; 2]| {
            let at = |s: f64| f64::from(ground.height_at((p[0] + across[0] * s) as f32, (p[1] + across[1] * s) as f32));
            at(0.0).max(at(f64::from(LIFT_GAUGE) / 2.0)).max(at(-f64::from(LIFT_GAUGE) / 2.0))
        };
        let floor = |p: P| f64::from(ground.height_at(p[0] as f32, p[1] as f32));
        if floor(points[0]) > floor(*points.last().unwrap()) {
            points.reverse();
        }
        // Pylons on the long spans.
        let mut line = vec![points[0]];
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let spans = ((b[0] - a[0]).hypot(b[1] - a[1]) / f64::from(LIFT_SPAN)).ceil().max(1.0) as usize;
            line.extend((1..=spans).map(|i| {
                let t = i as f64 / spans as f64;
                [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
            }));
        }
        let n = line.len();
        let base: Vec<f64> = line.iter().map(|&p| floor(p)).collect();
        let mut cable: Vec<f64> = (0..n).map(|i| base[i] + f64::from(if i == 0 || i + 1 == n { LIFT_STATION_CABLE } else { LIFT_PYLON_CABLE })).collect();
        let from_start: Vec<f64> = line.iter().scan((0.0, line[0]), |(run, last), &p| {
            *run += (p[0] - last[0]).hypot(p[1] - last[1]);
            *last = p;
            Some(*run)
        }).collect();
        let total = from_start[n - 1];
        // The seat rises from a rider's knees at a station to the full clearance a few metres out.
        let seat_at_station = f64::from(LIFT_STATION_CABLE - LIFT_CHAIR_DROP);
        let wanted = |run: f64| (seat_at_station + run.min(total - run) * 0.3).min(f64::from(LIFT_CLEARANCE));
        for _ in 0..64 {
            let mut raised = false;
            for i in 0..n - 1 {
                let (a, b) = (line[i], line[i + 1]);
                let length = (b[0] - a[0]).hypot(b[1] - a[1]);
                let across = [-(b[1] - a[1]) / length.max(1e-6), (b[0] - a[0]) / length.max(1e-6)];
                let steps = (length / 4.0).ceil().max(1.0) as usize;
                for k in 1..steps {
                    let t = k as f64 / steps as f64;
                    let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                    let seat = cable[i] + (cable[i + 1] - cable[i]) * t - f64::from(LIFT_CHAIR_DROP);
                    let short = height(p, across) + wanted(from_start[i] + length * t) - seat;
                    if short <= 0.01 {
                        continue;
                    }
                    // Raise the pylons at the span's ends, never a station.
                    match (i == 0, i + 2 == n) {
                        (true, true) => {}
                        (true, false) => cable[i + 1] += short / t,
                        (false, true) => cable[i] += short / (1.0 - t),
                        (false, false) => {
                            cable[i] += short;
                            cable[i + 1] += short;
                        }
                    }
                    raised = true;
                    for j in 1..n - 1 {
                        cable[j] = cable[j].min(base[j] + f64::from(LIFT_PYLON_MAX));
                    }
                    break;
                }
            }
            if !raised {
                break;
            }
        }
        manifest.lifts.push(Lift {
            kind: *kind,
            seats: *seats,
            points: line.iter().map(|p| [cm(p[0]), cm(p[1])]).collect(),
            base: base.iter().map(|&h| cm(h)).collect(),
            cable: cable.iter().map(|&h| cm(h)).collect(),
        });
    }
}

/// Puts fences and power lines (`(LINE_*, line)`) in the tiles they cross, which must exist already.
/// A fence is cut at tile borders like a road. A power line is its poles, each span in the tile of
/// its first pole, so every pole is in one tile only and a span never needs two.
fn lay_lines(built: &mut Built, lines: &[(u8, Vec<P>)], half: f64, tile_size: f64, ground: &Ground) {
    let inner = half - EDGE_MARGIN;
    let Built { manifest, tiles } = built;
    let mut put = |key: (u16, u16), kind: u8, points: Vec<[i32; 2]>, end: bool| {
        if let Some(tile) = tiles.get_mut(&key) {
            let base = points.iter().map(|&[x, z]| cm(f64::from(ground.height_at(x as f32 / 100.0, z as f32 / 100.0)))).collect();
            tile.lines.push(Line { kind, points, base, end });
        }
    };
    for (kind, points) in lines {
        let span = match *kind {
            LINE_POWER => POLE_SPAN,
            LINE_POWER_MAJOR => PYLON_SPAN,
            _ => {
                for run in clip_polyline(points, inner) {
                    per_tile(manifest, half, tile_size, &run, |key, points| put(key, *kind, points, true));
                }
                continue;
            }
        };
        let at = |p: P| [cm(p[0]), cm(p[1])];
        let outside = |p: &P| p[0].abs() > inner || p[1].abs() > inner;
        let mut run: Vec<[i32; 2]> = Vec::new();
        let mut owner = None;
        for pair in poles(points, span).windows(2) {
            let key = (!pair.iter().any(outside)).then(|| manifest.tile_at(cm(pair[0][0]), cm(pair[0][1])));
            if key != owner {
                // The next span is elsewhere: the run ends on a pole of its own only where the line leaves the world.
                if let Some(owner) = owner {
                    put(owner, *kind, std::mem::take(&mut run), key.is_none());
                }
                run = vec![at(pair[0])];
                owner = key;
            }
            run.push(at(pair[1]));
        }
        if let Some(owner) = owner {
            put(owner, *kind, run, true);
        }
    }
}

/// The roof over each of `rings` (footprints, local metres, closed or not): measured
/// (`field::Field`) when `measured` and the returns cover the footprint, else fitted. Buildings are
/// independent, so they are done on all cores; one that lies wholly outside the painted tiles is
/// skipped, as the tiles drop it anyway.
#[allow(clippy::too_many_arguments)]
fn fit_roofs(
    cloud: &Cloud,
    source: &(impl Fn(f64, f64) -> (f64, f64) + Sync),
    measured: bool,
    half: f64,
    tile_size: f64,
    mask: Option<&std::collections::BTreeSet<(u16, u16)>>,
    ground: &Ground,
    rings: &[(&[P], Option<f64>)],
) -> Vec<Option<Top>> {
    crate::progress::stage("roofs");
    let painted = |ring: &[P]| {
        let Some(mask) = mask else { return true };
        let (lo, hi) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let span = |a: f64, b: f64| ((a + half) / tile_size).floor() as i64..=((b + half) / tile_size).floor() as i64;
        span(lo[0], hi[0]).any(|c| span(lo[1], hi[1]).any(|r| c >= 0 && r >= 0 && mask.contains(&(c as u16, r as u16))))
    };
    let fit = |(points, ceiling): (&[P], Option<f64>)| {
        let ring = if points.len() > 1 && points.first() == points.last() { &points[..points.len() - 1] } else { points };
        if ring.len() < 3 || !painted(ring) {
            return None;
        }
        let mut returns = cloud.over(ring, source);
        // Over a house that may only be so tall, what stands higher is the trees round it.
        if let Some(ceiling) = ceiling {
            let top = floor_of(ground, ring) + ceiling;
            returns.retain(|r| r[2] <= top);
        }
        if measured && let Some(field) = crate::field::Field::measure(ring, &returns) {
            return Some(Top::Measured(field));
        }
        roof::fit(ring, &returns).map(Top::Fitted)
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Option<Top>>>> = rings.iter().map(|_| Default::default()).collect();
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&ring) = rings.get(at) else { break };
                    *slots[at].lock().unwrap() = Some(fit(ring));
                    crate::progress::steps(done.fetch_add(1, std::sync::atomic::Ordering::Relaxed), rings.len());
                }
            });
        }
    });
    slots.into_iter().map(|slot| slot.into_inner().unwrap().expect("every building was taken")).collect()
}

/// Returns of a bridge's deck as measured, `[x, z, y]`: an empty list where nothing was measured.
pub type Deck = Vec<[f64; 3]>;

/// The deck returns within half of `width` of the centre line `points`.
fn deck_returns(cloud: &Cloud, source: &impl Fn(f64, f64) -> (f64, f64), points: &[P], width: f64) -> Deck {
    if points.len() < 2 {
        return Vec::new();
    }
    let reach = width / 2.0;
    let (lo, hi) = points.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let around = [[lo[0] - reach, lo[1] - reach], [lo[0] - reach, hi[1] + reach], [hi[0] + reach, hi[1] + reach], [hi[0] + reach, lo[1] - reach]];
    let near = |p: P| {
        points.windows(2).any(|pair| {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let length = dx * dx + dz * dz;
            let t = if length > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length).clamp(0.0, 1.0) } else { 0.0 };
            (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dz * t) <= reach
        })
    };
    cloud.over(&around, source).into_iter().filter(|r| near([r[0], r[1]])).map(|r| [r[0], r[1], r[2]]).collect()
}

/// The pylons a bridge is built with: those of the landmark it lies at, if any.
fn pylons_for(frame: Frame, points: &[P]) -> Pylons {
    let n = points.len().max(1) as f64;
    let mid = [points.iter().map(|p| p[0]).sum::<f64>() / n, points.iter().map(|p| p[1]).sum::<f64>() / n];
    bridge::LANDMARKS
        .iter()
        .find(|(lat, lon, radius, _)| {
            let at = frame.project(*lat, *lon);
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
    if let Some(kind) = tag("golf").and_then(crate::osm_golf::surface_kind) {
        return Some(kind);
    }
    if let Some(value) = tag("natural") {
        return match value {
            "wood" => Some(COVER_WOOD),
            "scrub" | "heath" | "grassland" => Some(COVER_PARK),
            "wetland" => Some(match tag("wetland") {
                Some("bog" | "swamp" | "fen") => COVER_MARSH_DEEP,
                Some("wet_meadow") => COVER_MARSH_FAINT,
                _ => COVER_MARSH,
            }),
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

/// Draw order: ground uses first, then green, water and marsh on top. The client paints in list order.
fn cover_rank(kind: u8) -> u8 {
    match kind {
        COVER_URBAN | COVER_INDUSTRIAL | COVER_FARM => 0,
        COVER_WOOD | COVER_PARK | COVER_SAND | COVER_THICKET_SLOW | COVER_THICKET_WALK | COVER_THICKET_FIGHT => 1,
        // A course over the park it lies in: its fairways, and on them its tees, greens and bunkers.
        COVER_GOLF_FAIRWAY => 3,
        COVER_GOLF_GREEN | COVER_GOLF_TEE | COVER_GOLF_BUNKER => 4,
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
                    Top::Fitted(Roof::Leaf(if next() < 0.5 {
                        Shape::Gable { y: top, at, ridge, pitch: 0.6 }
                    } else {
                        Shape::Hip { y: top, at, ridge, pitch: 0.6, half: 0.0 }
                    }))
                });
                buildings.push((vec![[x, z], [x + half_size, z], [x + half_size, z + half_size], [x, z + half_size]], height, roof, None, None));
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
    let bridges = vec![(vec![[pond_x, pond_z - 50.0], [pond_x, pond_z + 50.0]], 7.0, ROAD_MINOR, bridge::Pylons::Middle, Deck::new())];
    // A tram down the middle street, so the offline world has a track to draw.
    let middle = start + (count / 2) as f64 * PITCH;
    let rails = vec![(vec![[middle, -half], [middle, half]], 1.435, RAIL_TRAM)];
    assemble(Frame::default(), half, tile_size, None, buildings, roads, bridges, rails, Vec::new(), Vec::new(), covers, trees, &[], ground, water)
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
        assert_eq!(cover_kind(&tags(&[("natural", "wetland")])), Some(COVER_MARSH));
        assert_eq!(cover_kind(&tags(&[("natural", "wetland"), ("wetland", "bog")])), Some(COVER_MARSH_DEEP));
        assert_eq!(cover_kind(&tags(&[("natural", "wetland"), ("wetland", "wet_meadow")])), Some(COVER_MARSH_FAINT));
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
        let heights = vec![300i32; 121 * 121];
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
    fn railways_come_from_tags() {
        let way = |tags: &[(&str, &str)]| Way { id: 3, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), geometry: vec![] };
        assert_eq!(rail_style(&way(&[("railway", "rail")])), Some((1.435, RAIL_TRAIN)));
        assert_eq!(rail_style(&way(&[("railway", "narrow_gauge"), ("gauge", "891")])), Some((0.891, RAIL_TRAIN)));
        assert_eq!(rail_style(&way(&[("railway", "rail"), ("gauge", "1520;1435")])), Some((1.52, RAIL_TRAIN)));
        assert_eq!(rail_style(&way(&[("railway", "tram")])), Some((1.435, RAIL_TRAM)));
        assert_eq!(rail_style(&way(&[("railway", "rail"), ("tunnel", "yes")])), None);
        assert_eq!(rail_style(&way(&[("railway", "abandoned")])), None);
        assert_eq!(rail_style(&way(&[("railway", "subway")])), None);
    }

    #[test]
    fn a_track_is_cut_at_tile_borders_and_keeps_trees_off_its_bed() {
        let rails = vec![(vec![[-140.0, 10.0], [140.0, 10.0]], 1.435, RAIL_TRAIN)];
        let trees = vec![([20.0, 10.5], TREE_BIRCH, 10.0, 2.0, false), ([20.0, 25.0], TREE_BIRCH, 10.0, 2.0, false)];
        let built = assemble(Frame::default(), 150.0, 100.0, None, vec![], vec![], vec![], rails, vec![], vec![], vec![], trees, &[], Ground::default(), Placed::default());
        let pieces: Vec<(&(u16, u16), &Rail)> = built.tiles.iter().flat_map(|(key, tile)| tile.rails.iter().map(move |rail| (key, rail))).collect();
        assert_eq!(pieces.len(), 3, "a 280 m track crosses three 100 m tiles");
        assert!(pieces.iter().all(|(_, rail)| rail.gauge == 144 && rail.kind == RAIL_TRAIN));
        let trees: Vec<&Tree> = built.tiles.values().flat_map(|tile| &tile.trees).collect();
        assert_eq!(trees.len(), 1, "the tree on the ballast is dropped");
        assert!(trees[0].solid, "the one beside the line is near it");
        assert!(built.manifest.spawns.iter().all(|&[_, z]| (z - 1000).abs() > 100), "no spawn on the track");
    }

    #[test]
    fn power_poles_stand_at_bends_and_evenly_between() {
        // 120 m east with a wobble that is no bend, then 40 m south.
        let line = [[0.0, 0.0], [50.0, 0.5], [120.0, 0.0], [120.0, 40.0]];
        let found = poles(&line, 55.0);
        assert_eq!(found.len(), 5, "{found:?}");
        assert_eq!(found[3], [120.0, 0.0]);
        assert_eq!(found[4], [120.0, 40.0]);
        assert!(found.windows(2).all(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]) <= 55.0));
        // An end close behind a bend is the end, not a pole a few metres past it.
        assert_eq!(poles(&[[0.0, 0.0], [100.0, 0.0], [100.0, 5.0]], 200.0), [[0.0, 0.0], [100.0, 5.0]]);
    }

    #[test]
    fn a_power_line_puts_each_pole_in_one_tile_and_a_fence_is_cut() {
        let manifest = Manifest { version: WORLD_VERSION, half_extent: cm(150.0), tile_size: cm(100.0), ..Default::default() };
        let mut built = Built { manifest, tiles: [(0, 1), (1, 1), (2, 1)].into_iter().map(|key| (key, Tile::default())).collect() };
        let lines = vec![(LINE_POWER, vec![[-140.0, 10.0], [140.0, 10.0]]), (LINE_FENCE, vec![[-140.0, 20.0], [140.0, 20.0]])];
        lay_lines(&mut built, &lines, 150.0, 100.0, &Ground::default());
        let power: Vec<&Line> = built.tiles.values().flat_map(|tile| &tile.lines).filter(|line| line.kind == LINE_POWER).collect();
        // Six spans of 46.7 m: poles at -140, -93, -47, 0, 47, 93, 140.
        assert_eq!(power.iter().map(|line| line.points.len() - usize::from(!line.end)).sum::<usize>(), 7, "{power:?}");
        assert_eq!(power.iter().filter(|line| line.end).count(), 1, "only the last run ends on a pole of its own");
        let fences = built.tiles.values().flat_map(|tile| &tile.lines).filter(|line| line.kind == LINE_FENCE).count();
        assert_eq!(fences, 3);
    }

    #[test]
    fn a_lift_runs_uphill_and_its_chairs_clear_a_hump_between_the_pylons() {
        use shared::consts::{LIFT_CLEARANCE, LIFT_PYLON_CABLE, LIFT_STATION_CABLE};
        // A slope rising 0.2 m a metre to the north (-Z), with a 10 m hump at z = -60.
        let (side, cell) = (61u32, 500);
        let heights = (0..side * side)
            .map(|k| {
                let z = -15_000.0 + f64::from(k / side) * 500.0;
                let hump = 1_000.0 * (1.0 - ((z + 6_000.0) / 2_000.0).powi(2)).max(0.0);
                (-z * 0.2 + hump) as i32
            })
            .collect();
        let ground = Ground { origin: -15_000, cell, side, heights };
        let manifest = Manifest { version: WORLD_VERSION, half_extent: cm(150.0), tile_size: cm(100.0), ..Default::default() };
        let mut built = Built { manifest, tiles: (0..3).flat_map(|c| (0..3).map(move |r| ((c, r), Tile::default()))).collect() };
        // Drawn from the top down, with one pylon at z = -20.
        let lifts = vec![(LIFT_CHAIR, 2, vec![[0.0, -120.0], [0.0, -20.0], [0.0, 120.0]]), (LIFT_CHAIR, 2, vec![[0.0, 0.0], [400.0, 0.0]])];
        lay_lifts(&mut built, &lifts, 150.0, &ground);
        assert_eq!(built.manifest.lifts.len(), 1, "the one that leaves the world is dropped");
        let lift = &built.manifest.lifts[0];
        assert_eq!(lift.points[0], [0, 12_000], "it starts at the bottom");
        // Both spans are shorter than `LIFT_SPAN`: no pylon is put in.
        assert_eq!(lift.points.len(), 3, "{:?}", lift.points);
        assert_eq!(lift.cable[0] - lift.base[0], cm(f64::from(LIFT_STATION_CABLE)));
        assert!(lift.cable[1] - lift.base[1] >= cm(f64::from(LIFT_PYLON_CABLE)));
        let path = shared::lifts::Path::new(lift);
        let mut along = 30.0;
        while along < path.up - 30.0 {
            let chair = path.at(along);
            let under = ground.height_at(chair.seat[0], chair.seat[2]);
            assert!(chair.seat[1] - under >= LIFT_CLEARANCE - 0.05, "{} m over the ground at {along}", chair.seat[1] - under);
            along += 2.0;
        }
    }

    #[test]
    fn stony_dots_are_soft_patches_and_areas_fill_to_their_edge() {
        use crate::ocad::isom::Stony;
        let manifest = Manifest { version: WORLD_VERSION, half_extent: cm(100.0), tile_size: cm(100.0), ..Default::default() };
        let mut built = Built { manifest, tiles: [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().map(|key| (key, Tile::default())).collect() };
        let cobbles = Stony { level: 0.45, angular: false, per_block: 0.0 };
        let blocks = Stony { level: 0.9, angular: true, per_block: 15.0 };
        // Two dots near the middle of tile (0, 0), a block field over the corner of (1, 1).
        let dots = vec![([-51.0, -51.0], cobbles), ([-47.0, -51.0], cobbles)];
        let areas = vec![(vec![vec![[60.0, 60.0], [100.0, 60.0], [100.0, 100.0], [60.0, 100.0]]], blocks)];
        lay_stones(&mut built, &dots, &areas, 100.0, 100.0);
        let tile = &built.tiles[&(0, 0)];
        assert_eq!(tile.stones.len(), 50 * 50);
        let at = |tile: &Tile, x: f64, z: f64| tile.stones[(z / 2.0) as usize * 50 + (x / 2.0) as usize];
        let middle = at(tile, 50.0, 49.0);
        // Two overlapping patches: cobbles, filled in to nearly their symbol's level and not past it.
        assert!(middle & STONES_ANGULAR == 0 && f64::from(middle) > 0.4 * 127.0 && f64::from(middle) <= 0.45 * 127.0 + 0.5, "{middle}");
        assert_eq!(at(tile, 5.0, 5.0), 0, "a patch fades out within its reach");
        assert!(built.tiles[&(0, 1)].stones.is_empty() && built.tiles[&(1, 0)].stones.is_empty(), "nothing reaches these");
        let corner = &built.tiles[&(1, 1)];
        assert_eq!(at(corner, 80.0, 80.0), STONES_ANGULAR | 114);
        assert_eq!(at(corner, 40.0, 40.0), 0);
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

    fn square(side: f64) -> Vec<P> {
        vec![[0.0, 0.0], [side, 0.0], [side, side], [0.0, side]]
    }

    #[test]
    fn an_unpaved_residential_way_is_a_gravel_road() {
        let way = |tags: &[(&str, &str)]| Way { id: 3, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), geometry: vec![] };
        assert_eq!(surfaced(&way(&[("highway", "residential"), ("surface", "gravel")]), ROAD_MINOR), ROAD_GRAVEL);
        assert_eq!(surfaced(&way(&[("highway", "secondary"), ("surface", "unpaved")]), ROAD_MAJOR), ROAD_GRAVEL);
        // Paved or untagged, it stays asphalt; a footpath stays a path whatever it is surfaced with.
        assert_eq!(surfaced(&way(&[("highway", "residential"), ("surface", "asphalt")]), ROAD_MINOR), ROAD_MINOR);
        assert_eq!(surfaced(&way(&[("highway", "primary")]), ROAD_MAJOR), ROAD_MAJOR);
        assert_eq!(surfaced(&way(&[("highway", "footway"), ("surface", "gravel")]), ROAD_PATH), ROAD_PATH);
    }

    #[test]
    fn small_untagged_houses_are_one_or_two_storeys() {
        let way = |tags: &[(&str, &str)]| Way { id: 3, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), geometry: vec![] };
        let house = way(&[("building", "yes")]);
        // A 4.5 m shed is one storey under a roof rising 0.8 per metre to its ridge; a 7 m house two.
        let [eaves, top] = cap(&house, &square(4.5)).unwrap();
        assert!((eaves - 3.5).abs() < 1e-9 && (top - 5.3).abs() < 1e-9, "{eaves} {top}");
        let [eaves, top] = cap(&house, &square(7.0)).unwrap();
        assert!((eaves - 6.5).abs() < 1e-9 && (top - 9.3).abs() < 1e-9, "{eaves} {top}");
        // Tagged, tall by kind, or big enough to trust the laser: no cap.
        assert_eq!(cap(&way(&[("building", "yes"), ("height", "14")]), &square(7.0)), None);
        assert_eq!(cap(&way(&[("building", "yes"), ("building:levels", "3")]), &square(7.0)), None);
        assert_eq!(cap(&way(&[("building", "church")]), &square(7.0)), None);
        assert_eq!(cap(&house, &square(17.0)), None);
    }

    #[test]
    fn the_trees_round_a_small_house_are_not_its_roof() {
        use crate::lidar::Point;
        let ring = square(7.0);
        let mut cloud = Cloud::new([-10.0, -10.0, 20.0, 20.0]);
        let mut add = |e: f64, n: f64, z: f64| cloud.add(&Point { e, n, z, class: 1, ret: 1, returns: 1, intensity: 0 }, 0.0);
        // A flat roof 4 m up under the edge of a crown 16 m up that covers two thirds of it.
        for i in 0..14 {
            for j in 0..14 {
                let (e, n) = (0.25 + f64::from(i) * 0.5, 0.25 + f64::from(j) * 0.5);
                add(e, n, if e > 2.4 { 16.0 } else { 4.0 });
            }
        }
        let tops = fit_roofs(&cloud, &|x, z| (x, z), false, 100.0, 100.0, None, &Ground::default(), &[(&ring, None), (&ring, Some(9.3))]);
        let Some(Top::Fitted(Roof::Leaf(Shape::Flat { y }))) = tops[0] else { panic!("{:?}", tops[0]) };
        assert!(y > 15.0, "uncapped, the crown is the roof: {y}");
        let Some(Top::Fitted(Roof::Leaf(Shape::Flat { y }))) = tops[1] else { panic!("{:?}", tops[1]) };
        assert!((y - 4.0).abs() < 0.3, "capped, the roof is {y}");
    }
}
