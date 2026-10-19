# World generation

How `client/public/world.msgpack` and `client/public/tiles/` are made, from which open data, and
how to run, cache and debug it. The format itself is described in [design.md](design.md) §8; this
file is about the *pipeline*. The code is `crates/osm-import`.

```
OpenStreetMap (Overpass) ─┐  buildings, roads, railways, ground cover
Lantmäteriet DTM  ────────┤  terrain heights ──► ground grid
Lantmäteriet Hydrografi ──┤  lakes, rivers, streams ► water surface, carved beds
Lantmäteriet laser ───────┤  roof shapes ──────► buildings' roofs
                          │  canopy ───────────► trees (every tree in the woods too)
Umeå tree register ───────┘  planted trees ────► archetypes by species
                              │
                      osm-import (assemble) ─► world.msgpack + tiles/{col}_{row}.msgpack
```

## Running it

```sh
set -a; . ./.env.local; set +a        # credentials, see below
cargo run -p osm-import               # 8.6 km square of central Umeå, Backen to Nydala, 250 m tiles
scripts/world.sh                      # the same: loads .env.local, builds in release mode
```

Restart the server afterwards: it reads the whole world at start. The importer checks the terrain first (relief is limited to 100 m, and the error says where the extremes are), then fetches. It wipes
`client/public/tiles/` and writes the manifest last.

| Flag | Meaning |
|---|---|
| `--center lat,lon --half m` | Square around a point (default 63.8225,20.235, 4300 m: Backen to Nydala; any more takes in a hill over the 100 m relief limit). The world is always a square: `--bbox` uses the largest one around its centre. |
| `--bbox s,w,n,e` | An exact box; overrides `--center`/`--half`. |
| `--tile m` | Tile side, default 250 m. |
| `--cell m` | Terrain sample spacing, default 5 m, must divide `--tile`. `0` is flat ground and needs no credentials. |
| `--dem file.tif` | Take elevation from a local DTM GeoTIFF instead of Lantmäteriet. |
| `--no-water` | Leave out lakes, rivers and streams (skips the water download). |
| `--osm-water` | Take lakes, rivers, streams and the sea from OpenStreetMap instead of Hydrografi. Slower (Overpass, one query per block), but it has the sea. |
| `--no-roofs` | Buildings stay boxes of their tagged height. The point cloud (the slowest and biggest download) is then only read for trees; add `--no-lidar-trees` to skip it completely. |
| `--no-trees` | Skip the tree register. |
| `--no-minimap` | Skip `minimap.png`, the pre-rendered top-down map (2048 px, written next to `world.msgpack`) that the client shows as minimap and map. |
| `--no-lidar-trees` | Only the register's trees: no woods, no garden trees. |
| `--min-tree-height m` | Shortest tree taken from the laser, default 3 m above the terrain. |
| `--synthetic` | Offline procedural grid city with hills and a few trees. Needs no network at all. |
| `--overpass url,url` | Overpass endpoints, tried in order (the public ones are often busy; four mirrors by default, since city-sized boxes failed on two). |
| `--overpass-chunk m` | Ask Overpass for squares of about this many metres (default 2000) and merge, so a city-sized box doesn't time out. Each square is cached, so an interrupted run resumes. |
| `--cache dir`, `--no-cache`, `--refresh`, `--clear-cache` | The download cache, below. |
| `--out file` | Where `world.msgpack` goes; `tiles/` is created next to it. |
| `--progress` | Print a `@progress {json}` line on stderr as the stage, its steps, and the download and cache rates change. The map editor runs with this, so its job panel can show them. |

What works with which credentials:

| You have | Command | You get |
|---|---|---|
| nothing | `--synthetic`, or `--cell 0` | procedural city, or OSM on flat ground, boxes for buildings |
| nothing, but a DTM tile | `--dem m708_75.tif --no-roofs` | OSM with terrain |
| Lantmäteriet credentials | default | terrain, water, fitted roofs, trees |

Without credentials the importer says so and builds buildings as boxes; terrain needs `--dem` or
`--cell 0`.

## Which world, and when

