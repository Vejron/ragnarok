# GPU cost on high-resolution screens

On a Retina laptop the game's frame cost is pixels and memory traffic, not draw calls: the renderer
spends about 0.3 ms of CPU a frame, and a 3024 × 1964 drawing buffer is six million pixels, drawn
with 4× multisampling into 16-bit float colour. This write-up covers how that cost is measured, where
it went, what was changed and what was tried and dropped.

## Measuring: `client/scripts/gpu-bench.mjs`

```sh
node client/scripts/gpu-bench.mjs --breakdown              # the world in worlds/active.json
node client/scripts/gpu-bench.mjs --world klappudden --views forest,water,night
```

It drives the real game in headless Chrome over Metal, in a 1512 × 982 window at device scale 2, so
the drawing buffer is what a 14" MacBook Pro's is (the game's own UI takes some height: 3024 × 1678).
At each view it stops the game's loop and times `--frames` renders to
`device.queue.onSubmittedWorkDone()`: whole frames to the GPU's finish. Headless Metal's per-pass
timestamps are misattributed (a quarter-size pass can show 2.7 ms), so they are not used. Each variant
is timed alternately with the full frame, `--rounds` times, and the medians are printed, so drift over
the run falls on both alike.

- **Views:** `forest` (the spawn), `vista` (the highest ground within 350 m), `water` (the nearest shore,
  looking out) and `night` (the spawn at night with the headlamp on).
- **`--breakdown`:** times the frame with each costly part turned off in turn: shadows, reflection,
  sky, bloom, ink, shafts and flare, effects, trees (everything with `levels`) and grass.
- **Your own variants:** `--extra 'name=<js>'` adds one. The JS sees `g` (the game), `r` (the renderer)
  and `e` (the engine scene), and returns a function that undoes it.
- **Screenshots:** a screenshot of each view goes to `client/bench-out/` (untracked), read back from the
  canvas. `--shoot` also saves one per variant.
- **Heavy worlds:** on Kläppudden at device scale 2, reading the canvas back wedges the page. Time it with
  `--no-shots`, and take the pictures with `--scale 1`.
- **Logging:** `--verbose` prints each view and timing as it goes.

## Where the time went

`docksta`, quality high, 3024 × 1678 at pixel ratio 2, M-series Mac, ms per frame on the GPU:

| step | forest | vista | night |
|---|---|---|---|
| before | 5.17 | 3.95 | 5.59 |
| 1. the multisampled colour never leaves the tile | 4.85 | 3.81 | 5.48 |
| 2. depth pre-pass for trees | 3.88 | 3.25 | 4.32 |
| 3. pixel ratio 1.5 with sharpening (the new `high`) | 2.48 | 2.09 | 2.71 |

The breakdown before (ms each part cost): trees 2.56, sky 0.79, effects 0.50, bloom 0.44, shadows
0.26, ink 0.20, grass 0.19, reflection 0.16 (no water in view), shafts and flare 0.05. After step 2:
trees 1.40, bloom 0.32, sky 0.26, effects 0.17, shadows 0.13, ink 0.10, grass 0.03.

### 1. The multisampled colour never leaves the tile

The main pass used to store its 4× HDR image whenever an effect existed, which is always, and the
effects pass loaded it again and resolved it at its end: about 160 MB written and read back a frame at
this size. Now the main pass always resolves as it ends (`storeOp: 'discard'`), so on a tiled GPU the
samples never reach memory. Particles, ribbons, flashes and the overlay draw at one sample a pixel onto
the resolved image.

They still need the depth: a short pass copies the multisampled depth's first sample into a single-sample
depth texture (`sceneDepth` in `renderer.ts`). The effects pass binds that read-only, so their depth test
is unchanged. Everything else that reads depth after the main pass (ink, shafts, beams, the flare, soft
particles) reads it too, a quarter of the memory. The overlay's pipelines are single-sampled.

### 2. Depth pre-pass for alpha-tested surfaces (`ShaderMaterial.depthPrepass`)

Tree impostors are quads stacked many deep in a forest, alpha-tested from three pictures. A tiled GPU
removes hidden pixels for free only for surfaces that cannot discard. So every layer was shaded: three
pictures' colour, normal and depth reads and the lighting. Drawing every tree as an impostor cost 0.57 ms
*more* than the default, and solids out to 120 m cost 0.47 ms less.

A material with `depthPrepass` is now drawn twice in the main pass:

1. **Depth only**, with the override constant `PREPASS` 1. The foliage reads only the pictures'
   coverage, decides what it discards and returns.
2. **Shaded** with depth compare `equal` and no depth write (`PREPASS` 2). Only the surface in front
   passes, it discards nothing, and each pixel is shaded once.

The vertex position is `@invariant`, so both draws land on the same depth. The reflection draws the
material once, as before. The trees went from 2.3 ms to 1.4 ms, with no change to the picture.

### 3. Fewer pixels, sharpened

Each pass's cost goes with the pixels, so a lower pixel ratio is the largest lever left: 1.5 instead of
2 is 56% of the pixels and 36% less GPU time. The browser stretches the canvas to the screen, which
softens it, so the output pass now sharpens with AMD's contrast-adaptive sharpening (`Renderer.sharpen`,
set by `scene.ts` from how far the ratio is below the screen's). Its four extra reads cost 0.06 ms. At
1:1 on a 2× screen the difference from ratio 2 is hard to see; the trees' dither while they fade is a
little coarser. The `high` preset's `maxPixelRatio` is now 1.5, as `medium`'s was. The adaptive scale
still lowers it further while frames are slow.

