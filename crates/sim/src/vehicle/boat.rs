//! What carries a boat (`Lift::Boat`): a planing hull, on the Stridsbåt 90's two waterjets or the
//! raft's outboard, tuned by its `Boat`. It floats at its `draft` on open water, heaving and pitching
//! with the waves under it. Its drive pushes it ahead and, held back (the CB90's reversing buckets
//! down, the raft's outboard put astern), stops it, its bow digging in and its hull squatting into
//! the water as it does, and takes it astern; it swings it round even standing still, and under way
//! it turns as hard as `turn_rate`. As it speeds up its bow lifts and
//! it climbs out of the water onto the plane, and it banks into its turns as a planing hull does. Where
//! the water is too shallow to float it, it is aground: nothing drives or steers it, and it sits on the
//! bed, as it does run up a beach, unless it is a rubber boat that scrapes on over it (`Boat::scrape`).
//! A wreck fills and sinks.
//!
//! The river has its way with it as much as its hull lets it: the water turns it where its bow and
//! stern are in water running differently (`Boat::lever`), a flat floor slides out of its turns
//! (`Boat::keel`) and an idle one is taken up by the current (`Boat::grip`), an outboard turns it only as
//! hard as it pushes (`Boat::thrust_steer`), and off the crest of a wave whose back falls away faster
//! than it can, it flies and slams back down.
//!
//! Like `sail` it changes the body's velocities directly, from the body's state, the controls and the
//! tick alone, so the client predicts its own boat with the same.

use super::water::Water;
use shared::vehicles::Boat;
use super::*;

/// The share of the draft it floats at from which the jets bite and it is under way, and from which
/// it wholly is. Run up a beach, it sits lower than that on its keel.
const AFLOAT: [f32; 2] = [0.4, 0.75];
/// What is left of the water's hold on a wreck, which fills and goes down.
const WRECK_BUOYANCY: f32 = 0.3;
/// How far a wreck lists as it fills, radians.
const LIST: f32 = 0.4;
/// Astern the jets push with this share of their thrust.
const ASTERN_PUSH: f32 = 0.6;
/// The water's drag on the hull, per second.
const DRAG: f32 = 0.012;
/// From this speed through the water, m/s, it turns at its full rate under way rather than its rate
/// standing still.
const HELM_SPEED: f32 = 5.0;
/// The share of its turning an outboard gives it with no push at all: its leg in the water still
/// steers it a little as it moves.
const IDLE_STEER: f32 = 0.15;
/// Coming down onto the water faster than this, m/s, it slams: every m/s more takes this much off its
/// way through the water each second it lasts, m/s².
const SLAM_SPEED: f32 = 2.0;
const SLAM: f32 = 6.0;

/// `0` at `low`, `1` at `high`, smoothly in between.
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far onto the plane `hull` is at `speed` ahead, m/s: 0 off it, 1 wholly on it.
fn planing(hull: &Boat, speed: f32) -> f32 {
    smoothstep(hull.plane_speed[0], hull.plane_speed[1], speed)
}

/// How far its bow is lifted at `speed` ahead, radians: most as it climbs onto the plane, a little once
/// it is on it.
fn bow_rise(hull: &Boat, speed: f32) -> f32 {
    let climbing = smoothstep(1.0, hull.plane_speed[0], speed) * (1.0 - planing(hull, speed));
    hull.bow_rise * climbing + hull.bow_planing * planing(hull, speed)
}

