# Churches

A church is long, tall and steep, and fitted and furnished as a house it looks wrong: the ridge off
to one side, a roof flattened to a house's pitch, and row upon row of small house windows up its
walls. A building mapped as a church is therefore built by rules of its own: its roof by
`crates/osm-import/src/church.rs`, its windows and door by `shared::windows` (mirrored in
`client/src/render/windows.ts`), its inside by `shared::interior`, and its colours by the client.

Backens kyrka in the home world is the church these were written against: a nave 44.8 × 15.3 m
with a sacristy 7.3 × 7.0 m against its north wall.

## What is a church

`assemble::kind_of` reads the OpenStreetMap tags: `building=church`, `chapel` or `cathedral`, or
`amenity=place_of_worship` with `religion=christian`. The result is `Building::kind`
(`BUILDING_CHURCH`, else `BUILDING_PLAIN`), a byte in the tile after `roof_kind`
(`WORLD_VERSION` 20). Everything below hangs on it. A church is never held to a house's height
(`cap`).

## The roof

The general fit (`roof::fit`) lays its gable down the middle of the smallest rectangle round the
whole footprint. Round Backens kyrka that rectangle is 22.4 m wide, sacristy and all, so the ridge
came out 3.5 m north of the nave's own; and no gable may slope over 50°, where this roof slopes 58°.

`church::fit` instead:

1. Cuts the squared footprint into its rectangles, the biggest first (`catalogue::rectangles`, the
   same search the house catalogue uses, without turning or carrying in the side wings). A footprint
   that is not square is one rectangle, if it fills 85% of the rectangle round it.
2. Takes the biggest as the **nave**. It must be at least 1.5 times as long as wide and 60% of the
   footprint.
3. Fits one gable to the returns over the nave alone, its ridge down the nave's own middle: the
   eaves height and the slope, by the same re-weighted least squares as every other roof. The slope
   may be 31° to 70°. The returns are first moved back under a footprint drawn up to a metre off
   (`catalogue::misplaced`).
4. Drops the returns standing more than a metre over that gable (a tree, a spirelet) and fits again.
   If they are more than 15% of the nave's returns, something big stands on it, a tower, and no gable
   will do.
5. Cuts off what lies beyond each side and each end of the nave with a `Roof::Cut` along that wall,
   and fits its returns by themselves (`roof::fit_over`: flat, shed, gable or hip, as steep as a
   nave may be). The returns within a metre of the nave's wall are left out: they are the nave's
   eaves and wall. One whose returns stand over the nave's ridge is a tower.
6. An annexe that fits flat while its returns span more than 2 m is a steep little roof the few
   returns on it could not show. It gets a hip from its eaves (the returns' lowest 15%), as steep as
   the nave or as its returns reach. One with fewer than ten returns is flat.

Backens kyrka, from 611 returns: the nave's eaves 7.4 m over its floor and its ridge 19.6 m, a slope
of 1.59 (58°), missing its returns by 9 cm; the sacristy, from the few returns on it, a hip from 3.4 m up to
8.9 m. The home world's other church is too big and too sparsely seen, and is left as it was.

A church with a tower, a nave that is no long rectangle, or a gable that misses the returns by too
much (`MOST_MISS`) gets `None` and is left to the general fit, as before. The importer says how each
church came out (`church at [x, z] (n returns): ...`) and counts those fitted as churches.

## Windows and the door

`openings_of(look, range, kind)` works from the walls alone, as for a house. The pieces of wall
are first gathered into walls: pieces facing the same way within about a degree whose corners lie
within 6 cm of one plane are one wall. (They used to be matched by rounding, which split a gable
end, built in two halves either side of the ridge, wherever its corners rounded a centimetre apart;
that holds for houses too.)

- The church's **long walls** run the way its longest wall does. Each one at least 8 m long has one
  row of tall windows 1.6 m wide, one about every 6 m, from 2.2 m up to 1.2 m under the wall's top,
  6 m tall at most (lower walls scale the sill and the gap down). Their heads are round
  (`HEAD_ARCH`).
- The two widest walls across that are its **ends**. Each has a round window 2.2 m across
  (`HEAD_ROUND`) over its middle, 35% of the way up to the ridge and at least a metre over the
  door. The **door**, 2.0 × 3.2 m, is in the middle of the front end: the end further from the
  lesser walls, so away from a sacristy, which stands by the chancel.
- The lesser walls (an annexe's) keep a house's windows, in one row, and no door. The piece of the
  nave's wall that stands over an annexe's roof is blank.
- A chapel too small for the door to fit in either end is given windows as a house is.

The hole cut in the wall, in the collider and in the glass, is the opening's rectangle whatever its
head, so `facade`, `crossed` and `glass.ts` know nothing of arches. The client draws the round head
(`openingQuads`): the wall is filled in from the curve out to the rectangle, a hair in front of the
glass and in the wall's colour and pattern, with edges back to the wall, and the frame follows the
curve. A round that passes
through a filled-in corner is not stopped.

`windows-parity.spec.ts` holds the two sides to the same windows, a church among its houses.

## Inside, and colours

`interior::rooms(.., kind)` gives a church its ground floor and no upper floors or stairs: one hall,
open to the roof. Its door stands open, as on any building with an inside.

A church the importer has no colours for (`paint`) is whitewashed render under a copper roof gone dark green, drawn as sheet metal
(`CHURCH_COLORS` in `buildingmesh.ts`).

## Not done

- Towers and spires: such a church is fitted as any big building, and still gets a church's windows,
  on whatever walls that fit leaves it.
- The brick laid in the top of a gable end, pointed windows, a porch before the door.
- Wooden churches are white and dark all the same, and other faiths' buildings are plain buildings.
