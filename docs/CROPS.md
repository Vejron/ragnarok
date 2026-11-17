# Crops: wheat on the farmland that a soldier can hide in

Farmland used to be bare, pale ground. It now grows wheat 1.3 m high (`CROP_HEIGHT`). A crouched or
prone soldier is under it; a standing one shows head and shoulders. It hides people from other
players' eyes and from the bots, and it costs the GPU about what the grass does.

Status: built and tested. Looked at headless in `home` and `vindeln` from inside a field, from 40 to
220 m across one, and from 14 and 35 m up. Not yet played. The winter stubble is covered by a spec only: no
winter world was looked at.

## Where it grows: `Tile::crops`

The importer bakes where crops stand, so the client and the server agree on it.

- `Tile::crops` is a 2 m grid over the tile, laid out as `Tile::stones` is and written after it: one
  byte a cell, `CROP_NONE` or `CROP_WHEAT`. A tile without crops has an empty grid. `WORLD_VERSION` is 29.
- `lay_crops` (`crates/osm-import/src/assemble.rs`) sows a cell when its centre is in a field and the
  tile's cover there is still farmland once the woods, parks, marshes and water over it are painted.
  It leaves out roads, a track's ballast, streams, water deeper than 15 cm and building footprints.
- A field is `landuse=farmland`, drawn as a closed way or as a relation, or an orienteering map's
  cultivated land (ISOM 412). A farmyard, orchard, vineyard or nursery is drawn as farmland is and
  stays bare (`is_field`).
- The synthetic city has one field in place of its third park, so `fixtures/world` has wheat.
- `home` has 214 ha of wheat and `vindeln` 111 ha. The importer logs it (`crops: … ha of wheat`).

## Drawing it

### Near: stalks, by the grass system

`client/src/render/grass.ts` grows whatever its `Growth` says. `Game` makes two `Grass`: the meadows'
(`MEADOW`) and the fields' (`WHEAT`), each with its own chunks, material and density.

- A stalk is a blade twice as wide as grass (0.14 m) with a leaf to near its top and an ear over it,
  0.9 to 1.1 of `CROP_HEIGHT`. A field has no bare patches.
- Its mask is the tile's `crops` grid (`cropMask`). The meadow's mask is unchanged, so no grass grows
  in a field.
- Wind, the eight movers, blasts and craters work as for grass. The wind is the game's own, as for
  the grass and trees (`Grass.blowing`), and the lid's gusts follow it too (`CropLid.blowing`).
  Walking or driving through wheat parts it, which gives whoever is in it away.
- **The density is the same at every quality** (`WHEAT_DENSITY`: 8 stalks a square metre, all of them
  to 25 m, the last gone at 140 m), and the "grass" look does not turn it off. A thinner field on a
  lower setting would be a way to see through it.
- Past 75 m or so a stalk is one triangle (`Growth.simpleShare`), as grass is. Nearer than that the change from the full stalk shows as a patch popping in.
- In a winter world the same `Grass` grows `STUBBLE` instead: 0.1 to 0.25 m, sparse, to 45 m.

### Under it: straw

The terrain's texture paints the ground under the crops straw (`CROP_GROUND_COLOR`, `paintCover`),
not the pale earth of other farmland: the farmland's own polygon again, inside the sown cells and one
cell round them, so the straw ends at the edge the field was drawn with. What shows between the stalks is then more field, and the lid
meets the ground round it without a line. Not in winter.

### Far: a lid

The stalks stop at 140 m, and past them a soldier would stand in plain view on bare ground. So each
tile's crops also make a lid (`client/src/render/croplid.ts`): a solid slab 0.95 of `CROP_HEIGHT`
high in the ears' colour.

- Its outline follows the field's edge. Its corners are the middles of the 2 m crop cells, and it
  ends where the share of sown ground round a point crosses a half (`sownShare`), so a slanting edge
  is a line within half a metre, not the steps of its cells. A skirt goes down to the ground along
  the outline and along the tile's own edge.
- The vertex shader keeps it 0.3 m under the ground near the camera and lifts it between 77 and
  112 m (0.55 to 0.8 of the stalks' reach), while the stalks thin out.
- **From above it comes nearer.** Stalks seen from a helicopter are thin lines with the ground
  between them. Each metre the camera is more than 5 m over the ground counts as 4.5 m further off
  (`LID_EYE`, `LID_PER_HEIGHT`), so from some 30 m up the lid is up everywhere and only the ears of
  the near stalks stand out of it. On foot or in a car nothing changes.
- It is opaque and discards nothing. It is lit as the ground is, with one shadow tap. Its colour is
  measured (`LID_COLOR`): the one at which the lid, drawn over the same pixels as stalks 60 to 100 m
  off, comes out the same on the screen, from 3 m up and from 14 m. It is broken up by clumps a metre
  across (blurred out past 60 m), broader patches, and the gusts that the stalks lean with. Measure
  it again after changing the wheat's colours or the straw's.
- **It is open over craters.** A crater takes the sown share away toward its middle
  (`CropLid.dent`, from the same crater events as `Grass.dent`), so the lid has a round hole to 0.8
  of the crater's radius, where the stalks stop too, with a skirt round it. A crater narrower than
  the lid's 2 m corners can fall between them and leave no hole. Lids a barrage reaches are laid
  again once, not once a crater.
- It is laid on its own tile's ground, not the terrain's lookup: a neighbouring tile may not have
  streamed in yet, and the lid's far edge lies on it.
- A winter world has no lid.

## Hiding from the bots

The server reads the grid through `Physics::crop_at` (`crates/sim/src/cells.rs`, the lookup stony
ground already used). `senses::crop_cover` (`crates/server/src/mechanics/bots/senses.rs`) says how
much of someone in wheat is left to see:

| In wheat | Share |
|---|---|
| Standing | 0.5 |
| Crouched and moving | 0.15 |
| Crouched and still, or prone (crawling too) | 0: not seen at all |

- The share multiplies the awareness meter's other terms (BOTS.md).
- At 0 the bot does not see them even if it was watching them: it loses them as if they went behind
  a wall, and goes to where it last saw them. A tank's gunner is fooled the same way.
- Within 3 m (`CLOSE_IN`) a bot finds them anyway.
- The wheat must also stand 2 m from them toward the bot. At the field's near edge they are in plain view.
- It does not count in a vehicle, under a canopy, more than 0.5 m off the ground, or in a winter world.

Not done:

- A bot high above a field (a roof, a hill) does not see down into it.
- Gunfire from the wheat is heard, as anywhere, but does not give the shooter's place away beyond that.
- Bots do not use wheat as cover themselves.
- Wheat does not slow anyone.

## What it costs

`node client/scripts/gpu-bench.mjs --world home --views field,vista --parts crops,grass`, quality
high, 2268 × 1258, ms per frame. The `field` view stands in a field looking across it.

| view | frame | crops | grass |
|---|---|---|---|
| field | 2.73 | 0.15 | 0.02 |
| vista (no field near) | 2.70 | 0.00 | 0.02 |

Memory, standing 60 m inside a field (`mem-bench.mjs --eval`, the arena's bytes with and without):
the stalks hold 40 MB of vertices in 296 chunks (0.57 M vertices), the lids 1.5 MB, of 238 MB in all.
Away from fields they hold nothing. Most of it is the full chunks within 40 m; fewer stalks a square
metre (`WHEAT_DENSITY.perMetre`) is the lever, traded against how soon a line of sight is shut.

The bench's `grass` part matched nothing before this: no node was named `grass`. Each `Grass` now
names its root after its growth, so `grass` and `crops` can both be turned off.
