/** The Pak 38: a towed anti-tank gun, one gunner who lays it like a tank's gun and looks down its sight. */

import { KIND_PAK } from '@/net/codec'
import { PakModel } from '@/render/pak'
import { MORTAR_DISTANCE } from '@/render/scene'
import { PAK_PITCH_MAX, PAK_PITCH_MIN, PAK_TRAVERSE, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { PETROL, PLAIN, type VehicleKind } from './kind'

export const PAK: VehicleKind = {
  kind: KIND_PAK,
  model: (id) => new PakModel(id),
  ride: {
    distance: MORTAR_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: { name: 'gun', hull: [PAK_TRAVERSE, PAK_PITCH_MIN, PAK_PITCH_MAX] }, role: 'gunner', board: 'E: man the Pak 38', sight: true, gauges: ['ammo'] }],
  },
  // No engine at all.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // Its wheels mark the ground as a car's do while it is towed.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: null, bay: [0, 0, -0.7] },
}
