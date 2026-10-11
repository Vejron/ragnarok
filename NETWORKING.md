# Networking

This is the wire protocol between the browser client and the game server. Rust is the source of truth (`crates/shared/src/protocol.rs`, `consts.rs`, `quant.rs`). `client/src/net/codec.ts` mirrors it. Byte-for-byte fixtures in `protocol.rs` and `client/src/__tests__/codec.spec.ts` must change together.

[design.md §5](design.md#5-netcode) explains how the pieces fit together: prediction, the jitter buffer and interpolation.

## Session

1. **Connect:** the client opens WebTransport to `/transport`.
   - Development: `https://127.0.0.1:8698/transport`. The browser pins the self-signed certificate through `serverCertificateHashes`. The client fetches the hash from `/local-certificate-hash`, which Vite proxies to the server's `:8699/certificate-hash`.
   - Production: `https://<host>/transport` with an ACME certificate. UDP 443 carries HTTP/3.
2. **Origin check:** the server rejects any other origin. In production only `https://$RAGNAROK_DOMAIN` is allowed, and in development only `http://localhost:5173` and `http://127.0.0.1:5173`.
3. **Admission:** at 64 players the room is full and the session is refused.
4. **Welcome:** the server sends `welcome` on a unidirectional stream. The client ignores snapshots until it has one.
5. **Play:** the client sends `input` datagrams every tick and a `ping` every second. The server sends a `snapshot` datagram every 2 ticks, holding what changed near that player since the snapshot they last acknowledged (below).
6. **End:** the session ends on close, error, or after 10 s without any datagram. The server then removes the player and sends `leave` to everyone else.

## Messages

All messages are MessagePack arrays whose first element is a tag. The client→server and server→client tag spaces are separate.

### Client → server (datagrams)

| Tag | Name | Shape | Notes |
|---|---|---|---|
| 0 | `input` | `[0, version, baseline u32, [frame, …]]` | 1–4 frames, oldest first. Throttled to one per 8 ms. `baseline` is the `server_tick` of the newest snapshot the client has rebuilt, or 0 for none |
| 1 | `ping` | `[1, nonce u32]` | Echoed as `pong`. Throttled to one per 100 ms. |

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

While driving, `forward` is the throttle (negative brakes, then reverses), `strafe` is the steering, `buttons` bit 1 (jump) is the handbrake, and `yaw` is ignored. A helicopter's pilot (seat 0) uses `forward` to lean forward and back, `strafe` to turn, jump to climb and run (bit 2) to sink. A passenger's axes are ignored. A tank's driver steers with `strafe`, which turns the tank on the spot, and its gunner (seat 1) aims with `yaw` and `pitch` and shoots with the fire bit. The gunner's `yaw` is a world heading: the turret turns to it at a limited rate, whatever the hull does.

Each datagram repeats the newest 4 frames, so a single lost datagram loses nothing. The server ignores frames it has already seen.

### Server → client

| Tag | Name | Channel | Shape |
|---|---|---|---|
| 0 | `welcome` | stream | `[0, version, entity_id u32, tick_rate u8, snapshot_every u8, world_hash u32]` |
| 1 | `snapshot` | datagram | `[1, server_tick u32, baseline u32, ack u32, ride u32, [entity, …], [entity_delta, …], [vehicle, …], [vehicle_delta, …], [removed u32, …], drive, self, [shot, …], [projectile, …], [pickup, …]]` |
| 2 | `pong` | datagram | `[2, nonce u32]` |
| 3 | `leave` | stream | `[3, entity_id u32]` |
| 4 | `kill` | stream | `[4, killer u32, victim u32, weapon u8, killer_kills u16, victim_deaths u16]` |
| 5 | `scores` | stream | `[5, [[id u32, kills u16, deaths u16], …]]` |

- `baseline` is the `server_tick` of the snapshot this one is a change from, or 0 when it is not. Then every entity and vehicle in range is listed in full, in `[entity, …]` and `[vehicle, …]`. See [Delta snapshots](#delta-snapshots-and-interest).
- `ack` is the newest input `seq` of the receiving player already applied to this state. The client drops pending inputs up to `ack` and replays the rest.
- `ride` is the id of the vehicle the receiving player stands on top of, or 0. The server carries riders with the car, so the client predicts them in the car's frame. Only the receiver's own value is sent: other riders are plain entities whose positions already move with the car.
- `drive` is `nil` unless the receiving player sits in a vehicle's driver seat. Then it is the exact motion of that vehicle, which its pose does not carry, so the driver can predict it (below).
- `self` is `[hp u8, progress u8, [slot, …]]`, only for the receiving player. `hp` is their health, 0 when dead (other players only show dead or alive, in `flags`). `progress` is how far a reload or a bandage has got, 0..255, or 0 when neither is going on. Each `slot` is `[item u8, count u8, ammo u8]`, five of them, `item` 0 for an empty slot (`1` revolver, `2` AK5, `3` grenade, `4` bandage). The item in another player's hands is in bits 3..5 of their entity `flags`.
- `shot` is `[shooter u32, weapon u8, x i32, y i32, z i32]`: something since the previous snapshot, and where it ended (cm). It is only sent to a player within 300 m of where it ended. Weapons 0 (AK5) and 4 (revolver) are gunshots (a tracer from the shooter), 1 (a tank shell), 6 (a helicopter autocannon round), 2 a rocket, 5 a grenade and 3 a vehicle wreck going up are a burst at the point. It is cosmetic, so it rides the lossy datagram and is capped at 8 per snapshot, and dropped first if a snapshot would not fit.
- `projectile` is `[id u32, kind u8, x i32, y i32, z i32, vx i16, vy i16, vz i16]`, cm and cm/s: a rocket (`kind` 0) or grenade (`kind` 1) in flight. Clients interpolate it like an entity and aim a rocket along `v`. One that stops being listed has burst, and its burst arrives as a `shot` event. If a snapshot would not fit, shots are dropped first, then projectiles, then pickups.
- `pickup` is `[id u32, item u8, x i32, y i32, z i32]`, cm: an item lying on the ground within 300 m, nearest first and at most 24. They are static, so there is no delta: the list is the set, and one that is no longer listed has been taken. A player takes the nearest within 2 m by sending the use bit; the server hands it over before it considers boarding a vehicle.
- `kill` goes to everyone when a player dies. `killer` is 0 when the world did it (a fall). The totals are the new ones, so clients keep no tally of their own. `scores` is sent once to a player who has just joined.
- `world_hash` is FNV-1a over `world.msgpack`. A mismatch means the client loaded a different world than the server simulates.

`entity` is `[id u32, kind u8, x i32, y i32, z i32, yaw i16, vy i16, flags u8, knock_x i16, knock_z i16]`:

| Field | Unit |
|---|---|
| `kind` | 0 = stickman |
| `x, y, z` | centimetres, capsule centre (+X east, +Z south, +Y up) |
| `yaw` | milliradians |
| `vy` | vertical velocity, cm/s (needed to resume a predicted jump) |
| `flags` | bit 0: grounded, bit 1: dead (lying where it fell until it respawns) |
| `knock_x, knock_z` | knockback velocity, cm/s: the push a car gave the player, still sliding them. Needed to resume a predicted slide |

Players sitting in a vehicle are **not** listed as entities. The vehicle's `seats` say where they are.

`vehicle` is `[id u32, kind u8, x i32, y i32, z i32, qx i16, qy i16, qz i16, steer i8, [seat u32, …], turret]`:

| Field | Unit |
|---|---|
| `id` | shares the id space with players |
| `kind` | 1 = jeep, 2 = tank, 3 = helicopter, 4 = sports car |
| `x, y, z` | centimetres, chassis centre |
| `qx, qy, qz` | the x, y, z of the orientation quaternion, scaled by 32767. The sender negates the quaternion so that `w >= 0`, and the receiver rebuilds `w = sqrt(1 − x² − y² − z²)` |
| `steer` | front-wheel angle as a fraction of full lock (the kind's `*_MAX_STEER`), −127..127, + is right |
| `seat` | the player id in each seat, 0 when empty. Seat 0 is the driver. A jeep has 4 seats and a sports car 2; a tank has 2, the second being the gunner; a helicopter has 2, the pilot and a passenger |
| `turret` | `nil` for a vehicle without a gun, else `[yaw i16, pitch i16, shots u8, range u16]` (below) |

`turret` is `[yaw, pitch, shots, range]`:

| Field | Unit |
|---|---|
| `yaw` | milliradians, turret angle relative to the hull, + turns left |
| `pitch` | milliradians, barrel elevation, + is up |
| `shots` | shells fired so far, modulo 256. A client sees a shot as a change in this counter, and draws the flash from the barrel it is drawing |
| `range` | decimetres: how far the last shell flew before it hit something (3000 means it hit nothing) |

`drive` is `[vx i16, vy i16, vz i16, wx i16, wy i16, wz i16, steer i16, spin i16]`:

| Field | Unit |
|---|---|
| `vx, vy, vz` | linear velocity of the chassis, cm/s (world axes) |
| `wx, wy, wz` | angular velocity, milliradians per second (world axes), saturating |
| `steer` | front-wheel angle in milliradians, + is right. Not the `steer` fraction of the vehicle, which is too coarse to resume from. Zero for a tank and a helicopter |
| `spin` | the yaw rate a tank has ramped to, milliradians per second, + is clockwise. Zero for a helicopter, whose yaw rate is in `wy` |

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

`vehicle_delta` is `[id u32, dx i32, dy i32, dz i32, dqx i32, dqy i32, dqz i32, dsteer i32, seats, turret, hp u8]`. The first eight are differences of the `vehicle` fields of the same name. `seats` is `nil` when unchanged, else the new list. `turret` is `nil` when unchanged, else the new `[yaw, pitch, shots, range]`. `hp` is the new value.

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
