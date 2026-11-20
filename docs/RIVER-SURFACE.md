# A river's surface

A river is now drawn, and behaves, as running water rather than as a lake with a current. Its surface is shaped by the flow the importer solved ([RIVER-FLOW.md](RIVER-FLOW.md)): it stands higher where the current is braked and lower where it runs fast, and a rapid stands in waves across the current. The wind's swell is gone from it. Its ripples ride the current, its water is stained, and the boulders under its surface trail wakes.

## In play

- **No lake swell on a river.** A river used to count as open water twice its width, so a 100 m river carried most of the 60 m swell, in the same heading as on a lake. It now counts as a quarter of its width and at most 30 m, which leaves the two shortest swells: a chop of a few centimetres.
- **The surface is not level across.** Against a boulder or an island the water piles up, in the fast tongue beside it it dips, and on the outside of a bend it leans. This is centimetres to a few decimetres, and it is in the level the sim reads, so a hull tilts on it.
- **A rapid stands in waves.** Where the water runs fast and thin, crests lie across the river every 4 to 7 m, and stay where they are. No two are alike: most are low, a patch here and there stands up to 55 cm, and a crest wanders up and down the river along its length. They are part of the river's level, so a boat bucks through them and a swimmer rides over them. On top of them the sim adds a small surge (up to 10 cm) that rises and falls in place, crests across the current too.
- **The level is read between the samples.** The sim took the nearest ground sample's level, so a boat on a falling stretch went down steps a few centimetres high. It now blends the wet samples round a point, as the drawn surface does, and passes on the surface's slope.
- **How it looks.** A pool lies dark and mirrors its banks. A deep fast tongue slides by smooth, with long reflections. Thin fast water is broken and ruffled. In a rapid the foam gathers on the faces of the standing waves that meet the current and their backs run dark. Shallow water shows the bed in a warm tone and deep water is the stain's own colour. Lakes and the sea look as before.
- **A falling river mirrors its banks along its length.** The scene was mirrored in one level plane, and the water shows it only within 0.2 to 0.5 m of that plane. On a rapid's run falling 3 to 4 cm a metre, that is a strip 10 to 15 m long. Past it the water mirrored only the sky, and going up the river the plane stepped up 0.2 to 0.4 m every few metres, so the strip of banks and trees jumped along the water. The mirror now leans with the river, and glides as the camera moves.
- **The mirror never pops.** It no longer shows only on water within half a metre of its plane: it shows whole to 1.5 m off it and fades to the sky by 8 m (`MIRROR_NEAR`, `MIRROR_FAR`), a little out of place on water at another level, which the ripples hide. When the water about the camera changes, the plane slides to the new one over about half a second, and where water comes into reach or goes out of it the mirror fades in or out instead of switching. It looks for water up to 640 m away, and high over the water it draws the land further, so trees stay in it as the camera climbs.
- **Boulders trail wakes.** A boulder whose top lies just under the surface of fast water has a dip over it, a rim round it and a wake that trails away downstream and wanders. This is drawn only.
- **Each world sets its rivers' colour.** The editor's world panel has "River water colour"; a world that sets none gets a brown-green (`RIVER_TINT`). "Lakes and sea in the river's colour" stains all the world's water; without it lakes and the sea stay a clear blue.
- **A river is its colour from bank to bank.** The slack water along a bank, too slow to write down, was drawn as a lake's: blue patches with square edges. A river's water is now never written as quite still.
- **The solved current shows on the surface.** Before, one noise covered a river everywhere: carried with the current and stronger where it ran thin, but the same texture in a pool, along a seam and over a riffle. The bed's shape reached the surface only as centimetres of height. A river now shows what its current does:
  - **Streaks along the streamlines.** Ruffled lanes lie between glassy ones, and the foam is drawn out into threads along them. They are lines the solved water runs along: they part round a boulder or an island, crowd together through a narrows and spread out over a pool.
  - **Boils.** Where the current slows into deeper water (below a drop, at the head of a pool) the water dives, and it comes back up over the next 15 m or so in glassy domes a few metres across, rippled round their rims, that swell and fade as they drift down. They push the foam aside, leaving holes in it.
  - **Foam lines.** Where the surface converges the water roughens and its foam closes up into a line.
  - **Eddy lines.** Where the water spins (along the edge of a fast tongue, behind a boulder or a point), it whirls in small dimples that turn the way it spins, and roughens.
  - **Riffles.** Thin water running fast over a shallow bed stands in ripples that keep their place while the water runs through them, crests across the current, the more the more uneven the bed.
  - The noise that covered everything is now only fine detail: a slow, deep pool is nearly glassy.
