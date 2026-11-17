//! What carries a boat (`Lift::Boat`), the Stridsbåt 90: a planing hull on two waterjets. It floats at
//! `CB90_DRAFT` on open water, heaving and pitching with the waves under it. The jets drive it ahead
//! and, with their reversing buckets down, stop it dead inside two lengths, its bow digging in and its
//! hull squatting into the water as it does, and take it astern; they swing it round even
//! standing still, and under way they turn it as hard as `turn_rate`. As it speeds up its bow lifts and
//! it climbs out of the water onto the plane, and it banks into its turns as a planing hull does. Where
//! the water is too shallow to float it, it is aground: nothing drives or steers it, and it sits on the
//! bed, as it does run up a beach. A wreck fills and sinks.
//!
//! Like `sail` it changes the body's velocities directly, from the body's state, the controls and the
//! tick alone, so the client predicts its own boat with the same.

use super::water::Water;
use super::*;

/// The share of the draft it floats at from which the jets bite and it is under way, and from which
/// it wholly is. Run up a beach, it sits lower than that on its keel.
const AFLOAT: [f32; 2] = [0.4, 0.75];
/// How quickly the hull's rise and fall follows the water's, per second.
const HEAVE_DAMPING: f32 = 3.0;
/// What is left of the water's hold on a wreck, which fills and goes down.
const WRECK_BUOYANCY: f32 = 0.3;
/// How hard the hull is brought level with the water under it, per second², and how quickly its roll
/// and pitch die away, per second.
const LEVEL: f32 = 10.0;
const ROCK_DAMPING: f32 = 3.0;
/// How far a wreck lists as it fills, radians.
const LIST: f32 = 0.4;
/// Astern the jets push with this share of their thrust.
const ASTERN_PUSH: f32 = 0.6;
/// With the throttle let go, the hull drops off the plane and slows at this much, m/s².
const COAST: f32 = 0.8;
/// The water's drag on the hull, per second.
const DRAG: f32 = 0.012;
/// How much of its sideways drift the hull takes out each second: it slides a little in a hard turn.
const KEEL: f32 = 2.5;
/// From this speed through the water, m/s, it turns at its full rate under way rather than its rate
/// standing still; and the yaw rate follows the helm this quickly, per second.
const HELM_SPEED: f32 = 5.0;
const HELM: f32 = 3.0;

/// `0` at `low`, `1` at `high`, smoothly in between.
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far onto the plane it is at `speed` ahead, m/s: 0 off it, 1 wholly on it.
fn planing(speed: f32) -> f32 {
    smoothstep(CB90_PLANE_SPEED[0], CB90_PLANE_SPEED[1], speed)
}

/// How far its bow is lifted at `speed` ahead, radians: most as it climbs onto the plane, a little once
/// it is on it.
fn bow_rise(speed: f32) -> f32 {
    let climbing = smoothstep(1.0, CB90_PLANE_SPEED[0], speed) * (1.0 - planing(speed));
    CB90_BOW_RISE * climbing + CB90_BOW_PLANING * planing(speed)
}

