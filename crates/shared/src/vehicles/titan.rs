//! The Titan: everything the game knows about it.

use super::*;

/// The Titan (`KIND_TITAN`): a giant robot about fifty metres high, one pilot sealed in its head. Its
/// body is a single box, its torso, held up by two spring legs that are ray casts to the ground and
/// kept upright by a torque, exactly as a walker's is (its `Gait`). Its pilot moves its two fists
/// (`TITAN_ARMS`): they hit what they meet and hold what they close on. The right one holds a
/// three-barrel rotary cannon that fires heavy rounds along the forearm, and in the top of its left shoulder block are four vertical tubes
/// that send up the submarine's guided missile (`KIND_MISSILE`), which the pilot then flies. Its feet are solid
/// and crush what they come down on. Rounds are not shot at the box but at the model's own shape: its
/// torso's `Role::Hit` solids, and its arms and legs bent as the client draws them (`bend`).
pub const TITAN_SEATS: usize = 1;
/// Half the torso's box, metres. The hull frame has its origin at the box's centre.
pub const TITAN_HALF_EXTENTS: [f32; 3] = [6.0, 8.0, 4.0];
/// Distance of each foot from the middle of the hull, sideways, metres, and how long a leg is from the
/// bottom of the torso to the ground when it stands.
pub const TITAN_FOOT_HALF: f32 = 5.0;
pub const TITAN_LEG_LENGTH: f32 = 26.0;
/// How much shorter its legs are when it crouches, metres, and how far it leans forward then, as a tilt.
pub const TITAN_CROUCH: f32 = 16.0;
pub const TITAN_CROUCH_LEAN: f32 = 0.3;
/// How fast it stands up from a crouch, m/s.
pub const TITAN_RISE: f32 = 7.0;
/// Top speed, m/s, and its yaw rate at full stick, rad/s.
pub const TITAN_MAX_SPEED: f32 = 11.0;
pub const TITAN_TURN_RATE: f32 = 0.6;
/// The leg spring, per kilogram of the body: soft, so it settles `CAR_GRAVITY / TITAN_LEG_SPRING` into
/// its legs and takes a second to crouch or to stand.
pub const TITAN_LEG_SPRING: f32 = 12.0;
/// Where the hull's centre settles over flat ground when it stands, metres. Measured by
/// `a_titan_stands_fifty_metres_high_on_its_legs`.
pub const TITAN_RIDE_HEIGHT: f32 = 33.2;
/// Where the right shoulder is in the hull frame; the left is its mirror in X.
pub const TITAN_SHOULDER: [f32; 3] = [8.5, 6.0, 0.0];
/// The furthest and the nearest a fist's middle can be from its shoulder, metres, and a fist's radius.
pub const TITAN_REACH: f32 = 24.0;
pub const TITAN_MIN_REACH: f32 = 6.0;
pub const TITAN_FIST_RADIUS: f32 = 2.5;
/// A fist's top speed against the hull, m/s, and how hard it speeds up and slows, m/s².
pub const TITAN_FIST_SPEED: f32 = 45.0;
pub const TITAN_FIST_ACCEL: f32 = 140.0;
/// Where the right fist hangs when nobody asks anything of it, in the hull frame.
pub const TITAN_FIST_REST: [f32; 3] = [10.0, -15.0, -3.0];
/// The heaviest vehicle a closed fist picks up, kilograms: a tank, but not a ship.
pub const TITAN_LIFT_MASS: f32 = 60_000.0;
/// Half extents of a shin's column, metres, which stands where the foot is planted.
pub const TITAN_SHIN_HALF: [f32; 3] = [2.2, 6.0, 3.2];
/// The fuel its thrusters burn, as `Spec::tank`: 1 is full.
pub const TITAN_FUEL: f32 = 1.0;
/// The upward speed its leap gives, the climb its thrusters hold and how fast they carry it along, m/s,
/// and how high over the ground they take it, metres.
pub const TITAN_LEAP_SPEED: f32 = 14.0;
pub const TITAN_JET_CLIMB: f32 = 8.0;
pub const TITAN_JET_SPEED: f32 = 20.0;
pub const TITAN_CEILING: f32 = 120.0;
/// Coming down at this speed or faster, m/s, its landing is a slam, which is at its worst at
/// `TITAN_SLAM_FULL`.
pub const TITAN_SLAM_SPEED: f32 = 8.0;
pub const TITAN_SLAM_FULL: f32 = 35.0;
/// How far out a slam reaches, metres, at `TITAN_SLAM_SPEED` and at `TITAN_SLAM_FULL`.
pub const TITAN_SLAM_RADIUS: f32 = 20.0;
pub const TITAN_SLAM_RADIUS_FULL: f32 = 50.0;
/// The speed a soldier or a vehicle at the middle of the worst slam is thrown outward with, m/s, and
/// the health each loses there. Both fall away to nothing at its edge.
pub const TITAN_SLAM_KNOCK: f32 = 22.0;
pub const TITAN_SLAM_DAMAGE: f32 = 160.0;
pub const TITAN_SLAM_VEHICLE_DAMAGE: f32 = 220.0;

