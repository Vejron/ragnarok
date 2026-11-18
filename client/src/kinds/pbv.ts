/**
 * The Pbv 302: a driver, a gunner on the turret's 20 mm automatic cannon, and two troops sealed in the
 * back. It floats, and paddles across open water on its tracks (`Spec::swim`, in the sim).
 */

import { KIND_PBV } from '@/net/codec'
import { PBV_DISTANCE } from '@/render/scene'
import { loadPbvModel, PbvModel } from '@/render/pbv'
import { PBV_BARREL_MAX, PBV_BARREL_MIN, PBV_CANNON_COOLDOWN_TICKS, PBV_HALF_EXTENTS, PBV_MAX_SPEED, PBV_TRACK_HALF } from '@/sim/consts'
import { PBV_TRACKS } from '@/sim/slip'
import type { SeatView } from '@/seats'
import { DIESEL_SIX } from './engines'
import { DRIVE, type VehicleKind } from './kind'

/** A troop in the back: sealed in, sees out from behind the hull, and gets out by the rear doors. */
const TROOP: SeatView = { rig: { name: 'passenger' }, role: 'passenger', board: 'E: ride in the back' }

export const PBV: VehicleKind = {
  kind: KIND_PBV,
  model: (id) => new PbvModel(id),
  load: loadPbvModel,
  ride: {
    distance: PBV_DISTANCE,
    reload: PBV_CANNON_COOLDOWN_TICKS,
    seats: [
      { rig: DRIVE, role: 'driver', board: 'E: drive the Pbv 302', chase: true },
      // The turret turns all the way round; the gauge shows the barrel's heat.
      { rig: { name: 'gun', hull: [Math.PI, PBV_BARREL_MIN, PBV_BARREL_MAX] }, role: 'gunner', board: 'E: man the cannon', sight: true, gauges: ['heat'] },
      TROOP,
      TROOP,
    ],
  },
  // A diesel a little lower than the Strf 90's.
  sound: {
    chop: { type: 'bandpass', hz: 660 },
    engine: { name: 'gears', synth: DIESEL_SIX, idle: 650, redline: 2600, spin: 1.4, shifts: [0, 0.25, 0.5, 0.75], top: PBV_MAX_SPEED, level: 0.48, tracks: true },
  },
  fx: {
    wheels: PBV_TRACKS,
    drivenFrom: 0,
    exhaust: 'pipe',
    // The engine is under the roof grille behind the driver.
    bay: [0.1, 0.7, -0.3],
    roost: { at: [[-PBV_TRACK_HALF, PBV_HALF_EXTENTS[2] + 0.1], [PBV_TRACK_HALF, PBV_HALF_EXTENTS[2] + 0.1]], accel: [0.5, 3], width: 0.3, up: 3.5, power: 1 },
  },
}
