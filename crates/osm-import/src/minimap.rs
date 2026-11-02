//! Pre-renders the whole world as one top-down PNG (`minimap.png`, next to `world.msgpack`), so
//! the client's minimap and map are a static image instead of something it draws every frame.
//!
//! North is up: pixel x grows with world X (east), pixel y with world Z (south), so the image
//! maps the playable square `[-half, half]` onto `[0, SIZE)` in both axes.

use anyhow::Result;
use shared::world::{COVER_FARM, COVER_INDUSTRIAL, COVER_MARSH, COVER_MARSH_DEEP, COVER_MARSH_FAINT, COVER_PARK, COVER_ROCK, COVER_SAND, COVER_THICKET_FIGHT, COVER_THICKET_SLOW, COVER_THICKET_WALK, COVER_URBAN, COVER_WATER, COVER_WOOD, Manifest, NO_WATER, RAIL_TRAM, ROAD_FAINT, ROAD_GRAVEL, ROAD_MAJOR, ROAD_MINOR, ROAD_TRACK, THEME_WINTER, Tile};
use std::collections::BTreeMap;

/// Side of the image, pixels. 2048 px over 8.6 km is about 4 m a pixel.
pub const SIZE: usize = 2048;

type Rgb = [u8; 3];

const LAND: Rgb = [0xe9, 0xe5, 0xd9];
const WATER: Rgb = [0x8f, 0xb8, 0xe0];
const BUILDING: Rgb = [0xa9, 0x9d, 0x92];
const ROAD_MAJOR_COLOR: Rgb = [0xf2, 0xb8, 0x5c];
const ROAD_MINOR_COLOR: Rgb = [0xff, 0xff, 0xff];
const ROAD_PATH_COLOR: Rgb = [0xc4, 0xa2, 0x78];
const ROAD_GRAVEL_COLOR: Rgb = [0xf4, 0xef, 0xe4];
const ROAD_TRACK_COLOR: Rgb = [0xae, 0x8c, 0x62];
const ROAD_FAINT_COLOR: Rgb = [0xcf, 0xbd, 0x9c];
const RAIL_COLOR: Rgb = [0x6b, 0x6b, 0x6b];
const SNOW: Rgb = [0xf1, 0xf5, 0xfa];
const SKI_COLOR: Rgb = [0x1f, 0x6f, 0xd6];
/// How much of a winter map is snow laid over the summer's colours.
const SNOW_COVER: f64 = 0.7;

fn cover_color(kind: u8) -> Rgb {
    match kind {
        COVER_WATER => WATER,
        COVER_WOOD => [0xb4, 0xd2, 0x9c],
        COVER_PARK => [0xcb, 0xe3, 0xb2],
        COVER_FARM => [0xf0, 0xea, 0xc4],
        COVER_SAND => [0xf3, 0xe4, 0xb0],
        COVER_URBAN => [0xdc, 0xd6, 0xca],
        COVER_INDUSTRIAL => [0xd0, 0xd0, 0xda],
        COVER_THICKET_SLOW => [0x9c, 0xc8, 0x88],
        COVER_THICKET_WALK => [0x7f, 0xb6, 0x70],
        COVER_THICKET_FIGHT => [0x5e, 0x9e, 0x56],
        COVER_MARSH => [0x8f, 0xc4, 0xc4],
        COVER_MARSH_DEEP => [0x76, 0xb4, 0xb8],
        COVER_MARSH_FAINT => [0xae, 0xd4, 0xcc],
        COVER_ROCK => [0x5c, 0x56, 0x50],
        _ => LAND,
    }
}

/// An image laid over the drawn map, a print of an orienteering map where the world has one.
pub struct Overlay {
    pub width: usize,
    pub height: usize,
    /// RGB, row by row.
    pub pixels: Vec<u8>,
    /// Pixels to leave out.
    pub blank: Vec<bool>,
    /// The image position of a point in the world's frame.
    pub at: Box<dyn Fn(f64, f64) -> [f64; 2]>,
}

impl Overlay {
    /// The mean of what is not blank in the 2 × 2 pixels at `[x, y]`, if that is most of them.
    fn sample(&self, [x, y]: [f64; 2]) -> Option<Rgb> {
        let (mut sum, mut count) = ([0u32; 3], 0);
        for (i, j) in [(x, y), (x + 1.0, y), (x, y + 1.0), (x + 1.0, y + 1.0)] {
            if i < 0.0 || j < 0.0 || i >= self.width as f64 || j >= self.height as f64 {
                continue;
            }
            let at = j as usize * self.width + i as usize;
            if !self.blank[at] {
                count += 1;
                for (total, value) in sum.iter_mut().zip(&self.pixels[at * 3..at * 3 + 3]) {
                    *total += u32::from(*value);
                }
            }
        }
        (count >= 3).then(|| sum.map(|total| (total / count) as u8))
    }

