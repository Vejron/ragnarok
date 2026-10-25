//! Laser point clouds in COPC, read over HTTP range requests: only the octree nodes that touch the
//! window are downloaded. `fetch` finds Lantmäteriet's ("Laserdata Skog") through its STAC API;
//! `read_files` reads any list of files, such as IGN's LiDAR HD (`ign.rs`).
//!
//! A small COPC reader of our own: the public crates mis-walk this file's hierarchy pages. COPC is
//! a LAZ 1.4 file whose points are stored as one independently compressed chunk per octree node;
//! the `copc` EVLR lists the nodes (offset, size, point count) in pages that point at each other.

use crate::dem::{RangeReader, Window};
use anyhow::{Context, Result, bail, ensure};
use laz::laszip::LazVlr;
use laz::record::{LayeredPointRecordDecompressor, RecordDecompressor};
use std::io::{Cursor, Read, Seek, SeekFrom};

const STAC_ITEMS: &str = "https://api.lantmateriet.se/stac-hojd/v1/collections/dsm-skoglig-copc/items";

/// One laser return, SWEREF 99 TM metres and RH 2000 height.
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub e: f64,
    pub n: f64,
    pub z: f64,
    /// ASPRS class. Lantmäteriet's product only separates 2 ground (and a few 7 low noise, 18 high
    /// noise) from 1 unclassified, which is everything else: buildings, vegetation, cars. IGN's
    /// LiDAR HD has them all (`ign::class`).
    pub class: u8,
    /// 1-based return number and the number of returns of the pulse.
    pub ret: u8,
    pub returns: u8,
    pub intensity: u16,
}

/// Every return inside `window`, from the newest scan of each file that covers `bbox`
/// (`[south, west, north, east]`). With `only`, octree nodes that touch none of those rectangles
/// are not read, so a sparse world does not pay for the empty parts of its bounding square.
pub fn fetch(bbox: [f64; 4], window: Window, only: Option<&[Window]>, user: &str, password: &str, keep: impl FnMut(Point)) -> Result<usize> {
    let [south, west, north, east] = bbox;
    let client = reqwest::blocking::Client::builder().build()?;
    let url = format!("{STAC_ITEMS}?bbox={west},{south},{east},{north}&limit=50");
    eprintln!("querying {url}");
    let body = crate::cache::text("stac", &url, || {
        Ok(client.get(&url).basic_auth(user, Some(password)).send()?.error_for_status().context("STAC search")?.text()?)
    })?;
    let items: serde_json::Value = serde_json::from_str(&body).context("STAC search returned invalid JSON")?;
    // One file per square: the same square can be flown more than once; take the newest.
    let mut newest: std::collections::BTreeMap<String, (String, String, String)> = Default::default();
    for item in items["features"].as_array().map(Vec::as_slice).unwrap_or_default() {
        let (Some(id), Some(href)) = (item["id"].as_str(), item["assets"]["data"]["href"].as_str()) else { continue };
        let square = item["assets"]["data"]["proj:bbox"].to_string();
        let when = item["properties"]["datetime"].as_str().unwrap_or_default().to_string();
        if newest.get(&square).is_none_or(|(known, ..)| *known < when) {
            newest.insert(square, (when, id.to_string(), href.to_string()));
        }
    }
    let files: Vec<(String, String)> = newest.into_values().map(|(_, id, href)| (id, href)).collect();
    let total = read_files(&client, &files, window, only, (user, password), keep)?;
    if total == 0 {
        bail!("no laser data covers {bbox:?}");
    }
    Ok(total)
}

