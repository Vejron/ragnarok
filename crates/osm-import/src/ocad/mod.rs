//! Orienteering maps as a source of the world: an OCAD file read as a second OSM, for the boulders,
//! paths and walls that OSM does not have.
//!
//! Inside what the map covers it replaces OSM's roads, buildings and cover; outside it nothing
//! changes. The terrain, the trees and the water stay with their own sources: the laser is better
//! than contour lines, and lakes are carved into the ground from Hydrografi's outlines.
//!
//! The roads and buildings come out as OSM ways, so laser roofs and paint work on them unchanged.

pub mod file;
pub mod isom;
pub mod relief;

use crate::assemble::point_in_polygon;
use crate::minimap::Overlay;
use crate::overpass::{LatLon, Osm, Way};
use crate::region::{Place, Region};
use crate::trees::Planted;
use anyhow::{Context, Result, ensure};
use file::{Map, OBJECT_AREA, OBJECT_LINE, OBJECT_POINT, Paper};
use isom::{Feature, Shrubs, Stone};
use shared::world::{COVER_FARM, COVER_MARSH, COVER_ROCK, COVER_URBAN, COVER_WOOD, TREE_BUSH};
use std::collections::HashMap;
use std::path::Path;

type P = [f64; 2];

/// OCAD's code for SWEREF 99 TM.
const GRID_SWEREF: i32 = 13_002;
/// Metres on the ground of one unit of the paper at 1:1: units are 1/100 mm.
const UNIT: f64 = 1e-5;
/// Symbols from here on are not the terrain: overprint, legends, layout.
const FIRST_NOT_TERRAIN: i32 = 600_000;
/// Side of a building the map marks by its smallest symbol, metres.
const SMALL_BUILDING: f64 = 7.0;
/// Radius of a marsh the map marks by its smallest symbol, metres.
const SMALL_MARSH: f64 = 4.0;
/// Corners of that marsh.
const SMALL_MARSH_SIDES: usize = 10;
/// Width of a narrow marsh, drawn on the map as a line, metres.
const MARSH_WIDTH: f64 = 3.0;
/// Furthest apart two building centres can be and still be the same building, metres.
const MATCH_REACH: f64 = 25.0;
/// The least of each colour in what counts as blank paper.
const PAPER_WHITE: u8 = 235;
/// Way ids of map objects, far past OSM's.
const FIRST_ID: u64 = 1 << 60;

/// Where the paper sits: scale, rotation and origin, and a correction for a map drawn a little off.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    scale: f64,
    origin: [f64; 2],
    sin: f64,
    cos: f64,
}

impl Sheet {
    /// `shift` is `[dx, dz]` in the world's axes: east and south.
    pub fn new(scale: &file::Scale, shift: [f64; 2]) -> Sheet {
        let (sin, cos) = scale.angle.to_radians().sin_cos();
        Sheet { scale: scale.scale * UNIT, origin: [scale.x + shift[0], scale.y - shift[1]], sin, cos }
    }

    /// The point on the paper of an easting and northing.
    pub fn paper(&self, e: f64, n: f64) -> Paper {
        let (x, y) = (e - self.origin[0], n - self.origin[1]);
        [(x * self.cos + y * self.sin) / self.scale, (y * self.cos - x * self.sin) / self.scale]
    }

    /// Easting and northing of a point on the paper.
    pub fn ground(&self, [x, y]: Paper) -> (f64, f64) {
        let (x, y) = (x * self.scale, y * self.scale);
        (self.origin[0] + x * self.cos - y * self.sin, self.origin[1] + x * self.sin + y * self.cos)
    }
}

/// What a map adds to the world.
#[derive(Debug, Default)]
pub struct Source {
    /// The convex outline of the map, in the world's frame. Inside it the map replaces OSM.
    pub outline: Vec<P>,
    pub ways: Vec<Way>,
    /// `(centre line, width, ROAD_*)`, laid as they are: OSM's classes do not fit them.
    pub roads: Vec<(Vec<LatLon>, f64, u8)>,
    /// `(COVER_*, rings)`, the plain forest under the rest first.
    pub covers: Vec<(u8, Vec<Vec<P>>)>,
    pub planted: Vec<Planted>,
    /// `(rings, share of the trees kept)`.
    pub clearings: Vec<(Vec<Vec<P>>, f64)>,
    pub landforms: Vec<relief::Landform>,
    pub cliffs: Vec<relief::Cliff>,
    /// `(LINE_*, line)`: fences and power lines.
    pub lines: Vec<(u8, Vec<P>)>,
    /// How the paper sits on the ground, and the corners of what is drawn on it.
    pub sheet: Option<Sheet>,
    pub paper: [Paper; 2],
}

/// Reads the map at `path` and places it in `place`.
pub fn load(path: &Path, place: &Place, shift: [f64; 2]) -> Result<Source> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let map = file::parse(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    isom::check(&map)?;
    ensure!(place.region == Region::Sweden && map.scale.real && map.scale.grid == GRID_SWEREF, "only maps in SWEREF 99 TM can be placed, this one has grid code {} (real-world coordinates {})", map.scale.grid, map.scale.real);
    if map.encrypted > 0 {
        eprintln!("ocad: {} encrypted objects left out", map.encrypted);
    }
    Ok(from_map(&map, place, shift))
}

