//! How a helicopter and a plane are flown: the hull is levelled and turned by setting its
//! angular velocity, the rotor and propeller are forces on the body, and wings lift it.

use super::limbs::Planted;
use super::water::smoothstep;
use super::*;

/// Share of full engine force at `speed` for a top speed of `max`: all of it up to [`FULL_POWER`]
/// of the way, then less and less until none at `max`.
pub(crate) fn pull(speed: f32, max: f32) -> f32 {
    ((max - speed.abs()) / (max * (1.0 - FULL_POWER))).clamp(0.0, 1.0)
}

/// A plane's wheels for one tick: no engine, the front pair steered towards where the pilot looks
/// (and by A/D), and braked while the stick is pulled back or nobody is aboard.
pub(crate) fn roll_plane(car: &mut Car, controls: CarControls, body: &RigidBody, speed: f32, mass: f32, dt: f32) {
    let _ = body;
    let wanted = if controls.powered { controls.steer.clamp(-1.0, 1.0) } else { 0.0 };
    let lock = car.spec.steer_lock / (1.0 + speed.abs() / car.spec.steer_fade);
    let target = wanted * lock;
    let returning = target.abs() < car.steer.abs() || target * car.steer < 0.0;
    let step = STEER_RATE * if returning { STEER_RETURN } else { 1.0 } * dt;
    car.steer += (target - car.steer).clamp(-step, step);
    let braking = !controls.powered || controls.lift < -0.5;
    let brake = if braking { 0.5 * BRAKE_PER_KG * mass } else { COAST_BRAKE_PER_KG * mass };
    let steer = car.steer;
    for (index, wheel) in car.controller.wheels_mut().iter_mut().enumerate() {
        wheel.steering = if index < 2 { -steer } else { 0.0 };
        wheel.engine_force = 0.0;
        wheel.brake = brake;
    }
}

/// Keeps a car planted and lets it be steered in the air: downforce while any wheel is down, and a
/// nudge of pitch (throttle) and roll (steering) while none is.
pub(crate) fn assist(body: &mut RigidBody, (throttle, steer): (f32, f32), grounded: bool, dt: f32) {
    let rotation = *body.rotation();
    if grounded {
        let push = DOWNFORCE_PER_KG * body.mass() * body.linvel().length_squared() * dt;
        body.apply_impulse(rotation * Vector::new(0.0, -push, 0.0), true);
    } else {
        // + about X lifts the nose and + about Z lifts the right side, so forward and right are negative.
        let nudge = rotation * Vector::new(-throttle * AIR_PITCH * dt, 0.0, -steer * AIR_ROLL * dt);
        body.set_angvel(body.angvel() + nudge, true);
    }
}

/// Slows a sled (`Runners`) as its runners slide: the snow's friction while it is down, against the
/// way it slides over the ground and in proportion to how squarely it presses on it, and the air's
/// drag, on the ground or off it. Friction never reverses the slide, so it holds a sled still on a
/// slope gentler than `mu` and lets go on a steeper one. It leaves alone the speed along the hull's
/// up axis, which on the ground is the springs' to take: a resting car's wheels keep a little of it.
pub(crate) fn on_runners(body: &mut RigidBody, runners: &Runners, (throttle, steer): (f32, f32), grounded: bool, dt: f32) {
    if !grounded {
        fly_runners(body, runners, throttle, steer, dt);
    }
    let mut velocity = body.linvel();
    let up = *body.rotation() * Vector::new(0.0, 1.0, 0.0);
    if grounded {
        let sliding = velocity - up * velocity.dot(up);
        let speed = sliding.length();
        if speed > 1e-4 {
            let friction = (runners.mu * GRAVITY_SAG * up.y.max(0.0) * dt).min(speed);
            velocity -= sliding * (friction / speed);
        }
    }
    let speed = velocity.length();
    if speed > 1e-4 {
        velocity *= 1.0 - (runners.drag * speed * dt).min(1.0);
    }
    body.set_linvel(velocity, true);
}

/// How quickly a sled's spin in the air comes round to what its rider and its line ask for, per second.
const RUNNERS_AIR_GRIP: f32 = 8.0;

/// A sled in the air: its nose swings onto the line it flies (`Runners::follow`), leaned forward or
/// back by the throttle (`lean`), and its roll comes back to level. The steering turns it (`air_turn`),
/// banking it into the turn. A share of its weight is held up (`float`), so it sails far off a bump.
fn fly_runners(body: &mut RigidBody, runners: &Runners, throttle: f32, steer: f32, dt: f32) {
    let rotation = *body.rotation();
    let velocity = body.linvel();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let right = rotation * Vector::new(1.0, 0.0, 0.0);
    let (throttle, steer) = (throttle.clamp(-1.0, 1.0), steer.clamp(-1.0, 1.0));
    // The line it flies, and how far its nose is above it; W leans the nose down, S lifts it.
    let line = if velocity.length() > 1.0 { (velocity.y / velocity.length()).clamp(-1.0, 1.0).asin() } else { nose.y.clamp(-1.0, 1.0).asin() };
    let pitch = nose.y.clamp(-1.0, 1.0).asin();
    let wanted_pitch = line - throttle * runners.lean;
    // Its right side's height, against level banked into the turn: steering right drops the right side.
    let roll = right.y.clamp(-1.0, 1.0).asin();
    let wanted_roll = -steer * runners.lean;
    // + about X lifts the nose and + about Z lifts the right side; a right turn is negative about Y.
    let local = Vector::new((wanted_pitch - pitch) * runners.follow, 0.0, (wanted_roll - roll) * runners.follow);
    let wanted = rotation * local + Vector::new(0.0, -steer * runners.air_turn, 0.0);
    let spin = body.angvel();
    body.set_angvel(spin + (wanted - spin) * (1.0 - (-RUNNERS_AIR_GRIP * dt).exp()), true);
    body.set_linvel(velocity + Vector::new(0.0, runners.float * GRAVITY_SAG * dt, 0.0), true);
}

