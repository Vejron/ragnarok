# Architecture migration: towards entities and components

A running record of moving Ragnarok from per-family structs and kind switches to an entity-component
architecture, and from a hand-mirrored client simulation to one shared simulation. It holds the plan,
the rules every step keeps, and a log of what has been done. Update it in the same commit as the work.

Branch: `refactor/ecs-migration`.

## Why

Adding a feature has got steadily more expensive. Adding the loitering drone (`6d90409`) touched 38 files
across the server, shared crate, client simulation, rendering, input, HUD and audio. The causes, as
measured on `master` at `b832836`:

1. **Behaviour picked by kind id.** 143 `kind == KIND_*` branches in about 29 files on both sides of the
   wire. `Gunnery::on_tick` is one `if kind == X { work_x() }` per vehicle; `createVehicleModel` and
   `cameraDistance` on the client are parallel if-chains; the HUD seat role is a nine-way ternary.
2. **Families as one struct with optional parts.** `Vehicle` carries `gun`, `arms`, `drone` and
   `turret_hp` as options; `PlaneArms` also holds a hen's eggs; `Rocket` is bullets, grenades, shells
   and flares. On the wire `thrust.air` is a Roadster's air, a hen's stamina or a drone's battery, and
   `damage[4]` is a wheel or a turret.
3. **One collection per family, each enumerated by hand.** `players`, `vehicles`, `rockets`, `pickups`,
   `roosters` and `bots` share one id space but live in separate maps. `blast_at` loops over each in
   turn; roosters have their own hit test and damage function. A new NPC is a new map, a new hit test,
   a new blast loop, a new snapshot section.
4. **The simulation exists twice.** `client/src/sim/vehicle.ts`, `movement.ts`, `carve.ts`,
   `terrain.ts`, `items.ts`, `vehicles.ts` and `net/codec.ts` mirror Rust by hand, about 3 to 4 thousand
   lines, held in step by fixtures. The server runs `rapier3d` 0.36 and the client
   `@dimforge/rapier3d-compat` 0.21.
5. **Hubs.** `client/src/game.ts` is 1,918 lines with 98 fields and 15 `Map`s keyed by entity id, each
   cleaned up by hand. `mechanics/combat.rs` does damage, blasts, every projectile, hitscan, items,
   scoring and respawn.

Performance is not the motivation: a full room of 64 costs 2 to 3.5 ms of the 16.7 ms tick, and there are
hundreds of dynamic entities, not hundreds of thousands. An ECS is chosen for structure, so components
are the capabilities and a new thing is a new combination of them.

## Rules for every step

- **Tests stay green.** `cargo test --workspace` and `cd client && npx vitest run` pass at every commit.
  Tests that poke a migrated collection are rewritten against the new API, keeping what they assert.
- **The wire does not change until Phase 3.** Snapshots are built from components into today's
  `EntityState`, `VehicleState` and friends, so the client needs no change while the server moves.
- **Behaviour is kept.** Iteration that consumed `world.rng` or produced output in id order (the old
  maps were `BTreeMap`s) stays in id order: queries are sorted by `NetId` where order is observable.
- **No performance regression.** `full_room_tick_cost` (release, fixture city) is re-run after each
  family is migrated and compared with the baseline below.
- **Static world stays out.** Buildings, trees, terrain and carving keep their spatial structures; the
  ECS holds the dynamic entities only.
- **One family at a time**, old and new side by side, legacy collection deleted as its family lands.

## Target shape

Server: `bevy_ecs` used on its own (no Bevy app, no scheduler: the ordered `Mechanic` list stays the
schedule). Components, grouped by what they give an entity:

| Group | Components |
|---|---|
| Identity | `NetId(u32)` (the wire id, shared id space), `NetKind(u8)` (which model the client draws; a rendering hint, never a logic branch) |
| Place | `Transform { position, yaw }`, later a rigid-body handle for physics-driven entities |
| Life | `Health { hp, max }`, `Team`, `Hurtbox` (capsule now; box and armour profile later), `Respawn`, `Home`, `DropOnDeath`, `Wreckable` |
| Control | `InputFrame` on anything controllable (written by the network, a bot brain or a drone link), `Seats`, `RemoteLink` |
| Movement | `CharacterController`, and one per lift model: `Wheeled(spec)`, `Rotor`, `Wing`, `Legs`, `Quad`, `Loiter` |
| Weapons | `WeaponMount { seat, aim, projectile, rate, heat, ammo, refill }`: the tank cannon, heli autocannon and pods, plane guns and bombs, ring gun, mortar and hen eggs all become data |
| Projectiles | `Projectile`, `Ballistic`, `Fuse(Contact / Timer / Proximity)`, `Bounce`, `Burst(Blast)`, `Light` |
| Resources | `Energy` (one meaning per component: air, stamina, battery) |
| Brains | `Wander`, `BotBrain`, `Crew` |

Cross-cutting: one damage pipeline for everything with `Health`, one hit test and one blast loop over
everything with a `Hurtbox`, and later one spatial grid for blasts, hitscan, interest and bot senses.
Events (`Damage`, `Kill`, `Blast`, `Noise`) replace the `world.hits`, `kills` and `noises` vectors.

Client (Phase 4): an entity store keyed by `NetId` holding replicated components, with one presenter per
component (model, turret, thrust effects, tyres, audio, HUD gauge) that owns its per-entity state and its
`onAdd` / `onRemove`. Camera rigs and input modes become a registry picked by seat data.

## Plan

Two tracks that do not depend on each other: **A** (shared simulation in WASM) and **B** (server ECS).
Phase 3 and 4 need both.

### Phase 0: guardrails

- [x] Branch `refactor/ecs-migration`.
- [x] Baseline: Rust and client test suites (below).
- [x] Baseline: `full_room_tick_cost` (below).
- [ ] Generate the client's `items.ts` and `vehicles.ts` tables from Rust, as `consts.generated.ts`
      already is, and drop their half of the parity fixture.

### Phase 1 (track A): one simulation, native and WASM

- [x] Install the toolchain: `rustup target add wasm32-unknown-unknown`, `wasm-bindgen-cli` 0.2.129
      (the `wasm-bindgen` crate must be pinned to the same version).
- [x] Extract `crates/sim`: the server's `physics` module (Rapier world, character controller, the
      vehicle lift models in `vehicle`, `flight` and `spec`), with no tokio or salvo. It builds for
      `wasm32-unknown-unknown` unchanged. Movement rules, carve and terrain stay in `shared`, which
      `sim` depends on.
- [ ] Re-key the lift models on spec data instead of kind (the remaining `KIND_*` matches in
      `sim/src/vehicle.rs`).
- [x] One `step_character` for both sides: the server's `Movement` mechanic and (next) the client's
      prediction call the same function, ride and inherit included.
- [x] A static scene that streams: `Physics::streamed`, `add_tile`, `remove_tile`, `refresh`, and
      snapshot vehicles as solid stand-ins (`set_obstacle`), built with the server's own collider code.
- [x] `crates/sim-wasm` and a client build step; the client's stickman prediction on it.
- [x] Move vehicle prediction to it (`VehicleDriver`, `CarSim`), so the client's second Rapier world
      is the server's too; then delete `sim/vehicle.ts`, the rest of `sim/physics.ts` and
      `@dimforge/rapier3d-compat`.
