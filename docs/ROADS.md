# Roads: lanes, paint and the rail down a 2+1 road

Roads are asphalt ribbons laid on the ground (`client/src/render/roads.ts`). This write-up covers
three things:

- **Width and lanes** from OpenStreetMap's tags.
- **White paint**, as on Swedish roads.
- **A steel guard rail (mitträcke)** down the middle of a divided road, the way a Swedish 2+1 road has one.

It also covers how the asphalt is built so that its shoulder and the ground never show through it.

## Width and lanes (`osm-import/src/lanes.rs`)

`lanes::style` reads a highway way's tags. Swedish roads carry `lanes` well, and `width` often, which
comes from the national road database. Paths, tracks and paved paths have no lanes and keep the
widths of their class (`assemble::road_style`).

| Tagged | Width |
|---|---|
| one lane (a one-way street, a slip road, a carriageway, a service road) | 5.5 m (`ONE_LANE`), whatever its class |
| two lanes or more | lanes × 3.25 m (3 m on a minor road) + 1 m of shoulders: 7.5 m for a main road of two, 7 m for a street |
| a roundabout's ring | at least 6 m (`ROUNDABOUT`) |
| a paved track | 4.5 m |
| a `width` tag | used, brought to within 1 m under and 2 m over the width above |

**One-way roads.** A road is one-way if it is tagged `oneway`, or if it is a motorway or a roundabout.
A one-way road has one lane unless tagged otherwise, two on a motorway. Each carriageway of a
divided road is therefore as wide as its own lanes, not as a whole road of its class.

**Direction of the lanes.** `Road::forward` is how many lanes run the way the road's points do:
- `lanes:forward` if the way has it;
- otherwise half the lanes, rounded up;
- on a one-way road, all of them (none for `oneway=-1`).

`Road::one_way` says whether the road is one-way at all, which `forward` cannot for a road of a single lane: a service road and a slip road both have one lane, `forward`. Traffic drives by it ([TRAFFIC.md](TRAFFIC.md)).

**Knock-on effects.** The ground is graded to the new width (`condition.rs`), and bridges take it too.

## Paint

`Road::marks` holds the lines painted on a road (`MARK_*` in `crates/shared/src/world.rs`). The
importer decides them:

- **Which roads are painted.** A major road is painted unless it has `lane_markings=no`. A minor road
  is painted only with `lane_markings=yes`: most Swedish tertiary and residential roads have no lines.
- **Edge lines** (kantlinje). They are dashed, with 1 m of paint and a 2 m gap. On a motorway or a
  `motorroad=yes` they are solid (`MARK_EDGE`, `MARK_EDGE_SOLID`).
- **Centre line.**
  - It is dashed, with 3 m of paint and a 9 m gap (`MARK_CENTRE`).
  - It is solid (spärrlinje, `MARK_CENTRE_SOLID`) where overtaking is barred: on `overtaking=no`, or
    where `lanes:forward` gives unequal lanes each way, as on the single side of a 2+1 stretch with no
    barrier.
- **Lane lines** between lanes that run the same way are dashed like the centre line (`MARK_LANES`).
- **Divided roads.** A carriageway of a divided road gets no edge line on its rail side
  (`MARK_NO_LEFT_EDGE`). On a slow divided road paved up to its other carriageway, that edge line
  is solid instead (`MARK_LEFT_SOLID`).

### How the paint is drawn

The paint is drawn by the asphalt's own shader (`engine/src/materials/asphalt.wgsl.ts`), not by a
layer of its own. A layer over the asphalt would fight it for which is on top over bumpy ground, and
would cost pixels.

Each asphalt vertex carries three things in `custom`:
- **How far along the road it is,** in metres. It counts from `Road::along`, the distance along the
  OSM way at the piece's first point, so a dash's rhythm runs on across tile borders.
- **How far right of the middle it is,** in metres.
- **The road's half width with its paint packed in** (`paintProfile`, `PROFILE_SCALE`).

The lanes lie between the edge lines, the ones running against the points on the left (traffic drives
on the right).

**Junctions.** The paint stops where another road's asphalt is over it. `custom.x` holds how deep into
the whole paved surface a vertex is (`Reach`, as for the edges), and where that disagrees with the
road's own distance from its middle, the point is inside another road. A way that carries straight on
agrees, so its lines run through.

