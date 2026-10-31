//! The tupp: everything the game knows about it.

use super::*;

/// The tupp (`KIND_TUPP`): a giant robot rooster of the Swedish Hedemora breed, one driver in a cockpit
/// on its back. It walks, leaps, flaps and glides exactly as the hen does (the `WALKER_*` legs and the
/// `HEN_*` wings), and lays nothing: its head follows the driver's look and its eyes burn a laser.
pub const TUPP_SEATS: usize = 1;
pub const TUPP_HALF_EXTENTS: [f32; 3] = WALKER_HALF_EXTENTS;
/// The head turns within `TUPP_HEAD_LIMIT` of the way the legs face, and `TUPP_HEAD_PITCH_MIN..=_MAX` up
/// and down (radians, hull frame), at `TUPP_HEAD_SLEW` rad/s, about `TUPP_HEAD_PIVOT` in the hull frame.
pub const TUPP_HEAD_LIMIT: f32 = 0.9;
pub const TUPP_HEAD_PITCH_MIN: f32 = -0.6;
pub const TUPP_HEAD_PITCH_MAX: f32 = 0.5;
pub const TUPP_HEAD_SLEW: f32 = 3.5;
pub const TUPP_HEAD_PIVOT: [f32; 3] = [0.0, 2.45, -2.3];
/// How far the beam starts ahead of the pivot, and how far either side of the middle the eyes are,
/// metres. It is settled from between them; the two beams are drawn converging on where it ended.
pub const TUPP_EYE_AHEAD: f32 = 0.55;
pub const TUPP_EYE_X: f32 = 0.27;
/// The laser: `TUPP_LASER_CHARGE_TICKS` of holding the trigger, then a beam for `TUPP_LASER_BURN_TICKS`
/// that strikes every `TUPP_LASER_HIT_TICKS` out to `TUPP_LASER_RANGE`, then `TUPP_LASER_RECOVER_TICKS`
/// to cool. One whole burn finishes anything short of a tank, through up to `TUPP_LASER_PEN` mm of plate.
pub const TUPP_LASER_CHARGE_TICKS: u32 = TICK_RATE * 5 / 4;
pub const TUPP_LASER_BURN_TICKS: u32 = TICK_RATE * 3 / 2;
pub const TUPP_LASER_HIT_TICKS: u32 = TICK_RATE / 10;
pub const TUPP_LASER_RECOVER_TICKS: u32 = TICK_RATE * 3;
pub const TUPP_LASER_RANGE: f32 = 250.0;
/// How wide the beam is, metres from its middle to its edge: it burns the first soldier or vehicle inside that.
pub const TUPP_LASER_RADIUS: f32 = 1.5;
pub const TUPP_LASER_DAMAGE: u8 = 30;
pub const TUPP_LASER_VEHICLE_DAMAGE: u8 = 14;
pub const TUPP_LASER_PLANE_DAMAGE: u8 = 30;
pub const TUPP_LASER_PEN: f32 = 60.0;

const LEG_SOLIDS: [Solid; 2] = super::walker::legs(TUPP_HALF_EXTENTS[1]);

/// The hen's hull: its pilot sealed in the cab up front, and the fuel at the back.
const ARMOR: Armor = Armor {
    plates: ALLOY.plates,
    modules: &[
        Module::crew(0, [0.0, 0.4, -0.9], [0.4, 0.45, 0.45]),
        Module::new(Part::Fuel, [0.0, -0.3, 1.0], [0.5, 0.3, 0.4], 100),
    ],
    canvas: false,
};

pub(super) const TUPP: VehicleDef = VehicleDef {
    kind: KIND_TUPP,
    name: "Tupp",
    hp: 180,
    seats: TUPP_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: LEGS,
    // The hen's light hull, so it flies as far as she does.
    spec: Spec {
        half_extents: TUPP_HALF_EXTENTS,
        // About 4.5 tonnes. Nothing below uses the wheel fields; the legs are the `WALKER_*` constants.
        density: 200.0,
        wheel_radius: 0.0,
        track_half: 0.0,
        axles: &[],
        hard_point_y: 0.0,
        suspension_rest: 0.0,
        max_suspension_force: 0.0,
        engine_force: 0.0,
        max_speed: WALKER_MAX_SPEED,
        steer_lock: 0.0,
        turn_rate: WALKER_TURN_RATE,
        lift: Lift::Walker,
        linear_damping: 0.05,
        com_drop: 0.0,
        tank: Some(HEN_STAMINA),
        buttons: Buttons::Leap,
        aims: false,
        solids: &LEG_SOLIDS,
        tongue: 0.0,
    },
    armor: ARMOR,
    drone: None,
    mount_seat: None,
};

/// The laser eyes: the head turns to the driver's look.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Mount { yaw_limit: Some(TUPP_HEAD_LIMIT), pitch: (TUPP_HEAD_PITCH_MIN, TUPP_HEAD_PITCH_MAX), slew: TUPP_HEAD_SLEW },
    muzzle: Muzzle::One(TUPP_HEAD_PIVOT),
    fire: Fire::Beam {
        charge: TUPP_LASER_CHARGE_TICKS,
        burn: TUPP_LASER_BURN_TICKS,
        recover: TUPP_LASER_RECOVER_TICKS,
        length: TUPP_EYE_AHEAD,
        radius: TUPP_LASER_RADIUS,
        damage: TUPP_LASER_DAMAGE,
        vehicle: TUPP_LASER_VEHICLE_DAMAGE,
        plane: TUPP_LASER_PLANE_DAMAGE,
        weapon: WEAPON_LASER,
        range: TUPP_LASER_RANGE,
        pen: Pen::ap(TUPP_LASER_PEN, TUPP_LASER_PEN, 1.0, 20.0),
    },
    trigger: BUTTON_FIRE,
    cooldown: TUPP_LASER_HIT_TICKS,
    spread: 0.0,
    recoil: 0.0,
    heat: 0.0,
    ammo: None,
    needs: &[Need::Upright],
    show: Show::Turret,
}];