- [ ] The remaining mirrors: `carve.ts` (also drawn from, so it may stay as a reader of the WASM
      side), `terrain.ts`, the item and vehicle tables, the codec.

### Phase 2 (track B): server ECS, one family at a time

- [x] ECS foundation in `World`: the `bevy_ecs` world, the `NetId` index, sorted queries.
- [x] Generic life: `Health`, `Hurtbox`, `Respawn`, `Home`, `DropOnDeath`; one hit test, one damage
      path and one blast loop for ECS entities.
- [x] Roosters (the proof: 3 files).
- [x] Pickups.
- [x] Cache query states, so the per-call setup cost seen in the measurements below does not grow
      with the families that follow (a per-tick id list proved unnecessary).
- [x] Projectiles (rockets, grenades, shells, bullets, flares) as `Projectile` + fuse + burst.
- [x] Weapon mounts as data (`shared::mounts`), worked by one routine in place of the `work_*`
      functions.
- [x] Vehicles as entities, with `Mounts` and `Drone` as components of their own.
- [x] Per-kind branches in `vehicles.rs` to data (`VehicleDef::body`).
- [x] A vehicle's health and wreck state as `Health`, `HurtBy` and `Wreck`.
- [x] `Seats` out of `Vehicle`.
- [x] The sim's per-kind controls and solids into its `Spec`.
- [x] Players as entities (`Player` component).
- [x] Bots' minds as a `Bot` component on the player's entity.
- [ ] A player's health and hit test joined with the generic ones.
- [ ] Events in place of `hits`, `kills`, `noises`.

### Phase 3: component replication (protocol v2)

- [ ] Per entity: id, component mask, changed components; codec generated from Rust.

### Phase 4: client entity store and presenters

- [ ] Store and presenters; split `game.ts`; camera and input registries.

### Phase 5: prefabs

- [ ] A vehicle is one definition, one model, and a lift system only when it moves in a new way.
      Measure: files touched per new kind (38 today, aim for 3 to 5).

## Baselines

Taken on `refactor/ecs-migration` before any change (`b832836`), macOS arm64, Rust 1.97.1, Node 24.16.

| Suite | Result |
|---|---|
| `server` | 355 passed, 2 ignored |
| `shared` | 59 passed |
| `osm-import` | 81 passed, 2 ignored |
| `editor` | 2 passed |
| client (vitest) | 50 files, 375 passed |

`full_room_tick_cost`, release, fixture city (228 buildings, 110 trees, 64 people), four runs:

| | Before (`b832836`) | After roosters and pickups |
|---|---|---|
| Mechanics, average per tick | 1.67 to 1.70 ms | 1.64 to 1.66 ms |
| Mechanics, worst tick | 2.92 to 3.26 ms | 2.61 to 2.95 ms |
| Snapshots for the room, per call | 759 to 782 µs | 727 to 741 µs |
| `Pickups` | 79 to 94 ns | 612 to 659 ns |
| `Npcs` (+ `Life` after) | 197 to 348 ns | 1.3 to 1.5 µs |
| `Combat` | 30.6 to 31.6 µs | 34.1 to 34.3 µs |

The totals are unchanged within noise. The migrated mechanics cost a few hundred nanoseconds to a few
microseconds more, from building a query state per call and sorting ids; that is about 0.03% of the
budget and is noted as a follow-up in Phase 2.

## Log

Newest last.

### 2026-10-22: Phase 0 started

- Created the branch and this document; took the test baselines above, all green.
- The WASM toolchain is not installed (no `wasm32` target, no `wasm-bindgen`), so track A waits for the
  go-ahead to install it. Track B does not need it and starts now, with the families that do not touch
  vehicle physics (roosters, pickups, projectiles).
- Chose `bevy_ecs` (0.20) over `hecs`: change detection will drive delta replication in Phase 3, and
  resources and events fit the mechanics. Its scheduler is not used.

### 2026-10-22: Phase 2 started: ECS foundation, roosters and pickups

The server's first families now live in a `bevy_ecs` world. The wire, the client and every behaviour
the tests pin are unchanged.

**Foundation** (`crates/server/src/entities.rs`):

- `Entities` wraps a `bevy_ecs::world::World` and an index from the wire id to the ECS `Entity`.
  `World::entities` holds it, beside the legacy maps. `spawn(id, bundle)` attaches `NetId(id)`.
- `ids::<Filter>()` returns matching ids **sorted**, so passes that draw random numbers or make
  output keep the order the `BTreeMap`s gave them. Order-free passes query directly (the pickup timer
  is a plain `query::<&mut Pickup>()`).
- Components are registered when the world is made, so read-only helpers can use `try_query` on a
  shared borrow. Mechanics register their own (`mechanics::register_components`).
- Generic components: `NetId`, `NetKind`, `Transform`, `Health`, `Hurtbox`, `Home`, `Respawn`,
  `DropOnDeath`, and the `Respawned` marker.
- `Mechanic` gained `name()` (from the type name). The tick-cost test prints the names from the real
  list instead of a hand-kept one, which would have gone stale as soon as `Life` was added.

**Life** (`mechanics/life.rs`, a new mechanic run just before `Npcs`):

- `first_hurtbox_hit`: one ray test for every living entity with a `Hurtbox`. Ties go to the lower id.
- `blasted`: one blast gather over the same entities, by distance to the capsule centre.
- `damage_entity`: takes health off, and on death starts `Respawn` and leaves `DropOnDeath`. A
  `unique` drop clears the earlier one first.
- `place_at_home`, and the respawn countdown in `Life::on_tick`. The entity is marked `Respawned`
  so the mechanic owning its other state can start that afresh.
- `open_ground` moved here from `npcs.rs` (bots use it too).
- `combat.rs` no longer knows about roosters. Hitscan, bullets and blasts call the generic functions,
  and the hit marker's kind (`HIT_ROOSTER`) comes from the `Hurtbox`.

**Roosters** (`mechanics/npcs.rs`):

- `Rooster`, `RoosterMode` and `World::roosters` are gone. A rooster is the bundle `npcs::rooster(home)`:
  `NetKind(KIND_ROOSTER)`, `Transform`, `Health`, `Hurtbox`, `Home`, `Respawn`, `DropOnDeath` (the
  golden egg) and `Wander`.
- `Wander` is a generic brain (idle, walk, call) with a `Gait` of numbers, and `ROOSTER_GAIT` is one gait.
  A second wandering creature is a new bundle and a gait, not a new map, hit test or blast loop.
- The snapshot draws every living entity with `NetKind` + `Transform` + `Health` as an `EntityState`.
  `FLAG_CROWING` comes from `npcs::calling`.
- The tests were rewritten against components, keeping their assertions. Two were added: a rooster
  back from the dead stands about first, and one killed again leaves only one egg lying about.

**Pickups** (`mechanics/pickups.rs`):

- `Pickup` is now a component, owned by the mechanic. The position moved to `Transform`.
  `World::pickups` is gone.
- `World` has a small facade beside the component: `add_pickup`, `pickup_ids`, `pickup`,
  `pickup_position`, `remove_pickups`, and for tests `pickup_mut` and `clear_pickups`. Tests and other
  mechanics go through it, so the next storage change will not touch them again. The 14
  `world.pickups.clear()` calls in test set-ups became `world.clear_pickups()`.
