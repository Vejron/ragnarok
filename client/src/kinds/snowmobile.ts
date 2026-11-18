/** The snowmobile: two riders one behind the other, a gun in tow, and a two-stroke's scream up any slope. */

import { KIND_SNOWMOBILE } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, SNOWMOBILE_MAX_SPEED } from '@/sim/consts'
import { SNOWMOBILE_WHEELS } from '@/sim/slip'
import { TWO_STROKE_TWIN } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_ALONG, type VehicleKind } from './kind'

export const SNOWMOBILE: VehicleKind = {
  kind: KIND_SNOWMOBILE,
  model: (id) => new CarModel('snowmobile', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'sled', board: 'E: drive the snowmobile', chase: true, gauges: ['air'] }, RIDE_ALONG] },
  sound: {
    chop: PLAIN_CHOP,
    // A belt drive has no gears: the revs climb once and hold while the speed builds, over the track's clatter.
    engine: { name: 'cvt', synth: TWO_STROKE_TWIN, idle: 1500, redline: 8200, spin: 0.35, shifts: [0], top: SNOWMOBILE_MAX_SPEED, level: 0.4, tracks: true },
  },
  fx: {
    wheels: SNOWMOBILE_WHEELS,
    drivenFrom: 2,
    exhaust: 'pipe',
    bay: [0, 0.1, -1.0],
    // A tall rooster tail straight back off the one narrow track.
    roost: { at: [[0, 1.65]], accel: [2, 10], width: 0.2, up: 7, power: 1 },
  },
}
