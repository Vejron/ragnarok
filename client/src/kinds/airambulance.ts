/** The air ambulance: an unarmed helicopter's pilot and three passengers. */

import { KIND_AIR_AMBULANCE } from '@/net/codec'
import { AirAmbulanceModel, loadAirAmbulanceModel } from '@/render/airambulance'
import { AIR_AMBULANCE_DISTANCE } from '@/render/scene'
import type { SeatView } from '@/seats'
import { TURBOSHAFT } from './engines'
import { DRIVE, PLAIN_CHOP, type VehicleKind } from './kind'

/**
 * A passenger has nothing to aim, so they look round the whole helicopter as a driver does, rather
 * than over their own shoulder from inside its cabin.
 */
const RIDER: SeatView = { rig: DRIVE, role: 'rider', board: 'E: ride along', enclosed: true }

export const AIR_AMBULANCE: VehicleKind = {
  kind: KIND_AIR_AMBULANCE,
  model: () => new AirAmbulanceModel(),
  load: loadAirAmbulanceModel,
  ride: {
    distance: AIR_AMBULANCE_DISTANCE,
    // Nothing aboard fires, so there is no reload to show.
    reload: null,
    downwash: { reach: 20, radius: 7.5 },
    seats: [{ rig: { name: 'pilot' }, role: 'airPilot', board: 'E: fly the air ambulance', lever: true, gauges: ['rotor'], enclosed: true }, RIDER, RIDER, RIDER],
  },
  sound: {
    chop: PLAIN_CHOP,
    // Two small turbines, which wind the rotor up slowly and then hold its speed.
    engine: { name: 'rotor', synth: TURBOSHAFT, idle: 0, redline: 30000, spin: 3, shifts: [0], top: 70, level: 0.4 },
  },
  // No wheels: its rotor blows the dust up when it is low.
  fx: { wheels: null, drivenFrom: null, exhaust: null, bay: [0, 1.6, 1.1] },
}
