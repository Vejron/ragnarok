/** The pirate ship: sailed by the wind from her helm, with six guns on her deck that are vehicles of their own. */

import { KIND_SHIP } from '@/net/codec'
import { ShipModel } from '@/render/ship'
import { PLAIN, PETROL, DRIVE, type VehicleKind } from './kind'

/** She is 18 m long and her mast 12 m tall: the camera stands well off. */
const SHIP_DISTANCE = 24

export const SHIP: VehicleKind = {
  kind: KIND_SHIP,
  model: (id) => new ShipModel(id),
  ride: {
    distance: SHIP_DISTANCE,
    reload: null,
    seats: [{ rig: DRIVE, role: 'helm', board: 'E: take the helm', chase: true, gauges: ['wind'] }],
  },
  // No engine: the wind and the water.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // No wheels: she floats, and carves a wake.
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 0, 0], floats: true },
}
