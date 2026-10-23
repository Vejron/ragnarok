/** The loitering drone: flown from afar through its nose camera or its belly camera, always cruising. */

import { KIND_LOITER } from '@/net/codec'
import { LoiterModel } from '@/render/loiter'
import { LOITER_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { LOITER_RANGE } from '@/sim/items'
import { JEEP_WHEELS } from '@/sim/slip'
import { PLAIN, type VehicleKind } from './kind'

export const LOITER: VehicleKind = {
  kind: KIND_LOITER,
  model: (id) => new LoiterModel(id),
  // Its look is kept steep enough to dive, not to loop.
  ride: { distance: LOITER_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: { name: 'fpv', pitch: [-1.2, 0.6], link: LOITER_RANGE, belly: true }, role: 'loiter' }] },
  sound: {
    // A small two-stroke: a rasping buzz, its cylinders a little out of step.
    voices: [['sawtooth', 1, 0.5], ['sawtooth', 1.021, 0.35], ['square', 2, 0.2], ['square', 0.5, 0.25]],
    ...PLAIN,
    engine: { name: 'twostroke', top: 45, hz: 80 },
  },
  // A car's wheel points, which touch only when it comes down.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