/// Its longest step and the shortest it walks with, metres; how long a foot is in the air and how long
/// the weight takes to go from one foot to the other, seconds; and how high a foot lifts in the middle
/// of a step, metres. A leg 26 m long swings no faster than this: the slowness is what makes it big.
pub const TITAN_STRIDE: f32 = 18.0;
pub const TITAN_STRIDE_MIN: f32 = 10.0;
pub const TITAN_STEP_TIME: f32 = 1.3;
pub const TITAN_SHIFT_TIME: f32 = 0.35;
pub const TITAN_STEP_LIFT: f32 = 4.5;
/// Half extents of each thigh, a solid on the hull that hangs from the hip, metres.
pub const TITAN_THIGH_HALF: [f32; 3] = [2.0, 3.5, 2.4];
/// The bones of its limbs as the model is built (`UPPER`, `FORE`, `THIGH`, `SHIN` and `PAD` in
/// `tools/vehicles/build_titan.py`): shoulder to elbow, elbow to the fist's middle, hip to knee and knee
/// to ankle, metres, and how far the ankle is above the sole.
pub const TITAN_UPPER_ARM_LENGTH: f32 = 12.0;
pub const TITAN_FOREARM_LENGTH: f32 = 12.0;
pub const TITAN_THIGH_LENGTH: f32 = 13.5;
pub const TITAN_SHIN_LENGTH: f32 = 13.5;
pub const TITAN_ANKLE_HEIGHT: f32 = 1.5;
/// Which way the right elbow bends, in the hull frame: out, down and back. The left mirrors it in X.
pub const TITAN_ELBOW_POLE: [f32; 3] = [0.7, -0.5, 0.6];
/// Which way the knees bend, in the hull frame: forward, a little down.
pub const TITAN_KNEE_POLE: [f32; 3] = [0.0, -0.15, -1.0];
/// How thick each bone is to a round, as the radius of a capsule along it, metres: the upper arm, the
/// forearm, the thigh and the shin with their plates.
pub const TITAN_UPPER_ARM_RADIUS: f32 = 2.1;
pub const TITAN_FOREARM_RADIUS: f32 = 2.5;
pub const TITAN_THIGH_RADIUS: f32 = 2.6;
pub const TITAN_SHIN_RADIUS: f32 = 2.7;
/// Where it is boarded from and where its pilot is set down, in the hull frame: on the ground between
/// its feet, and ahead of them.
pub const TITAN_BOARD: [f32; 3] = [0.0, -32.2, 0.0];
pub const TITAN_EXIT: [f32; 3] = [0.0, -32.2, -10.0];
/// The pilot's eye, in the hull frame: at the visor, just clear of the head's own plates.
pub const TITAN_EYE: [f32; 3] = [0.0, 12.0, -3.2];
/// The neck its head turns on, in the hull frame: the middle of the collar's top.
pub const TITAN_HEAD_PIVOT: [f32; 3] = [0.0, 9.4, 0.0];
/// How far its head turns to follow its pilot's look, radians: to either side of the way the hull
/// faces, down and up.
pub const TITAN_HEAD_TURN: f32 = 1.0;
pub const TITAN_HEAD_DOWN: f32 = 1.2;
pub const TITAN_HEAD_UP: f32 = 0.7;
/// How fast its head turns, rad/s.
pub const TITAN_HEAD_SLEW: f32 = 4.0;
/// A fist moving at least this fast, m/s, hits what it meets; slower, it only shoves.
pub const TITAN_PUNCH_SPEED: f32 = 8.0;
/// What a punch takes off a soldier, per m/s of the fist's speed, and off a vehicle.
pub const TITAN_PUNCH_DAMAGE: f32 = 6.0;
pub const TITAN_PUNCH_VEHICLE_DAMAGE: f32 = 4.0;
/// How far around a foot coming down soldiers are knocked off their feet, metres, and how hard they
/// are thrown at its edge, m/s. Under the foot itself they are crushed.
pub const TITAN_STOMP_RADIUS: f32 = 14.0;
pub const TITAN_STOMP_KNOCK: f32 = 9.0;
/// What a foot coming down takes off a vehicle under it.
pub const TITAN_STOMP_VEHICLE_DAMAGE: f32 = 120.0;
/// A soldier thrown from a fist keeps this share of the fist's speed.
pub const TITAN_THROW_SHARE: f32 = 1.0;

