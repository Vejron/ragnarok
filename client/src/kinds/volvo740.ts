/** The Volvo 740: five seats in a square-cut saloon. */

import { KIND_VOLVO_740 } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { VOLVO_740_MAX_SPEED, VOLVO_740_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { VOLVO_740_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const VOLVO_740: VehicleKind = {
  kind: KIND_VOLVO_740,
  model: (id) => new CarModel('volvo_740', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Volvo 740', chase: true, enclosed: true }, ...Array.from({ length: VOLVO_740_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 750, redline: 5500, spin: 0.65, shifts: [0, 0.19, 0.4, 0.62, 0.84], top: VOLVO_740_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: VOLVO_740_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.2, -1.7] },
}
