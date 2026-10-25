//! IGN's LiDAR HD over France, found and fetched from the Géoplateforme (`data.geopf.fr`). It is
//! open data (Etalab 2.0, credit "IGN – LiDAR HD"): no account and no credentials.
//!
//! LiDAR HD is cut in 1 km squares of Lambert-93 (EPSG:2154, heights IGN69). The WFS layer
//! `IGNF_LIDAR-HD_METADONNEE:metadata` has one feature per square and edition. A square's
//! `coordonnees_nw` is its north-west corner in kilometres, `url_npl` its classified point cloud
//! (COPC, LAS 1.4 point format 6, about 17 returns a square metre), and `url_mnt` its 0.5 m terrain
//! model (MNT) as a WMS-R `GetMap`. The aerial photo comes from the same WMS-R (`photo`). The importer reads the cloud with range requests
//! (`lidar::read_files`) and asks the WMS-R only for the windows it needs (`mnt`).

use crate::dem::{Dem, Window};
use anyhow::{Context, Result, bail};

const WFS: &str = "https://data.geopf.fr/wfs/ows";
const LAYER: &str = "IGNF_LIDAR-HD_METADONNEE:metadata";
const WMS: &str = "https://data.geopf.fr/wms-r";
const MNT_LAYER: &str = "IGNF_LIDAR-HD_MNT_ELEVATION.ELEVATIONGRIDCOVERAGE.LAMB93";
/// IGN's aerial photo, 20 cm (BD ORTHO), for roof colours.
const PHOTO_LAYER: &str = "HR.ORTHOIMAGERY.ORTHOPHOTOS";
/// The MNT's pixel, metres. Pixel centres lie on multiples of it.
pub const MNT_PIXEL: f64 = 0.5;
/// Features asked for in one WFS page.
const PAGE: usize = 1000;
/// The largest MNT window asked for in one request, metres a side: about 2.5 MB of float32 (and
/// about 1.7 MB of aerial photo).
const MNT_CHUNK: f64 = 400.0;

/// ASPRS classes as IGN uses them in LiDAR HD.
pub mod class {
    pub const UNCLASSIFIED: u8 = 1;
    pub const GROUND: u8 = 2;
    /// Vegetation under 0.5 m, 0.5 to 1.5 m, and over 1.5 m.
    pub const LOW_VEGETATION: u8 = 3;
    pub const MEDIUM_VEGETATION: u8 = 4;
    pub const HIGH_VEGETATION: u8 = 5;
    pub const BUILDING: u8 = 6;
    pub const WATER: u8 = 9;
    pub const BRIDGE: u8 = 17;
    /// Permanent structures above ground that are not buildings: walls, pylons, chimneys.
    pub const PERMANENT_ABOVE_GROUND: u8 = 64;
    pub const ARTEFACT: u8 = 65;
    /// Points IGN added under water and bridges to model the terrain there.
    pub const VIRTUAL: u8 = 66;
    pub const OTHER_BUILT: u8 = 67;
}

/// One 1 km square of LiDAR HD.
#[derive(Clone, Debug, PartialEq)]
pub struct Square {
    /// `"0525_6701"`: the north-west corner in kilometres of Lambert-93.
    pub name: String,
    /// `[e0, n0, e1, n1]` metres.
    pub window: Window,
    /// The classified point cloud (COPC).
    pub cloud: String,
    /// When IGN published this edition, `YYYY-MM-DD`.
    pub edition: String,
}

fn overlaps([a0, b0, a1, b1]: Window, [c0, d0, c1, d1]: Window) -> bool {
    a0 < c1 && c0 < a1 && b0 < d1 && d0 < b1
}

/// The LiDAR HD squares that overlap `window` (Lambert-93 metres), newest edition of each, in name
/// order. The index answers are cached like any download.
pub fn squares(window: Window) -> Result<Vec<Square>> {
    let [e0, n0, e1, n1] = window;
    let client = reqwest::blocking::Client::builder().build()?;
    let mut newest: std::collections::BTreeMap<String, Square> = Default::default();
    for page in 0.. {
        let url = format!(
            "{WFS}?SERVICE=WFS&VERSION=2.0.0&REQUEST=GetFeature&TYPENAMES={LAYER}&OUTPUTFORMAT=application/json&SRSNAME=EPSG:2154&BBOX={e0},{n0},{e1},{n1},EPSG:2154&COUNT={PAGE}&STARTINDEX={}&SORTBY=coordonnees_nw",
            page * PAGE
        );
        let body = crate::cache::text("ign-index", &url, || {
            eprintln!("querying {url}");
            Ok(client.get(&url).send()?.error_for_status().context("LiDAR HD index")?.text()?)
        })?;
        let answer: serde_json::Value = serde_json::from_str(&body).context("the LiDAR HD index returned invalid JSON")?;
        let features = answer["features"].as_array().map(Vec::as_slice).unwrap_or_default();
        for feature in features {
            let Some(square) = parse(&feature["properties"]) else { continue };
            // The bounding box query also finds the squares that only touch the window's edge.
            if !overlaps(square.window, window) {
                continue;
            }
            if newest.get(&square.name).is_none_or(|known| known.edition < square.edition) {
                newest.insert(square.name.clone(), square);
            }
        }
        if features.len() < PAGE {
            break;
        }
    }
    Ok(newest.into_values().collect())
}

