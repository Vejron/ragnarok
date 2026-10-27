import { Ribbons, type RibbonStyle } from '@ragnarok/engine'
import { PROJECTILE_BOMB, PROJECTILE_BULLET, PROJECTILE_CANNON, PROJECTILE_EGG, PROJECTILE_FLARE, PROJECTILE_GRENADE, PROJECTILE_MORTAR, PROJECTILE_ROCKET, PROJECTILE_SHELL } from '@/net/codec'

/** A white-hot core and a pale smoke line behind a rocket: the flame is short, the smoke hangs. */
const ROCKET: RibbonStyle[] = [
  { color: 0xd8d8d2, colorEnd: 0x8a8a86, alpha: 0.5, life: 2.4, width: 0.45, taper: 4, step: 0.6 },
  { color: 0xfff4c8, colorEnd: 0xff5a10, glow: 6, alpha: 1, life: 0.3, width: 0.5, taper: 0.1, blend: 'additive', fog: false, step: 0.35 },
]
/** A tank or mortar shell: a hot streak and a thin, dark wake. */
const SHELL: RibbonStyle[] = [
  { color: 0x6a6a68, colorEnd: 0x9a9a96, alpha: 0.3, life: 1.6, width: 0.5, taper: 2.5, step: 0.8 },
  { color: 0xffd890, colorEnd: 0xff4a10, glow: 4, alpha: 1, life: 0.28, width: 0.55, taper: 0.1, blend: 'additive', fog: false, step: 0.5 },
]
const CANNON: RibbonStyle[] = [
  { color: 0xffe0a0, colorEnd: 0xff5a10, glow: 5, alpha: 1, life: 0.16, width: 0.24, taper: 0.1, blend: 'additive', fog: false, step: 0.4 },
]
const BULLET: RibbonStyle[] = [
  { color: 0xfff6d0, colorEnd: 0xff8a20, glow: 7, alpha: 1, life: 0.1, width: 0.07, taper: 0.2, blend: 'additive', fog: false, step: 0.6 },
]
const BOMB: RibbonStyle[] = [{ color: 0x707070, colorEnd: 0x9a9a9a, alpha: 0.25, life: 1.2, width: 0.15, taper: 3, step: 0.5 }]
const GRENADE: RibbonStyle[] = [{ color: 0x909090, colorEnd: 0xb0b0b0, alpha: 0.22, life: 0.7, width: 0.08, taper: 2, step: 0.4 }]

/** A flare's climb: a hot streak and a pale smoke line that hangs where it went up. */
const FLARE: RibbonStyle[] = [
  { color: 0xe6e2da, colorEnd: 0xa8a6a0, alpha: 0.4, life: 6, width: 0.35, taper: 3, step: 0.8 },
  { color: 0xfff2d0, colorEnd: 0xff7020, glow: 6, alpha: 1, life: 0.35, width: 0.3, taper: 0.1, blend: 'additive', fog: false, step: 0.4 },
]

function stylesFor(kind: number): RibbonStyle[] {
  if (kind === PROJECTILE_FLARE) return FLARE
  if (kind === PROJECTILE_ROCKET) return ROCKET
  if (kind === PROJECTILE_SHELL || kind === PROJECTILE_MORTAR) return SHELL
  if (kind === PROJECTILE_CANNON) return CANNON
  if (kind === PROJECTILE_BULLET) return BULLET
  if (kind === PROJECTILE_BOMB) return BOMB
  // A hen's egg leaves the grenade's faint wake.
  if (kind === PROJECTILE_GRENADE || kind === PROJECTILE_EGG) return GRENADE
  return ROCKET
}

/** Ribbon trails behind the projectiles in flight: exhaust, tracer fire and wakes, fading where they were laid. */
export class ProjectileTrails {
  readonly root = new Ribbons(8192)
  /** The kind each live projectile was last followed as, to end its ribbons with the right keys. */
  private readonly kinds = new Map<string, number>()

  /** Extends projectile `id`'s trail to its current position. `group` keeps rocket, bullet and machine gun round ids apart. */
  follow(group: 'r' | 'b' | 'm', id: number, kind: number, x: number, y: number, z: number) {
    const styles = stylesFor(kind)
    const key = `${group}${id}`
    this.kinds.set(key, kind)
    for (let i = 0; i < styles.length; i++) this.root.extend(`${key}:${i}`, x, y, z, styles[i]!)
  }

  /** The projectile is gone: its trails stay and fade. */
  end(group: 'r' | 'b' | 'm', id: number) {
    const key = `${group}${id}`
    const kind = this.kinds.get(key)
    if (kind === undefined) return
    this.kinds.delete(key)
    for (let i = 0; i < stylesFor(kind).length; i++) this.root.end(`${key}:${i}`)
  }

  update(dt: number) {
    this.root.update(dt)
  }

  dispose() {
    this.root.removeFromParent()
  }
}
