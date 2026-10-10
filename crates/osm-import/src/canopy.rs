//! Trees from the laser cloud: a canopy height model, treetops, crowns, and an archetype per tree.
//!
//! Lantmäteriet's "Laserdata Skog" is classified only into ground (2) and everything else (1), so
//! buildings, cars and vegetation are not separated. Buildings are masked with their OSM footprints;
//! what stands more than a few metres above the terrain outside them is taken to be a tree.

use crate::lidar::Point;
use shared::world::*;

/// Raster cell, metres. The cloud is about one non-ground return per m², so finer is empty.
const CELL: f64 = 1.0;
/// Lowest thing that counts as a tree, metres above the terrain. Shrubs and hedges stay below.
pub const MIN_HEIGHT: f64 = 3.0;
/// Crown cells are those at least this fraction of the treetop's height.
const CROWN_FRACTION: f64 = 0.5;
/// A crown never reaches farther than this from its top, metres.
const MAX_CROWN_RADIUS: f64 = 8.0;
/// Segments with a larger area are something else (a roof the footprints missed), m².
const MAX_CROWN_AREA: f64 = 200.0;

/// A tree found in the cloud, in SWEREF 99 TM.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detected {
    pub e: f64,
    pub n: f64,
    /// Metres above the terrain.
    pub height: f64,
    /// Radius of the circle with the crown's area, metres.
    pub crown: f64,
    /// Share of the crown's returns that came from a pulse with several returns.
    pub multi: f64,
    /// Mean return intensity over the crown.
    pub intensity: f64,
    /// Height of the lowest returns in the crown, as a fraction of the tree's height.
    pub base: f64,
}

pub struct Canopy {
    e0: f64,
    n0: f64,
    cols: usize,
    rows: usize,
    /// Highest return per cell, metres (any datum), NaN when none.
    top: Vec<f32>,
    /// Lowest non-ground return per cell, NaN when none.
    low: Vec<f32>,
    count: Vec<u16>,
    multi: Vec<u16>,
    intensity: Vec<u32>,
    masked: Vec<bool>,
}

impl Canopy {
    pub fn new([e0, n0, e1, n1]: [f64; 4]) -> Self {
        let (cols, rows) = (((e1 - e0) / CELL).ceil() as usize + 1, ((n1 - n0) / CELL).ceil() as usize + 1);
        let size = cols * rows;
        Self { e0, n0, cols, rows, top: vec![f32::NAN; size], low: vec![f32::NAN; size], count: vec![0; size], multi: vec![0; size], intensity: vec![0; size], masked: vec![false; size] }
    }

    fn index(&self, e: f64, n: f64) -> Option<usize> {
        let (c, r) = (((e - self.e0) / CELL).floor(), ((n - self.n0) / CELL).floor());
        (c >= 0.0 && r >= 0.0 && (c as usize) < self.cols && (r as usize) < self.rows).then(|| r as usize * self.cols + c as usize)
    }

    /// Takes a return into the model; ground and noise are not canopy.
    pub fn add(&mut self, p: &Point) {
        if matches!(p.class, 2 | 7 | 18) {
            return;
        }
        let Some(i) = self.index(p.e, p.n) else { return };
        let z = p.z as f32;
        if self.top[i].is_nan() || z > self.top[i] {
            self.top[i] = z;
        }
        if self.low[i].is_nan() || z < self.low[i] {
            self.low[i] = z;
        }
        self.count[i] = self.count[i].saturating_add(1);
        if p.returns > 1 {
            self.multi[i] = self.multi[i].saturating_add(1);
        }
        self.intensity[i] = self.intensity[i].saturating_add(u32::from(p.intensity));
    }

    /// Keeps trees out of `ring` (SWEREF 99 TM, a building's footprint), with `margin` metres around
    /// it for eaves.
    pub fn mask(&mut self, ring: &[(f64, f64)], margin: f64) {
        if ring.len() < 3 {
            return;
        }
        let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
        for &(e, n) in ring {
            lo = (lo.0.min(e), lo.1.min(n));
            hi = (hi.0.max(e), hi.1.max(n));
        }
        let c0 = ((lo.0 - margin - self.e0) / CELL).floor().max(0.0) as usize;
        let c1 = (((hi.0 + margin - self.e0) / CELL).floor().max(0.0) as usize).min(self.cols - 1);
        let r0 = ((lo.1 - margin - self.n0) / CELL).floor().max(0.0) as usize;
        let r1 = (((hi.1 + margin - self.n0) / CELL).floor().max(0.0) as usize).min(self.rows - 1);
        for r in r0..=r1 {
            for c in c0..=c1 {
                let at = (self.e0 + (c as f64 + 0.5) * CELL, self.n0 + (r as f64 + 0.5) * CELL);
                if inside(at, ring) || distance_to_ring(at, ring) <= margin {
                    self.masked[r * self.cols + c] = true;
                }
            }
        }
    }

