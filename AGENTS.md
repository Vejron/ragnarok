# Working on Ragnarok

A browser multiplayer game. A server-authoritative Rust server (Rapier physics, WebTransport) runs the game, and a Vue 3 + WebGPU client predicts and draws it. [README.md](README.md) covers running it, and [docs/design.md](docs/design.md) the architecture. Each feature has its own write-up in [docs/](docs/).

## Where things are

| Path | What |
|---|---|
| `crates/shared` | Everything the server, physics and client agree on: constants (`consts.rs`), vehicle definitions (`vehicles/<kind>.rs`), weapon mounts (`mounts.rs`), armour (`armor.rs`), items, the wire messages (`protocol.rs`) and replicated components (`replicated.rs`) |
| `crates/sim` | The physics (Rapier). Built natively for the server and to WASM (`crates/sim-wasm`) for the client's prediction |
| `crates/server` | The game. `game/` is the room, the loop and replication (`replicate.rs` builds what clients see); `mechanics/` is one module per feature (gunnery, combat, vehicles, drones, bots and so on), each a `Mechanic` |
| `crates/osm-import`, `crates/editor` | The world importer and the map editor's backend |
| `client/src` | `kinds/` is how each vehicle kind is presented; `render/` holds the models and effects; `entities/presenters/` draws what the server replicates; `sim/` mirrors the shared tables; `net/` is the codec |
| `engine/` | The WebGPU renderer, a package of its own |
| `tools/vehicles`, `tools/character` | Blender scripts that build the `.glb` models in `client/public/models/` |
| `worlds/<name>/` | A world's `area.json` (editor placements), `scenario.json` and its generated files |

## Generated files: the edit loop

The client's copies of shared data are generated from the Rust. Never edit a `*.generated.*` file by hand.

| You changed | Run | It writes |
|---|---|---|
| A `pub const` in `crates/shared/src/consts.rs` or `vehicles/*.rs` | `cd client && npm run gen:consts` | `client/src/sim/consts.generated.ts` |
| A `VehicleDef`, item or other table | `UPDATE_PARITY=1 cargo test -p shared` | `client/src/sim/tables.generated.json` |
| A message in `protocol.rs` or a component in `replicated.rs` | `UPDATE_PARITY=1 cargo test -p shared` | `client/src/net/*.generated.ts`, `client/src/entities/units.generated.ts` and the `__tests__/*.fixture.json` files |

