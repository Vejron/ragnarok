/** The FPV drone: flown through its camera from afar, by whoever launched it. */

import { KIND_DRONE } from '@/net/codec'
import { DroneModel } from '@/render/drone'
import { DRONE_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { DRONE_RANGE } from '@/sim/items'
import { JEEP_WHEELS } from '@/sim/slip'
import { DRONE_MOTORS } from './engines'
import { PLAIN_CHOP, type VehicleKind } from './kind'

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
    chop: PLAIN_CHOP,
    engine: { name: 'props', synth: DRONE_MOTORS, idle: 6000, redline: 12000, spin: 0.3, shifts: [0], top: 30, level: 0.2 },
  },
  // A car's wheel points, which touch only when it lands.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
