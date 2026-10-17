# Networking

This is the wire protocol between the browser client and the game server. Rust is the source of truth (`crates/shared/src/protocol.rs`, `consts.rs`, `quant.rs`). `client/src/net/codec.ts` mirrors it. Byte-for-byte fixtures in `protocol.rs` and `client/src/__tests__/codec.spec.ts` must change together.

[design.md §5](design.md#5-netcode) explains how the pieces fit together: prediction, the jitter buffer and interpolation.

## Session

1. **Connect:** the client opens WebTransport to `/transport`, with the player's settings from `/settings.html` as query parameters: `?name=Åsa&color=3&spawn=x,z&spawn=x,z`. All are optional. `name` is cut to 16 characters with control and direction-changing characters removed, and an empty one becomes `Player <id>`. `color` indexes the 12 outfit colours (`PLAYER_COLORS` in `client/src/profile.ts`). Each `spawn` is a point in centimetres in the world's local frame; the server keeps up to 5, drops any outside the world, in water or within 1 m of a building, and uses the rest in turn, one per spawn. Unknown parameters are ignored, so later settings (a class, a loadout) can be added the same way.
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

**Reliable stream.** The server opens one unidirectional stream per session, when it has its first reliable message (`welcome`), and writes every reliable message to it, in order. Each message is a 4-byte big-endian length and then that many bytes of MessagePack. Messages therefore arrive in the order they were sent: a `kill` never overtakes the `leave` of its victim, and the joiner's `profiles`, `scores`, `destroyed` and `damaged` catch-up come right behind the `welcome`. The client refuses a length over 4 MiB and drops the session on a malformed stream. The server drops a session whose stream accepts nothing for 5 s, or whose outgoing queue (64 messages, shared with snapshots) is full when a reliable message is due, rather than let it miss one: it reconnects and gets the catch-up.


| Tag | Name | Channel | Shape |
|---|---|---|---|
| 0 | `welcome` | reliable stream | `[0, version, entity_id u32, tick_rate u8, snapshot_every u8, world_hash u32]` |
| 1 | `snapshot` | datagram | `[1, server_tick u32, baseline u32, ack u32, ride u32, [entity, …], [entity_delta, …], [vehicle, …], [vehicle_delta, …], [removed u32, …], drive, self, [shot, …], [projectile, …], [pickup, …], [bullet, …]]` |
| 2 | `pong` | datagram | `[2, nonce u32]` |
| 3 | `leave` | reliable stream | `[3, entity_id u32]` |
| 4 | `kill` | reliable stream | `[4, killer u32, victim u32, weapon u8, killer_kills u16, victim_deaths u16, style u8, yaw i16 mrad, power u8 (0.2 m/s)]`; `style` is `DEATH_*` (body, head, blast, roadkill, overkill) and `yaw`/`power` are the way and how hard the body is thrown, so every client stages the death alike |
| 5 | `scores` | reliable stream | `[5, [[id u32, kills u16, deaths u16], …]]` |
| 6 | `destroyed` | reliable stream | `[6, [[kind u8, col u16, row u16, index u32, yaw i16], …]]` |
| 7 | `damaged` | reliable stream | `[7, [[col u16, row u16, index u32, stage u8, marked u8, x i32, y i32, z i32], …]]` |
| 8 | `hit` | reliable stream | `[8, role u8, kind u8, amount u8, weapon u8, bearing i16, x i32, y i32, z i32, target u32]` |
| 9 | `rtc` | reliable stream | `[9, from u32, kind u8, json string]` |
| 10 | `carved` | reliable stream | `[10, live u8, [[col u16, row u16, index u32, x i32, y i32, z i32, [blasted u16, …], [fell u16, …]], …]]` |
| 11 | `profiles` | reliable stream | `[11, [[id u32, name string, color u8], …]]` |

- `baseline` is the `server_tick` of the snapshot this one is a change from, or 0 when it is not. Then every entity and vehicle in range is listed in full, in `[entity, …]` and `[vehicle, …]`. See [Delta snapshots](#delta-snapshots-and-interest).
- `ack` is the newest input `seq` of the receiving player already applied to this state. The client drops pending inputs up to `ack` and replays the rest.
- `ride` is the id of the vehicle the receiving player stands on top of, or 0. The server carries riders with the car, so the client predicts them in the car's frame. Only the receiver's own value is sent: other riders are plain entities whose positions already move with the car.
- `drive` is `nil` unless the receiving player sits in a vehicle's driver seat. Then it is the exact motion of that vehicle, which its pose does not carry, so the driver can predict it (below).
- `self` is `[hp u8, progress u8, [slot, …]]`, only for the receiving player. `hp` is their health, 0 when dead (other players only show dead or alive, in `flags`). `progress` is how far a reload or a bandage has got, 0..255, or 0 when neither is going on. Each `slot` is `[item u8, count u8, ammo u8]`, five of them, `item` 0 for an empty slot (`1` revolver, `2` AK5, `3` grenade, `4` bandage, `5` sniper, `6` golden egg, dropped by a rooster and healing to full, `7` shotgun, `8` FPV drone). The item in another player's hands is in bits 3..6 of their entity `flags`.
- `shot` is `[shooter u32, weapon u8, x i32, y i32, z i32]`: something since the previous snapshot, and where it ended (cm). It is only sent to a player within 300 m of where it ended. Weapon 13 (shotgun) is a muzzle blast with the sound, and each further pellet is 14, a spark where it landed; 15 is a flying bullet ending (its `shooter` is the bullet's id, see `bullet`). 16 is a shot glancing off a vehicle's armor (its `shooter` is the vehicle's id): sparks and a ping, no blast. 17 is a tank's turret being shot off (`shooter` is the tank): the turret flies away as debris. Weapons 0 (AK5) and 4 (revolver) are not sent as shots any more: their rounds are bullets. 1 (a tank shell), 6 (a helicopter autocannon round), 2 a rocket, 5 a grenade, 11 a plane's bomb, 19 an FPV drone's charge and 3 a vehicle wreck going up are a burst at the point. A drone shot down pops as a 6. 10 is a plane's machine-gun round that hit something, a spark at the point: every round's tracer is drawn by the clients from the gun's shot counter, so a round that hits nothing sends no event at all. It is cosmetic, so it rides the lossy datagram and is capped at 8 per snapshot, and dropped first if a snapshot would not fit.
- `projectile` is `[id u32, kind u8, x i32, y i32, z i32, vx i16, vy i16, vz i16]`, cm and cm/s: a rocket (`kind` 0), grenade (1), tank shell (2), autocannon round (3), sniper round (4) or plane's bomb (5) in flight. Clients interpolate it like an entity and aim a rocket along `v`. One that stops being listed has burst, and its burst arrives as a `shot` event. Gun bullets (kind 4) are not listed: see `bullet`. If a snapshot would not fit, shots are dropped first, then projectiles, then pickups, then bullets.
- `bullet` is `[id u32, shooter u32, item u8, x i32, y i32, z i32, vx i16, vy i16, vz i16, tick u32]`, cm and cm/s: a gun round that left the muzzle on server tick `tick`, sent once (in two consecutive snapshots, so a lost datagram does not lose it, and clients ignore an id they have seen) to every player within 300 m of its line out to its gun's range, nearest first, at most 32. It is not listed again: it moves `v*dt` a tick and then `v.y` loses `20 * 0.25 * dt`, so a client works out `p + v*n*dt - (0, 20*0.25*dt^2*n(n+1)/2, 0)` for `n` ticks after `tick`. It ends with a `shot` of weapon 15 and `id` in `shooter`, or when its gun's range is flown. Bullets are shed after pickups when a snapshot would not fit.
- `pickup` is `[id u32, item u8, x i32, y i32, z i32]`, cm: an item lying on the ground within 300 m, nearest first and at most 24. They are static, so there is no delta: the list is the set, and one that is no longer listed has been taken. A player takes the nearest within 2 m by sending the use bit; the server hands it over before it considers boarding a vehicle.
- `kill` goes to everyone when a player dies. `killer` is 0 when the world did it (a fall). The totals are the new ones, so clients keep no tally of their own. `scores` is sent once to a player who has just joined.
- `profiles` names players and gives their outfit colour. A player who has just joined gets everyone's, right after `welcome`; everyone else gets the joiner's. Snapshots can arrive before it, so a client draws a stickman in a colour chosen by id until its profile arrives.
- `destroyed` is the only world state that changes: buildings and trees that are gone. `kind` is 0 for a tree and 1 for a building; `(col, row, index)` names it by its tile and its position in that tile's list (`ObjId`), which both sides already agree on and which a re-import renumbers. `yaw` (milliradians, 0 faces -Z) is the way a tree falls, or the direction a building's blast came from (its rubble is thrown the other way). It goes to everyone when something falls, and a player who has just joined gets one message holding everything destroyed so far, before the first snapshot. Only the fact is replicated: the fall, the rubble and the dust are drawn by each client, and the trunk or the building's walls stop colliding on both sides at once. A building falls to rockets, grenades and shells (a rifle only chips it), and a tree to anything.
- `hit` is sent to one player only, once per tick per `(role, kind)` with `amount` (hp lost) summed. `role` 0 is *dealt* (you hurt something: kind 0 stickman, 1 vehicle, 2 a stickman this finished, 3 rooster) and 1 is *taken* (something hurt you, or the vehicle you sit in). For a taken hit `bearing` is the yaw in milliradians (0 faces -Z, as a look) from you towards the attacker or blast; -32768 means no direction (a fall, a crash). `weapon` is 0 for vehicle hits. `(x, y, z)` (cm) is where the thing hit is (chest height) and `target` is its id, for the damage number, debris and flash. It drives the hit marker, hit sound, screen flash and direction arcs; non-lethal damage otherwise reaches a victim only as a lower `hp`.
- `damaged` tells everyone that standing buildings are hurt, so they can look it. `stage` is 0 (whole) to 3 (about to collapse), entered at 66%, 33% and 15% of the building's health. An entry is sent when a building enters a worse stage, or when a blast of at least 5 damage hits it; then `marked` is 1 and `(x, y, z)` (cm) is where the blast landed (clients no longer use it: charring comes from the removed cells). Rifle chips send nothing until they cross into a worse stage. A player who has just joined gets one message with the stage of every hurt building (never marked). The dust a building about to fall sheds is drawn by each client, and the burn around a hole is worked out from the `carved` cells; the holes themselves come from `carved`. Trees are not reported.
- `carved` tells everyone which cells a blast took out of a standing building. A building is cut into a grid of cells (`crates/shared/src/carve.rs`, mirrored in `client/src/sim/carve.ts`: 1 m cells, bigger for a building that would otherwise pass 6 000 small triangles or 60 000 cells), a cell being named by `(y * dims.z + z) * dims.x + x` over the building's bounding box. Both sides cut the walls and roof into the same small triangles with integer arithmetic on centimetres and give each the cell of its centroid, so a cell number means the same triangles to both (pinned by `cell_numbers_match_the_fixture` and `carve.spec.ts`). `(x, y, z)` (cm) is where the blast landed; `blasted` are the cells within its reach, roughened per cell so the edge is ragged, and `fell` the cells that were left with no chain of cells down to the building's lowest layer. Rockets, grenades and shells bite; a rifle only chips health. Only the cell numbers are replicated: the hole, the thrown pieces and the dust are drawn by each client, and the walls stop colliding in the removed cells on both sides at once (the server swaps the building's trimesh for the remaining small triangles plus the floor; the client does the same with the tile's building mesh). `live` is 1 for a bite that just happened and 0 for the catch-up a joiner gets after `welcome`, which lists every removed cell under `blasted` and animates nothing. A building left with under 30% of its cells collapses as in `destroyed`, which follows in the same tick.
- `rtc` relays proximity voice signaling. The server writes `from` itself (the sender's entity id), so it cannot be forged, and drops it silently if the receiver's queue is full: WebRTC retries. The voice itself goes peer to peer between players within about 40 m (up to 8 connections, opened at 35 m and closed at 50 m); the lower entity id sends the offer. ICE uses STUN, plus TURN from `VITE_ICE_SERVERS` if set.
- `world_hash` is FNV-1a over `world.msgpack`. A mismatch means the client loaded a different world than the server simulates.

`entity` is `[id u32, kind u8, x i32, y i32, z i32, yaw i16, vy i16, flags u8, knock_x i16, knock_z i16]`:

| Field | Unit |
|---|---|
| `kind` | 0 = stickman, 5 = Hedemora rooster (an NPC: `yaw` is its heading, `flags` bit 7 is crowing), 12 = the Orange Plague (below) |
| `x, y, z` | centimetres, capsule centre (+X east, +Z south, +Y up) |
| `yaw` | milliradians |
| `vy` | vertical velocity, cm/s (needed to resume a predicted jump) |
| `flags` | bit 0: grounded, bit 1: dead (lying where it fell until it respawns), bit 2: crouching, bits 3..6: the item in hand (`ITEM_*`, 0 for none), bit 7: crowing (a rooster) |
| `knock_x, knock_z` | knockback velocity, cm/s: the push a car gave the player, still sliding them. Needed to resume a predicted slide |

Players sitting in a vehicle are **not** listed as entities. The vehicle's `seats` say where they are.

The Orange Plague (kind 12, a 40 m boss) is sent to everyone wherever they are, and some fields mean something else for him: bits 3..6 of `flags` are his mode (`PLAGUE_*`: walk, stomp, strike, recover) and bit 1 means he is toppling; `stance` is how far through that mode he is, 0..255 (a walk's place in its two-stride cycle, otherwise how much is done), from which both sides pose his skeleton and so his hitboxes (`shared::plague`, `client/src/sim/plague-body.ts`); `vy` is his health in tenths of a percent plus 1024 times the `FACE_*` bits of what of his face has been shot out (eyes, ears, mouth), and is not interpolated; `knock_x, knock_z` are where his wind-up lands, relative to his feet, in decimetres. What he says is a reliable `[12, trigger, variant, subject]` bark.

`vehicle` is `[id u32, kind u8, x i32, y i32, z i32, qx i16, qy i16, qz i16, steer i8, [seat u32, …], turret, hp u8, damage, arms, thrust]`:

| Field | Unit |
|---|---|
| `id` | shares the id space with players |
| `kind` | 1 = jeep, 2 = tank, 3 = helicopter, 4 = sports car, 6 = plane, 7 = walker, 8 = Strv S, 9 = field mortar, 10 = Roadster, 11 = FPV drone (5 is the rooster, an entity). A drone is listed only while it flies or falls: see [DRONES.md](DRONES.md) |
| `x, y, z` | centimetres, chassis centre |
| `qx, qy, qz` | the x, y, z of the orientation quaternion, scaled by 32767. The sender negates the quaternion so that `w >= 0`, and the receiver rebuilds `w = sqrt(1 − x² − y² − z²)` |
| `steer` | front-wheel angle as a fraction of full lock (the kind's `*_MAX_STEER`), −127..127, + is right |
| `seat` | the player id in each seat, 0 when empty. Seat 0 is the driver. A jeep has 4 seats, and a sports car and a Roadster 2; a tank has 2, the second being the gunner; a helicopter has 2, the pilot and the nose gunner; a plane has 2, the pilot (who fires the forward guns and drops the bombs) and the rear gunner; a walker and a Strv S have 1, the driver, who also fires (the Strv S's gun is fixed in the hull, so its `turret.yaw` is always 0 and only `pitch` moves) |
| `turret` | `nil` for a vehicle without a gun, else `[yaw i16, pitch i16, shots u8, range u16, flags u8]` (below). A plane's is the rear gunner's ring |
| `hp` | health, 0 is a burning wreck nobody can board |
| `damage` | `nil` while unhurt, else eight bytes: front, rear, left, right, then wheels FL, FR, RL, RR, each 0..255 (a wheel at 255 is gone). A tank has no wheels to hurt, so its byte 4 (`DAMAGE_TURRET`) is 255 (`TURRET_GONE`) once its turret has been shot off |
| `arms` | `nil` for anything but a plane, else `[shots u8, heat u8, rear_heat u8, bombs u8, flags u8]` (below) |
| `thrust` | `nil` for anything but a Roadster or a drone, else `[air u8, flags u8]`: the air left in its tank, 0..255 of full, and the nozzles firing (bit 0 the rear boost nozzles, bit 1 the lift nozzles). Other clients draw the jets from `flags`; the driver's prediction resumes from `air`. A drone sends its battery as `air` and sets bit 2 while its pilot still flies it |

`turret` is `[yaw, pitch, shots, range, flags]`:

| Field | Unit |
|---|---|
| `yaw` | milliradians, turret angle relative to the hull, + turns left |
| `pitch` | milliradians, barrel elevation, + is up |
| `shots` | shells fired so far, modulo 256. A client sees a shot as a change in this counter, and draws the flash from the barrel it is drawing |
| `range` | decimetres: how far the last shell flew before it hit something (3000 means it hit nothing). A field mortar's is its range dial |
| `flags` | `TURRET_*`: bit 0 (`TURRET_STOWED`) is set while a field mortar is hitched to a car, and clients draw it folded for travel |

`arms` is a plane's pilot weapons and engine, `[shots, heat, rear_heat, bombs, flags]`:

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

The client learns which seat it occupies from this list. There is no separate seat event, so the seat state can never disagree with the snapshot stream it is drawn from.

Boarding and leaving use the `use` button bit (4) in an input frame. It is one-shot, like fire: the server ignores both on repeated frames.

## Delta snapshots and interest

A snapshot is built per player from what is within `INTEREST_RADIUS` (300 m on the ground plane) of their stickman, and then reduced to the change from a baseline. [design.md §5.8](design.md#58-interest-management-and-delta-snapshots) says why; this is the format.

| List | Contents |
|---|---|
| `[entity, …]` | Entities not in the baseline, in full (shape above) |
| `[entity_delta, …]` | Entities that were in it and changed |
| `[vehicle, …]` | Vehicles not in the baseline, in full |
| `[vehicle_delta, …]` | Vehicles that were in it and changed |
| `[removed, …]` | Ids, of players or vehicles, that were in the baseline and are no longer listed: out of range, left, or gone |

`entity_delta` is `[id u32, dx i32, dy i32, dz i32, dyaw i32, dvy i32, flags u8, dknock_x i32, dknock_z i32]`. Each field but `id` and `flags` is the new value minus the baseline's, in the same units as `entity`. `flags` is the new value. A change in the same entity's `kind` never happens.

`vehicle_delta` is `[id u32, dx i32, dy i32, dz i32, dqx i32, dqy i32, dqz i32, dsteer i32, seats, turret, hp u8, damage, arms, thrust]`. The first eight are differences of the `vehicle` fields of the same name. `seats` is `nil` when unchanged, else the new list. `turret` is `nil` when unchanged, else the new `[yaw, pitch, shots, range, flags]`. `hp` is the new value. `damage` is `nil` when unchanged, else the new eight bytes (all zeros is a repair). `arms` is `nil` when unchanged, else the new five bytes. `thrust` is `nil` when unchanged, else the new two bytes.

The client's scene after a snapshot is:

1. Start from the scene it rebuilt for snapshot `baseline` (empty if `baseline` is 0).
2. Remove every id in `removed`.
3. Add the new entities and vehicles.
4. Apply each delta to the entity or vehicle of that id. A delta for an id the scene does not hold is ignored.

Anything not mentioned is unchanged. A client that does not hold `baseline` drops the snapshot. Keep at least the last 64 rebuilt scenes. The server uses only baselines among the last 32 snapshots it sent that player, so one it still offers is one the client still has.

**The server diffs against the newest `baseline` the client acknowledged**, not against the snapshot it sent last. Loss then costs nothing but a larger next delta, and nothing has to be retransmitted. A baseline the server no longer holds (an acknowledgement over about a second old, or one that was never sent) gets a whole snapshot with `baseline` 0.

**Interest has hysteresis.** A thing enters at 300 m and stays until 340 m. **The packet limit** is kept by shedding, in order, effects, rockets, then the farthest new and changed entities and vehicles; the shed ones come in a later snapshot. `removed` is never shed.

## Rates and limits

| | Value |
|---|---|
| Simulation tick | 60 Hz |
| Snapshot rate | 30 Hz |
| Input datagrams | 60 Hz, about 35 bytes each, so about 2 KB/s up |
| Snapshot size | about 28 bytes of header, plus about 24 per entity or 10 per entity delta, 25 to 40 per vehicle or about 15 per vehicle delta, and 20 more for the driver. Only what is in range and has changed is sent: 30 players running in view is about 320 bytes, so about 10 KB/s down. Standing still costs nothing |
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
