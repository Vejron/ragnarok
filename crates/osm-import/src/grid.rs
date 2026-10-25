//! The fixed global tile grid every world is painted on.
//!
//! The grid is anchored at one reference point (`ORIGIN`) and cut into 256 m tiles with signed
//! indices `(gx, gy)`: +X east, +Z south, as in the world's own frame. Tiles are grouped in
//! `BLOCK_TILES` x `BLOCK_TILES` blocks (about 2 km), and downloads are made per block. Because blocks
//! sit at fixed places, a request's cache key depends only on the block and never on which world
//! asked for it: two worlds that share a block share its cache entries, and a block's state can be
//! probed without downloading anything.
//!
//! Each region has a grid of its own (`Region::grid`): Sweden's is anchored here and cut in the
//! equirectangular frame, France's is cut in Lambert-93 metres. A tile index means nothing without
//! its region.

use crate::region::{Place, Region, enclosing};
use crate::{assemble, cache, hydro, overpass, trees};
use serde::{Deserialize, Serialize};
use shared::world::Frame;

/// Sweden's grid anchor: central Umeå, the importer's default centre.
pub const ORIGIN: (f64, f64) = (63.8225, 20.235);
/// Side of a tile, metres. A power of two, so a tile sampled every 1, 2 or 4 m is `2^k + 1` samples a
/// side: the size the client's terrain simplification (RTIN) fits exactly.
pub const TILE: f64 = 256.0;
/// Tiles along one side of a download block.
pub const BLOCK_TILES: i32 = 8;

/// A tile of the global grid.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Tile {
    pub gx: i32,
    pub gy: i32,
}

/// A download block of the global grid, in block units.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Block {
    pub bx: i32,
    pub by: i32,
}

/// What the cache holds for one block.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Status {
    pub overpass: bool,
    pub hydro: bool,
    pub trees: bool,
}

impl Status {
    /// Enough to build the block's tiles without the network: the map data itself. Water and trees
    /// are optional extras, so a block without them still counts.
    pub fn ready(self) -> bool {
        self.overpass
    }
}

impl Tile {
    /// Metres of the tile's north-west corner, in the grid's frame.
    pub fn corner(self) -> (f64, f64) {
        (f64::from(self.gx) * TILE, f64::from(self.gy) * TILE)
    }

    /// The tile of `region`'s grid containing the point `(lat, lon)`.
    pub fn at(region: Region, lat: f64, lon: f64) -> Tile {
        let [x, z] = region.grid().project(lat, lon);
        Tile { gx: (x / TILE).floor() as i32, gy: (z / TILE).floor() as i32 }
    }

    pub fn block(self) -> Block {
        Block { bx: self.gx.div_euclid(BLOCK_TILES), by: self.gy.div_euclid(BLOCK_TILES) }
    }

    /// `[south, west, north, east]` of the tile.
    pub fn bbox(self, region: Region) -> [f64; 4] {
        rect(region, self.gx, self.gy, 1)
    }
}

impl Block {
    /// `[south, west, north, east]` of the block, rounded so the same block always yields the same
    /// text (and so the same cache key).
    pub fn bbox(self, region: Region) -> [f64; 4] {
        rect(region, self.bx * BLOCK_TILES, self.by * BLOCK_TILES, BLOCK_TILES)
    }

    /// The block's cache state, from the keys the downloads would use. Hydrografi and the tree
    /// register are Swedish: a French block never has them.
    pub fn status(self, region: Region) -> Status {
        let bbox = self.bbox(region);
        let swedish = region == Region::Sweden;
        Status {
            overpass: cache::has("overpass", &overpass::query(bbox)),
            hydro: swedish && cache::has("hydro", &hydro::first_url("StandingWater", bbox)),
            trees: swedish && cache::has("trees", &trees::filter(bbox)),
        }
    }
}