- Drops on death (`Life`) and the snapshot's `pickup_states` use the same facade, in id order as before.

**Checks:**

- `cargo test --workspace`: server 356 passed (355 before, plus one new test), shared 59,
  osm-import 81, editor 2.
- Client vitest: 50 files, 375 passed. The client did not change.
- `cargo clippy -p server --all-targets`: nothing new. The two notes left, in `game.rs` and
  `gunnery.rs`, were there before.
- The real server starts on the generated world, lays out the editor's six items and the rooster, and
  listens.
- Timings: the table under Baselines.

Not yet checked: a game in a browser (shooting the rooster, picking up and dropping items).

**Next:** cache the query states (the Phase 2 follow-up above), then projectiles. `Rocket` becomes
`Projectile` plus a fuse and a burst component, and `step_rockets`, `step_bullet`, `step_grenade`,
`step_flare` and `step_shells` become systems over those components. Vehicles wait for Phase 1's
`crates/sim`, or go first with their physics left in `Physics`, if track A stays blocked.

### 2026-10-22: toolchain installed; physics extracted into `crates/sim`

- Installed the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.129. Binaryen (`wasm-opt`)
  is not installed; it only shrinks the output and can wait.
- `crates/server/src/physics/` moved to `crates/sim/src/` with `git mv`, so history follows: `mod.rs`
  became `lib.rs`, and `tests.rs`, `vehicle.rs` and `vehicle/` came along as they were.
  `pub(in crate::physics)` became `pub(crate)`, and the server's `crate::physics::` paths became `sim::`.
- The server no longer depends on `rapier3d`: nothing outside `physics/` used it. Only `sim` does.
- Test-only helpers (`set_car_roll`, `set_car_dive`, `move_character`) sit behind a `testing` feature,
  which the server turns on only as a dev-dependency.
- `cargo build -p sim --target wasm32-unknown-unknown --release` builds as it is. The server's Rapier
  0.36 physics, its character controller and every lift model can run in the browser with no changes.
- Tests: server 258 and sim 98, which is the same 356 as before, split between the crates.

**Next:** `crates/sim-wasm` (a `cdylib` made with `wasm-bindgen`) and a client build step. Then the
client's stickman prediction runs on `sim`, in place of `sim/movement.ts` and the `rapier3d-compat`
character motor, checked against the existing `predict.spec.ts` fixture.

### 2026-10-22: one character step, and a static scene that streams

**One `step_character`.** The ride and inherit logic around `move_character_scaled` moved from the
server's `Movement` mechanic into `sim::Physics::step_character`, so the client's prediction can call the
same function. Moving it turned up a drift between the mirrors. When a stickman steps off a moving
roof, the server decays the knockback and then adds the car's speed. `stepCharacter` in
`client/src/sim/physics.ts` adds the speed and then decays. So every step-off was a small
misprediction, which reconciliation hid. The server's order stands, since it is the authority, and
the client gets it by running the same code.

**Streaming** (`crates/sim/src/stream.rs`):

- `static_colliders` is split into the slab, the walls and the objects (buildings, bridges, trunks).
  Its output and order are unchanged, and the server still builds everything at once.
- `Physics::streamed(&Manifest)` starts with only the slab and the walls.
- `add_tile(&Manifest, col, row, &Tile)` adds a tile's own heightfield, laid out as the server's grid
  is. Its buildings and trees are built by assembling a world of that one tile with terrain switched
  off, so the ids (`ObjId`) and the triangle ranges are exactly the server's. Terrain is off because
  `WorldFile::assemble` would otherwise allocate a world-sized ground grid on every tile.
- `remove_tile` and `refresh` cover tiles leaving and queued changes. `refresh` applies queued
  `destroy` / `replace_building` and updates the broad-phase, for a world that never calls `step_cars`.
- `set_obstacle` / `remove_obstacle` put snapshot vehicles in as fixed bodies with the same solids
  `add_car` gives them. Those solids are factored out as `vehicle::extra_solids` and `heavy`. They
  replace `extraSolids` in `physics.ts`, which was a hand copy. A tank's turret can be turned off.
- `in_parallel` runs in sequence on `wasm32`, where spawning a thread panics at run time even though
  it compiles.

**Tests** (`crates/sim/src/stream/tests.rs`, 5 new, sim now 103):

- The fixture city, built whole and streamed tile by tile, gives the same hit, within 1 mm, for every
  ray of a grid of downward and sideways probes (more than 100 of them hit something).
- A stickman running and jumping for 10 s over the city ends up within 1 cm of the same run on the
  whole world.
- Removing a tile leaves only the slab under it.
- Destroyed buildings and trees stop colliding.
- Obstacles are solid, carry their velocity, and a tank's turret comes off and goes back on.

### 2026-10-22: the client's stickman runs on the server's physics

**`crates/sim-wasm`:**

- A `wasm-bindgen` `cdylib` over `sim`. `ClientSim` is made from the `world.msgpack` bytes and takes
  tiles as their files' bytes, decoded in Rust by `shared`'s own `Tile::decode`.
- It fells trees and buildings and carves buildings by `ObjId` (tile and index, as the server names
  them), places snapshot vehicles as obstacles, and steps the stickman with `Physics::step_character`.
- Character state crosses the boundary as 11 `f64`, which hold every `f32` and every id exactly.
- `stream.rs` gained `carve_building`, which cuts a streamed tile's building into cells with the
  server's `building_shell` and `solid_of`. For that, each streamed tile keeps its one-tile world.

**Build:**

- `client/scripts/build-wasm.ts` (`npm run build:wasm`) runs `cargo build --target
  wasm32-unknown-unknown --release -p sim-wasm` and `wasm-bindgen --target web` into
  `client/src/sim/wasm/`, which is git-ignored.
- npm's `predev`, `pretest`, `pretype-check` and `prebuild` run it. When nothing changed it takes well
  under a second.
- The output is 2.7 MB, 0.91 MB gzipped. For comparison, `rapier3d-compat` ships 4.3 MB (1.66 MB
  gzipped), because it base64-embeds its WASM. Until vehicle prediction moves over, the page loads
  both.
- CI's client job now installs the `wasm32` target and `wasm-bindgen` 0.2.129 (`taiki-e/install-action`),
  and `scripts/deploy.sh` checks for `cargo` and `wasm-bindgen`.

**Client:**

- `sim/motor.ts` (`SimMotor`) replaces `RapierMotor`, with the same jobs: tiles, vehicles as solids,
  ray casts for the trajectory previews, and the step.
- `Predictor` now takes a step function (`Stepper`) instead of a motor.
- `worldview.ts` names destroyed and carved buildings by tile and index, where it used to keep
  collider slots for the stickman's world. The driver's world still does that, until it moves too.
- `world.ts` keeps the raw manifest and tile bytes (`WorldData.bytes`, `TileData.bytes`) for the WASM
  side.
- Deleted: `sim/movement.ts` (the mirrored rules) and, from `sim/physics.ts`, `stepCharacter`, the
  stance change, `inherit` and `RapierMotor`, about 260 lines of mirror. `f32` moved to `sim/round.ts`.
  `physics.ts` keeps only what `VehicleDriver` uses.

**Tests:**

