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
 Browser (Vue 3 + WebGPU)                         Server (Rust, single process)
┌──────────────────────────────────┐   WebTransport   ┌──────────────────────────────────────┐
│ InputController ─► Predictor     │  (HTTP/3, QUIC)  │ session task (one per player)        │
│   60 Hz fixed tick   │ Rapier WASM│ ── datagrams ──► │   decode, rate-limit, netem           │
│                      ▼           │   inputs, ping   │        │ Command (mpsc)               │
│ Transport ◄── snapshots, pong ── │ ◄── datagrams ── │        ▼                              │
│   │       ◄── welcome, leave ─── │ ◄── uni streams ─│ game task: owns World, 60 Hz fixed    │
│   ▼                              │                  │   JitterBuffer ─► Mechanics ─► Rapier │
│ ServerClock + EntityStore        │                  │   every 2nd tick: Snapshot per player │
│   │                              │                  └──────────────────────────────────────┘
│   ▼                              │                         ▲
│ Scene: tiles, stickmen, camera   │  world.msgpack + tiles/ ┘ (same files on both sides)
└──────────────────────────────────┘             ▲
                                         osm-import (CLI): Overpass API ─► world.msgpack, tiles/
```

| Crate or directory | Role |
|---|---|
| `crates/shared` | Protocol types, quantization, movement rules, world-file format, terrain sampling, constants. Shared by the server and the tools. |
| `crates/server` | Salvo HTTP/3 server: WebTransport sessions, the authoritative game loop, Rapier physics, mechanics, per-player replication (§5.8). |
| `crates/osm-import` | CLI that turns an OSM bounding box into `world.msgpack` and a grid of tile files. |
| `client` | Vue 3 + Vite app. Handles prediction and interpolation; draws with the WebGPU engine (`engine`). |

The client's prediction, of the stickman and of the vehicle it drives, runs the server's own physics: `crates/sim` (the Rapier world, the character controller and the vehicles), built to WASM through `crates/sim-wasm` and wrapped by `client/src/sim/motor.ts`. Carving too: the client carves buildings with Rust through stateless `carve_*` WASM functions. Everything else the client needs from Rust is generated (§2.1). The one copy kept by hand is `client/src/sim/terrain.ts`, 84 lines of height lookup called every frame from 38 places, where a call into WASM each time would cost more than the copy; a fixture recorded from Rust pins it. The physics checkpoints recorded from native (`crates/sim` ↔ `predict.spec.ts`, `vehicle.spec.ts`) fail when prediction and server drift apart.

### 2.1 How the code is organised

**Server: entities and components.** Players, bots, creatures, vehicles, projectiles and pickups are entities in a `bevy_ecs` world (`crates/server/src/entities.rs`), used on its own: no Bevy app and no scheduler, the ordered `Mechanic` list (§11) is the schedule. A thing is what its components say, not its kind: `Health` and `Hurtbox` make it shootable and blastable, `Respawn` and `Home` bring it back after it dies, `Trail` lets a shot be lag-compensated (§5.6), and `NetKind` only tells the client which model to draw. Behaviour is never picked by a kind id; vehicles carry their weapons (`shared::mounts`), body traits, armour and drone link as data. Static things (buildings, trees, terrain, carving) keep their own spatial structures. Query states are cached in `Entities`, so prefer `ids`, `ids_where`, `all`, `count` and `scan` to a fresh query.

**Server: one damage path, and events.** Anything with `Health` is hurt through `life::damage_entity`, found by `life::first_hurtbox_hit` (the one hit test, standing capsule, seated body or armour, rewound by `Trail`) and blasted by the one blast loop. Mechanics say what happened as typed events (`crates/server/src/events.rs`: `Damaged`, `Died`, `Noise`) rather than building wire messages: `Mechanic::react` answers them at the end of the tick (`Scores` counts kills, `Bots` turns a hurt bot towards the attacker) and `game::feedback` turns them into the `hit` and `kill` messages. A reaction that must be seen later in the same tick (a bandage spoiled, a dead rider out of the seat) is a hook (`events::Hooks`). A new reaction to a death touches nothing under `mechanics/combat*`.

**One definition per vehicle kind.** A kind is its Rust definition in `crates/shared/src/vehicles/<kind>.rs` (facts, crew, body, mounts, physics `Spec`, armour, drone), one presentation in `client/src/kinds/<kind>.ts` (model, seats, sound, effects) and, if it has one, a model. A new kind that moves like an existing one is five files: `KIND_*` in `consts.rs`, the definition and its place in `VEHICLES`, the presentation and its place in `KINDS`; a new model adds a sixth. The tables and constants reach the client generated. A kind that moves in a new way adds a lift model and its system in `crates/sim`. Optional: the world's spawn mix (`mechanics/vehicles.rs`), the editor's palette (`editor/placements.ts`), HUD hints in `App.vue` for a new seat role.

**On the wire.** Entities are replicated as components (§5.8, `crates/shared/src/replicated.rs`). Components marked `[owner]` (`Vitals`, `Hotbar`, `Drive`) go only to the entity's owner. A rider is a stickman entity with `Seated` and `Look`; the vehicle does not list its crew. Every other message is one entry of the `messages!` list in `protocol.rs` ([NETWORKING.md](NETWORKING.md)). `PROTOCOL_VERSION` is bumped whenever the wire changes.

**Generated for the client.** Rust is the source; the client's copies are written by Rust tests or a script, and parity specs fail when they are stale:

| Generated file | By | From |
|---|---|---|
| `client/src/net/messages.generated.ts`, `client/src/__tests__/messages.fixture.json` | `UPDATE_PARITY=1 cargo test -p shared` | `messages!` in `protocol.rs` |
| `client/src/net/components.generated.ts` | the same | `replicated!` in `replicated.rs` |
| `client/src/entities/units.generated.ts` | the same | the units and blending of the component schema |
| `client/src/sim/tables.generated.json` | the same | items, vehicles, trees (`items.rs`, `vehicles.rs`) |
| `client/src/sim/consts.generated.ts` | `npm run gen:consts` | `crates/shared/src/consts.rs` |

**Client: an entity store and presenters.** `client/src/entities/store.ts` holds the replicated components by id; a presenter per component (`entities/presenters/`: people, riders, seating, vehicles, projectiles, pickups, roosters, guns, jets, smoke, wear) owns its per-entity state and its `onAdd`/`onRemove`. `seats.ts` and `cameras.ts` pick a camera rig and the HUD from a kind's seat data. `game.ts` wires them; `net/codec.ts` is only framing.

**Cost.** A full room of 64 costs about 1.7 ms of the 16.7 ms tick (`cargo test --release -p server -- --ignored --nocapture full_room`), about 420 µs for its snapshots, about 810 bytes a player a snapshot. The client decodes a snapshot in about 5.7 µs. [SERVER-PERFORMANCE.md](SERVER-PERFORMANCE.md) has the method. Changes to the server should be measured against these.

### 2.2 Adding things

Rust is the source of truth, so start there; the client's copies are regenerated (`UPDATE_PARITY=1 cargo test -p shared`, and `npm run gen:consts` for constants) and the parity specs say what is stale. Tests stay green and clippy clean at each step; measure `full_room` if a tick path changes.

**A vehicle.** The five files of §2.1. Copy the nearest kind (`jeep.rs` for a wheeled one, `heli.rs`, `plane.rs`, `walker.rs`, `loiter.rs` for the others): the definition holds its health, seats, body traits, mounts (weapons, see below), physics `Spec`, armour and drone link. On the client copy the nearest `kinds/<kind>.ts` and its model. Run the shared tests with `UPDATE_PARITY=1` and check the sim, server and client suites.

**A weapon.** On foot: a row in `crates/shared/src/items.rs` (an `ITEM_*` id, an `ItemDef`, `ITEM_COUNT`): damage, cooldown, magazine, spread, falloff, projectile or hitscan, pellets, stack, weight in the pickup mix. The client's table is regenerated; add a hand model, sound or HUD icon if it needs one. Anything that needs a new way to fire is a new `ItemKind` handled in `mechanics/combat.rs` (`fire_gun`, `use_item`). On a vehicle: a mount row in `shared::mounts` on its definition. A projectile is an entity made of what it does (`Projectile`, `Velocity`, a fall, a fuse, a `Burst`; see `mechanics/projectiles.rs`), so a new one is a new combination, plus a `PROJECTILE_*` id and, if it looks new, a branch in `entities/presenters/projectiles.ts`.

**An NPC.** A bundle of components (see `rooster()` in `mechanics/npcs.rs`): `NetKind`, `Transform`, `Health`, `Hurtbox`, `Home`, `Respawn`, `DropOnDeath`, `Trail`, plus a mind component of its own (`Wander` for the rooster; a bot is a player with `Bot`). Being shot, dying, dropping an item and coming back are then `Life`'s and need no code. Write a mechanic with `on_start` to place it and `on_tick` to move it, and add it to `mechanics::default_mechanics`. Give it a `KIND_*` id in `consts.rs`, a presenter in `client/src/entities/presenters/` (`wants` says which components it draws; copy `roosters.ts`) and register it in `game.ts`. If it sends something new to the client, that is a component (below).

**A rule or reaction.** A mechanic that answers `Damaged`, `Died` or `Noise` in `react` (§2.1). Mechanics that count or score are like `Scores`; nothing in combat changes.

**A replicated component.** An entry in `replicated!` in `crates/shared/src/replicated.rs` with an id, a name, whether it is `diff` or `set`, `[owner]` if only its owner should be told, and fields with their units. Fill it where the server builds the entity's state (`game/replicate.rs`), regenerate, and read it in a presenter. Bump `PROTOCOL_VERSION`.

**A message.** An entry in `messages!` in `protocol.rs` with its tags, fields and units. The client decoder or writer and the fixture are generated; add the handling in `net/` or `game.ts`, update [NETWORKING.md](NETWORKING.md) and bump `PROTOCOL_VERSION`.

Transport, TLS and deployment follow [flax3d](https://github.com/Vejron/flax3d):
- In development, a self-signed certificate is pinned through `serverCertificateHashes`, and Vite proxies the hash.
- In production, an ACME certificate comes from certon, with the built client served by the same binary.

## 3. Authority model

The server is fully authoritative.

1. **Clients send inputs.** Each tick produces an `InputFrame`: sequence number, move axes, facing and buttons.
2. **The server simulates.** It applies exactly one input per player per tick in a single fixed-rate loop.
3. **The server publishes state.** Snapshots carry the state of the entities near the receiving player, as a change from the last snapshot they acknowledged (§5.8). Each one also carries the receiving player's `ack`: the newest input already included.

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

Remote entities are drawn in the past, at `renderTick = estServerTick − interpDelay`. This way there are almost always two snapshots to blend between.

- **The store.** `client/src/entities/store.ts` buffers every replicated entity's components per server tick, inserted in sorted order (datagrams can reorder) and capped at 32.
- **Blending.** Each field blends as its unit says (`UNITS` in `crates/shared/src/replicated.rs`):
  - positions and steering are interpolated linearly;
  - angles take the shortest arc, and rotations are normalised lerps;
  - counters, flags and gauges come from the nearer snapshot.

  Rust writes the conversion into game units and the blend as code (`entities/units.generated.ts`).
- **Past the newest snapshot** the entity holds its last state; there is no extrapolation yet.
- **Ending.** An entity the newest snapshot no longer has ends at that tick, and goes when the picture gets there.
- **Presenters** (`entities/presenters/`) draw what the store holds, each chosen by the components an entity has: projectiles, items, stickmen, riders in their seats, the rooster, vehicles, and one per vehicle component (guns, wear, smoke, jets).
- **Kinds.** How the client presents each vehicle kind is its file in `client/src/kinds/`: its model, its seats (camera rig, HUD role, prompt), its sound and what it leaves on the ground. What a kind is in the game is its Rust definition, one file per kind in `crates/shared/src/vehicles/`, which reaches the client as `vehicleDef`.
- **Around them**, `client/src/game.ts` keeps the loop and wires the rest together:
  - `events.ts` plays shots, blasts and bullets when the picture reaches them;
  - `seats.ts` holds the types of a seat's presentation, and `cameras.ts` holds the camera rigs;
  - `surroundings.ts` does splashes, wading, grass and contact shadows;
  - `prompt.ts`, `gauges.ts` and `arcs.ts` hold the E prompt, a plane's gauges and sights, and the arcs drawn ahead of a throw or a bomb.

### 5.5 Clock sync and adaptive delay

`ServerClock` (`client/src/net/clock.ts`) estimates the newest server tick from snapshot arrivals: `serverTick ≈ localMs·rate/1000 + offset`.

- **Asymmetric smoothing:** early arrivals pull the offset quickly (weight 0.05) and late ones slowly (0.01). The clock tracks the fastest path, not the average delay.
- **Jitter:** the smoothed absolute deviation of arrivals, in ticks.
- **Interpolation delay:** `2 × SNAPSHOT_EVERY + 2 × jitter` ticks, clamped to 3–30. On a clean link this is about 67 ms. It grows only while the network is jittery.
- **Resync:** a deviation over 60 ticks (server restart, sleeping tab) resets the clock.

### 5.6 Combat and lag compensation

Health is the `Health` component (a player's `Vitals` mirrors it to its owner), and `life::damage_entity` is the only place it goes down (§2.1). At zero a player dies: the killer is credited by `Scores` from the `Died` event, a `kill` goes out on a reliable stream, and the body stays where it fell (flag `dead`) until `RESPAWN_TICKS` have passed. Then `Spawn::place` puts it at a spawn point with full health. Falling below `KILL_Y` is a death with no killer. Dead players cannot fire, board a vehicle, be hit or be knocked.

**The hotbar.** Every player has five slots (the `Hotbar` component, `[item, count, ammo]`) and starts each life with the AK5 and sniper (full magazines), a flare gun with three flares, an FPV drone and a loitering drone (`game::starting_hotbar`). The client says which is in hand in the `slot` field of every input; `BUTTON_FIRE` then does whatever that item does, while standing. Items are defined once, in `shared::items` (the client's table is generated from it): a damage, a cooldown, a magazine and so on, so a new weapon is a row in a table. Dying drops the lot; the next life starts from the standard loadout again. Changing slot drops a reload or a bandage in progress.

**Items.** Seated players use none of them: a gunner's trigger belongs to the gun.

- **AK5**: automatic, from the eye along yaw and pitch with a small random spread, 12 damage, 10 shots a second, 30 rounds, 2.5 s reload. Each round is a real bullet, see **Bullets** below. A vehicle it meets takes a quarter of the damage, so it is a nuisance and not a way to kill one.
- **Revolver**: the same bullets, semi-automatic (each shot is a fresh press of fire), 35 damage, 2 shots a second, 6 rounds, 2 s reload, tiny spread.
- **Bullets**: the AK5, revolver and sniper fire `PROJECTILE_BULLET`s: 280 m/s, falling under a quarter of gravity (about 1 m over the AK5's 200 m), stepped once a tick as a ray over the segment flown against statics, vehicles, roosters and standing players (`mechanics/projectiles.rs`), so they cannot tunnel. They take time to arrive: the shooter leads a moving target. Lag compensation is kept by shifting the clock: a bullet carries the `back` ticks the shooter's picture was behind, and each step meets stickmen where they stood `back` ticks before that step, so a shot that was on target on the shooter's screen at fire time, and a target that kept still, hits. A gun's damage, falloff and weapon id come from the gun it left (`Rocket::item`); the distance a round has flown (`Rocket::flown`) is what falloff reads. **Bullets are not replicated as projectiles**: a flight is deterministic (`p + v*n*dt - (0, g*dt^2*n(n+1)/2, 0)`), so the server announces each once as a `BulletSpawn` (id, shooter, gun, origin, velocity, tick; in two snapshots, in case one is lost, nearest first, at most `MAX_BULLET_SPAWNS`, to clients whose view the line crosses) and the clients fly it themselves (`client/src/sim/ballistics.ts`, tested against the server's stepping). Where it ended is a `WEAPON_BULLET` shot event whose `shooter` is the bullet's id; one that hits nothing just runs out its range on both sides. Listing every live bullet every snapshot cost 45 to 75 ms a snapshot round for a room at full fire (SERVER-PERFORMANCE.md).
- **Falloff**: every round does less the further it flew before it hit a stickman or a rooster (`shared::items::Falloff`: full damage up to `start`, a straight line down to `min` of it at `end`, never below 1). Revolver 30 m to 150 m down to 60%, AK5 25 m to 200 m down to 50%. The sniper's round and vehicle guns have none. The distance is how far the bullet or ray went from the eye.
- **Shotgun**: semi-automatic hitscan, ten pellets per pull, each its own ray with its own scatter (0.06 rad from the hip) through the same rewound world, 12 damage a pellet (headshot pellets double), 1 s between shots, 5 rounds, 3 s reload (the whole magazine at once), 40 m range. Falloff from 8 m down to 25% at 40 m: all ten pellets at point blank are 120, which kills; across a street it is a nuisance. Every pellet is announced as a shot event where it ended (the first as `WEAPON_SHOTGUN`, the rest as `WEAPON_PELLET`), so one pull takes ten of the snapshot's `MAX_SHOT_EVENTS` (24). The client draws no rays for it: a muzzle blast, and a spark and dust puff where each pellet landed. Each pellet that lands is its own `Hit`.
- **Aiming**: holding aim (right mouse, `BUTTON_ALT`) with any gun eases the field of view from 75° to 50°, pulls the over-the-shoulder camera 30% closer (the pivot, and so the server's crosshair ray, is unchanged), slows mouse look to 0.6 and tightens the reticle. The server scales the gun's spread by `AIM_SPREAD_SCALE` (0.25). Aim also slows walking by a per-gun multiplier (`aim_speed`: revolver 0.8, AK5 0.6, shotgun 0.7, sniper 0.4), applied to walk, run and crouch alike through `aim_speed_scale`; the server reads the held item from the input's slot and the client prediction does the same from its hotbar, so they stay in step. The revolver has a 0.015 rad hip spread so this matters for it too. The sniper goes further, see below.
- **Sniper**: semi-automatic, fires a bullet (see **Bullets**: 280 m/s, twice a tank shell, under a quarter of gravity) that hurts whoever it meets when it arrives (a vehicle takes a quarter) and does not burst; 90 damage, one shot every 2 s, 5 rounds, 3 s reload, 400 m range, rare. Wild from the hip (0.05 rad spread). Holding aim (right mouse, `BUTTON_ALT`) looks down the scope: the client goes first person at the eye with an 18° field of view, hides the player model, shows a scope overlay and slows mouse look. The server sees the same bit and, for a gun with `scope`, fires from the eye straight along the look direction with no spread, which is exactly the centre of that view.
- **Grenade**: thrown on a press, a little above the look direction, at 16 m/s. It is a projectile entity of kind grenade: it falls under gravity, bounces off whatever it meets (`Physics::cast_ray_normal`, keeping 40% of its speed) and bursts in a 5 m blast when its 3 s fuse runs out, whatever it is touching. Stacks of 3. While one is in hand the client draws its path (`sim/grenade.ts` steps it exactly like the server, against the same static colliders and cars, and `render/trajectory.ts` draws the dots, over everything, and a ring at the first landing), so aiming higher visibly throws further.
- **Bandage**: a press starts a 2 s application (only below full health); at the end it restores 40 and is used up. Taking damage or changing slot ruins it. Stacks of 5.
- **Flare gun**: a press fires a flare (`PROJECTILE_FLARE`) from the eye at 55 m/s, 0.6 rad above the look (`FLARE_LOFT`: the on-foot look only reaches 0.6 up, so a level look sends it up at about 34° and a full look up at about 69°, some 65 m high). It climbs under gravity; once it starts to fall its parachute opens, the sink is held to 2.5 m/s and its sideways drift dies away. It burns out 20 s after firing, which for a steep shot is still in the air; one fired low stops on whatever it meets and burns there. It hurts nobody. The client draws a white-hot core under a swaying canopy, a blooming halo and dripping sparks, and holds a steady flickering point light on it (`PointLights::hold`, 120 m reach) that lights the ground below, which matters most at night. At most `MAX_FLARES` (8) burn at once. Stacks of 3; the gun is the stack, and goes with the last flare. In hand it borrows the revolver's model and pose.
- **FPV drone**: launched on a press and flown by its owner in place of their body, which kneels where it stood and can still be shot. It flies where its pilot looks, and goes off in a 4 m blast on fire, on a hard hit or next to someone. E recalls it. A flat battery, a hurt pilot or 250 m of range drops it, still armed. One hit pops it. It is a short-lived vehicle held through a remote seat: see [DRONES.md](DRONES.md).
- **Rocket launcher**: a real projectile, stepped each tick at 45 m/s. Each step is a ray over the segment it flew, against the world, vehicles and players, so it cannot tunnel. It bursts on the first hit or after four seconds, in a 5 m blast. The machinery is kept but no item launches one yet.
- **Tank shell**: a projectile (`PROJECTILE_SHELL`), 140 m/s from the muzzle, falling under gravity, bursting on the first wall, vehicle or stickman or after 6 s, with the same blast rules.

**Pickups** (`mechanics/pickups.rs`, pickup entities). `PICKUP_COUNT` items are scattered once at start around the map's spawn points, on open ground (a ray from above must meet the terrain, not a roof), weighted by `ItemDef::weight`. Pressing use within 2 m takes the nearest: it joins a stack of the same item if there is room, else the first empty slot, else swaps with the slot in hand and leaves that on the ground. A taken item comes back 30 s later. The mechanic runs before `Vehicles` and consumes the use bit, so reaching for a gun beside a car does not board it.

**Rooster** (`mechanics/npcs.rs`, rooster entities). A Hedemora rooster, the first NPC. It lives at `ROOSTER_HOME` (a latitude/longitude, projected with the world's origin; the first spawn if that is outside the world), and cycles standing about, walking to a random spot within `ROOSTER_WANDER_RADIUS`, and now and then crowing (`FLAG_CROWING` in its entity `flags`, which the client turns into the `crow` clip and a synthesised "kokoo!"). It is hit by shots, sniper rounds and blasts like a player: it has a `Hurtbox` and `Health`, so the one hit test and damage path (§2.1) serve it. At zero health it leaves a golden egg (`ITEM_GOLDEN_EGG`, a `Pickup` with `dropped` set, gone once taken) and is back at home after `ROOSTER_RESPAWN_TICKS`. The egg is a heal item with `damage: 255`: used from the hotbar it channels like a bandage and restores full health. Its model is built by `tools/npc/build_rooster.py`.

**Bots** (`mechanics/bots.rs`, players with a `Bot` component). Players with no connection, on a team (`Player::team`; people are `TEAM_NONE`, all against all, and a team never hurts itself). `Bots` runs first each tick and writes their `InputFrame`, so everything else treats them as people. They patrol a route, find their way round buildings over a lazily built 1 m walking grid (`navigation.rs`, A*), spot enemies by sight, go to look at gunshots, blasts and hits they hear (`Noise` events, `Bot::hit_from`), pursue and shoot (picking a gun for the distance, leading the target, with reaction time and aim error set by `Bot::skill`), search and return. A bot's shot leaves its eye along its look, not the camera's crosshair. Two can crew a tank (`mechanics/bots/crew.rs`, `Bot::crew`): the driver drives along the roads or a route over the real tank physics, and the gunner lays the gun for the shell's fall. Placed in `area.json` or added with `RAGNAROK_BOTS` (and crews with `RAGNAROK_TANKS`). A world's `scenario.json` (`scenario.rs`, `Game::scenario`) brings on squads with triggers, sets objectives and ends in a win or a loss, told to clients with `notice` messages. `profiles` tell clients each player's team and whether they are a bot, so the scoreboard is split by side and the minimap marks gunfire: see [BOTS.md](BOTS.md).

**Blasts** (`combat::blast`) throw standing players away and hurt them, and shove and hurt vehicles, all falling off linearly to the radius. The shell does 125 at its centre, which kills outright within about 1.4 m; the rocket does 110, which kills within about half a metre of a direct hit.

**Vehicles** have health (car, Roadster and helicopter 100, tank 250, plane 70). At zero it is wrecked: everyone inside is thrown out and killed (credited to whoever last hurt it), it goes up in a blast that can take the next one with it, nobody can board it, and after `WRECK_TICKS` it is put back at its home with full health. Seated players take no direct damage: they die with the vehicle.

**Lag compensation.** Every input frame carries `view`, the server tick its picture showed (modulo 65536), which the client takes from its render tick. The server records where each body was at the end of the last 16 ticks (the `Trail` component). A rifle shot tests players where they stood at `view`, at most 15 ticks (250 ms) back and never forwards, so a shooter hits what they saw. Walls and vehicles are tested in the present, and rockets are simulated forward on the server, so neither is rewound. This is the Source/Overwatch model. A shooter with a lot of lag can still be killed round a corner by the target's own view of events, which is the usual trade.

**Events.** The server sends cosmetic events (tracers, bursts) in the snapshot's `shots` list. The client queues each until its render tick reaches the snapshot's tick, so a tracer leaves the shooter where the shooter is drawn.

### 5.7 Measured behaviour

Measured with two headless Chrome clients and 60 ms ±30 ms latency each way plus 5% loss on both legs (about 160 ms RTT):
- Local corrections peaked at 3.5 cm, with the 95th percentile at 1 cm.
- The remote player's apparent speed never exceeded 8.4 m/s against a run speed of 7.5 m/s, so there were no pops.

### 5.8 Interest management and delta snapshots

A room of 64 can't be sent to everyone whole 30 times a second, and most of it isn't in view anyway. Each player's snapshot is built from two filters (`crates/server/src/replication.rs`).

**Interest.** Only what is within `INTEREST_RADIUS` (300 m, on the ground plane) of the receiving player's stickman is sent: stickmen, creatures, vehicles, projectiles, the nearest items on the ground, and effects. Whatever the player already holds stays until it is `INTEREST_HYSTERESIS` (40 m) further out, so something on the edge doesn't flicker. 300 m is more than a rifle (200 m) or a rocket (180 m) reaches, and the fog hides most of the rest. Out of range is not a `leave`: the server stops listing the thing, and the client drops it. Scores, kills and `leave` still go to everyone.

**Components.** What is sent of each thing is the components it has (`crates/shared/src/replicated.rs`): `place`, `heading`, `turret`, `seated`, `damage` and so on, each with a bit of its own. A new kind of thing, or a new capability on an old one, is a new combination of components, or one new component in that one list. The client's types and decoder are generated from it, so nothing on the wire is written twice.

**Deltas.** The server keeps, per player, the scenes it sent in the last `BASELINE_WINDOW` (32) snapshots. Every input datagram carries `baseline`, the tick of the newest snapshot the client has rebuilt. The next snapshot lists only the difference from that scene, one update per entity:

- new entities, with every component they have;
- changed ones, with only the components that changed; position, heading, orientation and the like as differences (cm, mrad), the rest whole; and the components they lost;
- nothing at all for something that did not change;
- ids that are gone or out of range.

The client keeps its last 64 rebuilt scenes (`client/src/net/replica.ts`), applies the updates to the one the snapshot names and acknowledges the result. The server stores the scene the client will hold, with the same integer arithmetic, so both sides agree without an echo.

Loss needs no extra machinery. A snapshot that is lost is never acknowledged, so the server keeps diffing against the older one and the next delta is just a little bigger. If the acknowledged snapshot has left the window (a stall of more than a second) the server sends a whole one, with `baseline` 0.

**The datagram budget.** A snapshot still has to fit `MAX_PACKET`. If it doesn't, the server sheds in this order: effects, projectiles, items on the ground, bullets, then the farthest updates of everything else, and a thing that was shed arrives in a later snapshot, nearest first. Removals are never shed. A crowd of 64 in view therefore degrades to "the nearest N are up to date" and not to a dropped packet.

**What it buys.** Thirty players running flat out take about 230 bytes a snapshot as deltas, against 645 whole (`a_delta_is_a_good_deal_smaller_than_a_whole_snapshot`); protocol v1, a row per family, took 354. Standing players cost nothing once acknowledged, and a parked vehicle likewise; a moving tank sends its pose and nothing of its unchanged turret, seats or health. The bigger win is the interest filter: bytes now depend on how many players are near, not how many are connected.

## 6. Transport mapping

WebTransport over HTTP/3 (QUIC):

| Channel | Used for | Why |
|---|---|---|
| Datagrams (unreliable, unordered) | Inputs, snapshots, ping/pong | Latest state wins; retransmitting stale state only adds latency |
| Server-opened unidirectional streams (reliable) | `welcome`, `leave`, and later vehicle seat changes and chat | Rare events that must arrive |

Datagrams are capped at 1200 bytes (`MAX_PACKET`). Interest management and delta compression (§5.8) keep a snapshot well inside that, and anything that still doesn't fit is trimmed farthest-first, so the cap holds for any room size.

## 7. Serialization and quantization

- Every message is a MessagePack **array** with a leading tag byte. Field names are never sent. Rust uses `rmp-serde` tuple structs and the client uses `@msgpack/msgpack`.
- Quantization:
  - Positions are `i32` centimetres.
  - Angles are `i16` milliradians wrapped to [-π, π).
  - Vertical velocity is `i16` cm/s.
  - Move axes are `i8` in −127..127.
- **The client predicts with the quantized input**, so the client and server feed the simulation identical values.
- A whole snapshot entity is about 20 bytes, and a delta for a running one about 10. A typical input datagram is about 35 bytes.
- Deltas are differences of the same integers, so applying them is exact on both sides (§5.8).
- `PROTOCOL_VERSION` is checked in `welcome` and in every input.

## 8. World pipeline (OpenStreetMap)

`cargo run -p osm-import`. The default is an 8.6 km square of central Umeå, Backen to Nydala (63.8225, 20.235, half-side 4300 m); `scripts/world-dev.sh` makes a ~7.6 km² dev world around Mariehem instead. `--center lat,lon --half metres` picks another place, `--bbox south,west,north,east` an exact box, `--tile` the tile size (250 m), and `--synthetic` generates a grid city offline.

**The full pipeline is documented in [WORLD.md](WORLD.md)**: every data source and licence (OpenStreetMap, Lantmäteriet's height model and laser data, Umeå's tree register), where the credentials go (`.env.local`), all flags, and the download cache (`.cache/osm-import/`, git-ignored; `--refresh`, `--clear-cache`). This section covers what the output means.

1. **Fetch:** an Overpass query for buildings, highways and ground cover (`way` and multipolygon `relation` with `landuse`, `natural` or `leisure`; `out geom`; water is not among them: it comes from Lantmäteriet's Hydrografi, see [WORLD.md](WORLD.md)), with a fallback mirror and a few retry rounds, since the public servers shed load.
2. **Project:** equirectangular projection to local metres around the bbox centre (+X east, +Z south, +Y up).
3. **Buildings:**
   - Footprints with at least 4 m² of area, fully inside the playable square.
   - Height comes from `height`, then `building:levels × 3 + 1`, then a varied 6–12 m.
   - Each building becomes walls, a roof and a floor in the collision mesh (`crates/osm-import/src/roof.rs`). The floor is at the lowest ground under the footprint (`base`). Without laser data the roof is flat at the tagged height; with it, see *Roofs* below.
4. **Roads:** highway ways are clipped to the square (Liang–Barsky) and classed as major (10 m), minor (4.5–7 m) or path (2.5–5 m). They stay 2D polylines. The client paints them into the terrain texture (below), and the server uses them to line parked vehicles up with the street.
   - **Cover:** closed ways and multipolygon relations are classed as water, wood, park (grass, meadow, parks, pitches), farm, sand, urban or industrial. Relation members are joined end to end into rings, inner rings being holes (filled even-odd). Open pieces that never close are dropped. They are not simulated. Water is separate (`Tile.water`, `Tile.streams`, from Hydrografi): the surface height per ground sample and stream centre lines; lakes and rivers are carved into the ground, so the physics sees their beds.
5. **Spawns:** points sampled along roads that are outside every footprint and at least 1.5 m from walls. Up to 64 are picked, at least 15 m apart, sorted centre-first. The server picks among the 6 most central. They are 2D, and the ground gives their height.
6. **Tiles:** the square is cut into a grid of `tile_size` squares.
   - A building goes whole into the tile its centre is in, with its collision triangles, so a tile never needs its neighbour.
   - A road is cut at tile borders, and each tile keeps its own pieces. Cover rings are clipped to the tile (Sutherland–Hodgman) and sorted so ground uses are painted first, green next and water last.
   - Coordinates stay world-absolute, so a tile is drawn and collided with where it is, with no offset.
   - Every tile is written, even an empty one, since each carries its piece of the ground.
7. **Output:** `client/public/world.msgpack` (the *manifest*) and `client/public/tiles/{col}_{row}.msgpack`. The manifest holds the origin, extent, tile size, the ground sample spacing, the spawns, and each tile with the FNV-1a hash of its file. The default area is about 400 KiB of tiles in 61 files, plus the ground when terrain is on.

The manifest's FNV-1a hash is sent in `welcome`. Since it lists each tile's hash it vouches for every tile, so a client or server with a stale file shows a warning (client) or refuses to start (server).

### Streaming

- **Server:** loads every tile at start and merges them into one collision mesh (`WorldFile::assemble`). The simulation needs all of it, and the mesh is small.
- **Client** (`client/src/stream.ts`): `TileStreamer` fetches the tiles whose nearest edge is within 450 m of the camera focus, nearest first, at most four at a time, and drops those beyond 550 m. Each tile arrives as a merged-by-colour mesh group and as a trimesh collider in both Rapier worlds (the stickman's and the driven vehicle's). A tile that fails is retried after 5 s. The game waits for the tiles around the spawn before it starts.
- The ground slab, boundary walls and grass plane belong to the whole world, not to a tile. A tile that is late means walking through a building for a moment, which the server then corrects, and never falling out of the world.

### Terrain

The ground is a grid of heights, from Lantmäteriet's *Markhöjdmodell* (1 m DTM, CC BY 4.0, `stac-hojd/v1/collections/dtm-cog`; item `708_75` covers Backen). `crates/osm-import/src/dem.rs` does the reading:

- **Source:** a local GeoTIFF (`--dem`), or the STAC item for the bounding box fetched with HTTP range requests and Basic credentials from `LM_USER` / `LM_PASS`. The files are cloud-optimised GeoTIFFs in SWEREF 99 TM, so only the blocks under the world are transferred.
- **Frame:** each sample point is turned from local metres into latitude and longitude with the same equirectangular projection OSM uses, then into SWEREF 99 TM (Krüger series, no `proj`), so buildings and ground line up.
- **Resampling:** the mean of the 1 m cells within each sample's `--cell` square (5 m by default, must divide the tile size). Heights are `i16` centimetres above the lowest sample in the world, so the lowest ground is y = 0 and `KILL_Y` still means what it did. Gaps take the lowest ground. A relief over 100 m is refused, since the boundary walls (200 m) and the helicopter ceiling (120 m) are at fixed heights.
- **Format:** each tile holds `tile_size / cell + 1` samples a side, the last row and column shared with its neighbour, so a tile stands alone. `WorldFile::assemble` joins them into one `Ground` for the server. `Ground::height_at` (mirrored by `heightIn` in `client/src/sim/terrain.ts`) interpolates on the same triangles as Rapier's heightfield, so a spawn, a camera or a building base agrees with what the physics will collide with.
- **Detail:** `--detail <m>` (off by default; the dev world uses 1) bakes a slope map into each tile, `Tile::detail`: `(dh/dx, dh/dz)` as `i8` pairs every `--detail` metres, from the 1 m height model on top of the ground's own triangles (`crates/osm-import/src/detail.rs`; water beds stay as carved). It is lighting only: the client turns it into an object-space normal map on the terrain material (`bakeNormalMap`, `render/cover.ts`), so the ground can be sampled every 4 m for physics and mesh and still shade ditches and banks. A tile with no relief finer than its ground carries none.
- **Texture:** each tile's terrain mesh has its own 1024 × 1024 canvas texture (4 texels per metre), painted when the tile arrives (`client/src/render/cover.ts`): grass, then the cover patches. UVs run 0..1 over the tile with `flipY` off, so canvas `(0, 0)` is the tile's `(x0, z0)` corner. The texture and its material are freed with the tile. A world without terrain has no terrain mesh, so it shows the plain grass plane.
- **Shading:** the terrain uses a toon material with an 8-step ramp and smooth normals computed across tile seams from the neighbour's heights (a tile re-shades its loaded neighbours' edges when it arrives). Each vertex also carries a baked brightness (`render/shade.ts`): hillshade from a low light, darker on steep ground and in hollows, plus a faint low-frequency wobble.
- **Roads:** ribbons laid on the terrain (`render/roads.ts`), not painted, so edges stay sharp at any distance: one mesh per road kind, casings below fills, round discs at joints, and a per-layer polygon offset so overlapping roads settle in a fixed order.
- **Spawning:** players and vehicles are placed at the ground height under their spawn point. Buildings sit at the lowest ground under their footprint.

### Roofs

Lantmäteriet's *Laserdata Skog* (`stac-hojd/v1/collections/dsm-skoglig-copc`, COPC, about 2 returns/m², class 1 unclassified and class 2 ground; buildings are not classified) gives each footprint a roof. `lidar.rs` reads it over range requests with a small COPC reader of its own (the public crates mis-walk this file's hierarchy; only the octree nodes touching the world are downloaded, about 17 M returns for Backen). Ground and noise are dropped, the rest is bucketed in a 25 m grid.

- **Foliage:** canopy returns more than once, a roof once. The fit keeps single-return pulses (when enough are left), fits, and drops returns more than 1 m above that first fit (trees, antennas), then fits again. On Backen the single-return filter alone cut the median misfit by about 40 %.
- **Shapes** (`roof::model`): the returns inside a footprint are fitted, in the footprint's smallest enclosing rectangle, with a flat roof, one plane (*pulpettak*, at most 35°), a gable (*sadeltak*, ridge along either axis, ridge offset searched) and a hip (*valmat tak*, ridge along the long axis). Least squares with re-weighting so chimneys do not drag it; the cost is the mean squared residual clipped at 1 m². A richer model has to cost 30 % less than the simpler one (10 % for hip over gable), pitch must be 9°–50°, and the shape has to stay within 1.5 m of the measured heights at the rectangle's corners and centre, so a plane fitted to a few returns cannot climb away from them.
- **Parts** (`roof::tree`): a `Roof` is one shape or a straight `Cut` across the rectangle with a roof on each side (at most two levels). Only rectangles of at least 70 m² are cut; each part needs 24 returns and 4 m of width, and the parts together must cost at most 50 % of the single fit and save at least 0.04. Fewer than 10 returns, or 0.3 per m², leaves a flat roof at the OSM height.
- **Mesh** (`roof::build`): each earcut triangle of the footprint is cut along the cuts and then the creases (ridge and hip lines) into planar pieces; walls run up to the roof and are split where a crease or a cut meets the footprint, so gable ends are filled. Where two parts meet at different heights a step wall (double-sided) closes the gap, cut wherever a crease or another cut crosses the seam so it follows the bends of the roofs either side. Heights are absolute local y, so a roof on a slope stays level.
- **Format:** `Building` carries `first`, `walls`, `roof` (triangle ranges in the tile's collision mesh), `roof_kind` and `cells`, its 72 cm cubes as the importer baked them. A baked building keeps only its floor in the mesh; both sides collide with its cubes merged into boxes (`Shell::boxes`), and the client draws their outer faces (`Shell.faces`), so what is drawn is what is collided with; walls and roofs get separate colour palettes.
- **Result for Backen:** of 1 075 buildings, 808 gable, 117 flat, 85 shed and 65 hip (by the most complex part); 212 have more than one part.
- **Registration:** the footprints and the laser data only line up with a correct SWEREF 99 TM transform; the closed-form conformal latitude is pinned by a test against an independent series (an earlier series was 69 m off in northing). The same error had also shifted the DEM.

Not handled yet:
- Courtyard buildings (OSM multipolygons): the outer ring becomes a solid block and the courtyard's trees can bend its roof fit.
- Dormers, chimneys, half-hips and mansards. Parts are separated by straight cuts across the footprint's rectangle, so a roof whose parts do not line up with it is approximated.
- Railways. (Waterways are done: streams are ribbons, see §8 and WORLD.md.)
- Laser trees are approximate: crowns of equal height merge, species come from intensity and crown shape (see WORLD.md), and trees under 3 m are missed.

### Trees

Street and park trees come from Umeå kommun's open data (`trad-som-forvaltas-av-gator-och-parker`, ~32 000 trees; fetched by `osm-import`, skip with `--no-trees`). The register has position and species only, so each species is mapped by genus to one of seven archetypes (`TREE_*` in `crates/shared/src/world.rs`): birch (≈60 %), pine, spruce/fir/larch, ornamental (rowan, cherry, apple), broadleaf (maple, lime, oak), columnar (poplar, aspen, `Fastigiata`/`Erecta` cultivars) and low-wide (willow, alder). Height and crown come from the archetype, varied ±20 % by tree id and shrunk for recently planted trees. Trees on buildings or roads are dropped.

The register is only what was planted. **Every other tree comes from the laser point cloud** (`canopy.rs`): a canopy height model, treetops and crowns by watershed, and an archetype from the laser intensity (conifers are brighter) and the crown's shape, calibrated against the register's species. Register trees keep their species and take the laser's measured size. Details and accuracy in [WORLD.md](WORLD.md).

Each `Tree` is in its tile (`Tile.trees`). The client draws one instanced mesh per archetype per tile (`client/src/render/trees.ts`). Trunks are solid where `Tree::solid` (the register's, and trees within 20 m of a road or building; woods are scenery): an upright cylinder up to half the tree's height, built from `Tree::trunk_radius` / `trunk_height` on the server (`static_colliders`) and mirrored in `addTileCollider` and `decodeTile`.

The archetypes are rows of `TREE_KINDS` (`crates/shared/src/world.rs`), mirrored in `client/src/world.ts` and checked by `parity.spec.ts`. Besides the seven trees there are two **rocks** (boulder, slab) and a **bush**. A rock is solid to four fifths of its height with a radius from its crown, and is not an `Obj`: it blocks people, cars and bullets, but nothing damages or fells it, so it costs no `destroyed` traffic. A bush is never solid. The importer scatters both deterministically over wood, park and sand covers (`props.rs`; never in water, and through the same rejection near buildings and roads as every tree), so no new tile fields or `WORLD_VERSION` bump; they appear when the world is next generated. To add an archetype: a `TREE_KINDS` row on both sides, a prototype in `render/trees.ts`, and optionally a source in `props.rs`.

## 9. Physics

Both sides use Rapier 3D 0.36 through `crates/sim`: natively on the server, and built to WASM for the client's prediction (`crates/sim-wasm`), one world for the stickman and the vehicle it drives.

- **Static scene** (`static_colliders` / `buildStaticColliders`): a ground slab (on a flat world its top is y = 0 and it is the ground; on a terrain it is a safety net 12 m below y = 0, under the deepest water bed, so beds carved below the lowest dry ground can be driven and swum into), a heightfield for the terrain (one per tile on the client, one for the whole world on the server, same surface), four boundary walls up to y = 200, and the building trimesh.
- **Bitten buildings** (`shared::carve`, `World::carve`, `client/src/sim/carve.ts`): every building is a shell of 72 cm cubes on one world lattice, and it is drawn (`client/src/render/voxels.ts`) and collided with as those cubes on both sides, so you stand on the top of a cube, not on the roof inside it. A blast that reaches a standing building cuts a ragged hole of cells out of its walls and roof, and every cell left with no chain of cells to the ground falls, so cutting the foot of a wall brings the top down with it. The building's collider is replaced by the boxes of the cubes that are left, on the server at the next `step_cars` and on each client when the `carved` message arrives, so a hole is a doorway for people, bullets and vehicles. Carved buildings are also checked for structure now and then (`Shell::strain`, `World::settle`, `mechanics/structure.rs`): about 0.5 s after the first bite, and again after every check that took something, at most two buildings a tick. Each cell's distance to an anchor (up is free, down 1, sideways 2) and the weight it carries are compared with the same numbers for the whole building, so an untouched building never fails; a cell more than 6 cells further out than it was, or carrying over three times as much plus four, crumbles, as a `carved` entry with only `fell`. A check takes only the worst of them (at most 5% of the cells standing, 40 at least), so a big collapse comes in rounds and the client has time to show each one. A piece that fell leans towards the cells gone from round it (`leanOf` in `render/damage.ts`, worked out from the removed cells alone, so nothing more is sent): it tips top first and slides that way. A check costs about 0.15 ms on a house and about 1 ms on the biggest in the committed city. A building never collapses as a whole however much of it is gone: its health stops at 1, the worst damage stage, and shot cubes only darken. See NETWORKING.md for the cells and the message.
- **Stickman:** a capsule (half-height 0.5 m, radius 0.35 m, 1.7 m tall) moved by `KinematicCharacterController`.
  - Controller settings: offset 2 cm, slide on, autostep 35 cm, 50° max slope.
  - Characters are shape-cast, not colliders, so players don't block each other. Prediction then never depends on where remote players really are.
- **Movement rules** (`crates/shared/src/movement.rs`, run by both sides through `Physics::step_character`):
  - Horizontal speed is set directly from input (walk 4 m/s, run 7.5 m/s), plus any knockback velocity, which keeps 92% of itself per tick on the ground and 99% in the air.
  - Holding `BUTTON_CROUCH` (C or Ctrl, a CROUCH button on touch) overrides run and caps speed at 2 m/s, and lowers the eye 0.3 m for rifle and rocket origins. The capsule keeps its size, so collision and hit boxes do not change. Pressing crouch while running sends `BUTTON_SLIDE` once: the server launches the player along their heading as 12 m/s of knockback (well above the 7.5 m/s run) (only grounded, and not mid-slide), which bleeds off at 4% a tick while crouch stays held instead of the usual 8%. Being crouched, grounded and above 2 m/s of knockback is what clients draw as the `slide` clip, with the body turned along its heading, so no extra state travels. The server sets `FLAG_CROUCHING` in the entity flags, so other clients play the `crouch_*` clips.
  - Gravity is 20 m/s² and jump speed is 6.5 m/s.
  - The character is grounded only when not moving upward.
- **Two-pass move:** horizontal movement, then vertical, as separate controller calls. A combined pass makes the controller stall for a tick every few seconds on flat ground, in both native and WASM, and each stall is a misprediction.
- **Snap to ground** (`SNAP_DISTANCE`, 0.3 m) is on for the horizontal pass of a character that was standing and is not jumping, and off for the vertical pass, where it sinks the capsule slowly. Without it, running down a 20° slope is mostly airborne ticks, which stops jumping and slows the knockback decay. Native and WASM agree to about 1 cm on a slope run (`ramp_run_matches_fixture` ↔ `predict.spec.ts`).
- **Native vs WASM:**
  - The stickman is the same Rust code on both sides, built twice. Native and WASM floats are not promised bit-exact, but the fixtures (`predict.spec.ts`, checkpoints recorded from native) agree to well under 1 mm.
  - The client builds its scene a tile at a time (`Physics::streamed`, `add_tile`) with the server's collider code; `crates/sim/src/stream/tests.rs` checks that the streamed fixture city collides as the whole one does.
  - Reconciliation only ever replays about one RTT of ticks, so long-horizon drift never shows.

Vehicles are server-side dynamic rigid bodies (§10). The physics pipeline steps once per tick, and only does real work once a vehicle exists.
- **Cars are solid, and hit.** The character controller treats cars as ordinary obstacles. The client's Rapier world holds a box collider per car at the pose of the newest snapshot, so a parked car blocks the predicted stickman exactly as it blocks the real one. A car moving into a stickman instead throws them (§10).
- **Ray-cast cars.** `DynamicRayCastVehicleController` with a box chassis and four suspension rays, at 9.81 m/s² gravity (stickmen use 20).
- Vehicle code is in `crates/server/src/physics/vehicle.rs`. [VEHICLES-I.md](VEHICLES-I.md) covers the tuning and the tests.

## 10. Entities and vehicles

- **Entities:** players, creatures, vehicles, projectiles and items on the ground share one id space. Each is replicated as the components it has (§5.8): a stickman is a place, a heading, motion and flags; a vehicle adds an orientation, seats, health and whatever it carries (a turret, arms, thrust).
- **Seats (built):** each vehicle has a seat list. Seat 0 is the driver, and the others are passengers or gunners. A car has 4.
  - `BUTTON_USE` boards the nearest vehicle with a free seat within `ENTER_RADIUS` (4 m), taking the lowest free seat. Pressing it again leaves.
  - Leaving puts the stickman beside the car on the seat's side, or on the other side, the back or the front if that spot is blocked.
  - Seat state travels in the snapshot (each rider's `seated` component), not as reliable events. The client's view of its seat can then never disagree with the snapshots it draws. Reliable events stay for things that really are one-off, such as chat.
- **Input routing (built):** while seated, a player's input frames drive their seat instead of the stickman. The driver's forward axis is throttle and brake, the strafe axis is steering, and jump is the handbrake. A seated player is left out of the entity list, and their position follows the car.
- **Knockback (built):** after the cars step, any standing player whose capsule overlaps a car is thrown: a horizontal velocity of 1.1 × the car's speed (at least 2.5 m/s, at most 20), mostly along the car's heading with some outward push, plus a hop. `Character::knock` carries it. It is added to walking and bleeds off per tick (§9). It is in the snapshot, so the client's reconciliation replays it like any other state. A 12-tick cooldown makes one pass one hit. Damage is combat's: see §5.6.
- **Riding on top (built):** a stickman standing on a car is carried by it.
  - After each tick, `Movement` probes 6 cm below the feet for a car. The hit sets `Character::ride`.
  - On the next tick `Vehicles` moves every rider by the car's rigid motion since the last tick (rotation included), before the rider walks.
  - Riders are never knocked back.
  - Jumping off, or stepping off into the air, adds the car's horizontal velocity to the stickman's knockback. They keep the car's speed instead of dropping away from it.
  - The snapshot tells each player its own `ride`. The client predicts a rider in the frame where the car stands still at its newest snapshot pose, and draws it carried to wherever the interpolated car is. The stickman stays glued to the roof, and the walking on top is still predicted.
- **Driver prediction (built):** the driver's own vehicle is predicted. Everyone else's vehicles are still server-simulated and interpolated.
  - The driven vehicle is a body in the client's own `sim` world, the one the stickman walks (`VehicleDriver`, `client/src/sim/driver.ts`), stepped by the server's `drive_car` and `step_cars` with the server's `CarControls::driven`. Other vehicles are solid stand-ins in it, at the newest snapshot pose, with the solids `add_car` gives them.
  - The WASM build lands on the checkpoints the native build recorded for every vehicle kind (`crates/sim/src/vehicle/tests.rs` ↔ `vehicle.spec.ts`).
  - The snapshot carries the driver's `drive` motion (velocity, angular velocity, steering, spin). On each snapshot the sim is set to that state at `ack` and the pending inputs are replayed, like the stickman. The leftover is a position and rotation offset that decays (10/s). More than 3 m is a snap.
  - The turret, seats and every other vehicle still come from interpolated snapshots.
- **Vehicles:**
  - Car (built): ray-cast wheels, four seats, an open top.
  - Tank (built): six unsteered ray-cast wheels under a 13.5 t hull, with a driver and a gunner. The physics sets the yaw rate directly, so it turns on the spot like a tracked vehicle. See [VEHICLES-II.md](VEHICLES-II.md).
  - Gunner seat (built): seat 1 of a tank. Instead of driving, that player's input aims the turret and fires the gun. The `Gunnery` mechanic does this, apart from `Vehicles`, for every weapon mount a kind lists in `shared::mounts`. The turret follows the gunner's world heading at 1.4 rad/s (the barrel at 0.9 rad/s, between −7° and +29°), so turning the hull doesn't drag the aim. A shell is a projectile that leaves the muzzle at 140 m/s, falls under gravity and bursts on the first wall, vehicle or stickman it meets, with a 90-tick reload. The blast (7 m radius) throws stickmen and shoves vehicles. The snapshot carries `turret` angles and a shot counter.
  - Helicopter (built): no wheels. The hull's attitude is driven to a lean set by the stick (angular velocity towards the wanted up axis), the yaw rate is set directly, and a force along the up axis cancels gravity and holds the vertical speed the climb input asks for. Stateless, so the pilot predicts it like a car. Two seats: the pilot, and a gunner on an autocannon nose gun that fires small explosive shells. See [VEHICLES-II.md](VEHICLES-II.md#the-helicopter).
  - Hen (built): a giant robot hen on the walker's legs and balance, one seat. Space leaps about 6 m; held in the air its wings flap a slow climb until their stamina runs out, then spread and hold the fall to a 4 m/s glide, and W carries it forward. It lays egg bombs from under its tail that bounce and burst after 2 s, or at once when dropped from high up. Predicted by its driver like the others. See [VEHICLES-VII.md](VEHICLES-VII.md).
  - Plane (built): a Fokker Dr.I triplane stretched into a two-seater. Ray-cast wheels on the ground; in the air a stateless wing model (thrust, lift that fades below the stall speed, drag, a fin) and mouse-aim controls that fly it where the pilot looks. The pilot fires two forward machine guns (hitscan, with overheating) and drops bombs; the gunner works a ring-mounted gun behind. See [VEHICLES-IV.md](VEHICLES-IV.md).

## 11. Modular mechanics

Game rules are `Mechanic`s (`crates/server/src/mechanics/mod.rs`):

```rust
pub trait Mechanic: Send {
    fn on_start(&mut self, world: &mut World) {}
    fn on_join(&mut self, world: &mut World, id: u32) {}
    fn on_leave(&mut self, world: &mut World, id: u32) {}
    fn on_tick(&mut self, world: &mut World, dt: f32) {}
    fn hooks(&self, hooks: &mut Hooks) {}
    fn react(&mut self, world: &mut World) {}
}
```

- The room runs an ordered list (`mechanics::default_mechanics`): `Bots`, `Spawn`, `Pickups`, `Drones`, `Vehicles` (boarding, seats, driving, stepping car physics), `Gunnery` (turret aim and shells for gunner seats), `Movement` (the character controller), `Life`, `Npcs`, `Combat` and `Scores`.
- `on_start` runs once when the room is created. `Vehicles` uses it to park the cars.
- New features are new mechanics, such as `Pickups` or `Deathmatch`. Each owns its own state and reads and changes `World`.
- Mechanics say what happened as events (§2.1). `react` answers them after every mechanic has ticked, and the game loop turns what is left into reliable messages (`game::feedback`), so mechanics never touch sessions.

## 12. Art direction

- **Goofy and readable.**
  - Flat toon shading.
  - Candy-pastel buildings.
  - Green grass, pale roads with a darker edge and sandy paths, painted into the terrain; blue water, darker woods.
  - A light-blue sky with distance fog.
  - One shadow-casting sun that follows the player.
- **The player** is a low-poly (about 460 triangle) humanoid with a helmet and goggles, a backpack, and whatever it holds in its right hand (each item is its own mesh, shown or hidden by the item in hand, with three stances for the arms: rifle, pistol and hands-low), in a rigged GLB (`client/public/models/player.glb`). The outfit is tinted per player id. `tools/character/build_character.py` builds it, rig and clips included, in Blender (`Blender -b -P tools/character/build_character.py`).
  - Clips: `idle`, `walk_`/`run_` `fwd`/`back`/`left`/`right`, `jump_air` and `death`.
  - The body faces the camera yaw, so velocity in the body's frame picks the clips: strafing plays the side-step clips and backpedalling the backward ones. Weights blend smoothly and the cycle advances by distance covered, so the feet keep up with the ground.
  - The local player's torso (and so the gun) leans with the aim pitch, and every shot kicks it back. Tracers and the muzzle flash leave the tip of the gun in hand (the `muzzle` bone for the rifle, `muzzle_pistol` for the revolver). Other players' held items travel as three bits of `flags`, so everyone draws the same.
- **Camera:** third person, about 5 m behind, mouse-look with pointer lock.
- **Touch** (`client/src/TouchControls.vue`): when the primary pointer is coarse, tapping the canvas starts play in place of pointer lock. A floating stick appears under the left thumb (pushed past 90% it runs, except in a vehicle, where it steers). Dragging anywhere on the right looks. Buttons jump (brake in a car, climb in a helicopter), fire, reload and use, plus a descend button for the pilot, and the hotbar slots along the bottom are tappable. Everything feeds `InputController` (`setStick`, `lookBy`, `hold`, `tap`), so prediction and the wire format don't know touch exists.
- **Budget:** static geometry is merged by colour, so a tile is a handful of draw calls and the tiles around the player are about 100. A player is one skinned mesh in two materials (the tinted outfit and the fixed gear), so two draw calls and one skeleton of 19 bones. A full room of 64 adds at most about 130 draw calls, and a hidden player (in a car) costs nothing.

## 13. Security

- **Origin checks:** in production only `https://<domain>` is accepted, and in development only the Vite origin.
- **Packet validation:** datagrams over 1200 bytes are dropped unread. Malformed packets, wrong versions and oversized input batches are dropped.
- **Rate limits:** inputs at most every 8 ms and pings at most every 100 ms per session.
- **Room limits:** 64 players per room. Sessions silent for 10 s are dropped, so closed tabs don't linger as ghosts.
- **Authority:** clients can only express intent. Speed, collision and spawning are server-side, so speed and teleport hacks aren't possible.

