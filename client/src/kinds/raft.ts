/**
 * The raft: a small inflatable river-rafting boat for two. Its driver sits at the stern with a hand on
 * the big outboard's tiller; the other sits in the bow and fires their own gun. It floats, and the
 * outboard drives it (`Lift::Boat`, in the sim).
 */

import { KIND_RAFT } from '@/net/codec'
import { RaftModel } from '@/render/raft'
import { DRIVING_DISTANCE } from '@/render/scene'
import { RAFT_MAX_SPEED } from '@/sim/consts'
import { OUTBOARD } from './engines'
import { DRIVE, PLAIN_CHOP, RIDE_ALONG, type VehicleKind } from './kind'

export const RAFT: VehicleKind = {
  kind: KIND_RAFT,
  model: (id) => new RaftModel(id),
  ride: {
    distance: DRIVING_DISTANCE,
    reload: null,
    seats: [{ rig: DRIVE, role: 'driver', board: 'E: take the tiller', chase: true }, RIDE_ALONG],
  },
  sound: {
    chop: PLAIN_CHOP,
    // A big three-cylinder two-stroke outboard: a high, angry snarl that climbs as the raft gets up on the plane.
    engine: { name: 'marine', synth: OUTBOARD, idle: 900, redline: 5800, spin: 3, shifts: [0], top: RAFT_MAX_SPEED, level: 0.35 },
  },
  // No wheels: it carves a wake, and its propeller throws a rooster tail. Its motor is on the transom.
  fx: { wheels: null, drivenFrom: null, exhaust: 'pipe', bay: [0, 0.4, 2.0], floats: true, drive: 'outboard' },
}
