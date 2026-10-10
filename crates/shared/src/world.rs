//! The static world produced by `osm-import` and loaded by both server and client.
//!
//! It is a *manifest* (`client/public/world.msgpack`: where the world is, its spawn points and
//! which tiles exist) and one file per square *tile* (`client/public/tiles/{col}_{row}.msgpack`:
//! the buildings, roads and collision mesh inside it). The server reads every tile and merges
//! them into one `WorldFile`; the client streams the tiles around the player
//! (`client/src/world.ts`, `client/src/stream.ts`).
//!
//! Coordinates are local metres around `origin`, as centimetre integers: +X east, +Z south,
//! +Y up (three.js convention, so the client can use them as-is). Tiles use the same absolute
//! coordinates, not tile-local ones.

use serde::{Deserialize, Serialize};

pub const WORLD_VERSION: u8 = 2;

/// `world.msgpack`: everything about the world except the tiles' contents.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u8,
    /// Latitude/longitude of local (0, 0), in degrees.
    pub origin: (f64, f64),
    /// Half-extent of the playable square, centimetres.
    pub half_extent: i32,
    /// Side of a tile, centimetres. Tile `(col, row)` covers `[-half + col * size, ... + size)`
    /// in X and likewise in Z, so the grid is `ceil(2 * half / size)` tiles a side.
    pub tile_size: i32,
    /// Outdoor spawn points `[x, z]`, centimetres, centre first. Ground level is y = 0.
    pub spawns: Vec<[i32; 2]>,
    /// The tiles that have anything in them. Others are open ground.
    pub tiles: Vec<TileRef>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileRef {
    pub col: u16,
    pub row: u16,
    /// FNV-1a of the tile file, so the manifest's own hash vouches for every tile.
    pub hash: u32,
}

impl TileRef {
    /// Where the tile lives, relative to the directory holding `world.msgpack`.
    pub fn path(&self) -> String {
        format!("tiles/{}_{}.msgpack", self.col, self.row)
    }
}

/// One tile file: the buildings and roads inside it and their collision mesh.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    pub collision: CollisionMesh,
}

/// The whole world, as the server simulates it: the manifest with every tile merged in.
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

impl Manifest {
    pub fn encode(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("manifest always serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }

    /// The tile that contains local point `(x, z)`, centimetres, clamped to the grid.
    pub fn tile_at(&self, x: i32, z: i32) -> (u16, u16) {
        let side = self.grid();
        let cell = |v: i32| (((v as i64 + self.half_extent as i64) / self.tile_size.max(1) as i64).clamp(0, side as i64 - 1)) as u16;
        (cell(x), cell(z))
    }

    /// Tiles per side.
    pub fn grid(&self) -> u16 {
        ((2 * self.half_extent as i64 + self.tile_size as i64 - 1) / self.tile_size.max(1) as i64).max(1) as u16
    }
}

impl Tile {
    pub fn encode(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("tile always serializes")
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

impl WorldFile {
    /// Merges tiles into one world. Collision meshes are concatenated with their indices shifted.
    pub fn assemble(manifest: &Manifest, tiles: impl IntoIterator<Item = Tile>) -> Self {
        let mut world = WorldFile {
            version: manifest.version,
            origin: manifest.origin,
            half_extent: manifest.half_extent,
            spawns: manifest.spawns.clone(),
            ..Default::default()
        };
        for tile in tiles {
            let base = world.collision.vertices.len() as u32;
            world.collision.vertices.extend(tile.collision.vertices);
            world.collision.triangles.extend(tile.collision.triangles.into_iter().map(|[a, b, c]| [a + base, b + base, c + base]));
            world.buildings.extend(tile.buildings);
            world.roads.extend(tile.roads);
        }
        world
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
    fn manifest_and_tile_round_trip() {
        let manifest = Manifest {
            version: WORLD_VERSION,
            origin: (59.3, 18.06),
            half_extent: 30_000,
            tile_size: 25_000,
            spawns: vec![[10, 20]],
            tiles: vec![TileRef { col: 1, row: 0, hash: 7 }],
        };
        assert_eq!(Manifest::decode(&manifest.encode()).unwrap(), manifest);
        let tile = Tile {
            buildings: vec![Building { height: 800, footprint: vec![[0, 0], [100, 0], [100, 100]] }],
            roads: vec![Road { width: 600, kind: ROAD_MINOR, points: vec![[0, 0], [500, 500]] }],
            collision: CollisionMesh { vertices: vec![[0, 0, 0]], triangles: vec![[0, 0, 0]] },
        };
        assert_eq!(Tile::decode(&tile.encode()).unwrap(), tile);
        assert_eq!(manifest.tiles[0].path(), "tiles/1_0.msgpack");
    }

    #[test]
    fn tiles_are_found_by_position_and_merge_with_shifted_indices() {
        let manifest = Manifest { version: WORLD_VERSION, half_extent: 50_000, tile_size: 25_000, ..Default::default() };
        assert_eq!(manifest.grid(), 4);
        assert_eq!(manifest.tile_at(-50_000, -50_000), (0, 0));
        assert_eq!(manifest.tile_at(0, 0), (2, 2));
        assert_eq!(manifest.tile_at(49_999, 60_000), (3, 3), "outside the grid clamps to the edge");
        let square = |x| CollisionMesh { vertices: vec![[x, 0, 0], [x + 1, 0, 0], [x, 0, 1]], triangles: vec![[0, 1, 2]] };
        let a = Tile { collision: square(0), ..Default::default() };
        let b = Tile { collision: square(10), ..Default::default() };
        let world = WorldFile::assemble(&manifest, [a, b]);
        assert_eq!(world.collision.vertices.len(), 6);
        assert_eq!(world.collision.triangles, vec![[0, 1, 2], [3, 4, 5]]);
    }

    #[test]
    fn hash_is_fnv1a() {
        assert_eq!(world_hash(b""), 0x811c_9dc5);
        assert_eq!(world_hash(b"a"), 0xe40c_292c);
    }
}
