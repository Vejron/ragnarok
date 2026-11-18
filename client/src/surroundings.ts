/**
 * What people and vehicles do to the ground and water around them, drawn for everyone alike: splashes
 * and wading wakes, the water plane mirrored near the camera, grass parted underfoot and flattened by
 * rotors, and contact shadows.
 */

import { inMarshPool } from '@/render/marshnoise'
import type { Grass, Bender } from '@/render/grass'
import type { Scene } from '@/render/scene'
import { splash } from '@/render/splash'
import type { FxWorld } from '@/render/tyres'
import type { Placed } from '@/entities/presenters/vehicles'
import { rideView } from '@/seats'
import { CAPSULE_HALF_HEIGHT, CAPSULE_RADIUS } from '@/sim/consts'
import { halfExtentsOf } from '@/sim/kinds'
import { groundOf, type Footing } from '@/sim/groundsound'
import { surfaceWetness } from '@/sim/surface'
import { iced, winter } from '@/render/season'
import type { Waters } from '@/sim/waters'
import { NO_WATER } from '@/world'

/** Feet this deep (metres) under the surface are in the water. */
const WADE_DEPTH = 0.15
/** Water this deep over the feet, metres, makes a step a splash. */
const STEP_WADE = 0.08
/** Feet this far above the terrain, metres, stand on a floor: a house's storey, a deck, a roof. */
const STEP_FLOOR = 0.35
/** A body whose centre is this deep (metres) under the surface is a diver, too far down to stir it. */
const DIVE_DEPTH = 1.2
/** Seconds between splashes of someone going through a marsh's pools: about a stride. */
const MARSH_STRIDE = 0.32

/** A point on the ground plane. */
interface At {
  x: number
  y: number
  z: number
}

export class Surroundings {
  waters: Waters | null = null
  grass: Grass | null = null
  /** The fields' wheat, parted and flattened as the grass is. */
  crops: Grass | null = null
  private readonly wading = new Map<number, { wet: boolean; stride: number; x: number; z: number }>()
  private readonly benders: Bender[] = []

  constructor(
    private readonly scene: Scene,
    private readonly fx: () => FxWorld,
  ) {}

  /**
   * Splashes where something of about `size` metres struck, when `at` is at or under the water surface
   * (the server reports the carved bed). Returns whether it did.
   */
  watery(at: At, size: number, speed = 10): boolean {
    // The surface with its waves (`FxWorld.waterAt`).
    const level = this.fx().waterAt?.(at.x, at.z) ?? NO_WATER
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
    // The surface over the camera with its waves, as the water draws it.
    this.scene.setUnderwater(camera.y < (this.fx().waterAt?.(camera.x, camera.z) ?? NO_WATER))
  }

  /**
   * Splashes when person `key`'s feet go into water, and a small wake while they wade; a small splash a stride
   * through a marsh's pools. `speed` is how fast they move, m/s.
   */
  wade(key: number, x: number, y: number, z: number, speed: number, dt: number) {
    const level = this.fx().waterAt?.(x, z) ?? NO_WATER
    const wet = level !== NO_WATER && level - y > WADE_DEPTH
    // A diver well under leaves the surface still.
    const deep = level !== NO_WATER && level - y > DIVE_DEPTH
    const was = this.wading.get(key)
    if (!was) {
      // First seen: already wet is not a splash.
      this.wading.set(key, { wet, stride: 0, x, z })
      return
    }
    // Someone wading pushes a small wake of their own.
    if (wet && !deep && speed > 0.8 && dt > 0) {
      const velocity = [(x - was.x) / dt, (z - was.z) / dt] as const
      this.fx().waves?.hull(`wader ${key}`, { x, z, ahead: [velocity[0], velocity[1]], halfWidth: 0.3, halfLength: 0.3, depth: 0.25, velocity, churn: 0.4 })
    }
    was.x = x
    was.z = z
    if (wet && !was.wet) splash(this.fx(), { x, y: level, z }, 0.45, 2 + Math.min(speed, 8))
    else if (!wet && was.wet && speed > 1) splash(this.fx(), { x, y: level === NO_WATER ? y : level, z }, 0.3, 1)
    if (!wet && speed > 1) {
      const fx = this.fx()
      const wetness = surfaceWetness(fx.surfaceAt(x, z))
      if (wetness > 0 && inMarshPool(x, z, wetness)) {
        was.stride -= dt
        if (was.stride <= 0) {
          splash(fx, { x, y: this.scene.groundAt(x, z), z }, 0.25, 1 + Math.min(speed, 6) * 0.5)
          was.stride = MARSH_STRIDE * (0.85 + Math.random() * 0.3)
        }
      }
    }
    was.wet = wet
  }

  /**
   * What the feet of someone whose capsule is centred at `(x, y, z)` come down on: water over the ankles
   * is a wade, a step well above the terrain is a floor (a house's, a deck's), and otherwise the ground
   * as `groundOf` hears it.
   */
  stepGround(x: number, y: number, z: number): Footing {
    const feet = y - (CAPSULE_HALF_HEIGHT + CAPSULE_RADIUS)
    const fx = this.fx()
    const level = fx.waterAt?.(x, z) ?? NO_WATER
    if (level !== NO_WATER && !iced && level - feet > STEP_WADE) return 'wade'
    if (feet - this.scene.groundAt(x, z) > STEP_FLOOR) return 'floor'
    return groundOf(fx.surfaceAt(x, z), winter, iced)
  }

  /** Person `key` is no longer drawn, or is out of the water's reach (seated, dead). */
  dry(key: number) {
    this.wading.delete(key)
  }

  /** A contact shadow under a soldier whose capsule is centred at `(x, y, z)`. */
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
    if (!this.grass && !this.crops) return
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
    this.grass?.update(eye.x, eye.z, benders)
    this.crops?.update(eye.x, eye.z, benders)
  }
}