/// One tick of a Roadster's cold-gas thrusters; returns the `THRUST_*` nozzles that fired.
///
/// Boost pushes along the nose, past what the tyres could. Hop, with a wheel down, adds a burst of
/// upward speed; a car already rising that fast does not hop again, so one press is one hop. Held
/// in the air, the lift nozzles keep the car level (leaning it with the stick) and, once the hop
/// has topped out, hold a slow climb that fades out below the ceiling. Both drain `tank`, which refills while the car sits on its wheels with both controls let go.
/// Everything is a function of the body's state, the tank and the controls, so the client can replay it.
pub(crate) fn thrust(body: &mut RigidBody, controls: CarControls, tank: &mut f32, grounded: bool, floor: f32, dt: f32) -> u8 {
    let rotation = *body.rotation();
    let mass = body.mass();
    let mut firing = 0;
    if controls.boost && *tank > 0.0 {
        body.add_force(rotation * Vector::new(0.0, 0.0, -ROADSTER_BOOST_ACCEL * mass), true);
        *tank -= ROADSTER_BOOST_DRAIN * dt;
        firing |= THRUST_BOOST;
    }
    let rising = body.linvel().y;
    if controls.hop && grounded && *tank >= ROADSTER_HOP_COST && rising < ROADSTER_HOP_SPEED * 0.5 {
        body.set_linvel(body.linvel() + Vector::new(0.0, ROADSTER_HOP_SPEED, 0.0), true);
        *tank -= ROADSTER_HOP_COST;
        firing |= THRUST_LIFT;
    } else if controls.hop && !grounded && *tank > 0.0 {
        // The lift nozzles trim the car level, leaning it a little with the stick like a helicopter,
        // and turn it with the steering.
        let up = rotation * Vector::Y;
        let nose = rotation * Vector::new(0.0, 0.0, -1.0);
        let flat = Vector::new(nose.x, 0.0, nose.z);
        let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
        let (throttle, steer) = (controls.throttle.clamp(-1.0, 1.0), controls.steer.clamp(-1.0, 1.0));
        let lean = (Vector::Y + heading * (throttle * ROADSTER_HOVER_TILT) + heading.cross(Vector::Y) * (steer * ROADSTER_HOVER_TILT)).normalize();
        let mut wanted = up.cross(lean) * ROADSTER_HOVER_LEVEL;
        wanted.y = -steer * ROADSTER_HOVER_YAW;
        let current = body.angvel();
        body.set_angvel(current + (wanted - current) * (ROADSTER_HOVER_RESPONSE * dt).min(1.0), true);
        // Once the hop tops out they hold a slow climb. They push along the hull's down axis, so a
        // leaning car drifts the way it leans.
        if rising <= ROADSTER_HOVER_CLIMB {
            let target = ROADSTER_HOVER_CLIMB * ((ROADSTER_CEILING - (body.translation().y - floor)) / 20.0).clamp(-1.0, 1.0);
            let extra = ((target - rising) * ROADSTER_HOVER_GAIN).clamp(-ROADSTER_HOVER_ACCEL, ROADSTER_HOVER_ACCEL);
            body.add_force(up * ((CAR_GRAVITY + extra) / up.y.max(0.5) * mass), true);
        }
        *tank -= ROADSTER_HOVER_DRAIN * dt;
        firing |= THRUST_LIFT;
    }
    // Holding either control keeps the valves open, so an empty tank does not hop again by itself.
    if grounded && !controls.boost && !controls.hop {
        *tank += ROADSTER_AIR_REFILL * dt;
    }
    *tank = tank.clamp(0.0, ROADSTER_AIR);
    firing
}

/// One tick of a snowmobile's boost (its push is in `drive_car`): held with the throttle open it runs
/// the boost down, let go it fills again. Returns `THRUST_BOOST` while it is on.
pub(crate) fn boost(controls: CarControls, tank: &mut f32, dt: f32) -> u8 {
    let on = controls.boost && controls.throttle > 0.0 && *tank > 0.0;
    // Holding it with an empty boost keeps it from filling, so it does not stutter on and off.
    let rate = if on { -SNOWMOBILE_BOOST_DRAIN } else if controls.boost { 0.0 } else { SNOWMOBILE_BOOST_REFILL };
    *tank = (*tank + rate * dt).clamp(0.0, SNOWMOBILE_BOOST);
    if on { THRUST_BOOST } else { 0 }
}

/// One tick of a hen's wings, after [`walk`] has worked its legs; returns its `THRUST_*` flags.
///
/// With a foot down, jump leaps: a burst of upward speed and a little along the heading, once per
/// press (a hen already rising that fast does not leap again). With no foot down, the wings keep the
/// hull level, which the legs' balance cannot, leaning it with the stick and turning it with the
/// steering, so every leap lands on its feet. Jump held in the air flaps: once the leap has topped out
/// it holds a slow climb, and carries the hen forward with the stick.
/// With no stamina left the wings spread instead and hold its fall to a slow glide. Leaping and
/// flapping drain `stamina`, which comes back with a foot down and jump let go. A fallen or wrecked hen
/// does none of it. Everything is a function of the body's state, the stamina and the controls, so the
/// client can replay it.
pub(crate) fn flap(body: &mut RigidBody, controls: CarControls, stamina: &mut f32, standing: bool, grounded: bool, dt: f32) -> u8 {
    let mut flags = if grounded { 0 } else { THRUST_AIR };
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    if !standing || !controls.powered || up.y < WALKER_FALLEN_UP {
        return flags;
    }
    let mass = body.mass();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let flat = Vector::new(nose.x, 0.0, nose.z);
    let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
    let (throttle, steer) = (controls.throttle.clamp(-1.0, 1.0), controls.steer.clamp(-1.0, 1.0));
    let rising = body.linvel().y;
    if grounded {
        if controls.hop && *stamina >= HEN_JUMP_COST && rising < HEN_JUMP_SPEED * 0.5 {
            let forward = HEN_JUMP_FORWARD * if throttle >= 0.0 { throttle } else { throttle * WALKER_REVERSE_SHARE };
            body.set_linvel(body.linvel() + Vector::new(0.0, HEN_JUMP_SPEED, 0.0) + heading * forward, true);
            *stamina -= HEN_JUMP_COST;
            flags |= THRUST_LIFT;
        } else if !controls.hop {
            *stamina += HEN_STAMINA_REFILL * dt;
        }
        *stamina = stamina.clamp(0.0, HEN_STAMINA);
        return flags;
    }

    // The wings trim it level, leaning it a little with the stick, and turn it with the steering.
    let lean = (Vector::Y + heading * (throttle * HEN_AIR_TILT) + heading.cross(Vector::Y) * (steer * HEN_AIR_TILT)).normalize();
    let mut wanted = up.cross(lean) * HEN_AIR_LEVEL;
    wanted.y = -steer * HEN_AIR_YAW;
    let current = body.angvel();
    body.set_angvel(current + (wanted - current) * (HEN_AIR_RESPONSE * dt).min(1.0), true);
    if !controls.hop {
        return flags;
    }

    // Held, the wings carry it forward with the stick.
    let along = body.linvel().dot(heading);
    let ahead = (throttle.max(0.0) * HEN_GLIDE_SPEED - along).clamp(-HEN_GLIDE_ACCEL, HEN_GLIDE_ACCEL);
    let mut push = if throttle > 0.0 { heading * ahead } else { Vector::ZERO };
    if *stamina > 0.0 {
        // Flapping, once the leap tops out: a slow climb, as high as the stamina lasts (it does not come
        // back in the air). Until then the wings beat for free.
        if rising <= HEN_FLAP_CLIMB {
            push.y = CAR_GRAVITY + ((HEN_FLAP_CLIMB - rising) * HEN_FLAP_GAIN).clamp(-HEN_FLAP_ACCEL, HEN_FLAP_ACCEL);
            *stamina = (*stamina - HEN_FLAP_DRAIN * dt).max(0.0);
        }
        flags |= THRUST_LIFT;
    } else {
        // Spread and still, the wings hold its fall to a glide, and do nothing while it falls slower.
        push.y = (CAR_GRAVITY + (-HEN_GLIDE_SINK - rising) * HEN_GLIDE_GAIN).clamp(0.0, CAR_GRAVITY + HEN_FLAP_ACCEL);
        flags |= THRUST_GLIDE;
    }
    // A force rather than an impulse: it is integrated together with gravity, so the climb is exact.
    body.add_force(push * mass, true);
    flags
}