    fn paint(&self, canvas: &mut Canvas) {
        for row in 0..SIZE {
            for column in 0..SIZE {
                let [x, z] = [(column as f64 + 0.5) / canvas.scale - canvas.half, (row as f64 + 0.5) / canvas.scale - canvas.half];
                if let Some(color) = self.sample((self.at)(x, z)) {
                    canvas.pixels[(row * SIZE + column) * 3..][..3].copy_from_slice(&color);
                }
            }
        }
    }
}

struct Canvas {
    pixels: Vec<u8>,
    /// World metres to pixels.
    scale: f64,
    /// Half the playable square, metres.
    half: f64,
}

impl Canvas {
    fn new(half: f64) -> Self {
        let mut pixels = vec![0u8; SIZE * SIZE * 3];
        for chunk in pixels.chunks_exact_mut(3) {
            chunk.copy_from_slice(&LAND);
        }
        Canvas { pixels, scale: SIZE as f64 / (2.0 * half), half }
    }

    /// World centimetres to pixel coordinates.
    fn px(&self, p: [i32; 2]) -> [f64; 2] {
        [(f64::from(p[0]) / 100.0 + self.half) * self.scale, (f64::from(p[1]) / 100.0 + self.half) * self.scale]
    }

    /// Fills the rings with the even-odd rule.
    fn fill(&mut self, rings: &[Vec<[f64; 2]>], color: Rgb) {
        let (mut top, mut bottom) = (f64::MAX, f64::MIN);
        for p in rings.iter().flatten() {
            top = top.min(p[1]);
            bottom = bottom.max(p[1]);
        }
        let first = top.floor().max(0.0) as usize;
        let last = (bottom.ceil().max(0.0) as usize).min(SIZE - 1);
        let mut crossings = Vec::new();
        for y in first..=last {
            let sample = y as f64 + 0.5;
            crossings.clear();
            for ring in rings {
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    if (a[1] <= sample) != (b[1] <= sample) {
                        crossings.push(a[0] + (sample - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                    }
                }
            }
            crossings.sort_by(f64::total_cmp);
            for span in crossings.chunks_exact(2) {
                // Pixels whose centres lie inside the span.
                let start = (span[0] - 0.5).ceil().max(0.0) as usize;
                let end = ((span[1] - 0.5).floor() + 1.0).clamp(0.0, SIZE as f64) as usize;
                for x in start..end {
                    let at = (y * SIZE + x) * 3;
                    self.pixels[at..at + 3].copy_from_slice(&color);
                }
            }
        }
    }

    /// Draws a polyline `width` pixels thick, as a quad per segment.
    fn stroke(&mut self, points: &[[f64; 2]], width: f64, color: Rgb) {
        let r = width / 2.0;
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dz);
            if length < 1e-9 {
                continue;
            }
            // Extended by r at both ends, so joints are filled.
            let (ux, uz) = (dx / length, dz / length);
            let (a, b) = ([a[0] - ux * r, a[1] - uz * r], [b[0] + ux * r, b[1] + uz * r]);
            let (nx, nz) = (-uz * r, ux * r);
            self.fill(&[vec![[a[0] + nx, a[1] + nz], [b[0] + nx, b[1] + nz], [b[0] - nx, b[1] - nz], [a[0] - nx, a[1] - nz]]], color);
        }
    }
}

