# Umeloppet course preview

`<umeloppet-map>` is a web component for Skidveteranerna's site that replaces the Leaflet map of the Umeloppet course.
- **The map:** it shows the course on the world's map, and visitors can drag and zoom it.
- **The 3D course:** where the browser has WebGPU, a button ("Åk banan i 3D") starts the visitor on skis behind the start line. They can ski the course, or take one of the snowmobiles parked at the start.
- **Offline:** it runs entirely in the visitor's browser. There is no server and no multiplayer, and nothing is sent anywhere.

It is built from the game's own client code (the world view, the soldier and its ski physics in WASM, the snowmobile) but sits outside the game:
- **Nothing in `client/`, `engine/` or `crates/` refers to it.**
- **Game builds are unaffected:** the game's type-check, lint, tests and CI never see it.
- **Fixed once built:** a built bundle doesn't change, so game work can only affect it at the next rebuild. Fix it here then.

## On the club's page

Upload the folder `scripts/assets.sh` makes (see below), then put this where the map should go:

```html
<umeloppet-map></umeloppet-map>
<script type="module" src="https://example.org/umeloppet/umeloppet.js"></script>
```

The element finds its files next to its script.

**Attributes:**
- `base`: the folder holding the files, if they are not next to the script.
- `course`: another course file than `course.json` in that folder, such as a second distance.

**Size:** it is 520 px tall by default. Style it like any block element, e.g. `umeloppet-map { height: 70vh }`.

**Hosting:**
- **Cross-domain:** if the files are on another domain than the page, that host must send `Access-Control-Allow-Origin` (e.g. `*`). The element starts its workers through a same-origin shim, so that works too.
- **What visitors download:** the map view loads about 7 kB of script (gzipped), the course and the map image. The 3D view loads about 220 kB of script plus 1 MB of physics WASM (gzipped), then streams the world's tiles around the skier as they go.
- **Older browsers:** a browser without WebGPU (older Safari, some Android phones) gets the map only, with a note.

### In the club's Nuxt site on Netlify

The club's site is the Nuxt project `vb-ski-astro`, next to this repo. It is static, so the element is just more static files in it, served from the site's own domain (no CORS). It replaces the Leaflet map in `app/components/content/TrackMap.vue`, the `:track-map` on the Umeloppet page.

1. **Put the files in `public/`.** Gather them straight into the Nuxt project, so they are served at `/umeloppet-map/`. Not `/umeloppet/`: that is the race's page, and a folder of the same name makes Nuxt's dev server redirect the page to it:
   ```sh
   npm run build
   sh scripts/assets.sh ../../worlds/umeloppet ../../../vb-ski-astro/public/umeloppet-map
   ```
   It is about 47 MB in 174 files, almost all of it the world's tiles. Commit them with the site, or add them in the deploy step.
2. **Tell Vue the tag is not a Vue component,** in `nuxt.config.ts`:
   ```ts
   export default defineNuxtConfig({
     vue: { compilerOptions: { isCustomElement: (tag) => tag === 'umeloppet-map' } },
   })
   ```
3. **Put it on the page,** loading the script only there:
   ```vue
   <script setup lang="ts">
   useHead({ script: [{ src: '/umeloppet-map/umeloppet.js', type: 'module' }] })
   </script>

   <template>
     <ClientOnly><umeloppet-map /></ClientOnly>
   </template>
   ```
   - `<ClientOnly>` keeps the element out of the prerendered HTML. The element fills itself with its own children, and Vue's hydration would otherwise see children it did not render.
   - Leaving the page with Nuxt's navigation removes the element, which stops the 3D view and frees the GPU. Coming back makes a fresh one.
4. **Optionally cache the hashed files for good,** in `netlify.toml`. Their names change with every build. Everything else keeps Netlify's default, which checks for a newer copy, so a rebuilt world or course shows at once:
   ```toml
   [[headers]]
     for = "/umeloppet-map/assets/*"
     [headers.values]
       Cache-Control = "public, max-age=31536000, immutable"
   ```

Netlify serves `.wasm` as `application/wasm`, which the physics needs. No other headers are needed: nothing uses `SharedArrayBuffer`, so the site needs no cross-origin isolation.

**Controls:**
- **On a computer:** click to capture the mouse.
  - W A S D ski, the mouse steers.
  - Shift poles hard, C tucks.
  - E gets on or off a snowmobile, K puts skis on or off.
  - Esc lets go of the mouse; until the visitor clicks again, keys go to the page.
- **On a phone:** drag on the left to ski and on the right to look around. Next to a snowmobile a small "Snöskoter" button gets on it, and "Kliv av" gets off; the skis stay on.

