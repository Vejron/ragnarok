# Atmosphere: sky, sun, moon, stars, clouds and mist

The world has a sky behind it, and the light looks as it does through an eye or a lens. A physical atmosphere is lit by the real sun, so sunrises and sunsets
come from the same scattering that turns a real sky red. On top of it are a sun disc, a moon that
shows its phase, stars that turn through the night, and a layer of clouds that drift with the wind
and shade the ground. The fog takes its colour from that sky, so distant hills fade into the sky
behind them. Low mist pools at dawn. The water mirrors all of it.

It costs about 0.3 ms of GPU time on the main pass in a view with lots of sky, measured headless on
an M-series Mac. That covers the sky, the fog and the cloud shadows together. The atmosphere's tables
cost nothing per frame: one is rebuilt only when the sun has moved.

## The parts

| Part | Where | What |
|---|---|---|
| Physical model | `engine/src/scene/atmosphere.ts` | `ATMOSPHERE`: Rayleigh, Mie and ozone coefficients, km. A small CPU model of the same: `transmittance`, single-scattering `skyRadiance` |
| Settings | `engine/src/scene/sky.ts` | `Sky`, set on `Scene.sky`: the sun, the moon and its phase, stars, cloud cover and drift, cloud shadows, the tint, the night glow, the mist. `null` leaves the old flat background and fog |
| GPU tables and draw | `engine/src/passes/sky.ts` | `SkyPass`: the lookup tables, the cloud map, and the sky's draw |
| Shared WGSL | `engine/src/materials/sky.wgsl.ts` | `SKY_WGSL`, in every scene shader through `COMMON_WGSL`: `skyColor`, `skyHaze`, `cloudShade`, `fogAmount`, `fogTint` |
| Time of day | `client/src/render/daynight.ts` | `skyAt` (the light for an hour), `drawnSky` (fills the engine's `Sky`), `cloudsAt`, `moonPhaseAt` |
| Scene | `client/src/render/scene.ts` | Owns the `Sky`. `setTimeOfDay` and `setClock` drive it, `setWaterPlane` sets the mist's base, and it switches the sky off under water |

## The atmosphere

This is Hillaire's 2020 method ("A Scalable and Production Ready Sky and Atmosphere Rendering
Technique"). It uses three tables, all `rgba16float`, made by compute shaders in `SkyPass.update`:

- **Transmittance**, 256 × 64, made once. It holds how much sunlight gets through from the top of the
  atmosphere to any height and angle. It uses Bruneton's layout, which is fine near the horizon.
- **Multiple scattering**, 32 × 32, made once. It holds Hillaire's Ψ_ms: light scattered more than
  once, summed as a geometric series, per height and sun angle. Without it twilight skies come out
  too dark and too red.
- **Sky view**, 192 × 108. It holds the radiance seen from the ground (`ATMO_EYE`, 0.2 km) in every
  direction, for a sun of illuminance 1. It is rebuilt only when the sine of the sun's elevation
  moves by more than 2·10⁻⁴: a 20 µs dispatch every second or so of game time.
  - Across: the angle from the sun's bearing, squared, so it is fine near the sun.
  - Down: the angle from the zenith, fine near the horizon on both sides.

`skyViewUv(dir)` in `sky.wgsl.ts` is the lookup.

**Tint.** The table's radiance is scaled by `Sky.tint`, which the client sets per palette in
`balanceFor`: the CPU model's horizon at noon, averaged round the compass, is made to equal the
palette's sky colour. Noon looks as it always did. Every other hour is the atmosphere's own colour.

**Twilight gain.** After sunset the real sky darkens about a hundredfold within minutes, far faster
than the toon ground light, which is lit as an adapted eye sees it. `twilightGain` brightens the
atmosphere by the square root of how much darker its horizon is than at noon, up to 14×. The
afterglow therefore shows for the whole dusk, then gives way to the night's own glow (`Sky.night`,
`NIGHT_SKY`).

## What is drawn

