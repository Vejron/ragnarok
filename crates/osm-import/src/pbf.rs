//! A local OpenStreetMap extract: a Geofabrik `.osm.pbf` (Sweden, say), downloaded once and indexed
//! so that a world's box is read in well under a second, with no Overpass.
//!
//! The index keeps only what some pass wants (`osm::wants_*`), with every way's geometry resolved,
//! in one file, `.cache/osm/<name>.osmidx`. Elements are filed in a two-level grid by the middle of
//! their bounding box: in a fine cell (`FINE` degrees) when they fit in one, in a coarse cell
//! (`COARSE`) when they fit in that, else in one cell for the few that are huge (Vänern). A box reads
//! the cells of each level around it, so every element that reaches into it is found, and keeps the
//! elements whose lines cross it, as Overpass does.
//!
//! The file: an 8-byte magic, the cells' elements (each a run of MessagePack values), the `Table`
//! (MessagePack) and its offset as 8 little-endian bytes.

use crate::osm::{Data, LatLon, Member, Node, Relation, Way};
use anyhow::{Context, Result, bail};
use osmpbf::{BlobDecode, BlobReader, PrimitiveBlock};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

/// Where extracts and their indexes are kept. Not in the download cache, so `--clear-cache` and
/// `--refresh` leave a gigabyte download alone.
pub const DIR: &str = ".cache/osm";

const MAGIC: &[u8; 8] = b"RAGOSM01";

/// Bumped when the file's layout or what it keeps changes; an index of another version is ignored.
const VERSION: u32 = 1;

/// A fine cell, degrees of latitude and longitude: about 5.5 km square in Sweden.
const FINE: [f64; 2] = [0.05, 0.1];

/// A coarse cell, degrees of latitude and longitude.
const COARSE: [f64; 2] = [1.0, 2.0];

/// Grid levels: fine, coarse, and the one cell of huge elements.
const LEVELS: u8 = 3;

/// Coordinates are kept as OSM keeps them, in units of 1e-7 degrees.
const SCALE: f64 = 1e7;

/// One cell: its level, row and column.
type Cell = (u8, i32, i32);

/// What the end of the file says about the extract and where each cell's elements are.
#[derive(Serialize, Deserialize)]
struct Table {
    version: u32,
    /// The extract's name, as `sweden`.
    name: String,
    /// When the extract's data was current, Unix seconds (0 if the file does not say).
    timestamp: i64,
    /// Where the extract has data: rings of `[lon, lat]`, a hole where the flag is set. The extract's
    /// `.poly` if it came with one, else its header's bounding box.
    rings: Vec<(bool, Vec<[f64; 2]>)>,
    /// Each cell's offset, length in bytes and number of elements.
    cells: Vec<(Cell, u64, u64, u32)>,
}

/// An element as the index keeps it. Points are `[lat, lon, lat, lon, ...]` in 1e-7 degrees, each pair
/// after the first as its difference from the one before, so MessagePack spends few bytes on them.
#[derive(Serialize, Deserialize)]
enum Stored {
    Way(u64, Vec<(String, String)>, Vec<i32>),
    Relation(u64, Vec<(String, String)>, Vec<(u64, String, Vec<i32>)>),
    Node(u64, Vec<(String, String)>, [i32; 2]),
}

/// An indexed extract, ready to read.
pub struct Index {
    path: PathBuf,
    table: Table,
    cells: HashMap<Cell, (u64, u64)>,
}

