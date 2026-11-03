/** The snowmobile: two riders one behind the other, a gun in tow, and a two-stroke's scream up any slope. */

import { KIND_SNOWMOBILE } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, SNOWMOBILE_MAX_SPEED } from '@/sim/consts'
import { SNOWMOBILE_WHEELS } from '@/sim/slip'
import { DRIVE, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

export const SNOWMOBILE: VehicleKind = {
  kind: KIND_SNOWMOBILE,
  model: (id) => new CarModel('snowmobile', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'sled', board: 'E: drive the snowmobile', chase: true, gauges: ['air'] }, RIDE_ALONG] },
  sound: {
    voices: [['sawtooth', 1, 0.4], ['square', 2, 0.22], ['sawtooth', 0.5, 0.18]],
    ...PLAIN,
    // A belt drive has no gears: the revs climb once and hold while the speed builds, over the track's clatter.
    engine: { name: 'gears', shifts: [0], top: SNOWMOBILE_MAX_SPEED, hz: 70, level: 0.2, tracks: true },
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
