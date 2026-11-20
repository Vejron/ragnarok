# A river's surface

A river is now drawn, and behaves, as running water rather than as a lake with a current. Its surface is shaped by the flow the importer solved ([RIVER-FLOW.md](RIVER-FLOW.md)): it stands higher where the current is braked and lower where it runs fast, and a rapid stands in waves across the current. The wind's swell is gone from it. Its ripples ride the current, its water is stained, and the boulders under its surface trail wakes.

## In play

- **No lake swell on a river.** A river used to count as open water twice its width, so a 100 m river carried most of the 60 m swell, in the same heading as on a lake. It now counts as a quarter of its width and at most 30 m, which leaves the two shortest swells: a chop of a few centimetres.
- **The surface is not level across.** Against a boulder or an island the water piles up, in the fast tongue beside it it dips, and on the outside of a bend it leans. This is centimetres to a few decimetres, and it is in the level the sim reads, so a hull tilts on it.
- **A rapid stands in waves.** Where the water runs fast and thin, crests lie across the river every 4 to 7 m, and stay where they are. No two are alike: most are low, a patch here and there stands up to 55 cm, and a crest wanders up and down the river along its length. They are part of the river's level, so a boat bucks through them and a swimmer rides over them. On top of them the sim adds a small surge (up to 10 cm) that rises and falls in place, crests across the current too.
- **The level is read between the samples.** The sim took the nearest ground sample's level, so a boat on a falling stretch went down steps a few centimetres high. It now blends the wet samples round a point, as the drawn surface does, and passes on the surface's slope.
- **How it looks.** A pool lies dark and mirrors its banks. A deep fast tongue slides by smooth, with long reflections. Thin fast water is broken and ruffled. In a rapid the foam gathers on the faces of the standing waves that meet the current and their backs run dark. Shallow water shows the bed in a warm tone and deep water is the stain's own colour. Lakes and the sea look as before.
- **Boulders trail wakes.** A boulder whose top lies just under the surface of fast water has a dip over it, a rim round it and a wake that trails away downstream and wanders. This is drawn only.
- **Each world sets its rivers' colour.** The editor's world panel has "River water colour"; a world that sets none gets a brown-green (`RIVER_TINT`). "Lakes and sea in the river's colour" stains all the world's water; without it lakes and the sea stay a clear blue.
- **A river is its colour from bank to bank.** The slack water along a bank, too slow to write down, was drawn as a lake's: blue patches with square edges. A river's water is now never written as quite still.

## How it works

### The importer (`crates/osm-import`)

- **Reach.** `Surface::new` gives a river sample a reach of `RIVER_REACH` (a quarter) of the width there, at most `RIVER_REACH_MAX` (30 m). Nothing else changed for the swell: the sim and the shader gate each swell by the reach as before.
- **The lift.** `riverflow::solve` works under a rigid lid, and its projection takes `p[cell] − p[west]` off a face's velocity each step. A surface standing `p · dx / (g · dt)` higher would do the same, so that is how far the water stands above the even surface it was solved under (`Current::lift`, metres). `Stretch::current` scales it by the square of what it scales the current by.
- **Only across the river.** `Stretch::shape` takes each 20 m cross-section's mean lift out (blended between sections). How the surface falls along the course stays the fitted profile's to say: the lid's own idea of it lowered a whole rapid by decimetres and left it carrying a third less water. What is left is clamped softly to ±0.4 m (`SURFACE_LIFT_MAX`) and fades out over 60 m at each end of a stretch.
- **The bed does not move.** The level is raised or lowered and the depth with it. Water shallower than 1.2 m is not lowered, so the chute still floats a boat and a boulder's top stays as far under as it was put. The current is scaled by the old depth over the new, so the same water passes.
- **Standing waves.** Where the water runs faster than `CHURN_SPEEDS` (1.2 to 2.6 m/s) and its Froude number passes `TRAIN_FROUDES` (0.5 to 0.9), `shape` adds crests up to `TRAIN_HEIGHT` (25 cm, and no more than 0.35 of the depth). A wave stands on water that runs as fast as the wave would travel, so its length is `2π · speed² / g`; the water takes the two of `TRAIN_LENGTHS` (4.0, 5.2 and 6.8 m) nearest that, blended. The phase is the distance along the course, so a crest is a line of equal distance and never tears where the river turns. A crest is peaked and the trough between two is at the even surface, so the waves take no water off a bed. A length needs four samples of the grid, so only a world with a 1 m ground cell has them.
- **Uneven waves.** A crest's height is `TRAIN_HEIGHT` times between 0.2 and 2.2 (`TRAIN_UNEVEN`), by a noise of the place in patches 9 m across, squared so the high patches are few. Its line is moved up or down the river by up to 1.6 m (`TRAIN_WANDER`) by another.
- **Never quite still.** A sample of a flowing stretch whose current rounds to nothing is written as one step of `Tile::flow` (0.05 m/s) down the course. That is what tells the shader the water is a river's, and what floats there creeps on.
- **The tint.** `Manifest::river_tint` is `consts::RIVER_TINT` unless the world's `area.json` has `"river_tint": "#rrggbb"`, and `Manifest::tint_all` is its `"tint_all_water"`.

### The sim (`crates/sim`, `crates/shared`)

