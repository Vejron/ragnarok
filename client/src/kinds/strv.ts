/** The Strv S: one seat, whose driver aims the hull's fixed gun by turning it. */

import { KIND_STRV } from '@/net/codec'
import { STRV_DISTANCE } from '@/render/scene'
import { loadStrvModel, StrvModel } from '@/render/strvs'
import { SHELL_COOLDOWN_TICKS, STRV_MAX_SPEED } from '@/sim/consts'
import { TRACK_WHEELS } from '@/sim/slip'
import type { VehicleKind } from './kind'

export const STRV: VehicleKind = {
  kind: KIND_STRV,
  model: (id) => new StrvModel(id),
  load: loadStrvModel,
  ride: { distance: STRV_DISTANCE, reload: SHELL_COOLDOWN_TICKS, seats: [{ rig: { name: 'tanker' }, role: 'strv', board: 'E: drive the Strv S', chase: true, sight: true }] },
  sound: {
    voices: [['sawtooth', 1, 0.5], ['square', 0.5, 0.3], ['sawtooth', 2.01, 0.15]],
    q: 1.2,
    chop: { type: 'bandpass', hz: 650 },
    engine: { name: 'gears', shifts: [0], top: STRV_MAX_SPEED, hz: 36, level: 0.3, tracks: true },
  },
  // Its tracks leave the tank's marks.
  fx: { wheels: TRACK_WHEELS, drivenFrom: 0, exhaust: 'pipe', bay: [0, 1.1, 1.8] },
}
