//! Soldier movement rules that sit around the Rapier character controller. The server and the
//! client's prediction both run them, through `sim::Physics::step_character`.

use crate::consts::*;
use crate::protocol::InputFrame;
use crate::quant::{from_axis, from_mrad};

/// Horizontal velocity requested by an input, in m/s, as `[x, z]`.
pub fn wish_velocity(input: &InputFrame) -> [f32; 2] {
    wish_velocity_scaled(input, 1.0)
}

/// `wish_velocity` with the speed multiplied by `scale`, e.g. for walking slower while aiming a gun.
pub fn wish_velocity_scaled(input: &InputFrame, scale: f32) -> [f32; 2] {
    let strafe = from_axis(input.strafe);
    let forward = from_axis(input.forward);
    let yaw = from_mrad(input.yaw);
    let (sin, cos) = yaw.sin_cos();
    // Yaw 0 faces -Z. forward = (-sin, -cos), right = (cos, -sin).
    let mut x = -sin * forward + cos * strafe;
    let mut z = -cos * forward - sin * strafe;
    let length = (x * x + z * z).sqrt();
    if length > 1.0 {
        x /= length;
        z /= length;
    }
    let buttons = input.buttons();
    let speed = if buttons & BUTTON_PRONE != 0 {
        PRONE_SPEED
    } else if buttons & BUTTON_CROUCH != 0 {
        CROUCH_SPEED
    } else if buttons & BUTTON_RUN != 0 {
        RUN_SPEED
    } else {
        WALK_SPEED
    };
    [x * speed * scale, z * speed * scale]
}

/// Whether the parachute is open after this input. A press opens it in the air and cuts it when open; on the
/// ground it is always shut. A prone player cannot open one.
pub fn toggle_chute(chute: bool, grounded: bool, prone: bool, input: &InputFrame) -> bool {
    if grounded {
        return false;
    }
    if input.buttons() & BUTTON_CHUTE == 0 {
        return chute;
    }
    if chute { false } else { !prone }
}

/// The velocity a canopy is steered towards, `[x, z]` in m/s: full forward glides, back brakes.
pub fn glide_velocity(input: &InputFrame) -> [f32; 2] {
    let strafe = from_axis(input.strafe);
    let forward = from_axis(input.forward);
    let ahead = if forward >= 0.0 { forward * CHUTE_FORWARD } else { forward * CHUTE_BACK };
    let side = strafe * CHUTE_SIDE;
    let (sin, cos) = from_mrad(input.yaw).sin_cos();
    [-sin * ahead + cos * side, -cos * ahead - sin * side]
}

/// One tick of canopy steering: `knock`, which carries the glide, closes part of the gap to the wanted velocity.
pub fn steer_chute(knock: [f32; 2], input: &InputFrame) -> [f32; 2] {
    let [wx, wz] = glide_velocity(input);
    [knock[0] + (wx - knock[0]) * CHUTE_STEER, knock[1] + (wz - knock[1]) * CHUTE_STEER]
}

/// One tick of stamina on foot: the points spent after it, whether the player is winded, and the speed
/// a jump this tick takes off at. Running on the ground (moving, with run held, standing up and not
/// winded) spends it, and so does a rested jump; anything else with a foot down gets it back. Running
/// out leaves the player winded until `STAMINA_RECOVER` has come back.
pub fn spend_stamina(spent: u16, winded: bool, grounded: bool, prone: bool, input: &InputFrame) -> (u16, bool, f32) {
    let buttons = input.buttons();
    let jump_speed = if winded { JUMP_SPEED_WINDED } else { JUMP_SPEED };
    let jumping = grounded && buttons & BUTTON_JUMP != 0;
    let moving = input.forward != 0 || input.strafe != 0;
    let running = grounded && !winded && !prone && moving && buttons & BUTTON_RUN != 0 && buttons & (BUTTON_CROUCH | BUTTON_PRONE) == 0;
    let mut spent = spent;
    if jumping && !winded {
        spent = spent.saturating_add(STAMINA_JUMP_COST);
    }
    if running {
        spent = spent.saturating_add(STAMINA_RUN_DRAIN);
    } else if grounded && !jumping {
        spent = spent.saturating_sub(STAMINA_REFILL);
    }
    let spent = spent.min(STAMINA_FULL);
    let winded = spent == STAMINA_FULL || (winded && STAMINA_FULL - spent < STAMINA_RECOVER);
    (spent, winded, jump_speed)
}

