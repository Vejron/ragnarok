# Rendering

How the client turns the simulation into a picture: the scene, the light, the shading style, the
optional soft-lighting passes and what each one costs. It is all three.js (WebGL) in
`client/src/render/`; `client/src/game.ts` drives it once per animation frame. Networking and
prediction are in [NETWORKING.md](NETWORKING.md), where the world comes from is in
[WORLD.md](WORLD.md).

```
game.frame()
  ├─ fixed ticks, interpolation          (not rendering)
  ├─ update vehicles / characters         poses written onto their models
  ├─ scene.follow() / aimFrom()           camera + sun shadow frustum follow the target
  ├─ tiles, effects, pickups, rockets     streamed in / animated
  ├─ contact.begin() … add() … end()      blob shadows for this frame's poses
  └─ scene.render()
        ├─ AO and ink both off:  renderer.render()                      → canvas
        └─ AO or ink on:  EffectComposer
                     RenderPass     scene → 4× MSAA half-float target + depth texture
                     InkPass        depth bends sharply → dark outline   (if ink)
                     GTAOPass       depth + normals → AO → multiplied in (if AO)
                     OutputPass     tone mapping + sRGB                    → canvas
```

## The scene

`render/scene.ts` owns the `THREE.Scene`, the camera, the renderer and the lights. Everything else
adds a `root` group to it:

| Piece | File | What it is |
|---|---|---|
| Ground plane | `world.ts` `buildGround` | One grass plane under the world; shows only where a tile is late. |
| Tiles | `world.ts` `buildTile` | Terrain mesh, buildings, roads and trees of one 250 m tile. Streamed in and out around the player. |
| Vehicles | `car.ts`, `tank.ts`, `heli.ts` | glTF models from `client/public/models/`, one clone per vehicle. |
| Characters | `character.ts` | Skinned clones of one shared model (`Crowd`), outfit colour by player id. |
| Items, pickups, rockets | `items.ts`, `pickups.ts`, `rockets.ts` | Small hand-built meshes. |
| Effects | `effects.ts` | Muzzle flashes, tracers, fireballs, smoke: translucent `MeshBasicMaterial`s that fade out. |
| Trajectory | `trajectory.ts` | The arc a held grenade would take. |
| Field mortar | `mortar.ts`, `mortar-arc.ts` | A hand-built gun (no glTF), and the landing marker its gunner aims by. See [VEHICLES-V.md](VEHICLES-V.md). |
| Contact shadows | `contact.ts` | Blob shadows, below. |

The camera is a 75° perspective camera, near 0.25 m, far 2000 m. `Scene.follow` orbits it behind a
target by yaw/pitch (over the right shoulder on foot, further back per vehicle); `aimFrom` is the
gunner's view along a barrel. Both clamp it above the terrain via `groundAt`, and both also move the
sun so the shadow map stays centred on what the camera follows.

## Look: toy shading

The style is flat, bright and few-banded, not physically based.

- **Toon materials.** Almost everything is `MeshToonMaterial`, made through `toonMaterial()` in
  `toon.ts`. That gives it the *cel ramp* (`CEL_RAMP`): a 1×4 texture of four light levels, 96, 150,
  205 and 255, with nearest filtering, so each band is hard-edged and nothing is blended between
  them. A material that brings its own ramp keeps it: terrain and roads use an eight-step ramp
  (`TERRAIN_RAMP` in `world.ts`, dark and coarse in shade, fine near the light). Buildings, vehicles,
  characters and tree crowns all use the cel ramp. Lamps (headlights) are `MeshBasicMaterial`, so they
  stay bright.
- **Candy palette.** Building colours, roof colours, cover colours (`cover.ts`) and player outfits
  (`character.ts` `PALETTE`) are fixed lists, picked by index so they are stable between reloads.
- **Baked terrain tone.** `shade.ts` computes a brightness multiplier per terrain vertex: hillshade
  from a lower light than the sun, darker on steep ground and in hollows, brighter on rises, plus a
  faint low-frequency wobble. It is stored as vertex colours, so it costs nothing at runtime and
  hills read as hills even when the sun is high. Terrain normals are smoothed across tile seams by
  sampling the neighbouring tile (`HeightSource`).
- **Painted ground cover.** `cover.ts` rasterises each tile's land-cover polygons (wood, park,
  farm, sand, urban, industrial) into a per-tile canvas texture at 4 texels per metre. Water is not
  painted: see below.