fn from_map(map: &Map, place: &Place, shift: [f64; 2]) -> Source {
    let sheet = Sheet::new(&map.scale, shift);
    let degrees = |paper: Paper| {
        let (e, n) = sheet.ground(paper);
        place.region.degrees(e, n)
    };
    let local = |paper: Paper| {
        let (lat, lon) = degrees(paper);
        place.frame.project(lat, lon)
    };
    let latlon = |paper: Paper| {
        let (lat, lon) = degrees(paper);
        LatLon { lat, lon }
    };
    let terrain = || map.objects.iter().filter(|object| object.symbol < FIRST_NOT_TERRAIN);
    let outline = convex_hull(terrain().flat_map(|object| object.rings.iter().flatten().map(|&p| local(p))).collect());
    let (lo, hi) = map.objects.iter().flat_map(|object| object.rings.iter().flatten()).fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let mut source = Source { outline, sheet: Some(sheet), paper: [lo, hi], ..Source::default() };
    let way = |id: usize, tags: [(&str, &str); 1], geometry: Vec<LatLon>| Way { id: FIRST_ID + id as u64, tags: tags.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<HashMap<_, _>>(), geometry };
    // Laid under the rest, and cover is drawn low ground first: what lies below the forest is cut out of it.
    let mut base = vec![source.outline.clone()];
    let mut covers = Vec::new();
    for (index, object) in terrain().enumerate() {
        let Some(feature) = isom::feature(object.symbol) else { continue };
        let Some(first) = object.rings.first().and_then(|ring| ring.first()) else { continue };
        if let (Some(keep), OBJECT_AREA) = (isom::trees_kept(object.symbol), object.kind) {
            source.clearings.push((object.rings.iter().map(|ring| ring.iter().map(|&p| local(p)).collect()).collect(), keep));
        }
        match (feature, object.kind) {
            (Feature::Landform(form), OBJECT_POINT) => {
                let (e, n) = sheet.ground(*first);
                let isom::Form { along, across, target, power } = form;
                // A long knoll's symbol lies across its object's angle: at Kläppudden the height model
                // stands 0.19 m higher that way than along it, over 48 of them.
                source.landforms.push(relief::Landform { e, n, along, across, angle: object.angle + map.scale.angle + 90.0, target, power });
            }
            (Feature::Cliff(ledge), OBJECT_LINE) => {
                let points = object.rings[0].iter().map(|&p| sheet.ground(p).into()).collect();
                source.cliffs.push(relief::Cliff { points, target: ledge.height, face: ledge.face });
                let line: Vec<P> = object.rings[0].iter().map(|&p| local(p)).collect();
                let band = strip(&line, ledge.rock);
                source.clearings.push((vec![band.clone()], 0.0));
                covers.push((COVER_ROCK, vec![band]));
            }
            (Feature::Line(wire), OBJECT_LINE) => {
                let line: Vec<P> = object.rings[0].iter().map(|&p| local(p)).collect();
                if wire.cleared > 0.0 {
                    source.clearings.push((vec![strip(&line, wire.cleared)], 0.0));
                }
                source.lines.push((wire.kind, line));
            }
            (Feature::Stone(stone), OBJECT_POINT) => source.planted.extend(stones(stone, degrees(*first), index)),
            (Feature::Road(track), OBJECT_LINE) => {
                source.roads.push((object.rings[0].iter().map(|&p| latlon(p)).collect(), track.width, track.kind));
                if track.cleared > 0.0 {
                    let line: Vec<P> = object.rings[0].iter().map(|&p| local(p)).collect();
                    source.clearings.push((vec![strip(&line, track.cleared)], 0.0));
                }
            }
            (Feature::Building, OBJECT_AREA) => {
                let mut ring: Vec<LatLon> = object.rings[0].iter().map(|&p| latlon(p)).collect();
                if ring.first().map(|p| (p.lat, p.lon)) != ring.last().map(|p| (p.lat, p.lon)) {
                    ring.push(ring[0]);
                }
                source.ways.push(way(index, [("building", "yes")], ring));
            }
            (Feature::Building, OBJECT_POINT) => {
                let half = SMALL_BUILDING / 2.0 / (map.scale.scale * UNIT);
                let (sin, cos) = object.angle.to_radians().sin_cos();
                let corner = |x: f64, y: f64| latlon([first[0] + half * (x * cos - y * sin), first[1] + half * (x * sin + y * cos)]);
                source.ways.push(way(index, [("building", "yes")], vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0), corner(-1.0, -1.0)]));
            }
            (Feature::Cover(kind), OBJECT_AREA) => {
                let rings: Vec<Vec<P>> = object.rings.iter().map(|ring| ring.iter().map(|&p| local(p)).collect()).collect();
                if matches!(kind, COVER_FARM | COVER_URBAN) {
                    base.extend(rings.iter().cloned());
                }
                covers.push((kind, rings));
            }
            (Feature::Cover(COVER_MARSH), OBJECT_POINT) => covers.push((COVER_MARSH, vec![disc(local(*first), SMALL_MARSH)])),
            (Feature::Cover(COVER_MARSH), OBJECT_LINE) => {
                let line: Vec<P> = object.rings[0].iter().map(|&p| local(p)).collect();
                covers.push((COVER_MARSH, vec![strip(&line, MARSH_WIDTH)]));
            }
            (Feature::Thicket(kind, shrubs), OBJECT_AREA) => {
                let rings: Vec<Vec<P>> = object.rings.iter().map(|ring| ring.iter().map(|&p| local(p)).collect()).collect();
                source.planted.extend(thicket(&rings, shrubs, index).into_iter().map(|(at, height, crown)| {
                    let (lat, lon) = place.frame.unproject(at[0], at[1]);
                    Planted { lat, lon, kind: TREE_BUSH, height, crown, registered: false }
                }));
                covers.push((kind, rings));
            }
            _ => {}
        }
    }
    source.covers.push((COVER_WOOD, base));
    source.covers.extend(covers);
    source
}