/// The fist that holds the rotary cannon: the right (0 is the left).
pub const TITAN_GUN_HAND: usize = 1;
/// How far the gun's muzzles are ahead of the fist's middle along the forearm, metres.
pub const TITAN_GUN_LENGTH: f32 = 8.0;
/// Its barrels, which turn a third of a turn round the forearm's line between rounds, and how far each
/// is from that line, metres. Its rounds leave on the line itself.
pub const TITAN_GUN_BARRELS: u32 = 3;
pub const TITAN_GUN_BARREL_OFFSET: f32 = 0.75;
/// Ticks between rounds: two and a half a second.
pub const TITAN_GUN_COOLDOWN: u32 = 24;
/// How far a round strays from the bore, radians: a fist is no turret.
pub const TITAN_GUN_SPREAD: f32 = 0.006;
/// Heat a round puts in the barrels, of 255: about eight seconds of holding the trigger jams it.
pub const TITAN_GUN_HEAT: f32 = 37.0;
/// How fast its round leaves the muzzle, m/s: slower than a tank's shell, so it is seen to fly.
pub const TITAN_ROUND_SPEED: f32 = 110.0;
/// Its round's burst (`PROJECTILE_ROTARY`): how far it reaches, metres, what it takes off at the middle
/// and how hard it throws, m/s. Bigger than a tank's armour-piercing shell's, but a little over a third
/// of a mortar bomb's reach, so it bites a wall about 1.8 m round (`CARVE_REACH`) to the mortar's 5 m.
pub const TITAN_ROUND_RADIUS: f32 = 4.0;
pub const TITAN_ROUND_DAMAGE: f32 = 110.0;
pub const TITAN_ROUND_KNOCK: f32 = 14.0;
/// The crater it digs, radius and depth, metres, and how hard it knocks a vehicle it strikes, m/s.
pub const TITAN_ROUND_CRATER: [f32; 2] = [3.0, 0.5];
pub const TITAN_ROUND_HIT_KNOCK: f32 = 0.35;
/// The missile tubes: how many, and where each one's mouth is, in the hull frame. They are hatches set
/// in the top of the left shoulder block (`TUBES` in `tools/vehicles/build_titan.py`).
pub const TITAN_TUBES: u8 = 4;
pub const TITAN_TUBE_AT: [[f32; 3]; 4] = [[-9.6, 11.3, -1.0], [-7.6, 11.3, -1.0], [-9.6, 11.3, 1.0], [-7.6, 11.3, 1.0]];
/// Ticks to load one more tube: as long as a submarine's tube takes.
pub const TITAN_TUBE_RELOAD: u32 = 20 * TICK_RATE;
/// Ticks between launches, at the least.
pub const TITAN_TUBE_COOLDOWN: u32 = TICK_RATE;

