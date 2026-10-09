# Ragnarok — technical design

Ragnarok is a continuously evolving multiplayer online game. It runs in the browser on any device, in a goofy, low-poly version of a real city built from OpenStreetMap. Players are stickmen who run, jump and eventually commandeer cars, tanks and helicopters together.

This document describes how the game is built and why. The wire format is specified in [NETWORKING.md](NETWORKING.md). Developer commands are in [README.md](README.md).

## 1. Pillars

- **Instant to join.** Open a URL to play, with no install or account. The game works on desktop now and on touch devices later.
- **Feels local.** Your own stickman reacts the same frame you press a key, even at 150 ms ping. Other players move smoothly, never in jerks.
- **Fair by construction.** The server owns the truth. Clients send intent (inputs), never results (positions or hits).
- **A real place, made silly.** OSM gives every street and building. The art style makes it a toy.
- **Grows by modules.** Vehicles, pickups and game modes are added as mechanics, not by rewriting the loop.

## 2. Architecture

```
 Browser (Vue 3 + three.js)                         Server (Rust, single process)
┌──────────────────────────────────┐   WebTransport   ┌──────────────────────────────────────┐
│ InputController ─► Predictor     │  (HTTP/3, QUIC)  │ session task (one per player)        │
│   60 Hz fixed tick   │ Rapier WASM│ ── datagrams ──► │   decode, rate-limit, netem           │
│                      ▼           │   inputs, ping   │        │ Command (mpsc)               │
│ Transport ◄── snapshots, pong ── │ ◄── datagrams ── │        ▼                              │
│   │       ◄── welcome, leave ─── │ ◄── uni streams ─│ game task: owns World, 60 Hz fixed    │
│   ▼                              │                  │   JitterBuffer ─► Mechanics ─► Rapier │
│ ServerClock + Interpolator       │                  │   every 2nd tick: Snapshot per player │
│   │                              │                  └──────────────────────────────────────┘
│   ▼                              │                         ▲
│ Scene: world, stickmen, camera   │       world.msgpack ────┘ (same file on both sides)
└──────────────────────────────────┘             ▲
                                         osm-import (CLI): Overpass API ─► world.msgpack
```

| Crate or directory | Role |
|---|---|
| `crates/shared` | Protocol types, quantization, movement rules, world-file format, constants. Shared by the server and the tools. |
| `crates/server` | Salvo HTTP/3 server: WebTransport sessions, the authoritative game loop, Rapier physics, mechanics. |
| `crates/osm-import` | CLI that turns an OSM bounding box into `world.msgpack`. |
| `client` | Vue 3 + Vite + three.js app. Handles prediction, interpolation and rendering. |

The TypeScript client mirrors `crates/shared` by hand. Byte-for-byte fixture tests on both sides (`protocol.rs` ↔ `codec.spec.ts`) and a physics fixture (`physics.rs` ↔ `predict.spec.ts`) fail when the two languages drift apart.

