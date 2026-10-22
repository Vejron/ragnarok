import * as THREE from 'three'
import { damageSmoke } from '@/render/vehiclesmoke'
import { vehicleMaxHp } from '@/sim/consts'
import type { Frame, Presenter } from '../presenter'
import type { Entity } from '../units'
import type { VehicleWorld, Vehicles } from './vehicles'

/** A vehicle with no motion on record. */
const STILL = new THREE.Vector3()

/**
 * A plume from a hurt vehicle (`health`): thin steam under 40% health, black smoke and flame as it
 * dies, and a burning column, lighting what is round it, once it is a wreck.
 */
export class Smoke implements Presenter<Frame> {
  /** The unspent fraction of a puff, per vehicle, carried between frames. */
  private readonly owed = new Map<number, number>()

  constructor(private readonly world: VehicleWorld, private readonly vehicles: Vehicles) {}

  wants(entity: Entity) {
    return entity.health !== undefined && entity.orientation !== undefined
  }

  update(id: number, _: unknown, frame: Frame) {
    const placed = this.vehicles.placed.get(id)
    if (!placed) return
    const { kind, hp } = placed
    const limit = vehicleMaxHp(kind) * 0.4
    if (hp >= limit) {
      this.owed.delete(id)
      return
    }
    const position = [placed.x, placed.y, placed.z] as const
    const owed = damageSmoke(this.world.fx(), kind, position, new THREE.Quaternion(...placed.rotation), this.vehicles.fx(id)?.velocity ?? STILL, 1 - hp / limit, hp === 0, frame.dt, this.owed.get(id) ?? 0)
    this.owed.set(id, owed)
    // A burning wreck lights its surroundings with a flicker. Each light lives a fraction of a second, so one per frame holds it.
    if (hp === 0) {
      const flicker = 0.7 + 0.3 * Math.sin(frame.now * 0.021 + id * 1.7) * Math.sin(frame.now * 0.0073 + id)
      this.world.lights.flash(placed.x, placed.y + 1.5, placed.z, { color: 0xff7a24, intensity: 2.2 * flicker, radius: 16, life: 0.08 })
    }
  }

  remove(id: number) {
    this.owed.delete(id)
  }
}