/// A square from a feature's properties, or `None` if they are not as expected.
fn parse(properties: &serde_json::Value) -> Option<Square> {
    let corner = properties["coordonnees_nw"].as_str()?;
    let (e, n) = corner.split_once('-')?;
    let (e, n) = (e.parse::<f64>().ok()? * 1000.0, n.parse::<f64>().ok()? * 1000.0);
    // Some features still say http; the server answers both.
    let cloud = properties["url_npl"].as_str()?.replacen("http://", "https://", 1);
    let edition = properties["date_edition"].as_str().unwrap_or_default().trim_end_matches('Z').to_string();
    Some(Square { name: corner.replace('-', "_"), window: [e, n - 1000.0, e + 1000.0, n], cloud, edition })
}

/// The point clouds of `squares` as `(name, url)`, for `lidar::read_files`.
pub fn clouds(squares: &[Square]) -> Vec<(String, String)> {
    squares.iter().map(|s| (s.name.clone(), s.cloud.clone())).collect()
}

/// The MNT `GetMap` for the pixels whose centres lie in `[e0, n0, e1, n1]`, which must be multiples
/// of `MNT_PIXEL`: the box reaches half a pixel beyond the centres, as IGN's own tile links do.
fn mnt_url([e0, n0, e1, n1]: Window) -> String {
    let half = MNT_PIXEL / 2.0;
    let (width, height) = (((e1 - e0) / MNT_PIXEL).round() as u32 + 1, ((n1 - n0) / MNT_PIXEL).round() as u32 + 1);
    format!(
        "{WMS}?SERVICE=WMS&VERSION=1.3.0&EXCEPTIONS=text/xml&REQUEST=GetMap&LAYERS={MNT_LAYER}&FORMAT=image/geotiff&STYLES=&CRS=EPSG:2154&BBOX={},{},{},{}&WIDTH={width}&HEIGHT={height}",
        e0 - half,
        n0 - half,
        e1 + half,
        n1 + half
    )
}

/// `window` cut into squares of at most `MNT_CHUNK` metres, their edges on the pixel grid and on a
/// fixed lattice, so neighbouring worlds ask for (and cache) the same pieces.
fn chunks([e0, n0, e1, n1]: Window) -> Vec<Window> {
    let lattice = |v: f64| (v / MNT_CHUNK).floor() as i64;
    let mut out = Vec::new();
    for j in lattice(n0)..=lattice(n1) {
        for i in lattice(e0)..=lattice(e1) {
            let (x, y) = (i as f64 * MNT_CHUNK, j as f64 * MNT_CHUNK);
            out.push([x, y, x + MNT_CHUNK, y + MNT_CHUNK]);
        }
    }
    out
}

/// The 0.5 m terrain model over `rects` (Lambert-93 windows), fetched piece by piece from the
/// WMS-R and cached. Where LiDAR HD has no data (not flown yet, or a restricted zone) the pieces hold
/// no-data and `Dem::mean` finds nothing.
pub fn mnt(rects: &[Window]) -> Result<Dem> {
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(120)).build()?;
    let mut pieces: Vec<Window> = rects.iter().flat_map(|&rect| chunks(rect)).collect();
    pieces.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    pieces.dedup();
    let mut files = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let url = mnt_url(*piece);
        // A WMS error is an XML document, not a TIFF.
        let bytes = crate::cache::bytes("ign-mnt", &url, || get_map(&client, &url, "MNT", |bytes| bytes.starts_with(b"II") || bytes.starts_with(b"MM")))?;
        crate::progress::steps(index + 1, pieces.len());
        let half = MNT_PIXEL / 2.0;
        let [e0, n0, e1, n1] = *piece;
        files.push((bytes, [e0 - half, n0 - half, e1 + half, n1 + half]));
    }
    Dem::from_bytes(files)
}

