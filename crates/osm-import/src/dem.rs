//! Elevation from Lantmäteriet's *Markhöjdmodell* (the 1 m DTM, `stac-hojd/v1/collections/dtm-cog`):
//! cloud-optimised GeoTIFFs in SWEREF 99 TM (EPSG:3006 horizontally, heights in RH 2000).
//!
//! A file is either a local path (`--dem`) or found through the STAC API and read with HTTP
//! range requests, so only the blocks under the world's window are transferred. Downloads need
//! Basic credentials from `LM_USER` / `LM_PASS`.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use tiff::decoder::{Decoder, DecodingResult};
use tiff::tags::Tag;

const STAC_ITEMS: &str = "https://api.lantmateriet.se/stac-hojd/v1/collections/dtm-cog/items";

/// WGS 84 degrees to SWEREF 99 TM `(easting, northing)` metres (Krüger series, GRS 80).
pub fn sweref99tm(lat: f64, lon: f64) -> (f64, f64) {
    const A: f64 = 6_378_137.0;
    const F: f64 = 1.0 / 298.257_222_101;
    const K0: f64 = 0.9996;
    const LON0: f64 = 15.0;
    const FALSE_EASTING: f64 = 500_000.0;
    let e2 = F * (2.0 - F);
    let n = F / (2.0 - F);
    let a_hat = A / (1.0 + n) * (1.0 + n * n / 4.0 + n.powi(4) / 64.0);
    let beta = [
        n / 2.0 - 2.0 * n * n / 3.0 + 5.0 * n.powi(3) / 16.0 + 41.0 * n.powi(4) / 180.0,
        13.0 * n * n / 48.0 - 3.0 * n.powi(3) / 5.0 + 557.0 * n.powi(4) / 1440.0,
        61.0 * n.powi(3) / 240.0 - 103.0 * n.powi(4) / 140.0,
        49561.0 * n.powi(4) / 161_280.0,
    ];
    let phi = lat.to_radians();
    let dl = (lon - LON0).to_radians();
    // The conformal latitude, in closed form.
    let (e, s) = (e2.sqrt(), phi.sin());
    let phi_star = 2.0 * ((std::f64::consts::FRAC_PI_4 + phi / 2.0).tan() * ((1.0 - e * s) / (1.0 + e * s)).powf(e / 2.0)).atan() - std::f64::consts::FRAC_PI_2;
    let xi = (phi_star.tan() / dl.cos()).atan();
    let eta = (phi_star.cos() * dl.sin()).atanh();
    let mut northing = xi;
    let mut easting = eta;
    for (j, b) in beta.iter().enumerate() {
        let k = 2.0 * (j + 1) as f64;
        northing += b * (k * xi).sin() * (k * eta).cosh();
        easting += b * (k * xi).cos() * (k * eta).sinh();
    }
    (FALSE_EASTING + K0 * a_hat * easting, K0 * a_hat * northing)
}

/// A rectangle of elevations cut out of a DEM file. `n_top` is the northing of the top edge.
pub struct Raster {
    e0: f64,
    n_top: f64,
    scale: f64,
    width: usize,
    height: usize,
    data: Vec<f32>,
}

impl Raster {
    /// Mean of the valid cells within `half` metres of `(e, n)`, or `None` with no data there.
    fn mean(&self, e: f64, n: f64, half: f64) -> Option<f32> {
        let col = |e: f64| ((e - self.e0) / self.scale).floor();
        let row = |n: f64| ((self.n_top - n) / self.scale).floor();
        let (x0, x1) = (col(e - half).max(0.0), col(e + half).min(self.width as f64 - 1.0));
        let (y0, y1) = (row(n + half).max(0.0), row(n - half).min(self.height as f64 - 1.0));
        if x0 > x1 || y0 > y1 {
            return None;
        }
        let (mut sum, mut count) = (0.0f64, 0u32);
        for y in y0 as usize..=y1 as usize {
            for x in x0 as usize..=x1 as usize {
                let v = self.data[y * self.width + x];
                if v.is_finite() {
                    sum += v as f64;
                    count += 1;
                }
            }
        }
        (count > 0).then(|| (sum / count as f64) as f32)
    }
}

/// Elevations from one or more files, to sample by SWEREF 99 TM coordinates.
pub struct Dem {
    rasters: Vec<Raster>,
}

/// SWEREF 99 TM window `[e0, n0, e1, n1]`, metres.
pub type Window = [f64; 4];

impl Dem {
    /// The mean elevation within `half` metres of `(e, n)`, or `None` where no file has data.
    pub fn mean(&self, e: f64, n: f64, half: f64) -> Option<f32> {
        self.rasters.iter().find_map(|raster| raster.mean(e, n, half))
    }

