/** The ambulance: a Transporter in white and yellow with a light bar, its seven seats and its flat four. */

import { KIND_AMBULANCE } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { AMBULANCE_MAX_SPEED, AMBULANCE_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { AMBULANCE_WHEELS } from '@/sim/slip'
import { BOXER_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const AMBULANCE: VehicleKind = {
  kind: KIND_AMBULANCE,
  model: (id) => new CarModel('ambulance', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the ambulance', chase: true, gauges: ['boost'], enclosed: true }, ...Array.from({ length: AMBULANCE_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: BOXER_FOUR, idle: 800, redline: 4800, spin: 0.85, shifts: [0, 0.22, 0.46, 0.72], top: AMBULANCE_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: AMBULANCE_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.1, 1.7] },
}