/// The aerial photo (BD ORTHO, 20 cm, resampled to `paint::PIXEL`) over `rects` (Lambert-93
/// windows), fetched piece by piece from the WMS-R and cached.
pub fn photo(rects: &[Window]) -> Result<crate::paint::Photo> {
    let client = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(120)).build()?;
    let mut pieces: Vec<Window> = rects.iter().flat_map(|&rect| chunks(rect)).collect();
    pieces.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    pieces.dedup();
    let mut files = Vec::new();
    for (index, piece) in pieces.iter().enumerate() {
        let url = photo_url(*piece);
        let bytes = crate::cache::bytes("ign-photo", &url, || get_map(&client, &url, "aerial photo", |bytes| bytes.starts_with(b"\x89PNG")))?;
        crate::progress::steps(index + 1, pieces.len());
        files.push((bytes, *piece));
    }
    crate::paint::Photo::from_pngs(MNT_CHUNK, files)
}

/// The photo `GetMap` for `window`, whose edges are the pixels' edges.
fn photo_url([e0, n0, e1, n1]: Window) -> String {
    let (width, height) = (((e1 - e0) / crate::paint::PIXEL).round() as u32, ((n1 - n0) / crate::paint::PIXEL).round() as u32);
    format!("{WMS}?SERVICE=WMS&VERSION=1.3.0&EXCEPTIONS=text/xml&REQUEST=GetMap&LAYERS={PHOTO_LAYER}&FORMAT=image/png&STYLES=&CRS=EPSG:2154&BBOX={e0},{n0},{e1},{n1}&WIDTH={width}&HEIGHT={height}")
}

/// A WMS-R `GetMap` answer that `valid` accepts, retrying a failed request a few times.
fn get_map(client: &reqwest::blocking::Client, url: &str, what: &str, valid: impl Fn(&[u8]) -> bool) -> Result<Vec<u8>> {
    let mut attempt = 0;
    loop {
        let result = client.get(url).send().and_then(|r| r.error_for_status()).and_then(|r| r.bytes());
        match result {
            Ok(bytes) if valid(&bytes) => break Ok(bytes.to_vec()),
            Ok(bytes) => bail!("the {what} service answered {}", String::from_utf8_lossy(&bytes[..bytes.len().min(300)])),
            Err(error) if attempt < 4 => {
                attempt += 1;
                eprintln!("  {what} piece failed ({error}); retry {attempt}");
                crate::progress::retry(2 * attempt);
            }
            Err(error) => break Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_feature_is_a_square_named_by_its_north_west_corner() {
        let properties = serde_json::json!({
            "coordonnees_nw": "0525-6701",
            "url_npl": "http://data.geopf.fr/telechargement/download/LiDARHD-NUALID/X/LHD_FXX_0525_6701_PTS_LAMB93_IGN69.copc.laz",
            "date_edition": "2025-06-20Z",
        });
        let square = parse(&properties).unwrap();
        assert_eq!(square.name, "0525_6701");
        assert_eq!(square.window, [525_000.0, 6_700_000.0, 526_000.0, 6_701_000.0]);
        assert!(square.cloud.starts_with("https://"));
        assert_eq!(square.edition, "2025-06-20");
    }

    #[test]
    fn mnt_pieces_lie_on_a_fixed_lattice() {
        let a = chunks([525_010.0, 6_700_010.0, 525_300.0, 6_700_300.0]);
        assert_eq!(a, vec![[524_800.0, 6_700_000.0, 525_200.0, 6_700_400.0], [525_200.0, 6_700_000.0, 525_600.0, 6_700_400.0]]);
        // A pixel centre on each corner: 801 pixels a side.
        assert!(photo_url(a[0]).contains("BBOX=524800,6700000,525200,6700400&WIDTH=800&HEIGHT=800"), "{}", photo_url(a[0]));
        assert!(mnt_url(a[0]).contains("BBOX=524799.75,6699999.75,525200.25,6700400.25&WIDTH=801&HEIGHT=801"), "{}", mnt_url(a[0]));
    }

    /// Exploration (network): `cargo test -p osm-import ign_probe -- --ignored --nocapture`. Prints the
    /// squares over a window in Tours, and the classes and density of 100 m × 100 m of one.
    #[test]
    #[ignore]
    fn ign_probe() {
        let window = [525_400.0, 6_700_400.0, 525_500.0, 6_700_500.0];
        let found = squares(window).unwrap();
        eprintln!("{found:#?}");
        let client = reqwest::blocking::Client::new();
        let mut classes = std::collections::BTreeMap::new();
        let total = crate::lidar::read_files(&client, &clouds(&found), window, None, ("", ""), |p| *classes.entry(p.class).or_insert(0usize) += 1).unwrap();
        eprintln!("{total} returns (not ground or noise) in 1 ha: {classes:?}");
        let dem = mnt(&[window]).unwrap();
        eprintln!("MNT at the centre: {:?}", dem.mean(525_450.0, 6_700_450.0, 0.5));
    }
}