/// Near-white pixels joined to `seeds`. JPEG leaves paper a little off white.
fn blank_paper(pixels: &[u8], width: usize, height: usize, seeds: &[(usize, usize)]) -> Vec<bool> {
    let white = |i: usize, j: usize| pixels[(j * width + i) * 3..][..3].iter().all(|&value| value >= PAPER_WHITE);
    let mut blank = vec![false; width * height];
    let mut open: Vec<(usize, usize)> = seeds.iter().copied().filter(|&(i, j)| white(i, j)).collect();
    while let Some((i, j)) = open.pop() {
        if std::mem::replace(&mut blank[j * width + i], true) {
            continue;
        }
        let around = [(i.wrapping_sub(1), j), (i + 1, j), (i, j.wrapping_sub(1)), (i, j + 1)];
        open.extend(around.into_iter().filter(|&(a, b)| a < width && b < height && !blank[b * width + a] && white(a, b)));
    }
    blank
}

/// A value in `[0, 1)` from an object and a salt, so a build gives the same rocks every time.
fn unit(object: usize, salt: u64) -> f64 {
    let mut x = ((object as u64) << 16 ^ salt).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((x ^ (x >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// The rocks of one symbol: a few around the point where it stands for a group, all a little
/// different in size. No two boulders in a forest match.
fn stones(stone: Stone, (lat, lon): (f64, f64), object: usize) -> Vec<Planted> {
    let per_lon = lat.to_radians().cos();
    (0..u64::from(stone.count))
        .map(|i| {
            let (angle, reach) = (unit(object, i * 4) * std::f64::consts::TAU, unit(object, i * 4 + 1).sqrt() * stone.spread);
            let (north, east) = (reach * angle.cos(), reach * angle.sin());
            Planted {
                lat: lat + north / METRES_PER_DEGREE,
                lon: lon + east / (METRES_PER_DEGREE * per_lon),
                kind: stone.kind,
                height: stone.height * (0.85 + 0.3 * unit(object, i * 4 + 2)),
                crown: stone.crown * (0.85 + 0.3 * unit(object, i * 4 + 3)),
                registered: stone.solid,
            }
        })
        .collect()
}

const METRES_PER_DEGREE: f64 = 111_200.0;

/// `(position, height, crown)` of the shrubs of a thicket: thrown at its bounding box, the thicket's
/// holes and outside left bare.
fn thicket(rings: &[Vec<P>], shrubs: Shrubs, object: usize) -> Vec<(P, f64, f64)> {
    let Some(b) = crate::props::bounds(rings) else { return Vec::new() };
    let attempts = ((b[2] - b[0]) * (b[3] - b[1]) / shrubs.per_area) as u64;
    (0..attempts)
        .filter_map(|i| {
            let at = [b[0] + unit(object, i * 4) * (b[2] - b[0]), b[1] + unit(object, i * 4 + 1) * (b[3] - b[1])];
            crate::props::inside(rings, at).then(|| (at, shrubs.height * (0.75 + 0.5 * unit(object, i * 4 + 2)), shrubs.crown * (0.75 + 0.5 * unit(object, i * 4 + 3))))
        })
        .collect()
}

fn disc(centre: P, radius: f64) -> Vec<P> {
    (0..SMALL_MARSH_SIDES)
        .map(|i| {
            let (sin, cos) = (i as f64 / SMALL_MARSH_SIDES as f64 * std::f64::consts::TAU).sin_cos();
            [centre[0] + radius * cos, centre[1] + radius * sin]
        })
        .collect()
}

/// The outline of a band `width` metres wide along `line`.
fn strip(line: &[P], width: f64) -> Vec<P> {
    let mut left = Vec::with_capacity(line.len() * 2);
    let mut right = Vec::with_capacity(line.len());
    for (i, p) in line.iter().enumerate() {
        let (a, b) = (line[i.saturating_sub(1)], line[(i + 1).min(line.len() - 1)]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz).max(1e-6);
        let (nx, nz) = (-dz / length * width / 2.0, dx / length * width / 2.0);
        left.push([p[0] + nx, p[1] + nz]);
        right.push([p[0] - nx, p[1] - nz]);
    }
    right.reverse();
    left.extend(right);
    left
}

/// The convex hull, counter-clockwise when x runs right and z up (Andrew's monotone chain).
fn convex_hull(mut points: Vec<P>) -> Vec<P> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turn = |a: P, b: P, c: P| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let mut hull: Vec<P> = Vec::with_capacity(points.len() + 1);
    let chain = |hull: &mut Vec<P>, points: &mut dyn Iterator<Item = &P>| {
        let floor = hull.len();
        for &p in points {
            while hull.len() >= floor + 2 && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    };
    chain(&mut hull, &mut points.iter());
    chain(&mut hull, &mut points.iter().rev());
    hull
}

/// The part of `ring` on the side of the line `a`-`b` that `side` picks: `1.0` for the left.
fn half_plane(ring: &[P], a: P, b: P, side: f64) -> Vec<P> {
    let at = |p: P| side * ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]));
    let mut kept = Vec::with_capacity(ring.len() + 2);
    for (i, &p) in ring.iter().enumerate() {
        let q = ring[(i + 1) % ring.len()];
        let (dp, dq) = (at(p), at(q));
        if dp >= 0.0 {
            kept.push(p);
        }
        if (dp >= 0.0) != (dq >= 0.0) {
            let t = dp / (dp - dq);
            kept.push([p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]);
        }
    }
    kept
}

impl Source {
    pub fn covers(&self, at: P) -> bool {
        self.outline.len() >= 3 && point_in_polygon(at, &self.outline)
    }

    /// `osm` with the map in place of what it had inside the outline: buildings go, roads are cut
    /// where they enter, and the map's own ways join. Bridges stay, as the map draws none.
    pub fn over(&mut self, osm: Osm, place: &Place) -> Osm {
        self.report_offset(&osm, place);
        let inside = |point: &LatLon| self.covers(place.frame.project(point.lat, point.lon));
        let mut ways = Vec::with_capacity(osm.ways.len());
        for way in osm.ways {
            let bridge = way.tags.get("bridge").is_some_and(|value| value != "no");
            if way.tags.contains_key("building") {
                if !way.geometry.iter().any(inside) {
                    ways.push(way);
                }
            } else if way.tags.contains_key("highway") && !bridge && way.geometry.iter().any(inside) {
                let Way { id, tags, geometry } = way;
                let mut run = Vec::new();
                let mut piece = 0;
                for point in geometry.into_iter().chain([LatLon { lat: f64::NAN, lon: f64::NAN }]) {
                    if !point.lat.is_nan() && !inside(&point) {
                        run.push(point);
                    } else {
                        if run.len() >= 2 {
                            // `building_height` aside nothing reads the id, and these are no buildings.
                            ways.push(Way { id: id + (piece << 48), tags: tags.clone(), geometry: std::mem::take(&mut run) });
                            piece += 1;
                        }
                        run.clear();
                    }
                }
            } else {
                ways.push(way);
            }
        }
        ways.append(&mut self.ways);
        Osm { ways, areas: osm.areas }
    }

    /// `trees` without those the map's open land clears, or that would stand in a pit. The choice
    /// hangs on where a tree stands, so a build repeats.
    pub fn clear(&self, trees: Vec<Planted>, place: &Place) -> Vec<Planted> {
        let boxes: Vec<_> = self.clearings.iter().filter_map(|(rings, keep)| Some((crate::props::bounds(rings)?, rings, *keep))).collect();
        let before = trees.len();
        let kept: Vec<Planted> = trees
            .into_iter()
            .filter(|tree| {
                let at = place.frame.project(tree.lat, tree.lon);
                let spot = (at[0] * 100.0) as i64 as usize ^ ((at[1] * 100.0) as i64 as usize) << 24;
                let (e, n) = place.region.source(tree.lat, tree.lon);
                if self.landforms.iter().any(|form| form.target < 0.0 && (form.e - e).hypot(form.n - n) < form.along) {
                    return false;
                }
                !boxes.iter().any(|(b, rings, keep)| at[0] >= b[0] && at[0] <= b[2] && at[1] >= b[1] && at[1] <= b[3] && crate::props::inside(rings, at) && unit(spot, 7) >= *keep)
            })
            .collect();
        eprintln!("ocad: {} trees cleared from open land and pits, {} left", before - kept.len(), kept.len());
        kept
    }

    /// The image at `path`, a print of the map to its exact edges, placed for `minimap::render`. White
    /// is forest, but also the blank paper past the map's last sea: that is left out, from the corners
    /// of the print that lie outside the map.
    pub fn print(&self, path: &Path, place: &Place) -> Result<Overlay> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let mut decoder = zune_jpeg::JpegDecoder::new(zune_core::bytestream::ZCursor::new(&bytes));
        let pixels = decoder.decode().with_context(|| format!("decoding {}", path.display()))?;
        let (width, height) = decoder.dimensions().context("the image has no size")?;
        ensure!(pixels.len() == width * height * 3, "the image is not RGB");
        let sheet = self.sheet.context("the map has no sheet")?;
        let [lo, hi] = self.paper;
        let to_image = move |[x, y]: Paper| [(x - lo[0]) / (hi[0] - lo[0]) * width as f64, (hi[1] - y) / (hi[1] - lo[1]) * height as f64];
        let region = place.region;
        let frame = place.frame;
        let at = move |x: f64, z: f64| {
            let (lat, lon) = frame.unproject(x, z);
            let (e, n) = region.source(lat, lon);
            to_image(sheet.paper(e, n))
        };
        let corners = [(0, 0), (width - 1, 0), (0, height - 1), (width - 1, height - 1)];
        let outside = |&(i, j): &(usize, usize)| {
            let paper = [lo[0] + (i as f64 + 0.5) / width as f64 * (hi[0] - lo[0]), hi[1] - (j as f64 + 0.5) / height as f64 * (hi[1] - lo[1])];
            let (e, n) = sheet.ground(paper);
            let (lat, lon) = region.degrees(e, n);
            !self.covers(frame.project(lat, lon))
        };
        let blank = blank_paper(&pixels, width, height, &corners.iter().copied().filter(outside).collect::<Vec<_>>());
        Ok(Overlay { width, height, pixels, blank, at: Box::new(at) })
    }

    /// Says how far the map's buildings lie from OSM's, which is how a badly placed map shows.
    fn report_offset(&self, osm: &Osm, place: &Place) {
        let centres = |ways: &[Way]| -> Vec<P> {
            let centre = |way: &Way| {
                let sum = way.geometry.iter().fold([0.0; 2], |sum, g| {
                    let p = place.frame.project(g.lat, g.lon);
                    [sum[0] + p[0], sum[1] + p[1]]
                });
                [sum[0] / way.geometry.len() as f64, sum[1] / way.geometry.len() as f64]
            };
            ways.iter().filter(|way| way.tags.contains_key("building") && !way.geometry.is_empty()).map(centre).collect()
        };
        let theirs = centres(&osm.ways);
        let (mut dx, mut dz): (Vec<f64>, Vec<f64>) = centres(&self.ways)
            .into_iter()
            .filter_map(|ours| {
                let nearest = theirs.iter().min_by(|a, b| (a[0] - ours[0]).hypot(a[1] - ours[1]).total_cmp(&(b[0] - ours[0]).hypot(b[1] - ours[1])))?;
                ((nearest[0] - ours[0]).hypot(nearest[1] - ours[1]) < MATCH_REACH).then_some((nearest[0] - ours[0], nearest[1] - ours[1]))
            })
            .unzip();
        if dx.is_empty() {
            eprintln!("ocad: no building matches OSM's, so the map's placement is unchecked");
            return;
        }
        dx.sort_by(f64::total_cmp);
        dz.sort_by(f64::total_cmp);
        eprintln!("ocad: {} buildings match OSM's, median offset to them {:.1} m east, {:.1} m south (`ocad_shift` moves the map)", dx.len(), dx[dx.len() / 2], dz[dz.len() / 2]);
    }

    /// What is left of a cover outside the outline. The rest of a ring is cut into convex pieces, one
    /// past each side of the outline, so one cover can come back as several.
    pub fn outside(&self, rings: &[Vec<P>]) -> Vec<Vec<Vec<P>>> {
        let hull = &self.outline;
        if hull.len() < 3 {
            return vec![rings.to_vec()];
        }
        let area: f64 = (0..hull.len()).map(|i| hull[i][0] * hull[(i + 1) % hull.len()][1] - hull[(i + 1) % hull.len()][0] * hull[i][1]).sum();
        let left = area.signum();
        let mut pieces = Vec::new();
        for i in 0..hull.len() {
            let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
            let piece: Vec<Vec<P>> = rings
                .iter()
                .map(|ring| {
                    // Past this side, and within the ones before it: the pieces must not overlap.
                    let mut cut = half_plane(ring, a, b, -left);
                    for j in 0..i {
                        if cut.len() >= 3 {
                            cut = half_plane(&cut, hull[j], hull[(j + 1) % hull.len()], left);
                        }
                    }
                    cut
                })
                .filter(|ring| ring.len() >= 3)
                .collect();
            if !piece.is_empty() {
                pieces.push(piece);
            }
        }
        pieces
    }
}

