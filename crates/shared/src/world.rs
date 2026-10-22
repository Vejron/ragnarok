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

pub const WORLD_VERSION: u8 = 11;

/// Metres per degree of latitude; longitude is scaled by cos(latitude).
pub const METRES_PER_DEG_LAT: f64 = 110_540.0;
pub const METRES_PER_DEG_LON: f64 = 111_320.0;
/// The latitude longitude is scaled at, for every world: one fixed scale, not the origin's, so
/// worlds cut from the same global tile grid (`osm-import`'s `grid.rs`) agree on where a tile is
/// wherever their origins are.
pub const SCALE_LATITUDE: f64 = 63.8225;

/// Metres per degree of longitude, everywhere.
pub fn metres_per_deg_lon() -> f64 {
    METRES_PER_DEG_LON * SCALE_LATITUDE.to_radians().cos()
}

/// Equirectangular projection of `lat`, `lon` around `origin` (a latitude/longitude): `[x, z]` in
/// metres, +X east, +Z south. The importer places everything with this.
pub fn project(origin: (f64, f64), lat: f64, lon: f64) -> [f64; 2] {
    let x = (lon - origin.1) * metres_per_deg_lon();
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
    /// The tiles of the world. A tile of the square grid that is not listed is outside the world:
    /// there is no ground there, and nothing can enter it. The grid is the smallest square that
    /// holds the world, so the world can be any shape.
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
    /// Fine ground relief for the client's lighting: for each texel of a square grid over the tile
    /// (row-major, rows along +Z, `n × n` with `n = sqrt(len / 2)`), the slope `(dh/dx, dh/dz)` of the
    /// ground there, each in units of `1 / DETAIL_SLOPE_SCALE`. Finer than `ground`; nothing
    /// simulates it. Empty when the tile has no relief finer than its ground or none was baked.
    #[serde(default)]
    pub detail: Vec<i8>,
    /// Railway tracks, clipped to the tile. Drawn on the ground; nothing simulates them.
    #[serde(default)]
    pub rails: Vec<Rail>,
}

/// Units per unit of slope in `Tile::detail`: a slope of 2 (63°) is 127.
pub const DETAIL_SLOPE_SCALE: f32 = 63.5;

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
    /// Side of a tile, centimetres.
    #[serde(default)]
    pub tile_size: i32,
    /// Which tiles of the square grid belong to the world, `grid × grid`, row-major (`row * grid
    /// + col`). Empty means every tile does (a world built by hand).
    #[serde(default)]
    pub inside: Vec<bool>,
    /// Static collision mesh for everything above ground (buildings).
    pub collision: CollisionMesh,
    /// The terrain. With no samples the ground is the plane y = 0 spanning `half_extent`.
    pub ground: Ground,
    /// Every tree, for trunk colliders.
    pub trees: Vec<Tree>,
    /// Height of the water surface at each `ground` sample, centimetres, or `NO_WATER`. Filled by
    /// `assemble`; empty when no tile has water.
    #[serde(skip)]
    pub water: Vec<i16>,
    /// `buildings[i]`'s identity. Filled by `assemble`; empty for a world built by hand.
    #[serde(skip)]
    pub building_ids: Vec<ObjId>,
    /// `buildings[i]`'s triangles in the merged `collision` mesh, as `[start, end)`. Filled by
    /// `assemble`. `Building::first` is tile-local, so it cannot index the merged mesh.
    #[serde(skip)]
    pub building_triangles: Vec<[u32; 2]>,
    /// `trees[i]`'s identity. Filled by `assemble`; empty for a world built by hand.
    #[serde(skip)]
    pub tree_ids: Vec<ObjId>,
    /// Vehicles the map editor placed by hand. Empty means the server scatters its own.
    #[serde(skip)]
    pub placed_vehicles: Vec<PlacedVehicle>,
    /// Items the map editor placed by hand. Empty means the server scatters its own.
    #[serde(skip)]
    pub placed_items: Vec<PlacedItem>,
    /// Bots the map editor placed by hand. Empty means none (bar `RAGNAROK_BOTS`).
    #[serde(skip)]
    pub placed_bots: Vec<PlacedBot>,
}

/// A vehicle placed in the map editor: its kind, `[x, z]` in centimetres and heading in radians.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlacedVehicle {
    pub kind: u8,
    pub at: [i32; 2],
    pub yaw: f32,
}

/// A bot placed in the map editor: where it lives, its team (above zero; bots of one team never
/// hurt each other), how good a shot it is (`skill`, 0 to 1) and the waypoints it walks round,
/// `[x, z]` in centimetres. An empty route means it makes up a short loop around `at`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacedBot {
    pub at: [i32; 2],
    pub team: u8,
    pub skill: f32,
    pub route: Vec<[i32; 2]>,
}

