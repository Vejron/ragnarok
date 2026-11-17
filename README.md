# Ragnarok

A browser multiplayer game: goofy soldiers in a real city built from OpenStreetMap. There's a server-authoritative Rust game server (Salvo, WebTransport, Rapier) and a Vue 3 + WebGPU client (drawn by the [engine](engine/README.md)) with client-side prediction and snapshot interpolation.

The docs are in [docs/](docs/).

**How it works**

- [design.md](docs/design.md): architecture, netcode, world pipeline and roadmap
- [NETWORKING.md](docs/NETWORKING.md): wire protocol
- [RENDERING.md](docs/RENDERING.md): the client renderer: toon shading, light, effects, contact shadows, ink, far shadows
- [GPU-PERFORMANCE.md](docs/GPU-PERFORMANCE.md): the GPU's cost on Retina screens: a benchmark, the multisampled image kept in the tile, a depth pre-pass for tree impostors, a lower pixel ratio with sharpening, a sky panorama for the water to mirror, and solid trees further out with impostors that look like them
- [ATMOSPHERE.md](docs/ATMOSPHERE.md): the sky: a physical atmosphere with sunsets, the sun, a moon with phases, stars, drifting clouds and their shadows, sky-coloured fog and dawn mist, god rays, lens flare, eye adaptation and grading by the hour, lamp beams in the night air, and weather from the server's clock: rain, wet ground and puddles, lightning and rainbows
- [engine/README.md](engine/README.md): the WebGPU render engine the client draws with, a package of its own
- [SERVER-PERFORMANCE.md](docs/SERVER-PERFORMANCE.md): what a full room costs per tick, how it is measured, what is and is not worth optimising, and what shelling a big house costs server and client
- [TILE-STREAMING.md](docs/TILE-STREAMING.md): how the map reaches the browser
- [MEMORY.md](docs/MEMORY.md): what the client holds in memory and how to measure it: a tile's buildings welded and kept on the GPU only, the multisampled image given no memory, and the per-frame garbage

**Making worlds**

- [WORLD.md](docs/WORLD.md): generating the world: data sources, credentials, download cache, squared footprints and the catalogue of roof shapes (gable, hip, gambrel, cross-gabled wings, dormers)
- [FRANCE.md](docs/FRANCE.md): worlds in France from IGN LiDAR HD
- [EDITOR.md](docs/EDITOR.md): the map editor
- [SCENARIOS.md](docs/SCENARIOS.md): a guide to writing a world's `scenario.json` by hand, since there is no scenario editor

**How each feature was built**

