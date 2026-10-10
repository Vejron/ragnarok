//! Wire protocol: MessagePack arrays with a leading tag byte. Field names are never sent.
//! See NETWORKING.md for the full table; `client/src/net/codec.ts` mirrors this file.

use serde::{Deserialize, Serialize};

/// Client → server tags.
pub const C_INPUT: u8 = 0;
pub const C_PING: u8 = 1;

/// Server → client tags.
pub const S_WELCOME: u8 = 0;
pub const S_SNAPSHOT: u8 = 1;
pub const S_PONG: u8 = 2;
pub const S_LEAVE: u8 = 3;
pub const S_KILL: u8 = 4;
pub const S_SCORES: u8 = 5;

/// Entity kinds.
pub const KIND_STICKMAN: u8 = 0;
/// Vehicle kinds. These appear in `VehicleState`, not in `EntityState`.
pub const KIND_CAR: u8 = 1;
pub const KIND_TANK: u8 = 2;
pub const KIND_HELI: u8 = 3;

/// Health of a fresh vehicle of `kind`. Zero on the wire means wrecked.
pub fn vehicle_hp(kind: u8) -> u8 {
    match kind {
        KIND_TANK => 250,
        _ => 100,
    }
}

/// Seats in a vehicle of `kind`. Seat 0 is the driver (a helicopter's pilot); a tank's seat 1 is the gunner.
pub fn seat_count(kind: u8) -> usize {
    match kind {
        KIND_TANK => crate::consts::TANK_SEATS,
        KIND_HELI => crate::consts::HELI_SEATS,
        _ => crate::consts::CAR_SEATS,
    }
}

/// `EntityState::flags` bits.
pub const FLAG_GROUNDED: u8 = 1 << 0;
/// Dead and waiting to respawn: lying where they fell, and neither hit nor hurt by anything.
pub const FLAG_DEAD: u8 = 1 << 1;

/// Weapons, as named in shot and kill events.
pub const WEAPON_RIFLE: u8 = 0;
pub const WEAPON_SHELL: u8 = 1;
pub const WEAPON_ROCKET: u8 = 2;
/// A vehicle going up. Only ever the weapon of a shot event or a kill.
pub const WEAPON_WRECK: u8 = 3;

/// One simulation tick of player intent. The client predicts with exactly these quantized values,
/// so client and server feed the simulation identical inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFrame(
    /// Monotonic per-connection sequence number, starting at 1. One frame per client tick.
    pub u32,
    /// Strafe axis, -127 (left) ..= 127 (right).
    pub i8,
    /// Forward axis, -127 (back) ..= 127 (forward).
    pub i8,
    /// Facing in milliradians, `[-π, π)`. 0 faces -Z.
    pub i16,
    /// `BUTTON_*` bits.
    pub u8,
    /// Where the player looks up or down, milliradians (+ is up). Used by a tank gunner and the rifle.
    pub i16,
    /// The server tick this frame's picture showed, modulo 65536: the interpolated world the
    /// player was looking at when they pulled the trigger. The server rewinds other players to it.
    pub u16,
);

impl InputFrame {
    pub fn seq(&self) -> u32 {
        self.0
    }
    pub fn buttons(&self) -> u8 {
        self.4
    }
    pub fn pitch(&self) -> i16 {
        self.5
    }
    pub fn view(&self) -> u16 {
        self.6
    }
}

/// `[0, version, baseline, [frame, ...]]`: the newest `INPUT_REDUNDANCY` frames, oldest first.
/// `baseline` is the tick of the newest snapshot the client has rebuilt, or 0 for none: the server
/// diffs the next snapshot against it.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientInput(pub u8, pub u8, pub u32, pub Vec<InputFrame>);

/// `[1, nonce]`: latency probe, echoed as a `Pong`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Ping(pub u8, pub u32);

/// `[0, version, entity_id, tick_rate, snapshot_every, world_hash]`, sent once on a reliable stream.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Welcome(pub u8, pub u8, pub u32, pub u8, pub u8, pub u32);

/// One entity in a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityState(
    /// Entity id.
    pub u32,
    /// `KIND_*`.
    pub u8,
    /// Position of the body origin, centimetres.
    pub i32,
    pub i32,
    pub i32,
    /// Facing, milliradians.
    pub i16,
    /// Vertical velocity, cm/s. Needed to resume a predicted jump from the authoritative state.
    pub i16,
    /// `FLAG_*` bits.
    pub u8,
    /// Knockback velocity (a car hit them), cm/s along X and Z. Needed to resume a predicted
    /// slide from the authoritative state.
    pub i16,
    pub i16,
);

