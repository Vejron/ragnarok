# A rough sea of Gerstner waves

Open water used to be a height field: five swells, each a cosine sharpened into a peak
(`((1 + cos θ) / 2)^8`), about as steep as a gentle swell (`k·A` 0.03, crests under a metre). The water
only rose and fell. Now the swells are Gerstner waves and the sea is rough: they are shorter (70 to 6 m,
where they were 90 to 7.5 m), the long ones about as steep as a wind sea gets before it breaks, and their
crests pointed.

## What it looks like

- **The water goes round.** A point of the surface at rest at `p0` is carried across and up:
  `p = p0 − Σ Q·A·d·sin θ`, `y = Σ A·cos θ`, with `θ = k·d·p0 − ω·t + phase`. It moves forwards with
  the wave on a crest and back in a trough, so the water gathers under the crests, which stand narrow
  and sharp, and spreads over the troughs, which lie broad and flat.
- **The sea heaves.** The two long swells (70 and 45 m, 1.1 and 0.75 m high) are as steep as `k·A`
  0.1. The three short ones (20, 10 and 6 m) are half as steep, so a lake has a chop and the sea a
  heavy swell. On the open sea the surface is about 4 m from crest to trough at the most.
- **The crests break white.** Where the waves squeeze the surface into a crest, the shader lays whitecaps
  (`CAP_SQUEEZE` in `render/water.ts`: whole where the surface covers under 0.58 of its area at rest,
  none over 0.7). That is the sharpest few per cent of the sea, and hardly anything on a lake.
- **Boats and swimmers go with it.** The water's swing (`Wave::orbit`) is added to the current that
  carries a hull (`water_under`) and a swimmer (`Physics::swim`), so they surge to and fro as the crests
  pass, and drift slowly along with the swell (Stokes drift: a ship left alone goes about 0.2 m/s). An
  anchored hull holds where it is, as in a current.

## How it works

- **`shared::waves`.** `Swell` has a `steep`: how far its water swings across against how high it
  rises (1 is a real wave's round orbit). They are 2.0, 2.0, 1.8, 1.5 and 1.2, twice a real
  wave's on the long swells so the crests stand pointed. Together the swells swing less than would fold
  the surface over (`Σ Q·k·A` about 0.6): the surface squeezes to no less than about 0.46 of itself.
  Each runs about as fast as a real wave of its length, a little slower for the long ones.
  - `particle(x0, z0, …)` is where the water at rest at a point has gone: its shift, its height and
    how squeezed the surface is there. The renderer moves each vertex so.
  - `wave(x, z, …)` is the surface over a fixed point, which the sim asks for. The water standing
    there has come from elsewhere, so it is found first: three Newton steps on `p0 + shift(p0) = (x, z)`
    with the surface's stretch `∂p/∂p0` (a symmetric 2 × 2). Its slope is the stretch's inverse times
    the height's change with `p0`, and its rise is the water's own rise less what the swing carries
    away under the slope.
  - A Gerstner surface spends more of its width in the troughs than on the crests, so its height over a
    point averages `Q·k·A²/2` under its level. That is added back so the water keeps its level.
  - The height and the swing both scale with the share of the waves the depth leaves (`shore`), which is
    now an argument of `wave` and `particle`, so the water's edge stays put at the shore.
  - `FETCH` is now 2 to 8 wavelengths (it was 1 to 4), so a lake does not take on the long swells.
- **The sim.** `Waters::surface` passes on the swing as `Surface::sway`. Every float point costs four
  evaluations of the swells instead of one: a few hundred trigonometric calls a hull a tick.
- **The client.** `sim/waves.ts` mirrors `wave` and `particle`. `WAVES_WGSL`'s `gerstner()` is
  `particle` with the slope and squeeze. `displace` moves each vertex by it and lifts it by the wakes
  where it has gone. The fragment takes the waves' slope from `gerstner()` at the point the water
  rested (handed down in `vertexExtra[0].yz`), so the normals match the moved surface. The shader needs no
  solve.
  - The finer mesh about the camera (`WaterRefine`) and the tiles' own surface split the water at the
    refined square's edge by where the water rested, not where it has gone, in the fragment's `discard`
    and in the wakes' fade. Going by the moved position, both meshes dropped the water the swell had
    carried across the edge, which opened a gap as wide as the swing (up to about 3 m), with the
    green seabed showing through.

## Cost

Measured with `gpu-bench.mjs` over open sea (Holmön, `--at 'sea=-628,-384,-1.2,-0.25,6'`), the old water
and the new timed alternately: 4.7 and 4.8 ms a frame before, 3.7 and 3.7 ms after (with the first,
longer swells; the swells' lengths and swings change no work in the shader). The old shape's `pow`
in every swell and its whitecap test (on height and slope, which ran the lace noise over much of the
sea) cost more than Gerstner's few extra multiplies a vertex.

## Tests

- `shared/waves.rs`:
  - the slope and rise over a fixed point are the height's derivatives;
  - the water found over a point is where `particle` carries it, at the same height;
  - the crests stand higher than the troughs sink, and the surface never folds;
  - the water rolls forward under a crest and back in a trough;
  - the waves loop and keep the level;
  - small water has only the chop.
- `waves.spec.ts`: the client's `wave` and `particle` match the sim's, and carrying the water at rest
  under a point lands it there at the height the sim rides.
- The vehicle tests on a swell measure the bigger sea. The snowmobile's pond counts as a 400 m lake;
  head on into the open sea's swell a snowmobile flat out is swamped, and nothing tests that any more.
  The raft is compared with the CB90 on a 200 m lake's chop, as the long swell tilts any hull alike.
  The server's ship tests are about her deck and guns, and run on calm water.
