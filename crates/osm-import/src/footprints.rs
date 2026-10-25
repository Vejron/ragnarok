//! Buildings that OpenStreetMap does not have, found in a classified cloud's building returns
//! (IGN's class 6).
//!
//! France's OSM outlines come mostly from the cadastre and are better than any traced from the laser,
//! so OSM stays the outline wherever it has one. What the laser adds is everything it maps that OSM
//! misses: sheds, garages, new buildings. Per world tile, building returns are rasterised at
//! `STEP`, cells under or within `KNOWN_MARGIN` of an OSM outline are struck out, and every
//! connected region of what is left that is big and tall enough is traced into an outline.
//!
//! A region is cut into pieces on a fixed `PIECE` lattice, each its own building. What OSM misses in
//! a city centre is often a whole block (mapped there as a multipolygon, which is not read), and as
//! one building it would be one huge structure to check whenever it is hit, with its courtyards
//! roofed over. In pieces it is a row of houses, the courtyards mostly open.

use crate::field::STEP;
use crate::roof::Cloud;

type P = [f64; 2];

/// Smallest building taken from the laser, square metres.
const MIN_AREA: f64 = 20.0;
/// Lowest a roof may stand over the ground, metres: lower is a car port's shadow or a wall.
const MIN_HEIGHT: f64 = 2.0;
/// Returns this close to an OSM outline, metres, are that building's (eaves, a misaligned outline).
const KNOWN_MARGIN: f64 = 1.5;
/// How far around a tile its returns are read, metres, so a building across its edge is whole.
const REACH: f64 = 40.0;
/// Side of the lattice regions are cut on, metres; it divides the 256 m tile, so the cuts are the
/// same whichever tile a region is found from.
const PIECE: f64 = 32.0;
/// Smallest piece kept, square metres: what the cut leaves of a corner.
const MIN_PIECE: f64 = 2.0;
/// Most a traced outline may stray from the cells' edge, metres.
const SIMPLIFY: f64 = 0.6;

/// The outlines of buildings in the tiles `rects` (`[x0, z0, x1, z1]`, local metres) that none of
/// `known` covers. `source` maps local metres to the cloud's coordinates, `ground` gives the ground's
/// height. A building belongs to the tile its centre is in, so one across a tile's edge is found once.
/// Each building comes as its pieces on the `PIECE` lattice.
pub fn missing(cloud: &Cloud, source: &(impl Fn(f64, f64) -> (f64, f64) + Sync), rects: &[[f64; 4]], known: &[Vec<P>], ground: &(impl Fn(f64, f64) -> f64 + Sync)) -> Vec<Vec<Vec<P>>> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(rect) = rects.get(at) else { break };
                    let outlines = in_tile(cloud, source, *rect, known, ground);
                    found.lock().unwrap().push((at, outlines));
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap();
    found.sort_by_key(|(at, _)| *at);
    found.into_iter().flat_map(|(_, outlines)| outlines).collect()
}