#[cfg(test)]
mod tests {
    use super::file::{Object, Scale, Symbol};
    use super::*;
    use shared::world::*;

    fn place() -> Place {
        // Around the sheet's origin.
        let (lat, lon) = Region::Sweden.degrees(758_000.0, 7_070_000.0);
        Region::Sweden.around(lat, lon, 2000.0)
    }

    fn scale() -> Scale {
        Scale { scale: 10_000.0, x: 758_000.0, y: 7_070_000.0, angle: 0.0, grid: GRID_SWEREF, real: true }
    }

    fn map(objects: Vec<Object>) -> Map {
        let mut map = Map { scale: scale(), objects, ..Map::default() };
        map.symbols.insert(204_000, Symbol { number: 204_000, kind: 1, description: "Sten".into() });
        map
    }

    fn square(symbol: i32, kind: u8, side: f64) -> Object {
        Object { symbol, kind, angle: 0.0, rings: vec![vec![[0.0, 0.0], [side, 0.0], [side, side], [0.0, side]]] }
    }

    #[test]
    fn the_sheet_is_metres_on_the_ground() {
        let sheet = Sheet::new(&scale(), [0.0, 0.0]);
        // 10 mm on paper at 1:10 000 are 100 m.
        assert_eq!(sheet.ground([1000.0, -2000.0]), (758_100.0, 7_069_800.0));
        let shifted = Sheet::new(&scale(), [3.0, 4.0]);
        assert_eq!(shifted.ground([0.0, 0.0]), (758_003.0, 7_069_996.0));
    }

