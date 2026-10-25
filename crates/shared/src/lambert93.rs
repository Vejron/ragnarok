//! Lambert-93 (EPSG:2154), the French national grid: a Lambert conformal conic on GRS 80 with
//! standard parallels 44° and 49°, in metres. IGN's data (LiDAR HD, the MNT) is in it, and French
//! worlds use it as their frame (`world::Frame::Lambert93`).
//!
//! The formulas are IGN's (notes ALG0001-ALG0004 of NTG 71): the isometric latitude, the cone's
//! radius from it and back.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4};

/// A Lambert conformal conic in IGN's form: exponent `n`, constant `c`, the pole at `(xs, ys)`,
/// central meridian `lon0` (radians) and the ellipsoid's first eccentricity `e`.
#[derive(Clone, Copy, Debug)]
pub struct Conic {
    pub n: f64,
    pub c: f64,
    pub xs: f64,
    pub ys: f64,
    pub lon0: f64,
    pub e: f64,
}

/// GRS 80's first eccentricity.
const E_GRS80: f64 = 0.081_819_191_042_815_8;

/// Lambert-93: IGN's published constants.
pub const LAMBERT93: Conic = Conic { n: 0.725_607_765_053_267, c: 11_754_255.426_096, xs: 700_000.0, ys: 12_655_612.049_876, lon0: 3.0 * std::f64::consts::PI / 180.0, e: E_GRS80 };

/// The isometric latitude of `phi` (radians).
fn isometric(phi: f64, e: f64) -> f64 {
    let s = e * phi.sin();
    ((FRAC_PI_4 + phi / 2.0).tan() * ((1.0 - s) / (1.0 + s)).powf(e / 2.0)).ln()
}

/// The latitude (radians) whose isometric latitude is `l`, by IGN's fixed-point iteration.
fn latitude(l: f64, e: f64) -> f64 {
    let mut phi = 2.0 * l.exp().atan() - FRAC_PI_2;
    for _ in 0..12 {
        let s = e * phi.sin();
        let next = 2.0 * (((1.0 + s) / (1.0 - s)).powf(e / 2.0) * l.exp()).atan() - FRAC_PI_2;
        if (next - phi).abs() < 1e-12 {
            return next;
        }
        phi = next;
    }
    phi
}

impl Conic {
    /// Radians to `(x, y)` metres.
    pub fn forward_rad(&self, phi: f64, lambda: f64) -> (f64, f64) {
        let r = self.c * (-self.n * isometric(phi, self.e)).exp();
        let gamma = self.n * (lambda - self.lon0);
        (self.xs + r * gamma.sin(), self.ys - r * gamma.cos())
    }

    /// `(x, y)` metres to radians `(phi, lambda)`.
    pub fn inverse_rad(&self, x: f64, y: f64) -> (f64, f64) {
        let (dx, dy) = (x - self.xs, y - self.ys);
        let r = dx.hypot(dy);
        let gamma = dx.atan2(-dy);
        let l = -(r / self.c).ln() / self.n;
        (latitude(l, self.e), self.lon0 + gamma / self.n)
    }
}

/// WGS 84 / RGF 93 degrees to Lambert-93 `(easting, northing)` metres. RGF 93 and WGS 84 agree to
/// well under a metre, which is below anything a world resolves.
pub fn forward(lat: f64, lon: f64) -> (f64, f64) {
    LAMBERT93.forward_rad(lat.to_radians(), lon.to_radians())
}

/// Lambert-93 `(easting, northing)` metres to `(lat, lon)` degrees.
pub fn inverse(e: f64, n: f64) -> (f64, f64) {
    let (phi, lambda) = LAMBERT93.inverse_rad(e, n);
    (phi.to_degrees(), lambda.to_degrees())
}

/// Roughly mainland France and Corsica, `[south, west, north, east]` degrees: where a world is
/// French.
pub const FRANCE: [f64; 4] = [41.3, -5.2, 51.1, 9.6];

pub fn in_france(lat: f64, lon: f64) -> bool {
    let [s, w, n, e] = FRANCE;
    (s..=n).contains(&lat) && (w..=e).contains(&lon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_igns_worked_example() {
        // ALG0003: Lambert I (NTF, Clarke 1880 IGN), a point near Strasbourg.
        let lambert1 = Conic { n: 0.760_405_966_0, c: 11_603_796.976_7, xs: 600_000.0, ys: 5_657_616.674, lon0: 0.040_792_344_33, e: 0.082_483_256_8 };
        let (x, y) = lambert1.forward_rad(0.872_664_626_00, 0.145_512_099_00);
        assert!((x - 1_029_705.081_8).abs() < 0.01 && (y - 272_723.851).abs() < 0.01, "{x} {y}");
        // ALG0004, its inverse.
        let (phi, lambda) = lambert1.inverse_rad(1_029_705.083, 272_723.849);
        assert!((phi - 0.872_664_626).abs() < 1e-9 && (lambda - 0.145_512_099).abs() < 1e-9, "{phi} {lambda}");
    }

    #[test]
    fn the_false_origin_is_46_5_north_3_east() {
        let (e, n) = forward(46.5, 3.0);
        assert!((e - 700_000.0).abs() < 1e-3 && (n - 6_600_000.0).abs() < 1e-3, "{e} {n}");
    }

    #[test]
    fn the_inverse_returns_the_degrees() {
        for (lat, lon) in [(48.8566, 2.3522), (43.6, 1.44), (47.2184, -1.5536), (48.58, 7.75), (42.7, 9.45), (50.95, 1.85)] {
            let (e, n) = forward(lat, lon);
            let (back_lat, back_lon) = inverse(e, n);
            assert!((back_lat - lat).abs() < 1e-9 && (back_lon - lon).abs() < 1e-9, "{lat},{lon} -> {back_lat},{back_lon}");
        }
    }

    #[test]
    fn distances_are_true_to_a_fraction_of_a_percent() {
        // 1 km steps north and east, near Paris, Toulouse and Lille. Lambert-93's scale is within
        // 0.3 % of true in mainland France, and the degree lengths used here are good to 0.1 %.
        for (lat, lon) in [(48.85, 2.35), (43.6, 1.44), (50.63, 3.06)] {
            let (e0, n0) = forward(lat, lon);
            let north = forward(lat + 1000.0 / 111_132.0, lon);
            let east = forward(lat, lon + 1000.0 / (111_320.0 * lat.to_radians().cos()));
            let (dn, de) = ((north.0 - e0).hypot(north.1 - n0), (east.0 - e0).hypot(east.1 - n0));
            assert!((dn - 1000.0).abs() < 4.0 && (de - 1000.0).abs() < 4.0, "{lat}: {dn} {de}");
        }
    }

    #[test]
    fn north_is_up_on_the_central_meridian() {
        let (e0, n0) = forward(46.0, 3.0);
        let (e1, n1) = forward(46.01, 3.0);
        assert!((e1 - e0).abs() < 1e-6 && n1 > n0);
    }
}