fn in_tile(cloud: &Cloud, source: &impl Fn(f64, f64) -> (f64, f64), [x0, z0, x1, z1]: [f64; 4], known: &[Vec<P>], ground: &impl Fn(f64, f64) -> f64) -> Vec<Vec<Vec<P>>> {
    let (ox, oz) = (x0 - REACH, z0 - REACH);
    let (cols, rows) = (((x1 - x0 + 2.0 * REACH) / STEP).round() as usize, ((z1 - z0 + 2.0 * REACH) / STEP).round() as usize);
    let around = [[ox, oz], [ox, oz + rows as f64 * STEP], [ox + cols as f64 * STEP, oz + rows as f64 * STEP], [ox + cols as f64 * STEP, oz]];
    let mut top = vec![f32::NAN; cols * rows];
    for p in cloud.over(&around, source) {
        let (i, j) = (((p[0] - ox) / STEP).floor(), ((p[1] - oz) / STEP).floor());
        if i < 0.0 || j < 0.0 || i as usize >= cols || j as usize >= rows {
            continue;
        }
        let cell = &mut top[j as usize * cols + i as usize];
        if cell.is_nan() || *cell < p[2] as f32 {
            *cell = p[2] as f32;
        }
    }
    let centre = |i: usize, j: usize| [ox + (i as f64 + 0.5) * STEP, oz + (j as f64 + 0.5) * STEP];
    let mut free: Vec<bool> = (0..cols * rows)
        .map(|k| {
            let (i, j) = (k % cols, k / cols);
            let [x, z] = centre(i, j);
            !top[k].is_nan() && f64::from(top[k]) - ground(x, z) >= MIN_HEIGHT
        })
        .collect();
    // Strike out what the known outlines explain.
    let window = [ox, oz, ox + cols as f64 * STEP, oz + rows as f64 * STEP];
    for ring in known {
        let (lo, hi) = bounds(ring);
        if hi[0] + KNOWN_MARGIN < window[0] || lo[0] - KNOWN_MARGIN > window[2] || hi[1] + KNOWN_MARGIN < window[1] || lo[1] - KNOWN_MARGIN > window[3] {
            continue;
        }
        let cell = |v: f64, o: f64, n: usize| (((v - o) / STEP).floor().max(0.0) as usize).min(n - 1);
        for j in cell(lo[1] - KNOWN_MARGIN, oz, rows)..=cell(hi[1] + KNOWN_MARGIN, oz, rows) {
            for i in cell(lo[0] - KNOWN_MARGIN, ox, cols)..=cell(hi[0] + KNOWN_MARGIN, ox, cols) {
                let k = j * cols + i;
                if free[k] && (inside(centre(i, j), ring) || distance(centre(i, j), ring) < KNOWN_MARGIN) {
                    free[k] = false;
                }
            }
        }
    }
    // Close one-cell gaps between returns, so a roof is one region.
    let before = free.clone();
    for j in 1..rows.saturating_sub(1) {
        for i in 1..cols.saturating_sub(1) {
            if !before[j * cols + i] && free_near(&before, cols, i, j) >= 5 {
                free[j * cols + i] = true;
            }
        }
    }
    // Regions, 4-connected.
    let mut seen = vec![false; cols * rows];
    let mut out = Vec::new();
    for start in 0..cols * rows {
        if !free[start] || seen[start] {
            continue;
        }
        let mut region = vec![start];
        seen[start] = true;
        let mut k = 0;
        while k < region.len() {
            let c = region[k];
            k += 1;
            let (i, j) = (c % cols, c / cols);
            let mut visit = |n: usize| {
                if free[n] && !seen[n] {
                    seen[n] = true;
                    region.push(n);
                }
            };
            if i > 0 {
                visit(c - 1);
            }
            if i + 1 < cols {
                visit(c + 1);
            }
            if j > 0 {
                visit(c - cols);
            }
            if j + 1 < rows {
                visit(c + cols);
            }
        }
        if (region.len() as f64) * STEP * STEP < MIN_AREA {
            continue;
        }
        let n = region.len() as f64;
        let mid = region.iter().fold([0.0, 0.0], |s, &c| {
            let p = centre(c % cols, c / cols);
            [s[0] + p[0] / n, s[1] + p[1] / n]
        });
        if !(x0 <= mid[0] && mid[0] < x1 && z0 <= mid[1] && mid[1] < z1) {
            continue;
        }
        // The pieces: the region's cells by lattice square, each square's cells by connection.
        let square = |c: usize| {
            let [x, z] = centre(c % cols, c / cols);
            ((x / PIECE).floor() as i64, (z / PIECE).floor() as i64)
        };
        let mut by_square: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
        for &c in &region {
            by_square.entry(square(c)).or_default().push(c);
        }
        let mut building = Vec::new();
        for cells in by_square.into_values() {
            for piece in connected(&cells, cols) {
                if (piece.len() as f64) * STEP * STEP < MIN_PIECE {
                    continue;
                }
                if let Some(ring) = trace(&piece, cols, [ox, oz]) {
                    building.push(ring);
                }
            }
        }
        if !building.is_empty() {
            out.push(building);
        }
    }
    out
}

/// Footprints over this many square metres are cut on the lattice too (`pieces`).
pub const BIG: f64 = 1000.0;

/// `ring` (a footprint, local metres) cut into its pieces on the `PIECE` lattice, traced from its
/// cells: exact along the cuts, within `SIMPLIFY` of the outline elsewhere. A footprint inside one
/// square is itself.
pub fn pieces(ring: &[P]) -> Vec<Vec<P>> {
    let (lo, hi) = bounds(ring);
    if (lo[0] / PIECE).floor() == (hi[0] / PIECE).floor() && (lo[1] / PIECE).floor() == (hi[1] / PIECE).floor() {
        return vec![ring.to_vec()];
    }
    // The grid on the lattice's own steps, one cell of margin, so a cell never straddles a cut.
    let (ox, oz) = (((lo[0] / STEP).floor() - 1.0) * STEP, ((lo[1] / STEP).floor() - 1.0) * STEP);
    let (cols, rows) = (((hi[0] - ox) / STEP).ceil() as usize + 2, ((hi[1] - oz) / STEP).ceil() as usize + 2);
    let centre = |c: usize| [ox + ((c % cols) as f64 + 0.5) * STEP, oz + ((c / cols) as f64 + 0.5) * STEP];
    let mut by_square: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
    for c in 0..cols * rows {
        let p = centre(c);
        if inside(p, ring) {
            by_square.entry(((p[0] / PIECE).floor() as i64, (p[1] / PIECE).floor() as i64)).or_default().push(c);
        }
    }
    let mut out = Vec::new();
    for cells in by_square.into_values() {
        for piece in connected(&cells, cols) {
            if (piece.len() as f64) * STEP * STEP < MIN_PIECE {
                continue;
            }
            out.extend(trace(&piece, cols, [ox, oz]));
        }
    }
    out
}

