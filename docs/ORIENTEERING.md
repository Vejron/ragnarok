# Reading an orienteering map

What the symbols of ISOM 2017, the forest orienteering standard, mean and what `ocad/isom.rs` makes of
them. The key difference from a normal map: **the colours say how fast you can run through the terrain**,
not what grows there. Definitions are from the O-Map Wiki and ISOM 2017-2; where a density or size below
has no source it is a guess made for the game.

## Colours

| On the map | Means |
|---|---|
| White | Ordinary forest you can run through at full speed. It is not drawn: no symbol is the forest |
| Green | Slower going. The darker, the harder |
| Yellow | Open land. Darker yellow is rougher |
| Blue | Water and marsh |
| Black | Rock, paths, buildings, fences and other things made or hard |
| Brown | Landform: contours, knolls, earth banks |

Symbols combine. Marsh is an overlay: it is drawn with the vegetation on it, so green or yellow can show
under the blue lines. A white-green stripe pattern means the going is better in one direction.

## Vegetation

| Symbol | Name | Running speed | Sight | In the world |
|---|---|---|---|---|
| 405 | Forest | Full | Good | The wood base under the map |
| 406 | Slow running | 60-80% | Low | `COVER_THICKET_SLOW`, shrubs 1.7 m, one per 25 m² |
| 407 | Slow running, good visibility | 60-80% | Good | Same cover, shrubs 0.7 m, one per 18 m² |
| 408 | Walk | 20-60% | Low | `COVER_THICKET_WALK`, shrubs 1.9 m, one per 9 m² |
| 409 | Walk, good visibility | 20-60% | Good | Same cover, shrubs 0.8 m, one per 7 m² |
| 410 | Fight | Under 20% | Low | `COVER_THICKET_FIGHT`, shrubs 2.3 m, one per 3 m² |
| 401, 402 | Open land, with scattered trees | Better than forest | Open | `COVER_PARK` |
| 403, 404 | Rough open land, with scattered trees | Rough: heath, felled areas, long grass | Open | `COVER_PARK` |

Good visibility means the growth is low, brambles or heather, so it slows you but hides nobody. Shrubs
hide a soldier in the game but **do not slow one down**: terrain has no effect on movement yet. The trees
themselves come from the laser, so an open area on the map still has whatever trees stand there now.

Open land clears the laser's trees: 401, 403, 412 and sand keep none, 402 and 404 keep 15%, and 404.001
keeps 30%. A 417 prominent large tree is planted from the map, so it survives that.

## Knolls and pits

The brown dots are too small for the map's contours and mostly too small for the 1 m height model: on
Kläppudden the model shows only 13% of the knolls a metre tall, and none of the pits a metre deep, though
the standard's minimums are about that. `ocad/relief.rs` measures each one against a ring around it, moves
to the real bump if there is one within 2 m, and shapes into the ground what it lacks. The ground needs
`"cell": 1` to carry them.

| Symbol | Name | In the world |
|---|---|---|
| 109 | Small knoll, a dot | Round, radius 3 m, 1.5 m high |
| 110 | Small elongated knoll | 4 by 2 m, across the object's angle (measured on the height model), 1.3 m high |
| 111 | Small depression, a cup | Round, radius 2.5 m, 1.5 m deep; no trees in it |
| 112 | Pit, steep sides | Round, radius 2 m, 1.8 m deep; no trees in it |

## Marsh and water

| Symbol | Name | In the world |
|---|---|---|
| 307 | Uncrossable marsh | `COVER_MARSH_DEEP`: half open pools, reeds at their edges; it can still be crossed |
| 308 | Marsh, crossable, usually with a clear edge | `COVER_MARSH`; the point form is a disc 4 m in radius |
| 309 | Narrow marsh, a line | `COVER_MARSH`, a band 3 m wide |
| 310 | Indistinct marsh, seasonal or a gradual edge | `COVER_MARSH_FAINT`: damp ground, the odd pool |
| 301-306 | Lakes, streams, ditches | Ignored; water has its own sources |