/// One tick of a boat whose body is `body`, over `water` (`None` on dry land): before the pipeline steps.
pub(crate) fn boat(body: &mut RigidBody, spec: &Spec, controls: CarControls, condition: CarCondition, water: Option<&Water>, dt: f32) {
    // On dry land it is just a heavy box.
    let (Some(water), Lift::Boat(hull)) = (water, spec.lift) else { return };
    let [_, hy, _] = spec.half_extents;
    let rotation = *body.rotation();
    let under = (water.height - (body.translation().y - hy)).clamp(0.0, 2.0 * hy);
    if under <= 0.0 {
        // In the air off a crest: gravity alone has it.
        return;
    }
    let up = rotation * Vector::Y;
    let mut forward = rotation * Vector::new(0.0, 0.0, -1.0);
    forward.y = 0.0;
    let forward = forward.normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    // Everything the water does is to its way through the water: the current is taken off here and
    // given back at the end, so let go it drifts downstream. At anchor (the handbrake, as an empty
    // boat is held) it holds where it is.
    let drift = water.drift(controls.handbrake);
    let mut velocity = body.linvel() - drift;
    let speed = velocity.dot(forward);

    // It floats at its draft, less as it rises onto the plane, rising and falling with the water under
    // it; a wreck fills and goes down.
    // Held back while it runs ahead, the buckets come down: a crash stop, which digs it in.
    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let stopping = if speed > 0.5 { (-throttle).max(f32::from(controls.handbrake)).clamp(0.0, 1.0) * smoothstep(0.5, hull.plane_speed[1], speed) } else { 0.0 };
    let draft = hull.draft - hull.plane_lift * planing(hull, speed) + hull.brake_squat * stopping;
    // Too shallow to float, a rubber floor still scrapes on over the bed with some of its drive and grip.
    let afloat = if condition.wrecked { 0.0 } else { smoothstep(AFLOAT[0] * draft, AFLOAT[1] * draft, under).max(hull.scrape) };
    let buoyancy = if condition.wrecked { WRECK_BUOYANCY } else { 1.0 };
    let surfacing = if condition.wrecked { 0.0 } else { water.rise + (velocity.x + drift.x) * water.slope[0] + (velocity.z + drift.z) * water.slope[1] };
    // Coming down hard onto the water, it slams and loses way.
    let slam = (surfacing - velocity.y - SLAM_SPEED).max(0.0) * SLAM;
    velocity.y += buoyancy * CAR_GRAVITY * under / draft * dt;
    // It follows the surface up, but the water cannot pull it down: as the surface falls away its hold
    // goes with how deep the hull still sits in it, and off a crest falling faster than it, it flies.
    let follow = (surfacing - velocity.y) * (1.0 - (-hull.heave * dt).exp());
    velocity.y += if follow < 0.0 { follow * (under / draft).min(1.0) } else { follow };

    // The jets drive it ahead; held back while it runs ahead, the buckets come down and stop it, and
    // from a stop they take it astern. Let go, it drops off the plane and slows.
    let mut flat = Vector::new(velocity.x, 0.0, velocity.z);
    let mut slowing = DRAG * flat.length() + slam;
    let power = condition.power * afloat;
    if throttle > 0.0 {
        flat += forward * (throttle * hull.thrust * pull(speed, spec.max_speed * condition.speed) * power * dt);
    } else if throttle < 0.0 && speed > 0.5 {
        slowing += -throttle * hull.brake * afloat;
    } else if throttle < 0.0 {
        flat += forward * (throttle * hull.thrust * ASTERN_PUSH * pull(speed, hull.astern_speed * condition.speed) * power * dt);
    } else {
        slowing += hull.coast * afloat;
        flat *= (-hull.grip * afloat * dt).exp();
    }
    if controls.handbrake {
        slowing += hull.brake * afloat;
    }
    // The hull holds it to its heading, but for a slide in a hard turn.
    let side = flat.dot(right);
    flat -= right * side * (1.0 - (-hull.keel * afloat * dt).exp());
    let going = flat.length();
    if going > 0.0 {
        flat *= (going - slowing * dt).max(0.0) / going;
    }
    body.set_linvel(Vector::new(flat.x + drift.x, velocity.y, flat.z + drift.z), true);

    // The drive swings it round standing still, and turns it harder under way; an outboard only as hard
    // as it pushes. Let be, it holds it straight, but for the water turning it, where its bow and stern
    // are in water running differently.
    let mut spin = body.angvel();
    let flow = (speed / HELM_SPEED).clamp(0.0, 1.0);
    let rate = hull.spin_rate + (spec.turn_rate - hull.spin_rate) * flow;
    let push = (throttle.abs() * condition.power.min(1.0)).max(IDLE_STEER);
    let answer = 1.0 - hull.thrust_steer + hull.thrust_steer * push;
    let helm = -controls.steer.clamp(-1.0, 1.0) * rate * answer * condition.turn * condition.power.min(1.0);
    let yaw = helm + hull.lever * water.turn;
    spin.y += (yaw - spin.y) * (1.0 - (-hull.helm * afloat * dt).exp());
    // It rides on the water under it, its bow lifted as it climbs onto the plane, dug in as it crash
    // stops, and banked into its turn; a wreck lists over as it fills.
    let bank = (hull.bank * spin.y * speed).clamp(-hull.bank_max, hull.bank_max) * afloat;
    let trim = (bow_rise(hull, speed) - hull.brake_dip * stopping) * afloat;
    let list = if condition.wrecked { LIST } else { 0.0 };
    let level = (water.normal - right * (bank - list) - forward * trim).normalize();
    let steady = (-hull.rock * dt).exp();
    let local = rotation.inverse() * spin;
    spin = rotation * Vector::new(local.x * steady, local.y, local.z * steady);
    spin += up.cross(level) * (hull.level * (under / draft).min(1.0) * dt);
    body.set_angvel(spin, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::{KIND_CB90, KIND_RAFT};
    use shared::world::{NO_WATER, WORLD_VERSION, WorldFile};

    /// Flat ground at 0 over 1400 m in 5 m cells, with a lake 5 m deep everywhere within 600 m of the
    /// middle, its surface level with the shore.
    fn sea() -> WorldFile {
        lake(500)
    }

    /// `sea`, but `deep` centimetres deep.
    fn lake(deep: i32) -> WorldFile {
        let side = 281u32;
        let wet = |i: u32, j: u32| {
            let (x, z) = (i as f32 * 5.0 - 700.0, j as f32 * 5.0 - 700.0);
            x.abs() <= 600.0 && z.abs() <= 600.0
        };
        let mut heights = Vec::new();
        let mut water = Vec::new();
        for j in 0..side {
            for i in 0..side {
                heights.push(if wet(i, j) { -deep } else { 0 });
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
    fn let_go_on_a_river_it_drifts_downstream_and_at_anchor_it_holds() {
        // The lake running east at 2 m/s.
        let mut world = sea();
        let step = (2.0 / shared::world::FLOW_STEP) as i8;
        world.flow = world.water.iter().flat_map(|&w| if w == NO_WATER { [0, 0] } else { [step, 0] }).collect();
        let boat = |handbrake: bool| {
            let mut physics = Physics::new(&world).unwrap();
            physics.calm_waters();
            physics.add_car(1, KIND_CB90, [0.0, REST, 0.0], 0.0);
            run(&mut physics, CarControls { handbrake, ..CarControls::IDLE }, 20 * SECOND)
        };
        let drifting = boat(false);
        assert!(drifting.position[0] > 30.0 && drifting.position[0] < 40.0 && drifting.position[2].abs() < 1.0, "carried east: {:?}", drifting.position);
        let anchored = boat(true);
        assert!(anchored.position[0].hypot(anchored.position[2]) < 0.5, "it holds: {:?}", anchored.position);
    }

    #[test]
    fn its_jets_drive_it_through_the_water_so_upstream_it_makes_less_way() {
        let mut world = sea();
        let step = (3.0 / shared::world::FLOW_STEP) as i8;
        world.flow = world.water.iter().flat_map(|&w| if w == NO_WATER { [0, 0] } else { [0, step] }).collect();
        let mut physics = Physics::new(&world).unwrap();
        physics.calm_waters();
        // From the south shore heading north (-Z), against a current running south.
        physics.add_car(1, KIND_CB90, [0.0, REST, 560.0], 0.0);
        let pose = run(&mut physics, FULL_AHEAD, 35 * SECOND);
        let over_ground = -pose.velocity[2];
        assert!((over_ground - (CB90_MAX_SPEED - 3.0)).abs() < 1.5, "{over_ground} m/s over the ground");
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

    /// Where a raft floats at rest over the water's level.
    const RAFT_REST: f32 = RAFT_HALF_EXTENTS[1] - RAFT_DRAFT;

    /// A `kind` of boat afloat on `world` at its rest, at the middle, heading north.
    fn launched(world: &WorldFile, kind: u8, rest: f32) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        physics.calm_waters();
        physics.add_car(1, kind, [0.0, rest, 0.0], 0.0);
        physics
    }

    /// `world` with its water running at `flow(x, z)`, `[x, z]` m/s, at each wet sample.
    fn flowing(mut world: WorldFile, flow: impl Fn(f32, f32) -> [f32; 2]) -> WorldFile {
        let side = world.ground.side;
        let step = |v: f32| (v / shared::world::FLOW_STEP).round() as i8;
        world.flow = (0..side * side)
            .flat_map(|at| {
                let (x, z) = ((at % side) as f32 * 5.0 - 700.0, (at / side) as f32 * 5.0 - 700.0);
                if world.water[at as usize] == NO_WATER { [0, 0] } else { flow(x, z).map(step) }
            })
            .collect();
        world
    }

    #[test]
    fn a_raft_floats_a_hands_breadth_deep_and_its_outboard_has_it_on_the_plane_in_two_seconds() {
        let mut physics = launched(&sea(), KIND_RAFT, RAFT_REST);
        let pose = run(&mut physics, CarControls::IDLE, 5 * SECOND);
        assert!((pose.position[1] - RAFT_REST).abs() < 0.05 && pose.up_y > 0.999, "at {} not {RAFT_REST}", pose.position[1]);
        let pose = run(&mut physics, FULL_AHEAD, 2 * SECOND);
        assert!(pose.forward_speed > RAFT_PLANE_SPEED[1], "{} m/s after two seconds", pose.forward_speed);
        let pose = run(&mut physics, FULL_AHEAD, 8 * SECOND);
        assert!(pose.forward_speed > 0.95 * RAFT_MAX_SPEED && pose.forward_speed <= RAFT_MAX_SPEED + 0.1, "flat out at {} m/s", pose.forward_speed);
        assert!(pose.up_y > 0.99, "and it runs flat on the plane");
    }

    #[test]
    fn a_raft_runs_through_water_too_thin_to_float_the_combat_boat() {
        // 40 cm of water.
        let thin = lake(40);
        let mut raft = launched(&thin, KIND_RAFT, RAFT_REST);
        let raft = run(&mut raft, FULL_AHEAD, 5 * SECOND);
        assert!(raft.forward_speed > 10.0, "the raft runs at {} m/s", raft.forward_speed);
        let mut cb90 = launched(&thin, KIND_CB90, REST);
        let cb90 = run(&mut cb90, FULL_AHEAD, 5 * SECOND);
        assert!(cb90.forward_speed.abs() < 1.0, "the CB90 is aground: {} m/s", cb90.forward_speed);
    }

    #[test]
    fn a_raft_turns_inside_the_combat_boat() {
        // Flat out, then hard over for a quarter turn: the raft gets round in a third of the time.
        let quarter = |kind: u8, rest: f32| {
            let mut physics = launched(&sea(), kind, rest);
            run(&mut physics, FULL_AHEAD, 12 * SECOND);
            let mut ticks = 0;
            loop {
                let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 1);
                ticks += 1;
                // Heading north is -Z; a quarter turn to the right heads +X.
                if pose.world_dir([0.0, 0.0, -1.0])[0] > 0.99 || ticks > 20 * SECOND {
                    return ticks as f32 * TICK_DT;
                }
            }
        };
        let (raft, cb90) = (quarter(KIND_RAFT, RAFT_REST), quarter(KIND_CB90, REST));
        assert!(raft < 2.0 && raft < 0.5 * cb90, "a quarter turn takes the raft {raft} s and the CB90 {cb90} s");
    }

    #[test]
    fn let_go_on_an_eddy_line_a_raft_is_spun_round_and_the_combat_boat_barely() {
        // Up the river (-Z) at 3 m/s east of x = 0, and back down it at 1 m/s in the eddy to the west.
        let world = flowing(sea(), |x, _| [0.0, if x > 0.0 { -3.0 } else if x < 0.0 { 1.0 } else { -1.0 }]);
        let turned = |kind: u8, rest: f32| {
            let mut physics = launched(&world, kind, rest);
            let before = physics.car_pose(1).unwrap().yaw();
            let after = run(&mut physics, CarControls::IDLE, 3 * SECOND).yaw();
            after - before
        };
        // Its right side pushed ahead and its left held back, it turns left: a rising yaw.
        let (raft, cb90) = (turned(KIND_RAFT, RAFT_REST), turned(KIND_CB90, REST));
        assert!(raft > 0.6, "the raft came round {raft} rad in three seconds");
        assert!(cb90.abs() < 0.4 * raft, "the CB90 only {cb90} rad");
    }

    #[test]
    fn let_go_into_running_water_a_raft_is_taken_up_by_it_within_a_second() {
        // Both heading east, down a current running east at 3 m/s, from a standstill over the ground.
        let world = flowing(sea(), |_, _| [3.0, 0.0]);
        let carried = |kind: u8, rest: f32| {
            let mut physics = Physics::new(&world).unwrap();
            physics.calm_waters();
            physics.add_car(1, kind, [0.0, rest, 0.0], -std::f32::consts::FRAC_PI_2);
            run(&mut physics, CarControls::IDLE, SECOND).velocity[0]
        };
        let (raft, cb90) = (carried(KIND_RAFT, RAFT_REST), carried(KIND_CB90, REST));
        assert!(raft > 2.5, "the raft goes with it at {raft} m/s");
        assert!(cb90 < 1.5, "the CB90's way through the water holds it back to {cb90} m/s");
    }

    #[test]
    fn a_raft_coasting_answers_its_helm_lazily_and_under_power_whips_round_sliding() {
        let turn = |throttle: f32| {
            let mut physics = launched(&sea(), KIND_RAFT, RAFT_REST);
            run(&mut physics, FULL_AHEAD, 4 * SECOND);
            let before = physics.car_pose(1).unwrap().yaw();
            let mut slide = 0.0f32;
            for _ in 0..SECOND {
                let pose = run(&mut physics, CarControls { throttle, steer: 1.0, ..CarControls::IDLE }, 1);
                let right = pose.world_dir([1.0, 0.0, 0.0]);
                let v = pose.velocity;
                slide = slide.max((v[0] * right[0] + v[2] * right[2]).abs() / v[0].hypot(v[2]).max(0.1));
            }
            (before - physics.car_pose(1).unwrap().yaw(), slide)
        };
        let (coasting, _) = turn(0.0);
        let (powered, slide) = turn(1.0);
        assert!(powered > 1.3, "under power it came round {powered} rad in a second");
        assert!(coasting < 0.5 * powered, "coasting only {coasting} rad");
        assert!(slide > 0.25, "sliding out of the turn, its way {slide} of its speed sideways");
    }

    #[test]
    fn a_raft_is_thrown_off_a_crest_the_combat_boat_rides_over() {
        // Heading north (-Z), the surface rises 0.6 m over 15 m to a crest at z = -55 and falls back
        // over 5 m behind it.
        let mut world = sea();
        let side = world.ground.side;
        for (at, level) in world.water.iter_mut().enumerate() {
            let z = (at as u32 / side) as f32 * 5.0 - 700.0;
            if *level != NO_WATER {
                *level = match z as i32 {
                    -45 => 20,
                    -50 => 40,
                    -55 => 60,
                    _ => 0,
                };
            }
        }
        // How far its bottom gets clear of the water over the crest's back, metres, and how upright it
        // is after.
        let thrown = |kind: u8, rest: f32, hy: f32| {
            let mut physics = Physics::new(&world).unwrap();
            physics.calm_waters();
            physics.add_car(1, kind, [0.0, rest, 100.0], 0.0);
            let mut clear = 0.0f32;
            for _ in 0..20 * SECOND {
                let [x, y, z] = run(&mut physics, FULL_AHEAD, 1).position;
                if z < -55.0 && z > -75.0 {
                    clear = clear.max(y - hy - physics.water_surface(x, z).unwrap());
                }
            }
            (clear, physics.car_pose(1).unwrap().up_y)
        };
        let (raft, upright) = thrown(KIND_RAFT, RAFT_REST, RAFT_HALF_EXTENTS[1]);
        let (cb90, _) = thrown(KIND_CB90, REST, CB90_HALF_EXTENTS[1]);
        assert!(raft > 0.06, "the raft's bottom was {raft} m clear of the water");
        assert!(cb90 < 0.5 * raft, "the CB90's only {cb90} m");
        assert!(upright > 0.9, "and the raft landed upright");
    }

    #[test]
    fn a_raft_scrapes_over_a_shoal_the_combat_boat_runs_aground_on() {
        // 2 cm of water over a gravel bar across the lake from z = -30 to z = -60.
        let mut world = lake(60);
        let side = world.ground.side;
        for (at, ground) in world.ground.heights.iter_mut().enumerate() {
            let z = (at as u32 / side) as f32 * 5.0 - 700.0;
            if (-60.0..=-30.0).contains(&z) && world.water[at] != NO_WATER {
                *ground = -2;
            }
        }
        let crossed = |kind: u8, rest: f32| {
            let mut physics = launched(&world, kind, rest);
            run(&mut physics, FULL_AHEAD, 12 * SECOND);
            physics.car_pose(1).unwrap().position[2]
        };
        let raft = crossed(KIND_RAFT, RAFT_REST);
        assert!(raft < -80.0, "the raft got over it to z = {raft}");
        let cb90 = crossed(KIND_CB90, REST);
        assert!(cb90 > -45.0, "the CB90 ran aground on it at z = {cb90}");
    }

    #[test]
    fn a_raft_rocks_over_the_swell_and_sits_down_in_the_water_when_let_off() {
        // How far it tips at the most, radians, lying on the open swell for ten seconds.
        let rocked = |kind: u8, rest: f32| {
            let mut physics = Physics::new(&sea()).unwrap();
            physics.add_car(1, kind, [0.0, rest, 0.0], 0.3);
            let mut tilt = 0.0f32;
            for tick in 0..10 * SECOND {
                let pose = run(&mut physics, CarControls::IDLE, 1);
                if tick > 2 * SECOND {
                    tilt = tilt.max(pose.up_y.clamp(-1.0, 1.0).acos());
                }
            }
            tilt
        };
        let (raft, cb90) = (rocked(KIND_RAFT, RAFT_REST), rocked(KIND_CB90, REST));
        assert!(raft > 1.3 * cb90, "the raft tips {raft} rad, the CB90 {cb90}");
        // Flat out, then let go: how far it runs on.
        let mut physics = launched(&sea(), KIND_RAFT, RAFT_REST);
        let start = run(&mut physics, FULL_AHEAD, 6 * SECOND).position[2];
        let mut stopped = None;
        for tick in 0..10 * SECOND {
            if run(&mut physics, CarControls::IDLE, 1).forward_speed < 0.5 {
                stopped = Some((tick as f32 * TICK_DT, start - physics.car_pose(1).unwrap().position[2]));
                break;
            }
        }
        let (seconds, metres) = stopped.expect("it stopped");
        assert!(seconds < 2.0 && metres < 10.0, "let go flat out it ran on {metres} m over {seconds} s");
    }
}