The sky is one triangle over the screen at the far plane (depth 0, reversed). It is drawn in the main
pass after the opaque world and before anything translucent, with `greater-equal`, so it shades only
pixels nothing else covered. `skyColor(dir, lod)` builds the pixel, back to front:

1. **Haze** (`skyHaze`): the table, never looking below the horizon, greyed by heavy cover.
   - It is softly capped (`softCap`, on the brightest channel, as the bloom's threshold reads it).
   - The glare round a low sun is 10 to 40 times brighter than the rest of the sky. A large patch over
     the bloom's threshold would flood the screen.
2. **Stars** (`starField`):
   - one hashed star in about a fifth of the cells of a 3D grid over the sphere, a few bright, most
     faint, twinkling;
   - a faint Milky Way band clumped by the cloud map.
   - They turn about a pole tilted 30° once a day (`Sky.starTurn`), show as the sun sinks past about
     2° under the horizon (`Sky.stars`), and dim behind cloud.
3. **Moon** (`moonDisc`):
   - It is where the night's light comes from (`Sky.moonDir`): opposite the sun and held at least 25°
     up, as before, so its shadows match it.
   - It is drawn five times the real size, so it reads.
   - It is lit from the side its phase says (`Sky.moonPhase`, a radian angle: 0 full, π new). The phase
     runs over 8 game days of the server's clock (`moonPhaseAt`) and starts full.
   - The cloud map gives it darker seas, and it has a faint halo.
4. **Sun** (`sunDisc`): a disc twice the real size, darker at its limb, coloured by the sunlight that
   reaches it (`skyLight`, the CPU transmittance), with a close glare. Bloom does the rest.
5. **Clouds** (`cloudLayer`), over all of it:
   - **Layer:** a layer at 1800 m (`CLOUD_HEIGHT`), bowed over a small world so it sinks to the
     horizon about 40 km out.
   - **Cloud map:** made once on the CPU (`makeCloudMap`): 256² texels repeating every 6 km.
     - Red is broad cover: six octaves of value noise, evened out by rank so a cover of *c* clouds over
       a share *c* of the sky.
     - Green is finer detail.
   - **Mip level:** the sky reads it at the level the pixel's footprint asks for, from derivatives taken
     before any branch.
   - **Lighting:**
     - A cloud thick on its sunward side is in its own shade.
     - Thin edges toward the sun get a silver lining.
     - A low sun lights every cloud from below: the pink and orange of a sunset.
     - At night the moon lights them, faintly.
   - Far clouds sink into the haze.

## Fog and mist

`applyFog(rgb, flags, viewDepth, world)` (`vertex.wgsl.ts`) now takes the world position:

- **Colour** (`fogTint`): `skyHaze` along the view, so fogged terrain matches the sky behind it at
  the world's edge. It glows toward a low sun and is blue-grey away from it.
- **Amount** (`fogAmount`): the old smoothstep distance fog (150 to 600 m on high), which still hides
  the edge of what is loaded, combined with exponential **height mist** integrated along the view.
  - The density at the base is `MIST_DENSITY` (0.004 per m), times `mistAt` (thickest at sunrise and
    sunset).
  - It thins by e every 22 m up (`MIST_FALLOFF`).
  - The base follows the water in sight (`setWaterPlane`), so mist lies on lakes and low ground.

Particles, ribbons and flashes still fog to the flat `Fog.color`. The client sets that to the
horizon's colour all round (`Sky.sky` in `daynight.ts`), and the background clear colour takes the
same value.

Under water the client sets `scene.sky` to null for that frame, so the murk is the old flat fog and
nothing of the sky draws.

## Cloud shadows

`cloudShade(world)` follows the light (the sun, or the moon at night) up from a point to the cloud
layer and reads the broad cover there, one level blurred. It multiplies the sun term in the standard
shader (for receivers of shadows) and in `GROUND_LIGHT_WGSL` (grass, water, skid marks and so on).

- `Sky.cloudShadow` is how much light a cloud takes off: 0.55, or 0.3 on snow.
- It is off on the low preset (`Preset.cloudShadows`).

## Weather

`cloudsAt(seconds, wintry)` gives the cover and the wind's drift from the server's clock, so every
player sees the same clouds:

- The cover swings slowly (periods of 45 and 18 minutes) by ±0.18 about 0.4, or about 0.72 on a winter
  world.
- The wind carries the clouds at about 6 m/s.
- The drift wraps every 60 km, a whole number of map tiles, so it stays small for 32-bit floats.

In SOLO the clock never syncs, so the clouds stand still.

## The light it gives

`skyAt` still sets the sun, the hemisphere and the ground's tint, but now from the atmosphere:

- **Sunlight colour:** the palette's sun, shifted by how the sunlight's hue through the air differs
  from noon's, 75% of the way (`SUN_SHIFT`). It reddens toward the horizon.
- **Sky fill:** the hemisphere's sky colour shifts the same way with the sky's light on an upturned
  face, half way (`FILL_SHIFT`), and only while the sun is up. After sunset the fill is the night's
  blue, not the afterglow's red.

At noon both are the palette's exactly, as the tests check.

## Light: shafts, glare, the eye and the grade

These run after the effects pass, on the resolved HDR image. Together they cost about 0.08 ms a frame:
100 renders timed to the GPU's finish, on against off.

- **Light shafts** (`passes/shafts.ts`, `Scene.shafts`): Mitchell's post-process god rays, at a quarter
  of the screen's size.
  - A mask holds the sky's light near the light source. Whatever stands in front of the sky (depth
    above 0) is black, and the disc's glare is held down to 3.
  - A blur gathers the mask along 32 jittered steps toward the light, each step keeping 0.965 of the
    last.
  - The output pass adds the result. Gaps in trees, the edges of buildings and clouds become shafts.
  - The light is the sun, fading in as it clears the horizon. Once it is down it is the moon, at 0.3
    of the moonlight. Either fades out as it leaves the screen (`Renderer.placeLight`).
