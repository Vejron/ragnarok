# Water waves and spray

What moves through open water now moves the water. A wave equation is stepped on the GPU over a square about the camera, and boats, waders and splashes push into it. Bow waves, the V of a wake, a crash-stopping boat running into its own wave, and rings from a splash all come out of it rather than being drawn by hand. Waterjet boats throw spray in layers that look like water: sheets of fast water, drops, a mist that hangs in the wind, and glints of sun, all glowing against the light.

## In play

- **A moving hull pushes the water aside.** At speed its pointed bow cuts a bow wave and leaves a V of crests and troughs behind it, narrower the faster it goes. A dived submarine leaves the surface still: a hull pushes the water only as much as it breaks the surface. A boat that stops dead runs into its own wave. Its bow wave runs on ahead in arcs and the following wave catches up with its stern.
- **Foam** builds where a hull churns the water and where the water is steep. Fresh foam is solid white. As it thins it breaks into lace and streaks, and it fades over a couple of seconds.
- **Slow things barely stir it.** How hard something pushes the water grows as the square of its speed against the ripples' 3 m/s, as real wave making grows with the hull's Froude number. A swimmer at 1.2 m/s raises a couple of centimetres; a boat pushes fully from 3 m/s on.
- **Waders and swimmers** push a small wake of their own.
- **Vehicles going into the water** splash: a tank driving down a bank throws a crown of spray off its bow and shoves out a ring as wide as its hull, then a wake and water off its bow as it ploughs on. A helicopter coming down in a lake does the same. A walker's, a hen's or a Titan's feet splash where they come down in the water, and a Titan wading through a lake pushes a wave ahead of each stride.
- **Water breaks.** Crests ease off below half a metre, and steep water spends part of its motion as foam, so a hull at just the ripples' speed cannot keep adding to its own wave.
- **A splash** knocks a hole in the water that runs out as rings. Spray landing back on the water does the same, and leaves foam where it comes down.
- **A waterjet boat's spray** (the Stridsbåt 90, [VEHICLES-XIX.md](VEHICLES-XIX.md)):
  - under way, a rooster tail off its jets, and sheets off its bow once it is planing;
  - in a crash stop, sheets blasted forward past the bow along both sides, a wall of water off the bow, and a cloud the boat runs into.
- **A car or snowmobile skimming over the water** throws spray too (`JetSpray.skim`): fans of streaks and drops out to either side from its back corners, a little off its nose, and a rooster tail off its drive with a mist behind. A snowmobile's track throws a full rooster tail; a car's wheels about a third of one. It all grows with speed, to its fullest at 20 m/s.
  - A car skims on its wheels, not its hull: whether it is skimming goes by the lowest of its wheels against the surface. Before, it went by the hull's underside alone, which rides too high over the water on a car for it ever to count.
  - In a winter world with open water (`open_water`), the water is water: spray, splashes and wakes as in summer. Only frozen water is ice to the effects (`iced` in `render/season.ts`). Before, any winter world turned them all off.
- **Nothing rides on the waves.** They are only drawn: a boat or a swimmer floats on the sim's waves (`sim/waves.ts`), as before.

## How it works

### The wave field (`engine/src/fx/waves.ts`)

`WaveField` is an engine `Effect` that draws nothing. Its `prepare` runs compute passes before the main pass, so the water reads this frame's waves.

- **The grid.** 256 × 256 texels of 0.375 m, which is 96 m square, kept on whole texels about the camera. Each texel holds its height, its height a step before and its foam, in `rgba16float` storage textures that are stepped one into the other.
- **The steps.** It steps at a fixed 60 a second whatever the frame rate. After a stall it catches up at most four steps. A frame that takes no step keeps its stamps for the next that does, so no splash is lost at 120 Hz.
- **The water.** A damped wave equation, with ripples running at 3 m/s (`WAVE_SPEED`).
  - **The V.** A hull faster than the ripples leaves a V of half angle asin(3 / v): about 22° for a submarine at 8 m/s, 8° for a Stridsbåt 90 flat out.
  - **Why 3 m/s.** Real wakes keep their 19.5° Kelvin angle only up to a hull Froude number of about 0.6, and narrow beyond it as about 0.25 √(g L) / v (Rabaud and Moisy). For hulls of 15 to 25 m that is 3 m/s.
  - **Why not 6 m/s.** At first the ripples ran at 6 m/s. A submarine at 8 m/s then spread a V nearly 50° either side, and every hull's bow wave was a wide wall.
  - **The edge.** Waves reaching the field's edge are soaked up, not thrown back.
