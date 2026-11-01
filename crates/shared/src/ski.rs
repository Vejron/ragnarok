//! Skiing: where the groomed tracks are, and one tick of a soldier on skis. Speed is kept in
//! `Character::knock`, so a fall throws the ragdoll on at the speed it was going. The server and the
//! client's prediction both run this, through `sim::Physics::step_character`.

use std::collections::{HashMap, HashSet};

use crate::consts::*;
use crate::protocol::InputFrame;
use crate::quant::{from_axis, from_mrad};
use crate::world::SkiTrail;

/// Side of a cell of `SkiMap`, metres.
const CELL: f32 = 2.0;

/// The cells that lie on a groomed track. Lookups are by position alone, so it does not matter to
/// anything when a tile's tracks were added, and a tile that leaves can leave them behind.
#[derive(Clone, Debug, Default)]
pub struct SkiMap {
    cells: HashSet<(i32, i32)>,
    /// The pieces of track near each `SEGMENT_CELL`, for finding the grooves.
    segments: HashMap<(i32, i32), Vec<Segment>>,
    /// The pieces held, in centimetres: a tile that streams back in adds nothing twice.
    seen: HashSet<[i32; 4]>,
}

/// One straight piece of a track: its ends `[x, z]` and half its width, metres.
#[derive(Clone, Copy, Debug)]
struct Segment {
    a: [f32; 2],
    b: [f32; 2],
    half: f32,
}

/// Side of a cell of `SkiMap::segments`, metres.
const SEGMENT_CELL: f32 = 8.0;

/// The groove nearest a skier on a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Groove {
    /// Along the track, unit length, either way round.
    pub along: [f32; 2],
    /// How far the skier is off the groove's line, metres, towards `[-along[1], along[0]]`.
    pub off: f32,
}

impl SkiMap {
    pub fn add(&mut self, trail: &SkiTrail) {
        let reach = (crate::quant::from_cm(trail.width) / 2.0 / CELL).ceil() as i32;
        for pair in trail.points.windows(2) {
            if !self.seen.insert([pair[0][0], pair[0][1], pair[1][0], pair[1][1]]) {
                continue;
            }
            let [a, b] = [pair[0], pair[1]].map(|p| p.map(crate::quant::from_cm));
            self.add_segment(Segment { a, b, half: crate::quant::from_cm(trail.width) / 2.0 });
            let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / (CELL / 2.0)).ceil().max(1.0) as i32;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let (x, z) = cell(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
                for dx in -reach..=reach {
                    for dz in -reach..=reach {
                        if dx * dx + dz * dz <= reach * reach {
                            self.cells.insert((x + dx, z + dz));
                        }
                    }
                }
            }
        }
    }

    pub fn on_track(&self, x: f32, z: f32) -> bool {
        self.cells.contains(&cell(x, z))
    }

    /// Files a piece under every cell it passes through and their neighbours, so one look finds it.
    fn add_segment(&mut self, segment: Segment) {
        let Segment { a, b, .. } = segment;
        let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / (SEGMENT_CELL / 2.0)).ceil().max(1.0) as i32;
        let mut near = HashSet::new();
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let (x, z) = segment_cell(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    near.insert((x + dx, z + dz));
                }
            }
        }
        for key in near {
            self.segments.entry(key).or_default().push(segment);
        }
    }

    /// The nearer groove of the track under `(x, z)`, if a track is under it.
    pub fn groove(&self, x: f32, z: f32) -> Option<Groove> {
        let mut best: Option<(f32, Groove)> = None;
        for segment in self.segments.get(&segment_cell(x, z)).map(Vec::as_slice).unwrap_or_default() {
            let Segment { a, b, half } = *segment;
            let [dx, dz] = [b[0] - a[0], b[1] - a[1]];
            let length = dx.hypot(dz);
            if length < 1e-3 {
                continue;
            }
            let along = [dx / length, dz / length];
            let [px, pz] = [x - a[0], z - a[1]];
            let t = (px * along[0] + pz * along[1]).clamp(0.0, length);
            let distance = (px - along[0] * t).hypot(pz - along[1] * t);
            if distance > half || best.is_some_and(|(nearest, _)| nearest <= distance) {
                continue;
            }
            let side = -px * along[1] + pz * along[0];
            best = Some((distance, Groove { along, off: side - SKI_GROOVE_OFFSET.copysign(side) }));
        }
        best.map(|(_, groove)| groove)
    }
}