    #[test]
    fn a_rotated_sheet_turns_counter_clockwise() {
        let sheet = Sheet::new(&Scale { angle: 90.0, ..scale() }, [0.0, 0.0]);
        let (e, n) = sheet.ground([1000.0, 0.0]);
        assert!((e - 758_000.0).abs() < 1e-6 && (n - 7_070_100.0).abs() < 1e-6, "{e} {n}");
    }

    #[test]
    fn a_control_point_lands_where_the_region_says() {
        let source = from_map(&map(vec![Object { symbol: 204_000, kind: OBJECT_POINT, angle: 0.0, rings: vec![vec![[3000.0, -5000.0]]] }]), &place(), [0.0, 0.0]);
        let rock = &source.planted[0];
        let (e, n) = Region::Sweden.source(rock.lat, rock.lon);
        assert!((e - 758_300.0).abs() < 0.01 && (n - 7_069_500.0).abs() < 0.01, "{e} {n}");
    }

    #[test]
    fn symbols_become_their_features() {
        let source = from_map(&map(vec![square(101_000, OBJECT_LINE, 9000.0), square(505_000, OBJECT_LINE, 100.0), square(521_000, OBJECT_AREA, 300.0), square(401_000, OBJECT_AREA, 300.0), square(308_000, OBJECT_AREA, 300.0), square(501_000, OBJECT_AREA, 300.0)]), &place(), [0.0, 0.0]);
        assert_eq!((source.ways.len(), source.roads.len()), (1, 1));
        assert!(source.ways[0].tags["building"] == "yes");
        assert_eq!((source.roads[0].1, source.roads[0].2), (1.6, shared::world::ROAD_PATH));
        let ring = &source.ways[0].geometry;
        assert_eq!((ring[0].lat, ring[0].lon), (ring[ring.len() - 1].lat, ring[ring.len() - 1].lon), "a building is closed");
        // The forest first, with paving (drawn under forest) cut out of it and the open land not.
        let kinds: Vec<u8> = source.covers.iter().map(|cover| cover.0).collect();
        assert_eq!(kinds, [COVER_WOOD, COVER_PARK, COVER_MARSH, COVER_URBAN]);
        assert_eq!(source.covers[0].1.len(), 2);
    }

