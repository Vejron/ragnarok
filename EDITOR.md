# Map editor

A browser tool for choosing which part of the map the game is played on, and for placing spawn
points, vehicles, items and enemy bots on it by hand. It is a Vue page (`client/src/editor/`, opened as
`/editor.html`) on top of a small Salvo API (`crates/editor`). The editor never touches the game
itself: it edits files under `worlds/<name>/` that the importer and the server read.

```sh
cargo run -p editor          # API on 127.0.0.1:8700 (reads LM_USER/LM_PASS from .env.local)
cd client && npm run dev     # open http://localhost:5173/editor.html
```

The Vite dev server proxies `/api` to the editor API.

## What it does

### 1. Paint a world

The map is OpenStreetMap under a grid of 250 m tiles. Paint the tiles that should be part of the
world; the importer builds the smallest centred square that holds them and only the painted tiles
get ground (see `WORLD.md`, "Painting a world").

| Tool | Effect |
|---|---|
| Pan | Move and zoom the map. Hold Shift to paint while panning. |
| Paint | Click or drag to add tiles. The brush select sets the size. |
| Erase | Remove tiles. Shift or the right button erases with any tool. |

### 2. Place spawns, vehicles and items

| Tool | Select | Placed |
|---|---|---|
| Spawn | none | A player spawn point. |
| Vehicle | vehicle kind and a facing (8 compass directions) | A vehicle at that spot, facing that way. |
| Item | item kind | A pickup at that spot. |

- Click a painted tile to place a marker. Placing outside the painted tiles is refused.
- Shift or right-click removes the nearest marker of the current tool's kind.
- Markers cannot be dragged: remove and place again.
- Placing any of a kind **replaces the random ones** for that kind: placed spawns replace the
  generated spawn points, placed vehicles replace the scattered vehicles and placed items replace
  the scattered pickups. A kind with none placed behaves as before.
- Vehicles that do not fit at their spot when the server starts are skipped, with a warning in the
  log.

### Bots and tanks belong to squads

Bots are the server's enemy stickmen (see `BOTS.md`), and since scenarios they are placed in
**Scenario** mode, as members of a squad, with the Bot, Tank, Route and Pick tools. Each is drawn as
a square in its squad's team colour (a bot with its team number, a tank with a T and a dashed
outline), and its route as a line in the same colour.

| Tool | Click | Shift or right-click |
|---|---|---|
| Area | Place an area (a circle, 50 m to start with; set its radius in the panel). | |
| Bot | Add a bot to the picked squad on a painted tile. On a bot: pick it. | Remove the nearest bot or tank. |
| Tank | Add a tank to the picked squad. It comes with its own crew of two bots, besides the squad's listed bots. | Remove the nearest bot or tank. |
| Route | Add a waypoint to the picked bot's or tank's route. On the bot itself, its own spot goes in the route. | Remove the nearest bot or tank. |
| Pick | Pick a bot or tank, and its squad. | Remove the nearest bot or tank. |

A squad has one team, skill and loadout for all its bots; set them in the panel. Nothing is in the
game until a trigger spawns the squad (a trigger with **the scenario starts** does it at once).

**Loose bots.** Worlds saved before squads can have bots placed straight into `area.json`. They
are still read and drawn (read-only), and the sidebar offers **Convert to squads**: it makes a
squad for each team and skill that respawns at home, as loose bots do, adds a trigger that spawns
them at the start, saves the scenario and then the world without them.

### 3. Manage worlds

The side panel keeps several named worlds:

- **Save** writes `worlds/<name>/area.json` (the button shows `*` when there are unsaved changes).
  **New** starts an empty world, **Clear** empties the current one, **×** deletes a world.
- **Download** fills the importer's cache for the 2 km blocks the painted tiles lie in. The map
  shows which blocks are already cached.
- **Generate world** runs `osm-import --area` on the world and writes `world.msgpack` and its
  tiles next to `area.json`. A job panel shows the stage and download progress.
- **use** makes a generated world the active one (`worlds/active.json`). Restart the server and
  reload the game to play it.

## Files

`worlds/<name>/area.json` holds everything the editor authors. Positions are latitude/longitude, so
they survive the world being re-centred when tiles are added or removed:

```json
{
  "tiles":    [{ "gx": 0, "gy": 0 }],
  "spawns":   [{ "lat": 63.8258, "lon": 20.2630 }],
  "vehicles": [{ "kind": 1, "lat": 63.8259, "lon": 20.2631, "yaw": 0.0 }],
  "items":    [{ "item": 2, "count": 1, "lat": 63.8257, "lon": 20.2629 }],
  "bots":     [{ "lat": 63.8256, "lon": 20.2628, "team": 1, "skill": 0.5,
                 "route": [{ "lat": 63.8256, "lon": 20.2628 }, { "lat": 63.8260, "lon": 20.2640 }] }]
}
```

`spawns`, `vehicles`, `items` and `bots` are optional; older files without them still load. A
bot's `team`, `skill` and `route` are optional as well (1, 0.5 and none); the editor always
writes all three. `yaw` is in
radians as the simulation counts it (0 faces north, positive turns left); the editor stores
compass headings in 45° steps. `area.json` is committed; the generated files are git-ignored.

Placements are not part of `world.msgpack`. The server reads `area.json` next to the manifest on
start, so changing them needs a save and a server restart, but **no "Generate world"**.

A world's scenario, `scenario.json` in the same folder, is edited in **Scenario** mode (the sidebar's
switch): areas, squads of bots and tanks on the map, and triggers as forms. Problems are listed
before saving, and deleting the world deletes the scenario too. See [SCENARIOS.md](SCENARIOS.md) for how to write one, and [BOTS.md](BOTS.md), Scenarios, for the reference.

## API

All under `/api` on port 8700.

| Route | |
|---|---|
| `GET grid`, `GET cache`, `GET job` | Grid description, which blocks are cached, current job state. |
| `GET worlds` | List worlds, with whether each is generated and active. |
| `GET/PUT/DELETE worlds/{name}` | Read, save (tiles and placements) or delete a world. |
| `POST worlds/{name}/fetch` | Start downloading the world's blocks. |
| `POST worlds/{name}/generate` | Start generating the world. |
| `POST worlds/{name}/activate` | Make it the active world. |

## Code map

| | |
|---|---|
| `client/src/editor/Editor.vue` | The page: map, tools, markers, bot routes, world list, job panel. |
| `client/src/editor/placements.ts` | Placement types, vehicle and item kinds, bot teams and route lines, heading/yaw conversion, nearest-marker lookup. |
| `client/src/editor/api.ts` | Typed client for the API. |
| `crates/editor/src/main.rs` | The API, job runner and world files. |
| `crates/server/src/main.rs` | `apply_placements`: reads `area.json` at server start. |
| `crates/server/src/mechanics/{vehicles,pickups}.rs` | Spawn the placed vehicles and items. |
| `crates/server/src/game.rs` | `Game::new` adds the placed bots (`add_bot`); `mechanics/bots.rs` runs them. |
