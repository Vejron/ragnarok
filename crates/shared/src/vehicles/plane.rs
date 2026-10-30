//! The triplane: everything the game knows about it.

use super::*;

/// The triplane: a Fokker Dr.I stretched into a two-seater, flown by a pilot (seat 0) who also fires the
/// two forward guns and drops the bombs, with a gunner (seat 1) on a ring-mounted gun behind. It rolls on
/// ray-cast wheels like a car, and in the air its wings hold it up once it is fast enough (see `wing` in
/// `physics/vehicle.rs`). Same body axes as the car.
pub const PLANE_SEATS: usize = 2;
/// The fuselage box. The wings are a second, weightless box (`PLANE_WING_*`), centred in the hull frame.
pub const PLANE_HALF_EXTENTS: [f32; 3] = [0.5, 0.6, 2.8];
pub const PLANE_WING_CENTRE: [f32; 3] = [0.0, 0.45, -1.1];
pub const PLANE_WING_HALF_EXTENTS: [f32; 3] = [3.6, 0.85, 0.55];
pub const PLANE_WHEEL_RADIUS: f32 = 0.35;
pub const PLANE_TRACK_HALF: f32 = 0.8;
/// The main gear ahead of the centre, the tail skid behind it. The front pair steers on the ground.
pub const PLANE_AXLES_Z: [f32; 2] = [-1.5, 2.4];
pub const PLANE_HARD_POINT_Y: f32 = -0.5;
pub const PLANE_SUSPENSION_REST: f32 = 0.3;
pub const PLANE_MAX_STEER: f32 = 0.5;
/// Acceleration the engine gives at full throttle, m/s². With the stick released it runs at
/// `PLANE_CRUISE_THROTTLE` of that, and with the stick pulled back the blip switch cuts it to `PLANE_IDLE_THROTTLE`.
pub const PLANE_THRUST: f32 = 6.5;
pub const PLANE_CRUISE_THROTTLE: f32 = 0.6;
pub const PLANE_IDLE_THROTTLE: f32 = 0.03;
/// Air drag per (m/s)², which tops a level flight out at about 45 m/s on full throttle and 35 at cruise.
pub const PLANE_DRAG: f32 = PLANE_THRUST / (45.0 * 45.0);
/// Below this airspeed (m/s) the wings stall: their lift fades out over the last 30% of it and the nose drops.
pub const PLANE_STALL_SPEED: f32 = 17.0;
/// Above this airspeed the wings carry the plane level by themselves; below it they carry less, by the square.
pub const PLANE_LEVEL_SPEED: f32 = 28.0;
/// The speed the controls are tuned for: they lose authority below it, by its square.
pub const PLANE_CRUISE_SPEED: f32 = 35.0;
/// How fast the wings turn the flight path towards where the nose points, per second, and how
/// fast the fin kills a sideslip.
pub const PLANE_AOA_GAIN: f32 = 4.0;
pub const PLANE_SIDE_GAIN: f32 = 3.0;
/// The most the wings can pull, m/s² (about 4 g).
pub const PLANE_MAX_LIFT: f32 = 40.0;
/// Turn rates at full deflection, rad/s, and how fast the body's spin follows them, per second.
pub const PLANE_PITCH_RATE: f32 = 1.4;
pub const PLANE_ROLL_RATE: f32 = 3.0;
pub const PLANE_YAW_RATE: f32 = 0.5;
pub const PLANE_RESPONSE: f32 = 6.0;
/// How hard a stalled plane drops its nose towards where it is going, rad/s.
pub const PLANE_STALL_DROP: f32 = 1.5;
/// Where a plane with nobody at the stick points its nose, radians below the horizon: it glides down.
pub const PLANE_GLIDE_PITCH: f32 = -0.2;
/// Off-target angle (radians) beyond which the plane banks into the turn instead of levelling its wings.
pub const PLANE_BANK_ANGLE: f32 = 0.2;
/// The steepest bank the plane rolls into by itself, radians (about 75°): past it the pull does the turning.
pub const PLANE_MAX_BANK: f32 = 1.3;
/// Rate asked for per radian the target is off the nose, as a share of the full rate.
pub const PLANE_AIM_GAIN: f32 = 2.0;
/// How steeply a pilot's mouse can ask the plane to climb or dive, radians (75 degrees).
pub const PLANE_AIM_MAX_PITCH: f32 = 1.3;
/// With the stick centred the nose settles back to the horizon at this share of full pitch per radian off it.
pub const PLANE_LEVEL_GAIN: f32 = 1.5;
/// The engine fades out over the last 20 m below this height; the boundary walls stop at 200 m.
pub const PLANE_CEILING: f32 = 160.0;
/// Within this many metres of the edge of the world the plane is steered back towards the middle.
pub const PLANE_EDGE_MARGIN: f32 = 120.0;
/// A plane is parked where a straight, flat, clear run of at least this many metres lies ahead.
pub const PLANE_RUNWAY: f32 = 80.0;
/// The two forward guns on the cowling, which fire in turn along the nose. `PLANE_GUN_COOLDOWN_TICKS`
/// between rounds makes twelve a second between them. A round is a hitscan out to `PLANE_GUN_RANGE`.
pub const PLANE_GUN_PIVOTS: [[f32; 3]; 2] = [[-0.16, 0.72, -2.0], [0.16, 0.72, -2.0]];
pub const PLANE_GUN_COOLDOWN_TICKS: u32 = 5;
pub const PLANE_GUN_RANGE: f32 = 300.0;
pub const PLANE_GUN_SPREAD: f32 = 0.012;
/// What a round does to a soldier, and to a vehicle: a plane is all canvas, anything else shrugs it off.
pub const PLANE_GUN_DAMAGE: u8 = 14;
pub const PLANE_GUN_PLANE_DAMAGE: u8 = 7;
pub const PLANE_GUN_VEHICLE_DAMAGE: u8 = 3;
/// Guns heat by this much a round and cool by `GUN_COOL` a tick, out of 255. At 255 they jam until
/// they have cooled to `GUN_UNJAM`: about four seconds of fire, then three of waiting.
pub const PLANE_GUN_HEAT: f32 = 10.5;
/// The gunner's ring mount behind the pilot: a pivot in the hull frame, the barrel's length, how far
/// it lifts and dips, and its rate. It turns all the way round, but holds fire within
/// `PLANE_REAR_DEAD_ZONE` of the nose, where the pilot sits.
pub const PLANE_REAR_PIVOT: [f32; 3] = [0.0, 0.85, 0.7];
pub const PLANE_REAR_LENGTH: f32 = 0.9;
pub const PLANE_REAR_PITCH_MIN: f32 = -0.35;
pub const PLANE_REAR_PITCH_MAX: f32 = 1.0;
pub const PLANE_REAR_SLEW: f32 = 4.0;
pub const PLANE_REAR_DEAD_ZONE: f32 = 0.6;
pub const PLANE_REAR_COOLDOWN_TICKS: u32 = 8;
pub const PLANE_REAR_HEAT: f32 = 16.5;
/// Bombs on the rack under the fuselage, one dropped at most every `PLANE_BOMB_COOLDOWN_TICKS`. A
/// bomb leaves with the plane's velocity and falls; it bursts on the first thing it meets. The
/// rack fills again on the ground once the plane has stood still for `PLANE_REARM_TICKS`.
pub const PLANE_BOMBS: u8 = 4;
pub const PLANE_BOMB_PIVOT: [f32; 3] = [0.0, -0.85, -0.3];
pub const PLANE_BOMB_COOLDOWN_TICKS: u32 = 30;
/// The rack holds on to its bombs below this height above the ground, metres.
pub const PLANE_BOMB_MIN_HEIGHT: f32 = 4.0;
pub const PLANE_REARM_TICKS: u32 = 3 * TICK_RATE;
/// The pilot behind the wings, the gunner standing in the ring.
pub const PLANE_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.98, -0.45], [0.0, 1.15, 0.7]];