- **Foam keeps its shape.** It was carried by a lookup that blurred it across a cell every few steps, and mixed sideways as much as the water's momentum, so the trail below a rapid was a smooth cloud. It now keeps its streaks and edges. It gathers where the surface converges (in the slack water below a boulder, in lines down the seams) and thins where the water wells up, and an eddy line froths a little of itself.

## How it works

### The importer (`crates/osm-import`)

- **Reach.** `Surface::new` gives a river sample a reach of `RIVER_REACH` (a quarter) of the width there, at most `RIVER_REACH_MAX` (30 m). Nothing else changed for the swell: the sim and the shader gate each swell by the reach as before.
- **The lift.** `riverflow::solve` works under a rigid lid, and its projection takes `p[cell] − p[west]` off a face's velocity each step. A surface standing `p · dx / (g · dt)` higher would do the same, so that is how far the water stands above the even surface it was solved under (`Current::lift`, metres). `Stretch::current` scales it by the square of what it scales the current by.
- **Only across the river.** `Stretch::shape` takes each 20 m cross-section's mean lift out (blended between sections). How the surface falls along the course stays the fitted profile's to say: the lid's own idea of it lowered a whole rapid by decimetres and left it carrying a third less water. What is left is clamped softly to ±0.4 m (`SURFACE_LIFT_MAX`) and fades out over 60 m at each end of a stretch.
- **The bed does not move.** The level is raised or lowered and the depth with it. Water shallower than 1.2 m is not lowered, so the chute still floats a boat and a boulder's top stays as far under as it was put. The current is scaled by the old depth over the new, so the same water passes.
- **Standing waves.** Where the water runs faster than `CHURN_SPEEDS` (1.2 to 2.6 m/s) and its Froude number passes `TRAIN_FROUDES` (0.5 to 0.9), `shape` adds crests up to `TRAIN_HEIGHT` (25 cm, and no more than 0.35 of the depth). A wave stands on water that runs as fast as the wave would travel, so its length is `2π · speed² / g`; the water takes the two of `TRAIN_LENGTHS` (4.0, 5.2 and 6.8 m) nearest that, blended. The phase is the distance along the course, so a crest is a line of equal distance and never tears where the river turns. A crest is peaked and the trough between two is at the even surface, so the waves take no water off a bed. A length needs four samples of the grid, so only a world with a 1 m ground cell has them.
- **Uneven waves.** A crest's height is `TRAIN_HEIGHT` times between 0.2 and 2.2 (`TRAIN_UNEVEN`), by a noise of the place in patches 9 m across, squared so the high patches are few. Its line is moved up or down the river by up to 1.6 m (`TRAIN_WANDER`) by another.
- **Never quite still.** A sample of a flowing stretch whose current rounds to nothing is written as one step of `Tile::flow` (0.05 m/s) down the course. That is what tells the shader the water is a river's, and what floats there creeps on.
- **How the current stirs the surface** (`riverflow::marks`, which also makes the foam). From the solved current, on the solver's own cells:
  - how the surface spreads, the current's divergence, and how the water spins, its curl, each smoothed twice over a cell's neighbours (`STIR_SMOOTHINGS`). With the same water through every cross-section, the divergence is `−u·∇h / h`: the bed's own mark on the surface, below 0 where the current slows into deeper water;
  - where it converges, the water dives. What dives is carried on the current as a tracer that comes back up over `RISE_REACH` (15 m) of its course. The boil is what comes back up less what dives there (`Tile::stir`'s first value);
  - the stream function `ψ`, with `h·u = ∂ψ/∂z` and `h·v = −∂ψ/∂x`, summed from cell to cell outwards from the first and then settled over 40 sweeps (`LANE_SWEEPS`), so it agrees round an island whichever way it was summed. A line of equal `ψ` is a streamline, and between two of them the same water passes all along.
  - Stored as `Tile::stir` (boil and spin, two signed bytes a sample, in `STIR_STEP`, 0.005 1/s) and `Tile::lane` (`ψ`, an `i16` a sample in 1024ths of `LANE_FLUX`, 10 m³/s, wrapping round every 64 of them). Both are scaled with the current when it is scaled to carry the river's water.
- **The foam** (`riverflow::marks`). It is made as before, and also where the water spins faster than `FOAM_SPINS` (0.15 to 0.5 1/s) on water running near white-water speed, up to `FOAM_SPUN` (0.35) of what white water makes. It is carried by MacCormack's scheme (looked up where the water was a step ago, that lookup's own error found by carrying it back and taken off, the result kept within the values it came from), mixed sideways at `FOAM_MIXING` (a quarter) of the eddy viscosity, and gathered or thinned by the surface's divergence: `exp(−FOAM_GATHER · divergence · dt)` a step, with `FOAM_GATHER` 0.6 (1 would pile every bit of foam into the deepest pool, since the depth-averaged current slows there).
- **The tint.** `Manifest::river_tint` is `consts::RIVER_TINT` unless the world's `area.json` has `"river_tint": "#rrggbb"`, and `Manifest::tint_all` is its `"tint_all_water"`.

