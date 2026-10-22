/** The FPV drone: flown through its camera from afar, by whoever launched it. */

import { KIND_DRONE } from '@/net/codec'
import { DroneModel } from '@/render/drone'
import { DRONE_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { DRONE_RANGE } from '@/sim/items'
import { JEEP_WHEELS } from '@/sim/slip'
import { PLAIN, type VehicleKind } from './kind'

export const DRONE: VehicleKind = {
  kind: KIND_DRONE,
  model: (id) => new DroneModel(id),
  ride: {
    distance: DRONE_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    downwash: { reach: 4, radius: 1.5 },
    seats: [{ rig: { name: 'fpv', pitch: [-1.4, 1.0], link: DRONE_RANGE }, role: 'drone' }],
  },
  sound: {
    // Four props, slightly out of step with each other, so the whine beats.
    voices: [['sawtooth', 1, 0.35], ['sawtooth', 1.013, 0.3], ['square', 2.01, 0.12]],
    ...PLAIN,
    engine: { name: 'props', hz: 190 },
  },
  // A car's wheel points, which touch only when it lands.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
