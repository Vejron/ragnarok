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

pub const WORLD_VERSION: u8 = 44;

/// `Manifest::theme`: the world as it always was.
pub const THEME_PLAIN: u8 = 0;
/// `Manifest::theme`: snow on everything, lakes frozen, ski tracks where OSM has them.
pub const THEME_WINTER: u8 = 1;

/// Metres per degree of latitude; longitude is scaled by cos(latitude).
pub const METRES_PER_DEG_LAT: f64 = 110_540.0;
pub const METRES_PER_DEG_LON: f64 = 111_320.0;
/// The latitude longitude is scaled at, for every world: one fixed scale, not the origin's, so
/// worlds cut from the same global tile grid (`osm-import`'s `grid.rs`) agree on where a tile is
/// wherever their origins are.
pub const SCALE_LATITUDE: f64 = 63.8225;

/// Whether `name` is one a world directory may have: 1-40 characters of a-z, 0-9, `-` and `_`.
pub fn plain_world_name(name: &str) -> bool {
    (1..=40).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Metres per degree of longitude, everywhere.
pub fn metres_per_deg_lon() -> f64 {
    METRES_PER_DEG_LON * SCALE_LATITUDE.to_radians().cos()
}

/// Equirectangular projection of `lat`, `lon` around `origin` (a latitude/longitude): `[x, z]` in
/// metres, +X east, +Z south. A Swedish world's frame (`Frame::Equirect`).
pub fn project(origin: (f64, f64), lat: f64, lon: f64) -> [f64; 2] {
    let x = (lon - origin.1) * metres_per_deg_lon();
    let z = -(lat - origin.0) * METRES_PER_DEG_LAT;
    [x, z]
}

/// How a world's local metres lie on the Earth: +X east, +Z south, from local (0, 0). The importer
/// places everything with `project`, and the server places the editor's latitude/longitude with it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "(u8, f64, f64)", into = "(u8, f64, f64)")]
pub enum Frame {
    /// Equirectangular around `origin` (latitude, longitude), with longitude scaled at
    /// `SCALE_LATITUDE` for every world: Sweden's.
    Equirect { origin: (f64, f64) },
    /// Lambert-93 metres (`lambert93`), local (0, 0) at easting `e0`, northing `n0`: France's.
    /// Conformal, so a world anywhere in France is true to scale within 0.3 %.
    Lambert93 { e0: f64, n0: f64 },
}

impl Default for Frame {
    fn default() -> Self {
        Frame::Equirect { origin: (0.0, 0.0) }
    }
}

impl Frame {
    /// `[x, z]` metres of `(lat, lon)` degrees.
    pub fn project(self, lat: f64, lon: f64) -> [f64; 2] {
        match self {
            Frame::Equirect { origin } => project(origin, lat, lon),
            Frame::Lambert93 { e0, n0 } => {
                let (e, n) = crate::lambert93::forward(lat, lon);
                [e - e0, n0 - n]
            }
        }
    }

    /// `(lat, lon)` degrees of local `(x, z)` metres.
    pub fn unproject(self, x: f64, z: f64) -> (f64, f64) {
        match self {
            Frame::Equirect { origin } => (origin.0 - z / METRES_PER_DEG_LAT, origin.1 + x / metres_per_deg_lon()),
            Frame::Lambert93 { e0, n0 } => crate::lambert93::inverse(e0 + x, n0 - z),
        }
    }

    /// Latitude/longitude of local (0, 0).
    pub fn origin(self) -> (f64, f64) {
        self.unproject(0.0, 0.0)
    }
}

impl From<Frame> for (u8, f64, f64) {
    fn from(frame: Frame) -> Self {
        match frame {
            Frame::Equirect { origin } => (0, origin.0, origin.1),
            Frame::Lambert93 { e0, n0 } => (1, e0, n0),
        }
    }
}

impl TryFrom<(u8, f64, f64)> for Frame {
    type Error = String;
    fn try_from((kind, a, b): (u8, f64, f64)) -> Result<Self, String> {
        match kind {
            0 => Ok(Frame::Equirect { origin: (a, b) }),
            1 => Ok(Frame::Lambert93 { e0: a, n0: b }),
            _ => Err(format!("unknown world frame {kind}")),
        }
    }
}

/// `world.msgpack`: everything about the world except the tiles' contents.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u8,
    /// Where local (0, 0) is and how local metres map to latitude/longitude.
    pub frame: Frame,
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
    /// `THEME_*`: what the season is.
    pub theme: u8,
    /// The ski lifts, whole: both the server (riders) and the client (chairs) need every one of them
    /// wherever the player is, so they are not cut into the tiles.
    pub lifts: Vec<Lift>,
    /// Winter: the lakes and the sea stay open rather than freezing, so boats and submarines still
    /// sail. Nothing in any other season.
    pub open_water: bool,
    /// The bus lines, whole, for the same reason as the lifts: the server runs buses along them and
    /// the client stands a sign at every stop. Empty on a world with no public transport.
    pub bus_lines: Vec<BusLine>,
    /// The colour of the world's river water where it is deep, sRGB (`consts::RIVER_TINT` unless the world's
    /// `area.json` says otherwise). Only drawn: shallow water shows its bed through this colour.
    pub river_tint: [u8; 3],
    /// Whether the world's lakes and sea are that colour too, and not the clear blue they are
    /// without it. Only drawn.
    pub tint_all: bool,
    /// The fuel stations, whole for the same reason as the lifts: they stand from the start
    /// (`shared::station`), the server fills the tank of a vehicle within `FUEL_STATION_REACH` of one
    /// and heals whatever goes through its wash, and the client draws them and marks them on the map.
    /// From OpenStreetMap's `amenity=fuel`.
    pub fuel_stations: Vec<FuelStation>,
    /// The vehicle dealers (`shared::dealer`, DEALERS.md), whole for the same reason: each a carport
    /// against an OSM store's wall over its parking, where an ordered vehicle stands ready. The server
    /// sells from them and the client draws them and marks them on the map.
    pub dealers: Vec<Dealer>,
}

/// A vehicle dealer's carport (`shared::dealer`): a roof on posts against a store's wall, open on the
/// side facing the store's parking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dealer {
    /// The middle of the carport's open mouth, centimetres.
    pub at: [i32; 2],
    /// How far the carport's frame is turned about the vertical, milliradians, as a rotation about +Y:
    /// its +Z (out of the mouth onto the lot) goes to `(sin, cos)`.
    pub yaw: i16,
    /// Height of the carport's floor, centimetres.
    pub base: i32,
}

/// A fuel station (`shared::station`): a lot with a sign, a canopy over the pumps and behind it a
/// drive-through wash, all laid out in the station's own frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuelStation {
    /// The middle of the lot, centimetres.
    pub at: [i32; 2],
    /// How far the station's frame is turned about the vertical, milliradians, as a rotation about +Y:
    /// its +Z (back from the road it faces) goes to `(sin, cos)`. Its +X runs along that road.
    pub yaw: i16,
    /// Height of the lot's paving, centimetres.
    pub base: i32,
}

impl Manifest {
    /// Whether the world's water is ice: a winter world whose water was not left open.
    pub fn frozen(&self) -> bool {
        self.theme == THEME_WINTER && !self.open_water
    }
}

/// A bus line along the major roads: buses run from one end of it to the other and back, keeping to
/// the right of the road's middle, and halt at every stop.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BusLine {
    /// The middle of the road the line follows, centimetres, from one end to the other.
    pub points: Vec<[i32; 2]>,
    /// Where each stop is, as a distance along `points` from the first of them, centimetres, in
    /// order. The first is at the start and the last at the end.
    pub stops: Vec<u32>,
    /// Where the sign of each stop stands, centimetres: one a stop, on the verge of the road on the
    /// side clear of other roads, and never on a bridge.
    pub signs: Vec<[i32; 2]>,
}

