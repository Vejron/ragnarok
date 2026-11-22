//! Pre-renders the world as top-down pictures, so the client's maps are static images instead of
//! something it draws every frame: the whole world as one (`minimap.webp`, next to `world.msgpack`), the
//! overview the settings page and the embeds show, and any square of it at any scale (`render_square`),
//! which the map tiles the game shows are made of (`maptiles.rs`).
//!
//! North is up: pixel x grows with world X (east), pixel y with world Z (south).

use shared::world::{COVER_ASPHALT, COVER_FARM, COVER_GARDEN, COVER_GOLF_BUNKER, COVER_GOLF_FAIRWAY, COVER_GOLF_GREEN, COVER_GOLF_TEE, COVER_INDUSTRIAL, COVER_MARSH, COVER_MARSH_DEEP, COVER_MARSH_FAINT, COVER_PARK, COVER_ROCK, COVER_SAND, COVER_THICKET_FIGHT, COVER_THICKET_SLOW, COVER_THICKET_WALK, COVER_URBAN, COVER_WATER, COVER_WOOD, Manifest, NO_WATER, RAIL_TRAM, ROAD_FAINT, ROAD_MAJOR, ROAD_PAVED_PATH, ROAD_MINOR, ROAD_TRACK, THEME_WINTER, Tile, BUILDING_BRIDGE};
use std::collections::BTreeMap;

/// Side of the image, pixels. 2048 px over 8.6 km is about 4 m a pixel.
pub const SIZE: usize = 2048;
/// Side of the image when a print of an orienteering map is laid over it, pixels: finer, so the map's
/// symbols stay readable. 4096 px over 6 km is about 1.5 m a pixel, near what a print has.
pub const PRINT_SIZE: usize = 4096;
/// The WebP quality the picture is saved at, 0..100: the map's thin lines and symbols stay sharp.
const QUALITY: f32 = 85.0;

type Rgb = [u8; 3];

const LAND: Rgb = [0xe9, 0xe5, 0xd9];
const WATER: Rgb = [0x8f, 0xb8, 0xe0];
const BUILDING: Rgb = [0xa9, 0x9d, 0x92];
const ROAD_MAJOR_COLOR: Rgb = [0xf2, 0xb8, 0x5c];
const ROAD_MINOR_COLOR: Rgb = [0xff, 0xff, 0xff];
const ROAD_PATH_COLOR: Rgb = [0xc4, 0xa2, 0x78];
/// A paved footway or cycleway: grey like the asphalt it is.
const ROAD_PAVED_PATH_COLOR: Rgb = [0xd4, 0xd4, 0xd0];
const ROAD_TRACK_COLOR: Rgb = [0xae, 0x8c, 0x62];
const ROAD_FAINT_COLOR: Rgb = [0xcf, 0xbd, 0x9c];
const RAIL_COLOR: Rgb = [0x6b, 0x6b, 0x6b];
const SNOW: Rgb = [0xf1, 0xf5, 0xfa];
const SKI_COLOR: Rgb = [0x1f, 0x6f, 0xd6];
/// A bus line and its stops, and how wide the line and a stop's dot are drawn, pixels.
const BUS_LINE_COLOR: Rgb = [0xd0, 0x30, 0x30];
const BUS_STOP_COLOR: Rgb = [0x90, 0x10, 0x10];
const BUS_LINE_WIDTH: f64 = 2.0;
const BUS_STOP_DOT: f64 = 6.0;
/// How wide a bus line and a stop's dot are drawn on the ground, metres, where that is more than their
/// pixels.
const BUS_LINE_METRES: f64 = 3.0;
const BUS_STOP_METRES: f64 = 7.0;
/// How much of a winter map is snow laid over the summer's colours.
const SNOW_COVER: f64 = 0.7;

