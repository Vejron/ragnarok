/** A submarine's guided missile: flown from its nose by the officer who fired it. */

import { KIND_MISSILE } from '@/net/codec'
import { MissileModel } from '@/render/missile'
import { LOITER_DISTANCE } from '@/render/scene'
import { MISSILE_RANGE, MISSILE_SPEED } from '@/sim/consts'
import { PLAIN, type VehicleKind } from './kind'

export const MISSILE: VehicleKind = {
  kind: KIND_MISSILE,
  model: (id) => new MissileModel(id),
  // Its look may be steep, to dive onto what it is flown at.
  ride: { distance: LOITER_DISTANCE, reload: null, seats: [{ rig: { name: 'fpv', pitch: [-1.4, 1.0], link: MISSILE_RANGE }, role: 'missile' }] },
  sound: {
    // A rocket motor: a roar of noise with a rough low note under it.
    voices: [['sawtooth', 1, 0.3], ['square', 1.49, 0.2], ['sawtooth', 0.5, 0.3]],
    ...PLAIN,
    chop: { type: 'bandpass', hz: 900 },
    engine: { name: 'twostroke', top: MISSILE_SPEED, hz: 60 },
  },
  fx: { wheels: null, drivenFrom: null, exhaust: 'rocket', bay: [0, 0, 0.8] },
}