## The rail down a divided road (`osm-import/src/barrier.rs`)

OSM does not tag the barrier. It draws a road divided by one as two one-way ways, one per
carriageway. Reading every main road of central Sweden in the local extract shows:

- **A Swedish 2+1 road** is a `lanes=1` way and a `lanes=2` way, their centre lines 6 to 12 m apart.
- **A motorway's carriageways** are 10 to 24 m apart.
- **The other carriageway** is on the left in 99.8% of samples.
- **No main road carries a `divider` or `barrier` tag.**

So `barrier::lay` infers the rail.

### Pairing

A carriageway is a one-way `motorway`, `trunk`, `primary` or `secondary` way: not a slip road, a
roundabout or a tunnel. Every 4 m along it, the importer looks for another carriageway that:

- heads the other way;
- lies on its left;
- has its centre line within 30 m.

### Where the rail stands

- **Between the inner edges.** The rail stands midway between the two carriageways' inner edges. If
  their ribbons overlap, it stands at the point split by their half widths.
- **Only on fast roads.** Both carriageways must be fast: a motorway, a trunk road, a motorroad,
  `maxspeed` 70 or more, or a primary road with no `maxspeed` (most are country roads).
- **Left open at junctions.** It stops 20 m short of any junction on either carriageway, as the real
  barrier does. A junction is a point shared with another road. Two pieces of the same carriageway
  meeting end to end, running on in the same direction, do not count.
- **Short pieces dropped.** A rail shorter than 20 m is not built.
- **Gaps elsewhere.** It also stops on bridges and in tunnels, where there is no carriageway on the
  ground to pair with.

### Paving between close carriageways

Where the two carriageways' edges are less than 4 m apart, the strip between them is paved: an
unpainted `ROAD_MAJOR` road reaching 1 m under each. The two then read as one road with the rail
down it, and their edges lie inside the paved surface, so they do not crumble into a sliver of
verge. A slow divided road is paved between in the same way, with a solid line in place of the rail.

Where a wider two-way road forks into the two carriageways, both start on its centre line, and until
they have drawn apart the two together are narrower than the road they left: a waist in the asphalt.
The fork is paved as wide as that road, along the line between the carriageways, as far as their
outer edges are closer together than its width (`barrier::lay`).

### The rail itself

The rail is a `LINE_GUARDRAIL` line in the tile's `lines`, so it is built and broken like the walls
(docs/BARRIERS.md). Each 2 m piece is a collider that stops a light vehicle and breaks under a heavy
one, a blast or a Titan. It is drawn as the bridge railing brought down to 0.75 m:
- galvanised square posts;
- two square rails, at 0.69 m and 0.4 m.

### Scraping along it

A car that grazes the rail is turned aside, as by a real one, and drives on:

- **It pushes only straight across, and level.** `sim::fences::RailHooks` rewrites each contact
  with a rail piece so it pushes square to the rail's line, towards the car's side. Without it, a
  car scraping along caught its front corner on the square end of the next 2 m piece, where two
  pieces meet, and stopped dead at speed. A car hitting it square on was also pushed out over its
  top, since at 30 m/s a car moves further in a tick than the rail is thick.
- **It is slippery** (friction 0.05).
- **It breaks only under a blow.** A piece of any wall breaks by the speed it takes out of the car in
  one tick (`run_down_trees`), not by the car's own speed. A scraping car, or one leaning on the rail
  as it swings round, loses little to it. Before this, any touch above 7 m/s knocked a piece out and
  left the next piece's end in the car's way. The rail has 90 health, so it takes a 15 m/s blow.

A sports car at 108 km/h (`a_car_grazing_a_guard_rail_slides_along_it_unhurt_and_a_square_hit_breaks_it`):

| Angle into the rail | What happens |
|---|---|
| up to 17° | slides along, unhurt, nothing breaks |
| 29° | slides along, loses 18 of 100 health |
| 57° or more | the rail breaks and the car is wrecked |

## How the asphalt hides its shoulder and the ground

A paved road is two ribbons: the gravel shoulder (the casing) and the asphalt (the fill). Before
this change, the shoulder was a full-width sheet 2 cm under the asphalt, with its own vertices. Over
lumpy laser ground the two meshes' triangles cut the bumps differently, and the gravel poked
through the asphalt in flecks. Now:

