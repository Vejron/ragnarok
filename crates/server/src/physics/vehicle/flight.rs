//! How a helicopter and a plane are flown: the hull is levelled and turned by setting its
//! angular velocity, the rotor and propeller are forces on the body, and wings lift it.

use super::*;

/// Share of full engine force at `speed` for a top speed of `max`: all of it up to [`FULL_POWER`]
/// of the way, then less and less until none at `max`.
pub(in crate::physics) fn pull(speed: f32, max: f32) -> f32 {
    ((max - speed.abs()) / (max * (1.0 - FULL_POWER))).clamp(0.0, 1.0)
}

/// A plane's wheels for one tick: no engine, the front pair steered towards where the pilot looks
/// (and by A/D), and braked while the stick is pulled back or nobody is aboard.
pub(in crate::physics) fn roll_plane(car: &mut Car, controls: CarControls, body: &RigidBody, speed: f32, mass: f32, dt: f32) {
    let _ = body;
    let wanted = if controls.powered { controls.steer.clamp(-1.0, 1.0) } else { 0.0 };
    let lock = car.spec.steer_lock / (1.0 + speed.abs() / STEER_FADE_SPEED);
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
pub(in crate::physics) fn assist(body: &mut RigidBody, (throttle, steer): (f32, f32), grounded: bool, dt: f32) {
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

/// One tick of rotor flight for a powered helicopter body.
///
/// The disc leans by setting the angular velocity that carries the hull's up axis towards the
/// wanted lean (forward with the stick, sideways with the turn), and the yaw rate is set straight
/// from the stick, like a tank's. The rotor then pushes along the up axis hard enough to hover
/// however it leans, and holds a vertical speed set by the climb input. Everything is a function
/// of the body's state and the controls alone, so the client can replay it.
pub(in crate::physics) fn fly(body: &mut RigidBody, controls: CarControls, dt: f32) {
    let rotation = *body.rotation();
    let up = rotation * Vector::Y;
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let flat = Vector::new(nose.x, 0.0, nose.z);
    let heading = if flat.length() > 1e-3 { flat.normalize() } else { Vector::new(0.0, 0.0, -1.0) };
    let right = heading.cross(Vector::Y);

    let throttle = controls.throttle.clamp(-1.0, 1.0);
    let steer = controls.steer.clamp(-1.0, 1.0);
    let lean = (Vector::Y + heading * (throttle * HELI_TILT) + right * (steer * HELI_BANK)).normalize();
    let mut wanted = up.cross(lean) * HELI_LEVEL_GAIN;
    // Clockwise seen from above is a negative rotation about +Y.
    wanted.y = -steer * HELI_YAW_RATE;
    let current = body.angvel();
    body.set_angvel(current + (wanted - current) * (HELI_RESPONSE * dt).min(1.0), true);

    // The climb input asks for a vertical speed; with none held it asks for zero, so letting go of
    // the stick holds altitude. Climbing fades out below the ceiling and reverses above it.
    let lift = controls.lift.clamp(-1.0, 1.0);
    let headroom = ((HELI_CEILING - body.translation().y) / 20.0).clamp(-1.0, 1.0);
    let mut target = if lift > 0.0 { lift * HELI_CLIMB_SPEED * headroom.max(0.0) } else { lift * HELI_SINK_SPEED };
    if headroom < 0.0 {
        target = target.min(headroom * HELI_CLIMB_SPEED);
    }
    let extra = ((target - body.linvel().y) * HELI_VERT_GAIN).clamp(-HELI_VERT_ACCEL, HELI_VERT_ACCEL);
    let accel = (CAR_GRAVITY + extra) / up.y.max(0.5);
    // A force rather than an impulse: it is integrated together with gravity, so hovering is exact.
    body.add_force(up * (accel * body.mass()), true);
}

/// The share of a plane's thrust its throttle asks for. With the stick released it cruises in the
/// air but idles on the ground, so a pilot who climbs in does not roll off at once.
pub(in crate::physics) fn throttle_setting(throttle: f32, grounded: bool) -> f32 {
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
pub(in crate::physics) fn aim_direction(aim: [f32; 2], position: Vector, bound: f32) -> Vector {
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
pub(in crate::physics) fn edge_depth(position: Vector, bound: f32) -> f32 {
    let into = |coordinate: f32| ((coordinate.abs() - (bound - PLANE_EDGE_MARGIN)) / PLANE_EDGE_MARGIN).clamp(0.0, 1.0);
    into(position.x).max(into(position.z))
}

/// What a plane flies itself to do to head for `target`, a world direction: `[pitch, yaw, roll]`, each
/// -1..=1 as a share of the full rate. It banks into the turn and pulls once banked, takes the turn
/// level when the target is behind, and levels its wings when the target is nearly ahead. The pilot
/// uses it near the edge of the world and an empty plane to glide.
pub(in crate::physics) fn autopilot(rotation: Rotation, target: Vector, airspeed: f32) -> [f32; 3] {
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
        let banked = if off > 1.2 { (bank.abs() / PLANE_MAX_BANK).clamp(0.25, 1.0) } else { 1.0 };
        let margin = ((airspeed - PLANE_STALL_SPEED) / (0.4 * PLANE_STALL_SPEED)).clamp(0.2, 1.0);
        pitch *= banked * margin;
    }
    let yaw = (across * off * PLANE_AIM_GAIN).clamp(-1.0, 1.0);
    let level = (-bank * 2.0).clamp(-1.0, 1.0);
    let into = (across * off * 3.0).clamp(-1.0, 1.0);
    let blend = (off / PLANE_BANK_ANGLE).min(1.0);
    let roll = (level + (into - level) * blend).clamp((-PLANE_MAX_BANK - bank) * 3.0, (PLANE_MAX_BANK - bank) * 3.0);
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
/// alone, so the client can replay it.
pub(in crate::physics) fn wing(body: &mut RigidBody, controls: CarControls, power: f32, grounded: bool, bound: f32, dt: f32) {
    let rotation = *body.rotation();
    let nose = rotation * Vector::new(0.0, 0.0, -1.0);
    let up = rotation * Vector::Y;
    let right = rotation * Vector::X;
    let velocity = body.linvel();
    let position = body.translation();
    let airspeed = velocity.dot(nose).max(0.0);
    let speed = velocity.length();

    // How much the wings fly, 0 (stalled) to 1.
    let flying = ((airspeed - 0.7 * PLANE_STALL_SPEED) / (0.3 * PLANE_STALL_SPEED)).clamp(0.0, 1.0);
    // Level flight needs no angle of attack above `PLANE_LEVEL_SPEED`; slower, the wings carry less and
    // the plane sinks unless the nose comes up.
    // Close to the ground they carry it all (ground effect), so it lifts off at the stall speed.
    let carried = if grounded { 1.0 } else { (airspeed / PLANE_LEVEL_SPEED).powi(2).min(1.0) };
    let lift = (flying * (CAR_GRAVITY * carried - PLANE_AOA_GAIN * velocity.dot(up))).clamp(-PLANE_MAX_LIFT, PLANE_MAX_LIFT);
    let side = -PLANE_SIDE_GAIN * velocity.dot(right) * (airspeed / PLANE_STALL_SPEED).min(1.0);
    let headroom = ((PLANE_CEILING - position.y) / 20.0).clamp(0.0, 1.0);
    let thrust = if controls.powered { throttle_setting(controls.lift, grounded) * PLANE_THRUST * power * headroom } else { 0.0 };
    let accel = nose * thrust + up * lift + right * side - velocity * (speed * PLANE_DRAG);
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
        let auto = autopilot(rotation, hold, airspeed);
        let [pitch, yaw, roll] = if controls.powered {
            let bank = (-right.y).atan2(up.y);
            let steer = controls.steer.clamp(-1.0, 1.0);
            let pull = (-controls.throttle).clamp(-1.0, 1.0);
            // With the mouse the plane flies itself towards where it points, as the autopilot does; the
            // keys then take an axis over while they are held, and with neither it flies level.
            let mouse = controls.aim.map(|[yaw, pitch]| {
                let aim = aim_direction([yaw, pitch.clamp(-PLANE_AIM_MAX_PITCH, PLANE_AIM_MAX_PITCH)], position, bound);
                autopilot(rotation, aim, airspeed)
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
        let authority = (airspeed / PLANE_CRUISE_SPEED).powi(2).min(1.3);
        // + about X lifts the nose, + about Y turns left and + about Z lifts the right wing.
        let turn = Vector::new(pitch * PLANE_PITCH_RATE, -yaw * PLANE_YAW_RATE, -roll * PLANE_ROLL_RATE) * authority;
        rotation * turn + stalled
    } else {
        current
    };
    // On the ground the wheels steer, so the controls only take over as the plane lifts off.
    let response = (PLANE_RESPONSE * dt).min(1.0) * if grounded { flying } else { 1.0 };
    body.set_angvel(current + (wanted - current) * response, true);
}