### 4. The sky the water mirrors, once a frame (`SkyPass.drawPanorama`)

The water worked out the whole sky for every one of its pixels that mirrored it: haze, stars, the
clouds (four reads of the cloud map and their light), the discs and the rainbow. A compute pass now
draws the sky's backdrop above the horizon into a 256 × 256 panorama each frame (`Renderer.skyPanorama`,
the half-sphere folded into a square by `skyPanoramaUv`). The water reads it once a pixel and adds the
sun's and moon's discs exactly (`skyFrom`), since they are too small for its texels. `skyColor` is now
`skyFrom(dir, skyBackdrop(dir, lod))`, the same sum regrouped, so the sky itself is unchanged.

Kläppudden's shore view went from 5.08 to 4.05 ms at ratio 1.5, and the water looks the same.

`klappudden`, quality high (ratio 1.5, 2268 × 1258), ms per frame:

| view | before the panorama | after |
|---|---|---|
| forest | 3.88 | 3.96 |
| water | 5.08 | 4.05 |
| night | 4.04 | – |

The breakdown at the shore before it: sky 1.27 (most of it the water's reflection of it), reflection
0.33, effects 0.29, shadows 0.29, bloom 0.29, ink 0.19, grass 0.14.

### 5. Solid trees further out, and impostors that look like them

Trees changed to impostors at 40 m, and the pictures showed: wobbling outlines, spikes on the crowns,
a second outline round a tree while it faded and a screen pattern over it. Four changes, measured on
`home` at quality high (ratio 1.5, 2268 × 1258), ms per frame, with the trees' share (the frame with
them hidden, timed alternately):

| view | before | after | trees before | trees after |
|---|---|---|---|---|
| forest | 3.41 | 3.25 | 0.61 | 0.44 |
| vista | 2.92 | 2.86 | 0.59 | 0.54 |

- **The switch is at 80 m, not 40** (`TREE_LEVELS_FROM`), the fade 20 m. A solid is opaque, so the
  GPU shades each of its pixels once and its edges are anti-aliased; an impostor's pixels are three
  pictures' reads. Twice as far out cost almost no triangles (1.840 to 1.845 M in the forest) and
  saved 0.2 ms. Every tree as a solid saved 0.3 ms, at 0.5 M more triangles: the levels stay for
  weaker GPUs, which take `treeDetail` 0.7 and 0.5.
- **The pictures frame the tree's cylinder and store a distance to the true outline** (`impostors.ts`),
  and their views crowd the horizon. These change only what is baked (130 to 300 ms, once) and where a
  picture is looked up: the trees' share was 0.61 ms before and 0.63 ms after, within the noise.
- **The fade and the impostor's edge are counted out in samples** (`ShaderMaterial.coverage`): the
  fragment writes a sample mask, the impostor taking a share of a pixel's four samples and the solid
  the rest. The mask is written in the depth pre-pass, which discards already, so the GPU loses no
  hidden-surface removal by it; the shaded draw tests depth `equal` a sample and needs no mask.

`gpu-bench.mjs --parts solids,impostors,trees2x` draws the trees those ways, and `--quality low`
runs the preset with one sample a pixel.

## Smoke

Smoke is cheap here because it is drawn at half resolution: 200 puffs 3 to 5 m wide across the whole
view cost 0.13 ms, and a volley of five cannon shots with all their smoke, sparks and dust 0.15 ms
(`--parts gunsmoke`; [GUNSMOKE.md](GUNSMOKE.md) has the table). What was added to smoke is worked out
per particle in the compute pass (the churning air, the wind, the point lights that reach it), where
it costs by the pool's size and not by the pixels covered. The sun's shading of a puff is the one
thing added per pixel, and took the 200 puffs from 0.11 to 0.13 ms.

## Tried and dropped

- **`rg11b10ufloat` for the HDR image** (4 bytes a pixel, not 8, where the device has
  `rg11b10ufloat-renderable`): slower on Apple's GPU, forest 3.88 → 4.37 ms, so it was reverted.
- **A depth pre-pass for everything opaque:** a tiled GPU already shades each pixel of opaque,
  non-discarding surfaces once (hidden surface removal), so only alpha-tested materials gain.
- **Variants of the standard shader** (a plain one for characters, vehicles and buildings without the
  ground's stones, marsh, craters, worn and asphalt code): not built. Those branches are on uniform
  material flags and stay coherent, and the ground, which takes most of the screen, needs all of them.
- **A cached static shadow map:** not built. The whole shadow pass is 0.13 ms at this size.
- **GPU culling into indirect draws, and Hi-Z occlusion:** not built. The renderer's CPU time is about
  0.3 ms, WebGPU has no multi-draw-indirect (each indirect draw is still a call), and the pre-pass
  already makes hidden tree pixels cheap.
- **Every tree a solid, with no impostors:** 0.3 ms cheaper still on this GPU, at 0.5 M more triangles
  in a town's trees and more in a forest. Weaker GPUs pay more for triangles, and the CPU loop over
  trees grows with the square of the distance, so the impostor stays past 80 m.