/// Every return inside `window` of the COPC files `(name, url)`, read with range requests (no
/// credentials when the user is empty). With `only`, nodes that touch none of those rectangles are
/// not read.
pub fn read_files(client: &reqwest::blocking::Client, files: &[(String, String)], window: Window, only: Option<&[Window]>, (user, password): (&str, &str), mut keep: impl FnMut(Point)) -> Result<usize> {
    let mut total = 0;
    for (index, (id, href)) in files.iter().enumerate() {
        crate::progress::stage(&format!("laser, file {} of {}", index + 1, files.len()));
        eprintln!("reading {id}: {href}");
        let open = || RangeReader::open(client.clone(), href, user, password);
        let count = read(open, window, only, &mut keep).with_context(|| format!("reading {href}"))?;
        eprintln!("  {count} points");
        total += count;
    }
    Ok(total)
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}
fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn le_i32(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn le_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}
fn le_f64(b: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

fn bytes_at<R: Read + Seek>(source: &mut R, offset: u64, length: usize) -> Result<Vec<u8>> {
    source.seek(SeekFrom::Start(offset))?;
    let mut out = vec![0u8; length];
    source.read_exact(&mut out)?;
    Ok(out)
}

/// A node of the octree as the hierarchy lists it. `count == -1` means `offset` is another page.
#[derive(Clone, Copy, Debug)]
struct Entry {
    level: i32,
    x: i32,
    y: i32,
    offset: u64,
    size: i32,
    count: i32,
}

fn parse_page(bytes: &[u8]) -> Vec<Entry> {
    bytes
        .chunks_exact(32)
        .map(|b| Entry { level: le_i32(b, 0), x: le_i32(b, 4), y: le_i32(b, 8), offset: le_u64(b, 16), size: le_i32(b, 24), count: le_i32(b, 28) })
        .collect()
}

/// Decoder threads, and how many nodes are decoded before their points are handed on, which bounds
/// the memory held in between.
const WORKERS: usize = 8;
const BATCH: usize = 64;

/// Reads the file `open` gives, one reader for the header and the hierarchy and one per worker for the
/// nodes: each node is an independently compressed chunk, so they decode in parallel. Points reach
/// `keep` in node order, so the result does not depend on the threads' timing.
fn read<R: Read + Seek + Send>(open: impl Fn() -> Result<R> + Sync, [e0, n0, e1, n1]: Window, only: Option<&[Window]>, keep: &mut impl FnMut(Point)) -> Result<usize> {
    let mut source = open()?;
    // LAS 1.4 header (375 bytes), then the VLRs.
    let header = bytes_at(&mut source, 0, 375)?;
    ensure!(&header[0..4] == b"LASF", "not a LAS file");
    let header_size = le_u16(&header, 94) as u64;
    let vlr_count = le_u32(&header, 100);
    let format = header[104] & 0x3f;
    let record_length = le_u16(&header, 105) as usize;
    ensure!((6..=10).contains(&format), "point format {format} is not a LAS 1.4 layered format");
    let scale = [le_f64(&header, 131), le_f64(&header, 139), le_f64(&header, 147)];
    let offset = [le_f64(&header, 155), le_f64(&header, 163), le_f64(&header, 171)];

    let mut position = header_size;
    let (mut laz, mut copc) = (None, None);
    for _ in 0..vlr_count {
        let head = bytes_at(&mut source, position, 54)?;
        let user = String::from_utf8_lossy(&head[2..18]).trim_end_matches('\0').to_string();
        let (id, length) = (le_u16(&head, 18), le_u16(&head, 20) as usize);
        let data = bytes_at(&mut source, position + 54, length)?;
        position += 54 + length as u64;
        match (user.as_str(), id) {
            ("laszip encoded", 22204) => laz = Some(LazVlr::from_buffer(&data).context("laszip VLR")?),
            ("copc", 1) => copc = Some(data),
            _ => {}
        }
    }
    let laz = laz.context("no laszip VLR: file is not LAZ")?;
    let info = copc.context("no COPC info VLR: file is not COPC")?;
    let (center, halfsize) = ([le_f64(&info, 0), le_f64(&info, 8)], le_f64(&info, 24));
    let (root_offset, root_size) = (le_u64(&info, 40), le_u64(&info, 48));

    // Does the node `(level, x, y)` touch the window? Octree cubes, so z is not tested.
    let touches = |e: &Entry| {
        let size = 2.0 * halfsize / f64::from(1u32 << e.level.clamp(0, 30));
        let (min_e, min_n) = (center[0] - halfsize + f64::from(e.x) * size, center[1] - halfsize + f64::from(e.y) * size);
        let meets = |[a0, b0, a1, b1]: Window| min_e <= a1 && min_e + size >= a0 && min_n <= b1 && min_n + size >= b0;
        meets([e0, n0, e1, n1]) && only.is_none_or(|rects| rects.iter().any(|&rect| meets(rect)))
    };

    // Walk the hierarchy pages, skipping the subtrees that miss the window.
    let mut pages = vec![(root_offset, root_size as usize)];
    let mut nodes = Vec::new();
    while let Some((at, size)) = pages.pop() {
        for entry in parse_page(&bytes_at(&mut source, at, size)?) {
            if !touches(&entry) {
                continue;
            }
            match entry.count {
                -1 => pages.push((entry.offset, entry.size as usize)),
                0 => {}
                _ => nodes.push(entry),
            }
        }
    }
    nodes.sort_by_key(|n| n.offset);
    eprintln!("  {} nodes touch the window", nodes.len());

    let node_count = nodes.len();
    let mut count = 0;
    let mut done = 0;
    let mut sources: Vec<Option<R>> = (0..WORKERS).map(|_| None).collect();
    for batch in nodes.chunks(BATCH) {
        let next = std::sync::atomic::AtomicUsize::new(0);
        let slots: Vec<std::sync::Mutex<Option<Result<Vec<Point>>>>> = batch.iter().map(|_| Default::default()).collect();
        std::thread::scope(|scope| {
            for slot in sources.iter_mut() {
                scope.spawn(|| {
                    loop {
                        let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(node) = batch.get(at) else { break };
                        let result = (|| {
                            if slot.is_none() {
                                *slot = Some(open()?);
                            }
                            let source = slot.as_mut().expect("opened above");
                            decode(source, node, &laz, record_length, scale, offset, [e0, n0, e1, n1])
                        })();
                        *slots[at].lock().unwrap() = Some(result);
                    }
                });
            }
        });
        for slot in slots {
            let points = slot.into_inner().unwrap().expect("every node of the batch was taken")?;
            crate::progress::steps(done, node_count);
            done += 1;
            count += points.len();
            points.into_iter().for_each(&mut *keep);
        }
    }
    Ok(count)
}

/// The returns of one node that lie in the window and can be something other than ground or noise
/// (classes 2, 7 and 18, which neither the roofs nor the canopy use).
fn decode<R: Read + Seek>(source: &mut R, node: &Entry, laz: &LazVlr, record_length: usize, scale: [f64; 3], offset: [f64; 3], [e0, n0, e1, n1]: Window) -> Result<Vec<Point>> {
    let chunk = bytes_at(source, node.offset, node.size as usize)?;
    let mut decompressor = LayeredPointRecordDecompressor::new(Cursor::new(chunk));
    decompressor.set_fields_from(laz.items()).context("laz fields")?;
    let mut record = vec![0u8; record_length];
    let mut points = Vec::new();
    for _ in 0..node.count {
        decompressor.decompress_next(&mut record).context("decompressing a node")?;
        // Format 6+: the class is at byte 16.
        if matches!(record[16], 2 | 7 | 18) {
            continue;
        }
        let e = f64::from(le_i32(&record, 0)) * scale[0] + offset[0];
        let n = f64::from(le_i32(&record, 4)) * scale[1] + offset[1];
        if e < e0 || e > e1 || n < n0 || n > n1 {
            continue;
        }
        let z = f64::from(le_i32(&record, 8)) * scale[2] + offset[2];
        // Byte 14 holds the return number (low nibble) and the number of returns (high).
        points.push(Point { e, n, z, class: record[16], ret: record[14] & 0x0f, returns: record[14] >> 4, intensity: le_u16(&record, 12) });
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exploration: `LM_USER=… LM_PASS=… cargo test -p osm-import lidar_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn lidar_probe() {
        let (user, password) = (std::env::var("LM_USER").unwrap(), std::env::var("LM_PASS").unwrap());
        let (e, n) = crate::dem::sweref99tm(63.832127, 20.168286);
        let window = [e - 150.0, n - 150.0, e + 150.0, n + 150.0];
        let started = std::time::Instant::now();
        let mut points = Vec::new();
        fetch([63.83, 20.16, 63.834, 20.176], window, None, &user, &password, |p| points.push(p)).unwrap();
        eprintln!("{} points in {:?}", points.len(), started.elapsed());
        let mut classes = std::collections::BTreeMap::new();
        for p in &points {
            *classes.entry(p.class).or_insert(0usize) += 1;
        }
        eprintln!("classes {classes:?}");
        let (zmin, zmax) = points.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
        eprintln!("z {zmin:.1}..{zmax:.1}");
    }
}
