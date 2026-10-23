/** The triplane: a pilot with forward guns and bombs, and a rear gunner. */

import { KIND_PLANE } from '@/net/codec'
import { loadPlaneModel, PlaneModel } from '@/render/plane'
import { PLANE_DISTANCE } from '@/render/scene'
import { PLANE_REAR_PITCH_MAX, PLANE_REAR_PITCH_MIN } from '@/sim/consts'
import { PLANE_WHEELS } from '@/sim/slip'
import type { VehicleKind } from './kind'

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
    voices: [['square', 1, 0.4], ['sawtooth', 2, 0.22], ['square', 0.5, 0.3], ['sawtooth', 4.02, 0.08]],
    // A rotary's buzz is in its harmonics, with a nasal peak on them; the propeller beats low.
    q: 2.5,
    chop: { type: 'bandpass', hz: 220 },
    engine: { name: 'rotary', top: 45, hz: 85, level: 0.28 },
  },
  fx: { wheels: PLANE_WHEELS, drivenFrom: null, exhaust: 'castor', bay: [0, 0.5, -1.8] },
}
