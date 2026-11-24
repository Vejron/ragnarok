/** The Ford Sierra: five seats in a rounded family hatchback. */

import { KIND_SIERRA } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SIERRA_MAX_SPEED, SIERRA_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { SIERRA_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const SIERRA: VehicleKind = {
  kind: KIND_SIERRA,
  model: (id) => new CarModel('sierra', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Ford Sierra', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: SIERRA_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 800, redline: 5800, spin: 0.6, shifts: [0, 0.18, 0.38, 0.6, 0.82], top: SIERRA_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: SIERRA_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.2, -1.55] },
}
