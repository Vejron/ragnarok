/** The field mortar: a towed gun, one seat for its crew. */

import { KIND_MORTAR } from '@/net/codec'
import { MortarModel } from '@/render/mortar'
import { MORTAR_DISTANCE } from '@/render/scene'
import { MORTAR_COOLDOWN_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const MORTAR: VehicleKind = {
  kind: KIND_MORTAR,
  model: (id) => new MortarModel(id),
  ride: { distance: MORTAR_DISTANCE, reload: MORTAR_COOLDOWN_TICKS, seats: [{ rig: { name: 'mortar' }, role: 'mortar', board: 'E: man the mortar', gauges: ['range'], arc: 'mortar' }] },
  // No engine at all.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // Its wheels mark the ground as a car's do while it is towed.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
