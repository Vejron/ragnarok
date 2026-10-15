# Rendering

How the client turns the simulation into a picture: the scene, the light, the shading style, the
soft-lighting options and what each one costs. The drawing is done by the WebGPU engine in
[`engine/`](engine/README.md), a package of its own; the game's side of it is `client/src/render/`,
and `client/src/game.ts` drives it once per animation frame. Networking and prediction are in
[NETWORKING.md](NETWORKING.md), where the world comes from is in [WORLD.md](WORLD.md).

WebGPU is required. There is no WebGL fallback: without `navigator.gpu` the game says so and stops.
three.js is still a dependency of the client, but only for the vector, quaternion and matrix classes
the game logic uses; nothing of three draws.

```
game.frame()
  ├─ fixed ticks, interpolation          (not rendering)
  ├─ update vehicles / characters         poses written onto their nodes
  ├─ scene.follow() / aimFrom()           camera + sun shadow frustum follow the target
  ├─ tiles, effects, pickups, rockets     streamed in / animated
  ├─ contact.begin() … add() … end()      blob shadows for this frame's poses
  └─ scene.render()                       engine Renderer.render(scene, camera)
        ├─ effect prepare   compute: particle emit + simulate
        ├─ shadow pass      sun depth map
        ├─ main pass        4× MSAA, rgba16float, reversed depth
        ├─ effects half     smoke at half resolution into its own target (`halfResolution` effects)
        ├─ effects pass     that target laid over, then particles, ribbons + flashes, depth read-only
        ├─ overlay pass     the gunsight, over everything
        ├─ bloom            bright extract → mip chain → add back (when `scene.bloom`)
        └─ output pass      ink from depth, bloom, tone mapping, sRGB     → canvas
```

## The engine and the game's side of it

The engine ([engine/README.md](engine/README.md) has its design) keeps everything a frame needs on the
GPU between frames: one object record per mesh (written only when it changes), one material table,
one vertex arena. Meshes that share geometry and render state become one instanced draw, whatever
their materials. That is why the renderer costs a fraction of what three's did, and why the game can
make a `Material` per car or per player without it costing a pipeline.

The game talks to it through the same shapes as before: a `Scene` graph of `Group`s and `Mesh`es, a
`Material` per look, `Geometry` for vertex data. Things that differ from three, and matter when
touching this code:

| | three | engine |
|---|---|---|
| Vertex data | `BufferGeometry` + attributes | `Geometry` with typed arrays: `positions`, `normals`, `uvs`, `colors`, `custom`, `joints`, `weights`, `indices` |
| Changing vertices | `attribute.needsUpdate` | `geometry.needsUpdate()`, or `needsUpdateRange(first, count)` for a few vertices |
| Changing a material | set the field | set the field, then `material.needsUpdate()` (records are rewritten only then) |
| Rotation | `rotation` / `quaternion` | the same pair, kept in step; `rotation` takes an order |
| Transparency | `transparent` + `opacity` | `blend: 'alpha' \| 'additive'`; transparent meshes sort far to near and never cast shadows |
| Polygon offset | `polygonOffset*` | `depthBias` and `depthBiasSlope`, positive toward the camera |
| Fog / tone mapping per material | `fog`, `toneMapped` | `fog`; tone mapping is global (see below) |
| Shaders | GLSL `ShaderMaterial` | WGSL `ShaderMaterial` with a `surface()` function (water, skidmarks) |
| Sprites | `Sprite` | a quad `Mesh` turned to the camera (`damagenumbers.ts`) |
| Skinning | `SkinnedMesh`, `AnimationMixer` | `SkinnedMesh`, `Animator` (same weighted blend, driven by hand) |
| Loading | `GLTFLoader` | `parseGlb` (`render/models.ts` swaps in toon materials) |

Colours are linear in the engine; hex numbers are sRGB and converted on the way in (`setHex`,
`Color.hex`).

## The scene

`render/scene.ts` owns the engine `Scene`, the camera and the `Renderer`. The renderer needs the GPU,
which is requested asynchronously, so `Scene.init()` (called first thing in `Game.start`) must finish
before the first frame; `render()` does nothing until then. Everything else adds a `root` to the
scene:

| Piece | File | What it is |
|---|---|---|
| Ground plane | `world.ts` `buildGround` | One grass plane under the world; shows only where a tile is late. |
| Tiles | `world.ts` `buildTile` | Terrain mesh, buildings, roads and trees of one 250 m tile. Streamed in and out around the player. |
| Vehicles | `car.ts`, `tank.ts`, `heli.ts`, `plane.ts`, `walker.ts`, `strvs.ts`, `mortar.ts` | glTF models from `client/public/models/`, one clone per vehicle (the mortar is built by hand). |
| Characters | `character.ts` | Skinned clones of one shared model (`Crowd`), outfit colour by player id. |
| Items, pickups, rockets | `items.ts`, `pickups.ts`, `rockets.ts` | Small hand-built meshes. |
| Effects | `effects.ts` | Muzzle flashes, tracers, sparks, fireballs: `FlashEffects`, below. |
| Particles | `particles.ts` | Smoke, dust, gravel, sparks and flames: GPU particles, below. |
| Skidmarks | `skidmarks.ts` | A ring of quads with a birth time, faded in the shader. |
| Damage | `damage.ts`, `fracture.ts`, `rubble.ts` | A building redrawn from its cells (holes blasted through walls and roofs, the wall around a hole burnt black, pieces pushed in where this player's rounds landed), thrown and falling chunks, collapses. |
| Trajectory | `trajectory.ts` | The arc a held grenade would take. |
| Field mortar | `mortar.ts`, `mortar-arc.ts` | A hand-built gun, and the landing marker its gunner aims by. See [VEHICLES-V.md](VEHICLES-V.md). |
| Contact shadows | `contact.ts` | Blob shadows, below. |
| Gunsight | `gunsight.ts` | A gunner's reticle, an overlay laid on the camera. |

The camera is a 75° perspective camera, near 0.25 m, far 2000 m, with reversed depth (the near plane
maps to 1, so distant geometry keeps its precision). `Scene.follow` orbits it behind a target by
yaw/pitch (over the right shoulder on foot, further back per vehicle); `aimFrom` is the gunner's view
along a barrel. Both clamp it above the terrain via `groundAt`, and both also move the sun so the
shadow map stays centred on what the camera follows.

## Look: toy shading

The style is flat, bright and few-banded, not physically based.

- **Toon materials.** Almost everything is a `Material` made through `toonMaterial()` in `toon.ts`.
  That gives it the *cel ramp* (`CEL_RAMP`): four light levels, 96, 150, 205 and 255, so each band is
  hard-edged and nothing is blended between them. A material that brings its own ramp keeps it:
  terrain and roads use an eight-step ramp (`TERRAIN_RAMP` in `world.ts`, dark and coarse in shade and
  fine near the light). Buildings, vehicles, characters and tree crowns all use the cel ramp. Lamps
  (headlights) are unlit (`lit: false`), so they stay bright.
  The engine does three's `MeshToonMaterial` maths: `colour/π × (sun × ramp(N·L·0.5 + 0.5) × shadow +
  hemisphere(N)) + emissive`, then linear fog. A ramp is a row of one 256 × 64 texture, so the number
  of ramps is not a cost.
- **Candy palette.** Building colours, roof colours, cover colours (`cover.ts`) and player outfits
  (`character.ts` `PALETTE`) are fixed lists, picked by index so they are stable between reloads.
- **Baked terrain tone.** `shade.ts` computes a brightness multiplier per terrain vertex: hillshade
  from a lower light than the sun, darker on steep ground and in hollows, brighter on rises, plus a
  faint low-frequency wobble. It is stored as vertex colours, so it costs nothing at runtime and
  hills read as hills even when the sun is high. Terrain normals are smoothed across tile seams by
  sampling the neighbouring tile (`HeightSource`); when a neighbour arrives the colours and normals are
  re-sent in place (`restitchTerrain`).
- **Painted ground cover.** `cover.ts` rasterises each tile's land-cover polygons (wood, park,
  farm, sand, urban, industrial) into a per-tile canvas texture at 4 texels per metre. Water is not
  painted: see below. A tile's fine relief is an object-space normal map (`bakeNormalMap`), lighting
  only.
- **Water** (`water.ts`) is translucent geometry, not paint. Lakes and rivers are carved into the
  ground by the importer and drawn as a surface at the water level: every ground sample is a vertex,
  only cells touching water are triangulated, and a per-vertex depth (`water − ground`) is
  interpolated, so the shoreline is where depth crosses zero and islands stay dry. Streams are
  ribbons lifted 0.1 m over the terrain with a depth profile across them. One shared WGSL
  `ShaderMaterial` tints shallow to deep, drifts stepped light bands over the surface (along the
  stream's length for streams), draws a breathing foam line where depth is under ~0.35 m and fades
  alpha to nothing at the edge; it supports fog. It reads no depth texture, so it needs no extra
  pass. The geometry's `custom` attribute carries what the shader needs: depth in x, and the stream
  flow (along, across, is-stream) in yzw. The engine's clock drives the animation; `setWaterLight`
  points the reflection and glint at the palette's sky and sun.
- **Roads** are draped ribbons that follow the ground every metre, with a casing and fill layer, and
  round discs at joints. They share the terrain's ramp and sit over it by a depth bias per layer.
- **Trees** are instanced: one `InstancedMesh` per archetype per tile (at most ten draw calls per
  tile), built from merged unit-space parts with vertex colours. The parts are low-poly: a crown
  sphere is a 42-vertex icosphere, a cone is seven-sided, trunks have six sides. Normals are smoothed
  per vertex, so the faces read as bold shapes rather than a faceted mesh. Trees have their own small shader (`FOLIAGE_WGSL` in `trees.ts`): the same four cel
  bands, plus a 4×4 ordered dither that removes pixels within 2.5 m of the eye, so standing in a crown,
  or behind a felled one, you still see what you aim at. Trees take no shadow. A felled tree's instance
  is scaled to nothing in place.
- **Materials are shared** where they can be (per colour for buildings and terrain). Materials cost no
  shader compile in this engine (only the render state picks a pipeline), but sharing keeps the table
  small; only a tile's own cover texture and instance buffers are per-tile and are freed with it
  (`disposeTile`).

## Effects

Short-lived things are not meshes. They are records on the GPU, drawn by the engine's effects pass
after the solid scene, with the scene's depth bound read-only so they can be soft where they meet it.

- **`Effects`** (`effects.ts`, over the engine's `FlashEffects`) is every muzzle flash, tracer, spark,
  fireball and dust cloud. Each is one 80-byte record written when it is spawned, a formula of its
  age (`at + direction × (along·p + √p·sqrtAlong) + up × (lift·p − drop·p²)`, radius `size + grow·p`)
  that the vertex shader evaluates, so nothing is touched per frame and a shotgun blast is one or two
  draws instead of 60 meshes with 60 materials. A "ball" is a flat disc; a tracer is a camera-facing
  strip. `bright` pieces ignore fog.
- **Half resolution.** An effect with `halfResolution = true` (the alpha smoke pool) draws into a
  half-width, half-height target, cleared to nothing, with no depth attachment: its soft-particle
  fade reads the nearest of the four full-resolution depths each pixel covers, so smoke errs towards
  hiding at an edge rather than painting over what is in front. The target holds premultiplied colour
  and coverage and is blended over the image at the start of the effects pass. Smoke is big, soft
  overdraw whose cost goes with pixels covered, so this quarters it (the barrage of a dozen overlapping
  wrecks and shells at 2560×1600 went from about 35 ms to about 12 ms of GPU time). Small hard
  particles (sparks, gravel) are in the full-resolution additive pool and stay sharp.
- **`Particles`** (`particles.ts`, over the engine's `ParticleSystem`): smoke, dust, gravel, spray,
  sparks and flames, 4096 alpha-blended and 2048 additive. A particle can change colour over its life
  (`colorEnd`: fire cooling to ember red, soot greying), spin, stretch along its velocity (`stretch`:
  sparks), be multiplied past 1 (`glow`, so it blooms) and have a billowy noise-eaten edge
  (`shape: 'puff'`) instead of a smooth disc. The CPU only writes a 160-byte emit command
  per `spawn`; a compute pass expands commands into a pool and another integrates every live particle
  (drag, gravity, bounce on a ground height field, size, fade), and each blend class draws with one
  indirect call whose instance count the GPU wrote. When the pool is full, new particles are dropped.
  Particles are soft where they meet the ground and fogged with distance. Bouncing needs ground
  heights: the wrapper samples `groundAt` into a 256 m, 2 m grid around the camera and re-sends it
  after 32 m of travel (only once something has asked to bounce). `alive` is a tally kept on the CPU,
  for the explosion's budget, and does not need a read-back.
- **`ProjectileTrails`** (`trails.ts`, over the engine's `Ribbons`): smooth camera-facing ribbons
  behind rockets, shells, bombs and bullets, in a `RibbonStyle` per projectile kind (a hot additive
  core and a pale smoke line for a rocket, a bright tracer for a bullet). The CPU lays a point down
  each half metre or so; `update`/`extend` are CPU only and `prepare` writes this frame's segments as
  one storage buffer drawn instanced. A trail whose owner is gone (or stops being extended for half a
  second) is kept and fades; its segments' ages are written fresh each frame, so nothing is rebased.
- **Blast lights and shockwaves:** `scene.lights` (the engine's `PointLights`) takes short-lived point
  lights; `Game.blast` flashes one per explosion. The standard shader adds the strongest eight (by
  strength over distance from the camera) to lit surfaces, without shadows. The shockwave is a `Flash`
  with `ring: true`: a flat ring on the ground whose band is `length` metres thick.
- **Bloom** is an engine pass chain, not a mesh: `Scene.bloom` (`Bloom`: `strength`, `threshold`,
  `knee`, `radius`) extracts what is brighter than the threshold from the resolved HDR image at half
  size, blurs it down a mip chain (13-tap downsample, 3×3 tent upsample added back) and the output pass
  adds the result before exposure and tone mapping. HDR colours over 1 (`glow`) are what feed it. It
  is the `bloom` look option, off by default on touch devices.
- **Skidmarks** are an ordinary `Geometry` + WGSL `ShaderMaterial` with a ring of quads; a new quad is
  written with `needsUpdateRange` (four vertices go to the GPU) and fades by `now − birth` in the
  fragment shader.

Tone mapping applies to the whole HDR image, effects included (the engine has one output pass), so
unlike three's `toneMapped: false` materials a tracer's or the gunsight's colour is rolled off a
little by the soft palette's Neutral tone map: a pure yellow comes out about 12% dimmer.

## Light

One hemisphere light (sky colour above, ground bounce below) and one shadow-casting sun (about 42°
up, offset `[70, 80, 55]` m from its target). There is no point lighting. Background and fog share the
sky colour; fog runs 150–600 m.

The sun's shadow map is a square orthographic frustum centred on the camera's target. Its centre is
snapped to whole texels in the light's view, so the edges do not crawl as the target moves, and the
comparison sampler does a 3×3 PCF whose spread is `shadow.radius` texels (a uniform, so softness is not
a recompile) with the lookup moved along the surface normal to avoid acne. Transparent meshes cast no
shadow; meshes outside the sun's frustum are skipped.

## Soft-lighting options

Five independent switches, in `render/look.ts` (`Look`). The HUD panel in `App.vue` shows them while
the pointer is not captured; `Game.setLook` applies and saves them to `localStorage`
(`ragnarok.look`). Defaults are on, except far shadows, which start off on coarse-pointer (touch)
devices (bloom likewise). (Ambient occlusion was removed with the move to the engine: it added little to a
flat-shaded look and cost a second geometry pass.)

The saved choice carries a version (`LOOK_VERSION` in `look.ts`). When a default changes for existing
players, bump it and list the flag in `RESET_ON_BUMP`: older saves then drop that flag, so players
who never chose it get the new default. Ink is listed, so everyone starts with outlines on, including
players who had turned them off; a saved `ao` is ignored.

### Soft light (`soft`)

Swaps the `Palette` (`CLASSIC` ↔ `SOFT` in `look.ts`). Applied by `Scene.setLook`:

| | Classic | Soft |
|---|---|---|
| Sky / fog | `#9fd3f0` | `#b3dcf4` |
| Hemisphere sky / ground | `#dff3ff` / `#6b8f4e` | `#eaf4ff` / `#9bb871` |
| Hemisphere intensity | 1.0 | 1.5 |
| Sun colour / intensity | white / 2.4 | warm `#fff0d6` / 1.8 |
| Tone mapping | none | Neutral |
| Shadow blur radius | 1 | 4 |