| World | Where | Size | Made by | Used for |
|---|---|---|---|---|
| **Dev** | `client/public/dev/` (git-ignored) | ~7.9 km², 2.8 km square around 63.83976, 20.31514 (Mariehem, Umeå), 121 tiles of 256 m, ground every 4 m plus a 1 m slope map (`--cell 4 --detail 1`) | `scripts/world-dev.sh` | day-to-day development: quick to import, load and stream |
| **Big** | `client/public/world.msgpack` + `tiles/` + `minimap.png` (git-ignored) | 8.6 km square, Backen to Nydala | `scripts/world.sh` | production (what `Dockerfile` and `scripts/deploy.sh` ship), and checking things at full scale |
| **Editor worlds** | `worlds/<name>/` (`area.json` is committed, the generated files are git-ignored) | any shape: the painted 250 m tiles of the global grid | the map editor (below), or `osm-import --area worlds/<name>/area.json --out worlds/<name>/world.msgpack` | playing a chosen part of the map |
| **Fixture** | `fixtures/world/` (committed) | small synthetic city | `--synthetic --out fixtures/world/world.msgpack` | server and client tests only |

They never touch each other's files: the dev world lives in its own directory, and all of them share
the download cache (below), which they only ever add to.

### Developing against the dev world

```sh
scripts/world-dev.sh                  # once (needs LM_USER/LM_PASS in .env.local; --synthetic for offline)
RAGNAROK_WORLD=client/public/dev/world.msgpack cargo run -p server
cd client && VITE_WORLD_BASE=/dev/ npm run dev
```

- The server and the client are pointed at the world independently: `RAGNAROK_WORLD` (a path to a
  manifest; the tiles are read from `tiles/` next to it) and `VITE_WORLD_BASE` (the URL prefix the
  browser fetches `world.msgpack`, `tiles/` and `minimap.png` from; the dev server serves
  `client/public/` at `/`). Set both or neither. If they disagree the page warns that its world
  differs from the server's.
- To go back to the big world, unset both and restart the server and `npm run dev`.
- Restart the server after regenerating a world: it reads the whole world at start.
- The first run downloads what the big world's cache doesn't already hold (the dev area is on the east
  edge of the big world, so some terrain and laser blocks are new, and the Overpass, tree and water
  queries are new for this area). Later runs read everything from disk.
- Flags after the script name go to the importer, and a later `--center`/`--half` wins, so
  `scripts/world-dev.sh --center 63.8225,20.235 --half 750` is a smaller world somewhere else, still
  in `client/public/dev/`.

### Generating the big world (production)

```sh
scripts/world.sh                      # defaults: Backen to Nydala, into client/public/
scripts/deploy.sh                     # builds the client, bakes world + tiles into the image
WORLD_DIR=client/public/dev scripts/deploy.sh   # ship another world (e.g. the dev world) instead
```

`scripts/world.sh` loads `.env.local`, then runs the importer in release mode with the defaults
(`--center 63.8225,20.235 --half 4300`, 250 m tiles, 5 m terrain cells). The output is
`client/public/world.msgpack`, `client/public/tiles/` and `client/public/minimap.png`. The world is
not committed, so it must exist before a build: `Dockerfile` copies `world.msgpack` and `tiles/`
into the image and sets `RAGNAROK_WORLD=/app/world.msgpack`, and `scripts/deploy.sh` stops if they
are missing. Generate it on the machine that has the cache (see below) and don't `--refresh`
casually: a re-import is only byte-identical given the same cache. `scripts/deploy.sh` removes
`client/dist/dev` after the build so the dev world is never shipped.

### Deploying an editor-made world

```sh
WORLD_DIR=worlds/<name> DOMAIN=game.example.com scripts/deploy.sh user@host
```

Generate the world first (**Generate world** in the editor) and **Save** your placements. The script
ships `world.msgpack`, `tiles/` and `minimap.png`, and also the world's `area.json` as
`/app/area.json`, which the server reads for the hand-placed spawns, vehicles and items. Without it
the deployed server would pick them at random. It prints the placement counts before building.

### Painting a world (map editor)

```sh
cargo run -p editor                 # API on :8700 (reads LM_USER/LM_PASS from .env.local)
cd client && npm run dev            # open http://localhost:5173/editor.html
```

Paint 250 m tiles on OpenStreetMap, name the world and save it (`worlds/<name>/area.json`, a list of
`{gx, gy}` tiles of the global grid). **Download** fills the importer's cache for the blocks the
tiles lie in, **Generate world** runs `osm-import --area` on it, and **use** makes it the active
world: restart the server (it reads `worlds/active.json`, unless `RAGNAROK_WORLD` is set) and reload the
game (in dev the client reads the same file; `VITE_WORLD_BASE` overrides).

How it works:

- **One global grid.** Tiles are indexed `(gx, gy)` from a fixed anchor (`grid::ORIGIN`, central
  Umeå) and longitude is scaled at one fixed latitude for every world (`shared::world::SCALE_LATITUDE`),
  so a tile is the same ground whichever world it is in.
- **Downloads are per 2 km block of that grid** (8 x 8 tiles). The cache key of a block's Overpass,
  Hydrografi and tree requests depends on the block only, so worlds share entries, and the editor
  can tell which blocks are cached from the keys without downloading (`grid::Block::status`). The
  height model and laser blocks were already shared.
- **The world is the smallest square of tiles that holds the painted ones**, centred on it, with
  only the painted tiles in the manifest. A tile not listed is outside the world: it has no ground,
  nothing is generated there (and no elevation is downloaded for it), spawns and pickups avoid it, and
  the server and client put boundary walls along its edge (`boundary_walls` / `boundaryWalls`).
- **Not done yet:** the client's ground plane and minimap still cover the square; planes still turn back at
  the square's edge, not the shape's.

### Making another world

Any other area is the same command with `--out` pointing at a **dedicated** directory: the importer
deletes `<out dir>/tiles` before writing, and the default `--out` is the big world's location.

```sh
scripts/world.sh --center 63.83976,20.31514 --half 1375 --out client/public/dev/world.msgpack
```

- `--half` is half the side in metres, so the world is `2 × half` square. `--tile` must divide that
  side (250 m tiles: 2750 m is 11 tiles); `--cell` must divide `--tile`. Relief is limited to 100 m
  across the world.
- Run the server and client against it as above (`VITE_WORLD_BASE` is the directory under
  `client/public/`, with slashes: `/mine/`).

### Troubleshooting

| Symptom | Cause |
|---|---|
| The big world's tiles vanished | an import ran with the default `--out` (or a `--out` in `client/public/`); regenerate it with `scripts/world.sh` (cached, so it is quick) |
| Page warns the world differs from the server's | `RAGNAROK_WORLD` and `VITE_WORLD_BASE` name different worlds, or the world was regenerated and the server not restarted |
| `/dev/world.msgpack: 404` in the browser | the dev world isn't generated yet, or `VITE_WORLD_BASE` has no matching directory under `client/public/` |
| `LM_USER and LM_PASS are not set` | put them in `.env.local`, or add `--synthetic` |
| World version mismatch | the world was made by an older importer; regenerate it, or, from version 10, bake it in place with `cargo run -p osm-import -- --bake worlds/<name>/world.msgpack` |

## Credentials

Lantmäteriet's height model, point cloud and water need a (free) Geotorget account with the *Markhöjdmodell*,
*Laserdata Skog* and *Hydrografi Direkt* services enabled. The importer reads two environment variables:

```
LM_USER=...
LM_PASS=...
```

Put them in **`.env.local`** at the repository root. It is git-ignored; never commit it. The
importer does not read the file itself, so load it into the shell first:

```sh
set -a; . ./.env.local; set +a
```

The credentials are only sent to `api.lantmateriet.se` and `dl1.lantmateriet.se`, as HTTP Basic
auth. They are never written to the cache or to the world files. OSM and Umeå's register need none.

## Data sources

