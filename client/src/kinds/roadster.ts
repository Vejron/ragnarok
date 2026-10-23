/** The Roadster: the sports car's wheels under an electric motor, with cold-gas thrusters. */

import { KIND_ROADSTER } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { ROADSTER_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { SPORTS_WHEELS } from '@/sim/slip'
import { DRIVE, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

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
    // An electric motor: a clean whine and its inverter's harmonic, no combustion, and one gear.
    voices: [['sine', 1, 0.5], ['triangle', 2, 0.25], ['sine', 7.03, 0.06]],
    ...PLAIN,
    engine: { name: 'gears', shifts: [0], top: ROADSTER_MAX_SPEED, hz: 150, level: 0.1 },
  },
  // Nothing comes out of it but its thrusters' air.
  fx: { wheels: SPORTS_WHEELS, drivenFrom: 0, exhaust: null, bay: [0, 0.75, -1.35] },
}
