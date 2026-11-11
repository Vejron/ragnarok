/** The submarine: its captain steers and dives it, and its weapons officer looks through the periscope and fires missiles. */

import { KIND_SUBMARINE } from '@/net/codec'
import { SubmarineModel } from '@/render/submarine'
import { SUB_MAX_SPEED } from '@/sim/consts'
import { PLAIN, DRIVE, type VehicleKind } from './kind'

/** It is 22 m long and its sail stands 3 m over the deck: the camera stands well off. */
const SUB_DISTANCE = 26

export const SUBMARINE: VehicleKind = {
  kind: KIND_SUBMARINE,
  model: (id) => new SubmarineModel(id),
  ride: {
    distance: SUB_DISTANCE,
    reload: null,
    seats: [
      { rig: DRIVE, role: 'captain', board: 'E: take command of the submarine' },
      // Holding aim zooms the periscope, as a gun's sight does.
      { rig: { name: 'periscope' }, role: 'periscope', board: 'E: man the periscope', sight: true },
    ],
  },
  sound: {
    // Electric motors under water: a low hum, a little uneven.
    voices: [['sine', 1, 0.6], ['triangle', 2, 0.2], ['sine', 3.01, 0.08]],
    ...PLAIN,
    engine: { name: 'gears', shifts: [0], top: SUB_MAX_SPEED, hz: 55, level: 0.05 },
  },
  // No wheels: it carves a wake while it runs on the surface.
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 0, 6], floats: true },
}
