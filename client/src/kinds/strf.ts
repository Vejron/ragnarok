/** The Strf 90: a driver, a gunner on the turret's 40 mm autocannon, and two troops sealed in the back. */

import { KIND_STRF } from '@/net/codec'
import { STRF_DISTANCE } from '@/render/scene'
import { loadStrfModel, StrfModel } from '@/render/strf'
import { STRF_BARREL_MAX, STRF_BARREL_MIN, STRF_CANNON_COOLDOWN_TICKS, STRF_HALF_EXTENTS, STRF_MAX_SPEED, STRF_TRACK_HALF } from '@/sim/consts'
import { STRF_TRACKS } from '@/sim/slip'
import type { SeatView } from '@/seats'
import { DRIVE, type VehicleKind } from './kind'

/** A troop in the back: sealed in, sees out from behind the hull, and gets out by the ramp. */
const TROOP: SeatView = { rig: { name: 'passenger' }, role: 'passenger', board: 'E: ride in the back' }

export const STRF: VehicleKind = {
  kind: KIND_STRF,
  model: (id) => new StrfModel(id),
  load: loadStrfModel,
  ride: {
    distance: STRF_DISTANCE,
    reload: STRF_CANNON_COOLDOWN_TICKS,
    seats: [
      { rig: DRIVE, role: 'driver', board: 'E: drive the Strf 90', chase: true },
      // The turret turns all the way round; the gauges show the round loaded (AP or HE) and the barrel's heat.
      { rig: { name: 'gun', hull: [Math.PI, STRF_BARREL_MIN, STRF_BARREL_MAX] }, role: 'gunner', board: 'E: man the cannon', sight: true, gauges: ['ammo', 'heat'] },
      TROOP,
      TROOP,
    ],
  },
  // A diesel higher and busier than the tank's.
  sound: {
    voices: [['sawtooth', 1, 0.5], ['square', 0.5, 0.35], ['sawtooth', 2.02, 0.14]],
    q: 1.2,
    chop: { type: 'bandpass', hz: 720 },
    engine: { name: 'gears', shifts: [0], top: STRF_MAX_SPEED, hz: 38, level: 0.28, tracks: true },
  },
  fx: {
    wheels: STRF_TRACKS,
    drivenFrom: 0,
    exhaust: 'pipe',
    // The engine is in the front right, beside the driver.
    bay: [0.65, 0.75, -2.1],
    roost: { at: [[-STRF_TRACK_HALF, STRF_HALF_EXTENTS[2] + 0.1], [STRF_TRACK_HALF, STRF_HALF_EXTENTS[2] + 0.1]], accel: [0.5, 3], width: 0.32, up: 3.5, power: 1.1 },
  },
}
