//! Which country's data a world is built from. A region fixes the frame of its global tile grid,
//! the frame a world in it is laid out in, and the projected coordinates its height model and
//! point cloud come in.
//!
//! - **Sweden**: Lantmäteriet (DTM, laser, Hydrografi) in SWEREF 99 TM, and worlds in the fixed
//!   equirectangular frame (`Frame::Equirect`), as every world was before France.
//! - **France**: IGN (LiDAR HD, its MNT) in Lambert-93, and worlds in Lambert-93 itself
//!   (`Frame::Lambert93`), so a world's metres are IGN's and its tiles line up with IGN's grid.

use crate::{assemble, dem, grid};
use serde::{Deserialize, Serialize};
use shared::lambert93;
use shared::world::Frame;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    #[default]
    Sweden,
    France,
}

/// Lambert-93 point that tile (0, 0) of France's grid starts at (its north-west corner): the
/// projection's own false origin, 46.5° N 3° E, near the middle of France.
const FRANCE_ANCHOR: (f64, f64) = (700_000.0, 6_600_000.0);

impl Region {
    /// The region `(lat, lon)` lies in: France inside its box, Sweden everywhere else.
    pub fn at(lat: f64, lon: f64) -> Region {
        if lambert93::in_france(lat, lon) { Region::France } else { Region::Sweden }
    }

    pub fn name(self) -> &'static str {
        match self {
            Region::Sweden => "sweden",
            Region::France => "france",
        }
    }

    /// The frame the region's global tile grid is cut in: tile `(gx, gy)` covers
    /// `[gx, gx + 1) × [gy, gy + 1)` tiles of it.
    pub fn grid(self) -> Frame {
        match self {
            Region::Sweden => Frame::Equirect { origin: grid::ORIGIN },
            Region::France => Frame::Lambert93 { e0: FRANCE_ANCHOR.0, n0: FRANCE_ANCHOR.1 },
        }
    }

    /// The square world of half-side `half` metres around `(lat, lon)`: its frame, its half-side
    /// and the `[south, west, north, east]` box that holds it.
    pub fn around(self, lat: f64, lon: f64, half: f64) -> Place {
        match self {
            Region::Sweden => Place::sweden(assemble::bbox_around(lat, lon, half)),
            Region::France => {
                let (e, n) = lambert93::forward(lat, lon);
                Place::france(Frame::Lambert93 { e0: e, n0: n }, half)
            }
        }
    }

    /// The largest square world around the centre of `bbox` (`[south, west, north, east]`).
    pub fn within(self, bbox: [f64; 4]) -> Place {
        match self {
            Region::Sweden => Place::sweden(bbox),
            Region::France => {
                let [south, west, north, east] = bbox;
                let (lat, lon) = ((south + north) / 2.0, (west + east) / 2.0);
                let (e, n) = lambert93::forward(lat, lon);
                let frame = Frame::Lambert93 { e0: e, n0: n };
                let [x0, z0] = frame.project(south, west);
                let [x1, z1] = frame.project(north, east);
                Place::france(frame, ((x1 - x0).abs().min((z1 - z0).abs()) / 2.0).round())
            }
        }
    }

    /// The Lambert-93 window `[e0, n0, e1, n1]` of a tile of France's grid, grown by `margin`
    /// metres; `None` in Sweden, whose grid is not aligned with its data's.
    pub fn tile_window(self, tile: grid::Tile, margin: f64) -> Option<dem::Window> {
        let Frame::Lambert93 { e0, n0 } = self.grid() else { return None };
        let (x, z) = tile.corner();
        Some([e0 + x - margin, n0 - z - grid::TILE - margin, e0 + x + grid::TILE + margin, n0 - z + margin])
    }

    /// `(lat, lon)` of the region's projected `(easting, northing)`: the inverse of `source`.
    pub fn degrees(self, e: f64, n: f64) -> (f64, f64) {
        match self {
            Region::Sweden => dem::sweref99tm_inverse(e, n),
            Region::France => lambert93::inverse(e, n),
        }
    }

    /// The projected coordinates, `(easting, northing)` metres, that the region's height model and
    /// point cloud use for `(lat, lon)`.
    pub fn source(self, lat: f64, lon: f64) -> (f64, f64) {
        match self {
            Region::Sweden => dem::sweref99tm(lat, lon),
            Region::France => lambert93::forward(lat, lon),
        }
    }
}