### The sim (`crates/sim`, `crates/shared`)

- `Waters::level_at` blends the wet samples of the four round a point; whether the point is wet is still the nearest sample's to say. `Waters::surface` adds the blended level's slope to the waves' (level where one of the four is dry). The client's `Waters.levelAt` does the same; `planeNear` votes on the level to mirror by whole samples, and fits its plane to the blended level.
- `waves::churn(x, z, time, flow)` takes the current, not its speed. Four waves lie a quarter of a half-turn apart, each rising and falling in place, and water takes each by the fourth power of the cosine of the angle between its heading and the current's. The four shares add up to 1.5 whichever way the water runs, so the crests lie across the current wherever it turns, without a seam.
- The client's `Waters.surfaceAt` left the churn out, up to 22 cm in a rapid. It now adds it, as `Waters::surface` does.

### The client (`client/src/render`)

- **The mesh** (`watermesh.ts`). Each vertex of a lake's or river's surface has a normal: straight up on still water, and on running water leaning by the level's slope between the wet samples either side. The refiner blends them into the refined mesh (`MeshRefiner`, `facings` at binding 8).
- **The stir in the mesh** (`watermesh.ts`). A river's vertices carry it in their colour (`WaterBuffers.colors`, the material's `vertexColors`): the boil and spin as `0.5 + v / (2 · STIR_RANGE)`, and `ψ` as a turn round a circle (one turn to 64 streaks), its cosine and sine. The turn blends between vertices where the count it was made from wraps round, and the shader's noise repeats over the same 64, so no seam shows. A dry corner of a wet cell takes the colour of the corner it takes its current from. `MeshRefiner` blends the colours into the refined mesh (`colors` at binding 9, white for a source without them) and packs them as the arena's `unorm16x4`. To stay within the nine storage buffers the renderer's device asks for, the refiner takes each vertex's normal and uv y in one `vec4` (`facings`, binding 8).
- **The shader** (`water.ts`). Water with a current is a river:
  - `riverChop` carries three octaves of ripples along the current, all of them, in two copies half a cycle apart. A lake's broadest octave is left out. Their slope along the current is mostly taken out, the more the faster it runs, which draws them out along the flow.
  - `rough` is how broken the surface is: from the Froude number (`speed / √(g · depth)`, eased over `ROUGH_FROUDES`) and the foam the importer carried there. It sets the ripples' strength and stands in for the wind's gusts, and a smooth fast tongue keeps only `TONGUE_WHITE` of a rapid's white.
  - The still surface's slope (`lie`, from the normal) and the surge's light the surface with the swell's.
  - A dry corner of a wet cell takes the fastest wet corner's current in the mesh, so a river runs up to its shoreline. The stain is blended in by `smoothstep(0, 0.045, speed)`, which is all of it anywhere in a river and fades over a cell where one runs out into a lake; with `tint_all` every lake and the sea has it.
  - The colour is the stain's (`params.river`) where it is deep. Each metre of water passes between `RIVER_PASSES` of the light, most in the stain's strongest colour, over a path `RIVER_PATH` times the depth; the bed shows through what is left.
  - A rapid's foam is moved onto the faces that meet the current (`WAVE_FOAM` times the slope along the flow), and near the camera a finer noise breaks it into lace (`FOAM_LACE`).
  - **The stir.** `riverLane` turns the colour back into where the water is among its streaks, and `laneStreak` reads one noise across them (a row of the chop map), so a streak follows the streamline wherever it runs. It sets the ripples' strength between `1 − LANE_STREAKS` and `1 + LANE_STREAKS / 2`, and `LANE_FOAM` (half) of the foam's noise. Then, each worked out only where it shows:
    - where the water wells up faster than `BOILS` (0.003 to 0.025 1/s), `boils`: two copies of a noise `BOIL_SIZE` (3 m) a cell, carried down the current for `BOIL_CYCLE` (4.5 s) each and swelling and fading over it. A dome is glassy (the ripples down to 15%), its rim rippled, and it slopes up to `BOIL_SLOPE`. The foam is pushed off it;
    - where it gathers faster than `GATHERS` (0.004 to 0.035 1/s), the surface roughens and the foam closes up (its edge lowered by 0.12);
    - where it spins faster than `SPINS` (0.05 to 0.22 1/s), `whirls`: dimples where a carried noise is low, the surface falling into them and swept round the way it spins, up to `WHIRL_SLOPE`;
    - in thin water fast for its depth (`RIFFLE_FROUDES` 0.3 to 0.8, shallower than `RIFFLE_DEPTHS` 0.4 to 1.2 m), `riffles`: two octaves of a noise that is not carried, keeping only their slope along the current, scaled by how much the bed makes the surface gather or well up.
  - A river's ripples are at least `RIVER_CALM` (0.08, it was 0.25) of a broken run's.
