//! Knolls and pits in the ground. The map marks them with a dot or a cup because they are too small
//! for its contours, and most are too small for the 1 m height model as well: on Kläppudden one knoll
//! in eight stands a metre tall in it, and no pit is that deep. So each is measured, and the ground is
//! raised or lowered by what it lacks. One the model already shows is left as it is.

use std::collections::{BTreeSet, HashMap};

/// A knoll or pit on the map, in SWEREF 99 TM. Metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landform {
    pub e: f64,
    pub n: f64,
    /// Half its length, along `angle`, and half its width.
    pub along: f64,
    pub across: f64,
    /// Degrees counter-clockwise from east.
    pub angle: f64,
    /// Height to stand above the ground around it; below it for a pit.
    pub target: f64,
    /// Above 1 the sides are gentle at the rim, below 1 they are steep.
    pub power: f64,
}

/// A cliff on the map, in SWEREF 99 TM. Metres.
#[derive(Clone, Debug, PartialEq)]
pub struct Cliff {
    pub points: Vec<[f64; 2]>,
    /// Height of the step.
    pub target: f64,
    /// Width of the face.
    pub face: f64,
}

/// How far either side of a cliff's line the height model is read.
const CLIFF_READ: f64 = 2.0;
/// Between readings along the line.
const CLIFF_STEP: f64 = 2.0;
/// A smaller step than this does not say which side is down.
const CLIFF_SURE: f64 = 0.2;
/// Over this far the ground on both sides returns to the model, so no bank builds up.
const CLIFF_FADE: f64 = 8.0;
/// Over this far a cliff's ends run out.
const CLIFF_TAPER: f64 = 3.0;

/// `x` from 0 to 1 with no corner at either end.
fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A cliff with what the ground lacks of it.
#[derive(Clone, Debug)]
struct Face {
    points: Vec<[f64; 2]>,
    /// Distance along the line of each point.
    lengths: Vec<f64>,
    /// `1.0` when the left of the line is the high side.
    high: f64,
    /// Half the step to add: up on the high side, down on the low.
    half: f64,
    face: f64,
}

impl Face {
    fn at(&self, e: f64, n: f64) -> f64 {
        let mut nearest: Option<(f64, f64, f64)> = None;
        for (i, pair) in self.points.windows(2).enumerate() {
            let [[ax, an], [bx, bn]] = [pair[0], pair[1]];
            let (dx, dn) = (bx - ax, bn - an);
            let length = dx.hypot(dn).max(1e-9);
            let along = (((e - ax) * dx + (n - an) * dn) / length).clamp(0.0, length);
            let (px, pn) = (ax + dx * along / length, an + dn * along / length);
            let reach = (e - px).hypot(n - pn);
            if nearest.is_none_or(|(best, ..)| reach < best) {
                let left = (dx * (n - an) - dn * (e - ax)) / length;
                nearest = Some((reach, left, self.lengths[i] + along));
            }
        }
        let Some((_, left, along)) = nearest else { return 0.0 };
        let total = self.lengths[self.lengths.len() - 1];
        let taper = smooth(along.min(total - along) / CLIFF_TAPER.min(total / 2.0));
        let across = 2.0 * smooth((left * self.high + self.face / 2.0) / self.face) - 1.0;
        self.half * across * (1.0 - smooth((left.abs() - self.face / 2.0) / CLIFF_FADE)) * taper
    }
}

