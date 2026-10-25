//! Bushes and hedges from IGN's class 4 (medium vegetation, 0.5 to 1.5 m).
//!
//! The returns are binned into 1 m cells, and the cells into 2.5 m blocks: a block with enough
//! vegetated cells is one bush at their centroid, sized by how many there are. A lone shrub comes
//! out as one bush and a hedge as a row of them. Blocks under a tree's crown (class 5 in any of
//! their cells) are left out: those returns are the crown's lower branches, not undergrowth.

use crate::lidar::Point;

/// Raster cell, metres.
const CELL: f64 = 1.0;
/// Cells to a block side: one bush per block at most.
const BLOCK: usize = 2;
/// Medium-vegetation returns a cell needs to count, so a stray return is not a bush.
const MIN_RETURNS: u16 = 3;
/// Vegetated cells a block needs to hold a bush.
const MIN_CELLS: usize = 2;
/// Heights, metres above the terrain, the class allows (and a bush is drawn at).
const HEIGHT: (f64, f64) = (0.6, 1.6);

/// A bush found in the cloud, in the region's projected coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bush {
    pub e: f64,
    pub n: f64,
    /// Metres above the terrain.
    pub height: f64,
    /// Crown radius, metres.
    pub crown: f64,
}

pub struct Undergrowth {
    e0: f64,
    n0: f64,
    cols: usize,
    rows: usize,
    count: Vec<u16>,
    /// Highest medium-vegetation return per cell, NaN when none.
    top: Vec<f32>,
    /// A class-5 return fell in the cell.
    crowned: Vec<bool>,
}

impl Undergrowth {
    pub fn new([e0, n0, e1, n1]: [f64; 4]) -> Self {
        let (cols, rows) = (((e1 - e0) / CELL).ceil() as usize + 1, ((n1 - n0) / CELL).ceil() as usize + 1);
        let size = cols * rows;
        Self { e0, n0, cols, rows, count: vec![0; size], top: vec![f32::NAN; size], crowned: vec![false; size] }
    }

    fn index(&self, e: f64, n: f64) -> Option<usize> {
        let (c, r) = (((e - self.e0) / CELL).floor(), ((n - self.n0) / CELL).floor());
        (c >= 0.0 && r >= 0.0 && (c as usize) < self.cols && (r as usize) < self.rows).then(|| r as usize * self.cols + c as usize)
    }

    /// A medium-vegetation return.
    pub fn add(&mut self, p: &Point) {
        if let Some(i) = self.index(p.e, p.n) {
            self.count[i] = self.count[i].saturating_add(1);
            // NaN (no return yet) gives way to any height.
            self.top[i] = self.top[i].max(p.z as f32);
        }
    }

    /// A high-vegetation return: the cell is under a crown.
    pub fn crown(&mut self, p: &Point) {
        if let Some(i) = self.index(p.e, p.n) {
            self.crowned[i] = true;
        }
    }

    /// One bush per block with enough vegetated cells and no crown over it.
    pub fn detect(&self, ground: impl Fn(f64, f64) -> Option<f32>) -> Vec<Bush> {
        let mut found = Vec::new();
        for br in (0..self.rows).step_by(BLOCK) {
            for bc in (0..self.cols).step_by(BLOCK) {
                let mut cells = Vec::new();
                let mut crowned = false;
                for r in br..(br + BLOCK).min(self.rows) {
                    for c in bc..(bc + BLOCK).min(self.cols) {
                        let i = r * self.cols + c;
                        crowned |= self.crowned[i];
                        if self.count[i] >= MIN_RETURNS {
                            cells.push((c, r, self.top[i]));
                        }
                    }
                }
                if crowned || cells.len() < MIN_CELLS {
                    continue;
                }
                let k = cells.len() as f64;
                let e = self.e0 + (cells.iter().map(|c| c.0 as f64).sum::<f64>() / k + 0.5) * CELL;
                let n = self.n0 + (cells.iter().map(|c| c.1 as f64).sum::<f64>() / k + 0.5) * CELL;
                let top = cells.iter().map(|c| c.2).fold(f32::MIN, f32::max);
                let height = ground(e, n).map_or(1.0, |g| f64::from(top - g)).clamp(HEIGHT.0, HEIGHT.1);
                // The circle with the vegetated cells' area, a little fuller so a hedge closes up.
                let crown = (k * CELL * CELL / std::f64::consts::PI).sqrt() * 1.2;
                found.push(Bush { e, n, height, crown: crown.clamp(0.6, 1.5) });
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_: f64, _: f64) -> Option<f32> {
        Some(0.0)
    }

    fn point(e: f64, n: f64, z: f64) -> Point {
        Point { e, n, z, class: crate::ign::class::MEDIUM_VEGETATION, ret: 1, returns: 1, intensity: 100 }
    }

    /// Returns on a 0.25 m lattice over `[e0, e1] × [n0, n1]` at height `z`.
    fn patch(growth: &mut Undergrowth, [e0, n0, e1, n1]: [f64; 4], z: f64) {
        let mut e = e0;
        while e < e1 {
            let mut n = n0;
            while n < n1 {
                growth.add(&point(e, n, z));
                n += 0.25;
            }
            e += 0.25;
        }
    }

    #[test]
    fn a_shrub_is_one_bush_and_a_hedge_a_row() {
        let mut growth = Undergrowth::new([0.0, 0.0, 40.0, 40.0]);
        patch(&mut growth, [4.0, 4.0, 6.0, 6.0], 1.2);
        patch(&mut growth, [10.0, 20.0, 30.0, 21.0], 1.4);
        let bushes = growth.detect(flat);
        let shrub: Vec<_> = bushes.iter().filter(|b| b.n < 10.0).collect();
        assert_eq!(shrub.len(), 1, "{bushes:?}");
        assert!((shrub[0].e - 5.0).abs() < 0.6 && (shrub[0].height - 1.2).abs() < 0.05);
        assert_eq!(bushes.iter().filter(|b| b.n > 10.0).count(), 10, "{bushes:?}");
    }

    #[test]
    fn stray_returns_and_branches_under_a_crown_are_not_bushes() {
        let mut growth = Undergrowth::new([0.0, 0.0, 40.0, 40.0]);
        growth.add(&point(5.5, 5.5, 1.0));
        growth.add(&point(6.5, 5.5, 1.0));
        patch(&mut growth, [20.0, 20.0, 22.0, 22.0], 1.0);
        growth.crown(&Point { class: crate::ign::class::HIGH_VEGETATION, z: 8.0, ..point(21.0, 21.0, 0.0) });
        assert!(growth.detect(flat).is_empty());
    }
}
