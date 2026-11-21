# The forest's floor

The ground under the trees was one green with grass tufts on it. It is now the floor of the forest
that stands there: a pine heath, the moss under spruce, or a birch wood, each painted in its own
colours and grown with its own plants, with stumps and mossy stones among them.

It is all the client's, and all for show, as the grass is. Nothing in a world's files changed, the
server knows none of it, and nothing here is solid: a foot goes through a stump.

## Which forest stands where

The importer has already given every tree its species from Naturvårdsverket's layers
(`crates/osm-import/src/species.rs`), so the stand about a spot is the crowns about it. `forestOf`
(`client/src/render/forest.ts`) counts a tile's trees on a grid of 8 m cells, each by the area of its
crown, in its own cell and the eight about it, and keeps each cell's shares of three stands:

| Stand | Trees | Floor |
|---|---|---|
| `FLOOR_HEATH` | pine | dry and open: lingonberry, heather, reindeer lichen, thin hair-grass |
| `FLOOR_MOSS` | spruce | deep shade: moss, bilberry, hummocks, a few ferns, next to no grass |
| `FLOOR_LEAF` | birch and the other broadleaves | grass and ferns, last year's leaves |

Where the trees are few the shares are scaled down, and what is left is `FLOOR_MIXED`, a wood as it
was painted before. A mixed stand's floor is a patchwork of its trees' floors, and the line between
two stands is not a line: a painted texel blends the shares, and a cell of grass or a plant draws one
floor by chance against them (`floorAt`).

A tile knows only its own trees, so a cell at a tile's edge counts fewer neighbours. The shares are
of what it does count, so the floor carries on across the edge all the same.

The trees go to the tile worker as four numbers each (`woodsOf`: place, crown, kind).

## What is painted

`paintCover` (`terrainmesh.ts`) lays the stands' floors into each `COVER_WOOD` (`paintFloors`): each
stand's colour and layers (`FLOOR_COLORS`, `FLOOR_LAYERS`), as opaque as its share, smoothed between
the grid's cells. The heath has pale lichen in cushions a metre or two across, dark mats of
lingonberry and rust-brown needles; the moss has brighter drifts, dark damp hollows and bare needles;
the birch wood lusher and drier grass and leaves. Over them every tree drops its litter round its
foot (`LITTER`): a spruce's needles thick and dark out to the edge of its crown, a pine's thin and
rusty, a birch's leaves pale.

It is in the tile's cover texture, so it costs nothing a frame and is what the forest looks like
far off. Painting a wooded tile takes the worker 0.3 ms longer than it did (1100 trees).

## What grows

- **Grass** (`grass.ts`). The grass's mask has a shade for each floor (`FLOOR_SHADES`), with its
  own blade colour, and each floor keeps a share of a meadow's blades (`FLOOR_GRASS`): half in a
  mixed wood, three in ten on a heath, about one in eight in the moss, all of them under birch.
- **Dwarf shrubs, cushions and ferns** (`forestfloor.ts`, the `undergrowth` layer). 1.5 places a
  square metre are tried, and each grows what its floor's table has (`PLANTS`): bilberry, lingonberry
  or heather (a clump of leaf-shaped sprigs, heather's tips in flower), a cushion of lichen or a
  hummock of moss (a low mound), or a fern (five to seven arched fronds). Shrubs and ferns grow in
  drifts some ten metres across, each their own (`driftsAt`), with open floor between. All of it is lit
  as the ground under it and sways a little in the wind.
- **Stumps and stones** (the `deadwood` layer). 30 places are tried in every 32 m square
  (`TIMBER`): a stump at about one in eight and a stone under half a metre at one in five, most of
  each in the spruce. A stump's bark is the stand's (a birch's is white), and moss takes what faces
  up, most under spruce. Neither stands in a tree, a rock or a crater. Fallen trunks were tried and
  taken out: they added nothing to the look.

Where things stand is asked of the grass's mask (`Grass.growsAt`), so nothing grows on a road, in
water, under a house, on bare rock or in a crater, and a shell's crater clears what stood there.

In a winter world the plants are under the snow (as the grass is) and the timber carries snow on what
faces up. Turning the grass off turns the plants off; the timber stays.

## How it is drawn, and what it costs

The gardens round houses are drawn the same way, by the same `ChunkLayer` ([GARDENS.md](GARDENS.md)).

Each layer is built on the CPU in square chunks about the camera (16 m for the plants, 32 m for the
timber), one merged mesh a chunk, two chunks a frame at most. A chunk of plants takes 0.7 ms to
build and one of timber 0.2 ms. Nothing reads a chunk again, so only the GPU keeps it
(`uploadOnce`): about 4 MB of vertices in a spruce wood.

Every vertex carries its root. The vertex shader shrinks each thing into its root with distance, so
nothing pops: the plants thin as the grass does (all of them within 24 m, the last gone at 48 m), and
the timber shrinks away between 100 and 130 m. On medium and low (`FLOOR_LITE`) half the plants grow,
out to 30 m, and the timber reaches 80 m.

Everything is opaque and nothing discards, so on a tile-based GPU the hidden pixels are free. Measured
with `gpu-bench.mjs --world holmon`, the frame against the same frame with the `forestfloor` node off:

| View | Frame | Forest floor |
|---|---|---|
| Pine heath (`--at 'heath=-885,897,1.72,-0.25,1.7'`) | 2.13 ms | 0.05 ms |
| Spruce (`--at 'spruce=-648,648,1.72,-0.3,1.7'`) | 1.87 ms | 0.06 ms |
| Birch (`--at 'birch=-1392,1280,-1.2,-0.3,1.7'`) | 1.82 ms | 0.01 ms |

The grass in the same views is 0.03 to 0.05 ms, and under conifers there is now less of it.

## Not done

- The stumps and stones are not solid, and bots see through them.
- Hollows are painted, not dug: the ground's heights are the laser's.
- A thicket from an orienteering map keeps its own green.
