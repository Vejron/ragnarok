//! Ground vehicles: dynamic rigid bodies with ray-cast wheels (Rapier's `DynamicRayCastVehicleController`).
//!
//! The chassis is a box; each wheel is a suspension ray that pushes the chassis up and applies
//! engine, brake and side-grip impulses at its contact point. Body-local axes are +X right,
//! +Y up and -Z forward, so a yaw of 0 faces -Z exactly like a stickman.
//!
//! A car steers its front wheels. A tank has six unsteered wheels and turns on the spot: the
//! physics sets its yaw rate directly, which is how skid steering looks from the outside.
//! A helicopter has no wheels at all: its rotor is a push along the hull's up axis, and the hull is
//! levelled and turned by setting its angular velocity, so it is flown by leaning (see [`fly`]).
//! "Car" in the names below means any of these vehicles; [`Spec`] says which one.

use super::Physics;
use rapier3d::control::{DynamicRayCastVehicleController, WheelTuning};
use rapier3d::prelude::*;
use shared::consts::*;
use shared::protocol::{KIND_HELI, KIND_TANK};

/// Gravity acting on vehicle bodies. Stickmen use `GRAVITY`; the ray-cast suspension is tuned
/// around the real-world value.
pub const CAR_GRAVITY: f32 = 9.81;

/// What differs between vehicle kinds.
pub(super) struct Spec {
    pub half_extents: [f32; 3],
    /// Chassis density in kg/m³.
    density: f32,
    wheel_radius: f32,
    track_half: f32,
    /// Z of each axle, front first.
    axles: &'static [f32],
    hard_point_y: f32,
    suspension_rest: f32,
    max_suspension_force: f32,
    /// Total engine force over all wheels, newtons.
    engine_force: f32,
    max_speed: f32,
    /// Steering lock of the front axle at standstill, radians. Zero for a tank, which turns by `turn_rate`.
    steer_lock: f32,
    /// Yaw rate at full stick for a vehicle that turns on the spot, rad/s.
    turn_rate: f32,
    /// Flies on a rotor instead of rolling on wheels.
    flies: bool,
    linear_damping: f32,
}

static CAR: Spec = Spec {
    half_extents: CAR_HALF_EXTENTS,
    // The box is 6 m³, so a car weighs about 1.2 tonnes.
    density: 200.0,
    wheel_radius: CAR_WHEEL_RADIUS,
    track_half: CAR_TRACK_HALF,
    axles: &[-CAR_AXLE_Z, CAR_AXLE_Z],
    hard_point_y: CAR_HARD_POINT_Y,
    suspension_rest: CAR_SUSPENSION_REST,
    max_suspension_force: 60_000.0,
    engine_force: 7000.0,
    max_speed: CAR_MAX_SPEED,
    steer_lock: CAR_MAX_STEER,
    turn_rate: 0.0,
    flies: false,
    linear_damping: 0.05,
};

static TANK: Spec = Spec {
    half_extents: TANK_HALF_EXTENTS,
    // 13.5 m³ of hull is about 3.4 tonnes: three times the car, with a stronger engine to match.
    density: 250.0,
    wheel_radius: TANK_WHEEL_RADIUS,
    track_half: TANK_TRACK_HALF,
    axles: &TANK_AXLES_Z,
    hard_point_y: TANK_HARD_POINT_Y,
    suspension_rest: TANK_SUSPENSION_REST,
    max_suspension_force: 200_000.0,
    engine_force: 22_000.0,
    max_speed: TANK_MAX_SPEED,
    steer_lock: 0.0,
    turn_rate: TANK_TURN_RATE,
    flies: false,
    linear_damping: 0.05,
};

static HELI: Spec = Spec {
    half_extents: HELI_HALF_EXTENTS,
    // 3.8 m³ of hull is about 1.1 tonnes. Nothing below uses the wheel fields.
    density: 300.0,
    wheel_radius: 0.0,
    track_half: 0.0,
    axles: &[],
    hard_point_y: 0.0,
    suspension_rest: 0.0,
    max_suspension_force: 0.0,
    engine_force: 0.0,
    max_speed: 0.0,
    steer_lock: 0.0,
    turn_rate: 0.0,
    flies: true,
    // Air drag is what limits the speed: the rotor's lean gives about 4.9 m/s², so about 25 m/s.
    linear_damping: 0.2,
};

/// The spec for a vehicle kind. Unknown kinds are cars.
pub(super) fn spec(kind: u8) -> &'static Spec {
    match kind {
        KIND_TANK => &TANK,
        KIND_HELI => &HELI,
        _ => &CAR,
    }
}

/// Half extents of the chassis box of a vehicle kind.
pub fn half_extents(kind: u8) -> [f32; 3] {
    spec(kind).half_extents
}

/// How quickly a tank's yaw rate follows the stick, rad/s².
const TURN_ACCEL: f32 = 3.0;
/// Speed change a blast gives a vehicle at its centre, m/s.
const BLAST_VELOCITY: f32 = 5.0;
/// Impulse each wheel may use for braking per tick, per kilogram of chassis.
const BRAKE_PER_KG: f32 = 0.035;
/// Light brake that stands in for rolling resistance when the driver is off the pedals.
const COAST_BRAKE_PER_KG: f32 = 0.0015;
/// How fast the front wheels turn towards the steering input, radians per second.
const STEER_RATE: f32 = 3.0;
/// Steering lock shrinks with speed: `CAR_MAX_STEER / (1 + speed / STEER_FADE_SPEED)`.
const STEER_FADE_SPEED: f32 = 14.0;

