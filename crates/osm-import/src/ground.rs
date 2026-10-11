//! Samples terrain heights onto the grid `Ground` stores: `cell`-metre samples over the whole
//! square of tiles, in centimetres above the lowest sample (so the lowest ground is y = 0).

use anyhow::{Result, bail};
use shared::terrain::Ground;

/// Highest relief a world may have, metres. The boundary walls and the helicopter ceiling are at
/// fixed heights (`BOUNDARY_HEIGHT`, `HELI_CEILING`), so the ground has to stay well below them.
const MAX_RELIEF: f64 = 100.0;

/// Returns the ground and the elevation of its lowest sample, which is its y = 0.
/// Samples `elevation(x, z)` (metres above any datum, local metres, `None` where unknown) at every
/// grid point of a `grid × grid` square of `tile`-metre tiles centred on the origin.
pub fn build(half: f64, tile: f64, cell: f64, mut elevation: impl FnMut(f64, f64) -> Option<f64>) -> Result<(Ground, f64)> {
    let cm = |metres: f64| (metres * 100.0).round() as i32;
    let (tile_cm, cell_cm) = (cm(tile), cm(cell));
    if cell_cm < 50 || tile_cm % cell_cm != 0 {
        bail!("--cell ({cell} m) must be at least 0.5 m and divide --tile ({tile} m)");
    }
    let per_tile = (tile_cm / cell_cm) as u32;
    let grid = ((2 * cm(half) + tile_cm - 1) / tile_cm).max(1) as u32;
    let side = grid * per_tile + 1;
    let origin = -cm(half);
    let at = |i: u32| (origin + i as i32 * cell_cm) as f64 / 100.0;

    let mut metres = vec![f64::NAN; (side * side) as usize];
    for j in 0..side {
        for i in 0..side {
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
    let heights = metres.iter().map(|&h| (((if h.is_finite() { h } else { low }) - low) * 100.0).round() as i16).collect();
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
        let (ground, datum) = build(100.0, 100.0, 10.0, |x, _| Some(50.0 + x / 10.0)).unwrap();
        assert_eq!(datum, 40.0);
        assert_eq!((ground.side, ground.cell, ground.origin), (21, 1000, -10_000));
        assert_eq!(ground.heights[0], 0);
        assert_eq!(ground.heights[20], 2000);
        assert!((ground.height_at(0.0, 0.0) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn missing_samples_take_the_lowest_ground() {
        let (ground, _) = build(50.0, 100.0, 10.0, |x, _| (x < 0.0).then_some(7.0 + x / 10.0)).unwrap();
        // x = -10 m is the highest known sample (6 m, 4 m over the lowest); the gap is level with the lowest.
        assert_eq!(ground.heights[4], 400);
        assert_eq!(*ground.heights.last().unwrap(), 0);
    }

    #[test]
    fn rejects_cells_that_do_not_divide_the_tile() {
        assert!(build(100.0, 100.0, 30.0, |_, _| Some(0.0)).is_err());
    }
}