/// One tick of the thrusters in a legged kind's feet (`Gait::jets`), after [`walk`] has worked its
/// legs; returns its `THRUST_*` flags.
///
/// On the ground, jump leaps, once per press. In the air the thrusters keep the hull level, which the
/// legs' balance cannot, lean it with the stick and turn it with the steering. Jump held in the air
/// fires them: they hold a climb that fades out below the ceiling over `floor` (so held on the way
/// down they break the fall), and carry it along its heading with the stick. Leaping and thrust drain
/// `fuel`, which comes back on the ground with jump let go; with none left it falls. A fallen or
/// wrecked one does none of it. Everything is a function of the body's state, the fuel and the
/// controls, so the client can replay it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn jet(body: &mut RigidBody, gait: &Gait, jets: &shared::vehicles::Jets, controls: CarControls, fuel: &mut f32, full: f32, standing: bool, grounded: bool, floor: f32, dt: f32) -> u8 {
    let mut flags = if grounded { 0 } else { THRUST_AIR };
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    if !standing || !controls.powered || up.y < gait.fallen_up {
        return flags;
    }
    let mass = body.mass();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let heading = Vector::new(nose.x, 0.0, nose.z).normalize_or(Vector::new(0.0, 0.0, -1.0));
    let (throttle, steer) = (controls.throttle.clamp(-1.0, 1.0), controls.steer.clamp(-1.0, 1.0));
    let rising = body.linvel().y;
    if grounded {
        if controls.hop && *fuel >= jets.leap_cost && rising < jets.leap * 0.5 {
            let forward = jets.leap_forward * if throttle >= 0.0 { throttle } else { throttle * gait.reverse_share };
            body.set_linvel(body.linvel() + Vector::new(0.0, jets.leap, 0.0) + heading * forward, true);
            *fuel -= jets.leap_cost;
            flags |= THRUST_LIFT;
        } else if !controls.hop {
            *fuel += jets.refill * dt;
        }
        *fuel = fuel.clamp(0.0, full);
        return flags;
    }

    // The thrusters trim it level, leaning it a little with the stick, and turn it with the steering.
    let lean = (Vector::Y + heading * (throttle * jets.tilt)).normalize();
    let mut wanted = up.cross(lean) * jets.level;
    wanted.y = -steer * jets.yaw;
    let current = body.angvel();
    body.set_angvel(current + (wanted - current) * (jets.response * dt).min(1.0), true);
    if !controls.hop || *fuel <= 0.0 {
        return flags;
    }
    // Held: a climb that fades out below the ceiling, and a push along the heading with the stick.
    let top = if throttle >= 0.0 { jets.speed } else { jets.speed * gait.reverse_share };
    let ahead = (throttle * top - body.linvel().dot(heading)).clamp(-jets.push, jets.push);
    let mut push = if throttle != 0.0 { heading * ahead } else { Vector::ZERO };
    let climb = jets.climb * ((jets.ceiling - (body.translation().y - floor)) / 20.0).clamp(-1.0, 1.0);
    push.y = CAR_GRAVITY + ((climb - rising) * jets.gain).clamp(-jets.accel, jets.accel);
    *fuel = (*fuel - jets.drain * dt).max(0.0);
    // A force rather than an impulse: it is integrated together with gravity, so the climb is exact.
    body.add_force(push * mass, true);
    flags | THRUST_LIFT
}