    /// The trees at least `min_height` metres tall, where `ground(e, n)` is the terrain's elevation.
    pub fn detect(&self, ground: impl Fn(f64, f64) -> Option<f32>, min_height: f64) -> Vec<Detected> {
        let (cols, rows) = (self.cols, self.rows);
        let size = cols * rows;
        let centre = |c: usize, r: usize| (self.e0 + (c as f64 + 0.5) * CELL, self.n0 + (r as f64 + 0.5) * CELL);

        // Height above the terrain per cell; NaN where no return and no terrain is known.
        let mut chm = vec![f32::NAN; size];
        let mut terrain = vec![f32::NAN; size];
        for r in 0..rows {
            for c in 0..cols {
                let i = r * cols + c;
                if self.masked[i] || self.top[i].is_nan() {
                    continue;
                }
                let (e, n) = centre(c, r);
                if let Some(g) = ground(e, n) {
                    terrain[i] = g;
                    chm[i] = (self.top[i] - g).max(0.0);
                }
            }
        }
        // Fill empty cells from the cells around them (the cloud is sparse: ~1 return per m²).
        let measured = chm.clone();
        for r in 1..rows.saturating_sub(1) {
            for c in 1..cols.saturating_sub(1) {
                let i = r * cols + c;
                if !measured[i].is_nan() || self.masked[i] {
                    continue;
                }
                let (mut sum, mut known) = (0.0f32, 0);
                for dr in 0..3 {
                    for dc in 0..3 {
                        let j = (r + dr - 1) * cols + c + dc - 1;
                        if !measured[j].is_nan() {
                            sum += measured[j];
                            known += 1;
                        }
                    }
                }
                if known >= 3 {
                    chm[i] = sum / known as f32;
                }
            }
        }
        // Light smoothing, so one stray return does not make a tree.
        let mut smooth = vec![0.0f32; size];
        for r in 1..rows.saturating_sub(1) {
            for c in 1..cols.saturating_sub(1) {
                let (mut sum, mut known) = (0.0f32, 0);
                for dr in 0..3 {
                    for dc in 0..3 {
                        let v = chm[(r + dr - 1) * cols + c + dc - 1];
                        if !v.is_nan() {
                            sum += v;
                            known += 1;
                        }
                    }
                }
                if known > 0 && !chm[r * cols + c].is_nan() {
                    smooth[r * cols + c] = sum / known as f32;
                }
            }
        }

        // Treetops: the highest cell within a window that grows with the height.
        let min = min_height as f32;
        let mut tops: Vec<usize> = Vec::new();
        for r in 1..rows.saturating_sub(1) {
            for c in 1..cols.saturating_sub(1) {
                let i = r * cols + c;
                let h = smooth[i];
                if h < min {
                    continue;
                }
                let reach = ((0.12 * f64::from(h)).round() as usize).clamp(1, 3);
                let mut highest = true;
                'window: for rr in r.saturating_sub(reach)..=(r + reach).min(rows - 1) {
                    for cc in c.saturating_sub(reach)..=(c + reach).min(cols - 1) {
                        let j = rr * cols + cc;
                        // Ties go to the first cell in scan order.
                        if j != i && (smooth[j] > h || (smooth[j] == h && j < i)) {
                            highest = false;
                            break 'window;
                        }
                    }
                }
                if highest {
                    tops.push(i);
                }
            }
        }