Transport, TLS and deployment follow [flax3d](https://github.com/Vejron/flax3d):
- In development, a self-signed certificate is pinned through `serverCertificateHashes`, and Vite proxies the hash.
- In production, an ACME certificate comes from certon, with the built client served by the same binary.

## 3. Authority model

The server is fully authoritative.

1. **Clients send inputs.** Each tick produces an `InputFrame`: sequence number, move axes, facing and buttons.
2. **The server simulates.** It applies exactly one input per player per tick in a single fixed-rate loop.
3. **The server publishes state.** Snapshots carry every entity's state. Each one also carries the receiving player's `ack`: the newest input already included.

Clients never report positions, hits or pickups. Mechanics such as shooting will use server-side lag compensation (§5.6) instead of trusting the client.

## 4. Tick model

| | Rate | Notes |
|---|---|---|
| Simulation | 60 Hz (`TICK_RATE`) | Fixed `dt`, the same on server and client prediction |
| Snapshots | 30 Hz (every `SNAPSHOT_EVERY` = 2 ticks) | One datagram per player |
| Client input | 60 Hz | Each datagram repeats the last 4 frames |
| Ping | 1 Hz | Uses a timer, so background tabs stay connected |

The server loop is a single tokio task. It owns the `World`, ticks on `tokio::time::interval` (missed ticks burst to catch up) and never shares state behind locks. Sessions talk to it only through channels.

## 5. Netcode

### 5.1 Input redundancy

Every input datagram carries the newest `INPUT_REDUNDANCY` (4) frames. A lost datagram costs nothing: the next one contains the missing frames. The server keeps only frames with a sequence number newer than any it has seen.

### 5.2 Server input jitter buffer

Inputs arrive unevenly, but the simulation consumes exactly one input per player per tick (`crates/server/src/jitter.rs`).

- **Priming:** consumption starts only after `TARGET` (2) frames are queued. This is about 33 ms of buffer.
- **Underrun:** the server repeats the last input with jump stripped, and doesn't acknowledge the repeat. After `STARVE_LIMIT` (15) starved ticks, the player stops.
- **Overflow:** beyond `MAX` (8) frames, the queue drops back to `TARGET`. This happens when the client's clock runs fast or after a burst.

Repeats and drops cause small mispredictions. Reconciliation corrects them without the player noticing (§5.3).

### 5.3 Client prediction and reconciliation

The local player is simulated on the client with the same rules and the same Rapier scene (`client/src/sim/predict.ts`).

1. Each tick, the client samples input, quantizes it exactly as it will be sent, applies it locally and stores it as *pending*.
2. On a snapshot, it drops pending inputs with `seq <= ack`, resets to the authoritative state and replays the remaining pending inputs.
3. The difference between the old and new predicted positions is not snapped. It's added to a visual error offset that decays exponentially (rate 15/s, about 100 ms). Corrections over 3 m are treated as teleports and snapped.
4. Rendering blends between the last two ticks for refresh rates above 60 Hz.

### 5.4 Remote interpolation

Remote entities are drawn in the past, at `renderTick = estServerTick − interpDelay` (`client/src/net/interp.ts`). This way there are almost always two snapshots to blend between.

- Snapshots are buffered per entity by server tick, inserted in sorted order (datagrams can reorder) and capped at 32.
- Position is interpolated linearly. Yaw takes the shortest arc. Past the newest snapshot, the entity holds its last state (no extrapolation yet).

### 5.5 Clock sync and adaptive delay

`ServerClock` (`client/src/net/clock.ts`) estimates the newest server tick from snapshot arrivals: `serverTick ≈ localMs·rate/1000 + offset`.

- **Asymmetric smoothing:** early arrivals pull the offset quickly (weight 0.05) and late ones slowly (0.01). The clock tracks the fastest path, not the average delay.
- **Jitter:** the smoothed absolute deviation of arrivals, in ticks.
- **Interpolation delay:** `2 × SNAPSHOT_EVERY + 2 × jitter` ticks, clamped to 3–30. On a clean link this is about 67 ms. It grows only while the network is jittery.
- **Resync:** a deviation over 60 ticks (server restart, sleeping tab) resets the clock.

### 5.6 Planned: lag compensation

When hitscan weapons land, the shooter's input will carry the render tick it saw. The server will rewind other players' hitboxes to that tick, capped at about 250 ms, to resolve the shot. This is the Source/Overwatch model, and it fits naturally because the server already keeps per-tick state.

### 5.7 Measured behaviour

Measured with two headless Chrome clients and 60 ms ±30 ms latency each way plus 5% loss on both legs (about 160 ms RTT):
- Local corrections peaked at 3.5 cm, with the 95th percentile at 1 cm.
- The remote player's apparent speed never exceeded 8.4 m/s against a run speed of 7.5 m/s, so there were no pops.

## 6. Transport mapping

WebTransport over HTTP/3 (QUIC):

| Channel | Used for | Why |
|---|---|---|
| Datagrams (unreliable, unordered) | Inputs, snapshots, ping/pong | Latest state wins; retransmitting stale state only adds latency |
| Server-opened unidirectional streams (reliable) | `welcome`, `leave`, and later vehicle seat changes and chat | Rare events that must arrive |

Datagrams are capped at 1200 bytes (`MAX_PACKET`), and a full 32-player snapshot fits. Larger worlds will need interest management (§14) before that limit becomes a problem.

## 7. Serialization and quantization

- Every message is a MessagePack **array** with a leading tag byte. Field names are never sent. Rust uses `rmp-serde` tuple structs and the client uses `@msgpack/msgpack`.
- Quantization:
  - Positions are `i32` centimetres.
  - Angles are `i16` milliradians wrapped to [-π, π).
  - Vertical velocity is `i16` cm/s.
  - Move axes are `i8` in −127..127.
- **The client predicts with the quantized input**, so the client and server feed the simulation identical values.
- A typical snapshot entity is about 20 bytes, and a typical input datagram about 30 bytes.
- `PROTOCOL_VERSION` is checked in `welcome` and in every input.

## 8. World pipeline (OpenStreetMap)

`cargo run -p osm-import -- --bbox south,west,north,east`. The default is a roughly 600 m gridded block of Södermalm, Stockholm.

1. **Fetch:** an Overpass query (`way["building"]`, `way["highway"]`, `out geom`), with a fallback mirror. `--synthetic` generates a grid city offline.
2. **Project:** equirectangular projection to local metres around the bbox centre (+X east, +Z south, +Y up).
3. **Buildings:**
   - Footprints with at least 4 m² of area, fully inside the playable square.
   - Height comes from `height`, then `building:levels × 3 + 1`, then a varied 6–12 m.
   - Each building is extruded into a closed prism (walls, earcut roof and floor) in one static collision trimesh.
4. **Roads:** highway ways are clipped to the square (Liang–Barsky) and classed as major (10 m), minor (4.5–7 m) or path (2.5–5 m).
5. **Spawns:** points sampled along roads that are outside every footprint and at least 1.5 m from walls. Up to 64 are picked, at least 15 m apart, sorted centre-first. The server picks among the 6 most central.
6. **Output:** `client/public/world.msgpack`, about 130 KiB for the default block. The server loads the same file. Its FNV-1a hash is sent in `welcome`, so a client with a stale world shows a warning.

Not handled yet:
- Multipolygon relations, such as courtyard buildings.
- Terrain elevation. The ground is flat.
- Water and parks.
- Streaming beyond one block.

## 9. Physics

Both sides use Rapier 3D: `rapier3d` 0.36 natively and `@dimforge/rapier3d-compat` 0.21 in WASM.

- **Static scene** (`static_colliders` / `buildStaticColliders`): a ground slab with its top at y = 0, four 50 m boundary walls, and the building trimesh.
- **Stickman:** a capsule (half-height 0.5 m, radius 0.35 m, 1.7 m tall) moved by `KinematicCharacterController`.
  - Controller settings: offset 2 cm, slide on, autostep 35 cm, 50° max slope.
  - Characters are shape-cast, not colliders, so players don't block each other. Prediction then never depends on where remote players really are.
- **Movement rules** (`movement.rs` / `movement.ts`):
  - Horizontal speed is set directly from input: walk 4 m/s, run 7.5 m/s.
  - Gravity is 20 m/s² and jump speed is 6.5 m/s.
  - The character is grounded only when not moving upward.
- **Two-pass move:** horizontal movement, then vertical, as separate controller calls. A combined pass makes the controller stall for a tick every few seconds on flat ground, in both native and WASM, and each stall is a misprediction. Snap-to-ground is off because it sinks the capsule slowly in the two-pass setup.
- **Native vs WASM:**
  - The two builds aren't bit-exact. The client rounds its movement math to f32 to match the server.
  - The cross-language fixture agrees to well under 1 mm per tick and within 1 cm over 90 ticks of wall-sliding and jumping.
  - Reconciliation only ever replays about one RTT of ticks, so long-horizon drift never shows.

Vehicles will be server-side dynamic rigid bodies (§10), so the physics pipeline will start stepping a dynamic world. The static scene and character code stay as they are.

## 10. Entities and vehicles (planned)

- **Entities:** an entity is an id, a kind (`KIND_STICKMAN`, later `KIND_CAR`, `KIND_TANK`, `KIND_HELI`) and a kind-specific state in the snapshot. The protocol's entity array is versioned and can grow per kind.
- **Seats:** each vehicle has a seat list. Seat 0 is the driver, and the others are passengers or gunners.
  - `BUTTON_USE` near a vehicle sends an enter request, and the server assigns a free seat.
  - Seat changes go out as reliable events.
- **Input routing:** while seated, a player's input frames drive their seat's role instead of the stickman. The driver steers and accelerates; a gunner aims the turret. The stickman entity is hidden and attached to the vehicle.
- **Prediction:** at first, vehicles are server-simulated and interpolated like remote players, and the driver sees a small input delay. Later, the driver predicts their own vehicle with the same Rapier vehicle controller (`DynamicRayCastVehicleController`) and reconciles like the stickman.
- **Planned vehicles:**
  - Car: ray-cast wheels.
  - Tank: tracks approximated by wheels, plus a turret seat.
  - Helicopter: a simplified flight model with lift, a cyclic and a tail rotor. Two seats.

## 11. Modular mechanics

Game rules are `Mechanic`s (`crates/server/src/mechanics/mod.rs`):

```rust
pub trait Mechanic: Send {
    fn on_join(&mut self, world: &mut World, id: u32) {}
    fn on_leave(&mut self, world: &mut World, id: u32) {}
    fn on_tick(&mut self, world: &mut World, dt: f32) {}
}
```

- The room runs an ordered list. The MVP has `Spawn` (spawn points and respawning below `KILL_Y`) and `Movement` (the character controller).
- New features are new mechanics, such as `Vehicles`, `Pickups` or `Deathmatch`. Each owns its own state and reads and changes `World`.
- When mechanics need to talk to clients, `World` will gain an event outbox. The game loop will turn those events into reliable messages, so mechanics never touch sessions.

## 12. Art direction

- **Goofy and readable.**
  - Flat toon shading.
  - Candy-pastel buildings.
  - Green grass, grey roads and sandy paths.
  - A light-blue sky with distance fog.
  - One shadow-casting sun that follows the player.
- **The stickman** is procedural: a black capsule stick body with a big coloured head (per player id) and two dot eyes that show facing.
  - Arms and legs swing with ground speed.
  - Airborne pose: arms up, legs tucked.
- **Camera:** third person, about 5 m behind, mouse-look with pointer lock.
- **Budget:** static geometry is merged by colour, so a block is about 15 draw calls. Each stickman is about 10 meshes, and instancing comes later.

## 13. Security

- **Origin checks:** in production only `https://<domain>` is accepted, and in development only the Vite origin.
- **Packet validation:** datagrams over 1200 bytes are dropped unread. Malformed packets, wrong versions and oversized input batches are dropped.
- **Rate limits:** inputs at most every 8 ms and pings at most every 100 ms per session.
- **Room limits:** 32 players per room. Sessions silent for 10 s are dropped, so closed tabs don't linger as ghosts.
- **Authority:** clients can only express intent. Speed, collision and spawning are server-side, so speed and teleport hacks aren't possible.

## 14. Roadmap

1. **MVP (done):** server-authoritative stickmen on an OSM block, with prediction, reconciliation, interpolation, adaptive delay, network emulation and cross-language fixtures.
2. **Vehicles I:** a car entity, seats, enter and exit, driver input routing, server-simulated and interpolated.
3. **Vehicles II:** driver-side prediction, then the tank (turret seat) and helicopter.
4. **Combat:** a hitscan weapon with server-side lag compensation, health, respawns and the first game mode.
5. **Scale:**
   - Delta-compressed snapshots (against the last acked snapshot).
   - Interest management by distance.
   - Instanced stickmen.
   - Larger worlds streamed in tiles.
6. **Reach:** touch controls (virtual stick and buttons), a quality scaler for low-end GPUs, and a Docker and deploy script ported from flax3d.
7. **World:** OSM multipolygons, terrain elevation, water, parks and trees.