fn cover_color(kind: u8) -> Rgb {
    match kind {
        COVER_WATER => WATER,
        COVER_WOOD => [0xb4, 0xd2, 0x9c],
        COVER_PARK => [0xcb, 0xe3, 0xb2],
        COVER_FARM => [0xf0, 0xea, 0xc4],
        COVER_SAND => [0xf3, 0xe4, 0xb0],
        COVER_URBAN | COVER_GARDEN => [0xdc, 0xd6, 0xca],
        COVER_INDUSTRIAL => [0xd0, 0xd0, 0xda],
        // A car park: a road's surface, a shade off the white the roads are drawn in.
        COVER_ASPHALT => [0xec, 0xec, 0xee],
        COVER_THICKET_SLOW => [0x9c, 0xc8, 0x88],
        COVER_THICKET_WALK => [0x7f, 0xb6, 0x70],
        COVER_THICKET_FIGHT => [0x5e, 0x9e, 0x56],
        COVER_MARSH => [0x8f, 0xc4, 0xc4],
        COVER_MARSH_DEEP => [0x76, 0xb4, 0xb8],
        COVER_MARSH_FAINT => [0xae, 0xd4, 0xcc],
        COVER_ROCK => [0x5c, 0x56, 0x50],
        COVER_GOLF_FAIRWAY | COVER_GOLF_TEE => [0xb2, 0xde, 0x96],
        COVER_GOLF_GREEN => [0x96, 0xd6, 0x84],
        COVER_GOLF_BUNKER => [0xf6, 0xea, 0xc0],
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
    pub at: Box<dyn Fn(f64, f64) -> [f64; 2] + Send + Sync>,
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
        let size = canvas.size;
        for row in 0..size {
            for column in 0..size {
                let [x, z] = [(column as f64 + 0.5) / canvas.scale + canvas.origin[0], (row as f64 + 0.5) / canvas.scale + canvas.origin[1]];
                if let Some(color) = self.sample((self.at)(x, z)) {
                    canvas.pixels[(row * size + column) * 3..][..3].copy_from_slice(&color);
                }
            }
        }
    }
}

/// The rendered map: `size` × `size` pixels of RGB, row by row.
pub struct Canvas {
    pub size: usize,
    pub pixels: Vec<u8>,
    /// World metres to pixels.
    scale: f64,
    /// The world point at the picture's top left corner, metres `[x, z]`.
    origin: [f64; 2],
}

impl Canvas {
    /// A blank picture `size` pixels across of the square `metres` across from `origin` (its north-west corner).
    fn new(origin: [f64; 2], metres: f64, size: usize) -> Self {
        let mut pixels = vec![0u8; size * size * 3];
        for chunk in pixels.chunks_exact_mut(3) {
            chunk.copy_from_slice(&LAND);
        }
        Canvas { size, pixels, scale: size as f64 / metres, origin }
    }

    /// The picture as a lossy WebP at `quality` (0..100).
    pub fn webp_at(&self, quality: f32) -> Vec<u8> {
        webp::Encoder::from_rgb(&self.pixels, self.size as u32, self.size as u32).encode(quality).to_vec()
    }

    /// The picture as a lossy WebP.
    pub fn webp(&self) -> Vec<u8> {
        self.webp_at(QUALITY)
    }

    /// World centimetres to pixel coordinates.
    fn px(&self, p: [i32; 2]) -> [f64; 2] {
        [(f64::from(p[0]) / 100.0 - self.origin[0]) * self.scale, (f64::from(p[1]) / 100.0 - self.origin[1]) * self.scale]
    }

