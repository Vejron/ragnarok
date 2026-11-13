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
    /// A hull that floats on open water and a sail the wind fills (`crate::wind`), steered by its
    /// rudder. Aground, nothing drives it.
    Sail,
    /// A pressure hull that floats on the surface or holds any depth below it, driven by its motor,
    /// steered by its rudder, and taken up or down by its captain.
    Dive,
    /// A guided missile's motor and fins: always driving forward fast, turning towards the look.
    Missile,
    /// Nothing of its own: it is fixed to another vehicle (`Fitting`) and carried with it.
    Mounted,
}

impl Lift {
    /// Whether the kind has a wheel controller to update.
    pub fn wheeled(self) -> bool {
        matches!(self, Lift::Wheels | Lift::Wing)
    }
}

/// The speed at which a car's steering lock has halved, m/s: the usual `Spec::steer_fade`.
pub const STEER_FADE: f32 = 20.0;

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
    /// Where a car's tow joint holds it, as the Z of its towing eye on its centre line (hull frame): a
    /// mortar's tongue ahead of it, a Pak's trails behind. 0 for a kind nobody tows.
    pub tongue: f32,
    /// How much of its rear wheels' side grip a hard turn takes away, 0..1: they let go and the tail
    /// slides round, more the faster it goes and the more it is driven. 0 for a car that grips.
    pub slide: f32,
    /// How much of the true leverage its wheels' side grip has to roll it over the physics gives back,
    /// 0..1. The ray-cast wheels push sideways from near the centre of mass, so a car corners flat
    /// and never rolls; a kind with this tips over when it corners harder than its stance can carry.
    pub roll: f32,
    /// How far back from the front its hull's underside sweeps up to half its height, metres, as a
    /// sled's does over its upturned skis: it rides up onto a slope instead of digging its nose in.
    /// 0 for a plain box.
    pub nose: f32,
    /// Seconds its steering takes to swing from straight to full lock, so that a key held down eases
    /// into a turn rather than snapping to it. 0 swings at the usual rate, which is all but at once.
    pub steer_time: f32,
    /// The speed at which its steering lock has halved, m/s: the lock is `steer_lock / (1 + speed /
    /// steer_fade)`, so a smaller number steers less the faster it goes. `STEER_FADE` for most kinds.
    pub steer_fade: f32,
    /// How it does on open water, for a kind light and quick enough to plane over it. `None` sinks
    /// straight to the bed and wades there, as every other wheeled kind does.
    pub skim: Option<Skim>,
    /// How far back from the front its hull's sides taper in to the full width, metres, as a ship's
    /// bow does to its stem. 0 for a square front.
    pub bow: f32,
    /// How its wheels' springs give: firm on most, soft and long on a truck built for rough ground.
    pub springs: Springs,
    /// How its wings fly, for a `Lift::Wing` kind. `None` for everything else.
    pub airframe: Option<Airframe>,
    /// What holds back a kind that slides on runners (a sled): its friction and its air drag, which
    /// set how fast it runs downhill. `None` for a kind on wheels, which rolls with only a light brake
    /// when coasting.
    pub runners: Option<Runners>,
}

/// A kind on runners (a sled): what holds it back as it slides, in place of a wheel's rolling. It is
/// the soldier's skis' model (`SKI_MU_SNOW`, `SKI_DRAG`), so down a slope it reaches the speed at which
/// gravity's pull, `g·(sin θ − mu·cos θ)`, equals `drag·v²`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Runners {
    /// Sliding friction on snow: the share of its weight on the ground that holds it back.
    pub mu: f32,
    /// Air drag per (m/s)², m/s² for every (m/s)² of speed. Felt in the air as well as on the ground.
    pub drag: f32,
    /// In the air its nose turns towards the way it is flying, and its roll back to level, at this
    /// rate, per second: it sails off a bump nose first rather than diving or tumbling.
    pub follow: f32,
    /// How far forward (W) or back (S) the rider leans its nose off that line in the air, radians.
    pub lean: f32,
    /// How fast the rider turns it in the air at full stick, rad/s, banking into the turn.
    pub air_turn: f32,
    /// The share of its weight it is spared in the air: it floats further off a bump than it would fall.
    pub float: f32,
}

/// How a winged kind (`Lift::Wing`) flies: its engine, its wings and how fast it turns. What every
/// aeroplane shares (the autopilot, the throttle's settings, the ceiling) is the `PLANE_*` constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Airframe {
    /// Acceleration the engine gives at full throttle, m/s².
    pub thrust: f32,
    /// Air drag per (m/s)²: `thrust / drag` is the square of its top speed in level flight.
    pub drag: f32,
    /// Below this airspeed (m/s) the wings stall: their lift fades out over the last 30% of it.
    pub stall_speed: f32,
    /// Above this airspeed the wings carry it level by themselves; below it they carry less, by the square.
    pub level_speed: f32,
    /// The speed the controls are tuned for: they lose authority below it, by its square.
    pub cruise_speed: f32,
    /// Turn rates at full deflection, rad/s.
    pub pitch_rate: f32,
    pub roll_rate: f32,
    pub yaw_rate: f32,
    /// The most the wings can pull, m/s².
    pub max_lift: f32,
    /// Pontoons that carry it on open water. `None` drops through the surface to the bed.
    pub floats: Option<Floats>,
}

/// A seaplane's pontoons: where they are under the hull and how deep they sit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Floats {
    /// How far below the hull's centre the pontoons' undersides are, metres.
    pub keel: f32,
    /// How deep their undersides sit in still water with the seaplane at rest, metres.
    pub draft: f32,
    /// How tall a pontoon is, metres: under by more than this it holds no harder.
    pub depth: f32,
}

/// A wheeled kind's springs and dampers, which ride on the wheel's ray cast. The stiffness and the
/// damping are per unit of the chassis' mass, so a heavier hull sits no lower on the same springs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Springs {
    pub stiffness: f32,
    /// Damping as the spring is pushed in, and as it springs back out.
    pub compression: f32,
    pub damping: f32,
    /// How far, metres, a wheel can be pushed up from its rest length.
    pub travel: f32,
}

impl Springs {
    /// What every car rode on before there was a choice: stiff, with a short travel.
    pub const FIRM: Springs = Springs { stiffness: 25.0, compression: 2.2, damping: 3.0, travel: 0.3 };
}

/// How a wheeled kind skims over water: held up on the surface above `plane_speed`, sinking below
/// `sink_speed`, and between the two partly held. On the surface its wheels touch nothing, so the
/// water pushes and steers it instead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skim {
    /// Below this forward speed, m/s, nothing holds it up.
    pub sink_speed: f32,
    /// From this forward speed, m/s, it rides on the surface.
    pub plane_speed: f32,
    /// The share of its engine's pull it still has on the surface.
    pub thrust: f32,
    /// How much of its sideways sliding the water takes out each second: low drifts.
    pub grip: f32,
    /// How much speed a hard turn costs on the surface: the more, the sooner carving drops it in.
    pub bank_drag: f32,
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
    /// A snowmobile's: run boosts the engine from its `tank` while it lasts, and jump is the handbrake.
    Sled,
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
