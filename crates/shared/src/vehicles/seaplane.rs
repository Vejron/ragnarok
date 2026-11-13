//! The seaplane: everything the game knows about it.

use super::*;

/// The seaplane: a two-seat biplane of the 1920s on twin pontoons, flown by a pilot (seat 0) who also
/// fires the two forward guns and drops the bombs, with a gunner (seat 1) on a ring-mounted gun behind.
/// It is an amphibian: each pontoon carries a pair of small wheels, so it rolls on a road as the
/// triplane does, and on open water the pontoons carry it (`floats` in `crates/sim`). Heavier and slower
/// than the triplane, and tougher. Same body axes as the car.
pub const SEAPLANE_SEATS: usize = 2;
/// The fuselage box. The wings and the pontoons are weightless boxes beside it, in the hull frame.
pub const SEAPLANE_HALF_EXTENTS: [f32; 3] = [0.5, 0.6, 3.0];
pub const SEAPLANE_WING_CENTRE: [f32; 3] = [0.0, 0.45, -1.0];
pub const SEAPLANE_WING_HALF_EXTENTS: [f32; 3] = [4.4, 0.85, 0.6];
/// The two pontoons: how far either side of the centre line they are, the middle of each (y and z) and
/// half their size, metres.
pub const SEAPLANE_FLOAT_HALF_TRACK: f32 = 1.1;
pub const SEAPLANE_FLOAT_CENTRE: [f32; 2] = [-1.25, -0.4];
pub const SEAPLANE_FLOAT_HALF_EXTENTS: [f32; 3] = [0.3, 0.3, 2.3];
/// How far below the hull's centre the pontoons' undersides are, and how deep they sit at rest, metres.
pub const SEAPLANE_KEEL: f32 = 1.55;
pub const SEAPLANE_DRAFT: f32 = 0.25;
/// The wheels on the pontoons, a pair ahead of the centre and a pair behind. The front pair steers.
pub const SEAPLANE_WHEEL_RADIUS: f32 = 0.25;
pub const SEAPLANE_AXLES_Z: [f32; 2] = [-1.4, 1.6];
pub const SEAPLANE_HARD_POINT_Y: f32 = -1.25;
pub const SEAPLANE_SUSPENSION_REST: f32 = 0.3;
pub const SEAPLANE_MAX_STEER: f32 = 0.5;
/// How high its chassis centre rides over a road on its wheels, metres (measured by the sim's tests).
pub const SEAPLANE_RIDE_HEIGHT: f32 = 1.71;
/// Acceleration the engine gives at full throttle, m/s²: less than the triplane's, for the weight of the pontoons.
pub const SEAPLANE_THRUST: f32 = 5.5;
/// Air drag per (m/s)², which tops a level flight out at about 38 m/s on full throttle and 29 at cruise.
pub const SEAPLANE_DRAG: f32 = SEAPLANE_THRUST / (38.0 * 38.0);
/// Its wings stall below this airspeed, carry it level above the next, and its controls are tuned for the last, m/s.
pub const SEAPLANE_STALL_SPEED: f32 = 18.0;
pub const SEAPLANE_LEVEL_SPEED: f32 = 25.0;
pub const SEAPLANE_CRUISE_SPEED: f32 = 29.0;
/// Turn rates at full deflection, rad/s: it rolls and pitches more slowly than the triplane.
pub const SEAPLANE_PITCH_RATE: f32 = 1.2;
pub const SEAPLANE_ROLL_RATE: f32 = 2.2;
pub const SEAPLANE_YAW_RATE: f32 = 0.45;
/// The most the wings can pull, m/s² (about 3.5 g).
pub const SEAPLANE_MAX_LIFT: f32 = 34.0;
/// The two forward guns on the cowling, the gunner's ring behind the pilot and the bomb rack between the
/// pontoons: pivots in the hull frame. The guns, the ring and the rack work as the triplane's do
/// (`PLANE_GUN_*`, `PLANE_REAR_*`, `PLANE_BOMB*`).
pub const SEAPLANE_GUN_PIVOTS: [[f32; 3]; 2] = [[-0.16, 0.72, -2.2], [0.16, 0.72, -2.2]];
pub const SEAPLANE_REAR_PIVOT: [f32; 3] = [0.0, 0.85, 0.9];
pub const SEAPLANE_BOMB_PIVOT: [f32; 3] = [0.0, -0.85, -0.3];
/// The pilot behind the upper wing, the gunner standing in the ring.
pub const SEAPLANE_SEAT_HEADS: [[f32; 3]; 2] = [[0.0, 0.98, -0.3], [0.0, 1.15, 0.9]];

/// How the seaplane flies, and the pontoons that carry it on water.
const SEAPLANE_AIRFRAME: Airframe = Airframe {
    thrust: SEAPLANE_THRUST,
    drag: SEAPLANE_DRAG,
    stall_speed: SEAPLANE_STALL_SPEED,
    level_speed: SEAPLANE_LEVEL_SPEED,
    cruise_speed: SEAPLANE_CRUISE_SPEED,
    pitch_rate: SEAPLANE_PITCH_RATE,
    roll_rate: SEAPLANE_ROLL_RATE,
    yaw_rate: SEAPLANE_YAW_RATE,
    max_lift: SEAPLANE_MAX_LIFT,
    floats: Some(Floats { keel: SEAPLANE_KEEL, draft: SEAPLANE_DRAFT, depth: 2.0 * SEAPLANE_FLOAT_HALF_EXTENTS[1] }),
};

