/** The seaplane: an amphibious biplane on pontoons, armed as the triplane is. */

import { KIND_SEAPLANE } from '@/net/codec'
import { loadSeaplaneModel, SeaplaneModel } from '@/render/seaplane'
import { PLANE_DISTANCE } from '@/render/scene'
import { PLANE_REAR_PITCH_MAX, PLANE_REAR_PITCH_MIN, SEAPLANE_KEEL, SEAPLANE_STALL_SPEED } from '@/sim/consts'
import { SEAPLANE_WHEELS } from '@/sim/slip'
import type { VehicleKind } from './kind'

export const SEAPLANE: VehicleKind = {
  kind: KIND_SEAPLANE,
  model: (id) => new SeaplaneModel(id),
  load: loadSeaplaneModel,
  ride: {
    // It is a little bigger than the triplane, and stands taller on its pontoons.
    distance: PLANE_DISTANCE + 1.5,
    // Its guns heat and jam rather than reload.
    reload: null,
    stall: SEAPLANE_STALL_SPEED,
    seats: [
      { rig: { name: 'aviator' }, role: 'aviator', board: 'E: fly the seaplane', gauges: ['cockpit'], arc: 'bombs' },
      { rig: { name: 'gun', hull: [Math.PI, PLANE_REAR_PITCH_MIN, PLANE_REAR_PITCH_MAX] }, role: 'gunner', board: 'E: man the rear gun', gauges: ['cockpit'] },
    ],
  },
  sound: {
    voices: [['square', 1, 0.4], ['sawtooth', 2, 0.24], ['square', 0.5, 0.34], ['sawtooth', 3.01, 0.08]],
    // A radial turning slower than the triplane's rotary: a deeper drone, and the propeller beats low.
    q: 2.2,
    chop: { type: 'bandpass', hz: 180 },
    engine: { name: 'rotary', top: 38, hz: 68, level: 0.3 },
  },
  fx: { wheels: SEAPLANE_WHEELS, drivenFrom: null, exhaust: 'pipe', bay: [0, 0.5, -2.2], keel: SEAPLANE_KEEL },
}
