# Building with bricks

Bricks are a hotbar item. With them a player puts 72 cm cubes onto the world: against a building's wall, on its roof, on the ground, or on cubes already placed, and goes on building in any direction for as long as they like. Cubes are on the same lattice as the buildings' own cells, so a cube against a wall lines up with it, and they are drawn, collided with, blasted and brought down like those cells. A tower stands on the ground or on a building; blow out what holds it and it comes down on whoever is under it.

## What you get

- **The item** (`ITEM_BRICKS` = 11, "Bricks"). It is in everyone's starting loadout, in slot 5, where the loitering drone used to be. The loitering drone is still found lying about. Bricks are never used up, are not scattered as pickups, and can be placed in the map editor and named in a scenario loadout.
- **Place (click or F).** A cube goes where the crosshair is, on the near side of the face the crosshair ray meets. Hold the button to keep building, a cube every 10 ticks (1/6 s): looking at the last cube placed, the next one goes against it.
- **Dig (aim held, then click or F).** The placed cube the crosshair is on is taken out. Aim is the right mouse button. Building cells cannot be dug, only placed cubes.
- **Ghost.** With bricks in hand, a see-through white cube shows where the next one would go, or a faint grey one where it would be refused (nothing to rest on, or something in the way). With aim held, the cube that would be dug glows red instead.
- **At once.** Your own cubes appear, and dug ones go, the moment you click, and you can stand on them straight away. If the server does not agree, which is rare, the cube is taken back about a round trip later.
- **Colour.** A cube is the builder's outfit colour (`PLAYER_COLORS`, chosen in the settings).
- **Reach.** The middle of the cube must be within 6 m of the eye (`BUILD_REACH`).
- **Where a cube can go.** It is placed only when all of these hold:
  - nothing is there already: no cube, wall, roof, tree trunk or vehicle;
  - no soldier or creature stands in it;
  - it is not wholly under the ground;
  - it rests on something: another cube beside, above or below it, a standing cell of a building beside, above or below it, or the ground under it;
  - the world holds fewer than 200 000 cubes (`MAX_BUILT`).
- **Filling holes.** A cube may go where a blast took a building's cell, so a rocket hole can be bricked up.
- **On foot only.** Bricks do nothing from a seat.

## What happens to cubes

- **Bullets** wear cubes down as they wear down a building's wall. Each round takes its damage off the cube it strikes, and a brick has 40 hp: about four AK5 rounds, two from a revolver, one sniper round. A cube that breaks goes as if dug, and whatever only it held up falls. Cubes your own rounds hurt are drawn darker, a step for each third of their health gone, as a building's wall is drawn dented for the shooter. Other players' hits are not shown until the cube breaks.
- **Vehicles** break cubes as they break walls (`combat::ram_cubes`). The cube a vehicle hits takes 8 damage for each m/s the hit took out of it, so a 5 m/s knock breaks a brick. A hit harder than 6 m/s also bites a hole, as in a wall. Tanks and other heavy vehicles count for more (`body.ram`), and drones do not ram.
- **Blasts.** Any blast takes out the cubes within 60% of its radius, the same reach it bites a wall with. That covers rockets, grenades, shells, bombs, mortar rounds, drone charges and a wreck going up.
- **Falling.** When a cube goes (dug, blasted, or the building cell under it blasted away), the cubes joined to it are checked. A piece of cubes that no longer reaches the ground or a standing building cell comes down whole. It lands on whoever is under it as rubble, with the same damage as falling building cells: cubes × metres fallen (counted up to 8 m) × 4, vehicles at 40%, credited to nobody (`WEAPON_RUBBLE`). The check looks through at most 4 096 cubes of a piece (`BUILD_SUPPORT_SEARCH`). A bigger piece is taken as held, so one blast never costs a long tick.
- **No strength.** A cube holds anything that is joined to it: a beam can stick out as far as anyone builds it. Only the connection counts, not the weight.
- **Restarts.** Cubes live in the room. They are gone when the server restarts, as carved holes are.

## How it works

### One layer for the whole world

Placed cubes are not part of any building (`crates/shared/src/construct.rs`). A building's cells are numbered within its bounding box, which the importer fixed when the world was made. A build that goes past the box, or stands on its own, has no number there, and growing the box would renumber cells already sent in `carved`. So placed cubes live in their own sparse layer, `Construct`, named by their place on the world lattice (`Pos`: centimetres divided by `CELL`, rounded down). The lattice is the buildings' own, with cell corners at multiples of 72 cm, so the two line up exactly. Building cell numbers, `carved` and their fixtures are untouched.

