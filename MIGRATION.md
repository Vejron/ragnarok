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
- [ ] Build it for `wasm32` and load it in the client; prove the pipeline on movement first.
- [ ] Move vehicle prediction to it, so the client's Rapier is the server's Rapier; delete
      `sim/vehicle.ts` and the other mirrors.

### Phase 2 (track B): server ECS, one family at a time

- [x] ECS foundation in `World`: the `bevy_ecs` world, the `NetId` index, sorted queries.
- [x] Generic life: `Health`, `Hurtbox`, `Respawn`, `Home`, `DropOnDeath`; one hit test, one damage
      path and one blast loop for ECS entities.
- [x] Roosters (the proof: 3 files).
- [x] Pickups.
- [ ] Cache query states, and keep one id-ordered list per tick, so the per-call setup cost seen in
      the measurements below does not grow with the families that follow.
- [ ] Projectiles (rockets, grenades, shells, bullets, flares) as `Projectile` + fuse + burst.
- [ ] Vehicles, with weapon mounts replacing the `work_*` functions (after Phase 1's `crates/sim`).
- [ ] Players and bots.
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
