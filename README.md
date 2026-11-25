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
- [FRAME-PERFORMANCE.md](docs/FRAME-PERFORMANCE.md): the main thread's frame while tiles stream in and houses come down: a benchmark that drives and shells the real game, and what was taken out of the frame (buildings made solid from a queue, meshes packed in the workers, falling sections and heaps cut in the look worker, a house's faces kept between cuts)
- [MEMORY.md](docs/MEMORY.md): what the client holds in memory and how to measure it: a tile's buildings welded and kept on the GPU only, the multisampled image given no memory, and the per-frame garbage

**Making worlds**

- [WORLD.md](docs/WORLD.md): generating the world: data sources, credentials, download cache, squared footprints and the catalogue of roof shapes (gable, hip, gambrel, cross-gabled wings, dormers)
- [FRANCE.md](docs/FRANCE.md): worlds in France from IGN LiDAR HD
- [TERRAIN-SHAPING.md](docs/TERRAIN-SHAPING.md): the height model shaped to OpenStreetMap: houses on level pads, streams cut into the ground and never running uphill, roads level across with clean cuts and embankments, and underpasses dug out under roads and railways
- [EDITOR.md](docs/EDITOR.md): the map editor
- [SCENARIOS.md](docs/SCENARIOS.md): a guide to writing a world's `scenario.json` by hand, since there is no scenario editor

**How each feature was built**

