//! The tuning of a vehicle kind that the physics (`crates/sim`) reads: its chassis, wheels, suspension
//! and what holds it up.

use super::walker::*;

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
    /// The hull's side outline, for a kind whose hull is not a box: the corners of a convex shape as
    /// `[z, y]` in the hull frame, metres, which runs the hull's whole width. The physics drives on it and
    /// a round meets its faces as they lie, so a glacis slopes as it is drawn. Its bounding box is
    /// `half_extents`. `Spec::BOX` (empty) for a plain box.
    pub profile: &'static [[f32; 2]],
    /// How its wheels' springs give: firm on most, soft and long on a truck built for rough ground.
    pub springs: Springs,
    /// How its wings fly, for a `Lift::Wing` kind. `None` for everything else.
    pub airframe: Option<Airframe>,
    /// What holds back a kind that slides on runners (a sled): its friction and its air drag, which
    /// set how fast it runs downhill. `None` for a kind on wheels, which rolls with only a light brake
    /// when coasting.
    pub runners: Option<Runners>,
    /// How its legs carry it, for a `Lift::Walker` kind. `None` for everything else.
    pub gait: Option<Gait>,
    /// Its two arms, whose fists its pilot moves. `None` for everything but a Titan.
    pub arms: Option<Arms>,
}

impl Spec {
    /// The `profile` of a hull that is its plain box.
    pub const BOX: &'static [[f32; 2]] = &[];
}

/// A giant's two arms. Each fist is a ball the pilot moves about its shoulder: it goes where it is
/// asked to within the arm's reach, no faster than the arm can swing. The arm between is only drawn.
/// Points are in the hull frame for the right arm; the left mirrors them in X.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arms {
    /// Where the right shoulder is.
    pub shoulder: [f32; 3],
    /// The furthest and the nearest a fist's middle can be from its shoulder, metres.
    pub reach: f32,
    pub min_reach: f32,
    /// A fist's radius, metres.
    pub fist: f32,
    /// A fist's top speed against the hull, m/s, and how hard it speeds up and slows, m/s².
    pub speed: f32,
    pub accel: f32,
    /// Where the right fist hangs when nobody asks anything of it.
    pub rest: [f32; 3],
    /// The heaviest vehicle a closed fist picks up, kilograms.
    pub lift: f32,
    /// The arm's bones as they are drawn, shoulder to elbow and elbow to the fist's middle, metres, and
    /// how thick each is to a round, as the radius of a capsule along it.
    pub upper: f32,
    pub fore: f32,
    pub upper_radius: f32,
    pub fore_radius: f32,
    /// Which way the right elbow bends (see `titan::bend`).
    pub elbow_pole: [f32; 3],
}

/// A giant's feet, which are solid and which it stands on: a column for each shin. A planted foot does
/// not move, and the hull is carried over it at the end of a leg of fixed length, so it rises as it
/// passes over the foot and sinks into the next step. The feet step in turn: the weight goes onto one,
/// the other swings ahead to where the hull is going, and the weight follows it. See `walk` and
/// `step_limbs` in `crates/sim`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feet {
    /// Half extents of a shin's column, metres. It stands on the ground.
    pub half: [f32; 3],
    /// The longest step, from where a foot lifts to where it lands as the hull sees it, metres, and the
    /// shortest it takes when it is walked.
    pub stride: f32,
    pub stride_min: f32,
    /// How long a foot is in the air, seconds, and how high it lifts in the middle of that, metres.
    pub step_time: f32,
    pub lift: f32,
    /// How long the weight takes to go from one foot to the other, seconds: both are on the ground.
    pub shift_time: f32,
    /// How far towards the foot that carries it the hull moves, as a share of the way from the middle
    /// of the two, and how hard it is pulled there, 1/s².
    pub sway: f32,
    pub sway_gain: f32,
    /// How far the hull leans towards the foot that carries it, radians.
    pub tilt: f32,
    /// How much slower it goes over a foot than between two, as a share of its speed.
    pub pulse: f32,
    /// The share of its speed it keeps when it is crouched right down.
    pub crouch_pace: f32,
    /// The leg's bones as they are drawn, hip to knee and knee to ankle, metres, how far the ankle is
    /// above the sole, and how thick each bone is to a round, as the radius of a capsule along it.
    pub thigh: f32,
    pub shin: f32,
    pub ankle: f32,
    pub thigh_radius: f32,
    pub shin_radius: f32,
    /// Which way the knees bend (see `titan::bend`).
    pub knee_pole: [f32; 3],
}