## 14. Roadmap

1. **MVP (done):** server-authoritative stickmen on an OSM block, with prediction, reconciliation, interpolation, adaptive delay, network emulation and cross-language fixtures.
2. **Vehicles I (done):** a car, seats, enter and exit, driver input routing, server-simulated and interpolated. See [VEHICLES-I.md](VEHICLES-I.md).
3. **Vehicles II (done):** the tank with a gunner seat, driver-side prediction and the helicopter. See [VEHICLES-II.md](VEHICLES-II.md). Later: handling, crashes and sound ([VEHICLES-III.md](VEHICLES-III.md)), and the triplane ([VEHICLES-IV.md](VEHICLES-IV.md)).
4. **Combat (done):** free-for-all. Health, death and respawn, hitscan guns with lag compensation, vehicle damage and wrecks, kill feed and scoreboard. Hit feedback: the server sends each shooter and victim a targeted `hit` event (see NETWORKING.md); the client draws a crosshair marker, hit and hurt sounds, a screen-edge flash and arcs towards the attacker.
5. **Hotbar and pickups (done):** a five-slot hotbar with revolver, AK5, grenades and bandages, scattered around the map and taken with use. Per-item poses in the character model. Game modes and teams are next.
5. **Scale (done):** a room of 64 on a 2 km map.
   - Delta-compressed snapshots against the last acknowledged one, and interest management by distance (§5.8).
   - Rigged, skinned players (§12).
   - A larger world, Backen in Umeå, streamed in tiles (§8).