/// `cells` (indices of a grid `cols` wide) split into 4-connected groups.
fn connected(cells: &[usize], cols: usize) -> Vec<Vec<usize>> {
    let set: std::collections::HashSet<usize> = cells.iter().copied().collect();
    let mut seen = std::collections::HashSet::new();
    let mut groups = Vec::new();
    for &start in cells {
        if !seen.insert(start) {
            continue;
        }
        let mut group = vec![start];
        let mut k = 0;
        while k < group.len() {
            let c = group[k];
            k += 1;
            let mut near = vec![c + 1, c + cols];
            if c % cols > 0 {
                near.push(c - 1);
            }
            if c >= cols {
                near.push(c - cols);
            }
            if c % cols + 1 == cols {
                near.retain(|&n| n != c + 1);
            }
            for n in near {
                if set.contains(&n) && seen.insert(n) {
                    group.push(n);
                }
            }
        }
        groups.push(group);
    }
    groups
}

fn free_near(cells: &[bool], cols: usize, i: usize, j: usize) -> usize {
    [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)]
        .iter()
        .filter(|&&(di, dj)| cells[(j as isize + dj) as usize * cols + (i as isize + di) as usize])
        .count()
}

/// The outer outline of a region of cells, simplified, in local metres; `None` if it collapses.
fn trace(region: &[usize], cols: usize, [ox, oz]: P) -> Option<Vec<P>> {
    use std::collections::{HashMap, HashSet};
    let cells: HashSet<(i64, i64)> = region.iter().map(|&c| ((c % cols) as i64, (c / cols) as i64)).collect();
    // Each cell side with no cell beyond it, directed so that every loop turns the same way.
    let mut from: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    for &(i, j) in &cells {
        let mut edge = |a: (i64, i64), b: (i64, i64)| from.entry(a).or_default().push(b);
        if !cells.contains(&(i, j - 1)) {
            edge((i + 1, j), (i, j));
        }
        if !cells.contains(&(i + 1, j)) {
            edge((i + 1, j + 1), (i + 1, j));
        }
        if !cells.contains(&(i, j + 1)) {
            edge((i, j + 1), (i + 1, j + 1));
        }
        if !cells.contains(&(i - 1, j)) {
            edge((i, j), (i, j + 1));
        }
    }
    let mut starts: Vec<(i64, i64)> = from.keys().copied().collect();
    starts.sort_unstable();
    let mut best: Option<(f64, Vec<(i64, i64)>)> = None;
    for start in starts {
        while from.get(&start).is_some_and(|ends| !ends.is_empty()) {
            let mut ring = vec![start];
            let mut at = start;
            while let Some(next) = from.get_mut(&at).and_then(|ends| ends.pop()) {
                at = next;
                if at == start {
                    break;
                }
                ring.push(at);
            }
            let area = (0..ring.len())
                .map(|k| {
                    let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                    (a.0 * b.1 - b.0 * a.1) as f64
                })
                .sum::<f64>()
                .abs()
                / 2.0;
            if best.as_ref().is_none_or(|(known, _)| area > *known) {
                best = Some((area, ring));
            }
        }
    }
    let (_, ring) = best?;
    let points: Vec<P> = ring.iter().map(|&(i, j)| [ox + i as f64 * STEP, oz + j as f64 * STEP]).collect();
    let simple = simplify(&points, SIMPLIFY);
    (simple.len() >= 3).then_some(simple)
}

/// Douglas-Peucker on a closed ring: split at the vertex farthest from the first, simplify each half.
fn simplify(ring: &[P], tolerance: f64) -> Vec<P> {
    if ring.len() < 4 {
        return ring.to_vec();
    }
    let far = (1..ring.len()).max_by(|&a, &b| dist2(ring[0], ring[a]).total_cmp(&dist2(ring[0], ring[b]))).unwrap_or(1);
    let mut out = Vec::new();
    chain(&ring[..=far], tolerance, &mut out);
    out.pop();
    let mut back: Vec<P> = ring[far..].to_vec();
    back.push(ring[0]);
    chain(&back, tolerance, &mut out);
    out.pop();
    out
}