impl BusLine {
    /// The length of the line, metres.
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|pair| span(pair[0], pair[1])).sum()
    }

    /// The point `along` metres from the start of the line (held to its ends), metres, and the unit
    /// direction `[x, z]` the line runs there.
    pub fn at(&self, along: f32) -> ([f32; 2], [f32; 2]) {
        let metres = |p: [i32; 2]| [p[0] as f32 / 100.0, p[1] as f32 / 100.0];
        let mut left = along.max(0.0);
        let mut last = ([0.0; 2], [0.0, -1.0]);
        for pair in self.points.windows(2) {
            let length = span(pair[0], pair[1]);
            if length < 1e-3 {
                continue;
            }
            let (a, b) = (metres(pair[0]), metres(pair[1]));
            let way = [(b[0] - a[0]) / length, (b[1] - a[1]) / length];
            if left <= length {
                return ([a[0] + way[0] * left, a[1] + way[1] * left], way);
            }
            left -= length;
            last = (b, way);
        }
        match self.points.first() {
            Some(&first) if self.points.len() == 1 => (metres(first), last.1),
            _ => last,
        }
    }

    /// How far along the line the point of it nearest `p` (metres) is, and how far from the line `p`
    /// is, both metres.
    pub fn nearest(&self, p: [f32; 2]) -> (f32, f32) {
        let metres = |q: [i32; 2]| [q[0] as f32 / 100.0, q[1] as f32 / 100.0];
        let (mut best, mut gone) = ((0.0, f32::MAX), 0.0);
        for pair in self.points.windows(2) {
            let (a, b) = (metres(pair[0]), metres(pair[1]));
            let length = span(pair[0], pair[1]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let t = if length < 1e-3 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / (length * length)).clamp(0.0, 1.0) };
            let off = (p[0] - a[0] - d[0] * t).hypot(p[1] - a[1] - d[1] * t);
            if off < best.1 {
                best = (gone + length * t, off);
            }
            gone += length;
        }
        best
    }
}

/// The distance between two points given in centimetres, metres.
fn span(a: [i32; 2], b: [i32; 2]) -> f32 {
    ((b[0] - a[0]) as f32).hypot((b[1] - a[1]) as f32) / 100.0
}

/// A ski lift: a cable loop between two stations over a line of pylons. Every lift is built as a
/// chair lift, whatever OpenStreetMap calls it (`LIFT_*` keeps what it was).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Lift {
    /// `LIFT_*`: what OpenStreetMap maps it as.
    pub kind: u8,
    /// Riders a chair carries.
    pub seats: u8,
    /// The line of the lift, centimetres: the bottom station first, then each pylon, then the top
    /// station. Chairs go up on the right of it and come down on the left.
    pub points: Vec<[i32; 2]>,
    /// Height of the ground under each of `points`, centimetres.
    pub base: Vec<i32>,
    /// Height of the cable over each of `points`, centimetres above the world's zero (not the
    /// ground): low at the stations, so a chair's seat is at a rider's knees there.
    pub cable: Vec<i32>,
}

/// A chair lift, as OpenStreetMap's `aerialway=chair_lift` (and `mixed_lift`).
pub const LIFT_CHAIR: u8 = 0;
/// A surface lift that drags skiers up: a T-bar, a button lift or a rope tow.
pub const LIFT_DRAG: u8 = 1;
/// A cabin lift: a gondola or a cable car.
pub const LIFT_CABIN: u8 = 2;
/// Any other lift, such as a magic carpet.
pub const LIFT_OTHER: u8 = 3;

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
/// A msgpack array of every field, in order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    pub collision: CollisionMesh,
    /// Ground heights, centimetres, row-major with `tile_size / ground_cell + 1` samples a side.
    /// The last row and column are shared with the neighbouring tile, so a tile stands alone.
    /// Empty when the world has no terrain. Written rounded to `GROUND_STEP` (`heights`).
    #[serde(with = "heights")]
    pub ground: Vec<i32>,
    /// What covers the ground here (water, woods, parks, ...), clipped to the tile. The client
    /// bakes it, with the roads, into the terrain's texture; nothing simulates it.
    pub cover: Vec<Cover>,
    /// Individual trees (street and park trees) inside the tile.
    pub trees: Vec<Tree>,
    /// Height of the water surface at each ground sample, centimetres (same layout as `ground`),
    /// or `NO_WATER` where there is none. Empty when the tile has no water. Lakes and wide rivers
    /// are carved into `ground`, so the depth under a sample is `water - ground`; the client draws
    /// the surface wherever that is positive. Nothing simulates it.
    #[serde(with = "samples32")]
    pub water: Vec<i32>,
    /// How wide the open water is around each ground sample, in steps of `REACH_STEP` metres (same
    /// layout as `water`; 0 where it is dry): the whole lake's width, a river's where it is, and the
    /// most a byte holds on the sea. The waves rise with it (`waves::fetch`). Empty when `water` is.
    #[serde(with = "bytes")]
    pub reach: Vec<u8>,
    /// Which way and how fast a river's water runs at each ground sample: `x` then `z`, in steps of
    /// `FLOW_STEP` m/s, two entries a sample in the layout of `water` (0 on still water and where it
    /// is dry). It carries swimmers and whatever floats downstream, and is fastest in rapids. Empty
    /// when no water in the tile flows.
    #[serde(with = "samples8")]
    pub flow: Vec<i8>,
    /// How much of a river's surface is under foam at each ground sample, in 255ths (same layout as
    /// `water`): made where the river runs white, carried downstream and gathered where the water
    /// goes round. Only drawn; nothing in play reads it. Empty when the tile has none.
    #[serde(with = "bytes")]
    pub foam: Vec<u8>,
    /// How a river's current stirs its surface at each ground sample (same layout as `water`, two
    /// entries a sample, in steps of `STIR_STEP` 1/s): first how the surface wells up (above 0, where
    /// water that dived below a drop comes back up in boils) or gathers (below 0, where the current
    /// slows into deeper water and the surface converges in a foam line), then how fast the water
    /// spins (anticlockwise seen from above above 0), which it does along the edge of a fast tongue
    /// and in the eddy behind a boulder. Only drawn. Empty when no water in the tile flows.
    #[serde(with = "samples8")]
    pub stir: Vec<i8>,
    /// The river's stream function at each ground sample (same layout as `water`): how much water
    /// runs between a fixed line and here, m³/s, in `1 / LANE_STEPS` of `LANE_FLUX`, wrapping round.
    /// A line along which it is the same is a line the water runs along, so the client draws its
    /// streaks along them. Only drawn. Empty when no water in the tile flows.
    #[serde(with = "samples16")]
    pub lane: Vec<i16>,
    /// Streams: narrow watercourses drawn as ribbons draped on the ground, clipped to the tile.
    pub streams: Vec<Stream>,
    /// Bridges' steel (pylons, stay cables): ranges of this tile's collision triangles, collided
    /// with and drawn from the same mesh. Their concrete is in `buildings` (`BUILDING_BRIDGE`).
    pub bridges: Vec<Bridge>,
    /// Fine ground relief for the client's lighting: for each texel of a square grid over the tile
    /// (row-major, rows along +Z, `n × n` with `n = sqrt(len / 2)`), the slope `(dh/dx, dh/dz)` of the
    /// ground there, each in units of `1 / DETAIL_SLOPE_SCALE`. Finer than `ground`; nothing
    /// simulates it. Empty when the tile has no relief finer than its ground or none was baked.
    #[serde(with = "samples8")]
    pub detail: Vec<i8>,
    /// Railway tracks. On the ground they are clipped to the tile; on a deck they are in the tile of
    /// the bridge's section under them (`Rail::deck`). Only drawn; nothing simulates them.
    pub rails: Vec<Rail>,
    /// Groomed ski tracks, clipped to the tile. Skis run faster on them (`ski::SkiMap`).
    pub ski: Vec<SkiTrail>,
    /// Fences, walls and power lines (`Line`). A fence or wall is solid piece by piece (`line_pieces`),
    /// a power line at its poles.
    pub lines: Vec<Line>,
    /// Stony ground: for each cell of a square grid over the tile (row-major, rows along +Z, `n × n`
    /// with `n = sqrt(len)`), how stony it is in the low bits (`STONES_DENSITY`, 0 none) and whether
    /// the stones are blocks of a boulder field rather than cobbles (`STONES_ANGULAR`). Empty when the
    /// tile has none. The client draws it; it slows whoever crosses it.
    #[serde(deserialize_with = "bits", serialize_with = "bin")]
    pub stones: Vec<u8>,
    /// Standing crops: for each cell of a square grid over the tile (laid out as `stones` is), what
    /// grows there (`CROP_*`, `CROP_NONE` for nothing). Empty when the tile has none. The client draws
    /// them, and they hide a soldier who is low enough in them from the bots (`CROP_HEIGHT`).
    #[serde(deserialize_with = "bits", serialize_with = "bin")]
    pub crops: Vec<u8>,
    /// The buildings' own walls and roofs: what the importer built before it cut them into cells
    /// (`voxels::bake`). Cells are what is blasted and what holds a building up; what is drawn and
    /// collided with is these, with the windows cut out and the pieces in blasted cells gone
    /// (`facade`). Empty when no building of the tile has one.
    pub look: Look,
    /// The tombstones of the graveyards in the tile (`Grave`): solid, and nothing breaks them.
    pub graves: Vec<Grave>,
}

