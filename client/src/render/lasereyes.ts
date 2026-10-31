import { TURRET_BEAM, TURRET_CHARGING } from '@/net/codec'
import { TICK_RATE, TUPP_LASER_BURN_TICKS, TUPP_LASER_CHARGE_TICKS, TUPP_LASER_RANGE } from '@/sim/consts'
import type { Effects } from './effects'
import type { TuppModel } from './tupp'

const CHARGE = TUPP_LASER_CHARGE_TICKS / TICK_RATE
const BURN = TUPP_LASER_BURN_TICKS / TICK_RATE
/** How long the eyes go on smoking after a burn, seconds, and how many puffs a second. */
const COOL = 2.5
const COOL_PUFFS = 7

export interface LaserEyesWorld {
  effects: Effects
  sounds: {
    laserCharge(at: [number, number, number], seconds: number): () => void
    laserBeam(at: [number, number, number], seconds: number): () => void
    laserFizzle(at: [number, number, number]): void
  }
  lights: { flash(x: number, y: number, z: number, options: { color: number; intensity: number; radius: number; life: number }): void }
  /** The local player's own beam lit. */
  lit: () => void
}

/**
 * What one tupp's eyes are doing, from its `TURRET_*` flags: light gathering and a rising whine while
 * they charge, the beams and their roar while they burn, and smoke as they cool. Where a beam ends is
 * the newest shot event's (`TuppModel.beamEnd`); between events and before the first it runs along the
 * aim. Drawn every frame, as a held beam has to be.
 */
export class LaserEyes {
  private flags = 0
  private charged = 0
  private cooling = 0
  private puff = 0
  private stop: (() => void) | null = null

  update(tupp: TuppModel, flags: number, mine: boolean, dt: number, world: LaserEyesWorld) {
    const { effects, sounds, lights } = world
    const charging = (flags & TURRET_CHARGING) !== 0
    const burning = (flags & TURRET_BEAM) !== 0
    const { origin, direction } = tupp.barrel()
    const at: [number, number, number] = [origin.x, origin.y, origin.z]
    const eyes = tupp.eyes()

    if (charging && !(this.flags & TURRET_CHARGING)) {
      this.charged = 0
      this.stop = sounds.laserCharge(at, CHARGE)
    }
    if (burning && !(this.flags & TURRET_BEAM)) {
      this.stop?.()
      this.stop = sounds.laserBeam(at, BURN)
      tupp.beamEnd = null
      tupp.scorched = null
      for (const eye of eyes) effects.laserIgnite(eye)
      lights.flash(origin.x, origin.y, origin.z, { color: 0xff3018, intensity: 6, radius: 24, life: 0.3 })
      if (mine) world.lit()
    }
    if (this.flags & TURRET_BEAM && !burning) {
      this.stop?.()
      this.stop = null
      this.cooling = COOL
    }
    if (this.flags & TURRET_CHARGING && !charging && !burning) {
      this.stop?.()
      this.stop = null
      sounds.laserFizzle(at)
    }
    this.flags = flags

    tupp.shudder = burning ? 1 : charging ? (this.charged / CHARGE) ** 2 * 0.4 : 0
    tupp.rearing = burning ? 1 : charging ? Math.min(1, this.charged / CHARGE) * 0.7 : 0
    if (charging) {
      this.charged += dt
      const share = Math.min(1, this.charged / CHARGE)
      for (const eye of eyes) effects.laserCharge(eye, share, dt)
      lights.flash(origin.x, origin.y, origin.z, { color: 0xff2810, intensity: 0.5 + share * 2, radius: 5 + share * 7, life: 0.05 })
    }
    if (burning) {
      const end = tupp.beamEnd ?? origin.clone().addScaledVector(direction, TUPP_LASER_RANGE)
      for (const eye of eyes) effects.laserBeam(eye, end, dt)
      lights.flash(origin.x, origin.y, origin.z, { color: 0xff3018, intensity: 2.5, radius: 14, life: 0.05 })
    }
    if (this.cooling > 0) {
      this.cooling -= dt
      this.puff += dt * COOL_PUFFS * Math.min(1, this.cooling + 0.3)
      for (; this.puff >= 1; this.puff--) for (const eye of eyes) effects.laserSmoke(eye)
    }
  }

  dispose() {
    this.stop?.()
    this.stop = null
  }
}
