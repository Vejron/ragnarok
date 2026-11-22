# World generation

How `client/public/world.msgpack` and `client/public/tiles/` are made, from which open data, and
how to run, cache and debug it. The format itself is described in [design.md](design.md) §8; this
file is about the *pipeline*. The code is `crates/osm-import`.

```
OpenStreetMap ────────────┐  buildings, roads, railways, cover, golf, lifts, ski tracks,
  (Geofabrik extract,      │  lakes, rivers, streams, sea ► water surface, carved beds
   else Overpass)          │
Lantmäteriet DTM  ────────┤  terrain heights ──► ground grid
Lantmäteriet laser ───────┤  roof shapes ──────► buildings' roofs
                          │  canopy ───────────► trees (every tree in the woods too)
Umeå tree register ───────┤  planted trees ────► archetypes by species
NMD2023 species layers ───┘  forest mix ───────► species of the laser's trees
                              │
                      osm-import (assemble) ─► world.msgpack + tiles/{col}_{row}.msgpack
```

## Running it

```sh
set -a; . ./.env.local; set +a        # credentials, see below
cargo run -p osm-import               # 8.6 km square of central Umeå, Backen to Nydala, 256 m tiles
scripts/world.sh                      # the same: loads .env.local, builds in release mode
```

Restart the server afterwards: it reads the whole world at start. The importer checks the terrain first (relief is limited to 1500 m, `MAX_RELIEF`, and the error says where the extremes are), then fetches. It wipes
`client/public/tiles/` and writes the manifest last.

