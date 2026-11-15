//! Tombstones in the graveyards.
//!
//! OpenStreetMap maps a cemetery or a churchyard as an area and no stone in it, so they are stood
//! there deterministically: the same input always gives the same world. Where paths run through the
//! yard the stones stand in rows along both sides of each, facing it. A yard with no path worth the
//! name is filled with rows in pairs, back to back, with an aisle between each pair, turned to the
//! yard's longest side. No stone stands on a road, in a building, against a tree, in water or closer
//! to another than a plot is wide, and here and there a plot is left empty.

use crate::assemble::{Built, distance_to_segment, point_in_polygon};
use crate::props::{bounds, inside, mix, unit};
use shared::terrain::Ground;
use shared::world::{GRAVE_CROSS, GRAVE_HEADSTONE, GRAVE_LOW, Grave, NO_WATER, TREE_LOWWIDE, TREE_ROCK_BOULDER, TREE_ROCK_SLAB};

type P = [f64; 2];

/// A road or path near a yard: its centre line and half its width, metres.
pub type Way = (Vec<P>, f64);
/// A stone to stand: where, the way its face looks (radians, `Grave::yaw`) and its `GRAVE_*`.
pub type Stone = (P, f64, u8);

/// From one stone to the next along a row, metres.
const PITCH: f64 = 2.8;
/// From a path's edge to its first row of stones, metres.
const VERGE: f64 = 1.2;
/// From one row to the next behind it, away from a path, metres.
const ROW_GAP: f64 = 3.2;
/// Rows of stones on each side of a path.
const ROWS: usize = 3;
/// Rows without a path: between the two rows of a pair, back to back, and the aisle between pairs, metres.
const BACK_GAP: f64 = 2.2;
const AISLE: f64 = 3.6;
/// The least of a yard's paths, metres of them inside it, for its stones to follow them.
const PATHS_MIN: f64 = 15.0;
/// A stone stands this far inside the yard's edge, and off the edge of every road, metres.
const EDGE_CLEAR: f64 = 4.0;
const ROAD_CLEAR: f64 = 0.8;
/// No two stones are closer than this, metres.
const PLOT: f64 = 2.0;
/// A stone stands this far from a tree's trunk, metres.
const TRUNK_CLEAR: f64 = 1.0;
/// One plot in this many is left empty.
const EMPTY_ONE_IN: f64 = 6.0;
/// How far a stone is turned off its row, radians either way: they have settled over the years.
const WOBBLE: f64 = 0.06;
/// Most stones in one yard.
const MAX_PER_YARD: usize = 2000;
/// A stone stands this far inside the world's edge, metres.
const WORLD_MARGIN: f64 = 2.0;

/// The plots taken so far, across every yard: a cemetery is often mapped twice, or holds a churchyard.
#[derive(Default)]
pub struct Plots(std::collections::HashMap<(i64, i64), Vec<P>>);

impl Plots {
    fn cell(at: P) -> (i64, i64) {
        ((at[0] / PLOT).floor() as i64, (at[1] / PLOT).floor() as i64)
    }

    /// Whether a plot at `at` would have room.
    fn free(&self, at: P) -> bool {
        let (cx, cz) = Self::cell(at);
        (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (cx + dx, cz + dz))).filter_map(|key| self.0.get(&key)).flatten().all(|p| (p[0] - at[0]).hypot(p[1] - at[1]) >= PLOT)
    }

    fn take(&mut self, at: P) {
        self.0.entry(Self::cell(at)).or_default().push(at);
    }
}

/// How far `at` is from the nearest edge of `rings`, metres.
fn to_edge(rings: &[Vec<P>], at: P) -> f64 {
    rings.iter().filter(|ring| ring.len() > 2).flat_map(|ring| (0..ring.len()).map(move |i| distance_to_segment(at, ring[i], ring[(i + 1) % ring.len()]))).fold(f64::MAX, f64::min)
}