- [BOTS.md](docs/BOTS.md): server-run enemy soldiers, tank crews, and scenarios with squads, triggers, waves, objectives and a win or a loss
- [ARMOR.md](docs/ARMOR.md): armor zones, ricochets, weak spots, turret loss and the Strv S
- [LIMBS.md](docs/LIMBS.md): limb loss and part health
- [KNOCKDOWN.md](docs/KNOCKDOWN.md): blasts, cars and ragdolls
- [GHOSTS.md](docs/GHOSTS.md): coming back from death as a flying shadow that takes shape again, never right beside an enemy
- [SWIMMING.md](docs/SWIMMING.md): swimming, diving where you look, holding your breath and drowning
- [ZOMBIES.md](docs/ZOMBIES.md): zombies that rise in hordes at night, shamble after you and claw, and can be shot apart limb by limb
- [BUILDING.md](docs/BUILDING.md): placing cubes with bricks
- [INTERIORS.md](docs/INTERIORS.md): floors and stairs inside every house big enough, worked out from its footprint, with its door open and its floors blasted like its walls
- [CRATERS.md](docs/CRATERS.md) and [SCORCH.md](docs/SCORCH.md): shells digging the ground, and the marks they leave
- [GUNSMOKE.md](docs/GUNSMOKE.md): a shell leaving the barrel: fire, a bank of smoke sized to the gun, brake jets, sparks, dust and a barrel that goes on smoking, with smoke that churns, leans with the wind and is lit by flashes and lamps; small arms smoke too, every gun has a shaped muzzle flare, and the sniper kicks down its scope
- [HULL-DAMAGE.md](docs/HULL-DAMAGE.md): what a round leaves on a vehicle: dents, soot, holes ringed with blued steel and gouges, drawn by the mesh's own shader, and the blow that rocks it, a Titan too
- [DRONES.md](docs/DRONES.md): FPV and loitering drones
- [GYRO-AIM.md](docs/GYRO-AIM.md): aiming on a phone by turning it
- [BACKPACK.md](docs/BACKPACK.md): backpack mode
- [VEHICLES-I.md](docs/VEHICLES-I.md): cars, seats, driving
- [VEHICLES-II.md](docs/VEHICLES-II.md): the tank, its gunner seat and the shell, driver prediction and the helicopter; how tracks ride over humps, steps and side slopes
- [VEHICLES-III.md](docs/VEHICLES-III.md): handling, tyre effects, crash damage and vehicle sound
- [VEHICLES-IV.md](docs/VEHICLES-IV.md): the Fokker triplane
- [VEHICLES-V.md](docs/VEHICLES-V.md): the field mortar
- [VEHICLES-VI.md](docs/VEHICLES-VI.md): the Tesla Roadster
- [VEHICLES-VII.md](docs/VEHICLES-VII.md): the hen
- [VEHICLES-VIII.md](docs/VEHICLES-VIII.md): the tupp, a Hedemora rooster with laser eyes
- [VEHICLES-IX.md](docs/VEHICLES-IX.md): the technical, a pickup with a heavy machine gun in its bed, soft springs and big tyres
- [VEHICLES-X.md](docs/VEHICLES-X.md): the submarine, which dives, looks through its periscope and fires guided missiles its officer flies from the nose
- [VEHICLES-XI.md](docs/VEHICLES-XI.md): the seaplane, an amphibious biplane on pontoons that lands on water, armed as the triplane is
- [VEHICLES-XII.md](docs/VEHICLES-XII.md): the Strf 90, a CV90-style infantry fighting vehicle with a 40 mm autocannon firing armour-piercing or high explosive, and two troops who leave by the rear ramp
- [VEHICLES-XIII.md](docs/VEHICLES-XIII.md): the snowracer, a steerable sled for two that slides down ski slopes about as fast as a snowmobile drives, with a small motor to creep back up
- [VEHICLES-XIV.md](docs/VEHICLES-XIV.md): the Titan, a robot fifty metres high whose pilot moves its fists: it punches, picks up soldiers and vehicles and throws them, crushes what it steps on, fires heavy rounds from a three-barrel rotary cannon in its right fist and guided missiles from tubes on its left shoulder, and flies a while on the thrusters in its feet to come down as a blast
- [VEHICLES-XV.md](docs/VEHICLES-XV.md): the bus, a twelve-metre city bus with a driver and twelve passengers, heavy and slow
- [VEHICLES-XVI.md](docs/VEHICLES-XVI.md): the tracked vehicles at their real sizes (the tank as a Strv 104, the Strv S as a Stridsvagn 103, the Strf 90 as a CV90), with hulls driven on and shot at as their real side outlines, so the Strv S's long glacis turns most shots from ahead
- [VEHICLES-XVII.md](docs/VEHICLES-XVII.md): the Pbv 302, an amphibious tracked troop carrier with a 20 mm automatic cannon in a small turret, thin armour and two troops who leave by the rear doors; it floats, and its tracks paddle it across open water
- [VEHICLES-XVIII.md](docs/VEHICLES-XVIII.md): the AMV XA-361, an eight-wheeled amphibious armoured mortar carrier whose turret's two 120 mm tubes lob a pair of bombs out to 200 m, then reload for three seconds
- [VEHICLES-XIX.md](docs/VEHICLES-XIX.md): the Stridsbåt 90, a 16 m assault craft on waterjets that planes at 45 knots, banks into its turns and crash stops inside two lengths, its bow digging in behind a wall of spray; its driver aims the twin machine guns fixed on the cockpit by turning the boat, its gunner stands in an open ring with a 25 mm cannon that goes all the way round, and two riders fire from the after deck
- [VR.md](docs/VR.md): the game in a VR headset through WebXR on WebGPU, to pilot a Titan with your own hands; built but not yet run on a headset
- [CHURCHES.md](docs/CHURCHES.md): churches get a roof fit, windows, a door and colours of their own: one steep gable down the nave with the sacristy roofed lower, tall round-headed windows, and one open hall inside
- [BUILDING-STYLES.md](docs/BUILDING-STYLES.md): barns, outbuildings, halls, shops, blocks of flats, public buildings, fire stations, cabins, old wooden houses and post-war villas told from plain houses by their OpenStreetMap tags, the shops and police stations mapped inside them, the land use they stand in and their yard, each with its own doors, windows, inside and colours, and its front towards its road; the gates of barns, halls and garages are opened and shut with E and driven through
- [BRIDGES.md](docs/BRIDGES.md): bridges carved by blasts as houses are, in sections that each stand on a pier, so a blown pier or a deck cut across drops a span into the river; small arms do nothing to them and the 40 mm barely marks them
- [BARRIERS.md](docs/BARRIERS.md): fences and walls from OpenStreetMap and orienteering maps, solid piece by piece and breakable: a car drives through a fence and breaks it, a wall stops it unless it is going fast, and tanks, blasts and Titans go through both
- [GRAVEYARDS.md](docs/GRAVEYARDS.md): tombstones of three kinds in every cemetery and churchyard from OpenStreetMap, stood in rows along the yard's paths or in rows of their own where it has none, solid and unbreakable like rocks
- [LIFTS.md](docs/LIFTS.md): ski lifts from OpenStreetMap, every one built as a chair lift whose chairs run on the server's clock, boarded with E at a station and ridden round until you jump off
- [BUSES.md](docs/BUSES.md): public transport, a world option like winter: bus lines found along the major roads, with stops, and buses the server drives along them that anyone can ride, or take once the driver is gone
- [HEADLIGHTS.md](docs/HEADLIGHTS.md): every vehicle's lamps, which its driver switches with L: headlights along the nose, a helicopter's searchlight under its gun and a Titan's in its visor
- [WATER-WAVES.md](docs/WATER-WAVES.md): water that moves when something moves through it, a wave equation stepped on the GPU about the camera, so boats raise bow waves and leave a V behind, a boat that stops dead runs into its own wave and splashes send out rings, on water cut finer on the GPU near the camera; and a waterjet boat's spray in layers of streaks, drops, drifting mist and glints that glow against the sun

