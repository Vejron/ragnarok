# World generation

How `client/public/world.msgpack` and `client/public/tiles/` are made, from which open data, and
how to run, cache and debug it. The format itself is described in [design.md](design.md) §8; this
file is about the *pipeline*. The code is `crates/osm-import`.

```
OpenStreetMap (Overpass) ─┐  buildings, roads, ground cover
Lantmäteriet DTM  ────────┤  terrain heights ──► ground grid
Lantmäteriet laser ───────┤  roof shapes ──────► buildings' roofs
Umeå tree register ───────┘  trees ────────────► archetypes
                              │
                      osm-import (assemble) ─► world.msgpack + tiles/{col}_{row}.msgpack
```

## Running it

```sh
set -a; . ./.env.local; set +a        # credentials, see below
cargo run -p osm-import               # 2 km square around Backen, Umeå, 250 m tiles
```

Restart the server afterwards: it reads the whole world at start. The importer wipes
`client/public/tiles/` first and writes the manifest last.

| Flag | Meaning |
|---|---|
| `--center lat,lon --half m` | Square around a point (default Backen, Umeå, 1000 m). |
| `--bbox s,w,n,e` | An exact box; overrides `--center`/`--half`. |
| `--tile m` | Tile side, default 250 m. |
| `--cell m` | Terrain sample spacing, default 5 m, must divide `--tile`. `0` is flat ground and needs no credentials. |
| `--dem file.tif` | Take elevation from a local DTM GeoTIFF instead of Lantmäteriet. |
| `--no-roofs` | Buildings stay boxes of their tagged height; skips the point cloud (the slowest and biggest download). |
| `--no-trees` | Skip the tree register. |
| `--synthetic` | Offline procedural grid city with hills and a few trees. Needs no network at all. |
| `--overpass url,url` | Overpass endpoints, tried in order (the public ones are often busy). |
| `--cache dir`, `--no-cache`, `--refresh`, `--clear-cache` | The download cache, below. |
| `--out file` | Where `world.msgpack` goes; `tiles/` is created next to it. |

What works with which credentials:

| You have | Command | You get |
|---|---|---|
| nothing | `--synthetic`, or `--cell 0` | procedural city, or OSM on flat ground, boxes for buildings |
| nothing, but a DTM tile | `--dem m708_75.tif --no-roofs` | OSM with terrain |
| Lantmäteriet credentials | default | terrain, fitted roofs, trees |

Without credentials the importer says so and builds buildings as boxes; terrain needs `--dem` or
`--cell 0`.

## Credentials

Lantmäteriet's height model and point cloud need a (free) Geotorget account with the *Markhöjdmodell*
and *Laserdata Skog* services enabled. The importer reads two environment variables:

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
| Buildings, roads, water, woods, parks | OpenStreetMap through [Overpass](https://overpass-api.de) | ODbL, © OpenStreetMap contributors | `Building` footprints and heights, `Road`, `Cover` |
| Terrain | Lantmäteriet *Markhöjd grid 1 m* (DTM), STAC collection `dtm-cog`, cloud-optimised GeoTIFF | CC BY 4.0 | `Tile.ground`, building floors, tree bases |
| Laser points | Lantmäteriet *Laserdata Skog* (COPC), STAC collection `dsm-skoglig-copc` | CC BY 4.0 | roof fitting |
| Street and park trees | Umeå kommun open data, [`trad-som-forvaltas-av-gator-och-parker`](https://opendata.umea.se/explore/assets/trad-som-forvaltas-av-gator-och-parker/) | CC0 | `Tile.trees` |

### OpenStreetMap
One Overpass query for the bounding box returns buildings, highways and cover (landuse, natural,
leisure, riverbanks) as ways with inline geometry, plus multipolygon relations for cover. Heights
come from the `height` tag, else `building:levels` × 3 + 1, else a guess of 6–12 m. Overpass
often answers 504; the importer tries every endpoint, four rounds, 15 s apart.

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
- `.cache/` is not shared between machines or CI; the generated world is what is committed.

## Output and tests

`world.msgpack` is the manifest (origin, extent, spawns, tile list with hashes); each tile is
buildings, roads, collision mesh, ground heights, cover and trees, in world-absolute centimetres.
Both sides check `WORLD_VERSION`; bump it in `crates/shared/src/world.rs` **and**
`client/src/world.ts` when the layout changes. The server test
`parked_cars_in_the_shipped_world_settle_and_stay_put` and client `world.spec.ts` read the
committed files, so regenerate and commit them with any format change.

```sh
cargo test -p osm-import        # classification, roofs, tiling, cache
# exploration (needs credentials, network): lidar_probe prints class and height statistics
cargo test -p osm-import lidar_probe -- --ignored --nocapture
```
