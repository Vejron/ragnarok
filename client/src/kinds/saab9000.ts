/** The Saab 9000: five seats in a big turbo hatchback. */

import { KIND_SAAB_9000 } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SAAB_9000_MAX_SPEED, SAAB_9000_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { SAAB_9000_WHEELS } from '@/sim/slip'
import { TURBO_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const SAAB_9000: VehicleKind = {
  kind: KIND_SAAB_9000,
  model: (id) => new CarModel('saab_9000', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Saab 9000', chase: true, enclosed: true }, ...Array.from({ length: SAAB_9000_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: TURBO_FOUR, idle: 800, redline: 5800, spin: 0.55, shifts: [0, 0.18, 0.36, 0.57, 0.8], top: SAAB_9000_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: SAAB_9000_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.2, -1.6] },
}