/// What the driver asks of the car this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CarControls {
    /// -1 (reverse) ..= 1 (forward).
    pub throttle: f32,
    /// -1 (left) ..= 1 (right). A helicopter turns instead.
    pub steer: f32,
    pub handbrake: bool,
    /// -1 (sink) ..= 1 (climb). Only a helicopter has it.
    pub lift: f32,
    /// The rotor is turning, i.e. a pilot is aboard. Only a helicopter has it.
    pub powered: bool,
}

impl CarControls {
    /// Nobody asks for anything.
    pub const IDLE: CarControls = CarControls { throttle: 0.0, steer: 0.0, handbrake: false, lift: 0.0, powered: false };
}

/// A car's pose as the game loop and the wire need it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarPose {
    pub position: [f32; 3],
    /// Unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Speed along the forward axis, m/s; negative when reversing.
    pub forward_speed: f32,
    /// Front wheel angle as a fraction of `CAR_MAX_STEER`, + is right.
    pub steer: f32,
    /// Y component of the chassis' up axis: 1 upright, 0 on its side, -1 upside down.
    pub up_y: f32,
    /// Linear velocity, m/s.
    pub velocity: [f32; 3],
}

/// What a driver needs to resume predicting a car, beyond its pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarMotion {
    /// Linear velocity, m/s.
    pub linvel: [f32; 3],
    /// Angular velocity about the world axes, rad/s.
    pub angvel: [f32; 3],
    /// Front wheel angle, radians, + is right.
    pub steer: f32,
    /// Yaw rate a tank has ramped to, rad/s, + is clockwise.
    pub spin: f32,
}

/// A car touching a character.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarHit {
    pub car: u32,
    pub position: [f32; 3],
    /// Linear velocity of the chassis, m/s.
    pub velocity: [f32; 3],
}

pub(super) struct Car {
    spec: &'static Spec,
    body: RigidBodyHandle,
    collider: ColliderHandle,
    /// A tank's turret, a second solid on the same body. It has no mass of its own.
    turret: Option<ColliderHandle>,
    controller: DynamicRayCastVehicleController,
    /// Current front wheel angle, radians, + is right.
    steer: f32,
    /// Yaw rate asked for by a vehicle that turns on the spot, rad/s, + is clockwise (right). Zero leaves it alone.
    turn: f32,
    /// The yaw rate it is actually turning at, rad/s, + is clockwise. It ramps towards `turn`.
    spin: f32,
    /// What a helicopter was last asked to do, applied in the next [`Physics::step_cars`].
    flight: CarControls,
}

impl Car {
    /// Whether `collider` is one of this vehicle's solids.
    fn owns(&self, collider: ColliderHandle) -> bool {
        self.collider == collider || self.turret == Some(collider)
    }
}

fn pose_of(position: [f32; 3], yaw: f32) -> Pose {
    Pose::from_parts(Vector::from(position), Rotation::from_rotation_y(yaw))
}

/// How far below its feet a standing character probes for a car to ride. The character controller
/// rests 2 cm above whatever it stands on, so this reaches into the roof.
const RIDE_PROBE: f32 = 0.06;

