//! The Orange Plague's body: a skeleton, how he holds it in each of his modes, and the capsules that
//! cover it. The server hits, blasts and collides with these capsules; the client builds his model
//! on the same joints and mirrors the solid ones for its own character prediction
//! (`client/src/sim/plague-body.ts`, checked by `fixtures/parity/plague.json`).
//!
//! Everything is in his own frame unless said otherwise: feet on the ground at the origin, +X to
//! his right, +Y up, facing -Z. A pose is a pure function of what the snapshot carries (his mode, a
//! progress byte and whether he is toppling), so both sides pose him alike.

use crate::protocol::{FACE_EAR_LEFT, FACE_EAR_RIGHT, FACE_EYE_LEFT, FACE_EYE_RIGHT, FACE_MOUTH, PLAGUE_RECOVER, PLAGUE_STOMP, PLAGUE_STRIKE};
use std::f32::consts::{FRAC_PI_2, TAU};

/// The hips, above his feet, and how far each sits from his middle.
pub const HIP_Y: f32 = 17.5;
pub const HIP_X: f32 = crate::consts::PLAGUE_FOOT_SPREAD;
/// Thigh and shin lengths, metres: the ankle is `HIP_Y - THIGH - SHIN` up.
pub const THIGH: f32 = 8.3;
pub const SHIN: f32 = 7.8;
/// The shoulders, and the upper arm and forearm lengths.
pub const SHOULDER_X: f32 = 8.6;
pub const SHOULDER_Y: f32 = 29.5;
pub const UPPER_ARM: f32 = 6.8;
pub const FOREARM: f32 = 6.2;
/// The middle of his hand, past the wrist along the forearm.
pub const HAND_REACH: f32 = 7.6;
/// The middle of his head and its radius.
pub const HEAD: [f32; 3] = [0.0, 35.6, 0.0];
pub const HEAD_RADIUS: f32 = 3.7;
/// His face, from the middle of his head: each eye (his left first), each ear (a short upright
/// capsule), and his mouth (a short one across).
pub const EYE: [f32; 3] = [1.35, 0.8, -3.15];
pub const EYE_RADIUS: f32 = 0.75;
pub const EAR: [f32; 3] = [3.6, 0.15, 0.3];
pub const EAR_HALF: f32 = 0.85;
pub const EAR_RADIUS: f32 = 0.85;
pub const MOUTH: [f32; 3] = [0.0, -1.5, -3.05];
pub const MOUTH_HALF: f32 = 1.0;
pub const MOUTH_RADIUS: f32 = 0.7;

/// How far each leg swings either way while he walks, radians, and the arms the other way.
const LEG_SWING: f32 = 0.32;
const ARM_SWING: f32 = 0.22;
/// The swinging leg's knee bends this far at the middle of its swing.
const KNEE_LIFT: f32 = 0.6;

/// What a part of him is, for what a hit on it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Leg,
    Body,
    Arm,
    /// The right hand, which holds the gold phone: a hit there drops a call for an airstrike.
    Hand,
    Head,
    /// One of his eyes, ears or his mouth: a `FACE_*` bit. Each can be shot out on its own.
    Face(u8),
}

/// One capsule of his body: the segment `a`..`b` and its radius. A sphere has `a == b`. Solid ones
/// block characters and vehicles; the face is only there to be shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub part: Part,
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub radius: f32,
    pub solid: bool,
}

/// How he holds himself: every joint angle, radians. Hips, knees and elbows bend about X, shoulders
/// about X then Z (as a scene node's XYZ rotation does), and the elbow also about Z. Index 0 is his
/// left side. `tilt` leans the whole of him back about his feet, and `bob` lifts him.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub tilt: f32,
    pub bob: f32,
    pub hip: [f32; 2],
    pub knee: [f32; 2],
    pub shoulder: [[f32; 2]; 2],
    pub elbow: [[f32; 2]; 2],
}