- [BOTS.md](docs/BOTS.md): server-run enemy soldiers, tank crews, and scenarios with squads, triggers, waves, objectives and a win or a loss
- [ARMOR.md](docs/ARMOR.md): armor zones, ricochets, weak spots, turret loss and the Strv S
- [LIMBS.md](docs/LIMBS.md): limb loss and part health
- [CHARACTER-LOOKS.md](docs/CHARACTER-LOOKS.md): the people on foot: Quaternius' low-poly men and women (CC0) dressed as soldiers, police officers, ambulance medics and civilians, on one skeleton that every clip is retargeted onto, the Universal Animation Library's hand-keyed clips among the game's own
- [ANIMATION.md](docs/ANIMATION.md): what moves a character besides its clips: feet stood on the slope and held where they land, the left hand kept on the gun, jumps in the pose eased out, the aim followed a moment late and the head swaying with the hips
- [DOORS.md](docs/DOORS.md): getting in and out of a car by its doors, which swing open, bailing out of one going fast, police and ambulance crews who are the same people in the car and out of it, and doors shot or crashed off
- [KNOCKDOWN.md](docs/KNOCKDOWN.md): blasts, cars and ragdolls; every death a ragdoll thrown by the blow, with joint limits, that loses limbs and freezes where it lies
- [GHOSTS.md](docs/GHOSTS.md): coming back from death as a flying shadow that takes shape again, never right beside an enemy
- [SWIMMING.md](docs/SWIMMING.md): swimming, diving where you look, holding your breath and drowning
- [ZOMBIES.md](docs/ZOMBIES.md): zombies that rise in hordes at night, shamble after you and claw, and can be shot apart limb by limb
- [BUILDING.md](docs/BUILDING.md): placing cubes with bricks
- [INTERIORS.md](docs/INTERIORS.md): floors and stairs inside every house big enough, worked out from its footprint, with its door open and its floors blasted like its walls
- [CRATERS.md](docs/CRATERS.md) and [SCORCH.md](docs/SCORCH.md): shells digging the ground, and the marks they leave; the ground heals where nobody is near
- [REGROWTH.md](docs/REGROWTH.md): felled trees grow back through a sapling and a young tree, and broken fences and walls are mended, where nobody can see it
- [CROPS.md](docs/CROPS.md): wheat on the farmland, high enough to hide a crouched soldier from other players and from the bots
- [GUNSMOKE.md](docs/GUNSMOKE.md): a shell leaving the barrel: fire, a bank of smoke sized to the gun, brake jets, sparks, dust and a barrel that goes on smoking, with smoke that churns, leans with the wind and is lit by flashes and lamps; small arms smoke too, every gun has a shaped muzzle flare, and the sniper kicks down its scope
- [HULL-DAMAGE.md](docs/HULL-DAMAGE.md): what a round leaves on a vehicle: dents, soot, holes ringed with blued steel and gouges, drawn by the mesh's own shader, and the blow that rocks it, a Titan too
- [DRONES.md](docs/DRONES.md): FPV and loitering drones
- [GYRO-AIM.md](docs/GYRO-AIM.md): aiming on a phone by turning it
- [BACKPACK.md](docs/BACKPACK.md): backpack mode
- [DELIVERY.md](docs/DELIVERY.md): delivery mode, food orders on a phone
- [PHONE.md](docs/PHONE.md): the phone in the soldier's hands, its map and delivery apps
- [VEHICLES-I.md](docs/VEHICLES-I.md): cars, seats, driving
- [VEHICLES-II.md](docs/VEHICLES-II.md): the tank, its gunner seat and the shell, driver prediction and the helicopter; how tracks ride over humps, steps and side slopes
- [HELICOPTER.md](docs/HELICOPTER.md): the helicopter's semi-realistic flight model: collective lever, torque cyclic, pedals to the look, rotor speed, ground effect, translational lift, vortex ring and autorotation
- [VEHICLES-III.md](docs/VEHICLES-III.md): handling, tyre effects, crash damage, breaking through a house wall at speed, and vehicle sound
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
- [VEHICLES-XIX.md](docs/VEHICLES-XIX.md): the Stridsbåt 90, a 16 m assault craft on waterjets that planes at 45 knots, banks into its turns and crash stops inside two lengths, its bow digging in; its driver aims the twin machine guns fixed on the cockpit by turning the boat, its gunner stands in an open ring with a 25 mm cannon that goes all the way round, and two riders fire from the after deck
- [VEHICLES-XX.md](docs/VEHICLES-XX.md): the raft, a small inflatable river-rafting boat for two with a big outboard, shallow enough to run a rapid, where eddy lines spin it, it slides out of its turns, flies off crests and scrapes over the shallows; boats now carry their own tuning (`Lift::Boat`), and the boats' bow and crash-stop spray gave way to a white wash
- [VEHICLES-XXI.md](docs/VEHICLES-XXI.md): the air ambulance, an unarmed EC135 in Swedish ambulance yellow for a pilot and three passengers, faster than the gunship and slower to climb, with a searchlight and a solid tail; the client now tells a helicopter by its rotor rather than by its kind
- [VEHICLES-XXII.md](docs/VEHICLES-XXII.md): the EPA traktor, an old boxy saloon cut down into a two-seat pickup and held to 30 km/h, a Swedish teenager's first car, with a slow-vehicle triangle in its rear window and see-through glass
- [VEHICLES-XXIII.md](docs/VEHICLES-XXIII.md): the period cars of 1980 to 1995 that make up the traffic: Saab 900 Turbo and 9000, Volvo 240 and 740, Golf GTI, VW Transporter, Opel Kadett and Omega, Ford Sierra and Escort, with the jeep remodelled as a Ford Bronco and the sports car as an Opel Manta, all from one body builder
- [VEHICLES-XXIV.md](docs/VEHICLES-XXIV.md): a boost on run for every car, tank and boat, a few seconds of much more pull, with the top speed now 90% of the old one off it and 105% on it, and tyre dust only off tyres that slide, lock or spin
- [VR.md](docs/VR.md): the game in a VR headset through WebXR on WebGPU, to pilot a Titan with your own hands; built but not yet run on a headset
- [CHURCHES.md](docs/CHURCHES.md): churches get a roof fit, windows, a door and colours of their own: one steep gable down the nave with the sacristy roofed lower, tall round-headed windows, and one open hall inside
- [BUILDING-STYLES.md](docs/BUILDING-STYLES.md): barns, outbuildings, halls, shops, blocks of flats, public buildings, fire stations, cabins, old wooden houses, post-war villas and lean-to shelters (open on the side OSM says, under a roof falling to the back) told from plain houses by their OpenStreetMap tags, the shops and police stations mapped inside them, the land use they stand in and their yard, each with its own doors, windows, inside and colours, and its front towards its road; the gates of barns, halls and garages are opened and shut with E and driven through
- [ROADS.md](docs/ROADS.md): roads as wide as their tagged lanes, white Swedish paint (dashed edge lines, centre and lane lines, solid where overtaking is barred) drawn by the asphalt's own shader, a steel guard rail down the middle of every fast divided road such as a 2+1 road, inferred from its two one-way carriageways, and asphalt that no longer shows its shoulder or the ground through it
- [BRIDGES.md](docs/BRIDGES.md): bridges carved by blasts as houses are, in sections that each stand on a pier, so a blown pier or a deck cut across drops a span into the river; small arms do nothing to them and the 40 mm barely marks them; railways have them too, over roads and underpasses, with track on the deck that goes where the deck does
- [BARRIERS.md](docs/BARRIERS.md): fences and walls from OpenStreetMap and orienteering maps, solid piece by piece and breakable: a car drives through a fence and breaks it, a wall stops it unless it is going fast, and tanks, blasts and Titans go through both
- [GRAVEYARDS.md](docs/GRAVEYARDS.md): tombstones of three kinds in every cemetery and churchyard from OpenStreetMap, stood in rows along the yard's paths or in rows of their own where it has none, solid and unbreakable like rocks
- [LIFTS.md](docs/LIFTS.md): ski lifts from OpenStreetMap, every one built as a chair lift whose chairs run on the server's clock, boarded with E at a station and ridden round until you jump off
- [TRAFFIC.md](docs/TRAFFIC.md): traffic, a scenario option: a thousand cars that drive the world's roads by a road network the server makes of them, each a row in a table until a player is near enough to see it, and a real car only beside one, with a driver who can be dragged out, or held up at gunpoint to give the car up, flee or run you down
- [EMERGENCY.md](docs/EMERGENCY.md): police and ambulances, a scenario option: an ambulance whose medics carry off the dead and the badly hurt, a wanted level of one to five stars for a player who hurts civilians, police cars that plan their way by road with the traffic pulling over for the siren, and officers who arrest at first and shoot once it has gone far enough
- [BUSES.md](docs/BUSES.md): public transport, a world option like winter: bus lines found along the major roads, with stops, and buses the server drives along them that anyone can ride, or take once the driver is gone
- [HEADLIGHTS.md](docs/HEADLIGHTS.md): every vehicle's lamps, which its driver switches with L: headlights along the nose, a helicopter's searchlight under its gun and a Titan's in its visor
- [GERSTNER-SEA.md](docs/GERSTNER-SEA.md): a rough sea of Gerstner waves: the water goes round as each swell passes, gathering under sharp crests that break white and spreading over flat troughs, and carries boats and swimmers to and fro with it
- [WATER-WAVES.md](docs/WATER-WAVES.md): water that moves when something moves through it, a wave equation stepped on the GPU about the camera, so boats raise bow waves and leave a V behind, a boat that stops dead runs into its own wave and splashes send out rings, on water cut finer on the GPU near the camera; and a waterjet boat's spray in layers of streaks, drops, drifting mist and glints that glow against the sun
- [RIVER-CHANNELS.md](docs/RIVER-CHANNELS.md): rivers that OpenStreetMap draws as a line made real rivers: 10 m across on average and 4 m in a pinch, carved, one water with the pools painted over them, flowing into the big rivers without a step, with bridges that stay level with their banks; streams stay line water in the river's colour
- [SHORES.md](docs/SHORES.md): shorelines that follow the outline, not the sample grid; streams that fill their channels; no trees in the water
- [RIVERS.md](docs/RIVERS.md): rivers that run downstream, fastest where they fall, carrying swimmers and drifting boats (an empty or anchored one holds), with white water and standing waves in the rapids
- [RIVER-SURFACE.md](docs/RIVER-SURFACE.md): a river drawn and ridden as running water: its surface shaped by the solved flow (piled up before a boulder, dipping in a fast tongue, standing in waves across a rapid), no lake swell on it, ripples that ride the current, stained water in a colour each world sets, boulders that trail wakes, and the current drawn on it: streaks along its streamlines, boils where it wells up, foam lines where it gathers, whirls along eddy lines and riffles over a shallow bed
- [RIVER-FLOW.md](docs/RIVER-FLOW.md): a river's current solved at import as a flow (depth-averaged Navier-Stokes), over a bed cut to match: shallow, fast rapids and deep, slow pools, the same water through every cross-section, faster through a narrows and slack behind an island; a rapid's bed is rough, with bars, boulders and a deep chute for boats, and the foam made where its water breaks is carried downstream and drawn where it comes to lie
- [GROUND.md](docs/GROUND.md): ground in the colours of ground, not of a map: each cover varied by painted patches, bare rock as granite slabs with joints, lichen and a dark waterline, gardens of mown lawn with gravel round the houses, and car parks of asphalt; natural edges that curve, fray and grow shrubs where OpenStreetMap drew them straight
- [FOREST-FLOOR.md](docs/FOREST-FLOOR.md): the floor of the forest that stands there, from the trees' species: pine heath, spruce moss or birch wood, each painted in its own colours with needle litter under the trees, and grown with bilberry, lingonberry, heather, ferns, stumps and mossy stones
- [GARDENS.md](docs/GARDENS.md): gardens round houses by the kind of house: a flower bed along the sunny wall and a rhubarb patch out at the back
- [AUDIO.md](docs/AUDIO.md): engines made firing by firing from each vehicle's real machine (the Centurion's Meteor V12, a snowmobile's two-stroke twin, a rotary, a turbine, an electric motor), revving with the driver's throttle through its gearbox, over the crunch, roll and squeal of the ground it is on, and soldiers' footsteps on what they step on; and the world about you: wind, leaves, birds by day and a dawn chorus, crickets and owls at night, lapping water and running streams, rain on the ground or the roof, thunder after the lightning, and gunfire heard late and echoing from a kilometre off, all muffled in a closed cabin or under water

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
# Terrain and roofs come from Lantmäteriet (CC BY 4.0), water from OpenStreetMap, trees from Umeå's
# open data (CC0).
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

**Debug screenshots:** press F2 in the game to save what you see. The PNG carries, in a `ragnarok` text chunk, the world, the camera's position, direction, field of view and its view and projection matrices, the player's position, and what the crosshair ray hits with the building there (its tile, index and footprint). On the Vite dev server it is saved as `shots/<world>-<x>_<z>-<time>.png` at the repo root (git-ignored); elsewhere it is downloaded. `node client/scripts/shot-info.mjs <png>` prints what it says, and its `looking.point` finds that house in the importer's roof survey (`ROOF_AT`, see [WORLD.md](docs/WORLD.md)).

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