/// One tick of a rotorcraft's flight; returns its `THRUST_*` flags. `rpm` is the rotor's speed, a share
/// of what its governor holds, which this keeps; `power` is the share of its engine's power left
/// (`CarCondition::power`, 0 with the engine shot dead); `wind` is the air's own velocity, m/s.
///
/// The rotor's lift goes with the collective lever, the square of its speed and the air it meets: it
/// loses lift climbing through its own disc and gains it descending or flaring, gains it near the ground
/// (ground effect) and once it flies through clean air (translational lift), loses it sinking into its
/// own downwash (vortex ring state) and past the speed its retreating blade stalls at. The thin air under
/// the ceiling takes the engine's power away. It pushes along the hull's up axis, and the air drags on the hull along each of its
/// axes. All of it is measured through the air, which the wind moves.
///
/// The rotor's speed is an energy balance. It takes power to hold up the helicopter, more to climb or to
/// push through the air fast, and less at cruise than in a hover; the engine gives what it takes as far as
/// its power goes, and spins it up from stopped. Asked for more, the rotor slows and its lift goes. With
/// no engine, air coming up through the disc in a descent keeps the rotor turning (autorotation), and
/// pulling the lever then spends its speed on a flare.
///
/// The flight controls fly the hull by torques, as far as the rotor has authority, which grows with its
/// speed and its lift. The stick turns the hull (W/S pitch, A/D roll), and let go the stability
/// augmentation only stops it turning: it stays at the lean it was left at. The pedals turn the nose
/// towards where the pilot looks, the tail rotor also holding the main rotor's torque, and above a walking
/// pace a turn asked for is banked into. The rotor turns the hull by itself as well: the tail rotor's push
/// drifts it sideways, flapback lifts the nose with speed, transverse flow rolls it through translational
/// lift and a stalling retreating blade rolls and pitches it up. Everything is a function of the body's
/// state, the rotor's speed, the wind and the controls alone, so the client can replay it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fly(body: &mut RigidBody, rotor: &Rotorcraft, half_extents: [f32; 3], controls: CarControls, power: f32, rpm: &mut f32, wind: Vector, floor: f32, dt: f32) -> u8 {
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let right = rotation * Vector::X;
    let velocity = body.linvel();
    let mass = body.mass();
    let piloted = controls.powered;
    let spin = rpm.clamp(0.0, 1.0);
    let height = body.translation().y - floor;
    // How it moves through the air.
    let air = velocity - wind;
    let climb = air.y;
    let airspeed = Vector::new(air.x, 0.0, air.z).length();
    let local = rotation.inverse() * air;
    let ahead_air = (-local.z).max(0.0);
    // The air through the disc, + going up along it: the vertical speed, and a share of what the lean adds.
    let through = climb + (air.dot(up) - climb) * rotor.tilt_flow;

    // The rotor's lift, m/s². Air going down through the disc (a climb, or a nose-down dash) flattens its
    // angle onto the blades, and air coming up through it (a descent, or a flare) steepens it, the more
    // so the slower they turn.
    let pitch = controls.lift.clamp(0.0, 1.0) - through / (rotor.pitch_flow * spin.max(0.2));
    let translational = 1.0 + rotor.etl_gain * smoothstep(0.3 * rotor.etl_speed, 1.2 * rotor.etl_speed, airspeed);
    let ground = 1.0 + rotor.ground_effect * (1.0 - (height + rotor.hub).max(0.0) / (2.0 * rotor.radius)).max(0.0);
    // Sinking into its own downwash: worst at a descent of about `hover_inflow`, gone well past it, where
    // the air comes up through the disc cleanly, and flown out of with forward speed.
    let sink = -climb / rotor.hover_inflow;
    let ring = smoothstep(0.4, 0.8, sink) * (1.0 - smoothstep(1.4, 2.0, sink)) * (1.0 - smoothstep(0.3, 0.8, airspeed / rotor.hover_inflow));
    let vortex = 1.0 - rotor.vortex * ring;
    // Past the speed its retreating blade stalls at, the rotor loses lift and turns the hull.
    let stalled = (ahead_air - rotor.stall_speed).max(0.0);
    let stalling = 1.0 - (stalled / rotor.stall_speed).min(0.5);
    let lift = rotor.thrust * pitch * spin * spin * translational * ground * vortex * stalling;

    // Drag along each of the hull's axes, through the air.
    let [side, vertical, fore] = rotor.drag;
    let drag = rotation * Vector::new(-side * local.x * local.x.abs(), -vertical * local.y * local.y.abs(), -fore * local.z * local.z.abs());

    // The power it takes, in shares of the hover's induced power: driving the air down through the disc,
    // less the faster it flies across it and more as it climbs; pushing the hull through the air; and the
    // blades' own drag. Air coming up through the disc faster than the rotor drives it down gives power
    // back, which is what turns the rotor in autorotation.
    let load = (lift / CAR_GRAVITY).abs();
    let hover_flow = rotor.hover_inflow * load.sqrt();
    let across = if hover_flow > 1e-3 { hover_flow * hover_flow / (airspeed * airspeed + hover_flow * hover_flow).sqrt() } else { 0.0 };
    let induced = if through > 0.0 { ((0.5 * through).powi(2) + across * across).sqrt() - 0.5 * through } else { across };
    let parasite = (-drag.dot(air) / (CAR_GRAVITY * rotor.hover_inflow)).max(0.0);
    let need = load * (induced + through) / rotor.hover_inflow + parasite + rotor.profile * spin * spin * spin;
    // The engine's governor gives what the rotor takes and spins it up towards its governed speed, as far
    // as the air lets it: under the ceiling it fades to what a hover takes, and less above it.
    let hovering = 1.0 + rotor.profile;
    let thin = ((rotor.ceiling - height) / Rotorcraft::CEILING_FADE).clamp(-1.0, 1.0);
    let most = if thin >= 0.0 { hovering + (rotor.power - hovering) * thin } else { hovering * (1.0 + 0.6 * thin) };
    let available = if piloted { most * power.clamp(0.0, 1.0) } else { 0.0 };
    let energy = 2.0 * rotor.inertia * spin.max(0.2);
    let rise = ((1.0 - spin) * 4.0).min(rotor.spool);
    let given = (need + energy * rise).clamp(0.0, available);
    let braked = if piloted { 0.0 } else { rotor.brake };
    *rpm = (spin + ((given - need) / energy - braked) * dt).clamp(0.0, 1.0);

    // The controls, as angular accelerations about the hull's own axes: + about X lifts the nose, + about
    // Y turns left and + about Z lifts the right side.
    let flat = Vector::new(nose.x, 0.0, nose.z);
    let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
    let facing = (-heading.x).atan2(-heading.z);
    let spin_local = rotation.inverse() * body.angvel();
    let roll_now = right.y.clamp(-1.0, 1.0).asin();
    // Hovering, the pedals turn the nose towards the look; with nobody at them the tail holds it still.
    let looked = controls.aim.map(|[yaw, _]| yaw);
    let hover_turn = looked.map_or(0.0, |yaw| (shared::quant::wrap_angle(yaw - facing) * rotor.yaw_gain).clamp(-rotor.yaw_rate, rotor.yaw_rate));
    // Flying ahead through the air, it banks to bring its flight path round to the look, and the tail
    // keeps the nose on that path: the turn its bank makes, and a swing back for any slip.
    let cruising = smoothstep(rotor.coordinate_speed, 2.0 * rotor.coordinate_speed, ahead_air);
    let track = (-air.x).atan2(-air.z);
    let path_turn = looked.map_or(0.0, |yaw| shared::quant::wrap_angle(yaw - track) * rotor.yaw_gain);
    let bank = (ahead_air.max(1.0) * path_turn / CAR_GRAVITY).atan().clamp(-rotor.bank_max, rotor.bank_max);
    let slip = shared::quant::wrap_angle(facing - track);
    let banked_turn = CAR_GRAVITY * roll_now.tan() / ahead_air.max(1.0) - slip * rotor.weathercock;
    let turn = hover_turn + (banked_turn.clamp(-rotor.yaw_rate, rotor.yaw_rate) - hover_turn) * cruising;
    // The stick asks for a pitch and roll rate, and let go for none. At speed the bank a turn takes is
    // flown on top of the stick.
    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let steer = controls.steer.clamp(-1.0, 1.0);
    let pitch_wanted = -throttle * rotor.pitch_rate;
    let roll_wanted = -steer * rotor.roll_rate + ((bank - roll_now) * rotor.bank_gain).clamp(-rotor.roll_rate, rotor.roll_rate) * cruising;
    let authority = rotor.cyclic_accel * spin * spin * (0.3 + 0.7 * load.min(1.0));
    // Flown, the rate follows the stick; let go, the augmentation only damps it.
    let gain = |stick: f32| if stick.abs() > 0.05 { rotor.rate_gain } else { rotor.hold_gain };
    let pitching = ((pitch_wanted - spin_local.x) * gain(throttle)).clamp(-authority, authority);
    let rolling = ((roll_wanted - spin_local.z) * gain(if cruising > 0.0 { 1.0 } else { steer })).clamp(-authority, authority);
    // The main rotor's drag turns the hull against it (clockwise seen from above), and the tail rotor
    // cancels that out of what it can give.
    let reaction = -rotor.torque_yaw * given;
    let tail_most = rotor.yaw_accel * spin * spin;
    let tail = ((turn - spin_local.y) * rotor.rate_gain - reaction).clamp(-tail_most, tail_most);
    let yawing = tail + reaction;
    // What the rotor does to the hull by itself, scaled by how hard it works: flapback lifts the nose with
    // speed, transverse flow rolls it right through translational lift, and a stalling retreating blade
    // rolls it left and lifts the nose.
    let working = spin * spin * load.min(1.5);
    let flapping = rotor.flapback * ahead_air * working;
    let transverse = -rotor.transverse * clean_band(airspeed, rotor.etl_speed) * working;
    let stall_turn = rotor.blade_stall * stalled * working;
    // As a torque on the hull box's own inertia, so that a knock from outside is fought, not undone.
    let [hx, hy, hz] = half_extents;
    let inertia = Vector::new(hy * hy + hz * hz, hx * hx + hz * hz, hx * hx + hy * hy) * (mass / 3.0);
    let turning = Vector::new(pitching + flapping + stall_turn, yawing, rolling + transverse + stall_turn);
    body.add_torque(rotation * (turning * inertia), true);
    // The tail rotor's push to hold its yaw also pushes the helicopter sideways: + about Y swings the
    // tail to the right, which it does by pushing to the right.
    let drift = right * (tail * inertia.y / mass / rotor.tail_arm);
    // A force rather than an impulse: it is integrated together with gravity.
    body.add_force((up * lift + drag + drift) * mass, true);

    let airborne = height > half_extents[1] + 0.3;
    if piloted && airborne && *rpm < rotor.low_rotor { THRUST_LOW_ROTOR } else { 0 }
}

/// How far through the band of translational lift around `etl_speed` an `airspeed` is: 0 outside it,
/// 1 in the middle, where the transverse flow rolls hardest and the airframe shudders.
pub(crate) fn clean_band(airspeed: f32, etl_speed: f32) -> f32 {
    smoothstep(0.3 * etl_speed, 0.6 * etl_speed, airspeed) * (1.0 - smoothstep(0.8 * etl_speed, 1.3 * etl_speed, airspeed))
}

