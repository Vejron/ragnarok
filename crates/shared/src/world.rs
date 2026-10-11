//! The static world produced by `osm-import` and loaded by both server and client.
//!
//! It is a *manifest* (`client/public/world.msgpack`: where the world is, its spawn points and
//! which tiles exist) and one file per square *tile* (`client/public/tiles/{col}_{row}.msgpack`:
//! the buildings, roads, trees, ground heights and collision mesh inside it). The server reads every tile and merges
//! them into one `WorldFile`; the client streams the tiles around the player
//! (`client/src/world.ts`, `client/src/stream.ts`).
//!
//! Coordinates are local metres around `origin`, as centimetre integers: +X east, +Z south,
//! +Y up (three.js convention, so the client can use them as-is). Tiles use the same absolute
//! coordinates, not tile-local ones.

use serde::{Deserialize, Serialize};

use crate::terrain::Ground;

pub const WORLD_VERSION: u8 = 9;

/// Metres per degree of latitude; longitude is scaled by cos(latitude).
pub const METRES_PER_DEG_LAT: f64 = 110_540.0;
pub const METRES_PER_DEG_LON: f64 = 111_320.0;

/// Equirectangular projection of `lat`, `lon` around `origin` (a latitude/longitude): `[x, z]` in
/// metres, +X east, +Z south. The importer places everything with this.
pub fn project(origin: (f64, f64), lat: f64, lon: f64) -> [f64; 2] {
    let x = (lon - origin.1) * METRES_PER_DEG_LON * origin.0.to_radians().cos();
    let z = -(lat - origin.0) * METRES_PER_DEG_LAT;
    [x, z]
}

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
    /// Outdoor spawn points `[x, z]`, centimetres, centre first. Their height is the ground's.
    pub spawns: Vec<[i32; 2]>,
    /// The tiles that have anything in them. Others are open ground.
    pub tiles: Vec<TileRef>,
    /// Distance between ground samples, centimetres. Zero when the world has no terrain and the
    /// ground is the plane y = 0. It divides `tile_size`.
    pub ground_cell: i32,
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

/// One tile file: the buildings and roads inside it, their collision mesh and the ground under it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    pub collision: CollisionMesh,
    /// Ground heights, centimetres, row-major with `tile_size / ground_cell + 1` samples a side.
    /// The last row and column are shared with the neighbouring tile, so a tile stands alone.
    /// Empty when the world has no terrain.
    pub ground: Vec<i16>,
    /// What covers the ground here (water, woods, parks, ...), clipped to the tile. The client
    /// bakes it, with the roads, into the terrain's texture; nothing simulates it.
    pub cover: Vec<Cover>,
    /// Individual trees (street and park trees) inside the tile.
    pub trees: Vec<Tree>,
    /// Height of the water surface at each ground sample, centimetres (same layout as `ground`),
    /// or `NO_WATER` where there is none. Empty when the tile has no water. Lakes and wide rivers
    /// are carved into `ground`, so the depth under a sample is `water - ground`; the client draws
    /// the surface wherever that is positive. Nothing simulates it.
    #[serde(default)]
    pub water: Vec<i16>,
    /// Streams: narrow watercourses drawn as ribbons draped on the ground, clipped to the tile.
    #[serde(default)]
    pub streams: Vec<Stream>,
    /// Bridges: ranges of this tile's collision triangles (the importer builds them there, so
    /// they are collided with and drawn from the same mesh).
    #[serde(default)]
    pub bridges: Vec<Bridge>,
}

/// One bridge's triangles in a tile's collision mesh: `deck` triangles of concrete (deck, piers,
/// parapets) from `first`, then `steel` triangles (pylon, stay cables). A bridge crossing tile
/// borders has one entry per tile it touches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bridge {
    pub first: u32,
    pub deck: u32,
    pub steel: u32,
}

/// `Tile::water` entry for a sample with no water.
pub const NO_WATER: i16 = i16::MIN;

/// A stream's centre line, in the direction the water flows (digitised downstream).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    /// Width of the water, centimetres.
    pub width: i32,
    pub points: Vec<[i32; 2]>,
}

