//! Roofs for churches. A simple church is a nave, a long rectangle under one steep gable, with or
//! without lower annexes against it (a sacristy, a porch). Fitted freely (`roof::fit`) it comes out
//! wrong: the ridge is put down the middle of the rectangle round the whole footprint, annexe and
//! all, and a roof steeper than a house's is refused. Here the footprint is cut into its rectangles,
//! the biggest is the nave and gets its gable down its own middle, as steep as the laser shows it,
//! and what stands against each of its sides is cut off along that wall and roofed by itself.
//!
//! A church that is not so simple (a tower, a plan that is no long rectangle, a roof the gable
//! misses) is left to the general fit.

use crate::catalogue::{self, cost_of};
use crate::roof::{self, Roof, Shape, Wing};

type P = [f64; 2];

/// The slope of a nave's roof, rise per run: 31° to 70°. An annexe's may be as steep.
const PITCH: (f64, f64) = (0.6, 2.75);
/// A nave is at least this many times as long as it is wide, and this share of the footprint.
const NAVE_LONG: f64 = 1.5;
const NAVE_SHARE: f64 = 0.6;
/// A footprint that is not square is one rectangle, the nave, if it fills this share of the rectangle round it.
const FILLED: f64 = 0.85;
/// Most the nave's returns may miss its gable (`catalogue::cost_of`, metres squared; a steep roof
/// is missed by more than a shallow one, a return a little to the side being much higher or lower),
/// and the share of a flat roof's miss it may be.
const MOST_MISS: f64 = 0.3;
const OF_FLAT: f64 = 0.5;
/// Fewest returns a nave or an annexe is fitted to.
const MIN_POINTS: usize = 10;
/// An annexe whose returns stand this far over the nave's ridge, metres, is a tower.
const TOWER: f64 = 1.0;
/// Returns this near the nave's wall, metres, are left out of an annexe's fit: they are the nave's
/// eaves hanging over it, and its wall.
const BY_NAVE: f64 = 1.0;
/// An annexe that fits flat while its returns span this much, metres, between the lowest and the
/// highest tenths (`LOW`, `HIGH`, shares of them sorted) is no flat roof but a steep one the few
/// returns on it could not show: it gets a hip from its eaves at `LOW`, as steep as the nave's roof
/// or as its returns reach.
const NOT_FLAT: f64 = 2.0;
const LOW: f64 = 0.15;
const HIGH: f64 = 0.9;

