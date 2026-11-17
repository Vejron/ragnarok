//! How a ghost flies (GHOSTS.md). The server and the client's prediction both run it, through
//! `sim::Physics::step_character`; when a ghost may take shape again is the server's alone.

use crate::consts::*;
use crate::protocol::InputFrame;
use crate::quant::{from_axis, from_mrad};

/// One tick of a ghost's flight: the velocity `[x, y, z]` it flies at next, m/s, from the velocity
/// it had. Forward goes where the player looks, up or down as well; strafe goes sideways along the
/// ground; jump rises and crouch sinks. It eases towards what is asked at `GHOST_ACCEL`, so it drifts
/// to a stop rather than halting dead.
pub fn ghost_velocity(velocity: [f32; 3], input: &InputFrame, dt: f32) -> [f32; 3] {
    let strafe = from_axis(input.strafe);
    let forward = from_axis(input.forward);
    let (pitch_sin, pitch_cos) = from_mrad(input.pitch).sin_cos();
    let (sin, cos) = from_mrad(input.yaw).sin_cos();
    // Yaw 0 faces -Z. forward = (-sin, -cos), right = (cos, -sin).
    let ahead = forward * pitch_cos;
    let mut x = -sin * ahead + cos * strafe;
    let mut z = -cos * ahead - sin * strafe;
    let mut y = forward * pitch_sin;
    let length = (x * x + y * y + z * z).sqrt();
    if length > 1.0 {
        x /= length;
        y /= length;
        z /= length;
    }
    let buttons = input.buttons();
    let lift = if buttons & BUTTON_JUMP != 0 { GHOST_CLIMB } else { 0.0 } - if buttons & BUTTON_CROUCH != 0 { GHOST_CLIMB } else { 0.0 };
    let wanted = [x * GHOST_SPEED, (y * GHOST_SPEED + lift).clamp(-GHOST_SPEED, GHOST_SPEED), z * GHOST_SPEED];
    let gap = [wanted[0] - velocity[0], wanted[1] - velocity[1], wanted[2] - velocity[2]];
    let size = (gap[0] * gap[0] + gap[1] * gap[1] + gap[2] * gap[2]).sqrt();
    let step = GHOST_ACCEL * dt;
    if size <= step {
        wanted
    } else {
        let k = step / size;
        [velocity[0] + gap[0] * k, velocity[1] + gap[1] * k, velocity[2] + gap[2] * k]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quant::to_mrad;

    fn fly(input: &InputFrame, seconds: f32) -> [f32; 3] {
        let mut velocity = [0.0; 3];
        for _ in 0..(seconds * TICK_RATE as f32) as u32 {
            velocity = ghost_velocity(velocity, input, 1.0 / TICK_RATE as f32);
        }
        velocity
    }

    #[test]
    fn a_ghost_looking_up_flies_up_and_forward_at_full_speed() {
        let input = InputFrame { forward: 127, pitch: to_mrad(std::f32::consts::FRAC_PI_4), ..Default::default() };
        let [x, y, z] = fly(&input, 2.0);
        assert!(x.abs() < 1e-3 && z < -10.0 && y > 10.0, "{x} {y} {z}");
        assert!(((x * x + y * y + z * z).sqrt() - GHOST_SPEED).abs() < 1e-2);
    }

    #[test]
    fn a_ghost_left_alone_drifts_to_a_stop() {
        let still = ghost_velocity([GHOST_SPEED, 0.0, 0.0], &InputFrame::default(), 0.1);
        assert!(still[0] > 0.0 && still[0] < GHOST_SPEED);
        assert_eq!(fly(&InputFrame::default(), 1.0), [0.0; 3]);
    }

    #[test]
    fn jump_rises_and_crouch_sinks() {
        assert!((fly(&InputFrame { buttons: BUTTON_JUMP, ..Default::default() }, 1.0)[1] - GHOST_CLIMB).abs() < 1e-3);
        assert!((fly(&InputFrame { buttons: BUTTON_CROUCH, ..Default::default() }, 1.0)[1] + GHOST_CLIMB).abs() < 1e-3);
    }
}