impl Manifest {
    /// Ground samples per tile side, or 0 when the world is flat.
    pub fn tile_samples(&self) -> u32 {
        if self.ground_cell <= 0 { 0 } else { (self.tile_size / self.ground_cell) as u32 + 1 }
    }
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
    /// Outdoor spawn points `[x, z]`, centimetres. Their height is the ground's.
    pub spawns: Vec<[i32; 2]>,
    /// Static collision mesh for everything above ground (buildings).
    pub collision: CollisionMesh,
    /// The terrain. With no samples the ground is the plane y = 0 spanning `half_extent`.
    pub ground: Ground,
    /// Every tree, for trunk colliders.
    pub trees: Vec<Tree>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Building {
    pub height: i32,
    /// Closed footprint `[x, z]` without the repeated first point, counter-clockwise seen from above.
    pub footprint: Vec<[i32; 2]>,
    /// Height of the floor, centimetres: the lowest ground under the footprint. The roof is at
    /// `base + height`.
    pub base: i32,
    /// This building's triangles in the tile's collision mesh: `walls` triangles of wall from
    /// `first`, then `roof` triangles of roof, then the floor (not drawn).
    pub first: u32,
    pub walls: u32,
    pub roof: u32,
    /// `ROOF_*`: what the roof was fitted as.
    pub roof_kind: u8,
}

pub const ROOF_FLAT: u8 = 0;
pub const ROOF_SHED: u8 = 1;
pub const ROOF_GABLE: u8 = 2;
pub const ROOF_HIP: u8 = 3;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub width: i32,
    /// `ROAD_*`.
    pub kind: u8,
    pub points: Vec<[i32; 2]>,
}

/// A patch of ground cover: one or more rings filled with the even-odd rule, so inner rings are holes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cover {
    /// `COVER_*`.
    pub kind: u8,
    pub rings: Vec<Vec<[i32; 2]>>,
}

pub const COVER_WATER: u8 = 0;
pub const COVER_WOOD: u8 = 1;
pub const COVER_PARK: u8 = 2;
pub const COVER_FARM: u8 = 3;
pub const COVER_SAND: u8 = 4;
pub const COVER_URBAN: u8 = 5;
pub const COVER_INDUSTRIAL: u8 = 6;

/// One tree. All lengths are centimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    pub x: i32,
    pub z: i32,
    /// Height of the ground at the foot of the trunk.
    pub base: i32,
    /// Total height above `base`.
    pub height: i32,
    /// Radius of the crown.
    pub crown: i32,
    /// `TREE_*`: the archetype.
    pub kind: u8,
    /// Whether the trunk is a collider. Woods are many tens of thousands of trees, so only those
    /// near roads, buildings and parks are; the rest are scenery.
    pub solid: bool,
}

pub const TREE_BIRCH: u8 = 0;
pub const TREE_PINE: u8 = 1;
pub const TREE_SPRUCE: u8 = 2;
pub const TREE_ORNAMENTAL: u8 = 3;
pub const TREE_BROADLEAF: u8 = 4;
pub const TREE_COLUMNAR: u8 = 5;
pub const TREE_LOWWIDE: u8 = 6;

impl Tree {
    /// Trunk radius, centimetres, scaled with the tree's height relative to the archetype's.
    /// Client and server both build the trunk collider from this, so it must stay in one place
    /// (mirrored in `client/src/world.ts`).
    pub fn trunk_radius(&self) -> i32 {
        let (base_radius, base_height) = match self.kind {
            TREE_PINE | TREE_SPRUCE | TREE_COLUMNAR => (20, 1600),
            TREE_ORNAMENTAL => (10, 600),
            TREE_BROADLEAF => (30, 1400),
            TREE_LOWWIDE => (15, 800),
            _ => (15, 1200),
        };
        (base_radius * self.height / base_height).clamp(5, 60)
    }

