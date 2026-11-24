/** The jeep, a Ford Bronco with its top off: four open seats, its passengers firing their own guns. */

import { KIND_JEEP } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { JEEP_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL_V8 } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

export const JEEP: VehicleKind = {
  kind: KIND_JEEP,
  model: (id) => new CarModel('jeep', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Ford Bronco', chase: true, gauges: ['boost'] }, RIDE_ALONG, RIDE_ALONG, RIDE_ALONG] },
  sound: { chop: PLAIN_CHOP, engine: { name: 'gears', synth: { ...PETROL_V8, crackle: 0.3, pipe: 1.5, body: 300 }, idle: 700, redline: 4500, spin: 0.8, shifts: [0, 0.24, 0.48, 0.74], top: JEEP_MAX_SPEED, level: 0.4 } },
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
