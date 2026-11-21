/** The helicopter: a pilot with rocket pods, and a gunner on the nose gun. */

import { KIND_HELI } from '@/net/codec'
import { HeliModel, loadHeliModel } from '@/render/heli'
import { HELI_DISTANCE } from '@/render/scene'
import { HELI_GUN_COOLDOWN_TICKS, HELI_GUN_PITCH_MAX, HELI_GUN_PITCH_MIN, HELI_GUN_YAW_LIMIT } from '@/sim/consts'
import { TURBOSHAFT } from './engines'
import { PLAIN_CHOP, type VehicleKind } from './kind'

export const HELI: VehicleKind = {
  kind: KIND_HELI,
  model: (id) => new HeliModel(id),
  load: loadHeliModel,
  ride: {
    distance: HELI_DISTANCE,
    reload: HELI_GUN_COOLDOWN_TICKS,
    downwash: { reach: 18, radius: 6 },
    seats: [{ rig: { name: 'pilot' }, role: 'pilot', board: 'E: fly', lever: true, gauges: ['rotor'], enclosed: true }, { rig: { name: 'gun', hull: [HELI_GUN_YAW_LIMIT, HELI_GUN_PITCH_MIN, HELI_GUN_PITCH_MAX] }, role: 'gunner', board: 'E: man the nose gun', enclosed: true }],
  },
  sound: {
    chop: PLAIN_CHOP,
    // The turbine spools up slowly once the pilot is in, then holds its speed.
    engine: { name: 'rotor', synth: TURBOSHAFT, idle: 0, redline: 30000, spin: 3, shifts: [0], top: 60, level: 0.45 },
  },
  // No wheels: its rotor blows the dust up when it is low.
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 1.6, 0.4] },
}
