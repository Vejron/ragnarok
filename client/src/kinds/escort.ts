/** The Ford Escort: five seats in a small, slow hatchback. */

import { KIND_ESCORT } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { ESCORT_MAX_SPEED, ESCORT_SEATS, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { ESCORT_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const ESCORT: VehicleKind = {
  kind: KIND_ESCORT,
  model: (id) => new CarModel('escort', id),
  load: loadCarModels,
  ride: {
    distance: DRIVING_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Ford Escort', chase: true, enclosed: true }, ...Array.from({ length: ESCORT_SEATS - 1 }, () => RIDE_INSIDE)],
  },
  sound: {
    chop: PLAIN_CHOP,
    engine: { name: 'gears', synth: PETROL_FOUR, idle: 850, redline: 5600, spin: 0.65, shifts: [0, 0.2, 0.42, 0.66], top: ESCORT_MAX_SPEED, level: 0.4 },
  },
  fx: { wheels: ESCORT_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.2, -1.45] },
}