- **The mirror** (`Waters.planeNear`, the engine's `Renderer.reflectionPlane`). `planeNear` picks a level by its vote as before (or keeps last frame's plane), then fits a plane by weighted least squares to the water within `PLANE_BAND` of it, three times over (`PLANE_FITS`), so each fit takes in more of a river falling evenly. The fit reads `levelAt` (blended between the samples), so the plane moves by centimetres as the camera does and not by the samples' steps. `PLANE_STIFFNESS` holds the tilt back where the water is a narrow strip, and it leans at most `PLANE_TILT_MAX` (0.1). A lake fits level. The renderer mirrors the view in that plane (`p − 2 (n·p − d) n`), cuts away what lies under it by the frame's `mirror` (its normal and offset), and culls against it. The water shader reads the mirrored plane's height at each pixel (`params.plane`: the height at the origin, how much of the mirror shows, and the slope along x and z), and moves the mirror image by the surface's lean off the plane, not by the river's whole fall.
- **Easing it** (`easeMirror`, called by `Surroundings.mirrorWater` each frame). The plane shown follows the fit at `MIRROR_FOLLOW` (6 a second) and fades in and out at `MIRROR_FADE` (2 a second); with no water about it keeps its last plane while it fades, and the pass stops once it has gone. `planeNear` is still handed last frame's fit, not the eased plane, so its hold on one water is as before. `Scene.setWaterPlane` sets the renderer's `reflectionRange` to 300 m plus 6 m for each metre the camera is over the plane, at most 1000 m. Measured with `gpu-bench.mjs` on Vindelälven, 120 m over the river: 0.08 ms of a 4.94 ms frame (the whole mirror 0.28 ms); 5 m over it, 0.01 ms.
- **The wakes** (`waterwaves.ts`, `engine/src/fx/waves.ts`): see [WATER-WAVES.md](WATER-WAVES.md), "In a river".

## Cost

