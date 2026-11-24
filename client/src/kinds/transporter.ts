/** The VW Transporter: seven seats and a flat four chattering under the back floor. */

import { KIND_TRANSPORTER } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { TRANSPORTER_MAX_SPEED, TRANSPORTER_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { TRANSPORTER_WHEELS } from '@/sim/slip'
import { BOXER_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const TRANSPORTER: VehicleKind = {
  kind: KIND_TRANSPORTER,
  model: (id) => new CarModel('transporter', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the VW Transporter', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: TRANSPORTER_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: BOXER_FOUR, idle: 800, redline: 4800, spin: 0.85, shifts: [0, 0.22, 0.46, 0.72], top: TRANSPORTER_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: TRANSPORTER_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.1, 1.7] },
}