/// Draw-only building surfaces of a tile (`Tile::look`): a mesh wound as the collision mesh is, and
/// for each of the tile's buildings, by index, `[first, walls, roof]`: its wall triangles from `first`,
/// then its roof triangles. `[0, 0, 0]` for a building without a look (it keeps its own collision
/// triangles).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub vertices: Vec<[i32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub buildings: Vec<[u32; 3]>,
}

impl Look {
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Building `[first, walls, roof]`'s wall and roof triangles, nine centimetre numbers each.
    pub fn parts(&self, [first, walls, roof]: [u32; 3]) -> (Vec<crate::carve::Tri>, Vec<crate::carve::Tri>) {
        let tri = |t: u32| -> Option<crate::carve::Tri> {
            let [a, b, c] = self.triangles.get(t as usize)?.map(|v| self.vertices.get(v as usize).copied())
                ;
            let (a, b, c) = (a?, b?, c?);
            Some([a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]])
        };
        ((first..first + walls).filter_map(tri).collect(), (first + walls..first + walls + roof).filter_map(tri).collect())
    }
}

/// One building's look as the server keeps it (`WorldFile::looks`): its walls and roof, nine
/// centimetre numbers a triangle, and the windows and door on them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BuildingLook {
    pub walls: Vec<crate::carve::Tri>,
    pub roof: Vec<crate::carve::Tri>,
    pub windows: Vec<crate::windows::Window>,
    /// What stands inside it and is collided with (`interior::rooms`, without the steps drawn over
    /// the stairs): floors and the stairs' slopes. Empty for a house without.
    pub inside: Vec<crate::carve::Tri>,
    /// The gates standing shut, by their number among `windows` (`windows::gates`): every gate of a
    /// building with an inside, until someone opens it. A shut gate is wall to what collides with it.
    pub shut: Vec<u32>,
}

impl BuildingLook {
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty() && self.roof.is_empty()
    }

    /// Building `building`'s look, from its triangles `range` (`[first, walls, roof]`) of its tile's
    /// `look`: its walls and roof, its windows and doors worked out from them, what stands inside it,
    /// and every gate shut. Empty for a building with no triangles of look.
    pub fn of(look: &Look, range: [u32; 3], building: &Building) -> Self {
        if range[1] + range[2] == 0 {
            return Self::default();
        }
        let (walls, roof) = look.parts(range);
        let windows = crate::windows::openings_of(look, range, building.kind, building.front);
        let inside = crate::interior::rooms(&building.footprint, &walls, &roof, &windows, building.kind).into_iter().filter(|part| part.collided()).map(|part| part.tri).collect::<Vec<_>>();
        let shut = if inside.is_empty() { Vec::new() } else { crate::windows::gates(&windows) };
        BuildingLook { walls, roof, windows, inside, shut }
    }
}

/// `Tile::stones`: the share of a cell's ground under stones, out of this.
pub const STONES_DENSITY: u8 = 0x7f;
/// `Tile::stones`: set where the stones are the angular blocks of a boulder field.
pub const STONES_ANGULAR: u8 = 0x80;
/// `Tile::crops`: nothing is grown in the cell.
pub const CROP_NONE: u8 = 0;
/// `Tile::crops`: wheat, standing `CROP_HEIGHT` high.
pub const CROP_WHEAT: u8 = 1;

fn bits<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    Ok(Bits::deserialize(deserializer)?.0)
}

/// Bytes as a msgpack `bin`, as `Bits` writes them.
fn bin<S: serde::Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(bytes)
}

/// A fence, a wall or a power line in a tile. Each straight piece of a fence or wall is a collider
/// that can be broken (`line_pieces`); a power line is solid at its poles only.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Line {
    /// `LINE_*`.
    pub kind: u8,
    /// A fence's or wall's line, cut at the tile's edges, with a point at every post (`LineKindDef::step`),
    /// so that each piece between two points breaks on its own. A power line's poles, span after span: each span is in
    /// the tile its first pole stands in, so the last point may be a pole of the next tile.
    pub points: Vec<[i32; 2]>,
    /// Height of the ground under each of `points`, centimetres: a span's far pole can stand on a
    /// tile that is not loaded.
    pub base: Vec<i32>,
    /// Whether a pole stands at the last point: false where the line carries on in another tile, which
    /// has that pole. Always true for a fence or a wall.
    pub end: bool,
}

/// A wire fence on wooden posts, waist high.
pub const LINE_FENCE: u8 = 0;
/// A game fence: wire netting two metres high on tall posts. On an orienteering map, impassable.
pub const LINE_FENCE_HIGH: u8 = 1;
/// A power line on wooden poles, with three wires on a crossarm.
pub const LINE_POWER: u8 = 2;
/// A major power line on steel pylons.
pub const LINE_POWER_MAJOR: u8 = 3;
/// A low stone wall, as round a garden or a field.
pub const LINE_WALL: u8 = 4;
/// A high stone wall, over head height: a town's wall, or one mapped as tall.
pub const LINE_WALL_HIGH: u8 = 5;
/// A steel guard rail on posts down the middle of a divided road, as on a Swedish 2+1 road (mitträcke).
pub const LINE_GUARDRAIL: u8 = 6;

/// What a kind of fence or wall is: the size of each piece's collider and what breaks it. One table
/// for the importer, the physics and the server, and the client draws to the same sizes
/// (`client/src/world.ts` reads the table from `client/src/sim/tables.generated.json`).
#[derive(Clone, Copy, Debug)]
pub struct LineKindDef {
    pub kind: u8,
    /// Height of its top over the ground, metres.
    pub height: f32,
    /// Half its thickness across the line, metres.
    pub half: f32,
    /// The longest a piece is, metres: the importer puts a point (a fence's post) this far apart.
    pub step: f32,
    /// Damage a piece takes before it breaks.
    pub hp: u16,
    /// Whether it is a wall: it stops bullets, sight and light vehicles, where a fence stops only a
    /// soldier and breaks under anything that drives into it.
    pub wall: bool,
}

/// The fences and walls. A kind of line that is not here (a power line) has no pieces.
pub const LINE_KINDS: [LineKindDef; 5] = [
    LineKindDef { kind: LINE_FENCE, height: 1.25, half: 0.06, step: 3.0, hp: 5, wall: false },
    LineKindDef { kind: LINE_FENCE_HIGH, height: 2.05, half: 0.06, step: 4.0, hp: 10, wall: false },
    LineKindDef { kind: LINE_WALL, height: 1.0, half: 0.25, step: 3.0, hp: 60, wall: true },
    LineKindDef { kind: LINE_WALL_HIGH, height: 2.4, half: 0.3, step: 3.0, hp: 200, wall: true },
    LineKindDef { kind: LINE_GUARDRAIL, height: 0.75, half: 0.1, step: 2.0, hp: 90, wall: true },
];

/// The definition of a fence or wall kind; `None` for a power line.
pub fn line_kind_def(kind: u8) -> Option<&'static LineKindDef> {
    LINE_KINDS.iter().find(|def| def.kind == kind)
}

/// One straight piece of a fence or wall, between two points of its line: what breaks on its own.
#[derive(Clone, Copy, Debug)]
pub struct LinePiece {
    /// Its tile, and `line << 16 | segment`: which of the tile's lines, and which piece of it.
    pub id: ObjId,
    pub def: &'static LineKindDef,
    /// Its ends `[x, z]`, metres.
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// The ground under each end, metres.
    pub base: [f32; 2],
}

impl LinePiece {
    /// The middle of the piece, halfway up, metres.
    pub fn centre(&self) -> [f32; 3] {
        [(self.a[0] + self.b[0]) / 2.0, (self.base[0] + self.base[1]) / 2.0 + self.def.height / 2.0, (self.a[1] + self.b[1]) / 2.0]
    }

    /// How far `point` is from the piece's line at mid height, metres.
    pub fn distance(&self, point: [f32; 3]) -> f32 {
        let d = [self.b[0] - self.a[0], self.b[1] - self.a[1]];
        let length2 = (d[0] * d[0] + d[1] * d[1]).max(1e-6);
        let u = (((point[0] - self.a[0]) * d[0] + (point[2] - self.a[1]) * d[1]) / length2).clamp(0.0, 1.0);
        let near = [self.a[0] + d[0] * u, self.base[0] + (self.base[1] - self.base[0]) * u + self.def.height / 2.0, self.a[1] + d[1] * u];
        ((point[0] - near[0]).powi(2) + (point[1] - near[1]).powi(2) + (point[2] - near[2]).powi(2)).sqrt()
    }
}