    /// Fills the rings with the even-odd rule.
    fn fill(&mut self, rings: &[Vec<[f64; 2]>], color: Rgb) {
        let (mut top, mut bottom) = (f64::MAX, f64::MIN);
        for p in rings.iter().flatten() {
            top = top.min(p[1]);
            bottom = bottom.max(p[1]);
        }
        let first = top.floor().max(0.0) as usize;
        let last = (bottom.ceil().max(0.0) as usize).min(self.size - 1);
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
                let end = ((span[1] - 0.5).floor() + 1.0).clamp(0.0, self.size as f64) as usize;
                for x in start..end {
                    let at = (y * self.size + x) * 3;
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

/// Renders the world, `PRINT_SIZE` across under a print and `SIZE` otherwise.
pub fn render(manifest: &Manifest, tiles: &BTreeMap<(u16, u16), Tile>, overlay: Option<&Overlay>) -> Canvas {
    let half = f64::from(manifest.half_extent) / 100.0;
    render_square(manifest, tiles, overlay, [-half, -half], 2.0 * half, if overlay.is_some() { PRINT_SIZE } else { SIZE })
}

/// Renders the square `metres` across whose north-west corner is `origin` (metres `[x, z]`), `size`
/// pixels across. Only the world's tiles under it, and those next to them (whose roads and streams may
/// reach over), are drawn.
pub fn render_square(manifest: &Manifest, all: &BTreeMap<(u16, u16), Tile>, overlay: Option<&Overlay>, origin: [f64; 2], metres: f64, size: usize) -> Canvas {
    let half_cm = manifest.half_extent;
    let half = f64::from(half_cm) / 100.0;
    let side = f64::from(manifest.tile_size) / 100.0;
    let span = |from: f64| (((from + half) / side).floor() - 1.0, ((from + metres + half) / side).ceil() + 1.0);
    let ((c0, c1), (r0, r1)) = (span(origin[0]), span(origin[1]));
    let tiles: BTreeMap<(u16, u16), &Tile> = all.iter().filter(|&(&(col, row), _)| (c0..=c1).contains(&f64::from(col)) && (r0..=r1).contains(&f64::from(row))).map(|(&key, tile)| (key, tile)).collect();
    let mut canvas = Canvas::new(origin, metres, size);
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
        for (&(col, row), tile) in &tiles {
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
    for (kind, color) in [(ROAD_FAINT, ROAD_FAINT_COLOR), (2u8, ROAD_PATH_COLOR), (ROAD_TRACK, ROAD_TRACK_COLOR), (ROAD_PAVED_PATH, ROAD_PAVED_PATH_COLOR), (ROAD_MINOR, ROAD_MINOR_COLOR), (ROAD_MAJOR, ROAD_MAJOR_COLOR)] {
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

    // A bridge's sections are buildings to carve, not houses: its road is on the map already.
    for building in tiles.values().flat_map(|tile| &tile.buildings).filter(|building| building.kind != BUILDING_BRIDGE) {
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

    // The bus lines over everything, each with a dot at every stop.
    let (line_width, dot) = ((BUS_LINE_METRES * canvas.scale).max(BUS_LINE_WIDTH), (BUS_STOP_METRES * canvas.scale).max(BUS_STOP_DOT));
    for line in &manifest.bus_lines {
        canvas.stroke(&ring(&canvas, &line.points), line_width, BUS_LINE_COLOR);
        for &stop in &line.stops {
            let ([x, z], _) = line.at(stop as f32 / 100.0);
            let [px, pz] = canvas.px([(x * 100.0) as i32, (z * 100.0) as i32]);
            let r = dot / 2.0;
            canvas.fill(&[vec![[px - r, pz - r], [px + r, pz - r], [px + r, pz + r], [px - r, pz + r]]], BUS_STOP_COLOR);
        }
    }
    canvas
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
        tile.roads.push(Road { width: 800, kind: ROAD_MAJOR, carried: false, points: vec![[0, -90_000], [0, 90_000]], ..Default::default() });
        // A track along Z = 500 m, crossing the road.
        tile.rails.push(Rail { gauge: 144, kind: RAIL_TRAIN, points: vec![[-90_000, 50_000], [90_000, 50_000]], deck: None });
        let tiles = BTreeMap::from([((0, 0), tile)]);
        let canvas = render(&manifest, &tiles, None);
        assert_eq!(canvas.size, SIZE);
        let pixels = &canvas.pixels;
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