impl Index {
    /// Opens the index at `path`, reading only its table.
    pub fn open(path: &Path) -> Result<Index> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let mut magic = [0; 8];
        file.read_exact(&mut magic)?;
        if &magic != MAGIC {
            bail!("{} is not an extract index", path.display());
        }
        let end = file.seek(SeekFrom::End(-8))?;
        let mut offset = [0; 8];
        file.read_exact(&mut offset)?;
        let offset = u64::from_le_bytes(offset);
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; usize::try_from(end - offset)?];
        file.read_exact(&mut bytes)?;
        let table: Table = rmp_serde::from_slice(&bytes).with_context(|| format!("reading the table of {}", path.display()))?;
        if table.version != VERSION {
            bail!("{} is an index of version {}, not {VERSION}; index the extract again (`--fetch-osm`)", path.display(), table.version);
        }
        let cells = table.cells.iter().map(|&(cell, offset, len, _)| (cell, (offset, len))).collect();
        Ok(Index { path: path.to_path_buf(), table, cells })
    }

    /// Every index in `DIR`, opened once per run.
    pub fn all() -> &'static [Index] {
        static ALL: OnceLock<Vec<Index>> = OnceLock::new();
        ALL.get_or_init(|| {
            let mut found = Vec::new();
            for entry in std::fs::read_dir(DIR).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "osmidx") {
                    match Index::open(&path) {
                        Ok(index) => found.push(index),
                        Err(error) => eprintln!("osm: skipping {}: {error:#}", path.display()),
                    }
                }
            }
            found
        })
    }

    /// The first index whose extract covers every one of `pieces`.
    pub fn covering(pieces: &[[f64; 4]]) -> Option<&'static Index> {
        Index::all().iter().find(|index| pieces.iter().all(|&piece| index.covers(piece)))
    }

    /// Whether the extract has data all over `[south, west, north, east]`: its four corners are inside.
    pub fn covers(&self, [south, west, north, east]: [f64; 4]) -> bool {
        [[west, south], [east, south], [west, north], [east, north]].iter().all(|&p| inside(&self.table.rings, p))
    }

    pub fn name(&self) -> &str {
        &self.table.name
    }

    /// The day the extract's data was current, as `2026-11-13`.
    pub fn date(&self) -> String {
        date(self.table.timestamp)
    }

    /// Everything in the index that reaches into any of `pieces`, sorted by id within each type.
    pub fn read(&self, pieces: &[[f64; 4]]) -> Result<Data> {
        use std::os::unix::fs::FileExt;
        let mut wanted = BTreeSet::new();
        for piece in pieces {
            wanted.extend(around(*piece));
        }
        let file = std::fs::File::open(&self.path).with_context(|| format!("opening {}", self.path.display()))?;
        let boxes: Vec<[i32; 4]> = pieces.iter().map(|&[s, w, n, e]| [fixed(s), fixed(w), fixed(n), fixed(e)]).collect();
        let parts: Vec<Data> = wanted
            .into_par_iter()
            .filter_map(|cell| self.cells.get(&cell).copied())
            .map(|(offset, len)| -> Result<Data> {
                let mut bytes = vec![0; usize::try_from(len)?];
                file.read_exact_at(&mut bytes, offset)?;
                let mut data = Data::default();
                let mut reader = std::io::Cursor::new(&bytes[..]);
                while (reader.position() as usize) < bytes.len() {
                    let stored: Stored = rmp_serde::from_read(&mut reader).context("reading an extract cell; index the extract again")?;
                    take(stored, &boxes, &mut data);
                }
                Ok(data)
            })
            .collect::<Result<_>>()?;
        let mut data = Data::default();
        for part in parts {
            data.ways.extend(part.ways);
            data.relations.extend(part.relations);
            data.nodes.extend(part.nodes);
        }
        data.sort();
        Ok(data)
    }
}

/// `degrees` in 1e-7 degrees.
fn fixed(degrees: f64) -> i32 {
    (degrees * SCALE).round() as i32
}

fn degrees(fixed: i32) -> f64 {
    f64::from(fixed) / SCALE
}

/// Undoes `delta`, as `LatLon`s.
fn points(packed: &[i32]) -> Vec<LatLon> {
    let (mut lat, mut lon) = (0i32, 0i32);
    packed
        .chunks_exact(2)
        .map(|pair| {
            lat = lat.wrapping_add(pair[0]);
            lon = lon.wrapping_add(pair[1]);
            LatLon { lat: degrees(lat), lon: degrees(lon) }
        })
        .collect()
}

/// `[lat, lon]` pairs as `[lat, lon, dlat, dlon, ...]`.
fn delta(pairs: &[[i32; 2]]) -> Vec<i32> {
    let mut previous = [0i32, 0i32];
    let mut packed = Vec::with_capacity(pairs.len() * 2);
    for &[lat, lon] in pairs {
        packed.push(lat.wrapping_sub(previous[0]));
        packed.push(lon.wrapping_sub(previous[1]));
        previous = [lat, lon];
    }
    packed
}

