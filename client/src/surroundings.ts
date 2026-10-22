/**
 * What people and vehicles do to the ground and water around them, drawn for everyone alike: splashes
 * and wading ripples, the water plane mirrored near the camera, grass parted underfoot and flattened by
 * rotors, and contact shadows.
 */

import type { Effects } from '@/render/effects'
import type { Grass, Bender } from '@/render/grass'
import type { Scene } from '@/render/scene'
import { splash } from '@/render/splash'
import type { FxWorld } from '@/render/tyres'
import type { Placed } from '@/entities/presenters/vehicles'
import { rideView } from '@/seats'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS } from '@/sim/consts'
import { halfExtentsOf } from '@/sim/kinds'
import type { Waters } from '@/sim/waters'
import { NO_WATER } from '@/world'

/** Feet this deep (metres) under the surface are in the water. */
const WADE_DEPTH = 0.15
/** Wading ripples are drawn within this many metres of the camera. */
const WADE_RIPPLE_RANGE = 60

/** A point on the ground plane. */
interface At {
  x: number
  y: number
  z: number
}

export class Surroundings {
  waters: Waters | null = null
  grass: Grass | null = null
  private readonly wading = new Map<number, { wet: boolean; ripple: number }>()
  private readonly benders: Bender[] = []

  constructor(
    private readonly scene: Scene,
    private readonly effects: Effects,
    private readonly fx: () => FxWorld,
  ) {}

  /**
   * Splashes where something of about `size` metres struck, when `at` is at or under the water surface
   * (the server reports the carved bed). Returns whether it did.
   */
  watery(at: At, size: number, speed = 10): boolean {
    const level = this.waters?.levelAt(at.x, at.z) ?? NO_WATER
    if (level === NO_WATER || at.y > level + 0.05) return false
    splash(this.fx(), { x: at.x, y: level, z: at.z }, size, speed)
    return true
  }

  /** Picks the water level to mirror this frame, if any water is about the camera. */
  mirrorWater() {
    const camera = this.scene.camera.position
    const look = this.scene.camera.getWorldDirection({ x: 0, y: 0, z: 0 })
    const level = this.waters?.planeNear(camera.x, camera.z, look.x, look.z) ?? null
    this.scene.setWaterPlane(level)
  }

  /** Splashes when person `key`'s feet go into water, and ripples while they wade. `speed` is how fast they move, m/s. */
  wade(key: number, x: number, y: number, z: number, speed: number, dt: number) {
    const level = this.waters?.levelAt(x, z) ?? NO_WATER
    const wet = level !== NO_WATER && level - y > WADE_DEPTH
    const was = this.wading.get(key)
    if (!was) {
      // First seen: already wet is not a splash.
      this.wading.set(key, { wet, ripple: 0 })
      return
    }
    if (wet && !was.wet) splash(this.fx(), { x, y: level, z }, 0.45, 2 + Math.min(speed, 8))
    else if (!wet && was.wet && speed > 1) splash(this.fx(), { x, y: level === NO_WATER ? y : level, z }, 0.3, 1)
    if (wet && speed > 1) {
      was.ripple -= dt
      if (was.ripple <= 0 && Math.hypot(this.scene.camera.position.x - x, this.scene.camera.position.z - z) < WADE_RIPPLE_RANGE) {
        this.effects.ripple({ x, y: level + 0.02, z }, 0.3)
        was.ripple = 0.35
      }
    }
    was.wet = wet
  }

  /** Person `key` is no longer drawn, or is out of the water's reach (seated, dead). */
  dry(key: number) {
    this.wading.delete(key)
  }

  /** A contact shadow under a stickman whose capsule is centred at `(x, y, z)`. */
  blobFoot(x: number, y: number, z: number) {
    const height = y - (CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS) - this.scene.groundAt(x, z)
    this.scene.contact.add(x, z, 0.6, 0.6, 0, Math.max(0, height))
  }

  /** Contact shadows under every vehicle, as wide as its chassis and fading with height. */
  blobVehicles(cars: ReadonlyMap<number, Placed>) {
    for (const car of cars.values()) {
      const [hx, hy, hz] = halfExtentsOf(car.kind)
      const height = car.y - hy - this.scene.groundAt(car.x, car.z)
      this.scene.contact.add(car.x, car.z, hx * 1.2, hz * 1.1, car.yaw, Math.max(0, height))
    }
  }

  /**
   * Who parts the grass this frame: feet (the local player's at `self`, when on foot and alive), wheels and
   * tracks on it, and the downwash of anything with rotors low over it.
   */
  bendGrass(self: readonly number[] | null, others: readonly { x: number; z: number }[], cars: ReadonlyMap<number, Placed>) {
    const grass = this.grass
    if (!grass) return
    const benders = this.benders
    benders.length = 0
    if (self) benders.push({ x: self[0]!, z: self[2]!, radius: 0.7, strength: 1.1 })
    for (const other of others) benders.push({ x: other.x, z: other.z, radius: 0.7, strength: 1.1 })
    for (const car of cars.values()) {
      const [hx, hy, hz] = halfExtentsOf(car.kind)
      const height = Math.max(0, car.y - hy - this.scene.groundAt(car.x, car.z))
      const wash = rideView(car.kind).downwash
      if (wash) {
        // Downwash: wider and weaker the higher it hovers, gone past `reach` metres up.
        if (height < wash.reach) benders.push({ x: car.x, z: car.z, radius: wash.radius + height * 0.5, strength: 1.4 * (1 - height / wash.reach) })
      } else if (height < 1) {
        benders.push({ x: car.x, z: car.z, radius: Math.hypot(hx, hz) * 1.05, strength: 1.4 })
      }
    }
    const eye = this.scene.camera.position
    grass.update(eye.x, eye.z, benders)
  }
}
