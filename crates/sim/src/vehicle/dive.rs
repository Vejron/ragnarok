//! What carries a submarine (`Lift::Dive`): a pressure hull trimmed to float neutrally. Up, it rides at
//! `SUB_SURFACE_DEPTH`, heaving and pitching with the waves as a ship does; its captain takes it down
//! with run and up with jump at `SUB_DIVE_RATE`, and with neither held it holds whatever depth it is at.
//! It cannot rise past floating, and the bed (Rapier's collision) stops it going lower. The waves fade
//! out below the surface. Its motor drives it ahead or astern, its rudder turns it while it makes way,
//! and its keel holds it to its heading. Its bow dips as it dives and lifts as it rises. A wreck floods
//! and goes down.
//!
//! Like `sail` it changes the body's velocities directly, from the body's state, the controls and the
//! tick alone, so the client predicts its own submarine with the same.

use super::water::Water;
use super::*;

/// From this far below its floating depth, metres, it holds its depth rather than floating back up.
const SURFACE_BAND: f32 = 0.5;
/// How hard it is pulled to its floating depth, per second, and how quickly its rise or fall follows
/// what is asked, per second.
const SURFACE_SPRING: f32 = 1.5;
const TRIM: f32 = 3.0;
/// The waves' heave fades out over this depth below floating, metres.
const SWELL_DEPTH: f32 = 3.0;
/// How fast a wreck goes down, m/s.
const FLOODING: f32 = 1.2;
/// Astern it makes this share of its top speed, with this share of its push; held back while it goes
/// ahead, it slows at `BRAKE` m/s², and with nobody aboard at `STOP` m/s².
const ASTERN_SPEED: f32 = 0.4;
const ASTERN_PUSH: f32 = 0.5;
const BRAKE: f32 = 1.0;
const STOP: f32 = 0.6;
/// The water's drag on the hull, per second.
const DRAG: f32 = 0.08;
/// How much of its sideways drift the keel takes out each second.
const KEEL: f32 = 2.5;
/// The rudder bites fully from this speed, m/s, through the water, and the yaw rate follows it this
/// quickly, per second.
const RUDDER_SPEED: f32 = 2.5;
const RUDDER: f32 = 2.0;
/// How hard the hull is brought level, per second², and how quickly its roll and pitch die away.
const LEVEL: f32 = 6.0;
const ROCK_DAMPING: f32 = 2.0;
/// How far the bow dips diving (lifts rising) at `SUB_DIVE_RATE`, radians.
const PLANES: f32 = 0.08;

/// `0` at `low`, `1` at `high`, smoothly in between.
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What vertical speed, m/s, a submarine `depth` metres down (its middle below the surface) wants,
/// with its captain's `lift` (-1 dives, 1 rises) and the surface coming up under it at `surfacing` m/s.
fn wanted_rise(depth: f32, lift: f32, surfacing: f32, wrecked: bool) -> f32 {
    if wrecked {
        return -FLOODING;
    }
    let asked = lift.clamp(-1.0, 1.0) * SUB_DIVE_RATE;
    // Up to where it floats, and no higher.
    let floating = (depth - SUB_SURFACE_DEPTH) * SURFACE_SPRING + surfacing;
    if asked < 0.0 {
        asked
    } else if asked > 0.0 {
        asked.min(floating)
    } else if depth < SUB_SURFACE_DEPTH + SURFACE_BAND {
        floating
    } else {
        0.0
    }
}