| Data | Source | Licence | Used for |
|---|---|---|---|
| Buildings, roads, railways, woods, parks | OpenStreetMap through [Overpass](https://overpass-api.de) | ODbL, © OpenStreetMap contributors | `Building` footprints and heights, `Road`, `Rail`, `Cover` |
| Lakes, rivers, streams | Lantmäteriet *Hydrografi Direkt*, OGC API Features `ogc-features/v1/hydrografi` (`StandingWater`, `WatercoursePolygon`, `WatercourseLine`) | check the product's terms on Geotorget | `Tile.water`, `Tile.streams`, carved ground |
| Terrain | Lantmäteriet *Markhöjd grid 1 m* (DTM), STAC collection `dtm-cog`, cloud-optimised GeoTIFF | CC BY 4.0 | `Tile.ground`, building floors, tree bases |
| Laser points | Lantmäteriet *Laserdata Skog* (COPC), STAC collection `dsm-skoglig-copc` | CC BY 4.0 | roof fitting, trees |
| Street and park trees | Umeå kommun open data, [`trad-som-forvaltas-av-gator-och-parker`](https://opendata.umea.se/explore/assets/trad-som-forvaltas-av-gator-och-parker/) | CC0 | `Tile.trees` |

### OpenStreetMap
One Overpass query for the bounding box returns buildings, highways, railways and cover (landuse, natural,
leisure, riverbanks) as ways with inline geometry, plus multipolygon relations for cover. Heights
come from the `height` tag, else `building:levels` × 3 + 1, else a guess of 6–12 m. Overpass
often answers 504; the importer tries every endpoint, four rounds, 15 s apart, for each square. A box larger than `--overpass-chunk` is fetched as a grid of squares; ways and relations that cross a border come back twice and are merged by id.

### Railways
`railway=rail`, `light_rail`, `preserved` and `narrow_gauge` are `RAIL_TRAIN` tracks and `tram` is
`RAIL_TRAM`, in `Tile.rails`, cut at tile borders like roads; subways, tunnels and disused, abandoned
or planned lines are left out. The gauge is the `gauge` tag (millimetres, the first of a dual
gauge), else 1435 mm (1000 mm narrow gauge). A train track is drawn as a ballast bed 2.2 m wider
than the gauge (`BALLAST_EXTRA`, in the importer and `client/src/world.ts`) under the roads, with
sleepers and raised rails over them; a tram is rails alone, flush with the street. Nothing collides
with a track. Its ballast is gravel to the tyres (`sim/surface.ts`), it keeps trees off like a road,
it is a dark line on the minimap, and it never offers spawns. A railway with `bridge=*` gets a deck
built like a minor road bridge, but no road on it, and the rails are not drawn on the deck.

### Bridges (`bridge.rs`)
A highway way with `bridge=*` (and no `tunnel`) is still a road in `Tile.roads`, but it is also built
as geometry in the tile's collision mesh, so it is drawn and collided with like a building and
rivers no longer swallow it. OSM has no heights, so they are inferred: the deck runs between the
ground at its two ends, is lifted to the water surface plus a clearance (7 m major, 5.5 m minor,
3.5 m paths) where it crosses water, is limited to a 6 % grade, and carries on as a ramp down to
the ground if an end is still above it. Piers stand under stretches more than 2 m up, parapets
where there is a drop. OSM does not say which bridges are cable-stayed, so pylons with stay cables
come from `LANDMARKS` in `bridge.rs` (a position and radius per bridge): Lundabron has a tower at
each end, Kolbäcksbron one in the middle. Spans under 12 m are left to the terrain.
Triangles go into the tile of their position, listed in `Tile.bridges` as `(first, deck, steel)`
ranges; trees within 4 m of a bridge's edge are dropped (over water they are the laser's reading of
the pylon and cables). Bridge roads never offer spawns. Deck heights over the Umeälven can be
tuned by `clearance` in `bridge.rs`.

### Water (Hydrografi)
The default. `--osm-water` takes it from OpenStreetMap instead (see the end of this section).
Lakes (`StandingWater`) and wide rivers (`WatercoursePolygon`) are polygons; streams
(`WatercourseLine`) are centre lines in the direction the water flows. The service is queried per
2 km square (CRS84 bounding box, paged, cached like Overpass answers). It has **no surface height,
depth or width**, so:

- a body's surface is the median of the DTM inside it (the DTM is flat over water); a body whose
  10th and 90th percentile differ by more than 1.5 m is reported, since it is drawn flat;
- the bed is a bowl cut below the surface: `0.3 m` at the bank, deepest at the body's inscribed
  radius `R` (the largest distance from the bank of any of its samples). The deepest point depends
  on size: lakes `1 + 0.6·√R` (1.5 to 12 m), rivers `0.8 + 0.35·√R` (1.2 to 6 m), so a pond is about
  2 m and a 100 m lake about 7 m. The bed is never more than 12 m below y = 0, which is still the
  lowest *dry* ground;
- `Tile.water` holds the surface height at every ground sample (`NO_WATER` where none; samples
  next to water sit just under the bank so the drawn shoreline falls between samples). The depth
  at a sample is `water − ground`; islands are simply ground above the surface;
- streams are drawn 2.5 m wide as ribbons on the terrain (`Tile.streams`), and the stretches that
  run through a lake or river polygon are dropped.

OSM water (`natural=water`, `waterway=riverbank`, `landuse=basin|reservoir`) is no longer
imported. Without credentials, with `--cell 0` or with `--no-water` the world has no water.

### Terrain (DTM)
STAC finds the GeoTIFFs over the box; the importer reads only the 512 KiB blocks under the world
with HTTP range requests, takes the mean over each 5 m cell and stores it as `i16` cm above the
lowest sample. Coordinates are SWEREF 99 TM (EPSG:3006), heights RH 2000. The projection from
lat/lon is our own Krüger series in `dem.rs`, checked against an independent implementation in a
test.

### Laser points (roofs)
A small COPC reader of our own (`lidar.rs`) walks the octree and decompresses only nodes touching
the world, from the newest flight of each square. Returns are bucketed in a 25 m grid; roofs are
fitted per building as flat, shed, gable or hip, up to four parts (`roof.rs`). The data is
Lantmäteriet's class 1 "unclassified" for most returns, so foliage is rejected by shape, not by
class. A 2 km square is ~17 million points; reading them dominates the run time, so only the
octree nodes that touch a painted tile (plus 30 m) are read, eight nodes are decoded at a time, and
roofs are fitted on all cores, only for buildings on a painted tile. The importer prints how long
each stage took (`[laser: 41.2 s]`). The editor always runs the release importer: a debug one is
several times slower.

### Trees
The register has position, scientific and Swedish name, broadleaf/conifer and sometimes a planting
date, but **no height or crown size**. Each species is mapped by genus to one of seven archetypes
(see design.md §8 Trees; `classify` in `trees.rs`). Size is the archetype's, varied ±20 % by a
hash of the record id, scaled down for trees planted within 15 years of 2025, so a re-import gives
the same forest. Trees on a building footprint or within half a road's width of a road are dropped.
To add a species, extend the genus match in `classify`; its unit tests list the cultivar-name
cases (`'Erecta'`, `Fastigiata-Gruppen`, `fk ULTUNA`, "Saknas").

### Trees from the laser (`canopy.rs`)
The register is only what the municipality planted. Everything else (woods, gardens, yards) comes
from the point cloud. This product is classified only as ground (2) and *unclassified* (1), so
there is no vegetation class: trees are told from buildings by masking every OSM building outline
(plus 1.5 m of eaves) and from shrubs and cars by height.

1. **Canopy height model:** a 1 m raster of the highest non-ground return per cell minus the DTM
   at that cell. The cloud is ~1 return/m², so empty cells are filled from at least 3 known
   neighbours, then smoothed 3×3.
2. **Treetops:** local maxima at least `--min-tree-height` (3 m) tall, within a window that grows
   with height (3×3 up to 7×7 cells).
3. **Crowns:** a watershed from the tops, highest cells first, to cells at least half the top's
   height and within 8 m. Radius is that of the circle with the crown's area. Segments over
   200 m² (roofs the OSM footprints missed) or mostly interpolated are dropped.
4. **Archetype** (`canopy::classify`), from measurements of the crown: the *near-infrared
   intensity* separates conifers (about 800) from broadleaves (about 670); the height of the
   crown's lowest returns separates pine (≥ 0.47 of the height) from spruce; below 9 m a tree is
   ornamental (bright) or low-wide; the widest crowns are broadleaf and the slenderest columnar.
   Everything else is birch.
5. **Merge** (`trees::merge`): a detected tree within max(2 m, its crown) of a register tree *is*
   that tree: it keeps the register's species and takes the laser's measured height and crown.
   Unmatched register trees (saplings under 3 m) stay; unmatched detections are added.

The thresholds were fitted by the ignored `canopy::tests::calibration` test, which detects trees
over 1.2 km² and compares them with the register's species (rows register, columns rules; Umeå
east of Backen, ~850 register trees found by the laser):

