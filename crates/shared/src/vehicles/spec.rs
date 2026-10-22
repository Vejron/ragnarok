//! The tuning of a vehicle kind that the physics (`crates/sim`) reads: its chassis, wheels, suspension
//! and what holds it up.

/// What holds a vehicle up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lift {
    /// Its wheels, and nothing else.
    Wheels,
    /// A rotor, with no wheels at all.
    Rotor,
    /// Wheels on the ground and wings in the air, with a propeller to push it.
    Wing,
    /// Two spring legs that are ray casts, and a torque that keeps the hull upright.
    Walker,
    /// Four small rotors that hold it up and turn it towards where its pilot looks.
    Drone,
    /// A loitering drone's motor and wings: always cruising forward, turning towards the look.
    Loiter,
}

impl Lift {
    /// Whether the kind has a wheel controller to update.
    pub fn wheeled(self) -> bool {
        matches!(self, Lift::Wheels | Lift::Wing)
    }
}

/// How the physics moves a kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    /// Half extents of the chassis box, metres.
    pub half_extents: [f32; 3],
    /// Chassis density in kg/m³.
    pub density: f32,
    pub wheel_radius: f32,
    pub track_half: f32,
    /// Z of each axle, front first.
    pub axles: &'static [f32],
    pub hard_point_y: f32,
    pub suspension_rest: f32,
    pub max_suspension_force: f32,
    /// Total engine force over all wheels, newtons.
    pub engine_force: f32,
    pub max_speed: f32,
    /// Steering lock of the front axle at standstill, radians. Zero for a tank, which turns by `turn_rate`.
    pub steer_lock: f32,
    /// Yaw rate at full stick for a vehicle that turns on the spot, rad/s.
    pub turn_rate: f32,
    pub lift: Lift,
    pub linear_damping: f32,
    /// How far below the box's centre the centre of mass sits, metres. A low one keeps a car on its
    /// wheels through hard corners; zero leaves the box's own.
    pub com_drop: f32,
    /// A full tank of what its tricks run on: a Roadster's air, or a hen's wings' stamina. `None` for
    /// everything else. A walker with one has wings (see `flap`).
    pub tank: Option<f32>,
    /// What its driver's buttons do (see `CarControls::driven`).
    pub buttons: Buttons,
    /// It flies towards where its pilot looks, which the controls carry as `aim`.
    pub aims: bool,
    /// What it carries beside its hull box.
    pub solids: &'static [Solid],
}

/// What a driver's buttons do, beyond the stick (steer and throttle) every vehicle takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Buttons {
    /// Jump is the handbrake; a helicopter also climbs on jump and sinks on run.
    Drive,
    /// A Roadster's: run boosts, jump hops (then hovers), and crouch is the handbrake.
    Stunt,
    /// A hen's: jump leaps, then flaps or glides. There is no handbrake.
    Leap,
}

/// A solid a vehicle carries beside its hull box, fixed to the hull at `at` (hull frame). It adds no
/// mass, which keeps the hull's balance and the tuning that goes with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub shape: Shape,
    pub at: [f32; 3],
    pub role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// Upright, so a turret needs no turning with its gun.
    Cylinder { half_height: f32, radius: f32 },
    Cuboid([f32; 3]),
}

/// What else a solid is, beyond something to hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Part,
    /// It can be shot off (the sim's `Physics::set_turret`), and a strike on it is a turret hit.
    Turret,
    /// Wings: kept clear of the world when the vehicle is placed (the sim's `Physics::car_fits`).
    Wings,
}
