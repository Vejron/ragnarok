//! Samples terrain heights onto the grid `Ground` stores: `cell`-metre samples over the whole
//! square of tiles, in centimetres above the lowest sample (so the lowest ground is y = 0).

use anyhow::{Result, bail};
use crate::water::Grid;
use shared::terrain::Ground;

/// Highest relief a world may have, metres: what the boundary walls are built over
/// (`shared::consts::MAX_RELIEF`).
const MAX_RELIEF: f64 = shared::consts::MAX_RELIEF as f64;

/// Water beds may be cut this far below the lowest dry ground, metres, so y = 0 stays the lowest
/// land and the relief limit is about land only.
const MAX_BED_BELOW: f64 = 12.0;

/// The sample grid `build` makes for these sizes (metres): a `grid × grid` square of `tile`-metre
/// tiles centred on the origin, a sample every `cell` metres.
pub fn grid(half: f64, tile: f64, cell: f64) -> Result<Grid> {
    let cm = |metres: f64| (metres * 100.0).round() as i32;
    let (tile_cm, cell_cm) = (cm(tile), cm(cell));
    if cell_cm < 50 || tile_cm % cell_cm != 0 {
        bail!("--cell ({cell} m) must be at least 0.5 m and divide --tile ({tile} m)");
    }
    let per_tile = (tile_cm / cell_cm) as u32;
    let tiles = ((2 * cm(half) + tile_cm - 1) / tile_cm).max(1) as u32;
    Ok(Grid { origin: -cm(half), cell: cell_cm, side: tiles * per_tile + 1 })
}

/// Returns the ground and the elevation of its lowest dry sample, which is its y = 0.
/// Samples `elevation(x, z)` (metres above any datum, local metres, `None` where unknown) at every
/// grid point of a `grid × grid` square of `tile`-metre tiles centred on the origin. `bed(x, z,
/// elevation)` then gives the ground there, which differs from the elevation under water; it may
/// go up to `MAX_BED_BELOW` below y = 0.
pub fn build(
    half: f64,
    tile: f64,
    cell: f64,
    elevation: impl FnMut(f64, f64) -> Option<f64>,
    bed: impl Fn(f64, f64, Option<f64>) -> Option<f64>,
) -> Result<(Ground, f64)> {
    build_where(half, tile, cell, |_, _| true, elevation, bed)
}

/// As `build`, but only samples where `wanted(x, z)` are read from `elevation`: the rest are gaps,
/// so ground outside the world is neither downloaded nor counted against the relief limit.
pub fn build_where(
    half: f64,
    tile: f64,
    cell: f64,
    wanted: impl Fn(f64, f64) -> bool,
    mut elevation: impl FnMut(f64, f64) -> Option<f64>,
    bed: impl Fn(f64, f64, Option<f64>) -> Option<f64>,
) -> Result<(Ground, f64)> {
    let Grid { origin, cell: cell_cm, side } = grid(half, tile, cell)?;
    let at = |i: u32| (origin + i as i32 * cell_cm) as f64 / 100.0;

    let mut metres = vec![f64::NAN; (side * side) as usize];
    for j in 0..side {
        crate::progress::steps(j as usize, side as usize);
        for i in 0..side {
            if !wanted(at(i), at(j)) {
                continue;
            }
            if let Some(h) = elevation(at(i), at(j)) {
                metres[(j * side + i) as usize] = h;
            }
        }
    }
    let known = || metres.iter().copied().filter(|h| h.is_finite());
    let (low, high) = known().fold((f64::MAX, f64::MIN), |(lo, hi), h| (lo.min(h), hi.max(h)));
    if low > high {
        bail!("the elevation data has no samples inside the world");
    }
    if high - low > MAX_RELIEF {
        // Say where the extremes are, so the box can be moved or shrunk away from them.
        let place = |target: f64| {
            let index = metres.iter().position(|&h| h == target).unwrap_or(0) as u32;
            format!("({:.0}, {:.0})", at(index % side), at(index / side))
        };
        bail!(
            "relief of {:.0} m does not fit the format (at most {MAX_RELIEF} m): lowest {low:.1} m at (x, z) = {}, highest {high:.1} m at {}, in metres from the centre; shrink --half or move the box",
            high - low,
            place(low),
            place(high)
        );
    }
    // Gaps (water, missing tiles) take the lowest ground, so they are flat rather than cliffs.
    let heights = metres
        .iter()
        .enumerate()
        .map(|(index, &h)| {
            let dry = h.is_finite().then_some(h);
            let ground = bed(at(index as u32 % side), at(index as u32 / side), dry).unwrap_or(low).max(low - MAX_BED_BELOW);
            ((ground - low) * 100.0).round() as i32
        })
        .collect();
    eprintln!("terrain: {side}×{side} samples every {cell} m, {low:.1} to {high:.1} m above the datum ({:.0} m of relief)", high - low);
    Ok((Ground { origin, cell: cell_cm, side, heights }, low))
}

/// Gentle hills for `--synthetic`, so the offline world exercises slopes: up to about 12 m.
pub fn hills(x: f64, z: f64) -> f64 {
    6.0 + 4.0 * (x / 95.0).sin() * (z / 70.0).cos() + 2.0 * ((x + z) / 40.0).sin()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_are_relative_to_the_lowest_sample() {
        let (ground, datum) = build(100.0, 100.0, 10.0, |x, _| Some(50.0 + x / 10.0), |_, _, h| h).unwrap();
        assert_eq!(datum, 40.0);
        assert_eq!((ground.side, ground.cell, ground.origin), (21, 1000, -10_000));
        assert_eq!(ground.heights[0], 0);
        assert_eq!(ground.heights[20], 2000);
        assert!((ground.height_at(0.0, 0.0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn missing_samples_take_the_lowest_ground() {
        let (ground, _) = build(50.0, 100.0, 10.0, |x, _| (x < 0.0).then_some(7.0 + x / 10.0), |_, _, h| h).unwrap();
        // x = -10 m is the highest known sample (6 m, 4 m over the lowest); the gap is level with the lowest.
        assert_eq!(ground.heights[4], 400);
        assert_eq!(*ground.heights.last().unwrap(), 0);
    }

    #[test]
    fn beds_may_dip_below_the_lowest_land_but_not_far() {
        let (ground, datum) = build(50.0, 100.0, 10.0, |_, _| Some(10.0), |x, _, h| if x == 0.0 { Some(h.unwrap() - 1.0) } else if x > 0.0 { Some(-100.0) } else { h }).unwrap();
        assert_eq!(datum, 10.0);
        let row = ground.side as usize;
        assert_eq!(ground.heights[0], 0);
        assert_eq!(ground.heights[row / 2], -100);
        assert_eq!(ground.heights[row - 1], -1200);
    }

    #[test]
    fn rejects_cells_that_do_not_divide_the_tile() {
        assert!(build(100.0, 100.0, 30.0, |_, _| Some(0.0), |_, _, h| h).is_err());
    }
}
