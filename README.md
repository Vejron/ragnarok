# Ragnarok

A browser multiplayer game: goofy stickmen in a real city built from OpenStreetMap. There's a server-authoritative Rust game server (Salvo, WebTransport, Rapier) and a Vue 3 + WebGPU client (drawn by the [engine](engine/README.md)) with client-side prediction and snapshot interpolation.

- [design.md](design.md): architecture, netcode, world pipeline and roadmap
- [WORLD.md](WORLD.md): generating the world: data sources, credentials, download cache
- [NETWORKING.md](NETWORKING.md): wire protocol
- [SERVER-PERFORMANCE.md](SERVER-PERFORMANCE.md): what a full room costs per tick, how it is measured, what is and is not worth optimising
- [RENDERING.md](RENDERING.md): the client renderer: toon shading, light, effects, contact shadows, ink, far shadows
- [engine/README.md](engine/README.md): the WebGPU render engine the client draws with, a package of its own
- [VEHICLES-I.md](VEHICLES-I.md): how roadmap stage 2 (cars, seats, driving) was built
- [ARMOR.md](ARMOR.md): armor zones, ricochets, weak spots, turret loss and the Strv S
- [VEHICLES-II.md](VEHICLES-II.md): the tank, its gunner seat and the shell, driver prediction and the helicopter
- [VEHICLES-III.md](VEHICLES-III.md): handling, tyre effects, crash damage and vehicle sound
- [VEHICLES-IV.md](VEHICLES-IV.md): the Fokker triplane: flight, guns, bombs, model and sound

## Requirements

- Rust 1.85+ (edition 2024)
- Node 22.18+ or 24.12+
- A browser with WebTransport: Chrome, Edge or Firefox

## Run locally

```sh
# 1. World (generated, not committed; scripts/world.sh does the same as the first lines here)
cargo run -p osm-import                       # default: 8.6 km square of central Umeå, Backen to Nydala, in 250 m tiles
cargo run -p osm-import -- --center 59.3167,18.0613 --half 600   # any lat,lon and half-side in metres
cargo run -p osm-import -- --bbox 59.3290,18.0600,59.3345,18.0705   # any south,west,north,east
cargo run -p osm-import -- --synthetic        # offline grid city, with hills
# Terrain, water and roofs come from Lantmäteriet (CC BY 4.0; water needs the Hydrografi Direkt
# service enabled), trees from Umeå's open data (CC0). `--osm-water` takes the water from OpenStreetMap instead.
# Lantmäteriet needs a Geotorget account: put LM_USER and LM_PASS in .env.local (git-ignored) and
# load it. Downloads are cached in .cache/osm-import (--refresh, --clear-cache). See WORLD.md.
set -a; . ./.env.local; set +a
cargo run -p osm-import                       # terrain, roofs and trees
cargo run -p osm-import -- --dem m708_75.tif  # or from a DTM GeoTIFF you already have
cargo run -p osm-import -- --cell 0           # flat ground, no elevation (--cell is the sample spacing, 5 m)

# 1b. Or the small dev world (~7.6 km² around Mariehem, much faster; written to client/public/dev/)
scripts/world-dev.sh                          # --synthetic works here too; see WORLD.md, "Dev world"

# 2. Server (WebTransport on 127.0.0.1:8698, cert hash on :8699)
cargo run -p server

# 3. Client
cd client && npm install && npm run dev       # http://localhost:5173
```

Against the dev world, point both at it (unset both to go back to the big world):

```sh
RAGNAROK_WORLD=client/public/dev/world.msgpack cargo run -p server
cd client && VITE_WORLD_BASE=/dev/ npm run dev
```