/// The stones of the graveyard `yard` (rings filled by the even-odd rule, metres). `ways` are the roads
/// and paths about it: the stones follow those that run through it, and stand on none. `clear` is
/// whatever else must allow a stone (no building, tree or water there), and `plots` the plots taken.
pub fn layout(yard: &[Vec<P>], ways: &[Way], clear: impl Fn(P) -> bool, plots: &mut Plots) -> Vec<Stone> {
    let mut stones = Vec::new();
    let mut place = |at: P, facing: P, plots: &mut Plots| {
        let fits = stones.len() < MAX_PER_YARD
            && inside(yard, at)
            && to_edge(yard, at) >= EDGE_CLEAR
            && ways.iter().all(|(line, half)| line.windows(2).all(|pair| distance_to_segment(at, pair[0], pair[1]) >= half + ROAD_CLEAR))
            && plots.free(at)
            && clear(at);
        if !fits {
            return;
        }
        plots.take(at);
        let hash = mix(((at[0] * 100.0).round() as i64 as u64) << 32 ^ ((at[1] * 100.0).round() as i64 as u64 & 0xffff_ffff));
        if unit(hash) < 1.0 / EMPTY_ONE_IN {
            return;
        }
        let pick = unit(mix(hash ^ 1));
        let kind = if pick < 0.55 { GRAVE_HEADSTONE } else if pick < 0.75 { GRAVE_CROSS } else { GRAVE_LOW };
        let yaw = facing[0].atan2(facing[1]) + (unit(mix(hash ^ 2)) - 0.5) * 2.0 * WOBBLE;
        stones.push((at, yaw, kind));
    };
    // Each path's points a stone apart, where it is in the yard, with the way it runs there.
    let mut along: Vec<(P, P, f64)> = Vec::new();
    let mut length = 0.0;
    for (line, half) in ways {
        let points = crate::bridge::resample(line, PITCH);
        let within: Vec<bool> = points.iter().map(|&p| inside(yard, p)).collect();
        for (i, &at) in points.iter().enumerate().filter(|(i, _)| within[*i]) {
            let (a, b) = (points[i.saturating_sub(1)], points[(i + 1).min(points.len() - 1)]);
            let run = (b[0] - a[0]).hypot(b[1] - a[1]);
            if run > 0.0 {
                along.push((at, [(b[0] - a[0]) / run, (b[1] - a[1]) / run], *half));
            }
        }
        length += points.windows(2).zip(within.windows(2)).filter(|(_, within)| within[0] && within[1]).map(|(pair, _)| (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1])).sum::<f64>();
    }
    if length >= PATHS_MIN {
        // The rows nearest the paths first, so that none is crowded out by a row further back.
        for row in 0..ROWS {
            for &(at, way, half) in &along {
                for side in [1.0, -1.0] {
                    let out = half + VERGE + row as f64 * ROW_GAP;
                    place([at[0] - way[1] * side * out, at[1] + way[0] * side * out], [way[1] * side, -way[0] * side], plots);
                }
            }
        }
        return stones;
    }
    // No path: rows along the yard's longest side, from one end of it to the other.
    let Some((a, b)) = yard.iter().filter(|ring| ring.len() > 2).flat_map(|ring| (0..ring.len()).map(move |i| (ring[i], ring[(i + 1) % ring.len()]))).max_by(|p, q| {
        let length = |(a, b): &(P, P)| (b[0] - a[0]).hypot(b[1] - a[1]);
        length(p).total_cmp(&length(q))
    }) else {
        return stones;
    };
    let run = (b[0] - a[0]).hypot(b[1] - a[1]);
    if run <= 0.0 {
        return stones;
    }
    let (u, v) = ([(b[0] - a[0]) / run, (b[1] - a[1]) / run], [-(b[1] - a[1]) / run, (b[0] - a[0]) / run]);
    // The yard's reach along and across that side, from its first corner.
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in yard.iter().flatten() {
        let d = [p[0] - a[0], p[1] - a[1]];
        let at = [d[0] * u[0] + d[1] * u[1], d[0] * v[0] + d[1] * v[1]];
        (lo, hi) = ([lo[0].min(at[0]), lo[1].min(at[1])], [hi[0].max(at[0]), hi[1].max(at[1])]);
    }
    let mut across = lo[1] + EDGE_CLEAR;
    let mut pair = 0;
    while across <= hi[1] {
        // The first row of a pair looks back at the aisle before it, the second on to the next.
        let facing = if pair % 2 == 0 { [-v[0], -v[1]] } else { v };
        let mut reach = lo[0] + EDGE_CLEAR;
        while reach <= hi[0] {
            place([a[0] + u[0] * reach + v[0] * across, a[1] + u[1] * reach + v[1] * across], facing, plots);
            reach += PITCH;
        }
        across += if pair % 2 == 0 { BACK_GAP } else { AISLE };
        pair += 1;
    }
    stones
}

