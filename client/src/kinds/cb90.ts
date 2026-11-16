/**
 * The Stridsbåt 90: its driver steers it and aims the twin bow guns with the look, its gunner lays the
 * cannon in the roof turret all the way round, and two riders on the after deck fire their own guns. It
 * floats, and runs on its waterjets (`Lift::Boat`, in the sim).
 */

import { KIND_CB90 } from '@/net/codec'
import { CB90_DISTANCE } from '@/render/scene'
import { Cb90Model, loadCb90Model } from '@/render/cb90'
import { CB90_AIM_BACK, CB90_AIM_UP, CB90_CANNON_COOLDOWN_TICKS, CB90_CANNON_PITCH_MAX, CB90_CANNON_PITCH_MIN, CB90_MAX_SPEED, CB90_MG_PITCH_MAX, CB90_MG_PITCH_MIN, CB90_MG_YAW_LIMIT } from '@/sim/consts'
import { RIDE_ALONG, type VehicleKind } from './kind'

export const CB90: VehicleKind = {
  kind: KIND_CB90,
  model: (id) => new Cb90Model(id),
  load: loadCb90Model,
  ride: {
    distance: CB90_DISTANCE,
    reload: CB90_CANNON_COOLDOWN_TICKS,
    seats: [
      // The camera sits behind the bow guns and over the wheelhouse, and the guns follow the look while W/S/A/D steer.
      {
        rig: { name: 'pintle', limit: CB90_MG_YAW_LIMIT, pitch: [CB90_MG_PITCH_MIN, CB90_MG_PITCH_MAX], hull: true, eye: [CB90_AIM_BACK, CB90_AIM_UP] },
        role: 'coxswain',
        board: 'E: drive the Stridsbåt 90',
        chase: true,
        gauges: ['pintle'],
      },
      // The turret turns all the way round; the gauge shows the barrel's heat.
      { rig: { name: 'gun', hull: [Math.PI, CB90_CANNON_PITCH_MIN, CB90_CANNON_PITCH_MAX] }, role: 'gunner', board: 'E: man the cannon', sight: true, gauges: ['heat'] },
      RIDE_ALONG,
      RIDE_ALONG,
    ],
  },
  // Two big marine diesels: a deep, even drone.
  sound: {
    voices: [['sawtooth', 1, 0.45], ['square', 0.5, 0.4], ['sawtooth', 2.01, 0.14]],
    q: 1.1,
    chop: { type: 'bandpass', hz: 500 },
    engine: { name: 'gears', shifts: [0], top: CB90_MAX_SPEED, hz: 32, level: 0.3 },
  },
  // No wheels: it carves a wake. Its engines are under the after deck.
  fx: { wheels: null, drivenFrom: null, exhaust: 'pipe', bay: [0, 0.6, 6], floats: true },
}