- **The shoulder is a band along each edge.** It runs from 0.25 m inside the asphalt (`SHOULDER_UNDER`,
  past the asphalt's crumbled edge) out to its own edge.
- **No shoulder triangle lies under any road's asphalt,** at a joint's disc or a junction alike. The
  shoulder's `Layer.under` drops those triangles.
- **Ribbons lap at tile borders.** A ribbon reaching a tile border runs 0.3 m on past it
  (`BORDER_OVERLAP`). Each tile's piece ended with a cut of its own, and the two cuts never quite met,
  so a hairline of ground showed across the road there.

## Patches and cracks

**Mended patches** are laid in the road's own frame, along it and across it, so they lie square to
the road whatever its heading. One stretch in three of every 11 m holds one of these:
- a rectangle 1.2 to 6 m long down a wheel track;
- a lane-wide rectangle;
- a filled pothole under a metre square.

**Cracks** follow a noise with a finer one on it, so they zigzag. Their width is set in metres: the
noise's distance from its crossing is divided by how steeply it changes there. Before, the width was
in the noise's own units, so a crack swelled into a wide wobbly band wherever the noise was flat.
Some cracks are sealed with a 5 cm strip of tar, and some stretches of a painted road have the joint
down their middle sealed too.

## What it costs

`gpu-bench.mjs`'s spawn view (`forest`), beside a road at 2268×1258, timed against the same frame with
the asphalt look switched off:

| Asphalt shader | forest | vista |
|---|---|---|
| Before this change: mottling, grit, world-square patches, contour cracks | 0.18 ms | 0.03 ms |
| With paint, frame patches and metric cracks, every detail worked out at every pixel | 0.32 ms | 0.05 ms |
| The same, each detail worked out only where it can show | 0.16 ms | 0.01 ms |

Most of a road's pixels are far off, uncracked and unpainted. So:
- **Grit** is worked out only close up.
- **Crack noise** is worked out only in cracked stretches near enough to see one.
- **The tar joint** is drawn only within 0.2 m of the middle.
- **The paint's wear** is computed only under paint.
- **A crack's slope** comes from the value noise's own derivative (`asphaltNoiseSlope`), one evaluation
  an octave instead of three.

## Limits

- **No paint or guard rail on bridge decks.** The road on a deck is the roof of the `BUILDING_BRIDGE` mesh,
  drawn by the walls' shader, which has no frame along the road, so its patches lie square to the
  world.
- **No guard rails along the edges** of embankments, and no wire-rope (cable) barriers. The rail is
  the beam kind throughout.
- **2+1 stretches without a barrier** get the solid line only where `lanes:forward` is tagged.
- **Grass at the road's edge.** The grass mask is a 2 m grid (`sim/surface.ts`), so grass can stand on
  the asphalt up to a cell in from its edge.

## One road from junction to junction

OpenStreetMap cuts a road into a new way wherever a tag changes, and each way was graded, drawn and
painted alone. `network::join` (run before and after `sidepaths::prune`) joins two ways that meet end
to end at a point no other road touches and are the same road there: the same kind, lanes and paint,
both on the ground or both a bridge or tunnel, driven the same way. Widths within 1.5 m (`EVEN`) are
evened out to the longer piece's. `worlds/home` closes 775 joints.

A signed `maxheight` ends a road only where it runs under a railway's bridge, which leaves just that
much room over it (WORLD.md, "Bridges"); anywhere else a limit is no reason to cut one. Tracks are
joined the same by `network::join_tracks` (WORLD.md, "Railways").

On the client (`render/roads.ts`):

- **A bend is mitred.** The strips either side are cut on the line that halves it and meet edge to
  edge. Only a bend sharper than 60° (`MITRE`), or one whose strips are too short for the cut, still
  gets a round patch.
- **An end that meets another road** gets a patch no wider than the narrower of the two, and a wide
  road narrows to a narrower one it ends on over 8 m (`TAPER`), so nothing bulges out past the joint.
  A road that ends alone keeps its round cap. The paving between carriageways stays full width to its ends.
- **Each kind has its own depth rank**, so two kinds of path that cross do not flicker.
- **Major and minor roads share one asphalt colour**, which is also a bridge deck's.
