/** The Roadster: the sports car's wheels under an electric motor, with cold-gas thrusters. */

import { KIND_ROADSTER } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { ROADSTER_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { SPORTS_WHEELS } from '@/sim/slip'
import { ELECTRIC } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

export const ROADSTER: VehicleKind = {
  kind: KIND_ROADSTER,
  model: (id) => new CarModel('roadster', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'roadster', board: 'E: drive the Roadster', chase: true, gauges: ['air'] }, RIDE_ALONG],
  },
  sound: {
    chop: PLAIN_CHOP,
    // An electric motor: a clean whine and its gears' hiss, no combustion, and one gear.
    engine: { name: 'electric', synth: ELECTRIC, idle: 0, redline: 18000, spin: 0.5, shifts: [0], top: ROADSTER_MAX_SPEED, level: 0.25 },
  },
  // Nothing comes out of it but its thrusters' air.
  fx: { wheels: SPORTS_WHEELS, drivenFrom: 0, exhaust: null, bay: [0, 0.75, -1.35] },
}
