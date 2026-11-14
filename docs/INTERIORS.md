# Floors and stairs

A house big enough for it has an inside: a ground floor on a plinth, a floor for every storey that
fits under its eaves, and stairs between them. Each storey is one open room. Its door stands open,
so a soldier walks in, up the stairs, and fights from an upstairs window. Sheds, garages and other small buildings stay empty and shut, as every building was before.

Nothing of this is in the world file or on the wire. It is worked out from what a building already
has (its footprint, its look and the openings on its walls), so every world with looks
([WORLD.md](WORLD.md), "Looks") has floors without being imported again, and the world and protocol
versions are unchanged.

## What you get

- **Which houses.** A building with a look and a door whose footprint fits a box of at least 6 × 5 m
  and covers 40 m² (`MIN_LONG`, `MIN_SHORT`, `MIN_AREA`). On Kläppudden that is about half of them.
- **The door.** The painted door of such a house is an open doorway, its leaf swung in. The ground
  floor is 30 cm up (`PLINTH`), a step a soldier takes without jumping, so uneven ground inside the
  footprint stays under it.
- **Storeys.** A floor every 2.8 m, the height the rows of windows are apart, while the floor is
  below the eaves and there are 2 m under the ridge. An upper floor is a slab 20 cm thick (`SLAB`):
  boards on top, a plastered underside, and an edge round the hole the stairs come up through. A
  house with a pitched roof and low walls is one storey, open to its rafters.
- **Stairs.** One stairwell 2 × 4.7 m in a corner, with two flights side by side: the one by the
  wall goes up one way and the next comes back beside it, so every storey's flight starts where the
  last one ended. A flight is walked as a slope of about 42° and drawn as steps. A footprint with no
  corner the stairwell fits in gets a ground floor only.
- **Damage.** Floors and stairs are cells of the building like its outer walls:
  a blast takes a bite out of them, rounds wear them through, they hold the house up and they fall
  with what holds them. Blow a hole in a floor and you drop through it.
- **The camera.** Under a roof the on-foot camera keeps to the room: it comes in to the wall behind
  the soldier and beside his shoulder, and stays under the roof when the look goes out through a
  window.

## How it works

`shared::interior::rooms(footprint, walls, roof, windows)` (`crates/shared/src/interior.rs`) returns
triangles in centimetres, each with what it is: `PART_FLOOR` (the top of a slab), `PART_CEILING`
(its underside and its edge round the stairwell), `PART_RAMP` (the
slope a flight is climbed as, drawn as its underside) and `PART_STEP` (a tread or riser: drawn, not
collided with). The plan is laid out in the house's own axes, along its longest side. Only
`+ - * /` and `sqrt` are used and every corner is rounded to a centimetre, so the server's native
build and the client's WebAssembly one get the same triangles; `the_same_house_gets_the_same_floors`
and `interior.spec.ts` pin the same sum on both sides. Which corner the stairwell is tried in first
is decided by a number seeded from the footprint.

- **Server.** `WorldFile::assemble` keeps the collided parts in `BuildingLook::inside`.
  `sim::building_shell` adds them to the building's cells (`Shell::furnish`: the grid and every
  cell's number stay as the importer baked them, so `Carved` means what it did), and
  `facade::standing` collides with them, sliced by gone cells as the walls are, and cuts the doorway
  (`cut_windows` with `doorway`: from under the door's foot to `DOOR_RISE` over its top, the height
  a soldier stepping onto the plinth needs clear).
- **Client.** `client/src/sim/interior.ts` asks the WebAssembly build (`interior_of`) once per
  building and keeps the answer on the building's look range, so it goes along to the look worker,
  which has no WebAssembly. `shellOf` furnishes the cells (`carve_furnish`); the prediction's
  physics is the server's code and needs nothing more. `facadeFor` carries the parts
  (`Facade.inside`); `buildingmesh.ts` draws them from both sides, `lookFracture` leaves out the
  pieces in gone cells and `lookChunk` puts them in the section that falls. `Game.roomArm` is the
  camera.
- **Slabs keep off the lattice.** A floor's top and its underside are each moved at least 5 cm off the boundary between two
  layers of cells (`OFF_LATTICE`), so the cell it is cut into is the cell it is judged by when that
  cell goes.

## Limits

- A world without looks has no floors until it is imported again.
- On a steep site the ground shows through the uphill end of the ground floor, which lies 30 cm
  over the lowest ground under the house.
- Bots find their way on a flat grid: the open door lets them onto a ground floor, never upstairs.
- A storey is one empty room: no walls inside, no furniture, and nothing is placed indoors.
- Indoors the camera's shoulder comes in from a wall, so there the crosshair is a little off the
  line the server shoots along.