## Requirements

- Rust 1.85+ (edition 2024), with the WASM target: `rustup target add wasm32-unknown-unknown`
- `wasm-bindgen` of the version `crates/sim-wasm/Cargo.toml` pins: `cargo install wasm-bindgen-cli --version 0.2.129 --locked`. The client's prediction runs the server's own physics (`crates/sim`) built to WASM; `npm run dev`, `npm test`, `npm run type-check` and `npm run build` build it first (`npm run build:wasm`). It is built with WebAssembly SIMD (`.cargo/config.toml`), which Rapier's solver uses through `wide`: vehicles step about a third faster than without it. `wasm-opt` was tried and left out: about 5% slower for 45 KB less gzipped.
- Node 22.18+ or 24.12+
- A browser with WebTransport: Chrome, Edge or Firefox

## Run locally

```sh
# 1. World (generated, not committed; scripts/world.sh does the same as the first lines here)
cargo run -p osm-import                       # default: 8.6 km square of central Umeå, Backen to Nydala, in 256 m tiles
cargo run -p osm-import -- --center 59.3167,18.0613 --half 600   # any lat,lon and half-side in metres
cargo run -p osm-import -- --bbox 59.3290,18.0600,59.3345,18.0705   # any south,west,north,east
cargo run -p osm-import -- --synthetic        # offline grid city, with hills
cargo run --release -p osm-import -- --fetch-osm sweden   # once: OpenStreetMap offline (Geofabrik), no Overpass
# Terrain, water and roofs come from Lantmäteriet (CC BY 4.0; water needs the Hydrografi Direkt
# service enabled), trees from Umeå's open data (CC0). `--osm-water` takes the water from OpenStreetMap instead.
# Lantmäteriet needs a Geotorget account: put LM_USER and LM_PASS in .env.local (git-ignored) and
# load it. Downloads are cached in .cache/osm-import (--refresh, --clear-cache). See WORLD.md.
set -a; . ./.env.local; set +a
cargo run -p osm-import                       # terrain, roofs and trees
cargo run -p osm-import -- --dem m708_75.tif  # or from a DTM GeoTIFF you already have
cargo run -p osm-import -- --cell 0           # flat ground, no elevation (--cell is the sample spacing, 4 m)

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

Open http://localhost:5173 in two windows. Click to capture the mouse. Controls: WASD to move, Shift to run, Space to jump, Esc to release the mouse. Walk up to a car and press E to get in (the first to board drives, the next ones ride along) and E again to get out. Driving: W/S gas and brake, A/D steer, Space handbrake. Two tanks are parked among the cars: the first in drives (A/D turns it on the spot), the second takes the gun. The gunner aims with the mouse and fires with a click or F. On foot you start with empty hands: walk up to an item on the ground (revolver, AK5, grenade, bandage) and press E to take it. 1-5 or the mouse wheel pick a hotbar slot, a click (or F) uses what is in hand, R reloads; Tab shows the scores. Slot 5 holds bricks: a click puts a cube where the crosshair is (a ghost shows where), holding it keeps building, and right mouse held with a click digs a placed cube out; build up a wall, over a roof or out from the ground as far as you like ([BUILDING.md](docs/BUILDING.md)). It is a free-for-all: you die at zero health and three seconds later rise as a ghost: fly where you like and press E to take shape again, away from any enemy, or G to drop in at your entry point ([GHOSTS.md](docs/GHOSTS.md)). Grenades and tank shells also wreck vehicles. One helicopter is parked there too: its pilot leans with W/S, turns with A/D, climbs with Space and sinks with Shift, and one passenger takes the nose autocannon, aiming with the mouse and firing explosive rounds with a click or F. R swaps seats in vehicles, tank or helicopter. Two Fokker triplanes stand at the end of long straight roads: the pilot works the throttle with Space (more) and Shift (less, and the brake on the ground), steers with the mouse (the plane banks and turns towards where the view points, and a yellow cross shows where the nose is), and can take pitch and roll over by hand with W (nose down), S (nose up) and A/D while they are held; a click or F fires the two forward guns and C drops a bomb. The second aboard works the gun on the ring behind. A seaplane, where the map editor has placed one, is flown and armed the same way: it floats on its pontoons, taxis with A/D on a water rudder, takes off from and lands on open water, and has wheels for a road too. One Tesla Roadster is parked with the cars: Shift fires its rear cold-gas thrusters for a huge shove, Space hops it straight up (hold Space in the air to hover, leaning with W/S/A/D), and C is its handbrake. The air tank shows bottom right and refills while it sits on its wheels.

The world is `client/public/world.msgpack` plus `client/public/tiles/`, git-ignored and generated by step 1 (`scripts/world.sh --synthetic` is the offline one, and the tests use the small committed `fixtures/world/`): the server reads all of it at start and the browser streams the tiles around the player. Restart the server after regenerating the world. The page warns if its world differs from the server's.

**Player settings:** open `/settings.html` (or use the link on the click-to-play screen) to set your name, outfit colour and up to 5 spawn points on the world's map. You start at the first and spawn at the next one each time you die. Spots in water or inside a building are skipped. Settings are kept in the browser and sent when the game connects, so return to the game (or reload it) after changing them.

**Map editor:** `cargo run -p editor`, then open `/editor.html` on the Vite dev server to paint which 256 m tiles of the map make up a world, see which of them are cached, and keep several worlds to switch between (see [EDITOR.md](docs/EDITOR.md) and [WORLD.md](docs/WORLD.md)).

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

To ship a world other than `client/public`, set `WORLD_DIR`, e.g. `WORLD_DIR=worlds/ersboda-alidhem`; its `area.json` and `scenario.json` go in with it.

The deploy writes a brotli copy (`.br`) of every compressible file in `client/dist`, the world's tiles included (`client/scripts/precompress.mjs`), and the server sends it to any browser that accepts brotli. Only brotli is written: the server takes encodings in the order the browser lists them, and browsers list gzip first.

Commit first: the image is tagged with the current `HEAD`. A protocol bump shows players with the old page open the "reload" banner once the new container is up.
