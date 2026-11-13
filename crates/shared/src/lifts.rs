//! Ski lifts: where each chair of a lift is at a server tick. A chair is not an entity: the cable runs
//! at `LIFT_SPEED` from tick zero, so a chair's place is a function of its lift, its number and the
//! tick, the same on the server (which carries riders on it) and the client (which draws every chair;
//! `client/src/sim/lifts.ts` mirrors this file, and a test holds the two together). LIFTS.md.
//!
//! The cable is a loop: up the lift on the right of its line, round the top station's bull wheel,
//! down on the left and round the bottom station's wheel. It runs straight from pylon to pylon.

use crate::consts::{LIFT_CHAIR_DROP, LIFT_CHAIR_SPACING, LIFT_GAUGE, LIFT_MAX_SEATS, LIFT_SEAT_PITCH, LIFT_SPEED, LIFT_WHEEL_SEGMENTS, TICK_RATE};
use crate::world::Lift;

/// Which stretch of its loop a chair is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stretch {
    /// Going up, from the bottom station to the top.
    Up,
    /// Going round the top station's bull wheel.
    Top,
    /// Coming down.
    Down,
    /// Going round the bottom station's bull wheel.
    Bottom,
}

/// A lift's cable loop, laid out once: its corners and how far along the loop each one is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// The cable's corners, metres: up the right side, round the top wheel, down the left side and
    /// round the bottom wheel, back to the first.
    pub points: Vec<[f32; 3]>,
    /// Distance along the loop to each of `points`, metres, and to the end of the loop (`length`) last.
    pub along: Vec<f32>,
    /// Length of the loop, metres.
    pub length: f32,
    /// Where the stretches end along the loop, metres: going up, round the top, coming down.
    pub up: f32,
    pub top: f32,
    pub down: f32,
    /// Chairs on the loop, and the distance between them, metres.
    pub chairs: u32,
    pub spacing: f32,
    /// The loop's length and the chairs' spacing as the clock runs them, metres, the length rounded to
    /// the millimetre: the client works them out in f64 to the same numbers, so after months of ticks a
    /// chair is still where the other side has it.
    pub clock_length: f64,
    pub clock_spacing: f64,
    /// Riders on each chair.
    pub seats: u8,
}

/// Where a chair is: its seat's middle (metres), which way it faces (yaw, radians: 0 faces -Z), how
/// far round the loop it is (metres) and the stretch it is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chair {
    pub seat: [f32; 3],
    pub yaw: f32,
    pub along: f32,
    pub stretch: Stretch,
}

/// The yaw of a heading `[dx, dz]` (0 faces -Z).
fn yaw_of(dx: f32, dz: f32) -> f32 {
    (-dx).atan2(-dz)
}

/// Right of a yaw, across the ground.
pub fn right_of(yaw: f32) -> [f32; 2] {
    let (sin, cos) = yaw.sin_cos();
    [cos, -sin]
}

impl Path {
    /// Lays out a lift's loop. A lift of fewer than two points has an empty loop and no chairs.
    pub fn new(lift: &Lift) -> Self {
        let n = lift.points.len().min(lift.cable.len());
        if n < 2 {
            return Self::default();
        }
        let at = |i: usize| [lift.points[i][0] as f32 / 100.0, lift.cable[i] as f32 / 100.0, lift.points[i][1] as f32 / 100.0];
        // Across the line at each point, the mean of the spans either side of it.
        let ahead = |i: usize| {
            let (a, b) = (at(i.saturating_sub(1)), at((i + 1).min(n - 1)));
            let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
            let length = dx.hypot(dz).max(1e-6);
            [dx / length, dz / length]
        };
        let half = LIFT_GAUGE / 2.0;
        let side = |i: usize, sign: f32| {
            let [fx, fz] = ahead(i);
            let p = at(i);
            [p[0] - fz * half * sign, p[1], p[2] + fx * half * sign]
        };
        // Round a wheel at `centre`, from the side `from` (+1 right, -1 left) through `forward`.
        let wheel = |centre: [f32; 3], forward: [f32; 2], from: f32| {
            (1..LIFT_WHEEL_SEGMENTS).map(move |k| {
                let angle = std::f32::consts::PI * k as f32 / LIFT_WHEEL_SEGMENTS as f32;
                let (sin, cos) = angle.sin_cos();
                let (across, on) = (cos * half * from, sin * half);
                [centre[0] - forward[1] * across + forward[0] * on, centre[1], centre[2] + forward[0] * across + forward[1] * on]
            })
        };
        let mut points: Vec<[f32; 3]> = (0..n).map(|i| side(i, 1.0)).collect();
        let up_corners = points.len();
        points.extend(wheel(at(n - 1), ahead(n - 1), 1.0));
        let top_corners = points.len();
        points.extend((0..n).rev().map(|i| side(i, -1.0)));
        let down_corners = points.len();
        let back = ahead(0);
        points.extend(wheel(at(0), [-back[0], -back[1]], 1.0));
        let mut along = Vec::with_capacity(points.len() + 1);
        let mut total = 0.0;
        let mut exact = 0.0f64;
        for i in 0..points.len() {
            along.push(total);
            let (a, b) = (points[i], points[(i + 1) % points.len()]);
            total += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
            exact += (0..3).map(|k| (f64::from(b[k]) - f64::from(a[k])).powi(2)).sum::<f64>().sqrt();
        }
        along.push(total);
        let clock_length = (exact * 1000.0).round() / 1000.0;
        let chairs = (clock_length / f64::from(LIFT_CHAIR_SPACING)).floor().max(1.0) as u32;
        Self {
            clock_length,
            clock_spacing: clock_length / f64::from(chairs),
            up: along[up_corners - 1],
            top: along[top_corners],
            down: along[down_corners - 1],
            points,
            along,
            length: total,
            chairs,
            spacing: (clock_length / f64::from(chairs)) as f32,
            seats: lift.seats.clamp(1, LIFT_MAX_SEATS),
        }
    }