/// One pontoon, on the side `side` (-1 left, 1 right).
const fn pontoon(side: f32) -> Solid {
    Solid { shape: Shape::Cuboid(SEAPLANE_FLOAT_HALF_EXTENTS), at: [side * SEAPLANE_FLOAT_HALF_TRACK, SEAPLANE_FLOAT_CENTRE[0], SEAPLANE_FLOAT_CENTRE[1]], role: Role::Part }
}

/// The engine in the nose and the fuel tank ahead of the cockpit; the crew are in the open.
const ARMOR: Armor = Armor {
    plates: SOFT.plates,
    modules: &[
        Module::new(Part::Engine, [0.0, 0.1, -2.55], [0.4, 0.4, 0.45], 100),
        Module::new(Part::Fuel, [0.0, 0.15, -1.6], [0.3, 0.25, 0.3], 70),
    ],
    canvas: true,
};

pub(super) const SEAPLANE: VehicleDef = VehicleDef {
    kind: KIND_SEAPLANE,
    name: "Seaplane",
    hp: 90,
    seats: SEAPLANE_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Open,
    seat_heads: &SEAPLANE_SEAT_HEADS,
    crew_guns: false,
    // It stands taller than the triplane, on its pontoons.
    body: Body { drop_y: SEAPLANE_RIDE_HEIGHT + 0.3, ..WINGS },
    spec: Spec {
        half_extents: SEAPLANE_HALF_EXTENTS,
        // 7.2 m³ of fuselage is about 800 kg. The wings and the pontoons are weightless boxes.
        density: 110.0,
        wheel_radius: SEAPLANE_WHEEL_RADIUS,
        track_half: SEAPLANE_FLOAT_HALF_TRACK,
        axles: &SEAPLANE_AXLES_Z,
        hard_point_y: SEAPLANE_HARD_POINT_Y,
        suspension_rest: SEAPLANE_SUSPENSION_REST,
        max_suspension_force: 40_000.0,
        // The wheels only roll and steer; the propeller is a force on the body.
        engine_force: 0.0,
        max_speed: 0.0,
        steer_lock: SEAPLANE_MAX_STEER,
        turn_rate: 0.0,
        lift: Lift::Wing,
        // Drag is `SEAPLANE_DRAG`, by the square of the speed.
        linear_damping: 0.0,
        com_drop: 0.0,
        tank: None,
        buttons: Buttons::Drive,
        aims: true,
        solids: &[
            Solid { shape: Shape::Cuboid(SEAPLANE_WING_HALF_EXTENTS), at: SEAPLANE_WING_CENTRE, role: Role::Wings },
            pontoon(-1.0),
            pontoon(1.0),
        ],
        tongue: 0.0,
        slide: 0.0,
        roll: 0.0,
        nose: 0.0,
        steer_time: 0.0,
        steer_fade: STEER_FADE,
        skim: None,
        bow: 0.0,
        springs: Springs::FIRM,
        airframe: Some(SEAPLANE_AIRFRAME),
        runners: None,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
    board: None,
    exit: None,
    fittings: &[],
};

/// The triplane's weapons on the seaplane's pivots: the gunner's ring mount behind the cockpit, the
/// pilot's two forward guns, which take turns, and the bomb rack, which drops on crouch.
const MOUNTS: &[MountDef] = &[
    MountDef {
        seat: 1,
        aim: Aim::Mount { yaw_limit: None, pitch: (PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX), slew: PLANE_REAR_SLEW },
        muzzle: Muzzle::One(SEAPLANE_REAR_PIVOT),
        fire: plane::PLANE_GUN.with_length(PLANE_REAR_LENGTH),
        trigger: BUTTON_FIRE,
        cooldown: PLANE_REAR_COOLDOWN_TICKS,
        spread: PLANE_GUN_SPREAD,
        recoil: 0.0,
        heat: PLANE_REAR_HEAT,
        ammo: None,
        needs: &[Need::Clear(PLANE_REAR_DEAD_ZONE)],
        show: Show::Turret,
        eye: Some(Eye { back: GUNNER_BACK, up: GUNNER_UP }),
        sight: false,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::Turns(&SEAPLANE_GUN_PIVOTS),
        fire: plane::PLANE_GUN,
        trigger: BUTTON_FIRE,
        cooldown: PLANE_GUN_COOLDOWN_TICKS,
        spread: PLANE_GUN_SPREAD,
        recoil: 0.0,
        heat: PLANE_GUN_HEAT,
        ammo: None,
        needs: &[],
        show: Show::Guns,
        eye: None,
        sight: false,
    },
    MountDef {
        seat: 0,
        aim: Aim::Fixed,
        muzzle: Muzzle::One(SEAPLANE_BOMB_PIVOT),
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
        eye: None,
        sight: false,
    },
];