The idea: weaker sun and stronger, warmer fill so shaded faces keep colour instead of going grey, and
a tone map that rolls off highlights without shifting hues (Khronos Neutral, chosen over ACES because
it keeps authored toy colours). Switching it off restores the original look exactly.

### Contact shadows (`contact`)

`ContactShadows` (`contact.ts`) is one `InstancedMesh` of up to 128 flat quads textured with a soft
radial falloff, drawn transparent, unlit and without depth writes. Each frame `Game` calls `begin()`,
adds one blob per live character and vehicle, then `end()`. A blob is

- placed on the terrain at `(x, z)`, lifted 4 cm to avoid z-fighting (plus a depth bias),
- tilted to the terrain's slope from four height samples,
- sized from the vehicle's chassis half extents (`halfExtentsOf`), 0.6 m for a character,
- turned to the vehicle's yaw, and
- shrunk with height above the ground and gone by 24 m, so a helicopter does not leave a blob far
  below it.

It only knows the terrain height, so someone on a roof gets their blob on the ground beneath, hidden.
It is a separate thing from the sun's shadow: the shadow map is too coarse and offset to ground small
objects, the blob darkens the spot right under them.

### Ink outlines (`ink`)

Part of the engine's output pass (`engine/src/passes/output.wgsl.ts`), not a pass of its own: it reads
the scene's depth while the frame is converted for the screen. It draws a dark line wherever the
picture's depth bends sharply: a silhouette (a tree against the sky, a car against the grass) or an
edge where one thing sits in front of another. Each pixel's view depth is compared with its four
neighbours, `width` CSS pixels (1.2) away. The test is the second difference, `l + r − 2c` and
`d + u − 2c`, relative to the pixel's own depth, against a threshold of 0.05. A smooth surface, even a
long slope of ground seen at a low angle, changes its rate but does not bend, so it stays clear of
ink. The line darkens whatever it is drawn over (to 30% of its brightness, blended at 60%) rather than
painting a fixed colour, so the outline keeps the hue of what it outlines.