/// Whether the line through `pairs` (`[lat, lon]`, 1e-7 degrees) touches `[south, west, north, east]`:
/// a point inside it, or a segment through it.
fn crosses(pairs: &[[i32; 2]], &[south, west, north, east]: &[i32; 4]) -> bool {
    let inside = |[lat, lon]: [i32; 2]| (south..=north).contains(&lat) && (west..=east).contains(&lon);
    if pairs.iter().any(|&p| inside(p)) {
        return true;
    }
    // Liang-Barsky: clip each segment to the box.
    let (s, w, n, e) = (f64::from(south), f64::from(west), f64::from(north), f64::from(east));
    pairs.windows(2).any(|pair| {
        let ([y0, x0], [y1, x1]) = (pair[0].map(f64::from), pair[1].map(f64::from));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, q) in [(-dx, x0 - w), (dx, e - x0), (-dy, y0 - s), (dy, n - y0)] {
            if p == 0.0 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let t = q / p;
                if p < 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) }
                if t0 > t1 {
                    return false;
                }
            }
        }
        true
    })
}

fn unpack_pairs(packed: &[i32]) -> Vec<[i32; 2]> {
    let (mut lat, mut lon) = (0i32, 0i32);
    packed
        .chunks_exact(2)
        .map(|pair| {
            lat = lat.wrapping_add(pair[0]);
            lon = lon.wrapping_add(pair[1]);
            [lat, lon]
        })
        .collect()
}

/// Adds `stored` to `data` if it reaches into any of `boxes`.
fn take(stored: Stored, boxes: &[[i32; 4]], data: &mut Data) {
    let tags = |pairs: Vec<(String, String)>| pairs.into_iter().collect();
    match stored {
        Stored::Way(id, pairs, packed) => {
            let line = unpack_pairs(&packed);
            if boxes.iter().any(|b| crosses(&line, b)) {
                data.ways.push(Way { id, tags: tags(pairs), geometry: points(&packed) });
            }
        }
        Stored::Relation(id, pairs, members) => {
            if members.iter().any(|(_, _, packed)| {
                let line = unpack_pairs(packed);
                boxes.iter().any(|b| crosses(&line, b))
            }) {
                let members = members.into_iter().map(|(way, role, packed)| Member { way, role, geometry: points(&packed) }).collect();
                data.relations.push(Relation { id, tags: tags(pairs), members });
            }
        }
        Stored::Node(id, pairs, [lat, lon]) => {
            if boxes.iter().any(|b| crosses(&[[lat, lon]], b)) {
                data.nodes.push(Node { id, tags: tags(pairs), at: LatLon { lat: degrees(lat), lon: degrees(lon) } });
            }
        }
    }
}

/// The cells, of every level, where an element reaching into `[south, west, north, east]` is filed.
fn around([south, west, north, east]: [f64; 4]) -> Vec<Cell> {
    let mut cells = vec![(2, 0, 0)];
    for (level, [dlat, dlon]) in [(0u8, FINE), (1, COARSE)] {
        let row = |lat: f64| (lat / dlat).floor() as i32;
        let col = |lon: f64| (lon / dlon).floor() as i32;
        for r in row(south) - 1..=row(north) + 1 {
            for c in col(west) - 1..=col(east) + 1 {
                cells.push((level, r, c));
            }
        }
    }
    cells
}

/// The cell an element with this bounding box (`[south, west, north, east]`, 1e-7 degrees) is filed in.
fn cell_of([south, west, north, east]: [i32; 4]) -> Cell {
    let (lat, lon) = (degrees(south) / 2.0 + degrees(north) / 2.0, degrees(west) / 2.0 + degrees(east) / 2.0);
    let (height, width) = (degrees(north) - degrees(south), degrees(east) - degrees(west));
    for (level, [dlat, dlon]) in [(0u8, FINE), (1, COARSE)] {
        if height <= dlat && width <= dlon {
            return (level, (lat / dlat).floor() as i32, (lon / dlon).floor() as i32);
        }
    }
    (LEVELS - 1, 0, 0)
}

