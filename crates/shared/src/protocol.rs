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

/// Entity kinds.
pub const KIND_STICKMAN: u8 = 0;
/// Vehicle kinds. These appear in `VehicleState`, not in `EntityState`.
pub const KIND_CAR: u8 = 1;
pub const KIND_TANK: u8 = 2;

/// Seats in a vehicle of `kind`. Seat 0 is the driver; a tank's seat 1 is the gunner.
pub fn seat_count(kind: u8) -> usize {
    match kind {
        KIND_TANK => crate::consts::TANK_SEATS,
        _ => crate::consts::CAR_SEATS,
    }
}

/// `EntityState::flags` bits.
pub const FLAG_GROUNDED: u8 = 1 << 0;

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
    /// Where the player looks up or down, milliradians (+ is up). Used by a tank gunner.
    pub i16,
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
}

/// `[0, version, [frame, ...]]`: the newest `INPUT_REDUNDANCY` frames, oldest first.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientInput(pub u8, pub u8, pub Vec<InputFrame>);

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

/// `[1, server_tick, ack_input_seq, ride, [entity, ...], [vehicle, ...]]`, sent as a datagram.
/// `ack_input_seq` is the newest input of the *receiving* player that the state already includes.
/// `ride` is the id of the vehicle the receiving player stands on top of, or 0. It is carried
/// with that vehicle, so the client must predict relative to it.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot(pub u8, pub u32, pub u32, pub u32, pub Vec<EntityState>, pub Vec<VehicleState>);

/// `[2, nonce]`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Pong(pub u8, pub u32);

/// `[3, entity_id]`, sent on a reliable stream when a player leaves.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Leave(pub u8, pub u32);

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
        147, 0, 5, 146, 150, 41, 127, 0, 205, 6, 35, 0, 205, 1, 44, 150, 42, 208, 129, 127, 209, 248, 98, 11, 209, 255, 106,
    ];

    pub const SNAPSHOT_FIXTURE: &[u8] = &[
        150, 1, 205, 4, 210, 42, 9, 146, 154, 1, 0, 205, 4, 210, 210, 255, 254, 29, 192, 101, 205, 6, 35, 209, 254, 12, 0, 205, 2, 138, 209, 254, 212, 154, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 1, 0, 0, 146, 155, 9, 1, 209, 255, 6, 80, 205, 16, 4, 205, 4, 176, 209, 244, 72, 15, 208, 192, 148, 1, 0, 7, 0, 192, 155, 10, 2, 205, 1, 44, 90, 209, 248, 48, 0, 205, 2, 188, 0, 32, 146, 1, 7, 148, 209, 253, 68, 204, 250, 3, 205, 4, 210,
    ];

    fn sample_input() -> ClientInput {
        ClientInput(
            C_INPUT,
            PROTOCOL_VERSION,
            vec![
                InputFrame(41, 127, 0, 1571, 0, 300),
                InputFrame(42, -127, 127, -1950, BUTTON_JUMP | BUTTON_RUN | BUTTON_FIRE, -150),
            ],
        )
    }

    fn sample_snapshot() -> Snapshot {
        Snapshot(
            S_SNAPSHOT,
            1234,
            42,
            9,
            vec![
                EntityState(1, KIND_STICKMAN, 1234, -123_456, 101, 1571, -500, 0, 650, -300),
                EntityState(7, KIND_STICKMAN, 0, 85, 100_000, 0, 0, FLAG_GROUNDED, 0, 0),
            ],
            vec![
                VehicleState(9, KIND_CAR, -250, 80, 4100, 1200, -3000, 15, -64, vec![1, 0, 7, 0], None),
                VehicleState(10, KIND_TANK, 300, 90, -2000, 0, 700, 0, 32, vec![1, 7], Some(Turret(-700, 250, 3, 1234))),
            ],
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
    fn full_room_snapshot_fits_one_datagram() {
        let worst = EntityState(u32::MAX, 0, i32::MIN, i32::MIN, i32::MIN, i16::MIN, i16::MIN, 255, i16::MIN, i16::MIN);
        let worst_car = VehicleState(
            u32::MAX, 255, i32::MIN, i32::MIN, i32::MIN, i16::MIN, i16::MIN, i16::MIN, i8::MIN, vec![u32::MAX; CAR_SEATS],
            Some(Turret(i16::MIN, i16::MIN, 255, u16::MAX)),
        );
        // Every seat taken is a stickman fewer in the entity list.
        for cars in 0..=MAX_VEHICLES {
            let seated = (cars * CAR_SEATS).min(MAX_PLAYERS);
            let snapshot = Snapshot(S_SNAPSHOT, u32::MAX, u32::MAX, u32::MAX, vec![worst; MAX_PLAYERS - seated], vec![worst_car.clone(); cars]);
            let size = encode(&snapshot).len();
            assert!(size <= MAX_PACKET, "{cars} cars: {size} bytes");
        }
    }

    #[test]
    fn peek_rejects_garbage() {
        assert_eq!(peek_tag(&[]), None);
        assert_eq!(peek_tag(&[0x90]), None);
        assert_eq!(peek_tag(&[0x92, 0xcc, 0x01]), None);
    }
}
