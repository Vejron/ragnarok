# France: worlds from IGN LiDAR HD

The plan, and where it stands, for building worlds anywhere in France from IGN's open LiDAR. How
to generate one is in [WORLD.md](WORLD.md) (Regions).

## Why

The importer was built around Swedish data. Lantmäteriet's point cloud has about 1 return a square
metre and only two classes, ground and everything else. So the importer guesses:
- roofs are fitted to shapes, and foliage is rejected by shape;
- trees are told from buildings by masking OSM outlines;
- bridge decks are inferred from grade rules and water clearances.

IGN's **LiDAR HD** covers France under Etalab 2.0 (free for any use, credit "IGN – LiDAR HD"),
with no account. It has about 17 returns a square metre and full ASPRS classes, with a 0.5 m
terrain model (MNT) beside it. With that, buildings, trees, bridges and water can be read off the
data instead of guessed.

| Class | Meaning | Use |
|---|---|---|
| 2 | ground | (the MNT is made from it) |
| 3, 4, 5 | vegetation under 0.5 m, 0.5 to 1.5 m, over 1.5 m | 4: bushes, 5: trees |
| 6 | building | measured roofs, and the buildings OSM lacks |
| 9 | water | (in the MNT, so in the water levels) |
| 17 | bridge deck | deck heights |
| 64 | permanent structures above ground: walls, pylons | props (later) |
| 65, 66, 67 | artefacts, virtual points under water and bridges, other built | ignored |

One hectare of central Tours (around Pont Wilson) has 88k building returns, 12k high vegetation,
21k unclassified, and 4k low and medium vegetation.

## Decisions

- **A region in master, not a fork.** `osm-import` picks Sweden or France per world, from where
  the world lies or from `"region"` in the world's `area.json`.
- **LiDAR first.** Shapes come from the classes: roofs from class 6 as measured, every tree from
  vegetation, decks from 17. Outlines stay OSM's where it has one, because French OSM outlines come
  from the cadastre and beat anything traced. Class 6 supplies the buildings OSM is missing (Phase
  3).
- **First target: a flat French town.** The relief limit was 100 m then; it is 1000 m now (Phase 5).
- **A French world's frame is Lambert-93.** The old projection scaled longitude at Umeå's latitude
  (63.8°) for every world. That would stretch a French world 1.55 times east to west. Lambert-93
  (EPSG:2154) is conformal, true to scale within 0.3 % in mainland France, and the grid IGN's data
  comes in. So a French world's metres are IGN's, shifted to its centre, and nothing is reprojected.

## How it fits together

| | Sweden | France |
|---|---|---|
| World frame (`Manifest::frame`, `shared::world::Frame`) | equirectangular, scaled at 63.8° | Lambert-93 from the world's centre |
| Editor grid (`Region::grid`) | 256 m tiles from central Umeå | 256 m tiles of Lambert-93 from (700 000, 6 600 000) |
| Terrain | Lantmäteriet DTM (STAC, SWEREF 99 TM) | IGN MNT 0.5 m (WMS-R), or `--dem` |
| Laser | Lantmäteriet Laserdata Skog (STAC, credentials) | IGN LiDAR HD COPC (WFS index, open) |
| Water | Hydrografi, or `--osm-water` | OpenStreetMap |
| Planted trees | Umeå's register | Paris's register, in Paris |

The code:
- `shared/src/lambert93.rs`: the projection. `client/src/editor/lambert93.ts` is its copy for the
  editor.
- `osm-import/src/region.rs`: `Region`, and `Place`, which holds a world's frame, half-side,
  degree box and `source` coordinates.
- `osm-import/src/ign.rs`: the IGN index, the clouds and the MNT.
- `osm-import/src/field.rs`: measured roofs. `osm-import/src/footprints.rs`: buildings OSM lacks,
  and the 32 m pieces.

### IGN's services, as found (October 2026)

- **Tile index:** the WFS `https://data.geopf.fr/wfs/ows`, layer
  `IGNF_LIDAR-HD_METADONNEE:metadata`, one feature per 1 km square and edition. A bbox query in
  EPSG:2154 also returns squares that only touch the edge, so they are filtered.
  - `coordonnees_nw` is the north-west corner in km (`0525-6701` covers E 525–526 km, N 6700–6701 km).
  - `url_npl` is the COPC file; some features still say `http://`.
  - `url_mnt`/`url_mns`/`url_mnh` are WMS-R `GetMap`s of the terrain, surface and height models.
  - `date_edition` dates the edition: keep the newest of each square.