| Flag | Meaning |
|---|---|
| `--center lat,lon --half m` | Square around a point (default 63.8225,20.235, 4300 m: Backen to Nydala). The world is always a square: `--bbox` uses the largest one around its centre. |
| `--bbox s,w,n,e` | An exact box; overrides `--center`/`--half`. |
| `--tile m` | Tile side, default 256 m (the editor grid's). |
| `--cell m` | Terrain sample spacing, default 4 m (or an `--area` world's `cell`), must divide `--tile`; 1, 2 or 4 m make a 256 m tile `2^k + 1` samples a side, which the client simplifies best. `0` is flat ground and needs no credentials. |
| `--dem file.tif` | Take elevation from a local DTM GeoTIFF instead of Lantmäteriet. |
| `--no-water` | Leave out lakes, rivers and streams (skips the water download). |
| `--no-roofs` | Buildings stay boxes of their tagged height. The point cloud (the slowest and biggest download) is then only read for trees; add `--no-lidar-trees` to skip it completely. |
| `--no-trees` | Skip the tree register. |
| `--no-minimap` | Skip the pre-rendered top-down map: `minimap.webp` (2048 px, 4096 under a `minimap_image`; a lossy WebP next to `world.msgpack`, the overview the settings page and the embeds show) and the [map tiles](#map-tiles) the game shows. |
| `--maptiles world.msgpack` | Only redraw that world's map (`minimap.webp` and `maptiles/`) from its tiles as they are, and exit: a second or two. A world whose `area.json` lays a print over its map needs a whole import. |
| `--no-lidar-trees` | Only the register's trees: no woods, no garden trees. |
| `--min-tree-height m` | Shortest tree taken from the laser, default 3 m above the terrain. |
| `--species dir` | Where the unpacked NMD2023 species layers are, default `temp-maps/nmd2023`. See [Tree species](#tree-species). |
| `--ocad map.ocd` | An orienteering map to build from where it reaches. See [Orienteering maps](#orienteering-maps). |
| `--synthetic` | Offline procedural grid city with hills and a few trees. Needs no network at all. |
| `--fetch-osm sweden` | Download Geofabrik's extract (or take a local `.osm.pbf`), index it into `.cache/osm/` and exit. Worlds it covers then need no Overpass. See [Local OpenStreetMap extract](#local-openstreetmap-extract). |
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

## Tree species

The laser says where a tree is and how big. In Swedish forest, what it is comes from Naturvårdsverket's
NMD2023 species layers (CC0, credit "NMD, Naturvårdsverket"), in `species.rs`. A tree from a register
keeps the register's species, and France does not use the layers.

The layers are one 14 GB download, used locally and not fetched by the importer. Unpack the five that
are read once, about 10 GB, into the default directory:

```bash
unzip -j temp-maps/NMD2023_Tradslag_v1_1.zip 'NMD2023_tradslag_tall_v1_1.tif' \
  'NMD2023_tradslag_gran_v1_1.tif' 'NMD2023_tradslag_trivial_v1_1.tif' \
  'NMD2023_tradslag_adel_v1_1.tif' 'NMD2023_tradslag_lark_v1_1.tif' -d temp-maps/nmd2023
```

Without them the importer prints `no tree species` and the laser names every tree, as `canopy::classify`
always did. It was fitted to Umeå's street trees and makes a pine forest mostly birch, so a world built
that way is much worse in the woods.

Each pixel is 10 m and holds the crown share of a species, 0-100, or 255 for what is not forest. One
pixel is off by about 30 points, so the importer reads blocks of 3 × 3 pixels:

1. The block's forest pixels give its mix. With fewer than 3 of them, the block is not a stand and its
   trees are left to `canopy::classify`. That covers towns, fields and lone trees.
2. The mix becomes a whole number of trees of each species.
3. The trees rank among themselves. The dimmest in the laser's near-infrared are the broadleaves, and of
   the conifers the highest crown bases are the pines. Ranks need no threshold, so they hold across
   laser flights.
4. Shape picks the archetype of a deciduous tree: the layers call birch, aspen, alder and rowan all one
   thing.

**255 in the layers.** The product description says a layer is 0 where it was not made. In fact `adel`
and `lark` (and `bok`, `ekovradel` and `contorta`, which are not read) have 255 on about a fifth of the
forest around Umeå, so 255 means "not forest" only in `tall`, `gran` and `trivial`. Read naively, Umeå
looks like a quarter oak.

There are no archetypes for oak, beech, larch or contorta: noble broadleaves are `TREE_BROADLEAF`, larch
is `TREE_SPRUCE` and contorta stays in with the pine.

To check a world, `WORLD=kulla cargo test --release -p osm-import world_trees -- --ignored --nocapture`
prints its trees by kind and a hash of their positions, which a change of species keeps.

## Thinning

The laser finds every crown, and a forest of them costs the client more than a player would miss: foliage
is alpha-tested, so every crown that overlaps another is paid for pixel by pixel. `assemble` drops, after
the roads and bridges have taken theirs (`thin.rs`):

- **Trees against a house.** No trunk stands within `TRUNK_CLEARANCE` (1.5 m) of a wall, and no crown
  reaches one: a tree whose crown radius is more than its distance to the nearest wall goes. Register trees
  too. Rocks and shrubs may stand against a house.
- **Trees the laser read off a building or a bridge.** The cloud has only ground and everything else,
  and the map's outline or line lies a little off the real thing, so the wall of a tall house and the
  edge of a high deck come out as trees: pines with a 40 m trunk under a viaduct. A tree from the
  laser whose top is within 4 m of a roof or a roadway at least 10 m up (`reads_as_a_crown`), and
  which stands within 6 m of that house's wall (`GHOST_WALL`) or 12 m of that deck's edge
  (`GHOST_REACH`), is dropped. Planted trees are kept. In `worlds/home` that is 664 by bridges and 3,288 by houses.
- **Suppressed trees.** Tallest first, a tree no taller than `SUPPRESSED` (0.75) of a kept neighbour whose
  crown touches its own goes: it is in that tree's shade.
- **Crowded trees.** A tree nearer a kept neighbour of any height than `CROWDED` (0.6) of their two crown
  radii added goes: the two crowns are nearly one, and the taller stays.

Register trees are never thinned, only kept off the walls. In the home world 106,289 trees would stand
without it and 81,059 do: 2,688 went from against houses and 22,542 out of the woods. The import prints
both. The canopy's outline, its gaps and its tallest trees stay. To try other values on a world built
before thinning, `WORLD=<name> cargo test --release -p osm-import thinning -- --ignored --nocapture`
measures its trees; the rows after the first switch one of the two rules off.

## Orienteering maps

An OCAD map (`.ocd`, versions 9 to 2018) is a second source next to OSM, in `ocad/`. Where it reaches it
replaces OSM's roads, buildings and cover, and places what OSM does not have: every boulder, group of
boulders, stony ground and small path. Outside it nothing changes. A world names its map in `area.json`,
so that editor builds get it too:

```json
{ "tiles": [...], "cell": 1, "ocad": "temp-maps/ocad/map.ocd", "ocad_shift": [0, 0],
  "minimap_image": "temp-maps/ocad/map.png" }
```

Maps belong to their clubs, so they stay in the git-ignored `temp-maps/ocad/`. Check a new map first:

```bash
cargo run -p osm-import --bin ocd-dump -- temp-maps/ocad/map.ocd
```

That prints the version, the georeferencing, the tiles the map covers (a world needs them, and a margin)
and how many objects it has of each symbol.

- **The map must be in SWEREF 99 TM, grid code 13002, and drawn to ISOM 2017.** Anything else is refused
  with a message, and the world is built without it. The table is in `ocad/isom.rs`.
- **Placement.** The build prints `median offset` against OSM's buildings. `ocad_shift` moves the map, in
  metres east and south. Kläppudden needs none: 0.7 m over 13 buildings.
- **The outline** is the convex hull of the map's objects. Plain forest is undrawn in ISOM, so the whole
  outline is laid as wood under the rest, and no rocks are made up inside it.
- **What the map does not decide.** The terrain stays with the laser, but for the knolls and pits it adds
  (set `"cell": 1`, or they fall between the samples). The trees stay with the laser and NMD, but open land
  clears them. The water stays with OpenStreetMap's.
- **`minimap_image`** is a JPEG or PNG render of the map laid over the drawn minimap, which is then drawn
  4096 px across instead of 2048 so the map's symbols stay readable. With a world file beside it (`.pgw`,
  `.jgw` or `.wld`, as OCAD's georeferenced export writes, with a `.prj` that must be SWEREF 99 TM) it is
  placed by that, moved by `ocad_shift` as the map is. Without one it must show exactly the extent of the
  map's objects, which is what a plain OCAD export gives: `ocd-dump` prints that as paper bounds. It looks a
  few degrees turned, because the world's frame has true north up and the map SWEREF's grid north. Forest is
  not printed: a PNG's see-through pixels inside the map's outline are drawn white, and outside it the drawn
  map shows; in a JPEG, white paper joined to a corner outside the map is left out. It ships in
  `minimap.webp`, so a deployed world needs the mapmaker's permission for it as well.
- **What the symbols mean**, and what each becomes in the world: [ORIENTEERING.md](ORIENTEERING.md).
- **Fences, walls and power lines** go in the tiles' `lines` layer (`Line`, `LINE_*`), which is left
  out of a tile file that has none. A fence or wall is cut at tile borders like a road and has a point
  at every post, 3 m apart at most (4 m for a game fence); a power line is its poles, set at its bends and no more than 55 m apart (260 m for steel
  pylons), each span in the tile of its first pole, with the ground height under every point. They are
  drawn by `render/lines.ts`. A pole is a collider like a rock's, stopping soldiers, vehicles and
  bullets. Each piece of a fence or wall between two points is a collider that can be broken
  (`sim/src/fences.rs`): a fence stops soldiers only and breaks under any vehicle, a wall stops
  everything but a tank ([BARRIERS.md](BARRIERS.md)). They come from the orienteering map and from
  OSM: `barrier=fence` is a wire fence, or a game fence with `fence_type=game` or a `height` of 1.8 m
  or more; `barrier=wall` is a low stone wall, or a high one with such a `height` or as
  `barrier=city_wall`, opened where a road crosses it; `power=minor_line` is wooden poles and
  `power=line` steel pylons (`osm::line_kind`). Inside the map's outline OSM's are cut away
  and the map's stand alone. Every power line clears its lane of trees, 14 m wide under
  wooden poles and 40 m under pylons (`under_power`), leaving the shrubs: the laser reads the wires and
  pylons as trees standing in the lane.
- **Looks** (`Tile::look`): every building's own walls and roof, as the importer built them before
  `voxels::bake` cut them into 72 cm cells. Cells stay what is blasted and what holds a building up;
  what is drawn and collided with is the look, with the windows cut out of the walls
  (`shared::windows`, `render/windows.ts`) and the pieces in blasted cells left out
  (`shared::facade`, `render/lookfracture.ts`), the same on the server and every client, so a roof is
  the slope it is and a round goes through a window. A house big
  enough also gets floors and stairs, worked out from its footprint and look as the world is
  read and stored nowhere: [INTERIORS.md](INTERIORS.md).
- **Stony ground** goes in the tiles' `stones` layer: a 2 m grid of grades, the low seven bits how stony
  (`STONES_DENSITY`) and the top bit a boulder field's blocks (`STONES_ANGULAR`). It is written after
  `lines` (an empty `lines` is written to keep it in place) and left out when a tile has none, so a
  world without it is written as before. It slows soldiers on foot, and the client draws it. Only
  orienteering maps give it; [ORIENTEERING.md](ORIENTEERING.md) has how.
- **Crops** go in the tiles' `crops` layer, after `stones`: a 2 m grid of what is grown in each cell
  (`CROP_NONE`, `CROP_WHEAT`), left empty when a tile has none. Wheat stands on `landuse=farmland` and
  an orienteering map's cultivated land where the cover is still farmland once the woods, parks, marsh
  and water over it are painted, and not on a road, a track's ballast, a stream or deeper water, nor
  under a building. The client draws it and it hides soldiers from the bots: [CROPS.md](CROPS.md).
- **Not imported yet:** walls, and bridges, which stay OSM's.

## Map tiles

The game's maps (the minimap, the phone's Map app, [PHONE.md](PHONE.md)) are drawn from tiles, the way a
web map is: `maptiles/{z}/{x}_{y}.webp` next to `world.msgpack`, 512 px squares, lossy WebP at quality 80.
- **The levels.** Level 0 is one square over the whole world. Each level below splits every square in four, down to one square per world tile (256 m): 2 pixels a metre, a dozen times sharper than `minimap.webp` on a big world.
- **Drawing.** Every level is drawn from the world's shapes (`minimap::render_square`), not shrunk from the one below, so the roads stay crisp. Each square reads only the world tiles under it and their neighbours, and they are drawn in parallel.
- **Squares left out.** A square with no world tile under it is left out, and the client draws plain paper there.
- **The index.** `maptiles/index.json` gives the levels, the side of a level-0 square in metres, the world's north-west corner, and a hash of the whole render. The client asks for tiles by that hash (`?h=`), so the service worker can keep them for good and a new render is fetched afresh.
- **Size.** `home` (12.8 km) is 895 tiles in 7 levels, 10 MB, drawn in 1.5 s. A player fetches only the tiles in view: a few on the phone and on the minimap, the coarser one standing in while a finer one loads (`client/src/hud/maptiles.ts`, up to 192 kept).
- **Redrawing.** A whole import writes them. `osm-import --maptiles worlds/<name>/world.msgpack` redraws only the map of a world already built. `scripts/deploy.sh` ships them with the world.

## Which world, and when

| World | Where | Size | Made by | Used for |
|---|---|---|---|---|
| **Dev** | `client/public/dev/` (git-ignored) | ~7.9 km², 2.8 km square around 63.83976, 20.31514 (Mariehem, Umeå), 121 tiles of 256 m, ground every 1 m (`--cell 1`) | `scripts/world-dev.sh` | day-to-day development: quick to import, load and stream |
| **Big** | `client/public/world.msgpack` + `tiles/` + `minimap.webp` (git-ignored) | 8.6 km square, Backen to Nydala | `scripts/world.sh` | production (what `Dockerfile` and `scripts/deploy.sh` ship), and checking things at full scale |
| **Editor worlds** | `worlds/<name>/` (`area.json` is committed, the generated files are git-ignored) | any shape: the painted 256 m tiles of the global grid, ground at the world's `cell` (1, 2 or 4 m) | the map editor (below), or `osm-import --area worlds/<name>/area.json --out worlds/<name>/world.msgpack` | playing a chosen part of the map |
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
  browser fetches `world.msgpack`, `tiles/` and `minimap.webp` from; the dev server serves
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
(`--center 63.8225,20.235 --half 4300`, 256 m tiles, 4 m terrain cells). The output is
`client/public/world.msgpack`, `client/public/tiles/` and `client/public/minimap.webp`. The world is
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
ships `world.msgpack`, `tiles/` and `minimap.webp`, and also the world's `area.json` as
`/app/area.json`, which the server reads for the hand-placed spawns, vehicles and items. Without it
the deployed server would pick them at random. It prints the placement counts before building.

### Painting a world (map editor)

```sh
cargo run -p editor                 # API on :8700 (reads LM_USER/LM_PASS from .env.local)
cd client && npm run dev            # open http://localhost:5173/editor.html
```

Paint 256 m tiles on OpenStreetMap, name the world and save it (`worlds/<name>/area.json`, a list of
`{gx, gy}` tiles of the global grid). **Download** fills the importer's cache for the blocks the
tiles lie in, **Generate world** runs `osm-import --area` on it, and **use** makes it the active
world: restart the server (it reads `worlds/active.json`, unless `RAGNAROK_WORLD` is set) and reload the
game (in dev the client reads the same file; `VITE_WORLD_BASE` overrides).

How it works:

- **One global grid per region.** Tiles are indexed `(gx, gy)` from a fixed anchor. Sweden's is
  `grid::ORIGIN` (central Umeå), with longitude scaled at one fixed latitude for every world
  (`shared::world::SCALE_LATITUDE`). France's is cut in Lambert-93 metres from 46.5° N 3° E (see
  Regions, below). So a tile is the same ground whichever world it is in. The region picker in the
  World panel switches grids; a world's `area.json` keeps its `region` (absent means Sweden).
- **Downloads are per 2 km block of that grid** (8 x 8 tiles). The cache key of a block's Overpass
  and tree requests depends on the block only, so worlds share entries, and the editor
  can tell which blocks are cached from the keys without downloading (`grid::Block::status`). The
  height model and laser blocks were already shared.
- **The world is the smallest square of tiles that holds the painted ones**, centred on it, with
  only the painted tiles in the manifest. A tile not listed is outside the world: it has no ground,
  nothing is generated there (and no elevation is downloaded for it), spawns and pickups avoid it, and
  the server and client put boundary walls along its edge (`boundary_walls`, in the shared sim).
- **Not done yet:** the client's ground plane and minimap still cover the square; planes still turn back at
  the square's edge, not the shape's.

### Making another world

Any other area is the same command with `--out` pointing at a **dedicated** directory: the importer
deletes `<out dir>/tiles` before writing, and the default `--out` is the big world's location.

```sh
scripts/world.sh --center 63.83976,20.31514 --half 1375 --out client/public/dev/world.msgpack
```

- `--half` is half the side in metres, so the world is `2 × half` square. `--tile` must divide that
  side (256 m tiles: 2816 m is 11 tiles); `--cell` must divide `--tile`. Relief is limited to 100 m
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
| World version mismatch | the world was made by an older importer; re-import it |

## Regions: Sweden and France

A world is built from one country's data, chosen by where it lies (`region.rs`): France inside
`shared::lambert93::FRANCE` (a box around mainland France and Corsica), Sweden everywhere else.
An editor world says which in its `area.json`. The region fixes three things:

| | Sweden | France |
|---|---|---|
| World frame (`Manifest::frame`) | equirectangular around the centre, longitude scaled at `SCALE_LATITUDE` (63.8°) | Lambert-93 (EPSG:2154), shifted to the centre |
| Editor grid | 256 m tiles from central Umeå | 256 m tiles of Lambert-93 from (700 000, 6 600 000) |
| Height model and laser in | SWEREF 99 TM, from Lantmäteriet | Lambert-93, from IGN |

The fixed scale latitude suits Sweden only. At 47° N it would stretch a world 1.55 times east to
west, so French worlds use Lambert-93 as it is: conformal, true to scale within 0.3 % in mainland
France, and the grid IGN's data comes in. Its meridians converge, so a French world's grid north is
turned up to a few degrees from true north, and its tiles are drawn turned on the editor's map.

A French world needs no credentials: IGN's LiDAR HD and its 0.5 m terrain model are open data
(Etalab 2.0, credit "IGN – LiDAR HD"), fetched and cached like everything else (`ign.rs`). The laser
is classified, so it is read rather than guessed at. Roofs are measured from the building returns
(class 6, `field.rs`), buildings OSM lacks are traced from them (`footprints.rs`), bridge decks
come from class 17 and trees from class 5. Big buildings are cut into 32 m pieces. Roof colours
come from IGN's aerial photo (BD ORTHO, `paint.rs`): the photo's median inside each building, snapped
to a toon palette, stored as the building's `paint` (`--no-paint` leaves the colours to the client).
In Paris the city's tree register adds species.

```sh
cargo run --release -p osm-import -- --center 47.3966,0.6880 --half 384 --cell 1   # Tours, by Pont Wilson
```

`--dem` still takes a local GeoTIFF instead of IGN's MNT. IGN's download server allows about 10
requests a second, so a first import of a square kilometre takes a couple of minutes. How each
part works, what it costs the server and what is left is in [FRANCE.md](FRANCE.md).

## Credentials

Lantmäteriet's height model and point cloud need a (free) Geotorget account with the *Markhöjdmodell* and
*Laserdata Skog* services enabled. The importer reads two environment variables:

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
| Buildings, roads, railways, woods, parks, lakes, rivers, streams, the sea | OpenStreetMap through [Overpass](https://overpass-api.de) | ODbL, © OpenStreetMap contributors | `Building` footprints and heights, `Road`, `Rail`, `Cover`, `Tile.water`, `Tile.streams`, carved ground |
| Terrain | Lantmäteriet *Markhöjd grid 1 m* (DTM), STAC collection `dtm-cog`, cloud-optimised GeoTIFF | CC BY 4.0 | `Tile.ground`, building floors, tree bases |
| Laser points | Lantmäteriet *Laserdata Skog* (COPC), STAC collection `dsm-skoglig-copc` | CC BY 4.0 | roof fitting, trees |
| Street and park trees | Umeå kommun open data, [`trad-som-forvaltas-av-gator-och-parker`](https://opendata.umea.se/explore/assets/trad-som-forvaltas-av-gator-och-parker/) | CC0 | `Tile.trees` |

### OpenStreetMap
One read of the box (`osm::load`) returns everything any pass wants (`osm::wants_way`, `wants_relation`,
`wants_node`): buildings, highways, railways and cover (landuse, natural, leisure, and `amenity=grave_yard`) as ways with inline
geometry and multipolygon relations for cover, and also water, golf, lifts and ski tracks. Each pass then
takes its own view of the answer with the tag filter its own Overpass query once had (`Data::main`,
`osm_water::from`, `osm_golf::from`, `osm_lifts::from`, `osm_ski::from`), so a world comes out the same
whichever source served it. The read comes from a local extract when one covers every block of the world,
else from Overpass. Heights
come from the `height` tag, else `building:levels` × 3 + 1, else a guess of 6–12 m. `natural=wetland` is
marsh, by its `wetland` tag: `bog`, `swamp` and `fen` are `COVER_MARSH_DEEP`, `wet_meadow` is
`COVER_MARSH_FAINT` and the rest `COVER_MARSH` (docs/ORIENTEERING.md has how the client draws them).
`natural=bare_rock` is `COVER_ROCK`, and `landuse=residential` is `COVER_GARDEN`, lawn with gravel round
its houses, where commercial, retail and school grounds are the paved `COVER_URBAN` (docs/GROUND.md). The
outlines of natural ground (woods, heath, rock, marsh) are bent like shores away from roads, buildings and
ground in use, bare rock spreads onto rough ground beside it, and shrubs and young birch line the woods'
edges (`edges.rs`, `props::edge_scatter`; docs/GROUND.md, "Natural edges"). A
major or minor road is asphalt whatever its `surface` tag says: there is no gravel road kind, since a
beige road read as a path, not a road. A `highway=track` whose `surface` is paved (`asphalt`, `paved`, `concrete`) is a
3.5 m asphalt road (`ROAD_MINOR`); any other track is a worn path. A footway, path, cycleway or pedestrian
street with such a surface, and a cycleway or footway with no `surface` tag (nearly all are asphalt; a `path` stays a trail), is `ROAD_PAVED_PATH`: drawn and ridden as asphalt with no shoulder, but
graded, bridged and passed over as a path (no house faces it); steps stay worn. A major or minor road's width, lanes and paint come from its `lanes`, `width`, `oneway` and `lane_markings` tags (`lanes.rs`), each road keeping `Road::lanes`, `forward`, `marks` (`MARK_*`) and `along`, the distance along its way where the piece starts; a fast divided road gets a `LINE_GUARDRAIL` down its middle (`barrier.rs`). docs/ROADS.md has the rules. A `landuse=cemetery` or an
`amenity=grave_yard` is park cover with tombstones stood in it (docs/GRAVEYARDS.md). `landuse=farmland` is
farm cover with wheat grown on it (docs/CROPS.md); a farmyard, orchard, vineyard or nursery is the same cover, bare. A bridge keeps its class, which
sets its deck's height. Overpass
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

### Paths along a street (`sidepaths.rs`)
OSM maps a street's pavements and cycle track as ways of their own beside it, with a short way across
at every crossing. They add nothing to the street, and came out as a ribbon of asphalt beside the road
and a litter of stumps, so they are left out before anything reads the ways (`sidepaths::prune`): a
footway, cycleway or path tagged a `sidewalk` or a `crossing`, and one with 70 % of its length within
7 m of the edge of a street (a residential road or bigger, not a service road) and within about 30° of
its direction. A path of 40 m or less that led to one of those and now ends at nothing goes too. Steps,
paths in tunnels and paths that only touch a street stay. The rule is rough and errs towards dropping:
`worlds/home` loses 654 ways.

A short path (25 m or less, `LINK`) with a driven road at both its ends is a link across a corner or a
traffic island, and goes too. Before any of this, ways that are one road are joined (`network.rs`,
ROADS.md).

### Bridges (`bridge.rs`)
A highway way with `bridge=*` (and no `tunnel`) is still a road in `Tile.roads`, but it is also built
as geometry in the tile's collision mesh, so it is drawn and collided with like a building and
rivers no longer swallow it. OSM has no heights, so they are inferred: the deck runs between the
ground at its two ends, is lifted to the water surface plus a clearance (7 m major, 5.5 m minor,
3.5 m paths) where it crosses water, is limited to a 6 % grade, and carries on as a ramp down to
the ground if an end is still above it. Where a major or minor road on the ground passes under it,
the deck is lifted to leave 4.7 m clear under its slab (`HEADROOM_ROAD`), enough for a bus with room
to spare after the cells round its underside down, because the height model rarely has the
embankments the road above stands on. A road running alongside the deck does not count, nor one that
ends at a deck's end (a road joining the bridge), but one OSM cuts in two under the middle of the
deck does. A ramp is held up the same over a road it crosses, unless it has come down to within
0.3 m of it (`AT_GRADE`): that is a crossroads on the ground past the bridge's end. A deck over two
roads never dips between them: it runs level from one crest to the next (`fill_sags`). A ramp
follows the road that leads on from the bridge's end (`assemble::onward`: the road ending there that
leaves within 45° of the bridge's direction, and the next where that one ends), so it lands on its
road where the road bends; with no such road it carries straight on. Past a deck two carriageways
share, each carriageway has a ramp of its own, on its own line and as wide as it, at the height the
deck's profile has that far out along the line between them, with no railing between the two while
their edges are within 2 m (`STRAND_GAP`): one deck-wide ramp fitted neither road and folded over
itself at a bend. The paving between close carriageways (ROADS.md) is not a road to lead on along,
pass under or join. A bridge with an end at a point of another bridge, whose roadway there is higher,
starts on that deck at its height and has no ramp there (`joined` in `bridge::build_along`): a slip
road's bridge that leaves Kyrkbron out over the river was built from the water under its end, and
stopped far below the deck. From such an end it keeps to that bridge's roadway for as long as it
lies on it (`rides`): the two part slowly, side by side, and were at two heights where they overlapped. Where a deck or a ramp ends on the ground its last edge lies on it, 2 cm
up at each corner (`LANDING`), and a roadway less than 0.5 m up follows the ground's slope across
(`FOLLOWS`), so there is no step onto a bridge. A ground road is hidden only under the raised part of
a ramp (more than 0.3 m up); where the ramp is nearly down the road is drawn over it. A road that
joins a deck or a ramp where it stands more than 0.3 m above the ground (its end, or a point it runs
through, is a point of the bridge's way or of the road under the ramp) comes down from it on a ramp
of its own along its line, at 6 % (`assemble::slips`, `bridge::slip`), and a driven road joining the
ramp of another driven road does in turn; otherwise a slip road would end at the side of a lifted
deck, below it. A path comes down at 25 % (`PATH_GRADE`), as steps would, and its ramp lifts nothing
that meets it: at a road's grade a footpath off a bridge stayed in the air for 50 m and lifted each
path and street it met on the bank, and they the next. A road that
only ends under a deck, as OSM cuts a road in two under a bridge, passes under and gets none. Piers stand under stretches more than 2 m up, except
where a road, a track or another bridge goes under; another bridge running alongside (the other
carriageway of a divided road on a deck of its own) does not count. A footway or cycleway crossing beside a road
bridge all the way is on the same bridge: it gets no bridge of its own, and the road's deck is
widened out over it (`paths_alongside`). A deck wider than 14 m stands on two columns, one under
each half, rather than one pier wall across it (`WIDE_DECK`). Where there is
a drop beside the deck, a railing runs along each edge: a concrete edge beam 0.5 m wide rising
0.25 m above the roadway (`EDGE_BEAM_WIDTH`, `EDGE_BEAM_HEIGHT`), and on its outer half a post
0.1 m square at every 3 m cross section with a top rail at 1.1 m and a middle rail at 0.6 m
(`RAILING_HEIGHT`, `MID_RAIL_HEIGHT`). The deck is an edge beam wider each side than its road, so
the roadway between the beams keeps the road's width. The roadway is drawn as the road it carries
(`deck_paint`): asphalt for a paved road or path (`DECK_ASPHALT`), timber planks for a footpath or
a track (`DECK_PLANKS`), bare concrete for a railway. OSM does not say which bridges are cable-stayed, so pylons with stay cables
come from `LANDMARKS` in `bridge.rs` (a position and radius per bridge): Lundabron has a tower at
each end, Kolbäcksbron one in the middle. Spans under 12 m are left to the terrain, unless the
ground under them dips more than a metre.

What OSM has is made drivable before the decks are built (`assemble.rs`; BUSES.md tells how each
was found):

- **Ways of one bridge are joined.** OSM cuts a long bridge into several ways, and each built on its
  own came down to the ground where it ended. Bridge ways of a kind that meet end to end, two at a
  point, are one bridge (`joined_bridges`).
- **A bridge reaches the top of its bank.** OSM often ends a bridge part of the way down the bank,
  where the deck would stop metres below the road. A bridge whose road goes on rising more than a
  metre past its end is lengthened along that road to the top of the rise, up to 40 m
  (`onto_the_banks`).
- **A divided road crosses on one deck.** Two bridges of a kind that run beside each other all the
  way share a deck wide enough for both (`twin_decks`); the two stay roads, with no deck of their
  own (`Pylons::Carried`). Where decks of separate bridges still overlap, the railing between them
  is left out.
- **A road over a cutting gets a deck.** Where a path goes under a road OSM does not map as a
  bridge, the terrain has a trench across the road. A drivable road over a dip in the ground
  narrower than 24 m and more than 1.5 m deep gets a deck flush with the road, with no railings
  (`cuttings`, `Pylons::Flush`); the way under it stays open. The paving between close carriageways
  is not a road and gets none: along a viaduct it read the valley as a cutting and stood as a small
  bridge under the real one. A path or road OSM has in a short
  tunnel under a road is first dug out to leave headroom under that deck, with ramps down to it
  (TERRAIN-SHAPING.md, "Underpasses"), and a guard rail or fence across the road stands on the deck
  (BARRIERS.md).
- **No pier stands on a road** or a track that goes under the bridge.
- **A ramp comes down to the deck's lip** over the ground (0.12 m), not to within 0.4 m of it.

The concrete (deck, piers, railings) is cut into sections, halfway between piers, and each section is
a building of kind `BUILDING_BRIDGE` in the tile of its middle, carved by blasts as a house is but much
tougher ([BRIDGES.md](BRIDGES.md)). The steel (pylon, stay cables) goes into the tile of its position,
listed in `Tile.bridges` as `(first, steel)` ranges. Trees within 4 m of a bridge's edge are dropped (over water they are the laser's reading of
the pylon and cables). Bridge roads never offer spawns. Deck heights over the Umeälven can be
tuned by `clearance` in `bridge.rs`.

### Water
Water comes from the OpenStreetMap read (`osm_water::from`): lakes (`natural=water`, reservoirs) and
wide rivers (`waterway=riverbank`, `water=river`) are polygons, streams (`waterway=river|stream|canal`)
are centre lines in the direction the water flows, and the sea is built from the coastline (see "The sea").
The data has **no surface height or depth, and streams no width**, so:

- a lake's surface is the median of the DTM inside it (the DTM is flat over water); a lake whose
  10th and 90th percentile differ by more than 1.5 m is reported, since it is drawn flat;
- a river falls along its course (`water::river_levels`), because a whole river is often one polygon
  tens of kilometres long, and one level for all of it stands metres over its banks at one end of the
  world and sits in a trench at the other (Vindelälven falls 25 m through `worlds/vindeln`). Each
  connected stretch is measured along its length: the distance through the water from one end, whose
  whole cross-section is the start where it leaves the grid. The DTM's median every 20 m along it is
  fitted with the nearest profile that only falls one way (isotonic regression, in whichever direction
  fits better). The surface is level across the river and steps down at rapids. River polygons that
  touch are one stretch. Stretches of under 200 samples keep the median, and a stretch of 500 m or
  more is logged with its fall, discharge and speeds;
- the bed is a bowl cut below the surface: `0.3 m` at the bank, deepest at the body's inscribed
  radius `R` (the largest distance from the bank of any of its samples). The deepest point depends
  on size: lakes `1.5 + √R` (2 to 24 m), rivers `1 + 0.6·√R` (1.5 to 12 m), so a pond is about
  3.5 m, a 100 m lake about 8.5 m and a 20 m river about 3 m. The bowl is `1 − (1 − t)³` of the way
  down at `t = shore / R`: it falls away from the bank at three times its average slope and flattens
  in the middle. The bed is never more than 25 m below y = 0, which is still the lowest *dry* ground
  (`KILL_Y` is −50 m);
- a river that flows keeps only part of its bowl where it falls steeply: its bed is cut to carry
  the river's discharge by Manning's law, so a rapid is shallow (down to a quarter of its bowl, and
  1.2 m where the bowl allows) and a pool keeps its bowl. See [RIVER-FLOW.md](RIVER-FLOW.md);
- `Tile.water` holds the surface height at every ground sample (`NO_WATER` where none; samples
  next to water sit just under the bank so the drawn shoreline falls between samples). The depth
  at a sample is `water − ground`; islands are simply ground above the surface;
- `Tile.reach` holds how wide the open water is at each sample, in 4 m steps (`REACH_STEP`, so a
  byte reaches 1020 m): a lake's whole width (`2R`, the same over all of it), a river's width where
  you are (twice its local inscribed radius), and the most a byte holds on the sea. A bank sample
  takes the widest water beside it. The waves grow with it (see "Waves" below);
- `Tile.flow` holds which way and how fast a river runs at each sample, `x` then `z` as two signed
  bytes in 0.05 m/s steps (`FLOW_STEP`), empty in a tile where nothing flows. Downstream is the way
  the river's OSM centre lines are drawn where they cross it (else downhill). The current is solved
  over the river's bed as a flow (`riverflow.rs`): the same water through every cross-section, at up
  to 5 m/s, fastest mid-channel and slack behind islands. Lakes and the sea are still. See
  [RIVERS.md](RIVERS.md) and [RIVER-FLOW.md](RIVER-FLOW.md);
- `Tile.foam` holds how much of a river's surface is under foam at each sample, a byte in 255ths,
  empty in a tile with none: made where the river runs white and carried downstream by the solved
  current. Only the water shader reads it. See [RIVER-FLOW.md](RIVER-FLOW.md);
- `Tile.stir` holds how a river's current stirs its surface at each sample, two signed bytes in
  `STIR_STEP`s (0.005 1/s): how it wells up in boils or gathers into a foam line, and how fast it
  spins. `Tile.lane` holds its stream function, an `i16` a sample in 1024ths of `LANE_FLUX`
  (10 m³/s), wrapping round: the client draws the river's streaks along its lines. Both are empty
  in a tile where nothing flows, and only the water shader reads them. See
  [RIVER-SURFACE.md](RIVER-SURFACE.md);
- streams are drawn 2.5 m wide as ribbons on the terrain (`Tile.streams`), and the stretches that
  run through a lake or river polygon are dropped;
- a stream goes under the roads and railways it crosses, as through a culvert: a way tagged
  `tunnel=culvert` (or any `tunnel` but `no`) is left out, and wherever a stream line passes within
  a road's half width, or a track's ballast, plus 1.5 m (`CULVERT_SHOULDER`) of its centre line, that
  stretch is cut out (`assemble::culverted`). So the ribbon stops beyond the embankment on one side
  and starts again on the other, culvert tagged or not. Bridges are not cut: the stream runs on under
  the deck. The minimap shows the same short gap.

Water is no cover (`natural=water` is not a `Cover`). With `--cell 0` or with `--no-water` the world
has no water.

#### Waves
`shared::waves` is five Gerstner swells crossing (70, 45, 20, 10 and 6 m long), the same function of
place and server tick on the server, in the client's prediction and in the water shader
([GERSTNER-SEA.md](GERSTNER-SEA.md)). The water goes round as a swell passes, so it gathers under narrow,
sharp crests and spreads over broad, flat troughs. The swells grow with the open water they have to rise
over (fetch). A swell `L` metres long has none of its height on water narrower than `2L` and all of it on
water wider than `8L` (`FETCH`, a smoothstep between), read from `Tile.reach`. So a 20 m pond is all but
flat, a 200 m lake has a chop of about 1.2 m crest to trough, a 400 m one about 3.5 m, and water 560 m
across or more, and the sea, carries the whole swell (about 4 m crest to trough at the most, crests over
2 m). The waves also die away towards the shore, from 2.5 m deep to none at 0.3 m (`SHORE_DEPTHS`).
Where a river runs fast its surface heaves in standing waves as well (`waves::churn`, up to 22 cm
from 1.5 m/s to 3.5 m/s), which stay in place and rise and fall. The client mirrors the numbers in
`client/src/sim/waves.ts`, and `waves.spec.ts` holds the two together.

The water's surface meshes are built in the tile worker, which is sent only the parts of a tile it
reads (`tileParts`). `reach` must be among them. It was not at first: every surface was built with a
reach of 0 and drew only the flat level, while the sim floated boats and swimmers on the full swell,
so they rode waves nobody could see.

### Terrain (DTM)
STAC finds the GeoTIFFs over the box; the importer reads only the 512 KiB blocks under the world
with HTTP range requests, takes the mean over each 5 m cell and stores it as `i32` cm above the
lowest sample. Coordinates are SWEREF 99 TM (EPSG:3006), heights RH 2000. The projection from
lat/lon is our own Krüger series in `dem.rs`, checked against an independent implementation in a
test.

### Laser points (roofs)
A small COPC reader of our own (`lidar.rs`) walks the octree and decompresses only nodes touching
the world, from the newest flight of each square. Returns are bucketed in a 25 m grid; roofs are
fitted per building (see "Footprints and the roof catalogue" below). The data is
Lantmäteriet's class 1 "unclassified" for most returns, so foliage is rejected by shape, not by
class. A 2 km square is ~17 million points; reading them dominates the run time, so only the
octree nodes that touch a painted tile (plus 30 m) are read, eight nodes are decoded at a time, and
roofs are fitted on all cores, only for buildings on a painted tile. The importer prints how long
each stage took (`[laser: 41.2 s]`). The editor always runs the release importer: a debug one is
several times slower.

### Footprints and the roof catalogue
Before a roof is fitted, every footprint whose walls all run within 12° of one direction or its
right angle is squared (`square.rs`): the walls are made exactly parallel and at right angles,
jogs under 0.6 m are taken out and kinked walls made straight. It is kept only if it stays within
8% of the drawn area and 1 m of the drawn outline; a round tower or a wedge stays as drawn, and so
do footprints France cuts on its lattice (over 1000 m²).

A house (under 250 m², `roof::SPLIT_FROM`) gets its roof from a catalogue of common shapes
(`catalogue.rs`), not a free fit:

- The squared plan is cut into at most four rectangular wings. Each of its biggest rectangles
  (up to four, at least 1.5 m each way) is tried as the main wing in turn, and what is left is cut
  biggest first. On a T whose stem is wide, the stem run on through the bar is a bigger rectangle
  than the bar, and only the laser tells which way the ridge runs. A footprint that fills 85% of its
  smallest rectangle may also be that rectangle, one wing (a house drawn with a notch or a bay). Each
  plan is fitted with every shape below, and a plan pays 10% for each wing or porch it has more than
  the plainest plan of the house, so a cut that only follows a sloppy drawing loses to fewer wings.
  A side wing at least 0.6 times as deep as it is wide has its ridge run out from the main wing and
  is carried in under its roof to the ridge, so the two meet in valleys (an L, T or U house). A bit
  under 1.5 m wide is a porch, roofed as a lean-to: the slope of the wing it stands against carried
  on down past its eaves. A footprint that is not square, or needs more wings, is one wing: the
  smallest rectangle around it.
- The shapes are flat; shed (one slope along either side); gable; hip (a pyramid on a square
  wing); and gambrel (steep up to halfway in, then shallow). Gable, hip and gambrel have one eave
  height and one slope for the whole house, the ridge down the middle of every wing. A gable or hip
  slopes 9° to 50°.
- A footprint drawn a little off the house the laser saw (Kläppudden's orienteering map lies about
  0.7 m off) would read a gable as low or tilted. Before fitting, the returns are moved back under it
  by the shift, up to 1 m each way in 25 cm steps, under which a gable fits best, when that fits
  clearly better. Each plan is searched for its own shift. The roof is still built on the footprint
  as drawn, and a return moved off it is taken to lie at the eaves.
- Shapes are compared by their miss over the 75% of the returns they fit best, so a crown leaning
  over the roof does not hide its shape. The simplest shape that fits wins: each parameter past the
  first costs 15%, a hip 10% more than a gable, and nothing beats flat unless it misses by under 70%
  as much. OSM's `roof:shape` tag, where there is one, takes 30% off its shape's cost.
- A house whose best fit is flat or shed and still misses by about 0.3 m was seen only through
  trees: it gets the usual roof, a gable (or the shape OSM names) at 27° from the eaves a house its
  size has (one storey under 40 m², else two, as `cap` reckons them). A house tagged with its height,
  or tall, keeps what the laser shows.
- On a slope of 25° or more, at least 2.5 m from eave to ridge, a run of returns 2 to 6 m along the
  eave whose middle return stands 0.6 m or more over the slope is a dormer, when its own gently
  sloping roof takes 30% off what the slope round it misses by and its front is at least 1.2 m tall.
  At most three to a slope. The dormer search looks at the returns from before what stood over the
  first fit was dropped as foliage.
- In France a house keeps its measured roof (`field.rs`) when the catalogue misses its returns by
  over 0.35 m (a block of row houses, an odd roof).

A building from 250 m² up to 800 m² is fitted from the catalogue too, and keeps that roof when it
misses the returns by 0.3 m or less (a long T or L farmhouse, a plain block); it is never given the
usual roof of a house under trees. Any other building that big, and every one from 800 m² up, is
fitted freely (`roof::fit`), with the ridge always down the middle, and may be split into parts.

A church is neither: it is fitted as a nave under one steep gable with its annexes roofed lower
beside it (`church.rs`), and falls back to `roof::fit` when it is not that simple. Its windows, door
and inside are a church's too. See [CHURCHES.md](CHURCHES.md).

Barns, outbuildings, halls and shops are told from houses by their tags, the land use they stand in
and what stands round them (`classify.rs`), and each has doors, windows, an inside and colours of
its own. See [BUILDING-STYLES.md](BUILDING-STYLES.md).

The mesh (`roof::build`) cuts the footprint into pieces each under one plane of the roof. Every line
of the whole roof cuts every piece (valleys and dormer sides only the pieces of their wings), and
each corner of a piece is added to the sides of its neighbours, so pieces always meet corner to
corner and leave no cracks. A side no other piece shares is an outer wall; a side two pieces share
with the roof higher on one is a step wall facing the lower side: a dormer's front and cheeks, or
the end of a higher part. A wall whose foot stands 2 m or more over the building's lowest (a
dormer's front) gets small windows (0.8 × 0.7 m) and never the door (`shared::windows`, mirrored in
`render/windows.ts`).

To see why a house came out as it did, import its world with `ROOF_DUMP=<file>`: each house's
footprint, returns, OSM shape and eaves go into it as one JSON line. The ignored test
`catalogue::tests::survey_a_roof_dump` (`ROOF_DUMP=<file> cargo test --release -p osm-import --
--ignored survey_a_roof_dump --nocapture`) fits every house of the dump again and prints its plan
and shape; `ROOF_ONLY=<line>` prints one house's footprint, wings and roof, `ROOF_AT=x,z` (or the
`x,y,z` of an F2 screenshot's `looking.point`) does the same for the house standing there, and
`ROOF_HEIGHTS=<file>` writes each return with the fitted roof's height over it, to draw side by side.

The importer prints how many footprints it squared and how many roofs each shape got, with the
wings past the first, the dormers, the houses guessed under trees, and the French houses left to
their measured roofs. Kläppudden, re-imported: 351 of 361 footprints squared; 101 gable, 36 shed,
20 flat, 4 hip and 2 gambrel roofs, 45 side wings, 16 guessed under trees, no dormers.

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

### Local OpenStreetMap extract

```sh
cargo run --release -p osm-import -- --fetch-osm sweden     # once, and again to update it
```

downloads Geofabrik's `sweden-latest.osm.pbf` (about 950 MB) and `sweden.poly` (its border) into
**`.cache/osm/`**, then indexes it into `sweden.osmidx` (`pbf.rs`). After that every world inside Sweden
reads its OpenStreetMap from the index in under a second, with no network: the import says `osm: local
extract sweden of 2026-11-13`. A world the extract does not cover (France, or a block over the border)
falls back to Overpass, and says so. `--fetch-osm` also takes another Geofabrik region
(`europe/france/bretagne`) or the path of any `.osm.pbf` (its `.poly`, if beside it, sets what it covers;
else its header's bounding box). `--refresh` with it downloads again. `.cache/osm/` is not the download
cache, so `--clear-cache` leaves it.

The index keeps only what `osm::wants_*` keeps, with every way's geometry resolved. Indexing is three
parallel passes over the file: relations (and the ids of their member ways), ways (and their node ids),
then the nodes' coordinates. Each element is filed by the middle of its bounding box in a fine cell
(0.05° × 0.1°) if it fits in one, in a coarse cell (1° × 2°) if it fits in that, else among the few huge
ones (Vänern). Reading a box reads the cells of each level around it and keeps the elements whose lines
cross it, as Overpass's `(bbox)` does. Bump `pbf::VERSION` when what it keeps changes (a new tag in
`osm::wants_*`): an index of another version is skipped, with a message, until `--fetch-osm` is run again.

The map editor's block status counts a block covered by an extract as having its OpenStreetMap, and its
download job skips Overpass for such blocks. It opens the indexes once, at start: restart it after
`--fetch-osm`.

## The download cache

Re-running the importer would otherwise re-download hundreds of MiB each time. Everything fetched
goes through `cache.rs`:

- the Overpass answer (keyed by query, so any endpoint's answer serves). Blocks were once asked for in five
  queries (main, water, golf, lifts, ski); a block with all five cached is still read from them, and one
  with only some is asked for again as the one query;
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

Every building is baked into 72 cm cubes (`shared::carve::CELL`) as the tiles are written (`voxels.rs`, `Built::into_files`):
its `cells` hold the grid and two flags a cell (a cube is there; it is roof), packed by
`carve::pack` (each flag XORed with the one below it, then runs of zero bytes shortened), and its
walls and roof leave the collision mesh, which keeps only its floor. The server and the clients
merge the cells into boxes to collide with (`Shell::boxes`), and the clients make the faces to draw
from them. On the 450-tile Ersboda world that leaves
the tiles about as big as before (14.2 MB against 14.7 MB). Houses built wall to wall often both
have a cube in the same place, which would be drawn twice and flicker; after baking, `voxels::share`
gives each such cube to the first of them, across the whole world (35,626 cubes in Ersboda at 1 m, before the cubes were halved).
The manifest also carries what is needed whole wherever the player is, not cut into tiles: the ski
lifts and, on a world with public transport, the bus lines (`Manifest::bus_lines`, BUSES.md), which
came with `WORLD_VERSION` 22.

A road in a tile has `carried` set where a bridge's deck carries it (`WORLD_VERSION` 23): every
bridge's own road, and the stretch of a ground road that runs along under a deck over a cutting or
a bridge lengthened onto its bank (`carried_runs` in `assemble.rs`). Such a stretch is driven,
routed along and drawn on the minimap like any road, but the client does not paint it on the ground
under the deck, and nobody spawns on it. A road that crosses under a bridge is not carried.
`WORLD_VERSION` 24 added each bus stop's sign to its line (`BusLine::signs`).
`WORLD_VERSION` 25 added the graveyards' tombstones to each tile (`Tile::graves`, GRAVEYARDS.md).
`WORLD_VERSION` 26 added which way each building's road lies (`Building::front`, BUILDING-STYLES.md).
`WORLD_VERSION` 28 made a bridge's concrete into sections that are buildings (`BUILDING_BRIDGE`), leaving `Tile.bridges` only its steel (`Bridge { first, steel }`, BRIDGES.md).
`WORLD_VERSION` 30 added each river's current after `Tile.reach` (`Tile::flow`, RIVERS.md).
`WORLD_VERSION` 33 added the foam a river carries after `Tile.flow` (`Tile::foam`, RIVER-FLOW.md).
`WORLD_VERSION` 34 added the colour of the world's rivers to the manifest (`Manifest::river_tint`), and a river's `Tile::water` is now shaped by its current: not level across, and standing in waves in a rapid (RIVER-SURFACE.md).
`WORLD_VERSION` 35 added whether the lakes and the sea take that colour too (`Manifest::tint_all`).
`WORLD_VERSION` 36 added how a river's current stirs its surface after `Tile.foam` (`Tile::stir`, `Tile::lane`, RIVER-SURFACE.md).

Both sides check `WORLD_VERSION` and read only the current layout: there is no reading of older
worlds. When the layout changes, bump it in `crates/shared/src/world.rs` **and**
`client/src/world.ts` and re-import every world (`osm-import --area worlds/<name>/area.json --out worlds/<name>/world.msgpack`, or
**Generate world** in the editor). The server tests
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

### The sea
**The sea** has no polygon in OSM: coastline ways (land on the left) are joined, cut at the
edge of the world's grid and closed clockwise along it (`osm_water::sea`); islands become holes. The
height model leaves the sea out, so its surface is fixed at 0 m. Its bed is not a lake's bowl: it
follows only the distance `s` from land, `0.3 + 11.7·t·(1.5 − 0.5·t)` m with `t = s / max(S, 600 m)`
and `S` the farthest any sea sample is from land, so it shelves gently off the beach and keeps
deepening to 12 m at the far edge, with no flat floor. A world lying entirely in the sea (no
coastline crossing it) gets no sea.

## Hand-placed spawns, vehicles, items and animals

The map editor's Spawn, Vehicle, Item and Animal tools write `spawns`, `vehicles`, `items` and `animals` (latitude/longitude, plus `kind`/`yaw`, `item`/`count` or `kind`/`count`) into the world's `area.json`. They are not part of `world.msgpack`, so changing them needs a save and a server restart but no "Generate world". The server reads `area.json` next to the manifest on start. Placing any of a kind replaces the random ones: placed spawns replace the generated spawn points, placed vehicles replace the scattered vehicles, placed items replace the scattered pickups. Anything outside the painted tiles is dropped. Vehicle `yaw` is in radians as the simulation counts it (0 faces north, positive turns left).
