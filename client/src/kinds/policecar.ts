/** The police car: a Volvo 240 in white and blue with a light bar, its five seats and its old four. */

import { KIND_POLICE_CAR } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { POLICE_CAR_MAX_SPEED, POLICE_CAR_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { POLICE_CAR_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const POLICE_CAR: VehicleKind = {
  kind: KIND_POLICE_CAR,
  model: (id) => new CarModel('police_car', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the police car', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: POLICE_CAR_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    // The Volvo's four, revving a little higher to its higher top speed.
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 750, redline: 5600, spin: 0.7, shifts: [0, 0.2, 0.42, 0.66], top: POLICE_CAR_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: POLICE_CAR_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.2, -1.7] },
}
