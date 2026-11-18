/** The technical: a driver, a passenger firing their own guns, and a gunner on the machine gun in the bed. */

import { KIND_TECHNICAL } from '@/net/codec'
import { loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { TechnicalModel } from '@/render/technical'
import { TECHNICAL_MAX_SPEED, TECHNICAL_MG_COOLDOWN_TICKS, TECHNICAL_MG_PITCH_MAX, TECHNICAL_MG_PITCH_MIN } from '@/sim/consts'
import { TECHNICAL_WHEELS } from '@/sim/slip'
import { DIESEL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

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
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: DIESEL_FOUR, idle: 750, redline: 3800, spin: 0.9, shifts: [0, 0.2, 0.42, 0.68], top: TECHNICAL_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: TECHNICAL_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.4, -1.75] },
}
