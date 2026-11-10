//! Swimming and diving rules (SWIMMING.md). The server and the client's prediction both run them,
//! through `sim::Physics::step_character`; the breath is the server's alone.

use crate::consts::*;
use crate::protocol::InputFrame;
use crate::quant::{from_axis, from_mrad};

/// Whether a soldier with `depth` metres of water over their feet swims, given whether they swam
/// last tick: they start in water deeper than `SWIM_DEPTH` and stop once it is shallower than
/// `SWIM_LEAVE_DEPTH`. `None` is dry ground.
pub fn swims(swimming: bool, depth: Option<f32>) -> bool {
    match depth {
        None => false,
        Some(depth) if swimming => depth >= SWIM_LEAVE_DEPTH,
        Some(depth) => depth > SWIM_DEPTH,
    }
}

/// Whether a swimmer `gap` metres below the float height is under the water, or going under this
/// tick: deep enough already (`SWIM_UNDER`), or diving (`ducking`).
pub fn diving(gap: f32, input: &InputFrame) -> bool {
    gap > SWIM_UNDER || ducking(input)
}

/// Whether a swimmer means to go under: crouch held, or swimming forward while looking down past
/// `SWIM_DIVE_PITCH`.
pub fn ducking(input: &InputFrame) -> bool {
    input.buttons() & BUTTON_CROUCH != 0 || (input.forward > 0 && from_mrad(input.pitch) < -SWIM_DIVE_PITCH)
}

/// One tick of a swimmer: their new vertical speed and the horizontal velocity `[x, z]` they swim at,
/// m/s. `vy` is the vertical speed they had and `gap` how far below the float height they are, metres
/// (negative above it), and `rise` how fast the waves lift the surface over them, m/s, which a floating
/// swimmer bobs with. `fast` is a stroke with run held. Under the water, or diving, forward swims where
/// the player looks; at the surface it is flat and the water holds them at the float height. Space
/// kicks up and crouch goes down. Nothing takes a swimmer above the float height, but one `grounded`
/// above it, clambering up a bank, is not pulled back down either.
#[allow(clippy::too_many_arguments)]
pub fn swim_velocity(vy: f32, gap: f32, rise: f32, grounded: bool, input: &InputFrame, speed_scale: f32, fast: bool, dt: f32) -> (f32, [f32; 2]) {
    let speed = if fast { SWIM_FAST_SPEED } else { SWIM_SPEED } * speed_scale;
    let strafe = from_axis(input.strafe);
    let forward = from_axis(input.forward);
    let under = diving(gap, input);
    let (pitch_sin, pitch_cos) = if under { from_mrad(input.pitch).sin_cos() } else { (0.0, 1.0) };
    let (sin, cos) = from_mrad(input.yaw).sin_cos();
    // Yaw 0 faces -Z. forward = (-sin, -cos), right = (cos, -sin).
    let ahead = forward * pitch_cos;
    let mut x = -sin * ahead + cos * strafe;
    let mut z = -cos * ahead - sin * strafe;
    let mut up = forward * pitch_sin;
    let length = (x * x + z * z + up * up).sqrt();
    if length > 1.0 {
        x /= length;
        z /= length;
        up /= length;
    }
    let buttons = input.buttons();
    let wanted = if buttons & BUTTON_JUMP != 0 {
        SWIM_RISE
    } else if buttons & BUTTON_CROUCH != 0 {
        -SWIM_SINK
    } else if under && forward != 0.0 {
        up * speed
    } else if under {
        SWIM_DRIFT
    } else {
        (gap * SWIM_BUOYANCY + rise).clamp(-SWIM_RISE, SWIM_RISE)
    };
    // A dive from the surface goes straight under, and does not hang just below it.
    let wanted = if gap <= SWIM_UNDER && ducking(input) && buttons & BUTTON_JUMP == 0 { wanted.min(-SWIM_DUCK) } else { wanted };
    // The water holds them at the float height: nothing swims out of it.
    let held = gap * SWIM_BUOYANCY + rise;
    let wanted = wanted.min(if grounded { held.max(0.0) } else { held });
    let wanted = if grounded && buttons & BUTTON_CROUCH == 0 { wanted.max(0.0) } else { wanted };
    let rate = (SWIM_ACCEL + (wanted - vy).abs() * SWIM_DRAG) * dt;
    let vy = vy + (wanted - vy).clamp(-rate, rate);
    (vy, [x * speed, z * speed])
}