The client draws the three grades from one noise field (`client/src/render/marshnoise.ts`, the same in
the terrain shader, `engine/src/materials/marsh.wgsl.ts`): still pools in its lows, wet moss and peat at
their edges, tussocks of sedge on its highs. The wetter the grade, the more of it is pool. The terrain
reads the wetness from the blue of its relief map. The grass grows sedge in tufts there, with cottongrass,
and reeds round a bog's pools. Underfoot it is mud (`SURFACE_MUD*`): dark ruts and flung clods under
tyres, and splashes in the pools on foot and on wheels. Mist hangs over it at dawn and dusk
(`render/marshmist.ts`). In winter the snow covers all of it and there is no mist.

## Rock

| Symbol | Name | In the world |
|---|---|---|
| 204 | Boulder, at least about 1 m high | One solid rock, 1.3 m |
| 205, 206 | Large and gigantic boulder | One solid rock, 2.3 m and 3.5 m |
| 207 | Boulder cluster | Four solid rocks |
| 208 | Boulder field | Three low solid rocks per symbol |
| 210 | Stony ground, slows you down | Three small rocks per symbol, solid only near roads and buildings |
| 201 | Impassable cliff | A step in the ground, 3 m over 1 m, too steep to climb, with bare rock (`COVER_ROCK`) |
| 202 | Cliff | A step, 1.8 m over 2 m, a slow scramble, with bare rock |

A cliff is ground, not an object, so it needs `"cell": 1`; at 4 m it is a ramp. `ocad/relief.rs` reads the
height model either side of the line, takes the lower side as down, and adds what the step lacks, half up
and half down, fading out over 8 m. On Kläppudden the model has a median step of 1.26 m and the built
ground 1.83 m. `crates/sim` tests that a soldier cannot get up the first shape and can get up the second.

## Roads and paths

Each becomes a road of its own width and kind, not an OSM class. Inside the map, OSM's roads give way to
these.

| Symbol | Name | In the world |
|---|---|---|
| 502 | Wide road | 6 m, asphalt (`ROAD_MINOR`) |
| 503 | Road, maintained, mostly gravel here | 4 m, `ROAD_GRAVEL` |
| 504 | Vehicle track, for slow driving | 3 m, `ROAD_TRACK`: two dirt ruts, grass between |
| 505 | Footpath | 1.6 m dirt path |
| 506 | Small path | 1.0 m dirt path |
| 507 | Less distinct small path | 0.7 m, `ROAD_FAINT`: barely worn |
| 508 | Narrow ride, a line through the forest | 0.6 m faint trace in a 3 m strip cleared of trees |

All but 502 are dirt underfoot (`surface.ts`): tyres leave marks and throw dust.

## Fences and power lines

| Symbol | Name | In the world |
|---|---|---|
| 510 | Power line, cableway or ski lift | Tarred wooden poles 9.5 m tall, at most 55 m apart, a crossarm with three wires; a 14 m lane cleared of trees |
| 511 | Major power line | Steel portal pylons 23 m tall, at most 260 m apart, three phases and two earth wires; a 40 m lane |
| 516 | Fence | Wooden posts every 3 m, three strands of wire, 1.25 m high |
| 517 | Ruined fence | Left out |
| 518 | Impassable fence | A game fence: posts every 4 m and wire netting 2 m high, its mesh fine at the foot and wider toward the top |

A fence stops a soldier, who cannot climb either kind, and bots go round it; vehicles drive through it
and bullets pass it. A pole or pylon stops everything, bullets too. The wires stop nothing. A wire is
never drawn thinner than about a pixel; further off it is drawn that wide and fainter, so it fades instead
of breaking up, and it swings a little in the breeze.

## Numbering

The numbers belong to the standard, and older maps differ: a boulder is 206 in ISOM 2000 and 204 in
ISOM 2017. `isom::check` reads the map's own name for symbol 204 and refuses a map where it is no
boulder. Decimals are a mapper's variants: 204.001 is a mid-sized boulder on the Kläppudden map. 