/// The index of a piece in its tile: line `line` of the tile's lines, piece `segment` of it.
pub fn line_piece_index(line: usize, segment: usize) -> u32 {
    ((line as u32) << 16) | (segment as u32 & 0xffff)
}

/// Every piece of the fences and walls of `lines`, which are each tile's lines in the tile's order
/// with its `(col, row)` (`WorldFile::lines`). Server, physics and client number them alike.
pub fn line_pieces(lines: &[(u16, u16, Line)]) -> Vec<LinePiece> {
    let mut seen: std::collections::HashMap<(u16, u16), usize> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for (col, row, line) in lines {
        let count = seen.entry((*col, *row)).or_default();
        let number = *count;
        *count += 1;
        let Some(def) = line_kind_def(line.kind) else { continue };
        let at = |i: usize| [crate::quant::from_cm(line.points[i][0]), crate::quant::from_cm(line.points[i][1])];
        let base = |i: usize| line.base.get(i).map_or(0.0, |&base| crate::quant::from_cm(base));
        for i in 1..line.points.len() {
            let id = ObjId { col: *col, row: *row, index: line_piece_index(number, i - 1) };
            out.push(LinePiece { id, def, a: at(i - 1), b: at(i), base: [base(i - 1), base(i)] });
        }
    }
    out
}

/// Units per unit of slope in `Tile::detail`: a slope of 2 (63°) is 127.
pub const DETAIL_SLOPE_SCALE: f32 = 63.5;

/// One bridge's steel in a tile's collision mesh: `steel` triangles (pylon, stay cables) from
/// `first`. Its concrete (deck, piers, parapets) is in sections that are buildings
/// (`BUILDING_BRIDGE`). A bridge crossing tile borders has one entry per tile its steel touches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bridge {
    pub first: u32,
    pub steel: u32,
}

/// The step `Tile::reach` counts in, metres: a byte reaches 1020 m, wide enough for every swell.
pub const REACH_STEP: f32 = 4.0;

/// The step `Tile::flow` counts in, m/s: a byte holds up to 6.35 m/s, faster than any rapid.
pub const FLOW_STEP: f32 = 0.05;

/// The step `Tile::stir` counts in, 1/s: a byte holds up to 0.635 a second, more than the water
/// spins or wells up anywhere but over a boulder.
pub const STIR_STEP: f32 = 0.005;

/// How much water runs between one of a river's streaks and the next, m³/s (`Tile::lane`): the
/// streaks follow the current, part round a boulder and crowd together where the water is fast or deep.
pub const LANE_FLUX: f32 = 10.0;

/// The steps of `Tile::lane` to one `LANE_FLUX`: it counts the water in 1024ths of a streak and wraps
/// round every 64 streaks (`i16`), which the client's noise repeats over too.
pub const LANE_STEPS: f32 = 1024.0;

/// `Tile::water` entry for a sample with no water.
pub const NO_WATER: i32 = i32::MIN;

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
    /// Where local (0, 0) is and how local metres map to latitude/longitude.
    pub frame: Frame,
    /// Half-extent of the playable square, centimetres.
    pub half_extent: i32,
    pub buildings: Vec<Building>,
    pub roads: Vec<Road>,
    /// Outdoor spawn points `[x, z]`, centimetres. Their height is the ground's.
    pub spawns: Vec<[i32; 2]>,
    /// Side of a tile, centimetres.
    pub tile_size: i32,
    /// Which tiles of the square grid belong to the world, `grid × grid`, row-major (`row * grid
    /// + col`). Empty means every tile does (a world built by hand).
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
    pub water: Vec<i32>,
    /// How wide the open water is around each `ground` sample, as `Tile::reach`. Filled by
    /// `assemble` with `water`.
    #[serde(skip)]
    pub reach: Vec<u8>,
    /// Which way and how fast the water runs at each `ground` sample, as `Tile::flow` (two entries a
    /// sample). Filled by `assemble` with `water`; empty when no water flows.
    #[serde(skip)]
    pub flow: Vec<i8>,
    /// `THEME_*`, from the manifest.
    #[serde(skip)]
    pub theme: u8,
    /// The ski lifts, from the manifest.
    #[serde(skip)]
    pub lifts: Vec<Lift>,
    /// Winter with the water left open, from the manifest.
    #[serde(skip)]
    pub open_water: bool,
    /// The bus lines, from the manifest.
    #[serde(skip)]
    pub bus_lines: Vec<BusLine>,
    /// The fuel stations, from the manifest.
    #[serde(skip)]
    pub fuel_stations: Vec<FuelStation>,
    /// The vehicle dealers, from the manifest.
    #[serde(skip)]
    pub dealers: Vec<Dealer>,
    /// Every ski track. Filled by `assemble`.
    #[serde(skip)]
    pub ski: Vec<SkiTrail>,
    /// Every fence, wall and power line, in its tile's order, with the `(col, row)` of the tile it came in. Filled by `assemble`.
    #[serde(skip)]
    pub lines: Vec<(u16, u16, Line)>,
    /// Every tombstone. Filled by `assemble`.
    #[serde(skip)]
    pub graves: Vec<Grave>,
    /// Each tile's stony ground (`Tile::stones`), by its `(col, row)`; tiles without any are left out.
    /// Filled by `assemble`.
    #[serde(skip)]
    pub stones: Vec<(u16, u16, Vec<u8>)>,
    /// Each tile's standing crops (`Tile::crops`), by its `(col, row)`; tiles without any are left out.
    /// Filled by `assemble`.
    #[serde(skip)]
    pub crops: Vec<(u16, u16, Vec<u8>)>,
    /// `buildings[i]`'s identity. Filled by `assemble`; empty for a world built by hand.
    #[serde(skip)]
    pub building_ids: Vec<ObjId>,
    /// `buildings[i]`'s triangles in the merged `collision` mesh, as `[start, end)`. Filled by
    /// `assemble`. `Building::first` is tile-local, so it cannot index the merged mesh.
    #[serde(skip)]
    pub building_triangles: Vec<[u32; 2]>,
    /// Each building's own walls and roof and the windows on them (`BuildingLook`), by index like
    /// `buildings`, for a building that has a look; empty for one without. Its collider is built from
    /// these (`sim::building_solid`), with the windows cut out, and a round through a window is seen.
    pub looks: Vec<BuildingLook>,
    /// `trees[i]`'s identity. Filled by `assemble`; empty for a world built by hand.
    #[serde(skip)]
    pub tree_ids: Vec<ObjId>,
    /// Vehicles the map editor placed by hand. Empty means none: nothing is placed automatically.
    #[serde(skip)]
    pub placed_vehicles: Vec<PlacedVehicle>,
    /// Items the map editor placed by hand. Empty means none: nothing lies about unless placed.
    #[serde(skip)]
    pub placed_items: Vec<PlacedItem>,
    /// Bots the map editor placed by hand. Empty means none (bar `RAGNAROK_BOTS`).
    #[serde(skip)]
    pub placed_bots: Vec<PlacedBot>,
    /// Herds of wild animals the map editor placed by hand. Empty means none.
    #[serde(skip)]
    pub placed_animals: Vec<PlacedAnimal>,
    /// The golf course beside the world (`crate::golf::COURSE_FILE`), if it has one.
    #[serde(skip)]
    pub golf: Option<crate::golf::Course>,
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

/// A herd of `count` wild animals of `kind` (`KIND_DEER`, `KIND_STAG`) placed in the map editor,
/// grazing about `[x, z]` in centimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlacedAnimal {
    pub kind: u8,
    pub at: [i32; 2],
    pub count: u8,
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
    /// `BUILDING_*`: what the building is, where that changes its windows, its inside and its colours.
    pub kind: u8,
    /// Which way the road it stands by lies from its middle, `[x, z]` of a unit vector times 127:
    /// the wall facing that way is its front, with its door (`windows::openings_of`). `[0, 0]` where
    /// no road is near, and the front is its longest wall. A shelter's is the way its open side
    /// faces (`BUILDING_SHELTER`).
    pub front: [i8; 2],
    /// The building's 1 m cubes (`carve::Shell`), worked out by the importer from its walls and
    /// roof; the server and clients make the boxes to collide with, and the clients the faces to
    /// draw, from them. Empty
    /// (`dims` all 0) for a building too big to cut, which keeps its own triangles.
    pub cells: Cells,
    /// `[wall, roof]` as `0xRRGGBB`, where the importer knows them (France: the roof from the aerial
    /// photo). Without it a client picks the colours itself.
    pub paint: Option<[u32; 2]>,
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