/// The air left after one tick, out of `BREATH_FULL` ticks: one less under the water, `BREATH_REFILL`
/// more with the eyes out of it.
pub fn breathe(breath: u16, under: bool) -> u16 {
    if under { breath.saturating_sub(1) } else { breath.saturating_add(BREATH_REFILL).min(BREATH_FULL) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(forward: i8, buttons: u16, pitch: f32) -> InputFrame {
        InputFrame { forward, buttons, pitch: crate::quant::to_mrad(pitch), ..Default::default() }
    }

    #[test]
    fn swimming_starts_deeper_than_it_stops() {
        assert!(!swims(false, None));
        assert!(!swims(false, Some(1.2)), "wading");
        assert!(swims(false, Some(1.4)));
        assert!(swims(true, Some(1.2)), "still swimming on the way out");
        assert!(!swims(true, Some(1.0)));
        assert!(!swims(true, None));
    }

    #[test]
    fn a_floating_swimmer_stays_at_the_float_height() {
        let (vy, [x, z]) = swim_velocity(0.0, 0.0, 0.0, false, &frame(127, 0, 0.0), 1.0, false, TICK_DT);
        assert_eq!(vy, 0.0);
        assert!((x.hypot(z) - SWIM_SPEED).abs() < 1e-3, "flat out at swimming speed");
    }

    #[test]
    fn looking_well_down_while_swimming_forward_dives_straight_under() {
        let mut vy = 0.0;
        for _ in 0..60 {
            vy = swim_velocity(vy, 0.0, 0.0, false, &frame(127, 0, -1.1), 1.0, false, TICK_DT).0;
        }
        assert!(vy <= -SWIM_DUCK + 1e-3, "diving at {vy}");
        let level = swim_velocity(0.0, 0.0, 0.0, false, &frame(127, 0, -0.75), 1.0, false, TICK_DT).0;
        assert_eq!(level, 0.0, "looking down as one does swimming along is not a dive");
    }

    #[test]
    fn just_under_the_surface_the_water_floats_a_swimmer_back_up() {
        let mut vy = 0.0;
        for _ in 0..30 {
            vy = swim_velocity(vy, 0.6, 0.0, false, &frame(127, 0, -0.3), 1.0, false, TICK_DT).0;
        }
        assert!(vy > 1.0, "rising at {vy}");
    }

    #[test]
    fn under_the_water_looking_up_swims_up_and_still_drifts_up() {
        let mut vy = 0.0;
        for _ in 0..60 {
            vy = swim_velocity(vy, 3.0, 0.0, false, &frame(127, 0, 0.8), 1.0, false, TICK_DT).0;
        }
        assert!(vy > 1.0, "rising at {vy}");
        let mut vy = 0.0;
        for _ in 0..60 {
            vy = swim_velocity(vy, 3.0, 0.0, false, &frame(0, 0, 0.0), 1.0, false, TICK_DT).0;
        }
        assert!((vy - SWIM_DRIFT).abs() < 1e-3, "drifting at {vy}");
    }

    #[test]
    fn a_floating_swimmer_rises_and_falls_with_the_waves() {
        let mut vy = 0.0;
        for _ in 0..60 {
            vy = swim_velocity(vy, 0.0, 0.4, false, &frame(0, 0, 0.0), 1.0, false, TICK_DT).0;
        }
        assert!((vy - 0.4).abs() < 1e-3, "lifted with the swell: {vy}");
    }

    #[test]
    fn a_kick_up_stops_at_the_surface() {
        let vy = swim_velocity(SWIM_RISE, 0.0, 0.0, false, &frame(0, BUTTON_JUMP, 0.0), 1.0, false, TICK_DT).0;
        assert!(vy < SWIM_RISE, "braking at the float height: {vy}");
    }

    #[test]
    fn a_plunge_is_braked_within_a_second() {
        let mut vy = -20.0;
        for _ in 0..60 {
            vy = swim_velocity(vy, 2.0, 0.0, false, &frame(0, 0, 0.0), 1.0, false, TICK_DT).0;
        }
        assert!(vy.abs() < 1.0, "{vy}");
    }

    #[test]
    fn breath_runs_out_in_thirty_seconds_and_comes_back_in_five() {
        let mut breath = BREATH_FULL;
        for _ in 0..30 * TICK_RATE {
            breath = breathe(breath, true);
        }
        assert_eq!(breath, 0);
        for _ in 0..5 * TICK_RATE {
            breath = breathe(breath, false);
        }
        assert_eq!(breath, BREATH_FULL);
    }
}
