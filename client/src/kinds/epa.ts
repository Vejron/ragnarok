/** The EPA traktor: a driver and a passenger in a cut-down saloon, its big engine held to 30 km/h. */

import { KIND_EPA } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { EPA_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { EPA_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

export const EPA: VehicleKind = {
  kind: KIND_EPA,
  model: (id) => new CarModel('epa', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the EPA traktor', chase: true, gauges: ['boost'], enclosed: true }, { ...RIDE_ALONG, enclosed: true }] },
  sound: {
    chop: PLAIN_CHOP,
    // Locked into its two lowest gears: the engine screams at 30 km/h, as everyone in town hears.
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 800, redline: 4800, spin: 0.6, shifts: [0, 0.45], top: EPA_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: EPA_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.05, -1.6] },
}