/// A tile's water heights written as a msgpack `bin` of little-endian `i32`s, not an array of
/// numbers: a 1 m tile has 66,049, which the client then reads as one `Int32Array`. The water is one
/// level over wide areas, so it compresses to almost nothing as it is.
mod samples32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(samples: &[i32], serializer: S) -> Result<S::Ok, S::Error> {
        let bytes: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        serializer.serialize_bytes(&bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i32>, D::Error> {
        let bytes = super::Bits::deserialize(deserializer)?.0;
        if !bytes.len().is_multiple_of(4) {
            return Err(serde::de::Error::custom("a byte count that is not a multiple of 4 for i32 samples"));
        }
        Ok(bytes.chunks_exact(4).map(|quad| i32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]])).collect())
    }
}

/// The step a tile's ground heights are rounded to when written, centimetres. Finer steps are noise
/// in the elevation data, and dropping them makes the written heights (`heights`), most of what a
/// client downloads, a fifth smaller again after compression. On a 1 m grid it moves the lighting by
/// under half a percent and changes a slope by at most 2 cm a metre. 5 cm would save a fifth more,
/// but turns gentle slopes into terraces whose slope changes by up to 8 cm a metre.
pub const GROUND_STEP: i32 = 2;

/// A tile's ground heights as written: rounded to `GROUND_STEP`, each sample in steps replaced by
/// how far it is from what its neighbours predict (left + above - above-left, a plane through them),
/// zigzagged to unsigned and split into two planes, every sample's low byte and then every high byte.
/// On smooth ground the residuals are mostly 0 and ±1, so the file compresses to under half of the
/// plain heights'. The msgpack is `[step, bin]`; a tile without terrain has an empty `bin`.
mod heights {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Bits, GROUND_STEP};

    /// The grid's side, if `len` samples make a square one (0 for none).
    fn side(len: usize) -> Option<usize> {
        let side = (len as f64).sqrt().round() as usize;
        (side * side == len).then_some(side)
    }

    /// The plane through the left, above and above-left samples at `i`, or the one sample there is
    /// on the first row or column.
    fn predict(v: &[i32], i: usize, side: usize) -> i32 {
        let (x, z) = (i % side, i / side);
        match (x > 0, z > 0) {
            (true, true) => v[i - 1].wrapping_add(v[i - side]).wrapping_sub(v[i - side - 1]),
            (true, false) => v[i - 1],
            (false, true) => v[i - side],
            (false, false) => 0,
        }
    }

    /// The residual planes of `samples`, or why they cannot be written: a grid that is not square,
    /// or a residual past 16 bits (a cliff of over 650 m between neighbouring samples).
    pub(super) fn pack(samples: &[i32], step: i32) -> Result<Vec<u8>, &'static str> {
        let side = side(samples.len()).ok_or("ground heights that are not a square grid")?;
        let steps: Vec<i32> = samples.iter().map(|&h| (h + step / 2).div_euclid(step)).collect();
        let n = steps.len();
        let mut planes = vec![0; 2 * n];
        for i in 0..n {
            let residual = steps[i] - predict(&steps, i, side);
            let zigzag = u16::try_from((residual << 1) ^ (residual >> 31)).map_err(|_| "a ground height too far from its neighbours")?;
            planes[i] = zigzag as u8;
            planes[n + i] = (zigzag >> 8) as u8;
        }
        Ok(planes)
    }

    /// The heights, centimetres, that `pack` made `planes` from at `step`.
    pub(super) fn unpack(planes: &[u8], step: i32) -> Option<Vec<i32>> {
        if !planes.len().is_multiple_of(2) {
            return None;
        }
        let n = planes.len() / 2;
        let side = side(n)?;
        let mut steps = vec![0; n];
        for i in 0..n {
            let zigzag = planes[i] as i32 | (planes[n + i] as i32) << 8;
            steps[i] = predict(&steps, i, side).wrapping_add((zigzag >> 1) ^ -(zigzag & 1));
        }
        Some(steps.into_iter().map(|s| s.wrapping_mul(step)).collect())
    }

    pub fn serialize<S: Serializer>(samples: &[i32], serializer: S) -> Result<S::Ok, S::Error> {
        let planes = pack(samples, GROUND_STEP).map_err(serde::ser::Error::custom)?;
        (GROUND_STEP, Bits(planes)).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i32>, D::Error> {
        let (step, Bits(planes)) = <(i32, Bits)>::deserialize(deserializer)?;
        if step <= 0 {
            return Err(serde::de::Error::custom("a ground step that is not positive"));
        }
        unpack(&planes, step).ok_or_else(|| serde::de::Error::custom("ground residuals that are not two planes of a square grid"))
    }
}

/// A tile's open-water reach written as a msgpack `bin`, one byte a sample.
mod bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(samples: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(samples)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        Ok(super::Bits::deserialize(deserializer)?.0)
    }
}