/// One vehicle in a snapshot. Players sitting in a vehicle are not listed as entities: the vehicle's
/// seats say who is in it, and the client draws them from that.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleState(
    /// Vehicle id. Shares the id space with players.
    pub u32,
    /// `KIND_*`.
    pub u8,
    /// Position of the chassis centre, centimetres.
    pub i32,
    pub i32,
    pub i32,
    /// Chassis orientation as the `x, y, z` of a unit quaternion scaled by 32767, with `w >= 0`.
    pub i16,
    pub i16,
    pub i16,
    /// Front wheel steering as a fraction of `CAR_MAX_STEER`, -127..=127 (+ is right).
    pub i8,
    /// Player id in each seat, or 0 for an empty one. Seat 0 is the driver.
    pub Vec<u32>,
    /// The gun, for vehicles that have one; `nil` otherwise.
    pub Option<Turret>,
    /// Health; 0 is a burning wreck nobody can board.
    pub u8,
);

/// What changed in a standing entity since the baseline: `[id, dx, dy, dz, dyaw, dvy, flags,
/// dknock_x, dknock_z]`. Every field but `id` and `flags` is the new value minus the old one.
/// Entities that did not change at all are not listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDelta(
    pub u32,
    pub i32,
    pub i32,
    pub i32,
    pub i32,
    pub i32,
    /// `FLAG_*` bits, the new value.
    pub u8,
    pub i32,
    pub i32,
);

/// What changed in a vehicle since the baseline: `[id, dx, dy, dz, dqx, dqy, dqz, dsteer, seats,
/// turret, hp]`. The first eight are differences; `seats` and `turret` are `nil` when unchanged,
/// and `hp` is the new value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleDelta(
    pub u32,
    pub i32,
    pub i32,
    pub i32,
    pub i32,
    pub i32,
    pub i32,
    pub i32,
    pub Option<Vec<u32>>,
    pub Option<Turret>,
    pub u8,
);

/// A vehicle's gun, as it is drawn: the turret and barrel angles in the hull frame, and the last
/// shot. `shots` counts shots modulo 256, so a client sees a new shot as a change in the counter;
/// `range` is how far that shell flew before it hit something, in decimetres.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Turret(
    /// Turret yaw relative to the hull, milliradians (+ turns left).
    pub i16,
    /// Barrel elevation, milliradians (+ is up).
    pub i16,
    pub u8,
    pub u16,
);

/// The motion a driver needs to predict their own vehicle, which the vehicle's pose does not
/// carry. Sent only to the player in the driver seat: `[vx, vy, vz, wx, wy, wz, steer, spin]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveState(
    /// Linear velocity, cm/s.
    pub i16,
    pub i16,
    pub i16,
    /// Angular velocity about the world axes, milliradians per second.
    pub i16,
    pub i16,
    pub i16,
    /// Front wheel angle, milliradians, + is right.
    pub i16,
    /// Yaw rate a tank has ramped to, milliradians per second, + is clockwise.
    pub i16,
);

/// Something that just happened, for drawing: `[shooter, weapon, x, y, z]`, where `x, y, z` is
/// where it ended, in centimetres. A rifle shot is a tracer from the shooter's own position to
/// there; a rocket or a wreck is a burst at there (the shooter is 0 for a wreck with no culprit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShotEvent(pub u32, pub u8, pub i32, pub i32, pub i32);

/// A rocket in flight: `[id, x, y, z, vx, vy, vz]`, centimetres and cm/s. A client extrapolates
/// nothing: it interpolates these like any entity and aims the rocket along `v`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectileState(pub u32, pub i32, pub i32, pub i32, pub i16, pub i16, pub i16);

/// `[1, server_tick, baseline, ack_input_seq, ride, [new entity, ...], [entity delta, ...],
/// [new vehicle, ...], [vehicle delta, ...], [removed id, ...], drive, hp, [shot, ...],
/// [rocket, ...]]`, sent as a datagram.
/// `baseline` is the tick of the snapshot the lists are relative to, or 0 when they are not
/// (everything in range is then listed as new). `ack_input_seq` is the newest input of the
/// *receiving* player that the state already includes. `ride` is the id of the vehicle the
/// receiving player stands on top of, or 0. It is carried with that vehicle, so the client must
/// predict relative to it. `drive` is `nil` unless the receiving player is driving. `hp` is the
/// receiving player's own health; `0` is dead. `shots` are the rifle shots since the previous
/// snapshot, newest last, capped at `MAX_SHOT_EVENTS`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot(
    pub u8,
    pub u32,
    pub u32,
    pub u32,
    pub u32,
    pub Vec<EntityState>,
    pub Vec<EntityDelta>,
    pub Vec<VehicleState>,
    pub Vec<VehicleDelta>,
    pub Vec<u32>,
    pub Option<DriveState>,
    pub u8,
    pub Vec<ShotEvent>,
    pub Vec<ProjectileState>,
);

/// `[2, nonce]`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Pong(pub u8, pub u32);

/// `[3, entity_id]`, sent on a reliable stream when a player leaves.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Leave(pub u8, pub u32);

