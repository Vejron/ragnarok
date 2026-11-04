/** A ship's gun: fixed to her deck, laid by one gunner who looks along it, and firing round shot. */

import { KIND_SHIP_GUN } from '@/net/codec'
import { ShipGunModel } from '@/render/shipgun'
import { MORTAR_DISTANCE } from '@/render/scene'
import { SHIP_GUN_COOLDOWN_TICKS, SHIP_GUN_PITCH_MAX, SHIP_GUN_PITCH_MIN, SHIP_GUN_TRAVERSE } from '@/sim/consts'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const SHIP_GUN: VehicleKind = {
  kind: KIND_SHIP_GUN,
  model: (id) => new ShipGunModel(id),
  ride: {
    distance: MORTAR_DISTANCE,
    reload: SHIP_GUN_COOLDOWN_TICKS,
    seats: [{ rig: { name: 'gun', hull: [SHIP_GUN_TRAVERSE, SHIP_GUN_PITCH_MIN, SHIP_GUN_PITCH_MAX] }, role: 'gunner', board: 'E: man the gun', sight: true }],
  },
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 0, 0] },
}
