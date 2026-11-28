# Load time

How long a fresh page takes to show the game, where that time goes, and what was done about it. A
cold start is a first visit: nothing cached, no service worker.

## Measuring

```sh
node client/scripts/load-bench.mjs [--world umea] [--runs 5] [--down 20 --latency 40]
```

The bench builds the client as a deploy does (the release WASM, `vite build`), serves it over HTTP/2
with brotli as the game's server does, and loads it in headless Chrome with an empty cache, a number
of times. The game leaves a `load.<stage>` measure in the page's performance timeline for each stage
of `Game.start` (`client/src/loading.ts`); the bench prints when each began and ended, as medians,
and what was downloaded before the first frame. `first-frame` is the end of the first frame's
callback, and `shown` is when the GPU has finished what that frame sent it.

`--down` and `--latency` hold the page to a slower line with Chrome's network emulation. A worker's
own requests are not held back, so a throttled run compares builds; it does not predict a real line.
`--shot <png> --shot-at <ms>` saves a picture of the page that long into the last run.

Measure before and after a change, in one sitting, and keep the change only if it is faster.

## What a cold start was

Everything before the first frame ran one step after another: the GPU device, the manifest, the
physics' WebAssembly, every model in the game, the tiles within 450 m, and only then the first frame.
On the umea world (medians, ms since the page was asked for):

| | Local, unthrottled | 20 Mbit/s, 40 ms |
|---|---|---|
| The script runs | 91 | 350 |
| Renderer, manifest, physics | 133 | 802 |
| All 40 models | 186 | 1764 |
| The 16 tiles in range | 1846 | 3646 |
| **First frame** | **1992** | **3786** |

- **The tiles were most of it, and not as a download.** One worker prepares the tiles one after
  another, about 100 ms each in a town, so 16 tiles took 1.7 s however fast the line was.
- **The first tile with trees baked their impostor atlas on the main thread**, 520 ms, inside those
  1.7 s.
- **The models were half of the bytes** (2.2 MB of 4.2 MB brotli), and all but the soldier's are only
  seen once the server has sent a vehicle or an animal.
- **`player.glb` was 5.8 MB**, half of it a 2.9 MB JSON header: Blender wrote a translation, a
  rotation and a scale track for every bone in each of its 177 clips, 19,116 tracks.

## What was done

| | Local, unthrottled | 20 Mbit/s, 40 ms |
|---|---|---|
| First frame, before | 1992 | 3786 |
| The start in parallel | 2048 | 3485 |
| Tiles fetched early, the near ones awaited, the atlas baked in a worker | 809 | 1990 |
| Other models after what the first frame needs | 795 | 1549 |
| `player.glb` pruned | 798 | 1427 |

- **The start runs in parallel** (`Game.start`). The GPU device, the manifest, the physics'
  WebAssembly and the soldier's model are asked for at once, and each is awaited where it is first
  needed. The tile worker is started as soon as the manifest is in, so its script and its own copy of
  the physics are ready before its first tile.
- **Tiles are fetched and prepared before they can be attached** (`TileStreamer.seek`). Preparing a
  tile needs only the manifest and the tile worker, so it starts with the manifest, while the physics
  is still arriving; attaching waits for the physics and the trees' atlas.
- **The first frame waits for the near tiles only** (`PRIME_RADIUS`, 150 m): the ground to stand on
  and what is close. The rest of the 450 m arrive over the next second or two behind the pause
  screen, attached a few a frame as they are when walking. Their buildings are made solid from the
  queue (FRAME-PERFORMANCE.md).
- **The trees' impostor atlas is baked in a worker of its own** (`render/treebake.ts`), started when
  the manifest has said the season. The first tile with trees takes the finished atlas
  (`adoptTreeAtlas`). A new graphics device still bakes it on the main thread
  (`rebakeTreeImpostors`).
- **Vehicles' and animals' models are asked for after what the first frame needs**, so on a slow line
  they do not take its share, and `connect` waits for them before it joins: everything the server
  sends is drawn with its model. The pause screen says "Loading the vehicles" meanwhile.
- **Tracks that say nothing are pruned from the models** (`tools/gltf_prune.py`, called by the build
  scripts of the soldier, the rooster and the deer). Where every clip's track of one property of one
  bone only holds the bone's rest pose, all but the first are dropped. The `Animator` tops a property
  up with its rest value wherever the playing clips do not drive it, so with one track left it writes
  the same value every frame, whatever the blend. The soldier went from 19,116 tracks to 5,564, from
  5.77 MB to 3.69 MB (1.29 MB to 1.04 MB brotli), and its JSON from 2.9 MB to 1.0 MB.
- **Models and sounds are asked for by their content's hash** (`assetUrl`, `virtual:assets` in
  `vite.config.ts`), as tiles are. The server sends anything asked for with `?h=` as `immutable`, and
  the service worker keeps it for good (CacheFirst). Before, their names carried no hash and the
  server sent them with no `Cache-Control`, so every deploy made a returning player download every
  model again. This does nothing for a first visit.

## The loading screen

Until the first frame the page shows the stages of the start as a checklist, several running at once,
and one bar for the whole (`client/src/LoadingScreen.vue`). Its first state is plain HTML in
`index.html`, so it is on the screen while the script is still arriving. `Game` reports each stage
through its `onLoad` callback (`LoadTracker`), and the terrain stage counts its tiles. The bar's
shares (`STAGE_WEIGHTS`) are each stage's share of the throttled run above. A game that will not
start says why on the same screen, with the stage that failed marked.

The two large downloads (the physics and the soldier) fill their share only when they finish: the
bar does not follow their bytes.

## What was left

- **The atlas bake is now what a fast line waits for**: 580 ms in its worker, from the manifest at
  about 115 ms to 700 ms. It could be made faster, or baked when the client is built and shipped as
  two images, which trades the wait for a download.
- **The physics' WebAssembly is compiled twice**, on the main thread and in the tile worker. The
  worker now starts early enough that its copy is ready before its first tile, so sharing the
  compiled module was not done.
- **Every pipeline is created synchronously**, the materials' on first draw. The first frame's
  callback takes about 55 ms and the GPU another 60 ms to show it, so this is small beside the rest.
- **The soldier's model is still 1.0 MB brotli**: thirteen bodies, and 3,389 tracks that hold a pose
  other than the rest pose, which cannot be dropped the same way. The four stance sets of clips
  could be built in the client from one, and the vertex data quantised.
- **Each vehicle kind's model on first sight**, rather than all of them before joining.
