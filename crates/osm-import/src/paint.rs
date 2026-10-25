//! A building's colours: its roof's from the aerial photo over it, snapped to a small toon palette,
//! and its walls' from a muted palette (no photo sees a wall). France only, where IGN's BD ORTHO is
//! open (`ign::photo`); elsewhere a building has no paint and the client picks its colours.

use crate::assemble::point_in_polygon;
use crate::dem::Window;
use anyhow::{Context, Result};
use std::collections::HashMap;

type P = [f64; 2];

/// The photo's pixel, metres: fine enough for the roof of a garden shed.
pub const PIXEL: f64 = 0.5;
/// Fewer roof pixels than this (most of the roof under trees) and the roof is not sampled.
const MIN_SAMPLES: usize = 6;

/// French roofs, each as the photo shows it and as it is drawn: slate and zinc in the north and the
/// Loire, tile in the south and east, gravel, tar and membranes on flat ones. The photo's colours
/// are greyer and darker than the drawn ones (they were measured on the photo over Tours); the drawn
/// ones are bright enough to read as toon.
const ROOF_LOOKS: [([u8; 3], u32); 9] = [
    ([74, 76, 84], 0x56606e),    // slate
    ([105, 110, 118], 0x7b8694), // weathered slate
    ([150, 155, 158], 0xa4acb4), // zinc
    ([185, 183, 175], 0xc8c5bc), // gravel, concrete
    ([48, 49, 52], 0x46484d),    // tar
    ([215, 216, 212], 0xe2e0da), // membrane
    ([190, 135, 110], 0xbf5a3e), // terracotta
    ([215, 170, 140], 0xd2814f), // pale tile
    ([135, 104, 96], 0x8e5a46),  // old tile
];
/// The roof colours a building can be drawn in.
pub const ROOFS: [u32; 9] = {
    let mut out = [0; 9];
    let mut i = 0;
    while i < out.len() {
        out[i] = ROOF_LOOKS[i].1;
        i += 1;
    }
    out
};
/// Walls: limestone, plaster, sandstone, ochre, a little brick.
pub const WALLS: [u32; 8] = [0xf0e6cc, 0xf4f0e6, 0xe6d3b2, 0xead0a0, 0xd8d3c8, 0xe9c9b6, 0xdcc6a2, 0xc0785a];
/// The roof where the photo shows none (under trees, or a cloud of pixels outside the photo).
const UNSEEN_ROOF: u32 = ROOFS[0];

/// An aerial photo over part of a world: pieces of `side` metres on a lattice, RGB rows from north.
pub struct Photo {
    side: f64,
    pieces: HashMap<(i64, i64), Vec<u8>>,
}

impl Photo {
    /// A photo from PNG pieces, each `side` metres a side with its south-west corner on the
    /// lattice at `window[0], window[1]`, `side / PIXEL` pixels a side.
    pub fn from_pngs(side: f64, pieces: Vec<(Vec<u8>, Window)>) -> Result<Photo> {
        let pixels = (side / PIXEL).round() as usize;
        let mut out = HashMap::new();
        for (bytes, window) in pieces {
            let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().context("an aerial photo piece is not a PNG")?;
            let mut buffer = vec![0u8; reader.output_buffer_size().context("photo size")?];
            let info = reader.next_frame(&mut buffer)?;
            let channels = info.color_type.samples();
            anyhow::ensure!(info.width as usize == pixels && info.height as usize == pixels && info.bit_depth == png::BitDepth::Eight && channels >= 3, "an aerial photo piece is {}x{} {:?} {:?}", info.width, info.height, info.color_type, info.bit_depth);
            let rgb = buffer[..info.buffer_size()].chunks_exact(channels).flat_map(|p| [p[0], p[1], p[2]]).collect();
            out.insert(((window[0] / side).round() as i64, (window[1] / side).round() as i64), rgb);
        }
        Ok(Photo { side, pieces: out })
    }