    pub fn from_path(path: &std::path::Path, window: Window) -> Result<Self> {
        let file = std::io::BufReader::new(std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?);
        let raster = read_window(file, window)?.with_context(|| format!("{} does not cover the world", path.display()))?;
        Ok(Self { rasters: vec![raster] })
    }

    /// Finds the DEM files covering `bbox` (`[south, west, north, east]`) through STAC and reads the
    /// part of each that lies in `window`.
    pub fn from_stac(bbox: [f64; 4], window: Window, user: &str, password: &str) -> Result<Self> {
        let [south, west, north, east] = bbox;
        let client = reqwest::blocking::Client::builder().build()?;
        let url = format!("{STAC_ITEMS}?bbox={west},{south},{east},{north}&limit=50");
        eprintln!("querying {url}");
        let body = crate::cache::text("stac", &url, || Ok(client.get(&url).send()?.error_for_status().context("STAC search")?.text()?))?;
        let items: serde_json::Value = serde_json::from_str(&body).context("STAC search returned invalid JSON")?;
        let mut rasters = Vec::new();
        for item in items["features"].as_array().map(Vec::as_slice).unwrap_or_default() {
            let (Some(id), Some(href)) = (item["id"].as_str(), item["assets"]["data"]["href"].as_str()) else { continue };
            eprintln!("reading {id}: {href}");
            let reader = RangeReader::open(client.clone(), href, user, password)?;
            rasters.extend(read_window(reader, window)?);
        }
        if rasters.is_empty() {
            bail!("no elevation data covers {bbox:?}");
        }
        Ok(Self { rasters })
    }
}

/// Reads the pixels of `window` from a GeoTIFF, or `None` if the file does not overlap it.
fn read_window<R: Read + Seek>(source: R, [we, wn, ee, en]: Window) -> Result<Option<Raster>> {
    let mut decoder = Decoder::new(source).context("not a readable TIFF")?;
    let (width, height) = decoder.dimensions()?;
    let scale = decoder.get_tag_f64_vec(Tag::ModelPixelScaleTag).context("no ModelPixelScale: not a GeoTIFF")?;
    let tie = decoder.get_tag_f64_vec(Tag::ModelTiepointTag).context("no ModelTiepoint: not a GeoTIFF")?;
    anyhow::ensure!(scale.len() >= 2 && tie.len() >= 6, "unexpected GeoTIFF georeferencing");
    anyhow::ensure!((scale[0] - scale[1]).abs() < 1e-9, "non-square pixels");
    // Pixel-is-area vs pixel-is-point is a half pixel, which on a 1 m grid is below what we sample at.
    let (e_left, n_top, pixel) = (tie[3] - tie[0] * scale[0], tie[4] + tie[1] * scale[1], scale[0]);
    let nodata = decoder.get_tag_ascii_string(Tag::GdalNodata).ok().and_then(|text| text.trim().parse::<f64>().ok());

    // The pixels of the window, clipped to the file.
    let clip = |v: f64, max: u32| v.clamp(0.0, max as f64) as u32;
    let (px0, px1) = (clip(((we - e_left) / pixel).floor(), width), clip(((ee - e_left) / pixel).ceil(), width));
    let (py0, py1) = (clip(((n_top - en) / pixel).floor(), height), clip(((n_top - wn) / pixel).ceil(), height));
    if px0 >= px1 || py0 >= py1 {
        return Ok(None);
    }
    let (w, h) = ((px1 - px0) as usize, (py1 - py0) as usize);
    let mut data = vec![f32::NAN; w * h];

    let (chunk_w, chunk_h) = decoder.chunk_dimensions();
    let across = width.div_ceil(chunk_w);
    for cy in py0 / chunk_h..=(py1 - 1) / chunk_h {
        for cx in px0 / chunk_w..=(px1 - 1) / chunk_w {
            let index = cy * across + cx;
            let (data_w, data_h) = decoder.chunk_data_dimensions(index);
            let values: Vec<f32> = match decoder.read_chunk(index).with_context(|| format!("decoding chunk {index}"))? {
                DecodingResult::F32(v) => v,
                DecodingResult::F64(v) => v.into_iter().map(|x| x as f32).collect(),
                DecodingResult::I16(v) => v.into_iter().map(|x| x as f32).collect(),
                DecodingResult::U16(v) => v.into_iter().map(|x| x as f32).collect(),
                DecodingResult::I32(v) => v.into_iter().map(|x| x as f32).collect(),
                other => bail!("unsupported sample type {:?}", std::mem::discriminant(&other)),
            };
            for row in 0..data_h {
                let y = cy * chunk_h + row;
                if y < py0 || y >= py1 {
                    continue;
                }
                for col in 0..data_w {
                    let x = cx * chunk_w + col;
                    if x < px0 || x >= px1 {
                        continue;
                    }
                    let v = values[(row * data_w + col) as usize];
                    let valid = v.is_finite() && v > -1000.0 && nodata.is_none_or(|n| (v as f64 - n).abs() > 1e-3);
                    data[(y - py0) as usize * w + (x - px0) as usize] = if valid { v } else { f32::NAN };
                }
            }
        }
    }
    Ok(Some(Raster { e0: e_left + px0 as f64 * pixel, n_top: n_top - py0 as f64 * pixel, scale: pixel, width: w, height: h, data }))
}

