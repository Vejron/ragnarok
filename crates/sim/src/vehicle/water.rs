//! What open water does to a wheeled vehicle: a light, quick one planes over it (`Spec::skim`), held
//! up, pushed and steered by the water while its wheels touch nothing; anything slower sinks slowly to
//! the bed and wades there, where the water holds it to a crawl. Not how water behaves, but fun: keep
//! the speed up and it skims, carve too hard, brake or let go and it drops in. The surface has waves
//! (`shared::waves`): a skimming hull rides them, pitching and rolling with the water under it, is
//! thrown off their crests at speed, and loses speed when it slams back down.

use super::*;
use crate::waters::Waters;

/// How far below the surface a skimming hull's underside rides, metres.
const SKIM_DRAFT: f32 = 0.05;
/// The spring and damper that hold a skimming hull at its draft, per second² and per second.
const SKIM_SPRING: f32 = 60.0;
const SKIM_DAMPING: f32 = 10.0;
/// The most the water lifts a skimming hull by in a second, m/s².
const SKIM_LIFT_MAX: f32 = 40.0;
/// Drag on a skimming hull, per metre: what holds its top speed on water a little under the land's.
const SKIM_DRAG: f32 = 0.004;
/// How fast a hard turn bleeds a skimming hull's speed, per second at full lock, times `Skim::bank_drag`.
const BANK_DRAG: f32 = 0.55;
/// Slowing on the surface, m/s²: with the throttle let go, and braking (against travel or the handbrake).
const SKIM_COAST: f32 = 3.0;
const SKIM_BRAKE: f32 = 8.0;
/// How quickly a skimming hull's yaw rate follows its skis, per second.
const SKIM_STEER: f32 = 6.0;
/// `Skim::grip` times this is the share of its sideways slide the water takes out each second.
const SKIM_GRIP: f32 = 5.0;
/// How hard a skimming hull is levelled, per second², and how far it leans into a full-lock turn, radians.
const SKIM_LEVEL: f32 = 30.0;
const SKIM_LEAN: f32 = 0.25;
/// How far a skimming hull sits tail down, radians: this much just above `Skim::sink_speed`, as it
/// struggles to stay up, easing to `SKIM_SQUAT_FAST` of it at top speed, where it runs flat.
const SKIM_SQUAT: f32 = 0.16;
const SKIM_SQUAT_FAST: f32 = 0.3;
/// A hull's roll and pitch rates die away at this rate while it skims, per second.
const SKIM_STEADY: f32 = 4.0;
/// Below the surface: what is left of gravity's pull as the hull fills, a share, and how quickly its
/// rise or fall dies away, per second, so it settles to the bed instead of dropping like a stone.
const BUOYANCY: f32 = 0.5;
const SINK_DAMPING: f32 = 3.0;
/// Wading: drag on a hull in the water, per second and per metre, times how much of it is under.
const WADE_LINEAR: f32 = 1.5;
const WADE_QUAD: f32 = 1.0;
/// The steepest a hull may lean, as the up axis's height, and still plane.
const SKIM_UPRIGHT: f32 = 0.5;
/// Coming down onto the water faster than this, m/s, slams: every m/s more takes `SLAM` m/s² off its
/// speed while it lasts, and more again the more its nose is down.
const SLAM_SPEED: f32 = 3.0;
const SLAM: f32 = 6.0;
const SLAM_NOSE: f32 = 4.0;

/// The water under a hull: the surface's height there (metres, averaged along the hull), which way is
/// up off it, how fast it is rising under the hull's middle (m/s) and its slope there `[dh/dx, dh/dz]`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Water {
    pub height: f32,
    pub normal: Vector,
    pub rise: f32,
    pub slope: [f32; 2],
}

/// The water under a vehicle of `spec` whose body is `body`, `time` seconds into the waves' loop, or
/// `None` where it is dry. The surface is felt at the hull's middle and at its four sides, so a hull
/// pitches and rolls with the waves.
pub(crate) fn water_under(waters: &Waters, time: f32, body: &RigidBody, spec: &Spec) -> Option<Water> {
    let at = body.translation();
    let middle = waters.surface(at.x, at.z, time)?;
    let [hx, _, hz] = spec.half_extents;
    let mut forward = *body.rotation() * Vector::new(0.0, 0.0, -1.0);
    forward.y = 0.0;
    let forward = forward.normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    // A side that sticks out over dry land feels the water at the middle.
    let height = |offset: Vector| waters.surface(at.x + offset.x, at.z + offset.z, time).map_or(middle.height, |surface| surface.height);
    let (front, back) = (height(forward * hz), height(-forward * hz));
    let along = (front - back) / (2.0 * hz);
    let across = (height(right * hx) - height(-right * hx)) / (2.0 * hx);
    let normal = (Vector::Y - forward * along - right * across).normalize();
    Some(Water { height: (front + back + 2.0 * middle.height) / 4.0, normal, rise: middle.rise, slope: middle.slope })
}

