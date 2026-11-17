//! A tile's grid of cells (`Tile::stones`, `Tile::crops`) as something to look up by position. Stony
//! ground slows a soldier on foot by its grade (`shared::movement::stones_scale`), and server and client
//! read the same grid, so the going is predicted. Crops hide a soldier from the bots, which only the
//! server asks about.

use shared::quant::from_cm;
use std::collections::BTreeMap;

/// Every tile's grid of one kind, by the tile it came in.
#[derive(Clone, Debug, Default)]
pub struct TileCells {
    by_tile: BTreeMap<(u16, u16), Vec<u8>>,
    /// The world's half-extent and a tile's side, metres.
    half: f32,
    tile: f32,
}

impl TileCells {
    pub fn new(half_extent: i32, tile_size: i32) -> TileCells {
        TileCells { by_tile: BTreeMap::new(), half: from_cm(half_extent), tile: from_cm(tile_size) }
    }

    pub fn set(&mut self, tile: (u16, u16), cells: Vec<u8>) {
        if cells.is_empty() {
            self.by_tile.remove(&tile);
        } else {
            self.by_tile.insert(tile, cells);
        }
    }

    pub fn remove(&mut self, tile: (u16, u16)) {
        self.by_tile.remove(&tile);
    }

    /// The cell under `(x, z)`, 0 where the tile has no grid or is not here.
    pub fn at(&self, x: f32, z: f32) -> u8 {
        if self.by_tile.is_empty() || self.tile <= 0.0 {
            return 0;
        }
        let (u, v) = ((x + self.half) / self.tile, (z + self.half) / self.tile);
        if u < 0.0 || v < 0.0 {
            return 0;
        }
        let (col, row) = (u.floor(), v.floor());
        let Some(cells) = self.by_tile.get(&(col as u16, row as u16)) else { return 0 };
        let n = (cells.len() as f32).sqrt().round() as usize;
        let i = (((u - col) * n as f32) as usize).min(n - 1);
        let j = (((v - row) * n as f32) as usize).min(n - 1);
        cells[j * n + i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_cell_under_a_point() {
        // Tiles of 4 m, the world 8 m across: tile (1, 0) is x 0..4, z -4..0, in 2 × 2 cells.
        let mut stones = TileCells::new(400, 400);
        stones.set((1, 0), vec![1, 2, 3, 4]);
        assert_eq!(stones.at(0.5, -3.5), 1);
        assert_eq!(stones.at(3.5, -3.5), 2);
        assert_eq!(stones.at(0.5, -0.5), 3);
        assert_eq!(stones.at(3.9, -0.1), 4);
        assert_eq!(stones.at(-1.0, -1.0), 0, "a tile without stones");
        stones.remove((1, 0));
        assert_eq!(stones.at(0.5, -3.5), 0);
    }
}