        // Crowns: grow each top downhill, highest cells first, until a cell is less than half the
        // top's height or too far from it.
        let mut label = vec![u32::MAX; size];
        for (k, &i) in tops.iter().enumerate() {
            label[i] = k as u32;
        }
        let mut order: Vec<usize> = (0..size).filter(|&i| smooth[i] >= 0.5 * min && label[i] == u32::MAX).collect();
        order.sort_by(|&a, &b| smooth[b].total_cmp(&smooth[a]).then(a.cmp(&b)));
        let position = |i: usize| ((i % cols) as f64, (i / cols) as f64);
        for i in order {
            let (c, r) = (i % cols, i / cols);
            if c == 0 || r == 0 || c + 1 >= cols || r + 1 >= rows {
                continue;
            }
            // The labelled neighbour with the highest value gives the label.
            let mut best: Option<(f32, u32)> = None;
            for dr in 0..3 {
                for dc in 0..3 {
                    let j = (r + dr - 1) * cols + c + dc - 1;
                    if j != i && label[j] != u32::MAX && best.is_none_or(|(h, _)| smooth[j] > h) {
                        best = Some((smooth[j], label[j]));
                    }
                }
            }
            let Some((_, k)) = best else { continue };
            let top = tops[k as usize];
            let ((tc, tr), (cc, rc)) = (position(top), position(i));
            let far = ((tc - cc).powi(2) + (tr - rc).powi(2)).sqrt() * CELL > MAX_CROWN_RADIUS;
            if smooth[i] >= (CROWN_FRACTION as f32) * smooth[top] && !far {
                label[i] = k;
            }
        }

        // Per tree: the crown's cells.
        struct Sum {
            cells: u32,
            peak: f32,
            measured: u32,
            count: u32,
            multi: u32,
            intensity: u64,
            low: f64,
            lows: u32,
        }
        let mut sums: Vec<Sum> = tops.iter().map(|_| Sum { cells: 0, peak: 0.0, measured: 0, count: 0, multi: 0, intensity: 0, low: 0.0, lows: 0 }).collect();
        for i in 0..size {
            let l = label[i];
            if l == u32::MAX {
                continue;
            }
            let s = &mut sums[l as usize];
            s.cells += 1;
            if !measured[i].is_nan() {
                s.peak = s.peak.max(measured[i]);
                s.measured += 1;
                s.count += u32::from(self.count[i]);
                s.multi += u32::from(self.multi[i]);
                s.intensity += u64::from(self.intensity[i]);
                if !self.low[i].is_nan() && !terrain[i].is_nan() {
                    s.low += f64::from(self.low[i] - terrain[i]);
                    s.lows += 1;
                }
            }
        }
        let mut out = Vec::new();
        for (&top, s) in tops.iter().zip(&sums) {
            let height = f64::from(s.peak);
            let area = f64::from(s.cells) * CELL * CELL;
            // Mostly interpolated cells, or too little crown: noise.
            if height < min_height || s.count == 0 || s.measured * 2 < s.cells || area > MAX_CROWN_AREA || s.cells < 3 {
                continue;
            }
            let (c, r) = (top % cols, top / cols);
            let (e, n) = centre(c, r);
            out.push(Detected {
                e,
                n,
                height,
                crown: (area / std::f64::consts::PI).sqrt().clamp(0.8, MAX_CROWN_RADIUS),
                multi: f64::from(s.multi) / f64::from(s.count),
                intensity: s.intensity as f64 / f64::from(s.count),
                base: if s.lows > 0 { (s.low / f64::from(s.lows) / height).clamp(0.0, 1.0) } else { 0.0 },
            });
        }
        out
    }
}