- **Point clouds:** COPC, LAS 1.4 point format 6 (no colour), coordinates in centimetres, about
  16.7 M returns and 105 MB a km². Served from `data.geopf.fr/telechargement/download/...`, with
  range requests.
  - **The server allows about 10 requests a second** (`ratelimit-limit: 10`) and answers 429 over
    that. Unauthenticated range reads are paced at one per 115 ms across all threads, back off and
    retry, and ask for 2 MiB blocks instead of 512 KiB.
- **MNT:** WMS-R `https://data.geopf.fr/wms-r`, layer
  `IGNF_LIDAR-HD_MNT_ELEVATION.ELEVATIONGRIDCOVERAGE.LAMB93`, `image/geotiff`. The answer is an
  uncompressed float32 strip with GDAL no-data. Pixel centres lie on multiples of 0.5 m when the box
  reaches 0.25 m past them, as IGN's own links do. The importer asks for 400 m pieces on a fixed
  lattice (about 2.5 MB each), only under the painted tiles.
- **Aerial photo (BD ORTHO):** the same WMS-R, layer `HR.ORTHOIMAGERY.ORTHOPHOTOS` (20 cm), as
  `image/png`. The importer asks for the same 400 m pieces at 0.5 m: 800 × 800 pixels, about
  1.7 MB and 1.3 s each. LiDAR HD itself has no colour: every edition checked (Tours, Paris, 2025) is
  point format 6, and IGN's download catalogue has no colourised clouds.

## Progress

### Phase 1: the region and frame groundwork. Done
- [x] `shared::world::Frame` (`Equirect`/`Lambert93`) in the manifest in place of `origin`;
      `WORLD_VERSION` 15 (Rust and `client/src/world.ts`).
- [x] Lambert-93 forward and inverse, tested against IGN's worked example (ALG0003/0004), the false
      origin, round trips and scale.
- [x] `Region` and `Place` in the importer. Region-aware grid, layout, source windows, painted
      rectangles and roof lookup. Hydrografi and the tree register are skipped in France.
- [x] The server places editor placements, scenarios and the rooster with the frame.
- [x] Map editor: Sweden/France picker, `region` in `area.json`, tiles drawn as turned squares, and
      region-aware cache view and downloads.
- [x] `--bake` upgrades a version 14 world to 15 in place.
- [x] **Sweden unchanged:** the 57-tile `bug` world imports byte-identical tiles and minimap to
      before. A baked v14 manifest equals a fresh v15 one.

### Phase 2: IGN fetchers. Done
- [x] `ign::squares`: the LiDAR HD index for a window, newest edition per square, cached.
- [x] `lidar::read_files`: the COPC reader for any list of files, credentials optional;
      `lidar::fetch` keeps Lantmäteriet's STAC lookup.
- [x] `ign::mnt`: the 0.5 m MNT for the painted tiles, in cached pieces. `--dem` still overrides.
- [x] Classified laser in use: roofs are fitted to class 6 returns only, trees are found in class 5
      only.
- [x] Fixed on the way: `trees::merge` turned detections back into degrees with SWEREF 99 TM. It
      now uses the region's inverse.
- [x] Fixed on the way: two threads downloading the same cache entry at once both fetched it, and
      their renames collided. Now the second waits for the first.
- [x] **Tours, 768 m around Pont Wilson** (`--center 47.3966,0.6880 --half 384 --cell 1`):
  - Content: 940 buildings, 1,085 trees, the Loire with its islands and 14 m of relief.
  - Cost: 208 MiB downloaded in 1 min 44 s; 3.3 s to rebuild from the cache, byte-identical.

### Phase 3: LiDAR-first geometry. Done
- [x] **Measured roofs** (`field.rs`). A 0.5 m field of the highest class-6 return over each
      footprint, with gaps filled from their neighbours. It is meshed by cutting the footprint's
      triangles along the field's cells, with walls rising to the field along every edge, and
      `voxels::bake` cuts that into cubes like any roof. Towers, steps, domes and spires come out as
      measured: Notre-Dame keeps its two towers, buttresses, apse and its 86 m spire. `roof::fit`
      stays the fallback where returns cover under 35 % of a footprint. The palette (flat or pitched)
      follows the field's slopes.
  - Tried and dropped: a 3×3 median over the field. It saved only 4 % of the collision boxes, and
    it shaved the spire to 80 m.