## Making it

You need the race's GPX file. These steps are for a world called `umeloppet`.

1. **Paint the world around the course.** This writes `area.json` for a winter world. Its forest paths are not groomed (`groomed_paths: false`), so the map image shows no ski tracks but the course, which the element draws itself:
   ```sh
   cd embed/umeloppet
   node scripts/area.ts --gpx ../../worlds/umeloppet/course.gpx --out ../../worlds/umeloppet/area.json --margin 400
   ```
   - It covers every 256 m tile within 400 m of the course, with a spawn at the start.
   - You can open it in the map editor afterwards to add tiles.
2. **Build the world** with the importer. It needs the Lantmäteriet credentials in `.env.local`:
   ```sh
   cd ../.. && set -a && . ./.env.local && set +a
   cargo run --release -p osm-import -- --area worlds/umeloppet/area.json --out worlds/umeloppet/world.msgpack
   ```
3. **Write the course** in the world's metres. This writes `worlds/umeloppet/course.json`:
   ```sh
   cd embed/umeloppet
   node scripts/course.ts --gpx ../../worlds/umeloppet/course.gpx --world ../../worlds/umeloppet --name "Umeloppet" --length 20000
   ```
   - `--length` is the race's own length, metres. The GPX measures 17.8 km, but a lap is called 20 km (two laps make the 40 km that counts for Vasaloppet's seeding), so every distance along the course is stretched ×1.125: the readout, the km signs and the profile. The line on the ground and the speed are left as they are.
   - It holds the line, its heights (from the world's ground, the height model, for the profile), a marker at each kilometre, the start and finish, and two snowmobiles beside the start.
   - Run it again whenever the world is rebuilt or the GPX changes.
4. **Look at it** at http://localhost:5290/. The test page serves the world at `/world/`:
   ```sh
   npm run dev                                        # the umeloppet world
   UMELOPPET_WORLD=../../worlds/boda npm run dev      # or another winter world with a course.json
   ```
5. **Build and gather the upload:**
   ```sh
   npm run build
   sh scripts/assets.sh ../../worlds/umeloppet upload/
   ```
   `npm run check` type-checks the embed together with the client code it uses.

The scripts run on the client's Node and Vite (`../../client/node_modules`), so nothing is installed here. Run `npm install` in `client/` first if it has not been, and `npm run build:wasm` there after any physics change.

## How it works

| File | What |
|---|---|
| `src/element.ts` | The custom element. It shows the flat map, checks for WebGPU, starts and stops the 3D view, and handles the readout, the pause screen and the touch controls. |
| `src/flatmap.ts` | The map image with the course, km markers, flags, snowmobiles and skier drawn on a 2D canvas. It is used for the whole element and for the round minimap. |
| `src/solo.ts` | `SoloGame`, the game loop without a server, put together from the client's parts as `client/src/game.ts` does. The skier and the snowmobile ridden are stepped in the WASM sim and never corrected. The parked snowmobiles are fed to the vehicle presenters as if a server had replicated them. |
| `src/course.ts`, `src/course3d.ts` | The course file, the progress along it, the race clock (started through the START gate, stopped through MÅL once 80 % of the course is behind), and its look in 3D: orange marker sticks, km signs, and the start and finish arches. |
| `src/profile.ts` | The height profile under the map and the 3D view, from the world's ground under the course, with the skier marked on it. |
| `src/trails.ts` | Grooms the course into each tile as it streams in, 4 m wide as the importer grooms a track, takes the world's other ski tracks out, so the race line is the only one, and clears the trees and rocks within 4 m of it. It adds the course both to the decoded tile, for the drawn grooves, and to the tile's bytes, for the WASM sim, so skis snap into its grooves and glide on it. |
| `src/workers.ts` | Starts the game's module workers from another origin through a blob. |
| `vite.config.ts` | Aliases into `client/src` and `engine/src`. It rewrites the client's build-time `import.meta.env.BASE_URL` / `VITE_WORLD_BASE` to the element's runtime `base`, and serves the dev world. |

What it mirrors from the game by hand, and so may need updating after game changes:
- **The ski switch** (`mechanics/movement.rs`): only below `SKI_CLIP_SPEED`.
- **The tile file's layout** in `src/trails.ts`: `trees` is at index 5 of `TileFile` and `ski` at 11; rocks are tree kinds 7 and 8.
- **The local fall's ragdoll** (`Bodies.knockdowns`).
- **The frame loop's order** (`Game.start` and `Game.frame`).
- **The projection** in `scripts/course.ts` and `scripts/area.ts` (`shared::world::project`, `grid.rs`).
