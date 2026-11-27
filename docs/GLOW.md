# Glow without bloom, and a pen's ink

Bloom is gone. What it gave gunfire, fire and tracers is drawn where they are instead, and the ink
outlines are drawn as a pen draws them. This is how, and what it costs.

## Why bloom went

Bloom took the bright parts of the whole picture at half size and blurred them down six halving mips,
every frame, whether or not anything glowed: 0.28 ms of a 2.97 ms frame (`umea`, quality high,
2268×1258, an M-series Mac, `gpu-bench.mjs --parts bloom` on the last build that had it, timed
against the same frame with it off). The sun's
glow had already passed to the light shafts and the lens flare, so what bloom still did was soften a
handful of flashes. Those are things the game spawns itself, so each now brings its own glow, and a
frame with nothing burning pays nothing.

## The glow each light brings

- **Halos** (`halo` in `client/src/render/particles.ts`). Every flash (`Effects.flash`) spawns a second
  `glare` particle round it in the `halo` pool: `HALO_SPREAD` (4) times as wide but reaching no more
  than `HALO_REACH` (3.5 m) past the light, `HALO_STRENGTH` (0.16) as bright, in the light's own colour
  and lasting `HALO_LINGER` (1.6) times as long, so a flash dies away and does not blink. The pool is
  additive and drawn at half the screen's size (`new Particles(1024, true, true)` in `game.ts`): a
  halo is big and has no detail, so a quarter of the pixels is enough.
- **A white-hot heart has none.** Most flashes are a pair, a small white heart inside a wider flash
  of the fire's colour. Only the coloured one has a halo (`heart` in `Effects.flash`), so the glow of
  a gun is orange, not white. A flash that is white alone (a spark off steel) keeps a white halo.
- **Fire and lamps.** A blast has one halo round its fireball for as long as it burns (`explosion.ts`).
  A wreck's fire, a burning flare and the lamps under a fuel station's canopy and a dealer's carport
  have a halo spawned every frame that lives two frames (`lampHalo`), so they are as bright at any
  frame rate. Lamps have theirs within `LAMP_HALO_RANGE` (160 m), by night.
- **Sheaths** (`sheath` in `render/trails.ts`). Each hot ribbon behind a rocket, shell, cannon round,
  bullet or flare has a second ribbon `SHEATH_WIDTH` (5) times as wide and `SHEATH_GLOW` (0.12) as
  bright, in the colour the streak cools to, with `soft: true`: a ribbon that is brightest along its
  middle and falls away to nothing at its sides. A rifle's tracer (`Effects.tracer`) has a wide soft
  streak round it the same way, and its core is an HDR colour.
- **A glare is light, not a body** (`engine/src/fx/particles.wgsl.ts`). A soft particle fades where
  it nears the surface behind it, which would wipe out the halo of a round striking a wall. A `glare`
  shows whole on the surface it is cast from and is hidden only by what stands `GLARE_REACH` (0.5 m)
  in front of it. It is also drawn down to a far lower opacity than other particles (`GLARE_FLOOR`):
  by night the eye's adaptation showed where a halo was cut off as a ring.

## Fire is its own light

Fireballs and licks of flame are puffs in the smoke pool, so they billow as smoke does. They were also
shaded by the sun and darkened by night as smoke is, which bloom hid. A particle can now be `emissive`:
the sun does not shade it, lamps do not tint it, and the client does not dim it at night. The blast's
fireball, muzzle and backblast flame, a wreck's flames and a missile's launch fire are.

## White-hot

A colour far past white is light, not paint. The output pass (`whiteHot` in
`engine/src/passes/output.wgsl.ts`) blends a pixel toward its own brightest channel as that channel
climbs from `HOT_START` (2) to `HOT_FULL` (10), after the exposure and before grading, by
`Scene.whiteHot` (1). The heart of a flash burns white and its dimmer rim keeps its colour, in both
palettes; without it the classic palette, which has no tone mapping, clipped such a colour to a flat
saturated patch. The sky is capped well below 2 (`softCap`), so it is untouched.

## The pen's ink

The ink is still a depth test in the output pass (RENDERING.md, "Ink outlines"). Three things changed.

- **It fades with distance.** A line belongs to the nearer of the things it parts. It is up to 1.3
  times as strong close by, easing to its plain strength by 12 m, begins to fade at 0.3 of `Ink.fade`
  (400 m) and is gone by it, so far woods and ridges are not a tangle of lines.
- **It wavers** (`Ink.waver`, 0 to 1). Within the distance where it is at full strength the line
  strays up to about two pixels to either side of the edge it follows, swells to a heavier line along
  0.4 of its way, and now and then runs dry. The pen's hand is noise over the directions of the view,
  so a line keeps its shape as the camera turns and shifts slowly as you walk past, as a line drawn
  again would.
- **Hatching** (`Ink.hatch`, the "Pen hatching" look option, off by default). What a shadow falls on,
  and more lightly what is turned from the sun, is shaded with slanting strokes in the standard
  shader (`hatch` in `standard.wgsl.ts`): each wavers, has its own weight and is broken where the pen
  lifted. The strokes lie in the world on the face's own plane and stay about 7 pixels apart: every
  other one fades out as the surface recedes. It is a trial. It covers what the standard shader draws
  (walls, roads, vehicles, bare ground), not grass or trees.

How the waver stays cheap:

- **The taps are first taken as far out as the pen ever reaches.** A pixel with no edge there has none
  however the pen goes, and that is nearly all of them. Only the rest read the noise and tap again.
- **The noise is a texture.** Three value noises in arithmetic cost 0.17 ms where trees filled the
  view, five times the rest of the ink. It is now two reads of a 64×64 texture of random numbers
  (`penNoise`), the view's direction unfolded onto a square as an octahedron's faces are.
- **Every tap distance is the ink's own plus a multiple of two pixels.** A tree fading out near the
  camera is dithered away pixel by pixel, and taps an odd step from the ink's saw an edge at every one
  of them: blotches over the whole crown.

## What it costs

`node client/scripts/gpu-bench.mjs --views forest,vista,night --parts ink,firefight,firefightBare`,
`umea`, quality high, 2268×1258. `firefight` is four tank guns and three rifles firing across the
view and a shell bursting 25 m ahead, two hundredths of a second on; `firefightBare` is the same with
no halos, so their difference is the glow.

| | forest | vista | night |
|---|---|---|---|
| bloom, every frame (before) | 0.28 ms | | 0.27 ms |
| halos, in the firefight | 0.04 ms | 0.03 ms | 0.02 ms |
| ink, all of it | 0.07 ms | 0.11 ms | 0.07 ms |
| of which the waver | 0.01 ms | 0.04 ms | |
| hatching, when on | 0.17 ms | 0.12 ms | 0.17 ms |

Each number is a difference inside one run. The runs' whole frames (2.97 ms in `forest` with bloom,
2.74 ms after) are not from the same run, so they only agree with the table, they do not prove it.

## Not done

- **A depth test at the halo's centre** in place of a test at every pixel, as the lens flare has. The
  halo of a light behind a thin wall shows up to half a metre in front of it.
- **Heat shimmer and a blast's ripple in the air.** They need a read of the scene's colour.
- **Hatching on grass and under trees**, which have their own lighting.
