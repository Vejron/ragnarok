//! Turns OSM ways (or a procedural grid) into a tiled world: a manifest and one `Tile` per
//! square of the grid that has anything in it.

use crate::bridge::{self, Pylons};
use crate::lanes::Lanes;
use crate::overpass::{Area, LatLon, Osm, Way};
use crate::region::Place;
use crate::catalogue::Category;
use crate::roof::{self, Cloud, Roof, Shape, Top};
use crate::trees::Planted;
use crate::water::{Placed, STREAM_WIDTH};
use shared::terrain::Ground;
use shared::world::*;
use std::collections::{BTreeMap, HashMap};

type P = [f64; 2];
type V3 = [f64; 3];

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

pub(crate) fn distance_to_segment([px, pz]: P, [ax, az]: P, [bx, bz]: P) -> f64 {
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
    let tall = way.tags.get("building").is_some_and(|value| TALL.contains(&value.as_str())) || crate::classify::is_church(&way.tags);
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

/// The `surface` values of a paved way.
const PAVED: [&str; 5] = ["asphalt", "paved", "concrete", "concrete:plates", "chipseal"];

/// How wide a paved track is: a farm or forest road with asphalt on it, metres.
const PAVED_TRACK: f64 = 3.5;

/// Road width in metres and kind of a `highway` way with this `surface` tag, or `None` for ways that
/// are not walkable roads. A track with asphalt on it is an asphalt road, and a footway or cycleway
/// with it a paved path, not a worn one. A cycleway or footway with no `surface` tag is paved: in
/// Sweden it is nearly always an asphalt cycle path or pavement, and the gravel ones are mostly
/// tagged so. A `path` is a trail unless tagged otherwise.
pub(crate) fn road_style(highway: &str, surface: Option<&str>) -> Option<(f64, u8)> {
    let paved = surface.map_or(matches!(highway, "cycleway" | "footway"), |value| PAVED.contains(&value));
    if highway == "track" && paved {
        return Some((PAVED_TRACK, ROAD_MINOR));
    }
    let style = road_class(highway)?;
    Some(if paved && style.1 == ROAD_PATH && highway != "steps" { (style.0, ROAD_PAVED_PATH) } else { style })
}

/// Road width in metres and kind of a `highway` class, whatever it is surfaced with.
fn road_class(highway: &str) -> Option<(f64, u8)> {
    Some(match highway {
        "motorway" | "trunk" | "primary" | "secondary" | "motorway_link" | "trunk_link" | "primary_link" | "secondary_link" => (10.0, ROAD_MAJOR),
        "tertiary" | "tertiary_link" | "residential" | "unclassified" | "living_street" => (7.0, ROAD_MINOR),
        "service" => (4.5, ROAD_MINOR),
        "pedestrian" => (5.0, ROAD_PATH),
        "footway" | "path" | "cycleway" | "steps" | "track" => (2.5, ROAD_PATH),
        _ => return None,
    })
}

/// Gauge in metres and `RAIL_*` kind of a railway way, or `None` for lines that are not drawn:
/// underground, disused, planned, or not a track at all.
pub(crate) fn rail_style(way: &Way) -> Option<(f64, u8)> {
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

/// How far past a road's or track's edge a stream is taken underground, metres, so it comes out
/// beyond the embankment rather than on the shoulder.
const CULVERT_SHOULDER: f64 = 1.5;
/// Steps a stream is walked in to find where it goes under a road, metres.
const CULVERT_STEP: f64 = 0.5;
/// A piece of stream shorter than this between two crossings is left out, metres.
const SHORTEST_STREAM: f64 = 1.0;

/// A stream's centre line without the stretches where it passes under a road or a track (`ways`:
/// centre lines with their widths, metres), so it stops at one side and comes out at the other, as
/// from a culvert. Bridges are not among the ways: a stream runs on under a deck.
fn culverted(line: &[P], ways: &[(&[P], f64)]) -> Vec<Vec<P>> {
    let bounds = |points: &[P], grow: f64| {
        points.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - grow), b[1].min(p[1] - grow), b[2].max(p[0] + grow), b[3].max(p[1] + grow)])
    };
    let own = bounds(line, 0.0);
    let near: Vec<(&[P], f64)> = ways
        .iter()
        .map(|&(points, width)| (points, width / 2.0 + CULVERT_SHOULDER))
        .filter(|&(points, reach)| {
            let b = bounds(points, reach);
            b[0] <= own[2] && b[2] >= own[0] && b[1] <= own[3] && b[3] >= own[1]
        })
        .collect();
    if near.is_empty() || line.len() < 2 {
        return vec![line.to_vec()];
    }
    let under = |p: P| near.iter().any(|&(points, reach)| points.windows(2).any(|pair| distance_to_segment(p, pair[0], pair[1]) < reach));
    let mut runs = Vec::new();
    let mut run: Vec<P> = Vec::new();
    let mut was = under(line[0]);
    if !was {
        run.push(line[0]);
    }
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let at = |t: f64| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / CULVERT_STEP).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            if under(at(t)) == was {
                continue;
            }
            // Halve the step down to the road's reach: `lo` stays on the side the stream came from.
            let (mut lo, mut hi) = ((k - 1) as f64 / steps as f64, t);
            for _ in 0..12 {
                let middle = (lo + hi) / 2.0;
                if under(at(middle)) == was { lo = middle } else { hi = middle }
            }
            if was {
                run.push(at(hi));
            } else {
                run.push(at(lo));
                runs.push(std::mem::take(&mut run));
            }
            was = !was;
        }
        if !was {
            run.push(b);
        }
    }
    runs.push(run);
    runs.retain(|run| run.windows(2).map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum::<f64>() >= SHORTEST_STREAM);
    runs
}

/// Width of the ballast bed under a track of `gauge` metres.
pub(crate) fn ballast(gauge: f64) -> f64 {
    gauge + BALLAST_EXTRA
}

/// Groomed width of a ski track, metres: room for two classic tracks and a skating lane.
const SKI_TRAIL_WIDTH: f64 = 4.0;

/// Cuts `run` (already inside the world) at tile borders and hands each piece, in centimetres, to
/// `f` with the tile it lies in, so each tile holds only its own pieces.
/// How far along `points` the point of it nearest `p` is, metres: where a piece cut from a way starts on it.
fn arc_at(points: &[P], p: P) -> f64 {
    let mut walked = 0.0;
    let mut best = (f64::MAX, 0.0);
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = (b[0] - a[0]).hypot(b[1] - a[1]);
        let t = if length > 1e-9 { (((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / (length * length)).clamp(0.0, 1.0) } else { 0.0 };
        let distance = (a[0] + (b[0] - a[0]) * t - p[0]).hypot(a[1] + (b[1] - a[1]) * t - p[1]);
        if distance < best.0 {
            best = (distance, walked + t * length);
        }
        walked += length;
    }
    best.1
}

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

/// How far from its middle a building looks for the road it stands by, metres.
const FRONT_REACH: f64 = 60.0;

/// The stretches of the `roads` that are driven on (not paths), by the `FRONT_REACH` squares they
/// pass through: where `front_of` looks for a building's road.
fn streets(roads: &[(Vec<P>, f64, u8)]) -> HashMap<(i32, i32), Vec<[P; 2]>> {
    let mut grid: HashMap<(i32, i32), Vec<[P; 2]>> = HashMap::new();
    let cell = |v: f64| (v / FRONT_REACH).floor() as i32;
    for (points, _, _) in roads.iter().filter(|(.., kind)| matches!(*kind, ROAD_MAJOR | ROAD_MINOR | ROAD_TRACK)) {
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            for x in cell(a[0].min(b[0]))..=cell(a[0].max(b[0])) {
                for z in cell(a[1].min(b[1]))..=cell(a[1].max(b[1])) {
                    grid.entry((x, z)).or_default().push([a, b]);
                }
            }
        }
    }
    grid
}

/// `Building::front` for a building whose middle is `mid`: the way to the nearest point of a road
/// among `streets` within `FRONT_REACH`, or `[0, 0]` with none (or with a road through its middle).
fn front_of(mid: P, streets: &HashMap<(i32, i32), Vec<[P; 2]>>) -> [i8; 2] {
    let cell = |v: f64| (v / FRONT_REACH).floor() as i32;
    let mut nearest: Option<(f64, P)> = None;
    for x in cell(mid[0]) - 1..=cell(mid[0]) + 1 {
        for z in cell(mid[1]) - 1..=cell(mid[1]) + 1 {
            for [a, b] in streets.get(&(x, z)).into_iter().flatten() {
                let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
                let t = if dx == 0.0 && dz == 0.0 { 0.0 } else { (((mid[0] - a[0]) * dx + (mid[1] - a[1]) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0) };
                let to = [a[0] + dx * t - mid[0], a[1] + dz * t - mid[1]];
                let far = to[0].hypot(to[1]);
                if far < nearest.map_or(FRONT_REACH, |(least, _)| least) {
                    nearest = Some((far, to));
                }
            }
        }
    }
    match nearest {
        Some((far, to)) if far > 0.5 => [(to[0] / far * 127.0).round() as i8, (to[1] / far * 127.0).round() as i8],
        _ => [0, 0],
    }
}

/// A building to assemble: its footprint, its tagged height, its roof (if the laser gave one), its
/// `[wall, roof]` colours (if the photo gave them), the most it may rise over its floor (`cap`) and
/// what it is (`BUILDING_*`).
type Footprint = (Vec<P>, f64, Option<Top>, Option<[u32; 2]>, Option<f64>, u8);

/// A footprint to fit a roof to (`fit_roofs`): its ring, the most it may rise over its floor (`cap`),
/// the roof shape OpenStreetMap names for it, and what it is (`BUILDING_*`).
type Ring<'a> = (&'a [P], Option<f64>, Option<Category>, u8);

/// A building outline before its roof is fitted: a `Footprint` without the `Top`.
type Outline = (Vec<P>, f64, Option<[u32; 2]>, Option<f64>, u8);

/// Assembles the world from building outlines (metres, any winding) and road polylines, and cuts
/// it into square tiles of `tile_size` metres.
///
/// `ground` is the terrain over the whole grid of tiles (see `crate::ground`), or flat.
/// `trees` are `(position, TREE_*, height, crown radius, planted)`; those that fall on a road or a
/// track, stand too near a building (`thin::clear_of_walls`), or are thinned out of the woods
/// (`thin::woods`) are dropped. A tree is solid (has a trunk collider) when it is planted, in the
/// register, or stands within `SOLID_REACH` of a road, a track or a building.
/// Puts triangles `tris` (metres) into `mesh` in centimetres, corners shared through `shared`, and
/// returns how many went in: none that rounding to the centimetre made degenerate.
fn push_triangles(mesh: &mut CollisionMesh, shared: &mut HashMap<[i32; 3], u32>, tris: &[[V3; 3]]) -> u32 {
    let mut count = 0;
    for tri in tris {
        let mut index = [0u32; 3];
        for (slot, v) in tri.iter().enumerate() {
            let at = [cm(v[0]), cm(v[1]), cm(v[2])];
            index[slot] = *shared.entry(at).or_insert_with(|| {
                mesh.vertices.push(at);
                mesh.vertices.len() as u32 - 1
            });
        }
        if index[0] != index[1] && index[1] != index[2] && index[0] != index[2] {
            mesh.triangles.push(index);
            count += 1;
        }
    }
    count
}

