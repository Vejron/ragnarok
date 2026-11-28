# Frame performance

What the main thread spends a frame while the world streams in and while a house comes down, how it
is measured, and what was taken out of the frame. [GPU-PERFORMANCE.md](GPU-PERFORMANCE.md) is the
GPU's side, [SERVER-PERFORMANCE.md](SERVER-PERFORMANCE.md) the server's, and
[TILE-STREAMING.md](TILE-STREAMING.md) which thread makes what of a tile.

**Short version:** driving across a town, the worst frame's own time went from 26.6 ms to 10.8 ms, and
a shell landing on a house from as much as 27 ms to 4 ms, with the look, the falling sections and the
heaps as they were. Making a tile's buildings solid, interleaving vertices, falling sections and
rubble heaps were what the frame spent its time on. The worker threads do most of that now, and what
is left is a frame's share at a time.

## Measuring

```sh
# 40 s across worlds/test at 25 m/s, tiles streaming in all the way
node client/scripts/frame-bench.mjs --scenario drive --seconds 40 [--speed 25] [--heading 90]
# the cuts the server made of a house, replayed through `WorldView.carved` one every 0.6 s
HOUSE=10 DEMOLITION_OUT=$PWD/client/bench-out/cuts.json RAGNAROK_WORLD=$PWD/worlds/test/world.msgpack cargo test --release -p server -- --ignored --nocapture demolition_cost
node client/scripts/frame-bench.mjs --scenario demolish --cuts client/bench-out/cuts.json --every 0.6 [--shoot]
# the end of a long fight: craters already dug, people on foot and cars with their crews all round
node client/scripts/frame-bench.mjs --scenario chaos --world home --seconds 30 --craters 2000 --people 150 --cars 40 [--shoot]
```

It drives the real game in headless Chrome (Windows and Mac), loaded with `?bench`, and records a CPU
profile meanwhile. It prints:

- the spread of the frames' intervals and of their own time (`cpu`, the frame callback start to end);
- the time in each named span in the long frames and over the whole run;
- the functions that took the most time in the long frames, and in what ran between frames;
- the worst frames one by one, each with its spans.

Every frame record goes to `client/bench-out/<label>.json`. `--shoot` saves pictures of a demolition
(just after the first cut, half way, and at the end). `--probe <js>` runs code in the page before the
run, to count or switch something off; `--after <js>` runs code after it and prints what it returns.
Comparing the drawn look with a fresh `lookFracture` of the final cells is one such check.

**Spans** are named stretches of main-thread time, kept only under `?bench`, so they cost nothing in
play (`client/src/spans.ts`): `span(name, fn)` around a call, or `t = lap(name, t)` between the steps of
a long function. A span that ran outside any frame (a fetch's continuation, a worker's reply, a
network message) is counted between frames, shown with a `~`. `render/perf.ts` keeps each frame's
record, and the renderer reports the bytes it uploaded (`RenderStats.uploadBytes`).

### Traps

- **A long frame shows in the next frame's interval**, not its own. The report judges frames by their
  own time and puts what ran between frames apart.
- **Headless Chrome on Windows runs at about 165 Hz**, so a frame's interval is 6.1 ms when nothing is
  wrong. The first interval of a run is long while the profiler starts.
- **The game alone tries to reach a server**, and when it gives up it reloads a world that has been
  hurt. A demolition then forgot its damage half way and reloaded every tile. The bench stops those tries.
- **Node is not the browser:** the `tiles.bench` and `destruction.bench` specs time single stages in
  Node, close but not the same as Chrome; the frame-bench is the one to trust for frames.

## Where the frame went, and where it goes

Measured on a Windows desktop in headless Chrome at quality `high`, on `worlds/test` (Umeå, 2 m ground);
a slower laptop takes two or three times as long for each.

**Driving**, 40 s at 25 m/s, 20 tiles attached:

| | Before | After |
|---|---|---|
| Worst frame's own time | 26.6 ms | 10.8 ms |
| p99 / p95 | 6.1 / 3.7 ms | 4.2 / 3.4 ms |
| A tile's physics in its frame | up to 12.1 ms | up to 1.9 ms |
| `tileGroup` | up to 4.8 ms (9.6 seen) | up to 1.3 ms |
| `GateView.attach` | 1.3 to 2.6 ms | 0.1 ms |
| The frame's render while 12 MB uploads | 5 to 7 ms | 2 to 3 ms |
| Grass, most in a frame | 6.7 ms (9 seen) | about 4 ms |

**Shelling a house**, 21 cuts (a section of road bridge; `HOUSE=10` of `demolition_cost`):

