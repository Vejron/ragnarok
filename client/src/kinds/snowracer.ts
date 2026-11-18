/** The snowracer: a steerable sled for two, a little motor's buzz to get about, and gravity down a slope. */

import { KIND_SNOWRACER } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, SNOWRACER_MAX_SPEED } from '@/sim/consts'
import { SNOWRACER_WHEELS } from '@/sim/slip'
import { TWO_STROKE_SINGLE } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

export const SNOWRACER: VehicleKind = {
  kind: KIND_SNOWRACER,
  model: (id) => new CarModel('snowracer', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: ride the snowracer', chase: true }, RIDE_ALONG] },
  sound: {
    chop: PLAIN_CHOP,
    // A tiny single-cylinder two-stroke, a lawnmower's: a thin buzz that is soon at its top and holds
    // there however fast the slope takes the sled.
    engine: { name: 'cvt', synth: TWO_STROKE_SINGLE, idle: 2200, redline: 7000, spin: 0.3, shifts: [0], top: SNOWRACER_MAX_SPEED, level: 0.2 },
  },
  fx: {
    wheels: SNOWRACER_WHEELS,
    // Its runners slide; nothing it has spins.
    drivenFrom: null,
    exhaust: 'pipe',
    bay: [0, -0.13, 0.35],
  },
}
