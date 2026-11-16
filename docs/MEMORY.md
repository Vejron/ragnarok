# Memory

What the client holds while it runs, how to measure it, and what was done to hold less. The numbers are from a town (`worlds/vindeln`, 21 tiles attached at the spawn) at quality `high`, in headless Chrome 157 on an Apple GPU with a 2268×1258 drawing buffer.

## Measuring

```sh
node client/scripts/mem-bench.mjs --label <name> [--walk] [--held-only] [--eval <js>]
```

It drives the real game as `gpu-bench.mjs` does, lets the tiles round the spawn settle, and prints:

- **The JS heap and the typed arrays' storage** after a forced collection, on the page and in each worker. Typed arrays (meshes, heights, pixels) are not on the heap, and were five sixths of what the page held.
- **Every GPU texture and buffer alive, by label.** WebGPU cannot be asked what it holds, so the script wraps `createTexture` and `createBuffer` before the game starts and adds up the sizes. A transient attachment counts as nothing.
- **The GPU process's footprint** (`footprint <pid>`), which is where the textures and buffers really are. Its resident size (`ps`) leaves most of them out.
- **Allocation while the game runs**, from the sampling heap profiler: bytes a frame, the functions that allocated them, and the collections they cost. Typed arrays' storage is not in this figure.
- With `--walk`, the same totals after going 600 m off through four places and back.

By default it serves the client from `[::1]`, which the game server does not accept, so the client plays alone in the world as its files have it. That is the same every run, but it has no other players or vehicles in it: the entity code (interpolation, the vehicle presenters) is not measured. Pass `--url http://127.0.0.1:5173/` with a server and bots running for that, and compare such runs only with each other.

## Where it was, and where it is

| | Before | After |
|---|---|---|
| Page: typed arrays | 279 MB | 53 MB |
| Page: JS heap | 25 MB | 25 MB |
| Tile worker: JS heap | 81 MB | 5 MB |
| Tile worker: typed arrays | 23 MB | 14 MB |
| GPU objects the page made | 885 MB | 531 MB |
| of which the vertex arena | 512 MB (295 MB in use) | 246 MB (218 MB in use) |
| GPU process footprint | 1636 MB | about 1155 MB |
| Allocated a frame (playing alone) | 168 KB | 94 KB |
| Minor collections in 10 s | 82, 31 ms in all | 50, 14 ms in all |

The frame did not get slower. `gpu-bench.mjs`, old and new code run alternately twice: 4.37 and 4.29 ms against 3.94 and 3.88 ms at the spawn, and 3.84 and 3.87 ms against 3.53 and 3.46 ms looking the other way. The gain is the buildings' fewer vertices.

## What a tile's buildings cost

One mesh held most of everything: a tile's merged buildings. In the town that is 3.2 million vertices over 19 tiles, 207 MB in the arena (64 bytes a vertex, and 4 an index) and another 159 MB of the same arrays on the page.

- **Welded** (`weld` in `buildingmesh.ts`). The mesh was a triangle soup, three vertices a triangle. Two fifths of them are another's twin (the two triangles of a quad, faces side by side on a wall), so equal vertices of one building are now shared and the mesh is indexed: 1.84 million vertices. They are never shared between buildings, so a building that falls is still taken out by its own range of vertices.
- **Upload once** (`Geometry.uploadOnce`). The engine keeps a geometry's arrays after uploading them, to change them in place and to send them to a new device after a lost one. A tile's buildings, terrain, road ribbons and grass chunks are never read again, so they let their arrays go once the arena has them. A fallen building used to be collapsed by rewriting its positions and sending them again; `Geometry.collapseRange` now writes over the arena's copy.

`Texture.uploadOnce` does the same for the tile's painted cover (a 1024² bitmap, closed after the upload) and the trees' impostor atlas (two 1536² images, 19 MB).

**What that costs.** A released mesh or texture cannot be sent to a new device. When the renderer is replaced (the device was lost, or the quality's multisampling changed), `Scene.onRendererReplaced` has the game put the tiles through the streamer again (`Game.redrawWorld`) and the atlas is baked again. The game holds still for the second or so that takes, as there is no ground under anything meanwhile. Before, everything was uploaded again from memory on the next frame.

## The rest

- **The arena grows by half, and by 64 MB at most** once it is big. It doubled, and so stood at 512 MB for 295 MB of vertices. It still never shrinks: what is in use moves about (93 MB a few tiles from the town), the capacity stays at its peak.
- **The tile worker keeps a tile's roads and rails only.** It kept everything the tile was made from to drape the roads again on dented ground, and with it the buildings' faces and openings worked out from that.
- **The tile file is let go once the physics has read it**, and the buildings' cells are copies, not views that kept the file alive (and were cloned whole to the worker).
- **The multisampled colour is a transient attachment** (`TRANSIENT_ATTACHMENT`, where the browser has it), as are the reflection's multisampled colour and depth: cleared, drawn and discarded within a pass, they are given no memory. The footprint fell by about 120 MB, run alternately with and without; the frame's time did not move (within 0.1 ms).
- **Maps that only grew**: the renderer's pipeline cache by material (now weak), `HullMarks.lastHp` and `SkiTracks.spray` (now forgotten with the vehicle or skier).
- **The service worker's cache rule named `minimap.png`**; the file is `minimap.webp`, so the minimap was in no cache.

## Garbage

Per frame, measured playing alone:

- **The soldier's blend** (`Character.update`) built some thirty clip names by gluing strings, a `Map` and a regular expression per clip. The names are made once and looked up; the map is kept.
- **The renderer** built a string key for every textured mesh in sight to find its bind group, another for every material whose uniforms had moved to find its pipeline, and a new object per batch. All three are found by comparing numbers now.
- **Grass** looked over the few hundred chunks round the camera every frame. It does so when the camera has moved half a metre or the ground under a chunk changed.
- **The arena** made a new buffer for every upload. Uploads of up to 1 MB (a grass chunk, a few changed quads) share one.
- **Interpolation** made a wrapper and a velocity pair for every entity every frame; each track keeps its own. Not measured: there are no entities when playing alone.

## Open

- **`Character.update` (21 KB a frame) and `considerLevels` (17 KB)** are the largest that are left, and neither shows what it allocates by reading it: probably numbers boxed in code the engine has not optimised. Find out with a CPU profile before changing them.
- **The entity paths** (`blend` makes a new entity for each one between snapshots, the vehicle presenters, the snapshot decode) need a run against a server with bots.
- **The shadow map** is 64 MB as `depth32float` at 4096². `depth16unorm` would halve it; its precision over the shadow's reach is not checked.
- **The 34 tile textures** (87 MB) are uncompressed RGBA8 with mipmaps. A cover map at fewer texels a metre, or a compressed format, changes the picture.
- **The vertex layout** is 64 bytes for every mesh though a building uses 48 and terrain 32. One layout is what lets every mesh share one buffer.
- **The window glass** is still a soup (28% of its vertices are unique), and is rewritten in place when a pane breaks.
- **The reflection's targets** (about 14 MB resolved, the rest transient now) stay after leaving the water.
- **A carve-only WASM for the tile worker**, as TILE-STREAMING.md has it.
