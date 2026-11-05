//! Golf. A course as the importer lays it out and the server plays it: `worlds/<name>/golf.json`, beside
//! the world it belongs to, positions in world metres `[x, z]`. And the swing meter, which the server
//! and the client both step through a golfer's input frames, so the swing is scored as it was seen.

use crate::consts::{
    GOLF_CHARGE_TICKS, GOLF_CURVE_FRAMES, GOLF_MISHIT_POWER, GOLF_MISS_FRAMES, GOLF_OVERSWING, GOLF_PURE_FRAMES, GOLF_REST_FRAMES, GOLF_RETURN_FRAMES, GOLF_ROLL_BUNKER, GOLF_ROLL_FAIRWAY,
    GOLF_ROLL_GREEN, GOLF_ROLL_ROUGH,
};
use crate::world::{COVER_GOLF_BUNKER, COVER_GOLF_FAIRWAY, COVER_GOLF_GREEN, COVER_GOLF_TEE};
use serde::{Deserialize, Serialize};

/// The file name of a world's course, in its directory.
pub const COURSE_FILE: &str = "golf.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Course {
    pub holes: Vec<Hole>,
    /// The mown surfaces, which the ball rolls on as their `kind` (`COVER_GOLF_*`) says; anywhere else
    /// is rough.
    pub surfaces: Vec<Surface>,
}

/// One hole: played from `tee` to the cup at `cup`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hole {
    pub number: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub par: Option<u8>,
    pub tee: [f32; 2],
    pub cup: [f32; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    pub kind: u8,
    pub ring: Vec<[f32; 2]>,
}

impl Course {
    /// The kind of surface (`COVER_GOLF_*`) at `[x, z]`, or `None` in the rough. A green or a bunker
    /// lying on a fairway is what the ball is on.
    pub fn surface_at(&self, at: [f32; 2]) -> Option<u8> {
        let rank = |kind: u8| u8::from(kind != COVER_GOLF_FAIRWAY);
        self.surfaces.iter().filter(|surface| inside(at, &surface.ring)).map(|surface| surface.kind).max_by_key(|&kind| rank(kind))
    }

    /// Rolling friction at `[x, z]`, of the ball's weight.
    pub fn roll_at(&self, at: [f32; 2]) -> f32 {
        match self.surface_at(at) {
            Some(COVER_GOLF_GREEN) => GOLF_ROLL_GREEN,
            Some(COVER_GOLF_FAIRWAY | COVER_GOLF_TEE) => GOLF_ROLL_FAIRWAY,
            Some(COVER_GOLF_BUNKER) => GOLF_ROLL_BUNKER,
            _ => GOLF_ROLL_ROUGH,
        }
    }
}