- Vitest loads the WASM in `src/__tests__/setup.ts` with `initSync`, since Node's fetch cannot read
  files. `src/__tests__/worlds.ts` builds test worlds as manifest and tile bytes, the same way real
  ones arrive.
- `predict.spec.ts` runs against the real physics in place of a hand-written flat floor.
- The cross-language checkpoints recorded from native (the run against a wall and the 20° ramp) pass
  with the Rust code in WASM.
- The "walks off a moving roof" test now expects the server's order: knockback decays, then the car's
  speed is added. The two Predictor tests that wanted 1e-6 m now accept 1e-3 m, because the real
  controller nudges a replayed step by a fraction of a millimetre, where the fake floor moved exactly
  nothing.
- `movement.spec.ts` lost the 8 tests that copied `shared::movement`'s Rust tests one for one. It keeps
  the speed-scale and aim tests, on the real physics.
- Client: 50 files, 367 tests (375 before, less those 8).
- Rust: sim 103, sim-wasm 1, server 258, shared 59, osm-import 81, editor 2.
- `cargo clippy --workspace --all-targets -- -D warnings`, as CI runs it, is clean. Two old lints that
  current stable Rust flags were fixed in a separate commit (`chore(server): satisfy clippy`).

**Measured** on `worlds/ersboda-alidhem` (450 tiles, with terrain), in vitest on Node, in two rounds,
with the old `RapierMotor` put back temporarily for the comparison:

| | Old JS Rapier mirror | Server's code in WASM |
|---|---|---|
| Making all 450 tiles solid | 3.97 to 5.13 s | 1.25 to 2.10 s, decoding included |
| Slowest single tile | 34 to 62 ms | 31 to 55 ms |
| One stickman step | 19.8 to 22.6 µs | 17.1 to 19.0 µs |
| Apart after 50 s of the same inputs | | 1 mm |

**Browser:**

- Headless Chrome loaded the module through Vite's dev server: `init()` fetched the WASM (served as
  `application/wasm`).
- It built all 450 tiles of `ersboda-alidhem`, found the ground at the first spawn and walked the
  stickman 26.6 m over the terrain, staying grounded.
- Not yet checked: a full game session in a browser (WebGPU and WebTransport, playing against the
  server).
- `client/public/world.msgpack` and `client/public/dev/` are world version 9, older than the client's
  version 11. Regenerate them (`scripts/world.sh`, `scripts/world-dev.sh`) before playing on them.

**Next:** vehicle prediction on `sim` (`VehicleDriver` and `CarSim` in `sim/vehicle.ts`, about 1,300
lines that mirror `crates/sim/src/vehicle.rs` and `flight.rs`). Then `rapier3d-compat` goes.

### 2026-10-22: the driver's vehicle runs on the server's physics; the JS physics engine is gone

The client used to keep a second Rapier world, holding a second copy of every streamed tile, in which
`CarSim` (1,329 lines in `sim/vehicle.ts`) mirrored the server's `drive_car` and `step_cars` line by
line for every lift model. Now the vehicle the player drives is a body in the client's one `sim`
world, the one the stickman walks, stepped by the server's own code.

**`sim`:**

- `CarControls::driven(kind, &input)` is how a driver's input becomes controls. It moved from the
  server's `Vehicles` mechanic, which now calls it. The client's copy (`controlsOf`) had drifted:
  it gave every vehicle an aim, where the server gives one only to planes and drones. Nothing but a
  plane read it, so it did no harm yet, but it was the same kind of drift as the stickman's.
- `set_car_state` puts a vehicle in the state a snapshot reports, for reconciling: pose, `DriveState`
  motion (steering angle and a tank's ramped spin included) and the air tank.
- `refresh` no longer steps the pipeline while a vehicle is simulated, since that would move the
  predicted vehicle on. Its next `step_cars` brings the queries up to date. The old TS code had the
  same guard.
- `set_obstacle` never puts a stand-in for a vehicle that is simulated.
- `CAR_GRAVITY` moved to `shared/consts.rs`, so the client's renderers (the walker's and the hen's
  rest pose) get it generated instead of from the mirror.
- New Rust tests take over from TS tests that tested deleted TS code: the controls mapping (4),
  `CarCondition::of` with a tank's turret flag (2), the state round trip, and the refresh and
  stand-in guards. sim now has 111 tests.

**`sim-wasm`:** `startCar`, `stopCar`, `setCarState` (pose, motion, tank, wear and turret from the
snapshot), `controlsFor`, `driveCar` (`drive_car` then `step_cars`) and `carState`. Controls and car
state cross as fixed `f32` layouts.

**Client:**

- `sim/driver.ts` (`VehicleDriver`) keeps the prediction bookkeeping as it was: pending inputs,
  replay, the decaying correction and the interpolated pose. It now drives the car through
  `SimMotor`.
- `game.ts` makes it from the motor and no longer feeds it obstacles: the motor places every vehicle,
  and `sim` leaves out the one it simulates.
- `worldview.ts` keeps one world in step instead of two: no more driver tile colliders, felled-trunk
  slots or rebuilt building compounds.
- Deleted: `sim/vehicle.ts`, `sim/physics.ts` and the npm dependency `@dimforge/rapier3d-compat`.
  This change removes 1,991 lines and adds 761.

**Tests:**

- `vehicle.spec.ts` runs every native-recorded checkpoint on the WASM build: jeep, sports car, tank,
  Strv, damaged jeep, walker, Roadster, helicopter, drone, loitering drone, plane, the 12° slope and
  hen. All pass with the old tolerances.
- So do the reconciliation tests and the turret test, now through the motor.
- The aim checks compare with `toBeCloseTo`, because the aim now comes back as an `f32`.
- Three destruction tests (a fallen building, a carved hole, cubes rather than walls) now check that
  the cells the client draws (`carve.ts`) are the cells the server's physics cuts. That is a real
  cross-language check where before it compared TS with TS.
- The boundary-wall tests check, with rays, that a shaped world's walls come through to the client
  from the manifest's tile list.
- Deleted with the code they tested: `trunkColliderIndex` (2 tests) and `conditionOf` (2 tests,
  moved to Rust).
- Client: 50 files, 363 tests. Rust: sim 111, sim-wasm 2, server 258, shared 59, osm-import 81,
  editor 2. `cargo clippy --workspace --all-targets -- -D warnings` is clean.

**Measured** on `worlds/ersboda-alidhem`: a jeep driven 20 s by the same inputs, with the old `CarSim`
put back temporarily:

| | Old `CarSim` in JS Rapier | Server's code in WASM |
|---|---|---|
| One predicted tick, the 21 tiles loaded at the spawn | 292 to 328 µs | 12 to 33 µs (12 once warm) |
| One predicted tick, all 450 tiles | 10.3 to 12.5 ms | 14 to 32 µs |
| Apart after 20 s | | 2 cm |

A reconcile replays about 20 ticks, so it now costs about 0.24 ms instead of about 6 ms.

**Download:** the WASM grew to 2.76 MB (943 KB gzipped). `rapier3d-compat`, 4.3 MB (1.66 MB gzipped)
of base64-embedded WASM, is no longer bundled, so the page is about 1.6 MB gzipped lighter than
before this change, and lighter than `master`. One copy of the static scene is held in memory
instead of two.

