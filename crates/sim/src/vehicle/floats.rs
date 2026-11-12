//! What carries a seaplane on open water (`Airframe::floats`): its pontoons hold it at their draft,
//! heaving and rocking with the waves under it. The water drags on them hard while they plough, and
//! lets go as the wings take the weight and it rises onto the step, so a take-off from water is a
//! longer run than one from a road. A water rudder turns it while it taxis, its keels hold it to its
//! heading, and coming down too hard costs it its speed and digs its nose in. One that goes in so deep
//! that its fuselage is under, lies on its side or is a wreck has filled: it sinks to the bed.
//!
//! Like `sail` and `wade` it changes the body's velocities directly, from the body's state, the
//! controls and the tick alone, so the client predicts its own seaplane with the same. The wings
//! (`wing`) work on top of it, and where the water is shallow its wheels meet the bed, so it taxis up a
//! beach.

use super::water::Water;
use super::*;

/// How quickly the pontoons' rise and fall follows the water's, per second.
const HEAVE_DAMPING: f32 = 10.0;
/// How hard it is brought level with the water under it, per second², and how quickly its roll and
/// pitch die away, per second, with the pontoons at their draft.
const LEVEL: f32 = 10.0;
const ROCK_DAMPING: f32 = 3.0;
/// The water's drag on pontoons at their draft: a hold that keeps it still against an idling engine,
/// m/s², then per second, and per metre (by the square of the speed).
const STILL: f32 = 0.4;
const DRAG: f32 = 0.1;
const PLOUGH: f32 = 0.014;
/// Slowing with the throttle cut (Shift) or nobody aboard, m/s².
const BRAKE: f32 = 3.0;
/// How much of its sideways drift the pontoons' keels take out each second.
const KEEL: f32 = 3.0;
/// The yaw rate the water rudder gives at full deflection, rad/s, how quickly the hull follows it, per
/// second, the speed it bites fully from, m/s, and the share of its bite the propeller's wash gives it
/// at a standstill.
const RUDDER_RATE: f32 = 0.8;
const RUDDER: f32 = 3.0;
const RUDDER_SPEED: f32 = 4.0;
const RUDDER_WASH: f32 = 0.35;
/// Coming down onto the water faster than this, m/s, slams: every m/s more takes `SLAM` m/s² off its
/// speed while it lasts, more again the more its nose is down, and tips its nose down by `NOSE_OVER` rad/s².
const SLAM_SPEED: f32 = 2.5;
const SLAM: f32 = 6.0;
const SLAM_NOSE: f32 = 4.0;
const NOSE_OVER: f32 = 1.5;
/// The steepest it may lean, as the up axis's height, and still be carried by its pontoons.
const UPRIGHT: f32 = 0.5;
/// Filled: what is left of gravity's pull on it, a share, how quickly its rise or fall dies away, per
/// second, and the drag on it, per second.
const SUNK_BUOYANCY: f32 = 0.5;
const SUNK_DAMPING: f32 = 3.0;
const SUNK_DRAG: f32 = 1.5;

