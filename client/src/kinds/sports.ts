/** The sports car: two seats, rear-wheel drive, and a backfire when the driver lifts off. */

import { KIND_SPORTS } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, SPORTS_MAX_SPEED } from '@/sim/consts'
import { SPORTS_WHEELS } from '@/sim/slip'
import { PETROL_V8 } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

export const SPORTS: VehicleKind = {
  kind: KIND_SPORTS,
  model: (id) => new CarModel('sports', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the sports car', chase: true }, RIDE_ALONG] },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_V8, idle: 850, redline: 7200, spin: 0.45, shifts: [0, 0.16, 0.32, 0.5, 0.72], top: SPORTS_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: SPORTS_WHEELS, drivenFrom: 2, exhaust: 'pipe', backfire: true, bay: [0, 0.75, -1.35] },
}