/// The lat/lon bounding box of `size` x `size` tiles whose corner tile is `(gx, gy)`.
fn rect(region: Region, gx: i32, gy: i32, size: i32) -> [f64; 4] {
    let (x0, z0) = (f64::from(gx) * TILE, f64::from(gy) * TILE);
    let side = f64::from(size) * TILE;
    let grid = region.grid();
    if let Frame::Lambert93 { e0, n0 } = grid {
        // Not a rectangle of degrees: the box around the square, sides and all.
        let half = side / 2.0;
        return enclosing(Frame::Lambert93 { e0: e0 + x0 + half, n0: n0 - z0 - half }, half);
    }
    let corners = [(x0, z0), (x0 + side, z0), (x0, z0 + side), (x0 + side, z0 + side)].map(|(x, z)| grid.unproject(x, z));
    let round = |v: f64| (v * 1e6).round() / 1e6;
    let lats = corners.map(|c| c.0);
    let lons = corners.map(|c| c.1);
    let (south, north) = (lats.iter().copied().fold(f64::MAX, f64::min), lats.iter().copied().fold(f64::MIN, f64::max));
    let (west, east) = (lons.iter().copied().fold(f64::MAX, f64::min), lons.iter().copied().fold(f64::MIN, f64::max));
    [round(south), round(west), round(north), round(east)]
}

/// How a set of painted tiles becomes a world: the smallest square of tiles that holds them, laid
/// out in the world's own frame.
pub struct Layout {
    /// Where the square is: its frame (origin at its centre), half-side and box.
    pub place: Place,
    /// The painted tiles as world `(col, row)`.
    pub mask: std::collections::BTreeSet<(u16, u16)>,
    /// The blocks the painted tiles lie in, as `[south, west, north, east]`: what to download.
    pub pieces: Vec<[f64; 4]>,
}

/// The largest world side, in tiles: 30 km.
pub const MAX_SIDE: i32 = 120;

/// Lays out `tiles` of `region`'s grid as a world. The square is centred on the painted tiles'
/// bounding box, the world's origin is the square's centre, and every tile edge lies on the grid.
pub fn layout(region: Region, tiles: &[Tile]) -> anyhow::Result<Layout> {
    anyhow::ensure!(!tiles.is_empty(), "the area has no tiles");
    let min = |f: fn(&Tile) -> i32| tiles.iter().map(f).min().unwrap_or(0);
    let max = |f: fn(&Tile) -> i32| tiles.iter().map(f).max().unwrap_or(0);
    let (width, height) = (max(|t| t.gx) - min(|t| t.gx) + 1, max(|t| t.gy) - min(|t| t.gy) + 1);
    let side = width.max(height);
    anyhow::ensure!(side <= MAX_SIDE, "the area is {side} tiles across; at most {MAX_SIDE} ({} km)", f64::from(MAX_SIDE) * TILE / 1000.0);
    let (sx, sy) = (min(|t| t.gx) - (side - width) / 2, min(|t| t.gy) - (side - height) / 2);
    let half = f64::from(side) * TILE / 2.0;
    let centre = ((f64::from(sx) + f64::from(side) / 2.0) * TILE, (f64::from(sy) + f64::from(side) / 2.0) * TILE);
    let place = match region.grid() {
        // Exact: the world's frame is the grid's, moved to the square's centre.
        Frame::Lambert93 { e0, n0 } => Place::france(Frame::Lambert93 { e0: e0 + centre.0, n0: n0 - centre.1 }, half),
        grid => {
            let origin = grid.unproject(centre.0, centre.1);
            Place::sweden(assemble::bbox_around(origin.0, origin.1, half))
        }
    };
    let mask = tiles.iter().map(|t| ((t.gx - sx) as u16, (t.gy - sy) as u16)).collect();
    let pieces = blocks_of(tiles).into_iter().map(|b| b.bbox(region)).collect();
    Ok(Layout { place, mask, pieces })
}