Measured with `gpu-bench.mjs` at the Retina size (2268 × 1258 at quality `high`), looking down at Vindelälven from 3 to 4 m over it. Each frame was timed alternately against the same frame with the river drawn as a lake (the water material's `wave.w`):

| View | Frame | The river's look costs |
|---|---|---|
| In the rapid (x −500, z −1871) | 4.24 ms | 0.19 ms |
| Over the pool below the rapids (x −760, z −400) | 4.43 ms | 0.25 ms |

Since the current's stir was drawn (`WORLD_VERSION` 36), the same views measure:

| View | Frame | The river's look costs | Of which the stir | And the carried foam |
|---|---|---|---|---|
| In the rapid (x −500, z −1871) | 4.54 ms | 0.43 ms | 0.06 ms | under 0.05 ms |
| Over the pool below the rapids (x −760, z −400) | 5.00 ms | 0.63 ms | 0.15 ms | 0.17 ms |

The stir is timed with the material's `stir` switches (one each for the boils, whirls, riffles and streaks), each turned off alternately with the full frame. With a noise copy of their own and two octaves of riffles, the four cost 0.6 ms over the pool; the boils and whirls now read one carried noise between them, each from its own time, and the riffles one octave. The rest of the rise over the first measurement (0.25 ms) is not accounted for by any one part: the pool's foam now gathers where it slows, and the frames differ by ±0.05 ms from run to run.

Carrying only the broadest octave, as before, cost 0.20 to 0.23 ms ([RIVERS.md](RIVERS.md)), so the look is no dearer than it was. The shaped surface and the standing waves cost nothing to draw: they are where the vertices already were.

The wakes were timed apart, since the benchmark's paused frame does not step the wave field: 60 frames with the field stepping, the current charted and 8 boulders stirring took 4.01 ms each, against 4.00 ms stepping with neither and 4.06 ms not stepping at all. That is nothing measurable.

The import of `worlds/vindeln` takes as long as it did: the lift comes out of the pressure the solve already had, and the stir and the foam are marched in a few seconds together. The tiles grew to 73.7 MB with the stir and lanes (four bytes a sample in a tile where a river flows). A water mesh keeps three more floats a vertex (the normal).

## Tests

- `riverflow.rs`: the water piles up before an island and dips in the fast water beside it; it backs up above a narrows and runs down through it; an even channel's surface stays even. The water dives where it slows into a pool and boils up below, less and less; it spins one way and the other in the lee of an island; as much water runs between the banks' streamlines everywhere, and round both sides of an island. A boulder's foam gathers just below it and trails away from there.
- `water.rs`: a river's reach is a small share of its width; a rapid stands in waves across the current and a pool lies flat; a coarse grid gets no standing waves; the water stands higher against an island than beside it. The older tests of the bed, the chute and the discharge hold over the shaped surface.
- `shared/waves.rs`: the surge slopes along the current far more than across it, whichever way the river runs, and loops with the swell.
- `waters.spec.ts`: the mirrored plane leans with a falling river and lies level on a lake, and moves smoothly as the camera goes up the river; the mirror shown slides from one water to another without a jump, fades out and back in, and a camera high over water still finds it.
- `sim/waters.rs`: a falling river's level is read between its samples and slopes as it falls; beside a dry sample it is the wet ones' and lies flat.
- shared: the tile round trip and its byte layout carry `stir` and `lane`; `world.spec.ts` decodes them; `water.spec.ts` checks they reach the mesh's colours, a dry corner's too, and the tile worker. The refiner's blend of them runs on the GPU and was checked by eye.
- Client: `waves.spec.ts` holds the surge to the sim's; `waters.spec.ts` the blended level, the surge in `surfaceAt` and `rocksNear`; `water.spec.ts` the normals; `waterwaves.spec.ts` the boulders' stamps and the charted current.

## What it does not do

- **The streaks jump where two stretches meet.** Each stretch's stream function starts from its own line, so where one stretch of a river ends and the next begins, the streaks do not join up.
- **The stir is solved on 2 m cells.** A boil or an eddy line narrower than a few metres is smeared over the cells round it.
- **The crests lie across the course, not the current.** Where the water runs at an angle to the river (round a bar), the standing waves do not turn with it.
- **Two lengths blended can cancel.** Where the water's speed puts it half way between two of the three lengths and they are out of step, the crests are low.
- **A world with a 2 m or 4 m ground cell has no standing waves** in its level, only the surge.
- **The lift along the course is thrown away,** so the water does not back up above a narrows; it leans across the river only.
- **A wake is short.** The wave field blends its water as it carries it, which smooths a wake away within about 10 m.
- **The bed is not refracted.** It shows through the water where it is, by depth at the vertices. Drawing the water after the scene, against its depth, would do that, and was left out.