- `Waters::level_at` blends the wet samples of the four round a point; whether the point is wet is still the nearest sample's to say. `Waters::surface` adds the blended level's slope to the waves' (level where one of the four is dry). The client's `Waters.levelAt` does the same; `planeNear`, which votes on the level to mirror, still reads whole samples.
- `waves::churn(x, z, time, flow)` takes the current, not its speed. Four waves lie a quarter of a half-turn apart, each rising and falling in place, and water takes each by the fourth power of the cosine of the angle between its heading and the current's. The four shares add up to 1.5 whichever way the water runs, so the crests lie across the current wherever it turns, without a seam.
- The client's `Waters.surfaceAt` left the churn out, up to 22 cm in a rapid. It now adds it, as `Waters::surface` does.

### The client (`client/src/render`)

- **The mesh** (`watermesh.ts`). Each vertex of a lake's or river's surface has a normal: straight up on still water, and on running water leaning by the level's slope between the wet samples either side. The refiner blends them into the refined mesh (`MeshRefiner`, `normals` at binding 9).
- **The shader** (`water.ts`). Water with a current is a river:
  - `riverChop` carries three octaves of ripples along the current, all of them, in two copies half a cycle apart. A lake's broadest octave is left out. Their slope along the current is mostly taken out, the more the faster it runs, which draws them out along the flow.
  - `rough` is how broken the surface is: from the Froude number (`speed / √(g · depth)`, eased over `ROUGH_FROUDES`) and the foam the importer carried there. It sets the ripples' strength and stands in for the wind's gusts, and a smooth fast tongue keeps only `TONGUE_WHITE` of a rapid's white.
  - The still surface's slope (`lie`, from the normal) and the surge's light the surface with the swell's.
  - A dry corner of a wet cell takes the fastest wet corner's current in the mesh, so a river runs up to its shoreline. The stain is blended in by `smoothstep(0, 0.045, speed)`, which is all of it anywhere in a river and fades over a cell where one runs out into a lake; with `tint_all` every lake and the sea has it.
  - The colour is the stain's (`params.river`) where it is deep. Each metre of water passes between `RIVER_PASSES` of the light, most in the stain's strongest colour, over a path `RIVER_PATH` times the depth; the bed shows through what is left.
  - A rapid's foam is moved onto the faces that meet the current (`WAVE_FOAM` times the slope along the flow), and near the camera a finer noise breaks it into lace (`FOAM_LACE`).
- **The wakes** (`waterwaves.ts`, `engine/src/fx/waves.ts`): see [WATER-WAVES.md](WATER-WAVES.md), "In a river".

## Cost

Measured with `gpu-bench.mjs` at the Retina size (2268 × 1258 at quality `high`), looking down at Vindelälven from 3 to 4 m over it. Each frame was timed alternately against the same frame with the river drawn as a lake (the water material's `wave.w`):

| View | Frame | The river's look costs |
|---|---|---|
| In the rapid (x −500, z −1871) | 4.24 ms | 0.19 ms |
| Over the pool below the rapids (x −760, z −400) | 4.43 ms | 0.25 ms |

Carrying only the broadest octave, as before, cost 0.20 to 0.23 ms ([RIVERS.md](RIVERS.md)), so the look is no dearer than it was. The shaped surface and the standing waves cost nothing to draw: they are where the vertices already were.

The wakes were timed apart, since the benchmark's paused frame does not step the wave field: 60 frames with the field stepping, the current charted and 8 boulders stirring took 4.01 ms each, against 4.00 ms stepping with neither and 4.06 ms not stepping at all. That is nothing measurable.

The import of `worlds/vindeln` takes as long as it did: the lift comes out of the pressure the solve already had. The world's tiles are the same size. A water mesh keeps three more floats a vertex (the normal).

## Tests

- `riverflow.rs`: the water piles up before an island and dips in the fast water beside it; it backs up above a narrows and runs down through it; an even channel's surface stays even.
- `water.rs`: a river's reach is a small share of its width; a rapid stands in waves across the current and a pool lies flat; a coarse grid gets no standing waves; the water stands higher against an island than beside it. The older tests of the bed, the chute and the discharge hold over the shaped surface.
- `shared/waves.rs`: the surge slopes along the current far more than across it, whichever way the river runs, and loops with the swell.
- `sim/waters.rs`: a falling river's level is read between its samples and slopes as it falls; beside a dry sample it is the wet ones' and lies flat.
- Client: `waves.spec.ts` holds the surge to the sim's; `waters.spec.ts` the blended level, the surge in `surfaceAt` and `rocksNear`; `water.spec.ts` the normals; `waterwaves.spec.ts` the boulders' stamps and the charted current.

## What it does not do

- **The crests lie across the course, not the current.** Where the water runs at an angle to the river (round a bar), the standing waves do not turn with it.
- **Two lengths blended can cancel.** Where the water's speed puts it half way between two of the three lengths and they are out of step, the crests are low.
- **A world with a 2 m or 4 m ground cell has no standing waves** in its level, only the surge.
- **The lift along the course is thrown away,** so the water does not back up above a narrows; it leans across the river only.
- **A wake is short.** The wave field blends its water as it carries it, which smooths a wake away within about 10 m.
- **The bed is not refracted.** It shows through the water where it is, by depth at the vertices. Drawing the water after the scene, against its depth, would do that, and was left out.
