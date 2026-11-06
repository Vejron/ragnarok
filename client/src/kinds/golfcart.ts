/** The golf cart: two seats side by side, and an electric motor's quiet whine between the holes. */

import { KIND_GOLF_CART } from '@/net/codec'
import { CarModel, loadCarModels } from '@/render/car'
import { DRIVING_DISTANCE } from '@/render/scene'
import { GOLF_CART_MAX_SPEED, SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { GOLF_CART_WHEELS } from '@/sim/slip'
import { DRIVE, PLAIN, RIDE_ALONG, type VehicleKind } from './kind'

export const GOLF_CART: VehicleKind = {
  kind: KIND_GOLF_CART,
  model: (id) => new CarModel('golf_cart', id),
  load: loadCarModels,
  ride: { distance: DRIVING_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the golf cart', chase: true }, RIDE_ALONG] },
  sound: {
    // A small DC motor and its reduction gear: a thin whine that climbs with the speed, and soft.
    voices: [['sine', 1, 0.5], ['triangle', 3, 0.2], ['square', 5.02, 0.04]],
    ...PLAIN,
    engine: { name: 'gears', shifts: [0], top: GOLF_CART_MAX_SPEED, hz: 110, level: 0.06 },
  },
  fx: { wheels: GOLF_CART_WHEELS, drivenFrom: 2, exhaust: null, bay: [0, -0.1, 0.85] },
}