Open http://localhost:5173 in two windows. Click to capture the mouse. Controls: WASD to move, Shift to run, Space to jump, Esc to release the mouse. Walk up to a car and press E to get in (the first to board drives, the next ones ride along) and E again to get out. Driving: W/S gas and brake, A/D steer, Space handbrake. Two tanks are parked among the cars: the first in drives (A/D turns it on the spot), the second takes the gun. The gunner aims with the mouse and fires with a click or F. On foot you start with empty hands: walk up to an item on the ground (revolver, AK5, grenade, bandage) and press E to take it. 1-5 or the mouse wheel pick a hotbar slot, a click (or F) uses what is in hand, R reloads; Tab shows the scores. It is a free-for-all: you die at zero health and respawn after three seconds. Grenades and tank shells also wreck vehicles. One helicopter is parked there too: its pilot leans with W/S, turns with A/D, climbs with Space and sinks with Shift, and one passenger takes the nose autocannon, aiming with the mouse and firing explosive rounds with a click or F. R swaps seats in vehicles, tank or helicopter. Two Fokker triplanes stand at the end of long straight roads: the pilot works the throttle with Space (more) and Shift (less, and the brake on the ground), steers with the mouse (the plane banks and turns towards where the view points, and a yellow cross shows where the nose is), and can take pitch and roll over by hand with W (nose down), S (nose up) and A/D while they are held; a click or F fires the two forward guns and C drops a bomb. The second aboard works the gun on the ring behind.

The world is `client/public/world.msgpack` plus `client/public/tiles/`, git-ignored and generated by step 1 (`scripts/world.sh --synthetic` is the offline one, and the tests use the small committed `fixtures/world/`): the server reads all of it at start and the browser streams the tiles around the player. Restart the server after regenerating the world. The page warns if its world differs from the server's.

**Map editor:** `cargo run -p editor`, then open `/editor.html` on the Vite dev server to paint which 250 m tiles of the map make up a world, see which of them are cached, and keep several worlds to switch between (see `EDITOR.md` and `WORLD.md`).

## Test

```sh
cargo test --workspace          # protocol fixtures, deltas and interest, jitter buffer, physics, vehicles, game loop, importer
cd client && npm test           # codec fixtures, delta replica, tile streaming, instanced crowd, interpolation, clock, prediction, WASM-vs-native physics
cd client && npm run type-check && npm run lint
```

To try the netcode under a bad network:

```sh
RAGNAROK_SIM_LATENCY_MS=60 RAGNAROK_SIM_JITTER_MS=30 RAGNAROK_SIM_LOSS=0.05 cargo run -p server
```

The HUD shows the RTT, the players in view, the map tiles loaded, the interpolation delay, the last prediction correction and the count of unacknowledged inputs.

## Production

Set `RAGNAROK_DOMAIN` to serve the built client (`RAGNAROK_DIST`, default `client/dist`) over HTTPS with an ACME certificate on 443/80, with WebTransport on UDP 443. `RAGNAROK_ACME_CACHE` sets the certificate cache directory, and `RAGNAROK_ACME_STAGING` switches to Let's Encrypt staging. `RAGNAROK_WORLD` overrides the manifest's path; the server reads the tiles from the `tiles/` directory next to it.

```sh
(cd client && npm run build)
RAGNAROK_DOMAIN=game.example.com cargo run --release -p server
```

The first run issues the certificate and exits; start it again to load it.

### Deploying

`scripts/deploy.sh` builds the client and a `linux/amd64` image locally, ships it over SSH and swaps the container on the VPS, rolling back if the health check fails. The deploy target is kept in `.env.local` (git-ignored), which defines:

- `SSH_USER` and `SSH_HOST`: the SSH login of the VPS.
- `TARGET_DOMAIN`: the public hostname, passed to the script as `DOMAIN`.

```sh
set -a; . ./.env.local; set +a
DOMAIN="$TARGET_DOMAIN" scripts/deploy.sh "$SSH_USER@$SSH_HOST"
```

Commit first: the image is tagged with the current `HEAD`. A protocol bump shows players with the old page open the "reload" banner once the new container is up.
