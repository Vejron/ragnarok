//! Which species the forest's trees are, from Naturvårdsverket's NMD2023 species layers (CC0, "NMD,
//! Naturvårdsverket"): one 10 m GeoTIFF per species in SWEREF 99 TM, each pixel the share of the crown
//! cover, 0..=100, with 255 for what is not forest.
//!
//! The raster knows a stand's mix, not one tree's species, and one pixel is off by some 30 points
//! (SLU's evaluation). So it is read in blocks of 3 × 3 pixels: the block says how many of its trees
//! are of each species, and the laser says which ones, by rank among themselves. A rank needs no fixed
//! threshold, so unlike `canopy::classify` it does not depend on the flight the laser data came from.
//!
//! The layers are unpacked from the download into one directory; see docs/WORLD.md.

use crate::canopy::{self, Detected};
use anyhow::{Context, Result};
use shared::world::*;
use std::collections::BTreeMap;
use std::path::Path;
use tiff::decoder::{Decoder, DecodingResult};

/// The north-west corner of every layer, SWEREF 99 TM.
const WEST: f64 = 240_000.0;
const NORTH: f64 = 7_671_050.0;
/// Side of a pixel, metres.
const PIXEL: f64 = 10.0;
/// Pixels to a side of a block.
const BLOCK: i64 = 3;
/// A block with fewer forest pixels is not a stand: its trees are the laser's to name.
const MIN_PIXELS: u32 = 3;
const NOT_FOREST: u8 = 255;

const FILES: [&str; 5] = ["tall", "gran", "trivial", "adel", "lark"];

/// One pixel's layers, in the order of `FILES`.
type Pixel = [u8; 5];

/// A block's mix, in points that sum to about 100.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shares {
    pub pine: f64,
    /// With the larch, which the layers count as pine but `trees::classify` as spruce.
    pub spruce: f64,
    /// Birch, aspen, alder, rowan: the layers do not tell them apart.
    pub deciduous: f64,
    /// Oak, beech and the other noble broadleaves.
    pub noble: f64,
}

pub struct Species {
    /// The window's first column and row of the full raster.
    col0: i64,
    row0: i64,
    cols: usize,
    rows: usize,
    layers: [Vec<u8>; 5],
}

impl Species {
    /// Reads `window`, widened to whole blocks, from the layers in `dir`.
    pub fn open(dir: &Path, [e0, n0, e1, n1]: crate::dem::Window) -> Result<Self> {
        let (first, last) = (block(e0, n1), block(e1, n0));
        let (col0, row0) = (first.0 * BLOCK, first.1 * BLOCK);
        let (col1, row1) = ((last.0 + 1) * BLOCK, (last.1 + 1) * BLOCK);
        let mut layers: [Vec<u8>; 5] = Default::default();
        for (layer, name) in layers.iter_mut().zip(FILES) {
            let path = dir.join(format!("NMD2023_tradslag_{name}_v1_1.tif"));
            let file = std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
            *layer = read_window(std::io::BufReader::new(file), col0, row0, col1, row1).with_context(|| format!("reading {}", path.display()))?;
        }
        Ok(Self { col0, row0, cols: (col1 - col0) as usize, rows: (row1 - row0) as usize, layers })
    }

    fn pixel(&self, col: i64, row: i64) -> Option<Pixel> {
        let (c, r) = (col - self.col0, row - self.row0);
        if c < 0 || r < 0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        let i = r as usize * self.cols + c as usize;
        Some(self.layers.each_ref().map(|layer| layer[i]))
    }

    /// The mix of a block, or `None` if too little of it is forest.
    pub fn shares(&self, (x, y): (i64, i64)) -> Option<Shares> {
        let (mut sum, mut pixels) = (Shares { pine: 0.0, spruce: 0.0, deciduous: 0.0, noble: 0.0 }, 0);
        for row in y * BLOCK..(y + 1) * BLOCK {
            for col in x * BLOCK..(x + 1) * BLOCK {
                let Some(pixel) = self.pixel(col, row).and_then(forest) else { continue };
                sum.pine += pixel.pine;
                sum.spruce += pixel.spruce;
                sum.deciduous += pixel.deciduous;
                sum.noble += pixel.noble;
                pixels += 1;
            }
        }
        (pixels >= MIN_PIXELS).then(|| {
            let n = f64::from(pixels);
            Shares { pine: sum.pine / n, spruce: sum.spruce / n, deciduous: sum.deciduous / n, noble: sum.noble / n }
        })
    }
}