    /// The colour at `(e, n)` in the photo's coordinates, or `None` outside it.
    pub fn at(&self, e: f64, n: f64) -> Option<[u8; 3]> {
        let (i, j) = ((e / self.side).floor(), (n / self.side).floor());
        let rgb = self.pieces.get(&(i as i64, j as i64))?;
        let pixels = (self.side / PIXEL).round() as usize;
        let col = (((e - i * self.side) / PIXEL) as usize).min(pixels - 1);
        let row = ((((j + 1.0) * self.side - n) / PIXEL) as usize).min(pixels - 1);
        let at = (row * pixels + col) * 3;
        Some([rgb[at], rgb[at + 1], rgb[at + 2]])
    }
}

/// `[wall, roof]` of the building whose footprint is `rings` (local metres; one ring, or the pieces
/// it was cut into), `source` mapping local metres to the photo's coordinates.
pub fn of(photo: &Photo, rings: &[&[P]], source: impl Fn(f64, f64) -> (f64, f64)) -> [u32; 2] {
    let mut channels: [Vec<u8>; 3] = Default::default();
    for ring in rings {
        seen(photo, ring, &source, &mut channels);
    }
    let roof = median(channels).map_or(UNSEEN_ROOF, nearest_roof);
    [wall(rings), roof]
}

/// The median of each channel, or `None` from too few pixels.
fn median(channels: [Vec<u8>; 3]) -> Option<[u8; 3]> {
    if channels[0].len() < MIN_SAMPLES {
        return None;
    }
    Some(channels.map(|mut values| {
        let mid = values.len() / 2;
        *values.select_nth_unstable(mid).1
    }))
}