    /// The stretch of the loop `along` metres round it is on.
    pub fn stretch(&self, along: f32) -> Stretch {
        if along < self.up {
            Stretch::Up
        } else if along < self.top {
            Stretch::Top
        } else if along < self.down {
            Stretch::Down
        } else {
            Stretch::Bottom
        }
    }

    /// How far round the loop chair `index` is at `tick`, metres. Worked in f64, so a server up for
    /// months still places a chair to the millimetre.
    pub fn along(&self, index: u32, tick: u32) -> f32 {
        if self.clock_length <= 0.0 {
            return 0.0;
        }
        let run = f64::from(tick) / f64::from(TICK_RATE) * f64::from(LIFT_SPEED) + f64::from(index) * self.clock_spacing;
        // Scaled to the drawn loop, which f32 sums may make a hair longer or shorter.
        (run.rem_euclid(self.clock_length) / self.clock_length * f64::from(self.length)) as f32
    }

    /// Chair `index` at `tick`.
    pub fn chair(&self, index: u32, tick: u32) -> Chair {
        self.at(self.along(index, tick))
    }

    /// A chair `along` metres round the loop.
    pub fn at(&self, along: f32) -> Chair {
        if self.points.is_empty() {
            return Chair { seat: [0.0; 3], yaw: 0.0, along: 0.0, stretch: Stretch::Up };
        }
        // The last corner at or before `along`.
        let i = self.along.partition_point(|&s| s <= along).saturating_sub(1).min(self.points.len() - 1);
        let (a, b) = (self.points[i], self.points[(i + 1) % self.points.len()]);
        let span = (self.along[i + 1] - self.along[i]).max(1e-6);
        let t = ((along - self.along[i]) / span).clamp(0.0, 1.0);
        let seat = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t - LIFT_CHAIR_DROP, a[2] + (b[2] - a[2]) * t];
        Chair { seat, yaw: yaw_of(b[0] - a[0], b[2] - a[2]), along, stretch: self.stretch(along) }
    }

    /// Where rider `seat` (0 on the chair's left) sits on a chair: across it, `LIFT_SEAT_PITCH` apart.
    pub fn place(&self, chair: &Chair, seat: u8) -> [f32; 3] {
        let across = (f32::from(seat) - (f32::from(self.seats) - 1.0) / 2.0) * LIFT_SEAT_PITCH;
        let [rx, rz] = right_of(chair.yaw);
        [chair.seat[0] + rx * across, chair.seat[1], chair.seat[2] + rz * across]
    }

    /// The cable's velocity where a chair is, m/s.
    pub fn velocity(&self, chair: &Chair) -> [f32; 3] {
        let ahead = self.at((chair.along + 0.5).min(self.length - 1e-3));
        let behind = self.at((chair.along - 0.5).max(0.0));
        let (dx, dy, dz) = (ahead.seat[0] - behind.seat[0], ahead.seat[1] - behind.seat[1], ahead.seat[2] - behind.seat[2]);
        let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1e-6);
        [dx / length * LIFT_SPEED, dy / length * LIFT_SPEED, dz / length * LIFT_SPEED]
    }
}

