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
);

impl InputFrame {
    pub fn seq(&self) -> u32 {
        self.0
    }
    pub fn buttons(&self) -> u8 {
        self.4
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
);

/// `[1, server_tick, ack_input_seq, [entity, ...]]`, sent as a datagram. `ack_input_seq` is the
/// newest input of the *receiving* player that the state already includes.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot(pub u8, pub u32, pub u32, pub Vec<EntityState>);

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
        147, 0, 1, 146, 149, 41, 127, 0, 205, 6, 35, 0, 149, 42, 208, 129, 127, 209, 248, 98, 3,
    ];

    pub const SNAPSHOT_FIXTURE: &[u8] = &[
        148, 1, 205, 4, 210, 42, 146, 152, 1, 0, 205, 4, 210, 210, 255, 254, 29, 192, 101, 205,
        6, 35, 209, 254, 12, 0, 152, 7, 0, 0, 85, 206, 0, 1, 134, 160, 0, 0, 1,
    ];

    fn sample_input() -> ClientInput {
        ClientInput(
            C_INPUT,
            PROTOCOL_VERSION,
            vec![
                InputFrame(41, 127, 0, 1571, 0),
                InputFrame(42, -127, 127, -1950, BUTTON_JUMP | BUTTON_RUN),
            ],
        )
    }

    fn sample_snapshot() -> Snapshot {
        Snapshot(
            S_SNAPSHOT,
            1234,
            42,
            vec![
                EntityState(1, KIND_STICKMAN, 1234, -123_456, 101, 1571, -500, 0),
                EntityState(7, KIND_STICKMAN, 0, 85, 100_000, 0, 0, FLAG_GROUNDED),
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
        let worst = EntityState(u32::MAX, 0, i32::MIN, i32::MIN, i32::MIN, i16::MIN, i16::MIN, 255);
        let snapshot = Snapshot(S_SNAPSHOT, u32::MAX, u32::MAX, vec![worst; MAX_PLAYERS]);
        assert!(encode(&snapshot).len() <= MAX_PACKET);
    }

    #[test]
    fn peek_rejects_garbage() {
        assert_eq!(peek_tag(&[]), None);
        assert_eq!(peek_tag(&[0x90]), None);
        assert_eq!(peek_tag(&[0x92, 0xcc, 0x01]), None);
    }
}
