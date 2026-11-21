# Gardens

A house stood on bare lawn. Now many have a flower bed along the sunny wall and a rhubarb patch out
at the back, as a Swedish garden does. How likely each is follows the kind of house: an old
farmhouse keeps the garden its grandparents planted, a villa has its beds, a cabin in the woods
little or nothing.

It is all the client's, and all for show, as the forest's floor is ([FOREST-FLOOR.md](FOREST-FLOOR.md)).
Nothing in a world's files changed, the server knows none of it, and nothing here is solid.

## Where things grow

`gardenPlan` (`client/src/render/gardenplan.ts`) plans a tile's gardens in the tile worker, with the
rest of the tile (`prepareTile`). Each house is the tile's whose middle is on it, and everything is
hashed from where it stands, so the same world grows the same gardens.

| House (`BUILDING_*`) | Bed | Rhubarb |
|---|---|---|
| `FARMHOUSE`, an old wooden house | 60% | 65% |
| `PLAIN` | 60% | 30% |
| `VILLA` | 75% | 25% |
| `CABIN` | 15% | 30% |
| `FLATS` | 30% | |

Other buildings (barns, sheds, halls, shops, churches, public buildings) have none, nor does a
building of more than 700 m². A house's `front` faces its road and has its door, so:

- **The bed** lies along the wall that faces south (+z) most squarely, but never the front wall,
  and only if the wall is 3 m long or more. It runs 4 to 12 m along the wall, 0.6 m short of each
  corner, from 0.15 to 0.9 m out. It is painted as dug soil into the ground (`paintCover`) and the
  grass is kept off it (`grassMask`). Two rows of flower clumps, one every 0.45 m, the taller row
  against the wall, in two or three of seven colours (`FLOWER_COLORS`).
- **Rhubarb**, one to three plants 3 to 6 m out from the back wall, the one furthest from the road.

A thing grows only on grass, in a garden, a park or the plain ground nothing covers: not on a road,
a track, water, a field, paving or in a wood. It keeps clear of every building and of the trees, and
a house with a grave within 25 m stands in a churchyard and has no garden.

## How it is drawn

`Gardens` (`gardens.ts`) draws the plan with the forest floor's `ChunkLayer`: 32 m chunks about the
camera, one merged mesh each, two built a frame at most. Every vertex carries its root, and the
shader shrinks each thing into it with distance, so nothing pops: the flowers thin past 30 m, and
everything is gone between 60 and 80 m. On medium and low (`GARDENS_LITE`) the flowers thin past
20 m and everything is gone by 55 m. Unlike the forest floor, the gardens cast shadows.

- A **rhubarb** plant is five to seven leaves, each a broad crumpled fan on a red stalk.
- A **flower** clump is leafy stems (the forest floor's sprigs) under small star-shaped heads.

A shell's crater takes out what grew in it. In a winter world there are no gardens and no beds:
they are under the snow.

## What it costs

Measured with `gpu-bench.mjs --world holmon`, the frame against the same frame with the `gardens`
node off:

| View | Frame | Gardens |
|---|---|---|
| A garden with a bed and rhubarb (`--at 'garden=-1097,997,0.46,-0.25,1.7'`) | 2.22 ms | 0.01 ms |
| Across the village (`--at 'village=-1060,818,0.25,-0.1,1.7'`) | 2.25 ms | 0.02 ms |

## Not done

- Lilac and currant bushes were tried and taken out.
- The yard things that were thought of with this (paths and drives, firewood stacks, a flagpole, a
  mailbox) are not made.
