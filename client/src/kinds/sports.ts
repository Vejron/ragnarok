/** The sports car, an Opel Manta: four seats behind glass, rear-wheel drive, and a backfire when the driver lifts off. */

import { KIND_SPORTS } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, SPORTS_MAX_SPEED, SPORTS_SEATS } from '@/sim/consts'
import { SPORTS_WHEELS } from '@/sim/slip'
import { PETROL_FOUR } from './engines'
import { PLAIN_CHOP, DRIVE, RIDE_INSIDE, type VehicleKind } from './kind'

export const SPORTS: VehicleKind = {
  kind: KIND_SPORTS,
  model: (id) => new CarModel('sports', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the Opel Manta', chase: true, enclosed: true }, ...Array.from({ length: SPORTS_SEATS - 1 }, () => RIDE_INSIDE)] },
  sound: {
    chop: PLAIN_CHOP,
    // The GT/E's injected two-litre four, wound out hard: crackles off the throttle.
    engine: { name: 'gears', synth: { ...PETROL_FOUR, crackle: 0.6, pipe: 1.0 }, idle: 850, redline: 6500, spin: 0.5, shifts: [0, 0.17, 0.34, 0.53, 0.75], top: SPORTS_MAX_SPEED, level: 0.42 },
  },
  fx: { wheels: SPORTS_WHEELS, drivenFrom: 2, exhaust: 'pipe', backfire: true, bay: [0, 0.2, -1.6] },
}