Things to know:

- **Only silhouettes and depth jumps are outlined.** Creases with no depth change, such as the seam
  of a box, are not, so the lines come from geometry, not from normals.
- **Overlays and effects have no depth**, so they get no ink and do not break an outline.
- `Ink` (`darken`, `strength`, `threshold`, `width`) is a plain object on the scene; `Scene.setLook`
  sets it to `null` when the option is off, which skips the work in the shader.

### Far shadows (`far`)

Changes the sun shadow's frustum and map (`SHADOW_NEAR` / `SHADOW_FAR` in `scene.ts`):

| | Near | Far |
|---|---|---|
| Frustum side | 100 m | 300 m |
| Map size | 2048² | 4096² |
| Texel size | ≈ 0.05 m | ≈ 0.07 m |
| Caster range either side of target | 200 m | 300 m |

Switching sizes makes a new shadow map on the next frame, so there can be a one-frame hitch. The far
setting is about four times the shadow memory and fill cost.

## The gunsight

`GunSightView` (`gunsight.ts`) is a tank gunner's reticle: a crosshair along the barrel with a ladder
of range marks. Its parts are meshes with `overlay = true`: the engine draws them in a pass after the
effects pass, ignoring depth, so smoke or a wall in front never hides them. They are laid out in view
space under one group that `follow(camera)` puts on the camera each frame (`Scene.render` does it), so
the reticle is "in front of the camera at distance 1" exactly as the old overlay camera had it. The
ladder's bars are pooled, so a frame makes no new meshes.

