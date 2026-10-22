/** The hen: the walker's legs, with wings that leap, flap and glide, and eggs for bombs. */

import { KIND_HEN } from '@/net/codec'
import { HenModel, loadHenModel } from '@/render/hen'
import { HEN_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS } from '@/sim/consts'
import { JEEP_WHEELS } from '@/sim/slip'
import { DRIVE, PETROL, PLAIN, type VehicleKind } from './kind'

export const HEN: VehicleKind = {
  kind: KIND_HEN,
  model: (id) => new HenModel(id),
  load: loadHenModel,
  ride: { distance: HEN_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: DRIVE, role: 'hen', board: 'E: drive the hen', chase: true, gauges: ['air', 'eggs'] }] },
  // No engine to hum: its feet are heard by the footfalls.
  sound: { voices: PETROL, ...PLAIN, engine: { name: 'none' } },
  // A car's wheel points, high under its hull, which only touch when it lies fallen.
  fx: { wheels: JEEP_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 0.75, -1.35] },
}