/// `0` at `low`, `1` at `high`, smoothly in between.
fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low).max(1e-3)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One tick of water on a wheeled vehicle whose body is `body`, over `water`: after the wheels have had their say, before the pipeline steps. `steer` is its front
/// wheels' angle (+ is right) and `boosted` whether a snowmobile's boost is open. Returns whether it is
/// skimming, which counts as being on the ground. Does nothing to a hull out of the water. Everything
/// is a function of the body's state and the controls, so the client can replay it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wade(body: &mut RigidBody, spec: &Spec, controls: CarControls, steer: f32, condition: CarCondition, boosted: bool, water: &Water, dt: f32) -> bool {
    let [_, hy, _] = spec.half_extents;
    let rotation = *body.rotation();
    let under = (water.height - (body.translation().y - hy)).clamp(0.0, 2.0 * hy);
    if under <= 0.0 {
        return false;
    }
    let wet = under / (2.0 * hy);
    let up = rotation * Vector::Y;
    let mut forward = rotation * Vector::new(0.0, 0.0, -1.0);
    forward.y = 0.0;
    let forward = forward.normalize_or_zero();
    let right = Vector::new(-forward.z, 0.0, forward.x);
    let mut velocity = body.linvel();
    let speed = velocity.dot(forward);
    let plane = match spec.skim {
        Some(skim) if up.y > SKIM_UPRIGHT => smoothstep(skim.sink_speed, skim.plane_speed, speed),
        _ => 0.0,
    };

    // How fast the surface comes up under the hull: the waves rising, and the hull running up their slope.
    let surfacing = water.rise + velocity.x * water.slope[0] + velocity.z * water.slope[1];
    let falling = surfacing - velocity.y;
    // Held up at the surface while it planes, and lifted with it up a wave's face, which throws it off
    // the crest; filling and settling slowly while it does not.
    if plane > 0.0 {
        let lift = (CAR_GRAVITY + SKIM_SPRING * (under - SKIM_DRAFT) + SKIM_DAMPING * falling).clamp(0.0, SKIM_LIFT_MAX);
        velocity.y += plane * lift * dt;
    }
    let sinking = 1.0 - plane;
    velocity.y += sinking * BUOYANCY * wet * CAR_GRAVITY * dt;
    velocity.y *= (-sinking * SINK_DAMPING * wet * dt).exp();

    let mut flat = Vector::new(velocity.x, 0.0, velocity.z);
    // Slamming down onto the water costs speed, nose first the more so.
    let nose = (rotation * Vector::new(0.0, 0.0, -1.0)).dot(water.normal);
    let mut slowing = SLAM * (falling - SLAM_SPEED).max(0.0) * (1.0 + SLAM_NOSE * (-nose).max(0.0));
    if let Some(skim) = spec.skim.filter(|_| plane > 0.0) {
        // The wheels touch nothing out here, so the water pushes it along, as the engine would.
        let throttle = controls.throttle.clamp(-1.0, 1.0);
        let against = throttle * speed < -1.0;
        if throttle > 0.0 && !against {
            let (power, top) = if boosted { (SNOWMOBILE_BOOST_POWER, SNOWMOBILE_BOOST_SPEED) } else { (1.0, 1.0) };
            let headroom = pull(speed, spec.max_speed * top * condition.speed);
            flat += forward * (throttle * power * spec.engine_force * skim.thrust * headroom * condition.power / body.mass() * plane * dt);
        }
        slowing += plane * if against || controls.handbrake { SKIM_BRAKE } else if throttle == 0.0 { SKIM_COAST } else { 0.0 };
        // Carving hard costs speed: lean on it too long and it drops in.
        let turned = turned(spec, steer, speed);
        slowing += plane * (SKIM_DRAG * speed * speed + skim.bank_drag * BANK_DRAG * turned * turned * speed.abs());
        // The water takes out some of its sideways slide.
        let side = flat.dot(right);
        flat -= right * side * (1.0 - (-skim.grip * SKIM_GRIP * plane * dt).exp());

        // It turns as its skis point, leans into the turn and sits tail down while it is held level.
        let wheelbase = spec.axles.first().zip(spec.axles.last()).map_or(1.0, |(front, back)| (back - front).abs().max(0.5));
        let mut spin = body.angvel();
        let yaw = -speed * steer.tan() / wheelbase;
        spin.y += (yaw - spin.y) * (1.0 - (-SKIM_STEER * plane * dt).exp());
        let steady = (-SKIM_STEADY * plane * dt).exp();
        let local = rotation.inverse() * spin;
        spin = rotation * Vector::new(local.x * steady, local.y, local.z * steady);
        let hurry = ((speed - skim.sink_speed) / (spec.max_speed - skim.sink_speed).max(1.0)).clamp(0.0, 1.0);
        let squat = SKIM_SQUAT * (1.0 - (1.0 - SKIM_SQUAT_FAST) * hurry);
        let lean = (water.normal + right * (SKIM_LEAN * turned * steer.signum()) - forward * squat).normalize();
        spin += up.cross(lean) * (SKIM_LEVEL * plane * dt);
        body.set_angvel(spin, true);
    }
    // Wading: the deeper it is in, the thicker the going.
    let going = flat.length();
    slowing += sinking * wet * (WADE_LINEAR * going + WADE_QUAD * going * going);
    if going > 0.0 {
        flat *= (going - slowing * dt).max(0.0) / going;
    }
    body.set_linvel(Vector::new(flat.x, velocity.y, flat.z), true);
    plane > 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::{KIND_JEEP, KIND_SNOWMOBILE, KIND_TANK};
    use shared::world::{NO_WATER, THEME_WINTER, WORLD_VERSION, WorldFile};

    /// Flat ground at 0 over 200 m in 2.5 m cells, with a lake 3 m deep from x = -40..40 and
    /// z = -20..-90, its surface level with the shore.
    fn pond() -> WorldFile {
        let side = 81u32;
        let lake = |i: u32, j: u32| {
            let (x, z) = (i as f32 * 2.5 - 100.0, j as f32 * 2.5 - 100.0);
            x.abs() <= 40.0 && (-90.0..=-20.0).contains(&z)
        };
        let mut heights = Vec::new();
        let mut water = Vec::new();
        for j in 0..side {
            for i in 0..side {
                heights.push(if lake(i, j) { -300 } else { 0 });
                water.push(if lake(i, j) { 0 } else { NO_WATER });
            }
        }
        WorldFile { version: WORLD_VERSION, half_extent: 20_000, ground: shared::terrain::Ground { origin: -10_000, cell: 250, side, heights }, water, ..Default::default() }
    }

    fn drive(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    const TICKS_PER_SECOND: u32 = (1.0 / TICK_DT + 0.5) as u32;
    const FORWARD: CarControls = CarControls { throttle: 1.0, ..CarControls::IDLE };

    /// The pond with its waves stilled.
    fn calm(world: &WorldFile) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        physics.calm_waters();
        physics
    }

    /// A vehicle of `kind` on the calm lake's surface at z = -30, heading -Z at `speed` m/s.
    fn afloat(kind: u8, speed: f32) -> Physics {
        let mut physics = calm(&pond());
        physics.add_car(1, kind, [0.0, spec(kind).half_extents[1] + 0.05, -30.0], 0.0);
        physics.set_car_velocity(1, [0.0, 0.0, -speed]);
        physics
    }

    /// The lowest a vehicle driven by `controls` for `seconds` gets, and where it ends up.
    fn lowest(physics: &mut Physics, controls: CarControls, seconds: f32) -> (f32, CarPose) {
        let mut low = f32::INFINITY;
        let mut pose = physics.car_pose(1).unwrap();
        for _ in 0..(seconds * TICKS_PER_SECOND as f32) as u32 {
            pose = drive(physics, controls, 1);
            low = low.min(pose.position[1]);
        }
        (low, pose)
    }

    #[test]
    fn a_snowmobile_at_full_throttle_skims_across_the_lake() {
        let mut physics = calm(&pond());
        physics.add_car(1, KIND_SNOWMOBILE, [0.0, 1.0, 40.0], 0.0);
        let mut low = f32::INFINITY;
        let mut crossed = false;
        for _ in 0..12 * TICKS_PER_SECOND {
            let pose = drive(&mut physics, FORWARD, 1);
            let z = pose.position[2];
            if (-85.0..-30.0).contains(&z) {
                low = low.min(pose.position[1]);
            }
            crossed |= z < -95.0;
        }
        assert!(crossed, "it reached the far shore");
        assert!(low > -0.4, "it stayed on the surface, dipping to {low}");
    }

    #[test]
    fn a_slow_snowmobile_sinks_and_crawls_on_the_bed() {
        let mut physics = afloat(KIND_SNOWMOBILE, 4.0);
        let (_, sunk) = lowest(&mut physics, FORWARD, 4.0);
        assert!(sunk.position[1] < -2.0, "it went down to the bed: {:?}", sunk.position);
        let (_, crawled) = lowest(&mut physics, FORWARD, 4.0);
        let went = sunk.position[2] - crawled.position[2];
        assert!(went > 2.0 && went < 20.0, "it crawls along the bottom, {went} m in 4 s");
    }

    #[test]
    fn a_fast_snowmobile_holds_the_surface_straight_but_carving_or_letting_go_sinks_it() {
        let (low, _) = lowest(&mut afloat(KIND_SNOWMOBILE, 16.0), FORWARD, 4.0);
        assert!(low > -0.4, "straight on, it skims: down to {low}");
        let carve = CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE };
        let (_, carved) = lowest(&mut afloat(KIND_SNOWMOBILE, 16.0), carve, 4.0);
        assert!(carved.position[1] < -1.5, "a full-lock carve drops it in: {:?}", carved.position);
        let (_, coasted) = lowest(&mut afloat(KIND_SNOWMOBILE, 16.0), CarControls::IDLE, 5.0);
        assert!(coasted.position[1] < -1.5, "letting go drops it in: {:?}", coasted.position);
        let brake = CarControls { throttle: -1.0, ..CarControls::IDLE };
        let (_, braked) = lowest(&mut afloat(KIND_SNOWMOBILE, 16.0), brake, 3.0);
        assert!(braked.position[1] < -1.5, "braking drops it in: {:?}", braked.position);
    }

    /// How far a vehicle's nose points up from level, radians.
    fn pitch(pose: &CarPose) -> f32 {
        pose.world_dir([0.0, 0.0, -1.0])[1].asin()
    }

    #[test]
    fn a_skimming_snowmobile_sits_tail_down_the_more_the_slower_it_goes() {
        let mut slow = afloat(KIND_SNOWMOBILE, 11.0);
        let slow = pitch(&drive(&mut slow, FORWARD, TICKS_PER_SECOND / 2));
        let mut fast = afloat(KIND_SNOWMOBILE, 20.0);
        let fast = pitch(&drive(&mut fast, FORWARD, TICKS_PER_SECOND / 2));
        assert!(fast > 0.01 && slow > fast + 0.03, "nose up {slow} slow, {fast} fast");
    }

    #[test]
    fn a_jeep_only_skims_flat_out() {
        let (low, _) = lowest(&mut afloat(KIND_JEEP, 28.0), FORWARD, 2.0);
        assert!(low > -0.5, "flat out, it skims: down to {low}");
        let (_, slow) = lowest(&mut afloat(KIND_JEEP, 12.0), FORWARD, 4.0);
        assert!(slow.position[1] < -1.0, "at a snowmobile's speed it sinks: {:?}", slow.position);
    }

    #[test]
    fn a_tank_wades_slowly_along_the_bed() {
        let on_bed = |world: &WorldFile| {
            let mut physics = calm(world);
            physics.add_car(1, KIND_TANK, [0.0, -1.5, -40.0], 0.0);
            let start = drive(&mut physics, CarControls::IDLE, TICKS_PER_SECOND).position[2];
            start - drive(&mut physics, FORWARD, 5 * TICKS_PER_SECOND).position[2]
        };
        let dry = WorldFile { water: Vec::new(), ..pond() };
        let (wet, dry) = (on_bed(&pond()), on_bed(&dry));
        assert!(wet > 2.0 && wet < dry * 0.5, "the water holds it back: {wet} m against {dry} m dry");
    }

    /// A snowmobile driven flat out with the boost from the shore down the lake, slanting over its swell: the
    /// most its nose pitched up or down, how many ticks its hull was clear of the water, how far it rose
    /// and fell (metres), and whether it got across.
    fn swell_crossing(start: u32) -> (f32, u32, f32, bool) {
        let mut physics = Physics::new(&pond()).unwrap();
        physics.set_tick(start);
        physics.add_car(1, KIND_SNOWMOBILE, [0.0, 1.0, 30.0], 0.0);
        let flat_out = CarControls { throttle: 1.0, boost: true, ..CarControls::IDLE };
        let (mut pitched, mut clear, mut low, mut high, mut crossed) = (0.0f32, 0, f32::INFINITY, f32::NEG_INFINITY, false);
        for _ in 0..10 * TICKS_PER_SECOND {
            let pose = drive(&mut physics, flat_out, 1);
            let [x, y, z] = pose.position;
            // Past where it came off the shore.
            if (-85.0..-40.0).contains(&z) {
                pitched = pitched.max(pitch(&pose).abs());
                (low, high) = (low.min(y), high.max(y));
                let surface = physics.waters.surface(x, z, shared::waves::time(physics.tick)).unwrap();
                if y - spec(KIND_SNOWMOBILE).half_extents[1] > surface.height + 0.3 {
                    clear += 1;
                }
            }
            crossed |= z < -95.0;
        }
        (pitched, clear, high - low, crossed)
    }

    #[test]
    fn across_the_swell_a_snowmobile_rides_up_and_down_it() {
        for start in [0, 900, 1800, 2700] {
            let (pitched, clear, travel, crossed) = swell_crossing(start);
            assert!(crossed, "it got across from tick {start}");
            assert!(travel > 0.05, "it rose and fell {travel} m with the swell from tick {start}");
            assert!(pitched > 0.02 && pitched < 0.4, "it pitched gently with it, {pitched} rad from tick {start}");
            assert!(clear < 5, "slanting over the crests throws nothing in the air: {clear} ticks clear from tick {start}");
        }
    }

    /// The same, but the way the swell runs (towards −x, across the lake's width), where its crests come
    /// at it head on: its most ticks in a row clear of the water, the most it pitched, and whether it got across.
    fn along_the_swell(start: u32) -> (u32, f32, bool) {
        let mut physics = Physics::new(&pond()).unwrap();
        physics.set_tick(start);
        physics.add_car(1, KIND_SNOWMOBILE, [60.0, 1.0, -55.0], std::f32::consts::FRAC_PI_2);
        let flat_out = CarControls { throttle: 1.0, boost: true, ..CarControls::IDLE };
        let (mut run, mut longest, mut pitched, mut x_end) = (0, 0, 0.0f32, 0.0);
        for _ in 0..8 * TICKS_PER_SECOND {
            let pose = drive(&mut physics, flat_out, 1);
            let [x, y, z] = pose.position;
            x_end = x;
            if (-35.0..35.0).contains(&x) {
                pitched = pitched.max(pitch(&pose).abs());
                let surface = physics.waters.surface(x, z, shared::waves::time(physics.tick)).unwrap();
                run = if y - spec(KIND_SNOWMOBILE).half_extents[1] > surface.height + 0.3 { run + 1 } else { 0 };
                longest = longest.max(run);
            }
        }
        (longest, pitched, x_end < -45.0)
    }

    #[test]
    fn head_on_into_the_swell_a_fast_snowmobile_leaps_its_crests() {
        let leaps: Vec<_> = (0..3840).step_by(480).map(|start| (start, along_the_swell(start))).collect();
        // A crest met just wrong can bury its nose and swamp it: a rough sea is a risk at full boost.
        let crossed = leaps.iter().filter(|(_, (_, _, crossed))| *crossed).count();
        assert!(crossed >= leaps.len() - 2, "it mostly gets across: {leaps:?}");
        let airborne = leaps.iter().filter(|(_, (longest, _, _))| *longest >= 10).count();
        assert!(airborne >= leaps.len() - 1, "the crests throw it clear, mostly: {leaps:?}");
    }

    #[test]
    fn the_waves_run_on_the_tick() {
        assert_ne!(swell_crossing(0), swell_crossing(600), "another tick, other waves");
        assert_eq!(swell_crossing(77), swell_crossing(77 + shared::waves::LOOP_TICKS), "a loop later, the same");
    }

    #[test]
    fn frozen_water_is_driven_through_as_if_it_were_not_there() {
        let cross = |world: &WorldFile| {
            let mut physics = Physics::new(world).unwrap();
            physics.add_car(1, KIND_SNOWMOBILE, [0.0, 1.0, 40.0], 0.0);
            drive(&mut physics, FORWARD, 8 * TICKS_PER_SECOND).position
        };
        let frozen = WorldFile { theme: THEME_WINTER, ..pond() };
        let dry = WorldFile { water: Vec::new(), ..pond() };
        assert_eq!(cross(&frozen), cross(&dry));
    }
}
