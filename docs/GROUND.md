# Ground that looks like ground

The ground's colours were a map's: one flat, bright colour for each kind of cover. They are now what the
ground is, seen from standing height, and each varies as real ground does. Bare rock, which was never
imported, is granite slab. A residential area is gardens, not paving.

## What is imported

- `natural=bare_rock` is `COVER_ROCK` (`cover_kind` in `crates/osm-import/src/assemble.rs`). It was not
  in the importer's read of OpenStreetMap at all, so the local extract's index keeps it from version 8
  (`pbf.rs`); index an older extract again with `--fetch-osm`. An orienteering map's rock (symbol 214
  and its cliffs) is the same cover. No bush is scattered on rock (`props.rs`); the trees there are the
  laser's own.
- `landuse=residential` is `COVER_GARDEN`: lawn. `COVER_URBAN` is what is left of it, the paving round
  shops, schools and garages, and an orienteering map's paved areas.

## Colours and painted variation

`COVER_COLORS` (`client/src/render/terrainmesh.ts`) has each cover's colour: a forest's floor is the
dark green and brown of moss, needles and bilberry, a meadow a duller green than a map's, sand
grey-beige, rock a cool grey. The minimap keeps its map colours.

Over the flat colour `paintCover` lays each cover's layers (`COVER_LAYERS`), as it paints the tile's
cover texture in the tile worker. A layer is a small noise tile that repeats (`layerNoise`), made once
and filled into the cover's outline as a canvas pattern anchored to world metres, so a patch carries on
from one tile into the next. A layer either lightens and darkens the ground (`overlay`) or lays patches
of a tint over a share of it: drier grass in a meadow, moss and bare needles in a wood, worn patches in
a lawn, and on rock paler and darker stone, pale and dark lichen and the pink of a granite. All of it
is in the texture, so it costs nothing a frame, and it is what the ground looks like far off. The cover
texture is 4 texels a metre, so nothing painted is finer than about half a metre.

The baked tone (`toneOf` in `shade.ts`) keeps its hillshade and its darker steep ground and hollows,
which are what make the 1 m relief read on a slab. Its slow sine wobble is gone: the layers do that.

## Gardens

In a garden each house has `APRON_WIDTH` (2 m) of gravel round its walls, stroked along its footprint
and clipped to the garden. The lawn has short, even blades (`GARDEN_SHADE` in `grassmask.ts`,
`LAWN_HEIGHT` in `grass.ts`), and none on the gravel. A house's gravel stops at its tile's edge: a tile
is painted from its own buildings.

## Rock near the eye

Close up a painted texture is soft, so the terrain's shader draws the rock's fine detail
(`engine/src/materials/rock.wgsl.ts`, `MAT_ROCK`). The tile's stones texture has in its alpha how much
of the ground is rock (`stonesTexture`), and where it is the shader adds:

- **Joints.** Two sets of cracks, each running its own way a few metres apart, wandering, crossing and
  broken off in places, with earth in them and moss in some. A joint is never thinner than a pixel and
  fades as the pixel outgrows it.
- **Crusts of lichen** with sharp edges, a hand or two across.
- **Grain**, a few centimetres across, which also tips the normal, within a few metres.
- **The waterline's dark zone**, 0.25 to 1.1 m above the water, from the shore height the texture
  already has in blue. This one is drawn at any distance.

Everything but the dark zone is gone once a pixel spans 12 cm (`ROCK_FAR`), and lightens as much as it
darkens, so the rock does not change tone or shimmer with distance. Snow needs nothing new: it lies on
the flat and leaves steep rock bare.

## Cost

Measured with `gpu-bench.mjs --world holmon --at 'rock=1515,-781,-1.57,-0.6,0.3'`, the frame against
the same frame with `rock` off on the terrain's materials: 0.20 ms of a 2.36 ms frame with the screen
full of rock, and nothing in views without rock (the flag is only set on tiles that have some). The
noise is worked out in the shader, two value noises and two hashes a pixel, and a third noise within
a few metres; a first version with ten noises cost 0.46 ms. The painted layers add nothing to a frame.

## Not done

The edge between two covers is still the polygon's straight line. Scree and shingle are not imported:
they would be better as stony ground (`Tile::stones`) than as slab.