/// One tick of an FPV drone with its motors running. It is flown by velocity: the stick and the look
/// ask for one (see `DRONE_SPEED`), and the rotors push towards it as hard as `DRONE_MAX_ACCEL` lets them,
/// on top of holding the drone up. The hull leans the way they push and turns towards where its pilot
/// looks. Everything is a function of the body's state and the controls alone, so the client can replay it.
pub(crate) fn fly_drone(body: &mut RigidBody, controls: CarControls, floor: f32, dt: f32) {
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let flat = Vector::new(nose.x, 0.0, nose.z);
    let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
    let right = heading.cross(Vector::Y);
    let facing = (-heading.x).atan2(-heading.z);

    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let steer = controls.steer.clamp(-1.0, 1.0);
    let lift = controls.lift.clamp(-1.0, 1.0);
    let [aim_yaw, aim_pitch] = controls.aim.unwrap_or([facing, 0.0]);
    // Forward follows the look up and down; backing off stays level.
    let (rise, level) = if throttle > 0.0 { aim_pitch.sin_cos() } else { (0.0, 1.0) };
    let mut wanted = heading * (throttle * level * DRONE_SPEED) + right * (steer * DRONE_STRAFE_SPEED);
    wanted.y = throttle.max(0.0) * rise * DRONE_SPEED + if lift > 0.0 { lift * DRONE_CLIMB_SPEED } else { lift * DRONE_SINK_SPEED };
    let headroom = ((DRONE_CEILING - (body.translation().y - floor)) / 20.0).clamp(-1.0, 1.0);
    if wanted.y > 0.0 {
        wanted.y *= headroom.max(0.0);
    }
    if headroom < 0.0 {
        wanted.y = wanted.y.min(headroom * DRONE_CLIMB_SPEED);
    }
    let mut push = (wanted - body.linvel()) * DRONE_ACCEL_GAIN;
    let size = push.length();
    if size > DRONE_MAX_ACCEL {
        push *= DRONE_MAX_ACCEL / size;
    }
    // A force rather than an impulse: it is integrated together with gravity, so hovering is exact.
    body.add_force((push + Vector::Y * CAR_GRAVITY) * body.mass(), true);

    // The rotors tilt along what they push, never more than about 60 degrees over.
    let lean = Vector::new(push.x, (push.y + CAR_GRAVITY).max(CAR_GRAVITY * 0.5), push.z).normalize();
    let mut turn = up.cross(lean) * DRONE_LEVEL_GAIN;
    // + about Y turns left, as + yaw does.
    turn.y = (shared::quant::wrap_angle(aim_yaw - facing) * DRONE_YAW_GAIN).clamp(-DRONE_YAW_RATE, DRONE_YAW_RATE);
    let current = body.angvel();
    body.set_angvel(current + (turn - current) * (DRONE_RESPONSE * dt).min(1.0), true);
}

/// How a kind that always flies forward along its nose is flown: a loitering drone, or a guided missile.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Glide {
    /// What it always flies at, along its nose, m/s.
    pub speed: f32,
    /// How quickly the gap to that velocity closes, per second, and the hardest it is pushed, m/s².
    pub grip: f32,
    pub max_accel: f32,
    /// The hardest its motor speeds it up along its nose, m/s².
    pub speed_up: f32,
    /// The speed from which its fins turn it at their full rates, m/s; slower, they bite with the square
    /// of its share of it. 0 for one that turns as well at any speed.
    pub fin_speed: f32,
    /// It steers by pointing its nose with pitch and yaw and keeps its wings level (`skid`), as a
    /// missile does, rather than banking into turns as a plane does (`autopilot`).
    pub skid: bool,
    /// The most it turns about its own axes, rad/s, and how quickly its angular velocity follows, per second.
    pub pitch_rate: f32,
    pub yaw_rate: f32,
    pub roll_rate: f32,
    pub response: f32,
    /// The most it banks into a turn, radians, and how steeply its flier may ask it to climb or dive.
    pub max_bank: f32,
    pub max_pitch: f32,
    /// Climbing fades out over the last 20 m below this height over the ground, metres.
    pub ceiling: f32,
}

/// A loitering drone's.
pub(crate) const LOITER_GLIDE: Glide = Glide {
    speed: LOITER_SPEED,
    grip: LOITER_GRIP,
    max_accel: LOITER_MAX_ACCEL,
    speed_up: LOITER_MAX_ACCEL,
    fin_speed: 0.0,
    skid: false,
    pitch_rate: LOITER_PITCH_RATE,
    yaw_rate: LOITER_YAW_RATE,
    roll_rate: LOITER_ROLL_RATE,
    response: LOITER_RESPONSE,
    max_bank: LOITER_MAX_BANK,
    max_pitch: PLANE_AIM_MAX_PITCH,
    ceiling: LOITER_CEILING,
};

/// A guided missile's: faster and nimbler, and it may dive steeply onto what it is flown at.
pub(crate) const MISSILE_GLIDE: Glide = Glide {
    speed: MISSILE_SPEED,
    grip: MISSILE_GRIP,
    max_accel: MISSILE_MAX_ACCEL,
    speed_up: MISSILE_SPEED_UP,
    fin_speed: MISSILE_FIN_SPEED,
    skid: true,
    pitch_rate: MISSILE_PITCH_RATE,
    yaw_rate: MISSILE_YAW_RATE,
    roll_rate: MISSILE_ROLL_RATE,
    response: MISSILE_RESPONSE,
    max_bank: MISSILE_MAX_BANK,
    max_pitch: MISSILE_MAX_PITCH,
    ceiling: MISSILE_CEILING,
};