/// A tile's river lanes written as a msgpack `bin` of little-endian `i16`s, as `samples32` does the water.
mod samples16 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(samples: &[i16], serializer: S) -> Result<S::Ok, S::Error> {
        let bytes: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        serializer.serialize_bytes(&bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i16>, D::Error> {
        let bytes = super::Bits::deserialize(deserializer)?.0;
        if !bytes.len().is_multiple_of(2) {
            return Err(serde::de::Error::custom("a byte count that is not a multiple of 2 for i16 samples"));
        }
        Ok(bytes.chunks_exact(2).map(|pair| i16::from_le_bytes([pair[0], pair[1]])).collect())
    }
}

/// A tile's detail slopes written as a msgpack `bin`, one byte each, as `samples32` does the ground.
mod samples8 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(samples: &[i8], serializer: S) -> Result<S::Ok, S::Error> {
        let bytes: Vec<u8> = samples.iter().map(|&sample| sample as u8).collect();
        serializer.serialize_bytes(&bytes)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i8>, D::Error> {
        Ok(super::Bits::deserialize(deserializer)?.0.into_iter().map(|byte| byte as i8).collect())
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

/// A building like any other: a house, a block, a barn.
pub const BUILDING_PLAIN: u8 = 0;
/// A church or a chapel: a few tall round-headed windows along its long walls, a door and a round
/// window in a gable end, and one open hall inside (`windows::openings_of`, `interior::rooms`).
pub const BUILDING_CHURCH: u8 = 1;
/// A barn, a cowshed or a hay barn out in a field: a gate wide enough to drive in by in a long
/// wall, a few small windows or none, and one hall inside under a sheet-metal roof.
pub const BUILDING_BARN: u8 = 2;
/// What stands beside a house: a garage, a shed, a storehouse. One storey, a wide door and one
/// small window.
pub const BUILDING_OUTBUILDING: u8 = 3;
/// A factory, a workshop or a warehouse: sheet-metal walls, a gate for lorries in each long wall,
/// a ribbon of windows high up, and one hall inside.
pub const BUILDING_HALL: u8 = 4;
/// A shop or a supermarket: a front of tall panes with its door in the middle, and blank walls
/// round the rest of its one hall.
pub const BUILDING_SHOP: u8 = 5;
/// A block of flats, a dormitory or a hotel: a house's rows of windows and floors, in render or brick.
pub const BUILDING_FLATS: u8 = 6;
/// A school, a library, a town hall or a police station: big windows a storey apart, a double
/// door, floors and stairs, in yellow or red brick.
pub const BUILDING_PUBLIC: u8 = 7;
/// A cabin or a hut out on its own: a small house of dark timber.
pub const BUILDING_CABIN: u8 = 8;
/// An old wooden house, as on a farm or in the old part of a village: tall narrow windows close
/// together, in falu red, ochre or white boards.
pub const BUILDING_FARMHOUSE: u8 = 9;
/// A villa of the 1960s and 70s: wide low windows far apart, in brick, pale boards or render.
pub const BUILDING_VILLA: u8 = 10;
/// A fire station: a row of garage doors for the engines in its front, a public building's windows
/// round the rest, and one hall inside to drive into.
pub const BUILDING_FIRE_STATION: u8 = 11;
/// A section of a bridge: its deck, edge beams, railings and pier between two mid-spans
/// (`osm-import`'s `bridge::sections`). Drawn and collided from its own concrete, carved by blasts as
/// a house is but much harder to hurt (`combat`), standing on the ground under it rather than on its
/// lowest cells; no windows, no inside, no colours of a house. Its roof is the top of the deck, the
/// roadway, painted as what it carries (`DECK_ASPHALT`, `DECK_PLANKS`, `DECK_BALLAST`); everything else is
/// its walls, in `BRIDGE_CONCRETE`.
pub const BUILDING_BRIDGE: u8 = 12;
/// The colour of a bridge's concrete, `0xRRGGBB`: its sections' walls (sides, piers, edge beams and
/// railings).
pub const BRIDGE_CONCRETE: u32 = 0xb8b6ae;
/// The colour of the top of a deck that carries a paved road or path, `0xRRGGBB`: the road's
/// asphalt, drawn with its pattern.
pub const DECK_ASPHALT: u32 = 0x6e6e70;
/// The colour of the top of a deck that carries a footpath or a track, `0xRRGGBB`: weathered timber
/// planks laid across it.
pub const DECK_PLANKS: u32 = 0x8a7356;
/// The colour of the top of a deck that carries a railway, `0xRRGGBB`: the track's bed of crushed
/// stone. The sleepers and rails on it are the tile's `rails` (`Rail::deck`).
pub const DECK_BALLAST: u32 = 0x8a8478;
/// A lean-to (vindskydd) out on a trail or by a ski slope: three timber walls and a roof that is high
/// over the open side and falls towards the back, no door and no windows. `Building::front` is the
/// way the open side faces (OSM `direction`), and the importer leaves that wall out.
pub const BUILDING_SHELTER: u8 = 13;
/// How many kinds of building there are.
pub const BUILDING_KINDS: usize = 14;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub width: i32,
    /// `ROAD_*`.
    pub kind: u8,
    /// Whether a bridge's deck carries this stretch. It is driven, routed along and mapped like any
    /// road, but it is not drawn on the ground, which is the river bed or the way under the bridge.
    pub carried: bool,
    pub points: Vec<[i32; 2]>,
    /// How many lanes it has, both ways together; 0 for a path or a track, which have none.
    pub lanes: u8,
    /// How many of its `lanes` run the way its `points` do; on a one-way road, all of them.
    pub forward: u8,
    /// Which lines are painted on it, `MARK_*` flags; 0 for none.
    pub marks: u8,
    /// How far along the way it was cut from its first point is, centimetres: a dashed line's rhythm
    /// carries on across tile borders from it.
    pub along: i32,
    /// Whether it is driven one way only: along its `points` when its `forward` lanes are all of its
    /// `lanes`, against them when it has none `forward`. A road of a single lane can be either.
    pub one_way: bool,
}

/// White dashed lines along both edges, as on Swedish country roads (kantlinje): 1 m of paint, 2 m gap.
pub const MARK_EDGE: u8 = 1;
/// Solid white lines along both edges, as on a motorway or a motorroad. Wins over `MARK_EDGE`.
pub const MARK_EDGE_SOLID: u8 = 2;
/// Dashed lines between lanes that run the same way: 3 m of paint, 9 m gap.
pub const MARK_LANES: u8 = 4;
/// A dashed line between the two directions: 3 m of paint, 9 m gap.
pub const MARK_CENTRE: u8 = 8;
/// A solid line between the two directions (spärrlinje), where overtaking is barred. Wins over `MARK_CENTRE`.
pub const MARK_CENTRE_SOLID: u8 = 16;
/// No edge line on the left of the way its points run: a carriageway with a barrier beside it on that side.
pub const MARK_NO_LEFT_EDGE: u8 = 32;
/// The edge line on the left of the way its points run is solid: a carriageway of a slow divided road,
/// paved up to the other one, where the line between them may not be crossed.
pub const MARK_LEFT_SOLID: u8 = 64;

/// A railway track's centre line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rail {
    /// Distance between the rails, centimetres.
    pub gauge: i32,
    /// `RAIL_*`.
    pub kind: u8,
    pub points: Vec<[i32; 2]>,
    /// The deck that carries it, or `None` where it lies on the ground.
    pub deck: Option<RailDeck>,
}

/// What a stretch of track on a bridge lies on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RailDeck {
    /// The bridge's section (`BUILDING_BRIDGE`) it lies on, by its place in the same tile's
    /// `buildings`: the track is gone where that section's cells are.
    pub building: u32,
    /// Height of the deck's top under each of the track's points, centimetres.
    pub tops: Vec<i32>,
}

/// A groomed ski track's centre line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SkiTrail {
    /// Groomed width, centimetres.
    pub width: i32,
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
/// Paved ground round shops, schools and garages.
pub const COVER_URBAN: u8 = 5;
pub const COVER_INDUSTRIAL: u8 = 6;
/// Forest with undergrowth, from an orienteering map's greens: the higher, the denser.
pub const COVER_THICKET_SLOW: u8 = 7;
pub const COVER_THICKET_WALK: u8 = 8;
pub const COVER_THICKET_FIGHT: u8 = 9;
/// Marsh, by how wet: the plain kind is ordinary marsh, deep marsh cannot be crossed and has open
/// pools, faint marsh is damp ground.
pub const COVER_MARSH: u8 = 10;
/// Bare rock: OSM's `natural=bare_rock`, and an orienteering map's rock and the ground along its cliffs.
pub const COVER_ROCK: u8 = 11;
pub const COVER_MARSH_DEEP: u8 = 12;
pub const COVER_MARSH_FAINT: u8 = 13;
/// A golf course's playing surfaces, from OSM's `golf=*`: mown fairway, the green, a tee and a
/// bunker's sand.
pub const COVER_GOLF_FAIRWAY: u8 = 14;
pub const COVER_GOLF_GREEN: u8 = 15;
pub const COVER_GOLF_TEE: u8 = 16;
pub const COVER_GOLF_BUNKER: u8 = 17;
/// The gardens of a residential area (`landuse=residential`): mown lawn, with gravel round the houses.
pub const COVER_GARDEN: u8 = 18;
/// Asphalt laid as an area, not as a road: a car park (OSM's `amenity=parking`).
pub const COVER_ASPHALT: u8 = 19;

/// Whether cover `kind` is one of a golf course's playing surfaces, kept clear of trees and rocks.
pub fn is_golf_cover(kind: u8) -> bool {
    (COVER_GOLF_FAIRWAY..=COVER_GOLF_BUNKER).contains(&kind)
}

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

/// One tombstone of a graveyard. It stands upright on the ground, is solid, and nothing breaks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grave {
    /// Where it stands, centimetres.
    pub x: i32,
    pub z: i32,
    /// Height of the ground at its foot, centimetres.
    pub base: i32,
    /// The way its face looks, milliradians about the vertical: at 0 it faces +Z, and it turns
    /// towards +X as the angle grows.
    pub yaw: i16,
    /// `GRAVE_*`.
    pub kind: u8,
}

/// An upright slab with a rounded top.
pub const GRAVE_HEADSTONE: u8 = 0;
/// A stone cross on a plinth.
pub const GRAVE_CROSS: u8 = 1;
/// A low, wide slab leaning back on its base.
pub const GRAVE_LOW: u8 = 2;

/// The size of a kind of tombstone: its collider is this box, and the client draws it to fill the
/// same one (`client/src/world.ts` reads the table from `client/src/sim/tables.generated.json`).
#[derive(Clone, Copy, Debug)]
pub struct GraveKindDef {
    pub kind: u8,
    pub name: &'static str,
    /// Half its width across its face, metres.
    pub half_width: f32,
    /// Half its thickness from face to back, metres.
    pub half_depth: f32,
    /// Height of its top over the ground, metres.
    pub height: f32,
}

/// The tombstones. The low stone still stands higher than a soldier steps, so that a bot goes round it.
pub const GRAVE_KINDS: [GraveKindDef; 3] = [
    GraveKindDef { kind: GRAVE_HEADSTONE, name: "Headstone", half_width: 0.3, half_depth: 0.08, height: 0.95 },
    GraveKindDef { kind: GRAVE_CROSS, name: "Cross", half_width: 0.28, half_depth: 0.09, height: 1.2 },
    GraveKindDef { kind: GRAVE_LOW, name: "Low stone", half_width: 0.42, half_depth: 0.16, height: 0.58 },
];

/// The definition of a kind of tombstone. Unknown kinds are headstones, as they are drawn.
pub fn grave_kind_def(kind: u8) -> &'static GraveKindDef {
    GRAVE_KINDS.iter().find(|def| def.kind == kind).unwrap_or(&GRAVE_KINDS[0])
}