fn segment_cell(x: f32, z: f32) -> (i32, i32) {
    ((x / SEGMENT_CELL).floor() as i32, (z / SEGMENT_CELL).floor() as i32)
}

fn cell(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (z / CELL).floor() as i32)
}

/// What the skis are on this tick.
#[derive(Clone, Copy, Debug)]
pub struct SkiGround {
    /// The ground's unit normal.
    pub normal: [f32; 3],
    pub track: bool,
    /// The groove under the skis, on a track.
    pub groove: Option<Groove>,
}

impl SkiGround {
    pub const FLAT: SkiGround = SkiGround { normal: [0.0, 1.0, 0.0], track: false, groove: None };

    /// How fast a skier going at `velocity` (`[x, z]`) climbs, m/s: negative going down.
    pub fn climb(&self, velocity: [f32; 2]) -> f32 {
        let [nx, ny, nz] = self.normal;
        -(nx * velocity[0] + nz * velocity[1]) / ny.max(0.1)
    }

    /// How fast a skier meets this ground, flying at `velocity` (`[x, y, z]`), m/s: 0 when leaving it.
    pub fn impact(&self, velocity: [f32; 3]) -> f32 {
        let [nx, ny, nz] = self.normal;
        (-(nx * velocity[0] + ny * velocity[1] + nz * velocity[2])).max(0.0)
    }
}

/// One tick on the ground on skis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkiStep {
    pub knock: [f32; 2],
    /// Ticks the skis have been too far across the way they are going (`Character::ski_strain`).
    pub strain: u8,
    /// Caught an edge: the strain reached `SKI_STRAIN_FALL`.
    pub fell: bool,
}

fn dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

/// Where the skis point: where the player looks, turned a way off it by strafing. Unit length.
fn heading(input: &InputFrame) -> [f32; 2] {
    let (sin, cos) = from_mrad(input.yaw).sin_cos();
    let (ahead, right) = ([-sin, -cos], [cos, -sin]);
    let strafe = from_axis(input.strafe) * SKI_STRAFE_TURN;
    let [x, z] = [ahead[0] + right[0] * strafe, ahead[1] + right[1] * strafe];
    let length = x.hypot(z);
    [x / length, z / length]
}

/// The way along `groove` skis are held to, if they are: both where they point (`wanted`, unless the
/// player is aiming and does not steer) and how they are going (`facing`) lie near it. Pointing
/// further off lifts them out, and crossing a track at an angle never drops them in.
fn held(groove: Option<Groove>, wanted: [f32; 2], facing: [f32; 2], aiming: bool) -> Option<[f32; 2]> {
    let along = groove?.along;
    let way = if dot(along, facing) < 0.0 { [-along[0], -along[1]] } else { along };
    (dot(way, facing) > SKI_GROOVE_HOLD && (aiming || dot(way, wanted) > SKI_GROOVE_HOLD)).then_some(way)
}

