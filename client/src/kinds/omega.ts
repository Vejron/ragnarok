/** The Opel Omega: five seats and a smooth six that cruises fast. */

import { KIND_OMEGA } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { OMEGA_MAX_SPEED, OMEGA_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { OMEGA_WHEELS } from '@/sim/slip'
import { PETROL_SIX } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const OMEGA: VehicleKind = {
  kind: KIND_OMEGA,
  model: (id) => new CarModel('omega', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Opel Omega', chase: true, enclosed: true }, ...Array.from({ length: OMEGA_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_SIX, idle: 750, redline: 5800, spin: 0.6, shifts: [0, 0.18, 0.38, 0.6, 0.82], top: OMEGA_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: OMEGA_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.2, -1.65] },
}
