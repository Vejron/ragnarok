# Shaping the ground to what stands on it

The ground comes from Lantmäteriet's 1 m height model (France: IGN's MNT), which is the laser's
ground, filtered and smoothed. Under a road that leaves a lumpy crown and soft, smeared cuts. A
stream through a forest is a shallow smear the mesh barely shows, and a house stands on whatever
pits and humps the filter left where its walls were taken out. The importer therefore shapes the
model to what OpenStreetMap says stands on it before it samples the ground
(`crates/osm-import/src/condition.rs`). It aims for ground that looks built, not for a truer
measurement.

## What is shaped

The model is read onto a 1 m raster, only in 32 m blocks some feature reaches, and four shapers run
over it in this order:

1. **Building pads.** The ground inside a building's outline is pulled to its low quartile, by up to
   `PAD_MAX` (1 m), so a house on a slope sits into it rather than on a plinth, and a hillside is not
   dug out for a shed. The pull eases off over a 2 m apron outside the walls.
2. **Streams and ditches.** Every OSM stream line (the ones `water.rs` draws) gets a bed:
   - The bed follows the lowest ground within 1.5 m of the line, so a line drawn a little off the
     ditch still finds it.
   - That ground is binned every 10 m and fitted with the closest profile that never rises
     downstream (`water::isotonic`), so a hump or a road embankment doesn't dam it.
   - A bed as wide as the water drawn in it (2.5 m) is cut 0.5 m below that profile, with banks of
     1 in 1, so the water lies flat in its channel. No cell is cut more than 1 m below the model.

   Cutting only ever lowers the ground.
3. **Roads and rails.** For each way's long profile, the model is sampled every 2 m along the centre
   line, then taken through a median and a mean over the way's `Grade`: ±8 m for roads, ±5 m for
   paths and tracks, and ±25 m for rails. The profile is kept within the grade's `shift` (1.5 m,
   0.5 m and 2 m) of the model:
   - The carriageway is held to that level across its width, with a 2 % camber.
   - Beside it, the ground may differ from the edge by no more than a bank of 1 in 2. A road cut or
     an embankment therefore comes out as a clean bank, while a roadside ditch within that bank stays.

   Narrow ways run first, so the wider way wins where two meet. A road over a stream fills its
   channel back in, as over a culvert, and the drawn stream already stops at the road
   (`assemble::culverted`). Bridges, tunnels and ways with a negative `layer` are left alone.
4. **Underpasses.** A footway, cycleway or road in a short tunnel (`tunnel=yes` or a negative
   `layer`, no more than 60 m from portal to portal once the pieces OSM cuts it into are joined)
   that a road crosses over is dug out last. The height model has such a passage far too shallow,
   or not at all: the laser took the road's embankment for the ground, and grading the road filled
   in what dip there was. So:
   - The road's level over it is the lower of the highest ground along the road within 12 m either
     side of the crossing, which bridges whatever dip the model has under it.
   - The floor runs straight from portal to portal, but never higher than the headroom (3 m for a
     path, 4.7 m for a road) and the deck's thickness (0.7 m) under the lowest road over it.
   - The ways leading in at each portal come down to the floor at a 6 % grade, for up to 60 m.
   - The floor is the way's width and half a metre either side, with banks of 1 in 1 beside it.
     Nothing is dug more than 5 m below the model, and the ground is only ever lowered.

   The road over it then gets its deck from `assemble::cuttings`, which finds the dug passage as a
   trench across the road. With the headroom, the deck is a slab over the passage, where a deck lower
   over the ground would have been built solid down to it and closed the passage.

The result is kept as an offset from the model. Each ground sample adds the mean offset over its
own cell (`Shaping::mean`), and so do the detail slopes (`detail.rs`), so a 4 m ground and its 1 m
shading are shaped alike. Lakes, rivers and the sea are carved afterwards (`water::Surface::bed`),
as before.

## Using it

It is on for every world. `--raw-ground` builds a world from the model as it is, for a before and
after comparison. The import prints how much ground moved:

```
shaping: 3120 buildings, 85 streams, 1904 roads and rails, 18 underpasses; 912345 m² of ground moved 0.21 m on average
```

## Why not a new height model from the raw laser

Lantmäteriet's newer "Laserdata Skog" clouds are read already (`lidar.rs`), for roofs and trees.
A ground made from their class-2 returns would be newer, but it would be just as lumpy under roads.
It would also be less sure of narrow streams under trees, where few pulses reach the ground. The
roads, streams and yards were what looked wrong, so shaping to OSM comes first, and it works on
either model. A ground from the raw returns is still open, to be kept only if a side-by-side
comparison of hillshades shows it helps.
