//! Simulation and network constants. Mirrored in `client/src/sim/consts.ts`.

/// Wire schema version. Bumped on any incompatible protocol or world-format change.
pub const PROTOCOL_VERSION: u8 = 5;

/// Fixed simulation rate, in ticks per second.
pub const TICK_RATE: u32 = 60;
/// Seconds per simulation tick.
pub const TICK_DT: f32 = 1.0 / TICK_RATE as f32;
/// A snapshot is sent every this many ticks (60 / 2 = 30 Hz).
pub const SNAPSHOT_EVERY: u32 = 2;

/// Players admitted to the single room. A full snapshot of this many entities stays under one
/// datagram (`MAX_PACKET`), which is the bound until interest management lands.
pub const MAX_PLAYERS: usize = 32;
/// Datagrams above this size are dropped unread. Stays under a typical QUIC datagram MTU.
pub const MAX_PACKET: usize = 1200;
/// Each input datagram repeats this many of the most recent frames, so a lost packet costs nothing.
pub const INPUT_REDUNDANCY: usize = 4;

/// Input buttons, as bits of `InputFrame::buttons`.
pub const BUTTON_JUMP: u8 = 1 << 0;
pub const BUTTON_RUN: u8 = 1 << 1;
pub const BUTTON_USE: u8 = 1 << 2;
/// Gunner's trigger. Held to keep firing; the gun's reload paces it.
pub const BUTTON_FIRE: u8 = 1 << 3;

/// Stickman movement, in metres and seconds.
pub const WALK_SPEED: f32 = 4.0;
pub const RUN_SPEED: f32 = 7.5;
pub const JUMP_SPEED: f32 = 6.5;
pub const GRAVITY: f32 = 20.0;
/// Knockback (a car hitting a stickman) is a horizontal velocity added to the walking velocity.
/// Each tick it keeps this fraction of itself: quickly on the ground, barely in the air.
pub const KNOCK_KEEP_GROUND: f32 = 0.92;
pub const KNOCK_KEEP_AIR: f32 = 0.99;
/// Below this speed (m/s) knockback is dropped, so it always dies out exactly.
pub const KNOCK_MIN_SPEED: f32 = 0.05;
/// Fastest knockback the wire can carry and the server will give, m/s.
pub const KNOCK_MAX_SPEED: f32 = 20.0;
/// Terminal fall speed, so `vy` always fits its i16 cm/s wire field.
pub const MAX_FALL_SPEED: f32 = 50.0;

/// Stickman collision capsule. Total height is `2 * (half_height + radius)` = 1.7 m, with the
/// body origin at the capsule centre.
pub const CAPSULE_HALF_HEIGHT: f32 = 0.5;
pub const CAPSULE_RADIUS: f32 = 0.35;

/// Character controller tuning. The client builds its Rapier controller from the same values.
pub const CONTROLLER_OFFSET: f32 = 0.02;
pub const MAX_SLOPE_CLIMB_DEG: f32 = 50.0;
pub const AUTOSTEP_HEIGHT: f32 = 0.35;
pub const AUTOSTEP_MIN_WIDTH: f32 = 0.2;

/// Anything that falls below this is respawned (e.g. a player squeezed through a gap in the mesh).
pub const KILL_Y: f32 = -50.0;

/// Vehicles. Chassis and wheel geometry are shared with the client, which draws the car from them.
/// Body-local axes: +X right, +Y up, -Z forward (yaw 0 faces -Z, like a stickman).
pub const CAR_SEATS: usize = 4;
/// Vehicles in the world; bounds the vehicle list in a snapshot.
pub const MAX_VEHICLES: usize = 8;
/// A player can enter a vehicle whose centre is within this distance of their capsule centre.
pub const ENTER_RADIUS: f32 = 4.0;

/// Half extents of the chassis box, metres.
pub const CAR_HALF_EXTENTS: [f32; 3] = [0.9, 0.4, 2.1];
pub const CAR_WHEEL_RADIUS: f32 = 0.35;
/// Wheel hard points relative to the chassis centre: half the track, and the distance of the
/// axles from the centre along Z. Front axle is at -Z.
pub const CAR_TRACK_HALF: f32 = 0.85;
pub const CAR_AXLE_Z: f32 = 1.35;
pub const CAR_HARD_POINT_Y: f32 = -0.1;
pub const CAR_SUSPENSION_REST: f32 = 0.3;
/// Largest steering angle of the front wheels, radians. The wire carries it as -127..=127.
pub const CAR_MAX_STEER: f32 = 0.5;
/// Forward speed above which engine force fades to zero, m/s.
pub const CAR_MAX_SPEED: f32 = 30.0;

/// The tank: a heavy six-wheel hull that turns on the spot, with a driver (seat 0) and a gunner
/// (seat 1) who aims the turret and fires. Same body axes as the car.
pub const TANK_SEATS: usize = 2;
pub const TANK_HALF_EXTENTS: [f32; 3] = [1.3, 0.5, 2.6];
pub const TANK_WHEEL_RADIUS: f32 = 0.45;
pub const TANK_TRACK_HALF: f32 = 1.2;
/// Z of the three axles, front first.
pub const TANK_AXLES_Z: [f32; 3] = [-1.9, 0.0, 1.9];
pub const TANK_HARD_POINT_Y: f32 = -0.2;
pub const TANK_SUSPENSION_REST: f32 = 0.35;
pub const TANK_MAX_SPEED: f32 = 12.0;
/// Yaw rate at full stick, rad/s.
pub const TANK_TURN_RATE: f32 = 1.1;
/// Where the turret ring sits in the hull frame, and how far the muzzle is from the ring.
pub const TANK_TURRET_PIVOT: [f32; 3] = [0.0, 0.85, 0.3];
pub const TANK_BARREL_LENGTH: f32 = 3.2;
/// How fast the turret and barrel follow the gunner's aim, rad/s.
pub const TANK_TURRET_SLEW: f32 = 1.4;
pub const TANK_BARREL_SLEW: f32 = 0.9;
/// Barrel elevation limits, radians (+ is up).
pub const TANK_BARREL_MIN: f32 = -0.12;
pub const TANK_BARREL_MAX: f32 = 0.5;

/// The tank gun.
pub const SHELL_COOLDOWN_TICKS: u32 = 90;
pub const SHELL_RANGE: f32 = 300.0;
/// Speed a fired shell takes from the tank, m/s, backwards along the barrel.
pub const SHELL_RECOIL: f32 = 1.6;
pub const EXPLOSION_RADIUS: f32 = 7.0;
/// Knockback speed at the centre of a blast, m/s; it fades to zero at the edge.
pub const EXPLOSION_KNOCK: f32 = 18.0;
