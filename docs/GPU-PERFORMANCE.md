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
  sky, ink, shafts and flare, effects, trees (everything with `levels`) and grass. (Bloom was one
  of them until the pass was removed: [GLOW.md](GLOW.md).)
- **A firefight:** `--parts firefight,firefightBare` adds flashes, fire and a blast to the view, with
  and without their halos; the two numbers' difference is what the glow costs.
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

Wheat on the farmland (CROPS.md) is drawn by the same code as the grass, solid stalks near and an
opaque lid far off. Standing in a field in `home` it is 0.15 ms of a 2.73 ms frame, and nothing away
from one. The `field` view and the `crops` part measure it.

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

### 6. Leafy crowns

The deciduous trees' near solid became a crown of clumps and tufts (about 1,600 triangles, from about
100), opaque, with softer, warmer and cooler shading in the foliage shader ([RENDERING.md](RENDERING.md)).
Quality high (ratio 1.5, 2268 × 1258), the old and new code benched back to back, ms per frame with the
trees' share:

| world, view | before | after | trees before | trees after |
|---|---|---|---|---|
| home, forest | 2.99 | 3.16 | 0.41 | 0.65 |
| home, vista | 2.72 | 2.84 | 0.43 | 0.53 |
| vindeln, forest | 2.85 | 2.92 | 0.00 | 0.06 |
| vindeln, vista | 2.37 | 2.48 | 0.26 | 0.36 |

Then the pine and spruce, against the deciduous-only step, also back to back:

| world, view | trees, deciduous leafy | trees, conifers too |
|---|---|---|
| vindeln, forest | 0.05 | 0.09 |
| vindeln, vista | 0.34 | 0.42 |
| home, forest | 0.65 | 0.68 |
| home, vista | 0.56 | 0.65 |

A pine's two leafy crowns were 3,160 triangles; at 7 clumps each they are about 2,000 (home's vista
0.05 M triangles fewer, its trees' share within the noise). The spruce's tiers are 530.

The bark pattern on near trunks cost nothing measurable (home: forest 3.19 and 3.19 ms, vista 2.92
and 2.93, back to back): it is a few instructions on trunk pixels within 40 m. The birch's airier
crown (2,350 triangles, from 1,600) with its bent trunk and new bark cost nothing measurable either
(vindeln: forest 2.93 and 2.93 ms, vista 2.52 and 2.51). Swaying in the wind, a few lines of vertex
code, cost nothing either (vindeln: forest 2.93 and 2.94 ms, vista 2.52 and 2.52).

- **Leaf cards were not used.** Alpha-tested quads would have given finer leaves at the outline, but
  they discard, which is what made the impostors dear (step 2). The tufts make the outline from
  triangles instead.
- **The impostor bake bins its triangles.** Finding the outline tested each step of it against every
  triangle, and a leafy crown has thousands of both: the bake went from about 0.3 s to over 2 s. Each
  picture now lists its facing triangles under the pixels their boxes touch, and the bake of all kinds
  is about 0.6 s.
- **A level without the tufts from 40 m was dropped**: timed alternately in `home`, it made no difference
  in the forest (0.01 ms) and the vista was 0.07 ms dearer with it (it is a draw a kind more per tile).

### 7. Passes with nothing to draw, and work thrown away

A survey of the frame found work that bought nothing. `home`, quality high (2268 × 1258), ms per frame:

| view | before | after |
|---|---|---|
| forest | 3.04 | 2.90 |
| vista | 2.66 | 2.59 |
| water | 3.32 | 3.12 |
| night | 3.31 | 3.20 |

- **The effects pass and the half-resolution smoke pass ran every frame**, whatever was alive: the
  particle pools are always in the scene, and the passes were gated on that. The effects pass reloads
  and stores the whole image, and the smoke pass clears a target and blends it over. An `Effect` now
  says when it is `idle` (an empty pool, no flashes, no trails), and a pass that every effect is idle
  for is left out. This is most of the gain: the `effects` part was 0.10 to 0.14 ms with nothing in
  the air.
- **The sky panorama** is drawn only in a frame where a material in view binds it (`MaterialResources.watch`).
- **The shafts' target was cleared** every frame they were off, though the output pass does not read it then.
- **`select()` runs both of its sides.** The water chose between a lake's chop and a stream's bands
  with it, so every pixel of either paid for both. It branches now.
