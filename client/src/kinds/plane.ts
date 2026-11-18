/** The triplane: a pilot with forward guns and bombs, and a rear gunner. */

import { KIND_PLANE } from '@/net/codec'
import { loadPlaneModel, PlaneModel } from '@/render/plane'
import { PLANE_DISTANCE } from '@/render/scene'
import { PLANE_REAR_PITCH_MAX, PLANE_REAR_PITCH_MIN } from '@/sim/consts'
import { PLANE_WHEELS } from '@/sim/slip'
import { ROTARY_NINE } from './engines'
import { type VehicleKind } from './kind'

export const PLANE: VehicleKind = {
  kind: KIND_PLANE,
  model: (id) => new PlaneModel(id),
  load: loadPlaneModel,
  ride: {
    distance: PLANE_DISTANCE,
    // Its guns heat and jam rather than reload.
    reload: null,
    seats: [
      { rig: { name: 'aviator' }, role: 'aviator', board: 'E: fly the triplane', gauges: ['cockpit'], arc: 'bombs' },
      { rig: { name: 'gun', hull: [Math.PI, PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX] }, role: 'gunner', board: 'E: man the rear gun', gauges: ['cockpit'] },
    ],
  },
  sound: {
    // The propeller beats low.
    chop: { type: 'bandpass', hz: 220 },
    engine: { name: 'rotary', synth: ROTARY_NINE, idle: 600, redline: 1350, spin: 1.2, shifts: [0], top: 45, level: 0.5 },
  },
  fx: { wheels: PLANE_WHEELS, drivenFrom: null, exhaust: 'castor', bay: [0, 0.5, -1.8] },
}