/// `Read + Seek` over an HTTP resource, fetching 512 KiB blocks with range requests on demand.
pub(crate) struct RangeReader {
    client: reqwest::blocking::Client,
    url: String,
    user: String,
    password: String,
    length: u64,
    position: u64,
    blocks: HashMap<u64, Vec<u8>>,
}

const BLOCK: u64 = 512 * 1024;

impl RangeReader {
    pub(crate) fn open(client: reqwest::blocking::Client, url: &str, user: &str, password: &str) -> Result<Self> {
        // The length is cached too, so a fully cached file is read without touching the network.
        let length = crate::cache::text("length", url, || {
            let response = client.get(url).basic_auth(user, Some(password)).header("Range", "bytes=0-0").send()?;
            anyhow::ensure!(response.status() != reqwest::StatusCode::UNAUTHORIZED, "{url} refused the credentials in LM_USER / LM_PASS (see WORLD.md)");
            let response = response.error_for_status()?;
            let length = response
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit('/').next())
                .and_then(|v| v.parse::<u64>().ok())
                .context("server does not support range requests")?;
            Ok(length.to_string())
        })?
        .parse::<u64>()
        .context("cached file length is not a number; try --refresh")?;
        Ok(Self { client, url: url.into(), user: user.into(), password: password.into(), length, position: 0, blocks: HashMap::new() })
    }

    fn block(&mut self, index: u64) -> std::io::Result<&Vec<u8>> {
        if !self.blocks.contains_key(&index) {
            let (start, end) = (index * BLOCK, ((index + 1) * BLOCK).min(self.length) - 1);
            let bytes = crate::cache::bytes("range", &format!("{}#{index}", self.url), || {
                // Lantmäteriet's server resets connections now and then; a few tries get through.
                let mut attempt = 0;
                loop {
                    let result = self
                        .client
                        .get(&self.url)
                        .basic_auth(&self.user, Some(&self.password))
                        .header("Range", format!("bytes={start}-{end}"))
                        .send()
                        .and_then(|r| r.error_for_status())
                        .and_then(|r| r.bytes());
                    match result {
                        Ok(bytes) => break Ok(bytes.to_vec()),
                        Err(error) if attempt < 4 => {
                            attempt += 1;
                            eprintln!("  range {start}-{end} failed ({error}); retry {attempt}");
                            std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
                        }
                        Err(error) => break Err(error.into()),
                    }
                }
            })
            .map_err(std::io::Error::other)?;
            self.blocks.insert(index, bytes);
        }
        Ok(&self.blocks[&index])
    }
}

impl Read for RangeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.length || buf.is_empty() {
            return Ok(0);
        }
        let index = self.position / BLOCK;
        let offset = (self.position % BLOCK) as usize;
        let block = self.block(index)?;
        let n = buf.len().min(block.len() - offset);
        buf[..n].copy_from_slice(&block[offset..offset + n]);
        self.position += n as u64;
        Ok(n)
    }
}

impl Seek for RangeReader {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let next = match from {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => self.length as i64 + d,
            SeekFrom::Current(d) => self.position as i64 + d,
        };
        if next < 0 {
            return Err(std::io::Error::other("seek before start"));
        }
        self.position = next as u64;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backen_is_in_tile_708_75() {
        let (e, n) = sweref99tm(63.832127, 20.168286);
        assert!((750_000.0..760_000.0).contains(&e), "E = {e}");
        assert!((7_080_000.0..7_090_000.0).contains(&n), "N = {n}");
    }

    #[test]
    fn matches_an_independent_transverse_mercator() {
        // Snyder's series (Transverse Mercator, GRS 80, k0 0.9996, lon0 15°) for Backen, Umeå.
        let (e, n) = sweref99tm(63.832127, 20.168286);
        assert!((e - 754_096.68).abs() < 0.1 && (n - 7_088_603.15).abs() < 0.1, "{e} {n}");
    }

    #[test]
    fn central_meridian_has_the_false_easting() {
        let (e, _) = sweref99tm(60.0, 15.0);
        assert!((e - 500_000.0).abs() < 1e-6);
    }

    #[test]
    fn raster_mean_skips_nodata() {
        let raster = Raster { e0: 0.0, n_top: 4.0, scale: 1.0, width: 4, height: 4, data: (0..16).map(|i| if i == 5 { f32::NAN } else { 10.0 }).collect() };
        assert_eq!(raster.mean(2.0, 2.0, 1.0), Some(10.0));
        assert_eq!(raster.mean(100.0, 100.0, 0.5), None);
    }
}
