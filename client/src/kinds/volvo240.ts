/** The Volvo 240 estate: five seats in the brick, its old four droning along. */

import { KIND_VOLVO_240 } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { VOLVO_240_MAX_SPEED, VOLVO_240_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { VOLVO_240_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const VOLVO_240: VehicleKind = {
  kind: KIND_VOLVO_240,
  model: (id) => new CarModel('volvo_240', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Volvo 240', chase: true, enclosed: true }, ...Array.from({ length: VOLVO_240_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 750, redline: 5200, spin: 0.7, shifts: [0, 0.2, 0.42, 0.66], top: VOLVO_240_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: VOLVO_240_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.2, -1.7] },
}