The layer is kept in chunks of 16 × 16 × 16 cubes, in a `BTreeMap`, so everything that walks it does so in the same order on every machine. A chunk is the unit of drawing and collision. Placing a cube redraws one chunk and rebuilds one chunk's collider. A chunk is meshed by the same greedy pass as a building (`carve::faces_of`) and merged into boxes by the same pass (`carve::boxes_of`, taken out of `Shell::boxes` for this), so the server and the client collide with identical boxes.

### Server

- **Aiming** (`mechanics/building.rs`, `aim`). The crosshair ray is rebuilt as for a gun. It starts at the on-foot camera pivot (`ON_FOOT_SHOULDER` right of the capsule, `ON_FOOT_HEIGHT` up) and runs along the look, and Rapier gives where it meets the world and the surface normal there. The cube to place is the lattice cube half a cube out from the hit along the normal, and the cube to dig is the one half a cube in. Buildings are boxes, so their normals are along the axes. On terrain the normal leans and puts the cube on top of the slope.
- **Using bricks** is a branch of `combat::use_item` (`ItemKind::Build`): with fire held and the cooldown run out, it calls `building::use_bricks`, which digs with aim held and otherwise checks `may_place` and places.
- **What holds a cube** (`World::held_from_outside`, `game/construct.rs`): its bottom is at or below the terrain under its middle, or one of its six neighbours is a standing cell of a building. The buildings near it are found through the physics broad-phase (`objects_near`), and a building is cut into cells (`Carving`) the first time it is asked, as a blast does.
- **Falling** (`World::drop_cubes`, `Construct::unsupported`): a breadth-first walk from the cubes next to what went, through cubes sharing faces, until one is held from outside. A piece with none is taken out as `fell`, and one `Rubble` per column of it lands after the time the fall takes (`mechanics/structure.rs` hurts what is under it). A building cell cut by a blast or by `strain` (`World::finish_cut`) loosens the cubes beside it (`loosen_beside_cells`).
- **Blasts** call `World::blast_cubes` at the end of `combat::blast_at`. **Bullets** end in `combat::hit_object`. When the ray met a chunk of cubes (`Physics::ray_meets_construct`), the cube just behind the hit takes the round (`World::chip_cube`).
- **Colliders.** Every chunk that changed in a tick is rebuilt once, at the end of the tick (`World::flush_construct`). It goes through `Physics::set_construct`, applied with the other queued reshapes at the start of the next step. Chunk colliders are not destructible `Obj`s. `Physics::cube_blocked` says whether a new cube would overlap a building, a tree, a vehicle or other cubes; the ground and the boundary walls do not count.
- **Telling everyone.** What changed in a tick goes out as one `Built` message (`Game::broadcast_built`). A joiner gets every cube as `Built` with `live` false, in batches of 32 768. See [NETWORKING.md](NETWORKING.md).

### Client

- `client/src/sim/construct.ts` keeps the same layer, from the `built` messages (`WorldView.built`).
- Each changed chunk is redrawn once a frame (`render/construct.ts`, `ConstructView`), one mesh a chunk with a colour per vertex. The cubes just outside the chunk are meshed as a border that draws nothing (as `voxelBrick` does for buildings), so faces between two chunks are left out. A cube on a chunk's edge redraws its neighbour too. It is also handed to the prediction's physics (`SimMotor.setConstruct`, the WASM `setConstructChunk`), which merges it into boxes with the server's code. Chunks wait for the simulation if it does not exist yet, as during the catch-up on joining.
- Cubes that `fell` come down as one piece to the terrain under their middle, raise dust, and lie there for 4 s. Dug and blasted cubes go in a little dust.
- The ghost cube is `Game.brickAim`: the server's aim, with the client's prediction physics doing the ray cast. It is white where `mayPlace` (`sim/bricks.ts`) says a cube would go and grey where it would not.
- **Prediction** (`Game.buildFeedback`, `sim/bricks.ts`, `WorldView.predict`). On each input tick the client counts bricks' cooldown as the server does (`BrickTrigger`). On a tick the server would use bricks on, it aims from the predicted position with that frame's look, and checks the server's placing rules (`mayPlace`):
  - `cubeBlocked` and `buildingNear` are the client's physics, through WASM;
  - the ground is the client's terrain;
  - the only body looked at is the local player's.

  It then places (or digs) the cube in its own layer at once, drawn and solid at the next frame. Each prediction waits for the server's `built` to name its cube, until 1.5 round trips plus 400 ms have passed (`BRICK_PREDICTION_SLACK_MS`). A placed or removed entry for that place settles it, and a cube of someone else's colour there takes the predicted one's place. Anything not confirmed in time is taken back: the cube goes, or the dug cube returns.
