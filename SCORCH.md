# Scorch — making craters visible

Craters (CRATERS.md) dent the ground, but they hardly show. The terrain's shading is a relief map with one texel a ground cell (`reliefPixels`), so on 5 m ground a tank shell's crater, 5.6 m across its radius, is two texels from middle to rim. The terrain's eight-band toon ramp then rounds what slope is left into one band. The dent is there and the physics has it; the picture does not.

This adds marks drawn by the terrain's own shader, per pixel, from the list of craters. They draw the bowl's slope where the mesh is too coarse to, and a burnt patch on the ground.

Status: branch `feat/scorch`. Phases 1-3 are written and tested. Phase 1 was looked at in the game by the user and toned down after; phases 2 and 3 are seen only in the engine's `craters` example and compiled in headless Chrome, not yet played.

**Setting.** All of it is the `scorch` look (`render/look.ts`), "Scorched craters" in the in-game menu next to ink outlines and bloom, on by default. Off, the renderer uploads an empty grid (so no shader looks at a crater) and the craters stop smoking; the craters are still kept, so turning it back on shows every one. The dent in the ground is there either way.

## How it works

- **The list on the GPU.** The engine's `Scene` holds `craters` (`engine/src/scene/craters.ts`): up to `CRATER_CAPACITY` craters in a ring, each two `vec4f` (`x, z, radius, depth` in metres, then birth time in the renderer's clock). Next to it is a hash grid over the ground: square cells of `CRATER_CELL` metres, hashed into `CRATER_BUCKETS` buckets of `CRATER_SLOTS` `u32` (a count, then crater indices). A crater is listed in every cell its scorch reaches. When a bucket is full the oldest crater in it is dropped from that bucket, and when the ring wraps the crater it overwrites is taken out of its buckets first. The renderer uploads only what changed (crater slots and buckets), or everything when it is new (a lost device makes a new renderer, and the list survives in the scene).
- **Bound to every scene shader.** The two buffers are bindings 5 and 6 of the scene group (group 0), so the standard shader reads them, and so can custom shaders (grass, later) and effects.
- **Drawn by the ground's materials.** A material with `craters: true` (the terrain's, the roads' and rails' ribbons, and the flat ground plane's) looks up its fragment's cell, then works out each crater listed there:
  - **Slope.** The bowl is `depth · (1 − t²)²` with `t = d / r`, so its slope is known exactly. The shading normal is bent by it (plus a low lip just past the rim, drawn only), so the bowl reads at any ground resolution. Drawn `CRATER_SLOPE` (1.4) times as deep as it is, to be seen.
  - **The rim's shadow.** Inside the bowl the ray toward the sun is marched (6 steps) to the far side against the bowl's own heights, at the same exaggerated depth; where the rim stands above it, the sun is cut. Under a high sun there is none (a shell's walls are less steep than the light); toward evening a crescent of shadow fills the bowl on the sun's side. The sky's fill is also dimmed toward the middle (to 65%).
  - **Bare earth** in the bowl, **char** in the middle and in short **rays** out past the rim (at most 1.45 radii) (noise around the circle, seeded by the crater's position, so every crater differs and every player sees the same one), and a lighter ring of **thrown-out soil** at the rim.
  - **An ink line** on the rim, about a pixel and a half wide and broken in places like a pen stroke (from the screen-space derivative of the world position, taken before any branch), faded out for craters too small on screen to carry it.
  - Overlapping craters add their slopes (as their dents add) and take the strongest of each mark.
- **Cost.** Every terrain fragment reads one `u32` (the bucket's count); ground with no crater near stops there. Ground with craters loops over the few listed in its bucket.

## Phases

1. **Terrain marks** — the crater list and grid in the engine, the bindings, `MAT_CRATERS`, the shading (slope, rim shadow, earth, char, rays, soil, ink); the client feeds it from `WorldView.cratered` (catch-up craters are born long ago). Tests for the grid (hashing, eviction, the ring wrapping).
2. **Hot craters** — embers glowing in a fresh crater's middle and cooling over `CRATER_EMBER_LIFE` (20 s), from the birth time already in the buffer, nothing per frame; HDR, so they bloom. Smoke rising for `SMOULDER_SECONDS` (30 s) from the game's smoke particles, thick at first and thinning, at most `SMOULDER_MAX` (16) craters smoking at once: a column of puffs leaning downwind (a puff every 0.08 s fresh, 0.45 s cold, alpha 0.6 to 0.25), up to about 1,000 of the 4,096 smoke particles in a barrage.
3. **Grass** — blades round a crater's rim lie flat, away from its middle (`craterGrass`, added to the lean in the grass shader's `displace`), and blades over char or rays are burnt dark, most at the tips (in its `surface`). The bowl's bare earth is still the CPU's `Grass.dent`.
4. **Extras, if wanted** — heat shimmer over a fresh crater; clods of thrown-out dirt.

## Progress

- Toned down after a first look in the game: shorter, sparser rays (0.6 strength), a smaller and less black heart, ink at 0.35, soil at 0.5, slope at 1.4 (was 2); `CRATER_REACH` 1.6 radii (was 1.9).
- Phase 2: done. `CraterMark.heat` in `craters.wgsl.ts`, `CRATER_EMBER` added after lighting in `standard.wgsl.ts`; `render/smoulder.ts` (`Smoulders`), fed by `WorldView.cratered` for live craters and run in `WorldView.update`. `smoulder.spec.ts`: smoke thins and stops, the cap holds. The example has one fresh crater, glowing for its first 20 s.
- Phase 3: done. `craterGrass` (sharing `craterChar` with the ground, so burnt blades lie over burnt ground) in `craters.wgsl.ts`, used by `render/grass.ts`. The grass shader compiled in headless Chrome; not seen drawn.
- Setting: `Look.scorch`, `Scene.setLook` sets `scene.craters.visible`, `Game.setLook` sets `WorldView.scorch`; `Craters.visible` tested in `engine/test/craters.test.ts`.
- Phase 1: done.
  - Engine: `scene/craters.ts` (`Craters`, on `Scene.craters`), `gpu/craters.ts` (upload of what changed), `materials/craters.wgsl.ts` (`craterMarks`, `craterSun`, part of `COMMON_WGSL`), scene bindings 5 and 6 in `renderer.ts`, `MAT_CRATERS` and `Material.craters`, the marks applied in `standard.wgsl.ts` (colours and strengths are the `CRATER_*` constants at its top).
  - Client: `craters: true` on the terrain, ribbons and ground plane (`render/world.ts`); `WorldView.cratered` adds each crater to `scene.craters`; `dispose` clears them.
  - Tests: `engine/test/craters.test.ts` (the hash matches the shader's u32 maths, a crater is listed in every cell it reaches, a full bucket keeps its newest, the ring takes out what it overwrites, clearing); `craters.spec.ts` checks the view hands craters to the scene in metres.
  - Example: `engine/examples/craters/` (`npx vite` in `engine/`, then `/craters/`; `?still`, `?low` for an evening sun). Its main pass with ten craters on screen takes 0.6 ms of GPU at 1280×800 (not compared with none).
  - Not checked: the look in the game itself (the terrain's relief map and cover texture under it, day and night, 1 m against 5 m ground), and the cost with hundreds of craters on screen.

- Profiled (2026-10-26): the example in headless Chrome on the Mac's GPU, camera still, ground over about 80% of the screen, main pass GPU time (`?n=…`, `?plain`, `?off`, `?hot`).

  | 1920×1080 | main pass |
  |---|---|
  | no crater marks (`plain`), and marks on but hidden (`off`) | 0.18–0.19 ms |
  | the example's 11 craters | 0.24 ms |
  | +25 / +100 craters | 0.36 / 0.62 ms |
  | +250 to +4,000 (every bucket full: 15 craters a pixel) | 0.85–0.89 ms |
  | +4,000, all glowing | 0.91 ms |
  | at 2560×1440: none / full | 0.34 / 1.52 ms |

  The cost rises with craters per bucket, not with the number kept, and stops at `CRATER_SLOTS − 1`. CPU is unchanged (the list goes up only when it changes). Not measured: the grass, which runs `craterGrass` per blade vertex.
- Found while profiling, fixed: a full bucket dropped its oldest crater from that bucket only, so under a heavy barrage the crater was cut off square along the cell's edge while its neighbouring cells still drew it. Now `Craters.list` takes it out of every bucket it is in, so the oldest crater goes whole; tested in `craters.test.ts`. As fewer craters are listed then, packed ground costs 0.58–0.61 ms at 1080p (was 0.85–0.89).

- Every explosive now digs (CRATERS.md): rockets 4 m, grenades 3.5 m, autocannon rounds 2 m, hen's eggs 5 m of radius. Craters under `SMOULDER_MIN_RADIUS` (3 m, so the autocannon's) glow but do not smoke, so a burst of them does not put out a shell's smoke.

- Marks fade: a crater's marks (slope, earth, char, soil, ink, the grass's lean and burn) fade out over the last 30% of `CRATER_FADE` (30 minutes) from its birth, and `Craters.expire` then takes it out of the grid. A joiner's craters are born `age` seconds ago. The hole stays in the ground (CRATERS.md, "Lasting ground"). The example has one half-faded crater.

## Open questions

- Ground at 1 m already has the bowl in its relief map, so the slope is drawn twice there (steeper than it is). If it looks wrong, scale the slope by how coarse the tile's ground is (a value the material would carry).
- Flat worlds (no `Ground`) dig no craters, so they get no scorch either. A mark with no dent could be sent for them later.