/// The thighs: solids on the hull, from the hips down. They stop short of the knee, so that crouched
/// they clear the ground; below them the shins are the feet's own solids (`TITAN_FEET`). They are for
/// the physics only: rounds meet the legs as the model bends them.
///
/// After them, the torso as the model is built (`torso` in `tools/vehicles/build_titan.py`, its Blender
/// (x, y, z) the game's (x, z, -y)), which only rounds meet (`Role::Hit`): the chest that narrows to the
/// waist, the two plates on its front, the waist and its ribs, the pelvis and the codpiece, the flanks,
/// the collar and the plates from it up to the shoulders, the head with its cheek guards, face and fin,
/// the block over each shoulder and the skirt hanging from its outer edge, and the reactor on the back
/// with its two stacks. The box between the shoulders that it walks with is narrower than the shoulder
/// blocks, shorter than the head and wider than the waist.
const SOLIDS: [Solid; 24] = [
    Solid { shape: Shape::Cuboid(TITAN_THIGH_HALF), at: [-TITAN_FOOT_HALF, -TITAN_HALF_EXTENTS[1] - TITAN_THIGH_HALF[1], 0.0], role: Role::Part },
    Solid { shape: Shape::Cuboid(TITAN_THIGH_HALF), at: [TITAN_FOOT_HALF, -TITAN_HALF_EXTENTS[1] - TITAN_THIGH_HALF[1], 0.0], role: Role::Part },
    // The chest.
    Solid { shape: Shape::Frustum { top: [6.8, 4.2], bottom: [4.5, 3.2], half_height: 3.8 }, at: [0.0, 4.2, 0.0], role: Role::Hit },
    // The chest plates, standing proud of the chest's front.
    Solid { shape: Shape::Cuboid([6.9, 2.4, 0.7]), at: [0.0, 5.4, -4.6], role: Role::Hit },
    // The waist, and the ribs down its front.
    Solid { shape: Shape::Frustum { top: [4.2, 3.0], bottom: [3.7, 2.6], half_height: 3.4 }, at: [0.0, -3.0, 0.0], role: Role::Hit },
    Solid { shape: Shape::Cuboid([3.4, 2.65, 0.4]), at: [0.0, -2.8, -3.0], role: Role::Hit },
    // The pelvis and the codpiece.
    Solid { shape: Shape::Frustum { top: [5.2, 3.3], bottom: [5.8, 3.5], half_height: 1.2 }, at: [0.0, -7.4, 0.0], role: Role::Hit },
    Solid { shape: Shape::Cuboid([1.9, 1.7, 0.6]), at: [0.0, -8.1, -3.9], role: Role::Hit },
    // The flanks, with their vents.
    Solid { shape: Shape::Cuboid([0.75, 2.0, 2.6]), at: [-5.7, 2.6, 0.0], role: Role::Hit },
    Solid { shape: Shape::Cuboid([0.75, 2.0, 2.6]), at: [5.7, 2.6, 0.0], role: Role::Hit },
    // The collar, and the plates from it up to the shoulder blocks.
    Solid { shape: Shape::Cuboid([3.0, 0.8, 3.0]), at: [0.0, 8.6, 0.0], role: Role::Hit },
    Solid { shape: Shape::Cuboid([2.1, 0.7, 3.4]), at: [-5.1, 8.7, -0.2], role: Role::Hit },
    Solid { shape: Shape::Cuboid([2.1, 0.7, 3.4]), at: [5.1, 8.7, -0.2], role: Role::Hit },
    // The head, the cheek guards either side of it, the chin guard, visor and brow on its face, and its fin.
    Solid { shape: Shape::Frustum { top: [1.8, 2.2], bottom: [2.3, 2.5], half_height: 2.1 }, at: [0.0, 11.3, -0.4], role: Role::Hit },
    Solid { shape: Shape::Cuboid([2.85, 1.3, 1.5]), at: [0.0, 11.3, 0.2], role: Role::Hit },
    Solid { shape: Shape::Cuboid([2.1, 1.75, 0.5]), at: [0.0, 11.15, -2.75], role: Role::Hit },
    Solid { shape: Shape::Cuboid([0.6, 0.45, 1.8]), at: [0.0, 13.7, 0.2], role: Role::Hit },
    // The shoulder blocks (the missile hatches are in the top of the left one), and their skirts.
    Solid { shape: Shape::Cuboid([2.8, 1.25, 3.9]), at: [-9.2, 9.95, -0.2], role: Role::Hit },
    Solid { shape: Shape::Cuboid([2.8, 1.25, 3.9]), at: [9.2, 9.95, -0.2], role: Role::Hit },
    Solid { shape: Shape::Cuboid([0.65, 1.05, 3.5]), at: [-11.65, 7.65, -0.2], role: Role::Hit },
    Solid { shape: Shape::Cuboid([0.65, 1.05, 3.5]), at: [11.65, 7.65, -0.2], role: Role::Hit },
    // The reactor on the back, and its two stacks.
    Solid { shape: Shape::Cuboid([4.3, 3.2, 1.25]), at: [0.0, 4.0, 5.05], role: Role::Hit },
    Solid { shape: Shape::Cylinder { half_height: 1.75, radius: 0.9 }, at: [-2.8, 8.65, 5.3], role: Role::Hit },
    Solid { shape: Shape::Cylinder { half_height: 1.75, radius: 0.9 }, at: [2.8, 8.65, 5.3], role: Role::Hit },
];