| A cut on the main thread | Before | After |
|---|---|---|
| `BuildingDamage.carve` (first / later) | 26.7 / 1 to 19 ms | 4.0 / median 0.4 ms |
| of which the pieces thrown | up to 22 ms | up to 2.2 ms |
| The rubble heap, a second later | 11 ms | 0.6 ms (its ground sampled) |
| The prediction's collider (`reshape`) | 2.0 to 4.8 ms | median 2.0, worst 3.4 ms |

A whole house coming down at once (962 cells, `HOUSE=30`) costs 6.1 ms on the hit, of which 4.3 ms
is the dust and shards, and 3.9 ms for its collider.

## What was done

**A tile's buildings are made solid from a queue** (`Physics::build_pending`, `crates/sim/src/stream.rs`).
`ClientSim.addTile` made every building's collider in the tile's frame: openings, rooms and gates
(`BuildingLook::of`), its walls with the windows cut out, and a trimesh, serially in WebAssembly, up to
12 ms a town tile. Now `add_tile` adds the ground, trunks, fences and tombstones, and the buildings wait.

- Every frame `WorldView.buildNear` makes solid those within 80 m of the camera at once and as many more
  as about 2 ms allows, nearest first (`SimMotor.buildBuildings`).
- What is done to a building meanwhile is kept and built in: cells carved out (`carve_building`), gates
  opened or shut (`set_gates`), the building brought down (`destroy`).
- `prime` and a redrawn world make every building of the tiles they wait for solid before the game moves.
- Far buildings are solid within a second or two of their tile arriving. Until then the gunsight's range
  and the arcs see through them, which shows only in the client's readout.

**Meshes are packed where they are made** (`packVertices`, `GeometryData.packed`, `render/packing.ts`).
The arena interleaved every new mesh into its 64-byte layout on the main thread when it first drew it,
and that was the costliest part of a tile's frame. The tile worker now packs a tile's terrain, roads,
rails and buildings (`packPrepared`), and the water too, keeping the arrays the water's refinement reads
(`packAlongside`). The look worker packs what it draws, and the grass writes its blades straight in the
renderer's layout (`bladeGeometry`). The arena copies packed bytes as they are, and does not zero what it
is about to overwrite.

**More of a tile is made in the tile worker** (`prepareTile`): the shore, the stony ground and bare rock
(`stonesPixels`, `render/groundmaps.ts`), the fences, walls and power lines (`lineBuffers`), where the
gates are (`gatesOfTile`, `render/gatespots.ts`), and every packed mesh's bounds. `GateView.attach` had
worked out every house's openings and rooms again, though the worker had them.

**Falling sections and heaps are cut in the look worker** (`render/lookjobs.ts`). `lookChunk` sliced a
building's whole facade for each section that lost its hold, on the main thread, as the shell landed.

- The section goes to the worker with the redraw that shows its hole, and falls when that comes back,
  a frame or two later. Its velocity, spin and lean are drawn when it breaks away. Before, it fell at
  once while the wall it came from was still drawn whole until the worker's redraw came back.
- The heap is made there from the cells gone and a grid of the ground round the building
  (`heapRubble`), with the mounds of squares that did not change kept as before.
- The worker sends each section's few outermost corners and its slab's normal (`render/reach.ts`): a
  falling piece is measured every frame (`span`, `toppleOf`), and a section of wall has thousands of
  vertices.
- Without a worker, or if it fails, the same functions run on the main thread.

**A house's faces are kept between cuts** (`shared::facade::Standing`), on the server and in the
client's prediction alike. Cutting a house's faces round its holes was nearly all of its collider's
cost, and it was done for every face at every cut. What stands of a face depends only on the cells
within two of it. So a cut works out again only the faces near a cell that went or came back, and gives
the same triangles in the same order as cutting them all afresh. The server lets a building's faces go
a minute after its last cut. SERVER-PERFORMANCE.md has the server's numbers.

**Smaller things**:

- `refresh` steps the physics once, not twice. A tile's replayed damage refreshes once (`SimMotor.batch`).
- A carved building's collider is no longer left behind when its tile streams out. It became an
  invisible wall when the tile came back.
- A round that lands finds the cell it struck in JavaScript, with no copy of the grid into WebAssembly
  (`Shell.nearestCell`), and each building's footprint box is worked out once.
- The smoke over a hurt house rises from its cells, so its cube faces are no longer worked out on the
  first hit.
- `Shell.clusters` allocates nothing per neighbour. Grass chunks are built for about 1.5 ms a frame,
  not three a frame.

## A long fight: craters and crowds

After a quarter of an hour of shelling on `home` with the police out in full, the clients' frame rate
fell while the server kept up. `--scenario chaos` stands in for the end of such a session, as headless
Chrome has no server: it digs `--craters` within `--reach` metres of the player through
`WorldView.cratered`, feeds `--people` on foot (one in six dead) and `--cars` with their crews to the
entity store as the scenes snapshots rebuild, 30 a second, and walks the player in a circle while
another crater lands every `--every` seconds. Decoding a snapshot, and what `onSnapshot` gives the
prediction's physics, are not part of it.