    #[test]
    fn marsh_lies_over_the_forest_in_all_its_forms() {
        let line = Object { symbol: 309_000, kind: OBJECT_LINE, angle: 0.0, rings: vec![vec![[0.0, 0.0], [1000.0, 0.0]]] };
        let point = Object { symbol: 308_001, kind: OBJECT_POINT, angle: 0.0, rings: vec![vec![[2000.0, 0.0]]] };
        let source = from_map(&map(vec![square(101_000, OBJECT_LINE, 9000.0), square(308_000, OBJECT_AREA, 300.0), line, point]), &place(), [0.0, 0.0]);
        assert_eq!(source.covers.iter().filter(|cover| cover.0 == COVER_MARSH).count(), 3);
        assert_eq!(source.covers[0].1.len(), 1, "nothing to cut out of the forest: marsh is drawn on top");
        let band = &source.covers[2].1[0];
        assert_eq!(band.len(), 4);
        // The Swedish frame is about 1% narrow this far from its reference latitude.
        assert!((area(std::slice::from_ref(band)) - 300.0).abs() < 6.0, "100 m by 3 m: {}", area(std::slice::from_ref(band)));
    }

    #[test]
    fn a_thicket_is_shrubs_by_its_density_and_keeps_out_of_its_holes() {
        let ring = |a: f64, b: f64| vec![[a, a], [b, a], [b, b], [a, b]];
        let rings = vec![ring(0.0, 100.0), ring(40.0, 60.0)];
        let shrubs = Shrubs { per_area: 10.0, height: 2.0, crown: 1.0 };
        let found = thicket(&rings, shrubs, 3);
        assert!((850..1050).contains(&found.len()), "{}", found.len());
        assert_eq!(found, thicket(&rings, shrubs, 3));
        assert!(found.iter().all(|(at, ..)| !(40.0..60.0).contains(&at[0]) || !(40.0..60.0).contains(&at[1])));
    }

    #[test]
    fn a_green_becomes_cover_and_shrubs() {
        let source = from_map(&map(vec![square(101_000, OBJECT_LINE, 9000.0), square(408_000, OBJECT_AREA, 1000.0)]), &place(), [0.0, 0.0]);
        assert_eq!(source.covers[1].0, COVER_THICKET_WALK);
        // 100 m on a side, one shrub to 9 m².
        assert!((1000..1200).contains(&source.planted.len()), "{}", source.planted.len());
        assert!(source.planted.iter().all(|shrub| shrub.kind == TREE_BUSH && !shrub.registered));
    }

