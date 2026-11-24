import type { TileData } from '@/world'
import { insideFor } from '@/sim/interior'
import { isGate, openingsFor, type Opening } from './windows'

/*
 * Where a tile's gates are: worked out in the tile worker (`prepareTile`), which has every house's
 * openings and rooms to hand already, so the frame a tile is attached in does not work them out
 * again for every house to find the few with gates.
 */

/** A gate of a tile's building: the building's number, the gate's number among its openings, and the opening. */
export interface GateSpot {
  index: number
  opening: number
  o: Opening
}

/** The gates of `tile`'s buildings: each one's wide doors, where it has a floor to drive in on. */
export function gatesOfTile(tile: Pick<TileData, 'look'> & { buildings: readonly Pick<TileData['buildings'][number], 'look'>[] }): GateSpot[] {
  const out: GateSpot[] = []
  const look = tile.look
  if (!look) return out
  tile.buildings.forEach((building, index) => {
    if (!building.look) return
    const openings = openingsFor(look, building.look)
    if (!openings.some(isGate) || insideFor(look, building.look).pieces.length === 0) return
    openings.forEach((o, opening) => {
      if (isGate(o)) out.push({ index, opening, o })
    })
  })
  return out
}
