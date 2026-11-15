/** The Titan: one seat in its head, whose pilot walks it, moves its fists, fires the gun in its right fist and the missiles on its left shoulder. */

import { KIND_TITAN } from '@/net/codec'
import { TITAN_DISTANCE } from '@/render/scene'
import { loadTitanModel, TitanModel } from '@/render/titan'
import { TITAN_GUN_COOLDOWN } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const TITAN: VehicleKind = {
  kind: KIND_TITAN,
  model: (id) => new TitanModel(id),
  load: loadTitanModel,
  ride: { distance: TITAN_DISTANCE, reload: TITAN_GUN_COOLDOWN, seats: [{ rig: { name: 'titan' }, role: 'titan', board: 'E: pilot the Titan', gauges: ['air', 'heat', 'ammo', 'tubes'] }] },
  // No engine to hum: its feet are heard by the footfalls.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // A car's wheel points, far up under its torso, which only touch when it lies fallen. Its reactor is in its chest.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: null, bay: [0, 3.2, -4.5] },
}
