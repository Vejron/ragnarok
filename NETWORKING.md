# Networking

This is the wire protocol between the browser client and the game server. Rust is the source of truth (`crates/shared/src/protocol.rs`, `consts.rs`, `quant.rs`). `client/src/net/codec.ts` mirrors it. Byte-for-byte fixtures in `protocol.rs` and `client/src/__tests__/codec.spec.ts` must change together.

[design.md §5](design.md#5-netcode) explains how the pieces fit together: prediction, the jitter buffer and interpolation.

## Session

1. **Connect:** the client opens WebTransport to `/transport`.
   - Development: `https://127.0.0.1:8698/transport`. The browser pins the self-signed certificate through `serverCertificateHashes`. The client fetches the hash from `/local-certificate-hash`, which Vite proxies to the server's `:8699/certificate-hash`.
   - Production: `https://<host>/transport` with an ACME certificate. UDP 443 carries HTTP/3.
2. **Origin check:** the server rejects any other origin. In production only `https://$RAGNAROK_DOMAIN` is allowed, and in development only `http://localhost:5173` and `http://127.0.0.1:5173`.
3. **Admission:** at 32 players the room is full and the session is refused.
4. **Welcome:** the server sends `welcome` on a unidirectional stream. The client ignores snapshots until it has one.
5. **Play:** the client sends `input` datagrams every tick and a `ping` every second. The server sends a `snapshot` datagram every 2 ticks.
6. **End:** the session ends on close, error, or after 10 s without any datagram. The server then removes the player and sends `leave` to everyone else.

## Messages

All messages are MessagePack arrays whose first element is a tag. The client→server and server→client tag spaces are separate.

### Client → server (datagrams)

| Tag | Name | Shape | Notes |
|---|---|---|---|
| 0 | `input` | `[0, version, [frame, …]]` | 1–4 frames, oldest first. Throttled to one per 8 ms. |
| 1 | `ping` | `[1, nonce u32]` | Echoed as `pong`. Throttled to one per 100 ms. |

`frame` is `[seq u32, strafe i8, forward i8, yaw i16, buttons u8]`:

| Field | Unit | Range |
|---|---|---|
| `seq` | per-connection counter, one per client tick | starts at 1 |
| `strafe` | axis, + is right | −127..127 |
| `forward` | axis, + is forward | −127..127 |
| `yaw` | milliradians, 0 faces −Z | [−3142, 3141] |
| `buttons` | bits: 1 jump, 2 run, 4 use | |

Each datagram repeats the newest 4 frames, so a single lost datagram loses nothing. The server ignores frames it has already seen.

### Server → client

| Tag | Name | Channel | Shape |
|---|---|---|---|
| 0 | `welcome` | stream | `[0, version, entity_id u32, tick_rate u8, snapshot_every u8, world_hash u32]` |
| 1 | `snapshot` | datagram | `[1, server_tick u32, ack u32, [entity, …]]` |
| 2 | `pong` | datagram | `[2, nonce u32]` |
| 3 | `leave` | stream | `[3, entity_id u32]` |

- `ack` is the newest input `seq` of the receiving player already applied to this state. The client drops pending inputs up to `ack` and replays the rest.
- `world_hash` is FNV-1a over `world.msgpack`. A mismatch means the client loaded a different world than the server simulates.

`entity` is `[id u32, kind u8, x i32, y i32, z i32, yaw i16, vy i16, flags u8]`:

| Field | Unit |
|---|---|
| `kind` | 0 = stickman |
| `x, y, z` | centimetres, capsule centre (+X east, +Z south, +Y up) |
| `yaw` | milliradians |
| `vy` | vertical velocity, cm/s (needed to resume a predicted jump) |
| `flags` | bit 0: grounded |

## Rates and limits

| | Value |
|---|---|
| Simulation tick | 60 Hz |
| Snapshot rate | 30 Hz |
| Input datagrams | 60 Hz, about 35 bytes each, so about 2 KB/s up |
| Snapshot size | about 10 + 20 × players bytes; 32 players is about 650 bytes, so about 20 KB/s down |
| Max datagram | 1200 bytes; bigger ones are dropped unread |
| Room size | 32 players |
| Idle timeout | 10 s without any datagram |

## Versioning

Bump `PROTOCOL_VERSION` (Rust and TS) on any incompatible change. The server drops inputs with another version. The client refuses a `welcome` with another version and asks for a reload. `WORLD_VERSION` versions the world file separately.

## Debugging with a bad network

The server can simulate network conditions for every session, in both directions:

```sh
RAGNAROK_SIM_LATENCY_MS=60 RAGNAROK_SIM_JITTER_MS=30 RAGNAROK_SIM_LOSS=0.05 cargo run -p server
```

Each value applies one way, so RTT grows by about twice the latency. Jitter is uniform in `[0, jitter)` and reorders datagrams. Reliable streams are not delayed.