/// An item placed in the map editor: `[x, z]` in centimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlacedItem {
    pub item: u8,
    pub count: u8,
    pub at: [i32; 2],
}

/// Identity of a building or tree: its tile and its index in that tile's list. Tiles stream in
/// one at a time on the client, so this is the one id both sides can compute without the whole
/// world. A re-import renumbers it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjId {
    pub col: u16,
    pub row: u16,
    pub index: u32,
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
    /// `first`, then `roof` triangles of roof, then the floor (not drawn). A building with `cells`
    /// has no wall or roof triangles, only its floor: it is its cubes.
    pub first: u32,
    pub walls: u32,
    pub roof: u32,
    /// `ROOF_*`: what the roof was fitted as.
    pub roof_kind: u8,
    /// The building's 1 m cubes (`carve::Shell`), worked out by the importer from its walls and
    /// roof; the server and clients make the boxes to collide with, and the clients the faces to
    /// draw, from them. Empty
    /// (`dims` all 0) for a building too big to cut, which keeps its own triangles.
    #[serde(default)]
    pub cells: Cells,
}

/// A building's cubes as the importer baked them: the grid (`carve::Shell`'s `origin` and `dims`)
/// and a flag for each cell, packed by `carve::pack`: whether there is a cube there, and whether
/// it is roof (for its colour).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cells {
    pub origin: [i32; 3],
    pub dims: [u32; 3],
    pub solid: Bits,
    pub roof: Bits,
}

/// Bytes written as a msgpack `bin` (the client gets a `Uint8Array`), not an array of numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bits(pub Vec<u8>);

impl Serialize for Bits {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Bits {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Bits;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bytes")
            }
            fn visit_bytes<E: serde::de::Error>(self, bytes: &[u8]) -> Result<Bits, E> {
                Ok(Bits(bytes.to_vec()))
            }
            fn visit_byte_buf<E: serde::de::Error>(self, bytes: Vec<u8>) -> Result<Bits, E> {
                Ok(Bits(bytes))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Bits, A::Error> {
                let mut out = Vec::new();
                while let Some(byte) = seq.next_element()? {
                    out.push(byte);
                }
                Ok(Bits(out))
            }
        }
        deserializer.deserialize_bytes(Visit)
    }
}

impl Building {
    /// Hit points, as `Tree::hp`: a shed falls to a rocket or two, a block takes a dozen. Follows
    /// the volume, capped so even the biggest is a siege and not a campaign. Server only.
    pub fn hp(&self) -> u16 {
        let [min_x, min_z, max_x, max_z] = self.bounds();
        // The footprint's own area, by the shoelace formula, in square metres.
        let n = self.footprint.len();
        let twice: i64 = (0..n)
            .map(|i| {
                let (a, b) = (self.footprint[i], self.footprint[(i + 1) % n]);
                a[0] as i64 * b[1] as i64 - b[0] as i64 * a[1] as i64
            })
            .sum();
        let area = (twice.abs() as f32 / 2.0) / 10_000.0;
        // A thin or L-shaped footprint is not its bounding box, but never below a third of it.
        let boxed = ((max_x - min_x) as f32 / 100.0) * ((max_z - min_z) as f32 / 100.0);
        let area = area.max(boxed / 3.0);
        (area * self.height as f32 / 100.0 / 4.0).clamp(150.0, 1200.0) as u16
    }