Frame time on a desktop, 30 s on `home`, milliseconds (p50 / p95, and frames over 16.7 ms of some 1,790):

| Run | Before | After |
|---|---|---|
| 2,075 craters, nobody about | 1.5 / 17.6, 93 over | 1.3 / 2.6, none over |
| 150 on foot and 40 cars (205 soldiers drawn), no craters | 16.0 / 18.8, 523 over | 6.7 / 8.9, 12 over |
| Both at once | not run | 6.8 / 8.7, 2 over |

The longest frames still over are the first, where 205 soldiers are made at once.

- **Craters are found by place** (`render/cratergrid.ts`). The grass, the wheat, its lid, the stones,
  the forest floor and the gardens each kept a list of every crater, and asked all of it about every
  blade, stone and plant they laid: a grass chunk is about 3,000 blades, craters only heal with nobody
  within 400 m, and an autocannon digs one a round. Each layer now keeps a `CraterGrid`, which lists a
  crater under the 16 m cells its bowl reaches. Over the run the grass took 4.9 s with 2,075 craters,
  and takes 0.2 s, as it does with none.
- **The animator samples only the clips that play** (`Animator.apply`). A soldier has 177 clips of
  19,116 tracks between them, and the pose looked at every track of every bone to find the handful
  with a weight: 12.2 s of the 30 with the crowd, and 1.1 s now. The actions are gone through in the
  clips' order as before, so a bone's rotations are summed in the same order and the pose is the same
  (`animator.spec.ts`).
- **A soldier in a seat skips frames by distance** as one on foot does (`Character.seat`,
  `skipsFrame`). Every driver in the traffic and every crew of the police was posed every frame however
  far off. The body still goes where the vehicle takes it every frame; getting in or out is never
  skipped.
- **Voice is offered only to people.** The eight nearest soldiers were offered a connection whoever
  they were, and a civilian, an officer, a zombie or a bot never answers, so each was closed after 12 s
  and opened again.

- **A soldier keeps only the body it wears in the scene** (`Character.dress`). The model holds thirteen
  bodies, one for each look, and the twelve not worn were hidden but still placed every frame with the
  rest of it. They are taken out of the model and put back when the look changes. The scene's
  `updateWorld` took 2.8 s of the 30 with the crowd, and takes 2.0 s.
- **The blend goes through the clips that count** (`Character.playing`). Each pose went through the
  weights and targets of all 177 clips, in three maps, to move the few with a weight. The clips with a
  weight or a target are kept in a set of their own, and one that has faded below a millionth with
  nothing asking for it is let go. `Character.update` took 2.0 s, and takes 0.5 s.

What the crowd costs now, of the 30 s: the scene's `updateWorld` 2.0 s (a soldier is still some 50
nodes, its bones and the guns it does not hold among them, and all are worked out again as it moves),
the animator 1.7 s, the bones' matrices for the GPU 0.8 s (`BoneTable.sync`, once for each of a body's
two or three meshes).

Looked at and left:

- **Advancing the animator's actions over a plain array** in place of the map's values measured no
  faster, and was taken out again.
- **The point lights' list** was thought to fill (256) with beacons and headlamps held at any distance.
  The most held in the run was 55.
- **The ids of civilians and crews kept after they go** (`Soldiers.civilians`, the names in `profile.ts`,
  `Bodies.drippedAt`) are a number each and cost nothing a frame.

## Not done, and why

- **Spreading a tile's attach over frames.** What is left of it is a few milliseconds, and staging it
  would let messages arrive for a tile half attached.
- **An upload budget in the renderer.** Once the vertices come packed, a 12 MB upload costs the frame 2
  to 3 ms, so it was not needed.
- **Drawing the debris instanced.** Not measured to matter; up to 96 pieces fall per building.
- **Redrawing a ruined block's look per section.** The redraw is off the main thread, taking 30 to 60 ms
  here. Only how late the hole shows depends on it.

## Open

- **A crowd's soldiers** still cost about 30 µs each a frame. What is left is spread out: the scene
  walk over the bones, the animator, and the bones' matrices worked out once for each mesh of a body.
- **The server sets no limit on civilians on foot**, and sends everything within 300 m thirty times a
  second however far. Either would thin a crowd at its source, and both change the game.
- **Play-test on the laptop the game is played on.** These numbers are a desktop's.
- **The prediction's collider is the biggest cost of a cut left** (2 ms here, in WebAssembly). Most of
  it is now the trimesh's BVH and the broad phase taking the new collider. One collider per section of a
  building would rebuild less, at the price of a building being several objects to the physics.
- **The grass** is the most a frame spends when moving fast through meadows, about 4 ms at worst: it is
  built on the main thread from the ground's heights.