/// Thrusters in a legged kind's feet: it leaps from the ground, and they carry it up and about while
/// its pilot holds jump and the fuel in `Spec::tank` lasts. See `jet` in `crates/sim`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jets {
    /// The upward speed a leap gives, m/s, the speed along the heading a full stick adds to it, and
    /// what it costs of the tank.
    pub leap: f32,
    pub leap_forward: f32,
    pub leap_cost: f32,
    /// The climb the thrusters hold, m/s, how hard they chase it, 1/s, and the most they add to or take
    /// from holding its weight, m/s².
    pub climb: f32,
    pub gain: f32,
    pub accel: f32,
    /// What a second of thrust costs of the tank, and what a second on the ground gives back.
    pub drain: f32,
    pub refill: f32,
    /// How fast they carry it along its heading with the stick, m/s, and how hard they push to, m/s².
    pub speed: f32,
    pub push: f32,
    /// In the air: how far it leans with the stick, radians; how hard it is brought level, 1/s; how
    /// quickly its turning follows, 1/s; and how fast it turns, rad/s.
    pub tilt: f32,
    pub level: f32,
    pub response: f32,
    pub yaw: f32,
    /// How high over the ground the thrusters carry it, metres: the climb fades out below this.
    pub ceiling: f32,
}

/// Where a kind with `Feet` is in its walk. Foot 0 is the left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Both feet are planted, and no step is due.
    Planted,
    /// Both are planted, and the weight is coming off this foot for it to step.
    Shifting(usize),
    /// This foot is in the air, this far through its step, 0..1.
    Swinging(usize, f32),
    /// Both feet are off the ground, hanging under it: it has leapt, or is flying or falling.
    Airborne,
}

impl Step {
    /// The step as a replicated component carries it: 0 planted, 1..=100 the left foot that far through
    /// its step and 101..=200 the right, 201 and 202 the weight coming off the left and the right, 203
    /// off the ground.
    pub fn to_byte(self) -> u8 {
        match self {
            Step::Planted => 0,
            Step::Swinging(foot, progress) => 1 + 100 * (foot.min(1) as u8) + (progress.clamp(0.0, 1.0) * 99.0).round() as u8,
            Step::Shifting(foot) => 201 + foot.min(1) as u8,
            Step::Airborne => 203,
        }
    }

    /// The step a byte from `to_byte` stands for.
    pub fn from_byte(byte: u8) -> Step {
        match byte {
            0 => Step::Planted,
            1..=100 => Step::Swinging(0, f32::from(byte - 1) / 99.0),
            101..=200 => Step::Swinging(1, f32::from(byte - 101) / 99.0),
            201 => Step::Shifting(0),
            202 => Step::Shifting(1),
            _ => Step::Airborne,
        }
    }

    /// The foot that is in the air, and how far through its step it is.
    pub fn swinging(self) -> Option<(usize, f32)> {
        match self {
            Step::Swinging(foot, progress) => Some((foot, progress)),
            _ => None,
        }
    }
}

/// How a legged kind (`Lift::Walker`) stands, balances and walks: two spring legs that are ray casts
/// from its hips, a torque that keeps its hull upright and a push that walks it. See `walk` in
/// `crates/sim`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gait {
    /// Distance of each foot from the middle of the hull, sideways, metres.
    pub foot_half: f32,
    /// How long a leg is from the bottom of the hull to the ground when it stands, metres.
    pub leg_length: f32,
    /// How much shorter its legs are when it crouches, metres. 0 for a kind that cannot.
    pub crouch: f32,
    /// How far its hull leans forward when it crouches, as a tilt (up's horizontal part).
    pub crouch_lean: f32,
    /// Top speed forwards, m/s, and the share of it backwards.
    pub max_speed: f32,
    pub reverse_share: f32,
    /// Acceleration towards the speed asked for, and deceleration when it slows or reverses, m/s².
    pub accel: f32,
    pub brake: f32,
    /// Planted feet do not slide sideways: the push against it, per m/s of sideways speed, and its cap, m/s².
    pub grip_gain: f32,
    pub grip: f32,
    /// Yaw rate at full stick, rad/s, and the angular acceleration towards it, rad/s².
    pub turn_rate: f32,
    pub turn_accel: f32,
    /// Leg spring and damper, per kilogram of the whole body for the pair: the spring settles at
    /// `CAR_GRAVITY / spring` of compression.
    pub spring: f32,
    pub damping: f32,
    /// How hard the hull is pulled upright, rad/s² per radian of tilt, the damping on its tilt rate, per
    /// second, and the most the pull can be, rad/s²: sideways, and pitching, which it fights best.
    pub balance_gain: f32,
    pub balance_damp: f32,
    pub balance_max: f32,
    pub balance_max_pitch: f32,
    /// The catching step: a push towards where the top is falling, m/s² per unit of lean and per rad/s
    /// of fall, at most `catch_max`, beyond the lean and fall rate it tolerates.
    pub catch_gain: f32,
    pub catch_damp: f32,
    pub catch_max: f32,
    pub catch_lean_margin: f32,
    pub catch_fall_margin: f32,
    /// Extra lean into its heading at top speed, as a tilt, and how much of the tilt that would match
    /// its push it leans by.
    pub cruise_lean: f32,
    pub accel_lean: f32,
    /// A blast pushes it at its shoulders, this far above the middle of the hull, metres, so it tips it.
    pub blast_lever: f32,
    /// Below this `up_y` it has fallen, and the share of the full pull that stands it up again.
    pub fallen_up: f32,
    pub getup_share: f32,
    /// The fastest its legs straighten, m/s, as it stands up from a crouch: the spring pushes no harder
    /// than holds that speed. 0 for legs that push as hard as they are bent.
    pub rise: f32,
    /// The share of a blast's shove it takes: 1 for a walker, less for something far heavier.
    pub blast_share: f32,
    /// The share of the turn a round's knock gives where it strikes (`Physics::strike_car`) that it
    /// takes: 1 for a walker. Of the knock's push it takes `blast_share`, so a giant that takes little of
    /// the push and more of the turn rocks where it stands rather than sliding back.
    pub hit_share: f32,
    /// Its feet, when they are solids of their own. `None` leaves the legs to the hull's `solids`.
    pub feet: Option<Feet>,
    /// Thrusters in its feet, if it has them.
    pub jets: Option<Jets>,
}