/// One tick of a boat whose body is `body`, over `water` (`None` on dry land): before the pipeline steps.
pub(crate) fn boat(body: &mut RigidBody, spec: &Spec, controls: CarControls, condition: CarCondition, water: Option<&Water>, dt: f32) {
    // On dry land it is just a heavy box.
    let Some(water) = water else { return };
    let [_, hy, _] = spec.half_extents;
    let rotation = *body.rotation();
    let under = (water.height - (body.translation().y - hy)).clamp(0.0, 2.0 * hy);
    if under <= 0.0 {
        return;
    }
    let up = rotation * Vector::Y;
    let mut forward = rotation * Vector::new(0.0, 0.0, -1.0);
    forward.y = 0.0;
    let forward = forward.normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    let mut velocity = body.linvel();
    let speed = velocity.dot(forward);

    // It floats at its draft, less as it rises onto the plane, rising and falling with the water under
    // it; a wreck fills and goes down.
    // Held back while it runs ahead, the buckets come down: a crash stop, which digs it in.
    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let stopping = if speed > 0.5 { (-throttle).max(f32::from(controls.handbrake)).clamp(0.0, 1.0) * smoothstep(0.5, CB90_PLANE_SPEED[1], speed) } else { 0.0 };
    let draft = CB90_DRAFT - CB90_PLANE_LIFT * planing(speed) + CB90_BRAKE_SQUAT * stopping;
    let afloat = if condition.wrecked { 0.0 } else { smoothstep(AFLOAT[0] * draft, AFLOAT[1] * draft, under) };
    let buoyancy = if condition.wrecked { WRECK_BUOYANCY } else { 1.0 };
    let surfacing = if condition.wrecked { 0.0 } else { water.rise + velocity.x * water.slope[0] + velocity.z * water.slope[1] };
    velocity.y += buoyancy * CAR_GRAVITY * under / draft * dt;
    velocity.y += (surfacing - velocity.y) * (1.0 - (-HEAVE_DAMPING * dt).exp());

    // The jets drive it ahead; held back while it runs ahead, the buckets come down and stop it, and
    // from a stop they take it astern. Let go, it drops off the plane and slows.
    let mut flat = Vector::new(velocity.x, 0.0, velocity.z);
    let mut slowing = DRAG * flat.length();
    let power = condition.power * afloat;
    if throttle > 0.0 {
        flat += forward * (throttle * CB90_THRUST * pull(speed, spec.max_speed * condition.speed) * power * dt);
    } else if throttle < 0.0 && speed > 0.5 {
        slowing += -throttle * CB90_BRAKE * afloat;
    } else if throttle < 0.0 {
        flat += forward * (throttle * CB90_THRUST * ASTERN_PUSH * pull(speed, CB90_ASTERN_SPEED * condition.speed) * power * dt);
    } else {
        slowing += COAST * afloat;
    }
    if controls.handbrake {
        slowing += CB90_BRAKE * afloat;
    }
    // The hull holds it to its heading, but for a little slide in a hard turn.
    let side = flat.dot(right);
    flat -= right * side * (1.0 - (-KEEL * afloat * dt).exp());
    let going = flat.length();
    if going > 0.0 {
        flat *= (going - slowing * dt).max(0.0) / going;
    }
    body.set_linvel(Vector::new(flat.x, velocity.y, flat.z), true);

    // The jets swing it round standing still, and turn it harder under way; let be, they hold it
    // straight.
    let mut spin = body.angvel();
    let flow = (speed / HELM_SPEED).clamp(0.0, 1.0);
    let rate = CB90_SPIN_RATE + (spec.turn_rate - CB90_SPIN_RATE) * flow;
    let yaw = -controls.steer.clamp(-1.0, 1.0) * rate * condition.turn * condition.power.min(1.0);
    spin.y += (yaw - spin.y) * (1.0 - (-HELM * afloat * dt).exp());
    // It rides on the water under it, its bow lifted as it climbs onto the plane, dug in as it crash
    // stops, and banked into its turn; a wreck lists over as it fills.
    let bank = (CB90_BANK * spin.y * speed).clamp(-CB90_BANK_MAX, CB90_BANK_MAX) * afloat;
    let trim = (bow_rise(speed) - CB90_BRAKE_DIP * stopping) * afloat;
    let list = if condition.wrecked { LIST } else { 0.0 };
    let level = (water.normal - right * (bank - list) - forward * trim).normalize();
    let steady = (-ROCK_DAMPING * dt).exp();
    let local = rotation.inverse() * spin;
    spin = rotation * Vector::new(local.x * steady, local.y, local.z * steady);
    spin += up.cross(level) * (LEVEL * (under / draft).min(1.0) * dt);
    body.set_angvel(spin, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::KIND_CB90;
    use shared::world::{NO_WATER, WORLD_VERSION, WorldFile};

    /// Flat ground at 0 over 1400 m in 5 m cells, with a lake 5 m deep everywhere within 600 m of the
    /// middle, its surface level with the shore.
    fn sea() -> WorldFile {
        let side = 281u32;
        let wet = |i: u32, j: u32| {
            let (x, z) = (i as f32 * 5.0 - 700.0, j as f32 * 5.0 - 700.0);
            x.abs() <= 600.0 && z.abs() <= 600.0
        };
        let mut heights = Vec::new();
        let mut water = Vec::new();
        for j in 0..side {
            for i in 0..side {
                heights.push(if wet(i, j) { -500 } else { 0 });
                water.push(if wet(i, j) { 0 } else { NO_WATER });
            }
        }
        WorldFile { version: WORLD_VERSION, half_extent: 70_000, ground: shared::terrain::Ground { origin: -70_000, cell: 500, side, heights }, reach: vec![u8::MAX; water.len()], water, ..Default::default() }
    }

    const SECOND: u32 = (1.0 / TICK_DT + 0.5) as u32;
    const FULL_AHEAD: CarControls = CarControls { throttle: 1.0, ..CarControls::IDLE };
    const REST: f32 = CB90_HALF_EXTENTS[1] - CB90_DRAFT;

    /// A boat afloat at `[x, z]`, heading `yaw`.
    fn afloat(x: f32, z: f32, yaw: f32, calm: bool) -> Physics {
        let mut physics = Physics::new(&sea()).unwrap();
        if calm {
            physics.calm_waters();
        }
        physics.add_car(1, KIND_CB90, [x, REST, z], yaw);
        physics
    }

    fn run(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    /// The boat's bow above level, radians.
    fn bow_up(pose: &CarPose) -> f32 {
        pose.world_dir([0.0, 0.0, -1.0])[1].asin()
    }

    /// How far it leans to the left (its right side up), radians.
    fn lean_left(pose: &CarPose) -> f32 {
        pose.world_dir([1.0, 0.0, 0.0])[1].asin()
    }

    #[test]
    fn on_still_water_it_settles_at_its_draft_and_nothing_drives_it() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        let pose = run(&mut physics, CarControls::IDLE, 8 * SECOND);
        assert!((pose.position[1] - REST).abs() < 0.1, "at {} not {REST}", pose.position[1]);
        assert!(pose.up_y > 0.999);
        assert!(pose.position[0].hypot(pose.position[2]) < 0.5, "it stays put: {:?}", pose.position);
    }

    #[test]
    fn on_a_swell_it_rides_the_waves_and_stays_upright() {
        let mut physics = afloat(0.0, 0.0, 0.3, false);
        let (mut low, mut high, mut tilt) = (f32::INFINITY, f32::NEG_INFINITY, 1.0f32);
        for tick in 0..20 * SECOND {
            let pose = run(&mut physics, CarControls::IDLE, 1);
            if tick > 4 * SECOND {
                low = low.min(pose.position[1]);
                high = high.max(pose.position[1]);
                tilt = tilt.min(pose.up_y);
            }
        }
        assert!(low > REST - 1.2 && high < REST + 1.4, "it rides the swell, {low}..{high} about {REST}");
        assert!(tilt > 0.97, "it stays upright, up {tilt}");
    }

    #[test]
    fn flat_out_it_makes_forty_five_knots_on_the_plane_with_its_bow_up_on_the_way() {
        // From the south shore heading north (-Z).
        let mut physics = afloat(0.0, 560.0, 0.0, true);
        let mut steepest = 0.0f32;
        let mut thirty_knots = None;
        for tick in 0..40 * SECOND {
            let pose = run(&mut physics, FULL_AHEAD, 1);
            steepest = steepest.max(bow_up(&pose));
            if thirty_knots.is_none() && pose.forward_speed > 15.4 {
                thirty_knots = Some(tick as f32 / SECOND as f32);
            }
        }
        let pose = physics.car_pose(1).unwrap();
        assert!(pose.forward_speed > 0.95 * CB90_MAX_SPEED && pose.forward_speed <= CB90_MAX_SPEED + 0.1, "flat out at {} m/s", pose.forward_speed);
        let thirty_knots = thirty_knots.expect("it reached 30 knots");
        assert!((3.0..6.0).contains(&thirty_knots), "30 knots in {thirty_knots} s");
        assert!(pose.position[1] > REST + 0.6 * CB90_PLANE_LIFT, "it rides up on the plane: {} over {REST}", pose.position[1]);
        assert!(steepest > 0.04, "its bow lifts as it climbs onto the plane: {steepest}");
        assert!(bow_up(&pose) < steepest * 0.7, "and comes down again once it planes: {} against {steepest}", bow_up(&pose));
        assert!(pose.up_y > 0.99);
    }

    #[test]
    fn flat_out_it_crash_stops_inside_two_lengths_its_bow_digging_in() {
        let mut physics = afloat(0.0, 560.0, 0.0, true);
        let cruising = run(&mut physics, FULL_AHEAD, 35 * SECOND);
        assert!(cruising.forward_speed > 0.95 * CB90_MAX_SPEED);
        let start = cruising;
        let (mut pose, mut nose_down, mut lowest) = (start, 0.0f32, start.position[1]);
        for _ in 0..10 * SECOND {
            pose = run(&mut physics, CarControls { throttle: -1.0, ..CarControls::IDLE }, 1);
            nose_down = nose_down.max(-bow_up(&pose));
            lowest = lowest.min(pose.position[1]);
            if pose.forward_speed < 0.5 {
                break;
            }
        }
        let distance = (pose.position[0] - start.position[0]).hypot(pose.position[2] - start.position[2]);
        assert!(pose.forward_speed < 0.5, "stopped: {}", pose.forward_speed);
        assert!(distance < 2.0 * 2.0 * CB90_HALF_EXTENTS[2], "it stopped in {distance} m");
        assert!(nose_down > 0.06, "its bow dug in: {nose_down} rad");
        assert!(lowest < REST - 0.1, "it squatted into the water: {lowest} against {REST} at rest");
        // Held back still, it goes astern, its bow back up.
        let pose = run(&mut physics, CarControls { throttle: -1.0, ..CarControls::IDLE }, 6 * SECOND);
        assert!(pose.forward_speed < -2.0 && pose.forward_speed > -CB90_ASTERN_SPEED - 0.1, "astern at {}", pose.forward_speed);
        assert!(bow_up(&pose).abs() < 0.03, "level again: {}", bow_up(&pose));
    }

    #[test]
    fn its_jets_turn_it_on_the_spot_and_harder_under_way() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        let before = physics.car_pose(1).unwrap().yaw();
        let after = run(&mut physics, CarControls { steer: 1.0, ..CarControls::IDLE }, 3 * SECOND).yaw();
        // Right helm turns it right, which is a falling yaw.
        let spun = before - after;
        assert!(spun > 0.5 && spun < 1.0, "it swung round {spun} rad standing still");

        let mut physics = afloat(0.0, 560.0, 0.0, true);
        run(&mut physics, FULL_AHEAD, 25 * SECOND);
        let before = physics.car_pose(1).unwrap().yaw();
        let after = run(&mut physics, CarControls { steer: 1.0, ..FULL_AHEAD }, 2 * SECOND).yaw();
        assert!(before - after > 0.55, "under way it came round {} rad in two seconds", before - after);
    }

    #[test]
    fn it_banks_into_its_turns() {
        let mut physics = afloat(0.0, 560.0, 0.0, true);
        run(&mut physics, FULL_AHEAD, 25 * SECOND);
        // Left helm: it leans left, into the turn.
        let pose = run(&mut physics, CarControls { steer: -1.0, ..FULL_AHEAD }, 3 * SECOND);
        let lean = lean_left(&pose);
        assert!(lean > 0.08 && lean < CB90_BANK_MAX + 0.02, "it banks left by {lean}");
        let pose = run(&mut physics, CarControls { steer: 1.0, ..FULL_AHEAD }, 4 * SECOND);
        assert!(lean_left(&pose) < -0.08, "and right the other way: {}", lean_left(&pose));
    }

    #[test]
    fn driven_at_the_shore_it_stops_against_it_upright() {
        // Heading -Z from z = -480 at full speed: the shore is at z = -600.
        let mut physics = afloat(0.0, -480.0, 0.0, true);
        let pose = run(&mut physics, FULL_AHEAD, 20 * SECOND);
        assert!(pose.position[2] > -630.0 && pose.position[2] < -585.0, "it stopped at the shore: {:?}", pose.position);
        let pose = run(&mut physics, FULL_AHEAD, 5 * SECOND);
        assert!(pose.position[2] > -615.0, "held at full ahead it gets no further up the bank: {:?}", pose.position);
        assert!(pose.up_y > 0.9, "it stays upright");
    }

    #[test]
    fn a_wreck_fills_and_sinks_to_the_bed() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        physics.set_car_condition(1, CarCondition { wrecked: true, ..CarCondition::NEW });
        let pose = run(&mut physics, CarControls::IDLE, 20 * SECOND);
        assert!(pose.position[1] < -5.0 + CB90_HALF_EXTENTS[1] + 0.8, "it went down to the bed: {:?}", pose.position);
    }
}