    /// `[min_x, min_z, max_x, max_z]` of the footprint, centimetres.
    pub fn bounds(&self) -> [i32; 4] {
        self.footprint.iter().fold([i32::MAX, i32::MAX, i32::MIN, i32::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
    }

    /// The middle of the footprint's bounding box `[x, z]`, centimetres.
    pub fn centre(&self) -> [i32; 2] {
        let [min_x, min_z, max_x, max_z] = self.bounds();
        [(min_x + max_x) / 2, (min_z + max_z) / 2]
    }
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

/// A railway track's centre line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rail {
    /// Distance between the rails, centimetres.
    pub gauge: i32,
    /// `RAIL_*`.
    pub kind: u8,
    pub points: Vec<[i32; 2]>,
}

/// A track on its own ballast bed, with sleepers.
pub const RAIL_TRAIN: u8 = 0;
/// A track set flush into the street.
pub const RAIL_TRAM: u8 = 1;

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
/// A rounded boulder. `height` is its height and `crown` its radius.
pub const TREE_ROCK_BOULDER: u8 = 7;
/// A low, wide slab of rock.
pub const TREE_ROCK_SLAB: u8 = 8;
/// A shrub, never solid: something to walk through and hide behind.
pub const TREE_BUSH: u8 = 9;

/// What an archetype is: how its collider is sized and what can break it. One table for server and
/// client (`client/src/world.ts` builds its table from `client/src/sim/tables.generated.json`), so a
/// new kind of standing object is a row here, a prototype in `client/src/render/trees.ts`, and a
/// number in `TREE_KINDS`'s length.
#[derive(Clone, Copy, Debug)]
pub struct TreeKindDef {
    pub kind: u8,
    pub name: &'static str,
    /// Collider radius, centimetres, at `ref_height`; scales with the object's height.
    pub radius: i32,
    pub ref_height: i32,
    /// When non-zero the collider radius is this percentage of `crown` instead (rocks are as wide as
    /// they look).
    pub radius_pct_of_crown: i32,
    pub max_radius: i32,
    /// Height of the collider as a percentage of `height`: a tree is solid up to where the crown
    /// starts, a rock all the way up.
    pub collider_height_pct: i32,
    /// Whether shots, blasts and vehicles can fell it. Rocks never fall: they stay as cover.
    pub destructible: bool,
}

const fn tree_kind(kind: u8, name: &'static str, radius: i32, ref_height: i32) -> TreeKindDef {
    TreeKindDef { kind, name, radius, ref_height, radius_pct_of_crown: 0, max_radius: 60, collider_height_pct: 50, destructible: true }
}

pub const TREE_KINDS: [TreeKindDef; 10] = [
    tree_kind(TREE_BIRCH, "Birch", 15, 1200),
    tree_kind(TREE_PINE, "Pine", 20, 1600),
    tree_kind(TREE_SPRUCE, "Spruce", 20, 1600),
    tree_kind(TREE_ORNAMENTAL, "Ornamental", 10, 600),
    tree_kind(TREE_BROADLEAF, "Broadleaf", 30, 1400),
    tree_kind(TREE_COLUMNAR, "Columnar", 20, 1600),
    tree_kind(TREE_LOWWIDE, "Low and wide", 15, 800),
    TreeKindDef { radius_pct_of_crown: 80, max_radius: 200, collider_height_pct: 80, destructible: false, ..tree_kind(TREE_ROCK_BOULDER, "Boulder", 0, 1) },
    TreeKindDef { radius_pct_of_crown: 85, max_radius: 250, collider_height_pct: 85, destructible: false, ..tree_kind(TREE_ROCK_SLAB, "Rock slab", 0, 1) },
    tree_kind(TREE_BUSH, "Bush", 10, 100),
];

/// The definition of an archetype. Unknown kinds are birches, as they are drawn.
pub fn tree_kind_def(kind: u8) -> &'static TreeKindDef {
    TREE_KINDS.iter().find(|def| def.kind == kind).unwrap_or(&TREE_KINDS[0])
}

impl Tree {
    /// Collider radius, centimetres, from the archetype's table. Client and server both build the
    /// collider from this, so it must stay in one place (mirrored in `client/src/world.ts`).
    pub fn trunk_radius(&self) -> i32 {
        let def = tree_kind_def(self.kind);
        let radius = if def.radius_pct_of_crown > 0 { self.crown * def.radius_pct_of_crown / 100 } else { def.radius * self.height / def.ref_height };
        radius.clamp(5, def.max_radius)
    }

    /// Hit points: a slim tree falls to a few rifle rounds, a broad trunk wants a sniper round
    /// or a blast. Server only, but kept next to the trunk size it follows.
    pub fn hp(&self) -> u16 {
        (self.trunk_radius() * 4).max(30) as u16
    }

    /// Height of the collider above `base`, centimetres: the lower half of a tree, where the crown
    /// has not started. Mirrored in `client/src/world.ts`.
    pub fn trunk_height(&self) -> i32 {
        self.height * tree_kind_def(self.kind).collider_height_pct / 100
    }