/// The ski tick: gravity along the slope, friction and drag against the way it is going, push from
/// the poles, and the velocity turning towards the skis as far as they grip. Crouching is the tuck,
/// back the snowplow. Aim held coasts as it goes, not steering: the ski heading stays free of the look.
/// `armed` has a gun in hand, so the poles are stowed and only the legs push, however hard it tries.
pub fn ski_step(knock: [f32; 2], strain: u8, ground: SkiGround, input: &InputFrame, armed: bool, dt: f32) -> SkiStep {
    let buttons = input.buttons();
    let aiming = buttons & BUTTON_ALT != 0;
    let tuck = buttons & BUTTON_CROUCH != 0;
    let forward = from_axis(input.forward);
    let plow = forward < 0.0;
    let speed = knock[0].hypot(knock[1]);
    let wanted = heading(input);
    let [nx, ny, nz] = ground.normal;

    // Going slowly the skis are simply lifted round, and slipping back they still point up the hill;
    // past that, the velocity is what has to turn.
    let mut strain_next = strain.saturating_sub(SKI_STRAIN_EASE);
    let forwards = dot(knock, wanted);
    let shuffling = speed < SKI_SHUFFLE_SPEED || (forwards < 0.0 && speed <= SKI_SLIP_SPEED);
    let facing = if shuffling { wanted } else { [knock[0] / speed, knock[1] / speed] };
    let in_groove = held(ground.groove, wanted, facing, aiming);
    let (way, mut along) = if let Some(way) = in_groove {
        // The groove does the steering, round its bends too, and nothing strains.
        (way, dot(knock, way))
    } else if shuffling {
        (wanted, forwards)
    } else {
        let going = [knock[0] / speed, knock[1] / speed];
        if aiming {
            (going, speed)
        } else {
            let across = going[0] * wanted[1] - going[1] * wanted[0];
            let off = across.atan2(dot(going, wanted));
            let boost = if plow { SKI_PLOW_GRIP } else { 1.0 };
            let limit = SKI_GRIP * boost / speed * dt;
            let turn = off.clamp(-limit, limit);
            let (sin, cos) = turn.sin_cos();
            if speed >= SKI_FALL_MIN_SPEED && speed * off.abs() > SKI_EDGE * boost {
                strain_next = strain.saturating_add(1);
            }
            ([going[0] * cos - going[1] * sin, going[0] * sin + going[1] * cos], speed)
        }
    };

    let downhill = dot([nx * ny, nz * ny], way) * SKI_GRAVITY;
    // How steep it is going the way the skis go. Past the limit they just slip: nothing holds them.
    let hard = buttons & BUTTON_RUN != 0;
    let climb = (-dot([nx, nz], way) / ny.max(0.1)).atan();
    let limit = if hard { SKI_HERRING_SLOPE_RUN } else { SKI_HERRING_SLOPE };
    let pushing = forward > 0.0 && !tuck && !aiming && climb < limit;
    let stepping = pushing && climb > SKI_HERRING_FROM;
    let start = along;
    let boost = if hard && !armed { SKI_BOOST * (1.0 - along / SKI_BOOST_TOP).clamp(0.0, 1.0) } else { 0.0 };
    let power = if armed { SKI_KICK_POWER } else { SKI_POWER };
    let push = if pushing { forward * (power / along.abs().max(SKI_PUSH_MIN_SPEED) + boost) } else { 0.0 };
    along += (downhill + push) * dt;

    let mu = if in_groove.is_some() { SKI_MU_GROOVE } else if ground.track { SKI_MU_TRACK } else { SKI_MU_SNOW };
    let brake = mu * SKI_GRAVITY * ny + if plow { SKI_PLOW_BRAKE * -forward } else { 0.0 };
    let k = if tuck { SKI_DRAG_TUCK } else { SKI_DRAG };
    let slowed = (brake + k * along * along) * dt;
    // Friction stops a skier; it never starts one going the other way.
    along = if along.abs() <= slowed { 0.0 } else { along - slowed * along.signum() };
    // The wax holds against the snow under a push, and on a climb every step gets the skier that far up.
    if pushing && along < 0.0 {
        along = 0.0;
    }
    if stepping {
        let pace = if hard { SKI_HERRING_PACE_RUN } else { SKI_HERRING_PACE } * forward * (1.0 - SKI_HERRING_SLOWING * climb / limit);
        along = along.max(pace.min(start.max(0.0) + SKI_HERRING_ACCEL * dt));
    }
    let along = along.clamp(-SKI_SLIP_SPEED, SKI_MAX_SPEED);
    let mut next = [way[0] * along, way[1] * along];
    if let (Some(_), Some(groove)) = (in_groove, ground.groove) {
        // Drawn sideways on to its line, and only moving: a skier standing beside it stays there.
        let pull = (-groove.off * SKI_GROOVE_PULL).clamp(-SKI_GROOVE_PULL_MAX, SKI_GROOVE_PULL_MAX) * along.abs().min(1.0);
        next = [next[0] - groove.along[1] * pull, next[1] + groove.along[0] * pull];
    }
    SkiStep { knock: next, strain: strain_next, fell: strain_next >= SKI_STRAIN_FALL }
}

