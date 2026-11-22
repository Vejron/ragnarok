# Rivers that run

A river's water now flows. It runs downstream as its OSM centre line is drawn, fastest where its surface falls steeply, and carries swimmers and whatever floats on it. Rapids run white and heave in standing waves. Vindelälven falls 26 m through `worlds/vindeln`, and its rapids at Vindelforsarna run at 3 to 4.5 m/s.

## In play

- **Swimmers drift downstream.** The current is added to their strokes. A fast swimmer (3.2 m/s) can make headway up a gentle stretch but loses ground in a 4 m/s rapid. The current slows towards the banks, so a swimmer can always make for the shore. (A rapid is now shallow enough to stand in over much of its width: see [RIVER-FLOW.md](RIVER-FLOW.md).)
- **Boats, ships, submarines, amphibious vehicles and seaplanes drift** while they are crewed and nothing holds them. Their water physics now run on their speed through the water, so a boat driven upstream makes its top speed less the current over the ground, and one let go is carried along.
- **Mooring.** The handbrake is the anchor, and the server holds the handbrake on every empty vehicle. So a boat, ship, submarine or amphibious vehicle left in a river stays where it was left. A seaplane with nobody aboard is moored the same way. A driver who holds the handbrake in a current drops anchor: the hull brakes to a stop over the ground and holds there.
- **White water.** A rapid stands in waves across the current, which are part of the river's level, and from 1.2 m/s they surge up to 10 cm in place, the same on the server and in the client's prediction. A boat bucks over them and a swimmer bobs on them. See [RIVER-SURFACE.md](RIVER-SURFACE.md), which replaced the two standing waves in fixed headings (up to 22 cm) this had.
- **How it looks.** The river's ripples are carried along by its current (all of them now, and more: [RIVER-SURFACE.md](RIVER-SURFACE.md)). Foam trails away below each rapid along the current, from 1.5 m/s the water breaks white in patches, and in the fastest rapids it is white over dark water, thickest on the crests of the standing waves. Lakes look as before.

## How it works

### The importer (`crates/osm-import/src/water.rs`)

`river_levels` measures each river stretch along its course and fits a profile that only ever falls one way. It also finds which way the stretch runs:

- **Small rivers.** A river drawn as a line is levelled along that line instead, as a stretch of its own: see [RIVER-CHANNELS.md](RIVER-CHANNELS.md).
- **Downstream.** The river's OSM centre lines (`waterway=river`, drawn downstream) vote on which way the distance along the stretch grows (`downstream`). Where none crosses the stretch, the way the profile fits the ground better decides. The vote also picks which profile is used, so the water never runs uphill.
- **Direction.** The direction downstream is the gradient of a fast-marched (eikonal) distance from the same end (`march`). It is not taken from the chamfer distance used for the levels, whose 3-4 metric leans the gradient towards the eight grid directions: it pointed the current 18° off a straight river.
- **Slope.** The slope is measured over 60 m up and down the course. A stretch with no centre line and less than 0.3 m of fall stays still.

The current itself is then solved as a flow over a bed cut to carry the river's discharge, rough in the rapids with bars, boulders and a chute for boats, which [RIVER-FLOW.md](RIVER-FLOW.md) describes. It replaced a rule (`25·√slope` m/s along the course, blurred over 8 m and slowed towards the banks) under which a rapid ran at 4 m/s over a bed as deep as a pool's.

`Tile::flow` stores it as two signed bytes a sample in 0.05 m/s steps (`FLOW_STEP`), and is empty in a tile where nothing flows. The importer logs each long stretch's top speed and how much of it runs at 2.5 m/s or more.

### The sim (`crates/sim`)

- `Waters` keeps the current per tile and samples it bilinearly (`flow_at`). `Surface` carries it, and `waves::churn` adds the standing waves' surge by the current.
- `Water::drift(anchored)` is the current a hull is carried by, none at anchor. Each lift (`boat`, `sail`, `dive`, `floats`, and `swim` for amphibious hulls) takes the drift off the hull's velocity, runs its usual drag, keel, thrust and brake on what is left, and adds the drift back. The water's hold on the hull is therefore exactly its existing drag. A hull aground, held by none of it, is not carried.
- `Physics::swim` adds the current to a swimmer's move.
- Wheeled cars wading on the bed are not carried; their tyres hold them.

The client's prediction is the same sim in WASM, and loads the same tile bytes, so the drift is predicted.

### The client

- `decodeTile` reads `flow` after `reach`, and `tileParts` passes it to the tile worker. The client's `Waters` keeps it too (`flowAt`).
- A swimmer is animated, and stirs the water, by their speed through the water: their velocity less the current (`Soldiers`, `poseLocal`). Paced by their speed over the ground, a swimmer carried down a rapid stroked flat out while lying still in the water.
- Wakes go by speed through the water too. `WaterWaves` takes the current off a hull's velocity before it decides how hard the hull pushes and churns the water, for boats, waders and the titan's feet alike. `VehicleFx.skim` (through `FxWorld.currentAt`) does the same before deciding on a wake, planing spray, bow water and a waterjet's spray. A boat let drift down a rapid leaves the water as it found it, and one driven up it throws its full wake while barely gaining ground. The hull is still stamped with its velocity over the ground, which says where it was a step ago.
- `surfaceBuffers` puts the current in a lake's or river's `custom.zw` as `(x, z − FLOW_BIAS)`. Its `w` thus stays far below a stream's flag of 1, and `MeshRefiner` carries it into the refined mesh with no new attribute.
- The water shader (`render/water.ts`):
  - It carries the chop along the current: two copies of it, each dragged downstream for 1.2 s (`FLOW_CYCLE`) and then started over, half a cycle apart and crossfaded.
  - It draws foam streaks by smearing noise back along the current. Where they lie is no longer the current's speed there but the foam the importer found carried there (`TileData.foam`, in the mesh's `uv.y`): see [RIVER-FLOW.md](RIVER-FLOW.md). Rotating world coordinates by the current's direction, as first tried, sheared the noise into zigzags wherever the current turned.
  - It lights the surface with the slopes of `churnWaves`, the WGSL copy of `waves::churn`, and thickens the foam on their crests.
  - Still water skips all of this with one branch.

