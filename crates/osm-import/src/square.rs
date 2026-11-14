//! Squaring a building's footprint. Outlines from OpenStreetMap and orienteering maps are drawn by
//! hand: corners that should be right angles are a few degrees off, and a straight wall has a kink
//! or a short jog in it. Walls built on such an outline sit skewed under a roof fitted to its
//! rectangle. A footprint whose every wall runs within `SNAP_ANGLE` of one direction or its right
//! angle is redrawn with exact right angles here; any other (a round tower, a wedge on a street
//! corner) is left as drawn.

type P = [f64; 2];

/// Furthest a wall may lean off the building's main direction, or its right angle, to be squared, radians (12°).
const SNAP_ANGLE: f64 = 12.0 * std::f64::consts::PI / 180.0;
/// Two walls one after the other along the same direction are one wall when they are this close, metres.
const MERGE: f64 = 0.4;
/// A wall this short or shorter is a jog in the drawing, not a wall, metres: it goes, and its neighbours meet.
const MIN_EDGE: f64 = 0.6;
/// The squared outline may differ in area by this share of the drawn one at most.
const AREA_SHARE: f64 = 0.08;
/// Furthest a corner of the squared outline may lie from the drawn one, or a drawn corner from the squared, metres.
const MOST_MOVED: f64 = 1.0;

/// The direction a squared footprint's walls run along: its unit vector `[x, z]`. The other
/// walls run at right angles to it.
pub fn direction(ring: &[P]) -> P {
    // Edge angles taken four times over, so walls at right angles to each other count the same way.
    let (mut c, mut s) = (0.0, 0.0);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz);
        if length < 1e-9 {
            continue;
        }
        let angle = dz.atan2(dx);
        c += length * (4.0 * angle).cos();
        s += length * (4.0 * angle).sin();
    }
    let angle = s.atan2(c) / 4.0;
    [angle.cos(), angle.sin()]
}

/// Whether every wall of `ring` runs along `u` or at right angles to it, within a hair: a squared footprint.
pub fn is_square(ring: &[P], u: P) -> bool {
    (0..ring.len()).all(|i| {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz);
        let along = (dx * u[0] + dz * u[1]).abs() / length.max(1e-12);
        length < 1e-9 || !(0.01..=0.9999).contains(&along)
    })
}

/// One wall of the outline being squared: whether it runs along the main direction (`0`) or
/// across it (`1`), where it lies across that (its offset, metres), and how long it is.
#[derive(Clone, Copy, Debug)]
struct Wall {
    axis: usize,
    offset: f64,
    length: f64,
}

/// `ring` (local metres, either winding, not closed) with its walls made exactly parallel or at
/// right angles, short jogs taken out and nearly straight runs made straight, in the same winding.
/// `None` when a wall leans too far off the right angles to be one of them, or the squared outline
/// strays too far from the drawn one: the footprint is then built as drawn.
pub fn square(ring: &[P]) -> Option<Vec<P>> {
    if ring.len() < 4 {
        return None;
    }
    let u = direction(ring);
    let v = [-u[1], u[0]];
    let local = |p: P| [p[0] * u[0] + p[1] * u[1], p[0] * v[0] + p[1] * v[1]];
    let mut walls: Vec<Wall> = Vec::new();
    for i in 0..ring.len() {
        let (a, b) = (local(ring[i]), local(ring[(i + 1) % ring.len()]));
        let (da, db) = (b[0] - a[0], b[1] - a[1]);
        let length = da.hypot(db);
        if length < 1e-6 {
            continue;
        }
        // Along `u` the wall's lean is atan(db / da); across it, atan(da / db).
        let axis = if da.abs() >= db.abs() { 0 } else { 1 };
        let lean = if axis == 0 { (db / da).atan() } else { (da / db).atan() };
        if lean.abs() > SNAP_ANGLE {
            return None;
        }
        // A wall along `u` keeps its place across it (its `v`), and the other way about.
        let offset = if axis == 0 { (a[1] + b[1]) / 2.0 } else { (a[0] + b[0]) / 2.0 };
        walls.push(Wall { axis, offset, length });
    }
    // Merge runs along one direction, and take out jogs, until the walls turn at every corner.
    loop {
        let n = walls.len();
        if n < 4 {
            return None;
        }
        if let Some(i) = (0..n).find(|&i| walls[i].axis == walls[(i + 1) % n].axis) {
            let j = (i + 1) % n;
            let (a, b) = (walls[i], walls[j]);
            if (a.offset - b.offset).abs() > MOST_MOVED {
                return None;
            }
            let length = a.length + b.length;
            walls[i] = Wall { axis: a.axis, offset: (a.offset * a.length + b.offset * b.length) / length, length };
            walls.remove(j);
            continue;
        }
        let shortest = (0..n).min_by(|&i, &j| walls[i].length.total_cmp(&walls[j].length)).unwrap();
        if walls[shortest].length > MIN_EDGE {
            break;
        }
        // Its neighbours run the same way: the next pass merges them, if they are close enough.
        let (before, after) = (walls[(shortest + n - 1) % n], walls[(shortest + 1) % n]);
        if (before.offset - after.offset).abs() > MERGE.max(walls[shortest].length) + 1e-9 {
            break;
        }
        walls.remove(shortest);
    }
    if walls.len() % 2 == 1 {
        return None;
    }
    // A corner is where a wall starts: where the one before it meets it.
    let back = |[a, b]: P| [a * u[0] + b * v[0], a * u[1] + b * v[1]];
    let corners: Vec<P> = (0..walls.len())
        .map(|i| {
            let (w, next) = (walls[(i + walls.len() - 1) % walls.len()], walls[i]);
            if w.axis == 0 { back([next.offset, w.offset]) } else { back([w.offset, next.offset]) }
        })
        .collect();
    let (before, after) = (area(ring), area(&corners));
    if after.signum() != before.signum() || (after - before).abs() > AREA_SHARE * before.abs() || !simple(&corners) {
        return None;
    }
    let near = |p: P, ring: &[P]| (0..ring.len()).any(|i| distance_to_segment(p, ring[i], ring[(i + 1) % ring.len()]) <= MOST_MOVED);
    if !corners.iter().all(|&p| near(p, ring)) || !ring.iter().all(|&p| near(p, &corners)) {
        return None;
    }
    Some(corners)
}