- **Where prediction can be wrong:**
  - The server's cooldown is shared with the guns, so just after a shot the client can place a tick or two early. The aim has hardly moved, so it is the same cube.
  - Another player standing where the cube goes is not seen, so the cube shows and is then taken back.
  - Support from a building is judged by its collider reaching just past the cube's faces, not by its cells, which agrees with the server wherever the building is drawn as it is.
- A round of this player's that ends on a cube wears it locally (`WorldView.chipped`, `Construct.dent`), and its chunk is redrawn darker.

### Materials: room for toughness

Every cube is of a `Material` (`MATERIALS` in `crates/shared/src/construct.rs`), stored with the cube and sent with it (`BuiltCube.material`):

| Field | What it does | Brick |
|---|---|---|
| `hp` | Bullet damage that breaks a cube. | 40 (`CELL_HP`, as a building's wall) |
| `blast_reach` | Share of a blast's wall-biting reach that takes the cube out, at most 1. | 1 |

Bricks place `MATERIAL_BRICK`, the only material so far. A tougher block (concrete that takes a magazine and only goes right next to a blast) or a flimsier one is one row of `MATERIALS` and an item that places it. The wire, the storage and the rules already carry the material, so neither needs a protocol change. Damage is kept only for cubes that have been hit (`Construct::chip`, `Construct::hp`), so a whole build costs nothing. The client reads the same table (`tables.generated.json`, `MATERIALS` in `sim/construct.ts`) and keys each cube's faces by colour, material and wear. How a material looks is a row of `LOOKS` in `render/construct.ts`: the builder's colour mixed towards a tint. Bricks are the plain colour. A material with no row stops the client at load, as an item with no presentation does.

## Tuning

| Constant | Where | Value | What it does |
|---|---|---|---|
| `BUILD_REACH` | `shared::consts` | 6 m | Furthest a cube's middle may be from the eye. |
| `BUILD_COOLDOWN_TICKS` | `shared::consts` | 10 | Ticks between cubes while fire is held. |
| `MAX_BUILT` | `shared::consts` | 200 000 | Most cubes in the world. |
| `BUILD_SUPPORT_SEARCH` | `shared::consts` | 4 096 | Most cubes of one piece looked through for a hold. |
| `CHUNK` | `shared::construct` | 16 | Cubes a chunk is along each side. |
| `CARVE_REACH` | `mechanics/combat.rs` | 0.6 | Share of a blast's radius that bites walls, and so cubes (times `blast_reach`). |
| `MATERIALS` | `shared::construct` | brick | How tough each kind of cube is; see above. |

## Not done yet

- Only bricks exist. Another material needs a row in `MATERIALS`, a look in `LOOKS` and an item that places it.
- Bots walk round cubes (their walking grid is brought up to date where colliders change), but they do not build or shoot their way through.
- Cubes add nothing to the health or damage stage of a building they are built onto. They are not part of it, and they fall when what holds them goes.
- Cubes do not survive a server restart, as carved holes do not.

## Files

| File | What it holds |
|---|---|
| `crates/shared/src/construct.rs` | The layer: chunks, faces, boxes, support, materials, cube health, blasts' reach. |
| `crates/shared/src/carve.rs` | `boxes_of`, the box merge shared with buildings. |
| `crates/shared/src/items.rs`, `consts.rs` | `ITEM_BRICKS`, `ItemKind::Build`, the constants above. |
| `crates/shared/src/protocol.rs` | `Built`, `BuiltCube`, `CubePos`. |
| `crates/sim/src/lib.rs`, `vehicle.rs` | Chunk colliders, `cube_blocked`. |
| `crates/sim-wasm/src/lib.rs` | `setConstructChunk`. |
| `crates/server/src/game/construct.rs` | Placing, digging, blasts, falling, colliders. |
| `crates/server/src/mechanics/building.rs` | Aiming, the placing rules, using bricks; tests in `building/tests.rs`. |
| `client/src/sim/construct.ts` | The client's copy of the layer. |
| `client/src/sim/bricks.ts` | The placing rules and cooldown, mirrored for prediction. |
| `client/src/render/construct.ts` | Chunk meshes, falling pieces, the ghost. |
| `client/src/worldview.ts`, `game.ts` | The `built` message, the ghost's aim. |
| `client/src/__tests__/construct.spec.ts`, `build-prediction.spec.ts` | The client's tests. |