- **The struct field order is the wire format.** After adding, removing or reordering a field of a message or component, bump `PROTOCOL_VERSION` in `consts.rs`.
- **A new field also needs three hand edits** that `UPDATE_PARITY=1` does not make: the struct literals in `crates/shared/src/protocol/tests.rs` and `replicated/tests.rs` (which has two literals listing every component), and the expected snapshot in `client/src/__tests__/codec.spec.ts`. A new field on a component the client builds in its specs (`seated`, or the presenters' `Placed`) also breaks literals in several specs: `npm run type-check` names each one.
- **A quantised field arrives decoded.** The client reads a `u8 as byte` field as 0..1 and an `i16 as mrad` field as radians, not as the raw integer.
- **Flag and kind constants are mirrored by hand** in `client/src/net/codec.ts` (`TURRET_*`, `KIND_*` exports and so on). `consts-parity.spec.ts` checks them.
- **`gen-consts` parses the Rust as text.** Write constant arrays as literals (`[0.0, 1.79, 1.65]`), not expressions built from other constants.
- **Rebuild the WASM before testing.** The client's prediction runs `crates/sim` as WASM. `npm test` and `npm run type-check` rebuild it first, but `npx vitest run` does not, so run `npm run build:wasm` before it after any physics change.

## Adding a field to the predicted soldier

`sim::Character` crosses into the client's prediction as a flat array of numbers, so a new field means all of these:

1. `CHARACTER_LEN` and the `read`/`write` pair in `crates/sim-wasm/src/lib.rs`.
2. `CharacterState`, the scratch array's size and the packing in `client/src/sim/motor.ts`.
3. The starting state and `reconcile` in `client/src/game.ts`. Reconcile must be able to rebuild the field from what the server replicates (a stance, a flag), or a correction cannot restore it.
4. The state literals in `movement.spec.ts`, `predict.spec.ts` and `sim.bench.spec.ts`.

Anything in the sim that reads the tick (waves do) also needs the client to call `motor.setTick` before each predicted step.

## Checking your work

CI runs all of these, and all must pass:

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cd client && npm run build:wasm && npm run type-check && npm run lint && npx vitest run
```

To iterate faster:
- The first workspace build takes minutes, so test one crate with a filter, e.g. `cargo test -p sim -- technical` or `cargo test -p server -- gunnery`. The server crate has no lib target, so `--lib` fails on it.
- In the client, run one spec, e.g. `npx vitest run src/__tests__/car.spec.ts`.
- The client's real type check is `vue-tsc --build`. Running `tsc` on the root `tsconfig.json` checks nothing.
- Vehicle physics tests (`crates/sim/src/vehicle/tests.rs`) run on a small flat world that ends about 200 m out, so a long full-throttle run drives off it.
- Water in a test world has waves, and the surface depends on `Physics::set_tick`. Call `calm_waters()` for a test that needs still water.
- A server test that feeds input with `press(...)` (`vehicles/tests.rs` and others) sees it acted on a couple of ticks later, since the jitter buffer holds two frames before it uses one. Idle two ticks after a one-tick press before asserting on what it did.
- A server test's world has a fixed seed (`Game::new` under `cfg(test)`), and the client's specs that draw from `Math.random` or animate by `performance.now` mock them, so a test passes or fails the same way every run. Keep it so: never assert on wall-clock time or on unseeded chance. For a test suspected of failing now and then, run the test binary that `cargo test -p server --no-run` names 50 times and read the failing assert.
- **Keep `target/` small.** Rebuilds slow down badly once it has grown (a `shared` edit took 37 s at 267 GB and under 2 s after a `cargo clean`). When rebuilds feel slow, check `du -sh target` and clean it. `cargo test -p <crate>` and `cargo test --workspace` build the dependencies with different feature sets, so each keeps its own copy.
- `npm run build:wasm` uses the `wasm-dev` profile (optimised, incremental, a second or two after an edit); only `npm run build` builds the release WASM. Performance runs (`--ignored`, `BENCH=1`) still want `--release`.
- In zsh, `grep --include=*.rs` fails with "no matches found". Quote the glob, or leave it out.
- Blender is at `/Applications/Blender.app/Contents/MacOS/Blender`. To build one car and look at it: `Blender -b -P tools/vehicles/build_car.py -- --only <kind> --preview <dir>`.

## Looking at a bug in the game

- **The user's screenshots are in `shots/`.** F2 in the game saves `shots/<world>-<x>_<z>-<time>.png` at the repo root (git-ignored), so when the user says they saved a screenshot, it is the newest file there. `node client/scripts/shot-info.mjs <png>` prints what it carries: the world, the time of day (`phase`), the camera, the player's position, and what the crosshair hit, with the building's tile and index.
- **Reproduce it in headless Chrome** before guessing at the cause:
  ```sh
  cd client && VITE_WORLD_BASE=/worlds/<world>/ npx vite --port 5287 --strictPort   # in the background
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --enable-unsafe-webgpu \
    --use-angle=metal --remote-debugging-port=9333 --user-data-dir=<scratch dir> --window-size=1600,900 http://localhost:5287/
  node scripts/cdp.mjs eval "<js>"     # runs in the page; prints at most 3000 characters
  node scripts/cdp.mjs shot <out.png>  # a screenshot to read back
  ```
  Port 5199 belongs to another of the user's projects. WebTransport fails headless, so the game stays in SOLO, with no server behind it. The dev build exposes `window.__game`:
  - Move the player by setting `__game.predictor.state.position` and `.previous.position`. Aim the camera with `__game.input.yaw` and `.pitch`; a yaw of -π/2 looks along +x.
  - Set the shot's time of day with `__game.scene.setTimeOfDay(phase)`. `?time=` does nothing in SOLO.
  - Act on the world as the server would: `__game.view.chipped(x, y, z, damage)` dents a building, and `__game.view.carved([...])` takes cells out of it (`CarvedBuilding` in `net/messages.generated.ts`).
  - Hide the "Click to play" panel before a screenshot, or it covers the middle of the view.
  - To narrow a fault down, find a mesh by walking `__game.scene.scene` and switch it off, or its `receiveShadow`, and screenshot again.
  - Stop Chrome and Vite when you are done.

## Working on the renderer

The engine (`engine/`) is judged by what the GPU spends per frame, and on the high-resolution laptops it
is played on that is pixels and memory traffic, not draw calls. [docs/GPU-PERFORMANCE.md](docs/GPU-PERFORMANCE.md)
has the numbers.

- **Measure with `node client/scripts/gpu-bench.mjs`** before and after a change: whole frames to the
  GPU's finish at Retina size, each variant timed alternately with the full frame (`--extra`, `--parts`,
  `--breakdown`). Don't trust the per-pass GPU timers on headless Metal, and don't compare against an
  older run. A change that doesn't measure faster is reverted.
- **The GPU is tile-based (Apple).** It skips hidden opaque pixels itself, so overdraw of opaque,
  non-discarding surfaces is nearly free, and a depth pre-pass only pays for materials that `discard`
  (`ShaderMaterial.depthPrepass`). Never store a multisampled attachment: resolve it in the pass that
  draws it. After the main pass, read the single-sample `sceneDepth` and draw at one sample a pixel.
- **Pixels cost more than triangles.** Solid trees are cheaper than impostors, and the pixel ratio is
  the biggest lever. `rgba16float` was faster than `rg11b10ufloat`.
- **Work that repeats across the screen goes in a texture made once a frame**, by compute where it can
  (the sky panorama the water mirrors), not in every pixel's shader.
- **Memory is measured with `node client/scripts/mem-bench.mjs`** ([docs/MEMORY.md](docs/MEMORY.md)):
  what the page, the workers and the GPU hold, and the garbage a frame makes. A big mesh or texture
  that nothing reads again is made with `uploadOnce`, so only the GPU keeps it; it must then be made
  again when the renderer is replaced (`Scene.onRendererReplaced`).
- [RENDERING.md](docs/RENDERING.md)'s "Measuring, and traps" lists the headless and WGSL traps.

## Deploying

The README's "Deploying" section has the full steps. In short: commit first, because the image is tagged with `HEAD`, then load `.env.local` and run `scripts/deploy.sh`.

- **Ship the world that is live.** `WORLD_DIR` defaults to `client/public`, which holds no world unless one was generated there. Find the live world by matching its manifest against the local ones, and pass it as `WORLD_DIR` unless the user asks for another:
  ```sh
  set -a; . ./.env.local; set +a
  ssh "$SSH_USER@$SSH_HOST" 'docker exec ragnarok md5sum /app/world.msgpack'
  for f in worlds/*/world.msgpack; do echo "$(md5 -q "$f") $f"; done
  WORLD_DIR=worlds/<live world> DOMAIN="$TARGET_DOMAIN" scripts/deploy.sh "$SSH_USER@$SSH_HOST"
  ```
  After a world format change the re-imported worlds no longer match the live one: ask which world is live if it isn't clear.
- **A failed deploy rolls itself back** to the previous container. Read the logs it prints before trying again, and check what is running with `ssh "$SSH_USER@$SSH_HOST" 'docker ps --filter name=^ragnarok$'`.

## How the code is written

- **No backward compatibility.** The game is in development and nothing outside this repo reads its formats, so a new format replaces the old one. Never read an old format next to the new one, and never add a fallback, a "written before" path, a field left out or defaulted "so old files read the same", a cache kept "so earlier downloads are found again", a conversion tool for old data, or a test of old data. Instead:
  - Regenerate the data in the same change. For a tile or manifest change, re-import every world in `worlds/` (`osm-import --area worlds/<name>/area.json --out worlds/<name>/world.msgpack`) and `fixtures/world` (`osm-import --synthetic --out fixtures/world/world.msgpack`). For the wire, run `UPDATE_PARITY=1`.
  - Bump the version (`WORLD_VERSION` in both `world.rs` and `client/src/world.ts`, `PROTOCOL_VERSION`), so a mismatch fails loudly.

- **Comments are full sentences** that say what a thing is in the game's terms, with units (metres, m/s, radians, ticks): "How far the gun kicks back along its line when it fires, metres". Spelling is British (armour, metres, colour). Match the density of the code around you; most `pub const` items and every type carry a doc comment.
- **Rust is the single source of truth.** Gameplay numbers live in `crates/shared`, and the client reads the generated copies rather than repeating them.
- **Vehicle definitions are plain `const` data.** A `VehicleDef` and its `Spec` list every field, with no `Default`. A new `Spec` field therefore means touching every file in `vehicles/`; give it a named constant for the old behaviour (as `Springs::FIRM` does).
- **Mechanics are generic over the data.** Arming a vehicle is a `MountDef` row, not new gunnery code. Read what a `Show`, `Aim` or `Fire` variant already does before adding one.
- **Tests describe behaviour in their names**, e.g. `a_technicals_gun_jams_when_hot_shows_it_and_clears`. Physics numbers such as ride heights are measured by tests, then written into the constants.
- **Write up a feature as you build it.** A new feature gets a write-up in `docs/` (e.g. `VEHICLES-IX.md`) and a line in the README's list. Wire changes also update `docs/NETWORKING.md`.

## Skills

- `.claude/skills/add-vehicle`: the checklist for adding a vehicle kind, end to end.
- `.claude/skills/animate-soldier`: adding or reworking a clip of the soldier (`tools/character/build_character.py`), how to check it, and the client side.
- `.claude/skills/add-orienteering-map`: putting an OCAD map and its render into a world, from `temp-maps/ocad/` to a deploy.