/// Renders the world to PNG bytes.
pub fn render(manifest: &Manifest, tiles: &BTreeMap<(u16, u16), Tile>, overlay: Option<&Overlay>) -> Result<Vec<u8>> {
    let half_cm = manifest.half_extent;
    let mut canvas = Canvas::new(f64::from(half_cm) / 100.0);
    let ring = |canvas: &Canvas, points: &[[i32; 2]]| points.iter().map(|&p| canvas.px(p)).collect::<Vec<_>>();

    for tile in tiles.values() {
        for cover in &tile.cover {
            let rings: Vec<_> = cover.rings.iter().map(|r| ring(&canvas, r)).collect();
            canvas.fill(&rings, cover_color(cover.kind));
        }
    }

    // Lakes and rivers carved into the ground: every sample with water over it.
    let samples = manifest.tile_samples() as usize;
    if samples > 0 {
        let cell = f64::from(manifest.ground_cell);
        for (&(col, row), tile) in tiles {
            if tile.water.len() != samples * samples || tile.ground.len() != tile.water.len() {
                continue;
            }
            let x0 = f64::from(-half_cm) + f64::from(col) * f64::from(manifest.tile_size);
            let z0 = f64::from(-half_cm) + f64::from(row) * f64::from(manifest.tile_size);
            for j in 0..samples {
                for i in 0..samples {
                    let at = j * samples + i;
                    if tile.water[at] == NO_WATER || tile.water[at] <= tile.ground[at] {
                        continue;
                    }
                    // A cell centred on the sample.
                    let (cx, cz) = (x0 + i as f64 * cell, z0 + j as f64 * cell);
                    let corners = [[cx - cell / 2.0, cz - cell / 2.0], [cx + cell / 2.0, cz - cell / 2.0], [cx + cell / 2.0, cz + cell / 2.0], [cx - cell / 2.0, cz + cell / 2.0]]
                        .map(|c| canvas.px([c[0] as i32, c[1] as i32]));
                    canvas.fill(&[corners.to_vec()], WATER);
                }
            }
        }
    }
    for tile in tiles.values() {
        for stream in &tile.streams {
            let width = (f64::from(stream.width) / 100.0 * canvas.scale).max(1.5);
            canvas.stroke(&ring(&canvas, &stream.points), width, WATER);
        }
    }

    // Paths under minor roads under major ones.
    for (kind, color) in [(ROAD_FAINT, ROAD_FAINT_COLOR), (2u8, ROAD_PATH_COLOR), (ROAD_TRACK, ROAD_TRACK_COLOR), (ROAD_GRAVEL, ROAD_GRAVEL_COLOR), (ROAD_MINOR, ROAD_MINOR_COLOR), (ROAD_MAJOR, ROAD_MAJOR_COLOR)] {
        for road in tiles.values().flat_map(|tile| &tile.roads).filter(|road| road.kind == kind) {
            let floor = if kind == 2 { 1.0 } else { 2.0 };
            let width = (f64::from(road.width) / 100.0 * canvas.scale).max(floor);
            canvas.stroke(&ring(&canvas, &road.points), width, color);
        }
    }
    // Tracks over the roads, so a level crossing shows the line running through.
    for rail in tiles.values().flat_map(|tile| &tile.rails) {
        let width = if rail.kind == RAIL_TRAM { 1.0 } else { (crate::assemble::ballast(f64::from(rail.gauge) / 100.0) * canvas.scale).max(1.5) };
        canvas.stroke(&ring(&canvas, &rail.points), width, RAIL_COLOR);
    }

    for building in tiles.values().flat_map(|tile| &tile.buildings) {
        let footprint = ring(&canvas, &building.footprint);
        canvas.fill(&[footprint], BUILDING);
    }

    if manifest.theme == THEME_WINTER {
        for pixel in canvas.pixels.chunks_exact_mut(3) {
            for (channel, snow) in pixel.iter_mut().zip(SNOW) {
                *channel = (f64::from(*channel) * (1.0 - SNOW_COVER) + f64::from(snow) * SNOW_COVER) as u8;
            }
        }
        // After the snow, so the tracks stand out from it.
        for trail in tiles.values().flat_map(|tile| &tile.ski) {
            canvas.stroke(&ring(&canvas, &trail.points), (f64::from(trail.width) / 100.0 * canvas.scale).max(2.0), SKI_COLOR);
        }
    }

    if let Some(overlay) = overlay {
        overlay.paint(&mut canvas);
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, SIZE as u32, SIZE as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    encoder.write_header()?.write_image_data(&canvas.pixels)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{Building, RAIL_TRAIN, Rail, Road};

    #[test]
    fn draws_buildings_and_roads_where_they_are() {
        let manifest = Manifest { half_extent: 100_000, tile_size: 25_000, ..Default::default() }; // 1 km half
        let mut tile = Tile::default();
        // A 100 m building in the north-east quadrant, and a road along X = 0.
        tile.buildings.push(Building { footprint: vec![[20_000, -30_000], [30_000, -30_000], [30_000, -20_000], [20_000, -20_000]], ..Default::default() });
        tile.roads.push(Road { width: 800, kind: ROAD_MAJOR, points: vec![[0, -90_000], [0, 90_000]] });
        // A track along Z = 500 m, crossing the road.
        tile.rails.push(Rail { gauge: 144, kind: RAIL_TRAIN, points: vec![[-90_000, 50_000], [90_000, 50_000]] });
        let tiles = BTreeMap::from([((0, 0), tile)]);
        let bytes = render(&manifest, &tiles, None).unwrap();
        let decoded = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().unwrap();
        let mut reader = decoded;
        let mut pixels = vec![0u8; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut pixels).unwrap();
        let at = |x: usize, y: usize| [pixels[(y * SIZE + x) * 3], pixels[(y * SIZE + x) * 3 + 1], pixels[(y * SIZE + x) * 3 + 2]];
        let scale = SIZE as f64 / 2000.0; // pixels per metre: the square is 2 km across
        let to_px = |m: f64| ((m + 1000.0) * scale) as usize;
        assert_eq!(at(to_px(250.0), to_px(-250.0)), BUILDING);
        assert_eq!(at(to_px(0.0), to_px(0.0)), ROAD_MAJOR_COLOR);
        assert_eq!(at(to_px(500.0), to_px(500.0)), RAIL_COLOR);
        assert_eq!(at(to_px(0.0), to_px(500.0)), RAIL_COLOR, "the track crosses over the road");
        assert_eq!(at(to_px(500.0), to_px(250.0)), LAND);
    }
}