/// A pixel's mix, or `None` if it is not forest or the model had no answer for it (all layers 0).
///
/// 255 means "not forest" only in the layers made everywhere. `adel` and `lark` were made only where
/// those trees grow and have 255 on forest elsewhere (the product description says 0): none of them.
fn forest([tall, gran, trivial, adel, lark]: Pixel) -> Option<Shares> {
    if [tall, gran, trivial].contains(&NOT_FOREST) {
        return None;
    }
    let or_none = |value: u8| if value == NOT_FOREST { 0 } else { value };
    let (adel, lark) = (or_none(adel), or_none(lark).min(tall));
    if u16::from(tall) + u16::from(gran) + u16::from(trivial) + u16::from(adel) == 0 {
        return None;
    }
    Some(Shares { pine: f64::from(tall - lark), spruce: f64::from(gran) + f64::from(lark), deciduous: f64::from(trivial), noble: f64::from(adel) })
}

fn read_window<R: std::io::Read + std::io::Seek>(source: R, col0: i64, row0: i64, col1: i64, row1: i64) -> Result<Vec<u8>> {
    let mut decoder = Decoder::new(source).context("not a readable TIFF")?;
    let (width, height) = decoder.dimensions()?;
    anyhow::ensure!(decoder.chunk_dimensions() == (width, 1), "expected one row per strip");
    let mut out = Vec::with_capacity(((col1 - col0) * (row1 - row0)) as usize);
    for row in row0..row1 {
        let mut pixels = [NOT_FOREST].repeat((col1 - col0) as usize);
        if row >= 0 && row < i64::from(height) {
            let DecodingResult::U8(line) = decoder.read_chunk(row as u32).with_context(|| format!("decoding row {row}"))? else {
                anyhow::bail!("expected 8-bit pixels");
            };
            let (first, last) = (col0.max(0), col1.min(i64::from(width)));
            if first < last {
                pixels[(first - col0) as usize..(last - col0) as usize].copy_from_slice(&line[first as usize..last as usize]);
            }
        }
        out.extend(pixels);
    }
    Ok(out)
}

/// The block at SWEREF 99 TM `(e, n)`, counted east and south from the raster's corner.
pub fn block(e: f64, n: f64) -> (i64, i64) {
    let side = BLOCK as f64 * PIXEL;
    (((e - WEST) / side).floor() as i64, ((NORTH - n) / side).floor() as i64)
}

/// How many of `trees` trees are pine, spruce, deciduous and noble: the shares in whole trees, each
/// leftover to the largest remainder.
fn quota(shares: &Shares, trees: usize) -> [usize; 4] {
    let parts = [shares.pine, shares.spruce, shares.deciduous, shares.noble];
    let total: f64 = parts.iter().sum();
    let exact = parts.map(|part| part / total * trees as f64);
    let mut counts = exact.map(|value| value.floor() as usize);
    let mut order = [0, 1, 2, 3];
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    let left = trees - counts.iter().sum::<usize>();
    for &i in order.iter().take(left) {
        counts[i] += 1;
    }
    counts
}