## Performance notes

Measured in Chrome on a real GPU (Metal), 60 fps, the default look (soft, contact shadows, far
shadows and ink on), the same spot of the same world with no server connected, `?perf` readout:

| | master (three, WebGL) | engine |
|---|---|---|
| draw calls | 315 | 70 (18 of them the shadow pass) |
| triangles | 927k | 936k |
| renderer CPU time | 0.76 ms/frame | 0.29 ms/frame |
| GPU time by pass | n/a | shadow 0.8, main 1.1, effects 0.7, output 0.9 ms |

With a server and a crowd master measured about 840 draws and 1.25 ms (see the notes on the WebGPU
port, below); the engine's cost scales with the number of changed objects, not drawn ones. The
readout (F3, or `?perf`) shows draw calls, triangles, object records and how many were rewritten that
frame, materials, pipelines, and GPU milliseconds per pass when the browser exposes timestamp queries.

- Draw calls are kept down by instancing (trees, blobs, characters of one model), by building each
  tile's buildings as one mesh with a colour per vertex (a solo view went from 197 draws to 70, the
  shadow pass from 106 to 18), and by `parseToonModel(..., { merge: true })` for the tanks, walker,
  helicopter and planes. A node's lit, opaque pieces are joined into one vertex-coloured mesh, and
  parts with identical geometry (a tank's fourteen wheels) share one copy so they instance across
  every vehicle of the type. `paint`, glass and lamps stay separate, and the nodes stay, so the parts
  the game moves by name still move. With a server and a full set of vehicles in view that took the
  dev world from about 390 draws (250 of them shadow) to about 240 (120 shadow). Cars are not merged:
  `carwear.ts` dents and cuts their body parts by material. Not done: render bundles per tile, a
  cached shadow map for static casters, and GPU culling.
- Pixel ratio is capped at 2.
- The expensive options are far shadows (4096 map) and ink (one full-screen depth read, most of the
  output pass). Far shadows are off by default on touch devices.
- Everything here is a rendering concern only: none of it touches the simulation, so options can be
  changed at any time without affecting prediction or the server.

## History: why an engine

The client began on three.js's `WebGLRenderer` (this document's earlier versions). A port to three's
`WebGPURenderer` (branch `webgpu-renderer`, commit `77673b6`) worked but spent about twice master's
renderer CPU time: three walks the scene graph every frame, keeps a material and bind groups per
object, and makes a handful of native calls per draw, none of which a straight port avoids. That
branch is kept as a reference.

