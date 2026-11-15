# Graveyards

A cemetery or a churchyard has tombstones standing in it: in rows along its paths, or in rows of
their own where it has none. They are solid and nothing breaks them, so a graveyard is a place of
low cover. They are the tiles' `graves` layer (`Grave`, `GRAVE_*` in `crates/shared/src/world.rs`).

## Where they come from

OpenStreetMap maps a graveyard as an area and none of the stones in it. `osm::is_graveyard` reads
the two tags: `landuse=cemetery`, and `amenity=grave_yard` for the yard round a church. Both are
also mown grass (`COVER_PARK`). The importer keeps each one's rings and, once the tiles are built,
stands the stones (`graves::lay` in `crates/osm-import`, called from `from_osm`). The same world
always gets the same stones: nothing is drawn by chance but from a hash of where a stone stands.

## How a yard is laid out

`graves::layout` works on one yard, the roads and paths about it and the plots already taken.

**Along the paths.** Every road or path that runs inside the yard is followed, a point every 2.8 m.
At each point three stones stand on each side: the first 1.2 m outside the path's edge, the next
two 3.2 m further back each, all facing the path. The rows nearest the paths are stood first, so a
back row never crowds out a front one. This is used when at least 15 m of path lies inside the
yard, so a stub of a path at the gate does not count.

**In rows.** A yard without paths is filled with rows along its longest side, in pairs: the two
rows of a pair stand 2.2 m apart, back to back, and an aisle of 3.6 m lies between one pair and the
next. Stones are 2.8 m apart along a row.

A stone is stood only where all of this holds:

- it is inside the yard (holes in the area are respected) and 4 m from its edge;
- it is 0.8 m off the edge of every road and path;
- it is outside every building, 1 m from every tree's trunk, not under water, and inside the world;
- it is 2 m from every plot already taken, in this yard or another. A cemetery is often mapped
  twice, or holds a churchyard, and the plots are shared so it gets its stones once.

About one plot in six is left empty. A yard gets at most 2000 stones. The rocks the importer
scatters over every park (`props::scatter`) are taken away again inside a yard.

## The stones

`GRAVE_KINDS` is the one table of what each kind is; the client reads it through
`sim/tables.generated.json`.

| Kind | Share | Width | Thickness | Height |
|---|---|---|---|---|
| `GRAVE_HEADSTONE`, an upright slab with a rounded top | 55 % | 0.6 m | 0.16 m | 0.95 m |
| `GRAVE_CROSS`, a stone cross on a plinth | 20 % | 0.56 m | 0.18 m | 1.2 m |
| `GRAVE_LOW`, a low slab leaning back on its base | 25 % | 0.84 m | 0.32 m | 0.58 m |

A `Grave` is its position and the ground under it (centimetres), its heading (`yaw`, milliradians:
its face looks along +Z at 0 and turns towards +X) and its kind. The heading is the way to its path,
or along its row, turned up to 0.06 rad either way so the rows are not ruled.

## In play

| Where | What |
|---|---|
| `crates/sim` | `grave_collider`: the box of the kind, turned to its heading and sunk 0.3 m so it stands on a slope. It is no `Obj`, like a rock: it stops soldiers, every vehicle and bullets, and nothing damages it. `Physics::add_tile` builds the same boxes for the client's prediction |
| Server | Nothing of its own. Bots go round the stones because the navigation grid is made from the static colliders, and every kind stands higher than the 0.5 m a soldier steps over |
| Client | `render/graves.ts`: each kind is one shape made once to the size of its collider, and a tile draws its stones as one instanced mesh a kind, each culled on its own. A stone is tinted as grey, dark or red granite or pale limestone by a hash of its position. The material is the plain toon one, opaque, with snow in winter |

The minimap does not draw them: a graveyard shows there as the park it is.

## The offline world

`synthetic_city` makes its first two parks graveyards, one with a path across it and one without,
so `fixtures/world` has both layouts and every kind of stone.
