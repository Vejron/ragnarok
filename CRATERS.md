# Craters — plan

Everything explosive that bursts on the ground dents it. First it was only the heavy ones: the tank's shell (`PROJECTILE_SHELL`), the mortar round (`PROJECTILE_MORTAR`) and the plane's bomb (`PROJECTILE_BOMB`). Since the scorch marks (SCORCH.md) proved cheap, rockets, grenades, autocannon rounds and the hen's eggs dig too, smaller craters at about 0.8 of their blast radius (`CRATER_*` in `consts.rs`).

Status: branch `feat/craters`. Phases 1-4 are written and tested (Rust and Vitest suites green); not yet played in a browser, and the tuning in phase 5 is open. What is done, and what is knowingly left:

- Done: `Ground::dig` and its TypeScript mirror; the server's terrain split into one heightfield per tile (`sim::terrain_colliders`) so a crater rebuilds only the tiles it touches; `Burst::crater` for the shell, the mortar round and the bomb; `World::dig_crater` with the depth cap and the water skip; the `cratered` message and its catch-up (protocol 45); the client digging loaded and later-streamed tiles, `ClientSim.setTileGround`, and the worker's `reground` (terrain mesh, relief map and roads, seams of the neighbours).
- Left, knowingly: the surface raster (footsteps) and water drawn on a tile are not redone for a crater (grass is: see below); items lying on the ground and placed cubes beside a crater are not moved or re-checked; a tile with a baked slope map (`--detail`) keeps shading from it inside the crater; no dust or sound beyond a puff of dust; the measurement of the server's tick with many craters (`full_room` does not fire shells).

## Goals and non-goals

- A landing shell lowers the heightfield in a smooth bowl, with a slight raised lip. The server's physics, the client's prediction, the drawn mesh and every height lookup (`Ground::height_at`, `heightIn`) agree on the new surface.
- Late joiners and streamed-in tiles see the craters that already exist.
- Cheap on the wire: a crater is a few bytes, and both sides apply the same deterministic function.
- Not in scope: persistence across server restarts (craters reset with the world), water, digging through building floors, filling craters back in.

## How a crater works

A crater is `{ x, z (cm), radius (cm), depth (cm) }`. `shared::terrain::Ground::dig` lowers every sample within `radius` by

    depth * (1 - (d / r)^2)^2          d = distance from the centre, d < r

so it is smooth at the rim and deepest in the middle. Samples are `i32` cm, so the function is integer-rounded and identical in Rust and TypeScript (a fixture recorded from Rust pins the TypeScript copy, as for `terrain.ts`).

Rules that keep it safe:

