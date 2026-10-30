# Networking

This is the wire protocol between the browser client and the game server. Rust is the source of truth (`crates/shared/src/protocol.rs`, `consts.rs`, `quant.rs`). Every message is one entry of the `messages!` list in `protocol.rs`. The client's types and decoders (`client/src/net/messages.generated.ts`) and the bytes of one of every message (`client/src/__tests__/messages.fixture.json`) are generated from it by `UPDATE_PARITY=1 cargo test -p shared`, and the components' (`components.generated.ts`) from `replicated.rs`. `client/src/net/codec.ts` holds only the framing.

[design.md §5](design.md#5-netcode) explains how the pieces fit together: prediction, the jitter buffer and interpolation.

## Session

1. **Connect:** the client opens WebTransport to `/transport`, with the player's settings from `/settings.html` as query parameters: `?name=Åsa&color=3&spawn=x,z&spawn=x,z&loadout=2,5,0,8,11`. All are optional. `name` is cut to 16 characters with control and direction-changing characters removed, and an empty one becomes `Player <id>`. `color` indexes the 12 outfit colours (`PLAYER_COLORS` in `client/src/profile.ts`). Each `spawn` is a point in centimetres in the world's local frame; the server keeps up to 5, drops any outside the world, in water or within 1 m of a building, and uses the rest in turn, one per spawn. `loadout` is the item id in each of the 5 hotbar slots, 0 for an empty one; anything but all five, a golden egg or an item twice is dropped whole, which leaves the standard loadout. Unknown parameters are ignored, so later settings (a class) can be added the same way.
   - Development: `https://127.0.0.1:8698/transport`. The browser pins the self-signed certificate through `serverCertificateHashes`. The client fetches the hash from `/local-certificate-hash`, which Vite proxies to the server's `:8699/certificate-hash`.
   - Production: `https://<host>/transport` with an ACME certificate. UDP 443 carries HTTP/3.
2. **Origin check:** the server rejects any other origin. In production only `https://$RAGNAROK_DOMAIN` is allowed, and in development only `http://localhost:5173` and `http://127.0.0.1:5173`.
3. **Admission:** at 64 players the room is full and the session is refused.
4. **Welcome:** the server sends `welcome` first on its reliable stream (below). The client ignores snapshots until it has one.
5. **Play:** the client sends `input` datagrams every tick and a `ping` every second. The server sends a `snapshot` datagram every 2 ticks, holding what changed near that player since the snapshot they last acknowledged (below).
6. **End:** the session ends on close, error, or after 10 s without any datagram. The server then removes the player and sends `leave` to everyone else.

## Messages

All messages are MessagePack arrays whose first element is a tag. The client→server and server→client tag spaces are separate.

### Client → server (datagrams)

| Tag | Name | Shape | Notes |
|---|---|---|---|
| 0 | `input` | `[0, version, baseline u32, [frame, …]]` | 1–4 frames, oldest first. Throttled to one per 8 ms. `baseline` is the `server_tick` of the newest snapshot the client has rebuilt, or 0 for none |
| 1 | `ping` | `[1, nonce u32]` | Echoed as `pong`. Throttled to one per 100 ms. |
| 2 | `rtc` | `[2, peer u32, kind u8, json string]` | Voice signaling for player `peer`. **Not a datagram**: sent on one unidirectional stream the client opens, each message behind a 4-byte big-endian length (as below). `kind` 0 offer, 1 answer, 2 ICE candidate; `json` is the session description or candidate. Server drops more than 64 a second, payloads over 8 KiB and frames over 8 KiB + 64 |

`frame` is `[seq u32, strafe i8, forward i8, yaw i16, buttons u8, pitch i16, view u16, slot u8]`:

| Field | Unit | Range |
|---|---|---|
| `seq` | per-connection counter, one per client tick | starts at 1 |
| `strafe` | axis, + is right | −127..127 |
| `forward` | axis, + is forward | −127..127 |
| `yaw` | milliradians, 0 faces −Z | [−3142, 3141] |
| `buttons` | bits: 1 jump, 2 run, 4 use, 8 fire, 16 alt (unused on foot), 32 crouch, 64 slide, 128 reload | |
| `pitch` | milliradians, + looks up. Aims a tank gunner's barrel and a held gun or grenade | about [−1200, 600] |
| `view` | the server tick the picture on screen showed, modulo 65536. The server rewinds other players to it to resolve a gunshot, by at most 15 ticks and never forwards | |
| `slot` | the hotbar slot the player has in hand. Fire, reload and use-item act on it | 0..4 |

While driving, `forward` is the throttle (negative brakes, then reverses), `strafe` is the steering, `buttons` bit 1 (jump) is the handbrake, and `yaw` is ignored. A Roadster's driver boosts with run (bit 2), hops with jump (held in the air, it hovers) and pulls the handbrake with crouch (bit 32). A helicopter's pilot (seat 0) uses `forward` to lean forward and back, `strafe` to turn, jump to climb and run (bit 2) to sink. A passenger's axes are ignored. A tank's driver steers with `strafe`, which turns the tank on the spot, and its gunner (seat 1) aims with `yaw` and `pitch` and shoots with the fire bit. The gunner's `yaw` is a world heading: the turret turns to it at a limited rate, whatever the hull does.

Each datagram repeats the newest 4 frames, so a single lost datagram loses nothing. The server ignores frames it has already seen.

### Server → client

**Reliable stream.** The server opens one unidirectional stream per session, when it has its first reliable message (`welcome`), and writes every reliable message to it, in order. Each message is a 4-byte big-endian length and then that many bytes of MessagePack. Messages therefore arrive in the order they were sent: a `kill` never overtakes the `leave` of its victim, and the joiner's `profiles`, `scores`, `destroyed`, `damaged`, `carved`, `dug`, `cratered` and `built` catch-up come right behind the `welcome`. The client refuses a length over 4 MiB and drops the session on a malformed stream. The server drops a session whose stream accepts nothing for 5 s, or whose outgoing queue (64 messages, shared with snapshots) is full when a reliable message is due, rather than let it miss one: it reconnects and gets the catch-up.


| Tag | Name | Channel | Shape |
|---|---|---|---|
| 0 | `welcome` | reliable stream | `[0, version, entity_id u32, tick_rate u8, snapshot_every u8, world_hash u32]` |
| 1 | `snapshot` | datagram | `[1, server_tick u32, baseline u32, ack u32, ride u32, [update, …], [removed u32, …], [shot, …], [bullet, …]]` |
| 2 | `pong` | datagram | `[2, nonce u32]` |
| 3 | `leave` | reliable stream | `[3, entity_id u32]` |
| 4 | `kill` | reliable stream | `[4, killer u32, victim u32, weapon u8, killer_kills u16, victim_deaths u16, style u8, yaw i16 mrad, power u8 (0.2 m/s)]`; `style` is `DEATH_*` (body, head, blast, roadkill, overkill) and `yaw`/`power` are the way and how hard the body is thrown, so every client stages the death alike |
| 5 | `scores` | reliable stream | `[5, [[id u32, kills u16, deaths u16, held u16], …]]`; `held` is the seconds carrying the backpack ([BACKPACK.md](BACKPACK.md)) |
| 6 | `destroyed` | reliable stream | `[6, [[kind u8, col u16, row u16, index u32, yaw i16, force u8], …]]` |
| 7 | `damaged` | reliable stream | `[7, [[col u16, row u16, index u32, stage u8, mark [x i32, y i32, z i32] or nil], …]]` |
| 8 | `hit` | reliable stream | `[8, role u8, kind u8, amount u8, weapon u8, bearing i16 or nil, x i32, y i32, z i32, target u32]` |
| 9 | `rtc` | reliable stream | `[9, peer u32, kind u8, json string]` |
| 10 | `carved` | reliable stream | `[10, live bool, [[col u16, row u16, index u32, [x i32, y i32, z i32], [blasted u32, …], [fell u32, …]], …]]` |
| 11 | `profiles` | reliable stream | `[11, [[id u32, name string, color u8, team u8, bot bool], …]]` |
| 12 | `notice` | reliable stream | `[12, kind u8, text string]` |
| 13 | `built` | reliable stream | `[13, live bool, [[x i32, y i32, z i32, color u8, material u8], …], [[x i32, y i32, z i32], …], [[x i32, y i32, z i32], …]]`: placed, removed, fell |
| 14 | `cratered` | reliable stream | `[14, live bool, [[x i32, z i32, radius i32, depth i32, age u16], …]]` |
| 15 | `dug` | reliable stream | `[15, [[col u16, row u16, [skip, count, value…, …] i32s], …]]` |
| 18 | `backpack` | reliable stream | `[18, x i32, z i32, radius u16, carrier u32, held u16]`, cm: in a world with the backpack ([BACKPACK.md](BACKPACK.md)), the circle it is somewhere inside, who carries it (0 when nobody) and their seconds with it. Sent to everyone when the circle moves or the backpack changes hands, once a second besides, and to a joiner |

- `baseline` is the `server_tick` of the snapshot this one is a change from, or 0 when it is not. Then every entity in range comes whole. Each `update` is one entity's components: see [Entities and components](#entities-and-components) and [Delta snapshots](#delta-snapshots-and-interest).
- `ack` is the newest input `seq` of the receiving player already applied to this state. The client drops pending inputs up to `ack` and replays the rest.
- `ride` is the id of the vehicle the receiving player stands on top of, or 0. The server carries riders with the car, so the client predicts them in the car's frame. Only the receiver's own value is sent: other riders are plain entities whose positions already move with the car.
- What only the receiving player is told (their health and hotbar, and the motion of the vehicle they drive) comes as their own entities' owner components: `vitals`, `hotbar` and `drive` below.
- `shot` is `[shooter u32, weapon u8, x i32, y i32, z i32]`: something since the previous snapshot, and where it ended (cm). It is only sent to a player within 300 m of where it ended. Weapon 13 (shotgun) is a muzzle blast with the sound, and each further pellet is 14, a spark where it landed; 15 is a flying bullet ending (its `shooter` is the bullet's id, see `bullet`). 16 is a shot glancing off a vehicle's armor (its `shooter` is the vehicle's id): sparks and a ping, no blast. 17 is a tank's turret being shot off (`shooter` is the tank): the turret flies away as debris. Weapons 0 (AK5) and 4 (revolver) are not sent as shots any more: their rounds are bullets. 1 (a tank shell), 6 (a helicopter autocannon round), 2 a rocket, 5 a grenade, 11 a plane's bomb, 20 a hen's egg, 19 an FPV drone's charge and 3 a vehicle wreck going up are a burst at the point. A drone shot down pops as a 6. 10 is a plane's machine-gun round that hit something, a spark at the point: every round's tracer is drawn by the clients from the gun's shot counter, so a round that hits nothing sends no event at all. It is cosmetic, so it rides the lossy datagram and is capped at 8 per snapshot, and dropped first if a snapshot would not fit.
- Projectiles in flight are entities like any other, with `projectile` (its `PROJECTILE_*`: a rocket (0), grenade (1), tank shell (2), autocannon round (3), plane's bomb (5), mortar bomb (6), hen's egg (7) or flare (8)), `place` and `velocity`. Clients interpolate them and aim a rocket along its velocity. One that is removed has burst, and its burst arrives as a `shot` event. Gun bullets (kind 4) are not listed: see `bullet`.
- `bullet` is `[id u32, shooter u32, item u8, x i32, y i32, z i32, vx i16, vy i16, vz i16, tick u32]`, cm and cm/s: a gun round that left the muzzle on server tick `tick`, sent once (in two consecutive snapshots, so a lost datagram does not lose it, and clients ignore an id they have seen) to every player within 300 m of its line out to its gun's range, nearest first, at most 32. It is not listed again: it moves `v*dt` a tick and then `v.y` loses `20 * 0.25 * dt`, so a client works out `p + v*n*dt - (0, 20*0.25*dt^2*n(n+1)/2, 0)` for `n` ticks after `tick`. It ends with a `shot` of weapon 15 and `id` in `shooter`, or when its gun's range is flown. Bullets are shed after pickups when a snapshot would not fit.
- Items lying on the ground are entities with `pickup` (the `ITEM_*`) and `place`: within 300 m, the nearest 24 only. One that is removed has been taken, or the player walked away from it. A player takes the nearest within 2 m by sending the use bit; the server hands it over before it considers boarding a vehicle.
- `kill` goes to everyone when a player dies. `killer` is 0 when the world did it (a fall). The totals are the new ones, so clients keep no tally of their own. `scores` is sent once to a player who has just joined.
- `profiles` names players and gives their outfit colour, their team and whether they are a bot. `team` is 0 for people, who are all against all; players on the same other team are one side. A player who has just joined gets everyone's, right after `welcome`; everyone else gets the joiner's, and a bot's when it is added. Snapshots can arrive before it, so a client draws a soldier in a colour chosen by id until its profile arrives.
- `notice` is what a world's scenario tells everyone (see BOTS.md, Scenarios). `kind` 0 is a message, shown for a few seconds; 1 is the objective, shown until the next one, and empty clears it; 2 and 3 are the scenario being won and lost. A player who has just joined gets the current objective with the catch-up. Text is at most 200 characters.
- `destroyed` is the only world state that changes: buildings and trees that are gone. `kind` is 0 for a tree and 1 for a building; `(col, row, index)` names it by its tile and its position in that tile's list (`ObjId`), which both sides already agree on and which a re-import renumbers. `yaw` (milliradians, 0 faces -Z) is the way a tree falls, or the direction a building's blast came from (its rubble is thrown the other way), and `force` (0..255 for 0..1) how violently it went: 0 for the catch-up. It goes to everyone when something falls, and a player who has just joined gets one message holding everything destroyed so far, before the first snapshot. Only the fact is replicated: the fall, the rubble and the dust are drawn by each client, and the trunk or the building's walls stop colliding on both sides at once. A building falls to rockets, grenades and shells (a rifle only chips it), and a tree to anything.
- `hit` is sent to one player only, once per tick per `(role, kind)` with `amount` (hp lost) summed. `role` 0 is *dealt* (you hurt something: kind 0 soldier, 1 vehicle, 2 a soldier this finished, 3 rooster) and 1 is *taken* (something hurt you, or the vehicle you sit in). For a taken hit `bearing` is the yaw in milliradians (0 faces -Z, as a look) from you towards the attacker or blast; nil means no direction (a fall, a crash). `weapon` is 0 for vehicle hits. `(x, y, z)` (cm) is where the thing hit is (chest height) and `target` is its id, for the damage number, debris and flash. It drives the hit marker, hit sound, screen flash and direction arcs; non-lethal damage otherwise reaches a victim only as a lower `hp`.
- `damaged` tells everyone that standing buildings are hurt, so they can look it. `stage` is 0 (whole) to 3 (worst; a building never comes down, its health stops at 1), entered at 66%, 33% and 15% of the building's health. An entry is sent when a building enters a worse stage, or when a blast of at least 5 damage hits it; then `mark` is where the blast landed, `[x, y, z]` in cm, and otherwise nil (clients no longer use it: charring comes from the removed cells). Rifle chips send nothing until they cross into a worse stage. A player who has just joined gets one message with the stage of every hurt building (never marked). The dust a building at the worst stage sheds is drawn by each client, and the burn around a hole is worked out from the `carved` cells; the holes themselves come from `carved`. Trees are not reported.
- `carved` tells everyone which cells a blast took out of a standing building. A building is cut into a grid of cells (`crates/shared/src/carve.rs`, which the client runs too, built to WebAssembly): 72 cm cubes on one lattice for the whole world, a cell being named by `(y * dims.z + z) * dims.x + x` over the building's bounding box (a `u32`). A cell is solid where the walls or roof pass through it, found by sampling them at points at most 36 cm apart, with integer arithmetic on centimetres, once, by the importer, which bakes the cells into the tile (WORLD.md); the clients draw the solid cells as cubes (their outer faces merged into rectangles, `Shell.faces`), and both sides collide with the same cubes: merged into boxes by one deterministic pass (`Shell::boxes`), plus the building's floor, so a cell number means the same cube and boxes to both (pinned by `cell_numbers_match_the_fixture` and `carve.spec.ts`). `[x, y, z]` (cm) is where the blast landed; `blasted` are the cells within its reach, roughened per cell so the edge is ragged, and `fell` the cells that were left with no chain of cells down to the building's lowest layer. Rockets, grenades and shells bite; a rifle only chips health. Only the cell numbers are replicated: the hole, the thrown pieces and the dust are drawn by each client, and the walls stop colliding in the removed cells on both sides at once (the server swaps the building's collider for the boxes of the cubes left plus the floor; the client does the same with the tile's building collider). `live` is true for a bite that just happened and false for the catch-up a joiner gets after `welcome`, which lists every removed cell under `blasted` and animates nothing.
- `cratered` tells everyone that a blast (anything explosive) dug a crater where it burst on the ground (CRATERS.md). An entry is the crater's centre on the ground plane, its radius and its depth at the middle, all in whole centimetres. Both sides dig the same smooth bowl into the samples within `radius` of the centre with integer-rounded arithmetic (`Ground::dig` in `crates/shared/src/terrain.rs`, mirrored sample for sample by `digCrater` in `client/src/sim/terrain.ts` and pinned by a fixture), so only the four numbers travel. No sample goes more than `CRATER_MAX_DEPTH` under what the world file has, however many land. The order matters: craters are dug in the order listed. `live` is true for craters just dug, which a client digs into every loaded tile it reaches, and into a tile that streams in later when it arrives; it gives the physics the dented tile and draws the terrain and the roads on it again. `live` is false for the catch-up a joiner gets after `welcome` and `dug`: the craters of the last `CRATER_FADE_SECONDS` (at most `CRATER_MAX_KEPT`), in order, each with its `age` in seconds (0 when live). These are only marked on the ground, fading with their age (SCORCH.md); their heights came in `dug`.
- `dug` is sent to a joiner before the craters, in batches: every tile whose ground craters changed, as its difference from the tile file's heights in whole centimetres. `runs` covers the tile's `side × side` samples row-major (as `Tile::ground`): `skip` unchanged samples, then `count` changed ones and their values, repeated. The client lays it on the tile's file heights when it streams in, before the craters dug since it joined, so a joiner's ground is the server's however many craters there have been.
- `built` tells everyone what happened to the cubes players build with bricks ([BUILDING.md](BUILDING.md)). A cube is named by its place on the buildings' lattice of 72 cm cubes, `(x, y, z)` with its corner at `CELL` (72) cm times each (`crates/shared/src/construct.rs`), not by a building's cell number, as a build can go past any building's grid. `placed` are new cubes with their colour (the builder's, `PLAYER_COLORS`) and material (`construct::MATERIALS`, how tough it is). `removed` are cubes dug out, shot out, rammed or blasted, and `fell` are cubes left with nothing holding them up, which come down. Everything that changed in a tick is one message. Both sides keep the same cubes and collide with them as boxes merged by one deterministic pass, a chunk of 16³ cubes at a time (`chunk_boxes`, the WASM `setConstructChunk` on the client). `live` is false for the catch-up a joiner gets after `welcome`: every cube there is under `placed`, in messages of at most 32 768 cubes, with nothing animated.
- `rtc` relays proximity voice signaling. The server writes `peer` itself (the sender's entity id), so it cannot be forged, and drops it silently if the receiver's queue is full: WebRTC retries. The voice itself goes peer to peer between players within about 40 m (up to 8 connections, opened at 35 m and closed at 50 m); the lower entity id sends the offer. ICE uses STUN, plus TURN from `VITE_ICE_SERVERS` if set.
- `world_hash` is FNV-1a over `world.msgpack`. A mismatch means the client loaded a different world than the server simulates.

## Entities and components

Everything a snapshot shows of the world is an entity: a soldier, a creature (the Hedemora rooster),
a vehicle, a projectile or an item on the ground. They share one id space. An entity is the components
it has, and the client tells what a thing is from them, never from a family. Every component is
defined once, in `crates/shared/src/replicated.rs`; the client's types and decoder schema are
generated from it (`client/src/net/components.generated.ts`).

An `update` is `[id u32, set u32, (cleared u32), fields…]`:

- `set` is a bit mask of the components whose fields follow, in wire-id order. Each component's
  fields come one after another, with no array of their own.
- Bit 31 of `set` says a `cleared` mask follows: the components the entity no longer has, such as a
  repaired hull's `damage`.
- A component marked *diff* below is sent as the change from the receiver's baseline when the baseline
  has it, and as the value itself when it does not (a new entity, or one that just gained it).

| Bit | Component | Sent | Fields | Meaning |
|---|---|---|---|---|
| 0 | `place` | diff | `x, y, z` i32 | centimetres: a soldier's capsule centre, a vehicle's chassis centre (+X east, +Z south, +Y up) |
| 1 | `heading` | diff | `yaw` i16 | facing, milliradians, 0 faces -Z |
| 2 | `motion` | diff | `vy, knock_x, knock_z` i16 | a soldier's vertical speed and the slide a car's knock left them in, cm/s: what a predicted soldier resumes from |
| 3 | `flags` | whole | `flags, stance` u8 | bit 0: grounded, bit 1: dead (lying where it fell until it respawns), bit 2: crouching, bits 3..6: the item in hand (`ITEM_*`, 0 for none), bit 7: crowing (a rooster); `stance` is `STANCE_*` (standing, prone, under a parachute) |
| 4 | `orientation` | diff | `qx, qy, qz` i16 | the x, y, z of the orientation quaternion, scaled by 32767. The sender negates the quaternion so that `w >= 0`, and the receiver rebuilds `w = sqrt(1 − x² − y² − z²)` |
| 5 | `steer` | diff | `steer` i8 | front-wheel angle as a fraction of full lock (the kind's `*_MAX_STEER`), −127..127, + is right |
| 6 | `turret` | whole | `yaw, pitch` i16, `shots` u8, `range` u16, `flags` u8 | a gun on a mount (below). A plane's is the rear gunner's ring |
| 7 | | | | retired (was the vehicle's `crew`; see `seated`) |
| 8 | `thrust` | whole | `air, flags` u8 | The air left in its tank, 0..255 of full, and the nozzles firing (bit 0 the rear boost nozzles, bit 1 the lift nozzles). Other clients draw the jets from `flags`; the driver's prediction resumes from `air`. A drone sends its battery as `air` and sets bit 2 while its pilot still flies it. A hen sends its wings' stamina as `air`; bit 1 is set while it leaps or flaps, bit 3 while it glides with its wings spread and still, and bit 4 while neither foot is on the ground |
| 9 | `arms` | whole | `shots, heat, rear_heat, bombs, flags` u8 | a plane's pilot weapons and engine, or a hen's eggs (below) |
| 10 | | | | retired (was the vehicle's `seats`; see `seated`) |
| 11 | `health` | whole | `hp` u8 | a vehicle's health; 0 is a burning wreck nobody can board |
| 12 | `damage` | whole | `parts`: 8 × u8 | Absent while unhurt; front, rear, left, right, then wheels FL, FR, RL, RR, each 0..255 (a wheel at 255 is gone). A tank has no wheels to hurt, so its byte 4 (`DAMAGE_TURRET`) is 255 (`TURRET_GONE`) once its turret has been shot off |
| 13 | `velocity` | diff | `vx, vy, vz` i16 | a projectile's velocity, cm/s |
| 14 | `kind` | whole | `kind` u8 | which model it is drawn as: 0 = soldier, 5 = Hedemora rooster, and vehicles 1 = jeep, 2 = tank, 3 = helicopter, 4 = sports car, 6 = plane, 7 = walker, 8 = Strv S, 9 = field mortar, 10 = Roadster, 11 = FPV drone, 12 = hen (5 is the rooster, an entity). A drone is listed only while it flies or falls: see [DRONES.md](DRONES.md) |
| 15 | `projectile` | whole | `kind` u8 | what is in flight, `PROJECTILE_*` |
| 16 | `pickup` | whole | `item` u8 | what lies on the ground, `ITEM_*` |
| 17 | `vitals` | whole, owner | `hp, progress` u8 | the player's own health, 0 when dead (others only see dead or alive, in `flags`), and how far a reload or a bandage has got, 0..255, or 0 when neither is going on |
| 18 | `hotbar` | whole, owner | `slots`: 5 × `[item u8, count u8, ammo u8]` | the player's own hotbar, `item` 0 for an empty slot (`1` revolver, `2` AK5, `3` grenade, `4` bandage, `5` sniper, `6` golden egg, dropped by a rooster and healing to full, `7` shotgun, `8` FPV drone). The item in another player's hands is in bits 3..6 of their `flags` |
| 19 | `drive` | whole, owner | 8 × i16 | the exact motion of a vehicle a player drives, which its pose does not carry, for the driver's prediction (below) |
| 20 | `seated` | whole | `vehicle` u32, `seat` u8 | a player sitting in seat `seat` of `vehicle`; seat 0 drives. They have no `place` unless their body stays behind (a drone's pilot): clients draw them at the kind's seat head point. The client learns its own seat from it |
| 21 | `look` | diff | `pitch` i16 | how far a rider whose crew show looks up (+) or down, mrad; their `heading` is where they look about, in the world |
| 22 | `carry` | whole | `what` u8 | a soldier carrying the backpack (`CARRY_BACKPACK`); the client's own prediction walks twice as fast with it. The backpack on the ground is an entity of `kind` 15 with a `place` |

What each carries today:

| Thing | Components |
|---|---|
| Soldier | `kind`, `place`, `heading`, `motion`, `flags`; `vitals` and `hotbar` for themself; `seated` too for a drone's pilot |
| Rooster | `kind`, `place`, `heading`, `flags` |
| Rider | `kind`, `seated`; where their crew show (jeep, sports car, Roadster, helicopter, triplane, mortar), also `heading`, `look` and `flags` (what they hold); `vitals` and `hotbar` for themself |
| Vehicle | `kind`, `place`, `orientation`, `steer`, `health`, and whichever of `turret`, `thrust`, `arms`, `damage` it has; `drive` for its driver |
| Projectile | `projectile`, `place`, `velocity` |
| Item on the ground | `pickup`, `place` |

A component marked *owner* is sent only to the player who owns the entity: themself, and the vehicle
they drive. Everyone else is sent the entity without it. What a player owns is sent to them wherever it
is. A rider with no `place` is sent to whoever is sent their vehicle.

A jeep has 4 seats, and a sports car and a Roadster 2; a tank has 2, the second being the gunner; a helicopter has 2, the pilot and the nose gunner; a plane has 2, the pilot (who fires the forward guns and drops the bombs) and the rear gunner; a walker, a hen and a Strv S have 1, the driver, who also fires (a hen's driver lays its eggs) (the Strv S's gun is fixed in the hull, so its `turret.yaw` is always 0 and only `pitch` moves) |

`turret` is `[yaw, pitch, shots, range, flags]`:

| Field | Unit |
|---|---|
| `yaw` | milliradians, turret angle relative to the hull, + turns left |
| `pitch` | milliradians, barrel elevation, + is up |
| `shots` | shells fired so far, modulo 256. A client sees a shot as a change in this counter, and draws the flash from the barrel it is drawing |
| `range` | decimetres: how far the last shell flew before it hit something (3000 means it hit nothing). A field mortar's is its range dial |
| `flags` | `TURRET_*`: bit 0 (`TURRET_STOWED`) is set while a field mortar is hitched to a car, and clients draw it folded for travel |

`arms` is a plane's pilot weapons and engine, `[shots, heat, rear_heat, bombs, flags]`. A hen sends one too: `shots` counts the eggs it has laid (its tail flicks up on each), `bombs` is the eggs left, 0..6, and the rest stay 0:

| Field | Unit |
|---|---|
| `shots` | forward rounds fired so far, modulo 256; the left gun fires the even ones. Clients draw a tracer from the right gun for each step of the counter |
| `heat`, `rear_heat` | the forward guns' and the ring gun's heat, 0..255. At 255 they jam until they have cooled to 77 |
| `bombs` | bombs left on the rack, 0..4 |
| `flags` | bit 0: the blip switch is cutting the engine; bit 1: the forward guns are jammed; bit 2: the ring gun is jammed |

`drive` is `[vx i16, vy i16, vz i16, wx i16, wy i16, wz i16, steer i16, spin i16]`:

| Field | Unit |
|---|---|
| `vx, vy, vz` | linear velocity of the chassis, cm/s (world axes) |
| `wx, wy, wz` | angular velocity, milliradians per second (world axes), saturating |
| `steer` | front-wheel angle in milliradians, + is right. Not the `steer` fraction of the vehicle, which is too coarse to resume from. Zero for a tank and a helicopter; a plane's is its main wheels' |
| `spin` | the yaw rate a tank has ramped to, milliradians per second, + is clockwise. Zero for a helicopter and a plane, whose yaw rates are in `wy` |

The vehicle's `x, y, z` and `q*` give the rest of the state. Together they are everything the client's Rapier vehicle needs to continue the simulation from the server's state as of `ack`.

The client learns which seat it occupies from its own `seated`. There is no separate seat event, so the seat state can never disagree with the snapshot stream it is drawn from.

Boarding and leaving use the `use` button bit (4) in an input frame. It is one-shot, like fire: the server ignores both on repeated frames.

## Delta snapshots and interest

A snapshot is built per player from what is within `INTEREST_RADIUS` (300 m on the ground plane) of
their soldier, and then reduced to the change from a baseline. [design.md §5.8](design.md#58-interest-management-and-delta-snapshots)
says why; this is the format.

- `[update, …]`: an entity not in the baseline comes whole; one that was and changed sends only the
  components that changed, and clears those it lost.
- `[removed, …]`: ids that were in the baseline and are no longer listed: out of range, left, taken
  or gone.

The client's scene after a snapshot is:

1. Start from the scene it rebuilt for snapshot `baseline` (empty if `baseline` is 0).
2. Remove every id in `removed`.
3. Apply each update to the entity of that id: clear what `cleared` names, then set each component it
   sends, adding a *diff* one to the baseline's value when the baseline has it. An update for an id
   the scene does not hold adds that entity.

Anything not mentioned is unchanged. A client that does not hold `baseline` drops the snapshot. Keep at
least the last 64 rebuilt scenes. The server uses only baselines among the last 32 snapshots it sent
that player, so one it still offers is one the client still has.

**The server diffs against the newest `baseline` the client acknowledged**, not against the snapshot it
sent last. Loss then costs nothing but a larger next update, and nothing has to be retransmitted. A
baseline the server no longer holds (an acknowledgement over about a second old, or one that was never
sent) gets a whole snapshot with `baseline` 0.

**Interest has hysteresis.** A thing enters at 300 m and stays until 340 m. Items on the ground do not:
only the nearest 24 within 300 m are listed.

**The packet limit** is kept by shedding: effects (`shot`) first, then updates of projectiles, then of
items on the ground, then bullets, then the farthest updates of everything else. Whatever was shed
arrives in a later snapshot, nearest first. `removed` is never shed.

## Rates and limits

| | Value |
|---|---|
| Simulation tick | 60 Hz |
| Snapshot rate | 30 Hz |
| Input datagrams | 60 Hz, about 35 bytes each, so about 2 KB/s up |
| Snapshot size | about 25 bytes of header, plus about 19 per new soldier or 7 per moving one, 25 to 40 per new vehicle or about 10 to 15 per changing one, and 20 more for the driver. Only what is in range and has changed is sent: 30 players running in view is about 230 bytes, so about 7 KB/s down. Standing still costs nothing |
| Max datagram | 1200 bytes; bigger ones are dropped unread |
| Room size | 64 players |
| Interest radius | 300 m, left at 340 m |
| Idle timeout | 10 s without any datagram |

## Versioning

Bump `PROTOCOL_VERSION` (Rust and TS) on any incompatible change. The server drops inputs with another version. The client refuses a `welcome` with another version and asks for a reload. `WORLD_VERSION` versions the world file separately.

## Debugging with a bad network

The server can simulate network conditions for every session, in both directions:

```sh
RAGNAROK_SIM_LATENCY_MS=60 RAGNAROK_SIM_JITTER_MS=30 RAGNAROK_SIM_LOSS=0.05 cargo run -p server
```

Each value applies one way, so RTT grows by about twice the latency. Jitter is uniform in `[0, jitter)` and reorders datagrams. Reliable streams are not delayed.
