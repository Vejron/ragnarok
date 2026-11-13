/** The snowracer: a steerable sled for two, a little motor's buzz to get about, and gravity down a slope. */

import { KIND_SNOWRACER } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, SNOWRACER_MAX_SPEED } from '@/sim/consts'
import { SNOWRACER_WHEELS } from '@/sim/slip'
import { DRIVE, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

export const SNOWRACER: VehicleKind = {
  kind: KIND_SNOWRACER,
  model: (id) => new CarModel('snowracer', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: ride the snowracer', chase: true }, RIDE_ALONG] },
  sound: {
    // A tiny single-cylinder two-stroke, a lawnmower's: a thin buzz that is soon at its top and holds
    // there however fast the slope takes the sled.
    voices: [['square', 1, 0.3], ['sawtooth', 2, 0.15]],
    ...PLAIN,
    engine: { name: 'gears', shifts: [0], top: SNOWRACER_MAX_SPEED, hz: 95, level: 0.08 },
  },
  fx: {
    wheels: SNOWRACER_WHEELS,
    // Its runners slide; nothing it has spins.
    drivenFrom: null,
    exhaust: 'pipe',
    bay: [0, -0.13, 0.35],
  },
}