    /// Whether it can be felled; a rock cannot.
    pub fn destructible(&self) -> bool {
        tree_kind_def(self.kind).destructible
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
    /// Tiles per side of the square grid, as `Manifest::grid`.
    pub fn grid(&self) -> u16 {
        ((2 * self.half_extent as i64 + self.tile_size as i64 - 1) / self.tile_size.max(1) as i64).max(1) as u16
    }

    /// Whether local point `(x, z)`, centimetres, is inside the world: in the square and on a tile
    /// that belongs to it.
    pub fn is_inside(&self, x: i32, z: i32) -> bool {
        let half = self.half_extent as i64;
        if (x as i64).abs() > half || (z as i64).abs() > half {
            return false;
        }
        if self.inside.is_empty() || self.tile_size <= 0 {
            return true;
        }
        let grid = self.grid() as i64;
        let cell = |v: i32| ((v as i64 + half) / self.tile_size as i64).clamp(0, grid - 1);
        self.inside.get((cell(z) * grid + cell(x)) as usize).copied().unwrap_or(false)
    }

    /// Whether local point `(x, z)`, centimetres, is under water: the nearest ground sample has a
    /// water surface above the ground.
    pub fn wet(&self, x: i32, z: i32) -> bool {
        let ground = &self.ground;
        if self.water.len() != ground.heights.len() || ground.cell <= 0 {
            return false;
        }
        let last = ground.side as i64 - 1;
        let sample = |v: i32| ((v as i64 - ground.origin as i64 + ground.cell as i64 / 2) / ground.cell as i64).clamp(0, last);
        let at = (sample(z) * ground.side as i64 + sample(x)) as usize;
        self.water[at] != NO_WATER && self.water[at] > ground.heights[at]
    }

    /// Merges tiles, given with their `(col, row)`, into one world. Collision meshes are
    /// concatenated with their indices shifted, and the tiles' ground samples are joined into one grid.
    pub fn assemble(manifest: &Manifest, tiles: impl IntoIterator<Item = (u16, u16, Tile)>) -> Self {
        let mut world = WorldFile {
            version: manifest.version,
            origin: manifest.origin,
            half_extent: manifest.half_extent,
            spawns: manifest.spawns.clone(),
            tile_size: manifest.tile_size,
            ..Default::default()
        };
        let grid = manifest.grid() as usize;
        if !manifest.tiles.is_empty() && manifest.tiles.len() < grid * grid {
            world.inside = vec![false; grid * grid];
            for tile in &manifest.tiles {
                if let Some(slot) = world.inside.get_mut(tile.row as usize * grid + tile.col as usize) {
                    *slot = true;
                }
            }
        }
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
                if tile.water.len() == tile.ground.len() {
                    if world.water.is_empty() {
                        world.water = vec![NO_WATER; ground.heights.len()];
                    }
                    for j in 0..n {
                        let at = ((j0 + j) * ground.side + i0) as usize;
                        world.water[at..at + n as usize].copy_from_slice(&tile.water[(j * n) as usize..((j + 1) * n) as usize]);
                    }
                }
            }
            let base = world.collision.vertices.len() as u32;
            let triangle_base = world.collision.triangles.len() as u32;
            // A building's triangles run up to whatever the importer wrote next in the tile: the
            // next building, or a bridge, or the end of the mesh.
            let tile_end = tile.collision.triangles.len() as u32;
            let starts: Vec<u32> = tile.buildings.iter().map(|b| b.first).chain(tile.bridges.iter().map(|b| b.first)).collect();
            for (index, building) in tile.buildings.iter().enumerate() {
                let end = starts.iter().copied().filter(|&start| start > building.first).min().unwrap_or(tile_end).min(tile_end);
                world.building_ids.push(ObjId { col, row, index: index as u32 });
                world.building_triangles.push([triangle_base + building.first.min(end), triangle_base + end]);
            }
            world.tree_ids.extend((0..tile.trees.len()).map(|index| ObjId { col, row, index: index as u32 }));
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

    fn object(kind: u8, height: i32, crown: i32) -> Tree {
        Tree { x: 0, z: 0, base: 0, height, crown, kind, solid: true }
    }

    /// The same cases are asserted in `client/src/__tests__/parity.spec.ts`.
    #[test]
    fn colliders_are_sized_by_height_for_trees_and_by_crown_for_rocks() {
        assert_eq!(object(TREE_PINE, 1600, 300).trunk_radius(), 20);
        assert_eq!(object(TREE_PINE, 4000, 300).trunk_radius(), 50);
        assert_eq!(object(TREE_BIRCH, 100, 100).trunk_radius(), 5);
        assert_eq!(object(TREE_ROCK_BOULDER, 150, 100).trunk_radius(), 80);
        assert_eq!(object(TREE_ROCK_SLAB, 60, 200).trunk_radius(), 170);
        assert_eq!(object(TREE_ROCK_BOULDER, 150, 1000).trunk_radius(), 200);
        assert_eq!(object(TREE_BUSH, 100, 80).trunk_radius(), 10);
        // A tree is solid to half its height, a boulder to four fifths.
        assert_eq!(object(TREE_PINE, 1600, 300).trunk_height(), 800);
        assert_eq!(object(TREE_ROCK_BOULDER, 150, 100).trunk_height(), 120);
    }

    #[test]
    fn only_rocks_are_indestructible_and_every_kind_is_in_the_table() {
        for (index, def) in TREE_KINDS.iter().enumerate() {
            assert_eq!(def.kind as usize, index, "{} sits at its own id", def.name);
            assert_eq!(object(def.kind, 500, 100).destructible(), def.kind != TREE_ROCK_BOULDER && def.kind != TREE_ROCK_SLAB);
        }
    }

    #[test]
    fn assemble_names_each_building_and_its_merged_triangles() {
        let manifest = Manifest { version: WORLD_VERSION, half_extent: 20_000, tile_size: 20_000, ..Default::default() };
        // Tile A: two buildings (4 and 2 triangles) then a bridge (2). Tile B: one building (3).
        let building = |first| Building { first, ..Default::default() };
        let mesh = |triangles: usize| CollisionMesh { vertices: vec![[0, 0, 0]; 3], triangles: vec![[0, 1, 2]; triangles] };
        let a = Tile {
            buildings: vec![building(0), building(4)],
            bridges: vec![Bridge { first: 6, deck: 2, steel: 0 }],
            collision: mesh(8),
            trees: vec![Tree::default(); 2],
            ..Default::default()
        };
        let b = Tile { buildings: vec![building(0)], collision: mesh(3), trees: vec![Tree::default()], ..Default::default() };
        let world = WorldFile::assemble(&manifest, [(0, 0, a), (1, 0, b)]);
        assert_eq!(world.building_triangles, vec![[0, 4], [4, 6], [8, 11]]);
        let id = |col, row, index| ObjId { col, row, index };
        assert_eq!(world.building_ids, vec![id(0, 0, 0), id(0, 0, 1), id(1, 0, 0)]);
        assert_eq!(world.tree_ids, vec![id(0, 0, 0), id(0, 0, 1), id(1, 0, 0)]);
        assert_eq!(world.collision.triangles.len(), 11);
    }

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
            buildings: vec![Building { height: 800, footprint: vec![[0, 0], [100, 0], [100, 100]], base: 250, first: 0, walls: 6, roof: 2, roof_kind: ROOF_GABLE, cells: Cells { origin: [-100, 0, 200], dims: [2, 1, 5], solid: Bits(vec![0b1011, 3]), roof: Bits(vec![1, 0]) } }],
            roads: vec![Road { width: 600, kind: ROAD_MINOR, points: vec![[0, 0], [500, 500]] }],
            collision: CollisionMesh { vertices: vec![[0, 0, 0]], triangles: vec![[0, 0, 0]] },
            ground: vec![0, 10, 20, 30],
            cover: vec![Cover { kind: COVER_WATER, rings: vec![vec![[0, 0], [100, 0], [100, 100]]] }],
            trees: vec![Tree { x: 5, z: -7, base: 120, height: 1200, crown: 220, kind: TREE_BIRCH, solid: true }],
            water: vec![NO_WATER, 40, 40, NO_WATER],
            streams: vec![Stream { width: 200, points: vec![[0, 0], [300, 100]] }],
            bridges: vec![Bridge { first: 0, deck: 12, steel: 4 }],
            detail: vec![0, -3, 127, -127],
            rails: vec![Rail { gauge: 144, kind: RAIL_TRAIN, points: vec![[0, 0], [900, 50]] }],
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
    fn tile_water_is_joined_and_wet_finds_it() {
        let manifest = Manifest { version: WORLD_VERSION, half_extent: 2_000, tile_size: 2_000, ground_cell: 1_000, ..Default::default() };
        let dry = Tile { ground: vec![0; 9], ..Default::default() };
        // A pond over the right tile's middle sample, the grid's (3, 1) at x = 1000, z = -1000.
        let mut pond = vec![NO_WATER; 9];
        pond[4] = 50;
        let wet = Tile { ground: vec![0; 9], water: pond, ..Default::default() };
        let world = WorldFile::assemble(&manifest, [(0, 0, dry.clone()), (1, 0, wet), (0, 1, dry.clone()), (1, 1, dry)]);
        assert_eq!(world.water.len(), 25);
        assert!(world.wet(1_000, -1_000));
        assert!(world.wet(1_300, -800), "the nearest sample decides");
        assert!(!world.wet(-1_000, -1_000));
        assert!(!WorldFile::default().wet(0, 0), "a world without water is dry everywhere");
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
