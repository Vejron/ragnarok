# Courtyards

A building round a yard that is open to the sky: a block of flats round its court, a school round
its playground, a cloister. Before `WORLD_VERSION` 46 such a building was either missing from the
world or roofed over, yard and all.

## In play

- The yard is ground like any other: grass grows in it, trees stand in it, it rains in it and a
  round falls into it. A player may choose to spawn there.
- Walls stand round the yard as round the outside, with rows of windows, and come down the same way.
- No door opens into a yard: a yard has no way out but back through the building. The front door
  is on an outer wall, as before.
- Inside, the floors run round the yard and the stairs keep out of it.

## How OpenStreetMap maps one

A building with a yard is a multipolygon relation tagged `building=*`: its `outer` member ways make
the outline and its `inner` ones the yards. The importer now reads these (`osm::is_building_area`):

- Overpass is asked for `relation[type=multipolygon]["building"]`, and the local extract's index
  keeps them (`pbf::VERSION` 11: index an older extract again with `--fetch-osm sweden`).
- `Data::main` joins the members end to end into rings (`assemble::join_rings`) and hands
  `assemble` each outline as a way with the relation's tags (its id has `osm::FROM_RELATION` set, so
  it is never another way's) and its yards in `Osm::yards`. A relation of several outlines is
  several buildings, each with the yards that lie in it.
- An outline way that is tagged `building` itself is the same building: it is read once, from the
  relation. An inner way tagged as a building (a glazed court, a pavilion in the yard) is a
  building of its own, in the yard.
- Only relations that carry the `building` tag themselves are read. The old way of mapping, tags on
  the outer way and none on the relation, was cleaned out of OpenStreetMap in 2017.

## In the world file

`Building::holes` (`crates/shared/src/world.rs`): each yard a ring like `footprint`, centimetres,
without the repeated first point, but clockwise seen from above, so that walking any ring of a
building keeps what is built on the same side. Empty for most buildings. A yard under 4 m² is built
over (`assemble::MIN_FOOTPRINT`).

Nothing else in the file changed. A building's walls, roof and floor are triangles, its cells are
cut from those and its colliders made from either, so all of them follow from the mesh:

- **Mesh** (`roof::build_with`, `field::build`): the footprint is cut into triangles round its
  yards (`roof::triangulate`, earcut with holes). A side of a roof piece no other piece shares is an
  outer wall, and the sides along a yard are such sides: they get walls, wound as the outer walls
  are, so they face into the yard.
- **Roof fit** (`assemble::fit_roofs`): the laser's returns in a yard are its trees and its ground,
  and are left out. A block round a yard is fitted in parts (`roof::fit`), never from the catalogue
  of house roofs or as a church. Its outline is not squared, since outline and yards would be
  squared apart, and in France it is not cut on the lattice.
- **Trees, spawns, tombstones** (`assemble::Plan`): "in a building" is inside its outline and in
  none of its yards, and the distance to its walls counts the yards' walls. The canopy the laser
  finds trees in is masked over the roof only (`Canopy::mask`).
- **Minimap**: the yard shows as the ground it is (the fill is even-odd).
- **Hit points** (`Building::hp`): from the built area, without the yards.

## Doors and floors

Both are worked out from the look by the server and, with the same code or its mirror, by the
client, so both needed to be told of the yards:

- **Doors** (`windows::openings_round`, mirrored in `render/windows.ts`): a wall whose middle lies
  on a side of a yard is that yard's wall. Such walls come after the outer walls in the order the
  doors and windows are given out by, are never the front, and never get a door. They get windows.
- **Floors** (`interior::rooms`): each yard is let into the footprint by a cut (`with_yards`): one
  ring that runs round the outline, in along the cut, round the yard the other way and back out.
  Clipping its ears leaves the yard open. The cut is the shortest line from a corner of the yard to
  a corner of the ring that crosses no wall, found in whole centimetres, so the server and the
  WebAssembly build get the same floor. The stairwell is only put where no yard is.
- **The client's own floor** for a building without rooms (`facade.ts floorOf`) is made from the
  walls' bottom edges, chained into rings: a ring that runs the other way from the outline is a
  yard, and is let into the outline the same way (`geom.ts joinHoles`).
- The yards cross into the WebAssembly build as flat numbers, for each yard its corner count and
  then its corners (`yardsOf` in `world.ts`, `yards_of` in `crates/sim-wasm`).

## What is left out, and counted

The importer prints, with its other totals, how many buildings have a yard and how many mapped
buildings are not in the world, by why (`assemble::LeftOut`):

- a corner within 2 m of the world's edge or past it (a building is never clipped);
- under 4 m² of ground;
- at the end of a ski lift, where the lift's own station stands;
- in a tile the world does not keep;

and how many footprints could not be cut into triangles (`roof::UNCUT`): those are in the world
with no walls, and a number other than 0 is a footprint to look at. Umeå, Vindeln and Holmön have
none.

## A wall that went missing

Found on the way, in a school in Umeå with 50 corners: one of its walls was not there, and you
could see in. A corner of its footprint lay a millimetre off the line between two others, earcut
made a needle of a triangle of the three, and `roof_pieces`, which adds to each piece's sides the
corners of its neighbours that lie on them, added the needle's own third corner to its opposite
side. The piece then ran out along a wall and back, the wall counted as shared between two pieces,
and no wall was built. Now:

- a piece narrower than the snapping distance (`roof::SNAP`, 0.5 mm) is left out and its neighbour
  takes the corner;
- a piece is never given a corner it has, and corners are added until no piece gains one, since a
  side made by one corner put in may have the next lying on it (the school's east wall has three
  corners almost on one line);
- a side whose reverse is in the same piece is an outer side;
- and whatever the pieces come to, every side of the footprint and of its yards is checked for an
  outer side of a piece along all of it, and a wall is stood on any stretch without one, up to the
  roof over it. The importer prints how many (`roof::BARED`): 26 stretches in Umeå's 7 735
  buildings, none in Vindeln or Holmön. A number there is a roof whose pieces did not come out
  right, with its wall closed all the same.

`the_school_is_walled_all_round_under_the_roof_the_laser_gave_it` keeps the school, with the
corners and the roof the importer had for it.

## Limits

- A yard cannot be walked into from the street. OpenStreetMap maps a gateway through a building as
  a `tunnel=building_passage` way: its road is drawn, but no wall is opened for it yet.
- In France a block with a yard is one building however big it is, where one without is cut into
  32 m pieces; the biggest take longer to check when they are hit ([FRANCE.md](FRANCE.md)).
- The ground in a yard is levelled with the building's pad only as far as any ground under a
  building is, and the floor is at the lowest ground under the outline: in a yard on higher ground
  the grass stands against the lowest row of windows.
- The synthetic test world has no building with a yard.