/// `rails` are `(centre line, gauge, RAIL_*)`; `rail_bridges` carry tracks (kind `bridge::TRACK`),
/// or a road that is drawn already, and are built like road bridges, but draw no road. No rocks or shrubs are made up inside `mapped`, an orienteering map's outline.
#[allow(clippy::too_many_arguments)]
fn assemble(frame: Frame, half: f64, tile_size: f64, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, buildings: Vec<Footprint>, roads: Vec<(Vec<P>, f64, u8)>, paints: &[Lanes], bridges: Vec<(Vec<P>, f64, u8, Pylons, Deck)>, rails: Vec<(Vec<P>, f64, u8)>, rail_bridges: Vec<(Vec<P>, f64, u8, Pylons, Deck)>, ski: Vec<Vec<P>>, covers: Vec<(u8, Vec<Vec<P>>)>, trees: Vec<(P, u8, f64, f64, bool)>, mapped: &[P], ground: Ground, water: Placed) -> Built {
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
    let streets = streets(&roads);
    for (mut footprint, height, fitted, paint, ceiling, kind) in buildings {
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
            kind,
            front: front_of(centre, &streets),
            cells: Default::default(),
            paint,
        });
        footprints.push(footprint);
    }
    let mut candidates = Vec::new();
    let side = tile_size;
    // A road on a deck is a road like any other to drive and on the minimap, but it is not drawn on
    // the ground under the deck and never offers a spawn: the ground there is a river bed, or the
    // way under the bridge. That is every bridge's road, and the stretch of a ground road a deck
    // lies over (a deck over a cutting, a bridge lengthened onto its bank).
    let decks: Vec<(&[P], f64)> = bridges.iter().chain(&rail_bridges).filter(|bridge| bridge.3 != Pylons::Carried).map(|(points, width, ..)| (points.as_slice(), *width)).collect();
    let laid = roads.iter().zip(paints).flat_map(|((points, width, kind), paint)| carried_runs(points, &decks).into_iter().map(move |(run, carried)| (run, points, *width, *kind, *paint, carried)));
    for (points, way, width, kind, paint, carried) in laid.chain(bridges.iter().map(|(points, width, kind, ..)| (points.clone(), points, *width, *kind, Lanes::default(), true))) {
        for run in clip_polyline(&points, half) {
            // Sample along each run, not just at vertices, so long straight roads still offer spawns.
            for pair in run.windows(2).filter(|_| !carried) {
                let ([ax, az], [bx, bz]) = (pair[0], pair[1]);
                let steps = ((bx - ax).hypot(bz - az) / 8.0).ceil().max(1.0) as usize;
                candidates.extend((0..=steps).map(|s| {
                    let t = s as f64 / steps as f64;
                    [ax + (bx - ax) * t, az + (bz - az) * t]
                }));
            }
            per_tile(&manifest, half, side, &run, |key, points| {
                let along = points.first().map_or(0, |&[x, z]| cm(arc_at(way, [f64::from(x) / 100.0, f64::from(z) / 100.0])));
                let Lanes { lanes, forward, marks } = paint;
                tiles.entry(key).or_default().roads.push(Road { width: cm(width), kind, carried, points, lanes, forward, marks, along });
            });
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
    for (index, (points, width, kind, pylons, deck)) in bridges.iter().chain(&rail_bridges).enumerate() {
        let mut pieces = bridge::build(points, *width, *kind, *pylons, deck, &ground, &water.levels, inner);
        // No railing where another bridge's deck lies beside this one: the two carriageways of a
        // divided road, whose decks overlap, are one roadway. A deck reaches an edge beam past its road.
        let on_another = |p: P| bridges.iter().chain(&rail_bridges).enumerate().any(|(other, (line, across, _, pylons, _))| other != index && *pylons != Pylons::Carried && line.windows(2).any(|pair| distance_to_segment(p, pair[0], pair[1]) < across / 2.0 + bridge::EDGE_BEAM_WIDTH));
        pieces.retain(|piece| !piece.railing || !on_another(piece.at));
        // No pier in the way of what goes under the bridge: a road, a track, or another bridge's road
        // (but not a carriageway the deck itself carries). Another bridge running alongside, as the
        // other carriageway of a divided road on its own deck, goes beside it, not under it.
        let unit_of = |pair: &[P]| {
            let (dx, dz) = (pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]);
            let length = dx.hypot(dz).max(1e-9);
            [dx / length, dz / length]
        };
        let along = |p: P| points.windows(2).min_by(|a, b| distance_to_segment(p, a[0], a[1]).total_cmp(&distance_to_segment(p, b[0], b[1]))).map(unit_of);
        let under = |p: P| {
            let reach = |across: f64| across / 2.0 + width * 0.55 + PIER_CLEAR;
            let ground = roads.iter().chain(&rails).any(|(line, across, _)| line.windows(2).any(|pair| distance_to_segment(p, pair[0], pair[1]) < reach(*across)));
            let own = along(p);
            let crossing = bridges.iter().chain(&rail_bridges).enumerate().filter(|(other, bridge)| *other != index && bridge.3 != Pylons::Carried).any(|(_, (line, across, ..))| {
                line.windows(2).any(|pair| {
                    let beside = own.is_some_and(|d| {
                        let e = unit_of(pair);
                        (d[0] * e[0] + d[1] * e[1]).abs() > PARALLEL
                    });
                    !beside && distance_to_segment(p, pair[0], pair[1]) < reach(*across)
                })
            });
            ground || crossing
        };
        pieces.retain(|piece| !piece.pier || !under(piece.at));
        // The concrete in sections, each a building of its own that blasts carve (`BUILDING_BRIDGE`), in
        // the tile of its middle; the steel, which nothing breaks, per tile it lies in.
        let concrete: Vec<&bridge::Piece> = pieces.iter().filter(|piece| !piece.steel).collect();
        for section in bridge::sections(&concrete) {
            let parts: Vec<&bridge::Piece> = section.iter().map(|&k| concrete[k]).collect();
            let n = parts.len() as f64;
            let middle = parts.iter().fold([0.0, 0.0], |sum, piece| [sum[0] + piece.at[0] / n, sum[1] + piece.at[1] / n]);
            let corners: Vec<P> = parts.iter().flat_map(|piece| piece.tris.iter().flatten().map(|v| [v[0], v[2]])).collect();
            let footprint = crate::ocad::convex_hull(corners);
            let lowest = parts.iter().flat_map(|piece| piece.tris.iter().flatten().map(|v| v[1])).fold(f64::MAX, f64::min);
            let top = parts.iter().flat_map(|piece| piece.tris.iter().flatten().map(|v| v[1])).fold(f64::MIN, f64::max);
            // What falls from it lands on the ground or the river bed under it.
            let base = parts.iter().map(|piece| ground.height_at(piece.at[0] as f32, piece.at[1] as f32) as f64).fold(lowest, f64::min);
            // The faces of the deck that look up are its roof, the roadway: the rest its walls, the tops
            // of the edge beams and railings among them.
            let (mut roof, mut walls): (Vec<[V3; 3]>, Vec<[V3; 3]>) = (Vec::new(), Vec::new());
            for piece in &parts {
                for &tri in &piece.tris {
                    if piece.deck && bridge::faces_up(tri) { roof.push(tri) } else { walls.push(tri) }
                }
            }
            let tile = tiles.entry(manifest.tile_at(cm(middle[0]), cm(middle[1]))).or_default();
            let first = tile.collision.triangles.len() as u32;
            let mut shared_vertices: HashMap<[i32; 3], u32> = HashMap::new();
            let mut count = [0u32; 2];
            for (group, list) in [&walls, &roof].into_iter().enumerate() {
                count[group] = push_triangles(&mut tile.collision, &mut shared_vertices, list);
            }
            tile.buildings.push(Building {
                height: cm(top - base),
                footprint: footprint.iter().map(|&[x, z]| [cm(x), cm(z)]).collect(),
                base: cm(base),
                first,
                walls: count[0],
                roof: count[1],
                roof_kind: ROOF_FLAT,
                kind: BUILDING_BRIDGE,
                front: [0, 0],
                cells: Default::default(),
                paint: Some([BRIDGE_CONCRETE, bridge::deck_paint(*kind)]),
            });
        }
        let mut steel_by_tile: BTreeMap<(u16, u16), Vec<[V3; 3]>> = BTreeMap::new();
        for piece in pieces.iter().filter(|piece| piece.steel) {
            steel_by_tile.entry(manifest.tile_at(cm(piece.at[0]), cm(piece.at[1]))).or_default().extend(piece.tris.iter().copied());
        }
        for (key, steel) in steel_by_tile {
            let tile = tiles.entry(key).or_default();
            let first = tile.collision.triangles.len() as u32;
            let steel = push_triangles(&mut tile.collision, &mut HashMap::new(), &steel);
            tile.bridges.push(Bridge { first, steel });
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
    // Streams go under the roads and tracks they cross, and are cut at tile borders like roads,
    // keeping the direction of the flow.
    let crossed: Vec<(&[P], f64)> = roads.iter().map(|(points, width, _)| (&points[..], *width)).chain(rails.iter().map(|(points, gauge, _)| (&points[..], ballast(*gauge)))).collect();
    for line in water.streams.iter().flat_map(|line| culverted(line, &crossed)) {
        for run in clip_polyline(&line, half) {
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
    // Each tree that may stand where it is, with whether it is near a road or building.
    let mut standing: Vec<(crate::thin::Planted, bool)> = Vec::new();
    // Trees dropped for standing against a house: `[from the laser, planted]`.
    let mut against_walls = [0usize; 2];
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
        let wall = footprints
            .iter()
            .zip(&footprint_boxes)
            .filter(|(_, b)| in_box(b, at))
            .flat_map(|(footprint, _)| (0..footprint.len()).map(|i| distance_to_segment(at, footprint[i], footprint[(i + 1) % footprint.len()])))
            .fold(f64::MAX, f64::min);
        // A tree keeps its trunk and its crown off a house; rocks and shrubs may stand against one.
        if crate::thin::is_tree(kind) && !crate::thin::clear_of_walls(wall, crown) {
            against_walls[usize::from(planted)] += 1;
            continue;
        }
        standing.push(((at, kind, height, crown, planted), near || wall < SOLID_REACH));
    }
    // The woods are thinned among the trees that stand, so none is crowded out by one that does not.
    let keep = crate::thin::woods(&standing.iter().map(|(tree, _)| *tree).collect::<Vec<_>>());
    if !standing.is_empty() {
        eprintln!("trees: {} against a house ({} of them planted), {} thinned out of the woods", against_walls[0] + against_walls[1], against_walls[1], keep.iter().filter(|keep| !**keep).count());
    }
    for (((at, kind, height, crown, _), near), _) in standing.into_iter().zip(keep).filter(|(_, keep)| *keep) {
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
        if water.levels.len() == ground.heights.len() && water.reach.len() == water.levels.len() {
            let flowing = water.flow.len() == 2 * water.levels.len();
            let all = Ground { origin: ground.origin, cell: ground.cell, side: ground.side, heights: water.levels };
            let reach = Ground { origin: ground.origin, cell: ground.cell, side: ground.side, heights: water.reach.iter().map(|&steps| i32::from(steps)).collect() };
            // The current's two parts, cut as the levels are.
            let part = |k: usize| Ground { origin: ground.origin, cell: ground.cell, side: ground.side, heights: if flowing { water.flow.iter().skip(k).step_by(2).map(|&v| i32::from(v)).collect() } else { Vec::new() } };
            let (flow_x, flow_z) = (part(0), part(1));
            for col in 0..manifest.grid() {
                for row in (0..manifest.grid()).filter(|&row| inside(col, row)) {
                    let block = all.block(col as u32 * per_tile, row as u32 * per_tile, samples);
                    if block.iter().any(|&level| level != NO_WATER) {
                        let tile = tiles.entry((col, row)).or_default();
                        tile.water = block;
                        tile.reach = reach.block(col as u32 * per_tile, row as u32 * per_tile, samples).into_iter().map(|steps| steps as u8).collect();
                        if flowing {
                            let (x, z) = (flow_x.block(col as u32 * per_tile, row as u32 * per_tile, samples), flow_z.block(col as u32 * per_tile, row as u32 * per_tile, samples));
                            if x.iter().chain(&z).any(|&v| v != 0) {
                                tile.flow = x.into_iter().zip(z).flat_map(|(x, z)| [x as i8, z as i8]).collect();
                            }
                        }
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
    // What is painted on each of `roads`, and how many lanes it has.
    let mut paints: Vec<Lanes> = Vec::new();
    // The driven roads, as the rail down a divided road is worked out from, each with its place in `roads`.
    let mut divided: Vec<(usize, crate::barrier::Way)> = Vec::new();
    let mut bridges = Vec::new();
    let mut rails = Vec::new();
    // `(LINE_*, line)`: fences, walls and power lines.
    let mut lines: Vec<(u8, Vec<P>)> = Vec::new();
    let mut rail_bridges = Vec::new();
    let mut covers = Vec::new();
    // The graveyards, each its rings: tombstones are stood in them once the tiles are built (`graves::lay`).
    let mut yards: Vec<Vec<Vec<P>>> = Vec::new();
    // The fields, each its rings: wheat is grown in them once the tiles are built (`lay_crops`).
    let mut fields: Vec<Vec<Vec<P>>> = Vec::new();
    let line = |geometry: &[LatLon]| -> Vec<P> { geometry.iter().map(|g| frame.project(g.lat, g.lon)).collect() };
    for Area { tags, outer, inner, .. } in &osm.areas {
        let Some(kind) = cover_kind(tags) else { continue };
        let rings: Vec<Vec<P>> = join_rings(outer.iter().map(|w| line(w)).collect()).into_iter().chain(join_rings(inner.iter().map(|w| line(w)).collect())).collect();
        if !rings.is_empty() {
            if crate::osm::is_graveyard(tags) {
                yards.push(rings.clone());
            }
            if is_field(tags) {
                fields.push(rings.clone());
            }
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
    let squared = std::sync::atomic::AtomicUsize::new(0);
    let mut hints: Vec<Option<Category>> = Vec::new();
    // What each building is, in the order of the ways: from its tags, the land use it stands in and what stands round it.
    let kinds = {
        let open = |mut ring: Vec<P>| {
            if ring.len() > 1 && ring.first() == ring.last() {
                ring.pop();
            }
            ring
        };
        let mut lands: Vec<(crate::classify::Land, Vec<P>)> = Vec::new();
        for Area { tags, outer, .. } in &osm.areas {
            if let Some(land) = crate::classify::Land::of(tags) {
                lands.extend(join_rings(outer.iter().map(|w| line(w)).collect()).into_iter().map(|ring| (land, open(ring))));
            }
        }
        for way in &osm.ways {
            if let (Some(land), false) = (crate::classify::Land::of(&way.tags), is_building(way)) {
                let ring = line(&way.geometry);
                if ring.len() >= 4 && ring.first() == ring.last() {
                    lands.push((land, open(ring)));
                }
            }
        }
        let rings: Vec<Vec<P>> = osm.ways.iter().filter(|way| is_building(way)).map(|way| open(line(&way.geometry))).collect();
        let sites: Vec<(&crate::osm::Tags, &[P])> = osm.ways.iter().filter(|way| is_building(way)).zip(&rings).map(|(way, ring)| (&way.tags, ring.as_slice())).collect();
        let nodes: Vec<(u8, P)> = osm.pois.iter().filter_map(|node| Some((crate::classify::by_use(&node.tags)?, frame.project(node.at.lat, node.at.lon)))).collect();
        let (kinds, tally) = crate::classify::classify(&sites, &lands, &nodes);
        let [plain, churches, barns, outbuildings, halls, shops, flats, public, cabins, old, villas, fire, _bridges] = tally.kinds;
        eprintln!("buildings: {plain} houses, {old} old houses, {villas} villas, {churches} churches, {barns} barns, {outbuildings} outbuildings, {halls} halls, {shops} shops, {flats} blocks of flats, {public} public, {fire} fire stations, {cabins} cabins; {} known by their tags, {} by a node in them, {} by the land they stand on, {} by their yard", tally.by_tags, tally.by_node, tally.by_land, tally.by_yard);
        kinds
    };
    let mut kinds = kinds.into_iter();
    let outlines: Vec<Outline> = osm
        .ways
        .iter()
        .filter(|way| is_building(way))
        .flat_map(|way| {
            let kind = kinds.next().unwrap_or(BUILDING_PLAIN);
            let drawn = line(&way.geometry);
            let open = if drawn.len() > 1 && drawn.first() == drawn.last() { &drawn[..drawn.len() - 1] } else { &drawn[..] };
            // Walls drawn nearly square are made square (`square`); a footprint cut on France's lattice is left as it is.
            let big = measured && open.len() >= 3 && signed_area(open).abs() > crate::footprints::BIG;
            let points = match crate::square::square(open).filter(|_| !big) {
                Some(ring) => {
                    squared.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    ring
                }
                None => drawn.clone(),
            };
            let ring = if points.len() > 1 && points.first() == points.last() { &points[..points.len() - 1] } else { &points[..] };
            let hint = way.tags.get("roof:shape").and_then(|tag| Category::of_tag(tag));
            // A classified cloud's building returns are roof, whatever stands round it.
            let cap = if measured { None } else { cap(way, ring) };
            let height = cap.map_or(building_height(way), |[eaves, _]| building_height(way).min(eaves));
            let ceiling = cap.map(|[_, top]| top);
            let paint = paint(&[ring]);
            let found: Vec<Outline> = if big {
                crate::footprints::pieces(ring).into_iter().map(|piece| (piece, height, paint, ceiling, kind)).collect()
            } else {
                vec![(points.clone(), height, paint, ceiling, kind)]
            };
            hints.extend(found.iter().map(|_| hint));
            found
        })
        .collect();
    eprintln!("footprints: {} of {} squared", squared.into_inner(), outlines.len());
    let rings: Vec<Ring> = outlines.iter().zip(&hints).map(|((points, _, _, ceiling, kind), hint)| (points.as_slice(), *ceiling, *hint, *kind)).collect();
    let shapes = lidar.map(|cloud| fit_roofs(cloud, &source, measured, half, tile_size, mask, &ground, &rings)).unwrap_or_default();
    let mut shapes = shapes.into_iter();
    for (points, height, paint, ceiling, kind) in outlines {
        // Roofs come from the laser returns over the footprint, where there are any.
        buildings.push((points, height, shapes.next().flatten(), paint, ceiling, kind));
    }
    crate::progress::stage("buildings and roads");
    for (index, way) in osm.ways.iter().enumerate() {
        crate::progress::steps(index, osm.ways.len());
        let points = line(&way.geometry);
        if is_building(way) {
            continue;
        } else if let Some((width, kind, lanes)) = crate::lanes::style(&way.tags) {
            if is_bridge(way) {
                let pylons = pylons_for(frame, &points);
                let deck = decks.map(|cloud| deck_returns(cloud, &source, &points, width)).unwrap_or_default();
                bridges.push((points, width, kind, pylons, deck));
            } else {
                // A bridge keeps its class, which sets how high its deck stands (`bridge::clearance`).
                if matches!(kind, ROAD_MAJOR | ROAD_MINOR) {
                    divided.push((roads.len(), crate::barrier::Way { points: points.clone(), half: width / 2.0, carriageway: is_carriageway(&way.tags), fast: is_fast(&way.tags) }));
                }
                roads.push((points, width, kind));
                paints.push(lanes);
            }
        } else if golf.is_some() && way.tags.get("golf").and_then(|value| crate::osm_golf::surface_kind(value)).is_some() {
            // The course has its surfaces, every one of them, not only those with a landuse too.
            continue;
        } else if let Some(kind) = cover_kind(&way.tags) {
            // Only closed ways are areas; an open one is a bank or a fence.
            if points.len() >= 4 && points.first() == points.last() {
                if crate::osm::is_graveyard(&way.tags) {
                    yards.push(vec![points[..points.len() - 1].to_vec()]);
                }
                if is_field(&way.tags) {
                    fields.push(vec![points[..points.len() - 1].to_vec()]);
                }
                covers.push((kind, vec![points[..points.len() - 1].to_vec()]));
            }
        }
        // Not part of the chain above: a fence often runs round a closed landuse way of its own.
        if let Some(kind) = crate::osm::line_kind(&way.tags) {
            lines.push((kind, line(&way.geometry)));
        }
        // Not part of the chain above: a tram line is often the same way as the street it runs in.
        if let Some((gauge, kind)) = rail_style(way) {
            let points = line(&way.geometry);
            if is_bridge(way) {
                let pylons = pylons_for(frame, &points);
                let deck = decks.map(|cloud| deck_returns(cloud, &source, &points, ballast(gauge))).unwrap_or_default();
                rail_bridges.push((points, ballast(gauge), bridge::TRACK, pylons, deck));
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
            let whole = pieces.len() == 1;
            for ring in pieces {
                // A building in one piece is squared as OSM's are.
                let ring = if whole { crate::square::square(&ring).unwrap_or(ring) } else { ring };
                // Moored barges and houseboats are building returns too; a building stands on land.
                let n = ring.len() as f64;
                let mid = ring.iter().fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
                if bridge::water_at(&ground, &water.levels, mid).is_some() {
                    continue;
                }
                let returns = cloud.over(&ring, source);
                if let Some(top) = from_catalogue(&ring, &returns, true, None, None, &Tally::default()).or_else(|| crate::field::Field::measure(&ring, &returns).map(Top::Measured)) {
                    buildings.push((ring, 0.0, Some(top), paint, None, BUILDING_PLAIN));
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
        for (.., paint, _, _) in &buildings {
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
        paints.resize(roads.len(), Lanes::default());
        covers = covers.into_iter().flat_map(|(kind, rings)| if is_golf_cover(kind) { vec![rings] } else { ocad.outside(&rings) }.into_iter().map(move |rings| (kind, rings))).collect();
        covers.extend(ocad.covers.iter().cloned());
        // The map's cultivated land is its fields; OSM's inside its outline went with OSM's cover.
        fields.extend(ocad.covers.iter().filter(|(kind, _)| *kind == COVER_FARM).map(|(_, rings)| rings.clone()));
        mapped.clone_from(&ocad.outline);
    }
    // After the map's own, which would cut them away where it covers the course.
    covers.extend(course);
    // OSM's fences, walls and power lines, then the map's own, which replace OSM's inside its outline.
    let fences = lines.iter().filter(|(kind, _)| matches!(*kind, LINE_FENCE | LINE_FENCE_HIGH)).count();
    let walls = lines.iter().filter(|(kind, _)| matches!(*kind, LINE_WALL | LINE_WALL_HIGH)).count();
    eprintln!("osm: {fences} fences, {walls} walls, {} power lines", lines.len() - fences - walls);
    if let Some(ocad) = ocad {
        lines.extend(ocad.lines.iter().cloned());
    }
    // A divided road has a guard rail down its middle, and the strip between close carriageways is paved.
    let (at, ways): (Vec<usize>, Vec<crate::barrier::Way>) = divided.into_iter().unzip();
    let barrier = crate::barrier::lay(&ways);
    for (index, beside) in at.iter().zip(&barrier.beside) {
        paints[*index].marks |= match beside {
            crate::barrier::Beside::Rail => MARK_NO_LEFT_EDGE,
            crate::barrier::Beside::Paved => MARK_LEFT_SOLID,
            crate::barrier::Beside::Open => 0,
        };
    }
    eprintln!("roads: {} guard rails down divided roads, {:.1} km", barrier.rails.len(), barrier.rails.iter().flat_map(|rail| rail.windows(2)).map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1])).fold(0.0, |sum, length| sum + length) / 1000.0);
    lines.extend(barrier.rails.into_iter().map(|rail| (LINE_GUARDRAIL, rail)));
    for (points, width) in barrier.strips {
        roads.push((points, width, ROAD_MAJOR));
        paints.push(Lanes::default());
    }
    // A gate is a node in OSM, which is not read: a wall is opened where a road goes through it.
    let lines: Vec<(u8, Vec<P>)> = lines
        .into_iter()
        .flat_map(|(kind, points)| if matches!(kind, LINE_WALL | LINE_WALL_HIGH) { open_at_roads(&points, &roads) } else { vec![points] }.into_iter().map(move |points| (kind, points)))
        .collect();
    // A power line runs down a corridor cleared of trees; shrubs may grow under it. The laser reads
    // the wires and pylons as trees too, and those go with it.
    let before = trees.len();
    let corridors = power_corridors(&lines);
    trees.retain(|&(at, kind, ..)| !crate::thin::is_tree(kind) || !under_power(at, &corridors));
    eprintln!("trees: {} cleared from under power lines", before - trees.len());
    // The lines and the tombstones are laid on the ground once the tiles are built, so it is kept for them.
    let kept = (!lines.is_empty() || !lifts.is_empty() || !yards.is_empty()).then(|| ground.clone());
    // A lift's own station stands at each of its ends: the station building OSM or the laser has there goes.
    let ends: Vec<P> = lifts.iter().flat_map(|(.., points)| points.first().into_iter().chain(points.last())).copied().collect();
    let over_station = |ring: &[P]| ends.iter().any(|&end| point_in_polygon(end, ring) || (0..ring.len()).any(|i| distance_to_segment(end, ring[i], ring[(i + 1) % ring.len()]) < LIFT_STATION_CLEAR));
    buildings.retain(|(ring, ..)| !over_station(ring));
    // A road over a cutting OSM has no bridge for gets a deck, or it has a trench across it.
    let cuts = cuttings(&roads, &ground);
    eprintln!("bridges: {} decks over cuttings the roads cross", cuts.len());
    bridges.extend(cuts.into_iter().map(|(points, width, kind)| (points, width, kind, Pylons::Flush, Deck::new())));
    let before = bridges.len();
    let bridges = clear_of_bridges(onto_the_banks(joined_bridges(bridges), &roads, &ground));
    let (bridges, shared) = twin_decks(bridges);
    let (bridges, shared) = paths_alongside(bridges, shared);
    eprintln!("bridges: {before} ways built as {} bridges, {} of them one deck under two carriageways", bridges.len() - shared.len(), shared.len());
    // A shared deck draws no road of its own, as a railway's does not: its carriageways are the roads.
    // Nor does a deck over a cutting, whose road is among the roads already.
    rail_bridges.extend(shared);
    let (flush, bridges): (Vec<_>, Vec<_>) = bridges.into_iter().partition(|bridge| bridge.3 == Pylons::Flush);
    // A guard rail or a fence over a cutting stands on its deck, not down in it.
    let decks: Vec<(Vec<P>, f64)> = flush.iter().map(|(points, width, ..)| (points.clone(), *width)).collect();
    rail_bridges.extend(flush);
    // The stones follow a graveyard's paths and stand off every road: the roads as they are drawn, not cut at the tiles.
    let ways: Vec<(Vec<P>, f64)> = if yards.is_empty() { Vec::new() } else { roads.iter().map(|(points, width, _)| (points.clone(), *width)).collect() };
    let mut built = assemble(frame, half, tile_size, mask, buildings, roads, &paints, bridges, rails, rail_bridges, ski, covers, trees, &mapped, ground, water);
    if let Some(ground) = &kept {
        lay_lifts(&mut built, &lifts, half, ground);
        lay_lines(&mut built, &lines, &decks, half, tile_size, ground);
        crate::graves::lay(&mut built, &yards, &ways, half, ground);
        let stones: usize = built.tiles.values().map(|tile| tile.graves.len()).sum();
        eprintln!("graveyards: {} from OpenStreetMap, {stones} tombstones", yards.len());
    }
    if let Some(ocad) = ocad {
        lay_stones(&mut built, &ocad.stony_dots, &ocad.stony_areas, half, tile_size);
    }
    lay_crops(&mut built, &fields, half, tile_size);
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

/// Side of a cell of a tile's `crops`, metres, about: the tile is cut into a whole number of them.
const CROPS_CELL: f64 = 2.0;
/// Water deeper than this over the ground, centimetres, grows nothing.
const CROPS_WADING: i32 = 15;

/// Calls `paint(row, column)` for every cell of an `n × n` grid of `cell` metres from `(x0, z0)` whose
/// centre is inside `rings` (even-odd, so inner rings are holes).
fn fill_cells(rings: &[Vec<P>], x0: f64, z0: f64, cell: f64, n: usize, mut paint: impl FnMut(usize, usize)) {
    let mut crossings: Vec<f64> = Vec::new();
    for row in 0..n {
        let z = z0 + (row as f64 + 0.5) * cell;
        crossings.clear();
        for ring in rings.iter().filter(|ring| ring.len() > 2) {
            let mut b = ring[ring.len() - 1];
            for &a in ring {
                if (a[1] > z) != (b[1] > z) {
                    crossings.push(a[0] + (z - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
                b = a;
            }
        }
        crossings.sort_by(f64::total_cmp);
        for pair in crossings.chunks_exact(2) {
            let from = ((pair[0] - x0) / cell - 0.5).ceil().max(0.0) as usize;
            let to = ((pair[1] - x0) / cell - 0.5).floor();
            if to < 0.0 {
                continue;
            }
            for column in from..=(to as usize).min(n - 1) {
                paint(row, column);
            }
        }
    }
}

/// Calls `paint(row, column)` for every cell of the grid whose centre is within `reach` metres of the
/// line through `points`.
fn stroke_cells(points: &[P], reach: f64, x0: f64, z0: f64, cell: f64, n: usize, mut paint: impl FnMut(usize, usize)) {
    let span = |lo: f64, hi: f64, origin: f64| (((lo - reach - origin) / cell).floor().max(0.0) as usize, (((hi + reach - origin) / cell).ceil().max(0.0) as usize).min(n));
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let (i0, i1) = span(a[0].min(b[0]), a[0].max(b[0]), x0);
        let (j0, j1) = span(a[1].min(b[1]), a[1].max(b[1]), z0);
        for row in j0..j1 {
            for column in i0..i1 {
                let at = [x0 + (column as f64 + 0.5) * cell, z0 + (row as f64 + 0.5) * cell];
                if distance_to_segment(at, a, b) <= reach {
                    paint(row, column);
                }
            }
        }
    }
}

/// Fills each tile's `crops`: wheat on the farmland of `fields` (each a field's rings, metres) where
/// the tile's cover is still farmland once the woods, parks, marshes and water over it are painted,
/// and not on a road, a track's ballast, a stream or deeper water, nor under a building. The cover's
/// farmyards, orchards and nurseries are in no field, and stay bare.
fn lay_crops(built: &mut Built, fields: &[Vec<Vec<P>>], half: f64, tile_size: f64) {
    if fields.is_empty() {
        return;
    }
    let n = (tile_size / CROPS_CELL).round().max(1.0) as usize;
    let cell = tile_size / n as f64;
    let metres = |points: &[[i32; 2]]| -> Vec<P> { points.iter().map(|p| [f64::from(p[0]) / 100.0, f64::from(p[1]) / 100.0]).collect() };
    let bounds: Vec<Option<[f64; 4]>> = fields.iter().map(|rings| crate::props::bounds(rings)).collect();
    for (&(col, row), tile) in &mut built.tiles {
        if !tile.cover.iter().any(|cover| cover.kind == COVER_FARM) {
            continue;
        }
        let (x0, z0) = (-half + f64::from(col) * tile_size, -half + f64::from(row) * tile_size);
        // The cover on top in each cell, as the client paints it: in the tile's order, the last over the rest.
        let mut top = vec![u8::MAX; n * n];
        for cover in &tile.cover {
            let rings: Vec<Vec<P>> = cover.rings.iter().map(|ring| metres(ring)).collect();
            fill_cells(&rings, x0, z0, cell, n, |j, i| top[j * n + i] = cover.kind);
        }
        let mut crops = vec![CROP_NONE; n * n];
        for (rings, b) in fields.iter().zip(&bounds) {
            let Some(b) = b else { continue };
            if b[2] < x0 || b[0] > x0 + tile_size || b[3] < z0 || b[1] > z0 + tile_size {
                continue;
            }
            fill_cells(rings, x0, z0, cell, n, |j, i| {
                if top[j * n + i] == COVER_FARM {
                    crops[j * n + i] = CROP_WHEAT;
                }
            });
        }
        for road in &tile.roads {
            stroke_cells(&metres(&road.points), f64::from(road.width) / 200.0, x0, z0, cell, n, |j, i| crops[j * n + i] = CROP_NONE);
        }
        for rail in &tile.rails {
            stroke_cells(&metres(&rail.points), ballast(f64::from(rail.gauge) / 100.0) / 2.0, x0, z0, cell, n, |j, i| crops[j * n + i] = CROP_NONE);
        }
        for stream in &tile.streams {
            stroke_cells(&metres(&stream.points), f64::from(stream.width) / 200.0 + cell / 2.0, x0, z0, cell, n, |j, i| crops[j * n + i] = CROP_NONE);
        }
        for building in tile.buildings.iter().filter(|building| building.kind != BUILDING_BRIDGE) {
            fill_cells(&[metres(&building.footprint)], x0, z0, cell, n, |j, i| crops[j * n + i] = CROP_NONE);
        }
        // Water is sampled where the ground is: the sample nearest a cell's centre speaks for it.
        let samples = (tile.ground.len() as f64).sqrt().round() as usize;
        if samples > 1 && tile.water.len() == tile.ground.len() {
            let step = tile_size / (samples - 1) as f64;
            for j in 0..n {
                for i in 0..n {
                    let nearest = |k: usize| ((((k as f64 + 0.5) * cell) / step).round() as usize).min(samples - 1);
                    let at = nearest(j) * samples + nearest(i);
                    if tile.water[at] != NO_WATER && tile.water[at] - tile.ground[at] > CROPS_WADING {
                        crops[j * n + i] = CROP_NONE;
                    }
                }
            }
        }
        if crops.iter().any(|&value| value != CROP_NONE) {
            tile.crops = crops;
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

/// Half the width of the corridor cleared through the forest under a power line, metres: a local
/// line on wooden poles, then a main line on steel pylons.
const POLE_CORRIDOR: f64 = 7.0;
const PYLON_CORRIDOR: f64 = 20.0;

/// A cleared corridor under a power line: its half width, its bounding box grown by that, and the line.
type Corridor<'a> = (f64, [f64; 4], &'a [P]);

/// The corridors under the power lines among `lines`.
fn power_corridors(lines: &[(u8, Vec<P>)]) -> Vec<Corridor<'_>> {
    lines
        .iter()
        .filter_map(|(kind, points)| {
            let half = match *kind {
                LINE_POWER => POLE_CORRIDOR,
                LINE_POWER_MAJOR => PYLON_CORRIDOR,
                _ => return None,
            };
            let b = points.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - half), b[1].min(p[1] - half), b[2].max(p[0] + half), b[3].max(p[1] + half)]);
            Some((half, b, &points[..]))
        })
        .collect()
}

/// Whether `at` stands in one of the cleared `corridors` under a power line.
fn under_power(at: P, corridors: &[Corridor]) -> bool {
    corridors.iter().any(|&(half, b, points)| {
        at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3] && points.windows(2).any(|pair| distance_to_segment(at, pair[0], pair[1]) < half)
    })
}

/// Half the width of the lane cleared through the forest under a ski lift, metres.
const LIFT_LANE: f64 = 7.0;
/// Another bridge whose line runs within this cosine of a bridge's own direction runs alongside it,
/// not under it: its piers stand.
const PARALLEL: f64 = 0.9;
/// A pier stands this far clear of the edge of a road or a track under its bridge, metres.
const PIER_CLEAR: f64 = 1.0;

/// A road bridge as `from_osm` gathers it: its centre line, width, `ROAD_*`, pylons and deck returns.
type BridgeWay = (Vec<P>, f64, u8, Pylons, Deck);

/// Bridge ways that end within this of each other are one bridge, metres.
const BRIDGE_JOIN: f64 = 1.0;

/// OSM cuts a long bridge into several ways (a change of lanes, a speed limit), and each built on its
/// own would come down to the ground where it ends: ways of a kind that meet end to end, two at a
/// point, are joined into one bridge. Decks over cuttings stay as they are.
fn joined_bridges(mut bridges: Vec<BridgeWay>) -> Vec<BridgeWay> {
    let near = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) < BRIDGE_JOIN;
    loop {
        let ends = |bridge: &BridgeWay| [bridge.0[0], bridge.0[bridge.0.len() - 1]];
        let joinable = |bridge: &BridgeWay| bridge.0.len() >= 2 && bridge.3 != Pylons::Flush;
        // How many bridge ends meet at `p`: more than two is a fork, where nothing is joined.
        let meeting = |p: P| bridges.iter().filter(|bridge| bridge.0.len() >= 2).flat_map(&ends).filter(|&end| near(end, p)).count();
        let mut pair = None;
        'search: for i in 0..bridges.len() {
            for j in i + 1..bridges.len() {
                if !joinable(&bridges[i]) || !joinable(&bridges[j]) || bridges[i].2 != bridges[j].2 || bridges[i].1 != bridges[j].1 {
                    continue;
                }
                for (a, &end) in ends(&bridges[i]).iter().enumerate() {
                    for (b, &other) in ends(&bridges[j]).iter().enumerate() {
                        if near(end, other) && meeting(end) == 2 {
                            pair = Some((i, j, a == 0, b == 0));
                            break 'search;
                        }
                    }
                }
            }
        }
        let Some((i, j, at_start, other_start)) = pair else { return bridges };
        let (mut second, _, _, pylons, returns) = bridges.remove(j);
        let first = &mut bridges[i];
        // The first runs up to the joint and the second on from it.
        if at_start {
            first.0.reverse();
        }
        if !other_start {
            second.reverse();
        }
        first.0.extend(second.into_iter().skip(1));
        first.4.extend(returns);
        if first.3 == Pylons::None {
            first.3 = pylons;
        }
    }
}

/// How far along the road past a bridge's end the top of the bank is looked for, metres.
const BANK_REACH: f64 = 40.0;
/// A bridge is lengthened when the road rises this much past its end, metres.
const BANK_RISE: f64 = 1.0;

/// OSM often ends a bridge part of the way down the bank it springs from, and the terrain has the
/// bank as it is under the bridge: built between the ground at its ends, the deck would stop metres
/// below the road with a wall of earth before it. Each bridge whose road goes on rising past an end
/// is lengthened along that road (the `roads` and the other bridges that start where it ends) to the
/// top of the rise, as far as `BANK_REACH`. Decks over cuttings start and end on the road already.
fn onto_the_banks(mut bridges: Vec<BridgeWay>, roads: &[(Vec<P>, f64, u8)], ground: &Ground) -> Vec<BridgeWay> {
    let near = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) < BRIDGE_JOIN;
    let height = |p: P| f64::from(ground.height_at(p[0] as f32, p[1] as f32));
    // The road on from `end`, as points from there: the first way, not `own`, that starts or ends at it.
    let onward = |end: P, own: usize, bridges: &[BridgeWay]| -> Option<Vec<P>> {
        let ways = roads.iter().map(|road| &road.0).chain(bridges.iter().enumerate().filter(|(index, _)| *index != own).map(|(_, bridge)| &bridge.0));
        ways.filter(|way| way.len() >= 2).find_map(|way| {
            if near(way[0], end) {
                Some(way.clone())
            } else if near(way[way.len() - 1], end) {
                Some(way.iter().rev().copied().collect())
            } else {
                None
            }
        })
    };
    for index in 0..bridges.len() {
        if bridges[index].3 == Pylons::Flush || bridges[index].0.len() < 2 {
            continue;
        }
        for at_start in [true, false] {
            let end = if at_start { bridges[index].0[0] } else { bridges[index].0[bridges[index].0.len() - 1] };
            let Some(way) = onward(end, index, &bridges) else { continue };
            // The road past the end, a point every couple of metres as far as the reach.
            let along: Vec<P> = bridge::resample(&way, CUT_STEP).into_iter().take((BANK_REACH / CUT_STEP) as usize + 1).collect();
            let top = along.iter().map(|&p| height(p)).fold(f64::MIN, f64::max);
            if top - height(end) < BANK_RISE {
                continue;
            }
            // As far as where the road is first up on the bank.
            let Some(reached) = along.iter().position(|&p| height(p) >= top - 0.3) else { continue };
            let extra = along[1..=reached.max(1).min(along.len() - 1)].to_vec();
            let points = &mut bridges[index].0;
            if at_start {
                points.splice(0..0, extra.into_iter().rev());
            } else {
                points.extend(extra);
            }
        }
    }
    bridges
}

/// A road's end at a bridge often dips down the bank, which reads as a cutting, and the bridge is
/// then lengthened over it (`onto_the_banks`): a deck over a cutting with half its length or more
/// on a bridge's line is left out, where it would be a second small deck under the bridge.
fn clear_of_bridges(mut bridges: Vec<BridgeWay>) -> Vec<BridgeWay> {
    let covered = |cut: &BridgeWay, bridges: &[BridgeWay]| {
        let on = |p: P| bridges.iter().filter(|bridge| bridge.3 != Pylons::Flush).any(|(line, width, ..)| line.windows(2).any(|pair| distance_to_segment(p, pair[0], pair[1]) < width / 2.0));
        2 * cut.0.iter().filter(|&&p| on(p)).count() >= cut.0.len()
    };
    let keep: Vec<bool> = bridges.iter().map(|bridge| bridge.3 != Pylons::Flush || !covered(bridge, &bridges)).collect();
    let mut keep = keep.into_iter();
    bridges.retain(|_| keep.next().unwrap_or(true));
    bridges
}

/// How far apart a road near a deck is tested for lying on it, metres.
const CARRY_STEP: f64 = 2.0;
/// A road lies on a deck where it runs along it, within this of the deck's line in direction (the
/// cosine of the angle between them): a road that crosses under a bridge is on the ground.
const CARRY_ALONG: f64 = 0.8;

/// Cuts a ground road into runs on the ground (`false`) and runs a deck carries (`true`): where it
/// runs along a deck's line within half the deck's width. `decks` are `(centre line, width)`. Runs
/// meet at a shared point, and a road near no deck comes back whole and unchanged.
fn carried_runs(points: &[P], decks: &[(&[P], f64)]) -> Vec<(Vec<P>, bool)> {
    let near: Vec<&(&[P], f64)> = decks
        .iter()
        .filter(|(line, width)| {
            let grow = width / 2.0 + CARRY_STEP;
            let (x0, z0) = line.iter().fold((f64::MAX, f64::MAX), |(x, z), p| (x.min(p[0] - grow), z.min(p[1] - grow)));
            let (x1, z1) = line.iter().fold((f64::MIN, f64::MIN), |(x, z), p| (x.max(p[0] + grow), z.max(p[1] + grow)));
            points.iter().any(|p| p[0] >= x0 && p[0] <= x1 && p[1] >= z0 && p[1] <= z1) || points.windows(2).any(|pair| clip_segment(pair[0], pair[1], [x0, z0, x1, z1]).is_some())
        })
        .collect();
    if near.is_empty() || points.len() < 2 {
        return vec![(points.to_vec(), false)];
    }
    let on_deck = |a: P, b: P| {
        let (mid, d) = ([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0], [b[0] - a[0], b[1] - a[1]]);
        near.iter().any(|(line, width)| {
            line.windows(2).any(|pair| {
                let e = [pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]];
                let span = e[0].hypot(e[1]).max(1e-9);
                let along = (d[0] * e[0] + d[1] * e[1]).abs() / (d[0].hypot(d[1]) * span).max(1e-9);
                // Beside the segment, not past its ends (but for a metre, which closes the outside of a bend).
                let run = ((mid[0] - pair[0][0]) * e[0] + (mid[1] - pair[0][1]) * e[1]) / span;
                along > CARRY_ALONG && run > -1.0 && run < span + 1.0 && distance_to_segment(mid, pair[0], pair[1]) < width / 2.0
            })
        })
    };
    // Each segment in steps of `CARRY_STEP`: a run ends where the next step is on the other side.
    let mut runs: Vec<(Vec<P>, bool)> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / CARRY_STEP).ceil().max(1.0) as usize;
        let at = |k: usize| [a[0] + (b[0] - a[0]) * k as f64 / steps as f64, a[1] + (b[1] - a[1]) * k as f64 / steps as f64];
        let mut from = 0;
        for k in 0..steps {
            let carried = on_deck(at(k), at(k + 1));
            // The steps of this segment so far are one straight piece, ended where the side changes.
            if k + 1 == steps || on_deck(at(k + 1), at(k + 2)) != carried {
                match runs.last_mut() {
                    Some((run, side)) if *side == carried => run.push(at(k + 1)),
                    _ => runs.push((vec![at(from), at(k + 1)], carried)),
                }
                from = k + 1;
            }
        }
    }
    runs
}

/// The two carriageways of a divided road cross on bridges of their own, side by side, and their
/// decks, each as wide as a whole road, would lie over each other at two heights: two bridges of a
/// kind that run beside each other all the way share one deck, wide enough for both. Returns the
/// bridges, the two of a pair now roads with no deck of their own (`Pylons::Carried`), and the decks
/// they share.
fn twin_decks(mut bridges: Vec<BridgeWay>) -> (Vec<BridgeWay>, Vec<BridgeWay>) {
    let off = |p: P, line: &[P]| line.windows(2).map(|pair| distance_to_segment(p, pair[0], pair[1])).fold(f64::MAX, f64::min);
    // How far apart `a` runs from `b`, when every point of it is within one and a half road widths.
    let beside = |a: &BridgeWay, b: &BridgeWay| {
        let mut apart: Vec<f64> = a.0.iter().map(|&p| off(p, &b.0)).collect();
        apart.sort_by(f64::total_cmp);
        (a.0.len() >= 2 && b.0.len() >= 2 && apart[apart.len() - 1] < 1.5 * a.1).then(|| apart[apart.len() / 2])
    };
    let mut shared = Vec::new();
    for i in 0..bridges.len() {
        if bridges[i].3 == Pylons::Carried {
            continue;
        }
        let twin = (i + 1..bridges.len()).find_map(|j| {
            let same = bridges[j].3 != Pylons::Carried && bridges[i].2 == bridges[j].2 && (bridges[i].3 == Pylons::Flush) == (bridges[j].3 == Pylons::Flush);
            let apart = beside(&bridges[i], &bridges[j])?;
            beside(&bridges[j], &bridges[i]).filter(|_| same).map(|_| (j, apart))
        });
        let Some((j, apart)) = twin else { continue };
        // The line between the two, each point moved half way to the other carriageway.
        let other = &bridges[j].0;
        let middle: Vec<P> = bridges[i]
            .0
            .iter()
            .map(|&p| {
                let nearest = other.windows(2).map(|pair| closest_on_segment(p, pair[0], pair[1])).min_by(|a, b| (a[0] - p[0]).hypot(a[1] - p[1]).total_cmp(&(b[0] - p[0]).hypot(b[1] - p[1])));
                nearest.map_or(p, |q| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0])
            })
            .collect();
        let pylons = if bridges[i].3 == Pylons::None { bridges[j].3 } else { bridges[i].3 };
        let returns: Deck = bridges[i].4.iter().chain(&bridges[j].4).copied().collect();
        shared.push((middle, bridges[i].1 + apart, bridges[i].2, pylons, returns));
        // Two decks over a cutting were never roads of their own: they are gone, for the one they share.
        let gone = bridges[i].3 == Pylons::Flush;
        for way in [i, j] {
            bridges[way].3 = Pylons::Carried;
            if gone {
                bridges[way].0.clear();
            }
        }
    }
    bridges.retain(|bridge| !bridge.0.is_empty());
    (bridges, shared)
}

/// A footway or cycleway beside a road bridge is on it, metres: every point of each within this of
/// the other's edge, beyond the deck's half width and the path's.
const PATH_BESIDE: f64 = 4.0;

/// A path crossing beside a road bridge (`ROAD_PATH`, `ROAD_PAVED_PATH`, `ROAD_TRACK`) is on the same
/// bridge in reality, as a footway or cycleway along it: rather than a narrow bridge of its own with
/// a row of piers beside the road's, it is carried on the road's deck, widened out over it. The path
/// and a road bridge that had its own deck become roads with none (`Pylons::Carried`), and the deck
/// they share goes among `shared`, as `twin_decks` leaves them.
fn paths_alongside(mut bridges: Vec<BridgeWay>, mut shared: Vec<BridgeWay>) -> (Vec<BridgeWay>, Vec<BridgeWay>) {
    let off = |p: P, line: &[P]| line.windows(2).map(|pair| distance_to_segment(p, pair[0], pair[1])).fold(f64::MAX, f64::min);
    let path = |kind: u8| matches!(kind, ROAD_PATH | ROAD_PAVED_PATH | ROAD_TRACK);
    let open = |way: &BridgeWay| way.0.len() >= 2 && way.3 != Pylons::Carried && way.3 != Pylons::Flush;
    // How far the path's line runs from the deck's on the whole, when each runs beside the other all the way.
    let beside = |deck: &BridgeWay, way: &BridgeWay| {
        let reach = deck.1 / 2.0 + way.1 / 2.0 + PATH_BESIDE;
        let mut apart: Vec<f64> = way.0.iter().map(|&p| off(p, &deck.0)).collect();
        apart.sort_by(f64::total_cmp);
        (apart[apart.len() - 1] < reach && deck.0.iter().all(|&p| off(p, &way.0) < reach)).then(|| apart[apart.len() / 2])
    };
    for k in 0..bridges.len() {
        if !open(&bridges[k]) || !path(bridges[k].2) {
            continue;
        }
        // The deck: one already shared, or a road bridge's own.
        let in_shared = (0..shared.len()).find(|&d| open(&shared[d]) && !path(shared[d].2) && beside(&shared[d], &bridges[k]).is_some());
        let in_bridges = || (0..bridges.len()).find(|&d| d != k && open(&bridges[d]) && !path(bridges[d].2) && beside(&bridges[d], &bridges[k]).is_some());
        // `Ok` a deck among `shared`, `Err` a road bridge among `bridges`.
        let Some(target) = in_shared.map(Ok).or_else(|| in_bridges().map(Err)) else { continue };
        let deck = match target {
            Ok(d) => shared[d].clone(),
            Err(d) => bridges[d].clone(),
        };
        let way = &bridges[k];
        let Some(apart) = beside(&deck, way) else { continue };
        // From the deck's far edge to the path's: its line moves over by half what it grows.
        let width = deck.1 / 2.0 + apart + way.1 / 2.0;
        let shift = (width - deck.1) / 2.0;
        let line: Vec<P> = deck
            .0
            .iter()
            .map(|&p| {
                let nearest = way.0.windows(2).map(|pair| closest_on_segment(p, pair[0], pair[1])).min_by(|a, b| (a[0] - p[0]).hypot(a[1] - p[1]).total_cmp(&(b[0] - p[0]).hypot(b[1] - p[1])));
                nearest.map_or(p, |q| {
                    let d = (q[0] - p[0]).hypot(q[1] - p[1]).max(1e-9);
                    [p[0] + (q[0] - p[0]) / d * shift, p[1] + (q[1] - p[1]) / d * shift]
                })
            })
            .collect();
        let returns: Deck = deck.4.iter().chain(&way.4).copied().collect();
        let merged = (line, width, deck.2, deck.3, returns);
        match target {
            Ok(d) => shared[d] = merged,
            Err(d) => {
                bridges[d].3 = Pylons::Carried;
                shared.push(merged);
            }
        }
        bridges[k].3 = Pylons::Carried;
    }
    (bridges, shared)
}

/// The point of the segment `a`-`b` nearest `p`.
fn closest_on_segment(p: P, a: P, b: P) -> P {
    let d = [b[0] - a[0], b[1] - a[1]];
    let span = d[0] * d[0] + d[1] * d[1];
    let t = if span < 1e-12 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / span).clamp(0.0, 1.0) };
    [a[0] + d[0] * t, a[1] + d[1] * t]
}

/// How far apart the ground under a road is read for cuttings, metres.
const CUT_STEP: f64 = 2.0;
/// A dip in the ground under a road narrower than twice this is a cutting, metres: wider is a
/// valley the road goes down into.
const CUT_REACH: f64 = 12.0;
/// A cutting is this much below the road either side of it, at least, metres.
const CUT_DEPTH: f64 = 1.5;
/// A deck over a cutting starts this far back from its edges, metres, on level ground.
const CUT_MARGIN: f64 = 4.0;

/// The stretches of the drivable `roads` that cross a cutting: a narrow trench in the ground under the
/// road, as the terrain has where a path goes under a road that OSM does not map as a bridge. Each
/// is `(centre line, width, ROAD_*)`, to be built as a bridge, which keeps the way under it open.
///
/// The ground along the road is closed over (the highest ground within `CUT_REACH` either way, then
/// the lowest of that), which follows any slope but fills a dip narrower than twice the reach; where
/// the ground is `CUT_DEPTH` under that, the road is over a cutting.
fn cuttings(roads: &[(Vec<P>, f64, u8)], ground: &Ground) -> Vec<(Vec<P>, f64, u8)> {
    let reach = (CUT_REACH / CUT_STEP) as usize;
    let margin = (CUT_MARGIN / CUT_STEP) as usize;
    let mut found = Vec::new();
    for (points, width, kind) in roads.iter().filter(|(.., kind)| matches!(*kind, ROAD_MAJOR | ROAD_MINOR)) {
        let samples = bridge::resample(points, CUT_STEP);
        if samples.len() < 2 * reach {
            continue;
        }
        let heights: Vec<f64> = samples.iter().map(|p| f64::from(ground.height_at(p[0] as f32, p[1] as f32))).collect();
        let around = |i: usize| i.saturating_sub(reach)..(i + reach + 1).min(heights.len());
        let raised: Vec<f64> = (0..heights.len()).map(|i| heights[around(i)].iter().copied().fold(f64::MIN, f64::max)).collect();
        let closed: Vec<f64> = (0..heights.len()).map(|i| raised[around(i)].iter().copied().fold(f64::MAX, f64::min)).collect();
        let mut i = 0;
        while i < heights.len() {
            if closed[i] - heights[i] < CUT_DEPTH {
                i += 1;
                continue;
            }
            // The whole dip, out to where the ground is the road's again, and a little past it.
            let level = |k: usize| closed[k] - heights[k] < 0.2;
            let from = (0..i).rev().find(|&k| level(k)).unwrap_or(0).saturating_sub(margin);
            let to = ((i..heights.len()).find(|&k| level(k)).unwrap_or(heights.len() - 1) + margin).min(heights.len() - 1);
            found.push((samples[from..=to].to_vec(), *width, *kind));
            i = to + 1;
        }
    }
    found
}

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

/// How far to each side of a road's edge a wall it crosses is opened, metres.
const WALL_GAP: f64 = 0.5;
/// How finely a wall is walked to find where a road crosses it, metres.
const WALL_PROBE: f64 = 0.25;

/// The runs of the wall along `points` that stand clear of every road (`(line, width, kind)`): where
/// one crosses, the wall is opened to the road's width and `WALL_GAP` more on each side.
fn open_at_roads(points: &[P], roads: &[(Vec<P>, f64, u8)]) -> Vec<Vec<P>> {
    let Some(reach) = roads.iter().map(|(_, width, _)| width / 2.0 + WALL_GAP).reduce(f64::max) else { return vec![points.to_vec()] };
    let bounds = |points: &[P], grow: f64| points.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0] - grow), b[1].min(p[1] - grow), b[2].max(p[0] + grow), b[3].max(p[1] + grow)]);
    let wall = bounds(points, reach);
    // Only the road pieces near the wall are measured against.
    let near: Vec<(P, P, f64)> = roads
        .iter()
        .flat_map(|(line, width, _)| line.windows(2).map(move |pair| (pair[0], pair[1], width / 2.0 + WALL_GAP)))
        .filter(|(a, b, _)| a[0].min(b[0]) <= wall[2] && a[0].max(b[0]) >= wall[0] && a[1].min(b[1]) <= wall[3] && a[1].max(b[1]) >= wall[1])
        .collect();
    if near.is_empty() {
        return vec![points.to_vec()];
    }
    let clear = |p: P| near.iter().all(|&(a, b, reach)| distance_to_segment(p, a, b) > reach);
    let mut runs: Vec<Vec<P>> = Vec::new();
    let mut run: Vec<P> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / WALL_PROBE).ceil().max(1.0) as usize;
        // The last clear probe before a road and the first after it are the ends of the opening.
        let mut last = None;
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            if clear(p) {
                if (run.is_empty() || i == steps) && run.last() != Some(&p) {
                    run.push(p);
                }
                last = Some(p);
            } else {
                if let Some(last) = last.take().filter(|last| run.last() != Some(last)) {
                    run.push(last);
                }
                if run.len() > 1 {
                    runs.push(std::mem::take(&mut run));
                }
                run.clear();
            }
        }
    }
    if run.len() > 1 {
        runs.push(run);
    }
    runs
}

