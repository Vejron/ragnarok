/** The submarine: its captain steers and dives it, and its weapons officer looks through the periscope and fires missiles. */

import { KIND_SUBMARINE } from '@/net/codec'
import { SubmarineModel } from '@/render/submarine'
import { SUB_MAX_SPEED } from '@/sim/consts'
import { SUB_MOTORS } from './engines'
import { PLAIN_CHOP, DRIVE, type VehicleKind } from './kind'

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
    chop: PLAIN_CHOP,
    // Electric motors under water: a low hum.
    engine: { name: 'marine', synth: SUB_MOTORS, idle: 0, redline: 1800, spin: 2, shifts: [0], top: SUB_MAX_SPEED, level: 0.12 },
  },
  // No wheels: it carves a wake while it runs on the surface.
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 0, 6], floats: true },
}
