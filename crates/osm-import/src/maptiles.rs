//! The map as tiles, the way a web map serves it: a pyramid of squares `TILE_PX` pixels across, level
//! 0 one square over the whole world and each level below it splitting every square in four, down to
//! one square per world tile (`Manifest::tile_size`, `TILE_PX / 256 m` = 2 pixels a metre). The client's
//! minimap and the phone's map fetch the few squares they show, at the level their zoom wants (PHONE.md).
//!
//! Written as `maptiles/{z}/{x}_{y}.webp` next to `world.msgpack`, with `maptiles/index.json` saying
//! how the levels lie (`Index`). A square with no world tile under it is left out: the client draws
//! plain paper there. Every level is drawn from the world's shapes, not shrunk from the one below, so
//! its roads and lines stay crisp.

use crate::minimap::{Overlay, render_square};
use rayon::prelude::*;
use serde::Serialize;
use shared::world::{Manifest, Tile};
use std::collections::{BTreeMap, BTreeSet};

/// Side of a map tile, pixels.
pub const TILE_PX: usize = 512;
/// The WebP quality the tiles are saved at, 0..100.
const QUALITY: f32 = 80.0;
/// The folder the tiles go in, next to `world.msgpack`, and the file that says how they lie.
pub const DIR: &str = "maptiles";
pub const INDEX: &str = "index.json";

/// How the pyramid lies (`maptiles/index.json`), for the client.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Index {
    /// How many levels there are: 0 is the coarsest, `levels - 1` the finest.
    pub levels: u32,
    /// Side of a tile, pixels.
    pub tile_px: u32,
    /// Side of a level-0 tile, metres; each level below halves it.
    pub top_metres: f64,
    /// The world point at the north-west corner of every level, metres `[x, z]`: `-half_extent`.
    pub origin: [f64; 2],
    /// A hash of every tile, for the client to fetch them by (`?h=`), so a new render is fetched afresh.
    pub hash: u32,
}

/// One tile: its level and its place in that level's grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

impl Key {
    /// Where it is written, relative to the directory holding `world.msgpack`.
    pub fn path(&self) -> String {
        format!("{DIR}/{}/{}_{}.webp", self.z, self.x, self.y)
    }
}

/// How the pyramid of `manifest`'s world lies: as many levels as it takes for one square to cover it.
pub fn index(manifest: &Manifest) -> Index {
    let side = f64::from(manifest.tile_size) / 100.0;
    let across = (f64::from(manifest.half_extent) / 100.0 * 2.0 / side).ceil().max(1.0);
    let deepest = across.log2().ceil() as u32;
    let half = f64::from(manifest.half_extent) / 100.0;
    Index { levels: deepest + 1, tile_px: TILE_PX as u32, top_metres: side * f64::from(1u32 << deepest), origin: [-half, -half], hash: 0 }
}

/// Every tile of the pyramid that has a world tile under it (from `present`, the world tiles' `(col, row)`).
pub fn keys(index: &Index, present: &BTreeSet<(u16, u16)>) -> Vec<Key> {
    let deepest = index.levels - 1;
    let mut keys = BTreeSet::new();
    for &(col, row) in present {
        for z in 0..index.levels {
            let shift = deepest - z;
            keys.insert(Key { z, x: u32::from(col) >> shift, y: u32::from(row) >> shift });
        }
    }
    keys.into_iter().collect()
}

/// Renders the whole pyramid: every tile's path and WebP bytes, and the index with its hash.
pub fn render(manifest: &Manifest, tiles: &BTreeMap<(u16, u16), Tile>, overlay: Option<&Overlay>) -> (Index, Vec<(String, Vec<u8>)>) {
    let mut index = index(manifest);
    let present: BTreeSet<(u16, u16)> = tiles.keys().copied().collect();
    let keys = keys(&index, &present);
    let files: Vec<(String, Vec<u8>)> = keys
        .par_iter()
        .map(|key| {
            let metres = index.top_metres / f64::from(1u32 << key.z);
            let origin = [index.origin[0] + f64::from(key.x) * metres, index.origin[1] + f64::from(key.y) * metres];
            let canvas = render_square(manifest, tiles, overlay, origin, metres, TILE_PX);
            (key.path(), canvas.webp_at(QUALITY))
        })
        .collect();
    index.hash = files.iter().fold(0x811c_9dc5u32, |hash, (path, bytes)| shared::world::world_hash(&[&hash.to_le_bytes()[..], path.as_bytes(), bytes].concat()) ^ hash.rotate_left(5));
    (index, files)
}

