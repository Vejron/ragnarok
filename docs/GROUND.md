# Ground that looks like ground

The ground's colours were a map's: one flat, bright colour for each kind of cover. They are now what the
ground is, seen from standing height, and each varies as real ground does. Bare rock, which was never
imported, is granite slab. A residential area is gardens, not paving. The edges of natural ground curve,
wander and fray, where OpenStreetMap drew them straight.

## What is imported

- `natural=bare_rock` is `COVER_ROCK` (`cover_kind` in `crates/osm-import/src/assemble.rs`). It was not
  in the importer's read of OpenStreetMap at all, so the local extract's index keeps it from version 8
  (`pbf.rs`); index an older extract again with `--fetch-osm`. An orienteering map's rock (symbol 214
  and its cliffs) is the same cover. No bush is scattered on rock (`props.rs`); the trees there are the
  laser's own.
- `landuse=residential` is `COVER_GARDEN`: lawn. `COVER_URBAN` is what is left of it, the paving round
  shops, schools and garages, and an orienteering map's paved areas.
- `amenity=parking` drawn as an area is `COVER_ASPHALT`, a car park, whatever land use it also carries.
  One that is `parking=underground`, `multi-storey` or `rooftop`, or `location=underground`, is not
  ground and is left out. It grows no grass and is `SURFACE_ASPHALT` to tyres and boots
  (`coverSurface`). It is drawn as the roads are: `roadRibbons` (`client/src/render/roads.ts`) lays
  each car park as a sheet in the roads' own asphalt material (`Layer.area`: bands a metre deep,
  cut a metre apart along, so it keeps to the ground), over the worn paths and under the paved roads.
  The car parks and the paved roads are one surface (`Reach`): a road has no shoulder and no crumbled
  edge inside a car park, a car park none where a road leaves it or where a tile's border cuts it,
  and only its outline and its holes are dusted and broken as a road's edge is. The cover map under
  it is painted the asphalt's flat colour (`DECK_ASPHALT`), which is what shows under snow, when the
  sheet is not laid. A car park lies in a garden, a park or mown grass more often than not, so it is
  painted after them and before water and marsh (`cover_rank`), and no rock or bush is scattered on it
  (`props.rs`). The importer did not read it before, so the extract's index keeps it from version 9:
  index an older extract again with `--fetch-osm`. It came with `WORLD_VERSION` 40.

## Colours and painted variation

`COVER_COLORS` (`client/src/render/terrainmesh.ts`) has each cover's colour: a forest's floor is the
dark green and brown of moss, needles and bilberry, a meadow a duller green than a map's, sand
grey-beige, rock a cool grey. The minimap keeps its map colours. A wood is painted again by the
trees that stand in it, pine heath, spruce moss or birch wood ([FOREST-FLOOR.md](FOREST-FLOOR.md)).

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

## Natural edges

OpenStreetMap traces a wood or a heath as it does a car park: straight sides and sharp corners. Ground
left to itself has edges that curve and wander, and fray where one cover gives way to another. Three
things make them so, each at its own scale.

- **Bent outlines, over metres** (`crates/osm-import/src/edges.rs`). The outline of every cover tagged
  as ground left to itself (`edges::is_wild`: wood, scrub, heath, grassland, sand, beach, bare rock,
  wetland, meadow, forest, nature reserve) is rounded and bent by the noise the shores are
  (`shoreline::roughen`, see [SHORES.md](SHORES.md)). A wood that runs down to a lake moves with its
  shore. The bending fades out near anything man-made (`Hard::strength`): it is nothing within 1 m of
  a road's or track's surface, a wall or the outline of a field, a garden, paving or a golf course,
  and full 9 m away. So a wood stays straight along the road through it and the field it borders. A
  corner is rounded only where every outline through it runs on to the same neighbours, so a wood and
  the heath beside it, which share their edge, round and bend it alike. Points along a side the
  bending does not reach are left out, so a tile carries no more of them than it did.
- **Bare rock spreads onto rough ground.** Each point of a rock outline moves out from the rock, in
  steps of 0.5 m up to 4 m, while the ground there is steeper than 25° or a knoll 20 cm above the
  ground 2.5 m round it. The steps are averaged along the outline. Rock only grows: it is painted over
  the wood or heath beside it, so no gap opens.
- **Frayed lines, under a metre** (`client/src/render/fray.ts`). As a tile's ground is baked, each texel
  within 1.45 m of a line where one natural cover meets another (or the plain grass) may take the
  cover of a texel that far off, by a noise of where it is, of wavelengths 4, 1.5 and 0.5 m. A garden,
  a field, paving or golf never frays or spreads. The painted colours (`paintCover`), the rock and
  marsh masks the shader reads (`coverMask`) and where grass grows (`grassMask`) take the same moves,
  each at its own resolution, so the rock's joints and the blades stop where the painted edge does.
  The fraying fades out within 1.5 m of a tile's edge, since a tile has only its own covers.
- **Growth at a wood's edge** (`props::edge_scatter`). Every 3 m along a wood's outline, 7 times in 10,
  a shrub or (one in four) a young birch 2.5 to 5 m tall stands up to 2 m inside the wood or 4 m out of
  it. Nothing grows where the edge runs along something man-made. A young birch needs 4 m from the trees
  already standing, a shrub 1.5 m, so where the laser found trees at the edge, those are its growth. As
  with the other props, none is made in France or inside an orienteering map.

Fraying a tile is one pass over the texels near such lines. On a tile with 40 overlapping natural
covers, at 1024 texels a side, it takes 34 ms in the tile worker. The scanline the masks share now
files each side under the rows it crosses, which took the cover raster from 177 ms to 2 ms there.

## Not done

Scree and shingle are not imported: they would be better as stony ground (`Tile::stones`) than as
slab. The forest's edge is still OpenStreetMap's, bent: where the laser has the trees, their crowns
would draw it more truly.

A car park is asphalt and nothing more: the ground under it is not levelled, the trees mapped or
measured in it stay, and no bays are painted on it. A road's mended patches follow its line, so the
aisles of a car park have them and the asphalt between has only the mottling, grit and cracks.
