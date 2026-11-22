# Rivers solved as a flow

A river's current is no longer drawn along its course by rule. The importer solves it: an approximation of the Navier-Stokes equations, run once over each river when the world is built. The same water now passes every cross-section, so the river runs fast where it is narrow or shallow and slow where it is wide and deep. It parts round islands, keeps to the deep channel, dies away at the banks and lies slack in the bays below a point. The bed is cut to match: a rapid is shallow, a pool deep. A rapid's bed is also rough, with bars, pools and boulders round a deep chute, so its current gathers into tongues and breaks into foam where the water is thin or is braked.

Nothing is simulated while the game runs. The water's surface and current must stay the same function of place and tick on the server, in the client's prediction and in the shader, so the solve is baked into the world's tiles (`Tile::flow` and the ground under the water). The sim, the wire, the tile format and the client are unchanged.

## In play

- **Rapids are shallow.** Where Vindelälven falls through Vindelforsarna it is about 1.2 m deep on average and runs at 3 to 4.5 m/s; its pools are 3 to 6 m deep and drift at about 0.4 m/s. Before, it was as deep in a rapid as anywhere.
- **A rapid is not one even sheet.** Its bed has bars a few decimetres under the surface, pools two or three metres deep between them, and boulders that all but break the surface. The current runs in streaks over it: fast in the deep runs, slack behind a boulder. A pool's bed stays smooth.
- **Every hull still passes a big river, down its chute.** A lane about 8 m wide follows the deepest way down the river, wandering from bank to bank with it, with no boulder in it. Along its line Vindelälven is nowhere shallower than 1.2 m, and Umeälven (`worlds/home`) 1.7 m. The deepest draft is the AMV's 1.2 m, the CB90's is 0.8 m. Outside the chute a boat can ground on a bar or a boulder.
- **A small river's rapid can be waded, and no boat goes up it.** The small river in `worlds/home` (about 10 m³/s) is 0.6 m deep down its chute where it falls. A soldier swims from 1.3 m.
- **Foam shows where the water breaks**, not only where it is fast: over a bar, on and below a boulder, and where a fast run slows into a pool. A deep run at the same speed stays dark.
- **The current is fastest mid-channel and slow along the banks**, in a band a few metres wide, so a swimmer can still make for the shore.
- **Narrows run fast, bays lie slack.** Behind an island or a point the water is nearly still, and in places creeps back upstream. These eddies are weak: about 1,500 m² of Vindelälven's 1 km² runs back at 0.15 m/s or more.
- **A river drawn as several polygons is one river.** Vindelälven is three polygons in OpenStreetMap. They were levelled and set flowing one at a time, with a step in the current at each seam.

## How it works

### The bed (`crates/osm-import/src/water.rs`)

`river_levels` still fits each stretch a surface that falls along its course. It now treats all river polygons that touch as one stretch, and hands on what it measured as a `Stretch`: the samples, how far along the course each is, the slope there (never under `FLOW_MIN_SLOPE`, 15 cm a kilometre) and the direction downstream.

Every river carries one discharge from end to end. `Stretch::carries` sets it as what runs at `USUAL_SPEED` (0.4 m/s) through the stretch's usual cross-section, the median by area over each 20 m of its course. That gives Vindelälven 388 m³/s and Umeälven 391 m³/s; their real means are about 190 and 440.

`Stretch::shallows` then cuts the bed to carry it. By Manning's law, a cross-section `W` wide that falls at slope `S` passes the discharge `Q` at the mean depth

    h = (Q · n / (W · √S))^(3/5)

with the roughness `n` = 0.05 (`MANNING_N`, a stony bed). Each cross-section keeps the share `h / its bowl's mean depth` of the bowl the importer cut before, and never more than all of it:

- a pool needs more depth than its bowl has, and keeps its bowl;
- a rapid needs little, and is cut shallow;
- a narrow rapid needs more than a wide one, and is cut less.

The width is the whole cross-section's, over all its channels, and every channel keeps the same share. Taking each channel's own width instead cut a narrow side channel round an island 2.9 m deep beside 0.6 m of rapid, and the whole river jetted through it at 17 m/s.

### The rough bed (`Stretch::relief`)

The bed `shallows` asks for is smooth: the same bowl in every cross-section, scaled. Over it the current is one even band. `relief` roughens it, more the steeper the river falls there (`RAPID_SLOPES`, from 0.5 to 4 m a km):