The engine replaces it by design: persistent per-object records, a material table, one geometry
arena, automatic instancing and GPU-side effects, as described in [engine/README.md](engine/README.md).
Its own examples (`engine/examples`) benchmark the same scene against three's two renderers.

## Measuring, and traps

- `engine/scripts/inspect.mjs <url> [waitMs] [screenshot.png]` opens a page in headless Chrome on the
  real GPU, prints console errors and the page's stats and saves a screenshot. `EVAL="js"` runs a
  script in the page first (the game exposes `window.__game` in dev), which is how effects, damage
  numbers and the gunsight were checked.
- Compare builds on the same scene. Tile streaming follows the player, whose spawn comes from the
  server, so a client with no server connection draws different content and its numbers mean nothing
  against a connected one. The server only accepts the dev origins `localhost:5173` and
  `127.0.0.1:5173`.
- Headless Chrome with software rendering is about 100× too slow for timings. Use a real GPU.
- Two dev servers sharing one `node_modules` share Vite's dependency cache; give the second its own
  `cacheDir`, or the two builds' dependency bundles clash.

## Adding to the pipeline

- **A new model or effect:** add its root to the scene in `Game`'s constructor. Make its materials
  with `toonMaterial()` from `toon.ts` to match, set `castShadow` on anything that should cast, and
  call `material.needsUpdate()` after changing a material's colour or opacity. A glTF goes through
  `parseToonModel` (`models.ts`), which swaps in toon materials and keeps their names.
- **A new short-lived effect:** a `Flash` (`FlashEffects.spawn`) if it is a pure function of its age
  (flash, spark, puff, streak, ground ring); a `Particles.spawn` if it needs drag, gravity, bounce or
  to change colour as it ages; a ribbon (`ProjectileTrails.follow`) for a trail behind something that
  moves; `scene.lights.flash` to light the surroundings. Avoid spawning meshes per event.
- **Something that must never be hidden** (a reticle, a marker): a mesh with `overlay = true` and a
  `blend: 'alpha'` material. It ignores depth and sorts by `renderOrder`.
- **A new look option:** add a field to `Look` and `defaultLook`, apply it in `Scene.setLook`, and add
  a row to `LOOKS` in `App.vue`.
- **Palette tuning:** edit `SOFT` / `CLASSIC` in `look.ts`.
- **A new engine feature** goes in `engine/`, with a test or an example page; the engine must not
  import from the game.