/// Air drag on a skier off the ground: a tuck cuts through the air there too.
pub fn ski_flight(knock: [f32; 2], input: &InputFrame, dt: f32) -> [f32; 2] {
    let k = if input.buttons() & BUTTON_CROUCH != 0 { SKI_DRAG_TUCK } else { SKI_DRAG };
    let speed = knock[0].hypot(knock[1]);
    let left = 1.0 - (k * speed * dt).min(1.0);
    [knock[0] * left, knock[1] * left]
}

/// Whether the ground drops away from a skier: the climb rate going at `knock` falls too fast between
/// `here` and the ground a look ahead (`ahead`) for the legs to follow, so they leave it.
pub fn over_crest(knock: [f32; 2], here: SkiGround, ahead: SkiGround) -> bool {
    knock[0].hypot(knock[1]) >= SKI_CREST_SPEED && here.climb(knock) - ahead.climb(knock) > SKI_CREST_DROP
}

/// Whether landing like this is a fall: too hard for the legs (more can be taken in a tuck), or
/// going fast with the skis well across the way they are going.
pub fn bad_landing(impact: f32, knock: [f32; 2], input: &InputFrame) -> bool {
    let tuck = input.buttons() & BUTTON_CROUCH != 0;
    if impact > SKI_LANDING * if tuck { SKI_LANDING_TUCK } else { 1.0 } {
        return true;
    }
    let speed = knock[0].hypot(knock[1]);
    let (sin, cos) = from_mrad(input.yaw).sin_cos();
    speed > SKI_LANDING_SKEW_SPEED && dot([knock[0] / speed, knock[1] / speed], [-sin, -cos]) < SKI_LANDING_SKEW.cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quant::to_mrad;

    const DT: f32 = 1.0 / TICK_RATE as f32;

    /// Ground sloping down towards -Z (the way yaw 0 looks) at `degrees`.
    fn slope(degrees: f32, track: bool) -> SkiGround {
        let (sin, cos) = degrees.to_radians().sin_cos();
        SkiGround { normal: [0.0, cos, -sin], track, groove: None }
    }

    fn input(forward: i8, buttons: u16) -> InputFrame {
        InputFrame { forward, buttons, ..Default::default() }
    }

    /// Speed along -Z after `seconds` from `start`.
    fn run(start: f32, ground: SkiGround, input: &InputFrame, seconds: f32) -> f32 {
        let mut knock = [0.0, -start];
        for _ in 0..(seconds / DT) as u32 {
            knock = ski_step(knock, 0, ground, input, false, DT).knock;
        }
        -knock[1]
    }

    #[test]
    fn a_downhill_glides_with_nobody_pushing() {
        let speed = run(0.0, slope(6.0, true), &input(0, 0), 40.0);
        assert!(speed > 6.0, "{speed}");
    }

    #[test]
    fn a_tuck_goes_faster_downhill() {
        let upright = run(0.0, slope(10.0, true), &input(0, 0), 60.0);
        let tucked = run(0.0, slope(10.0, true), &input(0, BUTTON_CROUCH), 60.0);
        assert!(tucked > upright * 1.3, "{tucked} {upright}");
        assert!((13.0..=SKI_MAX_SPEED).contains(&tucked), "{tucked}");
    }

    #[test]
    fn uphill_is_slow_going_and_flat_is_not() {
        let full = input(127, BUTTON_RUN);
        let flat = run(0.0, SkiGround { track: true, ..SkiGround::FLAT }, &full, 40.0);
        let climb = run(0.0, slope(-10.0, true), &full, 40.0);
        assert!((9.0..SKI_BOOST_TOP).contains(&flat), "{flat}");
        assert!((1.5..6.5).contains(&climb) && climb < flat * 0.65, "{climb} {flat}");
    }

    #[test]
    fn a_groomed_track_is_faster_than_loose_snow() {
        let full = input(127, BUTTON_RUN);
        let track = run(0.0, SkiGround { track: true, ..SkiGround::FLAT }, &full, 40.0);
        let snow = run(0.0, SkiGround::FLAT, &full, 40.0);
        assert!(track > snow * 1.1, "{track} {snow}");
    }

    #[test]
    fn run_held_is_a_real_boost_at_low_speed() {
        let (easy, hard) = (input(127, 0), input(127, BUTTON_RUN));
        let track = SkiGround { track: true, ..SkiGround::FLAT };
        assert!(run(4.0, track, &hard, 1.0) > run(4.0, track, &easy, 1.0) + 1.5);
    }

    #[test]
    fn past_the_top_speed_run_held_gives_nothing_more() {
        let (easy, hard) = (input(127, 0), input(127, BUTTON_RUN));
        let steep = slope(8.0, true);
        assert_eq!(run(SKI_BOOST_TOP + 1.0, steep, &hard, 0.1), run(SKI_BOOST_TOP + 1.0, steep, &easy, 0.1));
    }

    #[test]
    fn stopping_on_the_flat_stays_stopped() {
        assert_eq!(run(3.0, SkiGround::FLAT, &input(0, 0), 30.0), 0.0);
    }

    #[test]
    fn a_push_holds_on_a_climb_and_slips_past_the_limit() {
        assert!(run(0.0, slope(-20.0, true), &input(127, 0), 5.0) >= 0.0);
        assert!(run(0.0, slope(-20.0, true), &input(0, 0), 5.0) < 0.0);
        assert!(run(0.0, slope(-35.0, true), &input(127, 0), 5.0) < 0.0);
    }

    #[test]
    fn a_herringbone_gets_up_what_a_push_would_not() {
        assert!(run(0.0, slope(-28.0, false), &input(127, 0), 5.0) > 0.5);
        assert!(run(0.0, slope(-40.0, false), &input(127, BUTTON_RUN), 5.0) > 0.7);
        assert!(run(0.0, slope(-44.0, false), &input(127, BUTTON_RUN), 5.0) < 0.0);
    }

    #[test]
    fn the_snowplow_stops_a_skier() {
        assert!(run(12.0, slope(3.0, true), &input(-127, 0), 8.0) < 0.5);
    }

    fn turned(degrees: f32) -> InputFrame {
        InputFrame { yaw: to_mrad(degrees.to_radians()), ..Default::default() }
    }

    fn falls(speed: f32, input: &InputFrame) -> bool {
        let (mut knock, mut strain) = ([0.0, -speed], 0);
        for _ in 0..30 {
            let step = ski_step(knock, strain, SkiGround::FLAT, input, false, DT);
            if step.fell {
                return true;
            }
            (knock, strain) = (step.knock, step.strain);
        }
        false
    }

    #[test]
    fn turning_hard_at_speed_is_a_fall_and_gently_is_not() {
        assert!(falls(15.0, &turned(80.0)));
        assert!(!falls(15.0, &turned(15.0)));
        assert!(!falls(4.0, &turned(170.0)));
    }

    #[test]
    fn the_snowplow_takes_a_harder_turn() {
        let plowing = InputFrame { forward: -127, ..turned(60.0) };
        assert!(falls(15.0, &turned(60.0)));
        assert!(!falls(15.0, &plowing));
    }

    /// `run`, with a gun in hand.
    fn run_armed(start: f32, ground: SkiGround, input: &InputFrame, seconds: f32) -> f32 {
        let mut knock = [0.0, -start];
        for _ in 0..(seconds / DT) as u32 {
            knock = ski_step(knock, 0, ground, input, true, DT).knock;
        }
        -knock[1]
    }

    #[test]
    fn a_gun_in_hand_leaves_only_the_legs() {
        let track = SkiGround { track: true, ..SkiGround::FLAT };
        let armed = run_armed(0.0, track, &input(127, 0), 40.0);
        let poling = run(0.0, track, &input(127, 0), 40.0);
        assert!((2.0..poling * 0.6).contains(&armed), "{armed} {poling}");
        assert!(run_armed(0.0, SkiGround::FLAT, &input(127, 0), 40.0) > 0.8, "moves off in loose snow too");
    }

    #[test]
    fn run_held_adds_nothing_to_the_kick() {
        let track = SkiGround { track: true, ..SkiGround::FLAT };
        assert_eq!(run_armed(0.0, track, &input(127, BUTTON_RUN), 10.0), run_armed(0.0, track, &input(127, 0), 10.0));
    }

    #[test]
    fn a_gun_in_hand_still_climbs() {
        assert!(run_armed(0.0, slope(-15.0, false), &input(127, 0), 5.0) > 1.0);
    }

    #[test]
    fn aiming_a_gun_does_not_push() {
        let track = SkiGround { track: true, ..SkiGround::FLAT };
        assert_eq!(run_armed(0.0, track, &input(127, BUTTON_ALT), 5.0), 0.0);
    }

    #[test]
    fn aiming_coasts_straight_whatever_the_look() {
        let aimed = InputFrame { buttons: BUTTON_ALT, ..turned(120.0) };
        let step = ski_step([0.0, -15.0], 0, SkiGround::FLAT, &aimed, false, DT);
        assert!(step.knock[0].abs() < 1e-3 && step.strain == 0, "{step:?}");
    }

    #[test]
    fn a_crest_throws_the_fast_and_not_the_slow() {
        let up = slope(-10.0, false);
        let down = slope(10.0, false);
        assert!(over_crest([0.0, -12.0], up, down));
        assert!(!over_crest([0.0, -4.0], up, down));
        assert!(!over_crest([0.0, -12.0], down, up), "a dip presses a skier down");
    }

    #[test]
    fn a_tuck_loses_less_in_the_air() {
        let fly = |buttons| {
            let mut knock = [0.0, -20.0];
            for _ in 0..60 {
                knock = ski_flight(knock, &input(0, buttons), DT);
            }
            -knock[1]
        };
        assert!(fly(BUTTON_CROUCH) > fly(0) + 0.5 && fly(0) < 20.0);
    }

    #[test]
    fn landings() {
        let straight = [0.0, -12.0];
        assert!(!bad_landing(5.5, straight, &input(0, 0)));
        assert!(bad_landing(9.0, straight, &input(0, 0)));
        assert!(!bad_landing(9.0, straight, &input(0, BUTTON_CROUCH)));
        assert!(bad_landing(2.0, straight, &turned(60.0)));
    }

    #[test]
    fn meeting_a_slope_at_its_own_angle_is_no_impact() {
        // Flying down at the slope's own 20° onto it, at 12 m/s.
        let (sin, cos) = 20f32.to_radians().sin_cos();
        assert!(slope(20.0, false).impact([0.0, -12.0 * sin, -12.0 * cos]) < 1e-4);
        assert!(slope(20.0, false).climb([0.0, -10.0]) < 0.0);
    }

    /// A track along Z through the origin: the skier `off` its groove, towards -X (going -Z, that is left).
    fn in_groove(off: f32) -> SkiGround {
        SkiGround { track: true, groove: Some(Groove { along: [0.0, 1.0], off }), ..SkiGround::FLAT }
    }

    #[test]
    fn a_groove_holds_a_look_a_little_off_it_and_nothing_strains() {
        let step = ski_step([0.0, -10.0], 3, in_groove(0.0), &turned(20.0), false, DT);
        assert!(step.knock[0].abs() < 1e-4 && step.knock[1] < -9.0, "{step:?}");
        assert_eq!(step.strain, 1);
    }

    #[test]
    fn looking_well_off_a_groove_turns_out_of_it() {
        let mut knock = [0.0, -10.0];
        for _ in 0..30 {
            knock = ski_step(knock, 0, in_groove(0.0), &turned(40.0), false, DT).knock;
        }
        assert!(knock[0] < -1.0, "{knock:?}");
    }

    #[test]
    fn strafing_turns_out_of_a_groove_too() {
        let right = InputFrame { strafe: 127, ..Default::default() };
        let step = ski_step([0.0, -10.0], 0, in_groove(0.0), &right, false, DT);
        assert!(step.knock[0] > 0.01, "{step:?}");
    }

    #[test]
    fn crossing_a_track_at_an_angle_is_not_caught_by_a_groove() {
        let across = InputFrame { yaw: to_mrad(50f32.to_radians()), ..Default::default() };
        let (sin, cos) = 50f32.to_radians().sin_cos();
        let step = ski_step([-sin * 10.0, -cos * 10.0], 0, in_groove(0.2), &across, false, DT);
        assert!(step.knock[0] < -7.0, "{step:?}");
    }

    #[test]
    fn a_groove_draws_skis_to_its_line() {
        let step = ski_step([0.0, -10.0], 0, in_groove(0.5), &input(0, 0), false, DT);
        // `off` is towards -X here, so the pull is towards +X.
        assert!(step.knock[0] > 1.0, "{step:?}");
    }

    #[test]
    fn a_skier_standing_by_a_groove_stays_where_they_are() {
        assert_eq!(ski_step([0.0, 0.0], 0, in_groove(0.5), &input(0, 0), false, DT).knock, [0.0, 0.0]);
    }

    #[test]
    fn a_groove_is_faster_than_the_track_around_it() {
        let full = input(127, BUTTON_RUN);
        assert!(run(0.0, in_groove(0.0), &full, 40.0) > run(0.0, SkiGround { track: true, ..SkiGround::FLAT }, &full, 40.0));
    }

    #[test]
    fn the_map_finds_the_nearer_groove_and_nothing_beside_the_track() {
        let mut map = SkiMap::default();
        let trail = SkiTrail { width: 400, points: vec![[0, 0], [10_000, 0]] };
        map.add(&trail);
        map.add(&trail);
        assert_eq!(map.segments.values().map(Vec::len).max(), Some(1));
        let left = map.groove(50.0, 1.0).unwrap();
        let right = map.groove(50.0, -0.5).unwrap();
        assert!((left.off - 0.2).abs() < 1e-3, "{left:?}");
        assert!((right.off - 0.3).abs() < 1e-3, "{right:?}");
        assert_eq!(map.groove(50.0, 3.0), None);
        assert_eq!(map.groove(120.0, 0.0), None);
    }

    #[test]
    fn the_map_holds_the_width_of_a_track() {
        let mut map = SkiMap::default();
        map.add(&SkiTrail { width: 400, points: vec![[0, 0], [10_000, 0]] });
        assert!(map.on_track(50.0, 1.0));
        assert!(!map.on_track(50.0, 12.0));
        assert!(!map.on_track(150.0, 0.0));
    }
}