impl Gait {
    /// The walker's legs, which the hen and the tupp walk on too: the `WALKER_*` constants.
    pub const WALKER: Gait = Gait {
        foot_half: WALKER_FOOT_HALF,
        leg_length: WALKER_LEG_LENGTH,
        crouch: 0.0,
        crouch_lean: 0.0,
        max_speed: WALKER_MAX_SPEED,
        reverse_share: WALKER_REVERSE_SHARE,
        accel: WALKER_ACCEL,
        brake: WALKER_BRAKE,
        grip_gain: WALKER_GRIP_GAIN,
        grip: WALKER_GRIP,
        turn_rate: WALKER_TURN_RATE,
        turn_accel: WALKER_TURN_ACCEL,
        spring: WALKER_LEG_SPRING,
        damping: WALKER_LEG_DAMPING,
        balance_gain: WALKER_BALANCE_GAIN,
        balance_damp: WALKER_BALANCE_DAMP,
        balance_max: WALKER_BALANCE_MAX,
        balance_max_pitch: WALKER_BALANCE_MAX_PITCH,
        catch_gain: WALKER_CATCH_GAIN,
        catch_damp: WALKER_CATCH_DAMP,
        catch_max: WALKER_CATCH_MAX,
        catch_lean_margin: WALKER_CATCH_LEAN_MARGIN,
        catch_fall_margin: WALKER_CATCH_FALL_MARGIN,
        cruise_lean: WALKER_CRUISE_LEAN,
        accel_lean: WALKER_ACCEL_LEAN,
        blast_lever: WALKER_BLAST_LEVER,
        fallen_up: WALKER_FALLEN_UP,
        getup_share: WALKER_GETUP_SHARE,
        rise: 0.0,
        blast_share: 1.0,
        hit_share: 1.0,
        feet: None,
        jets: None,
    };
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
    /// A Titan's: crouch bends its legs, which brings its fists down to the ground, and jump leaps, then
    /// fires the thrusters in its feet.
    Stride,
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
    /// Upright: a cylinder `half_height` either side of its middle with a ball of `radius` on each end,
    /// metres.
    Capsule { half_height: f32, radius: f32 },
    /// A box that narrows or widens upwards, as the Titan's builder's `taper` does: half its width (X)
    /// and its depth (Z) at the top and at the bottom, and half its height, metres.
    Frustum { top: [f32; 2], bottom: [f32; 2], half_height: f32 },
}

/// What else a solid is, beyond something to hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Part,
    /// It can be shot off (the sim's `Physics::set_turret`), and a strike on it is a turret hit.
    Turret,
    /// Wings: kept clear of the world when the vehicle is placed (the sim's `Physics::car_fits`).
    Wings,
    /// Only something for a round to hit: it is not in the physics at all, so it pushes nothing and
    /// nothing pushes it. A kind with any of these is shot at them (and at its limbs) instead of at its
    /// hull and its other solids, which are left to stand and walk on: see the sim's
    /// `Physics::armor_strike`.
    Hit,
}