/// Plain steel: the pylons' tubes and arms, the stations' posts.
pub const LOOK_STEEL: u8 = 0;
/// Dark steel: sheaves, hangers and the bull wheel.
pub const LOOK_DARK: u8 = 1;
/// A station's concrete deck.
pub const LOOK_CONCRETE: u8 = 2;
/// A station's roof.
pub const LOOK_ROOF: u8 = 3;

/// The shape of one solid part of a lift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A box of these half extents, metres, in its own frame: X across the lift's line, Z along it.
    Box([f32; 3]),
    /// An upright cylinder of this radius and half height, metres.
    Post { radius: f32, half: f32 },
}

/// One solid part of a lift's pylons and stations: its shape, its middle (metres), how far it is
/// turned about the vertical (radians, as a rotation about +Y: +Z goes to `(sin, cos)`) and how it
/// looks (`LOOK_*`). The server and the client's physics stand colliders of these, and the client
/// draws them, so what is seen is what is hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub shape: Shape,
    pub centre: [f32; 3],
    pub turn: f32,
    pub look: u8,
}

/// Distance across from a station's middle to its roof's posts, metres: clear of the widest chair
/// going by on the cable.
const STATION_POSTS: f32 = LIFT_GAUGE / 2.0 + 2.0;

/// The pylons and stations of `lift`, as solid parts. The chairs and the cable are not solid.
pub fn structure(lift: &Lift) -> Vec<Part> {
    let n = lift.points.len().min(lift.base.len()).min(lift.cable.len());
    let mut parts = Vec::new();
    if n < 2 {
        return parts;
    }
    let point = |i: usize| [lift.points[i][0] as f32 / 100.0, lift.points[i][1] as f32 / 100.0];
    let ahead = |i: usize| {
        let (a, b) = (point(i.saturating_sub(1)), point((i + 1).min(n - 1)));
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let length = dx.hypot(dz).max(1e-6);
        [dx / length, dz / length]
    };
    let post = |x: f32, z: f32, from: f32, to: f32, radius: f32, look: u8| Part { shape: Shape::Post { radius, half: (to - from) / 2.0 }, centre: [x, (from + to) / 2.0, z], turn: 0.0, look };
    for i in 0..n {
        let [x, z] = point(i);
        let (base, cable) = (lift.base[i] as f32 / 100.0, lift.cable[i] as f32 / 100.0);
        let [fx, fz] = ahead(i);
        let turn = fx.atan2(fz);
        // `along` metres along the line and `across` to its right, from this point.
        let at = |along: f32, across: f32| [x + fx * along - fz * across, z + fz * along + fx * across];
        let boxed = |[px, pz]: [f32; 2], y: f32, half: [f32; 3], look: u8| Part { shape: Shape::Box(half), centre: [px, y, pz], turn, look };
        if i == 0 || i + 1 == n {
            // A station: its deck reaching back under the line for boarding, the bull wheel on its
            // column just outside the turn, and a roof on four posts over it all.
            let out = if i == 0 { -1.0 } else { 1.0 };
            parts.push(boxed(at(-2.0 * out, 0.0), base - 0.15, [LIFT_GAUGE / 2.0 + 2.5, 0.25, 7.0], LOOK_CONCRETE));
            parts.push(post(x, z, cable - 0.175, cable + 0.175, LIFT_GAUGE / 2.0 + 0.1, LOOK_DARK));
            let [cx, cz] = at(out * 1.2, 0.0);
            parts.push(post(cx, cz, base - 0.3, cable + 2.1, 0.45, LOOK_STEEL));
            parts.push(boxed(at(out * 1.5, 0.0), cable + 2.2, [STATION_POSTS + 0.3, 0.125, 4.0], LOOK_ROOF));
            for across in [-STATION_POSTS, STATION_POSTS] {
                for along in [-2.2, 5.2] {
                    let [px, pz] = at(out * along, across);
                    parts.push(post(px, pz, base - 0.3, cable + 2.1, 0.12, LOOK_STEEL));
                }
            }
            continue;
        }
        // A pylon: a steel tube, a crossarm over the cables, and under each end of it a hanger and
        // the train of sheaves the cable runs over.
        let top = cable + 0.9;
        parts.push(post(x, z, base - 0.5, top, 0.35, LOOK_STEEL));
        parts.push(boxed([x, z], top, [LIFT_GAUGE / 2.0 + 0.6, 0.175, 0.175], LOOK_STEEL));
        for side in [-1.0, 1.0] {
            let end = at(0.0, side * LIFT_GAUGE / 2.0);
            parts.push(boxed(end, (top + cable) / 2.0, [0.06, (top - cable) / 2.0, 0.06], LOOK_DARK));
            parts.push(boxed(end, cable + 0.12, [0.1, 0.11, 1.2], LOOK_DARK));
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{LIFT_GAUGE, TICK_RATE};

    /// A lift 200 m long going north (-Z), rising 40 m, on one pylon.
    fn lift() -> Lift {
        Lift { kind: 0, seats: 2, points: vec![[0, 0], [0, -10_000], [0, -20_000]], base: vec![0, 2_000, 4_000], cable: vec![310, 2_900, 4_310] }
    }

    #[test]
    fn a_loop_goes_up_the_right_round_the_top_and_down_the_left() {
        let path = Path::new(&lift());
        let up = path.at(1.0);
        assert_eq!(up.stretch, Stretch::Up);
        // Facing north (-Z), right is +X.
        assert!((up.seat[0] - LIFT_GAUGE / 2.0).abs() < 1e-3, "{up:?}");
        assert!(up.yaw.abs() < 1e-3, "{up:?}");
        let down = path.at(path.top + 1.0);
        assert_eq!(down.stretch, Stretch::Down);
        assert!((down.seat[0] + LIFT_GAUGE / 2.0).abs() < 1e-3, "{down:?}");
        assert!((down.yaw.abs() - std::f32::consts::PI).abs() < 1e-3, "{down:?}");
        assert_eq!(path.stretch(path.length - 0.1), Stretch::Bottom);
        // The two sides (bent at the pylon) and the two half wheels.
        let side = (200.0f32.powi(2) + 40.0f32.powi(2)).sqrt();
        let expected = 2.0 * side + std::f32::consts::PI * LIFT_GAUGE;
        assert!((path.length - expected).abs() < 1.0, "{} vs {expected}", path.length);
    }

    #[test]
    fn a_chairs_seat_hangs_under_the_cable_and_is_low_at_the_stations() {
        let path = Path::new(&lift());
        let bottom = path.at(0.0);
        assert!((bottom.seat[1] - 0.5).abs() < 1e-3, "{bottom:?}");
        let pylon = path.at(path.along[1]);
        assert!((pylon.seat[1] - (29.0 - 2.6)).abs() < 0.05, "{pylon:?}");
    }

    #[test]
    fn chairs_move_at_the_lift_speed_and_keep_their_spacing() {
        let path = Path::new(&lift());
        let (a, b) = (path.along(0, 0), path.along(0, TICK_RATE));
        assert!((b - a - LIFT_SPEED).abs() < 1e-3);
        assert!((path.along(1, 0) - path.spacing).abs() < 1e-3);
        assert!(path.chairs as f32 * path.spacing - path.length < 1e-2);
        // Months into a server's life, still on the loop.
        assert!(path.along(3, 1_000_000_000) < path.length);
    }

    #[test]
    fn riders_sit_side_by_side_across_the_chair() {
        let path = Path::new(&lift());
        let chair = path.at(10.0);
        let (left, right) = (path.place(&chair, 0), path.place(&chair, 1));
        // Facing north, the second rider is east of the first.
        assert!((right[0] - left[0] - LIFT_SEAT_PITCH).abs() < 1e-3);
        assert_eq!(left[1], right[1]);
    }

    #[test]
    fn a_lift_stands_on_two_stations_and_a_pylon_and_no_chair_hits_a_post() {
        let parts = structure(&lift());
        // Each station: a deck, the wheel, its column, the roof and four posts. The pylon: tube, arm, two hangers, two trains.
        assert_eq!(parts.len(), 2 * 8 + 6);
        let path = Path::new(&lift());
        let widest = (f32::from(LIFT_MAX_SEATS) * LIFT_SEAT_PITCH + 0.1) / 2.0;
        for part in parts.iter().filter(|part| matches!(part.shape, Shape::Post { radius, .. } if radius < 0.2)) {
            let mut along = 0.0;
            while along < path.length {
                let chair = path.at(along);
                let near = (chair.seat[0] - part.centre[0]).hypot(chair.seat[2] - part.centre[2]);
                assert!(near > widest, "a chair at {along} m brushes a roof post at {:?}", part.centre);
                along += 0.5;
            }
        }
    }

    #[test]
    fn the_cable_carries_a_chair_forward_up_the_hill() {
        let path = Path::new(&lift());
        let velocity = path.velocity(&path.at(50.0));
        assert!(velocity[2] < -3.5 && velocity[1] > 0.5, "{velocity:?}");
    }
}