/// One tick of a seaplane's pontoons, for a body `body` over `water`: after the wheels have had their
/// say and before the wings (`wing`) and the pipeline. Returns whether the pontoons are carrying it,
/// which counts as being on the ground. Does nothing to one clear of the water.
pub(crate) fn float(body: &mut RigidBody, airframe: &Airframe, floats: &Floats, controls: CarControls, condition: CarCondition, water: &Water, dt: f32) -> bool {
    let rotation = *body.rotation();
    let at = body.translation();
    let under = (water.height - (at.y - floats.keel)).clamp(0.0, floats.depth);
    if under <= 0.0 {
        return false;
    }
    let up = rotation * Vector::Y;
    let mut velocity = body.linvel();
    // Filled, it goes down slowly and settles on the bed.
    if condition.wrecked || up.y < UPRIGHT || water.height > at.y {
        velocity.y += SUNK_BUOYANCY * CAR_GRAVITY * dt;
        velocity.y *= (-SUNK_DAMPING * dt).exp();
        let drag = (-SUNK_DRAG * dt).exp();
        body.set_linvel(Vector::new(velocity.x * drag, velocity.y, velocity.z * drag), true);
        return false;
    }
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let forward = Vector::new(nose.x, 0.0, nose.z).normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    let speed = velocity.dot(forward);
    // How much of the weight the pontoons carry: all of it at their draft, more pushed deeper, and less
    // as the wings lift them out. `hold` is how firmly the water has them, 0..1.
    let wet = under / floats.draft;
    let hold = wet.min(1.0);

    // It floats at its draft, rising and falling with the water under it.
    let surfacing = water.rise + velocity.x * water.slope[0] + velocity.z * water.slope[1];
    let falling = surfacing - velocity.y;
    velocity.y += CAR_GRAVITY * wet * dt;
    velocity.y += (surfacing - velocity.y) * (1.0 - (-HEAVE_DAMPING * hold * dt).exp());

    // The water drags on the pontoons, the more the deeper they plough; a hard arrival costs speed.
    let mut flat = Vector::new(velocity.x, 0.0, velocity.z);
    let going = flat.length();
    let slam = (falling - SLAM_SPEED).max(0.0);
    let mut slowing = hold * (STILL + DRAG * going + PLOUGH * going * going) + SLAM * slam * (1.0 + SLAM_NOSE * (-nose.dot(water.normal)).max(0.0));
    if !controls.powered || controls.lift < -0.5 {
        slowing += BRAKE * hold;
    }
    // The keels hold it to its heading.
    let side = flat.dot(right);
    flat -= right * side * (1.0 - (-KEEL * hold * dt).exp());
    let going = flat.length();
    if going > 0.0 {
        flat *= (going - slowing * dt).max(0.0) / going;
    }
    body.set_linvel(Vector::new(flat.x, velocity.y, flat.z), true);

    // The water rudder turns it while it taxis, with the propeller's wash at a standstill; as the wings
    // start to fly the stick takes over.
    let mut spin = body.angvel();
    let flying = ((speed - 0.7 * airframe.stall_speed) / (0.3 * airframe.stall_speed)).clamp(0.0, 1.0);
    let steer = if controls.powered { controls.steer.clamp(-1.0, 1.0) } else { 0.0 };
    let bite = (speed.abs() / RUDDER_SPEED).clamp(RUDDER_WASH, 1.0) * if speed < -0.5 { -1.0 } else { 1.0 };
    let yaw = -steer * RUDDER_RATE * bite;
    spin.y += (yaw - spin.y) * (1.0 - (-RUDDER * hold * (1.0 - flying) * dt).exp());
    // It rides level on the water under it, rocking a little as it settles, and a slam tips its nose in.
    let steady = (-ROCK_DAMPING * hold * dt).exp();
    let local = rotation.inverse() * spin;
    spin = rotation * Vector::new(local.x * steady, local.y, local.z * steady);
    spin += up.cross(water.normal) * (LEVEL * hold * dt);
    // + about the hull's X lifts the nose.
    spin -= rotation * Vector::X * (NOSE_OVER * slam * dt);
    body.set_angvel(spin, true);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::{KIND_PLANE, KIND_SEAPLANE};
    use shared::world::{NO_WATER, WORLD_VERSION, WorldFile};

    /// A world 1,200 m across in 5 m cells whose ground is `bed(x, z)` metres high, with still water at
    /// height 0 wherever the ground is below it.
    fn world(bed: impl Fn(f32, f32) -> f32) -> WorldFile {
        let side = 241u32;
        let mut heights = Vec::new();
        let mut water = Vec::new();
        for j in 0..side {
            for i in 0..side {
                let height = bed(i as f32 * 5.0 - 600.0, j as f32 * 5.0 - 600.0);
                heights.push((height * 100.0) as i32);
                water.push(if height < 0.0 { 0 } else { NO_WATER });
            }
        }
        WorldFile { version: WORLD_VERSION, half_extent: 120_000, ground: shared::terrain::Ground { origin: -60_000, cell: 500, side, heights }, water, ..Default::default() }
    }

    /// Flat ground at 0 with a lake 5 m deep everywhere within 300 m of the middle.
    fn sea() -> WorldFile {
        world(|x, z| if x.abs() <= 300.0 && z.abs() <= 300.0 { -5.0 } else { 0.0 })
    }

    const SECOND: u32 = (1.0 / TICK_DT + 0.5) as u32;
    /// How high a seaplane's middle floats over still water, metres.
    const REST: f32 = SEAPLANE_KEEL - SEAPLANE_DRAFT;

    /// A pilot with the throttle at `lift` (Space +1, Shift -1), the stick pulled `pull` and the
    /// rudder (and ailerons) put over `steer` (D is +1).
    fn pilot(lift: f32, pull: f32, steer: f32) -> CarControls {
        CarControls { lift, throttle: -pull, steer, powered: true, ..CarControls::IDLE }
    }

    /// A seaplane afloat in the middle of `world`'s still water at `z`, heading −Z.
    fn afloat(world: &WorldFile, z: f32, calm: bool) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        if calm {
            physics.calm_waters();
        }
        physics.add_car(1, KIND_SEAPLANE, [0.0, REST, z], 0.0);
        physics
    }

    fn fly(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    /// How far a pilot at full throttle, easing the stick back, runs before the middle is `height` metres up.
    fn take_off_run(physics: &mut Physics, height: f32) -> f32 {
        let start = physics.car_pose(1).unwrap().position;
        for _ in 0..40 * SECOND {
            let pose = fly(physics, pilot(1.0, 0.3, 0.0), 1);
            if pose.position[1] > start[1] + height {
                return start[2] - pose.position[2];
            }
        }
        panic!("it never left: {:?}", physics.car_pose(1));
    }

    #[test]
    fn a_seaplane_rests_on_its_pontoons_level_at_its_draft() {
        let mut physics = afloat(&sea(), 0.0, true);
        let pose = fly(&mut physics, CarControls::IDLE, 8 * SECOND);
        assert!((pose.position[1] - REST).abs() < 0.05, "its middle floats {} m up, not {REST}", pose.position[1]);
        assert!(pose.up_y > 0.999, "level: up {}", pose.up_y);
        assert!(pose.position[0].hypot(pose.position[2]) < 0.2, "nothing moves it: {:?}", pose.position);
    }

    #[test]
    fn a_seaplane_rides_a_swell_upright_and_stays_put() {
        let mut physics = afloat(&sea(), 0.0, false);
        // How far its pontoons' undersides are under the water at its middle, and how it heaves and leans.
        let (mut shallow, mut deep, mut low, mut high, mut tilt) = (f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY, 1.0f32);
        // Once round the waves' whole loop.
        for tick in 0..shared::waves::LOOP_TICKS + 4 * SECOND {
            let pose = fly(&mut physics, CarControls::IDLE, 1);
            if tick > 4 * SECOND {
                let [x, y, z] = pose.position;
                let under = physics.waters.surface(x, z, shared::waves::time(physics.tick)).unwrap().height - (y - SEAPLANE_KEEL);
                (shallow, deep) = (shallow.min(under), deep.max(under));
                (low, high, tilt) = (low.min(y), high.max(y), tilt.min(pose.up_y));
            }
        }
        assert!(high - low > 0.2, "it heaves on the swell, {low}..{high}");
        assert!(shallow > -0.4 && deep < 1.0, "its pontoons stay in the water and its fuselage out of it: {shallow}..{deep} m under");
        assert!(tilt > 0.9, "it stays upright, up {tilt}");
        let drift = physics.car_pose(1).unwrap().position;
        assert!(drift[0].hypot(drift[2]) < 3.0, "with nobody aboard it lies where it is: {drift:?}");
    }

    #[test]
    fn a_seaplane_takes_off_from_water_in_a_longer_run_than_from_a_road() {
        let mut physics = afloat(&sea(), 250.0, true);
        let wet = take_off_run(&mut physics, 5.0);
        let mut physics = Physics::new(&world(|_, _| 0.0)).unwrap();
        physics.add_car(1, KIND_SEAPLANE, [0.0, SEAPLANE_RIDE_HEIGHT + 0.1, 250.0], 0.0);
        fly(&mut physics, CarControls::IDLE, SECOND);
        let dry = take_off_run(&mut physics, 5.0);
        assert!(wet > dry * 1.2 && wet < 300.0, "{wet} m off the water, {dry} m off a road");
    }

    #[test]
    fn a_seaplane_lands_on_water_and_stays_afloat() {
        let mut physics = afloat(&sea(), 250.0, true);
        // Low over the water at approach speed, sinking a little, with the throttle cut.
        physics.set_car_state(1, [0.0, REST + 3.0, 250.0], [0.0, 0.0, 0.0, 1.0], CarMotion { linvel: [0.0, -1.5, -22.0], angvel: [0.0; 3], steer: 0.0, spin: 0.0 }, None);
        let mut lowest = f32::INFINITY;
        for _ in 0..25 * SECOND {
            lowest = lowest.min(fly(&mut physics, pilot(-1.0, 0.0, 0.0), 1).position[1]);
        }
        let pose = physics.car_pose(1).unwrap();
        assert!(lowest > REST - 0.5, "it never went under: down to {lowest}");
        assert!((pose.position[1] - REST).abs() < 0.1 && pose.up_y > 0.99, "it floats level: {pose:?}");
        assert!(pose.forward_speed.abs() < 0.5, "and has stopped: {} m/s", pose.forward_speed);
        assert!(pose.position[2] > -250.0, "within the lake: {:?}", pose.position);
    }

    #[test]
    fn a_seaplane_turns_with_its_water_rudder_while_taxiing() {
        let heading = |pose: &CarPose| {
            let nose = pose.world_dir([0.0, 0.0, -1.0]);
            (-nose[0]).atan2(-nose[2])
        };
        // At a standstill the propeller's wash turns it slowly; with way on, quickly.
        let mut physics = afloat(&sea(), 0.0, true);
        let stopped = heading(&fly(&mut physics, pilot(0.0, 0.0, 1.0), 3 * SECOND));
        assert!(stopped < -0.3, "right rudder turns it right at idle: {stopped} rad");
        let mut physics = afloat(&sea(), 0.0, true);
        physics.set_car_velocity(1, [0.0, 0.0, -6.0]);
        let moving = heading(&fly(&mut physics, pilot(0.0, 0.0, 1.0), SECOND));
        assert!(moving < -0.4, "and faster with way on: {moving} rad in a second");
        let pose = physics.car_pose(1).unwrap();
        assert!(pose.up_y > 0.98, "it stays level in the turn: up {}", pose.up_y);
    }

    #[test]
    fn a_seaplane_that_dives_into_the_water_goes_under() {
        let mut physics = afloat(&sea(), 0.0, true);
        // Nose 45° down at 35 m/s, too low to pull out.
        let (sin, cos) = (-std::f32::consts::FRAC_PI_8).sin_cos();
        physics.set_car_state(1, [0.0, 4.0, 0.0], [sin, 0.0, 0.0, cos], CarMotion { linvel: [0.0, -25.0, -25.0], angvel: [0.0; 3], steer: 0.0, spin: 0.0 }, None);
        let pose = fly(&mut physics, pilot(1.0, 0.0, 0.0), 8 * SECOND);
        assert!(pose.position[1] < -1.0, "it is under: {:?}", pose.position);
    }

    #[test]
    fn a_wrecked_seaplane_sinks() {
        let mut physics = afloat(&sea(), 0.0, true);
        fly(&mut physics, CarControls::IDLE, SECOND);
        physics.set_car_condition(1, CarCondition { wrecked: true, ..CarCondition::NEW });
        let pose = fly(&mut physics, CarControls::IDLE, 10 * SECOND);
        assert!(pose.position[1] < -2.0, "it went down: {:?}", pose.position);
    }

    #[test]
    fn a_seaplane_taxis_up_a_beach_onto_its_wheels() {
        // A lake on the +Z side whose bed shelves up to a flat shore 1.2 m over the water at z = -70.
        let beach = world(|_, z| (-3.0 - z * 0.06).clamp(-5.0, 1.2));
        let mut physics = afloat(&beach, 20.0, true);
        // A burst of throttle to run it up the sand, then the throttle cut.
        for _ in 0..20 * SECOND {
            let pose = fly(&mut physics, pilot(1.0, 0.0, 0.0), 1);
            if pose.position[2] < -85.0 || pose.forward_speed > 11.0 {
                break;
            }
        }
        let mut pose = physics.car_pose(1).unwrap();
        for _ in 0..30 * SECOND {
            let lift = if pose.position[2] > -85.0 && pose.forward_speed < 8.0 { 1.0 } else { -1.0 };
            pose = fly(&mut physics, pilot(lift, 0.0, 0.0), 1);
        }
        assert!(pose.position[2] < -75.0, "it got ashore: {:?}", pose.position);
        assert!((pose.position[1] - 1.2 - SEAPLANE_RIDE_HEIGHT).abs() < 0.15 && pose.up_y > 0.99, "it stands on its wheels: {pose:?}");
    }

    #[test]
    fn a_triplane_has_no_pontoons_and_drops_through_the_water() {
        let mut physics = Physics::new(&sea()).unwrap();
        physics.calm_waters();
        physics.add_car(1, KIND_PLANE, [0.0, 1.0, 0.0], 0.0);
        let pose = fly(&mut physics, CarControls::IDLE, 4 * SECOND);
        assert!(pose.position[1] < -3.0, "it lies on the bed: {:?}", pose.position);
    }
}
