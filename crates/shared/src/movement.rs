//! Stickman movement rules that sit around the Rapier character controller. Mirrored in
//! `client/src/sim/movement.ts`; the server and the client's prediction must agree exactly.

use crate::consts::*;
use crate::protocol::InputFrame;
use crate::quant::{from_axis, from_mrad};

/// Horizontal velocity requested by an input, in m/s, as `[x, z]`.
pub fn wish_velocity(input: &InputFrame) -> [f32; 2] {
    let strafe = from_axis(input.1);
    let forward = from_axis(input.2);
    let yaw = from_mrad(input.3);
    let (sin, cos) = yaw.sin_cos();
    // Yaw 0 faces -Z. forward = (-sin, -cos), right = (cos, -sin).
    let mut x = -sin * forward + cos * strafe;
    let mut z = -cos * forward - sin * strafe;
    let length = (x * x + z * z).sqrt();
    if length > 1.0 {
        x /= length;
        z /= length;
    }
    let speed = if input.buttons() & BUTTON_RUN != 0 { RUN_SPEED } else { WALK_SPEED };
    [x * speed, z * speed]
}

/// The first half of a movement step: the new vertical velocity and the translation to hand to
/// the character controller.
pub fn desired_translation(vy: f32, grounded: bool, input: &InputFrame, dt: f32) -> (f32, [f32; 3]) {
    let mut vy = vy;
    if grounded && input.buttons() & BUTTON_JUMP != 0 {
        vy = JUMP_SPEED;
    }
    vy = (vy - GRAVITY * dt).max(-MAX_FALL_SPEED);
    let [vx, vz] = wish_velocity(input);
    (vy, [vx * dt, vy * dt, vz * dt])
}

/// The second half: given what the controller allowed, settle the vertical velocity. Landing or
/// bumping a ceiling cancels it, so a jump never "sticks" to a roof.
pub fn settle_vertical(vy: f32, desired_y: f32, moved_y: f32, grounded: bool) -> f32 {
    let landed = grounded && vy <= 0.0;
    let bumped_ceiling = desired_y > 0.0 && moved_y < desired_y * 0.5;
    if landed || bumped_ceiling { 0.0 } else { vy }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_at_zero_yaw_is_minus_z() {
        let [x, z] = wish_velocity(&InputFrame(1, 0, 127, 0, 0));
        assert!(x.abs() < 1e-6);
        assert!((z + WALK_SPEED).abs() < 1e-5);
    }

    #[test]
    fn diagonal_is_not_faster() {
        let [x, z] = wish_velocity(&InputFrame(1, 127, 127, 0, BUTTON_RUN));
        assert!(((x * x + z * z).sqrt() - RUN_SPEED).abs() < 1e-4);
    }

    #[test]
    fn jump_only_from_ground() {
        let jump = InputFrame(1, 0, 0, 0, BUTTON_JUMP);
        assert!(desired_translation(0.0, true, &jump, TICK_DT).0 > 0.0);
        assert!(desired_translation(0.0, false, &jump, TICK_DT).0 < 0.0);
    }
}