- [x] **Buildings OpenStreetMap lacks** (`footprints.rs`):
  - per tile (plus 40 m), a 0.5 m raster of class-6 returns at least 2 m above the ground;
  - what lies on or within 1.5 m of an OSM outline is struck out;
  - one-cell gaps are closed, and regions of at least 20 m² are traced by their cell edges and
    simplified to within 0.6 m;
  - a region whose centre is on water is dropped (moored boats).
  
  France's OSM outlines come from the cadastre and are better than traced ones, so OSM stays the
  outline wherever it has one. That replaces the planned "match by overlap": with outlines this
  good, the laser only has to find what is missing. Tours gains 99 buildings and central Paris 497,
  mostly whole blocks that OSM maps as multipolygons, which the importer does not read.
- [x] **Big buildings in pieces.** Laser-found regions, and OSM footprints over 1,000 m², are cut on
      a fixed 32 m lattice (`footprints::pieces`), each piece its own building. As one building, a
      Paris block's structure check took 47 ms whenever it was hit, and the traced outline roofed
      over its courtyards. In pieces, the slowest check is 6 ms and the courtyards stay mostly open.
      `voxels::share` gives the cubes where two pieces meet to one of them.
- [x] **Bridge decks from class 17** (`bridge::measured_profile`):
  - each 3 m cross section of an OSM `bridge=*` way takes the median of the deck returns within
    2 m;
  - gaps between measured sections are filled linearly;
  - where the way runs on past the measured span (onto an embankment), the deck descends at the 6 %
    grade limit, never below the ground;
  - inference stays the fallback when under half the sections are measured.
  
  The Loire bridges at Tours measure flat at 13 m, where inference made 0 → 13 m ramps, and one
  dipped to 2 m.