    #[test]
    fn open_land_loses_its_trees_and_a_thicket_keeps_them() {
        let place = place();
        let open = square(401_000, OBJECT_AREA, 1000.0);
        let scattered = Object { symbol: 402_000, rings: vec![vec![[2000.0, 0.0], [3000.0, 0.0], [3000.0, 1000.0], [2000.0, 1000.0]]], ..open.clone() };
        let source = from_map(&map(vec![open, scattered]), &place, [0.0, 0.0]);
        let at = |paper_x: f64, paper_y: f64| {
            let (lat, lon) = Region::Sweden.degrees(758_000.0 + paper_x * 0.1, 7_070_000.0 + paper_y * 0.1);
            Planted { lat, lon, kind: TREE_PINE, height: 10.0, crown: 2.0, registered: false }
        };
        let grid = |x0: f64| (0..30).flat_map(move |i| (0..30).map(move |j| (x0 + 50.0 + f64::from(i) * 30.0, 50.0 + f64::from(j) * 30.0)));
        assert!(source.clear(grid(0.0).map(|(x, y)| at(x, y)).collect(), &place).is_empty());
        let thinned = source.clear(grid(2000.0).map(|(x, y)| at(x, y)).collect(), &place);
        assert!((60..210).contains(&thinned.len()), "{}", thinned.len());
        assert_eq!(thinned.len(), source.clear(grid(2000.0).map(|(x, y)| at(x, y)).collect(), &place).len());
        assert_eq!(source.clear(grid(5000.0).map(|(x, y)| at(x, y)).collect(), &place).len(), 900, "outside the open land");
    }

    #[test]
    fn a_pit_keeps_no_tree_and_a_long_knoll_keeps_its_angle() {
        let place = place();
        let pit = Object { symbol: 111_000, kind: OBJECT_POINT, angle: 0.0, rings: vec![vec![[0.0, 0.0]]] };
        let knoll = Object { symbol: 110_000, angle: 45.0, ..pit.clone() };
        let source = from_map(&map(vec![pit, knoll]), &place, [0.0, 0.0]);
        assert_eq!(source.landforms[1].angle, 135.0);
        let tree = |n: f64| {
            let (lat, lon) = Region::Sweden.degrees(758_000.0, n);
            Planted { lat, lon, kind: TREE_PINE, height: 10.0, crown: 2.0, registered: false }
        };
        assert_eq!(source.clear(vec![tree(7_070_001.0), tree(7_070_020.0)], &place).len(), 1);
    }

    #[test]
    fn blank_paper_stops_at_the_first_colour() {
        // White, a teal wall, white: the far side is forest, not paper.
        let row = [[255, 255, 255], [250, 251, 249], [0, 200, 200], [255, 255, 255]];
        let pixels: Vec<u8> = row.iter().flatten().copied().collect();
        assert_eq!(blank_paper(&pixels, 4, 1, &[(0, 0)]), [true, true, false, false]);
        assert_eq!(blank_paper(&pixels, 4, 1, &[(2, 0)]), [false; 4]);
    }

    #[test]
    fn the_paper_is_where_the_sheet_says() {
        let sheet = Sheet::new(&Scale { angle: 30.0, ..scale() }, [2.0, 3.0]);
        let (e, n) = sheet.ground([-29_888.0, -11_690.0]);
        let [x, y] = sheet.paper(e, n);
        assert!((x + 29_888.0).abs() < 1e-6 && (y + 11_690.0).abs() < 1e-6, "{x} {y}");
    }

    #[test]
    fn a_cliff_is_a_step_a_band_of_rock_and_no_trees() {
        let line = Object { symbol: 201_002, kind: OBJECT_LINE, angle: 0.0, rings: vec![vec![[0.0, 0.0], [1000.0, 0.0]]] };
        let source = from_map(&map(vec![square(101_000, OBJECT_LINE, 9000.0), line]), &place(), [0.0, 0.0]);
        assert_eq!(source.cliffs.len(), 1);
        assert_eq!(source.cliffs[0].points, [[758_000.0, 7_070_000.0], [758_100.0, 7_070_000.0]]);
        assert_eq!(source.covers[1].0, COVER_ROCK);
        assert_eq!(source.clearings[0].1, 0.0);
    }

    #[test]
    fn a_power_line_clears_its_lane_and_a_fence_does_not() {
        let along = |symbol| Object { symbol, kind: OBJECT_LINE, angle: 0.0, rings: vec![vec![[0.0, 0.0], [1000.0, 0.0]]] };
        let source = from_map(&map(vec![along(510_000), along(516_000)]), &place(), [0.0, 0.0]);
        assert_eq!(source.lines.iter().map(|line| line.0).collect::<Vec<_>>(), [LINE_POWER, LINE_FENCE]);
        assert_eq!(source.clearings.len(), 1);
        assert!((area(&source.clearings[0].0) - 1400.0).abs() < 28.0, "100 m by 14 m: {}", area(&source.clearings[0].0));
    }

    #[test]
    fn stone_groups_spread_and_repeat() {
        let group = isom::feature(207_000);
        let Some(Feature::Stone(stone)) = group else { panic!() };
        let a = stones(stone, (63.65, 20.13), 5);
        assert_eq!(a.len(), 4);
        assert_eq!(a.iter().map(|rock| rock.height).collect::<Vec<_>>(), stones(stone, (63.65, 20.13), 5).iter().map(|rock| rock.height).collect::<Vec<_>>());
        for rock in &a {
            let east = (rock.lon - 20.13) * METRES_PER_DEGREE * 63.65f64.to_radians().cos();
            assert!(((rock.lat - 63.65) * METRES_PER_DEGREE).hypot(east) <= stone.spread + 0.01);
        }
    }