- **Lens flare** (`passes/flare.ts`, `Scene.flare`): a starburst of glare on the sun and six ghosts
  along the line through the middle of the screen, added over the finished picture.
  - The vertex stage reads 25 depth taps round the sun, so the flare dims as the sun goes behind
    something, with no pass of its own.
  - The sun only: the moon does not flare.
- **Eye adaptation** (`passes/exposure.ts`, `Scene.adaptation`):
  - One compute workgroup meters the log-average luminance of a 32 × 32 grid, weighted toward the
    middle of the screen.
  - It eases toward the metering at 1.2 per second when the view darkens and 2.5 when it brightens.
  - The exposure is `(key / luminance) ^ 0.4`, between 0.8 and 1.6. The key, 0.3, is noon's metering,
    so noon keeps an exposure of about 1. Midnight (about 0.016) gets the most, 1.6.
  - A dark forest opens up, and looking into a bright sky dims the ground a little.
  - Everything stays on the GPU. `stats.exposure` and `stats.luminance` are read back every 30 frames.
- **Grade** (`Scene.grade`, set by `gradeAt` in `daynight.ts`), in the output pass before tone mapping:
  - **Golden hour:** split toning, with warm highlights (`#ffeedd`) and cool shadows (`#e4ecff`), and
    10% more saturation.
  - **Day:** a faint cool in the shade.
  - **Night:** the Purkinje shift, where dim colours go toward the blue-grey of rod vision (up to 0.85
    of the way) while lamps and fire keep their colour.
  - **Vignette:** a 0.16 vignette after tone mapping.

The shafts and the flare are the **Sun rays** look option (`Look.rays`), off by default on phones.
Adaptation and grading are always on.

## Checking it

- `?time=0.3` (dawn), `0.5`, `0.735` (sunset), `0.77` (dusk) and `0` (midnight) in the address pin
  the hour. In SOLO set it with `__game.scene.setTimeOfDay(phase)`.
- The WGSL can be checked without a browser by writing out `SKY_SHADERS` (`passes/sky.ts`) and
  `STANDARD_WGSL`, and running `naga` on them.
- `scene.scene.sky = null` in the console shows the world without the sky, for comparing.