fn inside((e, n): (f64, f64), ring: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let ((ei, ni), (ej, nj)) = (ring[i], ring[j]);
        if (ni > n) != (nj > n) && e < (ej - ei) * (n - ni) / (nj - ni) + ei {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn distance_to_ring((e, n): (f64, f64), ring: &[(f64, f64)]) -> f64 {
    let mut best = f64::MAX;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx * dx + dy * dy;
        let t = if length > 0.0 { (((e - a.0) * dx + (n - a.1) * dy) / length).clamp(0.0, 1.0) } else { 0.0 };
        best = best.min(((e - a.0 - t * dx).powi(2) + (n - a.1 - t * dy).powi(2)).sqrt());
    }
    best
}

/// Laser intensity above which a tree is a conifer (birch's upper quartile is ~730, pine's and
/// spruce's lower ~750), as fitted by the `calibration` test.
const CONIFER_INTENSITY: f64 = 760.0;
/// Pine's crown starts higher up the stem than spruce's (0.5 vs 0.4 of the height).
const PINE_BASE: f64 = 0.47;

/// The archetype for a detected tree. Thresholds were fitted against the Umeå register (see
/// `calibration` in the tests): the near-infrared intensity is what tells conifers from broadleaves,
/// the height of the crown's lowest returns pine from spruce.
pub fn classify(tree: &Detected) -> u8 {
    let ratio = tree.height / tree.crown;
    if tree.height < 9.0 {
        return if tree.intensity >= 800.0 { TREE_ORNAMENTAL } else { TREE_LOWWIDE };
    }
    if tree.intensity >= CONIFER_INTENSITY {
        return if tree.base >= PINE_BASE { TREE_PINE } else { TREE_SPRUCE };
    }
    if tree.crown >= 6.5 {
        return TREE_BROADLEAF;
    }
    if ratio >= 7.0 {
        return TREE_COLUMNAR;
    }
    TREE_BIRCH
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_: f64, _: f64) -> Option<f32> {
        Some(0.0)
    }

    /// Fills the canopy with an ellipsoidal crown, as returns on a ~0.7 m lattice.
    fn crown(canopy: &mut Canopy, e: f64, n: f64, height: f64, radius: f64) {
        let mut x = -radius;
        while x <= radius {
            let mut y = -radius;
            while y <= radius {
                let d = (x * x + y * y).sqrt() / radius;
                if d <= 1.0 {
                    let z = height * (1.0 - d * d * 0.8);
                    canopy.add(&Point { e: e + x, n: n + y, z, class: 1, ret: 1, returns: 1, intensity: 100 });
                }
                y += 0.7;
            }
            x += 0.7;
        }
    }

    #[test]
    fn finds_separate_trees_with_their_size() {
        let mut canopy = Canopy::new([0.0, 0.0, 60.0, 60.0]);
        crown(&mut canopy, 15.0, 15.0, 12.0, 3.0);
        crown(&mut canopy, 40.0, 30.0, 18.0, 2.0);
        let mut trees = canopy.detect(flat, MIN_HEIGHT);
        trees.sort_by(|a, b| a.e.total_cmp(&b.e));
        assert_eq!(trees.len(), 2, "{trees:?}");
        assert!((trees[0].e - 15.0).abs() < 1.5 && (trees[0].n - 15.0).abs() < 1.5);
        assert!((trees[0].height - 12.0).abs() < 1.0, "{}", trees[0].height);
        assert!((trees[1].height - 18.0).abs() < 1.0);
        assert!(trees[0].crown > 1.2 && trees[0].crown < 4.0, "{}", trees[0].crown);
    }

    #[test]
    fn two_neighbouring_crowns_are_two_trees() {
        let mut canopy = Canopy::new([0.0, 0.0, 40.0, 40.0]);
        crown(&mut canopy, 15.0, 20.0, 12.0, 3.0);
        crown(&mut canopy, 22.0, 20.0, 12.0, 3.0);
        assert_eq!(canopy.detect(flat, MIN_HEIGHT).len(), 2);
    }

    #[test]
    fn low_things_and_buildings_are_not_trees() {
        let mut canopy = Canopy::new([0.0, 0.0, 60.0, 60.0]);
        crown(&mut canopy, 15.0, 15.0, 2.0, 3.0);
        // A roof: flat at 9 m over a 10 × 10 m footprint, masked.
        for i in 0..14 {
            for j in 0..14 {
                canopy.add(&Point { e: 30.0 + f64::from(i) * 0.7, n: 30.0 + f64::from(j) * 0.7, z: 9.0, class: 1, ret: 1, returns: 1, intensity: 100 });
            }
        }
        canopy.mask(&[(30.0, 30.0), (40.0, 30.0), (40.0, 40.0), (30.0, 40.0)], 1.0);
        assert!(canopy.detect(flat, MIN_HEIGHT).is_empty());
    }

    #[test]
    fn height_is_above_the_terrain() {
        let mut canopy = Canopy::new([0.0, 0.0, 40.0, 40.0]);
        crown(&mut canopy, 20.0, 20.0, 112.0, 3.0);
        let trees = canopy.detect(|_, _| Some(100.0), MIN_HEIGHT);
        assert_eq!(trees.len(), 1);
        assert!((trees[0].height - 12.0).abs() < 1.0);
    }

    #[test]
    fn archetypes_follow_the_shape() {
        let tree = |height, crown, intensity, base| Detected { e: 0.0, n: 0.0, height, crown, multi: 0.0, intensity, base };
        assert_eq!(classify(&tree(12.0, 3.0, 650.0, 0.45)), TREE_BIRCH);
        assert_eq!(classify(&tree(14.0, 7.0, 650.0, 0.45)), TREE_BROADLEAF);
        assert_eq!(classify(&tree(5.0, 2.0, 900.0, 0.5)), TREE_ORNAMENTAL);
        assert_eq!(classify(&tree(16.0, 2.0, 650.0, 0.45)), TREE_COLUMNAR);
        assert_eq!(classify(&tree(15.0, 3.5, 820.0, 0.55)), TREE_PINE);
        assert_eq!(classify(&tree(15.0, 3.5, 800.0, 0.38)), TREE_SPRUCE);
    }

    /// Fits the archetype rules: detects trees over 1 km² of Umeå and prints, per register species
    /// group, the quartiles of what the laser saw for the register trees it found.
    /// `LM_USER=… LM_PASS=… cargo test --release -p osm-import calibration -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn calibration() {
        let (user, password) = (std::env::var("LM_USER").unwrap(), std::env::var("LM_PASS").unwrap());
        crate::cache::init(Some(std::path::PathBuf::from("../../.cache/osm-import")), false);
        let var = |name: &str, default: f64| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
        let (lat, lon) = (var("CAL_LAT", 63.832127), var("CAL_LON", 20.168286));
        let bbox = crate::assemble::bbox_around(lat, lon, 600.0);
        let (e, n) = crate::dem::sweref99tm(lat, lon);
        let window = [e - 600.0, n - 600.0, e + 600.0, n + 600.0];
        let dem = crate::dem::Dem::from_stac(bbox, window, &user, &password).unwrap();
        let mut canopy = Canopy::new(window);
        crate::lidar::fetch(bbox, window, &user, &password, |p| canopy.add(&p)).unwrap();
        let detected = canopy.detect(|e, n| dem.mean(e, n, 0.5), MIN_HEIGHT);
        let register = crate::trees::fetch(bbox).unwrap();
        eprintln!("{} detected, {} in the register", detected.len(), register.len());

        let mut groups: std::collections::BTreeMap<u8, Vec<Detected>> = Default::default();
        for planted in &register {
            let (pe, pn) = crate::dem::sweref99tm(planted.lat, planted.lon);
            let near = detected
                .iter()
                .filter(|d| (d.e - pe).hypot(d.n - pn) < 2.0)
                .min_by(|a, b| (a.e - pe).hypot(a.n - pn).total_cmp(&(b.e - pe).hypot(b.n - pn)));
            if let Some(found) = near {
                groups.entry(planted.kind).or_default().push(*found);
            }
        }
        let quartiles = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            let at = |q: f64| v[((v.len() - 1) as f64 * q) as usize];
            format!("{:6.2} {:6.2} {:6.2}", at(0.25), at(0.5), at(0.75))
        };
        for (kind, found) in &groups {
            eprintln!("kind {kind}: {} found", found.len());
            if found.len() < 5 {
                continue;
            }
            for (name, f) in [
                ("height", (|d: &Detected| d.height) as fn(&Detected) -> f64),
                ("crown", |d| d.crown),
                ("ratio", |d| d.height / d.crown),
                ("multi", |d| d.multi),
                ("intens", |d| d.intensity),
                ("base", |d| d.base),
            ] {
                eprintln!("   {name:7} {}", quartiles(found.iter().map(f).collect()));
            }
        }
        // What the rules make of the register trees the laser found: rows register, columns rules.
        let mut matrix = [[0usize; 7]; 7];
        for (&kind, found) in &groups {
            for d in found {
                matrix[usize::from(kind).min(6)][usize::from(classify(d)).min(6)] += 1;
            }
        }
        eprintln!("register \\ rules: birch pine spruce ornam broad column lowwide");
        for (kind, row) in matrix.iter().enumerate() {
            eprintln!("   {kind}: {row:?}");
        }
        let all = |f: fn(&Detected) -> f64| quartiles(detected.iter().map(f).collect());
        eprintln!("all detected: height {} | crown {} | multi {} | intens {}", all(|d| d.height), all(|d| d.crown), all(|d| d.multi), all(|d| d.intensity));
    }
}
