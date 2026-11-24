/** The Opel Kadett: five seats in an everyday hatchback. */

import { KIND_KADETT } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { KADETT_MAX_SPEED, KADETT_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { KADETT_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const KADETT: VehicleKind = {
  kind: KIND_KADETT,
  model: (id) => new CarModel('kadett', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Opel Kadett', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: KADETT_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 800, redline: 6000, spin: 0.6, shifts: [0, 0.18, 0.38, 0.6, 0.82], top: KADETT_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: KADETT_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.2, -1.45] },
}