- **Bars and pools.** The depth is multiplied by a smooth noise of the place, from 1/2.2 to 2.2 times in a rapid (`RAPID_SWING`) and by no more than 1.15 in a pool. The noise has features 40 m and 10 m wide (`BAR_WIDTHS`), and is averaged along the line of the course, so a bar lies drawn out along the current. Plain noise gave square lumps that showed the grid it was made on.
- **Boulders.** Where the river falls more than 3 m a km (`BOULDER_SLOPES`), one may lie in each 10 m square: a lump 2.4 to 5.2 m across whose top is 0.15 to 0.4 m under the surface. They never break it: a dry sample would change the river's outline and its shoreline. They are no smaller because the solver's cells are 2 m wide.
- **The same water still passes.** Manning's law lets `depth^(5/3)` through each metre of width, so each 20 m of the course is scaled until the sum of that over its samples is the smooth bed's.
- **The chute** (`Stretch::chute`). The cheapest way through the water from one end of the course to the other, where a step costs its length and far more the shallower it is, so the line keeps to deep water. With a gentler cost it cut the bends along the shallow bank inside them, and was 0.7 m deep there. The lane 4 m to each side of the line (`CHUTE_HALF`) has no boulders and keeps at least a quarter of its bowl (`RAPID_SHARE`), and 1.2 m wherever the bowl is deep enough for that (`RAPID_DEPTH`). This replaced a floor of the same depth under every sample of the rapid, which left the middle of a wide rapid one even sheet.

The noise is a hash of the place, so the bed is the same every run.

A rough bed holds the water back more than the smooth one it was made from: Vindelälven's passed three quarters of the river's water. Speeding the current up by a third to make that up put far more of the rapid at the 5 m/s limit. A bed that passes too little is too shallow, so `Stretch::current` cuts it deeper instead. A bed `a` times deeper runs `a^(2/3)` times faster, so `a^(5/3)` makes up the shortfall (1.19 times deeper for Vindelälven, never more than `DEEPENED`, 1.4). Only a bed cut shallower than its bowl is deepened, and no deeper than the bowl; a pool keeps its depth and has its current scaled as before, so the same water still passes every sample.

### The solver (`crates/osm-import/src/riverflow.rs`)

The surface is already fixed, so the water is taken as one layer under a rigid lid, and what is left of Navier-Stokes is the velocity `u` averaged over the depth `h`:

- the same volume through every cross-section, `∇·(h·u) = 0`;
- the surface's fall pushing the water downstream, `g·S`;
- the bed holding it back by Manning's law, `g·n²·|u|·u / h^(4/3)`;
- the water carrying its own momentum along, which makes it shoot past a point and leave slack water behind it;
- an eddy viscosity (0.3 m²/s) that spreads momentum sideways and stills the water at the banks.

`solve` marches these in pseudo-time until nothing changes by more than 2.5 mm/s a step:

1. It starts every face at the speed the fall and the bed alone would give it.
2. Each step carries the velocity along itself (semi-Lagrangian), applies the push and the bed's hold (the hold implicitly, so shallow water cannot overshoot), spreads momentum sideways, and then takes out whatever piles water up.
3. That last part is a pressure solve, `∇·(h∇p) = ∇·(h·u)`: conjugate gradients with a multigrid cycle as the preconditioner, each coarser level a cell for every 2 × 2 of the one before. Plain conjugate gradients took 188 s on Vindelälven, whose 5 km are 2,500 cells long; with the multigrid cycle the solve takes 5 s.