/// Where a world is: its region, its frame, its half-side and the box that holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub region: Region,
    pub frame: Frame,
    /// Half the side of the playable square, metres.
    pub half: f64,
    /// `[south, west, north, east]` around the square, for the downloads that ask by degrees.
    pub bbox: [f64; 4],
}

impl Place {
    /// A Swedish world: the frame and half-side `assemble::frame` finds for `bbox`, and the box as it
    /// was given, so the downloads ask for exactly what they always did.
    pub fn sweden(bbox: [f64; 4]) -> Place {
        let (origin, half) = assemble::frame(bbox);
        Place { region: Region::Sweden, frame: Frame::Equirect { origin }, half, bbox }
    }

    /// A French world in `frame`, with the box around its square.
    pub fn france(frame: Frame, half: f64) -> Place {
        Place { region: Region::France, frame, half, bbox: enclosing(frame, half) }
    }

    /// The region's `(easting, northing)` of local `(x, z)`.
    pub fn source(&self, x: f64, z: f64) -> (f64, f64) {
        match self.frame {
            // Lambert-93 is the source's own frame: no trip through degrees.
            Frame::Lambert93 { e0, n0 } if self.region == Region::France => (e0 + x, n0 - z),
            frame => {
                let (lat, lon) = frame.unproject(x, z);
                self.region.source(lat, lon)
            }
        }
    }
}

/// `[south, west, north, east]` around the `half`-metre square centred on `frame`'s origin. The
/// square need not be a rectangle of degrees (Lambert-93's meridians converge), so its sides are
/// sampled too.
pub fn enclosing(frame: Frame, half: f64) -> [f64; 4] {
    let mut box_ = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for i in 0..=8 {
        let t = -half + 2.0 * half * f64::from(i) / 8.0;
        for (x, z) in [(t, -half), (t, half), (-half, t), (half, t)] {
            let (lat, lon) = frame.unproject(x, z);
            box_ = [box_[0].min(lat), box_[1].min(lon), box_[2].max(lat), box_[3].max(lon)];
        }
    }
    // Rounded outward, so the same square always gives the same text and never loses a sliver.
    let [south, west, north, east] = box_;
    let (down, up) = (|v: f64| (v * 1e6).floor() / 1e6, |v: f64| (v * 1e6).ceil() / 1e6);
    [down(south), down(west), up(north), up(east)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_is_french_or_swedish() {
        assert_eq!(Region::at(48.86, 2.35), Region::France);
        assert_eq!(Region::at(63.82, 20.23), Region::Sweden);
    }

    #[test]
    fn a_french_world_is_a_true_square_in_lambert_93() {
        let place = Region::France.around(47.39, 0.69, 500.0);
        let Frame::Lambert93 { e0, n0 } = place.frame else { panic!("{:?}", place.frame) };
        assert_eq!(place.source(500.0, -500.0), (e0 + 500.0, n0 + 500.0));
        let [south, west, north, east] = place.bbox;
        for (x, z) in [(-500.0, -500.0), (500.0, 500.0), (500.0, -500.0), (-500.0, 500.0)] {
            let (lat, lon) = place.frame.unproject(x, z);
            assert!((south..=north).contains(&lat) && (west..=east).contains(&lon));
        }
    }

    #[test]
    fn a_french_tile_window_is_the_one_the_world_reads() {
        // The importer reads tile (col, row) of a painted world at `place.source` of its corners.
        let tiles = [grid::Tile { gx: -688, gy: -392 }, grid::Tile { gx: -687, gy: -390 }];
        let layout = grid::layout(Region::France, &tiles).unwrap();
        let (col, row) = (1.0, 2.0);
        let (x0, z0) = (-layout.place.half + col * grid::TILE, -layout.place.half + row * grid::TILE);
        let (e, n) = layout.place.source(x0, z0 + grid::TILE);
        let window = Region::France.tile_window(tiles[1], 0.0).unwrap();
        assert!((window[0] - e).abs() < 1e-6 && (window[1] - n).abs() < 1e-6, "{window:?} {e} {n}");
        assert_eq!(Region::Sweden.tile_window(tiles[0], 0.0), None);
    }

    #[test]
    fn a_swedish_world_keeps_its_box() {
        let bbox = assemble::bbox_around(63.84, 20.31, 1375.0);
        let place = Region::Sweden.within(bbox);
        assert_eq!(place.bbox, bbox);
        assert_eq!(place.half, assemble::frame(bbox).1);
    }
}
