# Ragnarok

A browser multiplayer game: goofy stickmen in a real city built from OpenStreetMap. There's a server-authoritative Rust game server (Salvo, WebTransport, Rapier) and a Vue 3 + three.js client with client-side prediction and snapshot interpolation.

- [design.md](design.md): architecture, netcode, world pipeline and roadmap
- [NETWORKING.md](NETWORKING.md): wire protocol

## Requirements

- Rust 1.85+ (edition 2024)
- Node 22.18+ or 24.12+
- A browser with WebTransport: Chrome, Edge or Firefox

## Run locally

```sh
# 1. World (already committed; regenerate to change the area)
cargo run -p osm-import                       # default ~600 m block of Södermalm, Stockholm
cargo run -p osm-import -- --bbox 59.3290,18.0600,59.3345,18.0705   # any south,west,north,east
cargo run -p osm-import -- --synthetic        # offline grid city

# 2. Server (WebTransport on 127.0.0.1:8698, cert hash on :8699)
cargo run -p server

# 3. Client
cd client && npm install && npm run dev       # http://localhost:5173
```

Open http://localhost:5173 in two windows. Click to capture the mouse. Controls: WASD to move, Shift to run, Space to jump, Esc to release the mouse.

Restart the server after regenerating the world. The page warns if its world differs from the server's.

## Test

```sh
cargo test --workspace          # protocol fixtures, jitter buffer, physics, game loop, importer
cd client && npm test           # codec fixtures, interpolation, clock, prediction, WASM-vs-native physics
cd client && npm run type-check && npm run lint
```

To try the netcode under a bad network:

```sh
RAGNAROK_SIM_LATENCY_MS=60 RAGNAROK_SIM_JITTER_MS=30 RAGNAROK_SIM_LOSS=0.05 cargo run -p server
```

The HUD shows the RTT, the interpolation delay, the last prediction correction and the count of unacknowledged inputs.

## Production

Set `RAGNAROK_DOMAIN` to serve the built client (`RAGNAROK_DIST`, default `client/dist`) over HTTPS with an ACME certificate on 443/80, with WebTransport on UDP 443. `RAGNAROK_ACME_CACHE` sets the certificate cache directory, and `RAGNAROK_ACME_STAGING` switches to Let's Encrypt staging. `RAGNAROK_WORLD` overrides the world file path.

```sh
(cd client && npm run build)
RAGNAROK_DOMAIN=game.example.com cargo run --release -p server
```

The first run issues the certificate and exits; start it again to load it.