/// The step across `cliff` at each reading along it: the left of the line less the right.
fn steps(cliff: &Cliff, height: &impl Fn(f64, f64) -> Option<f64>) -> Vec<f64> {
    let mut found = Vec::new();
    for pair in cliff.points.windows(2) {
        let [[ax, an], [bx, bn]] = [pair[0], pair[1]];
        let (dx, dn) = (bx - ax, bn - an);
        let length = dx.hypot(dn);
        if length < 1e-6 {
            continue;
        }
        let (ux, un) = (dx / length, dn / length);
        for i in 0..=(length / CLIFF_STEP) as usize {
            let at = i as f64 * CLIFF_STEP;
            let (e, n) = (ax + ux * at, an + un * at);
            if let (Some(left), Some(right)) = (height(e - un * CLIFF_READ, n + ux * CLIFF_READ), height(e + un * CLIFF_READ, n - ux * CLIFF_READ)) {
                found.push(left - right);
            }
        }
    }
    found
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

/// The median step of `cliffs` on `height`.
pub fn cliff_median(cliffs: &[Cliff], height: impl Fn(f64, f64) -> Option<f64>) -> f64 {
    median(cliffs.iter().map(|cliff| median(steps(cliff, &height)).abs()).collect())
}

/// How far past its edge the ground around a landform is read.
const RING_MARGIN: f64 = 3.0;
const RING_SAMPLES: usize = 16;
/// How far from the symbol the real bump may lie, metres. A mapper's dot is rarely exact.
const SNAP: i32 = 2;
/// What a metre away from the symbol must gain in height to be worth it, metres.
const SNAP_COST: f64 = 0.02;
/// Side of a lookup bin, metres.
const BIN: f64 = 16.0;

fn bin(e: f64, n: f64) -> (i64, i64) {
    ((e / BIN).floor() as i64, (n / BIN).floor() as i64)
}

/// A landform with what the ground lacks of it.
#[derive(Clone, Copy, Debug)]
struct Stamp {
    form: Landform,
    sin: f64,
    cos: f64,
    /// Added at the centre, signed.
    amount: f64,
}

impl Stamp {
    fn at(&self, e: f64, n: f64) -> f64 {
        let (dx, dy) = (e - self.form.e, n - self.form.n);
        let (along, across) = ((dx * self.cos + dy * self.sin) / self.form.along, (dy * self.cos - dx * self.sin) / self.form.across);
        let reach = along.hypot(across);
        if reach >= 1.0 {
            return 0.0;
        }
        self.amount * ((1.0 + (reach * std::f64::consts::PI).cos()) / 2.0).powf(self.form.power)
    }
}

/// The stamps of a map, measured against the height model.
#[derive(Debug, Default)]
pub struct Relief {
    stamps: Vec<Stamp>,
    bins: HashMap<(i64, i64), Vec<usize>>,
    faces: Vec<Face>,
    face_bins: HashMap<(i64, i64), Vec<usize>>,
}

/// How far `form` stands out of the ground around it where it is best shown, in the direction of its
/// target, and where that is. `None` off the height model.
fn measure(form: &Landform, height: &impl Fn(f64, f64) -> Option<f64>) -> Option<(f64, f64, f64)> {
    let radius = form.along.max(form.across) + RING_MARGIN;
    let mut around = 0.0;
    for i in 0..RING_SAMPLES {
        let (sin, cos) = (i as f64 / RING_SAMPLES as f64 * std::f64::consts::TAU).sin_cos();
        around += height(form.e + radius * cos, form.n + radius * sin)?;
    }
    around /= RING_SAMPLES as f64;
    let sign = form.target.signum();
    (-SNAP..=SNAP)
        .flat_map(|i| (-SNAP..=SNAP).map(move |j| (f64::from(i), f64::from(j))))
        .filter_map(|(i, j)| Some((sign * (height(form.e + i, form.n + j)? - around), form.e + i, form.n + j)))
        .max_by(|a, b| {
            // On even ground this stays on the symbol.
            let score = |at: &(f64, f64, f64)| at.0 - SNAP_COST * (at.1 - form.e).hypot(at.2 - form.n);
            score(a).total_cmp(&score(b))
        })
}

/// The median height of the knolls, and depth of the pits, on `height`: the check that the ground
/// got what it was meant to.
pub fn medians(forms: &[Landform], height: impl Fn(f64, f64) -> Option<f64>) -> [f64; 2] {
    [1.0, -1.0].map(|sign: f64| {
        let mut shown: Vec<f64> = forms.iter().filter(|form| form.target.signum() == sign).filter_map(|form| Some(measure(form, &height)?.0)).collect();
        shown.sort_by(f64::total_cmp);
        shown.get(shown.len() / 2).copied().unwrap_or(0.0)
    })
}

impl Relief {
    /// Measures `forms` on `height`, the height model at an easting and northing. Returns the relief
    /// and how many forms were already tall or deep enough.
    pub fn fit(forms: &[Landform], height: impl Fn(f64, f64) -> Option<f64>) -> (Relief, usize) {
        let mut relief = Relief::default();
        let mut enough = 0;
        for form in forms {
            let Some((shown, e, n)) = measure(form, &height) else { continue };
            let lacking = form.target.abs() - shown;
            if lacking <= 0.0 {
                enough += 1;
                continue;
            }
            let (sin, cos) = form.angle.to_radians().sin_cos();
            let index = relief.stamps.len();
            relief.stamps.push(Stamp { form: Landform { e, n, ..*form }, sin, cos, amount: lacking * form.target.signum() });
            let reach = form.along.max(form.across);
            let (lo, hi) = (bin(e - reach, n - reach), bin(e + reach, n + reach));
            for i in lo.0..=hi.0 {
                for j in lo.1..=hi.1 {
                    relief.bins.entry((i, j)).or_default().push(index);
                }
            }
        }
        (relief, enough)
    }

    /// Adds `cliffs`, measured on `height`. Returns how many the model already showed, and how many
    /// it could not say the low side of: those fall to the left of the line, where OCAD draws ticks.
    pub fn add_cliffs(&mut self, cliffs: &[Cliff], height: impl Fn(f64, f64) -> Option<f64>) -> (usize, usize) {
        let (mut enough, mut unsure) = (0, 0);
        for cliff in cliffs {
            let read = steps(cliff, &height);
            if read.is_empty() {
                continue;
            }
            let step = median(read);
            let sure = step.abs() >= CLIFF_SURE;
            unsure += usize::from(!sure);
            let high = if sure { step.signum() } else { -1.0 };
            let lacking = cliff.target - step * high;
            if lacking <= 0.0 {
                enough += 1;
                continue;
            }
            let mut lengths = vec![0.0];
            for pair in cliff.points.windows(2) {
                lengths.push(lengths[lengths.len() - 1] + (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]));
            }
            let index = self.faces.len();
            self.faces.push(Face { points: cliff.points.clone(), lengths, high, half: lacking / 2.0, face: cliff.face });
            let reach = cliff.face / 2.0 + CLIFF_FADE;
            let mut touched = BTreeSet::new();
            for pair in cliff.points.windows(2) {
                let (dx, dn) = (pair[1][0] - pair[0][0], pair[1][1] - pair[0][1]);
                let samples = (dx.hypot(dn) / (BIN / 2.0)).ceil().max(1.0) as usize;
                for s in 0..=samples {
                    let t = s as f64 / samples as f64;
                    let (e, n) = (pair[0][0] + dx * t, pair[0][1] + dn * t);
                    let (lo, hi) = (bin(e - reach, n - reach), bin(e + reach, n + reach));
                    touched.extend((lo.0..=hi.0).flat_map(|i| (lo.1..=hi.1).map(move |j| (i, j))));
                }
            }
            for key in touched {
                self.face_bins.entry(key).or_default().push(index);
            }
        }
        (enough, unsure)
    }

    /// What the stamps and cliffs add to the ground at an easting and northing. Where cliffs overlap
    /// the largest counts: two lines of one cliff are not a cliff twice the height.
    pub fn at(&self, e: f64, n: f64) -> f64 {
        let key = bin(e, n);
        let stamps: f64 = self.bins.get(&key).map_or(0.0, |stamps| stamps.iter().map(|&i| self.stamps[i].at(e, n)).sum());
        let faces = self.face_bins.get(&key).map_or(0.0, |faces| faces.iter().map(|&i| self.faces[i].at(e, n)).fold(0.0, |best: f64, change| if change.abs() > best.abs() { change } else { best }));
        stamps + faces
    }

    /// How many landforms the ground had to be changed for.
    pub fn len(&self) -> usize {
        self.stamps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stamps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOLL: Landform = Landform { e: 100.0, n: 100.0, along: 3.0, across: 3.0, angle: 0.0, target: 1.5, power: 1.0 };
    const PIT: Landform = Landform { target: -1.5, ..KNOLL };

    fn flat(_: f64, _: f64) -> Option<f64> {
        Some(10.0)
    }

    #[test]
    fn a_knoll_rises_out_of_flat_ground_to_its_target() {
        let (relief, enough) = Relief::fit(&[KNOLL], flat);
        assert_eq!((relief.len(), enough), (1, 0));
        assert!((relief.at(100.0, 100.0) - 1.5).abs() < 1e-9);
        assert!(relief.at(101.5, 100.0) > 0.0 && relief.at(101.5, 100.0) < 1.5);
        assert_eq!(relief.at(103.0, 100.0), 0.0);
        assert_eq!(relief.at(200.0, 100.0), 0.0);
    }

    #[test]
    fn a_pit_goes_down() {
        let (relief, _) = Relief::fit(&[PIT], flat);
        assert!((relief.at(100.0, 100.0) + 1.5).abs() < 1e-9);
    }

    fn hill(height: f64) -> impl Fn(f64, f64) -> Option<f64> {
        move |e, n| {
            let reach = (e - 100.0).hypot(n - 100.0);
            Some(if reach < 4.0 { height * (1.0 - reach / 4.0) } else { 0.0 })
        }
    }

    #[test]
    fn a_knoll_the_model_shows_is_left_alone() {
        let (relief, enough) = Relief::fit(&[KNOLL], hill(2.0));
        assert_eq!((relief.len(), enough), (0, 1));
        assert_eq!(relief.at(100.0, 100.0), 0.0);
    }

    #[test]
    fn a_half_shown_knoll_is_topped_up_to_its_target() {
        let (relief, _) = Relief::fit(&[KNOLL], hill(0.5));
        assert!((relief.at(100.0, 100.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_stamp_moves_onto_a_bump_beside_the_symbol() {
        let (relief, _) = Relief::fit(&[Landform { e: 98.0, n: 101.0, ..KNOLL }], hill(0.5));
        // The ring around the symbol just touches the hill.
        assert!((relief.at(100.0, 100.0) - 1.0).abs() < 0.05, "{}", relief.at(100.0, 100.0));
    }

    #[test]
    fn a_long_knoll_lies_along_its_angle() {
        let long = Landform { along: 4.0, across: 2.0, ..KNOLL };
        let (east, _) = Relief::fit(&[long], flat);
        assert!(east.at(103.0, 100.0) > 0.0 && east.at(100.0, 103.0) == 0.0);
        let (north, _) = Relief::fit(&[Landform { angle: 90.0, ..long }], flat);
        assert!(north.at(100.0, 103.0) > 0.0 && north.at(103.0, 100.0) == 0.0);
    }

    const LONG: f64 = 100.0;

    /// East along z = 0: the left of the line is north.
    fn cliff(target: f64) -> Cliff {
        Cliff { points: vec![[0.0, 0.0], [LONG, 0.0]], target, face: 1.0 }
    }

    fn added(ground: impl Fn(f64, f64) -> Option<f64>, cliff: Cliff) -> Relief {
        let mut relief = Relief::default();
        relief.add_cliffs(&[cliff], ground);
        relief
    }

    #[test]
    fn a_cliff_steps_flat_ground_to_its_height() {
        let relief = added(flat, cliff(3.0));
        // Unsure, so down is to the left, the north.
        let step = relief.at(50.0, -1.0) - relief.at(50.0, 1.0);
        assert!((step - 3.0).abs() < 0.3, "{step}");
    }

    #[test]
    fn a_cliff_rises_on_the_side_the_model_rises() {
        let rising_north = |_: f64, n: f64| Some(n * 0.1);
        let relief = added(rising_north, cliff(3.0));
        assert!(relief.at(50.0, 2.0) > 0.0 && relief.at(50.0, -2.0) < 0.0);
        let falling_north = |_: f64, n: f64| Some(n * -0.1);
        let relief = added(falling_north, cliff(3.0));
        assert!(relief.at(50.0, 2.0) < 0.0 && relief.at(50.0, -2.0) > 0.0);
    }

    #[test]
    fn a_cliff_the_model_shows_is_left_alone() {
        let scarp = |_: f64, n: f64| Some(if n > 0.0 { 4.0 } else { 0.0 });
        let mut relief = Relief::default();
        assert_eq!(relief.add_cliffs(&[cliff(3.0)], scarp), (1, 0));
        assert_eq!(relief.at(50.0, 1.0), 0.0);
    }

    #[test]
    fn a_half_shown_cliff_is_topped_up() {
        let scarp = |_: f64, n: f64| Some(if n > 0.0 { 1.0 } else { 0.0 });
        let relief = added(scarp, cliff(3.0));
        let step = relief.at(50.0, 1.0) - relief.at(50.0, -1.0);
        assert!((step - 2.0).abs() < 0.2, "{step}");
    }

    #[test]
    fn a_cliff_runs_out_at_its_ends_and_past_its_fade() {
        let relief = added(flat, cliff(3.0));
        assert_eq!(relief.at(0.0, 2.0), 0.0);
        assert_eq!(relief.at(LONG, 2.0), 0.0);
        assert!(relief.at(1.5, 2.0).abs() < relief.at(50.0, 2.0).abs());
        assert_eq!(relief.at(50.0, 9.0), 0.0);
    }

    #[test]
    fn two_lines_of_one_cliff_do_not_add_up() {
        let mut relief = Relief::default();
        relief.add_cliffs(&[cliff(3.0), cliff(3.0)], flat);
        assert_eq!(relief.at(50.0, 2.0), added(flat, cliff(3.0)).at(50.0, 2.0));
    }

    #[test]
    fn a_cliff_is_found_across_its_bins() {
        let bent = Cliff { points: vec![[10.0, 10.0], [40.0, 10.0], [40.0, 50.0]], target: 2.0, face: 1.0 };
        let relief = added(flat, bent);
        assert!(relief.at(30.0, 11.0) != 0.0 && relief.at(41.0, 30.0) != 0.0);
    }

    #[test]
    fn off_the_model_nothing_is_stamped() {
        assert!(Relief::fit(&[KNOLL], |_, _| None).0.is_empty());
    }

    #[test]
    fn stamps_are_found_across_bin_borders() {
        let (relief, _) = Relief::fit(&[Landform { e: 16.0, n: 16.0, ..KNOLL }], flat);
        assert!(relief.at(15.0, 15.5) > 0.0 && relief.at(17.0, 16.5) > 0.0);
    }
}