/// Its heavy round, the one load it has.
const CANNON: Fire = Fire::Shell { kind: PROJECTILE_ROTARY, alt: PROJECTILE_ROTARY, speed: TITAN_ROUND_SPEED, length: TITAN_GUN_LENGTH, ttl: SHELL_TTL_TICKS };

/// Its weapons: the rotary cannon in its right fist, fired with the right mouse button (but not while Shift
/// throws that fist as a punch), and the missile tubes in its left shoulder block, fired with G. Fallen, it
/// can do neither.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Fist { hand: TITAN_GUN_HAND },
        fire: CANNON,
        trigger: BUTTON_ALT,
        cooldown: TITAN_GUN_COOLDOWN,
        spread: TITAN_GUN_SPREAD,
        recoil: 0.0,
        heat: TITAN_GUN_HEAT,
        ammo: None,
        needs: &[Need::Upright, Need::Released(BUTTON_RUN)],
        show: Show::Turret,
        eye: None,
        sight: false,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Turns(&TITAN_TUBE_AT),
        fire: Fire::Guided { kind: KIND_MISSILE, speed: SUB_LAUNCH_SPEED, pitch: MISSILE_LAUNCH_PITCH },
        trigger: BUTTON_CHUTE,
        cooldown: TITAN_TUBE_COOLDOWN,
        spread: 0.0,
        recoil: 0.0,
        heat: 0.0,
        ammo: Some(Ammo { full: TITAN_TUBES, refill: Refill::Regrow(TITAN_TUBE_RELOAD) }),
        needs: &[Need::Upright],
        show: Show::Rack,
        eye: None,
        sight: false,
    },
];

/// Armour a tank would envy, all round: a rifle or a machine gun does nothing to it. The reactor is in
/// its chest behind the glowing slot between its chest plates, and the pilot in the head is above the box, out of any round's way.
const ARMOR: Armor = Armor {
    plates: &[
        Plate::hull("chest", Zone::Front, 90.0).glances(70.0),
        Plate::hull("flank", Zone::Side, 70.0).glances(72.0),
        Plate::hull("back", Zone::Back, 50.0).glances(74.0),
        Plate::hull("shoulders", Zone::Top, 60.0).glances(76.0),
        Plate::hull("hips", Zone::Belly, 60.0).glances(76.0),
    ],
    modules: &[Module::new(Part::Engine, [0.0, 1.0, -1.5], [2.0, 2.0, 1.5], 200)],
    canvas: false,
};