    /// Height of the trunk collider above `base`, centimetres: the lower half of the tree, where
    /// the crown has not started. Mirrored in `client/src/world.ts`.
    pub fn trunk_height(&self) -> i32 {
        self.height / 2
    }
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
    /// Merges tiles, given with their `(col, row)`, into one world. Collision meshes are
    /// concatenated with their indices shifted, and the tiles' ground samples are joined into one grid.
    pub fn assemble(manifest: &Manifest, tiles: impl IntoIterator<Item = (u16, u16, Tile)>) -> Self {
        let mut world = WorldFile {
            version: manifest.version,
            origin: manifest.origin,
            half_extent: manifest.half_extent,
            spawns: manifest.spawns.clone(),
            ..Default::default()
        };
        let n = manifest.tile_samples();
        if n > 0 {
            let per_tile = n - 1;
            let side = manifest.grid() as u32 * per_tile + 1;
            world.ground = Ground {
                origin: -manifest.half_extent,
                cell: manifest.ground_cell,
                side,
                heights: vec![0; (side * side) as usize],
            };
        }
        for (col, row, tile) in tiles {
            if n > 0 && tile.ground.len() == (n * n) as usize {
                let ground = &mut world.ground;
                let (i0, j0) = (col as u32 * (n - 1), row as u32 * (n - 1));
                for j in 0..n {
                    let at = ((j0 + j) * ground.side + i0) as usize;
                    ground.heights[at..at + n as usize].copy_from_slice(&tile.ground[(j * n) as usize..((j + 1) * n) as usize]);
                }
            }
            let base = world.collision.vertices.len() as u32;
            world.collision.vertices.extend(tile.collision.vertices);
            world.collision.triangles.extend(tile.collision.triangles.into_iter().map(|[a, b, c]| [a + base, b + base, c + base]));
            world.buildings.extend(tile.buildings);
            world.roads.extend(tile.roads);
            world.trees.extend(tile.trees);
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
            ground_cell: 5_000,
        };
        assert_eq!(Manifest::decode(&manifest.encode()).unwrap(), manifest);
        let tile = Tile {
            buildings: vec![Building { height: 800, footprint: vec![[0, 0], [100, 0], [100, 100]], base: 250, first: 0, walls: 6, roof: 2, roof_kind: ROOF_GABLE }],
            roads: vec![Road { width: 600, kind: ROAD_MINOR, points: vec![[0, 0], [500, 500]] }],
            collision: CollisionMesh { vertices: vec![[0, 0, 0]], triangles: vec![[0, 0, 0]] },
            ground: vec![0, 10, 20, 30],
            cover: vec![Cover { kind: COVER_WATER, rings: vec![vec![[0, 0], [100, 0], [100, 100]]] }],
            trees: vec![Tree { x: 5, z: -7, base: 120, height: 1200, crown: 220, kind: TREE_BIRCH, solid: true }],
            water: vec![NO_WATER, 40, 40, NO_WATER],
            streams: vec![Stream { width: 200, points: vec![[0, 0], [300, 100]] }],
            bridges: vec![Bridge { first: 0, deck: 12, steel: 4 }],
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
        let world = WorldFile::assemble(&manifest, [(0, 0, a), (1, 0, b)]);
        assert_eq!(world.collision.vertices.len(), 6);
        assert_eq!(world.collision.triangles, vec![[0, 1, 2], [3, 4, 5]]);
    }

    #[test]
    fn tile_ground_is_joined_into_one_grid() {
        // Two tiles of 2 cells a side (3 samples), 10 m cells: a grid of 2 × 2 tiles, 5 samples a side.
        let manifest = Manifest { version: WORLD_VERSION, half_extent: 2_000, tile_size: 2_000, ground_cell: 1_000, ..Default::default() };
        assert_eq!(manifest.tile_samples(), 3);
        let tile = |h: i16| Tile { ground: vec![h; 9], ..Default::default() };
        let world = WorldFile::assemble(&manifest, [(0, 0, tile(1)), (1, 0, tile(2)), (0, 1, tile(3)), (1, 1, tile(4))]);
        let g = &world.ground;
        assert_eq!((g.side, g.cell, g.origin), (5, 1_000, -2_000));
        // Shared edges take the later tile's value; interiors are each tile's own.
        assert_eq!(g.heights[0], 1);
        assert_eq!(g.heights[4], 2);
        assert_eq!(g.heights[4 * 5], 3);
        assert_eq!(g.heights[24], 4);
    }

    #[test]
    fn hash_is_fnv1a() {
        assert_eq!(world_hash(b""), 0x811c_9dc5);
        assert_eq!(world_hash(b"a"), 0xe40c_292c);
    }
}