/// The steepest a guard rail's foot rises or falls from one post to the next, rise over run: steeper
/// than any road, so a steeper drop is the rail running out over a cutting.
const GUARDRAIL_STEEPEST: f64 = 0.25;
/// The shortest stretch of guard rail kept between two such drops, metres: a shorter one is the
/// bottom of the cutting.
const GUARDRAIL_SHORTEST: f64 = 10.0;

/// A guard rail's line cut where its foot (`foot(x, z)`, metres, sampled every `step` metres) drops
/// or climbs more steeply than `GUARDRAIL_STEEPEST`, with the pieces shorter than `GUARDRAIL_SHORTEST`
/// left out: where it would run down into a cutting with no deck over it, it stops at the edge.
fn over_level(points: &[P], step: f64, foot: &impl Fn(f64, f64) -> f64) -> Vec<Vec<P>> {
    let samples = bridge::resample(points, step);
    if samples.is_empty() {
        return Vec::new();
    }
    let along: Vec<f64> = samples.iter().scan((0.0, samples[0]), |(run, last), &p| {
        *run += (p[0] - last[0]).hypot(p[1] - last[1]);
        *last = p;
        Some(*run)
    }).collect();
    let heights: Vec<f64> = samples.iter().map(|p| foot(p[0], p[1])).collect();
    let mut runs = Vec::new();
    let mut from = 0;
    for k in 1..=samples.len() {
        let steep = k < samples.len() && (heights[k] - heights[k - 1]).abs() > GUARDRAIL_STEEPEST * (along[k] - along[k - 1]).max(1.0);
        if k == samples.len() || steep {
            if along[k - 1] - along[from] >= GUARDRAIL_SHORTEST {
                runs.push(samples[from..k].to_vec());
            }
            from = k;
        }
    }
    runs
}