/// How it stands and walks: the walker's way, at ten times the size and a third of the pace.
pub const TITAN_GAIT: Gait = Gait {
    foot_half: TITAN_FOOT_HALF,
    leg_length: TITAN_LEG_LENGTH,
    crouch: TITAN_CROUCH,
    crouch_lean: TITAN_CROUCH_LEAN,
    max_speed: TITAN_MAX_SPEED,
    reverse_share: 0.5,
    accel: 3.0,
    brake: 4.0,
    grip_gain: 4.0,
    grip: 6.0,
    turn_rate: TITAN_TURN_RATE,
    turn_accel: 1.5,
    spring: TITAN_LEG_SPRING,
    damping: 5.5,
    // Stiff enough that the weight on its hips, which are behind it when it leans, does not bow it over.
    balance_gain: 10.0,
    balance_damp: 5.0,
    balance_max: 1.2,
    balance_max_pitch: 2.5,
    catch_gain: 12.0,
    catch_damp: 3.0,
    catch_max: 5.0,
    catch_lean_margin: 0.15,
    catch_fall_margin: 0.4,
    cruise_lean: 0.05,
    accel_lean: 0.5,
    blast_lever: 6.0,
    fallen_up: 0.6,
    // Lying down, it takes three times the pull that keeps it standing to lift its weight off the ground.
    getup_share: 3.0,
    rise: TITAN_RISE,
    blast_share: 0.04,
    // An armour-piercing shell on its chest rocks it about two degrees, and it is still again in under a
    // second and a half: `an_ap_shell_on_the_titans_chest_rocks_it_and_it_stands_again`.
    hit_share: 4.5,
    feet: Some(Feet {
        half: TITAN_SHIN_HALF,
        stride: TITAN_STRIDE,
        stride_min: TITAN_STRIDE_MIN,
        step_time: TITAN_STEP_TIME,
        lift: TITAN_STEP_LIFT,
        shift_time: TITAN_SHIFT_TIME,
        sway: 0.3,
        sway_gain: 4.0,
        tilt: 0.04,
        pulse: 0.3,
        crouch_pace: 0.4,
        thigh: TITAN_THIGH_LENGTH,
        shin: TITAN_SHIN_LENGTH,
        ankle: TITAN_ANKLE_HEIGHT,
        thigh_radius: TITAN_THIGH_RADIUS,
        shin_radius: TITAN_SHIN_RADIUS,
        knee_pole: TITAN_KNEE_POLE,
    }),
    jets: Some(TITAN_JETS),
};

/// The thrusters in its feet. A leap throws it about 10 m up; held, they climb at `TITAN_JET_CLIMB`
/// for about seven seconds, and held on the way down they slow a fall by 12 m/s every second.
pub const TITAN_JETS: Jets = Jets {
    leap: TITAN_LEAP_SPEED,
    leap_forward: 6.0,
    leap_cost: 0.1,
    climb: TITAN_JET_CLIMB,
    gain: 2.0,
    accel: 12.0,
    drain: 0.14,
    refill: 0.1,
    speed: TITAN_JET_SPEED,
    push: 5.0,
    tilt: 0.15,
    level: 3.0,
    response: 4.0,
    yaw: TITAN_TURN_RATE,
    ceiling: TITAN_CEILING,
};

/// Its arms.
pub const TITAN_ARMS: Arms = Arms {
    shoulder: TITAN_SHOULDER,
    reach: TITAN_REACH,
    min_reach: TITAN_MIN_REACH,
    fist: TITAN_FIST_RADIUS,
    speed: TITAN_FIST_SPEED,
    accel: TITAN_FIST_ACCEL,
    rest: TITAN_FIST_REST,
    lift: TITAN_LIFT_MASS,
    upper: TITAN_UPPER_ARM_LENGTH,
    fore: TITAN_FOREARM_LENGTH,
    upper_radius: TITAN_UPPER_ARM_RADIUS,
    fore_radius: TITAN_FOREARM_RADIUS,
    elbow_pole: TITAN_ELBOW_POLE,
};