- [x] **Trees:**
  - in France the canopy reads class 5 only, and no longer masks OSM outlines (that only cut crowns
    over roofs);
  - `canopy::classify_france` is fitted against the Paris register (`calibration_france`, around
    the Jardin des Plantes). IGN's intensity runs about a third lower than Lantmäteriet's, but
    conifers are again the bright ones (median 476–664 against 137 for broadleaves), with fewer
    multiple returns (0.84–0.87 against 0.98). The Swedish rules call 389 of 583 Paris broadleaves
    "birch"; the French ones get 444 right (76 %), 54 % of ornamentals, and 16 of 19 conifers as
    conifers;
  - pine against spruce is guessed from slenderness (the crown's base does not separate them here);
  - the canopy raster stays at 1 m: crowns segment well at it.
- [x] **The Paris tree register** (`trees::Register`): ~220,000 trees with species and measured
      height, used for any French world that touches Paris, as Umeå's is in Sweden. Central Paris
      gets 1,080 trees from it.
- [x] **Water levels:** nothing to add. IGN's MNT is made from the ground, water (class 9) and
      virtual points, so the existing "median of the terrain model inside the body" already reads
      the laser's water surface.
- [x] **Editor downloads** for French worlds: besides OSM, Download fetches the LiDAR index and the
      MNT under the painted tiles, the same pieces Generate asks for (`Region::tile_window`, tested
      to match). The point clouds are read node by node when the world is generated.

#### What it costs the server

`full_room_tick_cost` (64 players running and firing) and `whole_buildings_hold`, measured with
`RAGNAROK_WORLD=...`:

| World | Buildings | Collision boxes per 100 m² | Average tick | Worst tick | Slowest structure check |
|---|---|---|---|---|---|
| `bug` (Umeå, fitted roofs) | 1,021 | 14 | 2.5 ms | 5.9 ms | 6.6 ms |
| Tours | 1,083 | 89 | 3.5 ms | 7.3 ms | 3.1 ms |
| central Paris, fitted roofs | 566 | 58 | 4.2 ms | 20.9 ms | 15.0 ms |
| central Paris | 1,018 | 97 | 4.4 ms | 11.7 ms | 6.1 ms |

All within the 16.7 ms tick. Measured roofs make far more collision boxes than fitted ones: a
real roof's cube steps are irregular and merge into fewer long boxes. That is where the tick's extra
movement cost comes from. Making those boxes fewer is the first thing to try if a French world runs
heavy.

### Phase 4: colours. Done
- [x] **Roofs from the aerial photo** (`paint.rs`, `ign::photo`): the median colour of the photo
      inside the footprint, leaving out foliage and deep shadow, snapped to a toon palette of nine
      roofs. Each palette entry holds the colour as the photo shows it (measured over Tours) and as
      it is drawn, since photo roofs are much greyer. Tours comes out 90 % slate, Paris mostly zinc
      with gravel on the flat roofs and some tile.
- [x] **Walls** from a muted palette (limestone, plaster, sandstone, ochre, some brick), picked by
      where the building stands.
- [x] **One colour per building:** all pieces of a cut building, OSM or traced from the laser, are
      sampled together and share their colours. Without that a Paris block came out as a patchwork.
- [x] **In the world file:** `Building.paint` = `[wall, roof]` as `0xRRGGBB`, a trailing optional
      field left out when `None`. Swedish worlds and older files are the same bytes, so this needed
      no new world version. The client draws it where present (`buildingColors`, so rubble and
      fractures follow) and uses its own palettes otherwise. `--no-paint` turns it off, and a
      failing photo service only means no colours.
- [x] **No made-up rocks and bushes:** `props::scatter` (random boulders and shrubs in woods and
      parks) runs for Sweden only. In France the laser sees what is really there.
- [x] **Bushes from class 4** (`bushes.rs`): medium-vegetation returns in 1 m cells, one bush per
      2 m block with at least two cells of three or more returns, at their centroid and measured
      height (0.6–1.6 m). Blocks under a class-5 crown are left out (lower branches, not shrubs).
      A hedge comes out as a row of bushes. Tours gets 266 and central Paris 67, after the usual
      rejection on buildings and roads.

### Phase 5: high ground. Done
- [x] **Heights are `i32` centimetres** (`WORLD_VERSION` 16): `Tile::ground` and `::water`, the merged
      `Ground` and the client's `Int32Array` views. `NO_WATER` is `i32::MIN`. Ground and water take
      twice the bytes they did. `fixtures/world` was converted in place; other worlds are regenerated.
- [x] **`MAX_RELIEF` is 1000 m** (`shared::consts`), used by the importer's check.
- [x] **Boundary walls** reach `MAX_RELIEF + 1000` m above the lowest ground, the same in every world,
      since the streaming client builds them before it knows the ground.
- [x] **Ceilings above the terrain:** every flier's ceiling (helicopter, drones, plane, the hover car)
      is measured from the terrain heightfield under it (`terrain_below`, a ray that ignores
      buildings and bridges), not from y = 0. On a flat world that is the same as before.

### Later
- [ ] Fewer collision boxes for measured roofs (see the table above).
- [ ] Read OSM's multipolygon buildings, so whole blocks keep their cadastre outlines instead of
      laser-traced pieces.
- [ ] Class 64 as props (walls, pylons); the photo's colours on the minimap; BD TOPO's building use
      and heights, and its wall materials for wall colours (only about a quarter of the buildings in
      central Tours have them).
- [ ] Attribution "IGN – LiDAR HD" in the client's credits.

## Checking it

```sh
cargo test -p shared lambert93                 # the projection
cargo test -p osm-import -- region grid ign field footprints bridge canopy
cargo test -p osm-import ign_probe -- --ignored --nocapture            # network: one hectare of Tours
cargo test --release -p osm-import calibration_france -- --ignored --nocapture   # trees against the Paris register
cargo run --release -p osm-import -- --center 47.3966,0.6880 --half 384 --cell 1 --out worlds/tours/world.msgpack
cargo run --release -p osm-import -- --center 48.8530,2.3499 --half 384 --cell 1 --out worlds/paris/world.msgpack
RAGNAROK_WORLD=worlds/paris/world.msgpack cargo test --release -p server -- --ignored --nocapture whole_buildings_hold full_room
```

Look at `minimap.png` next to the world first: the river, the bridges and the old town's blocks
should be where the map has them, with no stretch east to west.
