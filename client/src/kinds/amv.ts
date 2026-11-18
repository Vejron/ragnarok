/**
 * The AMV XA-361: an eight-wheeled armoured mortar carrier. A driver, a mortar operator under the turret
 * who lays its two tubes with the range dial as a field mortar's gunner does, and a rider in the back, all
 * sealed in.
 */

import { KIND_AMV } from '@/net/codec'
import { AmvModel } from '@/render/amv'
import { loadCarModels } from '@/render/car'
import { AMV_DISTANCE } from '@/render/scene'
import { AMV_COOLDOWN_TICKS, AMV_MAX_SPEED, AMV_RELOAD_TICKS } from '@/sim/consts'
import { AMV_WHEELS } from '@/sim/slip'
import { DIESEL_SIX } from './engines'
import { DRIVE, type VehicleKind } from './kind'

export const AMV: VehicleKind = {
  kind: KIND_AMV,
  model: (id) => new AmvModel(id),
  load: loadCarModels,
  ride: {
    distance: AMV_DISTANCE,
    // A third of a second between the two tubes, then three seconds to load both.
    reload: AMV_COOLDOWN_TICKS,
    refill: AMV_RELOAD_TICKS,
    seats: [
      { rig: DRIVE, role: 'driver', board: 'E: drive the AMV', chase: true },
      { rig: { name: 'mortar' }, role: 'mortar', board: 'E: man the mortar', gauges: ['range'], arc: 'mortar' },
      { rig: { name: 'passenger' }, role: 'passenger', board: 'E: ride in the back' },
    ],
  },
  // A big diesel truck: low, with a hard knock.
  sound: {
    chop: { type: 'lowpass', hz: 300 },
    engine: { name: 'gears', synth: { ...DIESEL_SIX, whine: 0.2, whineRatio: 22 }, idle: 600, redline: 2300, spin: 1.4, shifts: [0, 0.2, 0.42, 0.68], top: AMV_MAX_SPEED, level: 0.46 },
  },
  fx: { wheels: AMV_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0.6, 0.9, -2.9] },
}