/// The distinct blocks that `tiles` need, in order.
pub fn blocks_of(tiles: &[Tile]) -> Vec<Block> {
    let mut blocks: Vec<Block> = tiles.iter().map(|t| t.block()).collect();
    blocks.sort();
    blocks.dedup();
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_origin_is_the_corner_of_four_tiles() {
        assert_eq!(Tile::at(Region::Sweden, ORIGIN.0 + 1e-6, ORIGIN.1 + 1e-6), Tile { gx: 0, gy: -1 });
        assert_eq!(Tile::at(Region::Sweden, ORIGIN.0 - 1e-6, ORIGIN.1 - 1e-6), Tile { gx: -1, gy: 0 });
    }

    #[test]
    fn a_tile_lies_inside_its_own_bbox() {
        for region in [Region::Sweden, Region::France] {
            let tile = Tile { gx: 7, gy: -13 };
            let [s, w, n, e] = tile.bbox(region);
            let (x, z) = tile.corner();
            let (lat, lon) = region.grid().unproject(x + TILE / 2.0, z + TILE / 2.0);
            assert!(s < lat && lat < n && w < lon && lon < e);
            assert_eq!(Tile::at(region, lat, lon), tile);
        }
    }

    #[test]
    fn tiles_group_into_blocks_with_negative_indices() {
        assert_eq!(Tile { gx: 7, gy: 0 }.block(), Block { bx: 0, by: 0 });
        assert_eq!(Tile { gx: 8, gy: -1 }.block(), Block { bx: 1, by: -1 });
        assert_eq!(Tile { gx: -1, gy: -8 }.block(), Block { bx: -1, by: -1 });
    }

    #[test]
    fn a_painted_area_is_a_square_in_the_same_frame() {
        // An L: three tiles in a row and one below the first.
        let tiles = [Tile { gx: 4, gy: 10 }, Tile { gx: 5, gy: 10 }, Tile { gx: 6, gy: 10 }, Tile { gx: 4, gy: 11 }];
        let layout = layout(Region::Sweden, &tiles).unwrap();
        assert_eq!(layout.place.half, 1.5 * TILE);
        let [north, west] = [tiles[0].bbox(Region::Sweden)[2], tiles[0].bbox(Region::Sweden)[1]];
        // The square's corner is the first painted tile's corner, on the global grid.
        let [x, z] = layout.place.frame.project(north, west);
        assert!((x + 1.5 * TILE).abs() < 0.2 && (z + 1.5 * TILE).abs() < 0.2, "{x} {z}");
        assert_eq!(layout.mask.iter().copied().collect::<Vec<_>>(), vec![(0, 0), (0, 1), (1, 0), (2, 0)]);
    }

    #[test]
    fn a_french_area_lies_exactly_on_its_grid() {
        let tiles = [Tile { gx: -400, gy: -300 }, Tile { gx: -398, gy: -300 }];
        let layout = layout(Region::France, &tiles).unwrap();
        assert_eq!(layout.place.half, 1.5 * TILE);
        // The first tile's north-west corner, through degrees and back: the square's west edge, its
        // middle row (the square is three tiles a side, the area one tile high).
        let (x, z) = tiles[0].corner();
        let (lat, lon) = Region::France.grid().unproject(x, z);
        let [x, z] = layout.place.frame.project(lat, lon);
        assert!((x + 1.5 * TILE).abs() < 1e-6 && (z + 0.5 * TILE).abs() < 1e-6, "{x} {z}");
    }

    #[test]
    fn a_block_bbox_does_not_depend_on_the_request() {
        let block = Block { bx: -2, by: 3 };
        assert_eq!(block.bbox(Region::Sweden), block.bbox(Region::Sweden));
        let tiles = [Tile { gx: -16, gy: 24 }, Tile { gx: -9, gy: 31 }, Tile { gx: -10, gy: 25 }];
        assert_eq!(blocks_of(&tiles), vec![block]);
    }
}