/// Stands the stones of `yards` (each a graveyard's rings, metres) in the tiles of the finished
/// world, each on the ground under it. `roads` are the world's roads and paths with their widths,
/// metres. The rocks scattered over a yard as over any park are taken away: a graveyard is tended.
pub fn lay(built: &mut Built, yards: &[Vec<Vec<P>>], roads: &[(Vec<P>, f64)], half: f64, ground: &Ground) {
    if yards.is_empty() {
        return;
    }
    let Built { manifest, tiles } = built;
    let metres = |[x, z]: [i32; 2]| [f64::from(x) / 100.0, f64::from(z) / 100.0];
    let cm = |v: f64| (v * 100.0).round() as i32;
    let inner = half - WORLD_MARGIN;
    let grown = |b: [f64; 4], by: f64| [b[0] - by, b[1] - by, b[2] + by, b[3] + by];
    let in_box = |b: &[f64; 4], p: P| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
    let footprints: Vec<Vec<P>> = tiles.values().flat_map(|tile| &tile.buildings).map(|building| building.footprint.iter().map(|&p| metres(p)).collect()).collect();
    let trunks: Vec<P> = tiles.values().flat_map(|tile| &tile.trees).filter(|tree| tree.kind <= TREE_LOWWIDE).map(|tree| metres([tree.x, tree.z])).collect();
    let samples = manifest.tile_samples() as usize;
    let mut plots = Plots::default();
    let mut stones: Vec<Stone> = Vec::new();
    for yard in yards {
        let Some(reach) = bounds(yard) else { continue };
        let near = grown(reach, ROWS as f64 * ROW_GAP + 20.0);
        let ways: Vec<Way> = roads.iter().filter(|(line, _)| line.iter().any(|&p| in_box(&near, p)) || line.windows(2).any(|pair| crosses(&near, pair[0], pair[1]))).map(|(line, width)| (line.clone(), width / 2.0)).collect();
        let footprints: Vec<&Vec<P>> = footprints.iter().filter(|ring| ring.iter().any(|&p| in_box(&near, p))).collect();
        let trunks: Vec<P> = trunks.iter().copied().filter(|&p| in_box(&near, p)).collect();
        // Under water where the tile's water stands over its ground at the nearest sample.
        let wet = |at: P| {
            let (col, row) = manifest.tile_at(cm(at[0]), cm(at[1]));
            let Some(tile) = tiles.get(&(col, row)) else { return true };
            if samples == 0 || tile.water.len() != samples * samples || tile.ground.len() != tile.water.len() {
                return false;
            }
            let local = |v: f64, index: u16| ((v * 100.0 + f64::from(manifest.half_extent) - f64::from(index) * f64::from(manifest.tile_size)) / f64::from(manifest.ground_cell)).round().clamp(0.0, samples as f64 - 1.0) as usize;
            let sample = local(at[1], row) * samples + local(at[0], col);
            tile.water[sample] != NO_WATER && tile.water[sample] > tile.ground[sample]
        };
        let clear = |at: P| {
            at[0].abs() <= inner
                && at[1].abs() <= inner
                && !footprints.iter().any(|ring| point_in_polygon(at, ring))
                && trunks.iter().all(|p| (p[0] - at[0]).hypot(p[1] - at[1]) >= TRUNK_CLEAR)
                && !wet(at)
        };
        stones.extend(layout(yard, &ways, clear, &mut plots));
    }
    let boxes: Vec<([f64; 4], &Vec<Vec<P>>)> = yards.iter().filter_map(|yard| Some((bounds(yard)?, yard))).collect();
    for tile in tiles.values_mut() {
        tile.trees.retain(|tree| {
            let at = metres([tree.x, tree.z]);
            !matches!(tree.kind, TREE_ROCK_BOULDER | TREE_ROCK_SLAB) || !boxes.iter().any(|(b, yard)| in_box(b, at) && inside(yard, at))
        });
    }
    for (at, yaw, kind) in stones {
        let Some(tile) = tiles.get_mut(&manifest.tile_at(cm(at[0]), cm(at[1]))) else { continue };
        // Its heading kept within a half turn either way, as the milliradians of an `i16` hold.
        let yaw = yaw.sin().atan2(yaw.cos());
        tile.graves.push(Grave { x: cm(at[0]), z: cm(at[1]), base: cm(f64::from(ground.height_at(at[0] as f32, at[1] as f32))), yaw: (yaw * 1000.0).round() as i16, kind });
    }
    // Tile files are hashed: the same stones in the same order, whatever order the yards came in.
    for tile in tiles.values_mut() {
        tile.graves.sort_by_key(|grave| (grave.x, grave.z, grave.kind));
    }
}