/// Writes the pyramid into `dir/maptiles`, replacing whatever was there.
pub fn write(dir: &std::path::Path, index: &Index, files: &[(String, Vec<u8>)]) -> std::io::Result<usize> {
    let root = dir.join(DIR);
    if root.exists() {
        std::fs::remove_dir_all(&root)?;
    }
    let mut bytes = 0;
    for (path, data) in files {
        let file = dir.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, data)?;
        bytes += data.len();
    }
    std::fs::write(root.join(INDEX), serde_json::to_string(index).expect("the index serialises"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{Building, ROAD_MAJOR, Road};

    /// A world 1 km across (half 500 m) of 256 m tiles: 4 × 4 of them, so 3 levels.
    fn manifest() -> Manifest {
        Manifest { half_extent: 50_000, tile_size: 25_600, ..Default::default() }
    }

    #[test]
    fn the_levels_go_from_one_square_over_the_world_to_one_per_world_tile() {
        let index = index(&manifest());
        assert_eq!(index.levels, 3);
        assert_eq!(index.top_metres, 1024.0);
        assert_eq!(index.origin, [-500.0, -500.0]);
    }

    #[test]
    fn a_square_with_no_world_tile_under_it_is_left_out() {
        let index = index(&manifest());
        let keys = keys(&index, &BTreeSet::from([(0, 0), (3, 2)]));
        assert_eq!(keys.iter().filter(|key| key.z == 2).count(), 2);
        assert_eq!(keys.iter().filter(|key| key.z == 1).map(|key| (key.x, key.y)).collect::<Vec<_>>(), vec![(0, 0), (1, 1)]);
        assert_eq!(keys.iter().filter(|key| key.z == 0).count(), 1);
    }

    #[test]
    fn a_tile_shows_what_the_whole_map_shows_there_but_sharper() {
        let manifest = manifest();
        let mut tile = Tile::default();
        // A 20 m house at (100 m, -150 m) and a road along x = -200 m.
        tile.buildings.push(Building { footprint: vec![[9_000, -16_000], [11_000, -16_000], [11_000, -14_000], [9_000, -14_000]], ..Default::default() });
        tile.roads.push(Road { width: 800, kind: ROAD_MAJOR, carried: false, points: vec![[-20_000, -45_000], [-20_000, 45_000]], ..Default::default() });
        let tiles = BTreeMap::from([((1, 1), tile)]);
        let whole = crate::minimap::render(&manifest, &tiles, None);
        // The finest tile over (100, -150): level 2's squares are 256 m from (-500, -500).
        let square = render_square(&manifest, &tiles, None, [-500.0 + 2.0 * 256.0, -500.0 + 1.0 * 256.0], 256.0, TILE_PX);
        let colour = |canvas: &crate::minimap::Canvas, [x, z]: [f64; 2], origin: [f64; 2], metres: f64| {
            let k = canvas.size as f64 / metres;
            let (i, j) = (((x - origin[0]) * k) as usize, ((z - origin[1]) * k) as usize);
            canvas.pixels[(j * canvas.size + i) * 3..][..3].to_vec()
        };
        let tile_origin = [12.0, -244.0];
        for at in [[100.0, -150.0], [0.0, -100.0]] {
            assert_eq!(colour(&square, at, tile_origin, 256.0), colour(&whole, at, [-500.0, -500.0], 1000.0), "{at:?}");
        }
        // The road lies outside this square's own world tile, a tile to its west, and is still drawn.
        let west = render_square(&manifest, &tiles, None, [-500.0 + 256.0, -500.0 + 256.0], 256.0, TILE_PX);
        assert_eq!(colour(&west, [-200.0, -150.0], [-244.0, -244.0], 256.0), colour(&whole, [-200.0, -150.0], [-500.0, -500.0], 1000.0));
    }

    #[test]
    fn the_pyramid_is_written_with_its_index() {
        let manifest = manifest();
        let tiles = BTreeMap::from([((0, 0), Tile::default()), ((3, 3), Tile::default())]);
        let (index, files) = render(&manifest, &tiles, None);
        assert_ne!(index.hash, 0);
        let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, ["maptiles/0/0_0.webp", "maptiles/1/0_0.webp", "maptiles/1/1_1.webp", "maptiles/2/0_0.webp", "maptiles/2/3_3.webp"]);
        assert!(files.iter().all(|(_, bytes)| bytes.starts_with(b"RIFF")));
    }
}