/// Appends the simplified open chain `points` to `out`, both ends included.
fn chain(points: &[P], tolerance: f64, out: &mut Vec<P>) {
    let (a, b) = (points[0], points[points.len() - 1]);
    let (mut worst, mut at) = (0.0, 0);
    for (k, &p) in points.iter().enumerate().take(points.len() - 1).skip(1) {
        let d = segment_distance(p, a, b);
        if d > worst {
            (worst, at) = (d, k);
        }
    }
    if worst > tolerance {
        chain(&points[..=at], tolerance, out);
        out.pop();
        chain(&points[at..], tolerance, out);
    } else {
        out.push(a);
        out.push(b);
    }
}

fn dist2(a: P, b: P) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)
}

fn segment_distance(p: P, a: P, b: P) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dz * dz;
    let t = if length > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length).clamp(0.0, 1.0) } else { 0.0 };
    dist2(p, [a[0] + dx * t, a[1] + dz * t]).sqrt()
}

fn distance(p: P, ring: &[P]) -> f64 {
    (0..ring.len()).map(|k| segment_distance(p, ring[k], ring[(k + 1) % ring.len()])).fold(f64::MAX, f64::min)
}

fn bounds(points: &[P]) -> (P, P) {
    points.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]))
}

fn inside([x, z]: P, ring: &[P]) -> bool {
    let mut hit = false;
    for k in 0..ring.len() {
        let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            hit = !hit;
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(cells: &[(usize, usize)], cols: usize) -> Vec<usize> {
        cells.iter().map(|&(i, j)| j * cols + i).collect()
    }

    fn area(ring: &[P]) -> f64 {
        (0..ring.len()).map(|k| ring[k][0] * ring[(k + 1) % ring.len()][1] - ring[(k + 1) % ring.len()][0] * ring[k][1]).sum::<f64>().abs() / 2.0
    }

    #[test]
    fn a_rectangle_of_cells_traces_to_four_corners() {
        let cells: Vec<(usize, usize)> = (2..12).flat_map(|i| (3..9).map(move |j| (i, j))).collect();
        let ring = trace(&region(&cells, 20), 20, [0.0, 0.0]).unwrap();
        assert_eq!(ring.len(), 4, "{ring:?}");
        assert!((area(&ring) - 10.0 * 6.0 * STEP * STEP).abs() < 1e-9);
    }

    #[test]
    fn an_l_keeps_its_notch() {
        let mut cells: Vec<(usize, usize)> = (0..20).flat_map(|i| (0..6).map(move |j| (i, j))).collect();
        cells.extend((0..6).flat_map(|i| (6..20).map(move |j| (i, j))));
        let ring = trace(&region(&cells, 30), 30, [0.0, 0.0]).unwrap();
        assert_eq!(ring.len(), 6, "{ring:?}");
        assert!((area(&ring) - (120.0 + 84.0) * STEP * STEP).abs() < 1e-9);
    }

    #[test]
    fn a_courtyard_is_traced_by_its_outside() {
        let cells: Vec<(usize, usize)> = (0..20).flat_map(|i| (0..20).map(move |j| (i, j))).filter(|&(i, j)| !((6..14).contains(&i) && (6..14).contains(&j))).collect();
        let ring = trace(&region(&cells, 30), 30, [0.0, 0.0]).unwrap();
        assert!((area(&ring) - 400.0 * STEP * STEP).abs() < 1e-9);
    }

    #[test]
    fn a_region_splits_into_connected_groups() {
        // Two bars side by side, not touching, and a cell touching the first only at a corner.
        let cells = region(&[(0, 0), (1, 0), (2, 0), (0, 2), (1, 2), (3, 1)], 10);
        let mut groups: Vec<usize> = connected(&cells, 10).iter().map(Vec::len).collect();
        groups.sort_unstable();
        assert_eq!(groups, vec![1, 2, 3]);
    }

    #[test]
    fn a_big_footprint_is_cut_on_the_lattice() {
        // 80 m × 40 m from (-8, 4): squares from x = -32 to 96 and z = 0 to 64, so 4 × 2 pieces.
        let ring = [[-8.0, 4.0], [72.0, 4.0], [72.0, 44.0], [-8.0, 44.0]];
        let pieces = pieces(&ring);
        assert_eq!(pieces.len(), 8);
        let total: f64 = pieces.iter().map(|p| area(p)).sum();
        assert!((total - 3200.0).abs() < 1e-6, "{total}");
        // A footprint inside one square is left as it is.
        let small = [[1.0, 1.0], [9.0, 1.0], [9.0, 9.0]];
        assert_eq!(super::pieces(&small), vec![small.to_vec()]);
    }

    #[test]
    fn a_jagged_edge_is_straightened() {
        // A rectangle whose long side steps a cell in and out, as scan edges do.
        let cells: Vec<(usize, usize)> = (0..40).flat_map(|i| (0..10 + (i % 2)).map(move |j| (i, j))).collect();
        let ring = trace(&region(&cells, 50), 50, [0.0, 0.0]).unwrap();
        assert!(ring.len() <= 6, "{ring:?}");
    }
}