/// The first half of a movement step: the new vertical velocity and the translation to hand to
/// the character controller. `knock` is the leftover knockback velocity, `[x, z]` in m/s. Under an open
/// `chute` it is the glide, and walking does not add to it. A jump takes off at `jump_speed`.
#[allow(clippy::too_many_arguments)]
pub fn desired_translation(vy: f32, grounded: bool, knock: [f32; 2], input: &InputFrame, dt: f32, speed_scale: f32, chute: bool, jump_speed: f32) -> (f32, [f32; 3]) {
    let mut vy = vy;
    if grounded && input.buttons() & BUTTON_JUMP != 0 {
        vy = jump_speed;
    }
    if chute {
        let target = -CHUTE_DESCENT;
        vy = if vy < target { (vy + CHUTE_DRAG * dt).min(target) } else { (vy - GRAVITY * dt).max(target) };
        return (vy, [knock[0] * dt, vy * dt, knock[1] * dt]);
    }
    vy = (vy - GRAVITY * dt).max(-MAX_FALL_SPEED);
    let [vx, vz] = wish_velocity_scaled(input, speed_scale);
    (vy, [(vx + knock[0]) * dt, vy * dt, (vz + knock[1]) * dt])
}

/// Starts a slide: a run-crouch press on the ground launches the player along where they were
/// heading, as knockback. Does nothing while one is already under way.
pub fn begin_slide(knock: [f32; 2], grounded: bool, input: &InputFrame) -> [f32; 2] {
    let buttons = input.buttons();
    let wanted = BUTTON_SLIDE | BUTTON_CROUCH;
    if !grounded || buttons & wanted != wanted || knock[0].hypot(knock[1]) >= SLIDE_RETRIGGER {
        return knock;
    }
    let [x, z] = wish_velocity(input);
    let length = x.hypot(z);
    if length < 1e-3 { knock } else { [x / length * SLIDE_SPEED, z / length * SLIDE_SPEED] }
}

/// Knockback after one tick: it bleeds off, fast on the ground and slowly in the air. Crouching
/// on the ground is a slide, which keeps more of it.
pub fn decay_knock(knock: [f32; 2], grounded: bool, crouching: bool) -> [f32; 2] {
    let keep = if !grounded {
        KNOCK_KEEP_AIR
    } else if crouching {
        SLIDE_KEEP
    } else {
        KNOCK_KEEP_GROUND
    };
    let next = [knock[0] * keep, knock[1] * keep];
    if (next[0] * next[0] + next[1] * next[1]).sqrt() < KNOCK_MIN_SPEED { [0.0, 0.0] } else { next }
}

/// The second half: given what the controller allowed, settle the vertical velocity. Landing or
/// bumping a ceiling cancels it, so a jump never "sticks" to a roof.
pub fn settle_vertical(vy: f32, desired_y: f32, moved_y: f32, grounded: bool) -> f32 {
    let landed = grounded && vy <= 0.0;
    let bumped_ceiling = desired_y > 0.0 && moved_y < desired_y * 0.5;
    if landed || bumped_ceiling { 0.0 } else { vy }
}

