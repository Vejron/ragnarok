/** The helicopter: a pilot with rocket pods, and a gunner on the nose gun. */

import { KIND_HELI } from '@/net/codec'
import { HeliModel, loadHeliModel } from '@/render/heli'
import { HELI_DISTANCE } from '@/render/scene'
import { HELI_GUN_COOLDOWN_TICKS, HELI_GUN_PITCH_MAX, HELI_GUN_PITCH_MIN, HELI_GUN_YAW_LIMIT } from '@/sim/consts'
import { DRIVE, PLAIN, type VehicleKind } from './kind'

export const HELI: VehicleKind = {
  kind: KIND_HELI,
  model: (id) => new HeliModel(id),
  load: loadHeliModel,
  ride: {
    distance: HELI_DISTANCE,
    reload: HELI_GUN_COOLDOWN_TICKS,
    downwash: { reach: 18, radius: 6 },
    seats: [{ rig: DRIVE, role: 'pilot', board: 'E: fly' }, { rig: { name: 'gun', hull: [HELI_GUN_YAW_LIMIT, HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX] }, role: 'gunner', board: 'E: man the nose gun' }],
  },
  sound: { voices: [['sawtooth', 1, 0.5], ['square', 0.5, 0.3]], ...PLAIN, engine: { name: 'rotor', hz: 40, level: 0.3 } },
  // No wheels: its rotor blows the dust up when it is low.
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 1.6, 0.4] },
}