/// `x` eased in and out over 0..1.
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The progress byte a snapshot carries for how far through `mode` he is: a walk's place in its
/// two-stride cycle (which wraps), or how much of a wind-up, a recovery or a fall is done.
pub fn progress_byte(mode: u8, toppling: bool, progress: f32) -> u8 {
    if mode == crate::protocol::PLAGUE_WALK && !toppling {
        (progress.rem_euclid(1.0) * 256.0) as u8
    } else {
        (progress.clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

/// The progress `progress_byte` sent.
pub fn progress_of(mode: u8, toppling: bool, byte: u8) -> f32 {
    if mode == crate::protocol::PLAGUE_WALK && !toppling { byte as f32 / 256.0 } else { byte as f32 / 255.0 }
}

/// How he stands in `mode` (`PLAGUE_*`), `progress` (0..1) of the way through it, or falling over.
pub fn pose(mode: u8, progress: f32, toppling: bool) -> Pose {
    let mut pose = Pose { shoulder: [[0.0, -0.1], [0.0, 0.1]], elbow: [[0.25, 0.0], [0.25, 0.0]], ..Pose::default() };
    if toppling {
        // Over backwards, slowly at first, arms flung up and legs kicking.
        let t = progress.clamp(0.0, 1.0);
        pose.tilt = t * t * FRAC_PI_2;
        pose.hip = [0.35 * t, 0.2 * t];
        pose.knee = [-0.3 * t, -0.5 * t];
        pose.shoulder = [[2.6 * t, -0.5 * t], [2.4 * t, 0.6 * t]];
        return pose;
    }
    match mode {
        PLAGUE_STOMP => {
            // The right foot rises through the first part of the wind-up and hangs there; he leans
            // back and spreads his arms to keep his balance.
            let lift = smooth(progress * 1.6);
            pose.hip = [0.0, 1.4 * lift];
            pose.knee = [-0.1 * lift, -1.4 * lift];
            pose.tilt = 0.1 * lift;
            pose.shoulder = [[0.3 * lift, -0.1 - 0.6 * lift], [0.3 * lift, 0.1 + 0.6 * lift]];
        }
        PLAGUE_RECOVER => {
            // The foot has come down: a crouch that straightens.
            let left = 1.0 - smooth(progress);
            pose.hip = [0.15 * left, 0.15 * left];
            pose.knee = [-0.3 * left, -0.3 * left];
            pose.tilt = -0.06 * left;
        }
        PLAGUE_STRIKE => {
            // The phone to his right ear, and his left arm pointing at where it will fall.
            let call = smooth(progress * 3.0);
            let point = smooth(progress * 2.0);
            pose.shoulder = [[1.5 * point, -0.1], [2.5 * call, 0.1 + 0.6 * call]];
            pose.elbow = [[0.25 - 0.15 * point, 0.0], [0.25 + 2.45 * call, -0.8 * call]];
        }
        _ => {
            // Walking: the left foot is forward and coming down at 0, the right at one half.
            let angle = progress * TAU;
            let (sin, cos) = angle.sin_cos();
            pose.hip = [LEG_SWING * cos, -LEG_SWING * cos];
            pose.knee = [-KNEE_LIFT * (-sin).max(0.0) - 0.05, -KNEE_LIFT * sin.max(0.0) - 0.05];
            pose.shoulder = [[-ARM_SWING * cos, -0.1], [ARM_SWING * cos, 0.1]];
            pose.bob = (2.0 * angle).sin().abs() * 0.4;
        }
    }
    pose
}

type Mat = [[f32; 3]; 3];

fn rot_x(t: f32) -> Mat {
    let (s, c) = t.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_z(t: f32) -> Mat {
    let (s, c) = t.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}

fn apply(m: &Mat, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

/// A joint's place and orientation in his frame.
#[derive(Clone, Copy)]
struct Frame {
    origin: [f32; 3],
    rotation: Mat,
}

impl Frame {
    fn point(&self, local: [f32; 3]) -> [f32; 3] {
        let r = apply(&self.rotation, local);
        std::array::from_fn(|i| self.origin[i] + r[i])
    }

    /// The frame of a child joint at `at` in this one, turned by `rotation` there.
    fn child(&self, at: [f32; 3], rotation: Mat) -> Frame {
        Frame { origin: self.point(at), rotation: mul(&self.rotation, &rotation) }
    }
}

const DOWN: [f32; 3] = [0.0, -1.0, 0.0];

fn down(length: f32) -> [f32; 3] {
    DOWN.map(|v| v * length)
}

/// Every capsule of his body in `pose`, in his frame. Its order is fixed: legs, body, head, arms and
/// then his face, each left side first.
pub fn capsules(pose: &Pose) -> Vec<Capsule> {
    let body = Frame { origin: [0.0, pose.bob, 0.0], rotation: rot_x(pose.tilt) };
    let mut out = Vec::with_capacity(24);
    let mut push = |part: Part, frame: &Frame, a: [f32; 3], b: [f32; 3], radius: f32, solid: bool| {
        out.push(Capsule { part, a: frame.point(a), b: frame.point(b), radius, solid });
    };
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let thigh = body.child([side * HIP_X, HIP_Y, 0.0], rot_x(pose.hip[i]));
        let shin = thigh.child(down(THIGH), rot_x(pose.knee[i]));
        push(Part::Leg, &thigh, [0.0; 3], down(THIGH), 2.7, true);
        // Stopping short of the ankle, so it does not reach below the sole.
        push(Part::Leg, &shin, [0.0; 3], down(SHIN - 1.2), 2.2, true);
        // The shoe, heel to toe, its sole on the ground when he stands straight.
        push(Part::Leg, &shin, [0.0, -SHIN - 0.2, 0.9], [0.0, -SHIN - 0.2, -2.9], 1.2, true);
    }
    push(Part::Body, &body, [-HIP_X, HIP_Y - 0.3, 0.3], [HIP_X, HIP_Y - 0.3, 0.3], 3.6, true);
    push(Part::Body, &body, [0.0, 19.8, -1.4], [0.0, 25.2, -0.8], 6.9, true);
    push(Part::Body, &body, [-6.6, 28.4, 0.0], [6.6, 28.4, 0.0], 3.1, true);
    push(Part::Head, &body, [0.0, 30.4, 0.0], [0.0, 32.4, 0.0], 2.3, true);
    push(Part::Head, &body, HEAD, HEAD, HEAD_RADIUS, true);
    push(Part::Head, &body, [0.0, 37.2, -0.5], [0.0, 37.2, -0.5], 3.3, true);
    for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let [sx, sz] = pose.shoulder[i];
        let [ex, ez] = pose.elbow[i];
        let upper = body.child([side * SHOULDER_X, SHOULDER_Y, 0.0], mul(&rot_x(sx), &rot_z(sz)));
        let fore = upper.child(down(UPPER_ARM), mul(&rot_x(ex), &rot_z(ez)));
        push(Part::Arm, &upper, [0.0; 3], down(UPPER_ARM), 2.0, true);
        push(Part::Arm, &fore, [0.0; 3], down(FOREARM), 1.7, true);
        push(if side > 0.0 { Part::Hand } else { Part::Arm }, &fore, down(HAND_REACH), down(HAND_REACH), 1.9, true);
    }
    let head = Frame { origin: body.point(HEAD), rotation: body.rotation };
    let at = |offset: [f32; 3], side: f32| [offset[0] * side, offset[1], offset[2]];
    for (bit, side) in [(FACE_EYE_LEFT, -1.0), (FACE_EYE_RIGHT, 1.0)] {
        push(Part::Face(bit), &head, at(EYE, side), at(EYE, side), EYE_RADIUS, false);
    }
    for (bit, side) in [(FACE_EAR_LEFT, -1.0f32), (FACE_EAR_RIGHT, 1.0)] {
        let [x, y, z] = at(EAR, side);
        push(Part::Face(bit), &head, [x, y - EAR_HALF, z], [x, y + EAR_HALF, z], EAR_RADIUS, false);
    }
    let [x, y, z] = MOUTH;
    push(Part::Face(FACE_MOUTH), &head, [x - MOUTH_HALF, y, z], [x + MOUTH_HALF, y, z], MOUTH_RADIUS, false);
    out
}

/// Where his feet are, in his frame: the bottom of each shoe under the ankle, his left first.
pub fn feet(pose: &Pose) -> [[f32; 3]; 2] {
    let body = Frame { origin: [0.0, pose.bob, 0.0], rotation: rot_x(pose.tilt) };
    std::array::from_fn(|i| {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let thigh = body.child([side * HIP_X, HIP_Y, 0.0], rot_x(pose.hip[i]));
        let shin = thigh.child(down(THIGH), rot_x(pose.knee[i]));
        shin.point([0.0, -SHIN - 1.4, -1.0])
    })
}

/// A point of his frame in the world, for him standing at `position` facing `yaw` (yaw 0 faces -Z,
/// as for players).
pub fn to_world(position: [f32; 3], yaw: f32, local: [f32; 3]) -> [f32; 3] {
    let (sin, cos) = yaw.sin_cos();
    [position[0] + cos * local[0] + sin * local[2], position[1] + local[1], position[2] - sin * local[0] + cos * local[2]]
}

/// `capsules` placed in the world.
pub fn world_capsules(pose: &Pose, position: [f32; 3], yaw: f32) -> Vec<Capsule> {
    capsules(pose).into_iter().map(|c| Capsule { a: to_world(position, yaw, c.a), b: to_world(position, yaw, c.b), ..c }).collect()
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The point of the segment `a`..`b` nearest `p`.
pub fn nearest_on_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    let ab = sub(b, a);
    let length = dot(ab, ab);
    let t = if length < 1e-9 { 0.0 } else { (dot(sub(p, a), ab) / length).clamp(0.0, 1.0) };
    std::array::from_fn(|i| a[i] + ab[i] * t)
}

/// How far `p` is from the surface of `capsule`, metres; 0 inside.
pub fn distance_to(capsule: &Capsule, p: [f32; 3]) -> f32 {
    let q = nearest_on_segment(p, capsule.a, capsule.b);
    let d = sub(p, q);
    (dot(d, d).sqrt() - capsule.radius).max(0.0)
}

/// Where a ray from `origin` along the unit vector `dir` first meets the sphere at `centre`.
fn ray_sphere(origin: [f32; 3], dir: [f32; 3], centre: [f32; 3], radius: f32) -> Option<f32> {
    let oc = sub(origin, centre);
    let b = dot(dir, oc);
    let c = dot(oc, oc) - radius * radius;
    let h = b * b - c;
    (h >= 0.0).then(|| -b - h.sqrt())
}

/// How far a ray from `origin` along the unit vector `dir` goes before it meets `capsule`, within
/// `max`; 0 if it starts inside.
pub fn ray_capsule(origin: [f32; 3], dir: [f32; 3], capsule: &Capsule, max: f32) -> Option<f32> {
    let (a, b, r) = (capsule.a, capsule.b, capsule.radius);
    if distance_to(capsule, origin) <= 0.0 {
        return Some(0.0);
    }
    let ba = sub(b, a);
    let oa = sub(origin, a);
    let baba = dot(ba, ba);
    let t = if baba < 1e-9 {
        ray_sphere(origin, dir, a, r)
    } else {
        // The side of the cylinder first, then whichever end cap the ray comes in by.
        let bard = dot(ba, dir);
        let baoa = dot(ba, oa);
        let k2 = baba - bard * bard;
        let k1 = baba * dot(oa, dir) - baoa * bard;
        let k0 = baba * dot(oa, oa) - baoa * baoa - r * r * baba;
        let h = k1 * k1 - k2 * k0;
        if h < 0.0 {
            None
        } else {
            let side = if k2.abs() > 1e-9 { Some((-k1 - h.sqrt()) / k2) } else { None };
            match side {
                Some(t) if (0.0..=baba).contains(&(baoa + t * bard)) => Some(t),
                _ => {
                    let y = side.map_or(baoa, |t| baoa + t * bard);
                    ray_sphere(origin, dir, if y <= 0.0 { a } else { b }, r)
                }
            }
        }
    };
    t.filter(|&t| (0.0..=max).contains(&t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::PLAGUE_WALK;

    fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
        let d = sub(a, b);
        dot(d, d).sqrt()
    }

    #[test]
    fn standing_he_is_forty_metres_tall_with_his_shoes_on_the_ground() {
        let body = capsules(&pose(PLAGUE_WALK, 0.25, false));
        let top = body.iter().map(|c| c.a[1].max(c.b[1]) + c.radius).fold(0.0, f32::max);
        let bottom = body.iter().map(|c| c.a[1].min(c.b[1]) - c.radius).fold(f32::MAX, f32::min);
        assert!((top - crate::consts::PLAGUE_HEIGHT).abs() < 1.0, "top {top}");
        assert!(bottom.abs() < 0.3, "bottom {bottom}");
    }

    #[test]
    fn the_phone_comes_up_to_his_right_ear_and_the_left_arm_points_ahead() {
        let body = capsules(&pose(PLAGUE_STRIKE, 1.0, false));
        let hand = body.iter().find(|c| c.part == Part::Hand).unwrap();
        let ear = body.iter().find(|c| c.part == Part::Face(FACE_EAR_RIGHT)).unwrap();
        assert!(dist(hand.a, ear.a) < 3.5, "hand {:?} ear {:?}", hand.a, ear.a);
        let left_hand = body.iter().find(|c| c.part == Part::Arm && c.a == c.b).unwrap();
        assert!(left_hand.a[2] < -10.0 && left_hand.a[0] < 0.0, "pointing ahead: {:?}", left_hand.a);
    }

    #[test]
    fn a_stomp_lifts_the_right_foot_high_and_leaves_the_left_down() {
        let [left, right] = feet(&pose(PLAGUE_STOMP, 1.0, false));
        assert!(left[1] < 0.5, "left {left:?}");
        assert!(right[1] > 6.0, "right {right:?}");
    }

    #[test]
    fn walking_the_left_foot_is_ahead_at_nought_and_the_right_at_one_half() {
        let [left, right] = feet(&pose(PLAGUE_WALK, 0.0, false));
        assert!(left[2] < right[2] - 4.0);
        let [left, right] = feet(&pose(PLAGUE_WALK, 0.5, false));
        assert!(right[2] < left[2] - 4.0);
    }

    #[test]
    fn toppled_he_lies_on_his_back() {
        let body = capsules(&pose(PLAGUE_WALK, 1.0, true));
        let head = body.iter().find(|c| c.part == Part::Head && c.a == c.b).unwrap();
        assert!(head.a[1] < 5.0 && head.a[2] > 30.0, "head {:?}", head.a);
    }

    #[test]
    fn his_face_stands_out_of_his_head() {
        let body = capsules(&pose(PLAGUE_WALK, 0.25, false));
        // Straight at his left eye from in front: the eye, not the head, is hit first.
        let eye = body.iter().find(|c| c.part == Part::Face(FACE_EYE_LEFT)).unwrap();
        let origin = [eye.a[0], eye.a[1], -50.0];
        let first = body.iter().filter_map(|c| ray_capsule(origin, [0.0, 0.0, 1.0], c, 100.0).map(|t| (t, c.part))).min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        assert_eq!(first.1, Part::Face(FACE_EYE_LEFT));
        let mouth = body.iter().find(|c| c.part == Part::Face(FACE_MOUTH)).unwrap();
        let origin = [mouth.a[0] + MOUTH_HALF, mouth.a[1], -50.0];
        let first = body.iter().filter_map(|c| ray_capsule(origin, [0.0, 0.0, 1.0], c, 100.0).map(|t| (t, c.part))).min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        assert_eq!(first.1, Part::Face(FACE_MOUTH));
    }

    #[test]
    fn rays_meet_capsules_on_the_side_and_the_caps() {
        let c = Capsule { part: Part::Body, a: [0.0, 0.0, 0.0], b: [0.0, 10.0, 0.0], radius: 1.0, solid: true };
        assert!((ray_capsule([-5.0, 5.0, 0.0], [1.0, 0.0, 0.0], &c, 100.0).unwrap() - 4.0).abs() < 1e-4);
        assert!((ray_capsule([0.0, 20.0, 0.0], [0.0, -1.0, 0.0], &c, 100.0).unwrap() - 9.0).abs() < 1e-4);
        assert!((ray_capsule([0.0, -5.0, 0.0], [0.0, 1.0, 0.0], &c, 100.0).unwrap() - 4.0).abs() < 1e-4);
        assert_eq!(ray_capsule([0.0, 5.0, 0.0], [1.0, 0.0, 0.0], &c, 100.0), Some(0.0));
        assert_eq!(ray_capsule([-5.0, 5.0, 0.0], [1.0, 0.0, 0.0], &c, 3.0), None);
        assert_eq!(ray_capsule([-5.0, 5.0, 3.0], [1.0, 0.0, 0.0], &c, 100.0), None);
        // A slanted capsule, hit square on its side.
        let s = Capsule { a: [0.0, 0.0, 0.0], b: [10.0, 10.0, 0.0], ..c };
        let t = ray_capsule([5.0, 5.0, -10.0], [0.0, 0.0, 1.0], &s, 100.0).unwrap();
        assert!((t - 9.0).abs() < 1e-4, "{t}");
    }

    #[test]
    fn progress_survives_the_wire() {
        for mode in [PLAGUE_WALK, PLAGUE_STOMP] {
            for p in [0.0, 0.3, 0.99] {
                let back = progress_of(mode, false, progress_byte(mode, false, p));
                assert!((back - p).abs() < 0.005, "{mode} {p} {back}");
            }
        }
        assert_eq!(progress_byte(PLAGUE_WALK, false, 1.0), 0, "a walk wraps");
    }

    /// The fixture the client's mirror of this module is checked against.
    fn parity_json() -> String {
        let cases: [(u8, f32, bool); 8] = [
            (PLAGUE_WALK, 0.0, false),
            (PLAGUE_WALK, 0.37, false),
            (PLAGUE_WALK, 0.81, false),
            (PLAGUE_STOMP, 0.4, false),
            (PLAGUE_STOMP, 1.0, false),
            (PLAGUE_STRIKE, 0.7, false),
            (PLAGUE_RECOVER, 0.3, false),
            (PLAGUE_WALK, 0.6, true),
        ];
        let v = |p: [f32; 3]| format!("[{:.4}, {:.4}, {:.4}]", p[0], p[1], p[2]);
        let entries: Vec<String> = cases
            .iter()
            .map(|&(mode, progress, toppling)| {
                let pose = pose(mode, progress, toppling);
                let body: Vec<String> = world_capsules(&pose, [10.0, 2.0, -5.0], 0.7)
                    .iter()
                    .map(|c| format!("      {{ \"a\": {}, \"b\": {}, \"radius\": {:.4}, \"solid\": {} }}", v(c.a), v(c.b), c.radius, c.solid))
                    .collect();
                let feet = feet(&pose).map(v).join(", ");
                format!("  {{\n    \"mode\": {mode}, \"progress\": {progress}, \"toppling\": {toppling},\n    \"feet\": [{feet}],\n    \"capsules\": [\n{}\n    ]\n  }}", body.join(",\n"))
            })
            .collect();
        format!("[\n{}\n]\n", entries.join(",\n"))
    }

    #[test]
    fn parity_fixture_matches_the_body() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/parity/plague.json");
        let actual = parity_json();
        if std::env::var_os("UPDATE_PARITY").is_some() {
            std::fs::write(&path, &actual).unwrap();
        }
        let committed = std::fs::read_to_string(&path).expect("fixtures/parity/plague.json; regenerate with UPDATE_PARITY=1");
        assert_eq!(committed, actual, "his body changed: regenerate the fixture and update client/src/sim/plague-body.ts");
    }
}
