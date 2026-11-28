/** The police van: a Transporter in police white and blue with a light bar, its seven seats and its flat four. */

import { KIND_POLICE_VAN } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { POLICE_VAN_MAX_SPEED, POLICE_VAN_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { POLICE_VAN_WHEELS } from '@/sim/slip'
import { BOXER_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const POLICE_VAN: VehicleKind = {
  kind: KIND_POLICE_VAN,
  model: (id) => new CarModel('police_van', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the police van', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: POLICE_VAN_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: BOXER_FOUR, idle: 800, redline: 4800, spin: 0.85, shifts: [0, 0.22, 0.46, 0.72], top: POLICE_VAN_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: POLICE_VAN_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.1, 1.7] },
}
