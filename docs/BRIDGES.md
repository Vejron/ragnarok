# Destructible bridges

Bridges can be brought down. Their concrete (deck, piers and railings) is carved by blasts as a house's
walls are (see [BUILDING.md](BUILDING.md) and `shared::carve`): a shell bites a ragged hole, and whatever
is left with nothing to hold it falls into the river. Bridges are much tougher than houses, though:

- **Small arms do nothing.** Rifles, pistols, the machine guns on vehicles and the rotary gun's bullets
  leave no mark, and no hole wears through.
- **The 40 mm barely marks them.** An autocannon round wears the 72 cm cell it hits, and about a dozen
  rounds on the same spot break one cell out.
- **Shells, rockets and bombs bite**, with a much smaller bite than they take of a house. Cutting a
  deck across takes several tank shells, and felling a pier more.
- **Driving into a bridge does nothing**, and neither does a Titan's foot coming down on it.
- **A Titan's punch bites it**, less than it would bite a house.

How the bridges are built (deck heights, ramps, piers and pylons) is in [WORLD.md](WORLD.md), "Bridges".
Over a small river a deck stays level with its banks: [RIVER-CHANNELS.md](RIVER-CHANNELS.md).

## Sections

The importer (`osm-import`'s `bridge::sections`) cuts each bridge's concrete into **sections**, halfway
between one pier and the next. Each section then stands on one pier, with half a span on either side
of it. A stretch with no piers (a ramp, or a deck that is solid down to the ground) is cut every
`MAX_SECTION` (48 m) or less, so no section's grid of cells gets too big.

Each section is a building of kind `BUILDING_BRIDGE` in the tile of its middle:

- Its walls are the deck's sides and underside, the pier, and the edge beams and railings. Its roof
  is the faces of the deck that look up: the roadway (`Piece::deck`). The tops of the edge beams
  and rails are walls, so they stay concrete.
- Its footprint is the convex hull of its corners.
- Its `base` is the lowest ground or river bed under it, which is where its rubble lands.
- Its `paint` is `BRIDGE_CONCRETE` for its walls, and for its roof the road it carries
  (`bridge::deck_paint`): `DECK_ASPHALT` for a paved road or path, `DECK_PLANKS` for a footpath or a
  track, `DECK_BALLAST` for a railway. The client draws the roof with the matching pattern
  (`buildingSurfaces`: `Surface.Asphalt` is the ground road's `asphaltLook` without its crumbling
  edge, `Surface.Planks` boards 20 cm wide, `Surface.Ballast` faint stones in the colour of a track's
  bed, snowed on in winter), and its cells keep it once the section is carved.
- A railway's sleepers and rails are not part of the section, and nothing collides with them. They
  are the tile's track (`Rail::deck`, WORLD.md "Railways"), which names the section it lies on. Each
  sleeper's length of it is drawn while the deck's top cell stands under its middle and under both
  rails (`render/decktrack.ts`, `deckStands`), so a hole takes its track along and none hangs across it.
- Its thin parts (rails, posts, the tops of the edge beams: any face with the face opposite it less
  than `THIN`, 35 cm, behind it, `lookfracture.ts`) are drawn plain when the section is hurt. They
  get none of a hole's soot, ragged corners, edge depth or dim inside, which are sized for a wall
  and would stand out of an 8 cm rail as black slabs. A whole section draws no inside at all.
- What stands on its roadway (the edge beams, posts and rails: the cells above a roof cell, or above
  another such cell, `Shell::rests`) rests on the cell under it. When that cell goes, so does
  everything stacked on it, and then whatever that leaves cut off (`unsupported_after`). Otherwise a
  railing, being one long line of cells held at both ends, would be left hanging across a hole
  blown out of the deck under it.
- What a cut leaves too narrow to stand falls (`NARROW`, 4 cells, about 2.9 m): a cell's width is
  the shorter run of standing cells through it along x and along z, four cells each way, counting
  a column as standing if anything stands in its layer or the one above or below (the cells are a
  shell, so a deck's sides are a line of their own). Only a cell left narrower than it stood when
  whole goes, so a pier or a footbridge built narrower stands. Without it, the strips along a
  deck's edges, with their edge beams and railings, hang on across the river as beams once the
  middle is blown out between them.
- A bridge is never weighed as a house is (`Shell::strain`, the structure checks `settle` runs). A
  pier is a shell of cells each carrying the whole deck, so a hole in its side left the cells round
  it far over their load, and shedding them spread until one Carl Gustaf round in a column brought
  a whole section down. A bridge falls only where a cut leaves it with nothing under it, too narrow,
  or cut off from the ground: a pier must be cut right through, and a wide deck's two columns both.
- A hole in a concrete deck shows no broken rafters (`edgeDetail`), only a planked one does.
- A hole's edge is drawn only where it borders a face still standing (`bordering`). When a whole
  stretch is gone, its faces were cut at different places either side of each corner, so their edges
  no longer meet. A house keeps those edges as the broken foot of its walls along the ground, but on
  a bridge they would be left drawn in the air where the deck was, as long dark lines over the river.
- `bridge_wreckage` (an ignored server test) shells the viaduct by Backen in `home` and draws what is
  left of each section; `WRECKAGE_OUT` writes the cuts for the client to replay (`view.carved`).
- It is a closed solid. Each 3 m stretch of deck is a box capped at both ends (`bridge::build`), so a
  section cut from the middle of the bridge has no open end. The client would draw an open end as the
  broken rim of a hole, left hanging in the air once the rest had fallen.

Like any building, `voxels::bake` cuts it into cells and moves its triangles to the tile's look. It
is therefore drawn and collided from its own smooth concrete, with the gone cells clipped out
(`shared::facade::standing`), the same on the server and every client.

A bridge has no windows (`windows::openings_of` and the client's `openingsOf` give none), so it has
no inside and no gates. It also has no floor, eaves or chimney, plain concrete walls instead of a house's patterns,
and it is not drawn on the minimap as a house (its road is on the minimap already). Grass still grows
under it.

The steel pylons and stay cables are not in the sections. They stay in the tile's collision mesh,
listed in `Tile.bridges` as `(first, steel)`, and nothing breaks them.

## What holds a bridge up

A house stands on its lowest layer of cells. A bridge stands on the ground under it: on the server,
`World::cut_building` calls `Shell::anchor_to_ground` for a bridge section. A cell is an anchor when
its bottom is within a cell of the ground under it (pier feet on the river bed, deck ends on the
banks). The ground used is as the importer left it, so a crater dug at a pier's foot does not leave the
pier standing on nothing.

From there the house rules apply unchanged:

- A cut that leaves a piece with no chain of cells to an anchor drops that piece at once.
- Every few ticks `Shell::strain` checks what is left against the whole section, and lets go of what
  now hangs too far or carries too much.

What this gives:

- **Blowing a pier through** drops the whole section: the deck on the pier and the half span on either
  side of it.
- **Cutting across the deck** drops the stretch that is no longer joined to the pier or to the bank.
- **Neighbouring sections** stand on their own piers and stay up.

Only the server decides what falls. The clients apply the cells they are sent (`Carved`), so the
anchoring needs nothing on the wire and no client code.

## Toughness

The rules are in `combat.rs` (`blast_bridge`, `punch_bridge`, `hit_object`, `ram_building`) and `projectiles.rs` (`punch`):

| Constant | Value | What it does |
|---|---|---|
| `BRIDGE_CELL_HP` (`shared::carve`) | 250 | A cell's health against what wears it. A house's cells have `CELL_HP`, 40. |
| `BRIDGE_SHRUG` | 20 | The part of a blast's damage, where it reaches the bridge, that does nothing. |
| `BRIDGE_BITE_DAMAGE` | 60 | The least damage that bites. Below it, or when the bite would be under `CARVE_MIN_RADIUS` (0.8 m), the blast wears the nearest cell by its damage less `BRIDGE_SHRUG`. |
| `BRIDGE_CARVE_REACH` | 0.2 | The share of the blast radius that bites, at full strength. A house's is `CARVE_REACH`, 0.45. |

A bridge has no health of its own and no damage stages. Only what is bitten or worn out of it counts.
The 40 mm armour-piercing round's hole (`punch`) is not made in a bridge; its small charge wears it as
any small blast does.

What each weapon does at the middle of its blast:

| Weapon | Damage | Against a bridge |
|---|---|---|
| Rifle, MG, rotary gun bullets | any | nothing |
| 40 mm HE (`PROJECTILE_CANNON`) | 42 | wears 22: about 12 rounds a cell |
| 40 mm AP charge | 35 | wears 15 |
| Tank AP charge | 70 | wears 50: its bite would be under `CARVE_MIN_RADIUS` |
| Rocket | 110 | bites 1.0 m |
| Tank HE shell | 125 | bites 1.4 m (3.2 m of a house) |
| Bomb | 110 | bites 1.2 m |
| Mortar bomb | 260 | bites 2.2 m |

A Titan's fist (`punch_bridge`) bites only when it moves at `BRIDGE_PUNCH_SPEED` (10 m/s) or more. Its
foot coming down (6 m/s) does not, so a Titan can cross a bridge without wrecking it. The bite is
0.8 m at that speed and 0.05 m more for each m/s past it, up to the fist's 2.5 m: 1.8 m at 30 m/s.

## Falling into water

On the client, a piece of a fallen section that goes into water splashes where it crosses the surface
(`Damage.waterAt` and `Damage.splashed`, wired to `render/splash.ts` in `game.ts`), and then sinks to
the bed.

## Not yet

- Vehicles' road routes (bots' and the buses') still run over a broken deck. A soldier's paths do not,
  because `navigation` reads the floors from the physics and rebuilds a chunk after a bite.
- The pylons and stay cables cannot be destroyed.