- **Hulls.** A hull is pushed in as a stamp: its waterline, its depth in the water and its velocity.
  - **The waterline.** It is as wide as the hull aft, and narrows over the front 80% of its length to a quarter of its beam at the stem, so the bow cuts a wedge rather than pushing a wall.
  - **The push.** The water under it is pushed down, deepest along the keel and a third as deep at its sides.
  - **The heap.** The displaced water heaps in a rim round it that reaches out 0.8 of the hull's half beam, 0.4 to 2 m.
  - **Round bodies** (waders, splashes) are circles.
  - **The narrowing.** It is done on the width, not by squeezing the distance across. Squeezing the distance near the stem by up to four times made the edge a few centimetres sharp, and at speed it kicked waves the size of the grid.
  - Each step adds only the *change* in what it displaces: where it is now, less where it was a step ago. This is the way Tessendorf's iWave does it.
  - Holding the hull's shape in the water instead keeps a channel open behind it. The channel fills from both sides and builds a hump over a metre high on the centreline.
  - The change goes into the height a step before as well. The water is moved, not thrown, and then flows back as waves. Adding it to the height alone kicks the water, and a hull faster than the waves stacks those kicks into crests that reach the 1.5 m clamp.
- **Depth changes.** How deep a hull sits changes as it speeds up, slows or dives. Each stamp carries how deep it sat in the last frame that stepped, and the change is spread over the frame's steps where the hull is.
  - Without that, the water under a hull was never pushed down when it started to stamp, but was let back up behind it as it moved off. That left a heap of its own size where a swimmer or a slow boat began, and a hole where it stopped.
  - `WaterWaves` follows each hull by a key (`vehicle <id>`, `wader <key>`). A hull that stops being stamped is lifted out where it last sat.
- **Breaking.** `BREAK` eases every height under 0.9 m. Water steeper than a slope of 0.5 loses up to 30% of its motion each step and whips up foam.
  - Before that, a hull at exactly 3 m/s built its wave to the 1.5 m clamp. Now it peaks at about 0.23 m.
- **Splashes.** A splash adds its hole once, in the frame's first step.
- **Frames without a step.** A frame that takes no step (above 60 Hz) keeps its splashes for the next frame that does, and drops its hulls. They are stamped again where they are next frame, and kept twice they would push twice.
- **The map.** A last pass writes what the water reads: height, slope and foam, in metres. The water's material samples it as `wakeMap` over `wakeArea` (`WAKES_WGSL` in `client/src/render/water.ts`):
  - **For the height** in the vertex shader (`wakeLift`), only on the refined mesh (below), fading in over the 6 m inside its edge.
  - **For the normal and foam** in the fragment shader, everywhere.
- **When it rests.** After 10 s with nothing pushing in, the water has settled and the steps stop.

### The refined water (`engine/src/fx/refine.ts`, `client/src/render/waterrefine.ts`)

The tiles' water has a vertex about every metre, and the wave field a texel every 0.375 m. A hump narrower than the vertex spacing landed on one or two vertices and showed as a pyramid. Spreading the height to the mesh's spacing instead made the waves too soft. So the water near the camera is cut finer on the GPU, a third of a metre apart.

