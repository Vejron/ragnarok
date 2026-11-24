/** The Saab 900 Turbo: four seats behind its wraparound glass, and a turbo that whistles in once the revs are up. */

import { KIND_SAAB_900 } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SAAB_900_MAX_SPEED, SAAB_900_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { SAAB_900_WHEELS } from '@/sim/slip'
import { TURBO_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const SAAB_900: VehicleKind = {
  kind: KIND_SAAB_900,
  model: (id) => new CarModel('saab_900', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Saab 900 Turbo', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: SAAB_900_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: TURBO_FOUR, idle: 850, redline: 6000, spin: 0.55, shifts: [0, 0.17, 0.34, 0.55, 0.78], top: SAAB_900_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: SAAB_900_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.2, -1.65] },
}
