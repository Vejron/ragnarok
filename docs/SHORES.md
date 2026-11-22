# Shores, streams and the trees beside them

A pond, a stream and the trees round them are built from OpenStreetMap's outlines and lines on a
grid of ground samples (1 m in the editor's worlds, 4 m by default). Drawn naively, the sample grid
shows: a staircase shoreline, a pale strip of a stream lying in the bottom of its ditch, and laser
trees standing in the water. This is how each is kept from showing.

## Natural outlines (`crates/osm-import/src/shoreline.rs`)

OSM's lakes and riverbanks are traced as long straight sides meeting at sharp corners. Before the
water is laid out, every lake, river polygon and coastline is made to look like a real shore:

- **A point every metre** (`SPACING`), so the outline can bend between OSM's nodes.
- **Its corners rounded**, cut back up to 4 m along each side, three times over (`ROUND`).
- **Bent by a smooth noise**, four octaves from 64 m across (up to 3 m) down to 3.5 m across
  (`WANDER`). At worst a point moves about 5 m.

Water bodies share edges: a lake and the river that drains it, the pieces of one riverbank, an
island and the lake round it. So the noise moves a point by where it is, the same for every
outline through it, never along its own outline's normal. A corner is rounded only where every
outline through it runs on to the same two neighbours; where two outlines part, as where a lake meets
its river, it is kept. A side is split at the same places from either end. A shared edge
therefore comes out the same from both polygons, with no dry sliver or seam between them. A
coastline's ends stay where they are, since the sea is closed along the world's edge from them.

The noise is a hash of the place, so every import gives the same shore. Everything else follows
the smoothed outline: the bed, the bank, the minimap, and the trees kept out of the water. The
outlines of woods, heath and rock are bent by the same noise (`roughen`), faded out near what is
man-made, so a wood at the water meets the shore ([GROUND.md](GROUND.md), "Natural edges").

## A shoreline on the outline (`crates/osm-import/src/water.rs`)

The client draws the water's edge where the depth, interpolated between samples, crosses zero
(`watermesh.ts` `surfaceBuffers`). The importer therefore makes that depth follow the outline:

- **Distances are to the outline itself**, not to the nearest dry sample (`shore_distance`).
  Wet samples within two cells of an outline measure to its nearest edge, and dry ones out to
  `bank_band` (two cells and 3 m), for the bank to be shaped. A wet sample only measures
  to an edge with dry ground beyond it, so where a lake's polygon meets a river's the line between
  them is open water. The rest of the water takes its neighbours' nearest bank points, swept over
  the grid both ways.
- **The bed shelves in at `SHORE_SLOPE`** (0.8 m a metre) from the waterline until it meets the
  bowl (`Kind::depth`), instead of starting 0.3 m deep at the outline. The first sample in is no
  longer at the foot of a one-metre wall.
- **The bank carries the slope on out of the water** (`bank_height`). A dry sample near the water
  rises from the waterline in proportion to its distance outside the outline:
  - one too low is raised, by no more than `BANK_RAISE` (0.2 m), to `SHORE_SLOPE`;
  - one too high is cut down: at `SHORE_SLOPE` over its first sample spacing, as the bed falls on
    the other side, then at `BANK_STEEP` (1.5 m a metre) out to `BANK_CUT_CELLS` (two) spacings,
    the cut easing back into the height model over `BANK_FADE` (3 m) further on.

  Before the cut, a river's high bank met the water at the nearest wet sample, since only the
  samples inside the polygon were carved, so its shoreline stepped along the 1 m grid however
  the outline ran. A bank too low to rise out of the water takes a surface that far under its
  ground instead (`levels`, up to 50 cm). Either way the depth is `-SHORE_SLOPE × distance` next
  to the water, and the ground crosses the water's level on the outline, so both the line where
  the water fades out and the line where the ground hides it follow the outline.

## The edge as drawn (`client/src/render`)

- **The water splits its cells as the terrain does.** RTIN's smallest triangles alternate their
  diagonals (`splitsAlongMainDiagonal`, `terrainmesh.ts`), and `surfaceBuffers` splits each water
  cell the same way. When it always used one diagonal, the line where the ground hid the water and
  the line where the water faded out crossed in every other cell, a zig-zag half a cell wide.
- **The shader fades the water out over the last 6 cm of depth** (`SHORE_FADE`, `water.ts`), so the
  part of a shore cell past the edge is not drawn. Before, that part showed as a jagged white strip
  of foam, lit by the mirror's sheen at a low angle.
- **The shoreline wanders and laps.** The depth is interpolated over the ground's cells, so its
  zero line is a run of straight pieces a cell long. The shader moves the line in by up to
  `SHORE_WANDER` (6 cm of depth) by a noise a few metres across, and by up to `SHORE_LAP` (4 cm)
  more, in and out every `SHORE_LAP_PERIOD` (9 s) out of step along the shore. The shore foam
  follows the same line. It only moves in, since the ground hides the water past the zero line.
- **The wet band on the ground is interpolated.** The stones texture's blue channel holds the
  ground's height over the water near it, from -1 to 3 m (`SHORE_ABOVE`, `stones.ts`), measured
  between the ground's samples at each 2 m texel. When it held 0 to 1 m, a bank rising through a
  texel clamped at both ends, and the band's edge followed the texels.

## Streams in their channels (`client/src/render/watermesh.ts`)

The importer cuts a stream's bed half a metre under its banks (`condition.rs`, see
[TERRAIN-SHAPING.md](TERRAIN-SHAPING.md)). The ribbon fills that channel:

- It is level across at `STREAM_FILL` (0.35 m) over the middle of the bed, 0.6 m wider than the bed
  on each side, so the 1-in-1 banks rise through it and the ground draws its edge.
- Its depth is the water over the ground at each vertex, negative past the banks, and the shader
  fades it there as it does a lake.
- It runs `STREAM_TIP` (1 m) past each end of its line, its depth tapering below zero, so it reaches
  into the pond it feeds and ends softly at a culvert. The line itself ends on the pond's drawn
  waterline (`Placed::out_of_water`), wherever OSM's points lie, so no ribbon runs on along the bed.

A river drawn as a line is no ribbon: it has banks and a shoreline like any river's
([RIVER-CHANNELS.md](RIVER-CHANNELS.md)).

The shader shades a stream by its own depth, in the river's colour: dark down the middle and clear at
the banks. It mirrors its banks more than the sky, since the scene's mirror is made for the lake's
level and misses it. Its shore foam is a fine line (`STREAM_FOAM_DEPTH`, 8 cm), not the lake's
35 cm, which had whitened the whole ribbon. Its sun glints are a third as strong.

## No trees in the water (`crates/osm-import/src/assemble.rs`)

The laser finds a crown that leans out over a pond at its top, so its tree's trunk would stand in
the water. Every tree and shrub, whether from the laser, a register, OCAD or the props, is dropped:

- where the water's depth at the trunk is above zero (`Placed::depth_at`, interpolated as the client
  draws the shoreline), or
- within a trunk's 0.5 m of the water drawn in a stream.

Rocks may still lie in water. The import counts what it dropped:

```
trees: 2686 against a house (21 of them planted), 506 in lakes and rivers, 489 in streams, …
```

## Tests

- importer:
  - `a_square_ponds_sides_wander_and_its_corners_round`, `two_bodies_sharing_a_side_still_meet_along_it`,
    `a_coastlines_ends_stay_put`, `the_same_outline_comes_out_the_same_every_import` (`shoreline.rs`)
  - `a_ponds_shoreline_follows_its_outline_not_the_samples`: on a round pond off the grid's lines,
    the interpolated zero is within 0.2 m of the outline everywhere, and the ground never rises
    out of the water inside it
  - `a_high_banked_ponds_shoreline_follows_its_outline`: the same, under a 3 m cliff and a bank
    rising at 2 m a metre
  - `a_bank_is_cut_down_to_the_water_and_eased_back_into_the_ground`
  - `a_ponds_bank_shelves_in_without_a_wall`
  - `where_a_lake_meets_a_river_the_water_is_not_shallow`
  - `trees_and_shrubs_do_not_grow_in_ponds_or_streams`
- client: `terrainmesh.spec.ts` checks that the water splits each cell along the full terrain's
  diagonal, and `water.spec.ts` checks that a stream fills its channel level across, wider than its bed,
  with real depths, and fades out past both ends