- **`MeshRefiner`** (an engine `Effect`) takes the source meshes' positions, `custom` and indices. It uploads them from their arrays once each time the set changes.
  - A compute pass, one thread per source triangle, cuts every triangle whose middle is in a square into nine. The new vertices are blended from its corners, written in the arena's vertex layout, and marked with uv (1, 0).
  - The result is copied into a fixed slot of the geometry arena, the slot of the refiner's own `geometry`. A mesh draws it with the sources' material.
  - It runs again only when the square or the sources change. Its unused triangles are empty.
  - Writing the arena directly would need it bound as storage, and it can be bigger than a default device's 128 MB binding (295 MB in a town). So the pass writes its own buffers and copies them across. `EffectContext` now carries the arena's vertex and index buffers for that.
- **`WaterRefine`** keeps an 80 m square about the camera.
  - **Snapping.** The square is snapped to the water's own vertex lattice, four vertices at a time, so no triangle straddles its edge.
  - **Capacity.** Room for two triangles a cell over the square, about 13,400 source triangles at a metre's spacing.
  - **Sources.** The lake and river surfaces (`WaterBuffers.surface`) whose bounds reach into the square.
- **The water's shader** (`refineArea`):
  - **The coarse water.** The tiles' own surfaces discard their pixels inside the square, which the refined mesh covers triangle for triangle. The water moves only up and down, so a pixel's x and z still say which triangle it is in.
  - **The lift.** Only refined vertices take the waves' height, and it fades to nothing before the square's edge, so the two meshes meet flat. Streams are left alone.
  - **Until the refined mesh has a slot in the arena** (its first frame), the square is off and the tiles draw everything.

### The client (`client/src/render/waterwaves.ts`)

`WaterWaves` owns the field. `FxWorld.waves` hands it to the effects as a `Stir`:

- `hull(stamp)` for something moving through the water;
- `splash(x, z, radius, depth, foam)` for something landing in it.

At most 64 stamps are taken a frame: the nearest and biggest first. Nothing further than 60 m from the camera is taken.

| Caller | What it pushes in |
|---|---|
| `VehicleFx.skim` (`render/tyres.ts`) | Any vehicle in the water, by its speed (`pushShare`): 0.6 of how deep its bottom (its hull, or its wheels or tracks where they are in water) is under the surface (at most 1 m), more in a crash stop, fading out as its top goes 0.5 m under |
| `VehicleFx.splashIn` | Any vehicle going into the water, driven in or dropped in, after a quarter of a second out of it: a crown of spray at its bow (or under it, dropped) and a hole as wide as its hull, deeper the faster it came. Not one first seen already in the water |
| `VehicleFx.skim`, ploughing | A vehicle driving through the water on its wheels or tracks, faster than 2.5 m/s, throws water off its bow. Wheels on the bed mean it is not skimming over the water, however close its hull is to the surface |
| `Vehicles.stepIn` (`entities/presenters/vehicles.ts`) | A walker's, a hen's or a Titan's foot coming down in water, and a Titan landing in it: a splash and a hole as big as the foot (3.5 m round for a Titan's) |
| `Vehicles.wade` | A Titan's soles while under the surface, each a hull pushed through the water as it swings (`titan <id> foot <n>`): a bow wave ahead of a striding foot, and the water lifting as it comes out |
| `Surroundings.wade` | A wader, 0.3 m round |
| `splash()` | Anything landing in the water |
| `JetSpray` | Where its drops come down, from their arcs |

It replaces `render/wakes.ts`, the ring and trench splats that were drawn into a byte texture on the CPU. The rings that `Effects.ripple` drew as flat particles at the water's still level are gone as well. They lay at the wrong height on a swell, and a splash, a wader, a hull or a missile launch now stirs the wave field itself.

### In a river

A river's water runs, and the field's water runs with it ([RIVER-SURFACE.md](RIVER-SURFACE.md)):