const PLANE_GUN: Fire = Fire::Hitscan { length: 0.0, damage: PLANE_GUN_DAMAGE, vehicle: PLANE_GUN_VEHICLE_DAMAGE, plane: PLANE_GUN_PLANE_DAMAGE, weapon: WEAPON_MG, range: PLANE_GUN_RANGE, pen: Pen::ball(10.0, 7.0, 300.0, 7.92) };

/// The engine in the nose and the fuel tank ahead of the cockpit; the crew are in the open.
const ARMOR: Armor = Armor {
    plates: SOFT.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.1, -2.35], [0.35, 0.35, 0.45], 90),
        Module::new(Part::Fuel, [0.0, 0.15, -1.5], [0.3, 0.25, 0.3], 70),
    ],
    canvas: true,
};

pub(super) const PLANE: VehicleDef = VehicleDef {
    kind: KIND_PLANE,
    name: "Triplane",
    hp: 70,
    seats: PLANE_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &PLANE_SEAT_HEADS,
    crew_guns: false,
    body: WINGS,
    spec: Spec {
        half_extents: PLANE_HALF_EXTENTS,
        // 6.7 m³ of fuselage is about 600 kg, a Dr.I fully loaded. The wings are a weightless second box.
        density: 90.0,
        wheel_radius: PLANE_WHEEL_RADIUS,
        track_half: PLANE_TRACK_HALF,
        axles: &PLANE_AXLES_Z,
        hard_point_y: PLANE_HARD_POINT_Y,
        suspension_rest: PLANE_SUSPENSION_REST,
        max_suspension_force: 30_000.0,
        // The wheels only roll and steer; the propeller is a force on the body.
        engine_force: 0.0,
        max_speed: 0.0,
        steer_lock: PLANE_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wing,
        // Drag is `PLANE_DRAG`, by the square of the speed.
        linear_damping: 0.0,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[Solid { shape: Shape::Cuboid(PLANE_WING_HALF_EXTENTS), at: PLANE_WING_CENTRE, role: Role::Wings }],
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};

/// The gunner's ring mount behind the cockpit, the pilot's two forward guns, which take turns, and
/// the bomb rack, which drops on crouch.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: None, pitch: (PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX), slew: PLANE_REAR_SLEW },
        muzzle: Muzzle::One(PLANE_REAR_PIVOT),
        fire: PLANE_GUN.with_length(PLANE_REAR_LENGTH),
        trigger: BUTTON_FIRE,
        cooldown: PLANE_REAR_COOLDOWN_TICKS,
        spread: PLANE_GUN_SPREAD,
        recoil: 0.0,
        heat: PLANE_REAR_HEAT,
        ammo: None,
        needs: &[Need::Clear(PLANE_REAR_DEAD_ZONE)],
        show: Show::Turret,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Turns(&PLANE_GUN_PIVOTS),
        fire: PLANE_GUN,
        trigger: BUTTON_FIRE,
        cooldown: PLANE_GUN_COOLDOWN_TICKS,
        spread: PLANE_GUN_SPREAD,
        recoil: 0.0,
        heat: PLANE_GUN_HEAT,
        ammo: None,
        needs: &[],
        show: Show::Guns,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::One(PLANE_BOMB_PIVOT),
        fire: Fire::Drop { kind: PROJECTILE_BOMB, toss: [0.0; 3], ttl: SHELL_TTL_TICKS * 2 },
        trigger: BUTTON_CROUCH,
        cooldown: PLANE_BOMB_COOLDOWN_TICKS,
        spread: 0.0,
        recoil: 0.0,
        heat: 0.0,
        ammo: Some(Ammo { full: PLANE_BOMBS, refill: Refill::Parked(PLANE_REARM_TICKS) }),
        // The rack only lets go in the air, where the bomb has room to fall clear.
        needs: &[Need::Height(PLANE_BOMB_MIN_HEIGHT)],
        show: Show::Rack,
    },
];