- **The fog's tint** (the sky behind it: an arccosine and a table read) was worked out where the fog
  amount was 0, which is everything nearer than the fog's start.
- **A pixel's own screen position** was found by a `viewProj` multiply per pixel in the foliage and
  the water. `Surface.pixel` hands a shader material the rasteriser's.
- **The grass's char** is the same over a blade and was looked up per pixel; its vertex stage, which
  already looks it up, hands it down. Bare stones skip the waterline's noise.

### 8. No `discard` in the main view's shaders

The mirror image is cut off at the water by a `discard`, behind a uniform test, and that statement
was in the standard shader and in every shader material. The standard shader also discards along the
ragged edges of worn paths and asphalt. A tiled GPU decides whether it can skip hidden pixels by the
pipeline, not by the pixel: a shader that can discard is shaded for every hidden layer, whether or
not it ever does. So the ground, the buildings and the grass paid for overdraw.

Taking the four statements out as a probe: forest 2.89 to 2.64 ms, vista 2.55 to 2.38. Built properly:

- The mirror image draws with modules of its own that hold the clip (`standardWgsl(…, reflect)`,
  `mirrorCode`), picked by `pipelineFor`'s variant 3. The main view's hold none.
- Only a worn or asphalt material's pipeline has the edge discard (`edges`).
- **An override constant does not do it.** `if (REFLECT && …) { discard; }` with `REFLECT` false in
  the main pipelines gained 0.04 ms where the probe gained 0.25: the statement has to be absent from
  the shader's text.

`home`, alternated: forest 2.93 to 2.78 ms, vista 2.65 to 2.45, water 3.31 to 3.02. The roads and
paths keep their discard, which is the rest of the probe's gain: an edge that fades by alpha or by
colour into the ground it already samples would win it back.

### 9. The water's shader, part by part

On `vindeln`'s shore the water is 1.8 ms of a 3.6 ms frame. With each part knocked out in turn (the
picture wrong, the timing honest), ms saved: the chop and the gusts 0.75, the swell's slope 0.31, the
lace and the froth 0.29, the mirrored sky 0.20, the ground's light 0.15, the caustics 0.15, the wakes
0.10. What could be saved with the picture unchanged was:

- **The gusts per vertex.** They are patches tens of metres across; two noises a pixel became two a vertex.
- **The lace and the froth only where there is foam** to break up: the shore's line, a breaking crest, a wake.
- **The caustics only through clear water**, and the chop's two finest octaves only where they have
  not faded out with distance.

`vindeln`, water view: 3.60 to 3.17 ms, the picture unchanged.

Then **the chop's noise from a map** (`chopPixels`, `mappedNoise` in `water.ts`). Every octave of the
chop is the same value noise at its own scale, turn and drift, four hashes and their blend each. The
noise is baked once, at load, over a lattice that wraps every 64 cells, 8 texels a cell (512², value
and slope in bytes, with mips), and an octave is one filtered read of it at the mip level the pixel's
span of water asks for. Nothing is made per frame. Each octave repeats (every 580 m for the broadest,
44 m for the finest), but turned and drifting differently they do not line up. The water's character
is the same; its ripples fall elsewhere, and far off it is a little smoother, since the mips average
what the formula left to shimmer. `vindeln`, water view: 3.15 to 2.95 ms.

The map is the material's first texture because a shader material's one sampler is its first
texture's, and the map must repeat; the mirror's lookup is kept a thousandth in from its edges for that.

A river is now drawn as running water (RIVER-SURFACE.md): three octaves of ripples carried by the
current in place of one, its own colour, and its surface lit by how the current shapes it. Timed
alternately against the same frame with the river drawn as a lake, looking down at Vindelälven from
3 to 4 m, that costs 0.19 ms of a 4.24 ms frame in the rapid and 0.25 ms of a 4.43 ms frame over the
pool below it: what carrying the one octave cost before (0.20 to 0.23 ms). The standing waves in its
level cost nothing, since they are where the vertices already were; working them out in the vertex
shader is what cost 1.3 to 1.8 ms (RIVERS.md).

### 10. Opaque draws nearest first (`Renderer.nearFirst`)

