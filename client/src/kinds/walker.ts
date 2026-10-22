/** The walker: one seat, whose driver aims its chin guns. */

import { KIND_WALKER } from '@/net/codec'
import { WALKER_DISTANCE } from '@/render/scene'
import { loadWalkerModel, WalkerModel } from '@/render/walker'
import { WALKER_GUN_COOLDOWN_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const WALKER: VehicleKind = {
  kind: KIND_WALKER,
  model: (id) => new WalkerModel(id),
  load: loadWalkerModel,
  ride: { distance: WALKER_DISTANCE, reload: WALKER_GUN_COOLDOWN_TICKS, seats: [{ rig: { name: 'walker' }, role: 'walker', board: 'E: drive the walker', chase: true }] },
  // No engine to hum: its feet are heard by the footfalls.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // A car's wheel points, high under its hull, which only touch when it lies fallen.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
