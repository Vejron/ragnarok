//! Fine ground relief, baked into each tile as slopes on a grid finer than the ground's. The
//! ground (`Tile::ground`) is what is simulated and what the client's mesh follows; this is only
//! what its lighting reads, so a 4 m mesh can still shade the 1 m height model's ditches, banks
//! and road cuts.
//!
//! The fine surface is the ground's own triangles plus what the 1 m model has that the coarse
//! samples' mean lost (`fine - coarse` at the same place). So water beds carved into the ground
//! stay as carved, and where the two models agree the result is exactly the ground.

use crate::assemble::Built;
use shared::terrain::Ground;
use shared::world::DETAIL_SLOPE_SCALE;

/// Largest correction the fine model may make to the ground, metres. Beyond it the model and the
/// ground disagree about more than relief (a gap, a bridge cut), so the correction is dropped.
const MAX_RESIDUAL: f64 = 2.0;

/// Fills `Tile::detail` of every tile of `built` with slopes every `detail` metres, which must
/// divide the tile. `ground` is the carved ground the tiles were cut from, `datum` its y = 0 as an
/// elevation. `coarse(x, z)` and `fine(x, z)` are the height model (local metres, elevation) averaged
/// over a ground cell and over a detail cell.
pub fn bake(built: &mut Built, ground: &Ground, datum: f64, detail: f64, coarse: impl Fn(f64, f64) -> Option<f64>, fine: impl Fn(f64, f64) -> Option<f64>) {
    let tile = built.manifest.tile_size as f64 / 100.0;
    let n = (tile / detail).round() as usize;
    if ground.is_flat() || n == 0 || (n as f64 * detail - tile).abs() > 1e-6 {
        return;
    }
    let side = ground.side as usize;
    let at = |i: usize| (ground.origin + i as i32 * ground.cell) as f64 / 100.0;
    // The height model on the ground's own grid, uncarved, to take the fine model's difference from.
    let mut heights = ground.heights.clone();
    for j in 0..side {
        for i in 0..side {
            if let Some(e) = coarse(at(i), at(j)) {
                heights[j * side + i] = ((e - datum) * 100.0).round() as i32;
            }
        }
    }
    let model = Ground { heights, ..ground.clone() };

    let mut baked = 0;
    let mut grid = vec![0f32; (n + 1) * (n + 1)];
    for (&(col, row), tile_data) in built.tiles.iter_mut() {
        let x0 = (ground.origin as i64 + col as i64 * built.manifest.tile_size as i64) as f64 / 100.0;
        let z0 = (ground.origin as i64 + row as i64 * built.manifest.tile_size as i64) as f64 / 100.0;
        for k in 0..=n {
            for i in 0..=n {
                let (x, z) = (x0 + i as f64 * detail, z0 + k as f64 * detail);
                let base = ground.height_at(x as f32, z as f32);
                let extra = fine(x, z)
                    .map(|e| e - datum - model.height_at(x as f32, z as f32) as f64)
                    .filter(|r| r.abs() <= MAX_RESIDUAL)
                    .unwrap_or(0.0);
                grid[k * (n + 1) + i] = base + extra as f32;
            }
        }
        let h = |i: usize, k: usize| grid[k * (n + 1) + i];
        let scale = DETAIL_SLOPE_SCALE / (2.0 * detail as f32);
        let quantise = |v: f32| (v * scale).round().clamp(-127.0, 127.0) as i8;
        let mut slopes = Vec::with_capacity(n * n * 2);
        for k in 0..n {
            for i in 0..n {
                // The cell's gradient, from its four corners.
                slopes.push(quantise((h(i + 1, k) - h(i, k)) + (h(i + 1, k + 1) - h(i, k + 1))));
                slopes.push(quantise((h(i, k + 1) - h(i, k)) + (h(i + 1, k + 1) - h(i + 1, k))));
            }
        }
        if slopes.iter().any(|&s| s != 0) {
            tile_data.detail = slopes;
            baked += 1;
        }
    }
    eprintln!("detail: {baked} tiles with a {n}×{n} slope map ({detail} m texels)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::Built;
    use shared::world::{Manifest, Tile};
    use std::collections::BTreeMap;

    fn built(ground: &Ground, tile_cm: i32) -> Built {
        let manifest = Manifest { tile_size: tile_cm, ground_cell: ground.cell, half_extent: -ground.origin, ..Default::default() };
        let mut tiles = BTreeMap::new();
        tiles.insert((0, 0), Tile::default());
        Built { manifest, tiles }
    }

    #[test]
    fn a_slope_the_ground_has_comes_back_as_that_slope() {
        // 40 m tile, 10 m ground cells, rising 0.5 m per metre along x.
        let side = 5usize;
        let heights = (0..side * side).map(|k| ((k % side) as i32) * 500).collect();
        let ground = Ground { origin: 0, cell: 1000, side: side as u32, heights };
        let mut world = built(&ground, 4000);
        world.manifest.half_extent = 0;
        let line = |x: f64, _z: f64| Some(x * 0.5);
        bake(&mut world, &ground, 0.0, 10.0, line, line);
        let slopes = &world.tiles[&(0, 0)].detail;
        assert_eq!(slopes.len(), 4 * 4 * 2);
        assert_eq!(slopes[0], (0.5 * DETAIL_SLOPE_SCALE).round() as i8);
        assert_eq!(slopes[1], 0);
    }

    #[test]
    fn the_fine_models_bump_shows_where_the_ground_is_flat() {
        let side = 5usize;
        let ground = Ground { origin: 0, cell: 1000, side: side as u32, heights: vec![0; side * side] };
        let mut world = built(&ground, 4000);
        world.manifest.half_extent = 0;
        // A 1 m bump in the middle of the tile that the 10 m mean smooths away.
        let bump = |x: f64, z: f64| Some(if (x - 20.0).abs() < 5.0 && (z - 20.0).abs() < 5.0 { 1.0 } else { 0.0 });
        bake(&mut world, &ground, 0.0, 5.0, |_, _| Some(0.0), bump);
        assert!(world.tiles[&(0, 0)].detail.iter().any(|&s| s != 0));
    }

    #[test]
    fn a_flat_tile_carries_nothing() {
        let side = 5usize;
        let ground = Ground { origin: 0, cell: 1000, side: side as u32, heights: vec![0; side * side] };
        let mut world = built(&ground, 4000);
        world.manifest.half_extent = 0;
        bake(&mut world, &ground, 0.0, 10.0, |_, _| Some(0.0), |_, _| Some(0.0));
        assert!(world.tiles[&(0, 0)].detail.is_empty());
    }
}
