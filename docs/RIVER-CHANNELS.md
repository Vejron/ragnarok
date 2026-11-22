# Rivers drawn as lines

OpenStreetMap draws a big river as a polygon from bank to bank, and a small one as a centre line
(`waterway=river`), here and there with a wider pool painted over it. The line used to be a stream: a
2.5 m ribbon on the ground. It is now a river like Umeälven. It has banks and a bed, a surface that
falls along it, a solved current with rapids and foam, and it can be swum, waded and drifted down.
Streams and canals drawn as lines are still ribbons ([SHORES.md](SHORES.md)), in the river's colour.

Tvärån through `worlds/home` is the example: 4.4 km of line with no polygon, falling 11 m into
Umeälven.

## In play

- **About 10 m across.** It is wider in slack reaches of an open valley (to 16 m), and narrows to as
  little as 4 m where the valley pinches, in a rapid, beside a house or a road, and between the ends
  of a short bridge.
- **One river.** The pools painted over a line are widenings of it: one surface and one current.
- **It meets the big river without a step**, and the big river backs up into its mouth.
- **Its banks can be climbed.** Where the valley is cut, it is cut at under 45°.
- **Bridges stay where they were.** A deck over a small river runs from bank to bank. Only a big
  river, a lake or the sea lifts a deck to sail under.

## How it works

### The channels (`crates/osm-import/src/channel.rs`)

`channel::lay` runs once the roads and buildings are read, and before the ground is sampled.

- **Courses.** The ways are joined end to start. Where rivers meet or part, the pair that makes the
  longest course runs on, and the others start or end on it.
- **Trunks and pools.** The sea, and any water at least `TRUNK_WIDTH` (30 m) wide on average (twice
  its area over its outline's length), is a trunk and is left as it was. Narrower water that a line
  runs through is a pool of that river, as is narrow river water that touches one.
- **Channels.** Each stretch of a course outside the trunks, inside the world, is a channel. It runs
  12 m on into the trunk at either end. A line that crosses under 100 m of a trunk's island is that
  trunk's own centre line and gets no channel.
- **Width.** What is wanted at each point comes from the valley's floor, the fall there and a slow
  wander. It is clamped to 4 to 16 m and to the room between houses, roads and tracks, changes by no
  more than 0.3 m a metre, and is then shifted so that the channel averages 10 m outside its pools.
- **Outline.** The zero line of the distance from the centre line less the half width, traced over a
  1 m raster. Offsetting the line to each side would fold over itself in a tight bend. Each bank
  wanders up to 0.8 m by itself where there is room. The outline is made after the shores are
  smoothed, whose 5 m wander would pinch a 4 m river shut.
- **Profile.** The lowest ground across the water, through a 24 m median (so an embankment across the
  valley is cut through), held under the lower bank and never rising downstream.

Each channel becomes a body of water under the mapped ones, which win where they overlap.

### The water (`crates/osm-import/src/water.rs`)

- `channel::courses_at` says which channel each sample of narrow water is, and how far along it.
- `channel_levels` sets each channel's surface, the longest first: its profile, held to the water at
  its two ends. Each channel is a stretch of its own, measured along its line. Measured by connected
  water, as the big rivers are, a small river would share one profile and one discharge with the
  river it runs into.
- `arm_levels` gives a pool's arm that reaches more than 24 m from the line its own fall. Level with
  where it leaves the river, the mill race by Umeälven was cut 8 m into the hillside.
- The bed, the rough rapids, the current and the foam are then made as any river's
  ([RIVER-FLOW.md](RIVER-FLOW.md)). A channel under 60 m is too short to solve, and drifts at 0.4 m/s.
- A channel's current lifts its surface by at most 6 cm (`CHANNEL_LIFT_MAX`), against a big river's
  40 cm, which stood as a hump across all of a 10 m river before every narrows.
- Once the current has shaped the big river, the mouths follow its level.
- The bank is cut at `CHANNEL_BANK` (0.95 m a metre), since at the big rivers' 1.5 a swimmer could
  not climb out. It is raised only where it is all but level with the water.

### The crossings (`bridge.rs`, `assemble.rs`)

`Placed::channel` marks the channels' samples and the banks cut down to them.

- Over a channel a deck needs 1.3 m from surface to roadway (`CHANNEL_CLEARANCE`), not the 3.5 to
  7 m of `bridge::clearance`.
- It is never taken for a culvert, it is an open span with no pier in the water, and its solid ends
  stop short of it.
- A bridge whose end is in a channel or its cut bank is carried on along its road to the bank
  (`out_of_channels`).
- A road, path or track that crosses a channel with no bridge in OSM gets a flush deck
  (`river_crossings`). Rivers are never cut for a culvert as streams are. A river that OSM itself
  puts in a tunnel (`tunnel=culvert`) still ends at it.

### The streams' colour (`client/src/render/water.ts`)

A stream takes the world's river colour. Its 0.35 m of water takes out light as 1.5 m of a river
does (`STREAM_LOOKS_DEEP`), so it is dark down the middle and not the colour of its bed, and its
pale bands are toned down.

## What it does not do

- **Ditches and drains are not imported.**
- **The width and depth are made up.** OSM has neither. Every such river carries what its cross-section
  would at 0.4 m/s (about 7 m³/s for Tvärån), however small the real one is.
- **A tributary brings no water.** The river it joins carries its own discharge from end to end.
- **The banks are only as fine as the ground's samples.** On home's 2 m samples a 4 m pinch is two
  samples of water.

## Tests

- `channel.rs`: the average and least width, narrowing past a house and under a short bridge, a
  hairpin with no hole, the longest course at a confluence, and a surface cut through a rise and
  under a low bank.
- `water.rs`: a pool and its line as one flowing water, a tributary meeting a big river, a pool's
  arm climbing the valley's side, and banks a soldier can climb.
- `bridge.rs`, `assemble.rs`: a deck level with a small river's banks, a short bridge still built, a
  bridge carried on to the bank, and a deck where a road has no bridge.
- `osm_water.rs`: a river's line is no ribbon, and a stream's still is.
