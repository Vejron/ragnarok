/** The Golf GTI: four seats, a revvy four and the quickest launch of the everyday cars. */

import { KIND_GTI } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { GTI_MAX_SPEED, GTI_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { GTI_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const GTI: VehicleKind = {
  kind: KIND_GTI,
  model: (id) => new CarModel('gti', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Golf GTI', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: GTI_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 900, redline: 7000, spin: 0.5, shifts: [0, 0.16, 0.32, 0.52, 0.75], top: GTI_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: GTI_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.2, -1.45] },
}
