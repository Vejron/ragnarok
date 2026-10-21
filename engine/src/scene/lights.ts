import { Color } from '../math/vec'

/** How a short-lived point light looks: a flash that lights what is near it and decays. */
export interface LightFlash {
  /** sRGB hex or a linear Color. */
  color: number | Color
  /** Peak strength: the light added to a surface facing it at point-blank range, as a multiple of white. */
  intensity: number
  /** Metres from the light at which it has fallen to nothing. */
  radius: number
  /** Seconds to decay to nothing. */
  life: number
}

interface Live {
  x: number
  y: number
  z: number
  r: number
  g: number
  b: number
  intensity: number
  radius: number
  life: number
  age: number
  /** Held at full strength for one frame (`hold`), not decaying. */
  steady: boolean
}

/** The most lights the renderer lights surfaces with in one frame; the strongest in view win. */
export const MAX_POINT_LIGHTS = 8

/**
 * Short-lived point lights (blasts, muzzle flashes, burning wrecks): `flash(...)` at an event and call
 * `update(dt)` once per frame. A light that stays on (a burning flare) is `hold(...)` every frame
 * instead. They light ordinary lit meshes, without shadows.
 */
export class PointLights {
  readonly live: Live[] = []
  private readonly color = new Color()

  constructor(readonly capacity = 32) {}

  flash(x: number, y: number, z: number, flash: LightFlash) {
    this.add(x, y, z, flash, false)
  }

  /** A light at full strength until the frame after the next `update`: call it every frame to keep it on. `life` is ignored. */
  hold(x: number, y: number, z: number, light: Omit<LightFlash, 'life'>) {
    this.add(x, y, z, { ...light, life: Infinity }, true)
  }

  private add(x: number, y: number, z: number, flash: LightFlash, steady: boolean) {
    const c = typeof flash.color === 'number' ? this.color.setHex(flash.color) : flash.color
    if (this.live.length >= this.capacity) {
      // Full: the oldest goes.
      let oldest = 0
      for (let i = 1; i < this.live.length; i++) if (this.live[i]!.age / this.live[i]!.life > this.live[oldest]!.age / this.live[oldest]!.life) oldest = i
      this.live.splice(oldest, 1)
    }
    this.live.push({ x, y, z, r: c.r, g: c.g, b: c.b, intensity: flash.intensity, radius: flash.radius, life: Math.max(flash.life, 1e-3), age: 0, steady })
  }

  update(dt: number) {
    let n = 0
    for (const l of this.live) {
      l.age += dt
      // A held light lives through the one update after it was held, so the frame drawn next shows it.
      if (l.steady) {
        if (l.life > 0) this.live[n++] = l
        l.life = 0
      } else if (l.age < l.life) this.live[n++] = l
    }
    this.live.length = n
  }

  /** The light's current strength multiplier, 1 at birth falling to 0. */
  static decay(l: { age: number; life: number; steady?: boolean }) {
    if (l.steady) return 1
    const p = Math.min(1, l.age / l.life)
    return (1 - p) * (1 - p)
  }
}
