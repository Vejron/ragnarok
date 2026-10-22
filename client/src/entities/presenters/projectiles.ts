import type { PointLights } from '@ragnarok/engine'
import type { Blips } from '@/blips'
import type { Particles } from '@/render/particles'
import type { Rockets } from '@/render/rockets'
import type { ProjectileTrails } from '@/render/trails'
import type { FxWorld } from '@/render/tyres'
import { rocketTrail } from '@/render/vehiclesmoke'
import { PROJECTILE_FLARE } from '@/net/codec'
import type { Presenter, Frame } from '../presenter'
import type { Sampled } from '../store'
import type { Entity } from '../units'

/** A burning flare's light: warm white, strong enough to light a field from where it hangs. */
const FLARE_LIGHT = 0xffe8c8
const FLARE_INTENSITY = 2
const FLARE_LIGHT_RADIUS = 120

export interface ProjectileWorld {
  models: Rockets
  trails: ProjectileTrails
  /** Sparks and halos. */
  glow: Particles
  lights: PointLights
  fx: () => FxWorld
  blips: Blips
}

/**
 * Rockets, shells, bombs and flares in flight: the model, its ribbon trail, a motor's flame and smoke,
 * a flare's light. When one is no longer listed it ends where it was last drawn; its burst is a shot
 * event of its own.
 */
export class Projectiles implements Presenter<Frame> {
  /** The unspent fraction of a smoke puff, per rocket in flight. */
  private readonly owed = new Map<number, number>()
  private readonly seen = new Set<number>()

  constructor(private readonly world: ProjectileWorld) {}

  wants(entity: Entity) {
    return entity.projectile !== undefined && entity.place !== undefined && entity.velocity !== undefined
  }

  update(id: number, { entity }: Sampled, frame: Frame) {
    const { x, y, z } = entity.place!
    const { vx, vy, vz } = entity.velocity!
    const kind = entity.projectile!.kind
    // Nobody is named as a rocket's shooter: the map marks it as gunfire from where it was first seen.
    if (!this.seen.has(id)) {
      this.seen.add(id)
      this.world.blips.fired(0, x, z, frame.now)
    }
    this.world.models.place(id, kind, [x, y, z], [vx, vy, vz])
    this.world.trails.follow('r', id, kind, x, y, z)
    if (kind === PROJECTILE_FLARE) this.burnFlare(id, x, y, z, frame)
    // Only a motor-driven rocket (kind 0) leaves a flame and a smoke trail.
    if (kind === 0) this.owed.set(id, rocketTrail(this.world.fx(), [x, y, z], [vx, vy, vz], frame.dt, this.owed.get(id) ?? 0))
  }

  remove(id: number) {
    this.world.models.remove(id)
    this.world.trails.end('r', id)
    this.owed.delete(id)
    this.seen.delete(id)
  }

  /**
   * A burning flare, drawn each frame it is listed: a blooming halo, sparks dripping off it, and a strong,
   * gently flickering light on everything below. The halo lives two frames, so it looks the same at any frame rate.
   */
  private burnFlare(id: number, x: number, y: number, z: number, frame: Frame) {
    const { now, dt } = frame
    const glow = this.world.glow
    const flicker = 0.85 + 0.15 * Math.sin(now * 0.031 + id) * Math.sin(now * 0.0117 + id * 2.3)
    this.world.lights.hold(x, y, z, { color: FLARE_LIGHT, intensity: FLARE_INTENSITY * flicker, radius: FLARE_LIGHT_RADIUS })
    const life = Math.max(dt, 1 / 240) * 2
    glow.spawn({ position: [x, y, z], velocity: [0, 0, 0], life, size: [1.6 * flicker, 1.4], color: 0xfff2dc, alpha: 0.6, glow: 2 })
    glow.spawn({ position: [x, y, z], velocity: [0, 0, 0], life, size: [0.5, 0.45], color: 0xffffff, alpha: 1, glow: 4 })
    if (Math.random() < dt * 10) {
      glow.spawn({
        position: [x, y, z],
        velocity: [(Math.random() - 0.5) * 1.5, -0.5 - Math.random(), (Math.random() - 0.5) * 1.5],
        life: 0.6 + Math.random() * 0.6,
        size: [0.14, 0.04],
        color: 0xffc46a,
        alpha: 1,
        gravity: 6,
        drag: 0.5,
        glow: 3,
      })
    }
  }
}
