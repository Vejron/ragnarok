/** A submarine's guided missile: flown from its nose by the officer who fired it. */

import { KIND_MISSILE } from '@/net/codec'
import { MissileModel } from '@/render/missile'
import { LOITER_DISTANCE } from '@/render/scene'
import { MISSILE_RANGE, MISSILE_SPEED } from '@/sim/consts'
import { ROCKET } from './engines'
import { type VehicleKind } from './kind'

export const MISSILE: VehicleKind = {
  kind: KIND_MISSILE,
  model: (id) => new MissileModel(id),
  // Its look may be steep, to dive onto what it is flown at.
  ride: { distance: LOITER_DISTANCE, reload: null, seats: [{ rig: { name: 'fpv', pitch: [-1.4, 1.0], link: MISSILE_RANGE }, role: 'missile' }] },
  sound: {
    chop: { type: 'bandpass', hz: 900 },
    engine: { name: 'flat', synth: ROCKET, idle: 0, redline: 6000, spin: 0.2, shifts: [0], top: MISSILE_SPEED, level: 0.5 },
  },
  fx: { wheels: null, drivenFrom: null, exhaust: 'rocket', bay: [0, 0, 0.8] },
}