/// Whether the segment from `p` to `q` cuts an edge of the box `b` (`[min x, min z, max x, max z]`):
/// a road that passes through it with neither end inside.
fn crosses(b: &[f64; 4], p: P, q: P) -> bool {
    let corners = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]];
    let side = |a: P, c: P, at: P| (c[0] - a[0]) * (at[1] - a[1]) - (c[1] - a[1]) * (at[0] - a[0]);
    (0..4).any(|i| {
        let (c, d) = (corners[i], corners[(i + 1) % 4]);
        side(p, q, c) * side(p, q, d) < 0.0 && side(c, d, p) * side(c, d, q) < 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{Manifest, Tile, WORLD_VERSION};

    fn square(min: f64, max: f64) -> Vec<Vec<P>> {
        vec![vec![[min, min], [max, min], [max, max], [min, max]]]
    }

    fn facing(yaw: f64) -> P {
        [yaw.sin(), yaw.cos()]
    }

    #[test]
    fn stones_stand_in_rows_on_both_sides_of_a_path_and_face_it() {
        // A 2.5 m path along z = 0 through a 60 m yard.
        let ways = vec![(vec![[-40.0, 0.0], [40.0, 0.0]], 1.25)];
        let stones = layout(&square(-30.0, 30.0), &ways, |_| true, &mut Plots::default());
        assert!(stones.len() > 70, "{}", stones.len());
        for side in [1.0, -1.0] {
            for row in 0..ROWS {
                let z = side * (1.25 + VERGE + row as f64 * ROW_GAP);
                let in_row: Vec<&Stone> = stones.iter().filter(|(at, ..)| (at[1] - z).abs() < 1e-6).collect();
                assert!(in_row.len() > 10, "row {row} on side {side}: {}", in_row.len());
                for (at, yaw, _) in in_row {
                    // It looks at the path: towards z = 0, within its wobble.
                    let look = facing(*yaw);
                    assert!(look[1] * -side > (WOBBLE * 1.01).cos(), "{at:?} looks {look:?}");
                }
            }
        }
        // Nothing beyond the last row.
        let last = 1.25 + VERGE + (ROWS - 1) as f64 * ROW_GAP;
        assert!(stones.iter().all(|(at, ..)| at[1].abs() < last + 1e-6));
    }

    #[test]
    fn a_graveyard_with_no_path_is_filled_with_rows_back_to_back() {
        // A yard 50 m by 30 m, its long side along x; a road passes outside it.
        let yard = vec![vec![[0.0, 0.0], [50.0, 0.0], [50.0, 30.0], [0.0, 30.0]]];
        let ways = vec![(vec![[-20.0, -10.0], [80.0, -10.0]], 3.0)];
        let stones = layout(&yard, &ways, |_| true, &mut Plots::default());
        let mut rows: Vec<f64> = stones.iter().map(|(at, ..)| at[1]).collect();
        rows.sort_by(f64::total_cmp);
        rows.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        // Rows along the long side, in pairs: back to back, then an aisle.
        assert!(rows.len() >= 8, "{rows:?}");
        for (index, pair) in rows.windows(2).enumerate() {
            let gap = if index % 2 == 0 { BACK_GAP } else { AISLE };
            assert!((pair[1] - pair[0] - gap).abs() < 1e-6, "{rows:?}");
        }
        for (at, yaw, _) in &stones {
            let row = rows.iter().position(|z| (z - at[1]).abs() < 1e-6).unwrap();
            let away = if row % 2 == 0 { -1.0 } else { 1.0 };
            assert!(facing(*yaw)[1] * away > (WOBBLE * 1.01).cos(), "row {row} looks {:?}", facing(*yaw));
        }
        // The yard is filled from end to end, a plot left empty here and there.
        let plots = (((50.0 - 2.0 * EDGE_CLEAR) / PITCH).floor() + 1.0) * rows.len() as f64;
        assert!((stones.len() as f64) > plots * 0.75 && (stones.len() as f64) < plots, "{} of {plots}", stones.len());
    }

    #[test]
    fn a_stub_of_a_path_at_the_gate_does_not_stand_for_the_yards_paths() {
        let ways = vec![(vec![[-40.0, 0.0], [-22.0, 0.0]], 1.25)];
        let stones = layout(&square(-30.0, 30.0), &ways, |_| true, &mut Plots::default());
        assert!(stones.iter().any(|(at, ..)| at[0] > 20.0), "the far end has stones too");
        assert!(stones.iter().all(|(at, ..)| distance_to_segment(*at, [-40.0, 0.0], [-22.0, 0.0]) >= 1.25 + ROAD_CLEAR), "none on the path");
    }

    #[test]
    fn stones_stay_inside_the_yard_and_off_paths_buildings_and_each_other() {
        // Two paths that cross, a chapel in one corner and a hole in the yard.
        let mut yard = square(-30.0, 30.0);
        yard.push(vec![[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]);
        let ways = vec![(vec![[-40.0, 0.0], [40.0, 0.0]], 1.25), (vec![[0.0, -40.0], [0.0, 40.0]], 2.5)];
        let chapel = [[-25.0, -25.0], [-10.0, -25.0], [-10.0, -10.0], [-25.0, -10.0]];
        let stones = layout(&yard, &ways, |at| !point_in_polygon(at, &chapel), &mut Plots::default());
        assert!(stones.len() > 100, "{}", stones.len());
        for (index, (at, ..)) in stones.iter().enumerate() {
            assert!(inside(&yard, *at) && to_edge(&yard, *at) >= EDGE_CLEAR, "{at:?}");
            assert!(!point_in_polygon(*at, &chapel), "{at:?} is in the chapel");
            assert!(at[1].abs() >= 1.25 + ROAD_CLEAR && at[0].abs() >= 2.5 + ROAD_CLEAR, "{at:?} is on a path");
            for (other, ..) in &stones[..index] {
                assert!((at[0] - other[0]).hypot(at[1] - other[1]) >= PLOT, "{at:?} and {other:?}");
            }
        }
    }

    #[test]
    fn the_same_graveyard_is_laid_the_same_way_every_time_with_all_three_kinds() {
        let ways = vec![(vec![[-40.0, 0.0], [40.0, 0.0]], 1.25)];
        let lay = || layout(&square(-30.0, 30.0), &ways, |_| true, &mut Plots::default());
        let stones = lay();
        assert_eq!(stones, lay());
        let count = |kind: u8| stones.iter().filter(|stone| stone.2 == kind).count();
        let (headstones, crosses, low) = (count(GRAVE_HEADSTONE), count(GRAVE_CROSS), count(GRAVE_LOW));
        assert!(headstones > crosses && headstones > low && crosses > 5 && low > 5, "{headstones} {crosses} {low}");
        // A yard mapped twice gets its stones once.
        let mut plots = Plots::default();
        let first = layout(&square(-30.0, 30.0), &ways, |_| true, &mut plots);
        assert_eq!(first, stones);
        assert!(layout(&square(-30.0, 30.0), &ways, |_| true, &mut plots).is_empty());
    }

    #[test]
    fn stones_are_stood_in_their_tiles_on_the_ground_off_trees_and_the_rocks_are_cleared() {
        use shared::world::{TREE_BIRCH, Tree};
        // Ground rising 0.1 m a metre to the east, over four tiles of 100 m.
        let (side, cell) = (41u32, 500);
        let heights = (0..side * side).map(|k| ((k % side) as i32 * cell) / 10).collect();
        let ground = Ground { origin: -10_000, cell, side, heights };
        let manifest = Manifest { version: WORLD_VERSION, half_extent: 10_000, tile_size: 10_000, ..Default::default() };
        let tree = |x: i32, z: i32, kind: u8| Tree { x, z, kind, height: 800, crown: 200, ..Default::default() };
        let mut built = Built { manifest, tiles: (0..2).flat_map(|c| (0..2).map(move |r| ((c, r), Tile::default()))).collect() };
        // A birch where a stone of the first row would stand, a boulder in the yard and one outside it.
        let row = 1.25 + VERGE;
        built.tiles.get_mut(&(1, 1)).unwrap().trees = vec![tree(2200, (row * 100.0) as i32, TREE_BIRCH), tree(1000, 2000, TREE_ROCK_BOULDER), tree(8000, 8000, TREE_ROCK_SLAB)];
        let roads = vec![(vec![[-90.0, 0.0], [90.0, 0.0]], 2.5), (vec![[-90.0, 80.0], [90.0, 80.0]], 6.0)];
        lay(&mut built, &[square(-30.0, 30.0)], &roads, 100.0, &ground);
        let graves: Vec<Grave> = built.tiles.values().flat_map(|tile| tile.graves.iter().copied()).collect();
        assert!(graves.len() > 70, "{}", graves.len());
        for (&(col, row), tile) in &built.tiles {
            assert!(!tile.graves.is_empty(), "the yard is over all four tiles");
            assert!(tile.graves.windows(2).all(|pair| (pair[0].x, pair[0].z) < (pair[1].x, pair[1].z)), "in order");
            for grave in &tile.graves {
                assert_eq!(built.manifest.tile_at(grave.x, grave.z), (col, row));
                assert!((grave.base - (grave.x + 10_000) / 10).abs() <= 1, "{grave:?} stands on the slope");
            }
        }
        assert!(graves.iter().all(|grave| f64::from(grave.x - 2200).hypot(f64::from(grave.z) - row * 100.0) >= TRUNK_CLEAR * 100.0), "none against the birch");
        let rocks: Vec<[i32; 2]> = built.tiles.values().flat_map(|tile| &tile.trees).filter(|tree| tree.kind != TREE_BIRCH).map(|tree| [tree.x, tree.z]).collect();
        assert_eq!(rocks, [[8000, 8000]], "the boulder in the yard is gone, the slab outside it stays");
    }
}