/// `points` (centimetres) with points added so that none is more than `step` metres from the next:
/// one at every post of a fence, and a wall cut into pieces that break on their own.
fn every(points: &[[i32; 2]], step: f32) -> Vec<[i32; 2]> {
    let mut out: Vec<[i32; 2]> = points.first().into_iter().copied().collect();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let length = f64::from(b[0] - a[0]).hypot(f64::from(b[1] - a[1])) / 100.0;
        let pieces = (length / f64::from(step)).ceil().max(1.0) as i32;
        for i in 1..=pieces {
            let t = f64::from(i) / f64::from(pieces);
            let p = [a[0] + (f64::from(b[0] - a[0]) * t).round() as i32, a[1] + (f64::from(b[1] - a[1]) * t).round() as i32];
            if out.last() != Some(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Puts fences, walls and power lines (`(LINE_*, line)`) in the tiles they cross, which must exist
/// already. A fence or wall is cut at tile borders like a road, and has a point at every post
/// (`LineKindDef::step`), each piece between two being what breaks. A power line is its poles, each span in the tile of
/// its first pole, so every pole is in one tile only and a span never needs two.
///
/// A line stands on the ground, or on the roadway where it crosses a deck over a cutting (`decks`:
/// each deck's line and width, `bridge::flush_top`). A guard rail is cut where its foot would drop
/// more steeply than any road climbs (`GUARDRAIL_STEEPEST`): it would be down in a cutting with no
/// deck over it.
fn lay_lines(built: &mut Built, lines: &[(u8, Vec<P>)], decks: &[(Vec<P>, f64)], half: f64, tile_size: f64, ground: &Ground) {
    let inner = half - EDGE_MARGIN;
    let Built { manifest, tiles } = built;
    let foot = |x: f64, z: f64| {
        let on_deck = decks.iter().filter_map(|(points, width)| bridge::flush_top(points, *width, ground, [x, z])).fold(f64::NAN, f64::max);
        if on_deck.is_finite() { on_deck } else { f64::from(ground.height_at(x as f32, z as f32)) }
    };
    let mut put = |key: (u16, u16), kind: u8, points: Vec<[i32; 2]>, end: bool| {
        if let Some(tile) = tiles.get_mut(&key) {
            let base = points.iter().map(|&[x, z]| cm(foot(f64::from(x) / 100.0, f64::from(z) / 100.0))).collect();
            tile.lines.push(Line { kind, points, base, end });
        }
    };
    for (kind, points) in lines {
        let span = match *kind {
            LINE_POWER => POLE_SPAN,
            LINE_POWER_MAJOR => PYLON_SPAN,
            _ => {
                let step = line_kind_def(*kind).map_or(f32::MAX, |def| def.step);
                let runs = clip_polyline(points, inner);
                let runs = if *kind == LINE_GUARDRAIL { runs.iter().flat_map(|run| over_level(run, f64::from(step), &foot)).collect() } else { runs };
                for run in runs {
                    per_tile(manifest, half, tile_size, &run, |key, points| {
                        let points = every(&points, step);
                        if points.len() > 1 {
                            put(key, *kind, points, true);
                        }
                    });
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
    rings: &[Ring],
) -> Vec<Option<Top>> {
    let tally = Tally::default();
    crate::progress::stage("roofs");
    let painted = |ring: &[P]| {
        let Some(mask) = mask else { return true };
        let (lo, hi) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let span = |a: f64, b: f64| ((a + half) / tile_size).floor() as i64..=((b + half) / tile_size).floor() as i64;
        span(lo[0], hi[0]).any(|c| span(lo[1], hi[1]).any(|r| c >= 0 && r >= 0 && mask.contains(&(c as u16, r as u16))))
    };
    let fit = |(points, ceiling, hint, kind): Ring| {
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
        // The eaves a house this size has, as `cap` reckons them; a house tagged with its height, or
        // tall, or big (no `ceiling`) keeps what the laser shows.
        let storeys = if signed_area(ring).abs() < SMALL_HOUSE { 1.0 } else { 2.0 };
        let eaves = ceiling.map(|_| floor_of(ground, ring) + storeys * STOREY + PLINTH);
        // A church is fitted as one: its nave under one steep roof, its annexes lower beside it.
        if kind == BUILDING_CHURCH {
            let found = crate::church::fit(ring, &returns);
            // There are few of them, and each is a landmark: say how each came out.
            match &found {
                Some(roof) => eprintln!("church at {:.0?} ({} returns): {roof:.2?}", ring[0], returns.len()),
                None => eprintln!("church at {:.0?} ({} returns): left to the general fit", ring[0], returns.len()),
            }
            if let Some(roof) = found {
                tally.churches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Some(Top::Fitted(roof));
            }
        }
        if let Some(top) = from_catalogue(ring, &returns, measured, hint, eaves, &tally) {
            return Some(top);
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
    let count = |n: &std::sync::atomic::AtomicUsize| n.load(std::sync::atomic::Ordering::Relaxed);
    eprintln!("roofs from the catalogue: {:?}, {} wings past the first, {} dormers, {} guessed under trees, {} left to the measured roof, {} roofs over {} m²; {} churches fitted as churches", tally.shapes.lock().unwrap(), count(&tally.wings), count(&tally.dormers), count(&tally.guessed), count(&tally.measured), count(&tally.big), roof::SPLIT_FROM, count(&tally.churches));
    slots.into_iter().map(|slot| slot.into_inner().unwrap().expect("every building was taken")).collect()
}

/// How the houses' roofs came out of the catalogue, for the importer to print.
#[derive(Default)]
struct Tally {
    shapes: std::sync::Mutex<BTreeMap<Category, usize>>,
    wings: std::sync::atomic::AtomicUsize,
    dormers: std::sync::atomic::AtomicUsize,
    measured: std::sync::atomic::AtomicUsize,
    guessed: std::sync::atomic::AtomicUsize,
    churches: std::sync::atomic::AtomicUsize,
    /// Buildings past `roof::SPLIT_FROM` the catalogue fitted closely enough to keep.
    big: std::sync::atomic::AtomicUsize,
}

/// A building this big or bigger (m²) is never roofed from the catalogue: a block, a hall, a school.
const CATALOGUE_MOST: f64 = 800.0;
/// How far the returns of a building past `roof::SPLIT_FROM` may miss the catalogue's roof, metres,
/// for it to be kept: a big house of a plain plan (a long T or L farmhouse) gets one clean roof, and
/// anything less plain is fitted freely in parts (`roof::fit`).
const BIG_MISS: f64 = 0.3;

/// How far a French house's returns may miss the catalogue's roof, metres, before its measured
/// roof (`field::Field`) is kept instead: a block of row houses or an odd roof that is no one house.
const FRENCH_MISS: f64 = 0.35;

/// The roof from the catalogue for a house-sized `ring` (under `roof::SPLIT_FROM`), or `None` for a
/// bigger building, too few returns, or (`measured`) a French roof the catalogue misses.
///
/// A house the laser saw only trees over is given the usual roof at the eaves `eaves` (local y,
/// metres) a house its size has (`cap`).
fn from_catalogue(ring: &[P], returns: &[[f64; 4]], measured: bool, hint: Option<Category>, eaves: Option<f64>, tally: &Tally) -> Option<Top> {
    let area = signed_area(ring).abs();
    if area >= CATALOGUE_MOST {
        return None;
    }
    // A building past `roof::SPLIT_FROM` keeps the catalogue's roof only where it fits it closely, and
    // is never given the usual roof of a house the laser did not see.
    let big = area >= roof::SPLIT_FROM;
    let eaves = if measured || big { None } else { eaves };
    dump_house(ring, returns, hint, eaves);
    let found = crate::catalogue::fit(ring, returns, hint, eaves)?;
    if big && found.miss > BIG_MISS {
        return None;
    }
    if big {
        tally.big.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    if found.guessed {
        tally.guessed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    if measured && found.miss > FRENCH_MISS {
        tally.measured.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return None;
    }
    *tally.shapes.lock().unwrap().entry(found.category).or_default() += 1;
    tally.wings.fetch_add(found.wings - 1, std::sync::atomic::Ordering::Relaxed);
    tally.dormers.fetch_add(found.dormers, std::sync::atomic::Ordering::Relaxed);
    Some(Top::Fitted(found.roof))
}

/// With `ROOF_DUMP` set to a file, each house's footprint (local metres), laser returns
/// (`[x, z, y, code]`), OSM roof shape and eaves are added to it as one JSON line: what a roof that
/// came out wrong can be fitted again from, in a test or a script, without importing the world.
fn dump_house(ring: &[P], returns: &[[f64; 4]], hint: Option<Category>, eaves: Option<f64>) {
    use std::io::Write;
    static FILE: std::sync::OnceLock<Option<std::sync::Mutex<std::fs::File>>> = std::sync::OnceLock::new();
    let file = FILE.get_or_init(|| {
        let path = std::env::var_os("ROOF_DUMP")?;
        Some(std::sync::Mutex::new(std::fs::File::create(path).expect("ROOF_DUMP is a file that can be written")))
    });
    let Some(file) = file else { return };
    let line = serde_json::json!({ "ring": ring, "returns": returns, "hint": hint.map(|c| format!("{c:?}")), "eaves": eaves });
    writeln!(file.lock().unwrap(), "{line}").expect("ROOF_DUMP is written");
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

/// Whether a road way may be a carriageway of a divided road: one way along its points, a through road
/// (not a slip road or a roundabout), and not in a tunnel.
fn is_carriageway(tags: &std::collections::HashMap<String, String>) -> bool {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    crate::lanes::oneway(tags) == crate::lanes::Oneway::Along
        && matches!(tag("highway"), Some("motorway" | "trunk" | "primary" | "secondary"))
        && tag("junction").is_none()
        && tag("tunnel").is_none_or(|value| value == "no")
}

/// The slowest a divided road with a rail down its middle is driven, km/h: slower is a town's street.
const RAILED_SPEED: u32 = 70;

/// Whether a road is driven fast enough for a rail down its middle: a motorway, a trunk road or a
/// motorroad, or signed at `RAILED_SPEED` and over. A primary road with no `maxspeed` is taken as a
/// country road, as most are in Sweden; a secondary one as a town's.
fn is_fast(tags: &std::collections::HashMap<String, String>) -> bool {
    let tag = |key: &str| tags.get(key).map(String::as_str);
    let signed = tag("maxspeed").and_then(|speed| speed.trim().parse::<u32>().ok());
    matches!(tag("highway"), Some("motorway" | "trunk")) || tag("motorroad") == Some("yes") || signed.map_or(tag("highway") == Some("primary"), |speed| speed >= RAILED_SPEED)
}

/// Whether a road way is carried on a bridge (and not, say, a tunnel with a stray bridge tag).
pub(crate) fn is_bridge(way: &Way) -> bool {
    let tag = |key: &str| way.tags.get(key).map(String::as_str);
    tag("bridge").is_some_and(|value| value != "no") && tag("tunnel").is_none_or(|value| value == "no")
}

/// Ground cover for a way's or relation's tags, or `None` if it is not cover we draw. Water is not
/// cover: it comes from `osm_water` (`water.rs`).
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
    // A churchyard is mown grass among its stones, as a cemetery is.
    if tag("amenity") == Some("grave_yard") {
        return Some(COVER_PARK);
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

/// Whether a way's or relation's tags make it a field under crops (`lay_crops`): farmland, and not the
/// farmyards, orchards, vineyards and nurseries that are drawn as it is.
fn is_field(tags: &std::collections::HashMap<String, String>) -> bool {
    cover_kind(tags) == Some(COVER_FARM) && tags.get("landuse").is_some_and(|value| value == "farmland")
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
/// Which of the synthetic city's parks, in the order they are drawn, is a field of wheat instead: it
/// keeps its fence and has no trees.
const SYNTHETIC_FIELD: usize = 2;

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
    // Each park's corner and side, metres.
    let mut parks = Vec::new();
    // The field's rings: wheat is grown in it once the tiles are built (`lay_crops`).
    let mut fields: Vec<Vec<Vec<P>>> = Vec::new();
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
                // A little park, or the one field of wheat.
                let ring = vec![[bx, bz], [bx + size, bz], [bx + size, bz + size], [bx, bz + size]];
                if parks.len() == SYNTHETIC_FIELD {
                    fields.push(vec![ring.clone()]);
                    covers.push((COVER_FARM, vec![ring]));
                } else {
                    covers.push((COVER_PARK, vec![ring]));
                }
                parks.push(([bx, bz], size));
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
                buildings.push((vec![[x, z], [x + half_size, z], [x + half_size, z + half_size], [x, z + half_size]], height, roof, None, None, BUILDING_PLAIN));
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
    // Two sides of each park are fenced or walled, so the offline world has both to draw and break.
    let lines: Vec<(u8, Vec<P>)> = parks
        .iter()
        .enumerate()
        .map(|(k, &([x, z], size))| ([LINE_FENCE, LINE_WALL, LINE_FENCE_HIGH, LINE_WALL_HIGH][k % 4], vec![[x + size - 1.0, z + 1.0], [x + 1.0, z + 1.0], [x + 1.0, z + size - 1.0]]))
        .collect();
    let kept = (!lines.is_empty()).then(|| ground.clone());
    // The first two parks are graveyards, so the offline world has tombstones to draw and hide behind:
    // one with a path across it for its stones to follow, one without, filled with rows.
    let yards: Vec<Vec<Vec<P>>> = parks.iter().take(2).map(|&([x, z], size)| vec![vec![[x, z], [x + size, z], [x + size, z + size], [x, z + size]]]).collect();
    if let Some(&([x, z], size)) = parks.first() {
        roads.push((vec![[x - 2.0, z + size / 2.0], [x + size + 2.0, z + size / 2.0]], 2.5, ROAD_PATH));
    }
    let ways: Vec<(Vec<P>, f64)> = roads.iter().map(|(points, width, _)| (points.clone(), *width)).collect();
    // A loose ring of trees in each park, round an open middle.
    for (k, ([x, z], size)) in parks.into_iter().enumerate().filter(|(k, _)| *k != SYNTHETIC_FIELD) {
        for (n, (u, v)) in [(0.2, 0.2), (0.5, 0.15), (0.8, 0.25), (0.85, 0.55), (0.75, 0.85), (0.45, 0.8), (0.15, 0.7), (0.2, 0.45)].into_iter().enumerate() {
            let kind = ((k + n) % 7) as u8;
            let (height, crown) = crate::trees::size(kind, (k * 10 + n) as u64, None);
            trees.push(([x + u * size, z + v * size], kind, height, crown, true));
        }
    }
    // A bridge over the pond (`synthetic_water`), so the offline world has one to draw and cross.
    let pond_x = start + (count - 1) as f64 * PITCH + STREET / 2.0 + 2.0 + (PITCH - STREET - 4.0) / 2.0;
    let pond_z = -half + 70.0;
    let bridges = vec![(vec![[pond_x, pond_z - 50.0], [pond_x, pond_z + 50.0]], 7.0, ROAD_MINOR, bridge::Pylons::Middle, Deck::new())];
    // A tram down the middle street, so the offline world has a track to draw.
    let middle = start + (count / 2) as f64 * PITCH;
    let rails = vec![(vec![[middle, -half], [middle, half]], 1.435, RAIL_TRAM)];
    let paints = vec![Lanes::default(); roads.len()];
    let mut built = assemble(Frame::default(), half, tile_size, None, buildings, roads, &paints, bridges, rails, Vec::new(), Vec::new(), covers, trees, &[], ground, water);
    if let Some(ground) = kept {
        lay_lines(&mut built, &lines, &[], half, tile_size, &ground);
        crate::graves::lay(&mut built, &yards, &ways, half, &ground);
    }
    lay_crops(&mut built, &fields, half, tile_size);
    built
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
    fn a_stream_stops_at_a_road_and_carries_on_beyond_it() {
        let road: &[P] = &[[0.0, -50.0], [0.0, 50.0]];
        let runs = culverted(&[[-20.0, 3.0], [20.0, 3.0]], &[(road, 6.0)]);
        assert_eq!(runs.len(), 2, "{runs:?}");
        let reach = 3.0 + CULVERT_SHOULDER;
        let (upstream, downstream) = (runs[0].last().unwrap(), runs[1][0]);
        assert!((upstream[0] + reach).abs() < 0.01 && (downstream[0] - reach).abs() < 0.01, "{runs:?}");
        assert_eq!((runs[0][0], *runs[1].last().unwrap()), ([-20.0, 3.0], [20.0, 3.0]), "the flow keeps its ends and direction");
    }

    #[test]
    fn a_stream_under_a_track_stops_at_its_ballast() {
        let track: &[P] = &[[0.0, -50.0], [0.0, 50.0]];
        let runs = culverted(&[[-20.0, 0.0], [20.0, 0.0]], &[(track, ballast(1.435))]);
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().flatten().all(|p| p[0].abs() >= ballast(1.435) / 2.0 + CULVERT_SHOULDER - 0.01), "{runs:?}");
    }

    #[test]
    fn a_stream_beside_a_road_is_not_cut() {
        let road: &[P] = &[[0.0, -50.0], [0.0, 50.0]];
        let line = vec![[10.0, -40.0], [10.0, 0.0], [12.0, 40.0]];
        assert_eq!(culverted(&line, &[(road, 6.0)]), vec![line]);
    }

    #[test]
    fn a_stream_under_a_bridge_is_left_whole() {
        // A bridge is not among the ways a stream goes under, so with only a bridge there it runs on.
        let line = vec![[-20.0, 0.0], [20.0, 0.0]];
        assert_eq!(culverted(&line, &[]), vec![line]);
    }

    #[test]
    fn a_stream_wholly_under_a_road_is_gone() {
        let road: &[P] = &[[0.0, -50.0], [0.0, 50.0]];
        assert!(culverted(&[[-1.0, 0.0], [1.0, 0.0]], &[(road, 6.0)]).is_empty());
    }

    #[test]
    fn cover_comes_from_tags() {
        let tags = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<std::collections::HashMap<_, _>>();
        assert_eq!(cover_kind(&tags(&[("natural", "water")])), None, "water comes from osm_water");
        assert_eq!(cover_kind(&tags(&[("landuse", "forest")])), Some(COVER_WOOD));
        assert_eq!(cover_kind(&tags(&[("leisure", "park")])), Some(COVER_PARK));
        assert_eq!(cover_kind(&tags(&[("landuse", "military")])), None);
        assert_eq!(cover_kind(&tags(&[("landuse", "cemetery")])), Some(COVER_PARK));
        assert_eq!(cover_kind(&tags(&[("amenity", "grave_yard")])), Some(COVER_PARK));
        assert_eq!(cover_kind(&tags(&[("natural", "wetland")])), Some(COVER_MARSH));
        assert_eq!(cover_kind(&tags(&[("natural", "wetland"), ("wetland", "bog")])), Some(COVER_MARSH_DEEP));
        assert_eq!(cover_kind(&tags(&[("natural", "wetland"), ("wetland", "wet_meadow")])), Some(COVER_MARSH_FAINT));
    }

    #[test]
    fn farmland_grows_wheat_but_not_on_its_roads_buildings_or_under_woods() {
        let manifest = Manifest { version: WORLD_VERSION, half_extent: cm(100.0), tile_size: cm(100.0), ..Default::default() };
        let square = |x0: f64, z0: f64, x1: f64, z1: f64| vec![[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
        let cover = |kind: u8, ring: &[P]| Cover { kind, rings: vec![ring.iter().map(|&[x, z]| [cm(x), cm(z)]).collect()] };
        // Tile (0, 0) spans -100..0 both ways. A field over its west half with a wood in one corner, a
        // road across it and a barn in it; a farmyard, drawn as farmland is, in the east half.
        let field = square(-100.0, -100.0, -50.0, 0.0);
        let yard = square(-40.0, -100.0, 0.0, -60.0);
        let barn = square(-90.0, -30.0, -80.0, -20.0);
        let tile = Tile {
            cover: vec![cover(COVER_FARM, &field), cover(COVER_FARM, &yard), cover(COVER_WOOD, &square(-100.0, -100.0, -80.0, -80.0))],
            roads: vec![Road { width: cm(6.0), kind: ROAD_MINOR, carried: false, points: vec![[cm(-100.0), cm(-50.0)], [cm(0.0), cm(-50.0)]], ..Default::default() }],
            buildings: vec![Building { footprint: barn.iter().map(|&[x, z]| [cm(x), cm(z)]).collect(), ..Default::default() }],
            ..Default::default()
        };
        let mut built = Built { manifest, tiles: [((0, 0), tile), ((1, 0), Tile::default())].into_iter().collect() };
        lay_crops(&mut built, &[vec![field]], 100.0, 100.0);
        let crops = &built.tiles[&(0, 0)].crops;
        assert_eq!(crops.len(), 50 * 50);
        let at = |x: f64, z: f64| crops[((z + 100.0) / 2.0) as usize * 50 + ((x + 100.0) / 2.0) as usize];
        assert_eq!(at(-70.0, -70.0), CROP_WHEAT, "the open field");
        assert_eq!(at(-90.0, -90.0), CROP_NONE, "under the wood painted over it");
        assert_eq!(at(-70.0, -49.0), CROP_NONE, "on the road");
        assert_eq!(at(-70.0, -45.0), CROP_WHEAT, "beside the road");
        assert_eq!(at(-85.0, -25.0), CROP_NONE, "under the barn");
        assert_eq!(at(-45.0, -70.0), CROP_NONE, "past the field's edge");
        assert_eq!(at(-20.0, -80.0), CROP_NONE, "a farmyard is no field");
        assert!(built.tiles[&(1, 0)].crops.is_empty(), "a tile without farmland has no grid");
    }

    #[test]
    fn only_farmland_is_a_field() {
        let tags = |pairs: &[(&str, &str)]| pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<std::collections::HashMap<_, _>>();
        assert!(is_field(&tags(&[("landuse", "farmland")])));
        for other in ["farmyard", "orchard", "vineyard", "plant_nursery", "meadow"] {
            assert!(!is_field(&tags(&[("landuse", other)])), "{other}");
        }
        // Farmland that something else makes a wood is the wood.
        assert!(!is_field(&tags(&[("landuse", "farmland"), ("natural", "wood")])));
    }

    #[test]
    fn the_synthetic_city_has_a_field_of_wheat() {
        let built = synthetic_city(300.0, 100.0, Ground::default(), Placed::default());
        let wheat: usize = built.tiles.values().map(|tile| tile.crops.iter().filter(|&&cell| cell == CROP_WHEAT).count()).sum();
        // One block of 44 m a side: 22 or 23 cells of 2 m each way, as their centres fall in it.
        assert!((22 * 22..=23 * 23).contains(&wheat), "{wheat}");
    }

    #[test]
    fn synthetic_parks_land_in_their_tiles_water_last() {
        let built = synthetic_city(150.0, 100.0, Ground::default(), Placed::default());
        assert!(built.tiles.values().any(|tile| !tile.cover.is_empty()));
    }

    #[test]
    fn two_carriageways_side_by_side_have_no_railing_between_them() {
        // Ground 6 m down for 80 m across the middle, and two 7 m roads crossing it 7.6 m apart: their
        // roads do not overlap, but their decks, an edge beam wider, do.
        let side = 61u32;
        let heights = (0..side * side).map(|k| if (-15_000 + (k % side) as i32 * 500).abs() < 4_000 { -600 } else { 0 }).collect();
        let ground = Ground { origin: -15_000, cell: 500, side, heights };
        let way = |z: f64| (vec![[-70.0, z], [70.0, z]], 7.0, ROAD_MINOR, Pylons::None, Deck::new());
        let built = assemble(Frame::default(), 150.0, 100.0, None, Vec::new(), Vec::new(), &[], vec![way(-3.8), way(3.8)], Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), &[], ground, Placed::default());
        let vertices: Vec<[f64; 3]> = built.tiles.values().flat_map(|tile| tile.collision.vertices.iter().map(|v| v.map(|c| f64::from(c) / 100.0))).filter(|v| v[0].abs() < 30.0).collect();
        // The deck is flat at its lip over the ground at the ends; between the two roads only its top.
        let middle = vertices.iter().filter(|v| v[2].abs() < 1.5).map(|v| v[1]).fold(f64::MIN, f64::max);
        let outer = vertices.iter().filter(|v| v[2].abs() > 7.0).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(middle < 0.2, "nothing stands on the roadway between the two, the highest thing there is at {middle}");
        assert!(outer > middle + bridge::RAILING_HEIGHT - 0.05, "a railing along the outer edges, its top {outer} over a deck at {middle}");
    }

    #[test]
    fn two_carriageways_side_by_side_keep_their_piers() {
        // A valley 15 m deep and 80 m across, crossed by two 7 m roads on decks of their own 8 m apart.
        let side = 61u32;
        let heights = (0..side * side).map(|k| if (-15_000 + (k % side) as i32 * 500).abs() < 4_000 { -1_500 } else { 0 }).collect();
        let ground = Ground { origin: -15_000, cell: 500, side, heights };
        let way = |z: f64| (vec![[-70.0, z], [70.0, z]], 7.0, ROAD_MINOR, Pylons::None, Deck::new());
        let built = assemble(Frame::default(), 150.0, 100.0, None, Vec::new(), Vec::new(), &[], vec![way(-4.0), way(4.0)], Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), &[], ground, Placed::default());
        // A pier's foot is half a metre into the valley's floor.
        let feet = |z0: f64, z1: f64| built.tiles.values().flat_map(|tile| tile.collision.vertices.iter()).filter(|v| v[1] < -1_400 && (z0..z1).contains(&(f64::from(v[2]) / 100.0))).count();
        assert!(feet(-8.0, 0.0) > 0, "the carriageway at -4 stands on piers");
        assert!(feet(0.0, 8.0) > 0, "the carriageway at 4 stands on piers");
    }

    #[test]
    fn a_footway_beside_a_road_bridge_is_carried_on_its_deck() {
        let road = (vec![[0.0, 0.0], [100.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, Deck::new());
        let path = (vec![[0.0, 8.0], [100.0, 8.0]], 2.5, ROAD_PATH, Pylons::None, Deck::new());
        let far = (vec![[0.0, 40.0], [100.0, 40.0]], 2.5, ROAD_PATH, Pylons::None, Deck::new());
        let (bridges, shared) = paths_alongside(vec![road, path, far], Vec::new());
        // One deck from the road's far edge (-5) to the path's (9.25), its own deck and the path's gone.
        assert_eq!(shared.len(), 1);
        let (line, width, kind, ..) = &shared[0];
        assert!((width - 14.25).abs() < 1e-6, "{width}");
        assert!((line[0][1] - 2.125).abs() < 1e-6, "{:?}", line[0]);
        assert_eq!(*kind, ROAD_MAJOR);
        assert_eq!(bridges.iter().map(|b| b.3).collect::<Vec<_>>(), [Pylons::Carried, Pylons::Carried, Pylons::None], "a path far off keeps its own bridge");
    }

    #[test]
    fn bridge_sections_are_buildings_with_cells_and_a_look_and_no_windows() {
        use crate::water::{Grid, Surface};
        let map = synthetic_water(300.0);
        let grid = Grid { origin: -30_000, cell: 500, side: 121 };
        let surface = Surface::new(&map, grid, |_, _| Some(3.0));
        let ground = Ground { origin: -30_000, cell: 500, side: 121, heights: vec![300i32; 121 * 121] };
        let placed = Placed::new(&map, &surface, 0.0, &ground);
        let mut built = synthetic_city(300.0, 100.0, ground, placed);
        for tile in built.tiles.values_mut() {
            crate::voxels::bake(tile);
        }
        let sections: Vec<&Building> = built.tiles.values().flat_map(|t| &t.buildings).filter(|b| b.kind == BUILDING_BRIDGE).collect();
        assert!(sections.len() >= 2, "the bridge over the pond is in sections, got {}", sections.len());
        assert!(sections.iter().all(|b| b.cells.dims.iter().all(|&d| d > 0) && b.paint == Some([BRIDGE_CONCRETE, DECK_ASPHALT]) && b.footprint.len() >= 3));
        // Each section is closed: every edge of its look is shared by two of its faces at least, so no
        // open end is left to be drawn as the rim of a hole.
        for tile in built.tiles.values() {
            for (building, range) in tile.buildings.iter().zip(&tile.look.buildings).filter(|(b, _)| b.kind == BUILDING_BRIDGE) {
                let mut edges: HashMap<(u32, u32), u32> = HashMap::new();
                for t in &tile.look.triangles[range[0] as usize..(range[0] + range[1] + range[2]) as usize] {
                    for k in 0..3 {
                        let (a, b) = (t[k], t[(k + 1) % 3]);
                        *edges.entry((a.min(b), a.max(b))).or_default() += 1;
                    }
                }
                let open = edges.values().filter(|&&n| n == 1).count();
                assert_eq!(open, 0, "a section at {:?} has {open} open edges", building.centre());
            }
        }
        let manifest = built.manifest.clone();
        let world = WorldFile::assemble(&manifest, built.tiles.into_iter().map(|((col, row), tile)| (col, row, tile)));
        let looks: Vec<&BuildingLook> = world.buildings.iter().zip(&world.looks).filter(|(b, _)| b.kind == BUILDING_BRIDGE).map(|(_, look)| look).collect();
        assert!(looks.iter().all(|look| !look.walls.is_empty() && !look.roof.is_empty() && look.windows.is_empty() && look.inside.is_empty()));
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
        assert!(built.tiles.values().flat_map(|t| &t.buildings).filter(|b| b.kind != BUILDING_BRIDGE).all(|b| b.footprint.iter().all(|p| p[0] < 25_000)), "the last column of blocks is left to the water");
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
        let built = assemble(Frame::default(), 150.0, 100.0, None, vec![], vec![], &[], vec![], rails, vec![], vec![], vec![], trees, &[], Ground::default(), Placed::default());
        let pieces: Vec<(&(u16, u16), &Rail)> = built.tiles.iter().flat_map(|(key, tile)| tile.rails.iter().map(move |rail| (key, rail))).collect();
        assert_eq!(pieces.len(), 3, "a 280 m track crosses three 100 m tiles");
        assert!(pieces.iter().all(|(_, rail)| rail.gauge == 144 && rail.kind == RAIL_TRAIN));
        let trees: Vec<&Tree> = built.tiles.values().flat_map(|tile| &tile.trees).collect();
        assert_eq!(trees.len(), 1, "the tree on the ballast is dropped");
        assert!(trees[0].solid, "the one beside the line is near it");
        assert!(built.manifest.spawns.iter().all(|&[_, z]| (z - 1000).abs() > 100), "no spawn on the track");
    }

    #[test]
    fn trees_keep_their_trunks_and_crowns_off_a_house_and_shrubs_need_not() {
        // A 10 m house with its west wall at x = 20; trees west of it, along one line.
        let house = (vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0], [20.0, 30.0]], 6.0, None, None, None, BUILDING_PLAIN);
        let trees = vec![
            ([19.0, 22.0], TREE_BIRCH, 8.0, 0.8, true),
            ([17.5, 25.0], TREE_BIRCH, 8.0, 3.0, false),
            ([16.0, 28.0], TREE_BIRCH, 8.0, 3.0, false),
            ([19.0, 26.0], TREE_BUSH, 1.5, 1.0, false),
        ];
        let built = assemble(Frame::default(), 150.0, 100.0, None, vec![house], vec![], &[], vec![], vec![], vec![], vec![], vec![], trees, &[], Ground::default(), Placed::default());
        let mut standing: Vec<(i32, u8)> = built.tiles.values().flat_map(|tile| &tile.trees).map(|tree| (tree.z, tree.kind)).collect();
        standing.sort();
        // The planted tree's trunk is a metre from the wall, the next one's crown reaches it.
        assert_eq!(standing, [(2600, TREE_BUSH), (2800, TREE_BIRCH)]);
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
        lay_lines(&mut built, &lines, &[], 150.0, 100.0, &Ground::default());
        let power: Vec<&Line> = built.tiles.values().flat_map(|tile| &tile.lines).filter(|line| line.kind == LINE_POWER).collect();
        // Six spans of 46.7 m: poles at -140, -93, -47, 0, 47, 93, 140.
        assert_eq!(power.iter().map(|line| line.points.len() - usize::from(!line.end)).sum::<usize>(), 7, "{power:?}");
        assert_eq!(power.iter().filter(|line| line.end).count(), 1, "only the last run ends on a pole of its own");
        let fences: Vec<&Line> = built.tiles.values().flat_map(|tile| &tile.lines).filter(|line| line.kind == LINE_FENCE).collect();
        assert_eq!(fences.len(), 3);
        // A post every 3 m at most, and the ground under each.
        for fence in fences {
            assert!(fence.points.windows(2).all(|pair| (pair[1][0] - pair[0][0]).abs() <= 300 && pair[1] != pair[0]), "{fence:?}");
            assert_eq!(fence.base.len(), fence.points.len());
        }
    }

    #[test]
    fn trees_are_cleared_from_under_power_lines_and_wider_under_pylons() {
        let lines = vec![(LINE_POWER, vec![[0.0, 0.0], [100.0, 0.0]]), (LINE_POWER_MAJOR, vec![[0.0, 100.0], [100.0, 100.0]]), (LINE_FENCE, vec![[0.0, 50.0], [100.0, 50.0]])];
        let corridors = power_corridors(&lines);
        assert!(under_power([50.0, 4.0], &corridors), "under the wires on poles");
        assert!(!under_power([50.0, 8.0], &corridors), "past the poles' corridor");
        assert!(under_power([50.0, 115.0], &corridors), "within the pylons' wider corridor");
        assert!(!under_power([50.0, 125.0], &corridors), "past the pylons' corridor");
        assert!(!under_power([50.0, 50.0], &corridors), "a fence clears nothing");
        assert!(!under_power([150.0, 0.0], &corridors), "beyond the line's end");
    }

    #[test]
    fn a_wall_opens_where_a_road_crosses_it() {
        let roads = vec![(vec![[0.0, -50.0], [0.0, 50.0]], 6.0, ROAD_MINOR), (vec![[500.0, 0.0], [600.0, 0.0]], 6.0, ROAD_MINOR)];
        let runs = open_at_roads(&[[-20.0, 0.0], [20.0, 0.0], [20.0, 30.0]], &roads);
        assert_eq!(runs.len(), 2, "{runs:?}");
        // Half the road and the gap, to the nearest probe.
        assert_eq!(runs[0][0], [-20.0, 0.0]);
        assert!((runs[0][1][0] + 3.5).abs() <= WALL_PROBE + 1e-9 && runs[0][1][0] < -3.5, "{runs:?}");
        assert!((runs[1][0][0] - 3.5).abs() <= WALL_PROBE + 1e-9 && runs[1][0][0] > 3.5, "{runs:?}");
        assert_eq!(runs[1][1..], [[20.0, 0.0], [20.0, 30.0]], "the corner past the road is kept");
        // No road near: the wall as it was.
        assert_eq!(open_at_roads(&[[-20.0, 80.0], [20.0, 80.0]], &roads), vec![vec![[-20.0, 80.0], [20.0, 80.0]]]);
    }

    #[test]
    fn osm_fences_walls_and_power_lines_are_laid_without_a_map() {
        let place = crate::region::Region::Sweden.around(63.82, 20.26, 150.0);
        let way = |id, (key, value): (&str, &str), x0: f64, x1: f64, z: f64| {
            let geometry = [[x0, z], [x1, z]].into_iter().map(|[x, z]| {
                let (lat, lon) = place.frame.unproject(x, z);
                LatLon { lat, lon }
            });
            crate::overpass::Way { id, tags: [(key.to_string(), value.to_string())].into(), geometry: geometry.collect() }
        };
        let osm = Osm {
            ways: vec![way(1, ("barrier", "fence"), -100.0, 100.0, 20.0), way(2, ("power", "minor_line"), -100.0, 100.0, -20.0), way(3, ("barrier", "wall"), -100.0, 100.0, 0.0)],
            areas: Vec::new(),
            pois: Vec::new(),
        };
        let ground = Ground { origin: -15_000, cell: 500, side: 61, heights: vec![0; 61 * 61] };
        let built = from_osm(&place, 100.0, None, &osm, &[], ground, Placed::default(), None, None, None, &[], None, None, &[]);
        let kinds: std::collections::BTreeSet<u8> = built.tiles.values().flat_map(|tile| &tile.lines).map(|line| line.kind).collect();
        assert_eq!(kinds, [LINE_FENCE, LINE_POWER, LINE_WALL].into());
    }

    #[test]
    fn farmland_from_osm_is_sown_whether_drawn_as_a_way_or_a_relation_and_a_farmyard_is_not() {
        let place = crate::region::Region::Sweden.around(63.82, 20.26, 150.0);
        let latlon = |points: &[P]| -> Vec<LatLon> {
            points.iter().map(|&[x, z]| { let (lat, lon) = place.frame.unproject(x, z); LatLon { lat, lon } }).collect()
        };
        let ring = |x0: f64, z0: f64, x1: f64, z1: f64| [[x0, z0], [x1, z0], [x1, z1], [x0, z1], [x0, z0]];
        let tags = |value: &str| -> std::collections::HashMap<String, String> { [("landuse".to_string(), value.to_string())].into() };
        let osm = Osm {
            ways: vec![
                crate::overpass::Way { id: 1, tags: tags("farmland"), geometry: latlon(&ring(-120.0, -120.0, -40.0, -40.0)) },
                crate::overpass::Way { id: 2, tags: tags("farmyard"), geometry: latlon(&ring(40.0, -120.0, 120.0, -40.0)) },
            ],
            areas: vec![Area { id: 3, tags: tags("farmland"), outer: vec![latlon(&ring(-120.0, 40.0, -40.0, 120.0))], inner: Vec::new() }],
            pois: Vec::new(),
        };
        let ground = Ground { origin: -15_000, cell: 500, side: 61, heights: vec![250; 61 * 61] };
        let built = from_osm(&place, 100.0, None, &osm, &[], ground, Placed::default(), None, None, None, &[], None, None, &[]);
        let wheat_at = |x: f64, z: f64| {
            let (col, row) = (((x + 150.0) / 100.0) as u16, ((z + 150.0) / 100.0) as u16);
            let crops = &built.tiles[&(col, row)].crops;
            let cell = |v: f64| ((v + 150.0) % 100.0 / 2.0) as usize;
            !crops.is_empty() && crops[cell(z) * 50 + cell(x)] == CROP_WHEAT
        };
        assert!(wheat_at(-80.0, -80.0), "a field drawn as a closed way");
        assert!(wheat_at(-80.0, 80.0), "a field drawn as a relation");
        assert!(!wheat_at(80.0, -80.0), "a farmyard");
        assert!(!wheat_at(0.0, 0.0), "open ground");
    }

    #[test]
    fn a_graveyard_from_osm_gets_stones_along_its_path_and_a_churchyard_without_one_gets_rows() {
        let place = crate::region::Region::Sweden.around(63.82, 20.26, 150.0);
        let way = |id, (key, value): (&str, &str), points: &[P]| {
            let geometry = points.iter().map(|&[x, z]| {
                let (lat, lon) = place.frame.unproject(x, z);
                LatLon { lat, lon }
            });
            crate::overpass::Way { id, tags: [(key.to_string(), value.to_string())].into(), geometry: geometry.collect() }
        };
        let ring = |x0: f64, z0: f64, x1: f64, z1: f64| [[x0, z0], [x1, z0], [x1, z1], [x0, z1], [x0, z0]];
        let osm = Osm {
            ways: vec![
                way(1, ("landuse", "cemetery"), &ring(-100.0, -40.0, -20.0, 40.0)),
                way(2, ("highway", "footway"), &[[-110.0, 0.0], [-10.0, 0.0]]),
                way(3, ("amenity", "grave_yard"), &ring(30.0, -30.0, 90.0, 30.0)),
                way(4, ("leisure", "park"), &ring(-60.0, 80.0, 60.0, 130.0)),
            ],
            areas: Vec::new(),
            pois: Vec::new(),
        };
        let ground = Ground { origin: -15_000, cell: 500, side: 61, heights: vec![250; 61 * 61] };
        let built = from_osm(&place, 100.0, None, &osm, &[], ground, Placed::default(), None, None, None, &[], None, None, &[]);
        let graves: Vec<Grave> = built.tiles.values().flat_map(|tile| tile.graves.iter().copied()).collect();
        let (cemetery, churchyard): (Vec<Grave>, Vec<Grave>) = graves.iter().partition(|grave| grave.x < 0);
        assert!(cemetery.len() > 100 && churchyard.len() > 100, "{} and {}", cemetery.len(), churchyard.len());
        assert!(graves.iter().all(|grave| grave.base == 250 && grave.z.abs() < 4000), "on the ground, and none in the park");
        // The cemetery's stand along its path only; the churchyard's fill it.
        assert!(cemetery.iter().all(|grave| grave.z.abs() < 1000 && grave.z.abs() > 200), "{:?}", cemetery.iter().map(|grave| grave.z).max());
        assert!(churchyard.iter().any(|grave| grave.z > 2000) && churchyard.iter().any(|grave| grave.z < -2000));
        // Both are mown grass, and no boulder is left in either.
        assert!(built.tiles.values().flat_map(|tile| &tile.cover).filter(|cover| cover.kind == COVER_PARK).count() >= 3);
        let rock_in_a_yard = |tree: &&Tree| matches!(tree.kind, TREE_ROCK_BOULDER | TREE_ROCK_SLAB) && tree.z.abs() < 3000;
        assert_eq!(built.tiles.values().flat_map(|tile| &tile.trees).filter(rock_in_a_yard).count(), 0);
    }

    #[test]
    fn the_synthetic_city_has_a_graveyard_along_a_path_and_one_in_rows() {
        let built = synthetic_city(300.0, 100.0, Ground::default(), Placed::default());
        let graves: Vec<Grave> = built.tiles.values().flat_map(|tile| tile.graves.iter().copied()).collect();
        assert!(graves.len() > 150, "{}", graves.len());
        let kinds: std::collections::BTreeSet<u8> = graves.iter().map(|grave| grave.kind).collect();
        assert_eq!(kinds.len(), GRAVE_KINDS.len());
        // None on a street or the path, in a house or against a fence's line.
        let roads: Vec<&Road> = built.tiles.values().flat_map(|tile| &tile.roads).collect();
        for grave in &graves {
            let at = [f64::from(grave.x) / 100.0, f64::from(grave.z) / 100.0];
            for road in &roads {
                let d = road.points.windows(2).map(|pair| distance_to_segment(at, [f64::from(pair[0][0]) / 100.0, f64::from(pair[0][1]) / 100.0], [f64::from(pair[1][0]) / 100.0, f64::from(pair[1][1]) / 100.0])).fold(f64::MAX, f64::min);
                assert!(d > f64::from(road.width) / 200.0 + 0.5, "{grave:?} is {d} m from a road");
            }
        }
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
    fn a_buildings_front_is_towards_the_nearest_road_that_is_driven_on() {
        // A street along z = 20, and a footpath along x = 5.
        let streets = streets(&[(vec![[-50.0, 20.0], [50.0, 20.0]], 6.0, ROAD_MINOR), (vec![[5.0, -50.0], [5.0, 50.0]], 2.0, ROAD_PATH)]);
        assert_eq!(front_of([0.0, 0.0], &streets), [0, 127], "the street, not the path beside it");
        assert_eq!(front_of([0.0, 40.0], &streets), [0, -127]);
        assert_eq!(front_of([70.0, 0.0], &streets), [-90, 90], "towards the street's end");
        assert_eq!(front_of([0.0, 100.0], &streets), [0, 0], "80 m off is by no road");
        assert_eq!(front_of([0.0, 20.2], &streets), [0, 0], "nor is a road through its middle");
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
    fn a_paved_track_is_an_asphalt_road_and_a_paved_footway_a_paved_path() {
        assert_eq!(road_style("track", Some("asphalt")), Some((PAVED_TRACK, ROAD_MINOR)));
        assert_eq!(road_style("track", Some("paved")), Some((PAVED_TRACK, ROAD_MINOR)));
        assert_eq!(road_style("track", Some("compacted")), Some((2.5, ROAD_PATH)));
        assert_eq!(road_style("track", None), Some((2.5, ROAD_PATH)));
        // A footway or cycleway with asphalt on it is a paved path; a gravel one or steps stay worn.
        assert_eq!(road_style("cycleway", Some("asphalt")), Some((2.5, ROAD_PAVED_PATH)));
        assert_eq!(road_style("pedestrian", Some("paving_stones")), Some((5.0, ROAD_PATH)));
        assert_eq!(road_style("footway", Some("gravel")), Some((2.5, ROAD_PATH)));
        // An untagged cycleway or footway is asphalt, an untagged path worn, a gravel cycleway gravel.
        assert_eq!(road_style("cycleway", None), Some((2.5, ROAD_PAVED_PATH)));
        assert_eq!(road_style("footway", None), Some((2.5, ROAD_PAVED_PATH)));
        assert_eq!(road_style("path", None), Some((2.5, ROAD_PATH)));
        assert_eq!(road_style("cycleway", Some("gravel")), Some((2.5, ROAD_PATH)));
        assert_eq!(road_style("steps", Some("concrete")), Some((2.5, ROAD_PATH)));
        // Any road is asphalt, whatever it is surfaced with.
        assert_eq!(road_style("service", Some("gravel")), Some((4.5, ROAD_MINOR)));
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
        let tops = fit_roofs(&cloud, &|x, z| (x, z), false, 100.0, 100.0, None, &Ground::default(), &[(&ring, None, None, BUILDING_PLAIN), (&ring, Some(9.3), None, BUILDING_PLAIN)]);
        let Some(Top::Fitted(Roof::Leaf(Shape::Flat { y }))) = tops[0] else { panic!("{:?}", tops[0]) };
        assert!(y > 15.0, "uncapped, the crown is the roof: {y}");
        let Some(Top::Fitted(Roof::Leaf(Shape::Flat { y }))) = tops[1] else { panic!("{:?}", tops[1]) };
        assert!((y - 4.0).abs() < 0.3, "capped, the roof is {y}");
    }
    #[test]
    fn a_road_over_a_trench_gets_a_deck_and_one_down_a_slope_does_not() {
        // A 300 m square of ground at a metre a cell, sloping up to the east at 5 %, with a trench
        // 8 m wide and 3 m deep running north to south at x = 20 m.
        let (cell, side) = (100, 301);
        let heights = (0..side * side).map(|k| {
            let x = (k % side) as f64 - 150.0;
            ((x * 0.05 + 10.0 - if (x - 20.0).abs() < 4.0 { 3.0 } else { 0.0 }) * 100.0) as i32
        }).collect();
        let ground = Ground { origin: -15_000, cell, side: side as u32, heights };
        let across = (vec![[-100.0, 0.0], [100.0, 0.0]], 10.0, ROAD_MAJOR);
        let along = (vec![[60.0, -100.0], [60.0, 100.0]], 7.0, ROAD_MINOR);
        let path = (vec![[-100.0, 50.0], [100.0, 50.0]], 2.5, ROAD_PATH);
        let cuts = cuttings(&[across, along, path], &ground);
        assert_eq!(cuts.len(), 1, "only the road across the trench, and no footpath");
        let (points, width, kind) = &cuts[0];
        assert_eq!((*width, *kind), (10.0, ROAD_MAJOR));
        let (west, east) = (points[0][0], points[points.len() - 1][0]);
        assert!(west < 16.0 - CUT_MARGIN + 2.0 && west > 0.0, "starts on the level before the trench: {west}");
        assert!(east > 24.0 + CUT_MARGIN - 2.0 && east < 40.0, "ends on the level past it: {east}");
        // Built as a bridge, its deck is level with the road either side.
        let pieces = bridge::build(points, *width, *kind, Pylons::Flush, &[], &ground, &[], 150.0);
        assert!(!pieces.is_empty(), "a short span over a trench is still built");
        // Flush with the road: nothing stands a metre over the deck, as a railing would.
        let top = pieces.iter().flat_map(|piece| piece.tris.iter().flatten()).map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!(top < 10.0 + 24.0 * 0.05 + CUT_MARGIN * 0.05 + 0.5, "no railing: {top}");
        // A guard rail down the road's middle stands on the deck over the trench, not down in it,
        // and one beside the road, with no deck under it, stops at the trench.
        let manifest = Manifest { version: WORLD_VERSION, half_extent: cm(150.0), tile_size: cm(300.0), ..Default::default() };
        let mut built = Built { manifest, tiles: [((0, 0), Tile::default())].into_iter().collect() };
        let rails = vec![(LINE_GUARDRAIL, vec![[-20.0, 0.0], [60.0, 0.0]]), (LINE_GUARDRAIL, vec![[-20.0, 20.0], [60.0, 20.0]])];
        lay_lines(&mut built, &rails, &[(points.clone(), *width)], 150.0, 300.0, &ground);
        let laid: Vec<&Line> = built.tiles[&(0, 0)].lines.iter().collect();
        let on_road: Vec<&&Line> = laid.iter().filter(|line| line.points[0][1] == 0).collect();
        assert_eq!(on_road.len(), 1, "one rail along the road: {laid:?}");
        let road_level = |x: f64| x * 0.05 + 10.0;
        for (point, base) in on_road[0].points.iter().zip(&on_road[0].base) {
            let x = f64::from(point[0]) / 100.0;
            assert!((f64::from(*base) / 100.0 - road_level(x)).abs() < 0.3, "at {x}: {base} against the road's {}", road_level(x));
        }
        let beside: Vec<&&Line> = laid.iter().filter(|line| line.points[0][1] != 0).collect();
        assert_eq!(beside.len(), 2, "cut at the trench: {beside:?}");
        assert!(beside.iter().all(|line| line.points.iter().all(|p| (f64::from(p[0]) / 100.0 - 20.0).abs() >= 3.0)), "{beside:?}");
    }

    #[test]
    fn bridge_ways_that_meet_end_to_end_are_one_bridge() {
        let way = |points: Vec<P>| (points, 10.0, ROAD_MAJOR, Pylons::None, Deck::new());
        // Two ways of one bridge, the second drawn towards the first, and a separate bridge.
        let joined = joined_bridges(vec![way(vec![[0.0, 0.0], [50.0, 0.0]]), way(vec![[120.0, 0.0], [50.0, 0.5]]), way(vec![[0.0, 300.0], [80.0, 300.0]])]);
        assert_eq!(joined.len(), 2);
        assert_eq!(joined[0].0, vec![[0.0, 0.0], [50.0, 0.0], [120.0, 0.0]]);
        // Three that meet at a point are a fork, and stay as they are; so does a deck over a cutting.
        let fork = joined_bridges(vec![way(vec![[0.0, 0.0], [50.0, 0.0]]), way(vec![[50.0, 0.0], [100.0, 20.0]]), way(vec![[50.0, 0.0], [100.0, -20.0]])]);
        assert_eq!(fork.len(), 3);
        let flush = (vec![[50.0, 0.0], [70.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::Flush, Deck::new());
        assert_eq!(joined_bridges(vec![way(vec![[0.0, 0.0], [50.0, 0.0]]), flush]).len(), 2);
    }

    #[test]
    fn the_two_carriageways_of_a_divided_road_cross_on_one_deck() {
        let way = |points: Vec<P>| (points, 10.0, ROAD_MAJOR, Pylons::None, Deck::new());
        // Two carriageways 8 m apart, and a bridge that only crosses them.
        let ways = vec![way(vec![[0.0, 0.0], [60.0, 0.0], [120.0, 0.0]]), way(vec![[120.0, 8.0], [0.0, 8.0]]), way(vec![[60.0, -80.0], [60.0, 80.0]])];
        let (roads, shared) = twin_decks(ways.clone());
        // The roads are where they were, the two carriageways with no deck of their own.
        assert_eq!(roads.iter().map(|road| road.0.clone()).collect::<Vec<_>>(), ways.iter().map(|way| way.0.clone()).collect::<Vec<_>>());
        assert_eq!(roads.iter().map(|road| road.3).collect::<Vec<_>>(), [Pylons::Carried, Pylons::Carried, Pylons::None]);
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].0, vec![[0.0, 4.0], [60.0, 4.0], [120.0, 4.0]], "along the middle of the two");
        assert_eq!(shared[0].1, 18.0, "wide enough for both");
        let ground = Ground { origin: -15_000, cell: 500, side: 61, heights: vec![0; 61 * 61] };
        assert!(bridge::build(&roads[0].0, 10.0, ROAD_MAJOR, Pylons::Carried, &[], &ground, &[], 150.0).is_empty(), "a carried road builds nothing");
    }
    #[test]
    fn a_deck_over_a_cutting_that_a_bridge_now_covers_is_left_out() {
        let bridge = (vec![[0.0, 0.0], [200.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, Deck::new());
        let cut = |from: f64, to: f64| ((0..=8).map(|k| [from + (to - from) * f64::from(k) / 8.0, 1.0]).collect::<Vec<P>>(), 10.0, ROAD_MAJOR, Pylons::Flush, Deck::new());
        // One under the bridge's end, one mostly past its end, one on another road.
        let (under, past, elsewhere) = (cut(4.0, 20.0), cut(-14.0, 2.0), (vec![[50.0, 80.0], [70.0, 80.0]], 10.0, ROAD_MAJOR, Pylons::Flush, Deck::new()));
        let kept = clear_of_bridges(vec![bridge.clone(), under, past.clone(), elsewhere.clone()]);
        assert_eq!(kept.iter().map(|way| way.0.clone()).collect::<Vec<_>>(), [bridge.0, past.0, elsewhere.0]);
    }

    #[test]
    fn a_road_is_carried_where_it_runs_along_a_deck_and_not_where_it_crosses_under_one() {
        let deck: Vec<P> = vec![[40.0, 0.0], [60.0, 0.0]];
        let decks = [(deck.as_slice(), 10.0)];
        // A road along the deck's line is on the ground up to the deck, carried over it, and on the ground again.
        let runs = carried_runs(&[[0.0, 0.0], [30.0, 0.0], [100.0, 0.0]], &decks);
        assert_eq!(runs.iter().map(|run| run.1).collect::<Vec<_>>(), [false, true, false]);
        assert_eq!(runs[0].0, vec![[0.0, 0.0], [30.0, 0.0], [40.0, 0.0]]);
        assert_eq!(runs[1].0, vec![[40.0, 0.0], [60.0, 0.0]]);
        assert_eq!(runs[2].0, vec![[60.0, 0.0], [100.0, 0.0]]);
        // One that crosses under it, and one far from it, stay whole on the ground.
        for road in [vec![[50.0, -40.0], [50.0, 40.0]], vec![[0.0, 90.0], [100.0, 90.0]]] {
            assert_eq!(carried_runs(&road, &decks), vec![(road.clone(), false)]);
        }
    }

    #[test]
    fn a_bridge_that_ends_down_the_bank_is_lengthened_to_the_top_of_it() {
        // Ground at 10 m west of x = 0 and falling east of it at one in three, down to a river bed.
        let (cell, side) = (100, 301);
        let heights = (0..side * side).map(|k| ((10.0 - ((k % side) as f64 - 150.0).max(0.0) / 3.0).max(0.0) * 100.0) as i32).collect();
        let ground = Ground { origin: -15_000, cell, side: side as u32, heights };
        // The bridge ends 12 m down the bank, where the road from the west meets it.
        let bridge = (vec![[12.0, 0.0], [120.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, Deck::new());
        let road = (vec![[-100.0, 0.0], [12.0, 0.0]], 10.0, ROAD_MAJOR);
        let longer = onto_the_banks(vec![bridge.clone()], &[road], &ground);
        let start = longer[0].0[0];
        assert!(start[0] <= 1.0 && start[0] > -6.0, "it starts at the top of the bank: {start:?}");
        assert_eq!(longer[0].0.last(), Some(&[120.0, 0.0]), "its other end is where it was");
        // With no road on from its end, or on level ground, it is left as it is.
        assert_eq!(onto_the_banks(vec![bridge.clone()], &[], &ground)[0].0, bridge.0);
        let level = (vec![[-120.0, 0.0], [-60.0, 0.0]], 10.0, ROAD_MAJOR, Pylons::None, Deck::new());
        let west = (vec![[-140.0, 0.0], [-120.0, 0.0]], 10.0, ROAD_MAJOR);
        assert_eq!(onto_the_banks(vec![level.clone()], &[west], &ground)[0].0, level.0);
    }

}