/// Ticks a blast throwing a soldier at `power` m/s knocks them down for: none below `KNOCKDOWN_POWER`.
pub fn knockdown_ticks(power: f32) -> u8 {
    if power < KNOCKDOWN_POWER {
        return 0;
    }
    let share = ((power - KNOCKDOWN_POWER) / (KNOCK_MAX_SPEED - KNOCKDOWN_POWER)).clamp(0.0, 1.0);
    (f32::from(KNOCKDOWN_MIN_TICKS) + share * f32::from(KNOCKDOWN_MAX_TICKS - KNOCKDOWN_MIN_TICKS)).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_at_zero_yaw_is_minus_z() {
        let [x, z] = wish_velocity(&InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: 0, pitch: 0, view: 0, slot: 0 });
        assert!(x.abs() < 1e-6);
        assert!((z + WALK_SPEED).abs() < 1e-5);
    }

    #[test]
    fn a_speed_scale_slows_every_gait() {
        let run = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 };
        let [_, z] = wish_velocity_scaled(&run, 0.5);
        assert!((z + RUN_SPEED * 0.5).abs() < 1e-4);
        assert_eq!(wish_velocity_scaled(&run, 1.0), wish_velocity(&run));
    }

    #[test]
    fn crouching_is_slow_even_when_running() {
        let [x, z] = wish_velocity(&InputFrame { seq: 1, strafe: 127, forward: 127, yaw: 0, buttons: BUTTON_CROUCH | BUTTON_RUN, pitch: 0, view: 0, slot: 0 });
        assert!(((x * x + z * z).sqrt() - CROUCH_SPEED).abs() < 1e-4);
    }

    #[test]
    fn slide_starts_once_along_the_run_and_lasts_while_crouched() {
        let press = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN | BUTTON_CROUCH | BUTTON_SLIDE, pitch: 0, view: 0, slot: 0 };
        let knock = begin_slide([0.0; 2], true, &press);
        assert!((knock[1] + SLIDE_SPEED).abs() < 1e-4 && knock[0].abs() < 1e-4);
        // Mid-slide, or in the air, or without the press, nothing happens.
        assert_eq!(begin_slide(knock, true, &press), knock);
        assert_eq!(begin_slide([0.0; 2], false, &press), [0.0; 2]);
        let held = InputFrame { seq: 1, strafe: 0, forward: 127, yaw: 0, buttons: BUTTON_RUN | BUTTON_CROUCH, pitch: 0, view: 0, slot: 0 };
        assert_eq!(begin_slide([0.0; 2], true, &held), [0.0; 2]);
        let crouched = (0..30).fold(knock, |k, _| decay_knock(k, true, true));
        let standing = (0..30).fold(knock, |k, _| decay_knock(k, true, false));
        assert!(crouched[1].abs() > standing[1].abs() * 2.0);
    }

    #[test]
    fn diagonal_is_not_faster() {
        let [x, z] = wish_velocity(&InputFrame { seq: 1, strafe: 127, forward: 127, yaw: 0, buttons: BUTTON_RUN, pitch: 0, view: 0, slot: 0 });
        assert!(((x * x + z * z).sqrt() - RUN_SPEED).abs() < 1e-4);
    }

    #[test]
    fn jump_only_from_ground() {
        let jump = InputFrame { seq: 1, strafe: 0, forward: 0, yaw: 0, buttons: BUTTON_JUMP, pitch: 0, view: 0, slot: 0 };
        assert!(desired_translation(0.0, true, [0.0; 2], &jump, TICK_DT, 1.0, false, JUMP_SPEED).0 > 0.0);
        assert!(desired_translation(0.0, false, [0.0; 2], &jump, TICK_DT, 1.0, false, JUMP_SPEED).0 < 0.0);
    }

    #[test]
    fn a_rested_jump_goes_1_6_times_as_high_as_a_winded_one() {
        let ratio = (JUMP_SPEED * JUMP_SPEED) / (JUMP_SPEED_WINDED * JUMP_SPEED_WINDED);
        assert!((ratio - 1.6).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn running_spends_stamina_and_walking_or_standing_gets_it_back() {
        let run = frame(127, BUTTON_RUN);
        assert_eq!(spend_stamina(0, false, true, false, &run), (STAMINA_RUN_DRAIN, false, JUMP_SPEED));
        // Run held without moving, crouched, or in the air is not running.
        assert_eq!(spend_stamina(0, false, true, false, &frame(0, BUTTON_RUN)).0, 0);
        assert_eq!(spend_stamina(100, false, true, false, &frame(127, BUTTON_RUN | BUTTON_CROUCH)).0, 100 - STAMINA_REFILL);
        assert_eq!(spend_stamina(100, false, false, false, &run).0, 100, "in the air nothing changes");
        assert_eq!(spend_stamina(100, false, true, false, &frame(127, 0)).0, 100 - STAMINA_REFILL);
        assert_eq!(spend_stamina(100, false, true, false, &frame(0, 0)).0, 100 - STAMINA_REFILL);
        assert_eq!(spend_stamina(5, false, true, false, &frame(0, 0)).0, 0);
    }

    #[test]
    fn a_rested_jump_costs_stamina_and_a_winded_one_is_lower_and_free() {
        let jump = frame(0, BUTTON_JUMP);
        assert_eq!(spend_stamina(0, false, true, false, &jump), (STAMINA_JUMP_COST, false, JUMP_SPEED));
        assert_eq!(spend_stamina(STAMINA_FULL, true, true, false, &jump), (STAMINA_FULL, true, JUMP_SPEED_WINDED));
        // A jump that empties the bar is still a full one, and leaves the player winded.
        assert_eq!(spend_stamina(STAMINA_FULL - 10, false, true, false, &jump), (STAMINA_FULL, true, JUMP_SPEED));
    }

    #[test]
    fn running_out_winds_the_player_until_enough_comes_back() {
        let run = frame(127, BUTTON_RUN);
        let (spent, winded, _) = spend_stamina(STAMINA_FULL - 1, false, true, false, &run);
        assert!(winded && spent == STAMINA_FULL);
        // Winded, run held is walking: it refills, but the player stays winded until `STAMINA_RECOVER`.
        let (mut spent, mut winded, _) = spend_stamina(spent, winded, true, false, &run);
        assert!(winded && spent == STAMINA_FULL - STAMINA_REFILL);
        let mut ticks = 1;
        while winded {
            (spent, winded, _) = spend_stamina(spent, winded, true, false, &run);
            ticks += 1;
        }
        assert!(STAMINA_FULL - spent >= STAMINA_RECOVER);
        assert_eq!(ticks, STAMINA_RECOVER.div_ceil(STAMINA_REFILL));
    }

    #[test]
    fn knockback_adds_to_walking_and_dies_out() {
        let idle = InputFrame::default();
        let (_, pushed) = desired_translation(0.0, true, [6.0, -3.0], &idle, TICK_DT, 1.0, false, JUMP_SPEED);
        assert!((pushed[0] - 6.0 * TICK_DT).abs() < 1e-6 && (pushed[2] + 3.0 * TICK_DT).abs() < 1e-6);
        let mut knock = [6.0, -3.0];
        for _ in 0..TICK_RATE * 3 {
            knock = decay_knock(knock, true, false);
        }
        assert_eq!(knock, [0.0, 0.0]);
        // In the air it lasts much longer.
        let air = (0..30).fold([6.0, 0.0], |k, _| decay_knock(k, false, false));
        assert!(air[0] > 4.0);
    }

    fn frame(forward: i8, buttons: u16) -> InputFrame {
        InputFrame { seq: 1, strafe: 0, forward, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 }
    }

    #[test]
    fn the_chute_toggles_in_the_air_only() {
        let press = frame(0, BUTTON_CHUTE);
        assert!(toggle_chute(false, false, false, &press));
        assert!(!toggle_chute(true, false, false, &press));
        assert!(!toggle_chute(false, true, false, &press), "no chute on the ground");
        assert!(!toggle_chute(true, true, false, &frame(0, 0)), "landing shuts it");
        assert!(toggle_chute(true, false, false, &frame(0, 0)), "it stays open");
        assert!(!toggle_chute(false, false, true, &press), "not while prone");
    }

    #[test]
    fn an_open_chute_slows_the_fall_to_a_steady_descent() {
        let idle = frame(0, 0);
        let mut vy = -40.0;
        for _ in 0..TICK_RATE * 3 {
            vy = desired_translation(vy, false, [0.0; 2], &idle, TICK_DT, 1.0, true, JUMP_SPEED).0;
        }
        assert!((vy + CHUTE_DESCENT).abs() < 1e-4, "settled at {vy}");
        // Without it the fall only speeds up.
        assert!(desired_translation(-40.0, false, [0.0; 2], &idle, TICK_DT, 1.0, false, JUMP_SPEED).0 < -40.0);
    }

    #[test]
    fn a_canopy_glides_forward_and_turns_lazily() {
        let mut knock = [0.0, 0.0];
        for _ in 0..TICK_RATE * 5 {
            knock = steer_chute(knock, &frame(127, 0));
        }
        assert!(knock[0].abs() < 1e-3 && (knock[1] + CHUTE_FORWARD).abs() < 0.1, "{knock:?}");
        // One tick of steering the other way barely moves it.
        let braked = steer_chute(knock, &frame(-127, 0));
        assert!((braked[1] - knock[1]).abs() < 1.0);
        // Walking does not add to the glide.
        let (_, moved) = desired_translation(-CHUTE_DESCENT, false, knock, &frame(127, BUTTON_RUN), TICK_DT, 1.0, true, JUMP_SPEED);
        assert!((moved[2] - knock[1] * TICK_DT).abs() < 1e-6);
    }

    #[test]
    fn a_push_knocks_down_from_its_threshold_for_longer_the_harder_it_is() {
        assert_eq!(knockdown_ticks(KNOCKDOWN_POWER - 0.01), 0, "a shove");
        assert_eq!(knockdown_ticks(KNOCKDOWN_POWER), KNOCKDOWN_MIN_TICKS);
        assert_eq!(knockdown_ticks(KNOCK_MAX_SPEED), KNOCKDOWN_MAX_TICKS);
        assert_eq!(knockdown_ticks(KNOCK_MAX_SPEED * 3.0), KNOCKDOWN_MAX_TICKS, "never longer");
        assert!(knockdown_ticks(12.0) > knockdown_ticks(9.0));
    }

}
