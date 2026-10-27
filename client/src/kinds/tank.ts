/** The tank: a driver, and a gunner in the turret who looks down the gun's sight. */

import { KIND_TANK } from '@/net/codec'
import { loadTankModel, TankModel } from '@/render/tank'
import { TANK_DISTANCE } from '@/render/scene'
import { SHELL_COOLDOWN_TICKS, TANK_BARREL_MAX, TANK_BARREL_MIN, TANK_MAX_SPEED } from '@/sim/consts'
import { TRACK_WHEELS } from '@/sim/slip'
import { DRIVE, type VehicleKind } from './kind'

export const TANK: VehicleKind = {
  kind: KIND_TANK,
  model: (id) => new TankModel(id),
  load: loadTankModel,
  ride: {
    distance: TANK_DISTANCE,
    reload: SHELL_COOLDOWN_TICKS,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: drive the tank', chase: true }, { rig: { name: 'gun', level: [TANK_BARREL_MIN, TANK_BARREL_MAX] }, role: 'gunner', board: 'E: man the gun', sight: true, gauges: ['ammo'] }],
  },
  sound: {
    voices: [['sawtooth', 1, 0.5], ['square', 0.5, 0.4], ['sawtooth', 1.51, 0.12]],
    q: 1.2,
    chop: { type: 'bandpass', hz: 650 },
    engine: { name: 'gears', shifts: [0], top: TANK_MAX_SPEED, hz: 30, level: 0.3, tracks: true },
  },
  fx: { wheels: TRACK_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 1.3, 1.6] },
}
