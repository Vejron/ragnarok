//! The static world file produced by `osm-import` and loaded by both server and client
//! (`client/public/world.msgpack`, decoded by `client/src/render/world.ts`).
//!
//! Coordinates are local metres around `origin`, as centimetre integers: +X east, +Z south,
//! +Y up (three.js convention, so the client can use them as-is).

use serde::{Deserialize, Serialize};

pub const WORLD_VERSION: u8 = 1;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldFile {
    pub version: u8,
    /// Latitude/longitude of local (0, 0), in degrees.
    pub origin: (f64, f64),
    /// Half-extent of the playable square, centimetres.
    pub half_extent: i32,
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    /// Outdoor spawn points `[x, z]`, centimetres. Ground level is y = 0.
    pub spawns: Vec<[i32; 2]>,
    /// Static collision mesh for everything above ground (buildings). The ground itself is an
    /// implicit plane at y = 0 spanning `half_extent`.
    pub collision: CollisionMesh,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub height: i32,
    /// Closed footprint `[x, z]` without the repeated first point, counter-clockwise seen from above.
    pub footprint: Vec<[i32; 2]>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub width: i32,
    /// `ROAD_*`.
    pub kind: u8,
    pub points: Vec<[i32; 2]>,
}

pub const ROAD_MAJOR: u8 = 0;
pub const ROAD_MINOR: u8 = 1;
pub const ROAD_PATH: u8 = 2;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CollisionMesh {
    pub vertices: Vec<[i32; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl WorldFile {
    pub fn encode(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("world file always serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

/// FNV-1a over the encoded file, sent in `Welcome` so a client can tell it loaded a different
/// world than the server simulates. Mirrored in `client/src/render/world.ts`.
pub fn world_hash(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |hash, &byte| (hash ^ byte as u32).wrapping_mul(0x0100_0193))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let world = WorldFile {
            version: WORLD_VERSION,
            origin: (59.3, 18.06),
            half_extent: 30_000,
            buildings: vec![Building { height: 800, footprint: vec![[0, 0], [100, 0], [100, 100]] }],
            roads: vec![Road { width: 600, kind: ROAD_MINOR, points: vec![[0, 0], [500, 500]] }],
            spawns: vec![[10, 20]],
            collision: CollisionMesh { vertices: vec![[0, 0, 0]], triangles: vec![[0, 0, 0]] },
        };
        assert_eq!(WorldFile::decode(&world.encode()).unwrap(), world);
    }

    #[test]
    fn hash_is_fnv1a() {
        assert_eq!(world_hash(b""), 0x811c_9dc5);
        assert_eq!(world_hash(b"a"), 0xe40c_292c);
    }
}