/// Adds the photo's pixels inside `ring` to `channels`, leaving out foliage and deep shadow.
fn seen(photo: &Photo, ring: &[P], source: impl Fn(f64, f64) -> (f64, f64), channels: &mut [Vec<u8>; 3]) {
    let ring = if ring.len() > 1 && ring.first() == ring.last() { &ring[..ring.len() - 1] } else { ring };
    if ring.len() < 3 {
        return;
    }
    let (mut x0, mut z0, mut x1, mut z1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &[x, z] in ring {
        (x0, z0, x1, z1) = (x0.min(x), z0.min(z), x1.max(x), z1.max(z));
    }
    let mut z = z0 + PIXEL / 2.0;
    while z < z1 {
        let mut x = x0 + PIXEL / 2.0;
        while x < x1 {
            if point_in_polygon([x, z], ring) {
                let (e, n) = source(x, z);
                if let Some(pixel) = photo.at(e, n).filter(|&p| !foliage(p) && !shadow(p)) {
                    for (channel, value) in channels.iter_mut().zip(pixel) {
                        channel.push(value);
                    }
                }
            }
            x += PIXEL;
        }
        z += PIXEL;
    }
}

/// Leaves and grass: green over both red and blue. Slate leans blue and tile red, so neither is.
fn foliage([r, g, b]: [u8; 3]) -> bool {
    i32::from(g) > i32::from(r) + 6 && i32::from(g) > i32::from(b) + 4
}

/// Too dark to tell what it is: the shadow of a chimney or the next building.
fn shadow([r, g, b]: [u8; 3]) -> bool {
    r.max(g).max(b) < 40
}

/// The drawn colour of the roof that looks most like `seen` in the photo. Lightness counts for less
/// than hue, since the photo's light changes with the sun and the slope while a roof's material
/// does not.
fn nearest_roof(seen: [u8; 3]) -> u32 {
    let [l, a, b] = lab(seen);
    let distance = |look: [u8; 3]| {
        let [l2, a2, b2] = lab(look);
        0.5 * (l - l2).powi(2) + (a - a2).powi(2) + (b - b2).powi(2)
    };
    ROOF_LOOKS.into_iter().min_by(|x, y| distance(x.0).total_cmp(&distance(y.0))).expect("a palette").1
}

/// A wall colour picked by where the building stands, so the same building gets the same one in
/// the next import.
fn wall(rings: &[&[P]]) -> u32 {
    let n = rings.iter().map(|ring| ring.len()).sum::<usize>().max(1) as f64;
    let [x, z] = rings.iter().flat_map(|ring| ring.iter()).fold([0.0, 0.0], |s, p| [s[0] + p[0] / n, s[1] + p[1] / n]);
    let mut hash = (x.round() as i64 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (z.round() as i64 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 33;
    WALLS[(hash % WALLS.len() as u64) as usize]
}

/// CIELAB (D65) of an sRGB colour.
fn lab(rgb: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(|v| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    });
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| if t > 216.0 / 24389.0 { t.cbrt() } else { (24389.0 / 27.0 * t + 16.0) / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 m photo piece at the origin, 8×8 pixels, all `colour` but a green west half when `tree`.
    fn photo(colour: [u8; 3], tree: bool) -> Photo {
        let pixels = (4.0 / PIXEL) as usize;
        let rgb = (0..pixels * pixels).flat_map(|i| if tree && i % pixels < pixels / 2 { [60, 110, 50] } else { colour }).collect();
        Photo { side: 4.0, pieces: HashMap::from([((0, 0), rgb)]) }
    }

    const SQUARE: [P; 4] = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]];
    /// Local metres are the photo's, z pointing south.
    fn source(x: f64, z: f64) -> (f64, f64) {
        (x, 4.0 - z)
    }

    #[test]
    fn photo_rows_run_from_the_north() {
        let mut photo = photo([0, 0, 0], false);
        photo.pieces.get_mut(&(0, 0)).unwrap()[..3].copy_from_slice(&[1, 2, 3]);
        assert_eq!(photo.at(0.1, 3.9), Some([1, 2, 3]), "the north-west pixel is the first");
        assert_eq!(photo.at(0.1, 0.1), Some([0, 0, 0]));
        assert_eq!(photo.at(-0.1, 1.0), None);
    }

    #[test]
    fn slate_and_tile_roofs_are_told_apart() {
        // Medians of the roofs in a photo of Tours.
        assert_eq!(of(&photo([74, 76, 84], false), &[&SQUARE], source)[1], ROOFS[0], "dark blue-grey is slate");
        assert_eq!(of(&photo([192, 141, 116], false), &[&SQUARE], source)[1], ROOFS[6], "bright red-brown is terracotta");
        assert_eq!(of(&photo([135, 106, 98], false), &[&SQUARE], source)[1], ROOFS[8], "dull red-brown is old tile");
        assert_eq!(of(&photo([211, 214, 208], false), &[&SQUARE], source)[1], ROOFS[5], "near white is a membrane");
    }

    #[test]
    fn the_pieces_of_a_building_are_one_roof() {
        // West half slate, east half tile: two pieces, one roof, the colour of the most pixels.
        let pixels = (4.0 / PIXEL) as usize;
        let rgb = (0..pixels * pixels).flat_map(|i| if i % pixels < 5 { [74, 76, 84] } else { [192, 141, 116] }).collect();
        let photo = Photo { side: 4.0, pieces: HashMap::from([((0, 0), rgb)]) };
        let (west, east): (&[P], &[P]) = (&[[0.0, 0.0], [2.0, 0.0], [2.0, 4.0], [0.0, 4.0]], &[[2.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 4.0]]);
        assert_eq!(of(&photo, &[east], source)[1], ROOFS[6]);
        assert_eq!(of(&photo, &[west, east], source), of(&photo, &[&SQUARE], source));
        assert_eq!(of(&photo, &[west, east], source)[1], ROOFS[0]);
    }

    #[test]
    fn trees_over_a_roof_are_left_out() {
        assert_eq!(of(&photo([192, 141, 116], true), &[&SQUARE], source)[1], ROOFS[6]);
        // All under a tree: the roof was not seen.
        assert_eq!(of(&photo([60, 110, 50], false), &[&SQUARE], source)[1], UNSEEN_ROOF);
    }

    #[test]
    fn a_wall_colour_depends_only_on_where_the_building_stands() {
        assert_eq!(wall(&[&SQUARE]), wall(&[&SQUARE.map(|[x, z]| [x, z])]));
        let walls: std::collections::BTreeSet<u32> = (0..64).map(|i| wall(&[&[[i as f64 * 37.0, 0.0], [i as f64 * 37.0 + 1.0, 1.0], [0.0, 1.0]]])).collect();
        assert!(walls.len() > 4, "walls vary from building to building: {walls:x?}");
    }
}