/// One tick of a loitering drone or a guided missile with its motor running (`glide`): a simplified,
/// nimble plane. It always flies at `glide.speed` along its nose: the wings and motor push its velocity there as hard as
/// `glide.max_accel` lets them, on top of holding it up, so there is no stall, no throttle and no
/// slide. The nose is steered as the plane's is: the mouse sets where to go and the autopilot banks into
/// the turn and pulls, levelling its wings as it arrives; W/S pitch and A/D roll by hand while held,
/// and with neither it flies level. Everything is a function of the body's state and the controls
/// alone, so the client can replay it.
pub(crate) fn glide(body: &mut RigidBody, glide: Glide, controls: CarControls, floor: f32, dt: f32) {
    let rotation = *body.rotation();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;

    let mut wanted = nose * glide.speed;
    // Climbing fades out under the ceiling, and above it it sinks back.
    let headroom = ((glide.ceiling - (body.translation().y - floor)) / 20.0).clamp(-1.0, 1.0);
    wanted.y = wanted.y.min(headroom * glide.speed);
    let mut push = (wanted - body.linvel()) * glide.grip;
    let size = push.length();
    if size > glide.max_accel {
        push *= glide.max_accel / size;
    }
    let along = push.dot(nose);
    if along > glide.speed_up {
        push -= nose * (along - glide.speed_up);
    }
    // A force rather than an impulse: it is integrated together with gravity, so level flight is exact.
    body.add_force((push + Vector::Y * CAR_GRAVITY) * body.mass(), true);
    // The fins bite with the air over them: slow, just out of a tube, it barely turns.
    let bite = if glide.fin_speed > 0.0 { (body.linvel().length() / glide.fin_speed).min(1.0).powi(2) } else { 1.0 };

    let bank = (-right.y).atan2(up.y);
    let steer = controls.steer.clamp(-1.0, 1.0);
    let pull = (-controls.throttle).clamp(-1.0, 1.0);
    let mouse = controls.aim.map(|[yaw, pitch]| {
        let pitch = pitch.clamp(-glide.max_pitch, glide.max_pitch);
        let target = Vector::new(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
        if glide.skid { skid(rotation, target) } else { autopilot(rotation, target, glide.speed, glide.max_bank) }
    });
    let level_pitch = (-nose.y.clamp(-1.0, 1.0).asin() * PLANE_LEVEL_GAIN).clamp(-1.0, 1.0);
    let pitch = if pull.abs() > 0.05 { pull } else { mouse.map_or(level_pitch, |m| m[0]) };
    let roll = if steer.abs() > 0.05 { steer } else { mouse.map_or((-bank * 2.0).clamp(-1.0, 1.0), |m| m[2]) };
    let roll = roll.clamp((-glide.max_bank - bank) * 3.0, (glide.max_bank - bank) * 3.0).clamp(-1.0, 1.0);
    let yaw = match mouse {
        Some(m) if steer.abs() <= 0.05 => m[1],
        _ => (steer * 0.3 + bank.sin() * 0.8).clamp(-1.0, 1.0),
    };
    // + about X lifts the nose, + about Y turns left and + about Z lifts the right wing.
    let turn = rotation * Vector::new(pitch * glide.pitch_rate * bite, -yaw * glide.yaw_rate * bite, -roll * glide.roll_rate);
    let current = body.angvel();
    body.set_angvel(current + (turn - current) * (glide.response * dt).min(1.0), true);
}

/// One tick of a walker: its two legs push the hull up from the ground, a torque keeps it upright and
/// the driver walks and turns it. `feet` is how far below each hip the ground is, `None` where it is
/// out of reach; a leg is a spring that pushes only while the foot is within its length. The legs are
/// the kind's `Gait`.
///
/// Everything is a force or a torque, so a blast or a ram that gives the hull more spin than the
/// balance torque (capped at `Gait::balance_max`) can take out tips it over. Once it is past
/// `Gait::fallen_up` the legs let go and the driver loses control, and a weak pull stands it up again
/// by itself. `standing` is false for a wreck, which has no pull at all. A kind that can crouch
/// (`Gait::crouch`) does while its driver holds `lift` down: its legs shorten and it leans forward.
/// Everything is a function of the body's state and the controls alone, so the client can replay it.
///
/// A kind with feet of its own (`Gait::feet`) stands on those instead, where they are `planted`: each
/// carries its share of the weight at the end of a leg of fixed length, so the hull rises as it goes
/// over a foot and sinks between two. It is drawn towards the foot that carries it, leans to it, and
/// goes slowest over it.
pub(crate) fn walk(body: &mut RigidBody, spec: &Spec, gait: &Gait, controls: CarControls, standing: bool, feet: [Option<f32>; 2], planted: Option<Planted>) {
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    let mass = body.mass();
    let [hx, hy, hz] = spec.half_extents;
    body.reset_forces(false);
    body.reset_torques(false);

    // Upright enough to stand on, and how much of the way from fallen to upright it is.
    let upright = ((up.y - gait.fallen_up) / (1.0 - gait.fallen_up)).clamp(0.0, 1.0);
    let fallen = up.y < gait.fallen_up;
    // How far down it crouches, 0..1: only a driver's doing.
    let crouch = if controls.powered { (-controls.lift).clamp(0.0, 1.0) } else { 0.0 };
    let leg_length = gait.leg_length - gait.crouch * crouch;

    // The legs: springs from each hip, pushing the hull up and damped by the foot's own speed.
    let stood = planted.zip(gait.feet);
    let mut grounded = 0;
    // The share of its weight its feet carry, where the hull is over the foot it is on (1 over it, 0 half
    // a stride from it), and where the middle of what carries it is.
    let mut carried = 0.0;
    let mut over = 0.0;
    let mut support = Vector::ZERO;
    if let Some((planted, shape)) = stood.filter(|_| standing && !fallen) {
        let pelvis = body.translation() + rotation * Vector::new(0.0, -hy, 0.0);
        let rate = body.linvel().y;
        let nose = rotation * Vector::new(0.0, 0.0, -1.0);
        let ahead = Vector::new(nose.x, 0.0, nose.z).normalize_or(Vector::new(0.0, 0.0, -1.0));
        for index in 0..2 {
            let weight = planted.weight[index];
            let side = if index == 0 { -1.0 } else { 1.0 };
            let hip = body.translation() + rotation * Vector::new(side * gait.foot_half, -hy, 0.0);
            let off = planted.at[index] - hip;
            let distance = pelvis.y - planted.at[index].y;
            // A foot the hull has been thrown clear of carries nothing.
            if weight <= 0.0 || distance >= gait.leg_length {
                continue;
            }
            // The leg is a strut from the foot: the further off the hip is, the lower it holds it.
            let out = off.x.hypot(off.z).min(0.8 * leg_length);
            let strut = (leg_length * leg_length - out * out).sqrt();
            let bent = if gait.rise > 0.0 { (strut - distance).min((CAR_GRAVITY + gait.damping * gait.rise) / gait.spring) } else { strut - distance };
            let push = (gait.spring * bent - gait.damping * rate).max(0.0) * mass * weight * upright.sqrt();
            body.add_force_at_point(Vector::new(0.0, push, 0.0), body.translation(), true);
            carried += weight;
            over += weight * (1.0 - (off.dot(ahead) / (0.5 * shape.stride)).powi(2)).max(0.0);
            support += planted.at[index] * weight;
        }
    } else if standing && !fallen {
        let spring = gait.spring * mass / 2.0;
        let damper = gait.damping * mass / 2.0;
        for (index, reach) in feet.iter().enumerate() {
            let Some(distance) = *reach else { continue };
            if distance >= gait.leg_length {
                continue;
            }
            grounded += 1;
            let side = if index == 0 { -1.0 } else { 1.0 };
            let hip = rotation * Vector::new(side * gait.foot_half, -hy, 0.0);
            let rate = (body.linvel() + body.angvel().cross(hip)).y;
            // Legs that straighten at a set pace push no harder than holds it.
            let bent = if gait.rise > 0.0 { (leg_length - distance).min((CAR_GRAVITY + gait.damping * gait.rise) / gait.spring) } else { leg_length - distance };
            let push = (spring * bent - damper * rate).max(0.0) * upright.sqrt();
            body.add_force_at_point(Vector::new(0.0, push, 0.0), body.translation() + hip, true);
        }
    }

    // What the drive asks for, as a push in the world's horizontal plane (an acceleration, m/s²), only
    // with both a driver and feet to push with.
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let flat = Vector::new(nose.x, 0.0, nose.z);
    let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
    let right = heading.cross(Vector::Y);
    let velocity = body.linvel();
    let footing = if stood.is_some() { carried } else { grounded as f32 / 2.0 };
    let share = upright * footing;
    let standing_on = footing > 0.0 && !fallen;
    let driven = controls.powered && standing_on;
    let mut push = Vector::ZERO;
    // Planted feet hold it still when nobody drives: it stops where it stands, as a walker does.
    if standing_on {
        let throttle = if driven { controls.throttle.clamp(-1.0, 1.0) } else { 0.0 };
        let mut top = if throttle >= 0.0 { gait.max_speed } else { gait.max_speed * gait.reverse_share };
        // On feet of its own it is slower crouched, and goes slowest as it passes over a foot.
        if let Some((_, shape)) = stood {
            top *= (1.0 - crouch * (1.0 - shape.crouch_pace)) * (1.0 + shape.pulse * (0.65 - over / carried));
        }
        let along = velocity.dot(heading);
        let wanted = throttle * top;
        // Slowing down is quicker than speeding up: it plants its feet and leans back.
        let limit = if wanted.abs() < along.abs() || wanted * along < 0.0 { gait.brake } else { gait.accel };
        let ahead = (wanted - along).clamp(-limit, limit);
        // Planted feet do not slide sideways.
        // On feet of its own it is drawn part of the way over the one that carries it.
        let drawn = stood.map_or(0.0, |(planted, shape)| {
            let middle = (planted.at[0] + planted.at[1]) * 0.5;
            let to = middle + (support / carried - middle) * shape.sway;
            (to - body.translation()).dot(right) * shape.sway_gain
        });
        let sideways = (drawn - velocity.dot(right) * gait.grip_gain).clamp(-gait.grip, gait.grip);
        push = (heading * ahead + right * sideways) * share;
    }

    // Balance: an acceleration that turns the hull's up axis towards where it should point, damped by its
    // tilt rate. A walker leans into what it pushes with, as a person does, and a little more the faster it
    // goes. Pitching is the axis it can fight best (`Gait::balance_max_pitch`), and falling forwards or
    // back it takes a catching step: a push under the top, so its feet get back under it. Fallen, only the
    // weak getting-up share is left.
    let mut catching = Vector::ZERO;
    if standing {
        // A kind that gets up with more than its standing pull eases back to it as it comes upright, so
        // that it does not drop again the moment its legs take over.
        let risen = if gait.getup_share > 1.0 { 1.0 + (gait.getup_share - 1.0) * (1.0 - upright) } else { 1.0 };
        let share = if fallen { gait.getup_share } else if footing > 0.0 { risen } else { 0.0 };
        let cruise = (velocity.dot(heading) / gait.max_speed).clamp(-1.0, 1.0) * gait.cruise_lean + crouch * gait.crouch_lean;
        // On feet of its own it leans to the one that carries it.
        let tilt = stood.map_or(0.0, |(planted, shape)| (planted.weight[1] - planted.weight[0]) * shape.tilt * footing);
        let target = (Vector::Y + push * gait.accel_lean / CAR_GRAVITY + heading * cruise + right * tilt).normalize();
        let error = up.cross(target);
        let mut tilt_rate = body.angvel();
        tilt_rate.y = 0.0;
        let raw = error * gait.balance_gain - tilt_rate * gait.balance_damp;
        let pitch = raw.dot(right);
        let wanted = (raw - right * pitch).clamp_length_max(gait.balance_max) + right * pitch.clamp(-gait.balance_max_pitch, gait.balance_max_pitch);
        let inertia = mass * (hx * hx + hy * hy + hz * hz) / 3.0;
        body.add_torque(wanted * (share * inertia), true);

        if footing > 0.0 && !fallen {
            // How far along its heading the top has gone past the lean it walks with, and how fast it is
            // falling that way, each beyond a margin the walking lean and tilt rate itself uses.
            let lean = (up.dot(heading) - cruise) * (1.0 / up.y.max(0.3));
            let mut fall = body.angvel().cross(up);
            fall.y = 0.0;
            let dead = |x: f32, margin: f32| x.signum() * (x.abs() - margin).max(0.0);
            let step = (dead(lean, gait.catch_lean_margin) * gait.catch_gain + dead(fall.dot(heading), gait.catch_fall_margin) * gait.catch_damp)
                .clamp(-gait.catch_max, gait.catch_max);
            catching = heading * (step * footing);
        }
    }

    // The push goes through the middle of the hull: the lean comes from the balance, not from a torque.
    let total = push + catching;
    if total != Vector::ZERO {
        body.add_force_at_point(total * mass, body.translation(), true);
    }
    if !driven {
        return;
    }
    // Clockwise seen from above is a negative rotation about +Y.
    // It turns slower the faster it walks, so its feet can keep up with the heading.
    let turn_rate = gait.turn_rate / (1.0 + velocity.dot(heading).abs() / gait.max_speed);
    let yaw = (-controls.steer.clamp(-1.0, 1.0) * turn_rate - body.angvel().y) * 4.0;
    let inertia = mass * (hx * hx + hz * hz) / 3.0;
    body.add_torque(Vector::new(0.0, yaw.clamp(-gait.turn_accel, gait.turn_accel) * inertia * share, 0.0), true);
}

/// The share of a plane's thrust its throttle asks for. With the stick released it cruises in the
/// air but idles on the ground, so a pilot who climbs in does not roll off at once.
pub(crate) fn throttle_setting(throttle: f32, grounded: bool) -> f32 {
    let throttle = throttle.clamp(-1.0, 1.0);
    if throttle > 0.0 {
        PLANE_CRUISE_THROTTLE + throttle * (1.0 - PLANE_CRUISE_THROTTLE)
    } else if throttle < 0.0 {
        PLANE_CRUISE_THROTTLE + throttle * (PLANE_CRUISE_THROTTLE - PLANE_IDLE_THROTTLE)
    } else if grounded {
        PLANE_IDLE_THROTTLE
    } else {
        PLANE_CRUISE_THROTTLE
    }
}

/// The unit direction a pilot looking at `[yaw, pitch]` wants to fly, turned back towards the middle
/// near the edge of a world `bound` metres from its centre to each side.
pub(crate) fn aim_direction(aim: [f32; 2], position: Vector, bound: f32) -> Vector {
    let [yaw, pitch] = aim;
    let direction = Vector::new(-yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    // How far into the margin it is along each axis, 0 to 1, towards the side it is on.
    let into = |coordinate: f32| ((coordinate.abs() - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN).clamp(0.0, 1.0) * coordinate.signum();
    let (x, z) = (into(position.x), into(position.z));
    let near = x.abs().max(z.abs());
    if near == 0.0 {
        return direction;
    }
    // Inwards and level, blended in the deeper it goes.
    let inward = Vector::new(-x, 0.0, -z).normalize();
    let blended = direction * (1.0 - near) + inward * near;
    if blended.length() > 1e-3 { blended.normalize() } else { inward }
}

/// How deep into the edge margin `position` is, 0 (outside it) to 1 (at the wall).
pub(crate) fn edge_depth(position: Vector, bound: f32) -> f32 {
    let into = |coordinate: f32| ((coordinate.abs() - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN).clamp(0.0, 1.0);
    into(position.x).max(into(position.z))
}

/// How a skid-to-turn missile is steered towards `target` (a world direction), as `[pitch, yaw, roll]`
/// commands in -1..1: it points its nose straight at it with pitch and yaw, as cruciform fins do,
/// rather than banking into the turn, and keeps its wings level. Nose straight up or down, where the
/// wings have no level to find, it holds its roll, and it levels them as the nose comes down past about
/// 80 degrees.
fn skid(rotation: Rotation, target: Vector) -> [f32; 3] {
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;
    let local = rotation.inverse() * target;
    let off = (-local.z).clamp(-1.0, 1.0).acos();
    let lateral = local.x.hypot(local.y);
    // Dead astern, it turns right rather than not at all.
    let (across, above) = if lateral > 1e-3 { (local.x / lateral, local.y / lateral) } else { (1.0, 0.0) };
    let steep = ((nose.y.abs() - 0.92) / 0.07).clamp(0.0, 1.0);
    let bank = (-right.y).atan2(up.y);
    [(above * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0), (across * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0), (-bank * 2.0).clamp(-1.0, 1.0) * (1.0 - steep)]
}

/// What a plane flies itself to do to head for `target`, a world direction, banking no more than `max_bank`: `[pitch, yaw, roll]`, each
/// -1..=1 as a share of the full rate. It banks into the turn and pulls once banked, takes the turn
/// level when the target is behind, and levels its wings when the target is nearly ahead. The pilot
/// uses it near the edge of the world and an empty plane to glide.
pub(crate) fn autopilot(rotation: Rotation, target: Vector, airspeed: f32, max_bank: f32) -> [f32; 3] {
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;
    let local = rotation.inverse() * target;
    // How far off the nose the target is, and which way round it lies seen from the cockpit:
    // `across` right and `above` up, a unit pair.
    let off = (-local.z).clamp(-1.0, 1.0).acos();
    let lateral = local.x.hypot(local.y);
    let (mut across, mut above) = if lateral > 1e-3 { (local.x / lateral, local.y / lateral) } else { (1.0, 0.0) };
    let bank = (-right.y).atan2(up.y);
    // Anywhere behind, a level turn to whichever side it is on, rather than a loop over the top or a
    // dive: the target is taken to be level off that wing, which the bank brings overhead.
    if off > std::f32::consts::FRAC_PI_2 {
        // Nearly dead astern, keep turning the way it already leans, so it does not dither.
        let lean = if across.abs() < 0.5 && bank.abs() > 0.2 { bank } else { across };
        let side = if lean < 0.0 { -1.0 } else { 1.0 };
        (across, above) = (side * bank.cos(), bank.sin().abs());
    }
    let mut pitch = (above * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0);
    if pitch > 0.0 {
        // A wide turn waits for the bank before it pulls, and nobody pulls hard near the stall.
        let banked = if off > 1.2 { (bank.abs() / max_bank).clamp(0.25, 1.0) } else { 1.0 };
        let margin = ((airspeed - PLANE_STALL_SPEED) / (0.4 * PLANE_STALL_SPEED)).clamp(0.2, 1.0);
        pitch *= banked * margin;
    }
    let yaw = (across * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0);
    let level = (-bank * 2.0).clamp(-1.0, 1.0);
    let into = (across * off * 3.0).clamp(-1.0, 1.0);
    let blend = (off / PLANE_BANK_ANGLE).min(1.0);
    let roll = (level + (into - level) * blend).clamp((-max_bank - bank) * 3.0, (max_bank - bank) * 3.0);
    [pitch, yaw, roll.clamp(-1.0, 1.0)]
}

/// One tick of a plane's flight, applied before the rigid-body step on top of its wheels.
///
/// The propeller pushes along the nose. Above the stall speed the wings hold the plane up, turn its
/// flight path towards where the nose points (lift grows with the angle between them) and the fin
/// kills sideslip; below it the lift fades away and the nose drops towards where the plane is going.
/// Drag grows with the square of the speed.
///
/// The mouse sets a heading: the plane turns towards `aim` as the autopilot would, banking into the turn
/// and levelling when it arrives; W/S and A/D take pitch and roll over by hand while they are held.
/// The controls are a stick and a throttle. Throttle: `lift` is Space (+1, full) and Shift (-1, cut),
/// with cruise when neither is held. Stick: `throttle` is the forward axis, W (+1) pushes the nose down and
/// S pulls it up; with the stick centred the nose settles back to the horizon. A/D (`steer`) roll the
/// wings, which level again when released, and a banked plane turns of its own accord with a little
/// rudder from the stick. Authority is lost at low speed. Near the edge of the world the plane turns itself
/// back, and with nobody aboard it glides. Everything is a function of the body's state and the controls
/// alone, so the client can replay it. What differs between one aeroplane and the next is its `airframe`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wing(body: &mut RigidBody, airframe: &Airframe, controls: CarControls, power: f32, grounded: bool, bound: f32, floor: f32, dt: f32) {
    let rotation = *body.rotation();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;
    let velocity = body.linvel();
    let position = body.translation();
    let airspeed = velocity.dot(nose).max(0.0);
    let speed = velocity.length();

    // How much the wings fly, 0 (stalled) to 1.
    let flying = ((airspeed - 0.7 * airframe.stall_speed) / (0.3 * airframe.stall_speed)).clamp(0.0, 1.0);
    // Level flight needs no angle of attack above `airframe.level_speed`; slower, the wings carry less and
    // the plane sinks unless the nose comes up.
    // Close to the ground they carry it all (ground effect), so it lifts off at the stall speed.
    let carried = if grounded { 1.0 } else { (airspeed / airframe.level_speed).powi(2).min(1.0) };
    let lift = (flying * (CAR_GRAVITY * carried - PLANE_AOA_GAIN * velocity.dot(up))).clamp(-airframe.max_lift, airframe.max_lift);
    let side = -PLANE_SIDE_GAIN * velocity.dot(right) * (airspeed / airframe.stall_speed).min(1.0);
    let headroom = ((PLANE_CEILING - (position.y - floor)) / 20.0).clamp(0.0, 1.0);
    let thrust = if controls.powered { throttle_setting(controls.lift, grounded) * airframe.thrust * power * headroom } else { 0.0 };
    let accel = nose * thrust + up * lift + right * side - velocity * (speed * airframe.drag);
    // A force rather than an impulse, integrated together with gravity.
    body.add_force(accel * body.mass(), true);

    // A stalled plane noses over towards where it is going.
    let stalled = if grounded || speed < 2.0 { Vector::ZERO } else { nose.cross(velocity / speed) * (PLANE_STALL_DROP * (1.0 - flying)) };
    let current = body.angvel();
    let wanted = if controls.powered || !grounded {
        let heading = (-nose.x).atan2(-nose.z);
        let near = edge_depth(position, bound);
        // What the plane flies itself to do: glide on level (a little nose down with nobody aboard) and,
        // near the edge, turn back to the middle.
        let hold = aim_direction([heading, if controls.powered { 0.0 } else { PLANE_GLIDE_PITCH }], position, bound);
        let auto = autopilot(rotation, hold, airspeed, PLANE_MAX_BANK);
        let [pitch, yaw, roll] = if controls.powered {
            let bank = (-right.y).atan2(up.y);
            let steer = controls.steer.clamp(-1.0, 1.0);
            let pull = (-controls.throttle).clamp(-1.0, 1.0);
            // With the mouse the plane flies itself towards where it points, as the autopilot does; the
            // keys then take an axis over while they are held, and with neither it flies level.
            let mouse = controls.aim.map(|[yaw, pitch]| {
                let aim = aim_direction([yaw, pitch.clamp(-PLANE_AIM_MAX_PITCH, PLANE_AIM_MAX_PITCH)], position, bound);
                autopilot(rotation, aim, airspeed, PLANE_MAX_BANK)
            });
            let level_pitch = (-nose.y.clamp(-1.0, 1.0).asin() * PLANE_LEVEL_GAIN).clamp(-1.0, 1.0);
            let pitch = if pull.abs() > 0.05 { pull } else { mouse.map_or(level_pitch, |m| m[0]) };
            let roll = if steer.abs() > 0.05 { steer } else { mouse.map_or((-bank * 2.0).clamp(-1.0, 1.0), |m| m[2]) };
            let roll = roll.clamp((-PLANE_MAX_BANK - bank) * 3.0, (PLANE_MAX_BANK - bank) * 3.0).clamp(-1.0, 1.0);
            let yaw = match mouse {
                Some(m) if steer.abs() <= 0.05 => m[1],
                _ => (steer * 0.3 + bank.sin() * 0.8).clamp(-1.0, 1.0),
            };
            let own = [pitch, yaw, roll];
            // The autopilot takes over completely by a third of the way in.
            let taken = (near * 3.0).min(1.0);
            std::array::from_fn(|i| own[i] * (1.0 - taken) + auto[i] * taken)
        } else {
            auto
        };
        let authority = (airspeed / airframe.cruise_speed).powi(2).min(1.3);
        // + about X lifts the nose, + about Y turns left and + about Z lifts the right wing.
        let turn = Vector::new(pitch * airframe.pitch_rate, -yaw * airframe.yaw_rate, -roll * airframe.roll_rate) * authority;
        rotation * turn + stalled
    } else {
        current
    };
    // On the ground the wheels steer, so the controls only take over as the plane lifts off.
    let response = (PLANE_RESPONSE * dt).min(1.0) * if grounded { flying } else { 1.0 };
    body.set_angvel(current + (wanted - current) * response, true);
}