- **The current's map.** `WaterWaves.chart` hands the field the current over a 192 m square about the camera as 64 × 64 texels (`WaveField.setFlow`, `FLOW_GRID`), a byte a component in `FLOW_STEP`s. The square lies on lines 24 m apart, so it is made again only when the camera has gone that far (4096 lookups), and now and then for tiles that have come in. Still water has no map.
- **Carried water.** In `advance`, a texel in a current takes the water that was upstream of it a step ago: itself and its four neighbours, each blended from the four texels round where it was (`wasBetween`). All five must move together. Carrying the middle one alone feeds the waves at long wavelengths instead of moving them, and the field filled the whole river with 20 cm of noise that never settled.
- **Pushing by speed through the water.** A stamp's movement in a step is its own less the current's. So a boat drifting with the river makes no waves, as before, and something that stands still pushes the water that runs onto it.
- **Boulders.** `Waters.rocksNear` finds the samples within 40 m of the camera where the water is no deeper than 0.45 m, no deeper than at any sample beside them, and runs at 1 m/s or more. The nearest 24 are stamped each frame as round bodies 1.3 m from their middle, standing still, sitting up to 22 cm deep by the square of the current's speed (`pushShare`). Each gets a dip over it, a rim round it and a wake that trails away downstream. Boats and waders have the stamps first when both want more than the field takes.
- **Never at rest near a rapid.** A stamp each frame keeps the field stepping, where still water lets it rest after ten seconds.

### Spray (`client/src/render/jetspray.ts`)

`JetSpray` throws a waterjet boat's water from two pools of its own, so that a crash stop cannot crowd out smoke and dust:
- 8,192 crisp drops at full resolution;
- 1,536 puffs of mist at half resolution;
- the glints go in the shared additive pool.

`VehicleFx.jets` hands it the boat's state each frame. Every layer is emitted as GPU bursts (`Particles.emit`):

- **Streaks:** small `drop` particles stretched along their motion. A sheet of fast water reads as streaks.
- **Drops:** bigger ones flung every way, which arc and fall back.
- **Mist:** big `puff` particles that hang, roll and drift downwind.
- **Glints:** additive `glare` specks, only by day.

A new particle flag, `scatter` (`PF_SCATTER`), marks water. Its sunlight is scattered forward (Henyey–Greenstein, g = 0.5), so mist and spray seen against the sun glow about six times brighter than from the side, and its drops glint.

Where the drops come down is worked out on the CPU from each stream's arc. It could not be done in the particle shader: that already binds the eight storage buffers a default WebGPU device allows.

## Cost

Measured with `client/scripts/gpu-bench.mjs` on Vindeln's water view, at quality high (2268 × 1258, 2.56 ms a frame), timed alternately with the plain frame:
- a hull driven through the wave field every frame costs nothing measurable (0.00 ms);
- a crash stop's spray in view costs 0.14 ms;
- the refined water, about 120,000 more triangles in that view, costs 0.16 ms (2.51 ms with it).

The bench's search for a shore now steps west from the spawn as well as east, which Vindeln needs.

## Tests

- `client/src/__tests__/waterwaves.spec.ts`:
  - the field's stepping rate, catching up and following the camera;
  - its rest once still;
  - the stamp limit and which stamps are kept;
  - a boat's hull pressed in along its heading, deeper in a crash stop;
  - spray stirring the water where it lands.
- `client/src/__tests__/waterrefine.spec.ts`:
  - the refined water waits for water and sizes itself to the vertex spacing;
  - it leaves the tiles their water until its mesh has a slot;
  - it keeps the square on the cells' lines and takes only the surfaces that reach into it.
- `tyres.spec.ts`: a car skimming fast throws spray, more the faster, and none standing still; a snowmobile on a winter world's open water throws more than a car (its rooster tail).
- `tyres.spec.ts`: a tank driving into a lake splashes once as big as itself, then pushes a wake and throws water off its bow; one first seen in the water does not splash; a helicopter dropping in splashes.
- `tyres.spec.ts`: a waterjet boat throws no spray lying still, a rooster tail of streaks, drops and mist under way, and more than twice the water and mist in a crash stop.
- The GPU side was checked in headless Chrome:
  - the field read back after a hull ran through it: a bow wave and a V, at first about 0.6 m by the hull and ±0.2 m 25 m behind;
  - after the waterline and wave speed changed, a CB90 at 20 m/s and a submarine at 8 m/s side by side: narrow Vs from pointed bows, peaking at 0.4 to 0.5 m by the hulls;
  - the CB90 driven past the camera, from behind and in a crash stop.