/// Whether `[x, z]` is inside `ring` (even-odd).
pub fn inside([x, z]: [f32; 2], ring: &[[f32; 2]]) -> bool {
    let mut odd = false;
    let mut j = ring.len().wrapping_sub(1);
    for i in 0..ring.len() {
        let ([xi, zi], [xj, zj]) = (ring[i], ring[j]);
        if (zi > z) != (zj > z) && x < (xj - xi) * (z - zi) / (zj - zi) + xi {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// The swing meter of the classic three clicks. Fire pressed starts it, held fills the power, and let
/// go sets it; a marker then runs back across the bar to the line at 0, and fire pressed again strikes.
/// Stepped once per input frame (`step`), it comes out the same wherever it is stepped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meter {
    pub stage: Stage,
    /// Whether fire was held in the last frame: every click wants a fresh press.
    pub held: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Stage {
    #[default]
    Idle,
    /// Fire held `frames` frames since it was pressed.
    Charging { frames: u32 },
    /// Let go at `power`, the marker running back for `frames` frames.
    Returning { power: f32, frames: u32 },
    /// Struck `frames` frames ago.
    Resting { frames: u32 },
}

/// How the club met the ball: `power` of a full swing (more in an overswing), and `curve`, from -1 (a
/// hook, to the left) through 0 (straight) to 1 (a slice, to the right); doubled in an overswing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub power: f32,
    pub curve: f32,
    /// Struck too far off the line: topped, weak and wild.
    pub mishit: bool,
}

/// The power `frames` frames of held fire charge.
pub fn charge(frames: u32) -> f32 {
    (frames as f32 / GOLF_CHARGE_TICKS as f32).min(GOLF_OVERSWING)
}

/// Frames from letting go at `power` until the marker is on the line.
pub fn line_frames(power: f32) -> u32 {
    (power * GOLF_RETURN_FRAMES as f32).round() as u32
}

/// The strike of a swing let go at `power`, fire pressed again `error` frames after the marker reached
/// the line (before it, negative).
pub fn strike(power: f32, error: i32) -> Strike {
    let off = error.unsigned_abs();
    let side = error.signum() as f32;
    if off > GOLF_MISS_FRAMES {
        return Strike { power: power * GOLF_MISHIT_POWER, curve: side, mishit: true };
    }
    // (Plus 0 makes a straight strike's -0 a plain 0.)
    let curve = (off.saturating_sub(GOLF_PURE_FRAMES) as f32 / GOLF_CURVE_FRAMES as f32).min(1.0) * side + 0.0;
    Strike { power, curve: if power > 1.0 { 2.0 * curve } else { curve }, mishit: false }
}

impl Meter {
    /// One input frame, with fire held or not: the strike, if this frame made one.
    pub fn step(&mut self, fire: bool) -> Option<Strike> {
        let fresh = fire && !self.held;
        self.held = fire;
        let (stage, struck) = match self.stage {
            Stage::Idle if fresh => (Stage::Charging { frames: 0 }, None),
            Stage::Idle => (Stage::Idle, None),
            Stage::Charging { frames } => {
                // Let go, or held to the top of the overswing, where it goes by itself.
                let frames = if fire { frames + 1 } else { frames };
                let power = charge(frames);
                if !fire || power >= GOLF_OVERSWING { (Stage::Returning { power, frames: 0 }, None) } else { (Stage::Charging { frames }, None) }
            }
            Stage::Returning { power, frames } => {
                let frames = frames + 1;
                let error = frames as i32 - line_frames(power) as i32;
                if fresh || error > GOLF_MISS_FRAMES as i32 {
                    (Stage::Resting { frames: 0 }, Some(strike(power, error)))
                } else {
                    (Stage::Returning { power, frames }, None)
                }
            }
            Stage::Resting { frames } if frames + 1 >= GOLF_REST_FRAMES => (Stage::Idle, None),
            Stage::Resting { frames } => (Stage::Resting { frames: frames + 1 }, None),
        };
        self.stage = stage;
        struck
    }

    /// Where the meter stands: the power charged or let go at, and the marker, on the bar where the
    /// line is 0 and a full swing 1; `None` while it is not going.
    pub fn reading(&self) -> Option<(f32, f32)> {
        match self.stage {
            Stage::Charging { frames } => Some((charge(frames), charge(frames))),
            Stage::Returning { power, frames } => Some((power, power - frames as f32 / GOLF_RETURN_FRAMES as f32)),
            _ => None,
        }
    }
}

/// Sample swings for the client's own meter to replay (`parity_json`): fire held or not in each frame
/// (`1`/`0`), and the strike it makes, at which frame.
pub fn meter_traces() -> Vec<(String, Option<(usize, Strike)>)> {
    let full = GOLF_CHARGE_TICKS as usize;
    let half = full / 2;
    let line = |held: usize| line_frames(charge(held as u32)) as usize;
    let swing = |held: usize, click: Option<usize>| {
        let mut fire = format!("00{}", "1".repeat(held + 1));
        if let Some(after) = click {
            fire.push_str(&"0".repeat(after - 1));
            fire.push_str("11");
        }
        fire.push_str(&"0".repeat(GOLF_RETURN_FRAMES as usize * 2));
        fire
    };
    let cases = [
        swing(half, Some(line(half))),
        swing(full, Some(line(full) - 4)),
        swing(full, Some(line(full) + 3)),
        swing(half, Some(line(half) - 12)),
        swing(half, None),
        "0".to_string() + &"1".repeat(full * 2) + &"0".repeat(GOLF_RETURN_FRAMES as usize * 2) + "1",
    ];
    cases
        .into_iter()
        .map(|fire| {
            let mut meter = Meter::default();
            let struck = fire.chars().enumerate().find_map(|(at, c)| meter.step(c == '1').map(|strike| (at, strike)));
            (fire, struck)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(kind: u8, [x0, z0]: [f32; 2], side: f32) -> Surface {
        Surface { kind, ring: vec![[x0, z0], [x0 + side, z0], [x0 + side, z0 + side], [x0, z0 + side]] }
    }

    /// Steps a fresh meter through `fire`, returning the first strike and the frame it came in.
    fn swing(fire: &[bool]) -> Option<(usize, Strike)> {
        let mut meter = Meter::default();
        fire.iter().enumerate().find_map(|(at, &held)| meter.step(held).map(|strike| (at, strike)))
    }

    /// Held for `held` frames after the press, then let go, then pressed again `after` frames later.
    fn clicks(held: u32, after: Option<u32>) -> Vec<bool> {
        let mut fire = vec![true; held as usize + 1];
        fire.push(false);
        if let Some(after) = after {
            fire.extend(std::iter::repeat_n(false, after as usize - 1));
            fire.push(true);
        }
        fire.extend(std::iter::repeat_n(false, 200));
        fire
    }

    #[test]
    fn a_click_on_the_line_strikes_straight_at_the_power_let_go_at() {
        let line = line_frames(charge(GOLF_CHARGE_TICKS / 2));
        let (_, strike) = swing(&clicks(GOLF_CHARGE_TICKS / 2, Some(line))).unwrap();
        assert_eq!(strike, Strike { power: 0.5, curve: 0.0, mishit: false });
        let (_, nearly) = swing(&clicks(GOLF_CHARGE_TICKS / 2, Some(line + GOLF_PURE_FRAMES))).unwrap();
        assert_eq!(nearly.curve, 0.0, "a frame off is still straight");
    }

    #[test]
    fn early_hooks_left_and_late_slices_right_and_far_off_is_a_mishit() {
        let line = line_frames(1.0);
        let early = swing(&clicks(GOLF_CHARGE_TICKS, Some(line - 4))).unwrap().1;
        assert!(early.curve < -0.3 && early.curve > -1.0 && !early.mishit, "a draw: {early:?}");
        let late = swing(&clicks(GOLF_CHARGE_TICKS, Some(line + GOLF_PURE_FRAMES + GOLF_CURVE_FRAMES))).unwrap().1;
        assert_eq!(late.curve, 1.0, "a full slice");
        let wild = swing(&clicks(GOLF_CHARGE_TICKS, Some(line - GOLF_MISS_FRAMES - 1))).unwrap().1;
        assert!(wild.mishit && wild.curve == -1.0 && wild.power < 0.5, "{wild:?}");
    }

    #[test]
    fn no_second_click_is_a_mishit_once_the_marker_is_past_the_line() {
        let (at, strike) = swing(&clicks(GOLF_CHARGE_TICKS, None)).unwrap();
        assert!(strike.mishit && strike.curve == 1.0);
        assert_eq!(at, GOLF_CHARGE_TICKS as usize + 1 + (line_frames(1.0) + GOLF_MISS_FRAMES + 1) as usize);
    }

    #[test]
    fn held_on_it_overswings_and_lets_go_by_itself_wanting_a_fresh_press() {
        let mut meter = Meter::default();
        let mut frames = 0;
        while !matches!(meter.stage, Stage::Returning { .. }) {
            assert!(meter.step(true).is_none());
            frames += 1;
        }
        let Stage::Returning { power, .. } = meter.stage else { unreachable!() };
        assert_eq!(power, GOLF_OVERSWING);
        assert!(frames > GOLF_CHARGE_TICKS && frames < GOLF_CHARGE_TICKS * 6 / 5);
        // Still held, it does not strike; let go and pressed on the line, it does, curve doubled.
        for _ in 0..line_frames(power) - 2 {
            assert!(meter.step(true).is_none());
        }
        assert!(meter.step(false).is_none());
        let strike = meter.step(true).unwrap();
        assert!(!strike.mishit && strike.power == GOLF_OVERSWING);
        assert_eq!(meter.reading(), None);
        // It rests before it starts again.
        meter.step(false);
        assert_eq!(meter.step(true), None);
        assert!(matches!(meter.stage, Stage::Resting { .. }));
    }

    #[test]
    fn the_reading_is_the_power_then_the_marker_running_back_to_the_line() {
        let mut meter = Meter::default();
        meter.step(true);
        for _ in 0..GOLF_CHARGE_TICKS {
            meter.step(true);
        }
        assert_eq!(meter.reading(), Some((1.0, 1.0)));
        meter.step(false);
        for _ in 0..line_frames(1.0) {
            meter.step(false);
        }
        let (power, marker) = meter.reading().unwrap();
        assert_eq!(power, 1.0);
        assert!(marker.abs() < 1e-5, "on the line: {marker}");
    }

    #[test]
    fn the_ball_rolls_by_what_it_is_on() {
        let course = Course { holes: Vec::new(), surfaces: vec![square(COVER_GOLF_FAIRWAY, [0.0, 0.0], 100.0), square(COVER_GOLF_GREEN, [40.0, 40.0], 20.0), square(COVER_GOLF_BUNKER, [10.0, 10.0], 5.0)] };
        assert_eq!(course.roll_at([50.0, 50.0]), GOLF_ROLL_GREEN);
        assert_eq!(course.roll_at([12.0, 12.0]), GOLF_ROLL_BUNKER);
        assert_eq!(course.roll_at([80.0, 20.0]), GOLF_ROLL_FAIRWAY);
        assert_eq!(course.roll_at([150.0, 20.0]), GOLF_ROLL_ROUGH);
    }
}