fn bounds<'a>(lines: impl IntoIterator<Item = &'a [[i32; 2]]>) -> Option<[i32; 4]> {
    let mut found: Option<[i32; 4]> = None;
    for &[lat, lon] in lines.into_iter().flatten() {
        let b = found.get_or_insert([lat, lon, lat, lon]);
        *b = [b[0].min(lat), b[1].min(lon), b[2].max(lat), b[3].max(lon)];
    }
    found
}

/// Whether `[lon, lat]` is in the area the rings make (inside an odd number of rings, holes included).
fn inside(rings: &[(bool, Vec<[f64; 2]>)], [x, y]: [f64; 2]) -> bool {
    let mut count = 0;
    for (_, ring) in rings {
        let mut hit = false;
        let mut previous = ring[ring.len() - 1];
        for &current in ring {
            if (current[1] > y) != (previous[1] > y) && x < previous[0] + (y - previous[1]) * (current[0] - previous[0]) / (current[1] - previous[1]) {
                hit = !hit;
            }
            previous = current;
        }
        count += usize::from(hit);
    }
    count % 2 == 1
}

/// The rings of an Osmosis `.poly` file: `[lon, lat]`, holes flagged.
fn read_poly(text: &str) -> Result<Vec<(bool, Vec<[f64; 2]>)>> {
    let mut rings = Vec::new();
    let mut lines = text.lines().map(str::trim).skip(1);
    while let Some(name) = lines.next() {
        if name.is_empty() || name == "END" {
            continue;
        }
        let mut ring = Vec::new();
        for line in lines.by_ref() {
            if line == "END" {
                break;
            }
            let values: Vec<f64> = line.split_whitespace().map(str::parse).collect::<Result<_, _>>().with_context(|| format!("a .poly line: {line}"))?;
            let [lon, lat] = values[..] else { bail!("a .poly line: {line}") };
            ring.push([lon, lat]);
        }
        if ring.len() >= 3 {
            rings.push((name.starts_with('!'), ring));
        }
    }
    Ok(rings)
}

