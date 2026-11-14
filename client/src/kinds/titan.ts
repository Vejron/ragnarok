/** The Titan: one seat in its head, whose pilot walks it and moves its fists. */

import { KIND_TITAN } from '@/net/codec'
import { TITAN_DISTANCE } from '@/render/scene'
import { loadTitanModel, TitanModel } from '@/render/titan'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const TITAN: VehicleKind = {
  kind: KIND_TITAN,
  model: (id) => new TitanModel(id),
  load: loadTitanModel,
  ride: { distance: TITAN_DISTANCE, reload: null, seats: [{ rig: { name: 'titan' }, role: 'titan', board: 'E: pilot the Titan', gauges: ['air'] }] },
  // No engine to hum: its feet are heard by the footfalls.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // A car's wheel points, far up under its torso, which only touch when it lies fallen. Its reactor is in its chest.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: null, bay: [0, 3.2, -4.5] },
}