impl Grave {
    /// The way its face looks, radians.
    pub fn heading(&self) -> f32 {
        f32::from(self.yaw) / 1000.0
    }
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
/// A golf hole's flagstick and the cup it stands in, never solid. `height` is the pole's.
pub const TREE_FLAG: u8 = 10;

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

pub const TREE_KINDS: [TreeKindDef; 11] = [
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
    TreeKindDef { destructible: false, ..tree_kind(TREE_FLAG, "Flag", 2, 230) },
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

/// The roads that are driven on, asphalt whatever their `surface` tag says: there is no gravel road.
pub const ROAD_MAJOR: u8 = 0;
pub const ROAD_MINOR: u8 = 1;
pub const ROAD_PATH: u8 = 2;
/// Worn into the ground, from an orienteering map: a vehicle track's two ruts, and a trace barely
/// worn into the ground.
pub const ROAD_TRACK: u8 = 3;
pub const ROAD_FAINT: u8 = 4;
/// A footway or cycleway with asphalt on it: drawn and ridden as asphalt, walked as a path.
pub const ROAD_PAVED_PATH: u8 = 5;

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
    /// Whether the world's water is ice, as `Manifest::frozen`.
    pub fn frozen(&self) -> bool {
        self.theme == THEME_WINTER && !self.open_water
    }

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
            frame: manifest.frame,
            half_extent: manifest.half_extent,
            spawns: manifest.spawns.clone(),
            tile_size: manifest.tile_size,
            theme: manifest.theme,
            lifts: manifest.lifts.clone(),
            open_water: manifest.open_water,
            bus_lines: manifest.bus_lines.clone(),
            fuel_stations: manifest.fuel_stations.clone(),
            dealers: manifest.dealers.clone(),
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
            world.ski.extend(tile.ski.iter().cloned());
            world.lines.extend(tile.lines.iter().map(|line| (col, row, line.clone())));
            world.graves.extend(tile.graves.iter().copied());
            if !tile.stones.is_empty() {
                world.stones.push((col, row, tile.stones.clone()));
            }
            if !tile.crops.is_empty() {
                world.crops.push((col, row, tile.crops.clone()));
            }
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
                    if world.reach.is_empty() {
                        world.reach = vec![0; ground.heights.len()];
                    }
                    let reach = tile.reach.len() == tile.water.len();
                    let flow = tile.flow.len() == 2 * tile.water.len();
                    if flow && world.flow.is_empty() {
                        world.flow = vec![0; 2 * ground.heights.len()];
                    }
                    for j in 0..n {
                        let (at, from) = (((j0 + j) * ground.side + i0) as usize, (j * n) as usize);
                        world.water[at..at + n as usize].copy_from_slice(&tile.water[from..from + n as usize]);
                        if reach {
                            world.reach[at..at + n as usize].copy_from_slice(&tile.reach[from..from + n as usize]);
                        }
                        if flow {
                            world.flow[2 * at..2 * (at + n as usize)].copy_from_slice(&tile.flow[2 * from..2 * (from + n as usize)]);
                        }
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
                let range = tile.look.buildings.get(index).copied().unwrap_or([0; 3]);
                world.looks.push(BuildingLook::of(&tile.look, range, building));
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
    fn only_rocks_and_flags_are_indestructible_and_every_kind_is_in_the_table() {
        for (index, def) in TREE_KINDS.iter().enumerate() {
            assert_eq!(def.kind as usize, index, "{} sits at its own id", def.name);
            assert_eq!(object(def.kind, 500, 100).destructible(), !matches!(def.kind, TREE_ROCK_BOULDER | TREE_ROCK_SLAB | TREE_FLAG));
        }
    }

    #[test]
    fn every_kind_of_tombstone_is_in_the_table_and_stands_higher_than_a_step() {
        for (index, def) in GRAVE_KINDS.iter().enumerate() {
            assert_eq!(def.kind as usize, index, "{} sits at its own id", def.name);
            assert!(def.height > 0.5 && def.half_width > def.half_depth, "{}", def.name);
        }
        assert_eq!(grave_kind_def(200).kind, GRAVE_HEADSTONE);
        assert!((Grave { yaw: 1571, ..Default::default() }.heading() - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn assemble_names_each_building_and_its_merged_triangles() {
        let manifest = Manifest { version: WORLD_VERSION, half_extent: 20_000, tile_size: 20_000, ..Default::default() };
        // Tile A: two buildings (4 and 2 triangles) then a bridge (2). Tile B: one building (3).
        let building = |first| Building { first, ..Default::default() };
        let mesh = |triangles: usize| CollisionMesh { vertices: vec![[0, 0, 0]; 3], triangles: vec![[0, 1, 2]; triangles] };
        let a = Tile {
            buildings: vec![building(0), building(4)],
            bridges: vec![Bridge { first: 6, steel: 2 }],
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
    fn both_frames_round_trip_their_degrees() {
        let sweden = Frame::Equirect { origin: (63.8225, 20.235) };
        let france = Frame::Lambert93 { e0: 652_000.0, n0: 6_862_000.0 };
        for (frame, (lat, lon)) in [(sweden, (63.83, 20.25)), (france, (48.86, 2.35))] {
            let [x, z] = frame.project(lat, lon);
            let (back_lat, back_lon) = frame.unproject(x, z);
            assert!((back_lat - lat).abs() < 1e-9 && (back_lon - lon).abs() < 1e-9, "{frame:?}");
        }
        // +X east and +Z south. Lambert-93's grid north is turned from true north by the meridians'
        // convergence, half a degree near Paris: about 6 m of z over these 730 m of x.
        let [x, z] = france.project(48.86, 2.36);
        let [x0, z0] = france.project(48.86, 2.35);
        assert!(x - x0 > 700.0 && (z - z0).abs() < 10.0, "{} {}", x - x0, z - z0);
        let [_, z1] = france.project(48.87, 2.35);
        assert!(z1 < z0);
    }

    #[test]
    fn a_fence_or_wall_is_numbered_piece_by_piece_within_its_tile_and_a_power_line_has_none() {
        let line = |kind, points: &[[i32; 2]]| Line { kind, points: points.to_vec(), base: vec![100; points.len()], end: true };
        let lines = vec![
            (0, 0, line(LINE_POWER, &[[0, 0], [5000, 0]])),
            (0, 0, line(LINE_FENCE, &[[0, 0], [300, 0], [600, 0]])),
            (1, 0, line(LINE_WALL, &[[0, 0], [0, 300]])),
        ];
        let pieces = line_pieces(&lines);
        let ids: Vec<ObjId> = pieces.iter().map(|piece| piece.id).collect();
        assert_eq!(ids, vec![ObjId { col: 0, row: 0, index: 1 << 16 }, ObjId { col: 0, row: 0, index: (1 << 16) | 1 }, ObjId { col: 1, row: 0, index: 0 }]);
        assert_eq!((pieces[1].a, pieces[1].b, pieces[1].base), ([3.0, 0.0], [6.0, 0.0], [1.0, 1.0]));
        assert!(pieces[2].def.wall && !pieces[0].def.wall);
        // A point beside the middle of the wall, at half its height, is that far from it.
        assert!((pieces[2].distance([2.0, 1.5, 1.5]) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn manifest_and_tile_round_trip() {
        let manifest = Manifest {
            version: WORLD_VERSION,
            frame: Frame::Lambert93 { e0: 652_000.0, n0: 6_862_000.0 },
            half_extent: 30_000,
            tile_size: 25_000,
            spawns: vec![[10, 20]],
            tiles: vec![TileRef { col: 1, row: 0, hash: 7 }],
            ground_cell: 5_000,
            theme: THEME_WINTER,
            lifts: vec![Lift { kind: LIFT_CHAIR, seats: 4, points: vec![[0, 0], [5_000, 20_000]], base: vec![100, 900], cable: vec![410, 1_210] }],
            open_water: true,
            bus_lines: vec![BusLine { points: vec![[0, 0], [30_000, 0], [30_000, 40_000]], stops: vec![0, 30_000, 70_000], signs: vec![[0, 560], [30_000, 560], [30_560, 40_000]] }],
            river_tint: [70, 80, 40],
            tint_all: true,
            fuel_stations: vec![FuelStation { at: [1_200, -3_400], yaw: 1_571, base: 250 }],
            dealers: vec![Dealer { at: [-5_000, 800], yaw: -3_000, base: 410 }],
        };
        assert_eq!(Manifest::decode(&manifest.encode()).unwrap(), manifest);
        // A bus line knows its length, and where it is and which way it runs any distance along it.
        let line = &manifest.bus_lines[0];
        assert_eq!(line.length(), 700.0);
        assert_eq!(line.at(100.0), ([100.0, 0.0], [1.0, 0.0]));
        assert_eq!(line.at(500.0), ([300.0, 200.0], [0.0, 1.0]));
        assert_eq!(line.at(900.0), ([300.0, 400.0], [0.0, 1.0]));
        assert_eq!(line.nearest([310.0, 100.0]), (400.0, 10.0));
        // A winter world whose water was left open is not frozen; one that was not, is.
        assert!(!manifest.frozen());
        assert!(Manifest { open_water: false, ..manifest.clone() }.frozen());
        let tile = Tile {
            buildings: vec![Building { height: 800, footprint: vec![[0, 0], [100, 0], [100, 100]], base: 250, first: 0, walls: 6, roof: 2, roof_kind: ROOF_GABLE, kind: BUILDING_CHURCH, front: [0, -127], cells: Cells { origin: [-100, 0, 200], dims: [2, 1, 5], solid: Bits(vec![0b1011, 3]), roof: Bits(vec![1, 0]) }, paint: Some([0xeee3c8, 0x4f5866]) }, Building { height: 300, ..Default::default() }],
            roads: vec![Road { width: 600, kind: ROAD_MINOR, carried: false, points: vec![[0, 0], [500, 500]], ..Default::default() }],
            collision: CollisionMesh { vertices: vec![[0, 0, 0]], triangles: vec![[0, 0, 0]] },
            ground: vec![0, 10, 20, 30],
            cover: vec![Cover { kind: COVER_WATER, rings: vec![vec![[0, 0], [100, 0], [100, 100]]] }],
            trees: vec![Tree { x: 5, z: -7, base: 120, height: 1200, crown: 220, kind: TREE_BIRCH, solid: true }],
            water: vec![NO_WATER, 40, 40, NO_WATER],
            reach: vec![0, 50, 50, 0],
            flow: vec![0, 0, 20, -4, 18, -6, 0, 0],
            foam: vec![0, 30, 200, 0],
            stir: vec![0, 0, 12, -3, -40, 127, 0, 0],
            lane: vec![0, -32768, 1024, 0],
            streams: vec![Stream { width: 200, points: vec![[0, 0], [300, 100]] }],
            bridges: vec![Bridge { first: 0, steel: 4 }],
            detail: vec![0, -3, 127, -127],
            rails: vec![Rail { gauge: 144, kind: RAIL_TRAIN, points: vec![[0, 0], [900, 50]], deck: None }, Rail { gauge: 144, kind: RAIL_TRAIN, points: vec![[900, 50], [1800, 50]], deck: Some(RailDeck { building: 1, tops: vec![620, 640] }) }],
            ski: vec![SkiTrail { width: 400, points: vec![[0, 0], [900, 50]] }],
            lines: vec![Line { kind: LINE_POWER, points: vec![[0, 0], [5000, 0]], base: vec![120, 150], end: false }, Line { kind: LINE_WALL, points: vec![[0, 0], [300, 0]], base: vec![120, 125], end: true }],
            stones: vec![0, 40, STONES_ANGULAR | 90, 0],
            crops: vec![CROP_NONE, CROP_WHEAT, CROP_WHEAT, CROP_NONE],
            look: Look { vertices: vec![[0, 0, 0], [100, 0, 0], [0, 300, 0]], triangles: vec![[0, 1, 2]], buildings: vec![[0, 1, 0], [0, 0, 0]] },
            graves: vec![Grave { x: 40, z: -60, base: 120, yaw: -1571, kind: GRAVE_CROSS }],
        };
        assert_eq!(Tile::decode(&tile.encode()).unwrap(), tile);
        assert_eq!(tile.encode()[..3], [0xdc, 0, 22], "an array of all 22 fields");
        // An empty tile is every field still, each empty.
        let empty = Tile::default().encode();
        assert_eq!(empty[..3], [0xdc, 0, 22]);
        assert_eq!(Tile::decode(&empty).unwrap(), Tile::default());
        // A tile without its last field, the tombstones (`[]`, its last byte), is refused.
        assert_eq!(empty[empty.len() - 1], 0x90);
        let mut short = vec![0xdc, 0, 20];
        short.extend_from_slice(&empty[3..empty.len() - 1]);
        assert!(Tile::decode(&short).is_err());
        assert_eq!(manifest.tiles[0].path(), "tiles/1_0.msgpack");
    }

    #[test]
    fn water_and_detail_are_written_as_little_endian_bytes() {
        let tile = Tile { water: vec![NO_WATER, 40, 40], reach: vec![0, 7, 255], flow: vec![0, 0, 12, -80, 127, -128], foam: vec![0, 9, 250], stir: vec![0, 0, 4, -2, -128, 127], lane: vec![0, -2, 32767], detail: vec![-1, 5], ..Default::default() };
        let bytes = tile.encode();
        // msgpack `bin 8` (0xc4), its length, then the samples low byte first: what the client reads
        // as an `Int32Array` or `Int8Array` (`decodeTile`).
        let find = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(find(&[0xc4, 12, 0, 0, 0, 0x80, 40, 0, 0, 0, 40, 0, 0, 0]));
        assert!(find(&[0xc4, 2, 0xff, 5]));
        assert!(find(&[0xc4, 3, 0, 7, 255]), "the reach, a byte a sample");
        assert!(find(&[0xc4, 6, 0, 0, 12, 0xb0, 127, 0x80]), "the flow, two signed bytes a sample");
        assert!(find(&[0xc4, 3, 0, 9, 250]), "the foam, a byte a sample");
        assert!(find(&[0xc4, 6, 0, 0, 4, 0xfe, 0x80, 127]), "the stir, two signed bytes a sample");
        assert!(find(&[0xc4, 6, 0, 0, 0xfe, 0xff, 0xff, 0x7f]), "the lane, a little-endian i16 a sample");
        assert_eq!(Tile::decode(&bytes).unwrap(), tile);
    }

    #[test]
    fn ground_is_written_rounded_as_residuals_of_a_plane_through_its_neighbours() {
        // A 2 × 2 grid on a slope, one sample off it by a centimetre.
        let tile = Tile { ground: vec![100, 120, 140, 161], ..Default::default() };
        let bytes = tile.encode();
        // `[2, bin]`: in steps 50, 60, 70, 81 → residuals 50, 10, 20, 1 (81 - (60 + 70 - 50)),
        // zigzagged 100, 20, 40, 2, low bytes then high.
        let find = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        assert!(find(&[0x92, 2, 0xc4, 8, 100, 20, 40, 2, 0, 0, 0, 0]), "{bytes:x?}");
        assert_eq!(Tile::decode(&bytes).unwrap().ground, [100, 120, 140, 162], "rounded to the step");
        // Written again, it is the same.
        let again = Tile::decode(&bytes).unwrap();
        assert_eq!(Tile::decode(&again.encode()).unwrap(), again);
    }

    #[test]
    fn ground_reads_back_rounded_and_refuses_what_it_cannot_pack() {
        let hills: Vec<i32> = (0..9).map(|i| i * 37 - 140).collect();
        for ground in [hills, Vec::new()] {
            let back = Tile::decode(&Tile { ground: ground.clone(), ..Default::default() }.encode()).unwrap().ground;
            let rounded: Vec<i32> = ground.iter().map(|&h| (h + GROUND_STEP / 2).div_euclid(GROUND_STEP) * GROUND_STEP).collect();
            assert_eq!(back, rounded, "{ground:?}");
        }
        assert!(heights::pack(&[1, -2, 300], GROUND_STEP).is_err(), "not a square grid");
        assert!(heights::pack(&[0, 0, 0, 200_000], GROUND_STEP).is_err(), "a 2 km cliff");
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
        let mut reach = vec![0; 9];
        reach[4] = 9;
        let mut flow = vec![0; 18];
        flow[8..10].copy_from_slice(&[20, -7]);
        let wet = Tile { ground: vec![0; 9], water: pond, reach, flow, ..Default::default() };
        let world = WorldFile::assemble(&manifest, [(0, 0, dry.clone()), (1, 0, wet), (0, 1, dry.clone()), (1, 1, dry)]);
        assert_eq!(world.water.len(), 25);
        assert_eq!((world.reach.len(), world.reach[5 + 3]), (25, 9), "the reach is joined with the water");
        assert_eq!((world.flow.len(), &world.flow[2 * (5 + 3)..2 * (5 + 3) + 2]), (50, &[20i8, -7][..]), "and so is the flow");
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
        let tile = |h: i32| Tile { ground: vec![h; 9], ..Default::default() };
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