pub(super) const TITAN: VehicleDef = VehicleDef {
    kind: KIND_TITAN,
    name: "Titan",
    hp: 255,
    seats: TITAN_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: GIANT,
    spec: Spec {
        half_extents: TITAN_HALF_EXTENTS,
        // 1,536 m³ of torso at this density is about 2,300 tonnes. Nothing below uses the wheel fields:
        // the legs are `TITAN_GAIT`.
        density: 1500.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: TITAN_MAX_SPEED,
        steer_lock: 0.0,
        steer_axles: Spec::FRONT_STEER,
        turn_rate: TITAN_TURN_RATE,
        lift: Lift::Walker,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: Some(TITAN_FUEL),
        buttons: Buttons::Stride,
        aims: false,
        solids: &SOLIDS,
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        sweep: Spec::FLAT,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        swim: None,
        bow: 0.0,
        profile: Spec::BOX,
        springs: Springs::FIRM,
        airframe: None,
        rotor: None,
        gait: Some(TITAN_GAIT),
        arms: Some(TITAN_ARMS),
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: Some(TITAN_BOARD),
    exit: Some(TITAN_EXIT),
    fittings: &[],
};

/// Two-bone IK, as the client bends the Titan's limbs to draw them (`bend` in
/// `client/src/render/titan.ts`, which this must match): where the joint between a bone `a` long from
/// `root` and one `b` long to `target` is, bent towards `pole`, and where the second bone ends. A target
/// out of the bones' reach is brought to the end of it. Returns `(joint, end)`.
pub fn bend(root: [f32; 3], target: [f32; 3], a: f32, b: f32, pole: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    use crate::vec3::{dot, sub};
    // A vector's direction, or the vector itself when it has no length, as three.js normalises.
    let unit = |v: [f32; 3]| {
        let length = dot(v, v).sqrt();
        if length > 0.0 { v.map(|c| c / length) } else { v }
    };
    let mut along = sub(target, root);
    let reach = dot(along, along).sqrt().max((a - b).abs() + 0.01).min(a + b - 0.01);
    if dot(along, along) < 1e-8 {
        along = [0.0, -1.0, 0.0];
    }
    let along = unit(along);
    // How far along the line the joint is, and how far off it.
    let foot = (a * a - b * b + reach * reach) / (2.0 * reach);
    let off = (a * a - foot * foot).max(0.0).sqrt();
    let across = dot(pole, along);
    let mut side: [f32; 3] = std::array::from_fn(|i| pole[i] - along[i] * across);
    if dot(side, side) < 1e-6 {
        side = std::array::from_fn(|i| [1.0, 0.0, 0.0][i] - along[i] * along[0]);
    }
    let side = unit(side);
    let joint = std::array::from_fn(|i| root[i] + along[i] * foot + side[i] * off);
    let end = std::array::from_fn(|i| root[i] + along[i] * reach);
    (joint, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where a bend's joint and end are put, against what they are asked: the bones keep their lengths.
    #[test]
    fn a_bend_keeps_its_bones_lengths_and_bends_towards_its_pole() {
        let (joint, end) = bend([8.5, 6.0, 0.0], [10.0, -15.0, -3.0], TITAN_UPPER_ARM_LENGTH, TITAN_FOREARM_LENGTH, TITAN_ELBOW_POLE);
        let length = |a: [f32; 3], b: [f32; 3]| crate::vec3::distance(a, b);
        assert!((length([8.5, 6.0, 0.0], joint) - TITAN_UPPER_ARM_LENGTH).abs() < 1e-3, "{joint:?}");
        assert!((length(joint, end) - TITAN_FOREARM_LENGTH).abs() < 1e-3, "{joint:?} {end:?}");
        assert!((length(end, [10.0, -15.0, -3.0])) < 1e-3, "within reach, the end is the target: {end:?}");
        assert!(joint[0] > 9.25 && joint[2] > -1.5, "the elbow goes out and back: {joint:?}");
        // Out of reach, the arm points straight at it.
        let (_, end) = bend([0.0; 3], [0.0, -100.0, 0.0], 12.0, 12.0, TITAN_ELBOW_POLE);
        assert!((end[1] + 23.99).abs() < 1e-3, "{end:?}");
    }

    /// A bend's inputs: its root, its target, its two bones' lengths and its pole.
    type Case = ([f32; 3], [f32; 3], f32, f32, [f32; 3]);

    /// Inputs for the client's bend to be checked against, and what this bend makes of them.
    const BEND_CASES: [Case; 10] = [
        ([8.5, 6.0, 0.0], [10.0, -15.0, -3.0], 12.0, 12.0, [0.7, -0.5, 0.6]),
        ([-8.5, 6.0, 0.0], [-10.0, -15.0, -3.0], 12.0, 12.0, [-0.7, -0.5, 0.6]),
        ([8.5, 6.0, 0.0], [8.5, 6.0, -30.0], 12.0, 12.0, [0.7, -0.5, 0.6]),
        ([8.5, 6.0, 0.0], [9.0, 4.0, -5.0], 12.0, 12.0, [0.7, -0.5, 0.6]),
        ([8.5, 6.0, 0.0], [8.5, 6.0, 0.0], 12.0, 12.0, [0.7, -0.5, 0.6]),
        ([5.0, -8.0, 0.0], [5.0, -32.5, 0.0], 13.5, 13.5, [0.0, -0.15, -1.0]),
        ([-5.0, -8.0, 0.0], [-3.0, -20.0, -9.0], 13.5, 13.5, [0.0, -0.15, -1.0]),
        ([5.0, -8.0, 0.0], [5.0, -8.0, -40.0], 13.5, 13.5, [0.0, -0.15, -1.0]),
        ([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 12.0, 12.0, [1.0, 0.0, 0.0]),
        ([1.0, 2.0, 3.0], [-4.0, -6.0, 7.0], 9.0, 5.0, [0.2, 0.9, -0.4]),
    ];

    /// `client/src/__tests__/titan-bend.fixture.json` holds `BEND_CASES` and this bend's answers, for
    /// the client's bend to be checked against; `UPDATE_PARITY=1 cargo test -p shared` rewrites it.
    #[test]
    fn the_bend_fixture_matches_the_rust_bend() {
        let number = |v: f32| format!("{v:.5}").trim_end_matches('0').trim_end_matches('.').to_string();
        let point = |p: [f32; 3]| format!("[{}, {}, {}]", number(p[0]), number(p[1]), number(p[2]));
        let cases: Vec<String> = BEND_CASES
            .iter()
            .map(|&(root, target, a, b, pole)| {
                let (joint, end) = bend(root, target, a, b, pole);
                format!(
                    "    {{ \"root\": {}, \"target\": {}, \"a\": {}, \"b\": {}, \"pole\": {}, \"joint\": {}, \"end\": {} }}",
                    point(root),
                    point(target),
                    number(a),
                    number(b),
                    point(pole),
                    point(joint),
                    point(end)
                )
            })
            .collect();
        let actual = format!("{{\n  \"cases\": [\n{}\n  ]\n}}\n", cases.join(",\n"));
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/src/__tests__/titan-bend.fixture.json");
        if std::env::var_os("UPDATE_PARITY").is_some() {
            std::fs::write(&path, &actual).unwrap();
        }
        let committed = std::fs::read_to_string(&path).expect("client/src/__tests__/titan-bend.fixture.json; regenerate with UPDATE_PARITY=1");
        assert_eq!(committed, actual, "the Titan's bend changed: regenerate client/src/__tests__/titan-bend.fixture.json with UPDATE_PARITY=1");
    }
}