- **Water** (`water.ts`) is translucent geometry, not paint. Lakes and rivers are carved into the
  ground by the importer and drawn as a surface at the water level: every ground sample is a vertex,
  only cells touching water are triangulated, and a per-vertex `depth` (`water − ground`) is
  interpolated, so the shoreline is where depth crosses zero and islands stay dry. Streams are
  ribbons lifted 0.1 m over the terrain with a depth profile across them. One shared
  `ShaderMaterial` tints shallow to deep, drifts stepped light bands over the surface (along the
  stream's length for streams), draws a breathing foam line where depth is under ~0.35 m and fades
  alpha to nothing at the edge; it supports fog and tone mapping. It reads no depth texture, so it
  needs no extra pass; the water sits in its own group (`Game.waterRoot`) that the AO pass hides.
  `updateWater(seconds)` drives the animation from the frame loop.
- **Roads** are draped ribbons that follow the ground every metre, with a casing and fill layer, and
  round discs at joints. They share the terrain's ramp.
- **Trees** are instanced: one `InstancedMesh` per archetype per tile (at most ten draw calls per
  tile), built from merged unit-space parts with vertex colours. The parts are low-poly: a crown
  sphere is welded to 57 vertices and 80 triangles (was 183 and 320), a cone to 31 vertices (was 83),
  trunks have six sides. Normals are smoothed per vertex, so the faces read as bold shapes rather than
  a faceted mesh.
- **Materials are shared** where they can be (per colour for buildings and terrain), so streaming a
  tile in never compiles a shader; only a tile's own cover texture and instance buffers are
  per-tile and are freed with it (`disposeTile`).

## Light

One `HemisphereLight` (sky colour above, ground bounce below) and one shadow-casting
`DirectionalLight` (the sun, about 42° up, offset `[70, 80, 55]` m from its target). There is no
point lighting. Background and fog share the sky colour; fog runs 150–600 m.

The sun's shadow map is a square orthographic frustum centred on the camera's target, with
`PCFShadowMap`, `bias −0.001` and `normalBias 0.15`. Three's PCF filter honours `shadow.radius`, so
the edge softness is a runtime uniform, not a recompile.

## Soft-lighting options

Five independent switches, in `render/look.ts` (`Look`). The HUD panel in `App.vue` shows them while
the pointer is not captured; `Game.setLook` applies and saves them to `localStorage`
(`ragnarok.look`). Defaults are on, except AO, which starts off everywhere, and far shadows, which
start off on coarse-pointer (touch) devices.

The saved choice carries a version (`LOOK_VERSION` in `look.ts`). When a default changes for existing
players, bump it and list the flag in `RESET_ON_BUMP`: older saves then drop that flag, so players
who never chose it get the new default. AO is listed for version 2; ink for version 3, so everyone
starts with outlines on, including players who had turned them off.

### Soft light (`soft`)

Swaps the `Palette` (`CLASSIC` ↔ `SOFT` in `look.ts`). Applied by `Scene.setLook`:

| | Classic | Soft |
|---|---|---|
| Sky / fog | `#9fd3f0` | `#b3dcf4` |
| Hemisphere sky / ground | `#dff3ff` / `#6b8f4e` | `#eaf4ff` / `#9bb871` |
| Hemisphere intensity | 1.0 | 1.5 |
| Sun colour / intensity | white / 2.4 | warm `#fff0d6` / 1.8 |
| Tone mapping | none | `NeutralToneMapping` |
| Shadow blur radius | 1 | 4 |

The idea: weaker sun and stronger, warmer fill so shaded faces keep colour instead of going grey, and
a tone map that rolls off highlights without shifting hues (Khronos Neutral, chosen over ACES because
it keeps authored toy colours). Switching it off restores the original look exactly.

### Contact shadows (`contact`)

`ContactShadows` (`contact.ts`) is one `InstancedMesh` of up to 128 flat quads textured with a soft
radial falloff, drawn transparent and without depth writes. Each frame `Game` calls `begin()`, adds
one blob per live character and vehicle, then `end()`. A blob is

- placed on the terrain at `(x, z)`, lifted 4 cm to avoid z-fighting (plus a polygon offset),
- tilted to the terrain's slope from four height samples,
- sized from the vehicle's chassis half extents (`halfExtentsOf`), 0.6 m for a character,
- turned to the vehicle's yaw, and
- shrunk with height above the ground and gone by 24 m, so a helicopter does not leave a blob far
  below it.

It only knows the terrain height, so someone on a roof gets their blob on the ground beneath, hidden.
It is a separate thing from the sun's shadow: the shadow map is too coarse and offset to ground small
objects, the blob darkens the spot right under them.

### Ink outlines (`ink`)

`InkPass` in `render/ink.ts`, a `ShaderPass`, draws a dark line wherever the picture's depth bends
sharply: a silhouette (a tree against the sky, a car against the grass) or an edge where one thing
sits in front of another. Each pixel's view depth is compared with its four neighbours, 1.5 px away.
The test is the second difference, `l + r − 2c` and `d + u − 2c`, relative to the pixel's own depth,
against a threshold of 0.05. A smooth surface, even a long slope of ground seen at a low angle,
changes its rate but does not bend, so it stays clear of ink. An earlier version compared each pixel
with its neighbour directly, and turned the grazing ground nearly black. The line colour is
the line darkens whatever it is drawn over (to 30% of its brightness, blended at 60%) rather than
painting a fixed colour, so the outline keeps the hue of what it outlines.

The pass needs the scene's depth. The composer's target carries a `DepthTexture`, and `InkPass` runs
straight after `RenderPass`, before any pass that swaps the buffers, so the depth it reads is the
scene's own. It linearises depth with the camera's near and far. `threshold` and `width` are uniforms
in `ink.ts` for tuning.

Things to know:

- **It needs the depth of a multisampled target.** The target has 4× MSAA with a depth texture, which
  depends on the driver resolving depth when the buffers are read. If outlines are missing or the
  frame goes black on some GPU, check the console for a shader compile error first. A GLSL local
  named like a built-in (`step`, say) stops the shader compiling, and a failed shader draws nothing.
- **Only silhouettes and depth jumps are outlined.** Creases with no depth change, such as the seam
  of a box, are not, so the lines come from geometry, not from normals.

### Ambient occlusion (`ao`)

Three's `GTAOPass`, run in the same `EffectComposer` chain as ink. The chain is built the first time
either is switched on (`Scene.buildComposer`):

1. `RenderPass` draws the scene into a half-float target with 4× MSAA and a depth texture. The
   canvas's own `antialias` does not apply once rendering goes through a target, hence the explicit
   samples.
2. `InkPass` draws the outlines from that depth (see above). It runs only when ink is on.
3. `GTAOPass` re-renders the scene with a normal-material override for depth and normals, computes
   AO, denoises it and multiplies it into the image. Radius 1.1 m, `scale 1.2`, 12 samples,
   blend 0.85: it is meant to seat things and darken creases, not outline them. It runs only when AO
   is on (`gtao.enabled`).
4. `OutputPass` applies the renderer's tone mapping and the sRGB encode, because the passes work in
   linear light.

Things to know:

- **It costs a second geometry pass.** The composer renders at no more than 1.5× pixel ratio
  (`AO_PIXEL_RATIO`) to limit the fill cost; the result is upscaled to the canvas.
- **Excluded objects.** Translucent or line-like objects would appear as solid shapes in the depth
  picture and leave halos. `Scene.excludeFromAO(object)` registers a root; `GTAOPass.render` is
  wrapped to hide those roots for the length of its own pass only, so they are still drawn in the lit
  picture. Currently the contact shadows, the effects and the trajectory. Add new transparent roots
  here. (Three already hides `Points` and `Line`s itself.)
- **Depth precision.** Near 0.25 / far 2000 is poor for distant depth; AO only matters nearby, so it
  is not visible.
- Toggling AO and ink off keeps the composer built (cheap to re-enable), and `render()` goes back to
  drawing straight to the canvas once both are off.

### Far shadows (`far`)

Changes the sun shadow's frustum and map (`SHADOW_NEAR` / `SHADOW_FAR` in `scene.ts`):

| | Near | Far |
|---|---|---|
| Frustum side | 100 m | 300 m |
| Map size | 2048² | 4096² |
| Texel size | ≈ 0.05 m | ≈ 0.07 m |
| Depth range | 10–300 | 1–450 |

Switching sizes disposes the old shadow map; three recreates it on the next frame, so there can be a
one-frame hitch. The far setting is about four times the shadow memory and fill cost.

## Performance notes

- Draw calls are kept down by instancing (trees, blobs) and sharing materials; characters and
  vehicles are the main per-object draws.
- Pixel ratio is capped at 2 for the canvas, 1.5 for the composer (AO and ink).
- The expensive options are AO (second geometry pass + MSAA target) and far shadows (4096 map).
  Both are off by default on touch devices. Ink is on by default: it adds one full-screen pass over
  the composer's MSAA target, which is most of its cost.
- Everything here is a rendering concern only: none of it touches the simulation, so options can be
  changed at any time without affecting prediction or the server.

## Adding to the pipeline

- **A new model or effect:** add its root to the scene in `Game`'s constructor. If it is
  translucent, add it to `excludeFromAO`. Make its materials with `toonMaterial()` from `toon.ts` to
  match, and set
  `castShadow` on anything that should cast.
- **A new look option:** add a field to `Look` and `defaultLook`, apply it in `Scene.setLook`, and add
  a row to `LOOKS` in `App.vue`. Options that change a material's program (tone mapping does) are
  handled by three; shadow map *type* is not safe to change at runtime, which is why softness uses
  `shadow.radius` instead.
- **Palette tuning:** edit `SOFT` / `CLASSIC` in `look.ts`. AO strength is in `buildComposer`.