    #[test]
    fn a_hull_holds_all_its_points() {
        let points = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [5.0, 5.0], [5.0, 0.0], [5.0, 12.0]];
        let hull = convex_hull(points.clone());
        assert_eq!(hull.len(), 5);
        assert!(!hull.contains(&[5.0, 5.0]));
    }

    fn square_source() -> Source {
        Source { outline: convex_hull(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]), ..Source::default() }
    }

    fn area(rings: &[Vec<P>]) -> f64 {
        // Even-odd, so holes count against.
        let shoelace = |ring: &Vec<P>| (0..ring.len()).map(|i| ring[i][0] * ring[(i + 1) % ring.len()][1] - ring[(i + 1) % ring.len()][0] * ring[i][1]).sum::<f64>().abs() / 2.0;
        shoelace(&rings[0]) - rings[1..].iter().map(shoelace).sum::<f64>()
    }

    #[test]
    fn a_cover_keeps_only_what_lies_outside_the_map() {
        let source = square_source();
        let cover = vec![vec![[5.0, 5.0], [20.0, 5.0], [20.0, 8.0], [5.0, 8.0]]];
        let pieces = source.outside(&cover);
        assert_eq!(pieces.iter().map(|rings| area(rings)).sum::<f64>(), 30.0);
        assert!(pieces.iter().flatten().flatten().all(|p| p[0] >= 10.0));
        let around = vec![vec![[-5.0, -5.0], [15.0, -5.0], [15.0, 15.0], [-5.0, 15.0]]];
        assert_eq!(source.outside(&around).iter().map(|rings| area(rings)).sum::<f64>(), 300.0);
        assert!(source.outside(&[vec![[2.0, 2.0], [3.0, 2.0], [3.0, 3.0]]]).is_empty());
    }

    fn line(place: &Place, id: u64, tag: (&str, &str), points: &[P]) -> Way {
        let geometry = points.iter().map(|&[x, z]| {
            let (lat, lon) = place.frame.unproject(x, z);
            LatLon { lat, lon }
        });
        Way { id, tags: [(tag.0.to_string(), tag.1.to_string())].into(), geometry: geometry.collect() }
    }

    #[test]
    fn inside_the_outline_the_map_replaces_osm() {
        let place = place();
        let mut source = Source { outline: convex_hull(vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]), ..Source::default() };
        source.ways.push(line(&place, FIRST_ID, ("highway", "path"), &[[10.0, 10.0], [20.0, 20.0]]));
        let osm = Osm {
            ways: vec![
                line(&place, 1, ("building", "yes"), &[[50.0, 50.0], [55.0, 50.0], [55.0, 55.0]]),
                line(&place, 2, ("building", "yes"), &[[150.0, 50.0], [155.0, 50.0], [155.0, 55.0]]),
                line(&place, 3, ("highway", "service"), &[[-20.0, 50.0], [-10.0, 50.0], [50.0, 50.0], [150.0, 50.0], [160.0, 50.0]]),
                line(&place, 4, ("highway", "path"), &[[200.0, 0.0], [210.0, 0.0]]),
                line(&place, 5, ("bridge", "yes"), &[[50.0, 50.0], [55.0, 50.0]]),
                line(&place, 6, ("railway", "rail"), &[[50.0, 50.0], [55.0, 50.0]]),
            ],
            areas: Vec::new(),
        };
        let merged = source.over(osm, &place);
        let ids: Vec<u64> = merged.ways.iter().map(|way| way.id & 0xffff).collect();
        // The inner building goes, the road is cut in two, the rest stays, the map's path joins.
        assert_eq!(ids, [2, 3, 3, 4, 5, 6, 0]);
        assert_eq!(merged.ways[1].geometry.len() + merged.ways[2].geometry.len(), 4);
    }

    #[test]
    fn a_small_building_is_its_size_on_the_ground() {
        let object = Object { symbol: 521_003, kind: OBJECT_POINT, angle: 0.0, rings: vec![vec![[0.0, 0.0]]] };
        let place = place();
        let source = from_map(&map(vec![object]), &place, [0.0, 0.0]);
        let ring = &source.ways[0].geometry;
        let [x0, _] = place.frame.project(ring[0].lat, ring[0].lon);
        let [x1, _] = place.frame.project(ring[1].lat, ring[1].lon);
        let (e0, _) = Region::Sweden.source(ring[0].lat, ring[0].lon);
        let (e1, _) = Region::Sweden.source(ring[1].lat, ring[1].lon);
        assert!((e1 - e0 - SMALL_BUILDING).abs() < 0.01, "{}", e1 - e0);
        assert!(x1 > x0);
    }

    #[test]
    fn a_map_in_another_grid_is_refused() {
        let dir = std::env::temp_dir().join("ocad-grid-test.ocd");
        let mut builder = file::tests::Builder::new(12);
        builder.scale = "\tm10000\tx0\ty0\ta0\ti3033\tr1".into();
        std::fs::write(&dir, builder.build()).unwrap();
        assert!(load(&dir, &place(), [0.0, 0.0]).is_err());
        std::fs::remove_file(&dir).ok();
    }
}