- A crater never digs below `floor` (the lowest ground in the world, y = 0, plus a margin, and never under a water bed's surface), so `KILL_Y` and the slab under the terrain keep their meaning.
- Craters stack, but total depth at a sample is capped (`CRATER_MAX_DEPTH`), so a barrage makes a pit and not a hole.
- A shell only digs when it burst on the terrain: within `CRATER_REACH` of the ground height, not on a roof, a vehicle or in the air.
- A shell that landed on a building or a vehicle does not dig, but one that lands beside it does.

Sizes, to be tuned in play: tank shell about 2.5 m radius, 0.7 m deep; mortar about 3 m radius, 0.9 m deep; bomb about 6 m radius, 1.8 m deep. They come from the `Burst` row of each projectile.

## Server

1. `shared::terrain`: `Ground::dig(x, z, radius, depth, floor)` and its tests. The server's one `Ground` (`world.map.ground`) is mutated.
2. `sim`: replace the heightfield collider. The server has one for the whole world, which is the open question below. Start with replacing the whole collider and measure; if a replace spikes the tick, split it per tile the way `sim/stream.rs` already does for the client.
3. A `Burst` gains `crater: Option<Crater>` (radius, depth), set in `burst_of` for the three kinds. `projectiles.rs` calls `world.dig_crater` after the blast, only on a terrain hit.
4. Things resting on the dented ground must follow: placed cubes and carved buildings already fall when unsupported (`construct.rs`, `structure.rs`), so `dig_crater` asks them to re-check around the crater. Pickups and items on the ground are re-seated to the new height.
5. State: `World.craters: Vec<Crater>` in order, sent to joiners. Capped (oldest craters are forgotten for joiners past the cap, which only shows as a late joiner seeing a smoother ground there; the server's own ground keeps them).

## Wire

- A new reliable message `crater { craters: Vec<Crater> }` with `live: bool` like `carved` (live ones are animated, catch-up ones are state). Bump `PROTOCOL_VERSION`, update `NETWORKING.md`, regenerate (`UPDATE_PARITY=1 cargo test -p shared`).
- Sent in `Game::broadcast_*` once a tick like `broadcast_carved`, and as catch-up in the join sequence next to the carved catch-up.

## Client

1. `client/src/sim/terrain.ts` (or a sibling): the same `dig` on a `TileGround`'s `heights`, applied to every tile the crater touches (up to four). Pinned by a Rust fixture.
2. Tile data: the loaded tile's ground array is edited in place; a crater for a tile that is not loaded yet is kept and applied when it arrives (`WorldView` attach), so streaming order does not matter.
3. Physics: the prediction rebuilds the tile's heightfield (`stream.rs::tile_ground` through `add_tile`/a new `set_tile_ground`) in both the main and the worker copies, so the character and vehicle prediction match the server.
4. Rendering: the worker re-simplifies the tile mesh and relief map (about 5 ms and 8 ms at 1 m) and the neighbours' seams are redone. Roads, water and grass draped on the terrain are rebuilt with the tile, or follow the heights if they read them live. A short dust puff on `live` craters.
5. Everything that reads `heightIn` (38 places) just sees the new heights.

## Phases

1. **Shared maths** — `Ground::dig`, TS mirror, fixture, tests. No gameplay.
2. **Server side** — craters from the three shells, physics collider swap, measurement of the swap cost, `dig_crater` re-seating things.
3. **Wire** — message, catch-up, parity files, protocol bump, `NETWORKING.md`.
4. **Client** — apply to tile data, prediction collider, mesh and relief rebuild, late tiles, effects.
5. **Tuning and tests** — sizes, caps, a headless smoke test (recipe in the toolchain notes), `full_room` measurement, `design.md` §8 and the terrain paragraph.

## Open questions

- **Collider replacement cost** on the server's single whole-world heightfield. Measure first; the fallback is a heightfield per tile.
- **Detail maps**: the baked slope map (`Tile::detail`) knows nothing of craters. It is lighting only; a tile with a crater could drop its detail map and take normals from the heights.
- **Water**: craters that reach a water-covered sample are skipped in a first version.
- **Persistence**: none yet. If wanted later, the crater list is already the thing to save.

## Grass

`Grass.dent` (`client/src/render/grass.ts`) is told of every crater by `WorldView.cratered`, and again when a tile that a crater reaches is attached. It marks the chunks of blades over the crater to be grown again, which they are on the dented ground (the blades read `Terrain.sample`, so they stand on the bowl and not in the air above it), and it keeps grass from growing within 80% of the crater's radius, so the bowl is bare earth and its rim keeps its grass. Craters are remembered by `x,z,radius`, so replaying them on tile attach adds nothing twice. `craters.spec.ts` checks both.

## Measured (server)

Two ignored tests in `crates/server/src/game.rs`, run with `--release`:

- `shelling_tick_cost` (`SHELLS=n TICKS=n`): 63 players standing in a 240 m square of the committed city (5 m ground), and `n` shells a tick falling from 40 m on a 300 m square, a third each tank shell, mortar round and bomb; then the same with autocannon rounds (the same kind of blast, no crater when measured; they now dig small ones) as the baseline. It runs the real `Game::step`, snapshots included, with 63 bots-less players who only stand.
- `crater_rebuild_cost`: one crater (a mortar's) on a 1536 m square of synthetic rolling ground at 5, 2 and 1 m, 200 craters, some on seams.

| | craters | no craters | budget |
|---|---|---|---|
| 2 000 shots over 20 s (120/s), tick avg / p99 / worst | 3.1 / 6.1 / 8.2 ms | 2.9 / 5.8 / 7.3 ms | 16.7 ms |
| 9 600 shots over 20 s (480/s), tick avg / p99 / worst | 4.8 / 10.5 / 12 ms | 6.2 / 11 / 13-18 ms | 16.7 ms |
| one crater: dig and build the heightfields | 5 m: 0.14 ms, 2 m: 0.03 ms, 1 m: 0.13 ms | | |
| one crater: swap the colliders at the next step | 3-7 µs | | |
| a joiner's catch-up after 2 000 craters | 1.4 ms, 21 KB | | |

What it says:

- The craters themselves are cheap: about 0.15 ms to dig and rebuild the tiles a crater reaches, and a few µs to swap the colliders, at every ground sampling tried, so the per-tile split of the server's heightfield did what it was for. The tick cost is the shells' flight and blasts (buildings being bitten), which the baseline pays too. At 480 shells a second the crater run is not slower than the baseline (the autocannon blasts bite more buildings). A single 21 ms tick showed up once in six runs of the heavy case and did not come back; the same load repeated stays under 18 ms.
- The wire cost is small: `cratered` is 12 bytes a crater, about 1.2 KB a second to each player at 120 shells a second and 3.5 KB at 480. The rest of the difference in bytes against the baseline is the projectiles themselves, which the server replicates for shells, rounds and bombs and does not for autocannon rounds.
- The joiner's catch-up is the 2 000 most recent craters (`CRATER_MAX_KEPT`), about 21 KB.

What it does not say: the client's cost. A crater makes the tile worker mesh the terrain again, shade it and drape its roads (about 5 + 8 ms a 1 m tile by the numbers in design.md §8, not measured here), once for every tile it reaches and one at a time per tile, with dents in between coalesced; a barrage over one tile therefore costs the worker a continuous job but not the frame. The players here stand still and nothing drives; tanks and bots on top would add their own cost, which `full_room` measures.

## Lasting ground

Status: phases 1-4 written and tested (Rust, client and engine suites), on `feat/scorch` with the scorch work, not committed; not yet played. Measuring a large `Dug` (a shelled 1 m world) is left.

A joiner used to rebuild the dug ground by replaying the server's last `CRATER_MAX_KEPT` (2 000) craters onto the tile files. Past that the oldest were forgotten and the joiner's ground differed from the server's, and with every explosive digging (rockets, grenades, autocannon rounds) that came quickly. Now the server's dug ground is what a joiner gets, and the craters are only the scorch on it.

- **The heights.** The server already digs every crater into its one `Ground` and keeps the ground as the importer left it (`World::pristine`). It now also keeps which tiles a crater changed (`World::dug_tiles`). A joiner is sent `Dug`: for each of those tiles, the difference from the pristine ground (whole centimetres, as runs of `[skip, count, values…]` over the tile's `side × side` samples, row-major as in the tile file), in batches that stay well under the client's 4 MiB a message. The client keeps them by tile and lays them on a tile's file heights when it streams in (`WorldView.groundFor`), before the craters dug since it joined. The size is that of the ground dug, not of the number of craters, and nothing is ever forgotten until the server restarts.
- **Live craters** are as before: `Cratered { live: true }` the tick they are dug, dug by every client into the tiles it holds and replayed onto tiles that stream in later.
- **The scorch fades.** The server keeps the craters of the last `CRATER_FADE_SECONDS` (30 minutes, and at most `CRATER_MAX_KEPT`) with the tick each was dug. A joiner's `Cratered { live: false }` is those, each with its `age` in seconds, and is marks only: the client does not dig them (their heights came in `Dug`). The ground's shader fades a crater's marks out over the last 30% of `CRATER_FADE` (engine `scene/craters.ts`) from its birth time, which for a caught-up crater is `now − age`; `Craters.expire` then takes it out of the grid. The hole stays.
- **Protocol** 46: `CraterEntry` gains `age` (u16, seconds; 0 when live), and `Dug` is new (`S_DUG`).

Phases:

1. Wire: `CraterEntry.age`, `Dug`/`DugTile`, protocol 46, parity files, NETWORKING.md.
2. Server: `dug_tiles` kept in `apply_crater`, `dug_ground` for the joiner, the crater list pruned by age, the join sends `Dug` before the marks. Tests: a joiner's patched ground equals the server's after more craters than `CRATER_MAX_KEPT`; marks older than the fade are not sent.
3. Client: `WorldView.dug`, `groundFor` laying the patch, catch-up craters as marks only, with their age. Tests: a late joiner's ground from `Dug` equals an early one's.
4. Engine: the fade in `craterMarks` and `craterGrass`, `Craters.expire`.
