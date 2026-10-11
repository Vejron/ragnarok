# World generation

How `client/public/world.msgpack` and `client/public/tiles/` are made, from which open data, and
how to run, cache and debug it. The format itself is described in [design.md](design.md) §8; this
file is about the *pipeline*. The code is `crates/osm-import`.

```
OpenStreetMap (Overpass) ─┐  buildings, roads, ground cover
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
| `--no-water` | Leave out lakes, rivers and streams (skips the Hydrografi download). |
| `--no-roofs` | Buildings stay boxes of their tagged height. The point cloud (the slowest and biggest download) is then only read for trees; add `--no-lidar-trees` to skip it completely. |
| `--no-trees` | Skip the tree register. |
| `--no-lidar-trees` | Only the register's trees: no woods, no garden trees. |
| `--min-tree-height m` | Shortest tree taken from the laser, default 3 m above the terrain. |
| `--synthetic` | Offline procedural grid city with hills and a few trees. Needs no network at all. |
| `--overpass url,url` | Overpass endpoints, tried in order (the public ones are often busy; four mirrors by default, since city-sized boxes failed on two). |
| `--overpass-chunk m` | Ask Overpass for squares of about this many metres (default 2000) and merge, so a city-sized box doesn't time out. Each square is cached, so an interrupted run resumes. |
| `--cache dir`, `--no-cache`, `--refresh`, `--clear-cache` | The download cache, below. |
| `--out file` | Where `world.msgpack` goes; `tiles/` is created next to it. |

What works with which credentials:

| You have | Command | You get |
|---|---|---|
| nothing | `--synthetic`, or `--cell 0` | procedural city, or OSM on flat ground, boxes for buildings |
| nothing, but a DTM tile | `--dem m708_75.tif --no-roofs` | OSM with terrain |
| Lantmäteriet credentials | default | terrain, water, fitted roofs, trees |

Without credentials the importer says so and builds buildings as boxes; terrain needs `--dem` or
`--cell 0`.

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
| Buildings, roads, woods, parks | OpenStreetMap through [Overpass](https://overpass-api.de) | ODbL, © OpenStreetMap contributors | `Building` footprints and heights, `Road`, `Cover` |
| Lakes, rivers, streams | Lantmäteriet *Hydrografi Direkt*, OGC API Features `ogc-features/v1/hydrografi` (`StandingWater`, `WatercoursePolygon`, `WatercourseLine`) | check the product's terms on Geotorget | `Tile.water`, `Tile.streams`, carved ground |
| Terrain | Lantmäteriet *Markhöjd grid 1 m* (DTM), STAC collection `dtm-cog`, cloud-optimised GeoTIFF | CC BY 4.0 | `Tile.ground`, building floors, tree bases |
| Laser points | Lantmäteriet *Laserdata Skog* (COPC), STAC collection `dsm-skoglig-copc` | CC BY 4.0 | roof fitting, trees |
| Street and park trees | Umeå kommun open data, [`trad-som-forvaltas-av-gator-och-parker`](https://opendata.umea.se/explore/assets/trad-som-forvaltas-av-gator-och-parker/) | CC0 | `Tile.trees` |

### OpenStreetMap
One Overpass query for the bounding box returns buildings, highways and cover (landuse, natural,
leisure, riverbanks) as ways with inline geometry, plus multipolygon relations for cover. Heights
come from the `height` tag, else `building:levels` × 3 + 1, else a guess of 6–12 m. Overpass
often answers 504; the importer tries every endpoint, four rounds, 15 s apart, for each square. A box larger than `--overpass-chunk` is fetched as a grid of squares; ways and relations that cross a border come back twice and are merged by id.

### Water (Hydrografi)
Lakes (`StandingWater`) and wide rivers (`WatercoursePolygon`) are polygons; streams
(`WatercourseLine`) are centre lines in the direction the water flows. The service is queried per
2 km square (CRS84 bounding box, paged, cached like Overpass answers). It has **no surface height,
depth or width**, so:

- a body's surface is the median of the DTM inside it (the DTM is flat over water); a body whose
  10th and 90th percentile differ by more than 1.5 m is reported, since it is drawn flat;
- the bed is cut below the surface by `0.3 m + 0.2 m` per metre from the shore (lakes, at most
  2.5 m; rivers `0.15`, at most 1.2 m), never more than 3 m below y = 0, which is still the lowest
  *dry* ground;
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
class. A 2 km square is ~17 million points; reading them dominates the run time.

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
buildings, roads, collision mesh, ground heights, cover and trees, in world-absolute centimetres.
Both sides check `WORLD_VERSION`; bump it in `crates/shared/src/world.rs` **and**
`client/src/world.ts` when the layout changes. The server tests
`parked_cars_in_the_fixture_world_settle_and_stay_put` and
`the_fixture_world_gets_its_items_on_open_ground` and client `world.spec.ts` read the small
synthetic world committed in `fixtures/world/`, so with any format change regenerate and commit it:
`cargo run -p osm-import -- --synthetic --out fixtures/world/world.msgpack`. The real world in
`client/public` is generated and git-ignored.

```sh
cargo test -p osm-import        # classification, roofs, tiling, cache
# exploration (needs credentials, network): lidar_probe prints class and height statistics
cargo test -p osm-import lidar_probe -- --ignored --nocapture
```
