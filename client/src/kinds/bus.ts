/** The bus: a driver at the front, who on a world with bus lines is the server's, and twelve passengers. */

import { KIND_BUS } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { BUS_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { BUS_WHEELS } from '@/sim/slip'
import { DRIVE, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

export const BUS: VehicleKind = {
  kind: KIND_BUS,
  model: (id) => new CarModel('bus', id),
  load: loadCarModels,
  // Twelve metres of bus want the camera further back than a car does. Every seat past the driver's is a passenger's.
  ride: { distance: DRIVING_DISTANCE * 2, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the bus', chase: true }, { ...RIDE_ALONG, board: 'E: board the bus' }] },
  sound: {
    // A big diesel at the back: a low drone, slow through its gears.
    voices: [['sawtooth', 1, 0.5], ['square', 0.5, 0.5], ['triangle', 2.01, 0.15]],
    ...PLAIN,
    engine: { name: 'gears', shifts: [0, 0.22, 0.48, 0.74], top: BUS_MAX_SPEED, hz: 28, level: 0.22 },
  },
  fx: { wheels: BUS_WHEELS, drivenFrom: 2, exhaust: 'pipe', bay: [0, 0.0, 5.3] },
}