/// One tick of rotor flight for a powered helicopter body.
///
/// The disc leans by setting the angular velocity that carries the hull's up axis towards the
/// wanted lean (forward with the stick, sideways with the turn), and the yaw rate is set straight
/// from the stick, like a tank's. The rotor then pushes along the up axis hard enough to hover
/// however it leans, and holds a vertical speed set by the climb input. Everything is a function
/// of the body's state and the controls alone, so the client can replay it.
fn fly(body: &mut RigidBody, controls: CarControls, dt: f32) {
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

impl CarPose {
    /// Where `point`, fixed to the car at `self`, ends up once the car has moved to `to`.
    pub fn carry(&self, to: &CarPose, point: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        let from = Rotation::from_xyzw(x, y, z, w);
        let [x, y, z, w] = to.rotation;
        let onto = Rotation::from_xyzw(x, y, z, w);
        let local = from.inverse() * (Vector::from(point) - Vector::from(self.position));
        (onto * local + Vector::from(to.position)).into()
    }

    /// Where `local`, a point in the vehicle's own frame, is in the world.
    pub fn world_point(&self, local: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        (Rotation::from_xyzw(x, y, z, w) * Vector::from(local) + Vector::from(self.position)).into()
    }

    /// The world direction of `local`, a direction in the vehicle's own frame.
    pub fn world_dir(&self, local: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        (Rotation::from_xyzw(x, y, z, w) * Vector::from(local)).into()
    }

    /// The direction in the vehicle's own frame of `world`, a world direction.
    pub fn local_dir(&self, world: [f32; 3]) -> [f32; 3] {
        let [x, y, z, w] = self.rotation;
        (Rotation::from_xyzw(x, y, z, w).inverse() * Vector::from(world)).into()
    }

    /// Heading of the forward axis in the stickman convention: 0 faces -Z, + turns left (towards -X).
    pub fn yaw(&self) -> f32 {
        let [x, y, z, w] = self.rotation;
        (2.0 * (x * z + w * y)).atan2(1.0 - 2.0 * (x * x + y * y))
    }
}

impl Physics {
    /// Whether a vehicle of `kind` at `position` / `yaw` is clear of buildings and walls.
    pub fn car_fits(&self, kind: u8, position: [f32; 3], yaw: f32) -> bool {
        let [hx, hy, hz] = spec(kind).half_extents;
        // Slightly inflated, so a car never starts touching a wall.
        let shape = SharedShape::cuboid(hx + 0.1, hy + 0.1, hz + 0.1);
        !self.touches_static(shape.as_ref(), pose_of(position, yaw))
    }

    /// Adds a vehicle of `kind` under `id`; read it back with [`Physics::car_pose`].
    pub fn add_car(&mut self, id: u32, kind: u8, position: [f32; 3], yaw: f32) {
        let spec = spec(kind);
        let [hx, hy, hz] = spec.half_extents;
        let body = RigidBodyBuilder::dynamic()
            .pose(pose_of(position, yaw))
            .linear_damping(spec.linear_damping)
            .angular_damping(0.8)
            .build();
        let body = self.bodies.insert(body);
        let collider = self.colliders.insert_with_parent(
            ColliderBuilder::cuboid(hx, hy, hz).density(spec.density).friction(0.4).build(),
            body,
            &mut self.bodies,
        );
        // The turret is a cylinder, so it needs no turning with the gun. It adds no mass, which keeps
        // the hull's balance and the tuning that goes with it.
        let turret = (kind == KIND_TANK).then(|| {
            let [x, y, z] = TANK_TURRET_PIVOT;
            self.colliders.insert_with_parent(
                ColliderBuilder::cylinder(TANK_TURRET_HALF_HEIGHT, TANK_TURRET_RADIUS)
                    .translation(Vector::new(x, y, z))
                    .density(0.0)
                    .friction(0.4)
                    .build(),
                body,
                &mut self.bodies,
            )
        });
        // A helicopter has no wheels, so its controller is never updated.
        let mut controller = DynamicRayCastVehicleController::new(body);
        // Our forward is -Z, but the controller only uses this axis to sign its own speed estimate,
        // which we do not read.
        controller.index_forward_axis = 2;
        let tuning = WheelTuning {
            suspension_stiffness: 25.0,
            suspension_compression: 2.2,
            suspension_damping: 3.0,
            max_suspension_travel: 0.3,
            side_friction_stiffness: 1.0,
            friction_slip: 12.0,
            max_suspension_force: spec.max_suspension_force,
        };
        for &z in spec.axles {
            for side in [-1.0, 1.0] {
                controller.add_wheel(
                    Vector::new(side * spec.track_half, spec.hard_point_y, z),
                    Vector::new(0.0, -1.0, 0.0),
                    // The axle points right so that `up × axle` (the rolling direction) is -Z.
                    Vector::new(1.0, 0.0, 0.0),
                    spec.suspension_rest,
                    spec.wheel_radius,
                    &tuning,
                );
            }
        }
        self.cars.insert(id, Car { spec, body, collider, turret, controller, steer: 0.0, turn: 0.0, spin: 0.0, flight: CarControls::IDLE });
    }

    /// Puts a car back upright and at rest.
    pub fn reset_car(&mut self, id: u32, position: [f32; 3], yaw: f32) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        car.steer = 0.0;
        car.turn = 0.0;
        car.spin = 0.0;
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_position(pose_of(position, yaw), true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Test helper: rolls a car about its forward axis and moves it.
    #[cfg(test)]
    pub fn set_car_roll(&mut self, id: u32, roll: f32, position: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.set_position(Pose::from_parts(Vector::from(position), Rotation::from_rotation_z(roll)), true);
            body.set_linvel(Vector::ZERO, true);
            body.set_angvel(Vector::ZERO, true);
        }
    }

    /// Applies the driver's controls to the wheels for the next [`Physics::step_cars`].
    pub fn drive_car(&mut self, id: u32, controls: CarControls, dt: f32) {
        let Some(car) = self.cars.get_mut(&id) else { return };
        if car.spec.flies {
            car.flight = controls;
            return;
        }
        let Some(body) = self.bodies.get(car.body) else { return };
        let mass = body.mass();
        let forward = body.rotation() * Vector::new(0.0, 0.0, -1.0);
        let speed = forward.dot(body.linvel());

        // Steering eases towards its target, and the lock narrows at speed.
        let lock = car.spec.steer_lock / (1.0 + speed.abs() / STEER_FADE_SPEED);
        let target = controls.steer.clamp(-1.0, 1.0) * lock;
        let step = STEER_RATE * dt;
        car.steer += (target - car.steer).clamp(-step, step);
        car.turn = controls.steer.clamp(-1.0, 1.0) * car.spec.turn_rate;

        // Pushing against the direction of travel brakes instead of reversing the engine.
        let against = controls.throttle * speed < -1.0;
        let (engine, brake) = if controls.throttle == 0.0 {
            (0.0, COAST_BRAKE_PER_KG * mass)
        } else if against {
            (0.0, BRAKE_PER_KG * mass)
        } else {
            let headroom = if controls.throttle > 0.0 { (1.0 - speed.abs() / car.spec.max_speed).max(0.0) } else { 0.5 };
            (controls.throttle * car.spec.engine_force / car.controller.wheels().len() as f32 * headroom, 0.0)
        };
        let handbrake = if controls.handbrake { BRAKE_PER_KG * mass } else { 0.0 };

        // Only a car's first axle steers and is spared the handbrake.
        let front_wheels = if car.spec.steer_lock > 0.0 { 2 } else { 0 };
        for (index, wheel) in car.controller.wheels_mut().iter_mut().enumerate() {
            let front = index < front_wheels;
            // Wheels are added front pair first. The steering angle is negated: +angle turns the
            // wheel about the controller's up axis, which turns the car left.
            wheel.steering = if front { -car.steer } else { 0.0 };
            wheel.engine_force = engine;
            wheel.brake = if front { brake } else { brake.max(handbrake) };
        }
    }

    /// Advances every car and the rigid-body world by one tick.
    pub fn step_cars(&mut self, dt: f32) {
        if self.cars.is_empty() {
            return;
        }
        for car in self.cars.values().filter(|car| car.spec.flies) {
            if let Some(body) = self.bodies.get_mut(car.body) {
                // Forces persist from step to step until reset.
                body.reset_forces(false);
                if car.flight.powered {
                    fly(body, car.flight, dt);
                }
            }
        }
        for car in self.cars.values_mut().filter(|car| !car.spec.flies) {
            // Wheel rays must not hit their own chassis.
            let filter = QueryFilter::default().exclude_rigid_body(car.body);
            let queries = self.broad_phase.as_query_pipeline_mut(
                self.narrow_phase.query_dispatcher(),
                &mut self.bodies,
                &mut self.colliders,
                filter,
            );
            car.controller.update_vehicle(dt, queries);
        }
        // A tank turns on the spot, but only while some track is on the ground. The wheels' side
        // grip fights any spin every tick, so the rate is kept here and written back each time.
        for car in self.cars.values_mut().filter(|car| !car.spec.flies) {
            let grounded = car.controller.wheels().iter().any(|wheel| wheel.raycast_info().is_in_contact);
            if car.turn == 0.0 || !grounded {
                car.spin = 0.0;
                continue;
            }
            car.spin += (car.turn - car.spin).clamp(-TURN_ACCEL * dt, TURN_ACCEL * dt);
            if let Some(body) = self.bodies.get_mut(car.body) {
                let mut spin = body.angvel();
                // Clockwise seen from above is a negative rotation about +Y.
                spin.y = -car.spin;
                body.set_angvel(spin, true);
            }
        }
        self.step_pipeline();
    }

    /// The car, if any, overlapping a standing character capsule centred at `position`.
    pub fn car_hit(&self, position: [f32; 3]) -> Option<CarHit> {
        self.car_touching(position)
    }

    /// The car, if any, that a character centred at `position` is standing on.
    pub fn car_under(&self, position: [f32; 3]) -> Option<CarHit> {
        self.car_touching([position[0], position[1] - RIDE_PROBE, position[2]])
    }

    fn car_touching(&self, position: [f32; 3]) -> Option<CarHit> {
        let queries = self.broad_phase.as_query_pipeline(
            self.narrow_phase.query_dispatcher(),
            &self.bodies,
            &self.colliders,
            QueryFilter::only_dynamic(),
        );
        let (hit, _) = queries.intersect_shape(Pose::from_translation(Vector::from(position)), self.character_shape.shape()).next()?;
        let (&car, body) = self.cars.iter().find(|(_, car)| car.owns(hit)).and_then(|(id, car)| Some((id, self.bodies.get(car.body)?)))?;
        Some(CarHit { car, position: body.translation().into(), velocity: body.linvel().into() })
    }

    /// How far a ray from `origin` along the unit vector `dir` travels before it hits the world or a
    /// vehicle other than `ignore`, up to `max`. `None` when it reaches `max` clear.
    pub fn cast_ray(&self, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: u32) -> Option<f32> {
        self.cast_ray_hit(origin, dir, max, ignore).map(|(distance, _)| distance)
    }

    /// Like [`Self::cast_ray`], and also says which vehicle it hit, if it was a vehicle and not the world.
    pub fn cast_ray_hit(&self, origin: [f32; 3], dir: [f32; 3], max: f32, ignore: u32) -> Option<(f32, Option<u32>)> {
        let mut filter = QueryFilter::default();
        if let Some(car) = self.cars.get(&ignore) {
            filter = filter.exclude_rigid_body(car.body);
        }
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, filter);
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (collider, distance) = queries.cast_ray(&ray, max, true)?;
        let car = self.cars.iter().find(|(_, car)| car.owns(collider)).map(|(&id, _)| id);
        Some((distance, car))
    }

    /// How far a ray travels before it meets the world or a vehicle, and the surface normal there.
    pub fn cast_ray_normal(&self, origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<(f32, [f32; 3])> {
        let queries = self.broad_phase.as_query_pipeline(self.narrow_phase.query_dispatcher(), &self.bodies, &self.colliders, QueryFilter::default());
        let ray = Ray::new(Vector::from(origin), Vector::from(dir));
        let (_, hit) = queries.cast_ray_and_get_normal(&ray, max, true)?;
        Some((hit.time_of_impact, hit.normal.into()))
    }

    /// Changes vehicle `id`'s velocity by `delta` m/s at once, as a gun's recoil does.
    pub fn kick_car(&mut self, id: u32, delta: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            body.apply_impulse(Vector::from(delta) * body.mass(), true);
        }
    }

    /// Adds `spin` rad/s to vehicle `id`'s angular velocity, given in its own frame (+X right,
    /// +Z back), so a wreck can be sent end over end or onto its side.
    pub fn spin_car(&mut self, id: u32, spin: [f32; 3]) {
        let Some(car) = self.cars.get(&id) else { return };
        if let Some(body) = self.bodies.get_mut(car.body) {
            let world = *body.rotation() * Vector::from(spin);
            let now = body.angvel();
            body.set_angvel(now + world, true);
        }
    }

    /// Shoves every vehicle within `radius` of `point` away from it, harder the closer it is.
    /// Returns each one it reached with how close it was: 1 at the point, 0 at the edge.
    pub fn blast_cars(&mut self, point: [f32; 3], radius: f32) -> Vec<(u32, f32)> {
        let point = Vector::from(point);
        let mut reached = Vec::new();
        for (&id, car) in &self.cars {
            let Some(body) = self.bodies.get_mut(car.body) else { continue };
            let away = body.translation() - point;
            let distance = away.length();
            if distance >= radius {
                continue;
            }
            let falloff = 1.0 - distance / radius;
            let direction = (away / distance.max(1e-3) + Vector::new(0.0, 0.4, 0.0)).normalize();
            body.apply_impulse(direction * falloff * BLAST_VELOCITY * body.mass(), true);
            reached.push((id, falloff));
        }
        reached
    }

    pub fn car_motion(&self, id: u32) -> Option<CarMotion> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        Some(CarMotion { linvel: body.linvel().into(), angvel: body.angvel().into(), steer: car.steer, spin: car.spin })
    }

    pub fn car_pose(&self, id: u32) -> Option<CarPose> {
        let car = self.cars.get(&id)?;
        let body = self.bodies.get(car.body)?;
        let rotation = *body.rotation();
        let forward = rotation * Vector::new(0.0, 0.0, -1.0);
        Some(CarPose {
            position: body.translation().into(),
            rotation: [rotation.x, rotation.y, rotation.z, rotation.w],
            forward_speed: forward.dot(body.linvel()),
            steer: if car.spec.steer_lock > 0.0 { car.steer / car.spec.steer_lock } else { 0.0 },
            up_y: (rotation * Vector::new(0.0, 1.0, 0.0)).y,
            velocity: body.linvel().into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::consts::TICK_DT;
    use shared::protocol::{KIND_CAR, KIND_HELI, KIND_TANK};
    use shared::world::{CollisionMesh, WORLD_VERSION, WorldFile};

    fn flat() -> WorldFile {
        WorldFile { version: WORLD_VERSION, half_extent: 20_000, ..Default::default() }
    }

    /// A 4 m tall box from x = -5..5 at z = -60..-58, straight ahead of a car at the origin.
    fn world_with_wall() -> WorldFile {
        let v = |x: f32, y: f32, z: f32| [(x * 100.0) as i32, (y * 100.0) as i32, (z * 100.0) as i32];
        let vertices = vec![
            v(-5.0, 0.0, -60.0), v(5.0, 0.0, -60.0), v(5.0, 0.0, -58.0), v(-5.0, 0.0, -58.0),
            v(-5.0, 4.0, -60.0), v(5.0, 4.0, -60.0), v(5.0, 4.0, -58.0), v(-5.0, 4.0, -58.0),
        ];
        let triangles = vec![
            [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6],
            [3, 0, 4], [3, 4, 7], [4, 5, 6], [4, 6, 7], [0, 2, 1], [0, 3, 2],
        ];
        WorldFile { collision: CollisionMesh { vertices, triangles }, ..flat() }
    }

    fn with_car(world: &WorldFile) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        physics.add_car(1, KIND_CAR, [0.0, 1.0, 0.0], 0.0);
        physics
    }

    fn run(physics: &mut Physics, controls: CarControls, ticks: u32) -> CarPose {
        for _ in 0..ticks {
            physics.drive_car(1, controls, TICK_DT);
            physics.step_cars(TICK_DT);
        }
        physics.car_pose(1).unwrap()
    }

    /// Heading of the car's forward axis: 0 faces -Z, positive turns left.
    fn yaw(pose: &CarPose) -> f32 {
        pose.yaw()
    }

    const FORWARD: CarControls = CarControls { throttle: 1.0, ..CarControls::IDLE };
    const COAST: CarControls = CarControls::IDLE;

    /// A scripted drive of a car and a tank on flat ground, for the fixture in `vehicle.spec.ts`.
    fn scripted_controls(tick: u32) -> CarControls {
        let phase = tick / 60;
        match phase {
            0 => COAST,
            1 | 2 => FORWARD,
            3 | 4 => CarControls { throttle: 1.0, steer: 0.6, ..CarControls::IDLE },
            5 => CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE },
            6 => CarControls { throttle: -1.0, ..CarControls::IDLE },
            _ => CarControls { throttle: 0.5, handbrake: true, ..CarControls::IDLE },
        }
    }

    /// `(kind, tick, position)` after `scripted_controls`, mirrored in `vehicle.spec.ts`.
    const SCRIPTED_CHECKPOINTS: [(u8, u32, [f32; 3]); 8] = [
        (KIND_CAR, 90, [0.0, 0.6574, -0.7191]),
        (KIND_CAR, 210, [1.1072, 0.6573, -14.8450]),
        (KIND_CAR, 300, [15.9257, 0.6572, -25.5077]),
        (KIND_CAR, 480, [28.5726, 0.6572, -54.8037]),
        (KIND_TANK, 90, [0.0, 0.9400, -0.7659]),
        (KIND_TANK, 210, [0.4323, 0.9400, -13.2166]),
        (KIND_TANK, 300, [9.7875, 0.9400, -22.8965]),
        (KIND_TANK, 480, [22.4889, 0.9400, -32.2509]),
    ];

    #[test]
    fn scripted_drive_matches_fixture() {
        for kind in [KIND_CAR, KIND_TANK] {
            let mut physics = Physics::new(&flat()).unwrap();
            physics.add_car(1, kind, [0.0, 1.5, 0.0], 0.0);
            for tick in 0..480 {
                physics.drive_car(1, scripted_controls(tick), TICK_DT);
                physics.step_cars(TICK_DT);
                let done = tick + 1;
                if let Some((_, _, expected)) = SCRIPTED_CHECKPOINTS.iter().find(|(k, t, _)| *k == kind && *t == done) {
                    let position = physics.car_pose(1).unwrap().position;
                    let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                    assert!(close, "kind {kind} tick {done}: {position:?}");
                }
            }
        }
    }

    /// A 12° slope rising towards -Z (the way `FORWARD` drives): 5 m cells over 200 m. `vehicle.spec.ts`
    /// builds the same heights.
    fn ramp() -> WorldFile {
        const SLOPE: f32 = 0.2125566;
        let side = 41u32;
        let heights = (0..side)
            .flat_map(|j| (0..side).map(move |_| j))
            .map(|j| ((100.0 - (j as f32 * 5.0 - 100.0)) * SLOPE * 100.0).round() as i16)
            .collect();
        WorldFile { ground: shared::terrain::Ground { origin: -10_000, cell: 500, side, heights }, ..flat() }
    }

    /// `(tick, position)` of a car driven `FORWARD` up `ramp` from [0, _, 40], mirrored in `vehicle.spec.ts`.
    const RAMP_CHECKPOINTS: [(u32, [f32; 3]); 3] = [
        (60, [-0.0002, 13.4981, 39.7061]),
        (180, [-0.0014, 15.3390, 31.0135]),
        (360, [-0.0055, 21.1910, 3.4710]),
    ];

    #[test]
    fn a_car_climbs_a_slope_like_wasm() {
        let world = ramp();
        let mut physics = Physics::new(&world).unwrap();
        let ground = world.ground.height_at(0.0, 40.0);
        physics.add_car(1, KIND_CAR, [0.0, ground + 1.5, 40.0], 0.0);
        for tick in 0..360 {
            physics.drive_car(1, FORWARD, TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, expected)) = RAMP_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "tick {done}: {position:?}");
            }
        }
        let pose = physics.car_pose(1).unwrap();
        let ground = world.ground.height_at(pose.position[0], pose.position[2]);
        assert!(pose.position[2] < 20.0, "it climbs: {pose:?}");
        assert!(pose.position[1] - ground < 1.2 && pose.position[1] - ground > 0.3, "rides on the slope: {pose:?} on {ground}");
        assert!(pose.up_y > 0.9);
    }

    #[test]
    fn settles_on_its_suspension() {
        let mut physics = with_car(&flat());
        let pose = run(&mut physics, COAST, 180);
        assert!(pose.up_y > 0.99, "{pose:?}");
        assert!(pose.position[1] > 0.5 && pose.position[1] < 1.0, "ride height {pose:?}");
        assert!(pose.forward_speed.abs() < 0.1);
    }

    #[test]
    fn accelerates_forward_to_a_sensible_speed() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, FORWARD, 60 * 6);
        assert!(pose.position[2] < -20.0, "should drive towards -Z: {pose:?}");
        assert!(pose.position[0].abs() < 1.0, "no drift sideways: {pose:?}");
        assert!(pose.forward_speed > 8.0 && pose.forward_speed <= CAR_MAX_SPEED + 1.0, "{pose:?}");
        assert!(pose.up_y > 0.98, "{pose:?}");
    }

    #[test]
    fn reverses() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: -1.0, ..COAST }, 60 * 3);
        assert!(pose.position[2] > 3.0 && pose.forward_speed < -1.0, "{pose:?}");
    }

    #[test]
    fn steering_right_turns_the_heading_clockwise() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 60 * 3);
        assert!(yaw(&pose) < -0.3, "right turn lowers the yaw: {}", yaw(&pose));
        assert!(pose.position[0] > 1.0, "ends up to the right (+X): {pose:?}");
        assert!(pose.steer > 0.0);
    }

    #[test]
    fn steering_left_mirrors_it() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: -1.0, ..CarControls::IDLE }, 60 * 3);
        assert!(yaw(&pose) > 0.3 && pose.position[0] < -1.0, "{pose:?}");
    }

    #[test]
    fn brakes_to_a_stop_and_coasts_down() {
        let mut physics = with_car(&flat());
        run(&mut physics, FORWARD, 60 * 4);
        let braked = run(&mut physics, CarControls { throttle: -1.0, ..COAST }, 60 * 3);
        assert!(braked.forward_speed.abs() < 1.0 || braked.forward_speed < 0.0, "{braked:?}");
        let mut physics = with_car(&flat());
        let moving = run(&mut physics, FORWARD, 60 * 4);
        let coasted = run(&mut physics, COAST, 60 * 10);
        assert!(coasted.forward_speed < moving.forward_speed * 0.5, "{moving:?} -> {coasted:?}");
    }

    #[test]
    fn a_wall_stops_the_car() {
        let mut physics = with_car(&world_with_wall());
        let pose = run(&mut physics, FORWARD, 60 * 12);
        assert!(pose.position[2] > -58.0 - 0.1, "stopped before the wall: {pose:?}");
        assert!(pose.position[2] < -50.0, "reached the wall: {pose:?}");
    }

    #[test]
    fn spawn_checks_see_walls() {
        let physics = Physics::new(&world_with_wall()).unwrap();
        assert!(physics.car_fits(KIND_CAR, [0.0, 1.0, 0.0], 0.0));
        assert!(!physics.car_fits(KIND_CAR, [0.0, 1.0, -58.5], 0.0));
    }

    #[test]
    fn cars_are_solid_to_characters() {
        use crate::physics::{Character, STANDING_Y};
        use shared::protocol::InputFrame;
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let mut character = Character { position: [-4.0, STANDING_Y, 0.0], grounded: true, ..Default::default() };
        // Yaw -π/2 faces +X, into the car's flank.
        let toward_car = InputFrame(1, 0, 127, -1571, 0, 0, 0, 0);
        for _ in 0..180 {
            physics.move_character(&mut character, &toward_car, TICK_DT);
        }
        let x = character.position[0];
        assert!(x < -CAR_HALF_EXTENTS[0] - CAPSULE_RADIUS + 0.1 && x > -3.0, "stopped at the car's side: {x}");
        assert!(physics.car_hit(character.position).is_none(), "resting against a car is not a hit");
    }

    #[test]
    fn a_moving_car_overlaps_a_character_in_its_path() {
        use crate::physics::STANDING_Y;
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 60);
        let in_path = [0.0, STANDING_Y, -12.0];
        let mut hit = None;
        for _ in 0..180 {
            run(&mut physics, FORWARD, 1);
            hit = hit.or(physics.car_hit(in_path));
        }
        let hit = hit.expect("the car drives into the spot");
        assert_eq!(hit.car, 1);
        assert!(hit.velocity[2] < -3.0, "{hit:?}");
    }

    fn with_tank(world: &WorldFile) -> Physics {
        let mut physics = Physics::new(world).unwrap();
        physics.add_car(1, KIND_TANK, [0.0, 1.2, 0.0], 0.0);
        physics
    }

    #[test]
    fn a_tank_settles_and_drives_slower_than_a_car() {
        let mut physics = with_tank(&flat());
        let rest = run(&mut physics, COAST, 180);
        assert!(rest.up_y > 0.99 && rest.position[1] > 0.6 && rest.position[1] < 1.3, "{rest:?}");
        let pose = run(&mut physics, FORWARD, 60 * 8);
        assert!(pose.position[2] < -20.0 && pose.position[0].abs() < 1.0, "{pose:?}");
        assert!(pose.forward_speed > 5.0 && pose.forward_speed <= TANK_MAX_SPEED + 0.5, "{pose:?}");
    }

    #[test]
    fn a_tank_turns_on_the_spot() {
        let mut physics = with_tank(&flat());
        run(&mut physics, COAST, 120);
        let start = physics.car_pose(1).unwrap();
        let right = run(&mut physics, CarControls { steer: 1.0, ..COAST }, 120);
        assert!(yaw(&right) < -0.8, "two seconds of right stick turns clockwise: {}", yaw(&right));
        let moved = (right.position[0] - start.position[0]).hypot(right.position[2] - start.position[2]);
        assert!(moved < 1.5, "pivots instead of driving a circle: {moved} m");
        let back = run(&mut physics, CarControls { steer: -1.0, ..COAST }, 120);
        assert!(yaw(&back) > yaw(&right) + 0.8, "left stick turns back: {} -> {}", yaw(&right), yaw(&back));
        // Let go and it stops turning.
        let held = run(&mut physics, COAST, 90);
        assert!((yaw(&run(&mut physics, COAST, 30)) - yaw(&held)).abs() < 0.05);
    }

    #[test]
    fn a_tank_can_turn_and_drive_together() {
        let mut physics = with_tank(&flat());
        run(&mut physics, COAST, 60);
        let pose = run(&mut physics, CarControls { throttle: 1.0, steer: 1.0, ..CarControls::IDLE }, 60 * 2);
        assert!(yaw(&pose) < -1.0 && pose.position[0] > 2.0 && pose.up_y > 0.95, "{pose:?}");
    }

    fn with_heli(height: f32) -> Physics {
        let mut physics = Physics::new(&flat()).unwrap();
        physics.add_car(1, KIND_HELI, [0.0, height, 0.0], 0.0);
        physics
    }

    const PILOTED: CarControls = CarControls { powered: true, ..CarControls::IDLE };

    /// A scripted flight for the fixture in `vehicle.spec.ts`: hover, climb, lean forward, turn, sink.
    fn scripted_flight(tick: u32) -> CarControls {
        match tick / 60 {
            0 => PILOTED,
            1 | 2 => CarControls { lift: 1.0, ..PILOTED },
            3 | 4 => CarControls { throttle: 1.0, ..PILOTED },
            5 | 6 => CarControls { throttle: 1.0, steer: 0.7, ..PILOTED },
            _ => CarControls { throttle: -0.5, steer: -1.0, lift: -1.0, ..PILOTED },
        }
    }

    /// `(tick, position)` of a helicopter started at `[0, 3, 0]`, mirrored in `vehicle.spec.ts`.
    const FLIGHT_CHECKPOINTS: [(u32, [f32; 3]); 4] = [
        (90, [0.0, 3.6099, 0.0]),
        (210, [0.0, 11.8079, -0.0842]),
        (330, [0.1181, 13.1328, -8.3432]),
        (480, [12.1913, 11.3589, -26.3039]),
    ];

    #[test]
    fn scripted_flight_matches_fixture() {
        let mut physics = with_heli(3.0);
        for tick in 0..480 {
            physics.drive_car(1, scripted_flight(tick), TICK_DT);
            physics.step_cars(TICK_DT);
            let done = tick + 1;
            if let Some((_, expected)) = FLIGHT_CHECKPOINTS.iter().find(|(t, _)| *t == done) {
                let position = physics.car_pose(1).unwrap().position;
                let close = position.iter().zip(expected).all(|(a, e)| (a - e).abs() < 0.001);
                assert!(close, "tick {done}: {position:?}");
            }
        }
    }

    #[test]
    fn a_powered_helicopter_hovers() {
        let mut physics = with_heli(10.0);
        let pose = run(&mut physics, PILOTED, 60 * 5);
        assert!((pose.position[1] - 10.0).abs() < 0.2 && pose.up_y > 0.999, "{pose:?}");
        assert!(pose.velocity.iter().all(|v| v.abs() < 0.1), "{pose:?}");
    }

    #[test]
    fn an_unpowered_helicopter_falls_and_rests() {
        let mut physics = with_heli(10.0);
        let pose = run(&mut physics, CarControls::IDLE, 60 * 4);
        assert!(pose.position[1] < 1.2 && pose.velocity[1].abs() < 0.5, "{pose:?}");
    }

    #[test]
    fn it_climbs_and_sinks_on_the_lift_input() {
        let mut physics = with_heli(10.0);
        let up = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 2);
        assert!(up.position[1] > 14.0 && up.velocity[1] > 2.0, "{up:?}");
        // It still has the speed of the climb to shed first.
        let down = run(&mut physics, CarControls { lift: -1.0, ..PILOTED }, 60 * 6);
        assert!(down.position[1] < up.position[1] - 3.0 && down.velocity[1] < -3.0, "{up:?} -> {down:?}");
    }

    #[test]
    fn leaning_forward_flies_forward_at_a_sensible_speed_without_losing_height() {
        let mut physics = with_heli(20.0);
        let pose = run(&mut physics, CarControls { throttle: 1.0, ..PILOTED }, 60 * 8);
        assert!(pose.position[2] < -60.0 && pose.position[0].abs() < 1.0, "flies towards -Z: {pose:?}");
        assert!(pose.forward_speed > 12.0 && pose.forward_speed < 30.0, "{pose:?}");
        assert!((pose.position[1] - 20.0).abs() < 2.0, "holds altitude while leaning: {pose:?}");
        assert!(pose.up_y > 0.85, "{pose:?}");
        let back = run(&mut physics, CarControls { throttle: -1.0, ..PILOTED }, 60 * 3);
        assert!(back.forward_speed < pose.forward_speed - 5.0, "{pose:?} -> {back:?}");
    }

    #[test]
    fn the_stick_turns_it_like_a_tank_clockwise_on_the_right() {
        let mut physics = with_heli(10.0);
        let right = run(&mut physics, CarControls { steer: 1.0, ..PILOTED }, 60 * 2);
        assert!(yaw(&right) < -1.0, "{}", yaw(&right));
        let left = run(&mut physics, CarControls { steer: -1.0, ..PILOTED }, 60 * 4);
        assert!(yaw(&left) > yaw(&right) + 1.0, "{} -> {}", yaw(&right), yaw(&left));
    }

    #[test]
    fn it_levels_out_after_a_knock() {
        let mut physics = with_heli(15.0);
        run(&mut physics, PILOTED, 30);
        physics.set_car_roll(1, 0.8, [0.0, 15.0, 0.0]);
        let pose = run(&mut physics, PILOTED, 60 * 3);
        assert!(pose.up_y > 0.99, "{pose:?}");
    }

    #[test]
    fn the_ceiling_stops_the_climb() {
        let mut physics = with_heli(HELI_CEILING - 30.0);
        let pose = run(&mut physics, CarControls { lift: 1.0, ..PILOTED }, 60 * 20);
        assert!(pose.position[1] > HELI_CEILING - 10.0 && pose.position[1] < HELI_CEILING + 8.0, "{pose:?}");
    }

    #[test]
    fn rays_see_walls_and_ignore_their_own_vehicle() {
        let mut physics = with_car(&world_with_wall());
        run(&mut physics, COAST, 60);
        let origin = [0.0, 1.5, 0.0];
        // Starting inside the car's own box, the ray still reaches the wall at z = -58.
        let distance = physics.cast_ray(origin, [0.0, 0.0, -1.0], 300.0, 1).expect("the wall is in range");
        assert!((distance - 58.0).abs() < 0.1, "{distance}");
        assert!(physics.cast_ray(origin, [0.0, 0.0, 1.0], 100.0, 1).is_none(), "nothing behind");
        assert!(physics.cast_ray(origin, [0.0, 0.0, -1.0], 30.0, 1).is_none(), "out of range");
        // Aimed at the car from outside, the ray stops at its side.
        let at_car = physics.cast_ray([-10.0, 0.5, 0.0], [1.0, 0.0, 0.0], 50.0, 99).expect("the car is solid to shells");
        assert!((at_car - (10.0 - CAR_HALF_EXTENTS[0])).abs() < 0.2, "{at_car}");
    }

    #[test]
    fn a_blast_shoves_a_nearby_vehicle_away() {
        let mut physics = with_car(&flat());
        run(&mut physics, COAST, 120);
        physics.blast_cars([-3.0, 0.0, 0.0], EXPLOSION_RADIUS);
        let after = run(&mut physics, COAST, 20);
        assert!(after.velocity[0] > 1.0, "pushed towards +X: {:?}", after.velocity);
        let mut far = with_car(&flat());
        run(&mut far, COAST, 120);
        far.blast_cars([-30.0, 0.0, 0.0], EXPLOSION_RADIUS);
        assert!(run(&mut far, COAST, 20).velocity[0].abs() < 0.2);
    }

    #[test]
    fn a_tanks_turret_is_solid_above_the_hull() {
        let mut physics = with_tank(&flat());
        let rest = run(&mut physics, COAST, 180);
        let [px, py, pz] = TANK_TURRET_PIVOT;
        // Above the hull's top, so only the turret can stop it. The ring sits off the hull's centre.
        let height = rest.position[1] + py;
        let hit = physics.cast_ray_hit([-10.0, height, rest.position[2] + pz], [1.0, 0.0, 0.0], 30.0, 99).expect("the turret is solid");
        assert!((hit.0 - (10.0 + px - TANK_TURRET_RADIUS)).abs() < 0.2, "{hit:?}");
        assert_eq!(hit.1, Some(1));
        // The gun's own tank does not stop its shell.
        assert!(physics.cast_ray([-10.0, height, rest.position[2] + pz], [1.0, 0.0, 0.0], 30.0, 1).is_none());
        // A standing character on the roof of the turret is carried by the tank.
        let on_top = [rest.position[0] + px, rest.position[1] + py + TANK_TURRET_HALF_HEIGHT + crate::physics::STANDING_Y - 0.05, rest.position[2] + pz];
        assert_eq!(physics.car_under(on_top).map(|hit| hit.car), Some(1));
    }
}