**Browser:** in headless Chrome through the dev server, `VehicleDriver` started predicting a jeep on
`ersboda-alidhem` (all 450 tiles) and drove it 24.6 m in 10 s, resting on the terrain.

Not yet checked: a full game session in a browser.

**Next:** phase 1 is down to the remaining mirrors: the codec and the item and vehicle tables, which
can be generated from Rust, and `carve.ts` and `terrain.ts`, which the renderer reads. Then Phase 1's
"re-key the lift models on spec data" and Phase 2's projectiles.

### 2026-10-22: projectiles are made of what they do

Every projectile used to be a `Rocket` in `World::rockets`, with nine kinds told apart by `if kind ==`
across five steppers in two mechanics (`step_rockets`, `step_bullet`, `step_grenade` and `step_flare`
in `combat.rs`, `step_shells` in `gunnery.rs`), and two more `burst` helpers that matched on kind for
the blast. Now they are ECS entities in `mechanics/projectiles.rs`, made of parts:

| Part | Components |
|---|---|
| Every projectile | `Projectile { kind, owner, source, ttl }`, `Transform`, `Velocity` |
| How it moves | `Gravity(scale)`, `Motor { accel, top }` (a rocket), `Sinks { drag, max_sink }` (a flare) |
| What meeting something does, one of | `Strikes { bodies, armor, lost_below }` (rockets, shells, bombs), `Bounces { radius, keep, smash }` (grenades, eggs), `Sticks { radius }` (flares), `Round { item, back, flown }` (a gun's bullet) |
| What bursting does | `Burst { weapon, radius, damage, knock, upward }`, absent for what only goes out |
| When it moves | `GunStage`: at the start of `Gunnery`'s tick, before the vehicle guns fire. Without it, at the end of `Combat`'s, in the tick it is thrown or fired |

- `prefab(kind)` gives each kind its parts, and `burst_of(kind)` its blast. `burst_at` sets off a blast
  that never flew (a barrel or a launcher pressed against a wall).
- `step(world, dt, stage)` runs each projectile's whole tick in id order, as before. The motion parts
  come first (`fly`), then its one contact handler.
- Nothing branches on `kind` any more. It is what the client draws. A new projectile is a new prefab.
  `a_projectile_is_what_it_is_made_of` builds one no item fires (a bouncing ball with a rocket motor
  that bursts like a grenade) from parts alone, and it works.
- The handlers are the old steppers ported nearly line for line, so behaviour is kept: armor
  ricochets, eggs smashing, flares that stick and burn, and rounds that fall off with distance and
  rewind for lag. The rule for falling out of the world stays per behaviour as it was: a shell is lost,
  a grenade bursts, a rocket has no such check.
- `World` has `launch`, `launch_as`, `in_flight` (a `BTreeMap<u32, Flying>` snapshot), and `flying`.
  `projectiles::count` is on the entities alone, so it can be asked while a player is borrowed.
  `Rocket` and `World::rockets` are gone.
- Every spawn site (rocket launcher, grenade, flare, gun rounds, tank, helicopter, mortar and Strv
  shells, helicopter pods, plane bombs, hen eggs) calls `world.launch(Flying { .. })`. The snapshot's
  `projectile_states` reads `in_flight`. The wire is unchanged.
- The tests' 42 reads and inserts on `world.rockets` now go through the facade. A mechanical rewrite
  turned one multi-line insert into `in_flight().insert(..)`, which would have written to a
  temporary copy; it was caught and fixed, and a search confirms nothing else writes to a snapshot.
- `combat.rs` is 979 lines (1,202 at the start of the migration) and `gunnery.rs` 446 (525). The
  change removes 461 lines and adds 148, plus the new module.

**Checks:**

- Server: 261 tests. That is 258 plus 3 new ones: every kind has exactly one contact behaviour, the
  right stage and the right blast; a projectile made of parts works; a shell waits for the guns.
- Client: 363 tests, unchanged. Clippy with `-D warnings` is clean.
- `full_room_tick_cost`: mechanics 1.57 to 1.61 ms (baseline 1.67 to 1.70), snapshots 0.69 to 0.71 ms
  (0.76 to 0.78), `Combat` 35 to 37 µs (31 at baseline), `Gunnery` under 1 µs.

**Next:** players and bots, the largest family, with events (`Damage`, `Kill`, `Noise`) in place of
the `hits`, `kills` and `noises` vectors; or first the weapon mounts on vehicles (`Gun`, `PlaneArms`
and the `work_*` functions in `gunnery.rs`), which now only launch projectiles.

### 2026-10-22: a vehicle's weapons are rows of data

`Gunnery::on_tick` was one `if kind == X { work_x() }` per armed kind, plus a second loop for hens.
There were eight hand-written `work_*` functions, and two state structs that each meant several things:
`Gun` was a tank's turret, a helicopter's nose gun and its rocket pods, and a plane's ring mount.
`PlaneArms` was a plane's guns and bombs and also a hen's eggs. Now:

- `crates/shared/src/mounts.rs` describes a weapon mount: `MountDef { seat, aim, muzzle, fire,
  trigger, cooldown, spread, recoil, heat, ammo, needs, show }`.
- `VehicleDef::mounts` lists a kind's mounts in the order they are worked. The `gun` and `arms` flags
  became methods derived from the mounts, so the client's parity fixture is byte-identical.

| Part | Variants |
|---|---|
| `Aim` | `Fixed` (pods, forward guns, racks), `Turret` (stabilised in the world: the tank), `Bore` (fixed in the hull, elevates only: the Strv S), `Mount` (follows the look as seen from the hull, optionally limited: the helicopter's nose gun, the walker's chin guns, the plane's ring), `Lob` (the mortar's dial and high lob) |
| `Muzzle` | `One(pivot)`, `Turns(pivots)` (pods, forward guns), `Twin` (the walker's two barrels) |
| `Fire` | `Shell { kind, speed, length, ttl }`, `Rocket`, `Hitscan` (a machine gun `Round`), `Drop { kind, toss }` (bombs, eggs) |
| `Ammo` | `full` with `Refill::Parked(ticks)` (the plane's rack) or `Refill::Regrow(ticks)` (the hen's clutch) |
| `Need` | `Upright`, `Turret` (block aiming and firing); `Untowed`, `Height`, `Clear` (block firing) |
| `Show` | which wire field shows it until protocol v2: `Turret`, `Guns`, `Rack` or `Hidden` |

- On the server, `Vehicle::mounts: Vec<Mount>` holds one state per definition: aim, cooldown, shots,
  range, heat, jam, ammo and refill. `Gun` and `PlaneArms` are gone.
  - `Vehicle::shown(show)` and `turret()` read them; the snapshot builds `Turret` and `Arms` from them.
  - `restock()` replaces `PlaneArms::fresh` on respawn. Only the arms are reset, as before; a turret
    keeps its aim and count.
- `gunnery.rs` has one `work` for every mount, with `turn`, `shoot`, `refill`, `can_work` and
  `can_fire`. Each arm is one of the old functions' steps, in the same order: the cooldown, then
  cooling, refill, seat and needs, aim, trigger, the shot, recoil and counters. It is 275 lines (446
  before; 525 at the start of the migration), with no branch on vehicle kind.
- Arming a new vehicle is a row in `mounts.rs`. A new way to aim or fire is a variant and its match
  arm. `a_mount_made_of_parts_no_vehicle_has_works` arms a jeep's passenger with a ring-mounted
  rocket launcher that runs out and regrows.
- **One change in order:** a hen's egg is now laid in vehicle-id order with the other weapons, not in
  a pass after all of them. On a tick where a hen and a lower-numbered gunned vehicle both fire, their
  projectiles get each other's ids. Nothing else changes; the wire is unchanged.
- I removed a duplicated `turret_hp` reset in the wreck respawn (a misindented copy from e60d7cc).

**Checks:**

- Server: 262 tests (261 plus the one above), all passing unchanged. The 49 test reads of `gun` and
  `arms` now go through `shown`.
- Shared: 62 (59 plus 3 consistency tests on the mount table). Client: 363, unchanged. Clippy with
  `-D warnings` is clean.
- `full_room_tick_cost`: mechanics 1.54 to 1.59 ms, snapshots 0.68 to 0.69 ms, `Gunnery` 0.86 to 0.89 µs
  (under 1 µs before).

**Next:** vehicles as ECS entities, with `Mounts`, `Seats` and `Drone` as components, which now have
clear boundaries. Or players and bots with events (`Damage`, `Kill`, `Noise`) in place of the `hits`,
`kills` and `noises` vectors.

### 2026-10-22: vehicles are entities

`World::vehicles` (a `BTreeMap<u32, Vehicle>`) is gone. `Vehicle` is a component, spawned with its
`NetId` by `add_vehicle` and despawned by `remove_vehicle` and the drones' `remove`.

- `Entities` gained generic accessors that borrow only `world.entities`, so a vehicle can be held
  while `world.physics` or `world.players` is changed, as the map allowed:
  - `of::<T>(id)` and `of_mut::<T>(id)` panic when the entity has no `T`, like indexing a map.
  - `has`, `count`, and `all::<T>()`, which gives every `(id, &T)` in id order.
  - Loops that changed every vehicle go `for vid in ids::<With<Vehicle>>()` with `of_mut`.
- The 180 call sites were rewritten mechanically: `[&id]` became `of`, `get` and `get_mut` became
  the ECS versions, `iter` became `all`, and `keys` became `ids`. The compiler then flagged the
  bindings that need `mut` for a `Mut<Vehicle>` and the `(&id, ..)` patterns that are now `(id, ..)`.
- Behaviour is unchanged: every pass still goes in id order.

**Checks:** server 262 tests, unchanged; clippy clean. `full_room_tick_cost`: mechanics 1.53 to 1.57 ms,
snapshots 0.69 to 0.71 ms. `Vehicles` is 28.7 µs (26), `Drones` 0.86 µs (0.43) and `Gunnery` 1.36 µs
(0.87). The difference is each pass building its query and sorting, which caching the query states
would remove (an open item above).

**Next:** split `Vehicle` into the components it is made of, starting with `Drone` and the weapon
mounts.

### 2026-10-22: a vehicle's weapons and a drone's link are components

`Vehicle` lost `mounts: Vec<Mount>` and `drone: Option<Drone>`. Both are components on the vehicle's
entity now, present only on the vehicles that have them.

- **`Mounts { defs, state }`**: the mounts' definitions and what each is doing. `add_vehicle` inserts
  it when the kind's `VehicleDef::mounts` is not empty.
  - Because it carries its own `defs`, an entity is armed by having one, whatever its kind. The test
    `a_mount_made_of_parts_no_vehicle_has_works` now bolts a rocket ring onto a jeep. The room's own
    tick works it: aim, run out, regrow.
  - `Gunnery` goes over `ids::<With<Mounts>>()` and never looks at the kind.
  - `shown`, `turret` and `restock` moved from `Vehicle` to `Mounts`. The respawn in `Vehicles` notes
    that a vehicle is put back as new and restocks its `Mounts` once the vehicle borrow ends.
- **`Drone`**: inserted by `drones::launch`. `Drones` goes over `ids::<With<Drone>>()`, and finding
  a pilot's drones is `all::<Drone>()`. `combat` asks `has::<Drone>`. The snapshot's battery gauge
  reads the component.
- Behaviour, order and the wire are unchanged.

**Checks:** server 262 tests, all passing; clippy clean. `full_room_tick_cost`: `Vehicles` 29 to 30 µs,
`Drones` 0.77 to 0.80 µs, `Gunnery` 1.29 to 1.37 µs, `Combat` 35 to 36 µs. Mechanics in total 1.53 to
1.62 ms, which moves with `Movement` (1.49 to 1.55 ms, not touched here) from run to run. Snapshots 0.69
to 0.72 ms.

**Next:** take `Seats` and the wreck state (`hp`, `wreck_in`, `last_attacker`) out of `Vehicle`, and
the remaining per-kind branches in `vehicles.rs` (drop height, ram weight, legs) into spec data. Then
players and bots.

### 2026-10-22: what a vehicle's body does is data

The `Vehicles` mechanic told kinds apart with `matches!(kind, KIND_TANK | KIND_STRV)` and the like in
nine places. Each was a trait of the vehicle's body under another name. They are now one `Body` per
kind in `VehicleDef` (`crates/shared/src/vehicles.rs`):

| Field | Was | Kinds |
|---|---|---|
| `drop_y` | `drop_height`'s match, `CAR_DROP_Y`, `PLANE_DROP_Y` | legs a leg's length plus the hull, a plane 1.5, the rest 1.0 |
| `legs` | `KIND_WALKER \| KIND_HEN` in boarding and in standing back up | walker, hen |
| `heavy` | `KIND_TANK \| KIND_STRV` in `plow_trees`, and the sim's `heavy()`, which now reads it | tank, Strv |
| `ram` | `TANK_RAM_FACTOR` | tank, Strv 3 |
| `crash` | `TANK_CRASH_FACTOR`, `PLANE_CRASH_FACTOR` | tank, Strv 0.2; plane 2 |
| `flies` | `KIND_PLANE \| KIND_HELI` when a felled tree would slow it | plane, helicopter, drones |
| `towed` | `KIND_MORTAR` for the brake and for hitching | mortar |

- The bodies are named: `CAR`, `TRACKS`, `ROTOR`, `WINGS`, `LEGS` and `TOWED`. A new kind picks one or
  writes its own.
- What still names a kind in `vehicles.rs` is on purpose: which vehicles a room places (the mix, the
  planes' runways, `add_tank`).
- The sim still names kinds in two places: `CarControls::driven` (Roadster and hen buttons, and which
  kinds fly to the pilot's aim) and `extra_solids` (turret, casemate, wings, legs). Those are physics,
  and belong in its `Spec`.
- The constants are moved, not changed, so behaviour is the same. The parity fixture and client
  constants are untouched.

**Checks:** server 262, sim 111, shared 62, client 363 tests, all passing; clippy clean.
`full_room_tick_cost`: `Vehicles` 28.7 to 29.3 µs, unchanged.

**Next:** `Seats` and the wreck state out of `Vehicle`, or players and bots with events in place of the
`hits`, `kills` and `noises` vectors.

### 2026-10-22: a vehicle's health and its wreck are components

`Vehicle` lost `hp`, `wreck_in` and `last_attacker`. What they held now lives in components that are
not about vehicles:

- **`Health`**, the same component the roosters use. A vehicle is spawned with
  `Health::full(vehicle_hp(kind))`. `damage_vehicle` takes it down; the snapshot's `hp` reads it.
- **`HurtBy(id)`** (new, in `entities.rs`): whoever last hurt it, credited with its death. It is absent
  while nobody has, not an `Option` inside a struct. The wreck's blast and a drone's pop read it.
- **`Wreck { left }`** (new, beside `Vehicle`): inserted by `combat::wreck`, counted down by `Vehicles`.
  At zero, `vehicles::restore` puts the vehicle back whole and removes `Wreck` and `HurtBy` together.
- `Entities::dead(id)` (any entity with `Health` at zero) replaces `Vehicle::wrecked()` at its 14
  call sites. Where a site holds a `Mut<Vehicle>`, it asks before taking the borrow.

Behaviour is kept, including two details:

- A vehicle at zero health with no `Wreck` comes back the next tick. This happens when `wreck` found no
  body, and `wreck_in` did the same at 0.
- A vehicle that falls out of the world comes back whole but keeps its `HurtBy`, as it kept
  `last_attacker`.

Two tests now also check that a restored wreck carries no `Wreck` and no `HurtBy`.

`turret_hp`, `damage` and `scrape_in` stay in `Vehicle` for now. They are what armour and crashes do
to a hull, and they move when those do.

**Checks:** server 262 tests, all passing; clippy clean. `full_room_tick_cost`: `Vehicles` 29.4 to
31.4 µs over three runs, within the noise of the 28.7 to 29.3 µs before. The extra lookups are
`dead()` per vehicle, which go away with cached query states.

**Next:** `Seats` out of `Vehicle`, so that anything can be ridden, then players and bots.

### 2026-10-22: who rides a vehicle is a component

`Vehicle::seats`, a fixed `[Option<u32>; CAR_SEATS]` read through `seat_count(kind)`, is now
`Seats`, a component on the vehicle's entity (`game/state.rs`).

- It holds exactly as many seats as the kind has (`Seats::empty(vehicle_def(kind).seats)`), so nothing
  slices by `seat_count` any more. The snapshot sends all of them, the same list as before.
- It derefs to the slice for indexing. Code that does not know the count asks `rider(i)`, which is
  `None` past the last seat, as the fixed array was. Gunnery's seat lookup, the crew bots and ram
  credit do this. Other helpers: `riders`, `vacant`, `free`, `take`, `take_all`.
- `seat`, `vacate`, `switch_seat`, `remove_vehicle`, the wreck, the turret coming off, a drone's
  launch and cut, and the scenario's tank clearing all work on `Seats`. `Vehicle` is left with
  driving (`kind`, `controls`, `home`, `stuck_ticks`) and what armour and crashes do to the hull.
- One test checked a helicopter's first three seats, the third of which it never had. It now checks
  the two it has.
- Behaviour, order and the wire are unchanged.

**Checks:** server 262 tests, all passing; clippy clean. `full_room_tick_cost`: `Vehicles` 30.9 to
31.9 µs over two settled runs (a third ran slow across every mechanic), against 29.4 to 31.4 µs with one
fewer component. Each component taken out of `Vehicle` adds a lookup per vehicle in the passes that
read it. The total, about 2 µs over this step and the last, is far inside the 16.7 ms budget, but it
is the per-call cost the open "cache query states" item is for, and that comes next.

**Next:** cache query states and look-ups (a per-tick id list, `QueryState`s kept between ticks).
Then players and bots.

### 2026-10-22: query states are kept between calls

The last few steps each added a little to the tick. A throwaway release measurement on the committed
city (16 vehicles, 64 entities) showed where the time went: building the query, not running it.

| Call | Before | After |
|---|---|---|
| `ids::<With<Vehicle>>()` | 385 ns | 56 ns |
| `all::<Vehicle>()` | 384 ns | 59 ns |
| `count::<Vehicle>()` | 301 ns | 15 ns |
| `get::<Health>(id)` | 5 ns | 6 ns |

`try_query` made a fresh `QueryState` on every call, matching it against every archetype.
`Entities` now keeps one `QueryState` per query type, keyed by `TypeId`, in a `RefCell`. Each use
brings it up to date with the archetypes made since its last use (`update_archetypes`, nearly free when
none were made) and reads it with `iter_manual`.

- `ids`, `all` and `count` use the kept states. Their signatures are unchanged, apart from
  `ids::<F: 'static>`.
- `scan::<D, F, _>(|iter| ..)` and `scan_mut` are new: an unordered pass over a kept query, for what
  does not depend on order.
- `scan` takes its state out of the cache while the closure runs, so the closure may ask for other
  queries, including the same one.
- The three direct `try_query` and `query` calls now use them: `first_hurtbox_hit` (once per round
  and per blast), `projectiles::count`, and the pickup timer.
- The plan's "one id-ordered list per tick" turned out unnecessary: sorting a few dozen ids is a few
  nanoseconds, against the hundreds the query state cost.
- Two new tests: a kept query sees entities of archetypes made after it, in id order and after a
  despawn; a scan nested in a scan works, and `scan_mut` writes through.

**Checks:** server 264 tests, all passing; clippy clean. `full_room_tick_cost` over three runs, before
and after:

| Mechanic | Before | After |
|---|---|---|
| `Combat` | 34.7–36.6 µs | 29.0–30.0 µs |
| `Vehicles` | 30.9–31.9 µs | 27.6–28.3 µs (below the 26–29 µs it cost as a map) |
| `Gunnery` | 1.34–1.43 µs | 0.65–0.70 µs |
| `Drones` | 0.77–1.02 µs | 0.46–0.48 µs |
| `Pickups` | 0.58–0.73 µs | 0.09–0.10 µs |
| `Life` | 0.67–0.94 µs | 0.10–0.15 µs |
| `Npcs` | 0.67–0.76 µs | 0.19–0.30 µs |

The extra cost of the families that follow is now per entity, not per call.

**Next:** players and bots, the largest family left, with events in place of the `hits`, `kills` and
`noises` vectors.

### 2026-10-22: players are entities

`World::players`, a `BTreeMap<u32, Player>`, is gone. A player, connected or bot, is an entity with a
`Player` component, under the same id. Bots' minds (`World::bots`) are still a map, keyed by the same
ids. They move next, as a `Bot` component on the player's entity.

**How it was converted.** A bracket-matching script rewrote the 300-odd lookups, about two thirds of
them in tests:

| Map form | Entity form |
|---|---|
| `players[&id]` | `entities.of::<Player>(id)` |
| `players.get(&id)` | `entities.get::<Player>(id)` |
| `players.get_mut(&id)` | `entities.get_mut::<Player>(id)` |
| `players.contains_key(&id)` | `entities.has::<Player>(id)` |

The 25 passes over the whole map were rewritten by hand:

- Read-only passes use `all::<Player>()`.
- Passes that change players go over `ids::<With<Player>>()` with `of_mut`, in id order as before.
- Filters such as "who pressed use" use the new `ids_where`, which filters in one unordered pass and
  sorts only the ids that pass.

**Borrows.** A player and another component now live in the same store, so code that held one while
reading the other had to be split. Each split reads first and writes after:

- `use_item` reads its seat before taking the player.
- The grenade and flare caps are counted before the player is taken again.
- `seat` checks seats and player, then writes each.
- The car controls copy the driver's input first.
- The rider loop copies the riders out.
- `knock_players` takes the player again after the drone check.
- The pickup trade decides on the player's side, then writes the pickup.

**Performance.** The first measurement showed `Combat` 8 µs slower. Each shot's player hit test
built and sorted a list of 64 ids, and 64 players fire. Three changes fixed it:

- `first_body_hit` is one unordered `scan` over `(NetId, Player)`, with no per-player look-up.
  `ray_capsule` accepts a hit at the limit, so in id order the later of two equal hits won; the scan
  keeps that rule (higher id).
- The id index hashes with a Fibonacci multiply (`IdHasher`) in place of SipHash. Ids are sequential,
  and every look-up by id goes through the index.
- `ids_where`, above.

**Checks:** server 264, sim 111, shared 62, osm-import 81 and editor 2 tests, all passing; clippy
clean. Behaviour and the wire are unchanged. `full_room_tick_cost`, run back to back against the last
commit, two runs each:

| Mechanic | Before | After |
|---|---|---|
| `Combat` | 32.3–33.2 µs | 27.8–28.3 µs |
| `Vehicles` | 31.2–32.2 µs | 32.1–32.4 µs |
| `Gunnery` | 0.87–0.95 µs | 0.69–0.70 µs |
| `Movement` | 1.62–1.63 ms | 1.62–1.66 ms |
| Mechanics, avg | 1.685–1.701 ms | 1.687–1.718 ms |

The machine ran about 3% slower this session than in the entries above, so compare these numbers only
with each other.

**Next:** bots' minds as a `Bot` component on the player's entity. After that, a player's `Health`
and hit test can join the generic ones roosters use, giving one hit test and one damage path for
everything that can be shot.

### 2026-10-22: a bot's mind is a component on its player

`World::bots` is gone. A bot is a player entity that also has a `Bot` component:

- `add_bot` puts the component on the new player's entity.
- `Bots::on_leave` removes it; despawning the player would drop it anyway.
- "Is this a bot" is `has::<Bot>(id)`, and the bot count is `count::<Bot>()`.
- The same converter as for players rewrote the 60-odd lookups. It also rewrote one `Vec` field that
  happens to be called `bots` (`placed.bots[..]` in a `main.rs` test), which was put back by hand.

`Bots::on_tick` used to take each mind out of the map, let it think with the whole world in hand,
then put it back. It now swaps an empty `Bot::new` in its place (no allocation) and back. The only
read of another mind while thinking is a tank driver reading its gunner's, which is never the mind
being thought with, so the placeholder is never seen. Moving the component off the entity and back
would have moved every player row between archetypes twice a tick.

**Checks:** server 264 tests, all passing; clippy clean. `full_room_tick_cost` with `BOTS=32`, back to
back against the last commit, two runs each:

| Mechanic | Before | After |
|---|---|---|
| `Bots` | 65.1–67.7 µs | 57.5–62.2 µs (the map remove and insert per bot are gone) |
| `Combat` | 49.3–50.8 µs | 49.5–53.8 µs |
| `Vehicles` | 36.3–36.9 µs | 34.7–38.4 µs |
| Mechanics, avg | 2.20–2.21 ms | 2.08–2.09 ms (mostly `Movement` noise) |

**Next:** a player's health and hit test joined with the generic ones. Players are tested by
`first_body_hit` and roosters by `first_hurtbox_hit`, and blasts have two loops as well. One hit test
and one damage path for everything that can be shot is what makes a new NPC cheap.

### 2026-10-22: one place decides what a round hits

`combat::hitscan` (rifles fired instantly, aircraft guns) and `projectiles::step_round` (gun rounds in
flight) each had their own copy of the same 30 lines: a rooster nearer than a stickman, else the
stickman with headshot, else the vehicle with armour and wheels, else the building or tree. A new
target would have had to be added to both. They differed only in values, and both now end in
`combat.rs`:

- `strike(&World, &Flight, wall, range) -> Strike`: what the last stretch of the flight meets. It
  changes nothing, so a bullet is still despawned before it does damage, as it was.
- `land(&mut World, &Flight, &Strike, &Round)`: the damage.
- `Flight` carries what differed:
  - `shooter`, never hit by their own round;
  - `attacker`, credited, `None` for a bullet whose owner has left;
  - `back`, the ticks of lag compensation;
  - `flown`, for falloff.
- A bullet is passed as a `Round` with its vehicle and plane damage both its chip (damage / 4, at least
  1), which is what it did before.

New test: a hitscan round and a bullet in flight both hurt a rooster, and the shooter sees the hit.
No test covered either path into a creature.

**Found, not changed:** rockets, shells and grenades that strike (`step_striking`) test stickmen only,
so they fly through roosters and burst on whatever is behind. Making them strike creatures too would
change behaviour, so it waits for a decision. With `strike` it is one line.

**Checks:** server 265 tests, all passing; clippy clean. `full_room_tick_cost`: `Combat` 29.2 µs on a
settled run, within the noise of the last entry; the work per round is the same.

**Next:** the sim's per-kind controls and solids into its `Spec`. Then Phase 0: generating the
client's tables from Rust, which is most of what a new kind costs in files.

### 2026-10-22: the sim's per-kind controls and solids are its spec

The sim's `Spec` table (`crates/sim/src/vehicle/spec.rs`) said how each kind drives and holds itself
up. Six other places in the sim still named kinds:

| Where | What it asked | Now |
|---|---|---|
| `CarControls::driven` | Roadster? hen? plane or drone? | `spec.buttons` (`Drive`, `Stunt`, `Leap`) and `spec.aims` |
| `extra_solids` | tank turret, Strv casemate, plane wings, legs | `spec.solids` |
| `car_fits` | is it a plane, whose wings must be clear? | the solids with `Role::Wings` |
| `set_turret`, `armor_strike` | `std::ptr::eq(spec, &TANK)` | the solids with `Role::Turret` |
| `stream.rs` replay obstacles | `kind == KIND_TANK` | the same |

- A `Solid` is a shape (`Cylinder` or `Cuboid`), a place on the hull and a `Role`: `Part`, `Turret`
  (it can be shot off, and a strike on it is a turret hit) or `Wings` (kept clear when the vehicle is
  placed).
- A walker's and a hen's legs come from one `const fn legs(hy)`, with the arithmetic of the old loop.
- `Buttons::Drive` is jump for the handbrake, and jump/run for a helicopter's climb and sink.
  `Stunt` is the Roadster's: run boosts, jump hops, crouch brakes. `Leap` is the hen's: jump leaps,
  with no handbrake.
- The only kind names left in the sim are the `kind → spec` table itself.

A vehicle that moves like an existing one is now a row in each table: `VehicleDef` with its mounts
and body in `shared`, and a `Spec` with its buttons and solids in the sim. A new way of moving still
needs a new `Lift`.

**Checks:** sim 111, server 265, shared 62, osm-import 81 and editor 2 tests, all passing; client 363
tests against a freshly built WASM sim, all passing; workspace clippy clean. `full_room_tick_cost`:
`Vehicles` 29.5 µs, unchanged; the per-kind lookups were already behind `spec()`.

**Next:** Phase 0, generating the client's `items.ts` and `vehicles.ts` from the Rust tables. That
covers most of the files a new kind touches, and the client is the half of the migration not yet
started.