For GPUs that are not tiled (integrated Intel and AMD), which shade every hidden layer drawn before
the one in front of it: opaque draws go nearest first in bands of 64 m, and by state within a band,
so they mostly still batch (a town's 139 draws became 237). On Apple's GPU it changes nothing
(within 0.03 ms either way), so it is off there and on for every other adapter. What it wins
elsewhere is not measured: the bench has only been run on a Mac.

## Uploads

Uploads cost the CPU, not the GPU, and only on frames that stream geometry in. The geometry arena
(`gpu/arena.ts`) sends an upload of 64 KB or less with `writeBuffer` from a JavaScript array it keeps,
and interleaves a bigger one straight into a ring of 4 MB staging buffers mapped for writing, copied
in on the GPU when the frame is submitted. A mesh bigger than the ring gets a staging buffer of its
own, made mapped. Main-thread time per upload, interleaving included, in headless Chrome on Metal:

| Upload | `writeBuffer` | Mapped staging |
|---|---|---|
| 16 KB | 0.03 ms | 0.06 ms |
| 256 KB | 0.32 ms | 0.18 ms |
| 1 MB | 1.18 ms | 0.65 ms |
| 4 MB | 1.25 ms | 1.39 ms |
| 16 MB | 3.04 ms | 2.80 ms |

Writing 128 KB into a mapped buffer cost the same whether the buffer was 256 KB or 16 MB, so the
browser does not copy the whole range on `unmap`. Uniforms and the per-frame records (objects, bones,
materials, lights) are small and stay on `writeBuffer`, uploading only the range that changed.
Readbacks (the pass timer, the particle count, the eye's adaptation) rotate their `MAP_READ` buffers,
start mapping after the submit, read the mapped range in place and never wait for the GPU.

## Smoke

Smoke is cheap here because it is drawn at half resolution: 200 puffs 3 to 5 m wide across the whole
view cost 0.13 ms, and a volley of five cannon shots with all their smoke, sparks and dust 0.15 ms
(`--parts gunsmoke`; [GUNSMOKE.md](GUNSMOKE.md) has the table). What was added to smoke is worked out
per particle in the compute pass (the churning air, the wind, the point lights that reach it), where
it costs by the pool's size and not by the pixels covered. The sun's shading of a puff is the one
thing added per pixel, and took the 200 puffs from 0.11 to 0.13 ms.

## Houses with a style of their own

Each building's colours, trim, plinth, weathering, curtains and lit windows
([BUILDING-STYLES.md](BUILDING-STYLES.md)) were added with no new vertex, draw or texture and no
`discard`: the style is a seed in the building's unused vertex fields, used by the wall shader and
by the tile worker's choice of colours. Timed on `vindeln` with the old and the new shader in the
same session, at a street of houses (`--at 'street=-40,5,1.2,-0.05,1.7'`) and with a boarded wall
filling the frame (`--at 'wall=8,20,-1.2,-0.05,1.7'`), with `--extra` variants that switch the
pattern off (`walls = false` on the merged meshes' material) and hide the glass:

| | street | wall |
|---|---|---|
| Frame, old shader | 2.28 ms | 2.06 ms |
| Frame, new shader | 2.30 to 2.31 ms | 2.09 to 2.10 ms |
| The pattern, old (`plainwalls`) | 0.02 ms | 0.02 ms |
| The pattern, new | 0.04 ms | 0.04 to 0.05 ms |
| The glass (`noglass`), old and new | 0.08 ms | 0.08 to 0.09 ms |

So the walls cost about 0.02 ms more than they did, 1% of the frame and about what two runs of the
same build differ by; the curtains and lamps measure nothing. Half of the extra is the tint and the
building's own numbers, half the plinth and the weathering (with both switched off the pattern was
0.03 ms).

- Drawing the building's numbers with eight hashes a pixel or with two (`wallOwn` gives four from
  one) measured the same, as did working out the boards' grain only within a few metres instead of
  at every distance. The hashes and a noise or two are not what a wall pixel costs.
- The glass's `glow` flag is one more test of a uniform flag in the standard shader, for every
  pixel of every material. It did not measure.
- **Corner boards** on boarded houses (a wall's two ends in each vertex's uv, a `min`, a compare and
  a blend near a corner) did not measure either. With them, and with the board's colour forced off
  in the shader, in runs one after the other: 2.30 to 2.31 ms against 2.30 ms on the street, 2.10
  to 2.11 ms against 2.10 ms at the wall, and the pattern (`plainwalls`) 0.03 to 0.05 ms both ways.

## Tried and dropped

- **Half-precision floats (`shader-f16`) for the water's colour maths.** The tint, the mirror's blend,
  the foam and the alpha were worked out in `f16` (Metal's `half`), with places, times, the waves and
  the glint's steep powers left in `f32`. Timed alternately with the `f32` shader on `vindeln`'s shore:
  3.01 ms against 3.02 ms, and 3.01 ms for the same frame timed twice, the picture unchanged. What a
  water pixel costs is its texture reads and the waves' sines, which stay `f32`; the maths that can be
  halved is too little of it to show. Of the other newer WGSL features Chrome 157 has, none bears on
  what a frame costs here: `immediates` replace per-draw uniforms, which the draw list already does
  without; `subgroups` speed reductions in compute, which are 0.1 ms in all; `fragment_depth` keeps the
  early depth test for a shader that writes depth, and only the full-screen depth copy does;
  `buffer_view` and `swizzle_assignment` change how code is written, not how fast it runs.
- **`rg11b10ufloat` for the HDR image** (4 bytes a pixel, not 8, where the device has
  `rg11b10ufloat-renderable`): slower on Apple's GPU, forest 3.88 → 4.37 ms, so it was reverted.
- **A depth pre-pass for everything opaque:** a tiled GPU already shades each pixel of opaque,
  non-discarding surfaces once (hidden surface removal), so only alpha-tested materials gain.
- **Variants of the standard shader** (a plain one for characters, vehicles and buildings without the
  ground's stones, marsh, craters, worn and asphalt code): not built. Those branches are on uniform
  material flags and stay coherent, and the ground, which takes most of the screen, needs all of them.
- **A cached static shadow map:** not built. The whole shadow pass is 0.13 ms at this size. The far
  shadow map (RENDERING.md, "Far shadows") is the cheap half of the idea: it is redrawn only every
  fourth frame.

- **The multisampled depth, not stored.** The main pass stores its 4× depth so a pass can copy one
  sample out for the effects, the ink and the shafts. Discarding it and skipping the copy (the picture
  broken) is the most that could be won: forest 2.74 to 2.65 ms, vista 2.41 to 2.30. A second colour
  attachment holding depth would give part of that back and cost the ink its exact edges.
- **No depth pre-pass for the trees:** forest 0.28 ms slower, vista the same. It stays.
- **The trees' switch nearer (60 m) or further (160 m):** both slower, by 0.03 to 0.09 ms.
- **A cheaper mirror image** (one sample a pixel, one shadow tap): the whole reflection is 0.01 to
  0.04 ms in `home` and on `vindeln`'s shore, so there is nothing to win.

## Shadows: two maps, stepped sun, gap-free filter

`vindeln`, quality high, ms the shadows cost (`--parts shadows`), and the whole frame:

| | forest shadows | forest frame | vista shadows | vista frame |
|---|---|---|---|---|
| before: one 300 m map at 4096² | 0.40 | 2.92 | 0.37 | 2.49 |
| near 100 m + far 600 m, both 2048², far drawn every frame | 0.77 | 3.28 | 0.58 | 2.71 |
| the same, far drawn every 4th frame | 0.42 | 2.95 | 0.32 | 2.46 |
| near map alone (far off) | 0.29 | | 0.21 | |

The far map's cost is its casters, not its texels: halving its size saved 0.03 ms and narrowing it to
400 m saved 0.1 ms. The new filter (Castaño's, 9 taps) costs what the old 3×3 did; one tap instead
saves 0.05 ms.
- **GPU culling into indirect draws, and Hi-Z occlusion:** not built. The renderer's CPU time is about
  0.3 ms, WebGPU has no multi-draw-indirect (each indirect draw is still a call), and the pre-pass
  already makes hidden tree pixels cheap.
- **Every tree a solid, with no impostors:** 0.3 ms cheaper on this GPU before the leafy crowns, at
  0.5 M more triangles in a town's trees and more in a forest. With the leafy crowns it is 3 to 5 ms
  dearer in `home` (`--parts solids`), so the impostors now earn their place here too. Weaker GPUs pay more for triangles, and the CPU loop over
  trees grows with the square of the distance, so the impostor stays past 80 m.