/// One tick of a submarine whose body is `body`, in `water` (`None` on dry land): before the pipeline
/// steps.
pub(crate) fn dive(body: &mut RigidBody, spec: &Spec, controls: CarControls, condition: CarCondition, water: Option<&Water>, dt: f32) {
    // Forces persist from step to step until reset. On dry land, or thrown clear of the water, it is
    // just a heavy box.
    body.reset_forces(false);
    let Some(water) = water else { return };
    let [_, hy, _] = spec.half_extents;
    let depth = water.height - body.translation().y;
    if depth <= -hy {
        return;
    }
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    let mut forward = rotation * Vector::new(0.0, 0.0, -1.0);
    forward.y = 0.0;
    let forward = forward.normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    let mut velocity = body.linvel();
    let speed = velocity.dot(forward);
    // How much of the hull the water holds up (all of it from floating down), and how much of the
    // waves still reach it.
    let held = smoothstep(-hy, SUB_SURFACE_DEPTH, depth);
    let swell = (-(depth - SUB_SURFACE_DEPTH).max(0.0) / SWELL_DEPTH).exp();

    // Neutral: the water bears its weight, and its trim takes it up or down as asked.
    let surfacing = (water.rise + velocity.x * water.slope[0] + velocity.z * water.slope[1]) * swell;
    let rise = wanted_rise(depth, controls.lift, surfacing, condition.wrecked);
    // Its weight is borne by a force, not an impulse: it is integrated together with gravity, so a
    // depth it holds is held exactly.
    body.add_force(Vector::Y * (CAR_GRAVITY * held * body.mass()), true);
    velocity.y += (rise - velocity.y) * (1.0 - (-TRIM * held * dt).exp());

    // The motor drives it ahead or astern; held back while going ahead, it brakes.
    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let mut flat = Vector::new(velocity.x, 0.0, velocity.z);
    let mut slowing = DRAG * flat.length();
    let power = condition.power * held;
    if throttle > 0.0 {
        flat += forward * (throttle * SUB_ACCEL * pull(speed, spec.max_speed * condition.speed) * power * dt);
    } else if throttle < 0.0 && speed > 0.3 {
        slowing += -throttle * BRAKE * held;
    } else if throttle < 0.0 {
        flat += forward * (throttle * SUB_ACCEL * ASTERN_PUSH * pull(speed, spec.max_speed * ASTERN_SPEED * condition.speed) * power * dt);
    }
    if controls.handbrake {
        slowing += STOP * held;
    }
    // The keel holds it to its heading.
    let side = flat.dot(right);
    flat -= right * side * (1.0 - (-KEEL * held * dt).exp());
    let going = flat.length();
    if going > 0.0 {
        flat *= (going - slowing * dt).max(0.0) / going;
    }
    body.set_linvel(Vector::new(flat.x, velocity.y, flat.z), true);

    // The rudder turns it as it makes way, and holds it straight when it is let be.
    let mut spin = body.angvel();
    let flow = (speed / RUDDER_SPEED).clamp(-1.0, 1.0);
    let yaw = -controls.steer.clamp(-1.0, 1.0) * spec.turn_rate * condition.turn * flow;
    spin.y += (yaw - spin.y) * (1.0 - (-RUDDER * held * dt).exp());
    // Level with the water on the surface and with the world below it, its bow dipping as it dives.
    let trim = PLANES * (velocity.y / SUB_DIVE_RATE).clamp(-1.0, 1.0);
    let level = (water.normal * swell + Vector::Y * (1.0 - swell) - forward * trim).normalize();
    let steady = (-ROCK_DAMPING * dt).exp();
    let local = rotation.inverse() * spin;
    spin = rotation * Vector::new(local.x * steady, local.y, local.z * steady);
    spin += up.cross(level) * (LEVEL * held * dt);
    body.set_angvel(spin, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::KIND_SUBMARINE;
    use shared::world::{NO_WATER, WORLD_VERSION, WorldFile};

    /// Flat ground at 0 over 600 m in 5 m cells, with a lake 10 m deep everywhere within 200 m of the
    /// middle (as deep as the sim's floor under a terrain lets a bed go), its surface level with the shore.
    fn sea() -> WorldFile {
        let side = 121u32;
        let wet = |i: u32, j: u32| {
            let (x, z) = (i as f32 * 5.0 - 300.0, j as f32 * 5.0 - 300.0);
            x.abs() <= 200.0 && z.abs() <= 200.0
        };
        let mut heights = Vec::new();
        let mut water = Vec::new();
        for j in 0..side {
            for i in 0..side {
                heights.push(if wet(i, j) { -1000 } else { 0 });
                water.push(if wet(i, j) { 0 } else { NO_WATER });
            }
        }
        WorldFile { version: WORLD_VERSION, half_extent: 60_000, ground: shared::terrain::Ground { origin: -30_000, cell: 500, side, heights }, water, ..Default::default() }
    }

    const SECOND: u32 = (1.0 / TICK_DT + 0.5) as u32;
    const AHEAD: CarControls = CarControls { throttle: 1.0, powered: true, ..CarControls::IDLE };
    const DOWN: CarControls = CarControls { lift: -1.0, powered: true, ..CarControls::IDLE };
    const UP: CarControls = CarControls { lift: 1.0, powered: true, ..CarControls::IDLE };
    const HOLD: CarControls = CarControls { powered: true, ..CarControls::IDLE };

    /// A submarine afloat at `[x, z]`, heading `yaw`.
    fn afloat(x: f32, z: f32, yaw: f32, calm: bool) -> Physics {
        let mut physics = Physics::new(&sea()).unwrap();
        if calm {
            physics.calm_waters();
        }
        physics.add_car(1, KIND_SUBMARINE, [x, -SUB_SURFACE_DEPTH, z], yaw);
        physics
    }

    fn run(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    #[test]
    fn on_still_water_it_floats_at_its_surface_depth() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        let pose = run(&mut physics, HOLD, 8 * SECOND);
        assert!((pose.position[1] + SUB_SURFACE_DEPTH).abs() < 0.1, "at {}", pose.position[1]);
        assert!(pose.up_y > 0.999);
    }

    #[test]
    fn on_a_swell_it_heaves_and_stays_upright() {
        let mut physics = afloat(0.0, 0.0, 0.3, false);
        let (mut low, mut high, mut tilt) = (f32::INFINITY, f32::NEG_INFINITY, 1.0f32);
        for tick in 0..20 * SECOND {
            let pose = run(&mut physics, HOLD, 1);
            if tick > 4 * SECOND {
                low = low.min(pose.position[1]);
                high = high.max(pose.position[1]);
                tilt = tilt.min(pose.up_y);
            }
        }
        assert!(low > -SUB_SURFACE_DEPTH - 1.5 && high < -SUB_SURFACE_DEPTH + 1.5, "it rides the swell, {low}..{high}");
        assert!(tilt > 0.97, "it stays upright, up {tilt}");
    }

    #[test]
    fn run_takes_it_down_and_let_go_it_holds_its_depth() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        run(&mut physics, HOLD, 2 * SECOND);
        let deep = run(&mut physics, DOWN, 3 * SECOND).position[1];
        assert!(deep < -SUB_SURFACE_DEPTH - 0.8 * 2.0 * SUB_DIVE_RATE, "it dived to {deep}");
        let held = run(&mut physics, HOLD, 10 * SECOND).position[1];
        assert!((held - deep).abs() < 1.0, "it held its depth: {deep} then {held}");
        let still = run(&mut physics, HOLD, 5 * SECOND).position[1];
        assert!((still - held).abs() < 0.1, "and stays there: {held} then {still}");
    }

    #[test]
    fn jump_brings_it_up_to_float_and_no_higher() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        run(&mut physics, DOWN, 5 * SECOND);
        let pose = run(&mut physics, UP, 15 * SECOND);
        assert!((pose.position[1] + SUB_SURFACE_DEPTH).abs() < 0.2, "it surfaced at {}", pose.position[1]);
        assert!(pose.velocity[1].abs() < 0.2, "and stopped there: {:?}", pose.velocity);
    }

    #[test]
    fn it_settles_on_the_bed_and_goes_no_lower() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        let pose = run(&mut physics, DOWN, 20 * SECOND);
        let bed = -10.0 + SUB_HALF_EXTENTS[1];
        assert!((pose.position[1] - bed).abs() < 0.3, "on the bed at {}, not {bed}", pose.position[1]);
        assert!(pose.up_y > 0.99);
    }

    #[test]
    fn its_motor_drives_it_and_the_rudder_turns_it_under_way_but_not_at_a_stand() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        run(&mut physics, CarControls { steer: 1.0, ..HOLD }, 3 * SECOND);
        assert!(physics.car_pose(1).unwrap().yaw().abs() < 0.05, "it does not turn standing still");
        let pose = run(&mut physics, AHEAD, 20 * SECOND);
        assert!(pose.forward_speed > 0.8 * SUB_MAX_SPEED, "it made way: {}", pose.forward_speed);
        let before = pose.yaw();
        let after = run(&mut physics, CarControls { steer: 1.0, ..AHEAD }, 3 * SECOND).yaw();
        assert!(before - after > 0.4, "it came round {before} to {after}");
    }

    #[test]
    fn deep_down_it_drives_as_well_as_on_the_surface() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        run(&mut physics, DOWN, 2 * SECOND);
        let pose = run(&mut physics, AHEAD, 20 * SECOND);
        assert!(pose.forward_speed > 0.8 * SUB_MAX_SPEED, "it made way: {}", pose.forward_speed);
        assert!(pose.position[1] < -4.0, "still down at {}", pose.position[1]);
    }

    #[test]
    fn a_wreck_floods_and_sinks_to_the_bed() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        physics.set_car_condition(1, CarCondition { wrecked: true, ..CarCondition::NEW });
        let pose = run(&mut physics, HOLD, 25 * SECOND);
        assert!(pose.position[1] < -10.0 + SUB_HALF_EXTENTS[1] + 0.8, "it went down to the bed: {:?}", pose.position);
    }
}
