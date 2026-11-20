# Shores, streams and the trees beside them

A pond, a stream and the trees round them are built from OpenStreetMap's outlines and lines on a
grid of ground samples (1 m in the editor's worlds, 4 m by default). Drawn naively, the sample grid
shows: a staircase shoreline, a pale strip of a stream lying in the bottom of its ditch, and laser
trees standing in the water. This is how each is kept from showing.

## A shoreline on the outline (`crates/osm-import/src/water.rs`)

The client draws the water's edge where the depth, interpolated between samples, crosses zero
(`watermesh.ts` `surfaceBuffers`). The importer therefore makes that depth follow the outline:

- **Distances are to the outline itself**, not to the nearest dry sample (`shore_distance`).
  Samples within two cells of an outline measure to its nearest edge. A wet sample only measures
  to an edge with dry ground beyond it, so where a lake's polygon meets a river's the line between
  them is open water. The rest of the water takes its neighbours' nearest bank points, swept over
  the grid both ways.
- **The bed shelves in at `SHORE_SLOPE`** (0.8 m a metre) from the waterline until it meets the
  bowl (`Kind::depth`), instead of starting 0.3 m deep at the outline. The first sample in is no
  longer at the foot of a one-metre wall.
- **The bank carries the slope on out of the water.** A dry sample next to the water is raised, by
  no more than `BANK_RAISE` (0.2 m), to rise from the waterline at the same slope. A bank too low to
  rise out of the water takes a surface that far under its ground instead (`levels`, 10 to 50 cm).
  Either way the depth is `-SHORE_SLOPE × distance` there, so the interpolated zero falls on the
  outline from both sides.

The shader fades the water out over the last 4 cm to the shoreline (`SHORE_FADE`, `water.ts`),
so the part of a shore cell past the edge is not drawn. Before, that part showed as a jagged white
strip of foam, lit by the mirror's sheen at a low angle.

## Streams in their channels (`client/src/render/watermesh.ts`)

The importer cuts a stream's bed half a metre under its banks (`condition.rs`, see
[TERRAIN-SHAPING.md](TERRAIN-SHAPING.md)). The ribbon fills that channel:

- It is level across at `STREAM_FILL` (0.35 m) over the middle of the bed, 0.6 m wider than the bed
  on each side, so the 1-in-1 banks rise through it and the ground draws its edge.
- Its depth is the water over the ground at each vertex, negative past the banks, and the shader
  fades it there as it does a lake.
- It runs `STREAM_TIP` (1 m) past each end of its line, its depth tapering below zero, so it reaches
  into the pond it feeds and ends softly at a culvert.

The shader shades a stream by its own depth: dark down the middle and clear at the banks, over a
muddy bed. It mirrors its banks more than the sky, since the scene's mirror is made for the lake's
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
  - `a_ponds_shoreline_follows_its_outline_not_the_samples`: on a round pond off the grid's lines,
    the interpolated zero is within 0.2 m of the outline everywhere
  - `a_ponds_bank_shelves_in_without_a_wall`
  - `where_a_lake_meets_a_river_the_water_is_not_shallow`
  - `trees_and_shrubs_do_not_grow_in_ponds_or_streams`
- client: `water.spec.ts` checks that a stream fills its channel level across, wider than its bed,
  with real depths, and fades out past both ends