/// The roof of the church on `ring` (local metres, counter-clockwise, not closed) for the laser
/// returns `points` (`[x, z, y, code]`, as `roof::fit` takes them), or `None` for too few returns or
/// a church that is not a nave with lower annexes.
pub fn fit(ring: &[P], points: &[[f64; 4]]) -> Option<Roof> {
    // The returns with foliage left out, but not yet what stands over a first fit: that fit is of
    // the wrong roof.
    let (_, returns) = roof::roof_returns(ring, points)?;
    let area = (0..ring.len()).map(|i| { let (a, b) = (ring[i], ring[(i + 1) % ring.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>().abs() / 2.0;
    let parts = match catalogue::rectangles(ring) {
        Some(parts) => parts,
        None => vec![roof::rectangle(ring).filter(|wing| area >= FILLED * 4.0 * wing.half[0] * wing.half[1])?],
    };
    let nave = *parts.first()?;
    let (long, wide) = (nave.half[0], nave.half[1]);
    if long < NAVE_LONG * wide || 4.0 * long * wide < NAVE_SHARE * area {
        return None;
    }
    // A footprint drawn a little off where the church stands: the returns are moved back under it.
    let under = |returns: &[[f64; 3]]| -> Vec<[f64; 3]> { returns.iter().copied().filter(|r| nave.contains([r[0], r[1]])).collect() };
    let shift = catalogue::misplaced(&under(&returns), &[nave]);
    let returns: Vec<[f64; 3]> = returns.iter().map(|r| [r[0] - shift[0], r[1] - shift[1], r[2]]).collect();

    // The nave: its eaves and its slope, the ridge down its middle.
    let mut over = under(&returns);
    if over.len() < MIN_POINTS {
        return None;
    }
    let inward = |p: P| (wide - nave.local(p).1.abs()).max(0.0);
    let gable = |set: &[[f64; 3]]| {
        let rows: Vec<[f64; 3]> = set.iter().map(|r| [1.0, inward([r[0], r[1]]), 0.0]).collect();
        let ys: Vec<f64> = set.iter().map(|r| r[2]).collect();
        let (c, _) = roof::solve_rows(&rows, &ys);
        (c[0], c[1])
    };
    let (mut eave, mut pitch) = gable(&over);
    // What stands well over it is a tree or a spirelet, if it is little; a lot is a tower, and no gable fits it.
    let kept: Vec<[f64; 3]> = over.iter().copied().filter(|r| r[2] - (eave + pitch * inward([r[0], r[1]])) <= roof::CANOPY).collect();
    if kept.len() < MIN_POINTS || (over.len() - kept.len()) as f64 > roof::MAX_STRAY * over.len() as f64 {
        return None;
    }
    over = kept;
    (eave, pitch) = gable(&over);
    if !(PITCH.0..=PITCH.1).contains(&pitch) {
        return None;
    }
    let miss = cost_of(&over, &|p| eave + pitch * inward(p));
    let level = over.iter().map(|r| r[2]).sum::<f64>() / over.len() as f64;
    if miss > MOST_MISS || miss > OF_FLAT * cost_of(&over, &|_| level) {
        return None;
    }
    let ridge = eave + pitch * wide;
    let mut roof = Roof::Leaf(Shape::Gable { y: ridge, at: nave.centre, ridge: nave.u, pitch });

    // The annexes: what lies beyond each side of the nave, then beyond each end, is cut off along
    // that wall and roofed by itself.
    let axes = [nave.u, [-nave.u[1], nave.u[0]]];
    let local = |p: P| { let (a, b) = nave.local(p); [a, b] };
    for (axis, sign) in [(1, 1.0), (1, -1.0), (0, 1.0), (0, -1.0)] {
        // Beyond an end is beyond it whatever side it is on; beyond a side only alongside the nave.
        let beyond = |p: P| { let at = local(p); sign * at[axis] > nave.half[axis] && (axis == 0 || at[0].abs() <= long) };
        let group: Vec<&Wing> = parts[1..].iter().filter(|part| beyond(part.centre)).collect();
        if group.is_empty() {
            continue;
        }
        // The rectangle round them, laid out along the nave or across it, long side first.
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for part in &group {
            for (i, j) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                let at = local(part.world(i * part.half[0], j * part.half[1]));
                (lo, hi) = ([lo[0].min(at[0]), lo[1].min(at[1])], [hi[0].max(at[0]), hi[1].max(at[1])]);
            }
        }
        let centre = nave.world((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
        let half = [(hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0];
        let around = if half[0] >= half[1] { Wing { centre, u: axes[0], half } } else { Wing { centre, u: axes[1], half: [half[1], half[0]] } };
        let theirs: Vec<[f64; 3]> = returns.iter().copied().filter(|r| beyond([r[0], r[1]]) && sign * local([r[0], r[1]])[axis] > nave.half[axis] + BY_NAVE).collect();
        let mut heights: Vec<f64> = theirs.iter().map(|r| r[2]).collect();
        heights.sort_by(f64::total_cmp);
        let share = |of: f64| heights.get(((heights.len() as f64 * of) as usize).min(heights.len().saturating_sub(1))).copied();
        if share(HIGH).is_some_and(|high| high > ridge + TOWER) {
            return None;
        }
        let mut part = if theirs.len() >= MIN_POINTS {
            roof::fit_over(&around, &theirs, (group.len() as u32 - 1).min(2), PITCH.1)
        } else {
            // Too small for the laser to have seen: flat, at what it did see or at the nave's eaves.
            Roof::Leaf(Shape::Flat { y: share(0.5).unwrap_or(eave) })
        };
        if let (Roof::Leaf(Shape::Flat { .. }), Some(low), Some(high)) = (&part, share(LOW), share(HIGH)) && high - low > NOT_FLAT {
            let steep = pitch.min((high - low) / around.half[1]);
            part = Roof::Leaf(Shape::Hip { y: low + steep * around.half[1], at: around.centre, ridge: around.u, pitch: steep, half: around.half[0] - around.half[1] });
        }
        let n = [axes[axis][0] * sign, axes[axis][1] * sign];
        let o = [nave.centre[0] + n[0] * nave.half[axis], nave.centre[1] + n[1] * nave.half[axis]];
        roof = Roof::Cut { o, n, low: Box::new(roof), high: Box::new(part) };
    }
    Some(roof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::CollisionMesh;

    /// A nave 30 × 12 m along x about the origin, and a sacristy 7 × 7 m against its far side near one end.
    fn church() -> Vec<P> {
        vec![[-15.0, -6.0], [15.0, -6.0], [15.0, 6.0], [13.0, 6.0], [13.0, 13.0], [6.0, 13.0], [6.0, 6.0], [-15.0, 6.0]]
    }

    /// The nave's roof: eaves 9 m up and a slope of 1.4 (54°), so its ridge is 17.4 m up.
    fn nave(p: P) -> f64 {
        9.0 + 1.4 * (6.0 - p[1].abs()).max(0.0)
    }

    fn inside(ring: &[P], [x, z]: P) -> bool {
        let mut hit = false;
        for i in 0..ring.len() {
            let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
            if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
                hit = !hit;
            }
        }
        hit
    }

    /// Laser returns every 0.7 m over `ring`, heights from `height`, with a little noise.
    fn sample(ring: &[P], height: impl Fn(P) -> f64) -> Vec<[f64; 4]> {
        let (lo, hi) = ring.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
        let mut out = Vec::new();
        let mut k = 0u64;
        let mut x = lo[0] + 0.13;
        while x < hi[0] {
            let mut z = lo[1] + 0.17;
            while z < hi[1] {
                if inside(ring, [x, z]) {
                    k += 1;
                    let noise = ((k * 2654435761) % 1000) as f64 / 1000.0 * 0.2 - 0.1;
                    out.push([x, z, height([x, z]) + noise, 0.0]);
                }
                z += 0.7;
            }
            x += 0.7;
        }
        out
    }

    /// The church's roof as it is: the nave's gable, and the sacristy flat 4.5 m up.
    fn truth(p: P) -> f64 {
        if p[1] > 6.0 { 4.5 } else { nave(p) }
    }

    #[test]
    fn a_churchs_ridge_runs_down_its_nave_not_the_box_round_its_annexe() {
        let ring = church();
        let roof = fit(&ring, &sample(&ring, truth)).unwrap();
        assert!((roof.height([0.0, 0.0]) - 17.4).abs() < 0.3, "ridge {} m up", roof.height([0.0, 0.0]));
        // The same height either side of the nave's middle: the general fit puts its ridge 3.5 m to the far side.
        for z in [-5.0, 5.0] {
            assert!((roof.height([-8.0, z]) - 10.4).abs() < 0.3, "{} m up at z {z}", roof.height([-8.0, z]));
        }
        let general = roof::fit(&ring, &sample(&ring, truth)).unwrap();
        assert!((general.height([-8.0, -5.0]) - general.height([-8.0, 5.0])).abs() > 1.0 || (general.height([0.0, 0.0]) - 17.4).abs() > 1.0, "the general fit is as good");
    }

    #[test]
    fn a_steep_church_roof_is_kept_steep() {
        let ring = vec![[-15.0, -6.0], [15.0, -6.0], [15.0, 6.0], [-15.0, 6.0]];
        let steep = |p: P| 8.0 + 2.0 * (6.0 - p[1].abs()).max(0.0);
        let Some(Roof::Leaf(Shape::Gable { y, pitch, ridge, .. })) = fit(&ring, &sample(&ring, steep)) else { panic!("not one gable") };
        assert!((pitch - 2.0).abs() < 0.1, "a slope of {pitch}");
        assert!((y - 20.0).abs() < 0.4 && ridge[0].abs() > 0.99, "ridge {y} m up along {ridge:?}");
    }

    #[test]
    fn a_sacristy_is_roofed_lower_than_the_nave() {
        let ring = church();
        let roof = fit(&ring, &sample(&ring, truth)).unwrap();
        assert!((roof.height([9.5, 9.5]) - 4.5).abs() < 0.3, "the sacristy is {} m up", roof.height([9.5, 9.5]));
        // And its mesh is whole: the nave's wall stands over the sacristy's roof.
        let mut mesh = CollisionMesh::default();
        let parts = roof::build(&mut mesh, &ring, 0.0, &roof);
        roof::tests::no_cracks(&mesh, parts);
        assert!((parts.top - 17.4).abs() < 0.3);
    }

    #[test]
    fn a_steep_sacristy_the_laser_hardly_saw_gets_a_hip_from_its_eaves() {
        // Its pyramid roof, eaves 3 m up and top 8.6 m, seen by a return every 2 m, and its walls by a few more.
        let ring = church();
        let pyramid = |p: P| 3.0 + 1.6 * (3.5 - (p[0] - 9.5).abs().max((p[1] - 9.5).abs())).max(0.0);
        let mut points: Vec<[f64; 4]> = sample(&ring, nave).into_iter().filter(|p| p[1] <= 6.0).collect();
        for i in 0..4 {
            for j in 0..3 {
                let p = [6.6 + 2.0 * f64::from(i), 7.6 + 2.0 * f64::from(j)];
                points.push([p[0], p[1], pyramid(p), 0.0]);
            }
        }
        points.extend([[6.2, 12.9, 1.0, 0.0], [9.0, 12.9, 6.5, 0.0], [12.8, 12.8, 2.0, 0.0]]);
        let roof = fit(&ring, &points).unwrap();
        let (top, edge) = (roof.height([9.5, 9.5]), roof.height([9.5, 12.8]));
        assert!(top > edge + 3.0 && top < 17.4, "the sacristy rises from {edge} to {top}");
        assert!((roof.height([0.0, 0.0]) - 17.4).abs() < 0.3, "and the nave is as it was");
    }

    #[test]
    fn a_church_with_a_tower_is_left_to_the_general_fit() {
        // A tower 6 × 6 m, 25 m tall, against the west end.
        let ring = vec![[-15.0, -6.0], [15.0, -6.0], [15.0, 6.0], [-15.0, 6.0], [-15.0, 3.0], [-21.0, 3.0], [-21.0, -3.0], [-15.0, -3.0]];
        assert_eq!(fit(&ring, &sample(&ring, |p| if p[0] < -15.0 { 25.0 } else { nave(p) })), None);
        // And one standing in the nave's own rectangle.
        let ring = vec![[-15.0, -6.0], [15.0, -6.0], [15.0, 6.0], [-15.0, 6.0]];
        assert_eq!(fit(&ring, &sample(&ring, |p| if p[0] < -8.0 { 25.0 } else { nave(p) })), None);
    }

    #[test]
    fn a_hall_that_is_no_long_nave_is_left_to_the_general_fit() {
        let ring = vec![[-7.0, -6.0], [7.0, -6.0], [7.0, 6.0], [-7.0, 6.0]];
        assert_eq!(fit(&ring, &sample(&ring, |p| 6.0 + 0.8 * (6.0 - p[1].abs()))), None);
    }
}