The grid is staggered (velocities through each cell's faces, pressure in its middle), and holds only the wet cells. Cells are at least 2 m wide, so a world sampled at 1 m is solved on every second sample and read back between them. Water enters and leaves at the two ends of the course, and where the river runs off the grid.

The fall drives a little more or less water over the bed than it was cut for: a pool's bowl and a rapid's bars and boulders hold it back, and the chute lets more through. What the solved current carries is therefore scaled to exactly the river's discharge, by deepening a rapid's bed (above) and scaling the current for the rest. Scaling the current is the solution for a fall scaled by the square of the same factor, since the bed's hold and the water's momentum both go by the speed squared. No current is written faster than 5 m/s (`FLOW_MAX`): a few samples where the outline pinches a channel shut would otherwise run at 7.

The result is the same bytes every run. Each stretch is solved on its own thread, and nothing inside a solve is parallel or random.

## Foam that remembers where it came from

The shader used to draw a river's foam by the current's speed at each pixel: streaks from 0.7 m/s, white from 1.5 m/s. A pool below a rapid was speckled evenly wherever it ran over 0.7 m/s, and slack water beside it looked like any lake.

Foam is now made in the rapids and carried. `riverflow::marks` (once `riverflow::foam`) works out, once the current is solved, how much of the surface is under foam at every sample:

- it is **made** where the water breaks, covering it in about 4 s (`FOAM_RISE`). That is in two places:
  - thin fast water: water that runs white, from 1.5 m/s and fully by 3.2 m/s (`WHITE_WATER_SPEEDS` in `crates/shared/src/consts.rs`, which the shader's own white water reads too), breaks by its Froude number, its speed over `√(g · depth)`: from 0.6, and all of it by 1.1 (`FOAM_FROUDES`). The sheet over a bar or a boulder is white, and a deep run at the same speed is a dark tongue. In an even channel the Froude number hardly depends on the depth, so it is the rough bed that tells them apart: the water is forced over a bar faster than it would run there alone;
  - fast water being braked: water that arrives at 1.5 m/s or more and loses 0.2 to 1 m/s each second (`FOAM_SLOWING`), below a boulder or a ledge and where a run meets a pool;
- it is **carried** along the solved current, and mixed sideways a little by the same eddy viscosity as the current, so it seeps into the slack water beside a fast run;
- it **thins** with age, to a third in 45 s (`FOAM_LIFE`).

Since `WORLD_VERSION` 36 it is also gathered where the surface converges and made a little along eddy lines, and it is carried by a scheme that keeps its edges, with a quarter of the sideways mixing: see [RIVER-SURFACE.md](RIVER-SURFACE.md).

This is marched to a steady state on the solver's cells, with no pressure solve, and takes no measurable time. The result is a trail that leaves each rapid along the main current, bends with it, and fades over a few hundred metres. Vindelälven has 168,000 m² under at least a tenth of foam, of its 1 km². It had 311,000 m² when foam was made by speed alone, over a smooth bed: a rapid was white from bank to bank.

It is stored as `Tile::foam`, a byte a sample, and reaches the shader as the water mesh's `uv.y` (`surfaceBuffers` in `client/src/render/watermesh.ts`). The mesh's `custom` attribute was full, and its uv's x only tells the refined mesh from the tiles'. `MeshRefiner` blends it into the refined mesh as it does `custom`. In the shader (`client/src/render/water.ts`) it replaces the streak-by-speed term: thin foam is scattered flecks and it closes up as it thickens, drawn with the froth noise that was already carried along the current. The white of the rapid itself is as before.

That froth noise is now read from the chop map (`mappedNoise`) and no longer worked out a pixel: a pool full of carried foam cost 0.53 ms with the worked-out noise. The second of its two crossfaded copies is turned, so the map's repeat does not show.

The eddies are weak, so this shows the flow's shape more than it shows spinning: where the fast water goes, and where it does not.

## What it costs

Importing `worlds/vindeln` (263 tiles, 1 km² of river in 260,000 solver cells) took 16.3 s before the solver and takes 20.3 s with it and the rough bed; the solve is 4.5 s of that, 165 steps. `worlds/home` solves Umeälven in 5.7 s, 141 steps. Both settle over the rough bed. The game pays nothing: the tiles hold the same fields as before.

The carried foam was measured with `gpu-bench.mjs` at the Retina size (2268 × 1258 at quality `high`) on two views of Vindelälven, each timed alternately with the foam turned off (the water material's `wave.z`):

| View | Frame | The carried foam costs | The frame before the foam |
|---|---|---|---|
| In the rapid (x −500, z −1871) | 4.73 ms | nothing measurable | 5.11 ms |
| Over the pool below the rapids, full of foam (x −760, z −400) | 4.72 ms | 0.23 ms | 4.99 ms |

The last column is the shader as it was, in a run of its own, so it is a rougher comparison than the alternated one. Both frames are cheaper than they were because the rapid's froth reads the chop map too. The tiles of `worlds/vindeln` grew from 62.3 to 63.8 MB (a byte a sample where a river has foam), and a foamy tile's water mesh keeps two more floats a vertex.

The benchmark gained `--at 'name=x,z,yaw,pitch,above'` for a view at a chosen place.

The importer logs each river of 500 m or more:

    water: a river falls 25.9 m over 5140 m, carrying 388 m³/s at up to 5.0 m/s, 63520 m² of it at 2.5 m/s or more and 167504 m² under foam; its steepest rapid keeps 17% of its depth, its rapids' rough bed was cut 1.19 times deeper to carry it, the chute down it is nowhere shallower than 1.2 m, and its bed's mean depth steps 1.10 m at most from one 20 m to the next (settled in 165 steps, 4.5 s)

Under the rule the solver replaced, the same river ran at 2.5 m/s or more over 141,000 m², at up to 4.0 m/s; over the smooth bed, 86,000 m². The rapids are a little slower on the whole and faster where they are squeezed, so the thresholds for foam and standing waves (`CHURN_SPEEDS`, and 0.7 and 1.5 m/s in the water shader) are left as they were.

## What it does not do

- **Lakes and the sea are still.** Only rivers flow, and only they carry foam: the river polygons, and the channels and pools of the rivers drawn as lines ([RIVER-CHANNELS.md](RIVER-CHANNELS.md)).
- **The level is not solved along the course.** It is still the profile fitted to the height model. Across the river it is now shaped by the pressure this solve finds under its lid, and a rapid stands in waves ([RIVER-SURFACE.md](RIVER-SURFACE.md)), but each cross-section's mean stays the profile's. A real river backs up above a narrows; this one runs faster through it instead, and where the bowl is too shallow for the water (Vindelälven's narrows above the fork) it runs at the 5 m/s limit.
- **No backwater profile shapes the bed.** Each cross-section's depth comes from its own slope. The level is the measured surface, which already holds the real river's backwater, and the slope is taken over 120 m, so the bed has no ledges: its mean depth changes by 1.1 m at most from one 20 m to the next on Vindelälven, a ramp of 1 in 18 where a pool meets a rapid. The importer logs that step for each river.
- **The bed's relief is made up.** Nothing measures a river's bed, so the bars and boulders are noise, put where the slope says a rapid is. They are not where the real ones are.
- **The bed's bars can show a faint straight seam** where the course's direction, which they are drawn out along, turns at once (behind an island, inside a sharp bend).
- **A side arm is a dead end.** A stretch has one course, measured from one end, and water enters only at the ends of that course. A tributary drawn as part of the river's polygon brings no water in.
- **The eddies are steady.** A real wake sheds vortices that drift downstream. (The boulders' drawn wakes do wander: they come from the wave field about the camera, not from this solve. See [WATER-WAVES.md](WATER-WAVES.md).) A solve that kept shedding would not settle, and is taken as it stands after 400 steps; both rivers in the worlds settle.

## Tests

- `riverflow.rs`, on channels made by hand:
  - `foam_is_made_in_a_rapid_and_trails_away_downstream_not_up`
  - `foam_spreads_from_the_fast_water_into_the_slack_water_behind_an_island`
  - `a_river_without_a_rapid_has_no_foam`
  - `the_current_slackens_behind_a_boulder_and_foam_trails_from_it`
  - `foam_is_made_over_a_shallow_bar_and_not_in_the_deep_tongue_beside_it`
  - `a_straight_even_channel_runs_at_mannings_speed_in_the_middle_and_slower_at_the_banks`
  - `the_same_water_passes_every_cross_section_and_runs_faster_through_a_narrows`
  - `the_current_parts_round_an_island_and_turns_back_behind_it`
  - `deep_water_runs_faster_than_the_shallows_beside_it`
  - `a_finer_grid_is_solved_on_every_second_sample_and_read_back_between_them`
  - `solving_a_river_twice_gives_the_same_current`
- `water.rs`, from outlines and a height model, as the importer runs:
  - `a_rapid_is_shallow_and_fast_and_a_pool_is_deep_and_slow` (and its foam trails below it, never above)
  - `a_rapids_bed_has_bars_and_boulders_and_a_pools_is_smooth`
  - `a_deep_chute_runs_unbroken_down_a_big_rivers_rapid`
  - `a_rough_bed_still_carries_the_rivers_discharge`
  - `a_river_runs_faster_through_a_narrows`
  - `the_current_parts_round_an_island_and_dies_at_the_bank`
  - `a_river_drawn_as_two_polygons_is_one_water`
  - `importing_a_river_twice_gives_the_same_current`
  - `a_falling_river_flows_downhill_and_fastest_at_rapids`, `a_level_river_runs_the_way_its_centre_line_is_drawn` and `a_bending_river_follows_its_course`, kept from before with their speeds measured again
- shared: the tile round trip and its byte layout carry `foam`.
- client: `world.spec.ts` decodes `foam`; `water.spec.ts` checks it reaches the mesh's `uv.y` and the tile worker. The refiner's blend of it runs on the GPU and was checked by eye, close over foamy water.