/// The archetype of each of `trees`, or `None` where `shares` has no mix for its block.
///
/// Leaves return less of the laser's near-infrared than needles, so a block's dimmest trees are its
/// broadleaves, the widest of them the noble ones. Pine's crown starts higher up the stem than
/// spruce's, so of the conifers the highest bases are the pines.
pub fn assign(trees: &[Detected], shares: impl Fn((i64, i64)) -> Option<Shares>) -> Vec<Option<u8>> {
    let mut blocks: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, tree) in trees.iter().enumerate() {
        blocks.entry(block(tree.e, tree.n)).or_default().push(i);
    }
    // Every ranking falls back on the position, so a re-import gives the same forest.
    let place = |a: usize, b: usize| trees[a].e.total_cmp(&trees[b].e).then(trees[a].n.total_cmp(&trees[b].n));
    let mut kinds = vec![None; trees.len()];
    for (at, mut members) in blocks {
        let Some(shares) = shares(at) else { continue };
        let [pine, _, deciduous, noble] = quota(&shares, members.len());
        members.sort_by(|&a, &b| trees[a].intensity.total_cmp(&trees[b].intensity).then(place(a, b)));
        let (leaves, needles) = members.split_at_mut(deciduous + noble);
        leaves.sort_by(|&a, &b| trees[b].crown.total_cmp(&trees[a].crown).then(place(a, b)));
        needles.sort_by(|&a, &b| trees[b].base.total_cmp(&trees[a].base).then(place(a, b)));
        for (rank, &i) in leaves.iter().enumerate() {
            kinds[i] = Some(if rank < noble { TREE_BROADLEAF } else { canopy::deciduous(&trees[i]) });
        }
        for (rank, &i) in needles.iter().enumerate() {
            kinds[i] = Some(if rank < pine { TREE_PINE } else { TREE_SPRUCE });
        }
    }
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tree in block (10, 10), `at` metres from its corner, with a 12 m birch's shape.
    fn tree(at: f64, intensity: f64, base: f64) -> Detected {
        Detected { e: WEST + 300.0 + at, n: NORTH - 300.0 - at, height: 12.0, crown: 3.0, multi: 0.8, intensity, base }
    }

    fn mix(pine: f64, spruce: f64, deciduous: f64, noble: f64) -> Shares {
        Shares { pine, spruce, deciduous, noble }
    }

    /// One block of the same pixel, plus any `others` over its first pixels.
    fn raster(pixel: Pixel, others: &[Pixel]) -> Species {
        let mut pixels = [pixel; 9];
        pixels[..others.len()].copy_from_slice(others);
        let layers = std::array::from_fn(|layer| pixels.iter().map(|p| p[layer]).collect());
        Species { col0: 30, row0: 30, cols: 3, rows: 3, layers }
    }

    #[test]
    fn a_tree_lands_in_the_block_it_stands_in() {
        assert_eq!(block(WEST + 0.5, NORTH - 0.5), (0, 0));
        assert_eq!(block(WEST + 30.5, NORTH - 59.5), (1, 1));
    }

    #[test]
    fn the_mix_is_the_mean_of_the_forest_pixels() {
        let (pine, spruce) = ([100, 0, 0, 0, 0], [0, 100, 0, 0, 0]);
        let shares = raster(pine, &[spruce, spruce, spruce]).shares((10, 10)).unwrap();
        assert!((shares.pine - 600.0 / 9.0).abs() < 1e-9 && (shares.spruce - 300.0 / 9.0).abs() < 1e-9, "{shares:?}");
    }

    #[test]
    fn two_hundred_fifty_five_is_no_forest_only_in_the_main_layers() {
        // The noble layers' "not made here", on real forest.
        assert_eq!(raster([60, 30, 10, 255, 255], &[]).shares((10, 10)), Some(mix(60.0, 30.0, 10.0, 0.0)));
        // Not forest.
        assert_eq!(raster([255, 255, 255, 255, 255], &[]).shares((10, 10)), None);
    }

    #[test]
    fn a_pixel_the_model_gave_up_on_does_not_count() {
        assert_eq!(forest([0, 0, 0, 0, 0]), None);
        assert_eq!(raster([0, 0, 0, 0, 0], &[[100, 0, 0, 0, 0]; 2]).shares((10, 10)), None, "two pixels are no stand");
        assert_eq!(raster([0, 0, 0, 0, 0], &[[100, 0, 0, 0, 0]; 3]).shares((10, 10)), Some(mix(100.0, 0.0, 0.0, 0.0)));
    }

    #[test]
    fn a_pixel_outside_the_window_is_no_forest() {
        assert_eq!(raster([100, 0, 0, 0, 0], &[]).shares((11, 10)), None);
    }

    #[test]
    fn larch_is_counted_as_spruce_and_out_of_the_pine() {
        assert_eq!(raster([70, 20, 10, 0, 30], &[]).shares((10, 10)), Some(mix(40.0, 50.0, 10.0, 0.0)));
        // Never more larch than pine.
        assert_eq!(raster([20, 70, 10, 0, 30], &[]).shares((10, 10)), Some(mix(0.0, 90.0, 10.0, 0.0)));
    }

    #[test]
    fn quotas_follow_the_shares_and_add_up() {
        assert_eq!(quota(&mix(50.0, 30.0, 20.0, 0.0), 10), [5, 3, 2, 0]);
        assert_eq!(quota(&mix(76.0, 14.0, 10.0, 0.0), 1), [1, 0, 0, 0]);
        for trees in 0..40 {
            assert_eq!(quota(&mix(33.3, 33.3, 33.3, 0.1), trees).iter().sum::<usize>(), trees);
        }
    }

    #[test]
    fn the_dimmest_trees_are_the_broadleaves_and_the_highest_bases_the_pines() {
        let trees = [tree(1.0, 900.0, 0.3), tree(2.0, 500.0, 0.3), tree(3.0, 800.0, 0.6), tree(4.0, 950.0, 0.2)];
        let kinds = assign(&trees, |_| Some(mix(25.0, 50.0, 25.0, 0.0)));
        assert_eq!(kinds, [Some(TREE_SPRUCE), Some(TREE_BIRCH), Some(TREE_PINE), Some(TREE_SPRUCE)]);
    }

    #[test]
    fn the_widest_broadleaf_is_the_noble_one() {
        let (mut wide, narrow) = (tree(1.0, 500.0, 0.3), tree(2.0, 400.0, 0.3));
        wide.crown = 5.0;
        assert_eq!(assign(&[wide, narrow], |_| Some(mix(0.0, 0.0, 50.0, 50.0))), [Some(TREE_BROADLEAF), Some(TREE_BIRCH)]);
    }

    #[test]
    fn the_shape_picks_the_deciduous_archetype_and_a_young_conifer_stays_one() {
        let mut low = tree(1.0, 900.0, 0.3);
        low.height = 5.0;
        assert_eq!(assign(&[low], |_| Some(mix(0.0, 0.0, 100.0, 0.0))), [Some(TREE_LOWWIDE)]);
        assert_eq!(assign(&[low], |_| Some(mix(100.0, 0.0, 0.0, 0.0))), [Some(TREE_PINE)]);
    }

    #[test]
    fn a_block_without_a_mix_is_left_to_the_laser() {
        assert_eq!(assign(&[tree(1.0, 900.0, 0.3)], |_| None), [None]);
    }

    #[test]
    fn the_input_order_does_not_change_the_forest() {
        let trees = [tree(1.0, 700.0, 0.4), tree(2.0, 700.0, 0.4), tree(3.0, 700.0, 0.4)];
        let reversed: Vec<Detected> = trees.iter().rev().copied().collect();
        let shares = |_| Some(mix(40.0, 30.0, 30.0, 0.0));
        let mut forward = assign(&trees, shares);
        forward.reverse();
        assert_eq!(forward, assign(&reversed, shares));
    }

    /// The real layers over a world, against the mix measured when this was built:
    /// `cargo test --release -p osm-import nmd_mix -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn nmd_mix() {
        #[derive(serde::Deserialize)]
        struct Area {
            tiles: Vec<crate::grid::Tile>,
        }
        let text = std::fs::read_to_string("../../worlds/kulla/area.json").unwrap();
        let area: Area = serde_json::from_str(&text).unwrap();
        let region = crate::region::Region::Sweden;
        let centres: Vec<(f64, f64)> = area.tiles.iter().map(|tile| {
            let [south, west, north, east] = tile.bbox(region);
            crate::dem::sweref99tm((south + north) / 2.0, (west + east) / 2.0)
        }).collect();
        let bound = |pick: fn(f64, f64) -> f64, init, axis: fn(&(f64, f64)) -> f64| centres.iter().map(axis).fold(init, pick);
        let window = [bound(f64::min, f64::MAX, |c| c.0) - 200.0, bound(f64::min, f64::MAX, |c| c.1) - 200.0, bound(f64::max, f64::MIN, |c| c.0) + 200.0, bound(f64::max, f64::MIN, |c| c.1) + 200.0];
        let species = Species::open(Path::new("../../temp-maps/nmd2023"), window).unwrap();
        let (mut sum, mut blocks) = ([0.0; 4], 0.0);
        for (e, n) in centres {
            for dx in -4..4 {
                for dy in -4..4 {
                    if let Some(s) = species.shares(block(e + f64::from(dx) * 30.0, n + f64::from(dy) * 30.0)) {
                        for (total, part) in sum.iter_mut().zip([s.pine, s.spruce, s.deciduous, s.noble]) {
                            *total += part;
                        }
                        blocks += 1.0;
                    }
                }
            }
        }
        let mean = sum.map(|part| part / blocks);
        eprintln!("kulla: {blocks} forest blocks, pine {:.1} spruce {:.1} deciduous {:.1} noble {:.1}", mean[0], mean[1], mean[2], mean[3]);
        assert!((70.0..=82.0).contains(&mean[0]), "{mean:?}");
        assert!(mean[3] < 0.5, "the noble layers' 255s were read as shares: {mean:?}");
    }

    /// A built world's trees by kind, and a hash of where they stand that a change of species keeps:
    /// `WORLD=kulla cargo test --release -p osm-import world_trees -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn world_trees() {
        let name = std::env::var("WORLD").unwrap();
        let dir = std::path::PathBuf::from("../../worlds").join(&name);
        let manifest = Manifest::decode(&std::fs::read(dir.join("world.msgpack")).unwrap()).unwrap();
        let (mut kinds, mut places) = ([0usize; 10], 0u64);
        for entry in &manifest.tiles {
            let tile = Tile::decode(&std::fs::read(dir.join(entry.path())).unwrap()).unwrap();
            for tree in &tile.trees {
                kinds[usize::from(tree.kind).min(9)] += 1;
                // Summed, so the order they are stored in does not matter.
                places = places.wrapping_add((u64::from(tree.x as u32) << 32 | u64::from(tree.z as u32)).wrapping_mul(0x9e37_79b9_7f4a_7c15));
            }
        }
        eprintln!("{name}: birch, pine, spruce, ornamental, broadleaf, columnar, lowwide, rocks, rocks, bushes {kinds:?}; places {places:016x}");
    }
}