6. **Reach (done):** touch controls (§12) and a Docker and deploy script ported from flax3d. A quality scaler was dropped: the art is low-poly and a frame is about a hundred draw calls.
7. **World:**
   - Terrain elevation (done): Lantmäteriet's 1 m height model as a heightfield in the tiles, walked, driven, shot and drawn (§8, §9).
   - Ground cover (done): roads, water, woods, parks and other landuse painted into a per-tile terrain texture (§8).
   - Roofs (done): flat, shed, gable and hip roofs, in up to four parts per building, fitted to Laserdata Skog with foliage rejected (§8).
   - Trees (done): Umeå's street and park tree register as seven archetypes, instanced and with trunk colliders (§8).
   - Trees in woods (done): detected from the point cloud, `Tree::solid` limits colliders (§8).
   - Water (done): Hydrografi lakes, rivers and streams with a surface mesh and a toy-style shader.
   - Next: courtyard building multipolygons.

## 15. Ideas

### Restructuring, refactoring, embetterment

Make the project more modular and maintainable, with clear separation of concerns between rendering, networking, and game logic.
Consider breaking the codebase into smaller, self-contained modules with well-defined interfaces, and adopting a consistent coding style and documentation standard.
Do this without negatively impacting the existing functionality or introducing unnecessary complexity or performance degradation.

