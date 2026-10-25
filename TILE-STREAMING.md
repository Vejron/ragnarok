# Tile streaming off the main thread

The goal: attaching a streamed-in tile never costs a frame. Today the streamer attaches at least one tile per frame, whatever it costs (`ATTACH_BUDGET_MS` in `client/src/stream.ts`), so one expensive tile is one visible hitch.

The buildings and the terrain are already built in workers (design.md, "Tile meshes off the main thread"). On 1 m tiles the main-thread cost has moved to the roads, rails and water draped on the ground. An overview with diagrams is in [Ragnarok Client Threads](https://claude.ai/artifact/WTFFc7ShVFFj1jNj5AYiea).

## Progress

| Phase | What | Status |
|---|---|---|
| 0 | Benchmark and baseline in the repo | ◐ benchmark done; browser trace left for the end |
| 1 | Hash a tile with an indexed loop | ☑ done (p95 0.99 ms, short of 0.5) |
| 2 | Cheaper road and rail draping on the main thread | ☑ done (4× faster, short of target; Phase 3 takes the rest) |
| 3 | Roads, rails and water in the terrain worker | ☑ done (seam re-drape and play-test left) |
| 4 | Decode: measure in the browser, then decide | ☐ not started |
| 5 | A fallen building removed in place | ☐ not started |
| 6 | Carve-only WASM for the buildings worker (optional) | ☐ not started |

Status: ☐ not started · ◐ in progress · ☑ done · ✕ dropped (say why in the log).

## Baseline

The 1 m dev world (`client/public/dev`, 121 tiles), timed in Node on 2026-10-25, at commit `9e319c7`. Times are in ms per tile.

| Stage | Runs on | Median | p95 | Max |
|---|---|---:|---:|---:|
| Roads + rails ribbons | main | **24.6** | **54.3** | **125.6** |
| Water surface + streams | main | 0.0 | 10.4 | 15.6 |
| Decode (msgpack) | main | 2.5 | 4.8 | 10.2 |
| Hash (`worldHash`) | main | 2.1 | 3.9 | 13.0 |
| `ClientSim.addTile` (decode in Rust, colliders) | main, WASM | 1.3 | 2.7 | 5.0 |
| Grass mask | main | 0.6 | 1.5 | 4.2 |
| Surface raster | main | 0.2 | 0.9 | 1.2 |
| Trees | main | 0.1 | 0.3 | 2.2 |
| Building mesh | buildings worker | 2.0 | 10.1 | 14.3 |
| Terrain mesh (finest level only) | terrain worker | 9.4 | 14.0 | 44.3 |
| Damage rebuild of a tile's buildings | main | — | — | 5.5 |

The road benchmark sampled only the tile's own ground (no `around`). In the game the roads also sample the neighbouring tiles, so they are probably a little slower.

## Targets

- **Work on the main thread when a tile attaches,** not counting the GPU upload: p95 under 4 ms, max under 8 ms, on the dev world.
- **Main-thread work when a fetch completes** (hash and decode): p95 under 2 ms.
- **In the browser:** no frame over 20 ms while driving across the dev world at 30 m/s, measured with a Chrome performance trace.

---

## Phase 0: Benchmark and baseline in the repo

The benchmarks so far lived in a scratchpad. Every later phase is judged against numbers, so the benchmark goes into the repo first.

- [x] Add `client/src/__tests__/tiles.bench.spec.ts`. It times every stage in the baseline table over the dev world and prints median, p95 and max, plus road and rail vertices a tile. It uses the real `around` (a `Terrain` with every tile's ground loaded), so road numbers match the game. Run it from `client/` with `BENCH=1 npx vitest run tiles.bench --silent=false`.
- [x] Make it skip itself unless `BENCH` is set and `client/public/dev` exists, so CI and fresh clones stay green.
- [ ] Record one Chrome performance trace driving across the dev world, as the in-browser baseline: how many frames are over 20 ms, and the worst frame and what it was spent on. Moved to the end, as the before/after check of the whole plan, since it needs a server and a scripted drive.
- [x] Fill the measured numbers into the log below.

**Done when** the benchmark reproduces the baseline table within noise. It does: roads + rails 24.7 / 55.2 / 101.4 ms with the real `around`.

## Phase 1: Hash a tile with an indexed loop

`worldHash` (`client/src/world.ts`) is FNV-1a over every byte with `for (const byte of bytes)`. The iterator is slow in V8. A plain `for (let i = 0; i < bytes.length; i++)` loop gives the same hash, and the server's copy in Rust is unaffected.

- [x] Switch to an indexed loop and check that the hash is unchanged (the known vectors in `world.spec.ts`).
- [x] Re-measure: median 2.10 → 0.55 ms, p95 3.97 → 0.99 ms.

**Done when** hashing takes p95 under 0.5 ms. Not quite: FNV-1a goes a byte at a time, so this is about as fast as JS goes (about 2.5 ns a byte). Hashing in Rust would gain little more, so this stays as it is.

## Phase 2: Cheaper road and rail draping on the main thread

The cheap step first, as with the buildings. Every ribbon vertex in `Layer.vertex` (`client/src/render/roads.ts`) calls:

- `normalAt`: 4 height samples.
- `toneAt` (`render/shade.ts`): the height, plus `ringMean`, which takes 8 more.

That is about 14 `heightIn` calls a vertex, with vertices 1 m apart along the road (`SPACING`) and across it (`ACROSS`). Rails use the same `Layer`.

- [x] Count vertices per tile: median 22k, p95 49k, max 92k. Strips are 77% of them and round ends 23%. The count follows from 1 m spacing, so the cost per vertex is what to cut.
- [x] `GroundShade` (`render/shade.ts`), one per `buildRoads` / `buildRails` call, filled where first needed and read bilinearly:
  - node heights and normals on the ground's own nodes, 16 m past the border;
  - ring means on a 3 m lattice.
  A vertex now costs about one height sample instead of about 14. The casing and fill, and neighbouring vertices, share nodes.
- [x] The vertex height is the collider's cell triangle interpolated from the cached node heights. It equals `heightIn` but for rounding, so the ribbons still sit on the collider's triangles.
- [x] `Layer` writes into growable typed arrays in place of `number[]`, and no longer spreads an array for every triangle.
- [x] `groundshade.spec.ts`: on a bumpy ground with a sharp ditch, height matches to 0.1 mm, and node normals and ring means are exact. Tone is off by 0.006 on average and at most 0.06 of its 0.55–1.2 range, and normals by at most 3°, both worst at the ditch's edge.
- [x] Re-measure: roads + rails 24.7 / 55.2 / 101.4 → 6.2 / 14.4 / 21.1 ms (median / p95 / max).
- [ ] Look at roads in play, especially at tile seams and in ditches.

**Done when** roads + rails take p95 under 5 ms and max under 15 ms, and look the same in play. Not met: 14.4 ms p95 and 21.1 ms max. What is left is spread thin (the one height sample, blending, the tone's three `sin`s, the triangles), so further work here would yield little. Phase 3 takes the whole cost off the main thread.

## Phase 3: Roads, rails and water in the terrain worker

Roads, rails and water are draped on the ground and need the neighbouring tiles at the seams. The terrain worker (`render/terrain.worker.ts`) already keeps every built tile's ground for exactly that purpose. It becomes the ground worker: everything that lies on the ground is built there.

- [x] **Split data from scene in roads, rails and water**, as `render/buildingmesh.ts` does for buildings:
  - `roadRibbons` (`roads.ts`) and `railRibbons` (`rails.ts`) return `Ribbon`s: positions, normals, tones and indices, with the material's colour and depth bias.
  - `surfaceBuffers` and `streamBuffers` (new `watermesh.ts`) return the water's arrays.
  - `drapedMeshes` (new `groundmesh.ts`) builds all of a tile's, sharing one `GroundShade` between its roads and rails.
  - None of these modules import the engine.
- [x] **Mesh and material creation stay on the main thread:** `ribbonMeshes` in `world.ts` (it took over `layerMaterial`), and `buildWater`, which now takes prepared buffers.
- [x] **Extend the terrain request.** `TerrainRequest.build` carries the tile's `DrapedTile` (roads, rails, water levels, streams). `TerrainReply.built` and `PreparedTerrain` carry the `Draped` meshes, sent as transferables.
- [x] **Use the worker's ground for heights.** The worker drapes through its own `Terrain` of the tiles it has built, as it does for relief.
- [x] **Attach.** `buildTile` uses `prepared.draped.ribbons` and `attachTile` passes `prepared.draped.water` to `buildWater`. Without them, both build on the main thread as before.
- [x] **Seams, unchanged at first.** Ribbons are draped once, with whatever neighbours are loaded, and never redone, as before.
- [ ] **Seams, optional follow-up.** When a neighbour arrives, re-drape the ribbons that end on the shared edge and send them like `shaded`, so roads meet as cleanly as the terrain's relief does.
- [x] **Remeshing.** `remesh` only rebuilds `terrainBuffers`. The ribbons are draped on the true ground and stay as they are.
- [ ] **Distant roads.** Check the far roads in play, as noted in the handoff: tiles beyond 320 m are drawn up to 50 cm off the true ground.
- [ ] **Rename (optional, last).** `TerrainWorker` → `GroundWorker`, and update design.md §8 and "Tile meshes off the main thread".
- [x] **Checked in Chrome** on the first 40 dev-world tiles, built in order through the real `TerrainWorker`. The ribbons and water came out bit for bit the same as the main thread's with the same neighbours. `buildTile` + `buildWater` on the main thread went from 22.4 / 38.2 / 52.8 ms with no worker to 0.30 / 0.70 / 4.7 ms with the workers' results (median / p95 / max).

**Done when** the attach-time target holds, roads, rails and water look the same in play, and the main-thread fallback still passes the tests.

## Phase 4: Decode: measure in the browser, then decide

Each tile is parsed three times:
1. msgpack in JS on the main thread (`decodeTile`).
2. Again in Rust in `ClientSim.addTile`.
3. Parts of it cloned to each worker.

Moving the JS decode into a worker only helps if the main thread then gets back less than a whole `TileData`, because cloning a whole `TileData` back costs about as much as decoding it.

- [ ] Measure `decodeTile` in Chrome, separately for the ground (`Float32Array`) and for the object-heavy parts (buildings, roads, trees, cover).
- [ ] If decoding is still over the target: have the workers decode what only they need from `tile.bytes` (one transfer of a copy of the bytes), and let the main thread decode only what it queries (buildings, trees, cover for surfaces and grass). Otherwise mark this phase ✕ with the numbers.

**Done when** hash and decode together take p95 under 2 ms in the browser, or the phase is dropped with numbers that show it doesn't matter.

## Phase 5: A fallen building removed in place

When a building falls, `rebuildBuildings` refills the whole tile's merged mesh: 4–5.5 ms on the worst tiles, on the main thread. Hurt buildings are drawn by `BuildingDamage` as shards, so the merged mesh only ever needs to lose a whole building.

- [ ] Have `buildingBuffers` return each building's vertex range, and keep the ranges with the tile's group.
- [ ] When a building falls, collapse its range to degenerate triangles (all corners at one point) and call `Geometry.needsUpdateRange`. Keep the full rebuild only for when a fallen building's range is unknown.
- [ ] Check the bounds (they stay as they are, which is fine for culling) and the shadow pass.

**Done when** a building falling costs the main thread under 0.5 ms.

## Phase 6: Carve-only WASM for the buildings worker (optional)

The buildings worker compiles the whole 2.8 MB simulation module (Rapier included) to call `carve_baked_faces`. Its first request takes about 46 ms, and the module holds memory of its own.

- [ ] Do this only if the worker's start-up or memory shows up in a trace or a memory profile.
- [ ] If it does: a `carve-wasm` crate that exposes only the `carve_*` functions from `crates/shared`, built alongside `sim-wasm` by `client/scripts/build-wasm.ts`, and imported by `buildings.worker.ts`.

**Done when** the worker's first request takes under 10 ms, or the phase is dropped.

---

## Not doing

- **Moving `ClientSim` (prediction and Rapier) into a worker.** Every step would be a message away: a frame of input latency and an asynchronous reconcile. `addTile` is 1.3 ms median, which doesn't justify that.
- **One worker pool that builds whole tiles.** Draping needs the neighbours' ground in one place, so the ground work can't be split across several workers without copying every ground into each.
- **WASM shared memory across threads.** It needs cross-origin isolation headers and WASM threads. That is far more than this calls for.

## Log

Newest first. One entry per change: date, phase, what was done, numbers before and after, commit.

- **2026-10-25 · phase 3.** The terrain worker builds each tile's roads, rails and water (`groundmesh.ts`). In Chrome, `buildTile` + `buildWater` on the main thread take 0.30 / 0.70 / 4.7 ms with the workers' results, against 22.4 / 38.2 / 52.8 ms without, and the output is bit for bit the same. Still to do: re-drape at seams (optional), look at roads and distant roads in play.

- **2026-10-25 · phase 2.** `GroundShade` caches node heights, node normals and ring means for each tile's roads and rails, and `Layer` writes typed arrays. Roads + rails 24.7 / 55.2 / 101.4 → 6.2 / 14.4 / 21.1 ms. The shading moves by 0.006 tone on average, at most 0.06 at a sharp ditch's edge. Still to do: look at roads in play.
- **2026-10-25 · phase 1.** `worldHash` uses an indexed loop: median 2.10 → 0.55 ms, p95 3.97 → 0.99 ms.
- **2026-10-25 · phase 0.** `tiles.bench.spec.ts` added. With the real `around`, the baseline is: roads + rails 24.7 / 55.2 / 101.4 ms, hash 2.1 / 4.0 / 4.3, decode 2.4 / 4.7 / 7.5, `ClientSim.addTile` 1.3 / 2.8 / 5.4, water 0.0 / 8.0 / 15.8. Removing one fallen building (full refill) takes 0.3 / 7.4 / 11.9.

- **2026-10-25 · plan.** Plan written from the measurements above. Building meshes already moved to a worker in `9e319c7`. Before that, `f0538a4` cut the worst building tile from about 11.5 to 6.2 ms in Node by writing straight into exact-size arrays.
