//! The ground: a regular grid of heights, sampled the same way by the importer, the server's
//! physics and the client (`client/src/sim/terrain.ts` mirrors `height_at`).
//!
//! Samples are `i32` centimetres. Row `j` runs along +Z and column `i` along +X, so sample
//! `(i, j)` sits at `(origin + i * cell, origin + j * cell)`. Each grid cell is split along the
//! diagonal from its `(x0, z1)` corner to its `(x1, z0)` corner, which is how Rapier's
//! heightfield triangulates by default, so `height_at` agrees with the collider's surface.

use serde::{Deserialize, Serialize};

/// A grid of ground heights. An empty grid is flat ground at y = 0.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    /// Local position of sample `(0, 0)`, centimetres, on both axes.
    pub origin: i32,
    /// Distance between samples, centimetres. Zero means no ground data.
    pub cell: i32,
    /// Samples per side.
    pub side: u32,
    /// `side * side` heights, centimetres, row-major (`j * side + i`).
    pub heights: Vec<i32>,
}

impl Ground {
    pub fn is_flat(&self) -> bool {
        self.cell <= 0 || self.heights.is_empty()
    }

    /// Height at sample `(i, j)`, metres, clamped to the grid.
    pub fn sample(&self, i: i64, j: i64) -> f32 {
        let last = self.side as i64 - 1;
        let (i, j) = (i.clamp(0, last), j.clamp(0, last));
        self.heights[(j * self.side as i64 + i) as usize] as f32 / 100.0
    }

    /// Ground height under local point `(x, z)`, metres, interpolated over the triangles the
    /// collider uses. Clamps to the edge outside the grid.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        if self.is_flat() {
            return 0.0;
        }
        let cell = self.cell as f32 / 100.0;
        let origin = self.origin as f32 / 100.0;
        let last = (self.side - 1) as f32;
        let gx = ((x - origin) / cell).clamp(0.0, last);
        let gz = ((z - origin) / cell).clamp(0.0, last);
        let (i, j) = ((gx.floor() as i64).min(self.side as i64 - 2), (gz.floor() as i64).min(self.side as i64 - 2));
        let (fx, fz) = (gx - i as f32, gz - j as f32);
        let h00 = self.sample(i, j);
        let h10 = self.sample(i, j + 1); // (x0, z1)
        let h01 = self.sample(i + 1, j); // (x1, z0)
        let h11 = self.sample(i + 1, j + 1);
        if fx + fz <= 1.0 {
            h00 + fx * (h01 - h00) + fz * (h10 - h00)
        } else {
            h11 + (1.0 - fx) * (h10 - h11) + (1.0 - fz) * (h01 - h11)
        }
    }

    /// Highest sample, metres.
    pub fn max_height(&self) -> f32 {
        self.heights.iter().copied().max().map_or(0.0, |h| h as f32 / 100.0)
    }

    /// Copies the `side × side` block of samples whose first sample is `(i0, j0)`.
    pub fn block(&self, i0: u32, j0: u32, side: u32) -> Vec<i32> {
        let mut out = Vec::with_capacity((side * side) as usize);
        for j in 0..side {
            for i in 0..side {
                out.push(self.heights[((j0 + j) * self.side + i0 + i) as usize]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Ground {
        // 3×3 samples 100 cm apart, height = 10 cm per cm of x: a plane sloping up towards +X.
        let heights = (0..3).flat_map(|_j| (0..3).map(|i| i * 100)).collect();
        Ground { origin: 0, cell: 100, side: 3, heights }
    }

    #[test]
    fn empty_ground_is_flat() {
        assert_eq!(Ground::default().height_at(5.0, -3.0), 0.0);
    }

    #[test]
    fn interpolates_a_plane_exactly_and_clamps() {
        let g = ramp();
        assert!((g.height_at(0.5, 0.3) - 0.5).abs() < 1e-5);
        assert!((g.height_at(1.25, 1.9) - 1.25).abs() < 1e-5);
        assert_eq!(g.height_at(-10.0, 0.0), 0.0);
        assert!((g.height_at(10.0, 0.0) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn diagonal_runs_from_x0z1_to_x1z0() {
        // One cell with only the (x1, z1) corner raised: below the diagonal the plane is flat.
        let g = Ground { origin: 0, cell: 100, side: 2, heights: vec![0, 0, 0, 100] };
        assert_eq!(g.height_at(0.4, 0.4), 0.0);
        assert!((g.height_at(0.75, 0.75) - 0.5).abs() < 1e-5);
    }

    /// A bumpy 9 × 9 grid of 5 m cells starting at (0, 0). `terrain.spec.ts` builds the same one
    /// and checks `TERRAIN_FIXTURE` against its own sampler.
    pub fn bumpy() -> Ground {
        let heights = (0..9).flat_map(|j| (0..9).map(move |i| (i * 37 + j * 91) % 400 - 100)).collect();
        Ground { origin: 0, cell: 500, side: 9, heights }
    }

    /// `(x, z, height)` on `bumpy()`, metres; mirrored in `terrain.spec.ts`.
    pub const TERRAIN_FIXTURE: [(f32, f32, f32); 6] = [
        (3.3, 7.9, 0.682),
        (12.5, 12.5, 2.2),
        (0.0, 0.0, -1.0),
        (39.99, 20.01, 1.60108),
        (-5.0, 50.0, 2.28),
        (21.7, 33.3, 2.6664),
    ];

    #[test]
    fn sampler_matches_the_fixture_the_client_checks() {
        let g = bumpy();
        for (x, z, expected) in TERRAIN_FIXTURE {
            let h = g.height_at(x, z);
            assert!((h - expected).abs() < 1e-4, "({x}, {z}): {h} != {expected}");
        }
    }

    #[test]
    fn blocks_cut_out_a_tile() {
        let g = ramp();
        assert_eq!(g.block(1, 1, 2), vec![100, 200, 100, 200]);
    }
}
