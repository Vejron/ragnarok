# Destructible bridges

Bridges can be brought down. Their concrete (deck, piers and parapets) is carved by blasts as a house's
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

## Sections

The importer (`osm-import`'s `bridge::sections`) cuts each bridge's concrete into **sections**, halfway
between one pier and the next. Each section then stands on one pier, with half a span on either side
of it. A stretch with no piers (a ramp, or a deck that is solid down to the ground) is cut every
`MAX_SECTION` (48 m) or less, so no section's grid of cells gets too big.

Each section is a building of kind `BUILDING_BRIDGE` in the tile of its middle:

- Its walls are the deck's sides and underside, the pier and the parapets. Its roof is the faces that
  look up: the top of the deck and of the parapets.
- Its footprint is the convex hull of its corners.
- Its `base` is the lowest ground or river bed under it, which is where its rubble lands.
- Its `paint` is `BRIDGE_CONCRETE` for walls and roof alike.
- It is a closed solid. Each 3 m stretch of deck is a box capped at both ends (`bridge::build`), so a
  section cut from the middle of the bridge has no open end. The client would draw an open end as the
  broken rim of a hole, left hanging in the air once the rest had fallen.

Like any building, `voxels::bake` cuts it into cells and moves its triangles to the tile's look. It
is therefore drawn and collided from its own smooth concrete, with the gone cells clipped out
(`shared::facade::standing`), the same on the server and every client.

A bridge has no windows (`windows::openings_of` and the client's `openingsOf` give none), so it has
no inside and no gates. It also has no floor, eaves or chimney, plain surfaces instead of a house's patterns,
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