### Two bugs it brought to light

- **The mirror flipped between levels.** The scene is mirrored in one plane, at the level `Waters.planeNear` votes for among the water about the camera. On a river that steps down through rapids, that water stands at several levels, and the vote went by 10 cm buckets, weighted towards where the camera looks. Turning the camera a little made another bucket win, and the reflection jumped. Now:
  - a level's votes are all the water within the 0.3 m the shader mirrors (`PLANE_BAND`), so a gently falling stretch is one surface;
  - the level mirrored last frame stays while it has 80% of the best one's votes (`PLANE_KEEP`). Turning from the water below a rapid to the water above it changes the mirror once, part way through the turn.
  - the plane now leans with the river, fitted to the water about the camera ([RIVER-SURFACE.md](RIVER-SURFACE.md), "The mirror").
- **The refined water near the camera vanished, showing the riverbed as a green square.** `MeshRefiner` writes its square into the geometry arena's buffers on the GPU, and only again when the square or its sources change. When something new grew the arena later in the same frame (a rocket's blast and crater, say), the arena copied its old buffers into new ones before that write ran, so the write was lost. A helicopter hovering over the water moved the square nowhere, so it stayed empty while the tiles kept leaving it to the refined mesh. The refiner now remembers the buffers it wrote into and writes again when they are replaced. This bug was older than the rivers.

- **A shell that went into the water burst as fire on land about half the time.** The server ends a round that goes into open water at the still level it meets, and bursts it there. The client (`Surroundings.watery`) judged that point against the surface with its waves: in a trough a few centimetres down, the point was above the water, and the blast was drawn as on land. It now counts a point as on the water when it is at or under either the still level or the waves.
- **The refined water had a dashed dark border, and was no use from high up.** The tiles' water and the refined mesh were cut along one line, as two meshes whose positions round differently. Here and there a pixel along it was drawn by neither, and the ink outlined the bed showing through. Now:
  - the refined mesh reaches a cell past its square and its shader discards what lies outside it, the same per-pixel test (`inRefine`) by which the tiles discard what lies in it;
  - more than 30 m over the water about the camera (`REFINE_HEIGHT`, measured from the mirrored level), or with none about, it stands down and the tiles draw all the water. Wakes still light the surface from up there; only their lift on the refined vertices, too small to see from that height, goes.

## Cost

Measured with `gpu-bench.mjs` at the Retina size (2268 × 1258 at quality `high`), standing in Vindelälven with the river filling most of the view. Each frame with the current was timed alternately against the same frame with the current zeroed in the water meshes:

| View | Frame | The current costs |
|---|---|---|
| In the rapid (4 m/s) | 5.40 ms | 0.20 ms |
| Downstream of it (about 2.7 m/s) | 6.03 ms | 0.23 ms |

Still water takes the same path as before and costs nothing more. The sim's cost is one bilinear lookup a floating hull or swimmer a tick. The tiles grow by 2 bytes a wet sample in tiles where a river flows: Vindeln's tiles are 2% bigger compressed.

The first version cost 1.1 to 1.7 ms. Timing its parts one at a time:
- Carrying the chop along (twice the noise): nothing measurable. Only its broadest octave is carried now.
- The foam: 0.3 to 0.5 ms; trimmed since.
- Lifting the drawn vertices by the standing waves: 1.3 to 1.8 ms on its own. The cause was not found: it is not the ink pass, nor the waves' arithmetic, which ran in both variants. The drawn surface is therefore no longer lifted by them; their slopes still light it.

## Tests

- importer (more in [RIVER-FLOW.md](RIVER-FLOW.md)):
  - `a_falling_river_flows_downhill_and_fastest_at_rapids`
  - `a_level_river_runs_the_way_its_centre_line_is_drawn`
  - the bend test checks the current turns with the river
- shared: `white_water_heaves_in_place_only_where_the_river_runs_fast`; the world round trip and merge tests cover `flow`
- sim:
  - `let_go_on_a_river_it_drifts_downstream_and_at_anchor_it_holds`
  - `its_jets_drive_it_through_the_water_so_upstream_it_makes_less_way`
  - `on_a_river_it_lies_moored_with_nobody_aboard_and_drifts_with_its_engine_idling`
  - `a_swimmer_is_carried_downstream_and_cannot_swim_up_a_rapid`
- client:
  - `waves.spec.ts` holds `churn` to the Rust one
  - `water.spec.ts` checks the current reaches the mesh and the tile worker
  - `waterwaves.spec.ts`: a hull on a river pushes and churns by its speed through the water, and a boat carried at the river's speed carves no wake
  - `world.spec.ts` decodes it