/// Signed area, positive counter-clockwise (x right, z up).
fn area(ring: &[P]) -> f64 {
    (0..ring.len()).map(|i| {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f64>() / 2.0
}

fn distance_to_segment(p: P, a: P, b: P) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dz * dz;
    let t = if length > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length).clamp(0.0, 1.0) } else { 0.0 };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dz)
}

/// Whether no two walls of `ring` that are not neighbours touch.
fn simple(ring: &[P]) -> bool {
    let n = ring.len();
    let cross = |o: P, a: P, b: P| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (a, b, c, d) = (ring[i], ring[(i + 1) % n], ring[j], ring[(j + 1) % n]);
            let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
            if d1 * d2 <= 0.0 && d3 * d4 <= 0.0 && (d1 != 0.0 || d2 != 0.0) {
                return false;
            }
        }
    }
    // Walls that double back on each other make corners in one place.
    (0..n).all(|i| (ring[i][0] - ring[(i + 1) % n][0]).hypot(ring[i][1] - ring[(i + 1) % n][1]) > 1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turned(points: &[P], angle: f64, at: P) -> Vec<P> {
        let (s, c) = angle.sin_cos();
        points.iter().map(|p| [at[0] + p[0] * c - p[1] * s, at[1] + p[0] * s + p[1] * c]).collect()
    }

    /// Whether every corner of `ring` is a right angle.
    fn right_angled(ring: &[P]) -> bool {
        (0..ring.len()).all(|i| {
            let (a, b, c) = (ring[(i + ring.len() - 1) % ring.len()], ring[i], ring[(i + 1) % ring.len()]);
            let (e, f) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            (e[0] * f[0] + e[1] * f[1]).abs() < 1e-6 * e[0].hypot(e[1]) * f[0].hypot(f[1])
        })
    }

    #[test]
    fn a_skewed_rectangle_comes_out_square() {
        // 12 × 8 m turned 0.4 rad, its corners pushed so they are 3° and 8° off square.
        let drawn = turned(&[[0.0, 0.0], [12.0, 0.4], [12.1, 8.0], [-0.5, 8.3]], 0.4, [100.0, -40.0]);
        let squared = square(&drawn).expect("squared");
        assert_eq!(squared.len(), 4);
        assert!(right_angled(&squared), "{squared:?}");
        assert!((area(&squared) - area(&drawn)).abs() < 0.05 * area(&drawn));
        let u = direction(&squared);
        assert!(is_square(&squared, u));
    }

    #[test]
    fn an_l_shape_keeps_its_six_corners() {
        let drawn = turned(&[[0.0, 0.0], [14.0, 0.1], [14.0, 6.0], [6.1, 6.0], [6.0, 12.0], [0.1, 12.1]], -1.0, [0.0, 0.0]);
        let squared = square(&drawn).expect("squared");
        assert_eq!(squared.len(), 6);
        assert!(right_angled(&squared));
        assert!(area(&squared) > 0.0, "the winding is kept");
    }

    #[test]
    fn a_short_jog_is_smoothed_away() {
        // A 10 × 6 house whose front wall has a 0.3 m step in it halfway along.
        let drawn = vec![[0.0, 0.0], [5.0, 0.0], [5.0, 0.3], [10.0, 0.3], [10.0, 6.0], [0.0, 6.0]];
        let squared = square(&drawn).expect("squared");
        assert_eq!(squared.len(), 4, "{squared:?}");
        assert!(right_angled(&squared));
    }

    #[test]
    fn a_wall_with_a_kink_is_made_straight() {
        let drawn = vec![[0.0, 0.0], [5.0, 0.1], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
        assert_eq!(square(&drawn).expect("squared").len(), 4);
    }

    #[test]
    fn a_round_tower_and_a_wedge_are_left_as_drawn() {
        let round: Vec<P> = (0..16).map(|i| { let a = i as f64 / 16.0 * std::f64::consts::TAU; [5.0 * a.cos(), 5.0 * a.sin()] }).collect();
        assert_eq!(square(&round), None);
        assert_eq!(square(&[[0.0, 0.0], [10.0, 0.0], [0.0, 8.0]]), None);
        assert_eq!(square(&[[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [3.0, 9.0]]), None);
    }

    #[test]
    fn a_square_footprint_is_unchanged() {
        let drawn = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 6.0], [0.0, 6.0]];
        let squared = square(&drawn).unwrap();
        for (a, b) in squared.iter().zip(&drawn) {
            assert!((a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9, "{squared:?}");
        }
    }
}
