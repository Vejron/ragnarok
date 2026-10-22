/**
 * Protocol v2's scene as protocol v1's rows, so `game.ts` runs unchanged on v2 until Phase 4 gives
 * each component a presenter of its own. Mirrors `shared::replicated::v1_rows`; `adapter.spec.ts`
 * holds the two to the same bytes. What an entity is follows from its components: a `pickup`, a
 * `projectile`, an `orientation` (a vehicle), else a stickman or a creature.
 */

import { pickupState, rocketState, type PickupState, type RawEntity, type RawVehicle, type RocketState, type WireScene } from './codec'
import type { Components } from './components.generated'
import type { Scene } from './replicated'

/** v1's integer rows of `scene`, each list in id order, as v1 put them on the wire. */
export function v1Rows(scene: Scene): { entities: RawEntity[]; vehicles: RawVehicle[]; rockets: number[][]; pickups: number[][] } {
  const entities: RawEntity[] = []
  const vehicles: RawVehicle[] = []
  const rockets: number[][] = []
  const pickups: number[][] = []
  for (const [id, e] of [...scene].sort(([a], [b]) => a - b)) {
    const { x, y, z } = e.place ?? { x: 0, y: 0, z: 0 }
    const kind = e.kind?.kind ?? 0
    if (e.pickup) pickups.push([id, e.pickup.item, x, y, z])
    else if (e.projectile) {
      const { vx, vy, vz } = e.velocity ?? { vx: 0, vy: 0, vz: 0 }
      rockets.push([id, e.projectile.kind, x, y, z, vx, vy, vz])
    } else if (e.orientation) vehicles.push(vehicleRow(id, kind, x, y, z, e))
    else {
      const motion = e.motion ?? { vy: 0, knock_x: 0, knock_z: 0 }
      const flags = e.flags ?? { flags: 0, stance: 0 }
      entities.push([id, kind, x, y, z, e.heading?.yaw ?? 0, motion.vy, flags.flags, flags.stance, motion.knock_x, motion.knock_z])
    }
  }
  return { entities, vehicles, rockets, pickups }
}

function vehicleRow(id: number, kind: number, x: number, y: number, z: number, e: Components): RawVehicle {
  const { qx, qy, qz } = e.orientation!
  const t = e.turret
  const a = e.arms
  return [
    id, kind, x, y, z, qx, qy, qz,
    e.steer?.steer ?? 0,
    e.seats?.riders ?? [],
    t ? [t.yaw, t.pitch, t.shots, t.range, t.flags] : null,
    e.health?.hp ?? 0,
    e.damage?.parts ?? null,
    a ? [a.shots, a.heat, a.rear_heat, a.bombs, a.flags] : null,
    e.thrust ? [e.thrust.air, e.thrust.flags] : null,
    (e.crew?.riders ?? []).map((rider) => [rider.yaw, rider.pitch, rider.held]),
  ]
}

/** What `game.ts` takes from a v1 snapshot's scene, built from a v2 one. */
export function v1Scene(scene: Scene): { wire: WireScene; rockets: RocketState[]; pickups: PickupState[] } {
  const { entities, vehicles, rockets, pickups } = v1Rows(scene)
  return { wire: { entities, vehicles }, rockets: rockets.map(rocketState), pickups: pickups.map(pickupState) }
}
