/** The tupp: a hen's legs and wings, and a head that turns to aim its laser eyes. */

import { KIND_TUPP } from '@/net/codec'
import { HEN_DISTANCE } from '@/render/scene'
import { loadTuppModel, TuppModel } from '@/render/tupp'
import { TUPP_LASER_RECOVER_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const TUPP: VehicleKind = {
  kind: KIND_TUPP,
  model: (id) => new TuppModel(id),
  load: loadTuppModel,
  ride: { distance: HEN_DISTANCE, reload: TUPP_LASER_RECOVER_TICKS, seats: [{ rig: { name: 'walker' }, role: 'tupp', board: 'E: drive the tupp', chase: true, gauges: ['air'] }] },
  // No engine to hum: its feet are heard by the footfalls.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // A car's wheel points, high under its hull, which only touch when it lies fallen.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