/// `[4, killer, victim, weapon, killer_kills, victim_deaths]`, sent on a reliable stream to everyone.
/// `killer` is 0 when the world did it (a fall); the totals are the new ones, so clients keep no
/// tally of their own.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Kill(pub u8, pub u32, pub u32, pub u8, pub u16, pub u16);

/// `[5, [[id, kills, deaths], ...]]`, sent on a reliable stream to a player who just joined.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Scores(pub u8, pub Vec<(u32, u16, u16)>);

/// Reads the tag of a `[tag, ...]` message without decoding the rest. Every message is a short
/// array (fixarray header) whose first element is a positive fixint.
pub fn peek_tag(bytes: &[u8]) -> Option<u8> {
    match bytes {
        [0x91..=0x9f, tag @ 0x00..=0x7f, ..] => Some(*tag),
        _ => None,
    }
}

pub fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    rmp_serde::to_vec(message).expect("protocol messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    rmp_serde::from_slice(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::*;

    // These fixtures are duplicated in `client/src/__tests__/codec.spec.ts`. If either side
    // changes, both tests must be updated together.

    pub const INPUT_FIXTURE: &[u8] = &[
        148, 0, 11, 205, 4, 206, 146, 151, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 205, 238, 72, 151, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106, 7,
    ];

    pub const SNAPSHOT_FIXTURE: &[u8] = &[
        158, 1, 205, 4, 210, 205, 4, 206, 42, 9, 145, 154, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 3, 0, 0, 145, 153, 1, 25, 253, 40, 208, 136, 205, 2, 138, 1, 209, 254, 212, 12, 145, 156, 10, 2, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 146, 1, 7, 148, 209, 253, 68, 204, 250, 3, 205, 4, 210, 0, 145, 155, 9, 100, 0, 208, 211, 3, 254, 0, 4, 148, 1, 0, 7, 0, 192, 100, 146, 3, 11, 152, 120, 251, 209, 246, 60, 30, 209, 254, 112, 15, 204, 250, 209, 253, 168, 73, 146, 149, 7, 0, 205, 5, 220, 204, 160, 209, 239, 152, 149, 1, 0, 236, 90, 205, 121, 24, 145, 151, 40, 205, 4, 176, 204, 150, 209, 252, 224, 205, 11, 184, 0, 209, 246, 60,
    ];

    fn sample_input() -> ClientInput {
        ClientInput(
            C_INPUT,
            PROTOCOL_VERSION,
            1230,
            vec![
                InputFrame(41, 127, 0, 1571, 0, 300, 61_000),
                InputFrame(42, -127, 127, -1950, BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, -150, 7),
            ],
        )
    }

    fn sample_snapshot() -> Snapshot {
        Snapshot(
            S_SNAPSHOT,
            1234,
            1230,
            42,
            9,
            vec![EntityState(7, KIND_STICKMAN, 0, 85, 100_000, 0, 0, FLAG_GROUNDED | FLAG_DEAD, 0, 0)],
            vec![EntityDelta(1, 25, -3, 40, -120, 650, FLAG_GROUNDED, -300, 12)],
            vec![VehicleState(10, KIND_TANK, 300, 90, -2000, 0, 700, 0, 32, vec![1, 7], Some(Turret(-700, 250, 3, 1234)), 0)],
            vec![VehicleDelta(9, 100, 0, -45, 3, -2, 0, 4, Some(vec![1, 0, 7, 0]), None, 100)],
            vec![3, 11],
            Some(DriveState(120, -5, -2500, 30, -400, 15, 250, -600)),
            73,
            vec![ShotEvent(7, WEAPON_RIFLE, 1500, 160, -4200), ShotEvent(1, WEAPON_RIFLE, -20, 90, 31_000)],
            vec![ProjectileState(40, 1200, 150, -800, 3000, 0, -2500)],
        )
    }

    #[test]
    fn input_matches_fixture() {
        assert_eq!(encode(&sample_input()), INPUT_FIXTURE);
        assert_eq!(decode::<ClientInput>(INPUT_FIXTURE), Some(sample_input()));
        assert_eq!(peek_tag(INPUT_FIXTURE), Some(C_INPUT));
    }

    #[test]
    fn snapshot_matches_fixture() {
        assert_eq!(encode(&sample_snapshot()), SNAPSHOT_FIXTURE);
        assert_eq!(decode::<Snapshot>(SNAPSHOT_FIXTURE), Some(sample_snapshot()));
        assert_eq!(peek_tag(SNAPSHOT_FIXTURE), Some(S_SNAPSHOT));
    }

    #[test]
    fn peek_rejects_garbage() {
        assert_eq!(peek_tag(&[]), None);
        assert_eq!(peek_tag(&[0x90]), None);
        assert_eq!(peek_tag(&[0x92, 0xcc, 0x01]), None);
    }
}
