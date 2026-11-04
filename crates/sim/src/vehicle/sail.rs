//! What carries a ship (`Lift::Sail`): it floats on open water, heaving and pitching with the waves
//! under its hull, and the wind in its sail drives it (`shared::wind`): hardest with it on the beam,
//! and into the wind, where the sail only flaps, a small hidden motor (`SHIP_MOTOR`) still pushes it. Its rudder turns it only while it makes way, and its keel holds it to
//! its heading. Where the water is too shallow to float it, it is aground: nothing drives or steers it,
//! and it sits on the bed. A wreck fills and sinks.
//!
//! Like `wade` it changes the body's velocities directly, from the body's state, the controls and
//! the tick alone, so the client predicts its own ship with the same.

use super::water::Water;
use super::*;

/// How deep the hull floats, metres: its underside settles this far below the surface.
const DRAFT: f32 = SHIP_DRAFT;
/// The share of `DRAFT` from which the keel is clear of the bed and the ship is under way, and from
/// which it is wholly so.
const AFLOAT: [f32; 2] = [0.6, 0.9];
/// How quickly the hull's rise and fall follows the water's, per second.
const HEAVE_DAMPING: f32 = 2.0;
/// What is left of the water's hold on a wreck, which fills and goes down.
const WRECK_BUOYANCY: f32 = 0.3;
/// How hard the hull is brought level with the water under it, per second², and how quickly its roll
/// and pitch die away, per second.
const LEVEL: f32 = 8.0;
const ROCK_DAMPING: f32 = 2.0;
/// How far it heels away from the wind with a full sail on the beam, radians, and how far a wreck lists.
const HEEL: f32 = 0.12;
const LIST: f32 = 0.4;
/// The wind's push with all sail set on a beam reach, m/s².
const SAIL_ACCEL: f32 = 2.0;
/// Backing the sail with the throttle held back slows it, m/s²; the anchor (the handbrake) more.
const BACK_SAIL: f32 = 1.2;
const ANCHOR: f32 = 2.5;
/// Stopped, sweeps push it astern: this hard, m/s², and this fast at most, m/s.
const SWEEPS: f32 = 0.5;
const SWEEPS_SPEED: f32 = 1.5;
/// The water's drag on the hull, per second.
const DRAG: f32 = 0.1;
/// How much of its sideways drift the keel takes out each second.
const KEEL: f32 = 2.5;
/// The rudder bites fully from this speed, m/s, through the water, and the yaw rate follows it this
/// quickly, per second.
const RUDDER_SPEED: f32 = 2.0;
const RUDDER: f32 = 3.0;

