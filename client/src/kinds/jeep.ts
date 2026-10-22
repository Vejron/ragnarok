/** The jeep: four seats, its passengers firing their own guns. */

import { KIND_JEEP } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { JEEP_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { DRIVE, PETROL, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

export const JEEP: VehicleKind = {
  kind: KIND_JEEP,
  model: (id) => new CarModel('jeep', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the jeep', chase: true }, RIDE_ALONG, RIDE_ALONG, RIDE_ALONG] },
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'gears', shifts: [0, 0.22, 0.45, 0.7], top: JEEP_MAX_SPEED, hz: 44, level: 0.22 } },
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
