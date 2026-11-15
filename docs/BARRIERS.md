# Fences and walls

Fences and walls stand in the way and can be broken, piece by piece. They are the tiles' `lines`
layer (`Line`, `LINE_*` in `crates/shared/src/world.rs`), which also holds the power lines.

## Where they come from

| Kind | From OpenStreetMap | From an orienteering map |
|---|---|---|
| `LINE_FENCE`, a wire fence | `barrier=fence` | 516 Fence |
| `LINE_FENCE_HIGH`, a game fence | `barrier=fence` with `fence_type=game` or a `height` of 1.8 m or more | 518 Impassable fence |
| `LINE_WALL`, a low stone wall | `barrier=wall` | none yet |
| `LINE_WALL_HIGH`, a high stone wall | `barrier=city_wall`, or `barrier=wall` with a `height` of 1.8 m or more | none yet |

`osm::line_kind` reads the tags. A `barrier=retaining_wall` is left out: it holds earth back on one
side, and a wall standing free there would look wrong. Inside an orienteering map's outline OSM's
lines are cut away, so a wall there is not built until the map's wall symbols are read.

OSM maps a gate as a node, which the importer does not read. A wall is therefore opened where a road
crosses it, to the road's width and half a metre more on each side (`open_at_roads`). A fence is not:
vehicles go through it anyway.

## Pieces

The importer puts a point at every post: at most 3 m apart, 4 m for a game fence (`LineKindDef::step`,
`every` in `assemble.rs`), with the ground height under each. The stretch between two points is a
**piece**, and a piece is what breaks. `shared::world::line_pieces` numbers them: piece `segment` of
the tile's line number `line` is `ObjId { col, row, index: line << 16 | segment }`. The server, the
physics and the client all work from the tile's own lines in the tile's order, so they agree without
anything more being sent, and a re-import renumbers them as it does trees.

`LINE_KINDS` is the one table of what each kind is: its height, half its thickness, its post spacing,
its hit points and whether it is a wall. The client reads it through `sim/tables.generated.json`.

| Kind | Height | Thickness | Hit points |
|---|---|---|---|
| Wire fence | 1.25 m | 0.12 m | 5 |
| Game fence | 2.05 m | 0.12 m | 10 |
| Low wall | 1.0 m | 0.5 m | 60 |
| High wall | 2.4 m | 0.6 m | 200 |

## What they stop

Each piece is a static box collider (`sim/src/fences.rs`), from 0.4 m under the ground to its top,
turned along the piece and tilted to its slope. It is an `Obj::Line`, destroyed as a tree is.

| | Wire or game fence | Low wall | High wall |
|---|---|---|---|
| A soldier | stopped | stopped | stopped |
| Bullets, sight, the camera | pass | stopped | stopped |
| A car | goes through, breaking it, and loses 5 % of its speed | stops; breaks it from about 10 m/s, keeping 60 % of its speed | stops |
| A tank, a Strv | goes through, breaking it | ploughs through, breaking it | ploughs through, breaking it |
| A Titan's fist or foot | breaks it | breaks it | breaks it at speed |
| A blast | breaks what is near | by distance, as a house's wall | by distance |
| Rifle rounds | pass | chip it at a third of their damage | the same |
| Bots | go round | go round | go round |

**A fence is very weak.** Its collider is in a collision group of its own that touches nothing
(`fence_groups`), so no rigid body ever meets it and neither does a query that names groups
(`past_fences`): wheel rays, rounds, sight. A soldier's step and the bots' grid ask without naming
groups, and they meet it. The server breaks the pieces a moving vehicle's hull overlaps
(`plow_trees` in `mechanics/vehicles.rs`). Had a car truly collided with the fence, the client's
predicted car would have stopped dead on it until the server's correction came, as it still does on a
tree; this way the prediction is right, and all the client is corrected by is the 5 %.

**A wall is in the trees' group** (`TREE_GROUP`), so it behaves as a trunk does: light vehicles
collide with it and break it at speed (`run_down_trees`), heavy ones leave it out of what they touch
and plough it.

The bots' grid shuts every cell a piece runs within 0.75 m of, and opens again when the piece goes
(`Physics::take_changed`).

## Breaking

`damage_object` takes health off a piece; at zero its collider goes and everyone is told in the
`destroyed` message with kind `OBJ_LINE` (2), by the same road as a felled tree, catch-up for a late
joiner included ([NETWORKING.md](NETWORKING.md)). Nothing new is on the wire but that kind.

The client keeps the broken pieces by tile (`WorldView.brokenLines`), takes them out of its own
physics (`SimMotor.breakLine`), and draws the tile's lines again without them (`rebuildLines`): the
posts, wires, netting and walls of a tile are three merged meshes, so one piece less is those three
built anew. A post stands while a piece on either side of it does. A live break throws splinters or
lumps of stone and dust the way the blow went and is heard (`FallingTrees.shatter`), with the felled
tree's crack for a fence and its thud for a wall until they have sounds of their own.

## Not done

- Walls from orienteering maps.
- Fences and walls on the minimap and in the map editor.
- A broken piece leaves nothing lying: no rubble, no trampled wire.