/// `0` at `low`, `1` at `high`, smoothly in between.
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One tick of a ship whose body is `body`, over `water` (`None` on dry land), in `wind` (a unit `[x,
/// z]`, which way it blows): before the pipeline steps.
pub(crate) fn sail(body: &mut RigidBody, spec: &Spec, controls: CarControls, condition: CarCondition, water: Option<&Water>, wind: [f32; 2], dt: f32) {
    // On dry land it is just a heavy box.
    let Some(water) = water else { return };
    let [_, hy, _] = spec.half_extents;
    let rotation = *body.rotation();
    let under = (water.height - (body.translation().y - hy)).clamp(0.0, 2.0 * hy);
    if under <= 0.0 {
        return;
    }
    let afloat = if condition.wrecked { 0.0 } else { smoothstep(AFLOAT[0] * DRAFT, AFLOAT[1] * DRAFT, under) };
    let up = rotation * Vector::Y;
    let mut forward = rotation * Vector::new(0.0, 0.0, -1.0);
    forward.y = 0.0;
    let forward = forward.normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    let mut velocity = body.linvel();
    let speed = velocity.dot(forward);

    // It floats at its draft, rising and falling with the water under it; a wreck fills and goes down.
    let buoyancy = if condition.wrecked { WRECK_BUOYANCY } else { 1.0 };
    let surfacing = if condition.wrecked { 0.0 } else { water.rise + velocity.x * water.slope[0] + velocity.z * water.slope[1] };
    velocity.y += buoyancy * CAR_GRAVITY * under / DRAFT * dt;
    velocity.y += (surfacing - velocity.y) * (1.0 - (-HEAVE_DAMPING * dt).exp());

    // The wind drives it as far as it fills the sail; held back, the sail is backed, and from a stop
    // the sweeps take it astern, off a bank it has run onto.
    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let drive = shared::wind::point_of_sail([forward.x, forward.z], wind).max(SHIP_MOTOR);
    let mut flat = Vector::new(velocity.x, 0.0, velocity.z);
    let mut slowing = DRAG * flat.length();
    if throttle > 0.0 {
        let headroom = pull(speed, spec.max_speed * drive * condition.speed);
        flat += forward * (throttle * SAIL_ACCEL * drive * headroom * condition.power * afloat * dt);
    } else if throttle < 0.0 && speed > 0.3 {
        slowing += -throttle * BACK_SAIL * afloat;
    } else if throttle < 0.0 {
        let afloat = if condition.wrecked { 0.0 } else { smoothstep(0.0, AFLOAT[0] * DRAFT, under) };
        flat += forward * (throttle * SWEEPS * pull(speed, SWEEPS_SPEED) * afloat * dt);
    }
    if controls.handbrake {
        slowing += ANCHOR * afloat;
    }
    // The keel holds it to its heading.
    let side = flat.dot(right);
    flat -= right * side * (1.0 - (-KEEL * afloat * dt).exp());
    let going = flat.length();
    if going > 0.0 {
        flat *= (going - slowing * dt).max(0.0) / going;
    }
    body.set_linvel(Vector::new(flat.x, velocity.y, flat.z), true);

    // The rudder turns it as it makes way through the water, and holds it straight when it is let be.
    let mut spin = body.angvel();
    let flow = (speed / RUDDER_SPEED).clamp(-1.0, 1.0);
    let yaw = -controls.steer.clamp(-1.0, 1.0) * spec.turn_rate * condition.turn * flow;
    spin.y += (yaw - spin.y) * (1.0 - (-RUDDER * afloat * dt).exp());
    // It rides level on the water under it, heeled away from the wind as it fills the sail, rocking a
    // little as it settles: the deeper it is in, the harder it is held. A wreck lists over as it fills.
    let filled = drive * throttle.max(0.0) * afloat;
    let heel = HEEL * filled * (wind[0] * right.x + wind[1] * right.z) + if condition.wrecked { LIST } else { 0.0 };
    let level = (water.normal + right * heel).normalize();
    let steady = (-ROCK_DAMPING * dt).exp();
    let local = rotation.inverse() * spin;
    spin = rotation * Vector::new(local.x * steady, local.y, local.z * steady);
    spin += up.cross(level) * (LEVEL * (under / DRAFT).min(1.0) * dt);
    body.set_angvel(spin, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::{KIND_SHIP, KIND_SHIP_GUN};
    use shared::world::{NO_WATER, WORLD_VERSION, WorldFile};

    /// Flat ground at 0 over 600 m in 5 m cells, with a lake 5 m deep everywhere within 200 m of the
    /// middle, its surface level with the shore.
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
                heights.push(if wet(i, j) { -500 } else { 0 });
                water.push(if wet(i, j) { 0 } else { NO_WATER });
            }
        }
        WorldFile { version: WORLD_VERSION, half_extent: 60_000, ground: shared::terrain::Ground { origin: -30_000, cell: 500, side, heights }, water, ..Default::default() }
    }

    const SECOND: u32 = (1.0 / TICK_DT + 0.5) as u32;
    const FULL_SAIL: CarControls = CarControls { throttle: 1.0, ..CarControls::IDLE };

    /// A ship afloat at `[x, z]`, heading `yaw`, on still water at a moment when the wind blows
    /// towards +X (it blows `shared::wind::HEADING` on average; at a quarter of its loop it is furthest
    /// round, so the tick is set where it crosses +X instead).
    fn afloat(x: f32, z: f32, yaw: f32, calm: bool) -> Physics {
        let mut physics = Physics::new(&sea()).unwrap();
        if calm {
            physics.calm_waters();
        }
        physics.add_car(1, KIND_SHIP, [x, SHIP_HALF_EXTENTS[1] - SHIP_DRAFT, z], yaw);
        physics.set_tick(due_east());
        physics
    }

    /// A tick at which the wind blows (nearly) straight towards +X.
    fn due_east() -> u32 {
        (0..shared::wind::LOOP_TICKS).step_by(30).min_by_key(|&tick| (shared::wind::wind(tick)[1].abs() * 1e6) as u32).unwrap()
    }

    fn sail_for(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    #[test]
    fn left_alone_on_a_swell_it_floats_at_its_draft_and_stays_upright() {
        let mut physics = afloat(0.0, 0.0, 0.3, false);
        let (mut low, mut high, mut tilt) = (f32::INFINITY, f32::NEG_INFINITY, 1.0f32);
        for tick in 0..20 * SECOND {
            let pose = sail_for(&mut physics, CarControls::IDLE, 1);
            if tick > 4 * SECOND {
                low = low.min(pose.position[1]);
                high = high.max(pose.position[1]);
                tilt = tilt.min(pose.up_y);
            }
        }
        let rest = SHIP_HALF_EXTENTS[1] - SHIP_DRAFT;
        assert!(low > rest - 1.2 && high < rest + 1.4, "it rides the swell, {low}..{high} about {rest}");
        assert!(high - low > 0.2, "it heaves on the swell, {low}..{high}");
        assert!(tilt > 0.97, "it stays upright, up {tilt}");
        let drift = physics.car_pose(1).unwrap().position;
        assert!(drift[0].hypot(drift[2]) < 3.0, "nothing drives it: {drift:?}");
    }

    #[test]
    fn on_still_water_it_settles_at_its_draft() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        let pose = sail_for(&mut physics, CarControls::IDLE, 8 * SECOND);
        let rest = SHIP_HALF_EXTENTS[1] - SHIP_DRAFT;
        assert!((pose.position[1] - rest).abs() < 0.1, "at {} not {rest}", pose.position[1]);
        assert!(pose.up_y > 0.999);
    }

    /// How far a ship heading `yaw` sails under full sail in `seconds`, with the wind towards +X.
    fn run(yaw: f32, seconds: u32) -> f32 {
        let mut physics = afloat(-60.0 * yaw.sin(), -60.0 * yaw.cos(), yaw, true);
        let start = physics.car_pose(1).unwrap().position;
        let end = sail_for(&mut physics, FULL_SAIL, seconds * SECOND).position;
        (end[0] - start[0]).hypot(end[2] - start[2])
    }

    #[test]
    fn it_sails_fastest_on_a_beam_reach_and_motors_slowly_into_the_wind() {
        // Yaw 0 heads -Z: square across a wind towards +X. -π/2 heads +X, before it; π/2 into it.
        let beam = run(0.0, 20);
        let broad = run(-0.8, 20);
        let into = run(std::f32::consts::FRAC_PI_2, 20);
        assert!(beam > 120.0, "a beam reach makes way: {beam} m in 20 s");
        assert!(broad > 0.6 * beam && broad < beam, "a broad reach is a little slower: {broad} against {beam}");
        assert!(into > 0.2 * beam && into < 0.45 * beam, "into the wind only the motor drives it: {into}");
    }

    #[test]
    fn the_rudder_turns_it_under_way_but_not_at_a_stand() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        sail_for(&mut physics, CarControls { steer: 1.0, ..CarControls::IDLE }, 3 * SECOND);
        assert!(physics.car_pose(1).unwrap().yaw().abs() < 0.05, "it does not turn standing still");
        sail_for(&mut physics, FULL_SAIL, 15 * SECOND);
        let before = physics.car_pose(1).unwrap().yaw();
        let after = sail_for(&mut physics, CarControls { steer: 1.0, ..FULL_SAIL }, 3 * SECOND).yaw();
        // Right rudder turns it right, which is a falling yaw.
        assert!(before - after > 0.4, "it came round {before} to {after}");
    }

    #[test]
    fn sailed_onto_the_shore_it_runs_aground_and_stays_there() {
        // Heading -Z from z = -150 on a beam reach: the shore is at z = -200.
        let mut physics = afloat(0.0, -150.0, 0.0, true);
        let pose = sail_for(&mut physics, FULL_SAIL, 60 * SECOND);
        assert!(pose.position[2] > -215.0 && pose.position[2] < -185.0, "it stopped at the shore: {:?}", pose.position);
        let stuck = pose.position;
        let pose = sail_for(&mut physics, FULL_SAIL, 10 * SECOND);
        assert!(vec3::distance(pose.position, stuck) < 1.0, "aground, the wind does not move it: {stuck:?} to {:?}", pose.position);
        assert!(vec3::length(pose.velocity) < 0.2);
    }

    #[test]
    fn on_dry_land_it_goes_nowhere() {
        let mut physics = afloat(0.0, -260.0, 0.0, true);
        let start = sail_for(&mut physics, CarControls::IDLE, 3 * SECOND).position;
        let end = sail_for(&mut physics, FULL_SAIL, 10 * SECOND).position;
        assert!((end[0] - start[0]).hypot(end[2] - start[2]) < 0.2, "stranded: {start:?} to {end:?}");
    }

    #[test]
    fn a_wreck_fills_and_sinks_to_the_bed() {
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        physics.set_car_condition(1, CarCondition { wrecked: true, ..CarCondition::NEW });
        let pose = sail_for(&mut physics, CarControls::IDLE, 20 * SECOND);
        assert!(pose.position[1] < -5.0 + SHIP_HALF_EXTENTS[1] + 0.8, "it went down to the bed: {:?}", pose.position);
    }

    #[test]
    fn its_guns_go_wherever_their_spot_on_deck_goes_and_kick_it_when_they_fire() {
        let mut physics = afloat(0.0, 0.0, 0.0, false);
        let fittings = shared::vehicles::vehicle_def(KIND_SHIP).fittings;
        for (index, fitting) in fittings.iter().enumerate() {
            let id = 10 + index as u32;
            physics.add_car(id, KIND_SHIP_GUN, [0.0, 50.0, 0.0], 0.0);
            physics.mount_car(id, 1, fitting.at, fitting.yaw);
        }
        assert_eq!(physics.fittings_of(1), (10..16).collect::<Vec<_>>());
        assert_eq!(physics.mounted_on(12), Some(1));
        // Turning, the deck at a gun goes a little faster or slower than at the middle.
        let check = |physics: &Physics, slip: f32| {
            let ship = physics.car_pose(1).unwrap();
            for (index, fitting) in fittings.iter().enumerate() {
                let gun = physics.car_pose(10 + index as u32).unwrap();
                let want = ship.world_point(fitting.at);
                assert!(vec3::distance(gun.position, want) < 1e-3, "gun {index} at {:?}, not {want:?}", gun.position);
                // It points out over its side.
                let out = gun.world_dir([0.0, 0.0, -1.0]);
                let side = ship.world_dir([fitting.at[0].signum(), 0.0, 0.0]);
                assert!(vec3::dot(out, side) > 0.99, "gun {index} points out");
                assert!(vec3::distance(gun.velocity, ship.velocity) < slip, "gun {index} goes as fast as the deck");
            }
        };
        sail_for(&mut physics, FULL_SAIL, 15 * SECOND);
        check(&physics, 0.5);
        sail_for(&mut physics, CarControls { steer: -1.0, ..FULL_SAIL }, 4 * SECOND);
        check(&physics, 0.3 * 5.0);

        // Fired, a starboard gun kicks back inboard, which rolls the ship.
        let mut physics = afloat(0.0, 0.0, 0.0, true);
        physics.add_car(10, KIND_SHIP_GUN, [0.0, 50.0, 0.0], 0.0);
        physics.mount_car(10, 1, fittings[0].at, fittings[0].yaw);
        sail_for(&mut physics, CarControls::IDLE, 8 * SECOND);
        let back = physics.car_pose(10).unwrap().world_dir([0.0, 0.0, SHIP_GUN_RECOIL]);
        physics.kick_car(10, back);
        let mut rolled = 0.0f32;
        for _ in 0..SECOND {
            let pose = sail_for(&mut physics, CarControls::IDLE, 1);
            rolled = rolled.max(1.0 - pose.up_y);
        }
        // A degree or two.
        assert!(rolled > 2e-4, "the kick rolled it: {rolled}");
        assert_eq!(physics.car_pose(10).map(|pose| vec3::distance(pose.position, physics.car_pose(1).unwrap().world_point(fittings[0].at)) < 1e-3), Some(true));
    }
}