| register \ rules | birch | pine | spruce | ornamental | broadleaf | columnar | low-wide |
|---|---|---|---|---|---|---|---|
| birch | 444 | 70 | 10 | 1 | 9 | 30 | 11 |
| pine | 16 | 37 | 4 | 0 | 1 | 0 | 0 |
| spruce | 4 | 5 | 14 | 1 | 1 | 4 | 0 |
| ornamental | 1 | 7 | 0 | 27 | 0 | 0 | 2 |

So about three in four birches, two in three pines, half the spruces and three in four ornamentals
are right. Broadleaf, columnar and low-wide are barely separable from the laser (young street
trees; the register's maples are 7 m tall) and are mostly wrong. Rerun it after changing the rules:
`CAL_LAT=63.835 CAL_LON=20.28 cargo test --release -p osm-import calibration -- --ignored --nocapture`.

Limits: the laser flight's date decides leaf-on or leaf-off; neighbouring crowns of the same height
merge into one tree; hedges and tall shrubs over 3 m count as trees; trees under 3 m are missed.

**Trunk colliders.** A real forest is tens of thousands of trees, so only trees with a collider
(`Tree::solid`) are the register's and those within 20 m of a road or a building; the rest are
scenery you drive through. Client and server both skip non-solid trees.

## The download cache

Re-running the importer would otherwise re-download hundreds of MiB each time. Everything fetched
goes through `cache.rs`:

- the Overpass answer (keyed by query, so any endpoint's answer serves);
- the tree register answer (keyed by bounding box);
- every STAC search;
- every 512 KiB block of DTM and point-cloud files, and each file's length.

Entries are plain files in **`.cache/osm-import/`** (git-ignored), named `kind-hash`. A default
2 km world is about 250 MiB, almost all point cloud. A second run reads it all from disk with no
network access and produces the same world, byte for byte.

| | |
|---|---|
| Use the cache (default) | `cargo run -p osm-import` |
| Clear it | `cargo run -p osm-import -- --clear-cache`, or `rm -rf .cache/osm-import` |
| Re-download and overwrite | `--refresh` |
| Bypass it | `--no-cache` |
| Put it elsewhere | `--cache /path/to/dir` |

Notes:
- Nothing expires. OSM, the tree register and new laser flights change over time; use `--refresh`
  (or clear the cache) when you want current data. A cache entry is never used for a different
  request: the key includes the whole URL or query.
- A partly downloaded run leaves a valid cache: entries are written atomically, so the next run
  resumes where it stopped.
- A different bounding box reuses whatever blocks overlap; only new blocks are fetched.
- Credentials are not part of any key or entry.
- `.cache/` is not shared between machines or CI. The generated world is **not** committed (it is
  git-ignored): given the cache a re-import is byte-identical, but without it OSM edits and new
  flights change the result, so keep the deploy machine's cache and don't `--refresh` casually.

## Output and tests

`world.msgpack` is the manifest (origin, extent, spawns, tile list with hashes); each tile is
buildings, roads, collision mesh, ground heights, cover, trees, water, streams, bridges, detail
and railway tracks, in world-absolute centimetres.

Every building is baked into 1 m cubes as the tiles are written (`voxels.rs`, `Built::into_files`):
its `cells` hold the grid and two flags a cell (a cube is there; it is roof), packed by
`carve::pack` (each flag XORed with the one below it, then runs of zero bytes shortened), and its
walls and roof leave the collision mesh, which keeps only its floor. The server and the clients
make the faces to collide with and draw from the cells. On the 450-tile Ersboda world that leaves
the tiles about as big as before (14.2 MB against 14.7 MB). `--bake` does it to a world written
before (version 10) in place, without downloading anything.
Both sides check `WORLD_VERSION`; bump it in `crates/shared/src/world.rs` **and**
`client/src/world.ts` when the layout changes. The server tests
`parked_cars_in_the_fixture_world_settle_and_stay_put` and
`the_fixture_world_gets_its_items_on_open_ground` and client `world.spec.ts` read the small
synthetic world committed in `fixtures/world/`, so with any format change regenerate and commit it:
`cargo run -p osm-import -- --synthetic --out fixtures/world/world.msgpack` (or `--bake` it, for a
change only to the buildings' cubes). The real world in
`client/public` is generated and git-ignored.

```sh
cargo test -p osm-import        # classification, roofs, tiling, cache
# exploration (needs credentials, network): lidar_probe prints class and height statistics
cargo test -p osm-import lidar_probe -- --ignored --nocapture
```

### Water from OpenStreetMap (`--osm-water`)
Lakes (`natural=water`, riverbanks, reservoirs), rivers and streams come from Overpass, one query per
2 km block. **The sea** has no polygon in OSM: coastline ways (land on the left) are joined, cut at the
edge of the world's grid and closed clockwise along it (`osm_water::sea`); islands become holes. The
height model leaves the sea out, so its surface is fixed at 0 m. A world lying entirely in the sea
(no coastline crossing it) gets no sea.

## Hand-placed spawns, vehicles and items

The map editor's Spawn, Vehicle and Item tools write `spawns`, `vehicles` and `items` (latitude/longitude, plus `kind`/`yaw` or `item`/`count`) into the world's `area.json`. They are not part of `world.msgpack`, so changing them needs a save and a server restart but no "Generate world". The server reads `area.json` next to the manifest on start. Placing any of a kind replaces the random ones: placed spawns replace the generated spawn points, placed vehicles replace the scattered vehicles, placed items replace the scattered pickups. Anything outside the painted tiles is dropped. Vehicle `yaw` is in radians as the simulation counts it (0 faces north, positive turns left).