/// `YYYY-MM-DD` of Unix seconds (Howard Hinnant's days-to-civil).
fn date(seconds: i64) -> String {
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The Geofabrik path of a region: `sweden` is `europe/sweden`, `europe/france/bretagne` is itself.
fn geofabrik(name: &str) -> String {
    if name.contains('/') { name.to_string() } else { format!("europe/{name}") }
}

/// Counts bytes into `progress` as a download is written, and prints how far it has come.
struct Counting<W> {
    inner: W,
    written: u64,
    total: Option<u64>,
    shown: u64,
}

impl<W: std::io::Write> std::io::Write for Counting<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(bytes)?;
        self.written += n as u64;
        crate::progress::received(n);
        if self.written - self.shown >= 50 << 20 {
            self.shown = self.written;
            match self.total {
                Some(total) => eprintln!("  {} of {} MiB", self.written >> 20, total >> 20),
                None => eprintln!("  {} MiB", self.written >> 20),
            }
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Downloads `url` to `path`, through a `.part` file so an interrupted download is never taken whole.
fn download(url: &str, path: &Path) -> Result<()> {
    eprintln!("downloading {url}");
    let client = reqwest::blocking::Client::builder().user_agent(concat!("ragnarok-osm-import/", env!("CARGO_PKG_VERSION"))).timeout(None).build()?;
    let mut response = client.get(url).send().with_context(|| format!("requesting {url}"))?.error_for_status().with_context(|| format!("downloading {url}"))?;
    let partial = path.with_extension("part");
    let file = std::fs::File::create(&partial).with_context(|| format!("creating {}", partial.display()))?;
    let mut writer = Counting { inner: std::io::BufWriter::new(file), written: 0, total: response.content_length(), shown: 0 };
    response.copy_to(&mut writer).with_context(|| format!("downloading {url}"))?;
    std::io::Write::flush(&mut writer)?;
    std::fs::rename(&partial, path)?;
    Ok(())
}

/// Makes the index of an extract: `what` is a Geofabrik region (`sweden`), downloaded to `DIR` unless
/// it is there already (or `refresh`), or the path of an `.osm.pbf` (with its `.poly` beside it, if
/// any). Returns the index's path.
pub fn fetch(what: &str, refresh: bool) -> Result<PathBuf> {
    std::fs::create_dir_all(DIR)?;
    let local = Path::new(what);
    let (pbf, poly, name) = if what.ends_with(".pbf") {
        let name = local.file_name().and_then(|n| n.to_str()).unwrap_or("extract").trim_end_matches(".osm.pbf").trim_end_matches("-latest").to_string();
        let poly = local.with_file_name(format!("{name}.poly"));
        (local.to_path_buf(), poly, name)
    } else {
        let region = geofabrik(what);
        let name = region.rsplit('/').next().unwrap_or(what).to_string();
        let pbf = Path::new(DIR).join(format!("{name}-latest.osm.pbf"));
        let poly = Path::new(DIR).join(format!("{name}.poly"));
        if refresh || !pbf.exists() {
            download(&format!("https://download.geofabrik.de/{region}-latest.osm.pbf"), &pbf)?;
        }
        if refresh || !poly.exists() {
            download(&format!("https://download.geofabrik.de/{region}.poly"), &poly)?;
        }
        (pbf, poly, name)
    };
    let out = Path::new(DIR).join(format!("{name}.osmidx"));
    build(&pbf, poly.exists().then_some(poly.as_path()), &name, &out)?;
    Ok(out)
}

/// A relation as the first pass reads it: its tags and its member ways' ids and roles.
struct RawRelation {
    id: i64,
    tags: Vec<(String, String)>,
    members: Vec<(i64, String)>,
}

/// A way as the second pass reads it. `tags` is `None` for a way kept only as a relation's member.
struct RawWay {
    id: i64,
    tags: Option<Vec<(String, String)>>,
    refs: Vec<i64>,
}

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Runs `each` on every data block of the extract, in parallel, and gathers what it returns.
fn blocks<T: Send>(path: &Path, each: impl Fn(&PrimitiveBlock, &mut Vec<T>) + Sync + Send) -> Result<Vec<T>> {
    let found: Vec<Vec<T>> = BlobReader::from_path(path)?
        .par_bridge()
        .map(|blob| -> Result<Vec<T>> {
            let mut found = Vec::new();
            if let BlobDecode::OsmData(block) = blob?.decode()? {
                each(&block, &mut found);
            }
            Ok(found)
        })
        .collect::<Result<_>>()?;
    Ok(found.into_iter().flatten().collect())
}

const MISSING: u64 = u64::MAX;

fn pack(lat: i32, lon: i32) -> u64 {
    (u64::from(lat as u32) << 32) | u64::from(lon as u32)
}

fn unpack(packed: u64) -> [i32; 2] {
    [(packed >> 32) as u32 as i32, packed as u32 as i32]
}

/// Indexes the extract at `pbf` into `out`: three passes over it (relations, ways, nodes), then each
/// element's geometry resolved and filed in its cell.
pub fn build(pbf: &Path, poly: Option<&Path>, name: &str, out: &Path) -> Result<()> {
    let started = std::time::Instant::now();
    eprintln!("indexing {}", pbf.display());
    let (timestamp, header_box) = {
        let mut reader = BlobReader::from_path(pbf)?;
        let blob = reader.next().transpose()?;
        match blob.as_ref().map(|blob| blob.decode()).transpose()? {
            Some(BlobDecode::OsmHeader(header)) => (header.osmosis_replication_timestamp().unwrap_or(0), header.bbox()),
            _ => (0, None),
        }
    };
    let rings = match (poly, header_box) {
        (Some(poly), _) => read_poly(&std::fs::read_to_string(poly).with_context(|| format!("reading {}", poly.display()))?)?,
        (None, Some(b)) => vec![(false, vec![[b.left, b.bottom], [b.right, b.bottom], [b.right, b.top], [b.left, b.top]])],
        (None, None) => bail!("{} has no bounding box and no .poly beside it", pbf.display()),
    };

    crate::progress::stage("index: relations");
    let relations: Vec<RawRelation> = blocks(pbf, |block, found| {
        let mut tags = Vec::new();
        for group in block.groups() {
            for relation in group.relations() {
                tags.clear();
                tags.extend(relation.tags());
                if crate::osm::wants_relation(&tags[..]) {
                    let members = relation.members().filter(|m| m.member_type == osmpbf::RelMemberType::Way).map(|m| (m.member_id, m.role().unwrap_or("").to_string())).collect();
                    found.push(RawRelation { id: relation.id(), tags: owned(&tags), members });
                }
            }
        }
    })?;
    let mut members: Vec<i64> = relations.iter().flat_map(|r| r.members.iter().map(|m| m.0)).collect();
    members.sort_unstable();
    members.dedup();
    eprintln!("  {} relations with {} member ways ({:.0} s)", relations.len(), members.len(), started.elapsed().as_secs_f64());

    crate::progress::stage("index: ways");
    let mut ways: Vec<RawWay> = blocks(pbf, |block, found| {
        let mut tags = Vec::new();
        for group in block.groups() {
            for way in group.ways() {
                tags.clear();
                tags.extend(way.tags());
                let wanted = crate::osm::wants_way(&tags[..]);
                if wanted || members.binary_search(&way.id()).is_ok() {
                    found.push(RawWay { id: way.id(), tags: wanted.then(|| owned(&tags)), refs: way.refs().collect() });
                }
            }
        }
    })?;
    ways.sort_unstable_by_key(|way| way.id);
    let mut ids: Vec<i64> = ways.iter().flat_map(|way| way.refs.iter().copied()).collect();
    ids.par_sort_unstable();
    ids.dedup();
    eprintln!("  {} ways with {} nodes ({:.0} s)", ways.len(), ids.len(), started.elapsed().as_secs_f64());

    crate::progress::stage("index: nodes");
    let coords: Vec<AtomicU64> = (0..ids.len()).map(|_| AtomicU64::new(MISSING)).collect();
    let nodes: Vec<Stored> = blocks(pbf, |block, found| {
        let mut tags = Vec::new();
        let mut take = |id: i64, lat: i32, lon: i32, tags: &[(&str, &str)]| {
            if let Ok(index) = ids.binary_search(&id) {
                coords[index].store(pack(lat, lon), Ordering::Relaxed);
            }
            if !tags.is_empty() && crate::osm::wants_node(tags) {
                found.push(Stored::Node(id as u64, owned(tags), [lat, lon]));
            }
        };
        for group in block.groups() {
            for node in group.dense_nodes() {
                tags.clear();
                tags.extend(node.tags());
                take(node.id(), node.decimicro_lat(), node.decimicro_lon(), &tags);
            }
            for node in group.nodes() {
                tags.clear();
                tags.extend(node.tags());
                take(node.id(), node.decimicro_lat(), node.decimicro_lon(), &tags);
            }
        }
    })?;
    eprintln!("  {} tagged nodes ({:.0} s)", nodes.len(), started.elapsed().as_secs_f64());

    crate::progress::stage("index: filing");
    // A node the extract cut off is left out of its way.
    let line = |refs: &[i64]| -> Vec<[i32; 2]> {
        refs.iter().filter_map(|id| ids.binary_search(id).ok()).map(|index| coords[index].load(Ordering::Relaxed)).filter(|&p| p != MISSING).map(unpack).collect()
    };
    let encode = |stored: &Stored| rmp_serde::to_vec(stored).expect("an element encodes");
    let mut filed: Vec<(Cell, Vec<u8>)> = ways
        .par_iter()
        .filter(|way| way.tags.is_some())
        .filter_map(|way| {
            let pairs = line(&way.refs);
            let bounds = bounds([&pairs[..]])?;
            (pairs.len() >= 2).then(|| (cell_of(bounds), encode(&Stored::Way(way.id as u64, way.tags.clone().unwrap_or_default(), delta(&pairs)))))
        })
        .collect();
    filed.par_extend(relations.par_iter().filter_map(|relation| {
        let members: Vec<(u64, String, Vec<[i32; 2]>)> = relation
            .members
            .iter()
            .filter_map(|(id, role)| ways.binary_search_by_key(id, |way| way.id).ok().map(|index| (*id as u64, role.clone(), line(&ways[index].refs))))
            .filter(|(_, _, pairs)| pairs.len() >= 2)
            .collect();
        let bounds = bounds(members.iter().map(|(_, _, pairs)| &pairs[..]))?;
        let members = members.into_iter().map(|(id, role, pairs)| (id, role, delta(&pairs))).collect();
        Some((cell_of(bounds), encode(&Stored::Relation(relation.id as u64, relation.tags.clone(), members))))
    }));
    filed.par_extend(nodes.par_iter().map(|node| {
        let Stored::Node(_, _, [lat, lon]) = node else { unreachable!() };
        (cell_of([*lat, *lon, *lat, *lon]), encode(node))
    }));
    drop((ways, relations, nodes, coords, ids));
    filed.par_sort_unstable_by_key(|(cell, _)| *cell);

    crate::progress::stage("index: writing");
    use std::io::Write;
    let partial = out.with_extension("part");
    let mut file = std::io::BufWriter::new(std::fs::File::create(&partial).with_context(|| format!("creating {}", partial.display()))?);
    file.write_all(MAGIC)?;
    let mut offset = MAGIC.len() as u64;
    let mut cells: Vec<(Cell, u64, u64, u32)> = Vec::new();
    for (cell, bytes) in &filed {
        match cells.last_mut() {
            Some(last) if last.0 == *cell => {
                last.2 += bytes.len() as u64;
                last.3 += 1;
            }
            _ => cells.push((*cell, offset, bytes.len() as u64, 1)),
        }
        file.write_all(bytes)?;
        offset += bytes.len() as u64;
    }
    let huge = cells.iter().find(|c| c.0.0 == LEVELS - 1).map_or((0, 0), |c| (c.2, c.3));
    let table = Table { version: VERSION, name: name.to_string(), timestamp, rings, cells };
    file.write_all(&rmp_serde::to_vec(&table)?)?;
    file.write_all(&offset.to_le_bytes())?;
    file.into_inner().map_err(|error| error.into_error())?.sync_all()?;
    std::fs::rename(&partial, out)?;
    eprintln!(
        "indexed {} elements of {name} ({}) into {} cells, {} MiB, in {:.0} s; {} huge elements, {} MiB, read with every world",
        filed.len(),
        date(timestamp),
        table.cells.len(),
        offset >> 20,
        started.elapsed().as_secs_f64(),
        huge.1,
        huge.0 >> 20,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_crossing_a_box_with_no_point_in_it_is_found() {
        let b = [0, 0, 10, 10];
        assert!(crosses(&[[-5, 5], [15, 5]], &b), "straight through");
        assert!(crosses(&[[5, 5]], &b), "a point inside");
        assert!(!crosses(&[[-5, -5], [-5, 15]], &b), "passes beside it");
        assert!(!crosses(&[[30, 0], [0, 30]], &b), "cuts the corner far outside");
    }

    #[test]
    fn deltas_come_back_as_the_points() {
        let pairs = [[638_225_123, 202_350_000], [638_225_200, 202_349_000], [-10, 5]];
        assert_eq!(unpack_pairs(&delta(&pairs)), pairs);
        assert_eq!(points(&delta(&pairs[..1])), [LatLon { lat: 63.8225123, lon: 20.235 }]);
    }

    #[test]
    fn an_element_is_found_from_every_box_it_reaches_into() {
        // A way about one fine cell wide, filed by its middle, is still in the cells a box at its end reads.
        let way = [fixed(63.81), fixed(20.17), fixed(63.85), fixed(20.26)];
        let filed = cell_of(way);
        assert_eq!(filed.0, 0);
        for piece in [[63.80, 20.16, 63.811, 20.171], [63.849, 20.259, 63.86, 20.27]] {
            assert!(around(piece).contains(&filed), "{piece:?}");
        }
        // A lake as big as Vänern is huge, read from everywhere.
        assert_eq!(cell_of([fixed(58.4), fixed(12.4), fixed(59.4), fixed(14.2)]).0, 1);
        assert_eq!(cell_of([fixed(58.4), fixed(12.4), fixed(60.6), fixed(14.2)]).0, 2);
    }

    #[test]
    fn a_poly_has_its_holes() {
        let rings = read_poly("none\n1\n 0 0\n 10 0\n 10 10\n 0 10\nEND\n!2\n 4 4\n 6 4\n 6 6\n 4 6\nEND\nEND\n").unwrap();
        assert_eq!(rings.len(), 2);
        assert!(inside(&rings, [1.0, 1.0]));
        assert!(!inside(&rings, [5.0, 5.0]));
        assert!(!inside(&rings, [11.0, 5.0]));
    }

    #[test]
    fn a_timestamp_is_a_day() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(1_794_528_000), "2026-11-13");
    }
}
