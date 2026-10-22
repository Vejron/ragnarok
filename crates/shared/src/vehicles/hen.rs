//! The hen: everything the game knows about it.

use super::*;

const LEG_SOLIDS: [Solid; 2] = super::walker::legs(HEN_HALF_EXTENTS[1]);

/// The walker's legs and hull, with wings and eggs in place of guns.
pub(super) const HEN: VehicleDef = VehicleDef {
    kind: KIND_HEN,
    name: "Hen",
    hp: 180,
    seats: HEN_SEATS,
    mounts: MOUNTS,
    car_wheels: false,
    crew: Exposure::Sealed,
    seat_heads: &[],
    crew_guns: false,
    body: LEGS,
    // A lighter hull than the walker's, so it gets off the ground more easily.
    spec: Spec {
        half_extents: HEN_HALF_EXTENTS,
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
    },
    armor: LIGHT,
};

/// The clutch: an egg from under the tail, with the hull's velocity and a little backwards.
const MOUNTS: &[MountDef] = &[MountDef {
    seat: 0,
    aim: Aim::Fixed,
    muzzle: Muzzle::One(HEN_VENT),
    fire: Fire::Drop { kind: PROJECTILE_EGG, toss: [0.0, 0.0, HEN_EGG_TOSS], ttl: HEN_EGG_FUSE_TICKS },
    trigger: BUTTON_FIRE,
    cooldown: HEN_EGG_COOLDOWN_TICKS,
    spread: 0.0,
    recoil: 0.0,
    heat: 0.0,
    ammo: Some(Ammo { full: HEN_EGGS, refill: Refill::Regrow(HEN_EGG_REGROW_TICKS) }),
    needs: &[Need::Upright],
    show: Show::Rack,
}];