Done: dynamic entities are `bevy_ecs` components, the client predicts with the server's own physics, and the messages, components and tables are generated from Rust (§2.1).

### Tile meshes off the main thread

Build tile meshes in a Web Worker, so the main thread never waits on them. Tiles are built on the main thread as they stream in (`attachTile` → `buildTile`).
- On Ersboda most tiles build in about 2 ms.
- The worst one, with very large buildings, takes about 21 ms. That is more than a 60 fps frame (16.7 ms), so it can cause a visible hitch.

This is a bigger change. The worker would make the buildings' faces (`Shell.facesWithRoof`) and the vertex arrays, and pass them back to the main thread without copying them. The scene graph and the Rapier colliders must still be made on the main thread. The client's carve state must then be kept in step between the two threads.

### Editor

Adding and removing weapons, vehicles, and other game entities should be straightforward, with minimal impact on unrelated systems. base engine should have a clear entity-component system, allowing for flexible composition and easy extension of game objects.
Generating terrain and game worlds should be flexible and data-driven, allowing for world generation from external data sources, procedural algorithms, and manual additions. This should probably be handled by a dedicated world generation module that can be easily extended and configured.
Probably built as a separate module that can be easily integrated with the rest of the engine, allowing for flexible world generation and easy updates to terrain and game content. Also consider providing tools for visualizing and editing the generated worlds within the engine. this editor should have a user-friendly interface and support common operations like adding, removing, and modifying terrain features and game objects.
It will be designed as an integrated part of the engine, leveraging the same rendering and entity systems as the main game, ensuring consistency and reducing duplication of effort. It should also support hot-reloading of assets and game logic to facilitate rapid iteration during development. it should provide a seamless experience for developers, allowing them to see changes in real-time and quickly test new features and content.
Change and tweak constants in a centralized configuration file or module, allowing for easy adjustments and experimentation without modifying the core engine code. This approach promotes maintainability and reduces the risk of introducing bugs when fine-tuning the game's behavior or performance.
### The Orange Plague (parked on branch `the-orange-plague`)

A 40 m satirical tycoon boss who walks the map "rezoning" its tallest buildings. He has two attacks: a stomp at his feet and an airstrike of rockets far off, both telegraphed. A brain picks them from a threat ledger, sight and hearing. One skeleton in `shared` drives his hitboxes and physics capsules on both sides. His eyes, ears and mouth can each be shot out, blinding, deafening or silencing him. The model is smooth and procedural, and his barks are voiced with clips recorded locally by Kokoro (`tools/voice`). Protocol version 32. Not merged. Still to do: the Press Conference and enrage phases, staggering from leg damage, and the gold-tower prop.
