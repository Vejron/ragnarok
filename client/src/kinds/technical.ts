/** The technical: a driver, a passenger firing their own guns, and a gunner on the machine gun in the bed. */

import { KIND_TECHNICAL } from '@/net/codec'
import { loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { TechnicalModel } from '@/render/technical'
import { TECHNICAL_MAX_SPEED, TECHNICAL_MG_COOLDOWN_TICKS, TECHNICAL_MG_PITCH_MAX, TECHNICAL_MG_PITCH_MIN } from '@/sim/consts'
import { TECHNICAL_WHEELS } from '@/sim/slip'
import { DRIVE, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

export const TECHNICAL: VehicleKind = {
  kind: KIND_TECHNICAL,
  model: (id) => new TechnicalModel(id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: TECHNICAL_MG_COOLDOWN_TICKS,
    seats: [
      { rig: DRIVE, role: 'driver', board: 'E: drive the technical', chase: true },
      RIDE_ALONG,
      // The gun turns all the way round on its post.
      { rig: { name: 'gun', hull: [Math.PI, TECHNICAL_MG_PITCH_MIN, TECHNICAL_MG_PITCH_MAX] }, role: 'gunner', board: 'E: man the machine gun', gauges: ['heat'], stand: true },
    ],
  },
  sound: {
    // A big diesel: low, with a hard knock in its harmonics.
    voices: [['sawtooth', 1, 0.5], ['square', 0.5, 0.45], ['square', 2.02, 0.1]],
    ...PLAIN,
    engine: { name: 'gears', shifts: [0, 0.2, 0.42, 0.68], top: TECHNICAL_MAX_SPEED, hz: 34, level: 0.24 },
  },
  fx: { wheels: TECHNICAL_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.4, -1.75] },
}
