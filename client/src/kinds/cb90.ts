/**
 * The Stridsbåt 90: its driver steers it from the cockpit and fires the twin machine guns fixed on its
 * front, which only elevate, by turning the boat; its gunner stands in the ring behind and turns the
 * cannon all the way round; two riders on the after deck fire their own guns. It floats, and runs on its
 * waterjets (`Lift::Boat`, in the sim).
 */

import { KIND_CB90 } from '@/net/codec'
import { CB90_DISTANCE } from '@/render/scene'
import { Cb90Model, loadCb90Model } from '@/render/cb90'
import { CB90_AIM_BACK, CB90_AIM_UP, CB90_CANNON_COOLDOWN_TICKS, CB90_CANNON_PITCH_MAX, CB90_CANNON_PITCH_MIN, CB90_MAX_SPEED, CB90_MG_PITCH_MAX, CB90_MG_PITCH_MIN } from '@/sim/consts'
import { RIDE_ALONG, type VehicleKind } from './kind'

export const CB90: VehicleKind = {
  kind: KIND_CB90,
  model: (id) => new Cb90Model(id),
  load: loadCb90Model,
  ride: {
    distance: CB90_DISTANCE,
    reload: CB90_CANNON_COOLDOWN_TICKS,
    seats: [
      // Behind the machine guns, as a Strv S's driver is behind its gun: the boat aims them, and the view eases back behind them.
      {
        rig: { name: 'tanker', pitch: [CB90_MG_PITCH_MIN, CB90_MG_PITCH_MAX], eye: [CB90_AIM_BACK, CB90_AIM_UP] },
        role: 'coxswain',
        board: 'E: drive the Stridsbåt 90',
        chase: true,
        gauges: ['pintle'],
      },
      // The cannon turns all the way round; its gunner stands in the ring behind it.
      { rig: { name: 'gun', hull: [Math.PI, CB90_CANNON_PITCH_MIN, CB90_CANNON_PITCH_MAX] }, role: 'gunner', board: 'E: man the cannon', gauges: ['heat'], stand: true },
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
  // No wheels: it carves a wake, and its waterjets throw spray. Its engines are under the after deck.
  fx: { wheels: null, drivenFrom: null, exhaust: 'pipe', bay: [0, 0.6, 6], floats: true, jets: true },
}
